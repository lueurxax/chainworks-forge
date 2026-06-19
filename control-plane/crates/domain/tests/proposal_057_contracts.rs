use domain::artifact_contracts::{
    normalize_contract_status, DegradedOutputPolicy, DegradedOutputPolicyMode,
    FailedExecutionSettlement,
};

#[test]
fn proposal_057_normalizes_agent_status_vocabularies() {
    let prepush = normalize_contract_status("prepush_review_v1", "PASS_WITH_NOTES").unwrap();
    assert_eq!(prepush.canonical_status, "pass");
    assert!(prepush.valid);
    assert_eq!(prepush.raw_status, "PASS_WITH_NOTES");

    let docs = normalize_contract_status("docs_report_v1", "synced").unwrap();
    assert_eq!(docs.canonical_status, "pass");

    let docs_aligned = normalize_contract_status("docs_report_v1", "aligned").unwrap();
    assert_eq!(docs_aligned.canonical_status, "pass");

    let invalid = normalize_contract_status("prepush_review_v1", "PASSISH").unwrap();
    assert_eq!(invalid.canonical_status, "invalid");
    assert!(!invalid.valid);
    assert!(invalid.validation_errors[0].contains("PASSISH"));
}

#[test]
fn proposal_057_normalizes_live_review_blocker_statuses() {
    let prepush_block = normalize_contract_status("prepush_review_v1", "changes_required").unwrap();
    assert_eq!(prepush_block.canonical_status, "block");
    assert!(prepush_block.valid);

    let prepush_conditional =
        normalize_contract_status("prepush_review_v1", "conditional_pass").unwrap();
    assert_eq!(prepush_conditional.canonical_status, "pass");
    assert!(prepush_conditional.valid);

    let review_changes =
        normalize_contract_status("implementation_review_summary_v1", "changes_required").unwrap();
    assert_eq!(review_changes.canonical_status, "needs_code_fixes");
    assert!(review_changes.valid);

    let review_blocked =
        normalize_contract_status("implementation_review_summary_v1", "blocked").unwrap();
    assert_eq!(review_blocked.canonical_status, "needs_code_fixes");
    assert!(review_blocked.valid);

    let security_fail = normalize_contract_status("security_report_v1", "fail").unwrap();
    assert_eq!(security_fail.canonical_status, "block");
    assert!(security_fail.valid);

    let security_pass_with_notes =
        normalize_contract_status("security_report_v1", "pass_with_notes").unwrap();
    assert_eq!(security_pass_with_notes.canonical_status, "pass");
    assert!(security_pass_with_notes.valid);
}

#[test]
fn proposal_094_normalizes_boundary_contract_statuses() {
    let decomposition =
        normalize_contract_status("proposal_decomposition_plan_v1", "split_required").unwrap();
    assert_eq!(decomposition.canonical_status, "split_required");
    assert!(decomposition.valid);

    let boundary = normalize_contract_status(
        "blocker_boundary_status_v1",
        "awaiting_human_boundary_approval",
    )
    .unwrap();
    assert_eq!(
        boundary.canonical_status,
        "awaiting_human_boundary_approval"
    );
    assert!(boundary.valid);

    let accepted =
        normalize_contract_status("blocker_boundary_human_decision_v1", "accept").unwrap();
    assert_eq!(accepted.canonical_status, "granted");
    assert!(accepted.valid);

    let rejected =
        normalize_contract_status("blocker_boundary_human_decision_v1", "reject").unwrap();
    assert_eq!(rejected.canonical_status, "rejected");
    assert!(rejected.valid);
}

#[test]
fn proposal_057_degraded_policy_is_default_deny_and_explicit_allow_only() {
    let default_policy = DegradedOutputPolicy::default();
    assert_eq!(default_policy.mode, DegradedOutputPolicyMode::Deny);
    assert!(!default_policy.allows(
        "prepush_review_v1",
        "provider_quota",
        FailedExecutionSettlement::ValidOutputsFromFailedExecution
    ));

    let allow = DegradedOutputPolicy::allow_valid_contract_outputs(
        vec!["prepush_review_v1".to_string()],
        vec!["provider_quota".to_string()],
    )
    .unwrap();
    assert!(allow.allows(
        "prepush_review_v1",
        "provider_quota",
        FailedExecutionSettlement::ValidOutputsFromFailedExecution
    ));
    assert!(!allow.allows(
        "docs_report_v1",
        "provider_quota",
        FailedExecutionSettlement::ValidOutputsFromFailedExecution
    ));
}
