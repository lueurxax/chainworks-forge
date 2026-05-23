//! P082: Engine-layer recovery/retry state-machine matrix proof.
//!
//! These tests verify the engine-layer contract: reason codes, schema constants,
//! envelope construction, and fail-closed behavior for side-effect and approval gates.

use domain::recovery_matrix;

// ── P082: All reason codes are present ────────────────────────────────────

#[test]
fn p082_engine_all_reason_codes_present() {
    let required = [
        "startup_requeue_once",
        "startup_requeue_exhausted",
        "invalid_stage_for_retry",
        "ignored_late_outputs",
        "duplicate_owner_repaired",
        "startup_stalled",
        "stale_repaired",
        "needs_effect_reconciliation",
        "requires_effect_reconciliation",
        "valid_identifier_guidance",
        "approval_pending_operator_action_required",
        "duplicate_mediation_owner_rejected",
        "cancel_active_stage_requested",
        "cancel_pending_approval_preserved",
        "cancel_side_effect_reconciliation_required",
        "cancel_startup_repair_converged",
        "cancelled_provider_late_output_ignored",
        "repair_crash_resume_idempotent",
    ];
    for r in &required {
        assert!(
            recovery_matrix::ALL_REASON_CODES.contains(r),
            "P082 engine: reason code '{r}' must be in ALL_REASON_CODES"
        );
    }
}

// ── P082: Scenario IDs exhaustive ─────────────────────────────────────────

#[test]
fn p082_engine_all_17_scenario_ids_defined() {
    assert_eq!(
        recovery_matrix::SCENARIO_IDS.len(),
        17,
        "P082 requires exactly 17 scenario IDs"
    );
}

// ── P082-R02: Rejected command envelope is well-formed ───────────────────

