use async_graphql::*;
use db::repos::projections::RunProjectionRow;
use domain::artifact_contracts::{
    HandoffTaskSummary, ImplementationSelfAssessmentSummary, RemainingCodeTaskSummary,
    TargetStageSummary, ValidationIssue,
};
use domain::mediation::LeadConflictMediationRecord;
use domain::run::Run;
use domain::workflow_conflict::{
    workflow_conflict_suggested_operator_action, CandidateTransitionEvaluation,
    CandidateTransitionResult, WorkflowConflictReason, WorkflowConflictRecord,
    WorkflowConflictStatus,
};

use crate::types::p031::{freshness_from_projection_lag, GqlFreshnessState};

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlRun {
    pub id: ID,
    pub idea_id: ID,
    pub status: String,
    pub workflow_id: String,
    pub workflow_title: String,
    pub workspace_root: String,
    pub artifact_root: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub cancellation_requested_at: Option<String>,
    pub cancellation_settled_at: Option<String>,
    pub cancellation_settlement_log: Option<String>,
    pub cancellation_settlement_summary: Option<String>,
    pub delivery_configuration_json: Option<String>,
    pub review_routing_json: Option<String>,
    pub workflow_family: Option<String>,
    pub project_key: Option<String>,
    pub risk_class: Option<String>,
    pub stack: Option<String>,
    pub workflow_snapshot_hash: Option<String>,
    pub catalog_snapshot_hash: Option<String>,
    pub drift_detected_at: Option<String>,
    pub drift_details_json: Option<String>,
    /// P050: Per-run meta root (read-only).
    pub chainworks_meta_root: Option<String>,
    /// Stage counts from the projection layer; None when reading a single run by ID.
    pub total_stages: Option<i64>,
    pub completed_stages: Option<i64>,
    pub failed_stages: Option<i64>,
    pub pending_approvals: Option<i64>,
    pub delivery_preflight_json: Option<String>,
    pub projection_present: bool,
    pub projection_updated_at: Option<String>,
    pub projection_lag: bool,
    pub freshness_state: GqlFreshnessState,
    pub active_artifact_index_json: Option<String>,
    pub run_state_projection_json: Option<String>,
    pub operator_overrides_json: Option<String>,
    pub workflow_conflict: Option<GqlWorkflowConflict>,
    pub implementation_handoff_status_json: Option<Json<serde_json::Value>>,
    pub legacy_discovery_overrides_json: Option<String>,
    pub implementation_self_assessment_summary: Option<GqlImplementationSelfAssessmentSummary>,
    pub main_sync_readback_json: Option<Json<serde_json::Value>>,
    pub knowledge_capsule_readback_json: Option<Json<serde_json::Value>>,
}

impl From<Run> for GqlRun {
    fn from(run: Run) -> Self {
        GqlRun {
            id: ID(run.id.to_string()),
            idea_id: ID(run.idea_id.to_string()),
            status: run.status.to_string(),
            workflow_id: run.workflow_id,
            workflow_title: run.workflow_title,
            workspace_root: run.workspace_root,
            artifact_root: run.artifact_root,
            started_at: run.started_at.to_rfc3339(),
            completed_at: run.completed_at.map(|t| t.to_rfc3339()),
            cancellation_requested_at: run.cancellation_requested_at.map(|t| t.to_rfc3339()),
            cancellation_settled_at: run.cancellation_settled_at.map(|t| t.to_rfc3339()),
            cancellation_settlement_log: run.cancellation_settlement_log,
            cancellation_settlement_summary: None,
            delivery_configuration_json: run.delivery_configuration_json,
            review_routing_json: run.review_routing_json,
            delivery_preflight_json: run.delivery_preflight_json,
            workflow_family: run.workflow_family,
            project_key: run.project_key,
            risk_class: run.risk_class,
            stack: run.stack,
            workflow_snapshot_hash: run.workflow_snapshot_hash,
            catalog_snapshot_hash: run.catalog_snapshot_hash,
            drift_detected_at: run.drift_detected_at.map(|t| t.to_rfc3339()),
            drift_details_json: run.drift_details_json,
            chainworks_meta_root: run.chainworks_meta_root,
            total_stages: None,
            completed_stages: None,
            failed_stages: None,
            pending_approvals: None,
            projection_present: false,
            projection_updated_at: None,
            projection_lag: true,
            freshness_state: GqlFreshnessState::ProjectionLag,
            active_artifact_index_json: None,
            run_state_projection_json: None,
            operator_overrides_json: None,
            workflow_conflict: None,
            implementation_handoff_status_json: None,
            legacy_discovery_overrides_json: None,
            implementation_self_assessment_summary: None,
            main_sync_readback_json: None,
            knowledge_capsule_readback_json: None,
        }
    }
}

