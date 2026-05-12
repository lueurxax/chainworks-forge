use std::str::FromStr;

use chrono::Utc;
use domain::workflow_conflict::{
    candidate_transition_hash, classify_workflow_conflict_reason,
    proposal_review_summary_v1_authority_table, proposal_review_summary_v1_field_authority,
    proposal_review_summary_v2_authority_table, proposal_review_summary_v2_field_authority,
    workflow_conflict_fingerprint, AdvisoryHintExtraction, AggregateFieldAuthority,
    CandidateTransitionEvaluation, CandidateTransitionResult, WorkflowAdvisoryRejectionRecord,
    WorkflowConflictReason, WorkflowConflictRecord, WorkflowConflictStatus,
};
use serde_json::Value;

const FIXTURE_INVENTORY: &str = include_str!("fixtures/proposal_017_fixture_inventory.json");

#[test]
fn proposal_017_conflict_enums_round_trip_surface_casing() {
    let reasons = [
        (
            WorkflowConflictReason::InvalidNextStageHint,
            "invalid_next_stage_hint",
            "INVALID_NEXT_STAGE_HINT",
        ),
        (
            WorkflowConflictReason::NoDeclarativeTransitionMatched,
            "no_declarative_transition_matched",
            "NO_DECLARATIVE_TRANSITION_MATCHED",
        ),
        (
            WorkflowConflictReason::MultipleDeclarativeTransitionsMatchedWithoutTieBreak,
            "multiple_declarative_transitions_matched_without_tie_break",
            "MULTIPLE_DECLARATIVE_TRANSITIONS_MATCHED_WITHOUT_TIE_BREAK",
        ),
        (
            WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition,
            "required_artifact_or_field_missing_for_transition",
            "REQUIRED_ARTIFACT_OR_FIELD_MISSING_FOR_TRANSITION",
        ),
        (
            WorkflowConflictReason::AggregateTransitionTruthConflicted,
            "aggregate_transition_truth_conflicted",
            "AGGREGATE_TRANSITION_TRUTH_CONFLICTED",
        ),
        (
            WorkflowConflictReason::WorkflowConflictUnverifiable,
            "workflow_conflict_unverifiable",
            "WORKFLOW_CONFLICT_UNVERIFIABLE",
        ),
        (
            WorkflowConflictReason::ImplementationHandoffUnavailable,
            "implementation_handoff_unavailable",
            "IMPLEMENTATION_HANDOFF_UNAVAILABLE",
        ),
    ];
    for (reason, snake, graphql) in reasons {
        assert_eq!(reason.to_string(), snake);
        assert_eq!(reason.graphql_name(), graphql);
        assert_eq!(WorkflowConflictReason::from_str(snake).unwrap(), reason);
        assert_eq!(
            serde_json::to_value(&reason).unwrap(),
            Value::String(snake.to_string())
        );
    }

    let statuses = [
        (
            WorkflowConflictStatus::Unresolved,
            "unresolved",
            "UNRESOLVED",
            true,
        ),
        (
            WorkflowConflictStatus::LeadMediationPending,
            "lead_mediation_pending",
            "LEAD_MEDIATION_PENDING",
            true,
        ),
        (
            WorkflowConflictStatus::OperatorConfirmationRequired,
            "operator_confirmation_required",
            "OPERATOR_CONFIRMATION_REQUIRED",
            true,
        ),
        (
            WorkflowConflictStatus::Resolved,
            "resolved",
            "RESOLVED",
            false,
        ),
        (
            WorkflowConflictStatus::Superseded,
            "superseded",
            "SUPERSEDED",
            false,
        ),
        (
            WorkflowConflictStatus::TerminalUnverifiable,
            "terminal_unverifiable",
            "TERMINAL_UNVERIFIABLE",
            false,
        ),
    ];
    for (status, snake, graphql, current_blocking) in statuses {
        assert_eq!(status.to_string(), snake);
        assert_eq!(status.graphql_name(), graphql);
        assert_eq!(status.is_current_blocking(), current_blocking);
        assert_eq!(WorkflowConflictStatus::from_str(snake).unwrap(), status);
    }

    let results = [
        (CandidateTransitionResult::Matched, "matched", "MATCHED"),
        (
            CandidateTransitionResult::NotMatched,
            "not_matched",
            "NOT_MATCHED",
        ),
        (
            CandidateTransitionResult::MissingInput,
            "missing_input",
            "MISSING_INPUT",
        ),
        (
            CandidateTransitionResult::InvalidExpression,
            "invalid_expression",
            "INVALID_EXPRESSION",
        ),
        (
            CandidateTransitionResult::EvaluationError,
            "evaluation_error",
            "EVALUATION_ERROR",
        ),
    ];
    for (result, snake, graphql) in results {
        assert_eq!(result.to_string(), snake);
        assert_eq!(result.graphql_name(), graphql);
        assert_eq!(CandidateTransitionResult::from_str(snake).unwrap(), result);
    }
}

