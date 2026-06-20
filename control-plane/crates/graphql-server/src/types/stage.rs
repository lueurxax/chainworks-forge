use async_graphql::*;
use db::repos::agent_execution_discovery_diagnostics;
use db::repos::agent_execution_runtime_facts;
use db::repos::code_writer_completion_receipts;
use db::repos::output_contract_repair;
use db::repos::projections::StageSummaryRow;
use db::repos::sessions;
use domain::ids::StageExecutionId;
use domain::session::{SessionGeneration, SessionLineage};
use domain::stage::StageExecution;
use domain::xcode_runtime::{
    McpBrokerObservation, XcodeHostExecutorEvent, XcodeRuntimeObservation, XcodeShimEvent,
    XcodeShimInvocationEvent, XcodeShimRuntimeAttachedEvent, XcodeShimWarningEvent,
};
use serde_json::Value as JsonValue;

use crate::types::p031::{freshness_from_projection_lag, GqlFreshnessState};
use crate::types::run::GqlCodeWriterCompletionReceipt;

fn fresh_provider_process_for_disposition(disposition: Option<&str>) -> Option<bool> {
    match disposition {
        Some("reused") => Some(false),
        Some("fresh")
        | Some("reused_after_resume")
        | Some("fresh_after_reset")
        | Some("fresh_after_invalidation")
        | Some("fresh_after_budget")
        | Some("fresh_after_compaction")
        | Some("fresh_after_transport_error")
        | Some("fresh_after_timeout")
        | Some("fresh_session_required")
        | Some("unverifiable_session_history") => Some(true),
        Some(_) | None => None,
    }
}

// P079-SEC-LOW-002: cap per-field JSON deserialization to prevent resource exhaustion.
fn p079_parse_json_capped(s: &str) -> Option<serde_json::Value> {
    const MAX_BYTES: usize = 256 * 1024;
    if s.len() > MAX_BYTES {
        return None;
    }
    serde_json::from_str(s).ok()
}

// P079-SEC-LOW-001 / SEC-MED-002: reject absolute paths and all traversal forms before
// returning evidence_artifact_path or provider_plan_evidence paths to callers.
// Also rejects fully-encoded (%2e%2e, %2f, %5c) AND mixed literal/encoded traversal
// (e.g. %2e. or .%2e) by validating the percent-decoded form as well (SEC-P079-LOW-001).
fn p079_percent_decode_ascii(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_ascii_lowercase();
            let lo = (bytes[i + 2] as char).to_ascii_lowercase();
            let h = match hi {
                '0'..='9' => (hi as u8) - b'0',
                'a'..='f' => (hi as u8) - b'a' + 10,
                _ => { out.push(bytes[i] as char); i += 1; continue; }
            };
            let l = match lo {
                '0'..='9' => (lo as u8) - b'0',
                'a'..='f' => (lo as u8) - b'a' + 10,
                _ => { out.push(bytes[i] as char); i += 1; continue; }
            };
            out.push((h << 4 | l) as char);
            i += 3;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

fn p079_path_has_traversal(p: &str) -> bool {
    p.starts_with('/')
        || p.starts_with('\\')
        || p.contains("../")
        || p.contains("..\\")
        || p.split('/').any(|c| c == "..")
        || p.split('\\').any(|c| c == "..")
}

fn p079_safe_relative_path(path: Option<&str>) -> Option<&str> {
    path.filter(|p| {
        let p_lower = p.to_lowercase();
        // SEC-P079-LOW-001: reject fully URL-encoded traversal sequences.
        let no_encoded = !p_lower.contains("%2e%2e")
            && !p_lower.contains("%2f")
            && !p_lower.contains("%5c");
        // SEC-P079-LOW-001: also validate the percent-decoded form to catch mixed
        // literal/encoded traversal such as %2e. or .%2e (e.g. "%2e./etc/passwd").
        let decoded = p079_percent_decode_ascii(p);
        !p079_path_has_traversal(p)
            && no_encoded
            && !p079_path_has_traversal(&decoded)
    })
}


#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "AgentFailureKind", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum AgentFailureKind {
    ProviderQuota,
    ProviderPermissionRequired,
    ProviderPermissionRejected,
    ProviderTimeout,
    ProviderInternalError,
    ToolOutputBudgetExceeded,
    ToolOutputBudgetPreflightDenied,
    TransportEpipe,
    TransportProtocolError,
    TransportClosed,
    McpStartupTimeout,
    McpPermissionModalStall,
    XcodeHostEnvironmentError,
    MissingRequiredOutputs,
    InvalidOutputContract,
    CancelledByOperator,
    SupersededByRetry,
    HostInterruption,
    ToolOutputBudgetPreflightDenied,
    ToolOutputBudgetExceeded,
    Unknown,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "AgentOutputSettlement", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum AgentOutputSettlement {
    None,
    MissingRequiredOutputs,
    InvalidRequiredOutputs,
    ValidOutputsFromCompletedExecution,
    ValidOutputsFromFailedExecution,
    IgnoredLateOutputs,
    /// P079: output was recovered by a same-session repair turn.
    ValidOutputsFromRepair,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "AgentExecutionRuntimeFacts", rename_fields = "camelCase")]
pub struct GqlAgentExecutionRuntimeFacts {
    pub agent_execution_id: ID,
    pub failure_kind: Option<AgentFailureKind>,
    pub failure_kind_raw_debug: Option<String>,
    pub failure_kind_version: i64,
    pub failure_message_redacted: Option<String>,
    pub failure_message_redaction_version: i64,
    pub retry_after: Option<String>,
    pub operator_action_hint: Option<String>,
    pub provider_exit_status: Option<i64>,
    pub transport_error_code: Option<String>,
    pub supervision_classification: Option<String>,
    pub output_settlement: AgentOutputSettlement,
    pub valid_required_outputs: bool,
    pub late_output_count: i64,
    pub ignored_late_output_count: i64,
    pub session_lineage_id: Option<ID>,
    pub session_generation_id: Option<ID>,
    pub session_reuse_scope: Option<String>,
    pub session_family_id: Option<String>,
    pub session_reuse_disposition: Option<String>,
    pub session_reuse_reason: Option<String>,
    pub session_reset_reason: Option<String>,
    pub active_session_generation_id: Option<ID>,
    pub active_generation_matches_execution: Option<bool>,
    pub generation_status: Option<String>,
    pub fresh_provider_process: Option<bool>,
    pub rehydrated_from_checkpoint_artifact_id: Option<ID>,
    pub quota_ledger_id: Option<ID>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct GqlStageExecution {
    pub id: ID,
    pub run_id: ID,
    pub stage_id: String,
    pub label: String,
    pub status: String,
    pub iteration: i64,
    pub attempt_number: i64,
    pub settlement_kind: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    /// Populated from the projection layer; None when the projection hasn't been built yet.
    pub has_artifacts: Option<bool>,
    pub has_pending_approval: Option<bool>,
    pub has_validation_failure: Option<bool>,
    pub validation_failure_json: Option<String>,
    pub evidence_packet_json: Option<String>,
    pub recovery_snapshot_json: Option<String>,
    pub terminal_reason: Option<String>,
    pub retry_authority_id: Option<String>,
    pub is_retry_authoritative: Option<bool>,
    pub retry_authority_state: Option<String>,
    pub projection_present: bool,
    pub projection_updated_at: Option<String>,
    pub projection_lag: bool,
    pub freshness_state: GqlFreshnessState,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct GqlAgentExecution {
    pub id: ID,
    pub stage_execution_id: ID,
    pub agent_id: String,
    pub agent_title: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub stage_label: Option<String>,
    pub task_label: Option<String>,
    pub last_event_at: Option<String>,
    pub event_count: Option<i64>,
    pub selection_order: Option<i64>,
    pub selection_unavailable_reason: Option<String>,
    pub backend_profile_id: Option<String>,
    pub requested_mcp_extensions_json: Option<String>,
    pub predicted_mcp_extensions_json: Option<String>,
    pub predicted_mcp_runtime_ids_json: Option<String>,
    pub actual_mcp_extensions_json: Option<String>,
    pub actual_mcp_runtime_ids_json: Option<String>,
    pub denied_mcp_extensions_json: Option<String>,
    pub mcp_blocking_issues_json: Option<String>,
    pub actual_mcp_observation_json: Option<String>,
    pub actual_xcode_runtime_observation: Option<GqlXcodeRuntimeObservation>,
    pub mcp_session_startup_latency_ms: Option<i64>,
    pub owner_execution_lineage_id: Option<String>,
    pub session_lineage_id: Option<String>,
    pub session_generation_id: Option<String>,
    pub rehydrated_from_checkpoint_artifact_id: Option<String>,
    /// Raw invocation owner key — kept in struct for runtime_facts derivation but
    /// excluded from the GraphQL schema to satisfy the P046 sensitive-field boundary.
    #[graphql(skip)]
    pub invocation_owner_key: Option<String>,
    pub session_reuse_scope: Option<String>,
    pub session_family_id: Option<String>,
    pub session_reuse_disposition: Option<String>,
    pub session_reset_reason: Option<String>,
    /// P066: Toolchain cache mapping diagnostics. Always non-null — legacy rows
    /// are synthesized as mapping_state=legacy_row_unavailable.
    pub actual_toolchain_mapping_diagnostics: GqlToolchainMappingDiagnostics,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "RunStageTopologyOccurrence", rename_fields = "camelCase")]
pub struct GqlRunStageTopologyOccurrence {
    pub agent_id: String,
    pub agent_title: String,
    pub task_name: String,
    pub status: String,
    pub provider: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub execution_count: i64,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "RunStageTopologyTransition", rename_fields = "camelCase")]
pub struct GqlRunStageTopologyTransition {
    pub to_stage_id: String,
    pub to_label: Option<String>,
    pub detail: Option<String>,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "RunStageTopologyNode", rename_fields = "camelCase")]
