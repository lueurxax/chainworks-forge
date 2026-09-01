use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use domain::codex_model_variant_policy::load_pinned_policy_v1;
use domain::tool_policy::{
    DEFAULT_TOOL_OUTPUT_MAX_BYTES, DEFAULT_TOOL_OUTPUT_MAX_LINES, GENERATED_ROOT_DENYLIST,
    TOOL_GUARD_VERSION, TOOL_POLICY_VERSION,
};
use tracing::info;
use uuid::Uuid;

use crate::adapters::{
    chainworks_sccache_dir, chainworks_shared_cargo_target_dir, find_sccache_binary, AcpAdapter,
    AcpLaunchSpec, AcpSessionNewSpec, CleanupPathSpec, LaunchResourceGuard,
};
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_CODEX_ACP_BINARY";
const PINNED_MODEL_VARIANT_POLICY_V1: &[u8] =
    include_bytes!("../../../../../examples/agents/codex-model-variant-matrix.v1.json");

/// Adapter for the OpenAI Codex CLI provider (`codex-acp`).
///
/// - Prepares an isolated runtime home with auth.json + config.toml
/// - Sets CODEX_HOME, HOME, TMPDIR, PATH env vars
/// - Mode: `"full-access"`, no `_meta` block
/// - Model mapped to Codex CLI catalog
pub struct CodexAdapter {
    binary_path: String,
}

impl CodexAdapter {
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR).unwrap_or_else(|_| "codex-acp".to_string());
        Self { binary_path }
    }

    pub fn new_with_binary(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
        }
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcpAdapter for CodexAdapter {
    fn provider_name(&self) -> &str {
        "codex"
    }

    fn prepare_launch_spec(
        &self,
        req: &ExecutionRequest,
        resources: &mut LaunchResourceGuard,
    ) -> Result<AcpLaunchSpec> {
        if self.binary_path.is_empty() {
            bail!(
                "CodexAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure codex-acp is on PATH"
            );
        }

        let runtime_home = prepare_runtime_home(&req.workspace_root)?;

        info!(
            provider = "codex",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            runtime_home = %runtime_home.display(),
            "Spawning Codex ACP subprocess"
        );

        // Build environment matching Swift makeSessionEnvironment
        let mut env = make_session_environment(&runtime_home);
        // Suppress verbose codex_otel/codex_core/rmcp INFO tracing that floods
        // stderr and causes memory pressure. codex-acp reads RUST_LOG via
        // EnvFilter::from_default_env(). Only show warnings and errors.
        env.push(("RUST_LOG".into(), "warn".into()));

        resources.add_cleanup_spec(CleanupPathSpec::stage_codex_session_store(
            runtime_home.clone(),
        ));
        Ok(AcpLaunchSpec::new(&self.binary_path)
            .with_envs(env)
            .with_provider_runtime_home(runtime_home))
    }

    fn prepare_session_new_spec(&self, req: &ExecutionRequest) -> Result<AcpSessionNewSpec> {
        // Build the base model id (without /effort suffix) and extract the
        // effort separately. Codex's session/new silently falls back to
        // medium when given a combined "model/effort" string, so we pass a
        // bare model and set reasoning_effort via session/set_config_option.
        let raw_model = req.model.as_deref().unwrap_or("gpt-5.6");
        let exact_effort = match (req.model.as_deref(), req.effort.as_deref()) {
            (Some(model), Some(effort)) if !model.contains('/') => {
                let policy = load_pinned_policy_v1(PINNED_MODEL_VARIANT_POLICY_V1)
                    .context("CodexAdapter: loading pinned model variant policy")?;
                policy.variant(model).and_then(|variant| {
                    variant
                        .allowed_efforts
                        .iter()
                        .any(|allowed| allowed == effort)
                        .then(|| effort.to_string())
                })
            }
            _ => None,
        };
        let (base_model, effort_from_model) = split_codex_model_effort(raw_model);
        let effort = req
            .effort
            .as_deref()
            .map(|e| e.to_lowercase())
            .or(effort_from_model);

        let mut config_options: Vec<(String, String)> = Vec::new();
        let mut exact_best_effort_config_options = Vec::new();
        if let Some(e) = exact_effort {
            exact_best_effort_config_options.push(("reasoning_effort".into(), e));
        } else if let Some(e) = effort.as_deref() {
            // Codex accepts low / medium / high / xhigh for reasoning_effort.
            config_options.push(("reasoning_effort".into(), e.to_string()));
        }

        let mut spec = AcpSessionNewSpec::new(base_model, "full-access");
        spec.exact_best_effort_config_options = exact_best_effort_config_options;
        spec.config_options = config_options;
        Ok(spec)
    }
}

// ---------------------------------------------------------------------------
// Runtime home preparation
// ---------------------------------------------------------------------------

/// Source Codex home: `$CODEX_HOME` or `~/.codex`
fn source_codex_home() -> PathBuf {
    if let Ok(explicit) = std::env::var("CODEX_HOME") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".codex")
}