#[test]
fn proposal_017_record_contracts_serialize_with_required_fields() {
    let now = Utc::now();
    let candidate = CandidateTransitionEvaluation {
        transition_id: "proposal_review_failed_refine".to_string(),
        from_state_id: "state_4_proposal_reviewed".to_string(),
        to_state_id: "state_5_proposal_refined".to_string(),
        condition_expression_id: Some("proposal_review_summary.pass_false".to_string()),
        result: CandidateTransitionResult::Matched,
        required_artifacts: vec!["proposal_review_summary".to_string()],
        missing_artifacts: vec![],
        missing_fields: vec![],
        source_artifact_ids: vec!["proposal_review_summary:latest".to_string()],
        source_agent_execution_id: Some("agent-exec-review-aggregate".to_string()),
        sanitized_diagnostic: None,
    };
    let conflict = WorkflowConflictRecord {
        conflict_id: "conflict-1".to_string(),
        conflict_fingerprint: "sha256:conflict".to_string(),
        run_id: "run-1".to_string(),
        stage_execution_id: Some("stage-1".to_string()),
        lineage_id: Some("lineage-1".to_string()),
        current_state_id: "state_4_proposal_reviewed".to_string(),
        reason: WorkflowConflictReason::NoDeclarativeTransitionMatched,
        operator_label: "No declarative transition matched".to_string(),
        status: WorkflowConflictStatus::Unresolved,
        candidate_transitions: vec![candidate.clone()],
        candidate_transition_hash: "sha256:candidates".to_string(),
        advisory_evidence_refs: vec!["run_state_projection.next_stage".to_string()],
        lead_agent_id: None,
        mediation_record_id: None,
        created_at: now,
        updated_at: now,
        resolved_at: None,
        superseded_by_conflict_id: None,
        resolution_record_json: None,
        terminal_failure_reason: None,
        diagnostic_redaction_tier: "operator_safe".to_string(),
    };
    let value = serde_json::to_value(&conflict).unwrap();
    for required in [
        "conflict_id",
        "conflict_fingerprint",
        "run_id",
        "stage_execution_id",
        "lineage_id",
        "current_state_id",
        "reason",
        "operator_label",
        "status",
        "candidate_transitions",
        "candidate_transition_hash",
        "advisory_evidence_refs",
        "lead_agent_id",
        "mediation_record_id",
        "created_at",
        "updated_at",
        "resolved_at",
        "superseded_by_conflict_id",
        "resolution_record_json",
        "terminal_failure_reason",
        "diagnostic_redaction_tier",
    ] {
        assert!(value.get(required).is_some(), "missing field {required}");
    }

    let advisory_hint = AdvisoryHintExtraction {
        source_artifact_id: "proposal_review_summary:latest".to_string(),
        source_agent_execution_id: Some("agent-exec-review-aggregate".to_string()),
        advisory_path: "$.next_stage".to_string(),
        raw_value_hash: "sha256:raw-advisory".to_string(),
        redacted_value: Some("state_3_proposal_drafted".to_string()),
        graph_membership_result: "absent_from_graph".to_string(),
        superseded_by_projection: false,
        included_in_candidate_transition_hash: true,
    };
    let rejection = WorkflowAdvisoryRejectionRecord {
        rejection_id: "rejection-1".to_string(),
        run_id: "run-1".to_string(),
        stage_execution_id: Some("stage-1".to_string()),
        lineage_id: Some("lineage-1".to_string()),
        current_state_id: "state_4_proposal_reviewed".to_string(),
        selected_transition_id: candidate.transition_id,
        selected_next_state_id: candidate.to_state_id,
        advisory_next_stage_hint: Some("state_3_proposal_drafted".to_string()),
        advisory_next_action: Some("revise_proposal".to_string()),
        advisory_hint_hash: "sha256:advisory".to_string(),
        advisory_hint_provenance: vec![advisory_hint],
        graph_membership_result: "absent_from_graph".to_string(),
        created_at: now,
    };
    let value = serde_json::to_value(&rejection).unwrap();
    assert_eq!(
        value["advisory_hint_provenance"][0]["included_in_candidate_transition_hash"],
        Value::Bool(true)
    );
    assert_eq!(
        value["selected_next_state_id"],
        Value::String("state_5_proposal_refined".to_string())
    );
}

