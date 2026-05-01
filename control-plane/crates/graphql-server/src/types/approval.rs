use async_graphql::*;
use db::repos::projections::ApprovalInboxRow;
use domain::approval::Approval;

use crate::types::p031::{GqlDisabledReasonCode, GqlFreshnessState, GqlWritePathState};

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
    pub freshness_state: GqlFreshnessState,
    pub disabled_reason_code: Option<GqlDisabledReasonCode>,
    pub write_path_state: GqlWritePathState,
    pub diagnostic_id: Option<String>,
    pub server_debug_detail: Option<String>,
    /// P072: Server-authored list of actions available for this approval.
    /// Possible values: "approve", "reject". Empty when not actionable.
    pub available_actions: Vec<String>,
    /// P072: Reason why actions are disabled, if any.
    pub disabled_reason: Option<String>,
    /// P072: Monotonic version for read-your-writes settlement tracking.
    pub settlement_version: Option<i64>,
}

impl From<Approval> for GqlApproval {
    fn from(a: Approval) -> Self {
        use domain::approval::ApprovalDecision;

        let diagnostic_id = a.id.to_string();
        let is_actionable = matches!(
            a.decision,
            ApprovalDecision::Pending | ApprovalDecision::Requested
        );
        // P072: available_actions reflects server-side actionability.
        let available_actions = if is_actionable {
            vec!["approve".to_string(), "reject".to_string()]
        } else {
            vec![]
        };
        let (disabled_reason_code, write_path_state, disabled_reason) =
            approval_actionability_fields(is_actionable, &a.decision.to_string());
        GqlApproval {
            id: ID(diagnostic_id.clone()),
            run_id: ID(a.run_id.to_string()),
            stage_id: a.stage_id,
            decision: a.decision.to_string(),
            requested_at: a.requested_at.to_rfc3339(),
            decided_at: a.decided_at.map(|t| t.to_rfc3339()),
            comment: a.comment,
            expires_at: a.expires_at.map(|t| t.to_rfc3339()),
            freshness_state: GqlFreshnessState::Live,
            disabled_reason_code,
            write_path_state,
            diagnostic_id: Some(diagnostic_id),
            server_debug_detail: None,
            available_actions,
            disabled_reason,
            settlement_version: None,
        }
    }
}

impl From<ApprovalInboxRow> for GqlApproval {
    fn from(r: ApprovalInboxRow) -> Self {
        let diagnostic_id = r.id.clone();
        let is_actionable = r.decision == "pending" || r.decision == "requested";
        // P072: available_actions reflects server-side actionability.
        let available_actions = if is_actionable {
            vec!["approve".to_string(), "reject".to_string()]
        } else {
            vec![]
        };
        let (disabled_reason_code, write_path_state, disabled_reason) =
            approval_actionability_fields(is_actionable, &r.decision);
        GqlApproval {
            id: ID(r.id),
            run_id: ID(r.run_id),
            stage_id: r.stage_id,
            decision: r.decision,
            requested_at: r.requested_at,
            decided_at: r.decided_at,
            comment: r.comment,
            expires_at: r.expires_at,
            freshness_state: GqlFreshnessState::Live,
            disabled_reason_code,
            write_path_state,
            diagnostic_id: Some(diagnostic_id),
            server_debug_detail: None,
            available_actions,
            disabled_reason,
            settlement_version: None,
        }
    }
}

fn approval_actionability_fields(
    is_actionable: bool,
    decision: &str,
) -> (
    Option<GqlDisabledReasonCode>,
    GqlWritePathState,
    Option<String>,
) {
    if is_actionable {
        (None, GqlWritePathState::Available, None)
    } else {
        (
            Some(GqlDisabledReasonCode::UnsupportedAction),
            GqlWritePathState::WritePathNotAvailable,
            Some(format!("approval already {decision}")),
        )
    }
}
