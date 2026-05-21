use async_graphql::*;
use db::repos::agent_execution_discovery_diagnostics;
use db::repos::agent_execution_runtime_facts;
use db::repos::code_writer_completion_receipts;
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

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "AgentFailureKind", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum AgentFailureKind {
    ProviderQuota,
    ProviderPermissionRequired,
    ProviderPermissionRejected,
    ProviderTimeout,
    ProviderInternalError,
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
    pub invocation_owner_key: Option<String>,
    pub session_reuse_scope: Option<String>,
    pub session_family_id: Option<String>,
    pub session_reuse_disposition: Option<String>,
    pub session_reuse_reason: Option<String>,
    pub session_reset_reason: Option<String>,
    pub provider_session_id: Option<String>,
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
    pub provider: String,
    pub model: Option<String>,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
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
            provider: execution.provider,
            model: execution.model,
            status: execution.status.to_string(),
            started_at: execution.started_at.to_rfc3339(),
            completed_at: execution.completed_at.map(|t| t.to_rfc3339()),
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
}

impl GqlAgentExecutionRuntimeFacts {
    fn from_facts_and_execution(
        facts: domain::agent::AgentExecutionRuntimeFacts,
        execution: &GqlAgentExecution,
        lineage: Option<&SessionLineage>,
        generation: Option<&SessionGeneration>,
        include_operator_debug: bool,
    ) -> Self {
        let provider_session_id =
            generation.and_then(|generation| generation.provider_session_id.clone());
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
            },
            valid_required_outputs: facts.valid_required_outputs,
            late_output_count: facts.late_output_count,
            ignored_late_output_count: facts.ignored_late_output_count,
            session_lineage_id: execution.session_lineage_id.clone().map(ID),
            session_generation_id: execution.session_generation_id.clone().map(ID),
            invocation_owner_key: execution.invocation_owner_key.clone(),
            session_reuse_scope: execution.session_reuse_scope.clone(),
            session_family_id: execution.session_family_id.clone(),
            session_reuse_disposition: execution.session_reuse_disposition.clone(),
            session_reuse_reason: facts.session_reuse_reason,
            session_reset_reason: execution.session_reset_reason.clone(),
            provider_session_id,
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
}