#[test]
fn proposal_017_proposal_review_summary_field_authority_is_deterministic() {
    let table = proposal_review_summary_v1_authority_table();
    assert_eq!(table.len(), 8);
    assert_eq!(
        proposal_review_summary_v1_field_authority("pass"),
        Some(AggregateFieldAuthority::TransitionAuthoritative)
    );
    assert_eq!(
        proposal_review_summary_v1_field_authority("blocker_count"),
        Some(AggregateFieldAuthority::TransitionAuthoritative)
    );
    assert_eq!(
        proposal_review_summary_v1_field_authority("blocking_issues"),
        Some(AggregateFieldAuthority::TransitionAuthoritative)
    );
    assert_eq!(
        proposal_review_summary_v1_field_authority("required_changes"),
        Some(AggregateFieldAuthority::TransitionAuthoritative)
    );
    assert_eq!(
        proposal_review_summary_v1_field_authority("decision"),
        Some(AggregateFieldAuthority::ContradictionBearing)
    );
    assert_eq!(
        proposal_review_summary_v1_field_authority("next_action"),
        Some(AggregateFieldAuthority::AdvisoryOnly)
    );
    assert_eq!(
        proposal_review_summary_v1_field_authority("next_stage"),
        Some(AggregateFieldAuthority::AdvisoryOnly)
    );
    assert_eq!(
        proposal_review_summary_v1_field_authority("summary"),
        Some(AggregateFieldAuthority::NonAuthoritative)
    );
    assert_eq!(proposal_review_summary_v1_field_authority("score"), None);
}

#[test]
fn proposal_017_proposal_review_summary_v2_field_authority_is_deterministic() {
    let table = proposal_review_summary_v2_authority_table();
    assert_eq!(table.len(), 8);
    assert_eq!(
        proposal_review_summary_v2_field_authority("pass"),
        Some(AggregateFieldAuthority::TransitionAuthoritative)
    );
    assert_eq!(
        proposal_review_summary_v2_field_authority("blocker_count"),
        Some(AggregateFieldAuthority::TransitionAuthoritative)
    );
    assert_eq!(
        proposal_review_summary_v2_field_authority("blocking_issues"),
        Some(AggregateFieldAuthority::TransitionAuthoritative)
    );
    assert_eq!(
        proposal_review_summary_v2_field_authority("blocking_required_changes"),
        Some(AggregateFieldAuthority::TransitionAuthoritative)
    );
    assert_eq!(
        proposal_review_summary_v2_field_authority("advisory_follow_ups"),
        Some(AggregateFieldAuthority::AdvisoryOnly)
    );
    assert_eq!(
        proposal_review_summary_v2_field_authority("decision"),
        Some(AggregateFieldAuthority::ContradictionBearing)
    );
    assert_eq!(
        proposal_review_summary_v2_field_authority("summary"),
        Some(AggregateFieldAuthority::NonAuthoritative)
    );
    assert_eq!(
        proposal_review_summary_v2_field_authority("required_changes"),
        None
    );
}

