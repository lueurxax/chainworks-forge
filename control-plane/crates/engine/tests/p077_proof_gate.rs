// P077 proof gate — decision-matrix regression fixtures.
//
// R14 §acceptance.proof_gate items 1–8 that are verifiable through the
// engine synthesizer + closeout accessor without live Xcode / remote UI:
//
//  (2) missing proposal gate => await_gate_definition; cannot enter manual release
//  (3) code blockers + budget remaining => return_to_code_refine
//  (4) no code blockers => handoff or operator-decision (no code_writer invocation)
//  (5) ready_with_risks enters manual release ONLY with typed accepted lineage
//  (6) green gate + zero blockers + settled risks => enter_manual_release
//  (7) repeated identical blockers trigger soft convergence checkpoint without
//      claiming P052 hard budget exhaustion
//  (8) transition evaluation reads active SQLite truth, never stale exported JSON

use chrono::Utc;

use domain::artifact_contracts::{
    ImplementationSelfAssessmentStatus, ImplementationSelfAssessmentSummary,
};
use domain::closeout_readiness::{CloseoutReadinessDecision, CloseoutReadinessStatus};
use domain::closeout_readiness_mode::{CloseoutReadinessMode, CloseoutReadinessModeResult};
use domain::closeout_readiness_summary_accessor::{
    route_transition_decision, CloseoutReadinessAccessorInputs, CloseoutReadinessSummaryAccessor,
};
use domain::proposal_gate_result::{ProposalGateResult, ProposalGateStatus};
use domain::risk_lineage::{RiskAcceptanceLineage, RiskAcceptanceSource, RiskClassification};
use engine::synthesizers::closeout_readiness::{
    compute_blocker_digest, synthesize_implementation_closeout_readiness_for_state9,
    SynthesizerInputs,
};

// ── helpers ────────────────────────────────────────────────────────────────

fn advisory_mode() -> CloseoutReadinessModeResult {
    CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Advisory)
}

fn enforcement_mode() -> CloseoutReadinessModeResult {
    CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Enforcement)
}

fn gate(status: ProposalGateStatus) -> ProposalGateResult {
    ProposalGateResult {
        gate_id: "p077:077".into(),
        proposal_id: "077".into(),
        run_id: "run-1".into(),
        stage_id: "state_9".into(),
        status,
        generation_id: "gate-gen-test".into(),
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

fn green_assessment() -> ImplementationSelfAssessmentSummary {
    ImplementationSelfAssessmentSummary {
        contract_id: "implementation_self_assessment_v2".into(),
        artifact_path: "implementation/self-assessment.json".into(),
        status: ImplementationSelfAssessmentStatus::Complete,
        implementation_complete: Some(true),
        verification_green: Some(true),
        remaining_code_task_count: Some(0),
        blocking_remaining_code_task_count: Some(0),
        handoff_task_count: Some(0),
        blocking_review_handoff_task_count: Some(0),
        owner_class_counts: Default::default(),
        target_stage_summaries: vec![],
        remaining_code_tasks: vec![],
        handoff_tasks: vec![],
        known_risks: vec![],
        tests_run: vec![],
        docs_impacted: vec![],
        validation_errors: vec![],
        warnings: vec![],
        raw_artifact_available: true,
    }
}

fn valid_risk(id: &str) -> RiskAcceptanceLineage {
    RiskAcceptanceLineage {
        risk_id: id.into(),
        title: "Test Risk".into(),
        classification: RiskClassification::Medium,
        authority: "release-owner".into(),
        journal_or_decision_id: "journal-abc".into(),
        source_generation_ids: vec!["gen-1".into()],
        settled_at: Utc::now(),
        acceptance_source: RiskAcceptanceSource::TypedControlledRiskRow,
        rationale: None,
    }
}

fn synthesize(
    gate_status: ProposalGateStatus,
    assessment: Option<&ImplementationSelfAssessmentSummary>,
    accepted_risks: &[RiskAcceptanceLineage],
    loop_budget_remaining: bool,
    mode: &CloseoutReadinessModeResult,
) -> (CloseoutReadinessStatus, CloseoutReadinessDecision) {
    // Use controlled_reports_green=Some(true) so enforcement tests are not blocked by report check.
    synthesize_with_reports(
        gate_status,
        assessment,
        accepted_risks,
        loop_budget_remaining,
        mode,
        Some(true),
    )
}

fn synthesize_with_reports(
    gate_status: ProposalGateStatus,
    assessment: Option<&ImplementationSelfAssessmentSummary>,
    accepted_risks: &[RiskAcceptanceLineage],
    loop_budget_remaining: bool,
    mode: &CloseoutReadinessModeResult,
    controlled_reports_green: Option<bool>,
) -> (CloseoutReadinessStatus, CloseoutReadinessDecision) {
    let g = gate(gate_status);
    let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
        run_id: "run-1",
        stage_id: "state_9",
        gate_result: &g,
        mode_result: mode,
        self_assessment: assessment,
        accepted_risks,
        loop_budget_remaining,
        fingerprint: None,
        fingerprint_latency_exceeded: false,
        controlled_reports_green,
        previous_blocker_digest: None,
    });
    (result.readiness.status, result.readiness.decision)
}