/// Create an isolated runtime home directory with auth.json and config.toml
/// copied from the source Codex home.
fn prepare_runtime_home(workspace_root: &str) -> Result<PathBuf> {
    let runtime_home = if workspace_root.is_empty() {
        std::env::temp_dir()
            .join("forge-codex-acp")
            .join(Uuid::new_v4().to_string())
    } else {
        Path::new(workspace_root)
            .join(".forge-codex-acp")
            .join(Uuid::new_v4().to_string())
    };

    std::fs::create_dir_all(&runtime_home)
        .with_context(|| format!("create runtime home: {}", runtime_home.display()))?;

    let source_home = source_codex_home();

    // Copy auth.json (credentials)
    let source_auth = source_home.join("auth.json");
    let runtime_auth = runtime_home.join("auth.json");
    if source_auth.exists() {
        std::fs::copy(&source_auth, &runtime_auth)
            .with_context(|| "copy auth.json to runtime home")?;
    } else {
        info!(
            "Codex: auth.json not found at {}; starting without copied auth",
            source_auth.display()
        );
    }

    // Copy config.toml (sanitized)
    let source_config = source_home.join("config.toml");
    let runtime_config = runtime_home.join("config.toml");
    if source_config.exists() {
        let data = std::fs::read_to_string(&source_config).with_context(|| "read config.toml")?;
        let sanitized = sanitize_runtime_config(&data);
        std::fs::write(&runtime_config, sanitized)
            .with_context(|| "write sanitized config.toml")?;
    }

    // Create required subdirectories
    for subdir in &["bin", "tmp", ".cache/clang/ModuleCache"] {
        std::fs::create_dir_all(runtime_home.join(subdir)).ok();
    }
    install_safe_search_wrappers(&runtime_home)?;

    Ok(runtime_home)
}

fn install_safe_search_wrappers(runtime_home: &Path) -> Result<()> {
    let bin = runtime_home.join("bin");
    std::fs::create_dir_all(&bin).with_context(|| format!("create bin dir: {}", bin.display()))?;
    for tool in ["rg", "find"] {
        let wrapper_path = bin.join(tool);
        std::fs::write(&wrapper_path, safe_search_wrapper_script(tool)).with_context(|| {
            format!(
                "write safe search wrapper for {tool}: {}",
                wrapper_path.display()
            )
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&wrapper_path)?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&wrapper_path, permissions)?;
        }
    }
    Ok(())
}

fn safe_search_wrapper_script(tool: &str) -> String {
    let denylist = GENERATED_ROOT_DENYLIST.join(", ").replace('"', "\\\"");
    format!(
        r#"#!/bin/bash
set -o pipefail
TOOL="{tool}"
POLICY_VERSION="{policy_version}"
GUARD_VERSION="{guard_version}"
MAX_LINES={max_lines}
MAX_BYTES={max_bytes}
DENYLIST="{denylist}"

error_text() {{
  cat >&2 <<EOF
tool_output_budget_preflight_denied:
Broad repository search must use bounded search and exclude generated/build roots.
Use a narrower query, for example:
  rg prompt_stream_failed control-plane/crates/acp/src
For repo-root search, include every generated-root exclude from runtime.health.toolOutputGuard.generatedRootDenylist and cap output.
Excluded roots include control-plane/target/**, **/target/**, **/.build/**, DerivedData, node_modules.
policy_version=${{POLICY_VERSION}} guard_version=${{GUARD_VERSION}}
EOF
}}

find_real_tool() {{
  local self_dir candidate IFS=:
  self_dir="$(cd "$(dirname "$0")" && pwd)"
  for dir in $PATH; do
    [ -z "$dir" ] && dir="."
    candidate="$dir/$TOOL"
    if [ -x "$candidate" ] && [ "$(cd "$dir" 2>/dev/null && pwd)" != "$self_dir" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}}

has_all_generated_excludes() {{
  local joined
  joined="$(printf ' %s ' "$@" | tr '[:upper:]' '[:lower:]')"
  case "$joined" in
    *--glob*|*" -g "*|*"-g!"*|*" -path "*|*" -prune "*|*--exclude*) ;;
    *) return 1 ;;
  esac
  case "$joined" in *control-plane/target*) ;; *) return 1 ;; esac
  case "$joined" in *"**/target"*) ;; *) return 1 ;; esac
  case "$joined" in *.build*) ;; *) return 1 ;; esac
  case "$joined" in *deriveddata*) ;; *) return 1 ;; esac
  case "$joined" in *node_modules*) ;; *) return 1 ;; esac
  case "$joined" in *.git*) ;; *) return 1 ;; esac
  case "$joined" in *.swiftpm*) ;; *) return 1 ;; esac
  case "$joined" in *.forge-codex-acp*) ;; *) return 1 ;; esac
  case "$joined" in *.junie*) ;; *) return 1 ;; esac
  case "$joined" in *.claude*) ;; *) return 1 ;; esac
  case "$joined" in *.codex*) ;; *) return 1 ;; esac
  case "$joined" in *.xcresult*) ;; *) return 1 ;; esac
  case "$joined" in *.dsym*) ;; *) return 1 ;; esac
  case "$joined" in *"**/build"*) ;; *) return 1 ;; esac
  case "$joined" in *"**/dist"*) ;; *) return 1 ;; esac
  return 0
}}

is_repo_root_cwd() {{
  [ -d .git ] || [ -d control-plane ] || [ -f "Chainworks Forge.xcodeproj/project.pbxproj" ]
}}