#[test]
fn proposal_017_conflict_reason_classification_matches_candidate_results() {
    let candidate =
        |transition_id: &str, result: CandidateTransitionResult| CandidateTransitionEvaluation {
            transition_id: transition_id.to_string(),
            from_state_id: "state_4_proposal_reviewed".to_string(),
            to_state_id: "state_5_proposal_refined".to_string(),
            condition_expression_id: Some(format!("{transition_id}.condition")),
            result,
            required_artifacts: vec!["proposal_review_summary".to_string()],
            missing_artifacts: vec![],
            missing_fields: vec![],
            source_artifact_ids: vec![],
            source_agent_execution_id: None,
            sanitized_diagnostic: None,
        };

    assert_eq!(
        classify_workflow_conflict_reason(&[candidate(
            "failed_review_refine",
            CandidateTransitionResult::Matched,
        )]),
        None,
        "one matched graph transition is not a blocking conflict"
    );
    assert_eq!(
        classify_workflow_conflict_reason(&[
            candidate("failed_review_refine", CandidateTransitionResult::Matched),
            candidate("failed_review_retry", CandidateTransitionResult::Matched),
        ]),
        Some(WorkflowConflictReason::MultipleDeclarativeTransitionsMatchedWithoutTieBreak)
    );
    assert_eq!(
        classify_workflow_conflict_reason(&[
            candidate(
                "failed_review_refine",
                CandidateTransitionResult::MissingInput
            ),
            candidate(
                "approved_implementation",
                CandidateTransitionResult::NotMatched
            ),
        ]),
        Some(WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition)
    );
    assert_eq!(
        classify_workflow_conflict_reason(&[
            candidate(
                "failed_review_refine",
                CandidateTransitionResult::MissingInput
            ),
            candidate(
                "approved_implementation",
                CandidateTransitionResult::InvalidExpression
            ),
        ]),
        Some(WorkflowConflictReason::WorkflowConflictUnverifiable),
        "invalid expressions dominate missing input because graph truth is unverifiable"
    );
    assert_eq!(
        classify_workflow_conflict_reason(&[
            candidate(
                "failed_review_refine",
                CandidateTransitionResult::NotMatched
            ),
            candidate(
                "approved_implementation",
                CandidateTransitionResult::NotMatched
            ),
        ]),
        Some(WorkflowConflictReason::NoDeclarativeTransitionMatched)
    );
}

