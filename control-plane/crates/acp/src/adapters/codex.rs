use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::info;
use uuid::Uuid;

use domain::ids::AgentExecutionId;

use crate::adapters::AcpAdapter;
use crate::transport::{run_acp_session, AcpSessionConfig};
use crate::{ExecutionRequest, ExecutionResult};

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

    async fn execute(&self, req: ExecutionRequest) -> Result<ExecutionResult> {
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

        let agent_execution_id = AgentExecutionId::new();

        // Build environment matching Swift makeSessionEnvironment
        let env = make_session_environment(&runtime_home);

        let mut child = Command::new(&self.binary_path)
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

        let model_str = build_codex_model_id(
            req.model.as_deref().unwrap_or("gpt-5"),
            req.effort.as_deref(),
        );
        let config = AcpSessionConfig {
            model: &model_str,
            mode: "full-access",
            extra: None,
        };

        let (status, artifact_paths) = run_acp_session(&mut child, &req, &config).await?;

        // Cleanup runtime home (best-effort, don't fail the run)
        let _ = std::fs::remove_dir_all(&runtime_home);

        Ok(ExecutionResult {
            agent_execution_id,
            status,
            artifact_paths,
            cost_cents: None,
        })
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

/// Map a model identifier to the Codex CLI catalog.
/// Matches Swift `mapModelForCodexCatalog`.
/// Build the Codex model ID by combining model + effort.
/// Codex catalog uses `model/effort` format: `gpt-5.4/high`, `gpt-5.3-codex/medium`.
/// If the model already contains `/`, it's used as-is.
/// Effort values from YAML must match the Codex catalog exactly (low/medium/high/xhigh).
fn build_codex_model_id(model: &str, effort: Option<&str>) -> String {
    let lowered = model.to_lowercase();
    if lowered.contains('/') {
        return lowered;
    }
    match effort {
        Some(e) => format!("{}/{}", lowered, e.to_lowercase()),
        None => lowered,
    }
}
