use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::info;

use domain::ids::AgentExecutionId;

use crate::adapters::AcpAdapter;
use crate::transport::{run_acp_session, AcpSessionConfig};
use crate::{ExecutionRequest, ExecutionResult};

const BINARY_ENV_VAR: &str = "CHAINWORKS_CODEX_ACP_BINARY";

/// Adapter for the OpenAI Codex CLI provider (`codex-acp`).
///
/// Spawns the binary given by `CHAINWORKS_CODEX_ACP_BINARY` (defaulting to
/// `codex-acp` on PATH) and communicates using the ACP JSON-RPC 2.0 protocol
/// over ndjson stdio.
///
/// Differences from the Claude adapter (matching `CodexACPTransport.swift`):
/// - Mode is `"full-access"` (write-enabled) rather than `"bypassPermissions"`
/// - Model catalog: `"gpt-5"`, `"gpt-5-codex"`, `"o4-mini"`; default `"gpt-5"`
/// - No `_meta.claudeCode.options` block in `session/new`
/// - Binary is `codex-acp`, invoked without extra arguments
pub struct CodexAdapter {
    binary_path: String,
}

impl CodexAdapter {
    /// Create a new adapter, resolving the binary from `CHAINWORKS_CODEX_ACP_BINARY`
    /// or falling back to `codex-acp` on PATH.
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR)
            .unwrap_or_else(|_| "codex-acp".to_string());
        Self { binary_path }
    }

    /// Construct with an explicit binary path — for testing and runtime injection.
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

        info!(
            provider = "codex",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Spawning Codex ACP subprocess"
        );

        let agent_execution_id = AgentExecutionId::new();

        // Codex ACP is invoked with no extra arguments (unlike `gemini --acp`).
        let mut child = Command::new(&self.binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| {
                format!("spawn Codex ACP subprocess: {}", self.binary_path)
            })?;

        // Codex-specific session config:
        // - mode: "full-access"  (write-enabled autonomous execution)
        // - no _meta block        (Claude-specific plugin control, not applicable)
        // - model mapped to Codex CLI catalog (matches Swift mapModelForCodexCatalog)
        let model_str = map_model_for_codex(req.model.as_deref().unwrap_or("gpt-5"));
        let config = AcpSessionConfig {
            model: &model_str,
            mode: "full-access",
            extra: None,
        };

        let (status, artifact_paths) = run_acp_session(&mut child, &req, &config).await?;

        Ok(ExecutionResult {
            agent_execution_id,
            status,
            artifact_paths,
            cost_cents: None,
        })
    }
}

/// Map a model identifier to the Codex CLI catalog.
/// Matches Swift `CodexACPTransport.mapModelForCodexCatalog`.
/// Known models pass through; others are mapped to the closest known model.
fn map_model_for_codex(model: &str) -> String {
    let lowered = model.to_lowercase();
    match lowered.as_str() {
        "gpt-5" | "gpt-5-codex" | "o4-mini" => lowered,
        _ => {
            if lowered.contains("gpt-5") { return "gpt-5".to_string(); }
            if lowered.contains("codex") { return "gpt-5-codex".to_string(); }
            if lowered.contains("o4-mini") { return "o4-mini".to_string(); }
            "gpt-5".to_string()
        }
    }
}
