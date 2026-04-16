use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::info;

use crate::adapters::AcpAdapter;
use crate::session::{AcpSession, AcpSessionHandle};
use crate::transport::AcpSessionConfig;
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_GEMINI_ACP_BINARY";

/// Adapter for the Gemini CLI provider (`gemini --acp`).
///
/// Spawns the binary given by `CHAINWORKS_GEMINI_ACP_BINARY` (defaulting to
/// `gemini` on PATH) with the `--acp` flag, and communicates using the ACP
/// JSON-RPC 2.0 protocol over ndjson stdio.
///
/// Note: Gemini CLI requires the `--acp` flag to enter ACP server mode.
/// Some early Gemini versions may not support `session/close` — the transport
/// ignores errors from that phase gracefully.
pub struct GeminiCliAdapter {
    binary_path: String,
}

impl GeminiCliAdapter {
    /// Create a new adapter, resolving the binary from `CHAINWORKS_GEMINI_ACP_BINARY`
    /// or falling back to `gemini` on PATH.
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR).unwrap_or_else(|_| "gemini".to_string());
        Self { binary_path }
    }

    /// Construct with an explicit binary path — for testing and runtime injection.
    pub fn new_with_binary(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
        }
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

    async fn open_session(&self, req: &ExecutionRequest) -> Result<AcpSessionHandle> {
        if self.binary_path.is_empty() {
            bail!(
                "GeminiCliAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure gemini is on PATH"
            );
        }

        info!(
            provider = "gemini",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Spawning Gemini ACP subprocess"
        );

        // Gemini CLI requires --acp to enable ACP server mode.
        let child = Command::new(&self.binary_path)
            .arg("--acp")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("spawn Gemini ACP subprocess: {} --acp", self.binary_path))?;

        // Gemini uses bypassPermissions mode; no _meta block needed.
        // Pass the model from YAML backend_profile; Gemini CLI accepts
        // its own catalog (e.g. gemini-2.5-pro, gemini-3-pro) and falls
        // back to auto-selection if unrecognized.
        let model_str = req.model.as_deref().unwrap_or("default").to_string();
        let config = AcpSessionConfig {
            model: &model_str,
            mode: "bypassPermissions",
            extra: None,
            config_options: Vec::new(),
        };
        let session = AcpSession::start(child, req, &config).await?;

        Ok(AcpSessionHandle::new(session))
    }
}