rg_is_broad() {{
  local saw_pattern=0 skip_next=0 root_count=0 top_count=0 arg root
  for arg in "$@"; do
    case "$arg" in --files|--type-list) saw_pattern=1 ;; esac
  done
  for arg in "$@"; do
    if [ "$skip_next" -eq 1 ]; then skip_next=0; continue; fi
    case "$arg" in
      -g|--glob|--iglob|--type|-t|--type-not|-T|-e|--regexp|-f|--file|--sort|--max-count|-m|--max-filesize) skip_next=1; continue ;;
      --glob=*|--iglob=*|--type=*|--type-not=*|--regexp=*|--file=*|--max-count=*|--max-filesize=*) continue ;;
      -*) continue ;;
    esac
    if [ "$saw_pattern" -eq 0 ]; then saw_pattern=1; continue; fi
    root="${{arg#./}}"; root="${{root%/}}"
    root_count=$((root_count + 1))
    case "$root" in
      ""|.|'$PWD'|'${{PWD}}') return 0 ;;
      control-plane|docs|scripts|examples|"Chainworks Forge"|"Chainworks ForgeTests"|"Chainworks ForgeUITests") top_count=$((top_count + 1)) ;;
    esac
  done
  [ "$root_count" -eq 0 ] && is_repo_root_cwd && return 0
  [ "$top_count" -ge 2 ] && return 0
  return 1
}}

find_is_broad() {{
  local root_count=0 arg root
  for arg in "$@"; do
    case "$arg" in
      --) continue ;;
      -*|'('|')'|'!') break ;;
    esac
    root="${{arg#./}}"; root="${{root%/}}"
    root_count=$((root_count + 1))
    case "$root" in
      ""|.|'$PWD'|'${{PWD}}'|control-plane) return 0 ;;
    esac
  done
  [ "$root_count" -eq 0 ] && is_repo_root_cwd && return 0
  return 1
}}

if ! has_all_generated_excludes "$@"; then
  if [ "$TOOL" = "rg" ] && rg_is_broad "$@"; then error_text; exit 96; fi
  if [ "$TOOL" = "find" ] && find_is_broad "$@"; then error_text; exit 96; fi
fi

REAL_TOOL="$(find_real_tool)" || {{ echo "tool_output_budget_preflight_denied: unable to locate real $TOOL outside Chainworks wrapper" >&2; exit 96; }}

"$REAL_TOOL" "$@" 2>&1 \
  | awk -v max="$MAX_LINES" 'NR <= max {{ print; next }} NR == max + 1 {{ print "tool_output_budget_exceeded: output truncated by Chainworks safe-search guard" }}' \
  | head -c "$MAX_BYTES"
status=${{PIPESTATUS[0]}}
exit "$status"
"#,
        tool = tool,
        policy_version = TOOL_POLICY_VERSION,
        guard_version = TOOL_GUARD_VERSION,
        max_lines = DEFAULT_TOOL_OUTPUT_MAX_LINES,
        max_bytes = DEFAULT_TOOL_OUTPUT_MAX_BYTES,
        denylist = denylist
    )
}

/// Sanitize config.toml for the isolated runtime.
/// - Strip sandbox settings
/// - Strip model/effort overrides so session/new model takes priority
/// - Strip inherited MCP/plugin/marketplace/skill/project configuration. ACP
///   runtime tool access must come from the workflow/engine contract, not the
///   operator's personal Codex config.
fn sanitize_runtime_config(source: &str) -> String {
    let mut current_block_dropped = false;
    let mut kept = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        let lowered = trimmed.to_lowercase();

        if lowered.starts_with('[') && lowered.ends_with(']') {
            let table = lowered.trim_matches(&['[', ']'][..]).trim();
            let root = table
                .split(['.', '"'])
                .find(|part| !part.is_empty())
                .unwrap_or(table);
            current_block_dropped = table == "notice.model_migrations"
                || matches!(
                    root,
                    "mcp_servers" | "plugins" | "marketplaces" | "skills" | "projects" | "features"
                );
            if current_block_dropped {
                continue;
            }
        }

        if current_block_dropped {
            continue;
        }

        if lowered.starts_with("sandbox")
            || lowered.starts_with("disable_sandbox")
            || lowered.starts_with("model")
            || lowered.starts_with("model_reasoning_effort")
            || lowered.starts_with("personality")
            || lowered.starts_with("hide_rate_limit")
        {
            continue;
        }

        kept.push(line);
    }

    let sanitized_body = kept.join("\n");
    // Force-write sandbox settings for runtime commands because Codex ACP no longer
    // treats the legacy disable_sandbox flag as sufficient on its own. The engine
    // owns the trust boundary for provider runs, and writable output/temp/toolchain
    // paths are required for proposal writers and implementation agents.
    let mut sanitized =
        String::from("sandbox_mode = \"danger-full-access\"\ndisable_sandbox = true\n");
    if !sanitized_body.is_empty() {
        sanitized.push_str(&sanitized_body);
    }

    sanitized
}

fn configured_toolchain_home() -> PathBuf {
    std::env::var("CHAINWORKS_TOOLCHAIN_HOME")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("TOOLCHAIN_HOME")
                .ok()
                .filter(|value| !value.is_empty())
        })
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("chainworks-forge-toolchains"))
}

