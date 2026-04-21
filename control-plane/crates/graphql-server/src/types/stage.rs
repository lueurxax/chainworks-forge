use async_graphql::*;
use db::repos::projections::StageSummaryRow;
use domain::ids::StageExecutionId;
use domain::stage::StageExecution;
use domain::xcode_runtime::{
    McpBrokerObservation, XcodeHostExecutorEvent, XcodeRuntimeObservation, XcodeShimEvent,
    XcodeShimInvocationEvent, XcodeShimWarningEvent,
};

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
}

#[derive(SimpleObject, Clone, Debug)]
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
}

impl From<domain::agent::AgentExecution> for GqlAgentExecution {
    fn from(execution: domain::agent::AgentExecution) -> Self {
        GqlAgentExecution {
            id: ID(execution.id.to_string()),
            stage_execution_id: ID(execution.stage_execution_id.to_string()),
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
                .map(GqlXcodeRuntimeObservation::from),
            mcp_session_startup_latency_ms: execution.mcp_session_startup_latency_ms,
        }
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
    ShimInvocation(GqlXcodeShimInvocationEvent),
    Warning(GqlXcodeShimWarningEvent),
}

impl From<XcodeShimEvent> for GqlXcodeShimEvent {
    fn from(event: XcodeShimEvent) -> Self {
        match event {
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
        }
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
        }
    }
}
