use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tracing::warn;

use crate::adapters::XcodeShimGrantCleanup;
use crate::transport::{AcpSessionConfig, AcpTransportSession};
use crate::{AcpCloseDiagnostic, ExecutionRequest, ExecutionResult};
use domain::ids::AgentExecutionId;

/// Transport-backed ACP session that can accept multiple prompt turns before
/// being closed.
pub struct AcpSession {
    transport: AcpTransportSession,
    cleanup_paths: Vec<PathBuf>,
    xcode_shim_grants: Vec<XcodeShimGrantCleanup>,
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
        Self::start_with_cleanup_paths_and_xcode_shim_grants(
            child,
            req,
            config,
            cleanup_paths,
            Vec::new(),
        )
        .await
    }

    pub async fn start_with_cleanup_paths_and_xcode_shim_grants(
        child: tokio::process::Child,
        req: &ExecutionRequest,
        config: &AcpSessionConfig<'_>,
        cleanup_paths: Vec<PathBuf>,
        xcode_shim_grants: Vec<XcodeShimGrantCleanup>,
    ) -> Result<Self> {
        let transport = match AcpTransportSession::start(child, req, config).await {
            Ok(transport) => transport,
            Err(err) => {
                cleanup_paths.iter().for_each(cleanup_path);
                xcode_shim_grants
                    .iter()
                    .for_each(XcodeShimGrantCleanup::remove);
                return Err(err);
            }
        };
        Ok(Self {
            transport,
            cleanup_paths,
            xcode_shim_grants,
        })
    }

    /// Send a prompt through the live ACP session and return the prompt
    /// result. The transport stays open for later reuse.
    pub async fn prompt(&mut self, req: &ExecutionRequest) -> Result<ExecutionResult> {
        self.prompt_with_optional_close_signal(req, None).await
    }

    pub async fn prompt_with_close_signal(
        &mut self,
        req: &ExecutionRequest,
        close_rx: &mut watch::Receiver<bool>,
    ) -> Result<ExecutionResult> {
        self.prompt_with_optional_close_signal(req, Some(close_rx))
            .await
    }

    async fn prompt_with_optional_close_signal(
        &mut self,
        req: &ExecutionRequest,
        close_rx: Option<&mut watch::Receiver<bool>>,
    ) -> Result<ExecutionResult> {
        self.xcode_shim_grants
            .iter()
            .for_each(|grant| grant.set_active_prompt(true));
        let prompt_result = match close_rx {
            Some(close_rx) => self.transport.prompt_with_close_signal(req, close_rx).await,
            None => self.transport.prompt(req).await,
        };
        self.xcode_shim_grants
            .iter()
            .for_each(|grant| grant.set_active_prompt(false));
        let (
            status,
            artifact_paths,
            discovered_artifacts,
            pre_prompt_expected_outputs,
            transcript_text,
            usage,
            xcode_shim_warning_events,
            acp_pre_initialize_local_latency_ms,
            acp_initialize_latency_ms,
            acp_session_new_latency_ms,
            acp_prompt_duration_ms,
            acp_pre_prompt_metadata_latency_ms,
            acp_pre_prompt_metadata_timeout,
            acp_pre_prompt_metadata_digest_bytes,
            legacy_broad_discovery_snapshot,
        ) = prompt_result?;
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
            xcode_shim_warning_events,
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
        for path in self.cleanup_paths.drain(..) {
            if let Err(error) = std::fs::remove_dir_all(&path) {
                warn!(
                    cleanup_path = %path.display(),
                    error = %error,
                    "Failed to remove ACP session cleanup path"
                );
            }
        }
        for grant in self.xcode_shim_grants.drain(..) {
            grant.remove();
        }
        close_result
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
    close_tx: watch::Sender<bool>,
}

impl AcpSessionHandle {
    pub fn new(session: AcpSession) -> Self {
        let (close_tx, _close_rx) = watch::channel(false);
        Self {
            inner: Arc::new(Mutex::new(session)),
            close_tx,
        }
    }

    /// Send a prompt through the live session.
    pub async fn prompt(&self, req: &ExecutionRequest) -> Result<ExecutionResult> {
        let mut close_rx = self.close_tx.subscribe();
        let mut session = self.inner.lock().await;
        session.prompt_with_close_signal(req, &mut close_rx).await
    }

    /// Close the live session.
    pub async fn close(&self) -> Result<()> {
        let _ = self.close_tx.send(true);
        let mut session = self.inner.lock().await;
        session.close().await?;
        Ok(())
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