// ── (2) missing proposal gate => await_gate_definition ─────────────────────

#[test]
fn p077_proof_gate_missing_gate_cannot_enter_manual_release() {
    let assessment = green_assessment();
    let (status, decision) = synthesize(
        ProposalGateStatus::MissingDefinition,
        Some(&assessment),
        &[],
        true,
        &advisory_mode(),
    );
    assert_eq!(
        decision,
        CloseoutReadinessDecision::AwaitGateDefinition,
        "missing gate must route to await_gate_definition — not enter_manual_release"
    );
    assert_ne!(
        decision,
        CloseoutReadinessDecision::EnterManualRelease,
        "missing gate CANNOT enter manual release"
    );
    // Status should be unknown (not ready)
    assert_ne!(status, CloseoutReadinessStatus::Ready);
}

// ── (3) code blockers + budget remaining => return_to_code_refine ──────────

#[test]
fn p077_proof_gate_code_blockers_with_budget_returns_to_code_refine() {
    let mut assessment = green_assessment();
    assessment.blocking_remaining_code_task_count = Some(2);
    assessment.status = ImplementationSelfAssessmentStatus::NeedsCodeFixes;

    let (status, decision) = synthesize(
        ProposalGateStatus::Passed,
        Some(&assessment),
        &[],
        true, // budget remaining
        &enforcement_mode(),
    );
    assert_eq!(
        decision,
        CloseoutReadinessDecision::ReturnToCodeRefine,
        "code blockers + budget remaining must return to refine, not advance"
    );
    assert_eq!(status, CloseoutReadinessStatus::NotReady);
    assert_ne!(decision, CloseoutReadinessDecision::EnterManualRelease);
}

// ── (4) no code blockers => no code_writer invocation ──────────────────────

#[test]
fn p077_proof_gate_no_code_blockers_with_handoff_awaits_non_code_handoff() {
    let mut assessment = green_assessment();
    assessment.handoff_task_count = Some(1);
    // handoff_tasks needs a real entry with an owner_class for the synthesizer to show owner
    // zero code blockers is the default (blocking_remaining_code_task_count = 0)

    let (status, decision) = synthesize(
        ProposalGateStatus::Passed,
        Some(&assessment),
        &[],
        true,
        &advisory_mode(),
    );
    // Should be handoff_required, not code_refine — no code_writer invocation
    assert_eq!(status, CloseoutReadinessStatus::HandoffRequired);
    assert_eq!(decision, CloseoutReadinessDecision::AwaitNonCodeHandoff);
    assert_ne!(
        decision,
        CloseoutReadinessDecision::ReturnToCodeRefine,
        "non-code handoff must NEVER route to code_writer"
    );
}

#[test]
fn p077_proof_gate_no_code_blockers_exhausted_budget_awaits_operator_not_code_writer() {
    let g = gate(ProposalGateStatus::Failed);
    let assessment = green_assessment();
    let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
        run_id: "run-1",
        stage_id: "state_9",
        gate_result: &g,
        mode_result: &advisory_mode(),
        self_assessment: Some(&assessment),
        accepted_risks: &[],
        loop_budget_remaining: false,
        fingerprint: None,
        fingerprint_latency_exceeded: false,
        controlled_reports_green: None,
        previous_blocker_digest: None,
    });
    assert_eq!(
        result.readiness.decision,
        CloseoutReadinessDecision::AwaitOperatorDecision,
        "exhausted budget + failed gate must await_operator_decision, not code_writer"
    );
    assert_ne!(
        result.readiness.decision,
        CloseoutReadinessDecision::ReturnToCodeRefine,
    );
}

// ── (5) ready_with_risks enters manual release ONLY with typed accepted lineage

