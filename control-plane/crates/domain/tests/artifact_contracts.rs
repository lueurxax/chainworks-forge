use domain::artifact_contracts::{
    invalid_implementation_self_assessment_summary, normalize_contract_status,
    parse_implementation_self_assessment_v2, ContractParseContext, HandoffOwnerClass,
    ImplementationSelfAssessmentStatus, IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
    IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
};
use serde_json::{json, Value};

fn context() -> ContractParseContext {
    ContractParseContext {
        run_id: "run-123".to_string(),
        declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.to_string()),
        canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.to_string(),
        ..ContractParseContext::default()
    }
}

fn valid_v2() -> Value {
    json!({
        "implementation_complete": true,
        "verification_green": true,
        "remaining_code_tasks": [],
        "handoff_tasks": [],
        "known_risks": [],
        "tests_run": ["cargo test -p domain"],
        "docs_impacted": []
    })
}

#[test]
fn complete_v2_derives_complete_status() {
    let summary = parse_implementation_self_assessment_v2(&valid_v2(), context());

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Complete);
    assert_eq!(summary.implementation_complete, Some(true));
    assert_eq!(summary.verification_green, Some(true));
    assert!(summary.validation_errors.is_empty());
}

#[test]
fn missing_required_top_level_field_is_invalid() {
    let mut raw = valid_v2();
    raw.as_object_mut().unwrap().remove("verification_green");

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Invalid);
    assert!(summary
        .validation_errors
        .iter()
        .any(|error| error.pointer == "/verification_green"));
}

#[test]
fn missing_blocking_field_is_invalid_not_nonblocking() {
    let mut raw = valid_v2();
    raw["remaining_code_tasks"] = json!([{
        "summary": "finish parser integration",
        "owner": "code_writer",
        "evidence": "targeted parser test still failing"
    }]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Invalid);
    assert_eq!(summary.remaining_code_task_count, Some(0));
    assert!(summary
        .validation_errors
        .iter()
        .any(|error| error.pointer == "/remaining_code_tasks/0/blocking"));
}

#[test]
fn missing_blocking_review_field_is_invalid_not_nonblocking() {
    let mut raw = valid_v2();
    raw["handoff_tasks"] = json!([{
        "summary": "collect CloudKit signed-in smoke evidence",
        "owner_class": "manual_evidence",
        "target_stage": "state_9_implementation_review",
        "evidence": "manual CloudKit smoke check remains outside code_writer"
    }]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Invalid);
    assert_eq!(summary.handoff_task_count, Some(0));
    assert!(summary
        .validation_errors
        .iter()
        .any(|error| error.pointer == "/handoff_tasks/0/blocking_review"));
}

#[test]
fn invalid_owner_class_is_invalid() {
    let mut raw = valid_v2();
    raw["handoff_tasks"] = json!([{
        "summary": "prepare release decision",
        "owner_class": "calendar",
        "target_stage": "state_13_release_gate",
        "blocking_review": true,
        "evidence": "release go/no-go owner must decide from review packet"
    }]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Invalid);
    assert!(summary
        .validation_errors
        .iter()
        .any(|error| error.code == "invalid_owner_class"));
}

#[test]
fn empty_nested_evidence_is_invalid() {
    let mut raw = valid_v2();
    raw["remaining_code_tasks"] = json!([{
        "summary": "wire db persistence",
        "owner": "code_writer",
        "blocking": true,
        "evidence": " "
    }]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Invalid);
    assert!(summary
        .validation_errors
        .iter()
        .any(|error| error.pointer == "/remaining_code_tasks/0/evidence"));
}

#[test]
fn non_array_task_fields_are_invalid() {
    let mut raw = valid_v2();
    raw["remaining_code_tasks"] = json!({"summary": "not an array"});

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Invalid);
    assert!(summary
        .validation_errors
        .iter()
        .any(|error| error.pointer == "/remaining_code_tasks"));
}

#[test]
fn malformed_array_entries_are_invalid() {
    let mut raw = valid_v2();
    raw["handoff_tasks"] = json!(["not an object"]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Invalid);
    assert!(summary
        .validation_errors
        .iter()
        .any(|error| error.pointer == "/handoff_tasks/0"));
}

#[test]
fn raw_only_v2_artifacts_are_invalid() {
    let mut raw_only_context = context();
    raw_only_context.declared_contract_id = None;

    let summary = parse_implementation_self_assessment_v2(&valid_v2(), raw_only_context);

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Invalid);
    assert!(summary
        .validation_errors
        .iter()
        .any(|error| error.code == "raw_only_v2_artifact"));
}

#[test]
fn malformed_json_summary_is_invalid_with_validation_evidence() {
    let summary = invalid_implementation_self_assessment_summary(
        context(),
        "malformed_json",
        "implementation self-assessment artifact is not valid JSON",
        "",
    );

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Invalid);
    assert!(summary.raw_artifact_available);
    assert!(summary
        .validation_errors
        .iter()
        .any(|error| error.code == "malformed_json" && error.pointer.is_empty()));
}

#[test]
fn blocked_status_exits_code_loop_when_verification_is_not_green() {
    let mut raw = valid_v2();
    raw["verification_green"] = json!(false);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Blocked);
    assert_eq!(summary.blocking_remaining_code_task_count, Some(0));
}

