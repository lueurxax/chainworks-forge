use async_graphql::*;
use domain::run::Run;
use db::repos::projections::RunProjectionRow;

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
    /// Stage counts from the projection layer; None when reading a single run by ID.
    pub total_stages: Option<i64>,
    pub completed_stages: Option<i64>,
    pub failed_stages: Option<i64>,
    pub pending_approvals: Option<i64>,
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
            total_stages: Some(r.total_stages),
            completed_stages: Some(r.completed_stages),
            failed_stages: Some(r.failed_stages),
            pending_approvals: Some(r.pending_approvals),
        }
    }
}