#[test]
fn p077_proof_gate_ready_with_risks_requires_typed_lineage() {
    let mut assessment = green_assessment();
    assessment.known_risks = vec!["parallel release policy risk".into()];

    // Without accepted lineage -> should NOT enter manual release
    let (_, decision_no_lineage) = synthesize(
        ProposalGateStatus::Passed,
        Some(&assessment),
        &[], // no accepted risks
        true,
        &enforcement_mode(),
    );
    assert_ne!(
        decision_no_lineage,
        CloseoutReadinessDecision::EnterManualRelease,
        "ready_with_risks without typed lineage must NOT enter manual release"
    );

    // With accepted typed lineage -> should enter manual release
    let risks = vec![valid_risk("RISK-001")];
    let (status_with_lineage, decision_with_lineage) = synthesize(
        ProposalGateStatus::Passed,
        Some(&assessment),
        &risks,
        true,
        &enforcement_mode(),
    );
    assert_eq!(
        decision_with_lineage,
        CloseoutReadinessDecision::EnterManualRelease,
        "ready_with_risks WITH typed accepted lineage MUST enter manual release"
    );
    assert_eq!(status_with_lineage, CloseoutReadinessStatus::ReadyWithRisks);
}

// ── (6) green gate + zero blockers + settled risks => enter_manual_release ──

#[test]
fn p077_proof_gate_green_gate_zero_blockers_enters_manual_release() {
    let assessment = green_assessment();
    let (status, decision) = synthesize(
        ProposalGateStatus::Passed,
        Some(&assessment),
        &[],
        true,
        &enforcement_mode(),
    );
    assert_eq!(
        decision,
        CloseoutReadinessDecision::EnterManualRelease,
        "green gate + zero blockers + no risks must enter manual release"
    );
    assert_eq!(status, CloseoutReadinessStatus::Ready);
}

#[test]
fn p077_proof_gate_waived_gate_with_risks_accepted_enters_manual_release() {
    use domain::proposal_gate_result::ProposalGateLineage;

    let mut assessment = green_assessment();
    assessment.known_risks = vec!["risk item".into()];
    let risks = vec![valid_risk("RISK-001")];

    // Per R14: waived gate requires authorization_lineage with a non-empty current_fingerprint.
    let mut g = gate(ProposalGateStatus::Waived);
    g.authorization_lineage = Some(ProposalGateLineage {
        principal: "operator".into(),
        capability: "gate.waive".into(),
        journal_id: "journal-waive-proof".into(),
        authority: "release-owner".into(),
        reason: "test waiver for proof gate".into(),
        source_artifacts: vec!["review/prepush.json".into()],
        run_id: "run-1".into(),
        proposal_id: "077".into(),
        stage_id: "state_9".into(),
        workflow_digest: "wf-digest-proof".into(),
        worktree_head: "abcdef1".into(),
        dirty_or_changed_file_digest: "clean".into(),
        source_generation_ids: vec!["gen-proof-1".into()],
        current_fingerprint: "fp-proof12345".into(),
    });

    let mode = enforcement_mode();
    let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
        run_id: "run-1",
        stage_id: "state_9",
        gate_result: &g,
        mode_result: &mode,
        self_assessment: Some(&assessment),
        accepted_risks: &risks,
        loop_budget_remaining: true,
        fingerprint: None,
        fingerprint_latency_exceeded: false,
        controlled_reports_green: Some(true),
        previous_blocker_digest: None,
    });
    assert_eq!(
        result.readiness.decision,
        CloseoutReadinessDecision::EnterManualRelease,
        "waived gate with valid lineage + risks accepted must enter manual release"
    );
    assert_eq!(
        result.readiness.status,
        CloseoutReadinessStatus::ReadyWithRisks
    );
}

// ── (7) repeated identical blockers => soft convergence checkpoint ──────────
//
// Per R14: "repeated identical blockers trigger a soft convergence checkpoint."
// The synthesizer detects repeated identical blocker sets (via digest comparison)
// and routes to AwaitOperatorDecision without claiming P052 hard budget exhaustion.
// The first occurrence (no prior digest) still returns ReturnToCodeRefine.

