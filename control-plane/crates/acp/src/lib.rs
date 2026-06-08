pub mod adapters;
pub mod manager;
pub mod session;
pub mod toolchain_lease;
pub mod toolchain_mapper;
pub mod transport;
pub mod xcode_broker;
pub mod xcode_shim;
pub mod xcode_target;

pub use manager::{
    AcpLiveSessionProcessBinding, AcpRuntimeManager, BrokeredXcodeLeaseAttachment,
    XcodeBrokerLeaseAttacher,
};
pub use session::{AcpSession, AcpSessionHandle};
pub use xcode_broker::{
    BrokerMcpPolicy, BrokerMcpPolicyDecision, XcodeBrokerHealthSnapshot, XcodeBrokerHealthState,
    XcodeBrokerHttpRouteState, XcodeMcpBackend, XcodeMcpBackendRequestContext, XcodeMcpBridgePool,
    XcodeMcpBridgePoolConfig, XcodeMcpLeaseState, XcodeMcpProcessBackend,
    XcodeMcpProcessBackendConfig,
};
#[cfg(unix)]
pub use xcode_shim::{
    current_process_uid, handle_xcode_shim_unix_stream,
    handle_xcode_shim_unix_stream_with_grant_resolver,
    handle_xcode_shim_unix_stream_with_peer_credentials, inspect_xcode_shim_process_binding,
    xcode_shim_peer_credentials, DefaultXcodeShimProcessInspector, XcodeShimGrantResolver,
    XcodeShimPeerCredentials, XcodeShimProcessInspector, XcodeShimResolvedDispatch,
};
pub use xcode_shim::{
    dispatch_xcode_shim_request, dispatch_xcode_shim_socket_request, XcodeHostExecutorPlan,
    XcodeHostExecutorPlanError, XcodeHostExecutorPlanInput, XcodeHostExecutorProcessConfig,
    XcodeHostExecutorProcessOutput, XcodeHostExecutorSimulatorCandidate, XcodeShimCommandPolicy,
    XcodeShimDispatchAttempt, XcodeShimDispatchAuthorization, XcodeShimDispatchGrant,
    XcodeShimDispatchOutcome, XcodeShimDispatchRequest, XcodeShimGrantRecord, XcodeShimGrantStore,
    XcodeShimProcessBinding, XcodeShimRouteDecision, XcodeShimSocketDispatchRequest,
};
pub use xcode_target::{
    probe_local_xcode_host, target_resolver_failure_class, HostProbeContext,
    LocalXcodeHostProbeConfig, XcodeProcessCandidate, XcodeTargetResolver,
    XcodeTargetSelectionConfidence, XcodeTargetSelectionInput, XcodeTargetSnapshot,
};

use std::collections::BTreeMap;

