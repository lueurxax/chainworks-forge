use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tracing::info;
use uuid::Uuid;

use crate::adapters::{AcpAdapter, AcpLaunchSpec, AcpSessionNewSpec, LaunchResourceGuard};
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_CODEX_ACP_BINARY";

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

        resources.add_cleanup_path(runtime_home);
        Ok(AcpLaunchSpec::new(&self.binary_path).with_envs(env))
    }

    fn prepare_session_new_spec(&self, req: &ExecutionRequest) -> Result<AcpSessionNewSpec> {
        // Build the base model id (without /effort suffix) and extract the
        // effort separately. Codex's session/new silently falls back to
        // medium when given a combined "model/effort" string, so we pass a
        // bare model and set reasoning_effort via session/set_config_option.
        let raw_model = req.model.as_deref().unwrap_or("gpt-5");
        let (base_model, effort_from_model) = split_codex_model_effort(raw_model);
        let effort = req
            .effort
            .as_deref()
            .map(|e| e.to_lowercase())
            .or(effort_from_model);

        let mut config_options: Vec<(String, String)> = Vec::new();
        if let Some(e) = effort.as_deref() {
            // Codex accepts low / medium / high / xhigh for reasoning_effort.
            config_options.push(("reasoning_effort".into(), e.to_string()));
        }

        let mut spec = AcpSessionNewSpec::new(base_model, "full-access");
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

    Ok(runtime_home)
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
    let rustup_home = toolchain_root.join("rust").join("rustup");
    let cargo_home = toolchain_root.join("rust").join("cargo");
    let cargo_target_dir = toolchain_root.join("rust").join("target");

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

    vec![
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
    ]
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
        assert!(value("RUSTUP_HOME").starts_with(value("TOOLCHAIN_HOME")));
        assert!(value("CARGO_HOME").starts_with(value("TOOLCHAIN_HOME")));
        assert!(value("CARGO_TARGET_DIR").starts_with(value("TOOLCHAIN_HOME")));
        assert!(value("RUSTUP_HOME").contains("/rust/rustup"));
        assert!(value("CARGO_HOME").contains("/rust/cargo"));
        assert!(value("CARGO_TARGET_DIR").contains("/rust/target"));
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