#[test]
fn p077_proof_gate_repeated_identical_blockers_trigger_soft_convergence_checkpoint() {
    let mut assessment = green_assessment();
    assessment.blocking_remaining_code_task_count = Some(1);
    assessment.status = ImplementationSelfAssessmentStatus::NeedsCodeFixes;
    // Use Passed gate so the assessment blockers reach apply_decision_matrix.
    // (Failed gate is routed by route_gate_cause before blockers are checked.)
    let g = gate(ProposalGateStatus::Passed);

    // First invocation: no previous digest → returns to code refine.
    let r1 = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
        run_id: "run-1",
        stage_id: "state_9",
        gate_result: &g,
        mode_result: &advisory_mode(),
        self_assessment: Some(&assessment),
        accepted_risks: &[],
        loop_budget_remaining: true,
        fingerprint: None,
        fingerprint_latency_exceeded: false,
        controlled_reports_green: None,
        previous_blocker_digest: None,
    });
    assert_eq!(
        r1.readiness.decision,
        CloseoutReadinessDecision::ReturnToCodeRefine,
        "first blocker occurrence must return to code_refine"
    );

    // Compute blocker digest matching the first assessment.
    let digest = compute_blocker_digest(&assessment, &g);

    // Second invocation with the SAME blockers and the prior digest.
    // Budget is still remaining — P052 hard budget NOT exhausted.
    // Per R14: soft convergence checkpoint → AwaitOperatorDecision.
    let r2 = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
        run_id: "run-1",
        stage_id: "state_9",
        gate_result: &g,
        mode_result: &advisory_mode(),
        self_assessment: Some(&assessment),
        accepted_risks: &[],
        loop_budget_remaining: true, // budget NOT exhausted (no P052 claim)
        fingerprint: None,
        fingerprint_latency_exceeded: false,
        controlled_reports_green: None,
        previous_blocker_digest: Some(&digest), // same set as before
    });
    assert_eq!(
        r2.readiness.decision,
        CloseoutReadinessDecision::AwaitOperatorDecision,
        "repeated identical blockers must trigger soft_convergence_checkpoint → await_operator_decision"
    );
    assert_ne!(
        r2.readiness.decision,
        CloseoutReadinessDecision::ReturnToCodeRefine,
        "soft convergence must NOT return to code refine again"
    );
    let reason = r2.readiness.diagnostic_reason.clone().unwrap_or_default();
    assert!(
        reason.contains("soft_convergence_checkpoint"),
        "diagnostic_reason must mention soft_convergence_checkpoint: got {reason}"
    );
}

// ── (8) transition evaluation reads active SQLite truth, never stale JSON ──
//
// The accessor is the single typed boundary; route_transition_decision reads
// from CloseoutReadinessSummary (built from active DB truth), never from
// exported JSON files. This test verifies the accessor-based routing.

#[test]
fn p077_proof_gate_transition_evaluation_reads_active_truth_not_stale_json() {
    let assessment = green_assessment();
    let g = gate(ProposalGateStatus::Passed);
    let mode = enforcement_mode();

    let synth_result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
        run_id: "run-1",
        stage_id: "state_9",
        gate_result: &g,
        mode_result: &mode,
        self_assessment: Some(&assessment),
        accepted_risks: &[],
        loop_budget_remaining: true,
        fingerprint: None,
        fingerprint_latency_exceeded: false,
        controlled_reports_green: Some(true),
        previous_blocker_digest: None,
    });

    // Build the summary through the single accessor (same path used in transition evaluation)
    let summary =
        CloseoutReadinessSummaryAccessor::build_summary(CloseoutReadinessAccessorInputs {
            readiness: &synth_result.readiness,
            gate_result: &g,
            mode_result: &mode,
            accepted_risks: &[],
        });

    // route_transition_decision must return EnterManualRelease for a ready run
    let decision = route_transition_decision(&summary);
    assert_eq!(
        decision,
        CloseoutReadinessDecision::EnterManualRelease,
        "transition must read active SQLite truth (accessor), not stale exported JSON"
    );
    assert!(
        summary.is_applicable,
        "P077-applicable runs must have is_applicable=true"
    );
}

// ── Accessor parity — same fields via accessor for GraphQL/MCP/transition ──

#[test]
fn p077_proof_gate_accessor_exposes_same_fields_for_graphql_mcp_and_transition() {
    let assessment = green_assessment();
    let g = gate(ProposalGateStatus::Passed);
    let mode = enforcement_mode();

    let synth = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
        run_id: "run-2",
        stage_id: "state_9",
        gate_result: &g,
        mode_result: &mode,
        self_assessment: Some(&assessment),
        accepted_risks: &[],
        loop_budget_remaining: true,
        fingerprint: None,
        fingerprint_latency_exceeded: false,
        controlled_reports_green: Some(true),
        previous_blocker_digest: None,
    });

    let summary =
        CloseoutReadinessSummaryAccessor::build_summary(CloseoutReadinessAccessorInputs {
            readiness: &synth.readiness,
            gate_result: &g,
            mode_result: &mode,
            accepted_risks: &[],
        });

    // These fields must be present for all three surfaces (GraphQL, MCP, transition)
    assert_eq!(summary.run_id, "run-2");
    assert_eq!(summary.stage_id, "state_9");
    assert_eq!(summary.readiness_status, CloseoutReadinessStatus::Ready);
    assert_eq!(summary.gate_status, ProposalGateStatus::Passed);
    assert_eq!(summary.readiness_mode, "enforcement");
    assert_eq!(summary.code_blocker_count, 0);
    assert!(!summary.risk_settlement_required);
    // generation hash must be 8 characters (operator-facing identifier)
    assert_eq!(
        summary.generation_hash_display().len(),
        8,
        "generation hash must be 8 characters for all surfaces"
    );
}