#[test]
fn handoff_tasks_do_not_force_code_fix_status() {
    let mut raw = valid_v2();
    raw["handoff_tasks"] = json!([{
        "summary": "write release approval packet",
        "owner_class": "release",
        "target_stage": "state_13_release_gate",
        "blocking_review": true,
        "evidence": "release approval remains a downstream operator decision"
    }]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(
        summary.status,
        ImplementationSelfAssessmentStatus::HandoffRequired
    );
    assert_eq!(summary.owner_class_counts.get("release"), Some(&1));
    assert_eq!(summary.blocking_review_handoff_task_count, Some(1));
}

#[test]
fn nonblocking_code_tail_does_not_keep_implementation_loop_open() {
    let mut raw = valid_v2();
    raw["implementation_complete"] = json!(false);
    raw["remaining_code_tasks"] = json!([{
        "summary": "defer catalog backfill until dogfood evidence is available",
        "owner": "code_writer",
        "blocking": false,
        "evidence": "proposal gate is green; rollout evidence is owned downstream"
    }]);
    raw["handoff_tasks"] = json!([{
        "summary": "collect dogfood evidence before catalog backfill",
        "owner_class": "manual_evidence",
        "target_stage": "phase_1_dogfood",
        "blocking_review": false,
        "evidence": "manual dogfood thresholds are not code-writer-owned"
    }]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(
        summary.status,
        ImplementationSelfAssessmentStatus::HandoffRequired
    );
    assert_eq!(summary.blocking_remaining_code_task_count, Some(0));
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.code == "suspicious_nonblocking_code_tasks_with_handoff"));
}

#[test]
fn nonblocking_code_writer_task_with_red_verification_forces_code_fixes() {
    let mut raw = valid_v2();
    raw["verification_green"] = json!(false);
    raw["remaining_code_tasks"] = json!([{
        "summary": "Phase 2 background continuation worker is not implemented",
        "owner": "code_writer",
        "blocking": false,
        "evidence": "p086-continuation-readback is red until the worker produces runtime evidence"
    }]);
    raw["handoff_tasks"] = json!([{
        "summary": "collect rollout evidence after worker implementation",
        "owner_class": "manual_evidence",
        "target_stage": "phase_2_operator_command",
        "blocking_review": true,
        "evidence": "runtime evidence depends on code-owned worker paths"
    }]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(
        summary.status,
        ImplementationSelfAssessmentStatus::NeedsCodeFixes
    );
    assert_eq!(summary.blocking_remaining_code_task_count, Some(0));
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.code == "suspicious_nonblocking_code_tasks_with_handoff"));
}

#[test]
fn generic_status_normalization_accepts_handoff_required() {
    let normalized = normalize_contract_status(
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
        "handoff_required",
    )
    .unwrap();

    assert!(normalized.valid);
    assert_eq!(normalized.canonical_status, "handoff_required");
}

#[test]
fn owner_class_unknown_is_allowed_and_counted() {
    let mut raw = valid_v2();
    raw["handoff_tasks"] = json!([{
        "summary": "triage external approval owner",
        "owner_class": "unknown",
        "target_stage": "state_9_implementation_review",
        "blocking_review": false,
        "evidence": "operator must classify the external approval owner"
    }]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(
        summary.status,
        ImplementationSelfAssessmentStatus::HandoffRequired
    );
    assert_eq!(
        summary.handoff_tasks[0].owner_class,
        HandoffOwnerClass::Unknown
    );
    assert_eq!(summary.owner_class_counts.get("unknown"), Some(&1));
}

#[test]
fn suspicious_classification_warnings_are_emitted() {
    let mut raw = valid_v2();
    raw["remaining_code_tasks"] = json!([{
        "summary": "non-blocking cleanup",
        "owner": "code_writer",
        "blocking": false,
        "evidence": "tbd"
    }]);
    raw["handoff_tasks"] = json!([
        {
            "summary": "manual smoke evidence",
            "owner_class": "unknown",
            "target_stage": "state_9_implementation_review",
            "blocking_review": false,
            "evidence": "n/a"
        },
        {
            "summary": "release decision",
            "owner_class": "unknown",
            "target_stage": "state_13_release_gate",
            "blocking_review": true,
            "evidence": "tbd"
        }
    ]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(
        summary.status,
        ImplementationSelfAssessmentStatus::HandoffRequired
    );
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.code == "suspicious_nonblocking_code_tasks_with_handoff"));
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.code == "multiple_unknown_owner_class"));
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.code == "vague_evidence"));
}

#[test]
fn display_arrays_omit_non_string_entries_and_clear_short_evidence_is_not_vague() {
    let mut raw = valid_v2();
    raw["known_risks"] = json!(["operator note", {"not": "display-safe"}]);
    raw["tests_run"] = json!(["CI green", true]);
    raw["remaining_code_tasks"] = json!([{
        "summary": "non-blocking cleanup",
        "owner": "code_writer",
        "blocking": false,
        "evidence": "CI green"
    }]);

    let summary = parse_implementation_self_assessment_v2(&raw, context());

    assert_eq!(summary.known_risks, vec!["operator note"]);
    assert_eq!(summary.tests_run, vec!["CI green"]);
    assert!(!summary
        .warnings
        .iter()
        .any(|warning| warning.code == "vague_evidence"));
}

#[test]
fn v1_compatibility_maps_seemingly_complete() {
    let mut legacy_context = context();
    legacy_context.declared_contract_id = Some("implementation_self_assessment".to_string());

    let summary = parse_implementation_self_assessment_v2(
        &json!({
            "seemingly_complete": true
        }),
        legacy_context,
    );

    assert_eq!(summary.implementation_complete, Some(true));
    assert_eq!(summary.status, ImplementationSelfAssessmentStatus::Unknown);
    assert_eq!(summary.remaining_code_task_count, None);
    assert_eq!(summary.blocking_remaining_code_task_count, None);
    assert_eq!(summary.handoff_task_count, None);
    assert_eq!(summary.blocking_review_handoff_task_count, None);
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.code == "legacy_v1_compatibility"));
    assert!(summary
        .warnings
        .iter()
        .any(|warning| warning.code == "legacy_v1_dimensions_unavailable"));
}