#[test]
fn proposal_017_conflict_hashes_are_stable_and_fingerprint_sorts_advisory_refs() {
    let candidates = vec![CandidateTransitionEvaluation {
        transition_id: "failed_review_refine".to_string(),
        from_state_id: "state_4_proposal_reviewed".to_string(),
        to_state_id: "state_5_proposal_refined".to_string(),
        condition_expression_id: Some("proposal_review_summary.pass_false".to_string()),
        result: CandidateTransitionResult::MissingInput,
        required_artifacts: vec!["proposal_review_summary".to_string()],
        missing_artifacts: vec!["proposal_review_summary".to_string()],
        missing_fields: vec![],
        source_artifact_ids: vec![],
        source_agent_execution_id: None,
        sanitized_diagnostic: Some("Declared artifact proposal_review_summary is absent".into()),
    }];
    let first_hash = candidate_transition_hash(&candidates);
    let second_hash = candidate_transition_hash(&candidates);
    assert!(first_hash.starts_with("sha256:"));
    assert_eq!(first_hash, second_hash);

    let refs_a = vec![
        "artifact_contract_advisories.next_stage".to_string(),
        "run_state_projection.next_action".to_string(),
    ];
    let refs_b = vec![
        "run_state_projection.next_action".to_string(),
        "artifact_contract_advisories.next_stage".to_string(),
    ];
    let fingerprint_a = workflow_conflict_fingerprint(
        "run-1",
        "state_4_proposal_reviewed",
        &WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition,
        &first_hash,
        &refs_a,
    );
    let fingerprint_b = workflow_conflict_fingerprint(
        "run-1",
        "state_4_proposal_reviewed",
        &WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition,
        &first_hash,
        &refs_b,
    );
    assert!(fingerprint_a.starts_with("sha256:"));
    assert_eq!(
        fingerprint_a, fingerprint_b,
        "fingerprint should be stable when advisory evidence is discovered in a different order"
    );
}

#[test]
fn proposal_017_phase0_fixture_inventory_names_required_groups() {
    let i: Value = serde_json::from_str(FIXTURE_INVENTORY).unwrap();
    assert_eq!(
        i["schema_version"],
        "proposal_017_phase0_fixture_inventory_v1"
    );

    let b = i["blocking_conflict_fixtures"].as_array().unwrap();
    for r in [
        "no_declarative_transition_matched",
        "multiple_declarative_transitions_matched_without_tie_break",
        "required_artifact_or_field_missing_for_transition",
        "aggregate_transition_truth_conflicted",
        "workflow_conflict_unverifiable",
        "implementation_handoff_unavailable",
    ] {
        assert!(b.iter().any(|f| f["reason"] == r));
        assert!(WorkflowConflictReason::from_str(r).is_ok());
    }

    let a = i["advisory_rejection_fixtures"].as_array().unwrap();
    assert!(a.iter().any(|f| {
        f["name"] == "d4f404b7_legal_refinement_rejects_absent_next_stage_advisory"
            && f["expected_current_conflict"] == false
            && f["expected_history_event"] == "non_blocking_advisory_rejection"
    }));

    let s = i["report_surface_fixtures"].as_array().unwrap();
    for e in [
        [
            "swift_report_json",
            "workflowConflict",
            "camelCase",
            "snake_case",
        ],
        [
            "mcp_reports_get",
            "workflow_conflict",
            "snake_case",
            "snake_case",
        ],
        [
            "graphql",
            "workflowConflict",
            "camelCase",
            "SCREAMING_SNAKE_CASE",
        ],
        [
            "latest_summary",
            "workflowConflict",
            "camelCase",
            "snake_case",
        ],
    ] {
        let f = s.iter().find(|f| f["surface"] == e[0]).unwrap();
        assert_eq!(f["object_key"], e[1]);
        assert_eq!(f["field_casing"], e[2]);
        assert_eq!(f["enum_casing"], e[3]);
    }

    let c = i["transition_cursor_resume_fixtures"].as_array().unwrap();
    for f in [
        "d4f404b7_legal_refinement_cursor_settled",
        "no_match_blocking_conflict_cursor_awaits_resolution",
        "lead_resolved_continuation_reenters_graph_settlement",
        "terminal_unverifiable_cursor_terminal_failure_reason",
        "restart_readback_prefers_cursor_and_workflow_conflict",
    ] {
        assert!(c.iter().any(|e| e == f));
    }

    let u = i["unknown_transition_input_fixtures"].as_array().unwrap();
    for e in [
        [
            "exists_unknown_artifact_invalid_expression",
            "invalid_expression",
        ],
        ["declared_absent_artifact_missing_input", "missing_input"],
    ] {
        assert!(u.iter().any(|f| {
            f["name"] == e[0] && f["expected_result"] == e[1] && f["expected_match"] == false
        }));
    }
}
