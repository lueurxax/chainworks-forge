use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tracing::warn;
use uuid::Uuid;

use crate::adapters::XcodeShimGrantCleanup;
use crate::adapters::{CleanupPathPolicy, CleanupPathSpec};
use crate::transport::{AcpSessionConfig, AcpTransportSession};
use crate::{
    AcpCloseDiagnostic, AcpCompletionCaptureSource, AcpExecutionError, AcpPromptProgressSink,
    AcpRuntimeReceipt, AcpRuntimeReceiptEvent, ExecutionRequest, ExecutionResult,
    NoopAcpPromptProgressSink, ProviderSessionStoreCapture,
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
                self.xcode_shim_grants
                    .iter()
                    .for_each(|grant| grant.set_active_prompt(false));
                if let Some(result) =
                    self.recover_codex_task_complete_after_prompt_error(req, error.to_string())
                {
                    return Ok(result);
                }
                if let Some(result) =
                    self.recover_claude_task_complete_after_prompt_error(req, error.to_string())
                {
                    return Ok(result);
                }
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
        if let AcpSessionCloseBehavior::ArchiveFailure(context) = &behavior {
            let archive_result = (|| -> Result<()> {
                if outcome.provider_session_store_capture.is_none() {
                    outcome.provider_session_store_capture =
                        stage_external_provider_session_store(context)?;
                }
                if let Some(receipt) = self.transport.runtime_receipt() {
                    outcome.provider_session_store_capture = Some(stage_runtime_receipt_capture(
                        outcome.provider_session_store_capture.take(),
                        context,
                        receipt,
                    )?);
                }
                if let Some(capture) = outcome.provider_session_store_capture.as_ref() {
                    finalize_provider_session_store_capture(capture, true, context)?;
                }
                Ok(())
            })();
            if let Err(error) = archive_result {
                warn!(
                    provider = %context.provider,
                    run_id = %context.run_id,
                    stage_id = %context.stage_id,
                    error = %error,
                    "Failed to archive provider session store for failed ACP session"
                );
            }
            outcome.provider_session_store_capture = None;
        }
        for grant in self.xcode_shim_grants.drain(..) {
            grant.remove();
        }
        Ok(outcome)
    }

    /// Stage provider-native session evidence without closing the transport.
    ///
    /// The engine finalizes this staged copy after output settlement. Successful
    /// executions delete it; failed settlement archives it with run metadata.
    pub fn stage_provider_session_store_for_outcome(
        &self,
        context: &ProviderSessionStoreArchiveContext,
    ) -> Result<Option<ProviderSessionStoreCapture>> {
        let mut capture = None;
        for spec in self.cleanup_paths.iter() {
            match stage_cleanup_spec_for_outcome(spec) {
                Ok(Some(staged)) => capture = Some(staged),
                Ok(None) => {}
                Err(error) => warn!(
                    cleanup_path = %spec.path.display(),
                    error = %error,
                    "Failed to stage ACP session cleanup path for outcome"
                ),
            }
        }
        if capture.is_none() {
            capture = stage_external_provider_session_store(context)?;
        }
        if let Some(receipt) = self.transport.runtime_receipt() {
            capture = Some(stage_runtime_receipt_capture(capture, context, receipt)?);
        }
        Ok(capture)
    }

    pub fn is_live(&mut self) -> bool {
        !self.transport.is_closed() && matches!(self.transport.try_wait(), Ok(None))
    }

    fn recover_codex_task_complete_after_prompt_error(
        &self,
        req: &ExecutionRequest,
        error_message: String,
    ) -> Option<ExecutionResult> {
        if req.provider.trim().to_ascii_lowercase() != "codex" {
            return None;
        }
        let recovery = recover_codex_task_complete_from_cleanup_paths(&self.cleanup_paths, req)?;
        let mcp_observation = self.transport.mcp_observation();
        let actual_mcp_extensions = mcp_observation
            .as_ref()
            .map(|observation| observation.actual_extensions.clone())
            .unwrap_or_default();
        let actual_mcp_runtime_ids = mcp_observation
            .as_ref()
            .map(|observation| observation.actual_runtime_ids.clone())
            .unwrap_or_default();
        let runtime_receipt = recovered_task_complete_runtime_receipt(
            self.transport.runtime_receipt().cloned(),
            &error_message,
        );

        Some(ExecutionResult {
            agent_execution_id: req.agent_execution_id.unwrap_or_else(AgentExecutionId::new),
            status: domain::agent::AgentStatus::Completed,
            artifact_paths: Vec::new(),
            discovered_artifacts: recovery.discovered_artifacts,
            pre_prompt_expected_outputs: Vec::new(),
            transcript_text: Some(recovery.text),
            completion_text_capture: recovery.completion_text_capture,
            cost_cents: None,
            usage: None,
            provider_session_id: Some(self.transport.session_id().to_string()),
            reused_existing_session: false,
            session_generation_id: None,
            mcp_observation,
            actual_mcp_extensions,
            actual_mcp_runtime_ids,
            mcp_session_startup_latency_ms: self.transport.mcp_session_startup_latency_ms(),
            xcode_shim_warning_events: Vec::new(),
            close_diagnostic: None,
            provider_session_store_capture: None,
            acp_pre_initialize_local_latency_ms: Some(
                self.transport.acp_pre_initialize_local_latency_ms(),
            ),
            acp_initialize_latency_ms: Some(self.transport.acp_initialize_latency_ms()),
            acp_session_new_latency_ms: Some(self.transport.acp_session_new_latency_ms()),
            acp_prompt_duration_ms: None,
            acp_pre_prompt_metadata_latency_ms: None,
            acp_pre_prompt_metadata_timeout: false,
            acp_pre_prompt_metadata_digest_bytes: 0,
            legacy_broad_discovery_snapshot: None,
            runtime_receipt,
            runtime_tool_path_preflight_json: None,
        })
    }

    fn recover_claude_task_complete_after_prompt_error(
        &self,
        req: &ExecutionRequest,
        error_message: String,
    ) -> Option<ExecutionResult> {
        if req.provider.trim().to_ascii_lowercase() != "claude" {
            return None;
        }
        let provider_session_id = req
            .provider_session_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| self.transport.session_id().to_string());
        let recovery = recover_claude_task_complete_from_projects_root(
            &claude_projects_root(),
            &provider_session_id,
            &req.expected_outputs,
        )?;
        let mcp_observation = self.transport.mcp_observation();
        let actual_mcp_extensions = mcp_observation
            .as_ref()
            .map(|observation| observation.actual_extensions.clone())
            .unwrap_or_default();
        let actual_mcp_runtime_ids = mcp_observation
            .as_ref()
            .map(|observation| observation.actual_runtime_ids.clone())
            .unwrap_or_default();
        let runtime_receipt = recovered_task_complete_runtime_receipt(
            self.transport.runtime_receipt().cloned(),
            &error_message,
        );

        Some(ExecutionResult {
            agent_execution_id: req.agent_execution_id.unwrap_or_else(AgentExecutionId::new),
            status: domain::agent::AgentStatus::Completed,
            artifact_paths: Vec::new(),
            discovered_artifacts: recovery.discovered_artifacts,
            pre_prompt_expected_outputs: Vec::new(),
            transcript_text: Some(recovery.text),
            completion_text_capture: recovery.completion_text_capture,
            cost_cents: None,
            usage: None,
            provider_session_id: Some(provider_session_id),
            reused_existing_session: false,
            session_generation_id: None,
            mcp_observation,
            actual_mcp_extensions,
            actual_mcp_runtime_ids,
            mcp_session_startup_latency_ms: self.transport.mcp_session_startup_latency_ms(),
            xcode_shim_warning_events: Vec::new(),
            close_diagnostic: None,
            provider_session_store_capture: None,
            acp_pre_initialize_local_latency_ms: Some(
                self.transport.acp_pre_initialize_local_latency_ms(),
            ),
            acp_initialize_latency_ms: Some(self.transport.acp_initialize_latency_ms()),
            acp_session_new_latency_ms: Some(self.transport.acp_session_new_latency_ms()),
            acp_prompt_duration_ms: None,
            acp_pre_prompt_metadata_latency_ms: None,
            acp_pre_prompt_metadata_timeout: false,
            acp_pre_prompt_metadata_digest_bytes: 0,
            legacy_broad_discovery_snapshot: None,
            runtime_receipt,
            runtime_tool_path_preflight_json: None,
        })
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
                let _ = context;
                Ok(capture)
            }
        },
    }
}

