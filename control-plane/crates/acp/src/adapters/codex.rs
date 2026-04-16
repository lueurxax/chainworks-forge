use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::info;
use uuid::Uuid;

use crate::adapters::AcpAdapter;
use crate::session::{AcpSession, AcpSessionHandle};
use crate::transport::AcpSessionConfig;
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_CODEX_ACP_BINARY";

/// Adapter for the OpenAI Codex CLI provider (`codex-acp`).
///
/// Matches Swift `CodexACPTransport`:
/// - Prepares an isolated runtime home with auth.json + config.toml
/// - Sets CODEX_HOME, HOME, TMPDIR, PATH env vars
/// - Mode: `"full-access"`, no `_meta` block
/// - Model mapped to Codex CLI catalog
pub struct CodexAdapter {
    binary_path: String,
}

impl CodexAdapter {
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR)
            .unwrap_or_else(|_| "codex-acp".to_string());
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

    async fn open_session(&self, req: &ExecutionRequest) -> Result<AcpSessionHandle> {
        if self.binary_path.is_empty() {
            bail!(
                "CodexAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure codex-acp is on PATH"
            );
        }

        // ── Prepare isolated runtime home (matches Swift prepareRuntimeHome) ──
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
        let env = make_session_environment(&runtime_home);

        let child = Command::new(&self.binary_path)
            .envs(env)
            // Suppress verbose codex_otel/codex_core/rmcp INFO tracing that
            // floods stderr and causes memory pressure. codex-acp reads RUST_LOG
            // via EnvFilter::from_default_env(). Only show warnings and errors.
            .env("RUST_LOG", "warn")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| {
                format!("spawn Codex ACP subprocess: {}", self.binary_path)
            })?;

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

        let config = AcpSessionConfig {
            model: &base_model,
            mode: "full-access",
            extra: None,
            config_options,
        };
        let session = AcpSession::start_with_cleanup(child, req, &config, Some(runtime_home)).await?;

        Ok(AcpSessionHandle::new(session))
    }
}

// ---------------------------------------------------------------------------
// Runtime home preparation (matches Swift CodexACPTransport)
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
/// copied from the source Codex home. Matches Swift `prepareRuntimeHome`.
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
        info!("Codex: auth.json not found at {}; starting without copied auth",
              source_auth.display());
    }

    // Copy config.toml (sanitized)
    let source_config = source_home.join("config.toml");
    let runtime_config = runtime_home.join("config.toml");
    if source_config.exists() {
        let data = std::fs::read_to_string(&source_config)
            .with_context(|| "read config.toml")?;
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
/// Matches Swift `sanitizeRuntimeConfig`:
/// - Strip sandbox settings
/// - Strip model/effort overrides so session/new model takes priority
fn sanitize_runtime_config(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim().to_lowercase();
            !trimmed.starts_with("sandbox")
                && !trimmed.starts_with("disable_sandbox")
                && !trimmed.starts_with("model")
                && !trimmed.starts_with("hide_rate_limit")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build environment variables matching Swift `makeSessionEnvironment`.
fn make_session_environment(runtime_home: &Path) -> Vec<(String, String)> {
    let home = runtime_home.to_string_lossy().to_string();
    let bin = runtime_home.join("bin").to_string_lossy().to_string();
    let tmp = runtime_home.join("tmp").to_string_lossy().to_string();
    let cache = runtime_home.join(".cache").to_string_lossy().to_string();

    let path = format!(
        "{}:{}",
        bin,
        std::env::var("PATH").unwrap_or_default()
    );

    vec![
        ("CODEX_HOME".into(), home.clone()),
        ("HOME".into(), home),
        ("TMPDIR".into(), tmp),
        ("PATH".into(), path),
        ("XDG_CACHE_HOME".into(), cache),
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
        assert_eq!(split_codex_model_effort("gpt-5.4"), ("gpt-5.4".into(), None));
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
        assert_eq!(split_codex_model_effort("gpt-5.4/"), ("gpt-5.4/".into(), None));
    }
}
