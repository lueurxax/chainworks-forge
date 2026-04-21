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

    let invalid = normalize_contract_status("prepush_review_v1", "PASSISH").unwrap();
    assert_eq!(invalid.canonical_status, "invalid");
    assert!(!invalid.valid);
    assert!(invalid.validation_errors[0].contains("PASSISH"));
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
