use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tracing::warn;
use uuid::Uuid;

use crate::adapters::XcodeShimGrantCleanup;
use crate::adapters::{CleanupPathPolicy, CleanupPathSpec};
use crate::transport::{AcpSessionConfig, AcpTransportSession};
use crate::{
    AcpCloseDiagnostic, AcpExecutionError, AcpPromptProgressSink, ExecutionRequest,
    ExecutionResult, NoopAcpPromptProgressSink, ProviderSessionStoreCapture,
};
use domain::ids::AgentExecutionId;

/// Transport-backed ACP session that can accept multiple prompt turns before
/// being closed.
pub struct AcpSession {
    transport: AcpTransportSession,
    cleanup_paths: Vec<CleanupPathSpec>,
    xcode_shim_grants: Vec<XcodeShimGrantCleanup>,
}

#[derive(Clone, Debug)]
pub enum AcpSessionCloseBehavior {
    Delete,
    StageForOutcome,
    ArchiveFailure(ProviderSessionStoreArchiveContext),
}

#[derive(Clone, Debug)]
pub struct ProviderSessionStoreArchiveContext {
    pub provider: String,
    pub run_id: String,
    pub stage_id: String,
    pub agent_id: String,
    pub agent_execution_id: Option<String>,
    pub session_generation_id: Option<String>,
    pub provider_session_id: Option<String>,
    pub failure_kind: String,
}

#[derive(Clone, Debug, Default)]
pub struct AcpCloseOutcome {
    pub diagnostic: Option<AcpCloseDiagnostic>,
    pub provider_session_store_capture: Option<ProviderSessionStoreCapture>,
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
        Self::start_with_cleanup_paths(
            child,
            req,
            config,
            cleanup_path
                .into_iter()
                .map(CleanupPathSpec::delete)
                .collect(),
        )
        .await
    }

    /// Start a new transport-backed session and remove every cleanup path when
    /// the session is eventually closed. If transport startup fails, the paths
    /// are removed before returning the error.
    pub async fn start_with_cleanup_paths(
        child: tokio::process::Child,
        req: &ExecutionRequest,
        config: &AcpSessionConfig<'_>,
        cleanup_paths: Vec<CleanupPathSpec>,
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
        cleanup_paths: Vec<CleanupPathSpec>,
        xcode_shim_grants: Vec<XcodeShimGrantCleanup>,
    ) -> Result<Self> {
        let transport = match AcpTransportSession::start(child, req, config).await {
            Ok(transport) => transport,
            Err(err) => {
                cleanup_paths
                    .iter()
                    .for_each(|spec| cleanup_path(&spec.path));
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
        self.prompt_with_optional_close_signal(req, None, Arc::new(NoopAcpPromptProgressSink))
            .await
    }

    pub async fn prompt_with_progress_sink(
        &mut self,
        req: &ExecutionRequest,
        progress_sink: Arc<dyn AcpPromptProgressSink>,
    ) -> Result<ExecutionResult> {
        self.prompt_with_optional_close_signal(req, None, progress_sink)
            .await
    }

    pub async fn prompt_with_close_signal(
        &mut self,
        req: &ExecutionRequest,
        close_rx: &mut watch::Receiver<bool>,
    ) -> Result<ExecutionResult> {
        self.prompt_with_optional_close_signal(
            req,
            Some(close_rx),
            Arc::new(NoopAcpPromptProgressSink),
        )
        .await
    }

    async fn prompt_with_optional_close_signal(
        &mut self,
        req: &ExecutionRequest,
        close_rx: Option<&mut watch::Receiver<bool>>,
        progress_sink: Arc<dyn AcpPromptProgressSink>,
    ) -> Result<ExecutionResult> {
        self.xcode_shim_grants
            .iter()
            .for_each(|grant| grant.set_active_prompt(true));
        let prompt_result = match close_rx {
            Some(close_rx) => {
                self.transport
                    .prompt_with_close_signal_and_progress_sink(req, close_rx, progress_sink)
                    .await
            }
            None => {
                self.transport
                    .prompt_with_progress_sink(req, progress_sink)
                    .await
            }
        };
        let prompt_result = match prompt_result {
            Ok(prompt_result) => prompt_result,
            Err(error) => {
                if let Some(receipt) = self.transport.runtime_receipt().cloned() {
                    let message = error.to_string();
                    return Err(
                        anyhow::Error::new(AcpExecutionError::new(message, Some(receipt)))
                            .context(error),
                    );
                }
                return Err(error);
            }
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
            completion_text_capture,
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
        ) = prompt_result;
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
            completion_text_capture,
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
            provider_session_store_capture: None,
            acp_pre_initialize_local_latency_ms: Some(acp_pre_initialize_local_latency_ms),
            acp_initialize_latency_ms: Some(acp_initialize_latency_ms),
            acp_session_new_latency_ms: Some(acp_session_new_latency_ms),
            acp_prompt_duration_ms: Some(acp_prompt_duration_ms),
            acp_pre_prompt_metadata_latency_ms: Some(acp_pre_prompt_metadata_latency_ms),
            acp_pre_prompt_metadata_timeout,
            acp_pre_prompt_metadata_digest_bytes,
            legacy_broad_discovery_snapshot,
            runtime_receipt: self.transport.runtime_receipt().cloned(),
            runtime_tool_path_preflight_json: None,
        })
    }

    /// Close the live ACP session and wait for the subprocess to exit.
    pub async fn close(&mut self) -> Result<Option<AcpCloseDiagnostic>> {
        Ok(self
            .close_with_behavior(AcpSessionCloseBehavior::Delete)
            .await?
            .diagnostic)
    }

    pub async fn close_with_behavior(
        &mut self,
        behavior: AcpSessionCloseBehavior,
    ) -> Result<AcpCloseOutcome> {
        let close_result = self.transport.close().await;
        let mut outcome = AcpCloseOutcome {
            diagnostic: close_result?,
            provider_session_store_capture: None,
        };
        for spec in self.cleanup_paths.drain(..) {
            match finalize_cleanup_spec(&spec, &behavior) {
                Ok(Some(capture)) => {
                    outcome.provider_session_store_capture = Some(capture);
                }
                Ok(None) => {}
                Err(error) => warn!(
                    cleanup_path = %spec.path.display(),
                    error = %error,
                    "Failed to finalize ACP session cleanup path"
                ),
            }
        }
        for grant in self.xcode_shim_grants.drain(..) {
            grant.remove();
        }
        Ok(outcome)
    }

    pub fn is_live(&mut self) -> bool {
        !self.transport.is_closed() && matches!(self.transport.try_wait(), Ok(None))
    }
}

