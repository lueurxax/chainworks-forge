use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::info;

use crate::adapters::AcpAdapter;
use crate::session::{AcpSession, AcpSessionHandle};
use crate::transport::AcpSessionConfig;
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_AUGGIE_ACP_BINARY";

/// Adapter for the Auggie provider.
///
/// Spawns the binary given by `CHAINWORKS_AUGGIE_ACP_BINARY` (defaulting to
/// `auggie` on PATH) and communicates using the ACP JSON-RPC 2.0 protocol
/// over ndjson stdio.
pub struct AuggieAdapter {
    binary_path: String,
}

impl AuggieAdapter {
    /// Create a new adapter, resolving the binary from `CHAINWORKS_AUGGIE_ACP_BINARY`
    /// or falling back to `auggie` on PATH.
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR)
            .unwrap_or_else(|_| "auggie".to_string());
        Self { binary_path }
    }

    /// Construct with an explicit binary path — for testing and runtime injection.
    pub fn new_with_binary(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
        }
    }
}

impl Default for AuggieAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcpAdapter for AuggieAdapter {
    fn provider_name(&self) -> &str {
        "auggie"
    }

    async fn open_session(&self, req: &ExecutionRequest) -> Result<AcpSessionHandle> {
        if self.binary_path.is_empty() {
            bail!(
                "AuggieAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure auggie is on PATH"
            );
        }

        info!(
            provider = "auggie",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Spawning Auggie ACP subprocess"
        );

        let child = Command::new(&self.binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| {
                format!("spawn Auggie ACP subprocess: {}", self.binary_path)
            })?;

        let config = AcpSessionConfig {
            model: "default",
            mode: "bypassPermissions",
            extra: None,
            config_options: Vec::new(),
        };
        let session = AcpSession::start(child, req, &config).await?;

        Ok(AcpSessionHandle::new(session))
    }
}
