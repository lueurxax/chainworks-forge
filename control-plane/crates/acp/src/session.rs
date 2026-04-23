use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::transport::{AcpSessionConfig, AcpTransportSession};
use crate::{ExecutionRequest, ExecutionResult};
use domain::ids::AgentExecutionId;

/// Transport-backed ACP session that can accept multiple prompt turns before
/// being closed.
pub struct AcpSession {
    transport: AcpTransportSession,
    cleanup_paths: Vec<PathBuf>,
}

impl AcpSession {
    /// Start a new transport-backed ACP session from a spawned subprocess.
    pub async fn start(
        child: tokio::process::Child,
        req: &ExecutionRequest,
        config: &AcpSessionConfig<'_>,
    ) -> Result<Self> {
        Self::start_with_cleanup(child, req, config, None).await
    }

    /// Start a new transport-backed session and remove `cleanup_path` when the
    /// session is eventually closed.
    pub async fn start_with_cleanup(
        child: tokio::process::Child,
        req: &ExecutionRequest,
        config: &AcpSessionConfig<'_>,
        cleanup_path: Option<PathBuf>,
    ) -> Result<Self> {
        Self::start_with_cleanup_paths(child, req, config, cleanup_path.into_iter().collect()).await
    }

    /// Start a new transport-backed session and remove every cleanup path when
    /// the session is eventually closed. If transport startup fails, the paths
    /// are removed before returning the error.
    pub async fn start_with_cleanup_paths(
        child: tokio::process::Child,
        req: &ExecutionRequest,
        config: &AcpSessionConfig<'_>,
        cleanup_paths: Vec<PathBuf>,
    ) -> Result<Self> {
        let transport = match AcpTransportSession::start(child, req, config).await {
            Ok(transport) => transport,
            Err(err) => {
                cleanup_paths.iter().for_each(cleanup_path);
                return Err(err);
            }
        };
        Ok(Self {
            transport,
            cleanup_paths,
        })
    }

    /// Send a prompt through the live ACP session and return the prompt
    /// result. The transport stays open for later reuse.
    pub async fn prompt(&mut self, req: &ExecutionRequest) -> Result<ExecutionResult> {
        let (
            status,
            artifact_paths,
            discovered_artifacts,
            transcript_text,
            usage,
            xcode_shim_warning_events,
        ) = self.transport.prompt(req).await?;
        let mcp_observation = self.transport.mcp_observation();
        let actual_mcp_extensions = mcp_observation
            .as_ref()
            .map(|observation| observation.actual_extensions.clone())
            .unwrap_or_default();
        let actual_mcp_runtime_ids = mcp_observation
            .as_ref()
            .map(|observation| observation.actual_runtime_ids.clone())
            .unwrap_or_default();
        Ok(ExecutionResult {
            agent_execution_id: req.agent_execution_id.unwrap_or_else(AgentExecutionId::new),
            status,
            artifact_paths,
            discovered_artifacts,
            transcript_text,
            cost_cents: usage.as_ref().and_then(|snapshot| snapshot.cost_cents),
            usage,
            provider_session_id: Some(self.transport.session_id().to_string()),
            reused_existing_session: false,
            session_generation_id: None,
            mcp_observation,
            actual_mcp_extensions,
            actual_mcp_runtime_ids,
            mcp_session_startup_latency_ms: self.transport.mcp_session_startup_latency_ms(),
            xcode_shim_warning_events,
            close_diagnostic: None,
        })
    }

    /// Close the live ACP session and wait for the subprocess to exit.
    pub async fn close(&mut self) -> Result<()> {
        self.transport.close().await?;
        for path in self.cleanup_paths.drain(..) {
            cleanup_path(&path);
        }
        Ok(())
    }

    pub fn is_live(&mut self) -> bool {
        !self.transport.is_closed() && matches!(self.transport.try_wait(), Ok(None))
    }
}

fn cleanup_path(path: &PathBuf) {
    let _ = std::fs::remove_dir_all(path);
}

/// Cloneable owned handle to a live ACP session.
///
/// The runtime manager stores these handles by generation id so later turns
/// can reuse the same transport/session pair via `session/prompt`.
#[derive(Clone)]
pub struct AcpSessionHandle {
    inner: Arc<Mutex<AcpSession>>,
}

impl AcpSessionHandle {
    pub fn new(session: AcpSession) -> Self {
        Self {
            inner: Arc::new(Mutex::new(session)),
        }
    }

    /// Send a prompt through the live session.
    pub async fn prompt(&self, req: &ExecutionRequest) -> Result<ExecutionResult> {
        let mut session = self.inner.lock().await;
        session.prompt(req).await
    }

    /// Close the live session.
    pub async fn close(&self) -> Result<()> {
        let mut session = self.inner.lock().await;
        session.close().await
    }

    pub async fn provider_session_id(&self) -> String {
        let session = self.inner.lock().await;
        session.transport.session_id().to_string()
    }

    pub async fn is_live(&self) -> bool {
        let mut session = self.inner.lock().await;
        session.is_live()
    }
}
