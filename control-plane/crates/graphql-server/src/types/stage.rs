use async_graphql::*;
use db::repos::agent_execution_discovery_diagnostics;
use db::repos::agent_execution_runtime_facts;
use db::repos::projections::StageSummaryRow;
use db::repos::sessions;
use domain::ids::StageExecutionId;
use domain::session::{SessionGeneration, SessionLineage};
use domain::stage::StageExecution;

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
    pub projection_present: bool,
    pub projection_updated_at: Option<String>,
    pub projection_lag: bool,
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
            projection_present: false,
            projection_updated_at: None,
            projection_lag: true,
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
        gql.projection_present = r.projection_present;
        gql.projection_updated_at = r.projection_updated_at;
        gql.projection_lag = r.projection_lag;
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
            projection_present: r.projection_present,
            projection_updated_at: r.projection_updated_at,
            projection_lag: r.projection_lag,
        }
    }
}
