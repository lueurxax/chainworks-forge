// P077: CloseoutReadinessSummaryAccessor — single typed accessor.
//
// R14 §architecture.single_accessor:
//   "CloseoutReadinessSummaryAccessor is the only typed accessor for transitions,
//    GraphQL runs.get/list, MCP runs.get/list, run-state/exported projections,
//    and macOS readback. No consumer parses review/implementation-closeout-readiness.json
//    for transition truth."
//
// The accessor reads active SQLite artifact-contract truth; it never parses
// exported JSON projections directly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::closeout_readiness::{CloseoutReadiness, CloseoutReadinessDecision, CloseoutReadinessStatus};
use crate::closeout_readiness_mode::CloseoutReadinessModeResult;
use crate::proposal_gate_result::{ProposalGateResult, ProposalGateStatus};
use crate::risk_lineage::RiskAcceptanceLineage;

/// Summary view returned by the accessor to all consumers.
/// This is the canonical shape for transitions, GraphQL, MCP, and macOS readback.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloseoutReadinessSummary {
    pub run_id: String,
    pub stage_id: String,
    pub readiness_status: CloseoutReadinessStatus,
    pub readiness_decision: CloseoutReadinessDecision,
    pub readiness_generation_id: String,
    pub readiness_mode: String,
    pub gate_status: ProposalGateStatus,
    pub gate_generation_id: String,
    pub diagnostic_reason: Option<String>,
    pub primary_unblock: Option<String>,
    pub code_blocker_count: u32,
    pub handoff_owner: Option<String>,
    pub risk_settlement_required: bool,
    pub accepted_risk_count: u32,
    pub fingerprint_hash: Option<String>,
    pub synthesized_at: DateTime<Utc>,
    /// True iff the run is P077-applicable (state_9 closeout with an active accessor generation).
    pub is_applicable: bool,
}

impl CloseoutReadinessSummary {
    /// The 8-character generation hash operator-facing identifier.
    pub fn generation_hash_display(&self) -> String {
        let hash = &self.readiness_generation_id;
        if hash.len() >= 8 {
            hash[..8].to_string()
        } else {
            hash.clone()
        }
    }
}

/// Inputs required to build a CloseoutReadinessSummary.
pub struct CloseoutReadinessAccessorInputs<'a> {
    pub readiness: &'a CloseoutReadiness,
    pub gate_result: &'a ProposalGateResult,
    pub mode_result: &'a CloseoutReadinessModeResult,
    pub accepted_risks: &'a [RiskAcceptanceLineage],
}

/// The single typed accessor for closeout readiness.
///
/// All consumers (transition evaluation, GraphQL, MCP, run-state, macOS)
/// must go through this accessor. No consumer may parse
/// review/implementation-closeout-readiness.json directly for transition truth.
pub struct CloseoutReadinessSummaryAccessor;

impl CloseoutReadinessSummaryAccessor {
    /// Build the canonical summary from active SQLite truth.
    /// This is the only path for transition evaluation.
    pub fn build_summary(inputs: CloseoutReadinessAccessorInputs<'_>) -> CloseoutReadinessSummary {
        let CloseoutReadinessAccessorInputs {
            readiness,
            gate_result,
            mode_result,
            accepted_risks,
        } = inputs;

        let fingerprint_hash = readiness
            .fingerprint
            .as_ref()
            .map(|fp| fp.short_hash());

        CloseoutReadinessSummary {
            run_id: readiness.run_id.clone(),
            stage_id: readiness.stage_id.clone(),
            readiness_status: readiness.status.clone(),
            readiness_decision: readiness.decision.clone(),
            readiness_generation_id: readiness.generation_id.clone(),
            readiness_mode: mode_result.effective_mode().as_str().to_string(),
            gate_status: gate_result.status.clone(),
            gate_generation_id: gate_result.generation_id.clone(),
            diagnostic_reason: readiness.diagnostic_reason.clone(),
            primary_unblock: readiness.primary_unblock.clone(),
            code_blocker_count: readiness.code_blocker_count,
            handoff_owner: readiness.handoff_owner.clone(),
            risk_settlement_required: readiness.risk_settlement_required,
            accepted_risk_count: accepted_risks.len() as u32,
            fingerprint_hash,
            synthesized_at: readiness.synthesized_at,
            is_applicable: true,
        }
    }

    /// Build a not-applicable summary for runs where P077 closeout readiness
    /// does not apply (e.g. non-proposal-backed, pre-state_9 runs).
    pub fn not_applicable(run_id: impl Into<String>, stage_id: impl Into<String>) -> CloseoutReadinessSummary {
        CloseoutReadinessSummary {
            run_id: run_id.into(),
            stage_id: stage_id.into(),
            readiness_status: CloseoutReadinessStatus::Unknown,
            readiness_decision: CloseoutReadinessDecision::AwaitOperatorDecision,
            readiness_generation_id: String::new(),
            readiness_mode: "advisory".into(),
            gate_status: ProposalGateStatus::MissingDefinition,
            gate_generation_id: String::new(),
            diagnostic_reason: None,
            primary_unblock: None,
            code_blocker_count: 0,
            handoff_owner: None,
            risk_settlement_required: false,
            accepted_risk_count: 0,
            fingerprint_hash: None,
            synthesized_at: Utc::now(),
            is_applicable: false,
        }
    }

