use async_graphql::*;
use db::repos::projections::RunProjectionRow;
use domain::run::Run;

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
    /// Stage counts from the projection layer; None when reading a single run by ID.
    pub total_stages: Option<i64>,
    pub completed_stages: Option<i64>,
    pub failed_stages: Option<i64>,
    pub pending_approvals: Option<i64>,
    pub delivery_preflight_json: Option<String>,
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
            total_stages: None,
            completed_stages: None,
            failed_stages: None,
            pending_approvals: None,
        }
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
            total_stages: Some(r.total_stages),
            completed_stages: Some(r.completed_stages),
            failed_stages: Some(r.failed_stages),
            pending_approvals: Some(r.pending_approvals),
        }
    }
}
