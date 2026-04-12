use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::info;

use domain::ids::AgentExecutionId;

use crate::adapters::AcpAdapter;
use crate::transport::{run_acp_session, AcpSessionConfig};
use crate::{ExecutionRequest, ExecutionResult};

const BINARY_ENV_VAR: &str = "CHAINWORKS_CLAUDE_ACP_BINARY";

/// Adapter for the Claude Agent provider (`claude-agent-acp`).
///
/// Spawns the binary given by `CHAINWORKS_CLAUDE_ACP_BINARY` (defaulting to
/// `claude-agent-acp` on PATH) and communicates with it using the ACP
/// JSON-RPC 2.0 protocol over ndjson stdio.
///
/// Protocol: `initialize` → `session/new` → `session/prompt` (streaming)
///   → `session/close` → graceful shutdown.
///
/// Artifact paths are discovered by diffing the workspace filesystem before
/// and after the session.
pub struct ClaudeAgentAdapter {
    binary_path: String,
}

impl ClaudeAgentAdapter {
    /// Create a new adapter, resolving the binary from `CHAINWORKS_CLAUDE_ACP_BINARY`
    /// or falling back to `claude-agent-acp` on PATH.
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR)
            .unwrap_or_else(|_| "claude-agent-acp".to_string());
        Self { binary_path }
    }

    /// Construct with an explicit binary path — for testing and runtime injection.
    pub fn new_with_binary(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
        }
    }
}

impl Default for ClaudeAgentAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcpAdapter for ClaudeAgentAdapter {
    fn provider_name(&self) -> &str {
        "claude"
    }

    async fn execute(&self, req: ExecutionRequest) -> Result<ExecutionResult> {
        if self.binary_path.is_empty() {
            bail!(
                "ClaudeAgentAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure claude-agent-acp is on PATH"
            );
        }

        info!(
            provider = "claude",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Spawning Claude ACP subprocess"
        );

        let agent_execution_id = AgentExecutionId::new();

        // Claude Agent ACP is invoked with no extra arguments.
        let mut child = Command::new(&self.binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| {
                format!("spawn Claude ACP subprocess: {}", self.binary_path)
            })?;

        let default_config = AcpSessionConfig::default();
        let model_str = req.model.as_deref().unwrap_or(default_config.model).to_string();
        let config = AcpSessionConfig {
            model: &model_str,
            ..default_config
        };
        let (status, artifact_paths) =
            run_acp_session(&mut child, &req, &config).await?;

        Ok(ExecutionResult {
            agent_execution_id,
            status,
            artifact_paths,
            cost_cents: None,
        })
    }
}