    /// Build a summary for a run awaiting its first readiness generation
    /// (synthesizer has not run yet).
    pub fn awaiting_first_generation(
        run_id: impl Into<String>,
        stage_id: impl Into<String>,
    ) -> CloseoutReadinessSummary {
        CloseoutReadinessSummary {
            run_id: run_id.into(),
            stage_id: stage_id.into(),
            readiness_status: CloseoutReadinessStatus::Unknown,
            readiness_decision: CloseoutReadinessDecision::AwaitOperatorDecision,
            readiness_generation_id: String::new(),
            readiness_mode: "advisory".into(),
            gate_status: ProposalGateStatus::MissingDefinition,
            gate_generation_id: String::new(),
            diagnostic_reason: Some("awaiting_first_generation".into()),
            primary_unblock: Some("Awaiting first readiness check".into()),
            code_blocker_count: 0,
            handoff_owner: None,
            risk_settlement_required: false,
            accepted_risk_count: 0,
            fingerprint_hash: None,
            synthesized_at: Utc::now(),
            is_applicable: true,
        }
    }
}

/// Decision routing for transition evaluation.
/// Reads only from CloseoutReadinessSummary (active SQLite truth via accessor),
/// never from stale exported JSON.
pub fn route_transition_decision(summary: &CloseoutReadinessSummary) -> CloseoutReadinessDecision {
    summary.readiness_decision.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::closeout_readiness::{CloseoutReadiness, CloseoutReadinessDecision, CloseoutReadinessStatus};
    use crate::closeout_readiness_mode::{CloseoutReadinessMode, CloseoutReadinessModeResult};
    use crate::proposal_gate_result::ProposalGateResult;

    fn make_readiness(status: CloseoutReadinessStatus, decision: CloseoutReadinessDecision) -> CloseoutReadiness {
        CloseoutReadiness {
            run_id: "run-1".into(),
            stage_id: "state_9".into(),
            status,
            decision,
            generation_id: "gen-abc123def".into(),
            readiness_mode: "advisory".into(),
            diagnostic_reason: None,
            primary_unblock: None,
            code_blocker_count: 0,
            handoff_owner: None,
            risk_settlement_required: false,
            fingerprint: None,
            synthesized_at: Utc::now(),
        }
    }

    fn make_gate(status: ProposalGateStatus) -> ProposalGateResult {
        ProposalGateResult {
            gate_id: "p077:077".into(),
            proposal_id: "077".into(),
            run_id: "run-1".into(),
            stage_id: "state_9".into(),
            status,
            generation_id: "gate-gen-1".into(),
            diagnostic_reason: None,
            executor_version: None,
            evidence_digest: None,
            exit_code: None,
            elapsed_ms: None,
            settled_at: Utc::now(),
            authorization_lineage: None,
            failure_classification: None,
        }
    }

    #[test]
    fn build_summary_exposes_same_fields_for_graphql_mcp_and_transition() {
        let readiness = make_readiness(
            CloseoutReadinessStatus::Ready,
            CloseoutReadinessDecision::EnterManualRelease,
        );
        let gate = make_gate(ProposalGateStatus::Passed);
        let mode = CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Enforcement);

        let summary = CloseoutReadinessSummaryAccessor::build_summary(CloseoutReadinessAccessorInputs {
            readiness: &readiness,
            gate_result: &gate,
            mode_result: &mode,
            accepted_risks: &[],
        });

        assert_eq!(summary.readiness_status, CloseoutReadinessStatus::Ready);
        assert_eq!(summary.gate_status, ProposalGateStatus::Passed);
        assert_eq!(summary.readiness_mode, "enforcement");
        assert!(summary.is_applicable);
        assert_eq!(summary.generation_hash_display().len(), 8);
    }

    #[test]
    fn not_applicable_summary_is_not_applicable() {
        let summary = CloseoutReadinessSummaryAccessor::not_applicable("run-1", "state_5");
        assert!(!summary.is_applicable);
    }

    #[test]
    fn awaiting_first_generation_has_awaiting_diagnostic_reason() {
        let summary = CloseoutReadinessSummaryAccessor::awaiting_first_generation("run-1", "state_9");
        assert!(summary.is_applicable);
        assert_eq!(
            summary.diagnostic_reason.as_deref(),
            Some("awaiting_first_generation")
        );
    }

    #[test]
    fn route_transition_decision_reads_from_summary_not_stale_json() {
        let readiness = make_readiness(
            CloseoutReadinessStatus::NotReady,
            CloseoutReadinessDecision::ReturnToCodeRefine,
        );
        let gate = make_gate(ProposalGateStatus::Failed);
        let mode = CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Advisory);

        let summary = CloseoutReadinessSummaryAccessor::build_summary(CloseoutReadinessAccessorInputs {
            readiness: &readiness,
            gate_result: &gate,
            mode_result: &mode,
            accepted_risks: &[],
        });

        let decision = route_transition_decision(&summary);
        assert_eq!(decision, CloseoutReadinessDecision::ReturnToCodeRefine);
    }

    #[test]
    fn generation_hash_display_is_8_chars_for_long_generation_id() {
        let readiness = make_readiness(CloseoutReadinessStatus::Ready, CloseoutReadinessDecision::EnterManualRelease);
        let gate = make_gate(ProposalGateStatus::Passed);
        let mode = CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Advisory);

        let summary = CloseoutReadinessSummaryAccessor::build_summary(CloseoutReadinessAccessorInputs {
            readiness: &readiness,
            gate_result: &gate,
            mode_result: &mode,
            accepted_risks: &[],
        });
        assert_eq!(summary.generation_hash_display().len(), 8);
    }
}
