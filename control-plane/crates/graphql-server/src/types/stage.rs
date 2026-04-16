use async_graphql::*;
use db::repos::projections::StageSummaryRow;
use domain::ids::StageExecutionId;
use domain::stage::StageExecution;

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
            mcp_session_startup_latency_ms: execution.mcp_session_startup_latency_ms,
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