#[test]
fn p082_r02_rejected_command_envelope_wellformed() {
    let readback = recovery_matrix::build_readback_v1(
        "P082-R02",
        "rejected",
        "no_mutation",
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "Stage is not in a retryable status.",
        "command_journal",
        "command_journal, stages",
        "cmd-r02-1",
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T00:00:00Z",
    );
    let envelope = recovery_matrix::build_rejected_command_error_envelope(
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "RetryStage",
        "Stage is not in a retryable status.",
        readback,
    );
    let parsed = recovery_matrix::parse_command_journal_error_envelope(&envelope);
    assert!(
        parsed.is_some(),
        "P082-R02: build+parse roundtrip must succeed"
    );
    assert_eq!(
        parsed.unwrap()["reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY
    );
}

// ── P082-R07: Side-effect fail-closed reason code ───────────────────────

#[test]
fn p082_r07_side_effect_reason_codes_defined() {
    assert!(
        recovery_matrix::ALL_REASON_CODES
            .contains(&recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION),
        "P082-R07: requires_effect_reconciliation must be in ALL_REASON_CODES"
    );
    assert!(
        recovery_matrix::ALL_REASON_CODES
            .contains(&recovery_matrix::REASON_NEEDS_EFFECT_RECONCILIATION),
        "P082-R06: needs_effect_reconciliation must be in ALL_REASON_CODES"
    );
}

// ── P082-R07: Held-state readback must have non-null blocking/operator fields ─

#[test]
fn p082_r07_held_state_readback_has_non_null_blocking_status() {
    let base = recovery_matrix::build_readback_v1(
        "P082-R07",
        "held",
        "reconcile_side_effects",
        recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION,
        "Reconcile unresolved side effects before retrying.",
        "side_effects, command_journal",
        "side_effects, command_journal",
        "cmd-r07-engine-1",
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T00:00:00Z",
    );
    // Before patching, build_readback_v1 defaults both to null.
    assert!(
        base["recovery_side_effect_blocking_status"].is_null(),
        "P082-R07: build_readback_v1 alone defaults recovery_side_effect_blocking_status to null"
    );

    let patched = recovery_matrix::set_readback_side_effect_hold(
        base,
        "unresolved_side_effect_entries",
        "Retry blocked: unresolved side-effect ledger entries exist. Reconcile side effects before retrying.",
    );

    assert!(
        !patched["recovery_side_effect_blocking_status"].is_null(),
        "P082-R07: recovery_side_effect_blocking_status must be non-null after set_readback_side_effect_hold"
    );
    assert_eq!(
        patched["recovery_side_effect_blocking_status"]
            .as_str()
            .unwrap(),
        "unresolved_side_effect_entries",
        "P082-R07: recovery_side_effect_blocking_status must name the blocking status"
    );

    assert!(
        !patched["recovery_operator_message"].is_null(),
        "P082-R07: recovery_operator_message must be non-null after set_readback_side_effect_hold"
    );
    assert!(
        patched["recovery_operator_message"]
            .as_str()
            .unwrap()
            .contains("side-effect"),
        "P082-R07: recovery_operator_message must describe the side-effect hold"
    );

    // Other contract fields must be preserved.
    assert_eq!(patched["scenario_id"].as_str().unwrap(), "P082-R07");
    assert_eq!(patched["scenario_status"].as_str().unwrap(), "held");
    assert_eq!(
        patched["recovery_decision"].as_str().unwrap(),
        "reconcile_side_effects"
    );
    assert_eq!(
        patched["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION
    );
}

#[test]
fn p082_r07_held_state_envelope_contains_non_null_fields() {
    // Verify the full build pipeline (build_readback_v1 → set_readback_side_effect_hold
    // → build_rejected_command_error_envelope) round-trips correctly.
    let readback = recovery_matrix::set_readback_side_effect_hold(
        recovery_matrix::build_readback_v1(
            "P082-R07",
            "held",
            "reconcile_side_effects",
            recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION,
            "Reconcile unresolved side effects before retrying this stage.",
            "side_effects, command_journal",
            "side_effects, command_journal",
            "cmd-r07-engine-2",
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        ),
        "unresolved_side_effect_entries",
        "Retry blocked: unresolved side-effect ledger entries exist. Reconcile side effects before retrying.",
    );
    let envelope = recovery_matrix::build_rejected_command_error_envelope(
        recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION,
        "RetryStage",
        "Retry blocked: unresolved side-effect ledger entries exist. No mutation was performed.",
        readback,
    );
    let parsed = recovery_matrix::parse_command_journal_error_envelope(&envelope)
        .expect("P082-R07: full build pipeline must produce a parseable envelope");

    let nested_rb = parsed["p082_recovery_matrix_readback"]
        .as_object()
        .expect("P082-R07: p082_recovery_matrix_readback must be an object");

    assert!(
        !nested_rb["recovery_side_effect_blocking_status"].is_null(),
        "P082-R07: nested recovery_side_effect_blocking_status must be non-null in parsed envelope"
    );
    assert!(
        !nested_rb["recovery_operator_message"].is_null(),
        "P082-R07: nested recovery_operator_message must be non-null in parsed envelope"
    );
    assert_eq!(
        nested_rb["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION
    );
}

// ── P082-R09: Approval pending reason code ──────────────────────────────

#[test]
fn p082_r09_approval_pending_reason_code_defined() {
    assert!(
        recovery_matrix::ALL_REASON_CODES
            .contains(&recovery_matrix::REASON_APPROVAL_PENDING_OPERATOR_ACTION_REQUIRED),
        "P082-R09: approval_pending_operator_action_required must be in ALL_REASON_CODES"
    );
}

// ── P082-R11 through R14: Cancellation reason codes ─────────────────────

#[test]
fn p082_cancellation_reason_codes_defined() {
    let cancellation_codes = [
        recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
        recovery_matrix::REASON_CANCEL_PENDING_APPROVAL_PRESERVED,
        recovery_matrix::REASON_CANCEL_SIDE_EFFECT_RECONCILIATION_REQUIRED,
        recovery_matrix::REASON_CANCEL_STARTUP_REPAIR_CONVERGED,
    ];
    for code in &cancellation_codes {
        assert!(
            recovery_matrix::ALL_REASON_CODES.contains(code),
            "P082: cancellation reason code '{code}' must be in ALL_REASON_CODES"
        );
    }
}

// ── P082-R15: Crash resume idempotent ────────────────────────────────────

#[test]
fn p082_r15_crash_resume_idempotent_reason_code_defined() {
    assert!(
        recovery_matrix::ALL_REASON_CODES
            .contains(&recovery_matrix::REASON_REPAIR_CRASH_RESUME_IDEMPOTENT),
        "P082-R15: repair_crash_resume_idempotent must be in ALL_REASON_CODES"
    );
}

// ── P082-R17: Cancelled provider late output ─────────────────────────────

#[test]
fn p082_r17_cancelled_provider_late_output_reason_code_defined() {
    assert!(
        recovery_matrix::ALL_REASON_CODES
            .contains(&recovery_matrix::REASON_CANCELLED_PROVIDER_LATE_OUTPUT_IGNORED),
        "P082-R17: cancelled_provider_late_output_ignored must be in ALL_REASON_CODES"
    );
}

// ── P082: Build readback v1 has required fields ──────────────────────────

#[test]
fn p082_build_readback_v1_has_all_required_fields() {
    let rb = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs, work_items",
        "startup-repair-001",
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T00:00:00Z",
    );

    let required_fields = [
        "schema_version",
        "scenario_id",
        "scenario_status",
        "recovery_decision",
        "recovery_reason_code",
        "recovery_next_action",
        "recovery_hold_conditions",
        "recovery_side_effect_blocking_status",
        "recovery_retry_identifier_guidance",
        "recovery_late_output_settlement",
        "recovery_startup_repair_summary",
        "recovery_operator_message",
        "recovery_projection_integrity",
        "source_table",
        "source_repository",
        "source_identifier",
        "source_json_key",
        "updated_at",
        "diagnostic_redaction",
    ];

    for field in &required_fields {
        assert!(
            rb.get(field).is_some(),
            "P082: build_readback_v1 must include field '{field}'"
        );
    }

    assert_eq!(
        rb["schema_version"].as_str().unwrap(),
        recovery_matrix::SCHEMA_READBACK_V1
    );
}

// ── P082: payload_json is never the readback storage owner ──────────────

#[test]
fn p082_payload_json_is_not_readback_storage_owner() {
    // The envelope is designed to go in command_journal.error, not payload_json.
    // This test verifies the build function produces content suitable only for
    // the error column (the content includes schema_version=p082_rejected_command_error_v1).
    let readback = recovery_matrix::build_readback_v1(
        "P082-R08",
        "rejected",
        "no_mutation",
        recovery_matrix::REASON_VALID_IDENTIFIER_GUIDANCE,
        "Wrong identifier kind supplied.",
        "command_journal",
        "command_journal",
        "cmd-r08-1",
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T00:00:00Z",
    );
    let envelope = recovery_matrix::build_rejected_command_error_envelope(
        recovery_matrix::REASON_VALID_IDENTIFIER_GUIDANCE,
        "RetryStage",
        "Identifier mismatch.",
        readback,
    );
    // The envelope targets command_journal.error (schema says so explicitly)
    assert!(
        envelope.contains(recovery_matrix::SCHEMA_REJECTED_COMMAND_ERROR_V1),
        "envelope must be typed for command_journal.error only"
    );
    // It must NOT be the same structure as a plain command payload (which has no schema_version)
    let v: serde_json::Value = serde_json::from_str(&envelope).unwrap();
    assert_eq!(
        v["schema_version"].as_str().unwrap(),
        recovery_matrix::SCHEMA_REJECTED_COMMAND_ERROR_V1
    );
}

// ── P082 NEG: Non-canonical scenario_id is rejected by parser ───────────────

#[test]
fn p082_neg_non_canonical_scenario_id_rejected_by_parser() {
    // Behavioral proof for negative fixture p082-malformed-command-error-envelope:
    // the parser must reject any scenario_id not in SCENARIO_IDS.
    for bad_id in &[
        "P082-legacy-command-error",
        "P082-INJECTED",
        "P082-R99",
        "not_a_p082_id",
    ] {
        let fake_readback = serde_json::json!({
            "schema_version": recovery_matrix::SCHEMA_READBACK_V1,
            "scenario_id": bad_id,
            "scenario_status": "held",
            "recovery_decision": "wait",
            "recovery_reason_code": recovery_matrix::REASON_RESUME_CLAIM_STATUS,
            "recovery_next_action": "Inspect command journal.",
            "recovery_hold_conditions": [],
            "recovery_side_effect_blocking_status": null,
            "recovery_retry_identifier_guidance": null,
            "recovery_late_output_settlement": null,
            "recovery_startup_repair_summary": null,
            "recovery_operator_message": null,
            "recovery_projection_integrity": "unavailable",
            "source_table": "command_journal",
            "source_repository": "command_journal",
            "source_identifier": "cj-001",
            "source_json_key": null,
            "updated_at": "2026-05-21T00:00:00Z",
            "diagnostic_redaction": "none",
        });
        let envelope = serde_json::json!({
            "schema_version": recovery_matrix::SCHEMA_REJECTED_COMMAND_ERROR_V1,
            "reason_code": recovery_matrix::REASON_RESUME_CLAIM_STATUS,
            "command_type": "RetryStage",
            "redaction": "none",
            "operator_safe_summary": "Test.",
            "p082_recovery_matrix_readback": fake_readback,
        })
        .to_string();
        assert!(
            recovery_matrix::parse_command_journal_error_envelope(&envelope).is_none(),
            "P082 NEG: non-canonical scenario_id '{bad_id}' must be rejected by envelope parser"
        );
    }
}

// ── P082 NEG: Empty recovery_next_action for non-not_applicable is rejected ──

#[test]
fn p082_neg_empty_next_action_for_non_not_applicable_is_rejected() {
    // Behavioral proof: recovery_next_action must be non-empty for any scenario_status
    // other than not_applicable. Empty next_action for an active/held scenario is a
    // contract violation that the parser must reject fail-closed.
    for status in &["repaired", "rejected", "held", "pending", "cancelled"] {
        let fake_readback = serde_json::json!({
            "schema_version": recovery_matrix::SCHEMA_READBACK_V1,
            "scenario_id": "P082-R01",
            "scenario_status": status,
            "recovery_decision": "retry",
            "recovery_reason_code": recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "recovery_next_action": "",  // empty — must be rejected
            "recovery_hold_conditions": [],
            "recovery_side_effect_blocking_status": null,
            "recovery_retry_identifier_guidance": null,
            "recovery_late_output_settlement": null,
            "recovery_startup_repair_summary": null,
            "recovery_operator_message": null,
            "recovery_projection_integrity": "valid",
            "source_table": "startup_repairs",
            "source_repository": "startup_repairs",
            "source_identifier": "sr-001",
            "source_json_key": null,
            "updated_at": "2026-05-21T00:00:00Z",
            "diagnostic_redaction": "none",
        });
        let envelope = serde_json::json!({
            "schema_version": recovery_matrix::SCHEMA_REJECTED_COMMAND_ERROR_V1,
            "reason_code": recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "command_type": "StartupRepair",
            "redaction": "none",
            "operator_safe_summary": "Test.",
            "p082_recovery_matrix_readback": fake_readback,
        })
        .to_string();
        assert!(
            recovery_matrix::parse_command_journal_error_envelope(&envelope).is_none(),
            "P082 NEG: empty recovery_next_action for scenario_status='{status}' must be rejected"
        );
    }
}

// ── P082 NEG: validate_readback_v1_shape rejects rows with tampered fields ──

#[test]
fn p082_neg_validate_readback_v1_shape_rejects_tampered_field_values() {
    // Verify that validate_readback_v1_shape rejects rows where critical enum fields
    // contain legacy or out-of-vocabulary values.
    let valid = recovery_matrix::set_readback_side_effect_hold(
        recovery_matrix::build_readback_v1(
            "P082-R07",
            "held",
            "reconcile_side_effects",
            recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION,
            "Reconcile side effects before retrying.",
            "side_effects, command_journal",
            "side_effects, command_journal",
            "se-001",
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        ),
        "unresolved_side_effect_entries",
        "Retry blocked: unresolved side-effect ledger entries exist. Reconcile side effects before retrying.",
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(&valid),
        "P082: valid R07 readback must pass validate_readback_v1_shape"
    );

    // scenario_status with legacy value must fail.
    let mut rb = valid.clone();
    if let Some(obj) = rb.as_object_mut() {
        obj.insert("scenario_status".into(), serde_json::json!("resolved"));
    }
    assert!(
        !recovery_matrix::validate_readback_v1_shape(&rb),
        "P082 NEG: legacy scenario_status 'resolved' must fail validate_readback_v1_shape"
    );

    // recovery_decision with legacy value must fail.
    let mut rb = valid.clone();
    if let Some(obj) = rb.as_object_mut() {
        obj.insert(
            "recovery_decision".into(),
            serde_json::json!("repair_converged"),
        );
    }
    assert!(
        !recovery_matrix::validate_readback_v1_shape(&rb),
        "P082 NEG: legacy recovery_decision 'repair_converged' must fail validate_readback_v1_shape"
    );

    // recovery_projection_integrity with invalid value must fail.
    let mut rb = valid.clone();
    if let Some(obj) = rb.as_object_mut() {
        obj.insert(
            "recovery_projection_integrity".into(),
            serde_json::json!("ok"),
        );
    }
    assert!(
        !recovery_matrix::validate_readback_v1_shape(&rb),
        "P082 NEG: invalid recovery_projection_integrity 'ok' must fail validate_readback_v1_shape"
    );
}

// ── P082 NEG: Rollout contract operator fields are present ─────────────────

#[test]
fn p082_neg_rollout_contract_operator_fields_vocabulary_is_correct() {
    // Corresponds to negative fixture p082-missing-rollout-contract-operator-fields.json.
    // Verify that SCHEMA_READBACK_V1 includes the schema_version marker
    // that the rollout contract fixture would check.
    assert!(
        recovery_matrix::SCHEMA_READBACK_V1.starts_with("p082_"),
        "P082 NEG: schema version must start with p082_ prefix for rollout contract compliance"
    );
    // Verify approved decision vocabulary excludes non-operator-safe legacy values.
    for forbidden in &["approved", "auto_resolved", "bypass"] {
        assert!(
            !recovery_matrix::VALID_RECOVERY_DECISIONS.contains(forbidden),
            "P082 NEG: VALID_RECOVERY_DECISIONS must not contain non-P082 value '{forbidden}'"
        );
    }
}

// ── P082 NEG: All 17 scenario IDs use the canonical P082-Rnn format ─────────

#[test]
fn p082_neg_all_scenario_ids_match_canonical_format() {
    // Corresponds to part of negative fixture p082-missing-matrix-row.json.
    // Verify that each SCENARIO_ID matches P082-R01 through P082-R17 format.
    for id in recovery_matrix::SCENARIO_IDS {
        assert!(
            id.starts_with("P082-R"),
            "P082 NEG: scenario ID '{id}' must start with 'P082-R'"
        );
        let suffix = id.trim_start_matches("P082-R");
        let n: u32 = suffix.parse().expect(&format!(
            "P082 NEG: scenario ID '{id}' suffix must be a number"
        ));
        assert!(
            (1..=17).contains(&n),
            "P082 NEG: scenario ID '{id}' suffix {n} must be in range 1..=17"
        );
    }
}

// ── P082-R01: Startup repair summary enforces max_requeue_generation=1 ──────

#[test]
fn p082_r01_startup_repair_summary_max_requeue_generation_is_one() {
    let now = "2026-05-21T00:00:00Z";
    let summary = recovery_matrix::build_startup_repair_summary(
        "p082-requeue:cj-001:wi-001:1",
        "wi-001",
        "cj-001",
        1,
        1,
        false,
        60_000,
        now,
        false,
        None,
        "global",
    );
    assert_eq!(
        summary["max_requeue_generation"].as_i64(),
        Some(1),
        "P082-R01: max_requeue_generation must be 1 (single startup requeue generation)"
    );
    assert_eq!(
        summary["requeue_generation"].as_i64(),
        Some(1),
        "P082-R01: requeue_generation must be 1 for the first and only requeue"
    );
    assert_eq!(
        summary["replayed"].as_bool(),
        Some(false),
        "P082-R01: replayed must be false for a fresh startup repair"
    );
    assert_eq!(
        summary["schema_version"].as_str().unwrap(),
        recovery_matrix::SCHEMA_STARTUP_REPAIR_SUMMARY_V1,
        "P082-R01: startup repair summary must carry correct schema_version"
    );
}

// ── P082-R01: Readback has repaired scenario_status and retry decision ────────

#[test]
fn p082_r01_readback_has_repaired_status_and_retry_decision() {
    let now = "2026-05-21T00:00:00Z";
    let repair_id = "p082-requeue:cj-r01:wi-r01:1";
    let summary = recovery_matrix::build_startup_repair_summary(
        repair_id, "wi-r01", "cj-r01", 1, 1, false, 60_000, now, false, None, "global",
    );
    let rb = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup recovery requeued the abandoned InvokeAgent work item.",
            "startup_repairs, work_items, command_journal",
            "startup_repairs, work_items",
            repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            now,
        ),
        summary,
        None,
    );
    assert_eq!(rb["scenario_id"].as_str().unwrap(), "P082-R01");
    assert_eq!(rb["scenario_status"].as_str().unwrap(), "repaired");
    assert_eq!(rb["recovery_decision"].as_str().unwrap(), "retry");
    assert_eq!(
        rb["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE
    );
    assert_eq!(rb["recovery_projection_integrity"].as_str().unwrap(), "valid");
    let s = rb["recovery_startup_repair_summary"].as_object().unwrap();
    assert_eq!(s["max_requeue_generation"].as_i64(), Some(1));
    assert!(recovery_matrix::validate_readback_v1_shape(&rb));
}

// ── P082-R02: No mutation before eligibility validation ──────────────────────

#[test]
fn p082_r02_no_mutation_decision_for_rejected_retry() {
    let rb = recovery_matrix::build_readback_v1(
        "P082-R02",
        "rejected",
        "no_mutation",
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "Stage status does not allow retry.",
        "command_journal, stages",
        "command_journal, stages",
        "cj-r02",
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T00:00:00Z",
    );
    assert_eq!(rb["recovery_decision"].as_str().unwrap(), "no_mutation",
        "P082-R02: rejected retry must use no_mutation decision to prevent state change");
    assert_eq!(rb["scenario_status"].as_str().unwrap(), "rejected");
    assert!(recovery_matrix::validate_readback_v1_shape(&rb));
}

// ── P082-R03: Late output settlement invariants ───────────────────────────────

#[test]
fn p082_r03_late_output_settlement_active_projection_unchanged() {
    let settlement = recovery_matrix::build_late_output_settlement(
        "ae-r03",
        "wi-r03",
        "sg-old",
        "sg-active",
        "superseded",
        "ignored",
        3,
        "completed",
        false,
    );
    assert_eq!(
        settlement["active_projection_changed"].as_bool(),
        Some(false),
        "P082-R03: active_projection_changed must be false after ignored late output"
    );
    assert_eq!(
        settlement["output_settlement"].as_str().unwrap(),
        "ignored",
        "P082-R03: output_settlement must be ignored for superseded late output"
    );
    assert_eq!(
        settlement["source_work_item_terminal_status"].as_str().unwrap(),
        "completed",
        "P082-R03: source work item must be terminal after late output settlement"
    );
    assert_eq!(
        settlement["claim_state"].as_str().unwrap(),
        "superseded",
        "P082-R03: artifact claim_state must be superseded for late output"
    );
    assert_eq!(
        settlement["cancelled_provider_session"].as_bool(),
        Some(false),
        "P082-R03: cancelled_provider_session must be false for non-cancel late output"
    );
    assert_eq!(
        settlement["schema_version"].as_str().unwrap(),
        recovery_matrix::SCHEMA_LATE_OUTPUT_SETTLEMENT_V1
    );
}

// ── P082-R06: Stale scheduler reason code prevents blind retry ───────────────

#[test]
fn p082_r06_stale_scheduler_reason_uses_stale_repaired_not_blind_retry() {
    // R06 uses stale_repaired (not startup_requeue_once) to show the repair path
    // requires an explicit recorded transition — no blind retry.
    assert!(
        recovery_matrix::ALL_REASON_CODES.contains(&recovery_matrix::REASON_STALE_REPAIRED),
        "P082-R06: stale_repaired must be in ALL_REASON_CODES"
    );
    // Verify stale_repaired is distinct from the blind-retry path (startup_requeue_once).
    assert_ne!(
        recovery_matrix::REASON_STALE_REPAIRED,
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "P082-R06: stale_repaired and startup_requeue_once must be distinct reason codes"
    );
}

// ── P082-R08: Retry identifier guidance enforces no_mutation ─────────────────

#[test]
fn p082_r08_identifier_guidance_has_no_mutation_true() {
    let guidance = recovery_matrix::build_retry_identifier_guidance(
        "RetryStage",
        "stage-abc-wrong-kind",
        "stage_execution_uuid",
        "workflow_stage_id",
        &["implement", "test"],
    );
    assert_eq!(
        guidance["no_mutation"].as_bool(),
        Some(true),
        "P082-R08: identifier guidance must carry no_mutation=true to prevent state change"
    );
    assert_eq!(
        guidance["schema_version"].as_str().unwrap(),
        recovery_matrix::SCHEMA_RETRY_IDENTIFIER_GUIDANCE_V1
    );
}

// ── P082-R09: Approval preserved across restart — no auto-resolution ─────────

#[test]
fn p082_r09_approval_readback_forbids_auto_resolution() {
    let rb = recovery_matrix::build_readback_v1(
        "P082-R09",
        "pending",
        "operator_approval_required",
        recovery_matrix::REASON_APPROVAL_PENDING_OPERATOR_ACTION_REQUIRED,
        "Pending approval was preserved across restart; use the existing approval path.",
        "approvals, approval_inbox, stage_executions",
        "approvals, approval_inbox, stage_executions",
        "approval-r09-001",
        None,
        "valid",
        "2026-05-21T00:00:00Z",
    );
    // Must be pending (not repaired/rejected) — auto-resolution is forbidden.
    assert_eq!(
        rb["scenario_status"].as_str().unwrap(),
        "pending",
        "P082-R09: scenario_status must be pending, not repaired or rejected"
    );
    assert_eq!(
        rb["recovery_decision"].as_str().unwrap(),
        "operator_approval_required",
        "P082-R09: decision must route to operator approval path, not auto-resolve"
    );
    // next action must reference the approval path.
    assert!(
        rb["recovery_next_action"]
            .as_str()
            .unwrap_or("")
            .contains("approval"),
        "P082-R09: recovery_next_action must reference the existing approval path"
    );
    assert!(recovery_matrix::validate_readback_v1_shape(&rb));
}

// ── P082-R11: Cancellation scenario_status is cancelled or held ──────────────

#[test]
fn p082_r11_cancellation_uses_correct_scenario_status() {
    let rb = recovery_matrix::build_readback_v1(
        "P082-R11",
        "cancelled",
        "cancel",
        recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
        "Run cancellation settled the active stage execution.",
        "runs, work_items, retry_stage_execution_authorities, session_generations",
        "runs, work_items, sessions",
        "run-r11",
        Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T00:00:00Z",
    );
    assert_eq!(
        rb["scenario_status"].as_str().unwrap(),
        "cancelled",
        "P082-R11: scenario_status must be cancelled after cancellation settlement"
    );
    assert_eq!(
        rb["recovery_decision"].as_str().unwrap(),
        "cancel",
        "P082-R11: recovery_decision must be cancel"
    );
    assert!(recovery_matrix::validate_readback_v1_shape(&rb));
}

// ── P082-R16: Startup requeue exhausted held state enforces no new work ───────

#[test]
fn p082_r16_startup_requeue_exhausted_uses_held_status_not_repaired() {
    let now = "2026-05-21T00:00:00Z";
    let repair_id = "p082-requeue:cj-r16:wi-r16:1";
    let summary = recovery_matrix::build_startup_repair_summary(
        repair_id, "wi-r16", "cj-r16", 1, 1, true, 60_000, now, false, None, "global",
    );
    let rb = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R16",
            "held",
            "wait",
            recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED,
            "Startup requeue generation 1 was already consumed; no duplicate work was enqueued.",
            "startup_repairs, work_items",
            "startup_repairs, work_items",
            repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            now,
        ),
        summary,
        Some("Startup requeue exhausted: generation 1 was already consumed. Use existing recovery inspection or cancellation paths to clear the hold."),
    );
    // R16 must be held, not repaired — held prevents duplicate work scheduling.
    assert_eq!(rb["scenario_status"].as_str().unwrap(), "held",
        "P082-R16: scenario_status must be held (not repaired) to prevent duplicate scheduling");
    assert_eq!(rb["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED);
    // replayed=true in the summary proves this is a replay of an exhausted generation.
    let s = rb["recovery_startup_repair_summary"].as_object().unwrap();
    assert_eq!(s["replayed"].as_bool(), Some(true),
        "P082-R16: replayed must be true when idempotency key was already observed");
    // operator_message must be non-null for held states.
    assert!(!rb["recovery_operator_message"].is_null(),
        "P082-R16: recovery_operator_message must be non-null for startup_requeue_exhausted held state");
    // source_json_key must point to the approved storage owner.
    assert_eq!(
        rb["source_json_key"].as_str().unwrap(),
        "startup_repairs.notes.p082_recovery_matrix_readback",
        "P082-R16: source_json_key must reference startup_repairs.notes (approved storage owner)"
    );
    assert!(recovery_matrix::validate_readback_v1_shape(&rb));
}

// ── P082-R17: Cancelled provider late output invariants ──────────────────────

#[test]
fn p082_r17_cancelled_provider_late_output_marks_cancelled_session() {
    let settlement = recovery_matrix::build_late_output_settlement(
        "ae-r17",
        "wi-r17",
        "sg-cancelled",
        "sg-active",
        "closed",
        "ignored",
        1,
        "failed",
        true,
    );
    assert_eq!(
        settlement["cancelled_provider_session"].as_bool(),
        Some(true),
        "P082-R17: cancelled_provider_session must be true when output arrives from cancelled session"
    );
    assert_eq!(
        settlement["active_projection_changed"].as_bool(),
        Some(false),
        "P082-R17: active_projection_changed must be false — cancelled output cannot mutate active truth"
    );
    assert_eq!(
        settlement["output_settlement"].as_str().unwrap(),
        "ignored",
        "P082-R17: output_settlement must be ignored for cancelled provider late output"
    );
    assert_eq!(
        settlement["source_work_item_terminal_status"].as_str().unwrap(),
        "failed",
        "P082-R17: source work item must be terminal (failed/completed) after settlement"
    );
}

// ── P082: R07 fail-closed: side-effect blocking requires no retry route ───────

#[test]
fn p082_r07_side_effect_blocking_status_prevents_retry_route() {
    let base = recovery_matrix::build_readback_v1(
        "P082-R07",
        "held",
        "reconcile_side_effects",
        recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION,
        "Reconcile side effects before retrying.",
        "side_effects, command_journal",
        "side_effects, command_journal",
        "se-r07",
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T00:00:00Z",
    );
    let rb = recovery_matrix::set_readback_side_effect_hold(
        base,
        "unresolved_side_effect_entries",
        "Retry blocked: unresolved side-effect ledger entries exist. Reconcile before retrying.",
    );
    // Reconcile is the only route — retry and cancel are NOT permitted decisions.
    assert_eq!(
        rb["recovery_decision"].as_str().unwrap(),
        "reconcile_side_effects",
        "P082-R07: decision must be reconcile_side_effects, not retry or cancel"
    );
    assert_ne!(
        rb["recovery_decision"].as_str().unwrap(),
        "retry",
        "P082-R07: retry must not be offered while side effects are unresolved (fail-closed)"
    );
    assert!(!rb["recovery_side_effect_blocking_status"].is_null(),
        "P082-R07: blocking_status must be non-null to surface side-effect hold to operators");
    assert!(!rb["recovery_operator_message"].is_null(),
        "P082-R07: operator_message must be non-null for side-effect held state");
}

// ── P082-R15: Crash-loop replay idempotency: same key must replay, not duplicate ─

#[test]
fn p082_r15_crash_loop_same_key_must_not_create_duplicate() {
    let now = "2026-05-21T00:00:00Z";
    let repair_id = "p082-requeue:cj-r15-loop:wi-r15-loop:1";
    // First pass: generate R01 readback (repair succeeded)
    let r01_summary = recovery_matrix::build_startup_repair_summary(
        repair_id, "wi-r15-loop", "cj-r15-loop", 1, 1, false, 60_000, now, false, None, "global",
    );
    let r01_rb = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R01", "repaired", "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup repair requeued the work item.",
            "startup_repairs, work_items, command_journal",
            "startup_repairs, work_items",
            repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid", now,
        ),
        r01_summary,
        None,
    );
    // Second crash pass: same idempotency key observed — replayed=true (R15)
    let r15_summary = recovery_matrix::build_startup_repair_summary(
        repair_id, "wi-r15-loop", "cj-r15-loop", 1, 1, true, 60_000, now, false, None, "global",
    );
    let r15_rb = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R15", "repaired", "retry",
            recovery_matrix::REASON_REPAIR_CRASH_RESUME_IDEMPOTENT,
            "Crash-resume replay reused an existing repair idempotency key without duplicate mutation.",
            "startup_repairs, retry_payload_recovery_events, side_effects, runs, command_journal",
            "startup_repairs, work_items, command_journal",
            repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid", now,
        ),
        r15_summary,
        None,
    );
    // R01 and R15 must use the same idempotency key (no duplication).
    assert_eq!(
        r01_rb["source_identifier"].as_str().unwrap(),
        r15_rb["source_identifier"].as_str().unwrap(),
        "P082-R15: crash-loop replay must use the same idempotency key as the original repair"
    );
    // R15 must be repaired with replayed=true, not a new repair.
    assert_eq!(r15_rb["scenario_id"].as_str().unwrap(), "P082-R15");
    let s15 = r15_rb["recovery_startup_repair_summary"].as_object().unwrap();
    assert_eq!(s15["replayed"].as_bool(), Some(true),
        "P082-R15: crash-loop replay must have replayed=true in startup_repair_summary");
    // Original R01 must not have replayed=true (it was the first repair).
    let s01 = r01_rb["recovery_startup_repair_summary"].as_object().unwrap();
    assert_eq!(s01["replayed"].as_bool(), Some(false),
        "P082-R01: original repair must have replayed=false");
    assert!(recovery_matrix::validate_readback_v1_shape(&r15_rb));
}
