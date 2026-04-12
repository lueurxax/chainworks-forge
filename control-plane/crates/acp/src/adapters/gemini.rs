use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{info, warn};

use domain::agent::AgentStatus;
use domain::ids::AgentExecutionId;

use crate::adapters::AcpAdapter;
use crate::{ExecutionRequest, ExecutionResult};

const EXEC_TIMEOUT_SECS: u64 = 300;
const BINARY_ENV_VAR: &str = "CHAINWORKS_GEMINI_ACP_BINARY";

/// Adapter for the Gemini CLI provider.
/// Spawns the ACP binary specified by CHAINWORKS_GEMINI_ACP_BINARY, writes the
/// serialized ExecutionRequest as JSON to its stdin, and collects artifact paths
/// (one per line) from stdout. Exits non-zero → AgentStatus::Failed.
pub struct GeminiCliAdapter {
    binary_path: String,
}

impl GeminiCliAdapter {
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR).unwrap_or_default();
        Self { binary_path }
    }
}

impl Default for GeminiCliAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcpAdapter for GeminiCliAdapter {
    fn provider_name(&self) -> &str {
        "gemini"
    }

    async fn execute(&self, req: ExecutionRequest) -> Result<ExecutionResult> {
        if self.binary_path.is_empty() {
            bail!(
                "GeminiCliAdapter: {} is not set — cannot invoke ACP subprocess",
                BINARY_ENV_VAR
            );
        }

        info!(
            provider = "gemini",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Invoking Gemini ACP subprocess"
        );

        let agent_execution_id = AgentExecutionId::new();
        let payload = serde_json::to_vec(&req).context("serialize ExecutionRequest")?;

        let mut child = Command::new(&self.binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("spawn Gemini ACP subprocess")?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&payload)
                .await
                .context("write ExecutionRequest to Gemini ACP stdin")?;
        }

        let output = timeout(
            Duration::from_secs(EXEC_TIMEOUT_SECS),
            child.wait_with_output(),
        )
        .await
        .context("Gemini ACP subprocess timed out after 300s")?
        .context("wait_with_output for Gemini ACP subprocess")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                run_id = %req.run_id,
                stage_id = %req.stage_id,
                exit_code = ?output.status.code(),
                stderr = %stderr,
                "Gemini ACP subprocess exited with failure"
            );
            return Ok(ExecutionResult {
                agent_execution_id,
                status: AgentStatus::Failed,
                artifact_paths: vec![],
                cost_cents: None,
            });
        }

        // Collect artifact paths: one absolute path per stdout line.
        let stdout = String::from_utf8_lossy(&output.stdout);
        let artifact_paths: Vec<String> = stdout
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect();

        Ok(ExecutionResult {
            agent_execution_id,
            status: AgentStatus::Completed,
            artifact_paths,
            cost_cents: None,
        })
    }
}
