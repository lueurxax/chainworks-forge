// P077: synthesize_implementation_closeout_readiness_for_state9
//
// R14 §architecture.state_9_sequence:
//   1. Import controlled review inputs.
//   2. Execute, import, or waive the proposal gate through the governed command path.
//   3. Call synthesize_implementation_closeout_readiness_for_state9 as a mandatory
//      engine function before transition evaluation.
//   4. Write proposal_gate_result_v1 and implementation_closeout_readiness_v1 active
//      generations in one closeout transaction.
//   5. Rebuild derived projections from the committed active pair.
//   6. Evaluate state transitions ONLY after the closeout transaction commits.
//
// Decision matrix from R14 §architecture.decision_matrix:
//   ready → enter_manual_release (implemented audit, current passed/waived gate,
//           green controlled reports, zero code blockers, no unaccepted risks)
//   ready_with_risks → enter_manual_release ONLY with typed accepted lineage
//   code blockers + budget remaining → return_to_code_refine
//   code blockers + budget exhausted → await_operator_decision
//   handoff/non-code without code blockers → handoff_required or enter_manual_release
//   missing proposal gate → await_gate_definition
//   malformed/stale/unauthorized/unavailable → fail closed with diagnostic_reason
//
// Gate cause routing from R14 §architecture.gate_cause_routing:
//   failed_code_owned_budget_remaining → return_to_code_refine
//   failed_unclear_or_budget_exhausted → await_operator_decision
//   missing_definition → await_gate_definition
//   stale/superseded/mismatched → rerun or import current receipt
//   unauthorized → await_operator_decision or reject unmanaged receipt
//   waived_current_fingerprint → continue matrix with waiver lineage

use chrono::Utc;
use uuid::Uuid;

use domain::closeout_readiness::{
    CloseoutFingerprint, CloseoutReadiness, CloseoutReadinessDecision, CloseoutReadinessStatus,
};
use domain::closeout_readiness_mode::CloseoutReadinessModeResult;
use domain::artifact_contracts::ImplementationSelfAssessmentSummary;
use domain::proposal_gate_result::{
    ProposalGateFailureClassification, ProposalGateResult, ProposalGateStatus,
};
use domain::risk_lineage::{risks_satisfy_enter_manual_release, RiskAcceptanceLineage};

/// Accepted latency budget for fingerprint computation per R14.
/// Exceeding this writes closeout_fingerprint_unavailable and fails closed.
const FINGERPRINT_LATENCY_BUDGET_MS: u64 = 5_000;

/// Inputs required by the synthesizer.
pub struct SynthesizerInputs<'a> {
    pub run_id: &'a str,
    pub stage_id: &'a str,
    pub gate_result: &'a ProposalGateResult,
    pub mode_result: &'a CloseoutReadinessModeResult,
    pub self_assessment: Option<&'a ImplementationSelfAssessmentSummary>,
    pub accepted_risks: &'a [RiskAcceptanceLineage],
    /// Whether the P052 loop budget has been exhausted.
    pub loop_budget_remaining: bool,
    /// Pre-computed fingerprint (None if latency budget was exceeded).
    pub fingerprint: Option<CloseoutFingerprint>,
    /// Whether the fingerprint latency budget was exceeded.
    pub fingerprint_latency_exceeded: bool,
    /// Whether all controlled reports (audit, docs, security, prepush, tests) are green.
    /// None means not yet wired (Phase-0 advisory compatibility).
    /// In enforcement mode, None or false fails closed before any Ready path.
    pub controlled_reports_green: Option<bool>,
    /// Digest of the previous code-blocker set for soft convergence detection.
    /// If the computed current blocker digest matches this value, the synthesizer
    /// routes to AwaitOperatorDecision (soft convergence checkpoint) instead of
    /// ReturnToCodeRefine, without claiming P052 hard budget exhaustion.
    pub previous_blocker_digest: Option<&'a str>,
}

/// Result from the synthesizer.
pub struct SynthesizerResult {
    pub readiness: CloseoutReadiness,
    /// Current blocker digest computed from the assessment + gate at synthesis
    /// time. Always present when a self-assessment exists; callers should
    /// persist this on the readiness generation row so the next synthesis can
    /// pass it as `previous_blocker_digest` and detect soft convergence.
    pub current_blocker_digest: Option<String>,
}