/// Build environment variables matching Swift `makeSessionEnvironment`.
fn make_session_environment(runtime_home: &Path) -> Vec<(String, String)> {
    let home = runtime_home.to_string_lossy().to_string();
    let bin = runtime_home.join("bin").to_string_lossy().to_string();
    let cache = runtime_home.join(".cache").to_string_lossy().to_string();
    let session_name = runtime_home
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("session");
    let toolchain_root = configured_toolchain_home()
        .join("providers")
        .join("codex")
        .join(session_name);
    let tmp = toolchain_root.join("tmp");
    let shared_rust_root = configured_toolchain_home()
        .join("providers")
        .join("codex")
        .join("shared")
        .join("rust");
    let rustup_home = shared_rust_root.join("rustup");
    let cargo_home = shared_rust_root.join("cargo");
    let cargo_target_dir = PathBuf::from(chainworks_shared_cargo_target_dir());
    let sccache_dir = PathBuf::from(chainworks_sccache_dir());

    // Keep toolchain caches outside CODEX_HOME: provider tool calls may run
    // under a sandbox where the isolated runtime home is readable but not a
    // writable root. The top-level contract is language-neutral; concrete
    // toolchain variables below are adapters for tools that require them.
    for dir in [
        &toolchain_root,
        &tmp,
        &rustup_home,
        &cargo_home,
        &cargo_target_dir,
        &sccache_dir,
    ] {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("create {} directory", dir.display()))
            .unwrap_or_else(|error| {
                tracing::warn!(
                    dir = %dir.display(),
                    error = %error,
                    "Codex session environment: failed to create toolchain cache dir"
                );
            });
    }

    let path = format!("{}:{}", bin, super::default_provider_path_value());
    let toolchain_home = toolchain_root.to_string_lossy().to_string();
    let tmp = tmp.to_string_lossy().to_string();
    let rustup_home = rustup_home.to_string_lossy().to_string();
    let cargo_home = cargo_home.to_string_lossy().to_string();
    let cargo_target_dir = cargo_target_dir.to_string_lossy().to_string();
    let sccache_dir = sccache_dir.to_string_lossy().to_string();

    let mut env = vec![
        ("CODEX_HOME".into(), home.clone()),
        ("HOME".into(), home),
        ("CHAINWORKS_TOOLCHAIN_HOME".into(), toolchain_home.clone()),
        ("TOOLCHAIN_HOME".into(), toolchain_home),
        ("TMPDIR".into(), tmp.clone()),
        ("TMP".into(), tmp.clone()),
        ("TEMP".into(), tmp),
        ("PATH".into(), path),
        ("XDG_CACHE_HOME".into(), cache),
        ("RUSTUP_HOME".into(), rustup_home),
        ("CARGO_HOME".into(), cargo_home),
        ("CARGO_TARGET_DIR".into(), cargo_target_dir),
        ("SCCACHE_DIR".into(), sccache_dir),
        ("SCCACHE_CACHE_SIZE".into(), "20G".into()),
    ];
    if let Some(sccache) = find_sccache_binary() {
        env.push(("RUSTC_WRAPPER".into(), sccache));
    }
    env
}