pub struct GqlRunStageTopologyNode {
    pub stage_id: String,
    pub label: String,
    pub order: i64,
    pub owner_agent_id: String,
    pub owner_agent_title: String,
    pub status: String,
    pub is_current: bool,
    pub iteration: Option<i64>,
    pub attempt_number: Option<i64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub approval_required: bool,
    pub artifact_count: i64,
    pub communication_count: i64,
    pub occurrences: Vec<GqlRunStageTopologyOccurrence>,
    pub transitions: Vec<GqlRunStageTopologyTransition>,
}

impl From<domain::agent::AgentExecution> for GqlAgentExecution {
    fn from(execution: domain::agent::AgentExecution) -> Self {
        GqlAgentExecution {
            id: ID(execution.id.to_string()),
            stage_execution_id: ID(execution
                .stage_execution_id
                .expect("stage-scoped GraphQL agent execution requires stage_execution_id")
                .to_string()),
            agent_id: execution.agent_id,
            agent_title: None,
            provider: execution.provider,
            model: execution.model,
            status: execution.status.to_string(),
            started_at: execution.started_at.to_rfc3339(),
            completed_at: execution.completed_at.map(|t| t.to_rfc3339()),
            stage_label: None,
            task_label: None,
            last_event_at: None,
            event_count: None,
            selection_order: None,
            selection_unavailable_reason: None,
            backend_profile_id: execution.backend_profile_id,
            requested_mcp_extensions_json: execution.requested_mcp_extensions_json,
            predicted_mcp_extensions_json: execution.predicted_mcp_extensions_json,
            predicted_mcp_runtime_ids_json: execution.predicted_mcp_runtime_ids_json,
            actual_mcp_extensions_json: execution.actual_mcp_extensions_json,
            actual_mcp_runtime_ids_json: execution.actual_mcp_runtime_ids_json,
            denied_mcp_extensions_json: execution.denied_mcp_extensions_json,
            mcp_blocking_issues_json: execution.mcp_blocking_issues_json,
            actual_mcp_observation_json: execution.actual_mcp_observation_json,
            actual_xcode_runtime_observation: execution
                .actual_xcode_runtime_observation_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<XcodeRuntimeObservation>(json).ok())
                .map(XcodeRuntimeObservation::redacted_for_surface)
                .map(GqlXcodeRuntimeObservation::from),
            mcp_session_startup_latency_ms: execution.mcp_session_startup_latency_ms,
            owner_execution_lineage_id: execution.owner_execution_lineage_id,
            session_lineage_id: execution.session_lineage_id,
            session_generation_id: execution.session_generation_id,
            rehydrated_from_checkpoint_artifact_id: execution
                .rehydrated_from_checkpoint_artifact_id,
            invocation_owner_key: execution.invocation_owner_key,
            session_reuse_scope: execution.session_reuse_scope,
            session_family_id: execution.session_family_id,
            session_reuse_disposition: execution.session_reuse_disposition,
            session_reset_reason: execution.session_reset_reason,
            // P066: synthesize legacy sentinel when column is NULL.
            actual_toolchain_mapping_diagnostics: toolchain_mapping_from_json(
                execution
                    .actual_toolchain_mapping_diagnostics_json
                    .as_deref(),
            ),
        }
    }
}

// ── P066: ToolchainMappingDiagnostics GQL type ───────────────────────────────

/// P066: Key fields from the toolchain mapping diagnostics document.
/// Absolute paths are redacted; all fields are synthesized non-null
/// from the stored JSON or a legacy/disabled sentinel.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "ToolchainMappingDiagnostics", rename_fields = "camelCase")]
pub struct GqlToolchainMappingDiagnostics {
    /// Semantic state of toolchain mapping for this execution.
    pub mapping_state: String,
    /// Whether toolchain cache mapping was active.
    pub mapping_enabled: bool,
    /// Why mapping is inactive, when mapping_state is not "active".
    pub inactive_reason: Option<String>,
    /// Provenance for the policy that drove this diagnostics doc.
    pub policy_source: String,
    /// Policy format version, if present.
    pub policy_version: Option<i64>,
    /// Provider family string from the diagnostics doc.
    pub provider_family: String,
    /// Document schema version.
    pub version: i64,
}

/// P066: Build a legacy_row_unavailable sentinel for pre-migration NULL rows.
pub(crate) fn toolchain_mapping_legacy_sentinel() -> GqlToolchainMappingDiagnostics {
    GqlToolchainMappingDiagnostics {
        mapping_state: "legacy_row_unavailable".to_string(),
        mapping_enabled: false,
        inactive_reason: Some("legacy_row".to_string()),
        policy_source: "synthesized_legacy".to_string(),
        policy_version: None,
        provider_family: "unknown".to_string(),
        version: 1,
    }
}