fn cleanup_path(path: &PathBuf) {
    let _ = std::fs::remove_dir_all(path);
}

fn finalize_cleanup_spec(
    spec: &CleanupPathSpec,
    behavior: &AcpSessionCloseBehavior,
) -> Result<Option<ProviderSessionStoreCapture>> {
    match spec.policy {
        CleanupPathPolicy::DeleteRecursively => {
            remove_cleanup_dir(&spec.path)?;
            Ok(None)
        }
        CleanupPathPolicy::StageCodexSessionStore => match behavior {
            AcpSessionCloseBehavior::Delete => {
                remove_cleanup_dir(&spec.path)?;
                Ok(None)
            }
            AcpSessionCloseBehavior::StageForOutcome => {
                let capture = stage_codex_session_store(&spec.path)?;
                remove_cleanup_dir(&spec.path)?;
                Ok(capture)
            }
            AcpSessionCloseBehavior::ArchiveFailure(context) => {
                let capture = stage_codex_session_store(&spec.path)?;
                remove_cleanup_dir(&spec.path)?;
                if let Some(capture) = capture.as_ref() {
                    finalize_provider_session_store_capture(capture, true, context)?;
                }
                Ok(None)
            }
        },
    }
}

fn remove_cleanup_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_dir_all(path).with_context(|| format!("remove cleanup path {}", path.display()))
}

fn app_support_runtime_root() -> PathBuf {
    if let Ok(explicit) = std::env::var("CHAINWORKS_SESSION_STORE_ROOT") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Library")
        .join("Application Support")
        .join("Chainworks Forge")
        .join("runtime")
}