/// synthesize_implementation_closeout_readiness_for_state9 — mandatory engine function.
///
/// Must be called before transition evaluation. Returns a CloseoutReadiness record
/// that MUST be committed through the closeout transaction before any transition
/// is evaluated.
///
/// Per R14 §architecture.state_9_sequence, this function:
///   - Applies the decision matrix
///   - Applies gate_cause_routing
///   - Verifies fingerprint composition
///   - Returns fail-closed on fingerprint latency breach
pub fn synthesize_implementation_closeout_readiness_for_state9(
    inputs: SynthesizerInputs<'_>,
) -> SynthesizerResult {
    let generation_id = format!("cr-{}", Uuid::new_v4());

    // Compute the current blocker digest once when an assessment is available.
    // Persisted on the readiness generation so the next synthesis can compare
    // against it for soft-convergence detection (BLK-011).
    let current_blocker_digest = inputs
        .self_assessment
        .map(|assessment| compute_blocker_digest(assessment, inputs.gate_result));

    // Fingerprint latency breach → fails closed.
    if inputs.fingerprint_latency_exceeded {
        return SynthesizerResult {
            readiness: CloseoutReadiness {
                run_id: inputs.run_id.to_string(),
                stage_id: inputs.stage_id.to_string(),
                status: CloseoutReadinessStatus::Unknown,
                decision: CloseoutReadinessDecision::AwaitOperatorDecision,
                generation_id,
                readiness_mode: inputs.mode_result.effective_mode().as_str().to_string(),
                diagnostic_reason: Some(
                    "closeout_fingerprint_unavailable: latency budget exceeded".into(),
                ),
                primary_unblock: Some("Fingerprint unavailable — rerun or waive".into()),
                code_blocker_count: 0,
                handoff_owner: None,
                risk_settlement_required: false,
                fingerprint: None,
                synthesized_at: Utc::now(),
            },
            current_blocker_digest,
        };
    }

    // Diagnostic/unknown/malformed mode → fails closed per R14.
    // These states cannot enter manual release without an operator decision grounded
    // in enforcement truth.
    if inputs.mode_result.is_diagnostic() {
        let mode_err = match inputs.mode_result {
            CloseoutReadinessModeResult::Diagnostic(e) => e.to_string(),
            _ => "unknown diagnostic reason".into(),
        };
        return SynthesizerResult {
            readiness: CloseoutReadiness {
                run_id: inputs.run_id.to_string(),
                stage_id: inputs.stage_id.to_string(),
                status: CloseoutReadinessStatus::Blocked,
                decision: CloseoutReadinessDecision::AwaitOperatorDecision,
                generation_id,
                readiness_mode: "advisory".into(),
                diagnostic_reason: Some(format!(
                    "diagnostic_mode: {} — cannot enter manual release without operator decision grounded in enforcement truth",
                    mode_err
                )),
                primary_unblock: Some("Resolve closeout_readiness_mode configuration".into()),
                code_blocker_count: 0,
                handoff_owner: None,
                risk_settlement_required: false,
                fingerprint: inputs.fingerprint,
                synthesized_at: Utc::now(),
            },
            current_blocker_digest,
        };
    }

    // Gate cause routing per R14 §architecture.gate_cause_routing.
    let gate_decision = route_gate_cause(inputs.gate_result, inputs.loop_budget_remaining);
    if let Some(gate_routed) = gate_decision {
        return SynthesizerResult {
            readiness: gate_routed_readiness(inputs, generation_id, gate_routed),
            current_blocker_digest,
        };
    }

    // Gate allows entry (passed or waived with valid fingerprint).
    // Apply the full decision matrix.
    let readiness = apply_decision_matrix(inputs, generation_id);
    SynthesizerResult {
        readiness,
        current_blocker_digest,
    }
}

/// Gate cause routing — returns Some if the gate status itself determines the decision.
/// Returns None if the gate status allows proceeding to the full decision matrix.
fn route_gate_cause(
    gate: &ProposalGateResult,
    budget_remaining: bool,
) -> Option<GateRouted> {
    match &gate.status {
        ProposalGateStatus::Passed => None,
        ProposalGateStatus::Waived => {
            // Per R14: only waived_current_fingerprint continues the matrix.
            // Waived without authorization_lineage or an empty/missing current_fingerprint
            // is treated as unauthorized — fails closed.
            match &gate.authorization_lineage {
                None => Some(GateRouted::UnauthorizedWaiver),
                Some(lineage) if lineage.current_fingerprint.trim().is_empty() => {
                    Some(GateRouted::StaleWaiver)
                }
                _ => None, // valid waived_current_fingerprint — continue matrix
            }
        }

        ProposalGateStatus::MissingDefinition => Some(GateRouted::AwaitGateDefinition),
        ProposalGateStatus::Stale | ProposalGateStatus::Superseded => {
            Some(GateRouted::RerunOrImportCurrentReceipt)
        }
        ProposalGateStatus::Unauthorized => Some(GateRouted::AwaitOperatorDecision),
        ProposalGateStatus::Failed => {
            // Per R14 §architecture.gate_cause_routing: use failure_classification
            // when present; fall back to budget_remaining heuristic.
            match &gate.failure_classification {
                Some(ProposalGateFailureClassification::CodeOwnedBudgetRemaining) => {
                    Some(GateRouted::ReturnToCodeRefine)
                }
                Some(ProposalGateFailureClassification::BudgetExhausted)
                | Some(ProposalGateFailureClassification::UnclearOrNonCodeOwned) => {
                    Some(GateRouted::AwaitOperatorDecision)
                }
                None => {
                    if budget_remaining {
                        Some(GateRouted::ReturnToCodeRefine)
                    } else {
                        Some(GateRouted::AwaitOperatorDecision)
                    }
                }
            }
        }
        ProposalGateStatus::Invalid => Some(GateRouted::AwaitOperatorDecision),
    }
}

