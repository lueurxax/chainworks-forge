use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use tokio::process::Command;
use tracing::info;

use crate::adapters::AcpAdapter;
use crate::session::{AcpSession, AcpSessionHandle};
use crate::transport::AcpSessionConfig;
use crate::ExecutionRequest;

const BINARY_ENV_VAR: &str = "CHAINWORKS_JUNIE_ACP_BINARY";

/// Adapter for the Junie provider.
///
/// Spawns the binary given by `CHAINWORKS_JUNIE_ACP_BINARY` (defaulting to
/// `junie` on PATH) and communicates using the ACP JSON-RPC 2.0 protocol
/// over ndjson stdio.
pub struct JunieAdapter {
    binary_path: String,
}

impl JunieAdapter {
    /// Create a new adapter, resolving the binary from `CHAINWORKS_JUNIE_ACP_BINARY`
    /// or falling back to `junie` on PATH.
    pub fn new() -> Self {
        let binary_path = std::env::var(BINARY_ENV_VAR)
            .unwrap_or_else(|_| "junie".to_string());
        Self { binary_path }
    }

    /// Construct with an explicit binary path — for testing and runtime injection.
    pub fn new_with_binary(path: impl Into<String>) -> Self {
        Self {
            binary_path: path.into(),
        }
    }
}

impl Default for JunieAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AcpAdapter for JunieAdapter {
    fn provider_name(&self) -> &str {
        "junie"
    }

    async fn open_session(&self, req: &ExecutionRequest) -> Result<AcpSessionHandle> {
        if self.binary_path.is_empty() {
            bail!(
                "JunieAdapter: binary path is empty — set {BINARY_ENV_VAR} \
                 or ensure junie is on PATH"
            );
        }

        info!(
            provider = "junie",
            run_id = %req.run_id,
            stage_id = %req.stage_id,
            agent_id = %req.agent_id,
            binary = %self.binary_path,
            "Spawning Junie ACP subprocess"
        );

        let child = Command::new(&self.binary_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| {
                format!("spawn Junie ACP subprocess: {}", self.binary_path)
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