impl GqlRun {
    pub fn from_projection_and_run(projection: RunProjectionRow, run: Run) -> Self {
        let mut gql = GqlRun::from(run);
        gql.status = projection.status;
        gql.cancellation_settlement_summary = projection.cancellation_settlement_summary;
        gql.chainworks_meta_root = projection.chainworks_meta_root.or(gql.chainworks_meta_root);
        gql.total_stages = Some(projection.total_stages);
        gql.completed_stages = Some(projection.completed_stages);
        gql.failed_stages = Some(projection.failed_stages);
        gql.pending_approvals = Some(projection.pending_approvals);
        gql.projection_present = projection.projection_present;
        gql.projection_updated_at = projection.projection_updated_at;
        gql.projection_lag = projection.projection_lag;
        gql.freshness_state = freshness_from_projection_lag(gql.projection_lag);
        gql
    }
}

impl From<RunProjectionRow> for GqlRun {
    fn from(r: RunProjectionRow) -> Self {
        GqlRun {
            id: ID(r.id),
            idea_id: ID(r.idea_id),
            status: r.status,
            workflow_id: r.workflow_id,
            workflow_title: r.workflow_title,
            workspace_root: r.workspace_root,
            artifact_root: r.artifact_root,
            started_at: r.started_at,
            completed_at: r.completed_at,
            cancellation_requested_at: r.cancellation_requested_at,
            cancellation_settled_at: r.cancellation_settled_at,
            cancellation_settlement_log: None,
            cancellation_settlement_summary: r.cancellation_settlement_summary,
            delivery_configuration_json: None,
            review_routing_json: None,
            delivery_preflight_json: None,
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: None,
            catalog_snapshot_hash: None,
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: r.chainworks_meta_root,
            total_stages: Some(r.total_stages),
            completed_stages: Some(r.completed_stages),
            failed_stages: Some(r.failed_stages),
            pending_approvals: Some(r.pending_approvals),
            projection_present: r.projection_present,
            projection_updated_at: r.projection_updated_at,
            projection_lag: r.projection_lag,
            freshness_state: freshness_from_projection_lag(r.projection_lag),
            active_artifact_index_json: None,
            run_state_projection_json: None,
            operator_overrides_json: None,
            workflow_conflict: None,
            implementation_handoff_status_json: None,
            legacy_discovery_overrides_json: None,
            implementation_self_assessment_summary: None,
            main_sync_readback_json: None,
            knowledge_capsule_readback_json: None,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlWorkflowConflict {
    pub conflict_id: ID,
    pub conflict_fingerprint: String,
    pub run_id: ID,
    pub stage_execution_id: Option<ID>,
    pub lineage_id: Option<String>,
    pub current_state_id: String,
    pub reason: GqlWorkflowConflictReason,
    pub operator_label: String,
    pub status: GqlWorkflowConflictStatus,
    pub candidate_transitions: Vec<GqlCandidateTransitionEvaluation>,
    pub candidate_transition_hash: String,
    pub advisory_evidence_refs: Vec<String>,
    pub lead_agent_id: Option<String>,
    pub mediation_record_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub resolved_at: Option<String>,
    pub superseded_by_conflict_id: Option<String>,
    pub resolution_record_json: Option<Json<serde_json::Value>>,
    pub terminal_failure_reason: Option<String>,
    pub diagnostic_redaction_tier: String,
    pub suggested_operator_action: Option<String>,
    /// P017 Phase B: Lead mediation readback (read-only, no mutations).
    pub lead_mediation: Option<GqlLeadMediation>,
}

/// P017 Phase B: Read-only mediation readback projected from the lead_conflict_mediations table.
#[derive(SimpleObject, Clone, Debug)]
pub struct GqlLeadMediation {
    pub id: ID,
    pub conflict_id: ID,
    pub lead_agent_id: String,
    pub status: String,
    pub resolution_mode: Option<String>,
    pub chosen_action: Option<String>,
    pub chosen_next_state_id: Option<String>,
    pub chosen_next_state_label: Option<String>,
    pub sanitized_progress: Option<String>,
    pub status_updates: Vec<GqlLeadMediationStatusUpdate>,
    pub validation_errors: Option<Json<serde_json::Value>>,
    pub confirmation_subject_id: Option<String>,
    pub superseded_by_event_ref: Option<String>,
    pub cost_summary: Option<Json<serde_json::Value>>,
    /// API-001 (P017 R2 audit): one entry per mediation-owned `agent_executions`
    /// row, ordered by `started_at`. Lets operators inspect the mediation's
    /// runtime facts, watchdog outcome, artifacts, and provider/timing details
    /// scoped to the workflow conflict.
    pub execution_attempts: Vec<GqlMediationExecutionAttempt>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlLeadMediationStatusUpdate {
    pub status: String,
    pub sanitized_progress: Option<String>,
    pub updated_at: String,
    pub attempt_number: i32,
}

/// P017 R2 / API-001: owner-aware mediation execution attempt projected
/// directly from `agent_executions`. `stage_execution_id` is nullable
/// because mediation-owned executions have no stage by design.
#[derive(SimpleObject, Clone, Debug)]
pub struct GqlMediationExecutionAttempt {
    pub agent_execution_id: ID,
    pub owner_kind: String,
    pub owner_id: String,
    pub mediation_record_id: ID,
    pub stage_execution_id: Option<ID>,
    pub agent_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub attempt_number: i32,
    pub runtime_facts: Option<Json<serde_json::Value>>,
    pub watchdog: Option<Json<serde_json::Value>>,
    /// Per-attempt cost is null until cost rollup lands on `agent_executions`
    /// (audit OPS-001 follow-up). Aggregate cost is on `cost_summary`.
    pub cost: Option<Json<serde_json::Value>>,
    /// Transcript ref placeholder — populated when the transcript-artifact
    /// linkage lands on `AgentExecution`.
    pub transcript_ref: Option<Json<serde_json::Value>>,
    pub artifacts: Vec<GqlMediationAttemptArtifact>,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlMediationAttemptArtifact {
    pub id: ID,
    pub name: String,
    pub format: String,
    pub file_path: String,
    pub report_kind: Option<String>,
    pub is_pinned: bool,
    /// P017 R5 / API-003: tiered attribution label mirrored from MCP.
    /// One of `"transcript_direct"` (tier 1, via
    /// `agent_executions.transcript_artifact_id`),
    /// `"execution_id_direct"` (tier 2, via
    /// `artifacts.agent_execution_id` — the cross-retry isolation
    /// guarantee), or `"agent_id_correlation"` (tier 3 legacy
    /// fallback for pre-R5 attempts).
    pub linkage: String,
}

impl From<&LeadConflictMediationRecord> for GqlLeadMediation {
    /// Synchronous projection used as a fallback / initial constructor.
    /// Always returns `execution_attempts: vec![]` and `attempt_number: 1`
    /// for the synthetic status update. Async builders below populate the
    /// owner-aware fields when a SqlitePool is available.
    fn from(record: &LeadConflictMediationRecord) -> Self {
        GqlLeadMediation {
            id: ID(record.id.clone()),
            conflict_id: ID(record.conflict_id.clone()),
            lead_agent_id: record.lead_agent_id.clone(),
            status: record.status.to_string(),
            resolution_mode: domain::mediation::derive_resolution_mode(record),
            chosen_action: record.chosen_action.clone(),
            chosen_next_state_id: record.chosen_next_state_id.clone(),
            chosen_next_state_label: record.chosen_next_state_label.clone(),
            sanitized_progress: record.sanitized_progress.clone(),
            status_updates: vec![GqlLeadMediationStatusUpdate {
                status: record.status.to_string(),
                sanitized_progress: record.sanitized_progress.clone(),
                updated_at: record.updated_at.to_rfc3339(),
                attempt_number: 1,
            }],
            validation_errors: record
                .validation_errors_json
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .map(Json),
            confirmation_subject_id: record.confirmation_subject_id.clone(),
            superseded_by_event_ref: record.superseded_by_event_ref.clone(),
            cost_summary: record
                .cost_summary_json
                .as_ref()
                .and_then(|json| serde_json::from_str(json).ok())
                .map(Json),
            execution_attempts: Vec::new(),
        }
    }
}

impl GqlLeadMediation {
    /// API-001 (P017 R2 audit): build the GraphQL projection enriched with
    /// mediation-owned execution attempts.
    ///
    /// Pulls every `agent_executions` row owned by this mediation, joins
    /// runtime facts, and projects them under `execution_attempts`. The
    /// synthetic single status update's `attempt_number` is replaced with
    /// the durable count of mediation-owned executions (was hard-coded `1`).
    pub async fn build_with_attempts(
        pool: &sqlx::SqlitePool,
        record: &LeadConflictMediationRecord,
    ) -> anyhow::Result<Self> {
        let mut projection = GqlLeadMediation::from(record);
        let attempts = build_mediation_execution_attempts(pool, record).await?;
        if !attempts.is_empty() {
            // Sync attempt_number on the status_updates entry to the durable count.
            if let Some(latest) = projection.status_updates.first_mut() {
                latest.attempt_number = attempts.len() as i32;
            }
        }
        projection.execution_attempts = attempts;
        Ok(projection)
    }
}

async fn build_mediation_execution_attempts(
    pool: &sqlx::SqlitePool,
    record: &LeadConflictMediationRecord,
) -> anyhow::Result<Vec<GqlMediationExecutionAttempt>> {
    let executions = db::repos::agent_executions::list_by_mediation_id(pool, &record.id).await?;
    let run_artifacts: Vec<domain::artifact::Artifact> =
        match record.run_id.parse::<domain::ids::RunId>() {
            Ok(run_id) => db::repos::artifacts::list_by_run(pool, run_id)
                .await
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };

    let mut attempts = Vec::with_capacity(executions.len());
    for (idx, execution) in executions.iter().enumerate() {
        let attempt_number = (idx + 1) as i32;
        let runtime_facts =
            db::repos::agent_execution_runtime_facts::find_by_execution_id(pool, execution.id)
                .await?;

        let runtime_facts_json = runtime_facts.as_ref().map(|f| {
            Json(serde_json::json!({
                "valid_required_outputs": f.valid_required_outputs,
                "failure_kind": f.failure_kind.as_ref().map(|k| k.to_string()),
                "failure_message_redacted": f.failure_message_redacted,
                "output_settlement": format!("{:?}", f.output_settlement).to_lowercase(),
                "late_output_count": f.late_output_count,
                "ignored_late_output_count": f.ignored_late_output_count,
                "operator_action_hint": f.operator_action_hint.as_ref().map(|h| format!("{:?}", h).to_lowercase()),
            }))
        });
        let watchdog_json = runtime_facts.as_ref().map(|f| {
            Json(serde_json::json!({
                "supervision_classification": f.supervision_classification,
                "provider_exit_status": f.provider_exit_status,
                "transport_error_code": f.transport_error_code,
                "retry_after": f.retry_after.map(|t| t.to_rfc3339()),
            }))
        });
        // P017 R5 / API-003: tiered artifact attribution — see MCP
        // mirror for the full rationale. Tier 1: direct transcript
        // artifact. Tier 2: direct execution-attempt FK
        // (`artifacts.agent_execution_id`) — cross-retry isolation.
        // Tier 3: legacy `agent_id` correlation only as fallback for
        // pre-R5 attempts with no direct linkage at all.
        let mut seen_artifact_ids: std::collections::HashSet<String> = Default::default();
        let mut artifact_refs: Vec<GqlMediationAttemptArtifact> = Vec::new();
        let transcript_artifact = if let Some(ref tid) = execution.transcript_artifact_id {
            if let Ok(parsed_id) = tid.parse::<domain::ids::ArtifactId>() {
                let found = db::repos::artifacts::find_by_id(pool, parsed_id)
                    .await
                    .ok()
                    .flatten();
                if let Some(ref a) = found {
                    seen_artifact_ids.insert(a.id.to_string());
                    artifact_refs.push(GqlMediationAttemptArtifact {
                        id: ID(a.id.to_string()),
                        name: a.name.clone(),
                        format: format!("{:?}", a.format).to_lowercase(),
                        file_path: a.file_path.clone(),
                        report_kind: a.report_kind.clone(),
                        is_pinned: a.is_pinned,
                        linkage: "transcript_direct".to_string(),
                    });
                }
                found
            } else {
                None
            }
        } else {
            None
        };
        // Tier 2: direct execution-attempt FK linkage.
        let direct_artifacts =
            db::repos::artifacts::list_by_agent_execution(pool, &execution.id.to_string())
                .await
                .unwrap_or_default();
        let attempt_has_direct_link = !direct_artifacts.is_empty();
        for a in direct_artifacts.iter() {
            if !seen_artifact_ids.insert(a.id.to_string()) {
                continue;
            }
            artifact_refs.push(GqlMediationAttemptArtifact {
                id: ID(a.id.to_string()),
                name: a.name.clone(),
                format: format!("{:?}", a.format).to_lowercase(),
                file_path: a.file_path.clone(),
                report_kind: a.report_kind.clone(),
                is_pinned: a.is_pinned,
                linkage: "execution_id_direct".to_string(),
            });
        }
        // Tier 3: legacy fallback (only when no direct linkage exists).
        if !attempt_has_direct_link && transcript_artifact.is_none() {
            for a in run_artifacts.iter() {
                if !seen_artifact_ids.insert(a.id.to_string()) {
                    continue;
                }
                if a.agent_id != execution.agent_id {
                    continue;
                }
                artifact_refs.push(GqlMediationAttemptArtifact {
                    id: ID(a.id.to_string()),
                    name: a.name.clone(),
                    format: format!("{:?}", a.format).to_lowercase(),
                    file_path: a.file_path.clone(),
                    report_kind: a.report_kind.clone(),
                    is_pinned: a.is_pinned,
                    linkage: "agent_id_correlation".to_string(),
                });
            }
        }

        // P017 R4 / API-002: cost + transcript_ref now populated from
        // per-execution columns when present.
        let cost_json = match (
            execution.total_cost_cents,
            execution.input_tokens,
            execution.output_tokens,
            execution.cached_input_tokens,
        ) {
            (None, None, None, None) => None,
            (cents, input, output, cached) => Some(Json(serde_json::json!({
                "total_cost_cents": cents,
                "input_tokens": input,
                "output_tokens": output,
                "cached_input_tokens": cached,
            }))),
        };
        let transcript_json = transcript_artifact.as_ref().map(|a| {
            Json(serde_json::json!({
                "artifact_id": a.id.to_string(),
                "file_path": a.file_path,
                "format": format!("{:?}", a.format).to_lowercase(),
            }))
        });

        attempts.push(GqlMediationExecutionAttempt {
            agent_execution_id: ID(execution.id.to_string()),
            owner_kind: execution
                .owner_kind
                .clone()
                .unwrap_or_else(|| "lead_conflict_mediation".to_string()),
            owner_id: execution
                .owner_id
                .clone()
                .unwrap_or_else(|| record.id.clone()),
            mediation_record_id: ID(record.id.clone()),
            stage_execution_id: execution.stage_execution_id.map(|id| ID(id.to_string())),
            agent_id: execution.agent_id.clone(),
            provider: execution.provider.clone(),
            model: execution.model.clone(),
            status: execution.status.to_string(),
            started_at: execution.started_at.to_rfc3339(),
            completed_at: execution.completed_at.map(|t| t.to_rfc3339()),
            attempt_number,
            runtime_facts: runtime_facts_json,
            watchdog: watchdog_json,
            cost: cost_json,
            transcript_ref: transcript_json,
            artifacts: artifact_refs,
        });
    }
    Ok(attempts)
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlCandidateTransitionEvaluation {
    pub transition_id: String,
    pub from_state_id: String,
    pub to_state_id: String,
    pub condition_expression_id: Option<String>,
    pub result: GqlCandidateTransitionResult,
    pub required_artifacts: Vec<String>,
    pub missing_artifacts: Vec<String>,
    pub missing_fields: Vec<String>,
    pub source_artifact_ids: Vec<String>,
    pub source_agent_execution_id: Option<String>,
    pub sanitized_diagnostic: Option<String>,
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlWorkflowConflictReason {
    InvalidNextStageHint,
    NoDeclarativeTransitionMatched,
    MultipleDeclarativeTransitionsMatchedWithoutTieBreak,
    RequiredArtifactOrFieldMissingForTransition,
    AggregateTransitionTruthConflicted,
    WorkflowConflictUnverifiable,
    ImplementationHandoffUnavailable,
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlWorkflowConflictStatus {
    Unresolved,
    LeadMediationPending,
    OperatorConfirmationRequired,
    Resolved,
    Superseded,
    TerminalUnverifiable,
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlCandidateTransitionResult {
    Matched,
    NotMatched,
    MissingInput,
    InvalidExpression,
    EvaluationError,
}

impl From<WorkflowConflictRecord> for GqlWorkflowConflict {
    fn from(record: WorkflowConflictRecord) -> Self {
        let suggested_operator_action =
            workflow_conflict_suggested_operator_action(&record).map(str::to_string);
        GqlWorkflowConflict {
            conflict_id: ID(record.conflict_id),
            conflict_fingerprint: record.conflict_fingerprint,
            run_id: ID(record.run_id),
            stage_execution_id: record.stage_execution_id.map(ID),
            lineage_id: record.lineage_id,
            current_state_id: record.current_state_id,
            reason: record.reason.into(),
            operator_label: record.operator_label,
            status: record.status.into(),
            candidate_transitions: record
                .candidate_transitions
                .into_iter()
                .map(GqlCandidateTransitionEvaluation::from)
                .collect(),
            candidate_transition_hash: record.candidate_transition_hash,
            advisory_evidence_refs: record.advisory_evidence_refs,
            lead_agent_id: record.lead_agent_id,
            mediation_record_id: record.mediation_record_id,
            created_at: record.created_at.to_rfc3339(),
            updated_at: record.updated_at.to_rfc3339(),
            resolved_at: record.resolved_at.map(|dt| dt.to_rfc3339()),
            superseded_by_conflict_id: record.superseded_by_conflict_id,
            resolution_record_json: record.resolution_record_json.map(Json),
            terminal_failure_reason: record.terminal_failure_reason,
            diagnostic_redaction_tier: record.diagnostic_redaction_tier,
            suggested_operator_action,
            lead_mediation: None, // Populated by enrichment when mediation_record_id is present
        }
    }
}

impl From<CandidateTransitionEvaluation> for GqlCandidateTransitionEvaluation {
    fn from(candidate: CandidateTransitionEvaluation) -> Self {
        GqlCandidateTransitionEvaluation {
            transition_id: candidate.transition_id,
            from_state_id: candidate.from_state_id,
            to_state_id: candidate.to_state_id,
            condition_expression_id: candidate.condition_expression_id,
            result: candidate.result.into(),
            required_artifacts: candidate.required_artifacts,
            missing_artifacts: candidate.missing_artifacts,
            missing_fields: candidate.missing_fields,
            source_artifact_ids: candidate.source_artifact_ids,
            source_agent_execution_id: candidate.source_agent_execution_id,
            sanitized_diagnostic: candidate.sanitized_diagnostic,
        }
    }
}

impl From<WorkflowConflictReason> for GqlWorkflowConflictReason {
    fn from(reason: WorkflowConflictReason) -> Self {
        match reason {
            WorkflowConflictReason::InvalidNextStageHint => Self::InvalidNextStageHint,
            WorkflowConflictReason::NoDeclarativeTransitionMatched => {
                Self::NoDeclarativeTransitionMatched
            }
            WorkflowConflictReason::MultipleDeclarativeTransitionsMatchedWithoutTieBreak => {
                Self::MultipleDeclarativeTransitionsMatchedWithoutTieBreak
            }
            WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition => {
                Self::RequiredArtifactOrFieldMissingForTransition
            }
            WorkflowConflictReason::AggregateTransitionTruthConflicted => {
                Self::AggregateTransitionTruthConflicted
            }
            WorkflowConflictReason::WorkflowConflictUnverifiable => {
                Self::WorkflowConflictUnverifiable
            }
            WorkflowConflictReason::ImplementationHandoffUnavailable => {
                Self::ImplementationHandoffUnavailable
            }
        }
    }
}

impl From<WorkflowConflictStatus> for GqlWorkflowConflictStatus {
    fn from(status: WorkflowConflictStatus) -> Self {
        match status {
            WorkflowConflictStatus::Unresolved => Self::Unresolved,
            WorkflowConflictStatus::LeadMediationPending => Self::LeadMediationPending,
            WorkflowConflictStatus::OperatorConfirmationRequired => {
                Self::OperatorConfirmationRequired
            }
            WorkflowConflictStatus::Resolved => Self::Resolved,
            WorkflowConflictStatus::Superseded => Self::Superseded,
            WorkflowConflictStatus::TerminalUnverifiable => Self::TerminalUnverifiable,
        }
    }
}

impl From<CandidateTransitionResult> for GqlCandidateTransitionResult {
    fn from(result: CandidateTransitionResult) -> Self {
        match result {
            CandidateTransitionResult::Matched => Self::Matched,
            CandidateTransitionResult::NotMatched => Self::NotMatched,
            CandidateTransitionResult::MissingInput => Self::MissingInput,
            CandidateTransitionResult::InvalidExpression => Self::InvalidExpression,
            CandidateTransitionResult::EvaluationError => Self::EvaluationError,
        }
    }
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlImplementationSelfAssessmentSummary {
    pub contract_id: String,
    pub artifact_path: String,
    pub status: String,
    pub implementation_complete: Option<bool>,
    pub verification_green: Option<bool>,
    pub remaining_code_task_count: Option<i32>,
    pub blocking_remaining_code_task_count: Option<i32>,
    pub handoff_task_count: Option<i32>,
    pub blocking_review_handoff_task_count: Option<i32>,
    pub owner_class_counts: Json<serde_json::Value>,
    pub target_stage_summaries: Vec<GqlTargetStageSummary>,
    pub remaining_code_tasks: Vec<GqlRemainingCodeTaskSummary>,
    pub handoff_tasks: Vec<GqlHandoffTaskSummary>,
    pub known_risks: Vec<String>,
    pub tests_run: Vec<String>,
    pub docs_impacted: Vec<String>,
    pub validation_errors: Vec<GqlValidationIssue>,
    pub warnings: Vec<GqlValidationIssue>,
    pub raw_artifact_available: bool,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlTargetStageSummary {
    pub target_stage: String,
    pub count: i32,
    pub blocking_review_count: i32,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlRemainingCodeTaskSummary {
    pub summary: String,
    pub owner: String,
    pub blocking: bool,
    pub evidence: String,
    pub source_pointer: String,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlHandoffTaskSummary {
    pub summary: String,
    pub owner_class: String,
    pub target_stage: String,
    pub blocking_review: bool,
    pub evidence: String,
    pub source_pointer: String,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlValidationIssue {
    pub code: String,
    pub message: String,
    pub pointer: String,
}

impl From<ImplementationSelfAssessmentSummary> for GqlImplementationSelfAssessmentSummary {
    fn from(summary: ImplementationSelfAssessmentSummary) -> Self {
        GqlImplementationSelfAssessmentSummary {
            contract_id: summary.contract_id,
            artifact_path: public_artifact_path(&summary.artifact_path),
            status: summary.status.to_string(),
            implementation_complete: summary.implementation_complete,
            verification_green: summary.verification_green,
            remaining_code_task_count: optional_saturating_i32(summary.remaining_code_task_count),
            blocking_remaining_code_task_count: optional_saturating_i32(
                summary.blocking_remaining_code_task_count,
            ),
            handoff_task_count: optional_saturating_i32(summary.handoff_task_count),
            blocking_review_handoff_task_count: optional_saturating_i32(
                summary.blocking_review_handoff_task_count,
            ),
            owner_class_counts: Json(
                serde_json::to_value(summary.owner_class_counts)
                    .unwrap_or_else(|_| serde_json::json!({})),
            ),
            target_stage_summaries: summary
                .target_stage_summaries
                .into_iter()
                .map(GqlTargetStageSummary::from)
                .collect(),
            remaining_code_tasks: summary
                .remaining_code_tasks
                .into_iter()
                .map(GqlRemainingCodeTaskSummary::from)
                .collect(),
            handoff_tasks: summary
                .handoff_tasks
                .into_iter()
                .map(GqlHandoffTaskSummary::from)
                .collect(),
            known_risks: summary.known_risks,
            tests_run: summary.tests_run,
            docs_impacted: summary.docs_impacted,
            validation_errors: summary
                .validation_errors
                .into_iter()
                .map(GqlValidationIssue::from)
                .collect(),
            warnings: summary
                .warnings
                .into_iter()
                .map(GqlValidationIssue::from)
                .collect(),
            raw_artifact_available: summary.raw_artifact_available,
        }
    }
}

impl From<TargetStageSummary> for GqlTargetStageSummary {
    fn from(summary: TargetStageSummary) -> Self {
        GqlTargetStageSummary {
            target_stage: summary.target_stage,
            count: saturating_i32(summary.count),
            blocking_review_count: saturating_i32(summary.blocking_review_count),
        }
    }
}

fn saturating_i32(value: usize) -> i32 {
    value.min(i32::MAX as usize) as i32
}

fn optional_saturating_i32(value: Option<usize>) -> Option<i32> {
    value.map(saturating_i32)
}

fn public_artifact_path(path: &str) -> String {
    if path.ends_with("implementation/self-assessment.json") {
        "implementation/self-assessment.json".to_string()
    } else {
        path.to_string()
    }
}

impl From<RemainingCodeTaskSummary> for GqlRemainingCodeTaskSummary {
    fn from(task: RemainingCodeTaskSummary) -> Self {
        GqlRemainingCodeTaskSummary {
            summary: task.summary,
            owner: task.owner,
            blocking: task.blocking,
            evidence: task.evidence,
            source_pointer: task.source_pointer,
        }
    }
}

impl From<HandoffTaskSummary> for GqlHandoffTaskSummary {
    fn from(task: HandoffTaskSummary) -> Self {
        GqlHandoffTaskSummary {
            summary: task.summary,
            owner_class: task.owner_class.to_string(),
            target_stage: task.target_stage,
            blocking_review: task.blocking_review,
            evidence: task.evidence,
            source_pointer: task.source_pointer,
        }
    }
}

impl From<ValidationIssue> for GqlValidationIssue {
    fn from(issue: ValidationIssue) -> Self {
        GqlValidationIssue {
            code: issue.code,
            message: issue.message,
            pointer: issue.pointer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturating_i32_clamps_oversized_counts() {
        assert_eq!(saturating_i32(42), 42);
        assert_eq!(saturating_i32(i32::MAX as usize), i32::MAX);
        assert_eq!(saturating_i32((i32::MAX as usize) + 1), i32::MAX);
    }

    #[test]
    fn target_stage_summary_conversion_clamps_counts() {
        let converted = GqlTargetStageSummary::from(TargetStageSummary {
            target_stage: "state_9_implementation_reviewed".to_string(),
            count: (i32::MAX as usize) + 10,
            blocking_review_count: (i32::MAX as usize) + 20,
        });

        assert_eq!(converted.count, i32::MAX);
        assert_eq!(converted.blocking_review_count, i32::MAX);
    }
}