fn stage_cleanup_spec_for_outcome(
    spec: &CleanupPathSpec,
) -> Result<Option<ProviderSessionStoreCapture>> {
    match spec.policy {
        CleanupPathPolicy::DeleteRecursively => Ok(None),
        CleanupPathPolicy::StageCodexSessionStore => stage_codex_session_store(&spec.path),
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

fn pending_session_store_root(provider: &str) -> PathBuf {
    app_support_runtime_root()
        .join("pending-session-stores")
        .join(provider)
        .join(Uuid::new_v4().to_string())
}

fn stage_codex_session_store(runtime_home: &Path) -> Result<Option<ProviderSessionStoreCapture>> {
    let mut captured_subdirs = Vec::new();
    let staging_root = pending_session_store_root("codex");

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

fn stage_external_provider_session_store(
    context: &ProviderSessionStoreArchiveContext,
) -> Result<Option<ProviderSessionStoreCapture>> {
    match canonical_provider_for_capture(&context.provider).as_deref() {
        Some("claude") => stage_claude_session_store(context),
        Some("junie") => stage_junie_session_store(context),
        _ => Ok(None),
    }
}

fn canonical_provider_for_capture(provider: &str) -> Option<&'static str> {
    let provider = provider.trim().to_ascii_lowercase();
    match provider.as_str() {
        "codex" => Some("codex"),
        "claude" | "claude-code" | "claude_code" => Some("claude"),
        "junie" => Some("junie"),
        _ => None,
    }
}

fn stage_claude_session_store(
    context: &ProviderSessionStoreArchiveContext,
) -> Result<Option<ProviderSessionStoreCapture>> {
    let Some(provider_session_id) = non_empty(context.provider_session_id.as_deref()) else {
        return Ok(None);
    };
    let projects_root = claude_projects_root();
    if !projects_root.exists() {
        return Ok(None);
    }
    let file_name = format!("{provider_session_id}.jsonl");
    let Some(source) = find_file_named(&projects_root, &file_name)? else {
        return Ok(None);
    };
    let relative = source.strip_prefix(&projects_root).unwrap_or(&source);
    let staging_root = pending_session_store_root("claude");
    let dest = staging_root.join("projects").join(relative);
    copy_file(&source, &dest)
        .with_context(|| format!("copy Claude provider transcript {}", source.display()))?;
    Ok(Some(ProviderSessionStoreCapture {
        provider: "claude".to_string(),
        staging_root: staging_root.to_string_lossy().into_owned(),
        captured_subdirs: vec![format!("projects/{}", relative.to_string_lossy())],
    }))
}

fn stage_junie_session_store(
    context: &ProviderSessionStoreArchiveContext,
) -> Result<Option<ProviderSessionStoreCapture>> {
    let Some(provider_session_id) = non_empty(context.provider_session_id.as_deref()) else {
        return Ok(None);
    };
    let sessions_root = junie_sessions_root();
    if !sessions_root.exists() {
        return Ok(None);
    }
    let source = sessions_root.join(provider_session_id);
    let source = if source.exists() {
        Some(source)
    } else {
        find_dir_named(&sessions_root, provider_session_id)?
    };
    let Some(source) = source else {
        return Ok(None);
    };
    let relative = source.strip_prefix(&sessions_root).unwrap_or(&source);
    let staging_root = pending_session_store_root("junie");
    let dest = staging_root.join("sessions").join(relative);
    copy_dir_recursive(&source, &dest)
        .with_context(|| format!("copy Junie provider session {}", source.display()))?;
    Ok(Some(ProviderSessionStoreCapture {
        provider: "junie".to_string(),
        staging_root: staging_root.to_string_lossy().into_owned(),
        captured_subdirs: vec![format!("sessions/{}", relative.to_string_lossy())],
    }))
}

fn stage_runtime_receipt_capture(
    capture: Option<ProviderSessionStoreCapture>,
    context: &ProviderSessionStoreArchiveContext,
    receipt: &AcpRuntimeReceipt,
) -> Result<ProviderSessionStoreCapture> {
    let provider = provider_slug_for_capture(&context.provider);
    let mut capture = capture.unwrap_or_else(|| ProviderSessionStoreCapture {
        provider: provider.clone(),
        staging_root: pending_session_store_root(&provider)
            .to_string_lossy()
            .into_owned(),
        captured_subdirs: Vec::new(),
    });
    let path = Path::new(&capture.staging_root).join("acp-runtime-receipt.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create runtime receipt parent {}", parent.display()))?;
    }
    fs::write(&path, serde_json::to_vec_pretty(receipt)?)
        .with_context(|| format!("write runtime receipt {}", path.display()))?;
    if !capture
        .captured_subdirs
        .iter()
        .any(|entry| entry == "acp-runtime-receipt.json")
    {
        capture
            .captured_subdirs
            .push("acp-runtime-receipt.json".to_string());
    }
    Ok(capture)
}