use anyhow::Result;
use domain::agent::AgentStatus;
use domain::discovery::{
    ExpectedOutputSpec, LegacyBroadDiscoveryPolicy, LegacyBroadDiscoverySnapshot,
    PrePromptExpectedOutputMetadata,
};
use domain::ids::{AgentExecutionId, RunId};
use domain::xcode_runtime::{XcodeRuntimeObservationUpdate, XcodeShimWarningEvent};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionRequest {
    /// Engine-persisted execution id, when this request is owned by a durable
    /// agent_executions row. ACP uses this only for runtime observation sinks.
    #[serde(default)]
    pub agent_execution_id: Option<AgentExecutionId>,
    pub run_id: RunId,
    /// Durable stage execution UUID when this request originates from the
    /// orchestrator. Adapter-level tests and legacy serialized requests fall
    /// back to `stage_id`.
    #[serde(default)]
    pub stage_execution_id: Option<String>,
    pub stage_id: String,
    /// Stage execution attempt number. P053 pre-prompt discovery metadata uses
    /// this to keep retry baselines distinct from earlier attempts.
    #[serde(default = "default_attempt_number")]
    pub attempt_number: u32,
    pub agent_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub workspace_root: String,
    pub prompt: String,
    /// Provisioned worktree root path (Proposal 007). When set and
    /// `worktree_write_enabled` is true, the ACP session uses this as cwd.
    #[serde(default)]
    pub worktree_root: Option<String>,
    /// Whether the agent has write access to the worktree.
    #[serde(default)]
    pub worktree_write_enabled: bool,
    /// Worktree strategy from catalog (e.g. "dedicated", "shared_implementation_worktree").
    #[serde(default)]
    pub worktree_strategy: Option<String>,
    /// Canonical output paths declared by the compiled workflow task.
    /// These must be reported back even when the agent overwrites an existing
    /// file instead of creating a brand-new one.
    #[serde(default)]
    pub expected_output_paths: Vec<String>,
    /// Typed P053 discovery specs. New runtime paths consume this when present;
    /// `expected_output_paths` remains a compatibility projection for older
    /// adapters and prompt rendering.
    #[serde(default)]
    pub expected_outputs: Vec<ExpectedOutputSpec>,
    /// Keep the ACP transport-backed session alive after the first prompt so
    /// a later turn can reuse it via `session/prompt`.
    #[serde(default)]
    pub keep_session_alive: bool,
    /// Prefer routing this turn through an already-open session handle.
    #[serde(default)]
    pub reuse_existing_session: bool,
    /// Existing live-session generation handle to reuse. When present, the
    /// runtime manager routes the prompt to the already-open session instead
    /// of spawning a new `session/new` flow.
    #[serde(default)]
    pub session_generation_id: Option<String>,
    /// Provider-native session id from the transport layer. This is mainly
    /// useful for tracing and for proving the bridge between generation ids
    /// and the underlying ACP session handle.
    #[serde(default)]
    pub provider_session_id: Option<String>,
    /// Executable MCP server payloads resolved from Chainworks extension ids
    /// into provider-local runtime ids before ACP startup.
    #[serde(default)]
    pub mcp_servers: Vec<AcpMcpServerPayload>,
    /// P050: Per-run meta root for workspace isolation.
    /// Set as `CHAINWORKS_META_ROOT` env var on ACP subprocess so
    /// YAML artifact path templates resolve to the per-run directory.
    #[serde(default)]
    pub chainworks_meta_root: Option<String>,
    /// P053 compatibility escape hatch. Broad workspace/worktree diffing is
    /// disabled unless the frozen run plan or audited retry override enables it.
    #[serde(default)]
    pub legacy_broad_discovery_policy: LegacyBroadDiscoveryPolicy,
    /// P051 direct Xcode command guard. When true, the runtime injects
    /// session-scoped PATH shims and dispatch credentials into the provider.
    #[serde(default)]
    pub xcode_shim_injection_signal: bool,
    /// P051 host execution policy bit. Treated as requiring shim credentials
    /// and included in session reuse/fingerprint decisions by the engine.
    #[serde(default)]
    pub requires_xcode_host_execution: bool,
    /// P017 Phase B: Execution owner discriminator. For stage-owned executions
    /// this is "stage_execution"; for mediation-owned it is "lead_conflict_mediation".
    #[serde(default = "default_owner_kind")]
    pub owner_kind: String,
    /// P017 Phase B: Durable owner identifier. For stage-owned executions this
    /// mirrors stage_execution_id; for mediation-owned it is the mediation record id.
    #[serde(default)]
    pub owner_id: Option<String>,
    /// P017 Phase B: Origin stage id for compatibility context. Present for
    /// mediation-owned executions to aid prompt rendering and correlation.
    #[serde(default)]
    pub origin_stage_id: Option<String>,
    /// P017 Phase B: Origin stage execution id for compatibility context.
    #[serde(default)]
    pub origin_stage_execution_id: Option<String>,
    /// P017 Phase B: Mediation record id when this execution is mediation-owned.
    #[serde(default)]
    pub mediation_record_id: Option<String>,
    /// P066 T20: TOOLCHAIN_HOME path for session-scoped Go mapping setup.
    /// When set with toolchain_go_scope_enabled=true, the manager prepares Go
    /// isolation directories before session handoff and registers them for
    /// cleanup on session close or failure (DiagCleanupPlan::DeleteOnClose).
    #[serde(default)]
    pub toolchain_home: Option<String>,
    /// P066 T20: Enables Go session-scoped toolchain isolation for this session.
    /// Requires toolchain_home + session_generation_id to be set.
    #[serde(default)]
    pub toolchain_go_scope_enabled: bool,
    /// P079: When set, this is a contract-repair turn. The transport applies the
    /// P079 repair permission posture: grant only `fs.write` requests whose target
    /// byte-matches a path in this set; deny everything else (unsafe_continuation).
    #[serde(default)]
    pub p079_repair_canonical_paths: Option<Vec<String>>,
}

fn default_owner_kind() -> String {
    "stage_execution".to_string()
}

fn default_attempt_number() -> u32 {
    1
}