enum GateRouted {
    AwaitGateDefinition,
    RerunOrImportCurrentReceipt,
    AwaitOperatorDecision,
    ReturnToCodeRefine,
    UnauthorizedWaiver,
    StaleWaiver,
}

fn gate_routed_readiness(
    inputs: SynthesizerInputs<'_>,
    generation_id: String,
    route: GateRouted,
) -> CloseoutReadiness {
    let (status, decision, reason, unblock) = match route {
        GateRouted::AwaitGateDefinition => (
            CloseoutReadinessStatus::Unknown,
            CloseoutReadinessDecision::AwaitGateDefinition,
            format!(
                "gate missing_definition: proposal-077 gate not registered; \
                 run ./scripts/test-gate.sh proposal-077 to register"
            ),
            Some("Register the proposal-077 gate script".into()),
        ),
        GateRouted::RerunOrImportCurrentReceipt => (
            CloseoutReadinessStatus::Unknown,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            format!(
                "gate {}: stale or superseded receipt — rerun or import a current receipt",
                inputs.gate_result.status
            ),
            Some("Rerun or import current governed gate receipt".into()),
        ),
        GateRouted::AwaitOperatorDecision => (
            CloseoutReadinessStatus::Blocked,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            format!(
                "gate {}: {}",
                inputs.gate_result.status,
                inputs
                    .gate_result
                    .diagnostic_reason
                    .as_deref()
                    .unwrap_or("operator decision required")
            ),
            Some("Operator decision required for gate settlement".into()),
        ),
        GateRouted::ReturnToCodeRefine => (
            CloseoutReadinessStatus::NotReady,
            CloseoutReadinessDecision::ReturnToCodeRefine,
            "gate failed: code work required; proposal gate failed with budget remaining"
                .to_string(),
            Some("Fix gate failures and re-run proposal-077 gate".into()),
        ),
        GateRouted::UnauthorizedWaiver => (
            CloseoutReadinessStatus::Blocked,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            "waived gate rejected: authorization_lineage missing — \
             unmanaged waivers are not permitted; re-submit with full lineage"
                .to_string(),
            Some("Re-submit gate waiver with authorization_lineage and current_fingerprint".into()),
        ),
        GateRouted::StaleWaiver => (
            CloseoutReadinessStatus::Blocked,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            "waived gate rejected: current_fingerprint empty — \
             stale waivers without current fingerprint are not permitted"
                .to_string(),
            Some("Re-submit gate waiver with a non-empty current_fingerprint".into()),
        ),
    };

    CloseoutReadiness {
        run_id: inputs.run_id.to_string(),
        stage_id: inputs.stage_id.to_string(),
        status,
        decision,
        generation_id,
        readiness_mode: inputs.mode_result.effective_mode().as_str().to_string(),
        diagnostic_reason: Some(reason),
        primary_unblock: unblock,
        code_blocker_count: 0,
        handoff_owner: None,
        risk_settlement_required: false,
        fingerprint: inputs.fingerprint,
        synthesized_at: Utc::now(),
    }
}

/// Compute a stable FNV-1a digest over the current blocking code task set.
/// Used for soft convergence detection. Public so callers can persist and
/// compare across invocations.
pub fn compute_blocker_digest(
    assessment: &ImplementationSelfAssessmentSummary,
    gate: &ProposalGateResult,
) -> String {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    let mut h = FNV_OFFSET;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(FNV_PRIME);
        }
    };
    let count = assessment.blocking_remaining_code_task_count.unwrap_or(0) as u64;
    feed(&count.to_le_bytes());
    feed(gate.status.as_str().as_bytes());
    feed(b"\x01");
    for task in &assessment.remaining_code_tasks {
        if task.blocking {
            feed(task.summary.as_bytes());
            feed(b"\x02");
        }
    }
    format!("{h:016x}")
}

