use async_graphql::*;
use db::repos::projections::RunProjectionRow;
use domain::artifact_contracts::{
    HandoffTaskSummary, ImplementationSelfAssessmentSummary, RemainingCodeTaskSummary,
    TargetStageSummary, ValidationIssue,
};
use domain::run::Run;

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
    pub implementation_self_assessment_summary: Option<GqlImplementationSelfAssessmentSummary>,
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
            implementation_self_assessment_summary: None,
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
            implementation_self_assessment_summary: None,
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