fn stage_codex_session_store(runtime_home: &Path) -> Result<Option<ProviderSessionStoreCapture>> {
    let mut captured_subdirs = Vec::new();
    let staging_root = app_support_runtime_root()
        .join("pending-session-stores")
        .join("codex")
        .join(Uuid::new_v4().to_string());

    for subdir in ["sessions", "archived_sessions"] {
        let source = runtime_home.join(subdir);
        if !source.exists() {
            continue;
        }
        let dest = staging_root.join(subdir);
        copy_dir_recursive(&source, &dest)
            .with_context(|| format!("copy provider session store {}", source.display()))?;
        captured_subdirs.push(subdir.to_string());
    }

    if captured_subdirs.is_empty() {
        if staging_root.exists() {
            let _ = fs::remove_dir_all(&staging_root);
        }
        return Ok(None);
    }

    Ok(Some(ProviderSessionStoreCapture {
        provider: "codex".to_string(),
        staging_root: staging_root.to_string_lossy().into_owned(),
        captured_subdirs,
    }))
}

pub fn finalize_provider_session_store_capture(
    capture: &ProviderSessionStoreCapture,
    preserve_failure: bool,
    context: &ProviderSessionStoreArchiveContext,
) -> Result<Option<PathBuf>> {
    let staging_root = Path::new(&capture.staging_root);
    if !staging_root.exists() {
        return Ok(None);
    }

    if !preserve_failure {
        fs::remove_dir_all(staging_root).with_context(|| {
            format!(
                "remove staged provider session store {}",
                staging_root.display()
            )
        })?;
        return Ok(None);
    }

    let archive_root = app_support_runtime_root()
        .join("session-store-archives")
        .join(&capture.provider)
        .join(Utc::now().format("%Y-%m-%d").to_string())
        .join(format!(
            "{}-{}",
            context
                .agent_execution_id
                .as_deref()
                .unwrap_or("unknown-agent-execution"),
            context
                .session_generation_id
                .as_deref()
                .unwrap_or("unknown-session-generation")
        ));
    if let Some(parent) = archive_root.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create provider archive parent {}", parent.display()))?;
    }
    if archive_root.exists() {
        fs::remove_dir_all(&archive_root).ok();
    }
    fs::rename(staging_root, &archive_root).or_else(|_| {
        copy_dir_recursive(staging_root, &archive_root)?;
        fs::remove_dir_all(staging_root)?;
        Ok::<_, anyhow::Error>(())
    })?;

    let metadata_path = archive_root.join("metadata.json");
    let metadata = json!({
        "provider": capture.provider,
        "run_id": context.run_id,
        "stage_id": context.stage_id,
        "agent_id": context.agent_id,
        "agent_execution_id": context.agent_execution_id,
        "session_generation_id": context.session_generation_id,
        "provider_session_id": context.provider_session_id,
        "failure_kind": context.failure_kind,
        "captured_subdirs": capture.captured_subdirs,
        "archived_at": Utc::now().to_rfc3339(),
    });
    fs::write(&metadata_path, serde_json::to_vec_pretty(&metadata)?).with_context(|| {
        format!(
            "write provider archive metadata {}",
            metadata_path.display()
        )
    })?;
    Ok(Some(archive_root))
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)
        .with_context(|| format!("create destination directory {}", dest.display()))?;
    for entry in fs::read_dir(source)
        .with_context(|| format!("read source directory {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = dest.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &dest_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &dest_path).with_context(|| {
                format!(
                    "copy file from {} to {}",
                    source_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
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
        self.prompt_with_progress_sink(req, Arc::new(NoopAcpPromptProgressSink))
            .await
    }

    /// Send a prompt through the live session and publish transport progress.
    pub async fn prompt_with_progress_sink(
        &self,
        req: &ExecutionRequest,
        progress_sink: Arc<dyn AcpPromptProgressSink>,
    ) -> Result<ExecutionResult> {
        let mut close_rx = self.close_tx.subscribe();
        let mut session = self.inner.lock().await;
        session
            .prompt_with_optional_close_signal(req, Some(&mut close_rx), progress_sink)
            .await
    }

    /// Close the live session.
    pub async fn close(&self) -> Result<()> {
        let _ = self.close_tx.send(true);
        let mut session = self.inner.lock().await;
        session.close().await?;
        Ok(())
    }

    pub async fn close_with_behavior(
        &self,
        behavior: AcpSessionCloseBehavior,
    ) -> Result<AcpCloseOutcome> {
        let _ = self.close_tx.send(true);
        let mut session = self.inner.lock().await;
        session.close_with_behavior(behavior).await
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static SESSION_STORE_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn write_file(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    #[test]
    fn stage_codex_session_store_copies_only_session_directories() {
        let _guard = SESSION_STORE_ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        let runtime_home = temp.path().join("runtime-home");
        write_file(&runtime_home.join("sessions").join("a.json"), "a");
        write_file(&runtime_home.join("archived_sessions").join("b.json"), "b");
        write_file(&runtime_home.join("tmp").join("noise.txt"), "noise");

        let capture = stage_codex_session_store(&runtime_home)
            .unwrap()
            .expect("capture expected");

        let staging_root = Path::new(&capture.staging_root);
        assert!(staging_root.join("sessions").join("a.json").exists());
        assert!(staging_root
            .join("archived_sessions")
            .join("b.json")
            .exists());
        assert!(!staging_root.join("tmp").exists());
        assert_eq!(
            capture.captured_subdirs,
            vec!["sessions".to_string(), "archived_sessions".to_string()]
        );
    }

    #[test]
    fn stage_codex_session_store_is_noop_when_no_session_dirs_exist() {
        let _guard = SESSION_STORE_ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        let runtime_home = temp.path().join("runtime-home");
        write_file(&runtime_home.join("tmp").join("noise.txt"), "noise");

        let capture = stage_codex_session_store(&runtime_home).unwrap();
        assert!(capture.is_none());
    }

    #[test]
    fn finalize_provider_session_store_capture_archives_failed_capture_with_metadata() {
        let _guard = SESSION_STORE_ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        std::env::set_var(
            "CHAINWORKS_SESSION_STORE_ROOT",
            temp.path()
                .join("archive-root")
                .to_string_lossy()
                .to_string(),
        );
        let staging_root = temp.path().join("staging-root");
        write_file(&staging_root.join("sessions").join("a.json"), "a");

        let capture = ProviderSessionStoreCapture {
            provider: "codex".into(),
            staging_root: staging_root.to_string_lossy().into_owned(),
            captured_subdirs: vec!["sessions".into()],
        };
        let context = ProviderSessionStoreArchiveContext {
            provider: "codex".into(),
            run_id: "run-1".into(),
            stage_id: "state_9".into(),
            agent_id: "proposal_implementation_auditor".into(),
            agent_execution_id: Some("agent-exec-1".into()),
            session_generation_id: Some("session-gen-1".into()),
            provider_session_id: Some("provider-session-1".into()),
            failure_kind: "provider_timeout".into(),
        };

        let archive_root = finalize_provider_session_store_capture(&capture, true, &context)
            .unwrap()
            .expect("archive root expected");
        assert!(archive_root.join("sessions").join("a.json").exists());
        assert!(archive_root.join("metadata.json").exists());
        assert!(!staging_root.exists());
        std::env::remove_var("CHAINWORKS_SESSION_STORE_ROOT");
    }

    #[test]
    fn finalize_provider_session_store_capture_removes_staged_copy_on_success() {
        let _guard = SESSION_STORE_ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        std::env::set_var(
            "CHAINWORKS_SESSION_STORE_ROOT",
            temp.path()
                .join("archive-root")
                .to_string_lossy()
                .to_string(),
        );
        let staging_root = temp.path().join("staging-root");
        write_file(&staging_root.join("sessions").join("a.json"), "a");

        let capture = ProviderSessionStoreCapture {
            provider: "codex".into(),
            staging_root: staging_root.to_string_lossy().into_owned(),
            captured_subdirs: vec!["sessions".into()],
        };
        let context = ProviderSessionStoreArchiveContext {
            provider: "codex".into(),
            run_id: "run-1".into(),
            stage_id: "state_9".into(),
            agent_id: "proposal_implementation_auditor".into(),
            agent_execution_id: Some("agent-exec-1".into()),
            session_generation_id: Some("session-gen-1".into()),
            provider_session_id: Some("provider-session-1".into()),
            failure_kind: "completed".into(),
        };

        let archive_root =
            finalize_provider_session_store_capture(&capture, false, &context).unwrap();
        assert!(archive_root.is_none());
        assert!(!staging_root.exists());
        std::env::remove_var("CHAINWORKS_SESSION_STORE_ROOT");
    }
}