/// Split a raw Codex model spec into (base_model, effort).
///
/// Accepts both `"gpt-5.4"` (base only) and `"gpt-5.4/high"` (combined).
/// For the combined form we lowercase both halves. The combined form arrives
/// when callers still use the legacy `model/effort` encoding; we unpack it so
/// the transport can set `reasoning_effort` via session config option.
fn split_codex_model_effort(model: &str) -> (String, Option<String>) {
    let lowered = model.to_lowercase();
    match lowered.split_once('/') {
        Some((base, eff)) if !eff.is_empty() => (base.to_string(), Some(eff.to_string())),
        _ => (lowered, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request_with_model_and_effort(
        model: Option<&str>,
        effort: Option<&str>,
    ) -> ExecutionRequest {
        ExecutionRequest {
            agent_execution_id: None,
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "stage_codex".to_string(),
            attempt_number: 1,
            agent_id: "agent_codex".to_string(),
            provider: "codex".to_string(),
            model: model.map(ToOwned::to_owned),
            effort: effort.map(ToOwned::to_owned),
            workspace_root: String::new(),
            prompt: "prompt".to_string(),
            worktree_root: None,
            worktree_write_enabled: false,
            worktree_strategy: None,
            expected_output_paths: Vec::new(),
            expected_outputs: Vec::new(),
            keep_session_alive: false,
            reuse_existing_session: false,
            session_generation_id: None,
            provider_session_id: None,
            provider_runtime_home: None,
            p079_repair_canonical_paths: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: None,
            legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            owner_kind: "stage_execution".to_string(),
            owner_id: None,
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: None,
            toolchain_go_scope_enabled: false,
        }
    }

    #[test]
    fn splits_bare_model() {
        assert_eq!(
            split_codex_model_effort("gpt-5.4"),
            ("gpt-5.4".into(), None)
        );
    }

    #[test]
    fn splits_combined_model() {
        assert_eq!(
            split_codex_model_effort("gpt-5.4/high"),
            ("gpt-5.4".into(), Some("high".into()))
        );
    }

    #[test]
    fn lowercases_both_halves() {
        assert_eq!(
            split_codex_model_effort("GPT-5.4/Xhigh"),
            ("gpt-5.4".into(), Some("xhigh".into()))
        );
    }

    #[test]
    fn handles_trailing_slash() {
        assert_eq!(
            split_codex_model_effort("gpt-5.4/"),
            ("gpt-5.4/".into(), None)
        );
    }

    #[test]
    fn codex_effort_inputs_enter_exactly_one_lane() {
        struct Case {
            model: Option<&'static str>,
            effort: Option<&'static str>,
            expected_model: &'static str,
            expected_exact: Option<&'static str>,
            expected_fuzzy: Option<&'static str>,
        }

        let cases = [
            Case {
                model: Some("gpt-5.6-sol"),
                effort: Some("max"),
                expected_model: "gpt-5.6-sol",
                expected_exact: Some("max"),
                expected_fuzzy: None,
            },
            Case {
                model: Some("gpt-5.6-terra"),
                effort: Some("xhigh"),
                expected_model: "gpt-5.6-terra",
                expected_exact: Some("xhigh"),
                expected_fuzzy: None,
            },
            Case {
                model: Some("gpt-5.6-luna"),
                effort: Some("ultra"),
                expected_model: "gpt-5.6-luna",
                expected_exact: None,
                expected_fuzzy: Some("ultra"),
            },
            Case {
                model: Some("gpt-5.6-sol"),
                effort: Some("HIGH"),
                expected_model: "gpt-5.6-sol",
                expected_exact: None,
                expected_fuzzy: Some("high"),
            },
            Case {
                model: Some("gpt-5.6-sol"),
                effort: Some(""),
                expected_model: "gpt-5.6-sol",
                expected_exact: None,
                expected_fuzzy: Some(""),
            },
            Case {
                model: Some("gpt-5.6"),
                effort: Some("high"),
                expected_model: "gpt-5.6",
                expected_exact: None,
                expected_fuzzy: Some("high"),
            },
            Case {
                model: Some("custom-model"),
                effort: Some("high"),
                expected_model: "custom-model",
                expected_exact: None,
                expected_fuzzy: Some("high"),
            },
            Case {
                model: Some("gpt-5.6-sol/max"),
                effort: None,
                expected_model: "gpt-5.6-sol",
                expected_exact: None,
                expected_fuzzy: Some("max"),
            },
            Case {
                model: Some("gpt-5.6-sol/max"),
                effort: Some("ultra"),
                expected_model: "gpt-5.6-sol",
                expected_exact: None,
                expected_fuzzy: Some("ultra"),
            },
            Case {
                model: Some("GPT-5.6-SOL"),
                effort: Some("max"),
                expected_model: "gpt-5.6-sol",
                expected_exact: None,
                expected_fuzzy: Some("max"),
            },
            Case {
                model: Some("gpt-5.6-sol"),
                effort: None,
                expected_model: "gpt-5.6-sol",
                expected_exact: None,
                expected_fuzzy: None,
            },
            Case {
                model: Some("gpt-5.6-sol/"),
                effort: None,
                expected_model: "gpt-5.6-sol/",
                expected_exact: None,
                expected_fuzzy: None,
            },
            Case {
                model: None,
                effort: None,
                expected_model: "gpt-5.6",
                expected_exact: None,
                expected_fuzzy: None,
            },
        ];

        let adapter = CodexAdapter::new_with_binary("/bin/codex-acp");
        for case in cases {
            let request = request_with_model_and_effort(case.model, case.effort);
            let spec = adapter.prepare_session_new_spec(&request).unwrap();
            let expected_exact = case
                .expected_exact
                .map(|value| vec![("reasoning_effort".to_string(), value.to_string())])
                .unwrap_or_default();
            let expected_fuzzy = case
                .expected_fuzzy
                .map(|value| vec![("reasoning_effort".to_string(), value.to_string())])
                .unwrap_or_default();

            assert_eq!(spec.model, case.expected_model, "model={:?}", case.model);
            assert_eq!(
                spec.exact_best_effort_config_options, expected_exact,
                "model={:?} effort={:?}",
                case.model, case.effort
            );
            assert_eq!(
                spec.config_options, expected_fuzzy,
                "model={:?} effort={:?}",
                case.model, case.effort
            );
            assert!(
                spec.exact_best_effort_config_options.is_empty() || spec.config_options.is_empty(),
                "effort must never enter both lanes"
            );
            assert!(spec.required_config_options.is_empty());
        }
    }

    #[test]
    fn launch_spec_records_exact_runtime_home_for_transport_monitor() {
        let tmp = tempfile::tempdir().unwrap();
        let req = ExecutionRequest {
            agent_execution_id: None,
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "stage_codex".to_string(),
            attempt_number: 1,
            agent_id: "agent_codex".to_string(),
            provider: "codex".to_string(),
            model: None,
            effort: None,
            workspace_root: tmp.path().to_string_lossy().into_owned(),
            prompt: "prompt".to_string(),
            worktree_root: None,
            worktree_write_enabled: false,
            worktree_strategy: None,
            expected_output_paths: Vec::new(),
            expected_outputs: Vec::new(),
            keep_session_alive: false,
            reuse_existing_session: false,
            session_generation_id: None,
            provider_session_id: None,
            provider_runtime_home: None,
            p079_repair_canonical_paths: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: None,
            legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            owner_kind: "stage_execution".to_string(),
            owner_id: None,
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: None,
            toolchain_go_scope_enabled: false,
        };
        let adapter = CodexAdapter::new_with_binary("/bin/codex-acp");
        let mut resources = LaunchResourceGuard::default();

        let spec = adapter
            .prepare_launch_spec(&req, &mut resources)
            .expect("launch spec");
        let runtime_home = spec
            .provider_runtime_home
            .as_ref()
            .expect("runtime home must be published");

        assert!(runtime_home.starts_with(tmp.path().join(".forge-codex-acp")));
        assert!(runtime_home.is_dir());
        let runtime_home_value = runtime_home.to_string_lossy();
        assert!(spec
            .env
            .iter()
            .any(|(name, value)| name == "CODEX_HOME" && value == runtime_home_value.as_ref()));
    }

    #[test]
    fn sanitize_runtime_config_strips_operator_tooling() {
        let source = r#"
model = "gpt-5.4"
model_reasoning_effort = "high"
sandbox_mode = "read-only"
disable_sandbox = false
personality = "pragmatic"

[projects."/Users/user/Documents/Chainworks Forge"]
trust_level = "trusted"

[notice]
hide_full_access_warning = true

[notice.model_migrations]
"gpt-5.2" = "gpt-5.2-codex"

[mcp_servers.xcode]
command = "xcrun"
args = ["mcpbridge"]
enabled = true

[mcp_servers.postgres.env]
DATABASE_URL = "postgres://secret"

[mcp_servers.chainworks-control-plane]
enabled = true
url = "http://127.0.0.1:4000/mcp"

[mcp_servers.chainworks-control-plane.http_headers]
Authorization = "Bearer secret"

[plugins."build-ios-apps@openai-curated"]
enabled = true

[marketplaces.openai-primary-runtime]
source = "/Users/user/.cache/codex-runtimes/codex-primary-runtime/plugins/openai-primary-runtime"

[skills.proposal_review]
path = "/Users/user/.codex/skills/proposal-review-triad"

[features]
multi_agent = true
"#;

        let sanitized = sanitize_runtime_config(source);

        assert!(!sanitized.contains("gpt-5.4"));
        assert!(!sanitized.contains("[projects."));
        assert!(!sanitized.contains("[mcp_servers."));
        assert!(!sanitized.contains("[plugins."));
        assert!(!sanitized.contains("[marketplaces."));
        assert!(!sanitized.contains("[skills."));
        assert!(!sanitized.contains("[features]"));
        assert!(!sanitized.contains("mcpbridge"));
        assert!(!sanitized.contains("DATABASE_URL"));
        assert!(!sanitized.contains("Bearer secret"));
        assert!(!sanitized.contains("codex-primary-runtime"));
        assert!(!sanitized.contains("multi_agent"));
        assert!(!sanitized.contains("sandbox_mode = \"read-only\""));
        assert!(!sanitized.contains("disable_sandbox = false"));
        assert!(sanitized.contains("sandbox_mode = \"danger-full-access\""));
        assert!(sanitized.contains("disable_sandbox = true"));
        let first_table = sanitized.find('[').expect("kept notice table");
        assert!(sanitized.find("sandbox_mode").expect("sandbox mode") < first_table);
        assert!(sanitized.find("disable_sandbox").expect("sandbox flag") < first_table);
        assert!(!sanitized.contains("model_migrations"));
        assert!(sanitized.contains("[notice]"));
        assert!(sanitized.contains("hide_full_access_warning"));
    }

    #[test]
    fn runtime_home_installs_safe_search_wrappers() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime_home = tmp.path().join(".forge-codex-acp").join("session-guard");
        std::fs::create_dir_all(&runtime_home).unwrap();

        install_safe_search_wrappers(&runtime_home).unwrap();

        for tool in ["rg", "find"] {
            let wrapper = runtime_home.join("bin").join(tool);
            let script = std::fs::read_to_string(&wrapper).unwrap();
            assert!(script.contains("tool_output_budget_preflight_denied"));
            assert!(script.contains(TOOL_POLICY_VERSION));
            assert!(script.contains(TOOL_GUARD_VERSION));
            assert!(script.contains("control-plane/target/**"));
            assert!(script.contains("head -c"));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                assert_eq!(
                    std::fs::metadata(&wrapper).unwrap().permissions().mode() & 0o111,
                    0o111
                );
            }
        }
    }

    #[test]
    fn safe_search_wrapper_script_denies_broad_root_searches() {
        let script = safe_search_wrapper_script("rg");

        assert!(script.contains("rg_is_broad"));
        assert!(script.contains("find_is_broad"));
        assert!(script.contains("MAX_LINES"));
        assert!(script.contains("MAX_BYTES"));
    }

    #[cfg(unix)]
    fn write_fake_tool(dir: &std::path::Path, tool: &str, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;

        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(tool);
        std::fs::write(&path, format!("#!/bin/bash\n{body}\n")).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }

    #[cfg(unix)]
    fn run_wrapped_tool(
        runtime_home: &std::path::Path,
        real_bin: &std::path::Path,
        cwd: &std::path::Path,
        tool: &str,
        args: &[String],
    ) -> std::process::Output {
        let path = format!(
            "{}:{}:/usr/bin:/bin",
            runtime_home.join("bin").display(),
            real_bin.display()
        );
        std::process::Command::new(runtime_home.join("bin").join(tool))
            .args(args)
            .current_dir(cwd)
            .env("PATH", path)
            .output()
            .unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn safe_search_wrapper_matches_domain_matrix_for_broad_and_narrow_rg() {
        use domain::tool_policy::{preflight_shell_command, ToolPreflightDecision};

        let tmp = tempfile::tempdir().unwrap();
        let runtime_home = tmp.path().join(".forge-codex-acp").join("session-guard");
        let repo = tmp.path().join("repo");
        let real_bin = tmp.path().join("real-bin");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(&runtime_home).unwrap();
        install_safe_search_wrappers(&runtime_home).unwrap();
        write_fake_tool(&real_bin, "rg", "printf 'real-rg\\n'");

        let partial_args = vec![
            "--glob".to_string(),
            "!control-plane/target/**".to_string(),
            "foo".to_string(),
            ".".to_string(),
        ];
        let partial = run_wrapped_tool(&runtime_home, &real_bin, &repo, "rg", &partial_args);
        assert_eq!(partial.status.code(), Some(96));
        let partial_stderr = String::from_utf8_lossy(&partial.stderr);
        assert!(partial_stderr.contains("tool_output_budget_preflight_denied"));
        assert!(partial_stderr.contains("rg prompt_stream_failed control-plane/crates/acp/src"));
        assert!(matches!(
            preflight_shell_command("rg --glob '!control-plane/target/**' foo ."),
            ToolPreflightDecision::Deny(_)
        ));

        let mut full_args = Vec::new();
        for root in GENERATED_ROOT_DENYLIST {
            full_args.push("--glob".to_string());
            full_args.push(format!("!{root}"));
        }
        full_args.extend(["foo".to_string(), ".".to_string()]);
        let full = run_wrapped_tool(&runtime_home, &real_bin, &repo, "rg", &full_args);
        assert_eq!(full.status.code(), Some(0));
        assert_eq!(String::from_utf8_lossy(&full.stdout).trim(), "real-rg");
        assert!(matches!(
            preflight_shell_command(&format!(
                "rg {} foo .",
                GENERATED_ROOT_DENYLIST
                    .iter()
                    .map(|root| format!("--glob '!{root}'"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )),
            ToolPreflightDecision::Allow
        ));

        let narrow_args = vec![
            "foo".to_string(),
            "control-plane/crates/acp/src".to_string(),
        ];
        let narrow = run_wrapped_tool(&runtime_home, &real_bin, &repo, "rg", &narrow_args);
        assert_eq!(narrow.status.code(), Some(0));
        assert_eq!(String::from_utf8_lossy(&narrow.stdout).trim(), "real-rg");
    }

    #[cfg(unix)]
    #[test]
    fn safe_search_wrapper_matches_domain_matrix_for_broad_and_narrow_find() {
        use domain::tool_policy::{preflight_shell_command, ToolPreflightDecision};

        let tmp = tempfile::tempdir().unwrap();
        let runtime_home = tmp.path().join(".forge-codex-acp").join("session-guard");
        let repo = tmp.path().join("repo");
        let real_bin = tmp.path().join("real-bin");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(&runtime_home).unwrap();
        install_safe_search_wrappers(&runtime_home).unwrap();
        write_fake_tool(&real_bin, "find", "printf 'real-find\\n'");

        let partial_args = vec![
            ".".to_string(),
            "-path".to_string(),
            "./control-plane/target".to_string(),
            "-prune".to_string(),
            "-o".to_string(),
            "-type".to_string(),
            "f".to_string(),
            "-print".to_string(),
        ];
        let partial = run_wrapped_tool(&runtime_home, &real_bin, &repo, "find", &partial_args);
        assert_eq!(partial.status.code(), Some(96));
        assert!(String::from_utf8_lossy(&partial.stderr)
            .contains("tool_output_budget_preflight_denied"));
        assert!(matches!(
            preflight_shell_command(
                "find . -path './control-plane/target' -prune -o -type f -print"
            ),
            ToolPreflightDecision::Deny(_)
        ));

        let mut full_args = vec![".".to_string()];
        for root in GENERATED_ROOT_DENYLIST {
            full_args.extend([
                "-path".to_string(),
                format!("./{}", root.trim_end_matches("/**")),
                "-prune".to_string(),
                "-o".to_string(),
            ]);
        }
        full_args.extend(["-type".to_string(), "f".to_string(), "-print".to_string()]);
        let full = run_wrapped_tool(&runtime_home, &real_bin, &repo, "find", &full_args);
        assert_eq!(full.status.code(), Some(0));
        assert_eq!(String::from_utf8_lossy(&full.stdout).trim(), "real-find");

        let narrow_args = vec![
            "control-plane/crates/acp/src".to_string(),
            "-type".to_string(),
            "f".to_string(),
        ];
        let narrow = run_wrapped_tool(&runtime_home, &real_bin, &repo, "find", &narrow_args);
        assert_eq!(narrow.status.code(), Some(0));
        assert_eq!(String::from_utf8_lossy(&narrow.stdout).trim(), "real-find");
    }

    #[cfg(unix)]
    #[test]
    fn safe_search_wrapper_caps_lines_and_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime_home = tmp.path().join(".forge-codex-acp").join("session-guard");
        let repo = tmp.path().join("repo");
        let real_bin = tmp.path().join("real-bin");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(&runtime_home).unwrap();
        install_safe_search_wrappers(&runtime_home).unwrap();

        write_fake_tool(
            &real_bin,
            "rg",
            "for i in $(seq 1 2100); do printf 'line-%04d\\n' \"$i\"; done",
        );
        let lines = run_wrapped_tool(
            &runtime_home,
            &real_bin,
            &repo,
            "rg",
            &[
                "foo".to_string(),
                "control-plane/crates/acp/src".to_string(),
            ],
        );
        assert_eq!(lines.status.code(), Some(0));
        let lines_stdout = String::from_utf8_lossy(&lines.stdout);
        assert!(lines_stdout.contains("line-2000"));
        assert!(lines_stdout.contains("tool_output_budget_exceeded: output truncated"));
        assert!(!lines_stdout.contains("line-2001"));

        write_fake_tool(
            &real_bin,
            "find",
            &format!(
                "python3 - <<'PY'\nimport sys\nsys.stdout.write('x' * {})\nPY",
                DEFAULT_TOOL_OUTPUT_MAX_BYTES + 1024
            ),
        );
        let bytes = run_wrapped_tool(
            &runtime_home,
            &real_bin,
            &repo,
            "find",
            &[
                "control-plane/crates/acp/src".to_string(),
                "-type".to_string(),
                "f".to_string(),
            ],
        );
        assert!(
            bytes.stdout.len() as u64 <= DEFAULT_TOOL_OUTPUT_MAX_BYTES,
            "wrapper must cap stdout at DEFAULT_TOOL_OUTPUT_MAX_BYTES"
        );
    }

    #[test]
    fn session_environment_keeps_toolchain_cache_outside_runtime_home() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime_home = tmp.path().join(".forge-codex-acp").join("session-1");
        std::fs::create_dir_all(&runtime_home).unwrap();

        let env = make_session_environment(&runtime_home);
        let value = |key: &str| {
            env.iter()
                .find_map(|(name, value)| (name == key).then_some(value.as_str()))
                .expect("env key should exist")
        };

        assert_eq!(value("CODEX_HOME"), runtime_home.to_string_lossy());
        assert_eq!(value("HOME"), runtime_home.to_string_lossy());
        assert_eq!(value("CHAINWORKS_TOOLCHAIN_HOME"), value("TOOLCHAIN_HOME"));
        assert!(value("TMPDIR").starts_with(value("TOOLCHAIN_HOME")));
        assert!(value("RUSTUP_HOME").contains("/providers/codex/shared/rust/rustup"));
        assert!(value("CARGO_HOME").contains("/providers/codex/shared/rust/cargo"));
        assert!(!value("CARGO_TARGET_DIR").contains("/.forge-codex-acp/session-1/"));
        assert!(value("RUSTUP_HOME").contains("/rust/rustup"));
        assert!(value("CARGO_HOME").contains("/rust/cargo"));
        assert!(!value("TOOLCHAIN_HOME").starts_with(runtime_home.to_string_lossy().as_ref()));
        assert!(!value("RUSTUP_HOME").starts_with(runtime_home.to_string_lossy().as_ref()));
        assert!(!value("CARGO_HOME").starts_with(runtime_home.to_string_lossy().as_ref()));
        assert!(!value("CARGO_TARGET_DIR").starts_with(runtime_home.to_string_lossy().as_ref()));
        assert!(std::path::Path::new(value("TOOLCHAIN_HOME")).is_dir());
        assert!(std::path::Path::new(value("TMPDIR")).is_dir());
        assert!(std::path::Path::new(value("RUSTUP_HOME")).is_dir());
        assert!(std::path::Path::new(value("CARGO_HOME")).is_dir());
        assert!(std::path::Path::new(value("CARGO_TARGET_DIR")).is_dir());
    }

    #[test]
    fn session_environment_uses_shared_cargo_target_and_sccache_when_available() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime_home = tmp.path().join(".forge-codex-acp").join("session-1");
        let shared_target = tmp.path().join("shared-cargo-target");
        let fake_bin = tmp.path().join("bin");
        let fake_sccache = fake_bin.join("sccache");
        std::fs::create_dir_all(&runtime_home).unwrap();
        std::fs::create_dir_all(&fake_bin).unwrap();
        std::fs::write(&fake_sccache, "#!/bin/sh\nexit 0\n").unwrap();
        let mut perms = std::fs::metadata(&fake_sccache).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
            std::fs::set_permissions(&fake_sccache, perms).unwrap();
        }

        let previous_target = std::env::var_os("CHAINWORKS_SHARED_CARGO_TARGET_DIR");
        let previous_sccache = std::env::var_os("RUSTC_WRAPPER");
        let previous_explicit_sccache = std::env::var_os("CHAINWORKS_SCCACHE_BINARY");
        let previous_path = std::env::var_os("PATH");
        std::env::set_var("CHAINWORKS_SHARED_CARGO_TARGET_DIR", &shared_target);
        std::env::remove_var("RUSTC_WRAPPER");
        std::env::remove_var("CHAINWORKS_SCCACHE_BINARY");
        std::env::set_var("PATH", fake_bin.to_string_lossy().as_ref());

        let env = make_session_environment(&runtime_home);

        match previous_target {
            Some(value) => std::env::set_var("CHAINWORKS_SHARED_CARGO_TARGET_DIR", value),
            None => std::env::remove_var("CHAINWORKS_SHARED_CARGO_TARGET_DIR"),
        }
        match previous_sccache {
            Some(value) => std::env::set_var("RUSTC_WRAPPER", value),
            None => std::env::remove_var("RUSTC_WRAPPER"),
        }
        match previous_explicit_sccache {
            Some(value) => std::env::set_var("CHAINWORKS_SCCACHE_BINARY", value),
            None => std::env::remove_var("CHAINWORKS_SCCACHE_BINARY"),
        }
        match previous_path {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }

        let value = |key: &str| {
            env.iter()
                .find_map(|(name, value)| (name == key).then_some(value.as_str()))
                .expect("env key should exist")
        };
        assert_eq!(value("CARGO_TARGET_DIR"), shared_target.to_string_lossy());
        assert_eq!(value("RUSTC_WRAPPER"), fake_sccache.to_string_lossy());
        assert_eq!(value("SCCACHE_CACHE_SIZE"), "20G");
        assert!(std::path::Path::new(value("CARGO_TARGET_DIR")).is_dir());
        assert!(std::path::Path::new(value("SCCACHE_DIR")).is_dir());
    }

    #[test]
    fn session_environment_adds_provider_bins_when_daemon_path_is_minimal() {
        let tmp = tempfile::tempdir().unwrap();
        let runtime_home = tmp.path().join(".forge-codex-acp").join("session-1");
        std::fs::create_dir_all(&runtime_home).unwrap();
        let previous_path = std::env::var_os("PATH");
        std::env::set_var("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");

        let env = make_session_environment(&runtime_home);

        match previous_path {
            Some(value) => std::env::set_var("PATH", value),
            None => std::env::remove_var("PATH"),
        }

        let path = env
            .iter()
            .find_map(|(name, value)| (name == "PATH").then_some(value.as_str()))
            .expect("PATH should exist");

        assert!(
            path.contains("/opt/homebrew/bin"),
            "PATH should include Homebrew bin for node-backed provider shims: {path}"
        );
    }
}