impl ExecutionRequest {
    pub fn brokered_xcode_intents(&self) -> Vec<&BrokeredXcodeMcpIntent> {
        self.mcp_servers
            .iter()
            .filter_map(|server| match &server.transport {
                ResolvedMcpServerTransport::XcodeBrokerIntent { intent }
                    if intent.provider_http_required =>
                {
                    Some(intent)
                }
                _ => None,
            })
            .collect()
    }
}

#[async_trait::async_trait]
pub trait XcodeRuntimeObservationSink: Send + Sync {
    async fn append_xcode_runtime_observation(
        &self,
        agent_execution_id: AgentExecutionId,
        update: XcodeRuntimeObservationUpdate,
    ) -> Result<()>;
}

pub struct NoopXcodeRuntimeObservationSink;

#[async_trait::async_trait]
impl XcodeRuntimeObservationSink for NoopXcodeRuntimeObservationSink {
    async fn append_xcode_runtime_observation(
        &self,
        _agent_execution_id: AgentExecutionId,
        _update: XcodeRuntimeObservationUpdate,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcpPromptProgressKind {
    PromptSent,
    MessageReceived,
    MeaningfulProgress,
    ProviderLocalActivity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcpPromptProgressUpdate {
    pub run_id: RunId,
    pub stage_execution_id: Option<String>,
    pub stage_id: String,
    pub agent_id: String,
    pub provider: String,
    pub session_generation_id: Option<String>,
    pub provider_session_id: String,
    pub kind: AcpPromptProgressKind,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub surface_label: Option<String>,
}

#[async_trait::async_trait]
pub trait AcpPromptProgressSink: Send + Sync {
    async fn record_acp_prompt_progress(&self, update: AcpPromptProgressUpdate) -> Result<()>;
}

pub struct NoopAcpPromptProgressSink;

#[async_trait::async_trait]
impl AcpPromptProgressSink for NoopAcpPromptProgressSink {
    async fn record_acp_prompt_progress(&self, _update: AcpPromptProgressUpdate) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub agent_execution_id: AgentExecutionId,
    pub status: AgentStatus,
    pub artifact_paths: Vec<String>,
    #[serde(default)]
    pub discovered_artifacts: Vec<DiscoveredArtifact>,
    /// P053 typed metadata captured for declared expected outputs immediately
    /// before this prompt turn is sent to the provider.
    #[serde(default)]
    pub pre_prompt_expected_outputs: Vec<PrePromptExpectedOutputMetadata>,
    /// Sanitized text streamed by the ACP provider during this prompt turn.
    /// This is persisted by the engine as recovery evidence when present.
    #[serde(default)]
    pub transcript_text: Option<String>,
    /// Prompt-level ACP completion text capture used for CHAINWORKS_OUTPUT
    /// extraction and durable diagnostics.
    #[serde(default)]
    pub completion_text_capture: AcpCompletionTextCaptureMetadata,
    pub cost_cents: Option<i64>,
    #[serde(default)]
    pub usage: Option<UsageSnapshot>,
    #[serde(default)]
    pub provider_session_id: Option<String>,
    #[serde(default)]
    pub reused_existing_session: bool,
    /// Live-session generation handle for reusable sessions. Populated when
    /// the manager keeps a session alive or routes a prompt through an
    /// existing live session.
    #[serde(default)]
    pub session_generation_id: Option<String>,
    /// Transport-owned observation of the MCP runtime truth accepted for this
    /// ACP session. When the provider does not return an explicit accepted
    /// server list, this records an explicit predicted-after-success fallback
    /// rather than silently treating predicted truth as observed truth.
    #[serde(default)]
    pub mcp_observation: Option<McpActualObservation>,
    #[serde(default)]
    pub actual_mcp_extensions: Vec<String>,
    #[serde(default)]
    pub actual_mcp_runtime_ids: Vec<String>,
    #[serde(default)]
    pub mcp_session_startup_latency_ms: Option<i64>,
    /// P051 residual Xcode path warnings detected in ACP session/update
    /// notifications during this prompt turn.
    #[serde(default)]
    pub xcode_shim_warning_events: Vec<XcodeShimWarningEvent>,
    /// Nonblocking diagnostics observed while closing a one-shot ACP session
    /// after the provider already returned a prompt result.
    #[serde(default)]
    pub close_diagnostic: Option<AcpCloseDiagnostic>,
    /// Provider session-store directories preserved from an isolated runtime
    /// home before cleanup. The engine either deletes these staged copies on
    /// overall success or archives them durably when settlement later fails.
    #[serde(default)]
    pub provider_session_store_capture: Option<ProviderSessionStoreCapture>,
    #[serde(default)]
    pub acp_pre_initialize_local_latency_ms: Option<u64>,
    #[serde(default)]
    pub acp_initialize_latency_ms: Option<u64>,
    #[serde(default)]
    pub acp_session_new_latency_ms: Option<u64>,
    #[serde(default)]
    pub acp_prompt_duration_ms: Option<u64>,
    #[serde(default)]
    pub acp_pre_prompt_metadata_latency_ms: Option<u64>,
    #[serde(default)]
    pub acp_pre_prompt_metadata_timeout: bool,
    #[serde(default)]
    pub acp_pre_prompt_metadata_digest_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_broad_discovery_snapshot: Option<LegacyBroadDiscoverySnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_receipt: Option<AcpRuntimeReceipt>,
    /// Provider launch preflight diagnostics captured before session spawn.
    /// P090 uses this for Junie tool-path remediation readback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_tool_path_preflight_json: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSessionStoreCapture {
    pub provider: String,
    pub staging_root: String,
    #[serde(default)]
    pub captured_subdirs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpCompletionCaptureStatus {
    Captured,
    Absent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpCompletionCaptureSource {
    TerminalFinalResponse,
    StreamedUpdateTail,
    CappedStream,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcpCompletionAbsenceReason {
    NoTerminalOrStreamText,
    TerminalResponseWithoutText,
    TerminalResponseCaptureTruncatedBeforeOutput,
    ExtractionInputTruncated,
    EmptyAfterSanitization,
    RawCaptureDisabled,
    RedactionFailed,
    StorageWriteFailed,
    RedactedStorageWriteFailed,
    CaptureDisabled,
    CaptureFailed,
    SessionReuseWithoutTerminalCapture,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpCompletionTextCaptureMetadata {
    pub capture_status: AcpCompletionCaptureStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_source: Option<AcpCompletionCaptureSource>,
    #[serde(default, skip)]
    pub captured_text: Option<String>,
    pub raw_byte_limit: u64,
    pub captured_byte_count: u64,
    pub completion_text_truncated: bool,
    pub extraction_input_truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extraction_input_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absence_reason: Option<AcpCompletionAbsenceReason>,
}

impl Default for AcpCompletionTextCaptureMetadata {
    fn default() -> Self {
        Self {
            capture_status: AcpCompletionCaptureStatus::Absent,
            capture_source: None,
            captured_text: None,
            raw_byte_limit: 0,
            captured_byte_count: 0,
            completion_text_truncated: false,
            extraction_input_truncated: false,
            extraction_input_sha256: None,
            absence_reason: Some(AcpCompletionAbsenceReason::NoTerminalOrStreamText),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpCloseDiagnostic {
    #[serde(default)]
    pub transport_error_code: Option<String>,
    #[serde(default)]
    pub provider_exit_status: Option<i64>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpRuntimeReceipt {
    pub schema_version: i64,
    pub transport_family: String,
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider_session_id: Option<String>,
    #[serde(default)]
    pub session_generation_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub failure_phase: Option<String>,
    #[serde(default)]
    pub jsonrpc_error_code: Option<i64>,
    #[serde(default)]
    pub provider_error_message_redacted: Option<String>,
    pub started_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    pub xcode_shim_injected: bool,
    pub requires_xcode_host_execution: bool,
    pub handshake: AcpRuntimeReceiptHandshake,
    pub counters: AcpRuntimeReceiptCounters,
    #[serde(default)]
    pub permission_roundtrips: Vec<AcpRuntimeReceiptPermissionRoundtrip>,
    #[serde(default)]
    pub first_events: Vec<AcpRuntimeReceiptEvent>,
    #[serde(default)]
    pub last_events: Vec<AcpRuntimeReceiptEvent>,
    /// P079-SEC-HIGH-001: set when the repair turn was terminated because
    /// the transport denied a non-canonical permission request. Any outputs
    /// from this turn must be discarded and the repair settled as
    /// `rejected_invalid` with `initial_failure_subtype = unsafe_continuation`.
    #[serde(default)]
    pub p079_unsafe_continuation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AcpRuntimeReceiptHandshake {
    #[serde(default)]
    pub initialize_sent_at_ms: Option<u64>,
    #[serde(default)]
    pub initialize_received_at_ms: Option<u64>,
    #[serde(default)]
    pub session_new_sent_at_ms: Option<u64>,
    #[serde(default)]
    pub session_new_received_at_ms: Option<u64>,
    #[serde(default)]
    pub prompt_sent_at_ms: Option<u64>,
    #[serde(default)]
    pub terminal_response_at_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AcpRuntimeReceiptCounters {
    pub total_messages: i64,
    pub session_update_count: i64,
    pub permission_request_count: i64,
    pub permission_grant_sent_count: i64,
    pub permission_grant_failed_count: i64,
    pub agent_message_chunk_count: i64,
    pub agent_thought_chunk_count: i64,
    pub tool_call_count: i64,
    pub tool_call_update_count: i64,
    pub plan_update_count: i64,
    pub meaningful_progress_count: i64,
    pub unknown_notification_count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpRuntimeReceiptEvent {
    pub at_ms: u64,
    pub kind: String,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcpRuntimeReceiptPermissionRoundtrip {
    pub request_id: String,
    pub requested_at_ms: u64,
    #[serde(default)]
    pub request_summary: Option<String>,
    #[serde(default)]
    pub request_payload: Option<String>,
    #[serde(default)]
    pub grant_sent_at_ms: Option<u64>,
    #[serde(default)]
    pub grant_summary: Option<String>,
    #[serde(default)]
    pub grant_payload: Option<String>,
    #[serde(default)]
    pub first_post_grant_event_at_ms: Option<u64>,
    #[serde(default)]
    pub first_post_grant_event_kind: Option<String>,
    #[serde(default)]
    pub first_post_grant_event_detail: Option<String>,
    #[serde(default)]
    pub outcome: Option<String>,
    /// P079-SEC-MED-001: structured decision fields captured at evaluation time.
    /// Only populated when P079 repair posture is active for this session.
    #[serde(default)]
    pub p079_tool_name: Option<String>,
    #[serde(default)]
    pub p079_normalized_path: Option<String>,
    #[serde(default)]
    pub p079_matched_canonical_path: Option<String>,
    #[serde(default)]
    pub p079_decision_reason: Option<String>,
    #[serde(default)]
    pub p079_resource_kind: Option<String>,
}

#[derive(Debug)]
pub struct AcpExecutionError {
    message: String,
    runtime_receipt: Option<AcpRuntimeReceipt>,
}

impl AcpExecutionError {
    pub fn new(message: impl Into<String>, runtime_receipt: Option<AcpRuntimeReceipt>) -> Self {
        Self {
            message: message.into(),
            runtime_receipt,
        }
    }

    pub fn runtime_receipt(&self) -> Option<&AcpRuntimeReceipt> {
        self.runtime_receipt.as_ref()
    }
}

impl std::fmt::Display for AcpExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AcpExecutionError {}

pub fn runtime_receipt_from_error(error: &anyhow::Error) -> Option<&AcpRuntimeReceipt> {
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<AcpExecutionError>() {
            return error.runtime_receipt();
        }
    }
    None
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AcpMcpServerPayload {
    pub id: String,
    #[serde(rename = "extensionId")]
    pub extension_id: String,
    pub transport: ResolvedMcpServerTransport,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolvedMcpServerTransport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    Platform {
        provider: String,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    XcodeBrokerIntent {
        intent: BrokeredXcodeMcpIntent,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrokeredXcodeMcpIntent {
    pub extension_id: String,
    pub runtime_id: String,
    pub server_id: String,
    #[serde(default)]
    pub workspace_root: Option<String>,
    #[serde(default)]
    pub xcode_pid_selector: Option<String>,
    #[serde(default)]
    pub runtime_profile_id: Option<String>,
    #[serde(default)]
    pub permission_profile_id: Option<String>,
    #[serde(default)]
    pub resolved_tool_allowlist_hash: Option<String>,
    pub provider_http_required: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct McpActualObservation {
    pub source: String,
    pub trust_level: String,
    pub actual_equals_predicted: bool,
    #[serde(default)]
    pub provider_session_id: Option<String>,
    #[serde(default)]
    pub actual_extensions: Vec<String>,
    #[serde(default)]
    pub actual_runtime_ids: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    pub cost_cents: Option<i64>,
    pub input_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub model_context_window: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredArtifact {
    pub name: String,
    pub content: Vec<u8>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_kind: DiscoveredArtifactSourceKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveredArtifactSourceKind {
    #[default]
    ProviderEnvelope,
    ChainworksOutput,
    ExactPath,
}
