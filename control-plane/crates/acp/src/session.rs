use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tracing::warn;

use crate::transport::{AcpSessionConfig, AcpTransportSession};
use crate::{AcpCloseDiagnostic, ExecutionRequest, ExecutionResult};
use domain::ids::AgentExecutionId;

/// Transport-backed ACP session that can accept multiple prompt turns before
/// being closed.
pub struct AcpSession {
    transport: AcpTransportSession,
    cleanup_path: Option<PathBuf>,
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
        let transport = AcpTransportSession::start(child, req, config).await?;
        Ok(Self {
            transport,
            cleanup_path,
        })
    }

    /// Send a prompt through the live ACP session and return the prompt
    /// result. The transport stays open for later reuse.
    pub async fn prompt(&mut self, req: &ExecutionRequest) -> Result<ExecutionResult> {
        let (
            status,
            artifact_paths,
            discovered_artifacts,
            pre_prompt_expected_outputs,
            transcript_text,
            usage,
            acp_pre_initialize_local_latency_ms,
            acp_initialize_latency_ms,
            acp_session_new_latency_ms,
            acp_prompt_duration_ms,
            acp_pre_prompt_metadata_latency_ms,
            acp_pre_prompt_metadata_timeout,
            acp_pre_prompt_metadata_digest_bytes,
            legacy_broad_discovery_snapshot,
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
            agent_execution_id: AgentExecutionId::new(),
            status,
            artifact_paths,
            discovered_artifacts,
            pre_prompt_expected_outputs,
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
            close_diagnostic: None,
            acp_pre_initialize_local_latency_ms: Some(acp_pre_initialize_local_latency_ms),
            acp_initialize_latency_ms: Some(acp_initialize_latency_ms),
            acp_session_new_latency_ms: Some(acp_session_new_latency_ms),
            acp_prompt_duration_ms: Some(acp_prompt_duration_ms),
            acp_pre_prompt_metadata_latency_ms: Some(acp_pre_prompt_metadata_latency_ms),
            acp_pre_prompt_metadata_timeout,
            acp_pre_prompt_metadata_digest_bytes,
            legacy_broad_discovery_snapshot,
        })
    }

    /// Close the live ACP session and wait for the subprocess to exit.
    pub async fn close(&mut self) -> Result<Option<AcpCloseDiagnostic>> {
        let close_result = self.transport.close().await;
        if let Some(path) = self.cleanup_path.take() {
            if let Err(error) = std::fs::remove_dir_all(&path) {
                warn!(
                    cleanup_path = %path.display(),
                    error = %error,
                    "Failed to remove ACP session cleanup path"
                );
            }
        }
        close_result
    }
}

/// Cloneable owned handle to a live ACP session.
///
/// The runtime manager stores these handles by generation id so later turns
/// can reuse the same transport/session pair via `session/prompt`.
#[derive(Clone)]
pub struct AcpSessionHandle {
    inner: Arc<Mutex<AcpSession>>,
    close_requested: Arc<Notify>,
}

impl AcpSessionHandle {
    pub fn new(session: AcpSession) -> Self {
        Self {
            inner: Arc::new(Mutex::new(session)),
            close_requested: Arc::new(Notify::new()),
        }
    }

    /// Send a prompt through the live session.
    pub async fn prompt(&self, req: &ExecutionRequest) -> Result<ExecutionResult> {
        let mut session = self.inner.lock().await;
        tokio::select! {
            result = session.prompt(req) => result,
            _ = self.close_requested.notified() => {
                session.close().await?;
                Err(anyhow::anyhow!(
                    "ACP session closed during active prompt (session={})",
                    session.transport.session_id()
                ))
            }
        }
    }

    /// Close the live session.
    pub async fn close(&self) -> Result<Option<AcpCloseDiagnostic>> {
        self.close_requested.notify_waiters();
        let mut session = self.inner.lock().await;
        session.close().await
    }

    pub async fn is_alive(&self) -> bool {
        let mut session = self.inner.lock().await;
        session.transport.is_alive()
    }

    pub async fn provider_session_id(&self) -> String {
        let session = self.inner.lock().await;
        session.transport.session_id().to_string()
    }
}