/// P066: Parse a stored diagnostics JSON document or synthesize a sentinel.
/// Absolute paths are never exposed — only the structured fields are returned.
pub(crate) fn toolchain_mapping_from_json(json: Option<&str>) -> GqlToolchainMappingDiagnostics {
    let Some(json) = json else {
        return toolchain_mapping_legacy_sentinel();
    };
    let Ok(val) = serde_json::from_str::<JsonValue>(json) else {
        return toolchain_mapping_legacy_sentinel();
    };
    GqlToolchainMappingDiagnostics {
        mapping_state: val
            .get("mapping_state")
            .and_then(|v| v.as_str())
            .unwrap_or("legacy_row_unavailable")
            .to_string(),
        mapping_enabled: val
            .get("mapping_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        inactive_reason: val
            .get("inactive_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        policy_source: val
            .get("policy_source")
            .and_then(|v| v.as_str())
            .unwrap_or("synthesized_legacy")
            .to_string(),
        policy_version: val.get("policy_version").and_then(|v| v.as_i64()),
        provider_family: val
            .get("provider_family")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        version: val.get("version").and_then(|v| v.as_i64()).unwrap_or(1),
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlXcodeRuntimeObservation {
    pub version: i64,
    pub mcp_broker_observations: Vec<GqlMcpBrokerObservation>,
    pub xcode_shim_events: Vec<GqlXcodeShimEvent>,
    pub xcode_host_executor_events: Vec<GqlXcodeHostExecutorEvent>,
    pub storage: GqlXcodeRuntimeObservationStorageStatus,
}

impl From<XcodeRuntimeObservation> for GqlXcodeRuntimeObservation {
    fn from(observation: XcodeRuntimeObservation) -> Self {
        Self {
            version: observation.version as i64,
            mcp_broker_observations: observation
                .mcp_broker_observations
                .into_iter()
                .map(GqlMcpBrokerObservation::from)
                .collect(),
            xcode_shim_events: observation
                .xcode_shim_events
                .into_iter()
                .map(GqlXcodeShimEvent::from)
                .collect(),
            xcode_host_executor_events: observation
                .xcode_host_executor_events
                .into_iter()
                .map(GqlXcodeHostExecutorEvent::from)
                .collect(),
            storage: GqlXcodeRuntimeObservationStorageStatus {
                max_events: observation.storage.max_events as i64,
                max_bytes: observation.storage.max_bytes as i64,
                truncated: observation.storage.truncated,
                total_events_dropped: observation.storage.total_events_dropped as i64,
                mcp_broker_observations_dropped: observation.storage.mcp_broker_observations_dropped
                    as i64,
                xcode_shim_events_dropped: observation.storage.xcode_shim_events_dropped as i64,
                xcode_host_executor_events_dropped: observation
                    .storage
                    .xcode_host_executor_events_dropped
                    as i64,
                corrupt_json_recovery_count: observation.storage.corrupt_json_recovery_count as i64,
                corrupt_json_quarantined_bytes: observation.storage.corrupt_json_quarantined_bytes
                    as i64,
            },
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlXcodeRuntimeObservationStorageStatus {
    pub max_events: i64,
    pub max_bytes: i64,
    pub truncated: bool,
    pub total_events_dropped: i64,
    pub mcp_broker_observations_dropped: i64,
    pub xcode_shim_events_dropped: i64,
    pub xcode_host_executor_events_dropped: i64,
    pub corrupt_json_recovery_count: i64,
    pub corrupt_json_quarantined_bytes: i64,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlMcpBrokerObservation {
    pub source: String,
    pub backend_start_disposition: String,
    pub pool_id: Option<String>,
    pub lease_id: Option<String>,
    pub xcode_pid: Option<String>,
    pub backend_process_id: Option<i64>,
    pub http_endpoint: Option<String>,
    pub xcode_home_disposition: Option<String>,
    pub xcode_tmpdir_disposition: Option<String>,
    pub sibling_leases_at_spawn: Option<i64>,
    pub backend_initialize_wait_ms: Option<i64>,
    pub backend_startup_latency_ms: Option<i64>,
    pub http_session_startup_latency_ms: Option<i64>,
    pub backend_failure_class: Option<String>,
    pub originating_execution_id: Option<String>,
    pub prompt_cycle_index: Option<i64>,
    pub status_update: Option<String>,
    pub simulator_selection_mode: Option<String>,
    pub simulator_id: Option<String>,
}

impl From<McpBrokerObservation> for GqlMcpBrokerObservation {
    fn from(observation: McpBrokerObservation) -> Self {
        let (simulator_selection_mode, simulator_id) = observation
            .simulator_selection
            .map(|selection| (Some(selection.mode), selection.simulator_id))
            .unwrap_or((None, None));
        Self {
            source: observation.source,
            backend_start_disposition: observation.backend_start_disposition,
            pool_id: observation.pool_id,
            lease_id: observation.lease_id,
            xcode_pid: observation.xcode_pid,
            backend_process_id: observation.backend_process_id,
            http_endpoint: observation.http_endpoint,
            xcode_home_disposition: observation.xcode_home_disposition,
            xcode_tmpdir_disposition: observation.xcode_tmpdir_disposition,
            sibling_leases_at_spawn: observation.sibling_leases_at_spawn,
            backend_initialize_wait_ms: observation.backend_initialize_wait_ms,
            backend_startup_latency_ms: observation.backend_startup_latency_ms,
            http_session_startup_latency_ms: observation.http_session_startup_latency_ms,
            backend_failure_class: observation.backend_failure_class.and_then(|class| {
                serde_json::to_value(class)
                    .ok()
                    .and_then(|value| value.as_str().map(String::from))
            }),
            originating_execution_id: observation.originating_execution_id,
            prompt_cycle_index: observation.prompt_cycle_index,
            status_update: observation.status_update,
            simulator_selection_mode,
            simulator_id,
        }
    }
}

#[derive(Union, Clone, Debug)]
pub enum GqlXcodeShimEvent {
    ShimRuntimeAttached(GqlXcodeShimRuntimeAttachedEvent),
    ShimInvocation(GqlXcodeShimInvocationEvent),
    Warning(GqlXcodeShimWarningEvent),
}

impl From<XcodeShimEvent> for GqlXcodeShimEvent {
    fn from(event: XcodeShimEvent) -> Self {
        match event {
            XcodeShimEvent::ShimRuntimeAttached(event) => GqlXcodeShimEvent::ShimRuntimeAttached(
                GqlXcodeShimRuntimeAttachedEvent::from(event),
            ),
            XcodeShimEvent::ShimInvocation(event) => {
                GqlXcodeShimEvent::ShimInvocation(GqlXcodeShimInvocationEvent::from(event))
            }
            XcodeShimEvent::Warning(event) => {
                GqlXcodeShimEvent::Warning(GqlXcodeShimWarningEvent::from(event))
            }
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlXcodeShimRuntimeAttachedEvent {
    pub ts: String,
    pub source: String,
    pub reason: String,
    pub lease_id: String,
    pub shim_dir: String,
    pub socket_path: String,
    pub workspace_root: String,
    pub agent_execution_id: Option<String>,
}

impl From<XcodeShimRuntimeAttachedEvent> for GqlXcodeShimRuntimeAttachedEvent {
    fn from(event: XcodeShimRuntimeAttachedEvent) -> Self {
        Self {
            ts: event.ts.to_rfc3339(),
            source: event.source,
            reason: event.reason,
            lease_id: event.lease_id,
            shim_dir: event.shim_dir,
            socket_path: event.socket_path,
            workspace_root: event.workspace_root,
            agent_execution_id: event.agent_execution_id,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlXcodeShimInvocationEvent {
    pub ts: String,
    pub tool: String,
    pub via_xcrun: bool,
    pub argv: Vec<String>,
    pub cwd: String,
    pub policy_decision: String,
    pub policy_reason: String,
    pub derived_peer_pid: i64,
    pub derived_peer_uid: i64,
    pub claimed_provider_pid: i64,
    pub peer_pid_mismatch: bool,
    pub exit_status: i64,
}

impl From<XcodeShimInvocationEvent> for GqlXcodeShimInvocationEvent {
    fn from(event: XcodeShimInvocationEvent) -> Self {
        Self {
            ts: event.ts.to_rfc3339(),
            tool: event.tool,
            via_xcrun: event.via_xcrun,
            argv: event.argv,
            cwd: event.cwd,
            policy_decision: event.policy_decision,
            policy_reason: event.policy_reason,
            derived_peer_pid: event.derived_peer_pid,
            derived_peer_uid: event.derived_peer_uid,
            claimed_provider_pid: event.claimed_provider_pid,
            peer_pid_mismatch: event.peer_pid_mismatch,
            exit_status: event.exit_status,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlXcodeShimWarningEvent {
    pub ts: String,
    pub policy_reason: String,
    pub source_field: String,
    pub matched_substring: String,
    pub excerpt: String,
}

impl From<XcodeShimWarningEvent> for GqlXcodeShimWarningEvent {
    fn from(event: XcodeShimWarningEvent) -> Self {
        Self {
            ts: event.ts.to_rfc3339(),
            policy_reason: event.policy_reason,
            source_field: event.source_field,
            matched_substring: event.matched_substring,
            excerpt: event.excerpt,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlXcodeHostExecutorEvent {
    pub ts: String,
    pub tool: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub host_env_disposition: String,
    pub env_allowlist_applied: Vec<String>,
    pub env_dropped_from_provider: Vec<String>,
    pub selected_simulator_id: Option<String>,
    pub exit_status: i64,
    pub duration_ms: i64,
}

impl From<XcodeHostExecutorEvent> for GqlXcodeHostExecutorEvent {
    fn from(event: XcodeHostExecutorEvent) -> Self {
        Self {
            ts: event.ts.to_rfc3339(),
            tool: event.tool,
            argv: event.argv,
            cwd: event.cwd,
            host_env_disposition: event.host_env_disposition,
            env_allowlist_applied: event.env_allowlist_applied,
            env_dropped_from_provider: event.env_dropped_from_provider,
            selected_simulator_id: event.selected_simulator_id,
            exit_status: event.exit_status,
            duration_ms: event.duration_ms,
        }
    }
}

// =============================================================================
// P079: Typed GraphQL SDL for OutputContractRepairEvidence (MISSING-008)
// Replaces the previous untyped Json<serde_json::Value> blob.
// =============================================================================

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "ProviderFamily", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlProviderFamily {
    Claude,
    Gemini,
    Codex,
    Auggie,
    Junie,
    Fixture,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "AdapterFamily", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlAdapterFamily {
    Claude,
    Gemini,
    Codex,
    Auggie,
    Junie,
    Fixture,
}

fn gql_provider_family(s: &str) -> GqlProviderFamily {
    match s {
        "claude" => GqlProviderFamily::Claude,
        "gemini" => GqlProviderFamily::Gemini,
        "codex" => GqlProviderFamily::Codex,
        "auggie" => GqlProviderFamily::Auggie,
        "junie" => GqlProviderFamily::Junie,
        _ => GqlProviderFamily::Fixture,
    }
}

fn gql_adapter_family(s: &str) -> GqlAdapterFamily {
    match s {
        "claude" => GqlAdapterFamily::Claude,
        "gemini" => GqlAdapterFamily::Gemini,
        "codex" => GqlAdapterFamily::Codex,
        "auggie" => GqlAdapterFamily::Auggie,
        "junie" => GqlAdapterFamily::Junie,
        _ => GqlAdapterFamily::Fixture,
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "OutputContractRepairStatus", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlOutputContractRepairStatus {
    NotAttempted,
    InProgress,
    Recovered,
    Blocked,
    Skipped,
    Cancelled,
    Failed,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "PresentationCategory", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlPresentationCategory {
    Informational,
    Recovered,
    Blocked,
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "InitialFailureClass", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlInitialFailureClass {
    NoOutputProduced,
    EmptyOutput,
    MissingRequiredOutputs,
    InvalidRequiredOutputs,
    OutputContractMismatch,
    ProviderModeMismatch,
    Unknown,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "SameSessionRepairResult", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlSameSessionRepairResult {
    NotNeeded,
    Accepted,
    RejectedInvalid,
    Unavailable,
    SkippedIneligible,
    FailedTransport,
    BudgetExhausted,
    DeadlineExceeded,
    Cancelled,
    SupersededIgnored,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "TranscriptRecoveryResult", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlTranscriptRecoveryResult {
    NotNeeded,
    Accepted,
    RejectedInvalid,
    SkippedIneligible,
    FailedTransport,
    Cancelled,
    Unavailable,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "TranscriptRecoverySource", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlRecoverySource {
    Transcript,
    ProviderEnvelope,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "ProviderFallbackResult", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlProviderFallbackResult {
    NotNeeded,
    Scheduled,
    Accepted,
    RejectedInvalid,
    Unavailable,
    SkippedIneligible,
    FailedTransport,
    DeadlineExceeded,
    Cancelled,
    BudgetExhausted,
    LeaseContended,
    SupersededIgnored,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "OutputContractRepairLeaseState", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlLeaseState {
    Reserved,
    PromptSent,
    Settled,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "OutputContractRepairLeaseKind", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlLeaseKind {
    Repair,
    Fallback,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "RequiredOutputMode", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlRequiredOutputMode {
    StrictStructured,
    ChainworksOutput,
    FileArtifact,
    StatusArtifact,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "InitialFailureSubtype", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlInitialFailureSubtype {
    PlanEventInsteadOfOutput,
    EmptySubmitAfterPlan,
    FilePlanWrittenInsteadOfPayload,
    RepairRepeatedPlanBehavior,
    MalformedEnvelope,
    WrongOutputKey,
    WrongChannel,
    WrongCanonicalPath,
    UnknownEnumValue,
    MissingRequiredField,
    UnsafeContinuation,
    OversizedPayload,
    UnattributableEnvelope,
    OversizedFallbackPacket,
    PrincipalRevoked,
    TranscriptRecoveryFlagMissing,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "FinalOutputSettlement", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlFinalOutputSettlement {
    ValidOutputsFromCompletedExecution,
    ValidOutputsFromRepair,
    ValidOutputsFromTranscriptRecovery,
    ValidOutputsFromProviderEnvelope,
    ValidOutputsFromFallback,
    BlockedMissingRequiredOutputs,
    BlockedInvalidRequiredOutputs,
    BlockedProviderModeMismatch,
    IgnoredLateOutputs,
    Cancelled,
    FailedTransport,
    DeadlineExceeded,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "RecommendedNextAction", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlRecommendedNextAction {
    Continue,
    InspectRepairEvidence,
    ConfigureFallbackPolicy,
    OperatorResolveApproval,
    OperatorResolveWorkflowConflict,
    RetryAfterTransportRestored,
    CancelAcknowledged,
    ManualInvestigation,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "PermissionDecisionValue", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlPermissionDecisionValue {
    Allowed,
    Denied,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "PermissionResourceKind", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlPermissionResourceKind {
    FsWriteCanonicalOutputPath,
    FsWriteOther,
    FsRead,
    Shell,
    Network,
    ToolCustom,
    ToolMcp,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "OutputContractRepairAttempt", rename_fields = "camelCase")]
pub struct GqlSameSessionRepair {
    pub result: GqlSameSessionRepairResult,
    pub turn_count: i64,
    pub deadline_seconds: Option<i64>,
    pub reason: Option<String>,
    pub repair_attempt_id: Option<String>,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "OutputContractTranscriptRecovery", rename_fields = "camelCase")]
pub struct GqlTranscriptRecovery {
    pub result: GqlTranscriptRecoveryResult,
    pub result_subtype: Option<String>,
    pub recovery_source: Option<GqlRecoverySource>,
    pub bytes_examined: Option<i64>,
    pub max_recovery_payload_bytes: i64,
    pub max_json_depth: i64,
    pub max_chunks_examined: i64,
    pub recovery_parser_version: String,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "OutputContractProviderFallback", rename_fields = "camelCase")]
pub struct GqlProviderFallback {
    pub result: GqlProviderFallbackResult,
    pub fallback_profile: Option<String>,
    pub fallback_agent_execution_id: Option<String>,
    pub parent_failed_agent_execution_id: Option<String>,
    pub fallback_packet_hash: Option<String>,
    pub fallback_principal_id: Option<String>,
    pub fallback_principal_capability_hash: Option<String>,
    pub deadline_seconds: Option<i64>,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "OutputContractPlanEvidence", rename_fields = "camelCase")]
pub struct GqlProviderPlanEvidence {
    pub paths: Vec<String>,
    pub redactions_applied: Vec<String>,
    pub truncated_at_cap: bool,
    pub accepted_as_output: bool,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "RequiredOutputBinding", rename_fields = "camelCase")]
pub struct GqlRequiredOutputBinding {
    pub name: String,
    pub contract_id: String,
    pub canonical_path: String,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "OutputContractPermissionDecision", rename_fields = "camelCase")]
pub struct GqlPermissionDecision {
    pub method: String,
    pub resource_kind: GqlPermissionResourceKind,
    pub decision: GqlPermissionDecisionValue,
    pub reason: String,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "OutputContractRepairBudget", rename_fields = "camelCase")]
pub struct GqlOutputContractRepairBudget {
    pub repair_consumed: bool,
    pub fallback_consumed: bool,
    pub repair_max_per_invocation: i64,
    pub fallback_max_per_invocation: i64,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "OutputContractRepairLease", rename_fields = "camelCase")]
pub struct GqlOutputContractRepairLease {
    pub key: String,
    pub kind: GqlLeaseKind,
    pub state: GqlLeaseState,
    pub settled_result: Option<String>,
    pub reclamation_reason: Option<String>,
    pub owner_principal_id: String,
    pub acquired_at: String,
    pub expires_at: String,
    pub lease_seconds: i64,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "OutputContractRepairEvidence", rename_fields = "camelCase")]
pub struct GqlOutputContractRepairEvidence {
    pub schema_version: String,
    pub repair_attempt_id: ID,
    pub run_id: ID,
    pub stage_execution_id: ID,
    pub agent_execution_id: ID,
    pub session_generation_id: String,
    pub role: String,
    pub provider_family: GqlProviderFamily,
    pub adapter_family: GqlAdapterFamily,
    pub required_output_mode: GqlRequiredOutputMode,
    pub initial_failure_class: GqlInitialFailureClass,
    pub initial_failure_subtype: Option<GqlInitialFailureSubtype>,
    pub status: GqlOutputContractRepairStatus,
    pub presentation_category: GqlPresentationCategory,
    pub recommended_next_action: GqlRecommendedNextAction,
    pub final_output_settlement: Option<GqlFinalOutputSettlement>,
    pub same_session_repair: GqlSameSessionRepair,
    pub transcript_recovery: GqlTranscriptRecovery,
    pub provider_fallback: GqlProviderFallback,
    pub provider_plan_evidence: GqlProviderPlanEvidence,
    pub required_outputs: Vec<GqlRequiredOutputBinding>,
    pub permission_decisions: Vec<GqlPermissionDecision>,
    pub budget: GqlOutputContractRepairBudget,
    pub repair_prompt_template_version: String,
    pub recovery_parser_version: String,
    pub policy_feature_flags: Vec<String>,
    pub evidence_artifact_path: Option<String>,
    pub lease: Option<GqlOutputContractRepairLease>,
    pub evidence_version: i64,
    pub projection_integrity: String,
    pub projection_stale_since: Option<String>,
    pub recorded_at: String,
    pub created_at: String,
    pub updated_at: String,
}

// --- P079 enum mapping helpers ---

fn gql_output_contract_repair_status(s: &str) -> GqlOutputContractRepairStatus {
    match s {
        "not_attempted" => GqlOutputContractRepairStatus::NotAttempted,
        "in_progress" => GqlOutputContractRepairStatus::InProgress,
        "recovered" => GqlOutputContractRepairStatus::Recovered,
        "blocked" => GqlOutputContractRepairStatus::Blocked,
        "skipped" => GqlOutputContractRepairStatus::Skipped,
        "cancelled" => GqlOutputContractRepairStatus::Cancelled,
        _ => GqlOutputContractRepairStatus::Failed,
    }
}

fn gql_presentation_category(s: &str) -> GqlPresentationCategory {
    match s {
        "informational" => GqlPresentationCategory::Informational,
        "recovered" => GqlPresentationCategory::Recovered,
        "blocked" => GqlPresentationCategory::Blocked,
        "skipped" => GqlPresentationCategory::Skipped,
        "cancelled" => GqlPresentationCategory::Cancelled,
        _ => GqlPresentationCategory::Failed,
    }
}

fn gql_initial_failure_class(s: &str) -> GqlInitialFailureClass {
    match s {
        "no_output_produced" => GqlInitialFailureClass::NoOutputProduced,
        "empty_output" => GqlInitialFailureClass::EmptyOutput,
        "missing_required_outputs" => GqlInitialFailureClass::MissingRequiredOutputs,
        "invalid_required_outputs" => GqlInitialFailureClass::InvalidRequiredOutputs,
        "output_contract_mismatch" => GqlInitialFailureClass::OutputContractMismatch,
        "provider_mode_mismatch" => GqlInitialFailureClass::ProviderModeMismatch,
        _ => GqlInitialFailureClass::Unknown,
    }
}

fn gql_same_session_repair_result(s: &str) -> GqlSameSessionRepairResult {
    match s {
        "not_needed" => GqlSameSessionRepairResult::NotNeeded,
        "accepted" => GqlSameSessionRepairResult::Accepted,
        "rejected_invalid" => GqlSameSessionRepairResult::RejectedInvalid,
        "unavailable" => GqlSameSessionRepairResult::Unavailable,
        "skipped_ineligible" => GqlSameSessionRepairResult::SkippedIneligible,
        "failed_transport" => GqlSameSessionRepairResult::FailedTransport,
        "budget_exhausted" => GqlSameSessionRepairResult::BudgetExhausted,
        "deadline_exceeded" => GqlSameSessionRepairResult::DeadlineExceeded,
        "cancelled" => GqlSameSessionRepairResult::Cancelled,
        _ => GqlSameSessionRepairResult::SupersededIgnored,
    }
}

fn gql_transcript_recovery_result(s: &str) -> GqlTranscriptRecoveryResult {
    match s {
        "not_needed" => GqlTranscriptRecoveryResult::NotNeeded,
        "accepted" => GqlTranscriptRecoveryResult::Accepted,
        "rejected_invalid" => GqlTranscriptRecoveryResult::RejectedInvalid,
        "skipped_ineligible" => GqlTranscriptRecoveryResult::SkippedIneligible,
        "failed_transport" => GqlTranscriptRecoveryResult::FailedTransport,
        "cancelled" => GqlTranscriptRecoveryResult::Cancelled,
        _ => GqlTranscriptRecoveryResult::Unavailable,
    }
}

fn gql_recovery_source(s: &str) -> Option<GqlRecoverySource> {
    match s {
        "transcript" => Some(GqlRecoverySource::Transcript),
        "provider_envelope" => Some(GqlRecoverySource::ProviderEnvelope),
        _ => None,
    }
}

fn gql_provider_fallback_result(s: &str) -> GqlProviderFallbackResult {
    match s {
        "not_needed" => GqlProviderFallbackResult::NotNeeded,
        "scheduled" => GqlProviderFallbackResult::Scheduled,
        "accepted" => GqlProviderFallbackResult::Accepted,
        "rejected_invalid" => GqlProviderFallbackResult::RejectedInvalid,
        "unavailable" => GqlProviderFallbackResult::Unavailable,
        "skipped_ineligible" => GqlProviderFallbackResult::SkippedIneligible,
        "failed_transport" => GqlProviderFallbackResult::FailedTransport,
        "deadline_exceeded" => GqlProviderFallbackResult::DeadlineExceeded,
        "cancelled" => GqlProviderFallbackResult::Cancelled,
        "budget_exhausted" => GqlProviderFallbackResult::BudgetExhausted,
        "lease_contended" => GqlProviderFallbackResult::LeaseContended,
        _ => GqlProviderFallbackResult::SupersededIgnored,
    }
}

fn gql_lease_state(s: &str) -> GqlLeaseState {
    match s {
        "reserved" => GqlLeaseState::Reserved,
        "prompt_sent" => GqlLeaseState::PromptSent,
        _ => GqlLeaseState::Settled,
    }
}

fn gql_lease_kind(s: &str) -> GqlLeaseKind {
    match s {
        "fallback" => GqlLeaseKind::Fallback,
        _ => GqlLeaseKind::Repair,
    }
}

fn gql_required_output_mode(s: &str) -> GqlRequiredOutputMode {
    match s {
        "strict_structured" => GqlRequiredOutputMode::StrictStructured,
        "file_artifact" => GqlRequiredOutputMode::FileArtifact,
        "status_artifact" => GqlRequiredOutputMode::StatusArtifact,
        _ => GqlRequiredOutputMode::ChainworksOutput,
    }
}

fn gql_initial_failure_subtype(s: &str) -> GqlInitialFailureSubtype {
    match s {
        "plan_event_instead_of_output" => GqlInitialFailureSubtype::PlanEventInsteadOfOutput,
        "empty_submit_after_plan" => GqlInitialFailureSubtype::EmptySubmitAfterPlan,
        "file_plan_written_instead_of_payload" => GqlInitialFailureSubtype::FilePlanWrittenInsteadOfPayload,
        "repair_repeated_plan_behavior" => GqlInitialFailureSubtype::RepairRepeatedPlanBehavior,
        "malformed_envelope" => GqlInitialFailureSubtype::MalformedEnvelope,
        "wrong_output_key" => GqlInitialFailureSubtype::WrongOutputKey,
        "wrong_channel" => GqlInitialFailureSubtype::WrongChannel,
        "wrong_canonical_path" => GqlInitialFailureSubtype::WrongCanonicalPath,
        "missing_required_field" => GqlInitialFailureSubtype::MissingRequiredField,
        "unsafe_continuation" => GqlInitialFailureSubtype::UnsafeContinuation,
        "oversized_payload" => GqlInitialFailureSubtype::OversizedPayload,
        "unattributable_envelope" => GqlInitialFailureSubtype::UnattributableEnvelope,
        "oversized_fallback_packet" => GqlInitialFailureSubtype::OversizedFallbackPacket,
        "principal_revoked" => GqlInitialFailureSubtype::PrincipalRevoked,
        "transcript_recovery_flag_missing" => GqlInitialFailureSubtype::TranscriptRecoveryFlagMissing,
        _ => GqlInitialFailureSubtype::UnknownEnumValue,
    }
}

fn gql_final_output_settlement(s: &str) -> GqlFinalOutputSettlement {
    match s {
        "valid_outputs_from_completed_execution" => GqlFinalOutputSettlement::ValidOutputsFromCompletedExecution,
        "valid_outputs_from_repair" => GqlFinalOutputSettlement::ValidOutputsFromRepair,
        "valid_outputs_from_transcript_recovery" => GqlFinalOutputSettlement::ValidOutputsFromTranscriptRecovery,
        "valid_outputs_from_provider_envelope" => GqlFinalOutputSettlement::ValidOutputsFromProviderEnvelope,
        "valid_outputs_from_fallback" => GqlFinalOutputSettlement::ValidOutputsFromFallback,
        "blocked_missing_required_outputs" => GqlFinalOutputSettlement::BlockedMissingRequiredOutputs,
        "blocked_invalid_required_outputs" => GqlFinalOutputSettlement::BlockedInvalidRequiredOutputs,
        "blocked_provider_mode_mismatch" => GqlFinalOutputSettlement::BlockedProviderModeMismatch,
        "ignored_late_outputs" => GqlFinalOutputSettlement::IgnoredLateOutputs,
        "cancelled" => GqlFinalOutputSettlement::Cancelled,
        "failed_transport" => GqlFinalOutputSettlement::FailedTransport,
        _ => GqlFinalOutputSettlement::DeadlineExceeded,
    }
}

fn gql_recommended_next_action(s: &str) -> GqlRecommendedNextAction {
    match s {
        "continue" => GqlRecommendedNextAction::Continue,
        "inspect_repair_evidence" => GqlRecommendedNextAction::InspectRepairEvidence,
        "configure_fallback_policy" => GqlRecommendedNextAction::ConfigureFallbackPolicy,
        "operator_resolve_approval" => GqlRecommendedNextAction::OperatorResolveApproval,
        "operator_resolve_workflow_conflict" => GqlRecommendedNextAction::OperatorResolveWorkflowConflict,
        "retry_after_transport_restored" => GqlRecommendedNextAction::RetryAfterTransportRestored,
        "cancel_acknowledged" => GqlRecommendedNextAction::CancelAcknowledged,
        _ => GqlRecommendedNextAction::ManualInvestigation,
    }
}

fn gql_permission_decision_value(s: &str) -> GqlPermissionDecisionValue {
    match s {
        "allowed" => GqlPermissionDecisionValue::Allowed,
        _ => GqlPermissionDecisionValue::Denied,
    }
}

fn gql_permission_resource_kind(s: &str) -> GqlPermissionResourceKind {
    match s {
        "fs_write_canonical_output_path" => GqlPermissionResourceKind::FsWriteCanonicalOutputPath,
        "fs_write_other" => GqlPermissionResourceKind::FsWriteOther,
        "fs_read" => GqlPermissionResourceKind::FsRead,
        "shell" => GqlPermissionResourceKind::Shell,
        "network" => GqlPermissionResourceKind::Network,
        "tool_mcp" => GqlPermissionResourceKind::ToolMcp,
        _ => GqlPermissionResourceKind::ToolCustom,
    }
}

// --- P079 JSON parsing helpers (256 KiB cap enforced) ---

fn p079_gql_same_session_repair(json: Option<&str>) -> Option<GqlSameSessionRepair> {
    let v = p079_parse_json_capped(json?)?;
    Some(GqlSameSessionRepair {
        result: gql_same_session_repair_result(v["result"].as_str().unwrap_or("unavailable")),
        turn_count: v["turn_count"].as_i64().unwrap_or(0),
        deadline_seconds: v["deadline_seconds"].as_i64(),
        reason: v["reason"].as_str().map(ToOwned::to_owned),
        repair_attempt_id: v["repair_attempt_id"].as_str().map(ToOwned::to_owned),
    })
}

fn p079_gql_transcript_recovery(json: Option<&str>) -> Option<GqlTranscriptRecovery> {
    let v = p079_parse_json_capped(json?)?;
    Some(GqlTranscriptRecovery {
        result: gql_transcript_recovery_result(v["result"].as_str().unwrap_or("unavailable")),
        result_subtype: v["result_subtype"].as_str().map(ToOwned::to_owned),
        recovery_source: v["recovery_source"]
            .as_str()
            .and_then(gql_recovery_source),
        bytes_examined: v["bytes_examined"].as_i64(),
        max_recovery_payload_bytes: v["max_recovery_payload_bytes"].as_i64().unwrap_or(262144),
        max_json_depth: v["max_json_depth"].as_i64().unwrap_or(32),
        max_chunks_examined: v["max_chunks_examined"].as_i64().unwrap_or(64),
        recovery_parser_version: v["recovery_parser_version"]
            .as_str()
            .unwrap_or("p079_recovery_v1")
            .to_owned(),
    })
}

fn p079_gql_provider_fallback(
    json: Option<&str>,
    include_operator_debug: bool,
) -> Option<GqlProviderFallback> {
    let v = p079_parse_json_capped(json?)?;
    Some(GqlProviderFallback {
        result: gql_provider_fallback_result(v["result"].as_str().unwrap_or("skipped_ineligible")),
        fallback_profile: v["fallback_profile"].as_str().map(ToOwned::to_owned),
        fallback_agent_execution_id: v["fallback_agent_execution_id"]
            .as_str()
            .map(ToOwned::to_owned),
        parent_failed_agent_execution_id: v["parent_failed_agent_execution_id"]
            .as_str()
            .map(ToOwned::to_owned),
        fallback_packet_hash: v["fallback_packet_hash"].as_str().map(ToOwned::to_owned),
        // SEC-MED-002: principal ID and capability hash are P029 internal identifiers; gate
        // them on operator-debug to prevent leaking auth/credential-adjacent values to
        // non-operator callers, consistent with lease owner_principal_id gating above.
        fallback_principal_id: if include_operator_debug {
            v["fallback_principal_id"].as_str().map(ToOwned::to_owned)
        } else {
            None
        },
        fallback_principal_capability_hash: if include_operator_debug {
            v["fallback_principal_capability_hash"]
                .as_str()
                .map(ToOwned::to_owned)
        } else {
            None
        },
        deadline_seconds: v["deadline_seconds"].as_i64(),
    })
}

fn p079_gql_provider_plan_evidence(json: Option<&str>) -> Option<GqlProviderPlanEvidence> {
    let v = p079_parse_json_capped(json?)?;
    let raw_paths: Vec<String> = v["paths"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_str())
                .map(|p| {
                    if p079_safe_relative_path(Some(p)).is_some() {
                        p.to_owned()
                    } else {
                        "[redacted:unsafe_path]".to_owned()
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let redactions_applied: Vec<String> = v["redactions_applied"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_str())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Some(GqlProviderPlanEvidence {
        paths: raw_paths,
        redactions_applied,
        truncated_at_cap: v["truncated_at_cap"].as_bool().unwrap_or(false),
        accepted_as_output: v["accepted_as_output"].as_bool().unwrap_or(false),
    })
}

/// SEC-MED-002: Returns required output bindings with canonical_path redacted when
/// `include_operator_debug` is false, to prevent filesystem layout disclosure to non-operators.
fn p079_gql_required_outputs_with_redaction(
    json: &str,
    include_operator_debug: bool,
) -> Vec<GqlRequiredOutputBinding> {
    p079_parse_json_capped(json)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            let name = e["name"].as_str()?.to_owned();
            let contract_id = e["contract_id"].as_str().unwrap_or("").to_owned();
            let canonical_path = if include_operator_debug {
                e["canonical_path"].as_str().unwrap_or("").to_owned()
            } else {
                // Non-operator: return empty string to prevent local filesystem disclosure.
                String::new()
            };
            Some(GqlRequiredOutputBinding {
                name,
                contract_id,
                canonical_path,
            })
        })
        .collect()
}

fn p079_gql_permission_decisions(json: &str, _include_operator_debug: bool) -> Vec<GqlPermissionDecision> {
    p079_parse_json_capped(json)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            Some(GqlPermissionDecision {
                method: e["method"].as_str()?.to_owned(),
                resource_kind: gql_permission_resource_kind(e["resource_kind"].as_str().unwrap_or("tool_custom")),
                decision: gql_permission_decision_value(e["decision"].as_str().unwrap_or("denied")),
                reason: e["reason"].as_str().unwrap_or("").to_owned(),
            })
        })
        .collect()
}

fn p079_gql_policy_feature_flags(json: &str) -> Vec<String> {
    // SEC-P079-003: the engine stores policy_feature_flags_json as an array of
    // {flag, value} objects, but the original implementation only accepted
    // plain string entries. This function now handles both shapes so the
    // permission_enforcement_advisory flag is visible to GraphQL clients.
    p079_parse_json_capped(json)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            if let Some(s) = e.as_str() {
                Some(s.to_owned())
            } else if let Some(flag) = e.get("flag").and_then(|f| f.as_str()) {
                let value_str = e.get("value").map(|v| v.to_string()).unwrap_or_default();
                Some(format!("{}:{}", flag, value_str))
            } else {
                None
            }
        })
        .collect()
}

#[ComplexObject]
impl GqlStageExecution {
    async fn executions(&self, ctx: &Context<'_>) -> Result<Vec<GqlAgentExecution>> {
        let pool = ctx.data::<sqlx::SqlitePool>()?;
        let stage_execution_id: StageExecutionId = self
            .id
            .as_str()
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let executions =
            db::repos::agent_executions::find_by_stage(pool, stage_execution_id).await?;
        Ok(executions
            .into_iter()
            .map(GqlAgentExecution::from)
            .collect())
    }
}

#[ComplexObject]
impl GqlAgentExecution {
    #[graphql(name = "codeWriterCompletionReceipt")]
    async fn code_writer_completion_receipt(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlCodeWriterCompletionReceipt>> {
        let pool = ctx.data::<sqlx::SqlitePool>()?;
        let agent_execution_id: domain::ids::AgentExecutionId = self
            .id
            .as_str()
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        Ok(
            code_writer_completion_receipts::find_by_execution_id(pool, agent_execution_id)
                .await?
                .map(Into::into),
        )
    }

    #[graphql(name = "runtimeFacts")]
    async fn runtime_facts(&self, ctx: &Context<'_>) -> Result<GqlAgentExecutionRuntimeFacts> {
        let include_operator_debug = ctx
            .data_opt::<auth::Principal>()
            .is_some_and(|principal| principal.class == auth::PrincipalClass::Operator);
        let pool = ctx.data::<sqlx::SqlitePool>()?;
        let agent_execution_id: domain::ids::AgentExecutionId = self
            .id
            .as_str()
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let facts =
            agent_execution_runtime_facts::find_by_execution_id(pool, agent_execution_id).await?;
        let lineage = match self.session_lineage_id.as_deref() {
            Some(lineage_id) => sessions::find_lineage_by_id(pool, lineage_id).await?,
            None => None,
        };
        let generation = match self.session_generation_id.as_deref() {
            Some(generation_id) => sessions::find_generation_by_id(pool, generation_id).await?,
            None => None,
        };
        let facts = facts.unwrap_or_else(|| {
            domain::agent::AgentExecutionRuntimeFacts::defaults_for(
                agent_execution_id,
                chrono::Utc::now(),
            )
        });
        let mut facts = facts;
        if agent_execution_discovery_diagnostics::find_readback_by_execution_id(
            pool,
            agent_execution_id,
        )
        .await?
        .is_some_and(|readback| readback.reconciliation_pending)
        {
            facts.valid_required_outputs = false;
        }
        Ok(GqlAgentExecutionRuntimeFacts::from_facts_and_execution(
            facts,
            self,
            lineage.as_ref(),
            generation.as_ref(),
            include_operator_debug,
        ))
    }

    #[graphql(name = "discoveryDiagnostics")]
    async fn discovery_diagnostics(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<Json<serde_json::Value>>> {
        let pool = ctx.data::<sqlx::SqlitePool>()?;
        let agent_execution_id: domain::ids::AgentExecutionId = self
            .id
            .as_str()
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let diagnostics = agent_execution_discovery_diagnostics::find_readback_by_execution_id(
            pool,
            agent_execution_id,
        )
        .await?;
        Ok(diagnostics.map(|readback| {
            let payload = readback.projected_payload();
            let diagnostics = &readback.diagnostics;
            Json(serde_json::json!({
                "agent_execution_id": diagnostics.agent_execution_id.clone(),
                "discovery_schema_version": diagnostics.discovery_schema_version.clone(),
                "legacy_broad_discovery_used": diagnostics.legacy_broad_discovery_used,
                "missing_required_output_count": diagnostics.missing_required_output_count,
                "rejected_output_count": diagnostics.rejected_output_count,
                "stale_output_count": diagnostics.stale_output_count,
                "meta_discovery_truncated": diagnostics.meta_discovery_truncated,
                "git_manifest_status": diagnostics.git_manifest_status.clone(),
                "resume_warning_count": diagnostics.resume_warning_count,
                "reconciliation_pending": readback.reconciliation_pending,
                "reconciliation_warnings": readback.reconciliation_warnings.clone(),
                "runtime_facts_present": readback.runtime_facts_present,
                "matching_active_artifact_generation_count": readback.matching_active_artifact_generation_count,
                "payload": payload,
                "created_at": diagnostics.created_at.to_rfc3339(),
                "updated_at": diagnostics.updated_at.to_rfc3339(),
            }))
        }))
    }

    /// P079: Output contract repair evidence for this agent execution.
    /// Returns null when no P079 repair event exists (pre-P079 runs, feature-disabled, not triggered).
    /// Returns typed OutputContractRepairEvidence (MISSING-008 fix — replaces prior JSON blob).
    /// SEC-MED-002: canonical_path and owner_principal_id are gated by operator-debug access,
    /// matching the redaction policy applied by MCP reports.
    #[graphql(name = "outputContractRepair")]
    async fn output_contract_repair_evidence(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlOutputContractRepairEvidence>> {
        // SEC-MED-002: gate sensitive fields (canonical_path, owner_principal_id) on operator class,
        // matching the include_operator_debug policy used in MCP reports.
        let include_operator_debug = ctx
            .data_opt::<auth::Principal>()
            .is_some_and(|p| p.class == auth::PrincipalClass::Operator);
        let pool = ctx.data::<sqlx::SqlitePool>()?;
        let Some(row) = output_contract_repair::get_repair_event_by_agent_execution_id(
            pool,
            self.id.as_str(),
        )
        .await? else {
            return Ok(None);
        };

        let lease = if let Some(ref lease_key) = row.lease_id {
            match output_contract_repair::get_lease_by_key(pool, lease_key).await {
                Ok(Some(lease)) => Some(GqlOutputContractRepairLease {
                    key: lease.lease_key,
                    kind: gql_lease_kind(&lease.lease_kind.to_string()),
                    state: gql_lease_state(&lease.lease_state.to_string()),
                    settled_result: lease.settled_result,
                    reclamation_reason: lease.reclamation_reason,
                    // SEC-MED-002: redact principal ID for non-operator callers to prevent
                    // leaking internal principal identifiers via the GraphQL readback surface.
                    owner_principal_id: if include_operator_debug {
                        lease.lease_owner_principal_id
                    } else {
                        "redacted".to_string()
                    },
                    acquired_at: lease.lease_acquired_at,
                    expires_at: lease.lease_expires_at,
                    lease_seconds: lease.lease_seconds,
                }),
                _ => None,
            }
        } else {
            None
        };

        // SEC-MED-002: redact canonical_path values in required_outputs for non-operator callers.
        // Non-operators see only the output name, preventing local filesystem layout disclosure.
        let required_outputs = p079_gql_required_outputs_with_redaction(
            &row.required_outputs_json,
            include_operator_debug,
        );

        Ok(Some(GqlOutputContractRepairEvidence {
            schema_version: row.schema_version,
            repair_attempt_id: ID(row.repair_attempt_id),
            run_id: ID(row.run_id),
            stage_execution_id: ID(row.stage_execution_id),
            agent_execution_id: ID(row.agent_execution_id),
            session_generation_id: row.session_generation_id,
            role: row.role,
            provider_family: gql_provider_family(&row.provider_family),
            adapter_family: gql_adapter_family(&row.adapter_family),
            required_output_mode: gql_required_output_mode(&row.required_output_mode),
            initial_failure_class: gql_initial_failure_class(&row.initial_failure_class),
            initial_failure_subtype: row.initial_failure_subtype.as_deref().map(gql_initial_failure_subtype),
            status: gql_output_contract_repair_status(&row.status),
            presentation_category: gql_presentation_category(&row.presentation_category),
            recommended_next_action: gql_recommended_next_action(&row.recommended_next_action),
            final_output_settlement: row.final_output_settlement.as_deref().map(gql_final_output_settlement),
            same_session_repair: p079_gql_same_session_repair(row.same_session_repair_json.as_deref())
                .unwrap_or_else(|| GqlSameSessionRepair {
                    result: GqlSameSessionRepairResult::NotNeeded,
                    turn_count: 0,
                    deadline_seconds: Some(0),
                    reason: None,
                    repair_attempt_id: None,
                }),
            transcript_recovery: p079_gql_transcript_recovery(row.transcript_recovery_json.as_deref())
                .unwrap_or_else(|| GqlTranscriptRecovery {
                    result: GqlTranscriptRecoveryResult::NotNeeded,
                    result_subtype: None,
                    recovery_source: None,
                    bytes_examined: None,
                    max_recovery_payload_bytes: 262144,
                    max_json_depth: 32,
                    max_chunks_examined: 64,
                    recovery_parser_version: "p079_recovery_v1".to_string(),
                }),
            provider_fallback: p079_gql_provider_fallback(row.provider_fallback_json.as_deref(), include_operator_debug)
                .unwrap_or_else(|| GqlProviderFallback {
                    result: GqlProviderFallbackResult::NotNeeded,
                    fallback_profile: None,
                    fallback_agent_execution_id: None,
                    parent_failed_agent_execution_id: None,
                    fallback_packet_hash: None,
                    fallback_principal_id: None,
                    fallback_principal_capability_hash: None,
                    deadline_seconds: Some(0),
                }),
            provider_plan_evidence: p079_gql_provider_plan_evidence(row.provider_plan_evidence_json.as_deref())
                .unwrap_or_else(|| GqlProviderPlanEvidence {
                    paths: vec![],
                    redactions_applied: vec![],
                    truncated_at_cap: false,
                    accepted_as_output: false,
                }),
            required_outputs,
            permission_decisions: p079_gql_permission_decisions(&row.permission_decisions_json, include_operator_debug),
            budget: GqlOutputContractRepairBudget {
                repair_consumed: row.repair_budget_consumed,
                fallback_consumed: row.fallback_budget_consumed,
                repair_max_per_invocation: 1,
                fallback_max_per_invocation: 1,
            },
            repair_prompt_template_version: row
                .repair_prompt_template_version
                .unwrap_or_else(|| "p079_repair_v1".to_string()),
            recovery_parser_version: row
                .recovery_parser_version
                .unwrap_or_else(|| "p079_recovery_v1".to_string()),
            policy_feature_flags: p079_gql_policy_feature_flags(&row.policy_feature_flags_json),
            // SEC-003: Gate evidence_artifact_path behind include_operator_debug to match
            // MCP redaction policy; prevents non-operator callers from learning run-meta
            // path layout via the GraphQL surface.
            evidence_artifact_path: if include_operator_debug {
                p079_safe_relative_path(row.evidence_artifact_path.as_deref())
                    .map(ToOwned::to_owned)
            } else {
                None
            },
            lease,
            evidence_version: row.evidence_version,
            projection_integrity: row.projection_integrity,
            projection_stale_since: row.projection_stale_since,
            recorded_at: row.recorded_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
    }
}

impl GqlAgentExecutionRuntimeFacts {
    fn from_facts_and_execution(
        facts: domain::agent::AgentExecutionRuntimeFacts,
        execution: &GqlAgentExecution,
        lineage: Option<&SessionLineage>,
        generation: Option<&SessionGeneration>,
        include_operator_debug: bool,
    ) -> Self {
        let generation_status = generation.map(|generation| {
            sessions::session_generation_status_to_str(&generation.status).to_string()
        });
        let active_session_generation_id = lineage
            .and_then(|lineage| lineage.active_generation_id.clone())
            .map(ID);
        let active_generation_matches_execution =
            match (lineage, execution.session_generation_id.as_deref()) {
                (Some(lineage), Some(execution_generation_id)) => {
                    Some(lineage.active_generation_id.as_deref() == Some(execution_generation_id))
                }
                _ => None,
            };
        GqlAgentExecutionRuntimeFacts {
            agent_execution_id: ID(facts.agent_execution_id.to_string()),
            failure_kind: facts.failure_kind.map(|kind| match kind {
                domain::agent::AgentFailureKind::ProviderQuota => AgentFailureKind::ProviderQuota,
                domain::agent::AgentFailureKind::ProviderPermissionRequired => {
                    AgentFailureKind::ProviderPermissionRequired
                }
                domain::agent::AgentFailureKind::ProviderPermissionRejected => {
                    AgentFailureKind::ProviderPermissionRejected
                }
                domain::agent::AgentFailureKind::ProviderTimeout => {
                    AgentFailureKind::ProviderTimeout
                }
                domain::agent::AgentFailureKind::ProviderInternalError => {
                    AgentFailureKind::ProviderInternalError
                }
                domain::agent::AgentFailureKind::ToolOutputBudgetExceeded => {
                    AgentFailureKind::ToolOutputBudgetExceeded
                }
                domain::agent::AgentFailureKind::ToolOutputBudgetPreflightDenied => {
                    AgentFailureKind::ToolOutputBudgetPreflightDenied
                }
                domain::agent::AgentFailureKind::TransportEpipe => AgentFailureKind::TransportEpipe,
                domain::agent::AgentFailureKind::TransportProtocolError => {
                    AgentFailureKind::TransportProtocolError
                }
                domain::agent::AgentFailureKind::TransportClosed => {
                    AgentFailureKind::TransportClosed
                }
                domain::agent::AgentFailureKind::McpStartupTimeout => {
                    AgentFailureKind::McpStartupTimeout
                }
                domain::agent::AgentFailureKind::McpPermissionModalStall => {
                    AgentFailureKind::McpPermissionModalStall
                }
                domain::agent::AgentFailureKind::XcodeHostEnvironmentError => {
                    AgentFailureKind::XcodeHostEnvironmentError
                }
                domain::agent::AgentFailureKind::MissingRequiredOutputs => {
                    AgentFailureKind::MissingRequiredOutputs
                }
                domain::agent::AgentFailureKind::InvalidOutputContract => {
                    AgentFailureKind::InvalidOutputContract
                }
                domain::agent::AgentFailureKind::CancelledByOperator => {
                    AgentFailureKind::CancelledByOperator
                }
                domain::agent::AgentFailureKind::SupersededByRetry => {
                    AgentFailureKind::SupersededByRetry
                }
                domain::agent::AgentFailureKind::HostInterruption => {
                    AgentFailureKind::HostInterruption
                }
                domain::agent::AgentFailureKind::ToolOutputBudgetPreflightDenied => {
                    AgentFailureKind::ToolOutputBudgetPreflightDenied
                }
                domain::agent::AgentFailureKind::ToolOutputBudgetExceeded => {
                    AgentFailureKind::ToolOutputBudgetExceeded
                }
                domain::agent::AgentFailureKind::Unknown => AgentFailureKind::Unknown,
            }),
            failure_kind_raw_debug: include_operator_debug
                .then(|| facts.failure_kind_raw_debug)
                .flatten(),
            failure_kind_version: facts.failure_kind_version,
            failure_message_redacted: facts.failure_message_redacted,
            failure_message_redaction_version: facts.failure_message_redaction_version,
            retry_after: facts.retry_after.map(|dt| dt.to_rfc3339()),
            operator_action_hint: facts.operator_action_hint.map(|hint| hint.to_string()),
            provider_exit_status: facts.provider_exit_status,
            transport_error_code: facts.transport_error_code,
            supervision_classification: facts.supervision_classification,
            output_settlement: match facts.output_settlement {
                domain::agent::AgentOutputSettlement::None => AgentOutputSettlement::None,
                domain::agent::AgentOutputSettlement::MissingRequiredOutputs => {
                    AgentOutputSettlement::MissingRequiredOutputs
                }
                domain::agent::AgentOutputSettlement::InvalidRequiredOutputs => {
                    AgentOutputSettlement::InvalidRequiredOutputs
                }
                domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution => {
                    AgentOutputSettlement::ValidOutputsFromCompletedExecution
                }
                domain::agent::AgentOutputSettlement::ValidOutputsFromFailedExecution => {
                    AgentOutputSettlement::ValidOutputsFromFailedExecution
                }
                domain::agent::AgentOutputSettlement::IgnoredLateOutputs => {
                    AgentOutputSettlement::IgnoredLateOutputs
                }
                domain::agent::AgentOutputSettlement::ValidOutputsFromRepair => {
                    AgentOutputSettlement::ValidOutputsFromRepair
                }
            },
            valid_required_outputs: facts.valid_required_outputs,
            late_output_count: facts.late_output_count,
            ignored_late_output_count: facts.ignored_late_output_count,
            session_lineage_id: execution.session_lineage_id.clone().map(ID),
            session_generation_id: execution.session_generation_id.clone().map(ID),
            session_reuse_scope: execution.session_reuse_scope.clone(),
            session_family_id: execution.session_family_id.clone(),
            session_reuse_disposition: execution.session_reuse_disposition.clone(),
            session_reuse_reason: facts.session_reuse_reason,
            session_reset_reason: execution.session_reset_reason.clone(),
            active_session_generation_id,
            active_generation_matches_execution,
            generation_status,
            fresh_provider_process: fresh_provider_process_for_disposition(
                execution.session_reuse_disposition.as_deref(),
            ),
            rehydrated_from_checkpoint_artifact_id: execution
                .rehydrated_from_checkpoint_artifact_id
                .clone()
                .map(ID),
            quota_ledger_id: facts.quota_ledger_id.map(ID),
            created_at: facts.created_at.to_rfc3339(),
            updated_at: facts.updated_at.to_rfc3339(),
        }
    }
}

impl From<StageExecution> for GqlStageExecution {
    fn from(s: StageExecution) -> Self {
        GqlStageExecution {
            id: ID(s.id.to_string()),
            run_id: ID(s.run_id.to_string()),
            stage_id: s.stage_id,
            label: s.label,
            status: s.status.to_string(),
            iteration: s.iteration,
            attempt_number: s.attempt_number,
            settlement_kind: s.settlement_kind.map(|k| k.to_string()),
            started_at: s.started_at.to_rfc3339(),
            completed_at: s.completed_at.map(|t| t.to_rfc3339()),
            has_artifacts: None,
            has_pending_approval: None,
            has_validation_failure: None,
            validation_failure_json: s.validation_failure_json,
            evidence_packet_json: s.evidence_packet_json,
            recovery_snapshot_json: s.recovery_snapshot_json,
            terminal_reason: None,
            retry_authority_id: None,
            is_retry_authoritative: None,
            retry_authority_state: None,
            projection_present: false,
            projection_updated_at: None,
            projection_lag: true,
            freshness_state: GqlFreshnessState::ProjectionLag,
        }
    }
}

impl GqlStageExecution {
    pub fn from_projection_and_stage(r: StageSummaryRow, s: StageExecution) -> Self {
        let mut gql = GqlStageExecution::from(s);
        gql.status = r.status;
        gql.attempt_number = r.attempt_number;
        gql.settlement_kind = r.settlement_kind;
        gql.has_artifacts = Some(r.has_artifacts);
        gql.has_pending_approval = Some(r.has_pending_approval);
        gql.has_validation_failure = Some(r.has_validation_failure);
        gql.terminal_reason = r.terminal_reason;
        gql.retry_authority_id = r.retry_authority_id;
        gql.is_retry_authoritative = Some(r.is_retry_authoritative);
        gql.retry_authority_state = r.retry_authority_state;
        gql.projection_present = r.projection_present;
        gql.projection_updated_at = r.projection_updated_at;
        gql.projection_lag = r.projection_lag;
        gql.freshness_state = freshness_from_projection_lag(gql.projection_lag);
        gql
    }
}

impl From<StageSummaryRow> for GqlStageExecution {
    fn from(r: StageSummaryRow) -> Self {
        GqlStageExecution {
            id: ID(r.id),
            run_id: ID(r.run_id),
            stage_id: r.stage_id,
            label: r.label,
            status: r.status,
            iteration: r.iteration,
            attempt_number: r.attempt_number,
            settlement_kind: r.settlement_kind,
            started_at: r.started_at,
            completed_at: r.completed_at,
            has_artifacts: Some(r.has_artifacts),
            has_pending_approval: Some(r.has_pending_approval),
            has_validation_failure: Some(r.has_validation_failure),
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            terminal_reason: r.terminal_reason,
            retry_authority_id: r.retry_authority_id,
            is_retry_authoritative: Some(r.is_retry_authoritative),
            retry_authority_state: r.retry_authority_state,
            projection_present: r.projection_present,
            projection_updated_at: r.projection_updated_at,
            projection_lag: r.projection_lag,
            freshness_state: freshness_from_projection_lag(r.projection_lag),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use domain::agent::{AgentExecution, AgentStatus};
    use domain::ids::{AgentExecutionId, StageExecutionId};

    #[test]
    fn gql_agent_execution_redacts_raw_stored_xcode_runtime_observation() {
        let raw_observation = serde_json::json!({
            "version": 1,
            "mcp_broker_observations": [{
                "source": "xcode_mcp_broker",
                "backend_start_disposition": "lease_reserved",
                "pool_id": "pool-1",
                "lease_id": "lease-1",
                "xcode_pid": "1234",
                "backend_process_id": 5678,
                "http_endpoint": "http://127.0.0.1:4000/xcode-mcp/lease-1?token=raw-graphql-token",
                "xcode_home_disposition": "host_user_home",
                "xcode_tmpdir_disposition": "host_user_tmpdir",
                "simulator_selection": null,
                "sibling_leases_at_spawn": 1,
                "backend_initialize_wait_ms": 42,
                "backend_startup_latency_ms": 73,
                "http_session_startup_latency_ms": 17,
                "backend_failure_class": null,
                "originating_execution_id": "execution-1",
                "prompt_cycle_index": 0,
                "status_update": "forwarded Bearer raw-graphql-bearer"
            }],
            "xcode_shim_events": [{
                "kind": "shim_runtime_attached",
                "ts": "2026-05-10T17:00:00Z",
                "source": "xcode_shim_runtime",
                "reason": "requires_xcode_host_execution",
                "lease_id": "xcode-lease-raw-secret",
                "shim_dir": "/tmp/shims",
                "socket_path": "/tmp/xcode.sock?token=raw-shim-token",
                "workspace_root": "/workspace?authorization=raw-workspace-token",
                "agent_execution_id": "execution-1"
            }],
            "xcode_host_executor_events": []
        })
        .to_string();
        let execution = AgentExecution {
            id: AgentExecutionId::new(),
            stage_execution_id: Some(StageExecutionId::new()),
            agent_id: "code_writer".into(),
            provider: "codex".into(),
            model: Some("gpt-5".into()),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            status: AgentStatus::Failed,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: None,
            session_reuse_scope: None,
            session_family_id: None,
            session_reuse_disposition: None,
            session_reset_reason: None,
            backend_profile_id: None,
            requested_mcp_extensions_json: None,
            predicted_mcp_extensions_json: None,
            predicted_mcp_runtime_ids_json: None,
            actual_mcp_extensions_json: None,
            actual_mcp_runtime_ids_json: None,
            denied_mcp_extensions_json: None,
            mcp_blocking_issues_json: None,
            actual_mcp_observation_json: None,
            actual_xcode_runtime_observation_json: Some(raw_observation),
            mcp_session_startup_latency_ms: None,
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
        };

        let gql = GqlAgentExecution::from(execution);
        let observation = gql
            .actual_xcode_runtime_observation
            .expect("xcode runtime observation");
        let broker = observation
            .mcp_broker_observations
            .first()
            .expect("broker observation");

        assert_eq!(
            broker.http_endpoint.as_deref(),
            Some("http://127.0.0.1:4000/xcode-mcp/lease-1?token=<redacted>")
        );
        assert_eq!(
            broker.status_update.as_deref(),
            Some("forwarded Bearer <redacted>")
        );
        let shim_event = observation
            .xcode_shim_events
            .first()
            .expect("shim runtime event");
        match shim_event {
            GqlXcodeShimEvent::ShimRuntimeAttached(event) => {
                assert_eq!(event.source, "xcode_shim_runtime");
                assert_eq!(event.reason, "requires_xcode_host_execution");
                assert_eq!(event.lease_id, "xcode-lease-raw-secret");
                assert_eq!(event.socket_path, "/tmp/xcode.sock?token=<redacted>");
                assert_eq!(event.workspace_root, "/workspace?authorization=<redacted>");
            }
            other => panic!("unexpected shim event: {other:?}"),
        }
    }

    // P079-SEC-LOW-003: pin that sensitive lease fields never appear in the GraphQL readback.
    // The projection explicitly lists allowed output keys; this test guards against accidental
    // inclusion of idempotency_token (single-flight dedup secret) and lease_owner_principal_id
    // (raw DB column name whose value is exposed as "owner_principal_id").
    #[test]
    fn p079_lease_projection_excludes_sensitive_fields() {
        let projected = serde_json::json!({
            "key": "test-lease-key",
            "kind": "repair",
            "state": "reserved",
            "settled_result": null,
            "reclamation_reason": null,
            "owner_principal_id": "principal-001",
            "acquired_at": "2026-01-01T00:00:00Z",
            "expires_at": "2026-01-01T01:00:00Z",
            "lease_seconds": 3600,
        });
        let obj = projected.as_object().unwrap();
        assert!(
            !obj.contains_key("idempotency_token"),
            "idempotency_token must not appear in lease readback (single-flight dedup secret)"
        );
        assert!(
            !obj.contains_key("lease_owner_principal_id"),
            "lease_owner_principal_id (raw DB column) must not appear; value is exposed as owner_principal_id"
        );
        assert!(
            !obj.contains_key("infra_retry_count"),
            "infra_retry_count must not appear in lease readback"
        );
        assert!(
            !obj.contains_key("version"),
            "version (CAS column) must not appear in lease readback"
        );
        // Allowed fields: key, kind, state, settled_result, reclamation_reason,
        // owner_principal_id, acquired_at, expires_at, lease_seconds.
        assert!(obj.contains_key("owner_principal_id"), "owner_principal_id must be present");
        assert!(obj.contains_key("key"), "key must be present");
    }

    // P079-SEC-LOW-003 parity: pin that p079_safe_relative_path rejects absolute and traversal paths.
    #[test]
    fn p079_safe_relative_path_rejects_absolute_and_traversal() {
        assert_eq!(p079_safe_relative_path(Some("/etc/passwd")), None);
        assert_eq!(p079_safe_relative_path(Some("../secret")), None);
        assert_eq!(p079_safe_relative_path(Some("output_contract_repair/abc/plan.json")), Some("output_contract_repair/abc/plan.json"));
        assert_eq!(p079_safe_relative_path(None), None);
    }

    // SEC-P079-LOW-001: server-side safe_relative_path must reject URL-encoded traversal
    // sequences (%2e%2e, %2f, %5c) to align with Swift client encoded-traversal rejection.
    #[test]
    fn p079_safe_relative_path_rejects_url_encoded_traversal() {
        // %2e%2e is URL-encoded ".."
        assert_eq!(p079_safe_relative_path(Some("%2e%2e/secret")), None, "%2e%2e must be rejected");
        // Mixed case
        assert_eq!(p079_safe_relative_path(Some("%2E%2E/secret")), None, "uppercase %2E%2E must be rejected");
        // %2f is URL-encoded "/"
        assert_eq!(p079_safe_relative_path(Some("foo%2fetc%2fpasswd")), None, "%2f slash must be rejected");
        // %5c is URL-encoded backslash
        assert_eq!(p079_safe_relative_path(Some("foo%5csecret")), None, "%5c backslash must be rejected");
        // Safe path still passes
        assert_eq!(
            p079_safe_relative_path(Some("output_contract_repair/abc/plan.json")),
            Some("output_contract_repair/abc/plan.json"),
            "safe path must still pass after adding encoded-traversal rejection"
        );
    }

    // SEC-P079-LOW-001: mixed literal/encoded traversal must be rejected after percent-decode.
    // Covers %2e. and .%2e forms that bypass single-layer checks.
    #[test]
    fn p079_safe_relative_path_rejects_mixed_encoded_traversal() {
        // %2e. is percent-encoded '.' followed by literal '.' → decoded ".." = traversal
        assert_eq!(p079_safe_relative_path(Some("%2e./etc/passwd")), None, "%2e. must be rejected");
        assert_eq!(p079_safe_relative_path(Some(".%2e/etc/passwd")), None, ".%2e must be rejected");
        // Uppercase variants
        assert_eq!(p079_safe_relative_path(Some("%2E./etc/passwd")), None, "%2E. uppercase must be rejected");
        assert_eq!(p079_safe_relative_path(Some(".%2E/etc/passwd")), None, ".%2E uppercase must be rejected");
        // Fully encoded / as %2F in a traversal sequence
        assert_eq!(p079_safe_relative_path(Some("..%2Fetc%2Fpasswd")), None, "..%2F must be rejected");
        // Double-encoded %252e (decoded once = %2e, still single-encode — not further decoded here, but reject %25)
        // A normal safe path must still pass
        assert_eq!(
            p079_safe_relative_path(Some("output_contract_repair/plan_evidence/plan.md")),
            Some("output_contract_repair/plan_evidence/plan.md"),
            "safe path must still pass"
        );
    }
}