/// Apply the full R14 decision matrix after gate cause routing passes.
fn apply_decision_matrix(inputs: SynthesizerInputs<'_>, generation_id: String) -> CloseoutReadiness {
    let mode = inputs.mode_result.effective_mode();
    let assessment = inputs.self_assessment;

    // No self-assessment → unknown.
    let Some(assessment) = assessment else {
        return CloseoutReadiness {
            run_id: inputs.run_id.to_string(),
            stage_id: inputs.stage_id.to_string(),
            status: CloseoutReadinessStatus::Unknown,
            decision: CloseoutReadinessDecision::AwaitOperatorDecision,
            generation_id,
            readiness_mode: mode.as_str().to_string(),
            diagnostic_reason: Some("implementation self-assessment unavailable".into()),
            primary_unblock: Some("Implementation self-assessment artifact required".into()),
            code_blocker_count: 0,
            handoff_owner: None,
            risk_settlement_required: false,
            fingerprint: inputs.fingerprint,
            synthesized_at: Utc::now(),
        };
    };

    // In enforcement mode, controlled reports must all be green before any Ready path.
    // None means reports haven't been wired yet (advisory compatibility) — still fails
    // closed in enforcement mode.
    if mode.is_enforcement() && inputs.controlled_reports_green != Some(true) {
        return CloseoutReadiness {
            run_id: inputs.run_id.to_string(),
            stage_id: inputs.stage_id.to_string(),
            status: CloseoutReadinessStatus::Blocked,
            decision: CloseoutReadinessDecision::AwaitOperatorDecision,
            generation_id,
            readiness_mode: mode.as_str().to_string(),
            diagnostic_reason: Some(
                "enforcement_mode: controlled reports required — audit, docs, security, \
                 prepush, and tests must be green before manual release"
                    .into(),
            ),
            primary_unblock: Some("Ensure all controlled reports are green".into()),
            code_blocker_count: 0,
            handoff_owner: None,
            risk_settlement_required: false,
            fingerprint: inputs.fingerprint,
            synthesized_at: Utc::now(),
        };
    }

    let blocking_code_tasks = assessment
        .blocking_remaining_code_task_count
        .unwrap_or(0);

    // Code blockers present.
    if blocking_code_tasks > 0 {
        let blocker_digest = compute_blocker_digest(assessment, inputs.gate_result);

        // Soft convergence checkpoint: repeated identical blockers without diff or
        // gate progress route to AwaitOperatorDecision, without claiming P052 hard
        // budget exhaustion.
        let is_soft_convergence = inputs
            .previous_blocker_digest
            .map(|prev| prev == blocker_digest.as_str())
            .unwrap_or(false);

        let (decision, reason) = if is_soft_convergence {
            (
                CloseoutReadinessDecision::AwaitOperatorDecision,
                "soft_convergence_checkpoint: repeated identical blockers without diff or gate \
                 progress — operator decision required"
                    .to_string(),
            )
        } else if inputs.loop_budget_remaining {
            (
                CloseoutReadinessDecision::ReturnToCodeRefine,
                format!(
                    "{blocking_code_tasks} blocking code task(s) remain; budget remaining — returning to refine"
                ),
            )
        } else {
            (
                CloseoutReadinessDecision::AwaitOperatorDecision,
                format!(
                    "{blocking_code_tasks} blocking code task(s) remain; budget exhausted — operator decision required"
                ),
            )
        };

        return CloseoutReadiness {
            run_id: inputs.run_id.to_string(),
            stage_id: inputs.stage_id.to_string(),
            status: CloseoutReadinessStatus::NotReady,
            decision,
            generation_id,
            readiness_mode: mode.as_str().to_string(),
            diagnostic_reason: Some(reason),
            primary_unblock: Some("Resolve blocking code tasks".into()),
            code_blocker_count: blocking_code_tasks as u32,
            handoff_owner: None,
            risk_settlement_required: false,
            fingerprint: inputs.fingerprint,
            synthesized_at: Utc::now(),
        };
    }

    // Handoff tasks without code blockers.
    let handoff_tasks = assessment.handoff_task_count.unwrap_or(0);
    if handoff_tasks > 0 {
        let handoff_owner = assessment
            .handoff_tasks
            .first()
            .map(|t| t.owner_class.as_str().to_string());

        return CloseoutReadiness {
            run_id: inputs.run_id.to_string(),
            stage_id: inputs.stage_id.to_string(),
            status: CloseoutReadinessStatus::HandoffRequired,
            decision: CloseoutReadinessDecision::AwaitNonCodeHandoff,
            generation_id,
            readiness_mode: mode.as_str().to_string(),
            diagnostic_reason: Some(format!(
                "{handoff_tasks} handoff task(s) require non-code settlement"
            )),
            primary_unblock: Some("Settle handoff tasks through governed channels".into()),
            code_blocker_count: 0,
            handoff_owner,
            risk_settlement_required: false,
            fingerprint: inputs.fingerprint,
            synthesized_at: Utc::now(),
        };
    }

    // Check risk lineage — risks must have typed accepted lineage.
    let unaccepted_risk_count = inputs
        .accepted_risks
        .iter()
        .filter(|r| r.validate().is_err())
        .count();

    if unaccepted_risk_count > 0 || inputs.accepted_risks.is_empty() && !assessment.known_risks.is_empty() {
        let risk_settlement_required = !assessment.known_risks.is_empty();
        if risk_settlement_required {
            let accepted_count = inputs.accepted_risks.len();
            let known_count = assessment.known_risks.len();
            if accepted_count < known_count || !risks_satisfy_enter_manual_release(inputs.accepted_risks) {
                return CloseoutReadiness {
                    run_id: inputs.run_id.to_string(),
                    stage_id: inputs.stage_id.to_string(),
                    status: CloseoutReadinessStatus::ReadyWithRisks,
                    decision: CloseoutReadinessDecision::AwaitOperatorDecision,
                    generation_id,
                    readiness_mode: mode.as_str().to_string(),
                    diagnostic_reason: Some(
                        "risks present: typed risk acceptance lineage required for enter_manual_release".into()
                    ),
                    primary_unblock: Some("Provide typed risk acceptance lineage".into()),
                    code_blocker_count: 0,
                    handoff_owner: None,
                    risk_settlement_required: true,
                    fingerprint: inputs.fingerprint,
                    synthesized_at: Utc::now(),
                };
            }
        }
    }

    // Check if all risks are acceptably settled.
    let has_unresolved_risks = !assessment.known_risks.is_empty()
        && !risks_satisfy_enter_manual_release(inputs.accepted_risks);

    if has_unresolved_risks {
        return CloseoutReadiness {
            run_id: inputs.run_id.to_string(),
            stage_id: inputs.stage_id.to_string(),
            status: CloseoutReadinessStatus::ReadyWithRisks,
            decision: CloseoutReadinessDecision::AwaitOperatorDecision,
            generation_id,
            readiness_mode: mode.as_str().to_string(),
            diagnostic_reason: Some(
                "ready_with_risks: acceptance_required — typed lineage needed".into()
            ),
            primary_unblock: Some("Settle risks through governed waiver or release-owner decision".into()),
            code_blocker_count: 0,
            handoff_owner: None,
            risk_settlement_required: true,
            fingerprint: inputs.fingerprint,
            synthesized_at: Utc::now(),
        };
    }

    // Risks present and all settled.
    if !assessment.known_risks.is_empty()
        && risks_satisfy_enter_manual_release(inputs.accepted_risks)
    {
        let gate_is_waived = matches!(inputs.gate_result.status, ProposalGateStatus::Waived);
        let decision = if gate_is_waived {
            CloseoutReadinessDecision::EnterManualRelease
        } else {
            CloseoutReadinessDecision::EnterManualRelease
        };

        return CloseoutReadiness {
            run_id: inputs.run_id.to_string(),
            stage_id: inputs.stage_id.to_string(),
            status: CloseoutReadinessStatus::ReadyWithRisks,
            decision,
            generation_id,
            readiness_mode: mode.as_str().to_string(),
            diagnostic_reason: None,
            primary_unblock: None,
            code_blocker_count: 0,
            handoff_owner: None,
            risk_settlement_required: false,
            fingerprint: inputs.fingerprint,
            synthesized_at: Utc::now(),
        };
    }

    // All checks passed — ready for manual release.
    CloseoutReadiness {
        run_id: inputs.run_id.to_string(),
        stage_id: inputs.stage_id.to_string(),
        status: CloseoutReadinessStatus::Ready,
        decision: CloseoutReadinessDecision::EnterManualRelease,
        generation_id,
        readiness_mode: mode.as_str().to_string(),
        diagnostic_reason: None,
        primary_unblock: None,
        code_blocker_count: 0,
        handoff_owner: None,
        risk_settlement_required: false,
        fingerprint: inputs.fingerprint,
        synthesized_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::closeout_readiness_mode::{
        CloseoutReadinessModeError, CloseoutReadinessMode, CloseoutReadinessModeResult,
    };
    use domain::artifact_contracts::{
        ImplementationSelfAssessmentStatus, ImplementationSelfAssessmentSummary,
    };
    use domain::proposal_gate_result::{ProposalGateLineage, ProposalGateResult};
    use domain::risk_lineage::{RiskAcceptanceLineage, RiskAcceptanceSource, RiskClassification};

    fn advisory_mode() -> CloseoutReadinessModeResult {
        CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Advisory)
    }

    fn enforcement_mode() -> CloseoutReadinessModeResult {
        CloseoutReadinessModeResult::Known(CloseoutReadinessMode::Enforcement)
    }

    fn diagnostic_mode() -> CloseoutReadinessModeResult {
        CloseoutReadinessModeResult::Diagnostic(CloseoutReadinessModeError::Unknown(
            "malformed_mode_value".into(),
        ))
    }

    fn passed_gate() -> ProposalGateResult {
        ProposalGateResult {
            gate_id: "p077:077".into(),
            proposal_id: "077".into(),
            run_id: "run-1".into(),
            stage_id: "state_9".into(),
            status: ProposalGateStatus::Passed,
            generation_id: "gate-gen-1".into(),
            diagnostic_reason: None,
            executor_version: None,
            evidence_digest: None,
            exit_code: Some(0),
            elapsed_ms: Some(500),
            settled_at: Utc::now(),
            authorization_lineage: None,
            failure_classification: None,
        }
    }

    fn gate_with_status(status: ProposalGateStatus) -> ProposalGateResult {
        let mut g = passed_gate();
        g.status = status;
        g
    }

    fn waived_gate_with_lineage() -> ProposalGateResult {
        let mut g = gate_with_status(ProposalGateStatus::Waived);
        g.authorization_lineage = Some(ProposalGateLineage {
            principal: "operator".into(),
            capability: "gate.waive".into(),
            journal_id: "journal-waive-1".into(),
            authority: "release-owner".into(),
            reason: "test waiver".into(),
            source_artifacts: vec!["review/prepush.json".into()],
            run_id: "run-1".into(),
            proposal_id: "077".into(),
            stage_id: "state_9".into(),
            workflow_digest: "wf-digest".into(),
            worktree_head: "abcdef1".into(),
            dirty_or_changed_file_digest: "clean".into(),
            source_generation_ids: vec!["gen-1".into()],
            current_fingerprint: "fp-abc12345".into(),
        });
        g
    }

    fn complete_assessment() -> ImplementationSelfAssessmentSummary {
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

    fn valid_risk_lineage() -> RiskAcceptanceLineage {
        RiskAcceptanceLineage {
            risk_id: "RISK-001".into(),
            title: "Parallel release policy".into(),
            classification: RiskClassification::Medium,
            authority: "release-owner".into(),
            journal_or_decision_id: "journal-abc".into(),
            source_generation_ids: vec!["gen-1".into()],
            settled_at: Utc::now(),
            acceptance_source: RiskAcceptanceSource::TypedControlledRiskRow,
            rationale: None,
        }
    }

    #[test]
    fn missing_gate_produces_await_gate_definition() {
        let gate = gate_with_status(ProposalGateStatus::MissingDefinition);
        let assessment = complete_assessment();
        let mode = advisory_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::AwaitGateDefinition,
            "missing gate must route to await_gate_definition"
        );
        assert!(
            !matches!(
                result.readiness.decision,
                CloseoutReadinessDecision::EnterManualRelease
            ),
            "missing gate cannot enter manual release"
        );
    }

    #[test]
    fn code_blockers_with_budget_remaining_returns_to_code_refine() {
        let gate = passed_gate();
        let mut assessment = complete_assessment();
        assessment.blocking_remaining_code_task_count = Some(2);
        assessment.status = ImplementationSelfAssessmentStatus::NeedsCodeFixes;
        let mode = advisory_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::ReturnToCodeRefine,
            "code blockers + budget remaining must return to code refine"
        );
        assert_eq!(result.readiness.code_blocker_count, 2);
    }

    #[test]
    fn code_blockers_with_exhausted_budget_awaits_operator_decision() {
        let gate = passed_gate();
        let mut assessment = complete_assessment();
        assessment.blocking_remaining_code_task_count = Some(1);
        let mode = advisory_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
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
            "code blockers + exhausted budget must await operator decision"
        );
    }

    #[test]
    fn no_code_blockers_with_handoff_routes_to_non_code_handoff() {
        let gate = passed_gate();
        let mut assessment = complete_assessment();
        assessment.handoff_task_count = Some(1);
        assessment.blocking_remaining_code_task_count = Some(0);
        let mode = advisory_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.status,
            CloseoutReadinessStatus::HandoffRequired
        );
        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::AwaitNonCodeHandoff,
        );
    }

    #[test]
    fn green_gate_green_reports_zero_blockers_no_risks_enters_manual_release() {
        let gate = passed_gate();
        let assessment = complete_assessment();
        let mode = enforcement_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: Some(true),
            previous_blocker_digest: None,
        });

        assert_eq!(result.readiness.status, CloseoutReadinessStatus::Ready);
        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::EnterManualRelease
        );
    }

    #[test]
    fn ready_with_risks_with_accepted_lineage_enters_manual_release() {
        let gate = passed_gate();
        let mut assessment = complete_assessment();
        assessment.known_risks = vec!["parallel release policy".into()];
        let mode = enforcement_mode();
        let risks = vec![valid_risk_lineage()];

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &risks,
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: Some(true),
            previous_blocker_digest: None,
        });

        assert_eq!(result.readiness.status, CloseoutReadinessStatus::ReadyWithRisks);
        assert_eq!(result.readiness.decision, CloseoutReadinessDecision::EnterManualRelease);
        assert!(!result.readiness.risk_settlement_required);
    }

    #[test]
    fn fingerprint_latency_exceeded_fails_closed() {
        let gate = passed_gate();
        let assessment = complete_assessment();
        let mode = advisory_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: true,
            controlled_reports_green: None,
            previous_blocker_digest: None,
        });

        assert_eq!(result.readiness.status, CloseoutReadinessStatus::Unknown);
        let reason = result.readiness.diagnostic_reason.unwrap_or_default();
        assert!(
            reason.contains("closeout_fingerprint_unavailable"),
            "diagnostic_reason must mention fingerprint unavailability"
        );
    }

    #[test]
    fn unauthorized_gate_awaits_operator_decision_not_code_refine() {
        let gate = gate_with_status(ProposalGateStatus::Unauthorized);
        let assessment = complete_assessment();
        let mode = advisory_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            "unauthorized gate must not invoke code_writer or return to refine"
        );
    }

    #[test]
    fn failed_gate_with_exhausted_budget_awaits_operator_decision() {
        let gate = gate_with_status(ProposalGateStatus::Failed);
        let assessment = complete_assessment();
        let mode = advisory_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
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
        );
    }

    #[test]
    fn synthesizer_produces_unique_generation_ids() {
        let gate = passed_gate();
        let assessment = complete_assessment();
        let mode = advisory_mode();

        let r1 = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: None,
        });
        let r2 = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: None,
        });

        assert_ne!(
            r1.readiness.generation_id, r2.readiness.generation_id,
            "each synthesis must produce a unique generation_id"
        );
    }

    // ── New fail-closed regression tests ─────────────────────────────────────

    #[test]
    fn diagnostic_mode_fails_closed_cannot_enter_manual_release() {
        let gate = passed_gate();
        let assessment = complete_assessment();
        let mode = diagnostic_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: Some(true),
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            "diagnostic mode must fail closed with await_operator_decision"
        );
        assert_ne!(
            result.readiness.decision,
            CloseoutReadinessDecision::EnterManualRelease,
            "diagnostic mode CANNOT enter manual release"
        );
        let reason = result.readiness.diagnostic_reason.unwrap_or_default();
        assert!(
            reason.contains("diagnostic_mode"),
            "diagnostic_reason must mention diagnostic_mode: got {reason}"
        );
    }

    #[test]
    fn unknown_malformed_mode_fails_closed() {
        let gate = passed_gate();
        let assessment = complete_assessment();
        let mode = CloseoutReadinessModeResult::Diagnostic(CloseoutReadinessModeError::Malformed(
            "conflicting values".into(),
        ));

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: Some(true),
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::AwaitOperatorDecision,
        );
        assert_eq!(result.readiness.status, CloseoutReadinessStatus::Blocked);
    }

    #[test]
    fn waived_gate_without_lineage_fails_closed() {
        let gate = gate_with_status(ProposalGateStatus::Waived);
        // authorization_lineage is None by default
        let assessment = complete_assessment();
        let mode = enforcement_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: Some(true),
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            "waived gate without authorization_lineage must fail closed"
        );
        assert_ne!(
            result.readiness.decision,
            CloseoutReadinessDecision::EnterManualRelease,
            "waived gate without lineage CANNOT enter manual release"
        );
        let reason = result.readiness.diagnostic_reason.unwrap_or_default();
        assert!(
            reason.contains("authorization_lineage missing"),
            "diagnostic_reason must mention missing authorization_lineage: got {reason}"
        );
    }

    #[test]
    fn waived_gate_with_empty_fingerprint_fails_closed() {
        let mut gate = gate_with_status(ProposalGateStatus::Waived);
        gate.authorization_lineage = Some(ProposalGateLineage {
            principal: "operator".into(),
            capability: "gate.waive".into(),
            journal_id: "j-1".into(),
            authority: "release-owner".into(),
            reason: "test".into(),
            source_artifacts: vec![],
            run_id: "run-1".into(),
            proposal_id: "077".into(),
            stage_id: "state_9".into(),
            workflow_digest: "wf".into(),
            worktree_head: "head".into(),
            dirty_or_changed_file_digest: "clean".into(),
            source_generation_ids: vec!["gen-1".into()],
            current_fingerprint: "".into(), // empty — stale waiver
        });
        let assessment = complete_assessment();
        let mode = enforcement_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: Some(true),
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            "waived gate with empty current_fingerprint must fail closed"
        );
        let reason = result.readiness.diagnostic_reason.unwrap_or_default();
        assert!(
            reason.contains("current_fingerprint empty"),
            "diagnostic_reason must mention empty fingerprint: got {reason}"
        );
    }

    #[test]
    fn waived_gate_with_valid_lineage_and_fingerprint_continues_matrix() {
        let gate = waived_gate_with_lineage();
        let assessment = complete_assessment();
        let mode = enforcement_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: Some(true),
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::EnterManualRelease,
            "waived gate with valid lineage and fingerprint must continue to matrix"
        );
    }

    #[test]
    fn enforcement_mode_without_controlled_reports_fails_closed() {
        let gate = passed_gate();
        let assessment = complete_assessment();
        let mode = enforcement_mode();

        // controlled_reports_green is None — should fail closed in enforcement mode.
        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            "enforcement mode without controlled reports must fail closed"
        );
        assert_eq!(result.readiness.status, CloseoutReadinessStatus::Blocked);
        let reason = result.readiness.diagnostic_reason.unwrap_or_default();
        assert!(
            reason.contains("enforcement_mode"),
            "diagnostic_reason must mention enforcement_mode: got {reason}"
        );
    }

    #[test]
    fn enforcement_mode_with_failed_controlled_reports_fails_closed() {
        let gate = passed_gate();
        let assessment = complete_assessment();
        let mode = enforcement_mode();

        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: Some(false),
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            "enforcement mode with failing controlled reports must fail closed"
        );
    }

    #[test]
    fn advisory_mode_without_controlled_reports_can_enter_manual_release() {
        let gate = passed_gate();
        let assessment = complete_assessment();
        let mode = advisory_mode();

        // Advisory mode doesn't require controlled_reports_green.
        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: None,
        });

        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::EnterManualRelease,
            "advisory mode can enter manual release without controlled reports"
        );
    }

    #[test]
    fn soft_convergence_repeated_identical_blockers_awaits_operator_decision() {
        let gate = passed_gate();
        let mut assessment = complete_assessment();
        assessment.blocking_remaining_code_task_count = Some(1);
        assessment.status = ImplementationSelfAssessmentStatus::NeedsCodeFixes;
        let mode = advisory_mode();

        // First invocation without previous digest → returns to code refine.
        let r1 = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: None, // no prior digest
        });
        assert_eq!(
            r1.readiness.decision,
            CloseoutReadinessDecision::ReturnToCodeRefine,
            "first occurrence with no previous digest must return to code refine"
        );

        // Compute the blocker digest from the first result to simulate a repeat.
        let digest = compute_blocker_digest(&assessment, &gate);

        // Second invocation with the same blockers and the prior digest → soft convergence.
        let r2 = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment),
            accepted_risks: &[],
            loop_budget_remaining: true, // budget NOT exhausted
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: Some(&digest), // same blockers as before
        });
        assert_eq!(
            r2.readiness.decision,
            CloseoutReadinessDecision::AwaitOperatorDecision,
            "repeated identical blockers must trigger soft convergence → await_operator_decision"
        );
        assert_ne!(
            r2.readiness.decision,
            CloseoutReadinessDecision::ReturnToCodeRefine,
            "soft convergence must NOT return to code refine"
        );
        let reason = r2.readiness.diagnostic_reason.unwrap_or_default();
        assert!(
            reason.contains("soft_convergence_checkpoint"),
            "diagnostic_reason must mention soft_convergence_checkpoint: got {reason}"
        );
    }

    #[test]
    fn different_blockers_do_not_trigger_soft_convergence() {
        let gate = passed_gate();
        let mut assessment1 = complete_assessment();
        assessment1.blocking_remaining_code_task_count = Some(1);
        let mut assessment2 = complete_assessment();
        assessment2.blocking_remaining_code_task_count = Some(2);
        let mode = advisory_mode();

        let digest1 = compute_blocker_digest(&assessment1, &gate);

        // Different blocker count → different digest → no soft convergence.
        let result = synthesize_implementation_closeout_readiness_for_state9(SynthesizerInputs {
            run_id: "run-1",
            stage_id: "state_9",
            gate_result: &gate,
            mode_result: &mode,
            self_assessment: Some(&assessment2),
            accepted_risks: &[],
            loop_budget_remaining: true,
            fingerprint: None,
            fingerprint_latency_exceeded: false,
            controlled_reports_green: None,
            previous_blocker_digest: Some(&digest1),
        });
        assert_eq!(
            result.readiness.decision,
            CloseoutReadinessDecision::ReturnToCodeRefine,
            "different blockers must not trigger soft convergence"
        );
    }
}
