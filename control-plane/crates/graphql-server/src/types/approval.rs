use async_graphql::*;
use db::repos::projections::ApprovalInboxRow;
use domain::approval::Approval;

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlApproval {
    pub id: ID,
    pub run_id: ID,
    pub stage_id: String,
    pub decision: String,
    pub requested_at: String,
    pub decided_at: Option<String>,
    pub comment: Option<String>,
    pub expires_at: Option<String>,
}

impl From<Approval> for GqlApproval {
    fn from(a: Approval) -> Self {
        GqlApproval {
            id: ID(a.id.to_string()),
            run_id: ID(a.run_id.to_string()),
            stage_id: a.stage_id,
            decision: a.decision.to_string(),
            requested_at: a.requested_at.to_rfc3339(),
            decided_at: a.decided_at.map(|t| t.to_rfc3339()),
            comment: a.comment,
            expires_at: a.expires_at.map(|t| t.to_rfc3339()),
        }
    }
}

impl From<ApprovalInboxRow> for GqlApproval {
    fn from(r: ApprovalInboxRow) -> Self {
        GqlApproval {
            id: ID(r.id),
            run_id: ID(r.run_id),
            stage_id: r.stage_id,
            decision: r.decision,
            requested_at: r.requested_at,
            decided_at: r.decided_at,
            comment: r.comment,
            expires_at: r.expires_at,
        }
    }
}