fn provider_slug_for_capture(provider: &str) -> String {
    if let Some(canonical) = canonical_provider_for_capture(provider) {
        return canonical.to_string();
    }
    let slug: String = provider
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    if slug.is_empty() {
        "acp".to_string()
    } else {
        slug
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

const PROVIDER_SESSION_STORE_RECOVERY_SCAN_LIMIT: usize = 20_000;
const PROVIDER_SESSION_STORE_RECOVERY_LINE_CAP_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
struct ProviderSessionStoreTaskCompleteRecovery {
    text: String,
    discovered_artifacts: Vec<crate::DiscoveredArtifact>,
    completion_text_capture: crate::AcpCompletionTextCaptureMetadata,
}

fn recover_codex_task_complete_from_cleanup_paths(
    cleanup_paths: &[CleanupPathSpec],
    req: &ExecutionRequest,
) -> Option<ProviderSessionStoreTaskCompleteRecovery> {
    cleanup_paths
        .iter()
        .filter(|spec| spec.policy == CleanupPathPolicy::StageCodexSessionStore)
        .find_map(|spec| recover_codex_task_complete_from_runtime_home(&spec.path, req))
}

fn recover_codex_task_complete_from_runtime_home(
    runtime_home: &Path,
    req: &ExecutionRequest,
) -> Option<ProviderSessionStoreTaskCompleteRecovery> {
    let text = latest_codex_task_complete_text(runtime_home)?;
    let discovered_artifacts =
        crate::transport::extract_output_envelopes(&text, &req.expected_outputs);
    if discovered_artifacts.is_empty() {
        return None;
    }
    let completion_text_capture = crate::transport::recovered_completion_text_capture_metadata(
        &text,
        AcpCompletionCaptureSource::ProviderSessionStoreTaskComplete,
    );
    Some(ProviderSessionStoreTaskCompleteRecovery {
        text,
        discovered_artifacts,
        completion_text_capture,
    })
}

#[cfg_attr(not(test), allow(dead_code))]
fn recover_claude_task_complete_from_projects_root(
    projects_root: &Path,
    provider_session_id: &str,
    expected_outputs: &[domain::discovery::ExpectedOutputSpec],
) -> Option<ProviderSessionStoreTaskCompleteRecovery> {
    let text = latest_claude_task_complete_text(projects_root, provider_session_id)?;
    let discovered_artifacts = crate::transport::extract_output_envelopes(&text, expected_outputs);
    if discovered_artifacts.is_empty() {
        return None;
    }
    let completion_text_capture = crate::transport::recovered_completion_text_capture_metadata(
        &text,
        AcpCompletionCaptureSource::ProviderSessionStoreFinalResponse,
    );
    Some(ProviderSessionStoreTaskCompleteRecovery {
        text,
        discovered_artifacts,
        completion_text_capture,
    })
}

fn latest_codex_task_complete_text(runtime_home: &Path) -> Option<String> {
    let mut latest = None;
    for root_name in ["sessions", "archived_sessions"] {
        let root = runtime_home.join(root_name);
        if !root.exists() {
            continue;
        }
        for file in jsonl_files_under(&root) {
            latest = latest_codex_task_complete_text_in_file(&file).or(latest);
        }
    }
    latest
}

#[cfg_attr(not(test), allow(dead_code))]
fn latest_claude_task_complete_text(
    projects_root: &Path,
    provider_session_id: &str,
) -> Option<String> {
    let provider_session_id = provider_session_id.trim();
    if provider_session_id.is_empty() || !projects_root.exists() {
        return None;
    }
    let expected_file_name = format!("{provider_session_id}.jsonl");
    let mut latest = None;
    for file in jsonl_files_under(projects_root) {
        if file.file_name().and_then(|name| name.to_str()) != Some(expected_file_name.as_str()) {
            continue;
        }
        latest = latest_claude_task_complete_text_in_file(&file).or(latest);
    }
    latest
}

fn jsonl_files_under(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0usize;
    while let Some(path) = stack.pop() {
        visited += 1;
        if visited > PROVIDER_SESSION_STORE_RECOVERY_SCAN_LIMIT {
            warn!(
                root = %root.display(),
                "Provider task-complete recovery stopped after scan limit"
            );
            break;
        }
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file()
                && path.extension().and_then(|ext| ext.to_str()) == Some("jsonl")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn latest_codex_task_complete_text_in_file(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    let mut latest = None;
    for line in reader.lines() {
        let Ok(line) = line else {
            continue;
        };
        if line.len() > PROVIDER_SESSION_STORE_RECOVERY_LINE_CAP_BYTES
            || !line.contains("CHAINWORKS_OUTPUT")
        {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(text) = codex_session_line_agent_message(&value) {
            latest = Some(text.to_string());
        }
    }
    latest
}

#[cfg_attr(not(test), allow(dead_code))]
fn latest_claude_task_complete_text_in_file(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    let mut latest = None;
    for line in reader.lines() {
        let Ok(line) = line else {
            continue;
        };
        if line.len() > PROVIDER_SESSION_STORE_RECOVERY_LINE_CAP_BYTES
            || !line.contains("CHAINWORKS_OUTPUT")
        {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(text) = claude_session_line_assistant_terminal_text(&value) {
            latest = Some(text);
        }
    }
    latest
}

fn codex_session_line_agent_message(value: &serde_json::Value) -> Option<&str> {
    let payload = value.get("payload")?;
    match (
        value.get("type").and_then(serde_json::Value::as_str),
        payload.get("type").and_then(serde_json::Value::as_str),
    ) {
        (Some("event_msg"), Some("task_complete")) => payload
            .get("last_agent_message")
            .and_then(serde_json::Value::as_str),
        (Some("event_msg"), Some("agent_message")) => {
            payload.get("message").and_then(serde_json::Value::as_str)
        }
        _ => None,
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn claude_session_line_assistant_terminal_text(value: &serde_json::Value) -> Option<String> {
    if value.get("type").and_then(serde_json::Value::as_str) != Some("assistant") {
        return None;
    }
    let message = value.get("message");
    let stop_reason = message
        .and_then(|message| message.get("stop_reason"))
        .or_else(|| value.get("stop_reason"));
    if stop_reason.is_some_and(serde_json::Value::is_null) {
        return None;
    }

    let mut segments = Vec::new();
    if let Some(message) = message {
        collect_claude_text_segments(message, &mut segments);
    }
    collect_claude_text_segments(value, &mut segments);
    segments.retain(|segment| segment.contains("CHAINWORKS_OUTPUT"));
    if segments.is_empty() {
        None
    } else {
        Some(segments.join("\n"))
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn collect_claude_text_segments(value: &serde_json::Value, segments: &mut Vec<String>) {
    if let Some(text) = value.get("text").and_then(serde_json::Value::as_str) {
        segments.push(text.to_string());
    }
    if let Some(text) = value.get("output").and_then(serde_json::Value::as_str) {
        segments.push(text.to_string());
    }
    match value.get("content") {
        Some(serde_json::Value::String(text)) => segments.push(text.clone()),
        Some(serde_json::Value::Array(items)) => {
            for item in items {
                if item
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|kind| kind == "text" || kind == "content")
                {
                    collect_claude_text_segments(item, segments);
                } else if item
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|text| text.contains("CHAINWORKS_OUTPUT"))
                {
                    collect_claude_text_segments(item, segments);
                }
            }
        }
        _ => {}
    }
    if let Some(result) = value.get("result") {
        collect_claude_text_segments(result, segments);
    }
}

fn recovered_task_complete_runtime_receipt(
    receipt: Option<AcpRuntimeReceipt>,
    original_error: &str,
) -> Option<AcpRuntimeReceipt> {
    let mut receipt = receipt?;
    let original_failure_phase = receipt.failure_phase.clone();
    receipt.status = "completed".to_string();
    receipt.failure_phase = None;
    let at_ms = receipt
        .last_events
        .last()
        .map(|event| event.at_ms.saturating_add(1))
        .unwrap_or(0);
    receipt
        .handshake
        .terminal_response_at_ms
        .get_or_insert(at_ms);
    receipt.last_events.push(AcpRuntimeReceiptEvent {
        at_ms,
        kind: "provider_session_store_task_complete_recovered".to_string(),
        detail: Some(format!(
            "original_failure_phase={}; original_error={}",
            original_failure_phase.unwrap_or_else(|| "unknown".to_string()),
            original_error
        )),
    });
    receipt.counters.agent_message_chunk_count =
        receipt.counters.agent_message_chunk_count.saturating_add(1);
    receipt.counters.meaningful_progress_count =
        receipt.counters.meaningful_progress_count.saturating_add(1);
    Some(receipt)
}

fn claude_projects_root() -> PathBuf {
    if let Ok(explicit) = std::env::var("CHAINWORKS_CLAUDE_SESSION_STORE_ROOT") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude")
        .join("projects")
}

fn junie_sessions_root() -> PathBuf {
    if let Ok(explicit) = std::env::var("CHAINWORKS_JUNIE_SESSION_STORE_ROOT") {
        if !explicit.is_empty() {
            return PathBuf::from(explicit);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".junie")
        .join("sessions")
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

fn copy_file(source: &Path, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create destination directory {}", parent.display()))?;
    }
    fs::copy(source, dest)
        .with_context(|| format!("copy file from {} to {}", source.display(), dest.display()))?;
    Ok(())
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

fn find_file_named(root: &Path, file_name: &str) -> Result<Option<PathBuf>> {
    find_entry_named(root, file_name, true)
}

fn find_dir_named(root: &Path, dir_name: &str) -> Result<Option<PathBuf>> {
    find_entry_named(root, dir_name, false)
}

fn find_entry_named(root: &Path, name: &str, want_file: bool) -> Result<Option<PathBuf>> {
    let mut stack = vec![root.to_path_buf()];
    let mut visited = 0usize;
    while let Some(dir) = stack.pop() {
        visited += 1;
        if visited > 20_000 {
            warn!(
                root = %root.display(),
                "Provider session-store search stopped after scan limit"
            );
            return Ok(None);
        }
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                warn!(
                    path = %dir.display(),
                    error = %error,
                    "Skipping unreadable provider session-store directory"
                );
                continue;
            }
        };
        for entry in entries {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let path = entry.path();
            if entry.file_name().to_string_lossy() == name
                && ((want_file && file_type.is_file()) || (!want_file && file_type.is_dir()))
            {
                return Ok(Some(path));
            }
            if file_type.is_dir() {
                stack.push(path);
            }
        }
    }
    Ok(None)
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

    pub async fn stage_provider_session_store_for_outcome(
        &self,
        context: &ProviderSessionStoreArchiveContext,
    ) -> Result<Option<ProviderSessionStoreCapture>> {
        let session = self.inner.lock().await;
        session.stage_provider_session_store_for_outcome(context)
    }

    pub async fn provider_session_id(&self) -> String {
        let session = self.inner.lock().await;
        session.transport.session_id().to_string()
    }

    pub async fn child_pid(&self) -> Option<u32> {
        let session = self.inner.lock().await;
        session.transport.child_pid()
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

    fn archive_context(
        provider: &str,
        provider_session_id: Option<&str>,
    ) -> ProviderSessionStoreArchiveContext {
        ProviderSessionStoreArchiveContext {
            provider: provider.into(),
            run_id: "run-1".into(),
            stage_id: "state_9".into(),
            agent_id: "proposal_implementation_auditor".into(),
            agent_execution_id: Some("agent-exec-1".into()),
            session_generation_id: Some("session-gen-1".into()),
            provider_session_id: provider_session_id.map(str::to_string),
            failure_kind: "provider_timeout".into(),
        }
    }

    fn expected_output(name: &str) -> domain::discovery::ExpectedOutputSpec {
        domain::discovery::ExpectedOutputSpec {
            output_name: name.into(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: format!("/tmp/{name}.json"),
            companion_of: None,
            display_label: name.into(),
            contract_id: Some(format!("{name}_v1")),
            required: true,
            reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
            max_bytes: 128 * 1024,
            aggregate_acceptance_cap_bytes: 256 * 1024,
            authorized_roots: Vec::new(),
            source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
        }
    }

    fn runtime_receipt(provider: &str) -> AcpRuntimeReceipt {
        AcpRuntimeReceipt {
            schema_version: 1,
            transport_family: "acp".into(),
            provider: provider.into(),
            model: None,
            provider_session_id: Some("provider-session-1".into()),
            session_generation_id: Some("session-gen-1".into()),
            status: "failed".into(),
            failure_phase: Some("read_poll_elapsed_without_message".into()),
            jsonrpc_error_code: None,
            provider_error_message_redacted: None,
            started_at: "2026-05-16T00:00:00Z".into(),
            completed_at: Some("2026-05-16T00:05:00Z".into()),
            xcode_shim_injected: false,
            requires_xcode_host_execution: false,
            handshake: Default::default(),
            counters: Default::default(),
            permission_roundtrips: Vec::new(),
            first_events: Vec::new(),
            last_events: Vec::new(),
            p079_unsafe_continuation: false,
        }
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
    fn codex_task_complete_recovery_extracts_last_agent_message_outputs() {
        let temp = tempdir().unwrap();
        let runtime_home = temp.path().join("runtime-home");
        let output_path = runtime_home.join("audit/proposal-vs-implementation.json");
        let output_path_string = output_path.to_string_lossy().into_owned();
        let session_line = serde_json::json!({
            "timestamp": "2026-05-31T08:30:25.238Z",
            "type": "event_msg",
            "payload": {
                "type": "task_complete",
                "turn_id": "turn-1",
                "last_agent_message": serde_json::json!({
                    "CHAINWORKS_OUTPUT": {
                        output_path_string.as_str(): {
                            "status": "needs_code_fixes",
                            "matches_proposal": false,
                            "missing_items": []
                        }
                    }
                }).to_string()
            }
        });
        write_file(
            &runtime_home.join("sessions/2026/05/31/rollout-2026-05-31.jsonl"),
            &format!("{session_line}\n"),
        );
        let req = ExecutionRequest {
            run_id: domain::ids::RunId::new(),
            stage_execution_id: None,
            stage_id: "state_9_implementation_reviewed".into(),
            attempt_number: 1,
            agent_execution_id: None,
            agent_id: "proposal_implementation_auditor".into(),
            provider: "codex".into(),
            model: Some("gpt-5.5".into()),
            effort: None,
            workspace_root: temp.path().to_string_lossy().into_owned(),
            prompt: "audit".into(),
            worktree_root: None,
            worktree_write_enabled: false,
            worktree_strategy: None,
            expected_output_paths: Vec::new(),
            expected_outputs: vec![domain::discovery::ExpectedOutputSpec {
                output_name: "audit_report".into(),
                output_role: domain::discovery::ExpectedOutputRole::Machine,
                target_path: output_path_string.clone(),
                companion_of: None,
                display_label: "Audit report".into(),
                contract_id: Some("audit_report_v1".into()),
                required: true,
                reuse_policy: domain::discovery::OutputReusePolicy::MustProduce,
                max_bytes: 128 * 1024,
                aggregate_acceptance_cap_bytes: 256 * 1024,
                authorized_roots: Vec::new(),
                source_generation_owner: domain::discovery::SourceGenerationOwner::Agent,
            }],
            keep_session_alive: false,
            reuse_existing_session: false,
            session_generation_id: None,
            provider_session_id: None,
            provider_runtime_home: None,
            mcp_servers: Vec::new(),
            chainworks_meta_root: None,
            legacy_broad_discovery_policy: domain::discovery::LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            owner_kind: "stage_execution".to_string(),
            owner_id: None,
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: None,
            toolchain_go_scope_enabled: false,
        };

        let recovery = recover_codex_task_complete_from_runtime_home(&runtime_home, &req)
            .expect("Codex task_complete recovery expected");

        assert_eq!(recovery.discovered_artifacts.len(), 1);
        assert_eq!(recovery.discovered_artifacts[0].name, output_path_string);
        assert_eq!(
            recovery.completion_text_capture.capture_source,
            Some(AcpCompletionCaptureSource::ProviderSessionStoreTaskComplete)
        );
        assert!(recovery.text.contains("\"CHAINWORKS_OUTPUT\""));
    }

    #[test]
    fn claude_session_store_recovers_latest_transcript_output_for_provider_session_id() {
        let temp = tempdir().unwrap();
        let projects_root = temp.path().join("claude-projects");
        let expected_outputs = vec![expected_output("implementation_report")];
        let older_line = serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "msg-older",
                "stop_reason": "end_turn",
                "content": [{
                    "type": "text",
                    "text": r#"{"CHAINWORKS_OUTPUT":{"implementation_report":{"status":"stale"}}}"#
                }]
            }
        });
        let latest_line = serde_json::json!({
            "type": "assistant",
            "message": {
                "id": "msg-latest",
                "stop_reason": "end_turn",
                "content": [{
                    "type": "text",
                    "text": r#"{"CHAINWORKS_OUTPUT":{"implementation_report":{"status":"ready"}}}"#
                }]
            }
        });
        write_file(
            &projects_root
                .join("-workspace")
                .join("provider-session-1.jsonl"),
            &format!("{older_line}\n{latest_line}\n"),
        );

        let recovery = recover_claude_task_complete_from_projects_root(
            &projects_root,
            "provider-session-1",
            &expected_outputs,
        )
        .expect("Claude recovery expected");

        assert_eq!(recovery.discovered_artifacts.len(), 1);
        assert_eq!(
            recovery.discovered_artifacts[0].name,
            "implementation_report"
        );
        assert!(
            std::str::from_utf8(&recovery.discovered_artifacts[0].content)
                .unwrap()
                .contains("\"ready\"")
        );
        assert!(!recovery.text.contains("stale"));
        assert_eq!(
            recovery.completion_text_capture.capture_source,
            Some(AcpCompletionCaptureSource::ProviderSessionStoreFinalResponse)
        );
    }

    #[test]
    fn claude_session_store_ignores_transcript_for_different_provider_session_id() {
        let temp = tempdir().unwrap();
        let projects_root = temp.path().join("claude-projects");
        let expected_outputs = vec![expected_output("implementation_report")];
        let line = serde_json::json!({
            "type": "assistant",
            "message": {
                "stop_reason": "end_turn",
                "content": [{
                    "type": "text",
                    "text": r#"{"CHAINWORKS_OUTPUT":{"implementation_report":{"status":"ready"}}}"#
                }]
            }
        });
        write_file(
            &projects_root
                .join("-workspace")
                .join("other-provider-session.jsonl"),
            &format!("{line}\n"),
        );

        let recovery = recover_claude_task_complete_from_projects_root(
            &projects_root,
            "provider-session-1",
            &expected_outputs,
        );

        assert!(recovery.is_none());
    }

    #[test]
    fn claude_session_store_returns_none_without_chainworks_output() {
        let temp = tempdir().unwrap();
        let projects_root = temp.path().join("claude-projects");
        let expected_outputs = vec![expected_output("implementation_report")];
        let line = serde_json::json!({
            "type": "assistant",
            "message": {
                "stop_reason": "end_turn",
                "content": [{
                    "type": "text",
                    "text": "Finished without a structured output envelope."
                }]
            }
        });
        write_file(
            &projects_root
                .join("-workspace")
                .join("provider-session-1.jsonl"),
            &format!("{line}\n"),
        );

        let recovery = recover_claude_task_complete_from_projects_root(
            &projects_root,
            "provider-session-1",
            &expected_outputs,
        );

        assert!(recovery.is_none());
    }

    #[test]
    fn stage_claude_session_store_copies_native_transcript_by_provider_session_id() {
        let _guard = SESSION_STORE_ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        std::env::set_var(
            "CHAINWORKS_SESSION_STORE_ROOT",
            temp.path()
                .join("pending-root")
                .to_string_lossy()
                .to_string(),
        );
        std::env::set_var(
            "CHAINWORKS_CLAUDE_SESSION_STORE_ROOT",
            temp.path()
                .join("claude-projects")
                .to_string_lossy()
                .to_string(),
        );
        let transcript = temp
            .path()
            .join("claude-projects")
            .join("-workspace")
            .join("provider-session-1.jsonl");
        write_file(&transcript, "{\"type\":\"assistant\"}\n");

        let capture = stage_external_provider_session_store(&archive_context(
            "claude",
            Some("provider-session-1"),
        ))
        .unwrap()
        .expect("capture expected");

        assert_eq!(capture.provider, "claude");
        assert!(Path::new(&capture.staging_root)
            .join("projects")
            .join("-workspace")
            .join("provider-session-1.jsonl")
            .exists());
        std::env::remove_var("CHAINWORKS_CLAUDE_SESSION_STORE_ROOT");
        std::env::remove_var("CHAINWORKS_SESSION_STORE_ROOT");
    }

    #[test]
    fn stage_junie_session_store_copies_native_session_directory() {
        let _guard = SESSION_STORE_ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        std::env::set_var(
            "CHAINWORKS_SESSION_STORE_ROOT",
            temp.path()
                .join("pending-root")
                .to_string_lossy()
                .to_string(),
        );
        std::env::set_var(
            "CHAINWORKS_JUNIE_SESSION_STORE_ROOT",
            temp.path()
                .join("junie-sessions")
                .to_string_lossy()
                .to_string(),
        );
        write_file(
            &temp
                .path()
                .join("junie-sessions")
                .join("session-1")
                .join("events.jsonl"),
            "{\"kind\":\"AgentMessageUpdatedEvent\"}\n",
        );

        let capture =
            stage_external_provider_session_store(&archive_context("junie", Some("session-1")))
                .unwrap()
                .expect("capture expected");

        assert_eq!(capture.provider, "junie");
        assert!(Path::new(&capture.staging_root)
            .join("sessions")
            .join("session-1")
            .join("events.jsonl")
            .exists());
        std::env::remove_var("CHAINWORKS_JUNIE_SESSION_STORE_ROOT");
        std::env::remove_var("CHAINWORKS_SESSION_STORE_ROOT");
    }

    #[test]
    fn stage_runtime_receipt_capture_creates_fallback_when_provider_store_missing() {
        let _guard = SESSION_STORE_ENV_LOCK.lock().unwrap();
        let temp = tempdir().unwrap();
        std::env::set_var(
            "CHAINWORKS_SESSION_STORE_ROOT",
            temp.path()
                .join("pending-root")
                .to_string_lossy()
                .to_string(),
        );

        let capture = stage_runtime_receipt_capture(
            None,
            &archive_context("gemini", Some("provider-session-1")),
            &runtime_receipt("gemini"),
        )
        .unwrap();

        assert_eq!(capture.provider, "gemini");
        assert!(Path::new(&capture.staging_root)
            .join("acp-runtime-receipt.json")
            .exists());
        assert!(capture
            .captured_subdirs
            .contains(&"acp-runtime-receipt.json".to_string()));
        std::env::remove_var("CHAINWORKS_SESSION_STORE_ROOT");
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
