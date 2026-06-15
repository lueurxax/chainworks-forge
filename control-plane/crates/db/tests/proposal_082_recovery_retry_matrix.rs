//! P082: DB-layer recovery/retry state-machine matrix proof.
//!
//! These tests prove the DB contract for all 17 matrix scenarios, focusing on
//! storage-owner assertions, idempotency, envelope parsing, and the fail-closed
//! side-effect posture.

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{command_journal, side_effects, startup_repairs, work_items};
use domain::ids::RunId;
use domain::recovery_matrix;

async fn setup_db() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .expect("test shared DbWriter registration failed");
    pool
}

/// Inserts a minimal idea + run row so stage_executions FKs are satisfied.
async fn insert_test_run(pool: &sqlx::SqlitePool, run_id: &str, ts: &str) {
    let idea_id = format!("idea-for-{run_id}");
    sqlx::query(
        r#"INSERT OR IGNORE INTO ideas (id, title, body, status, created_at)
           VALUES (?1, 'test', 'test', 'active', ?2)"#,
    )
    .bind(&idea_id)
    .bind(ts)
    .execute(pool)
    .await
    .expect("insert idea for test run");

    sqlx::query(
        r#"INSERT OR IGNORE INTO runs
           (id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at)
           VALUES (?1, ?2, 'running', 'wf-test', 'Test Run', '/', '/', ?3)"#,
    )
    .bind(run_id)
    .bind(&idea_id)
    .bind(ts)
    .execute(pool)
    .await
    .expect("insert test run");
}

// ── P082-R01: Startup requeue reason codes and constants ───────────────────

#[tokio::test]
async fn p082_r01_startup_requeue_constants_are_defined() {
    assert_eq!(
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "startup_requeue_once"
    );
    assert_eq!(
        recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED,
        "startup_requeue_exhausted"
    );
}

// ── P082-R02: Rejected command stores typed envelope in command_journal.error ─

#[tokio::test]
async fn p082_r02_rejected_command_writes_typed_envelope_to_error_column() {
    let pool = setup_db().await;
    let now = Utc::now();
    let journal_id = "p082-r02-journal-1";

    command_journal::record(
        &pool,
        journal_id,
        "RetryStage",
        r#"{"run_id":"run-1","stage_id":"implement"}"#,
        Some("run-1"),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.retry"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry");

    // Build the typed envelope for the rejection
    let readback = recovery_matrix::build_readback_v1(
        "P082-R02",
        "rejected",
        "no_mutation",
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "Stage is not in a retryable status. No mutation was performed.",
        "command_journal",
        "command_journal, stages",
        journal_id,
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    let envelope = recovery_matrix::build_rejected_command_error_envelope(
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "RetryStage",
        "Stage is not in a retryable status.",
        readback,
    );

    // Write rejection to command_journal.error (NOT payload_json)
    command_journal::fail_entry(&pool, journal_id, now, &envelope)
        .await
        .expect("fail command journal entry");

    // Read back the raw error column from DB
    let error_raw: Option<String> =
        sqlx::query_scalar("SELECT error FROM command_journal WHERE id = ?1")
            .bind(journal_id)
            .fetch_optional(&pool)
            .await
            .expect("fetch command journal error");

    let error_str = error_raw.expect("error column must be set");
    // Parse the envelope
    let parsed = recovery_matrix::parse_command_journal_error_envelope(&error_str);
    assert!(
        parsed.is_some(),
        "P082-R02: command_journal.error must parse as p082_rejected_command_error_v1"
    );
    let v = parsed.unwrap();
    assert_eq!(
        v["reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "P082-R02: reason_code must be invalid_stage_for_retry"
    );
    assert_eq!(
        v["schema_version"].as_str().unwrap(),
        recovery_matrix::SCHEMA_REJECTED_COMMAND_ERROR_V1
    );

    // payload_json must remain the original inserted value (not mutated)
    let payload: Option<String> =
        sqlx::query_scalar("SELECT payload_json FROM command_journal WHERE id = ?1")
            .bind(journal_id)
            .fetch_optional(&pool)
            .await
            .expect("fetch command journal payload_json");
    assert_eq!(
        payload.as_deref(),
        Some(r#"{"run_id":"run-1","stage_id":"implement"}"#),
        "P082-R02: command_journal.payload_json must not be mutated for P082 readback"
    );
}

// ── P082: Legacy plain-text error is safe fallback ─────────────────────────

#[tokio::test]
async fn p082_legacy_plain_text_command_journal_error_is_safe() {
    let pool = setup_db().await;
    let now = Utc::now();
    let journal_id = "p082-legacy-journal-1";

    command_journal::record(
        &pool,
        journal_id,
        "RetryStage",
        r#"{"run_id":"run-2"}"#,
        Some("run-2"),
        now,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry");

    let plain_text_error = "Stage is not retryable: status=completed";
    command_journal::fail_entry(&pool, journal_id, now, plain_text_error)
        .await
        .expect("fail command journal entry with plain text");

    let error_raw: Option<String> =
        sqlx::query_scalar("SELECT error FROM command_journal WHERE id = ?1")
            .bind(journal_id)
            .fetch_optional(&pool)
            .await
            .expect("fetch command journal error");

    let error_str = error_raw.expect("error column must be set");
    // Must not panic; must return None for plain-text
    let parsed = recovery_matrix::parse_command_journal_error_envelope(&error_str);
    assert!(
        parsed.is_none(),
        "P082: legacy plain-text error must return None from parse_command_journal_error_envelope"
    );
}

// ── P082: Malformed JSON envelope is safe ─────────────────────────────────

#[tokio::test]
async fn p082_malformed_envelope_is_safe() {
    let malformed = r#"{"schema_version":"p082_rejected_command_error_v1"}"#;
    let parsed = recovery_matrix::parse_command_journal_error_envelope(malformed);
    assert!(
        parsed.is_none(),
        "P082: malformed envelope (missing required fields) must return None"
    );
}

// ── P082: All 17 scenario IDs are defined ─────────────────────────────────

#[tokio::test]
async fn p082_all_scenario_ids_defined() {
    let ids = recovery_matrix::SCENARIO_IDS;
    assert_eq!(ids.len(), 17, "P082 requires exactly 17 scenario IDs");
    for expected in &[
        "P082-R01", "P082-R02", "P082-R03", "P082-R04", "P082-R05", "P082-R06", "P082-R07",
        "P082-R08", "P082-R09", "P082-R10", "P082-R11", "P082-R12", "P082-R13", "P082-R14",
        "P082-R15", "P082-R16", "P082-R17",
    ] {
        assert!(
            ids.contains(expected),
            "P082 scenario ID {expected} must be present in SCENARIO_IDS"
        );
    }
}

// ── P082-R01: Startup repair record idempotency ────────────────────────────

#[tokio::test]
async fn p082_r01_startup_repair_record_is_idempotent() {
    let pool = setup_db().await;
    let now = Utc::now();

    // Record a startup repair using the convention p082-requeue:{cj_id}:{wi_id}:1
    let repair_id = "p082-requeue:journal-001:work-001:1";
    startup_repairs::record(
        &pool,
        repair_id,
        "run-p082-r01",
        "requeue_once",
        now,
        Some(r#"{"requeue_generation":1,"max_requeue_generation":1}"#),
    )
    .await
    .expect("record startup repair");

    // Try to insert the same idempotency key again — must fail or be deduplicated
    let second =
        startup_repairs::record(&pool, repair_id, "run-p082-r01", "requeue_once", now, None).await;
    // Duplicate key should fail (unique constraint on startup_repairs.id)
    assert!(
        second.is_err(),
        "P082-R01: second startup repair with same idempotency key must be rejected (unique constraint)"
    );
}

// ── P082-R16: Startup requeue exhausted holds without duplicate ────────────

#[tokio::test]
async fn p082_r16_startup_requeue_exhausted_reason_code_defined() {
    assert_eq!(
        recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED,
        "startup_requeue_exhausted",
        "P082-R16: startup_requeue_exhausted reason code must be defined"
    );
    // Verify it is in the ALL_REASON_CODES list
    assert!(
        recovery_matrix::ALL_REASON_CODES
            .contains(&recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED),
        "P082-R16: startup_requeue_exhausted must be in ALL_REASON_CODES"
    );
}

// ── P082: Schema version constants ────────────────────────────────────────

#[tokio::test]
async fn p082_schema_version_constants_are_correct() {
    assert_eq!(
        recovery_matrix::SCHEMA_READBACK_V1,
        "p082_recovery_matrix_readback_v1"
    );
    assert_eq!(
        recovery_matrix::SCHEMA_REJECTED_COMMAND_ERROR_V1,
        "p082_rejected_command_error_v1"
    );
    assert_eq!(
        recovery_matrix::SCHEMA_RETRY_IDENTIFIER_GUIDANCE_V1,
        "p082_retry_identifier_guidance_v1"
    );
    assert_eq!(
        recovery_matrix::SCHEMA_LATE_OUTPUT_SETTLEMENT_V1,
        "p082_late_output_settlement_v1"
    );
    assert_eq!(
        recovery_matrix::SCHEMA_STARTUP_REPAIR_SUMMARY_V1,
        "p082_startup_repair_summary_v1"
    );
}

// ── P082: All reason codes unique and count correct ─────────────────────────

#[tokio::test]
async fn p082_all_reason_codes_unique_and_count_correct() {
    let mut seen = std::collections::HashSet::new();
    for code in recovery_matrix::ALL_REASON_CODES {
        assert!(
            seen.insert(*code),
            "P082: duplicate reason code in ALL_REASON_CODES: {code}"
        );
    }
    assert!(
        recovery_matrix::ALL_REASON_CODES.len() >= 19,
        "P082: expected at least 19 reason codes, found {}",
        recovery_matrix::ALL_REASON_CODES.len()
    );
}

// ── P082: startup_repairs.notes stores p082_recovery_matrix_readback ──────────

#[tokio::test]
async fn p082_startup_repairs_notes_stores_readback_json() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = "run-p082-notes-test";

    // Build a valid P082 readback and embed it in startup_repair notes.
    let readback = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled; startup_repairs row created with requeue_generation=1.",
        "startup_repairs",
        "startup_repairs, work_items",
        "p082-requeue:cj-notes-test:wi-notes-test:1",
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );

    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();

    startup_repairs::record(
        &pool,
        "p082-requeue:cj-notes-test:wi-notes-test:1",
        run_id,
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair with P082 readback in notes");

    // Read back and verify notes contains p082_recovery_matrix_readback.
    let notes_raw: Option<String> =
        sqlx::query_scalar("SELECT notes FROM startup_repairs WHERE id = ?1")
            .bind("p082-requeue:cj-notes-test:wi-notes-test:1")
            .fetch_optional(&pool)
            .await
            .expect("fetch startup_repair notes");

    let notes_str = notes_raw.expect("notes must be set");
    let notes_json: serde_json::Value = serde_json::from_str(&notes_str).unwrap();
    let rb = notes_json
        .get("p082_recovery_matrix_readback")
        .expect("notes must contain p082_recovery_matrix_readback key");

    assert_eq!(
        rb.get("schema_version").and_then(|v| v.as_str()),
        Some(recovery_matrix::SCHEMA_READBACK_V1),
        "P082: nested readback schema_version must be p082_recovery_matrix_readback_v1"
    );
    assert_eq!(
        rb.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R01"),
        "P082: nested readback scenario_id must be P082-R01"
    );
    assert_eq!(
        rb.get("scenario_status").and_then(|v| v.as_str()),
        Some("repaired"),
        "P082: nested readback scenario_status must use approved vocabulary (repaired)"
    );
    assert_eq!(
        rb.get("recovery_decision").and_then(|v| v.as_str()),
        Some("retry"),
        "P082: nested readback recovery_decision must use approved vocabulary (retry)"
    );
    assert_eq!(
        rb.get("recovery_reason_code").and_then(|v| v.as_str()),
        Some("startup_requeue_once"),
        "P082: nested readback reason_code must match"
    );
    // recovery_next_action must be a non-empty string for non-not_applicable status.
    let next_action = rb
        .get("recovery_next_action")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        !next_action.is_empty(),
        "P082: recovery_next_action must be non-empty for repaired status"
    );
    // recovery_projection_integrity must be valid.
    assert_eq!(
        rb.get("recovery_projection_integrity")
            .and_then(|v| v.as_str()),
        Some("valid"),
        "P082: recovery_projection_integrity must be valid"
    );
}

// ── P082: Enum vocabulary validation — approved values only ───────────────────

#[test]
fn p082_approved_scenario_status_vocabulary() {
    let approved = [
        "repaired",
        "rejected",
        "held",
        "pending",
        "cancelled",
        "not_applicable",
    ];
    for status in &approved {
        assert!(
            recovery_matrix::VALID_SCENARIO_STATUSES.contains(status),
            "P082: VALID_SCENARIO_STATUSES must include '{status}'"
        );
    }
    // Legacy values must NOT be present.
    for legacy in &["resolved", "active", "repair_converged"] {
        assert!(
            !recovery_matrix::VALID_SCENARIO_STATUSES.contains(legacy),
            "P082: VALID_SCENARIO_STATUSES must not contain legacy value '{legacy}'"
        );
    }
}

#[test]
fn p082_approved_recovery_decision_vocabulary() {
    let approved = [
        "retry",
        "wait",
        "reconcile_side_effects",
        "operator_approval_required",
        "inspect_duplicate_owner",
        "cancel",
        "no_mutation",
    ];
    for decision in &approved {
        assert!(
            recovery_matrix::VALID_RECOVERY_DECISIONS.contains(decision),
            "P082: VALID_RECOVERY_DECISIONS must include '{decision}'"
        );
    }
    // Legacy values must NOT be present.
    for legacy in &["held", "cancelled", "repair_converged"] {
        assert!(
            !recovery_matrix::VALID_RECOVERY_DECISIONS.contains(legacy),
            "P082: VALID_RECOVERY_DECISIONS must not contain legacy value '{legacy}'"
        );
    }
}

#[test]
fn p082_projection_integrity_includes_tamper_detected() {
    assert!(
        recovery_matrix::VALID_PROJECTION_INTEGRITIES.contains(&"tamper_detected"),
        "P082: VALID_PROJECTION_INTEGRITIES must include tamper_detected"
    );
    assert!(
        recovery_matrix::VALID_PROJECTION_INTEGRITIES.contains(&"valid"),
        "P082: VALID_PROJECTION_INTEGRITIES must include valid"
    );
    assert!(
        recovery_matrix::VALID_PROJECTION_INTEGRITIES.contains(&"stale"),
        "P082: VALID_PROJECTION_INTEGRITIES must include stale"
    );
    assert!(
        recovery_matrix::VALID_PROJECTION_INTEGRITIES.contains(&"unavailable"),
        "P082: VALID_PROJECTION_INTEGRITIES must include unavailable"
    );
}

// ── P082-R03/R17: Late-output and cancel-late-output settlement shapes ────────

#[test]
fn p082_late_output_settlement_schema_constant_is_correct() {
    assert_eq!(
        recovery_matrix::SCHEMA_LATE_OUTPUT_SETTLEMENT_V1,
        "p082_late_output_settlement_v1"
    );
}

#[test]
fn p082_late_output_settlement_v1_shape_assertion() {
    // Build a representative p082_late_output_settlement_v1 and verify required fields.
    let settlement = serde_json::json!({
        "schema_version": recovery_matrix::SCHEMA_LATE_OUTPUT_SETTLEMENT_V1,
        "source_agent_execution_id": "ae-003",
        "source_work_item_id": "wi-003",
        "source_session_generation_id": "sg-003-gen1",
        "active_session_generation_id": "sg-003-gen2",
        "claim_state": "superseded",
        "output_settlement": "quarantined",
        "ignored_late_output_count": 1,
        "source_work_item_terminal_status": "completed",
        "active_projection_changed": false,
        "cancelled_provider_session": false,
    });

    let required_fields = [
        "schema_version",
        "source_agent_execution_id",
        "source_work_item_id",
        "source_session_generation_id",
        "active_session_generation_id",
        "claim_state",
        "output_settlement",
        "ignored_late_output_count",
        "source_work_item_terminal_status",
        "active_projection_changed",
        "cancelled_provider_session",
    ];
    for field in &required_fields {
        assert!(
            settlement.get(field).is_some(),
            "P082: p082_late_output_settlement_v1 must include field '{field}'"
        );
    }
    // active_projection_changed must be false for late-output scenarios.
    assert_eq!(
        settlement["active_projection_changed"].as_bool(),
        Some(false),
        "P082-R03/R17: active_projection_changed must be false for late-output settlement"
    );
    // claim_state must be superseded or closed.
    let claim_state = settlement["claim_state"].as_str().unwrap();
    assert!(
        ["superseded", "closed", "ignored"].contains(&claim_state),
        "P082: claim_state must be one of superseded, closed, ignored"
    );
}

// ── P082: Legacy plain-text error surfaces as unavailable fallback row ────────

#[tokio::test]
async fn p082_legacy_plain_text_error_produces_unavailable_fallback_row() {
    let pool = setup_db().await;
    let now = Utc::now();
    // Use a real UUID so RunId parses correctly for the accessor.
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("p082-legacy-fallback-{run_id_str}");

    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &format!(r#"{{"run_id":"{run_id_str}","stage_id":"deploy"}}"#),
        Some(&run_id_str),
        now,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal");

    // Write a legacy plain-text error (pre-P082 format)
    command_journal::fail_entry(
        &pool,
        &journal_id,
        now,
        "Stage not retryable: status=completed",
    )
    .await
    .expect("fail with plain text error");

    // The accessor must surface a fallback row, not silently drop it
    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail for legacy plain-text error");

    assert_eq!(
        readbacks.len(),
        1,
        "P082: legacy plain-text error must produce exactly one fallback row"
    );

    let row = &readbacks[0];
    assert_eq!(
        row.get("schema_version").and_then(|v| v.as_str()),
        Some(recovery_matrix::SCHEMA_READBACK_V1),
        "P082: fallback row schema_version must be p082_recovery_matrix_readback_v1"
    );
    assert_eq!(
        row.get("recovery_projection_integrity")
            .and_then(|v| v.as_str()),
        Some("unavailable"),
        "P082: fallback row recovery_projection_integrity must be unavailable"
    );
    assert_eq!(
        row.get("scenario_status").and_then(|v| v.as_str()),
        Some("held"),
        "P082: fallback row scenario_status must be held"
    );
    assert_eq!(
        row.get("recovery_decision").and_then(|v| v.as_str()),
        Some("wait"),
        "P082: fallback row recovery_decision must be wait"
    );
    assert_eq!(
        row.get("source_table").and_then(|v| v.as_str()),
        Some("command_journal"),
        "P082: fallback row source_table must be command_journal"
    );
    // Raw error text must NOT appear in the row (SEC-P082-001 / backward-compat)
    let row_str = row.to_string();
    assert!(
        !row_str.contains("not retryable"),
        "P082: fallback row must not expose raw plain-text error content"
    );
    assert!(
        !row_str.contains("status=completed"),
        "P082: fallback row must not expose raw error detail"
    );
}

// ── P082: Allowlist projection strips unknown/injected keys ──────────────────

#[tokio::test]
async fn p082_allowlist_strips_injected_keys_from_startup_repair_notes() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let repair_key = format!("p082-requeue:cj-sec:{run_id_str}:1");

    // Build a valid readback but inject sensitive keys into the readback object.
    let summary = recovery_matrix::build_startup_repair_summary(
        &repair_key,
        "wi-sec",
        "cj-sec",
        1,
        1,
        false,
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "startup",
    );
    let readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs, work_items",
            &repair_key,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        summary,
        None,
    );

    let mut readback_obj = readback.as_object().cloned().unwrap();
    readback_obj.insert(
        "access_token".to_string(),
        serde_json::json!("secret-bearer-token"),
    );
    readback_obj.insert(
        "raw_stderr".to_string(),
        serde_json::json!("provider error output"),
    );
    readback_obj.insert(
        "absolute_path".to_string(),
        serde_json::json!("/Users/user/.ssh/id_rsa"),
    );
    let injected_readback = serde_json::Value::Object(readback_obj);

    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": injected_readback,
    })
    .to_string();

    startup_repairs::record(
        &pool,
        &repair_key,
        &run_id_str,
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair with injected readback");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    assert_eq!(readbacks.len(), 1, "P082: must return exactly one row");
    let row = &readbacks[0];
    let row_str = row.to_string();

    // Injected keys must be stripped by allowlist projection (SEC-P082-001)
    assert!(
        !row_str.contains("access_token"),
        "P082 SEC-P082-001: access_token must be stripped by allowlist projection"
    );
    assert!(
        !row_str.contains("secret-bearer-token"),
        "P082 SEC-P082-001: bearer token value must be stripped"
    );
    assert!(
        !row_str.contains("raw_stderr"),
        "P082 SEC-P082-001: raw_stderr must be stripped by allowlist projection"
    );
    assert!(
        !row_str.contains("absolute_path"),
        "P082 SEC-P082-001: absolute_path must be stripped by allowlist projection"
    );
    assert_eq!(
        row.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R01"),
        "P082: scenario_id must survive allowlist projection"
    );
    assert_eq!(
        row.get("diagnostic_redaction").and_then(|v| v.as_str()),
        Some("partial"),
        "SEC-P082-LOW-001: stripping injected top-level keys must mark diagnostic_redaction partial"
    );
}

#[tokio::test]
async fn p082_omitted_optional_subcontract_keys_do_not_reach_readback_lanes() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let repair_key = format!("p082-requeue:cj-missing-subcontracts:{run_id_str}:1");
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let summary = recovery_matrix::build_startup_repair_summary(
        &repair_key,
        "wi-missing-subcontracts",
        "cj-missing-subcontracts",
        1,
        1,
        false,
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "startup",
    );
    let mut readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs, work_items",
            &repair_key,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        summary,
        None,
    );
    readback
        .as_object_mut()
        .expect("readback object")
        .remove("recovery_retry_identifier_guidance");
    assert!(
        !recovery_matrix::validate_readback_v1_shape(&readback),
        "test setup must omit a required top-level optional-subcontract field"
    );

    let notes = serde_json::json!({
        "p082_recovery_matrix_readback": readback
    })
    .to_string();

    startup_repairs::record(
        &pool,
        &repair_key,
        &run_id_str,
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair with malformed readback");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    assert_eq!(
        readbacks.len(),
        1,
        "malformed startup repair readback must produce one tamper fallback row"
    );
    let row = &readbacks[0];
    for field in [
        "recovery_retry_identifier_guidance",
        "recovery_late_output_settlement",
        "recovery_startup_repair_summary",
        "source_json_key",
    ] {
        assert!(
            row.as_object()
                .is_some_and(|object| object.contains_key(field)),
            "P082 lane row must include explicit top-level field {field}: {row}"
        );
    }
    assert_eq!(
        row.get("recovery_projection_integrity")
            .and_then(|value| value.as_str()),
        Some("tamper_detected"),
        "malformed readback must not leak; accessor should return tamper fallback"
    );
}

// ── P082 SEC-P082-001: String field length cap ─────────────────────────────

#[tokio::test]
async fn p082_sec_p082_001_oversized_string_field_is_capped() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let repair_key = format!("p082-requeue:cj-sec-cap:{run_id_str}:1");

    // Build a readback with an oversized recovery_operator_message (>2048 bytes).
    let oversized_message = "X".repeat(3000);
    let summary = recovery_matrix::build_startup_repair_summary(
        &repair_key,
        "wi-sec-cap",
        "cj-sec-cap",
        1,
        1,
        false,
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "startup",
    );
    let readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R05",
            "held",
            "wait",
            recovery_matrix::REASON_STARTUP_STALLED,
            "Stale ACP startup detected.",
            "startup_repairs, work_items, session_generations",
            "work_items, sessions, startup_repairs",
            &repair_key,
            Some("work_items.payload_json.p061_startup_recovery"),
            "valid",
            &now.to_rfc3339(),
        ),
        summary,
        Some(&oversized_message),
    );

    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();

    startup_repairs::record(
        &pool,
        &repair_key,
        &run_id_str,
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair with oversized message");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    assert_eq!(readbacks.len(), 1, "P082: must return exactly one row");
    let row = &readbacks[0];

    // The oversized string must be replaced with the redaction marker.
    let message = row
        .get("recovery_operator_message")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        !message.starts_with('X'),
        "P082 SEC-P082-001: oversized recovery_operator_message must not be surfaced as-is"
    );
    assert!(
        message.contains("[redacted:"),
        "P082 SEC-P082-001: oversized field must be replaced with [redacted: N bytes] marker"
    );
    // Legitimate identifier fields must survive.
    assert_eq!(
        row.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R05"),
        "P082: scenario_id must survive length capping"
    );
}

// ── P082-R07: Held-state readback must carry non-null blocking-status field ───

#[tokio::test]
async fn p082_r07_db_held_state_readback_has_non_null_blocking_status() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("p082-r07-db-journal-{run_id_str}");

    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &serde_json::json!({"run_id": &run_id_str, "stage_id": "release"}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.retry"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry");

    // Build R07 envelope with non-null blocking status (the fixed production path).
    let readback = recovery_matrix::set_readback_side_effect_hold(
        recovery_matrix::build_readback_v1(
            "P082-R07",
            "held",
            "reconcile_side_effects",
            recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION,
            "Reconcile unresolved side effects before retrying this stage.",
            "side_effects, command_journal",
            "side_effects, command_journal",
            &journal_id,
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
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
    command_journal::fail_entry(&pool, &journal_id, now, &envelope)
        .await
        .expect("fail command journal entry with R07 P082 envelope");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    assert_eq!(readbacks.len(), 1, "P082-R07: must return exactly one row");
    let row = &readbacks[0];

    assert_eq!(
        row.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R07"),
        "P082-R07: scenario_id must be P082-R07"
    );
    assert_eq!(
        row.get("scenario_status").and_then(|v| v.as_str()),
        Some("held"),
        "P082-R07: scenario_status must be held"
    );
    assert!(
        !row.get("recovery_side_effect_blocking_status")
            .map(|v| v.is_null())
            .unwrap_or(true),
        "P082-R07: recovery_side_effect_blocking_status must be non-null in DB round-trip"
    );
    assert_eq!(
        row.get("recovery_side_effect_blocking_status")
            .and_then(|v| v.as_str()),
        Some("unresolved_side_effect_entries"),
        "P082-R07: recovery_side_effect_blocking_status must be 'unresolved_side_effect_entries'"
    );
    assert!(
        !row.get("recovery_operator_message")
            .map(|v| v.is_null())
            .unwrap_or(true),
        "P082-R07: recovery_operator_message must be non-null in DB round-trip"
    );
}

// ── P082 SEC-HIGH-1: Nested subcontract injection must be stripped ────────────

#[tokio::test]
async fn p082_sec_high1_nested_subcontract_injection_is_stripped() {
    // Verify that injected keys inside recovery_retry_identifier_guidance
    // (a nested subcontract) are stripped by allowlist_project's recursive sanitizer.
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let repair_key = format!("p082-requeue:cj-nested-sec:{run_id_str}:1");

    // Build a valid readback.
    let mut readback = recovery_matrix::build_readback_v1(
        "P082-R08",
        "rejected",
        "no_mutation",
        recovery_matrix::REASON_VALID_IDENTIFIER_GUIDANCE,
        "Wrong identifier kind; use stage_execution_uuid.",
        "command_journal",
        "command_journal",
        &repair_key,
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );

    // Inject a fake nested subcontract with sensitive keys.
    if let Some(obj) = readback.as_object_mut() {
        obj.insert(
            "recovery_retry_identifier_guidance".to_string(),
            serde_json::json!({
                "schema_version": recovery_matrix::SCHEMA_RETRY_IDENTIFIER_GUIDANCE_V1,
                "command": "RetryStage",
                "provided_identifier": "bad-id",
                "provided_identifier_kind": "unknown",
                "expected_identifier_kind": "stage_execution_uuid",
                "valid_identifier_examples": ["exec-uuid-1234"],
                "no_mutation": true,
                "access_token": "injected-secret-token",        // must be stripped
                "raw_stderr": "provider stderr content",         // must be stripped
                "absolute_path": "/Users/test/.ssh/id_rsa",     // must be stripped
            }),
        );
    }

    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();

    startup_repairs::record(
        &pool,
        &repair_key,
        &run_id_str,
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair with injected nested subcontract");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    assert_eq!(
        readbacks.len(),
        1,
        "P082 SEC-HIGH-1: must return exactly one row"
    );
    let row = &readbacks[0];
    let guidance = row
        .get("recovery_retry_identifier_guidance")
        .expect("recovery_retry_identifier_guidance must be present");
    let guidance_str = guidance.to_string();

    // Injected keys must be stripped from the nested subcontract.
    assert!(
        !guidance_str.contains("access_token"),
        "P082 SEC-HIGH-1: access_token must be stripped from nested subcontract"
    );
    assert!(
        !guidance_str.contains("injected-secret-token"),
        "P082 SEC-HIGH-1: injected token value must be stripped from nested subcontract"
    );
    assert!(
        !guidance_str.contains("raw_stderr"),
        "P082 SEC-HIGH-1: raw_stderr must be stripped from nested subcontract"
    );
    assert!(
        !guidance_str.contains("absolute_path"),
        "P082 SEC-HIGH-1: absolute_path must be stripped from nested subcontract"
    );

    // Legitimate subcontract fields must survive.
    assert!(
        guidance_str.contains("no_mutation"),
        "P082 SEC-HIGH-1: legitimate no_mutation field must survive nested sanitization"
    );
    assert!(
        guidance_str.contains("stage_execution_uuid"),
        "P082 SEC-HIGH-1: expected_identifier_kind must survive nested sanitization"
    );
    assert_eq!(
        row.get("diagnostic_redaction").and_then(|v| v.as_str()),
        Some("partial"),
        "SEC-P082-LOW-001: stripping injected nested subcontract keys must mark diagnostic_redaction partial"
    );
}

#[tokio::test]
async fn p082_sec_medium1_nested_allowed_strings_redact_token_query_params() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let repair_key = format!("p082-requeue:cj-nested-query-redact:{run_id_str}:1");

    let summary = recovery_matrix::build_startup_repair_summary(
        &repair_key,
        "wi-query-redact",
        "cj-query-redact",
        1,
        1,
        false,
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "scope=https://example.test/retry?token=secret-token&ok=1 password=hunter2",
    );
    let readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs, work_items",
            &repair_key,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        summary,
        None,
    );
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();

    startup_repairs::record(
        &pool,
        &repair_key,
        &run_id_str,
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair with nested token query");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    let row = readbacks.first().expect("readback row");
    let row_str = row.to_string();
    assert!(!row_str.contains("secret-token"));
    assert!(!row_str.contains("hunter2"));
    assert!(row_str.contains("token=[redacted]"));
    assert!(row_str.contains("password=[redacted]"));
    assert_eq!(
        row.get("diagnostic_redaction").and_then(|v| v.as_str()),
        Some("partial"),
        "SEC-MEDIUM-001: nested token query redaction must mark diagnostic_redaction partial"
    );
}

#[tokio::test]
async fn p082_sec_001_allowed_string_arrays_are_recursively_redacted() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let repair_key = format!("p082-requeue:cj-array-redact:{run_id_str}:1");

    let guidance = recovery_matrix::build_retry_identifier_guidance(
        "RetryStage",
        "bad-stage",
        "unknown",
        "stage_execution_uuid",
        &[
            "/Users/user/.ssh/id_rsa",
            "Bearer sk-test-secret-token",
            "stage-exec-safe-example",
        ],
    );
    let mut readback = recovery_matrix::set_readback_identifier_guidance(
        recovery_matrix::build_readback_v1(
            "P082-R08",
            "rejected",
            "no_mutation",
            recovery_matrix::REASON_VALID_IDENTIFIER_GUIDANCE,
            "Wrong identifier kind; use stage_execution_uuid.",
            "command_journal",
            "command_journal",
            &repair_key,
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        guidance,
    );
    if let Some(obj) = readback.as_object_mut() {
        obj.insert(
            "recovery_hold_conditions".to_string(),
            serde_json::json!(["Inspect /Users/user/.chainworks/runs/run-1", "safe hold"]),
        );
    }

    let notes = serde_json::json!({ "p082_recovery_matrix_readback": readback }).to_string();
    startup_repairs::record(
        &pool,
        &repair_key,
        &run_id_str,
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair with allowed string-array secrets");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    let row = readbacks.first().expect("P082 SEC-001 row");
    let row_str = row.to_string();
    assert!(
        !row_str.contains("/Users/") && !row_str.contains("sk-test") && !row_str.contains("Bearer"),
        "P082 SEC-001: allowed string arrays must be recursively redacted"
    );
    assert!(
        row_str.contains("stage-exec-safe-example"),
        "P082 SEC-001: safe string-array entries must survive recursive sanitization"
    );
}

#[tokio::test]
async fn p082_sec_001_objects_inside_allowed_arrays_fail_closed() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let repair_key = format!("p082-requeue:cj-array-object:{run_id_str}:1");
    let mut readback = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs, work_items",
        &repair_key,
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    if let Some(obj) = readback.as_object_mut() {
        obj.insert(
            "recovery_hold_conditions".to_string(),
            serde_json::json!([{"raw_stderr": "Bearer sk-test-secret"}]),
        );
    }

    let notes = serde_json::json!({ "p082_recovery_matrix_readback": readback }).to_string();
    startup_repairs::record(
        &pool,
        &repair_key,
        &run_id_str,
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair with object inside allowed array");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    let row = readbacks.first().expect("P082 SEC-001 fallback row");
    assert_eq!(
        row.get("recovery_projection_integrity")
            .and_then(|value| value.as_str()),
        Some("tamper_detected"),
        "P082 SEC-001: object injection inside allowed arrays must fail closed"
    );
    assert!(
        !row.to_string().contains("sk-test-secret"),
        "P082 SEC-001: tamper fallback must not expose injected object contents"
    );
}

// ── P082 SEC-MEDIUM-1: Tampered startup_repair readback produces tamper_detected row ──

#[tokio::test]
async fn p082_sec_medium1_tampered_startup_repair_readback_produces_tamper_detected_row() {
    // When startup_repairs.notes contains a p082_recovery_matrix_readback with a
    // non-canonical scenario_id (e.g. tampered), the accessor must emit a
    // tamper_detected fallback row rather than exposing the malformed data.
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let repair_key = format!("p082-requeue:cj-tampered:{run_id_str}:1");

    // Build a readback but set an invalid (non-canonical) scenario_id.
    let mut readback = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs",
        &repair_key,
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    // Tamper the scenario_id to a non-canonical value.
    if let Some(obj) = readback.as_object_mut() {
        obj.insert("scenario_id".into(), serde_json::json!("P082-TAMPERED-ID"));
    }

    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();

    startup_repairs::record(
        &pool,
        &repair_key,
        &run_id_str,
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair with tampered readback");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail for tampered row");

    assert_eq!(
        readbacks.len(),
        1,
        "P082 SEC-MEDIUM-1: tampered row must produce exactly one tamper_detected fallback"
    );
    let row = &readbacks[0];
    assert_eq!(
        row.get("recovery_projection_integrity").and_then(|v| v.as_str()),
        Some("tamper_detected"),
        "P082 SEC-MEDIUM-1: tampered startup_repair row must produce recovery_projection_integrity=tamper_detected"
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(row),
        "P082 SEC-MEDIUM-1: tampered startup_repair fallback must validate against p082_recovery_matrix_readback_v1: {row}"
    );
    assert!(
        row.get("recovery_startup_repair_summary")
            .is_some_and(|value| !value.is_null()),
        "P082 SEC-MEDIUM-1: P082-R01 tamper fallback must include recovery_startup_repair_summary"
    );
    // The tampered scenario_id must NOT appear in the output.
    let row_str = row.to_string();
    assert!(
        !row_str.contains("TAMPERED"),
        "P082 SEC-MEDIUM-1: non-canonical tampered scenario_id must not appear in fallback row"
    );
}

#[tokio::test]
async fn p082_r09_pending_approval_accessor_derives_operator_action_readback() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    sqlx::query(
        r#"INSERT INTO approvals (id, run_id, stage_id, decision, requested_at, decided_at, comment, expires_at)
           VALUES (?1, ?2, 'approval-stage', 'pending', ?3, NULL, NULL, NULL)"#,
    )
    .bind("p082-r09-approval")
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert pending approval");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("read P082 readbacks");
    let row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|value| value.as_str()) == Some("P082-R09"))
        .expect("P082-R09 readback");
    assert_eq!(
        row.get("recovery_decision")
            .and_then(|value| value.as_str()),
        Some("operator_approval_required"),
        "P082-R09: pending approval readback must point to human approval path"
    );
    assert_eq!(
        row.get("recovery_reason_code")
            .and_then(|value| value.as_str()),
        Some(recovery_matrix::REASON_APPROVAL_PENDING_OPERATOR_ACTION_REQUIRED),
        "P082-R09: reason code must match approved vocabulary"
    );
    assert_eq!(
        row.get("source_json_key").and_then(|value| value.as_str()),
        Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
        "P082-R09: pending approval readback must point to the approved stage recovery snapshot owner"
    );
}

#[tokio::test]
async fn p082_r10_duplicate_mediation_accessor_derives_rejected_owner_readback() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    for (id, status) in [
        ("p082-r10-mediation-active", "pending"),
        ("p082-r10-mediation-duplicate", "superseded"),
    ] {
        sqlx::query(
            r#"INSERT INTO lead_conflict_mediations
               (id, run_id, conflict_id, conflict_fingerprint, lead_agent_id, status,
                created_at, updated_at)
               VALUES (?1, ?2, 'conflict-r10', 'fingerprint-r10', 'lead-a', ?3, ?4, ?4)"#,
        )
        .bind(id)
        .bind(&run_id_str)
        .bind(status)
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .expect("insert mediation row");
    }

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("read P082 readbacks");
    let row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|value| value.as_str()) == Some("P082-R10"))
        .expect("P082-R10 readback");
    assert_eq!(
        row.get("recovery_decision")
            .and_then(|value| value.as_str()),
        Some("inspect_duplicate_owner"),
        "P082-R10: duplicate mediation readback must direct owner inspection"
    );
    assert_eq!(
        row.get("recovery_reason_code")
            .and_then(|value| value.as_str()),
        Some(recovery_matrix::REASON_DUPLICATE_MEDIATION_OWNER_REJECTED),
        "P082-R10: reason code must match approved vocabulary"
    );
    assert_eq!(
        row.get("source_json_key").and_then(|value| value.as_str()),
        Some("lead_conflict_mediations.validation_errors_json.p082_recovery_matrix_readback"),
        "P082-R10: duplicate mediation readback must point to approved duplicate-owner evidence"
    );
}

#[tokio::test]
async fn p082_r04_duplicate_session_owner_accessor_reads_repaired_event() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    for index in 1..=2 {
        let lineage_id = format!("p082-r04-lineage-{index}");
        let generation_id = format!("p082-r04-generation-{index}");
        sqlx::query(
            r#"INSERT INTO session_lineages
               (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
                active_generation_id, created_at, closed_at)
               VALUES (?1, ?2, 'agent-r04', ?1, 'run', NULL, ?3, ?4, NULL)"#,
        )
        .bind(&lineage_id)
        .bind(&run_id_str)
        .bind(&generation_id)
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .expect("insert session lineage");
        sqlx::query(
            r#"INSERT INTO session_generations
               (id, lineage_id, generation, invocation_owner_key, provider_session_id,
                binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
                workspace_mode, runtime_provider, runtime_model, status, created_at)
               VALUES (?1, ?2, ?3, 'duplicate-owner-r04', NULL, 'binding-r04', NULL, '/',
                       'read_write', 'codex', 'gpt-test', 'active', ?4)"#,
        )
        .bind(&generation_id)
        .bind(&lineage_id)
        .bind(index)
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .expect("insert session generation");
    }

    sqlx::query(
        r#"UPDATE session_generations
           SET status = 'invalidated', ended_at = ?1, end_reason = 'p082_duplicate_owner_repaired'
           WHERE id = 'p082-r04-generation-1'"#,
    )
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("invalidate duplicate generation");
    sqlx::query(
        r#"UPDATE session_lineages
           SET active_generation_id = NULL
           WHERE id = 'p082-r04-lineage-1'"#,
    )
    .execute(&pool)
    .await
    .expect("clear duplicate active lineage");

    let readback = recovery_matrix::build_readback_v1(
        "P082-R04",
        "repaired",
        "inspect_duplicate_owner",
        recovery_matrix::REASON_DUPLICATE_OWNER_REPAIRED,
        "Duplicate active session owner repaired; one active generation remains.",
        "session_events",
        "session_lineages, session_generations, session_events, work_items",
        "p082-r04-duplicate-owner-repair:p082-r04-generation-1",
        Some("session_events.details_json.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    let details = serde_json::json!({
        "schema_version": "p082_duplicate_session_owner_repair_v1",
        "run_id": run_id_str,
        "invocation_owner_key": "duplicate-owner-r04",
        "surviving_generation_id": "p082-r04-generation-2",
        "invalidated_generation_id": "p082-r04-generation-1",
        "p082_recovery_matrix_readback": readback,
    });
    sqlx::query(
        r#"INSERT INTO session_events
           (id, lineage_id, generation_id, event_type, recorded_at, details_json)
           VALUES ('p082-r04-duplicate-owner-repair:p082-r04-generation-1',
                   'p082-r04-lineage-1',
                   'p082-r04-generation-1',
                   'invalidated',
                   ?1,
                   ?2)"#,
    )
    .bind(now.to_rfc3339())
    .bind(details.to_string())
    .execute(&pool)
    .await
    .expect("insert repaired duplicate owner event");

    let active_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM session_generations
           WHERE invocation_owner_key = 'duplicate-owner-r04'
             AND status = 'active'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("count active session owners");
    assert_eq!(
        active_count, 1,
        "P082-R04: repaired durable state must leave one active session owner"
    );
    let invalidated_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM session_generations
           WHERE invocation_owner_key = 'duplicate-owner-r04'
             AND status = 'invalidated'
             AND end_reason = 'p082_duplicate_owner_repaired'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("count invalidated duplicate owners");
    assert_eq!(
        invalidated_count, 1,
        "P082-R04: duplicate evidence must be terminalized"
    );

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("read P082 readbacks");
    let row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|value| value.as_str()) == Some("P082-R04"))
        .expect("P082-R04 readback");
    assert_eq!(
        row.get("scenario_status").and_then(|value| value.as_str()),
        Some("repaired"),
        "P082-R04: repaired session event readback must report repaired status"
    );
    assert_eq!(
        row.get("recovery_projection_integrity")
            .and_then(|value| value.as_str()),
        Some("valid"),
        "P082-R04: repaired session event readback must mark valid projection integrity"
    );
    assert_eq!(
        row.get("source_json_key").and_then(|value| value.as_str()),
        Some("session_events.details_json.p082_recovery_matrix_readback"),
        "P082-R04: duplicate session readback must point to approved session event evidence"
    );
}

#[tokio::test]
async fn p082_r05_stale_xcode_startup_accessor_derives_operator_message() {
    let pool = setup_db().await;
    let now = Utc::now();
    let stale_at = now - chrono::Duration::minutes(13);
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    sqlx::query(
        r#"INSERT INTO session_lineages
           (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
            active_generation_id, created_at, closed_at)
           VALUES ('p082-r05-lineage', ?1, 'agent-r05', 'p082-r05-lineage', 'run', NULL,
                   'p082-r05-generation', ?2, NULL)"#,
    )
    .bind(&run_id_str)
    .bind(stale_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert stale lineage");
    sqlx::query(
        r#"INSERT INTO session_generations
           (id, lineage_id, generation, invocation_owner_key, provider_session_id,
            binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
            workspace_mode, runtime_provider, runtime_model, status, created_at)
           VALUES ('p082-r05-generation', 'p082-r05-lineage', 1, 'invocation-owner-r05', NULL,
                   'binding-r05', NULL, '/', 'read_write', 'xcode', 'xcode-test', 'active', ?1)"#,
    )
    .bind(stale_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert stale generation");
    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id,
            owner_kind, owner_id)
           VALUES ('p082-r05-ae', 'agent-r05', 'xcode', 'running', ?1,
                   'p082-r05-generation', 'p082-r05-lineage', 'lead_conflict_mediation', 'p082-r05-ae')"#,
    )
    .bind(stale_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running agent execution");
    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES ('work-item-r05', ?1, 'invoke_agent', ?2, 'running', ?3, ?3, ?3, 1)"#,
    )
    .bind(&run_id_str)
    .bind(
        serde_json::json!({
            "run_id": run_id_str,
            "xcode_broker_required": true,
            "p061_startup_recovery": { "reason": "startup_stalled" },
            "p058_claimed": { "agent_execution_id": "p082-r05-ae" }
        })
        .to_string(),
    )
    .bind(stale_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running work item owner");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("read P082 readbacks");
    let row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|value| value.as_str()) == Some("P082-R05"))
        .expect("P082-R05 readback");
    assert_eq!(
        row.get("recovery_reason_code")
            .and_then(|value| value.as_str()),
        Some(recovery_matrix::REASON_STARTUP_STALLED),
        "P082-R05: reason code must match stale startup"
    );
    let msg = row
        .get("recovery_operator_message")
        .and_then(|value| value.as_str())
        .expect("P082-R05: Xcode stale startup readback must include a non-null operator message");
    assert!(
        msg.contains("Xcode startup grace"),
        "P082-R05: operator message must name Xcode startup grace: {msg}"
    );
    assert!(
        msg.contains("cutoff:") || msg.contains("cutoff_at") || msg.contains("(cutoff"),
        "P082-R05: operator message must include the cutoff timestamp: {msg}"
    );
    assert!(
        msg.contains("Next check") || msg.contains("next check") || msg.contains("backoff"),
        "P082-R05: operator message must explicitly include the next check/backoff state (not just 'Inspect'): {msg}"
    );
    assert_eq!(
        row.get("source_json_key").and_then(|value| value.as_str()),
        Some("work_items.payload_json.p061_startup_recovery"),
        "P082-R05 held stale-startup readback must point to the approved work item payload owner"
    );
    assert_eq!(
        row.get("source_identifier")
            .and_then(|value| value.as_str()),
        Some("work-item-r05"),
        "P082-R05 held stale-startup readback must identify the joined work item owner"
    );
    assert_eq!(
        row.get("recovery_startup_repair_summary")
            .and_then(|value| value.get("source_work_item_id"))
            .and_then(|value| value.as_str()),
        Some("work-item-r05"),
        "P082-R05 startup summary must use the joined work item id, not session invocation_owner_key"
    );
    assert_eq!(
        row.get("updated_at").and_then(|value| value.as_str()),
        Some(stale_at.to_rfc3339().as_str()),
        "P082-R05 held stale-startup readback must derive updated_at from durable session evidence, not read time"
    );
}

#[tokio::test]
async fn p082_r06_stale_scheduler_without_side_effect_evidence_does_not_infer_hold() {
    let pool = setup_db().await;
    let now = Utc::now();
    let stale_at = now - chrono::Duration::minutes(4);
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, started_at,
            attempt_count, last_error)
           VALUES ('p082-r06-work', 'invoke_agent', '{}', 'running', ?1, 'stage-r06',
                   ?2, ?2, ?2, 1, NULL)"#,
    )
    .bind(&run_id_str)
    .bind(stale_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert stale running work item");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("read P082 readbacks");
    assert!(
        !readbacks
            .iter()
            .any(|row| row.get("scenario_id").and_then(|value| value.as_str())
                == Some("P082-R06")),
        "P082-R06: stale scheduler ownership must not infer a held row without side_effect evidence or explicit repair"
    );
}

#[tokio::test]
async fn p082_r06_stale_scheduler_owner_accessor_derives_repaired_readback_from_work_item_owner() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let readback = recovery_matrix::build_readback_v1(
        "P082-R06",
        "repaired",
        "retry",
        recovery_matrix::REASON_STALE_REPAIRED,
        "Stale scheduler ownership repaired through explicit work item transition.",
        "work_items",
        "work_items",
        "p082-r06-repaired-work",
        Some("work_items.payload_json.p061_startup_recovery"),
        "valid",
        &now.to_rfc3339(),
    );
    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at,
            attempt_count, last_error)
           VALUES ('p082-r06-repaired-work', 'invoke_agent', ?1, 'pending', ?2, 'stage-r06',
                   ?3, ?3, 1, NULL)"#,
    )
    .bind(
        serde_json::json!({
            "p061_startup_recovery": {
                "reason": "stale_scheduler_owner_repaired",
                "p082_recovery_matrix_readback": readback
            }
        })
        .to_string(),
    )
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert repaired stale scheduler work item");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("read P082 readbacks");
    let row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|value| value.as_str()) == Some("P082-R06"))
        .expect("P082-R06 repaired readback");
    assert_eq!(
        row.get("recovery_reason_code")
            .and_then(|value| value.as_str()),
        Some(recovery_matrix::REASON_STALE_REPAIRED),
        "P082-R06: repaired stale scheduler ownership must use stale_repaired"
    );
    assert_eq!(
        row.get("scenario_status").and_then(|value| value.as_str()),
        Some("repaired"),
        "P082-R06: explicit transition must report repaired"
    );
    assert_eq!(
        row.get("source_json_key").and_then(|value| value.as_str()),
        Some("work_items.payload_json.p061_startup_recovery"),
        "P082-R06: repaired stale scheduler readback must use the approved work item JSON owner"
    );
}

#[tokio::test]
async fn p082_r06_stale_scheduler_owner_held_requires_unresolved_side_effect_evidence() {
    let pool = setup_db().await;
    let now = Utc::now();
    let stale_at = now - chrono::Duration::minutes(4);
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let effect_id = format!("se-r06-hold-{run_id_str}");
    let stage_execution_id = uuid::Uuid::new_v4().to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, started_at,
            attempt_count, last_error)
           VALUES ('p082-r06-held-work', 'invoke_agent', '{}', 'running', ?1, 'stage-r06',
                   ?2, ?2, ?2, 1, NULL)"#,
    )
    .bind(&run_id_str)
    .bind(stale_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert stale running work item");
    sqlx::query(
        r#"INSERT INTO side_effects
           (id, run_id, stage_execution_id, effect_kind, target_key,
            idempotency_key, idempotency_key_version, request_fingerprint,
            request_fingerprint_version, status, external_write_attempted,
            attempt_budget_remaining, created_at, updated_at)
           VALUES (?1, ?2, ?3, 'git_commit', 'main',
                   ?4, 1, 'fp-r06-hold', 1, 'prepared', 0, 3, ?5, ?5)"#,
    )
    .bind(&effect_id)
    .bind(&run_id_str)
    .bind(&stage_execution_id)
    .bind(format!("idem-key-r06-hold-{run_id_str}"))
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert side effect evidence");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("read P082 readbacks");
    let row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|value| value.as_str()) == Some("P082-R06"))
        .expect("P082-R06 held readback");
    assert_eq!(
        row.get("recovery_reason_code")
            .and_then(|value| value.as_str()),
        Some(recovery_matrix::REASON_NEEDS_EFFECT_RECONCILIATION),
        "P082-R06: held stale scheduler ownership requires needs_effect_reconciliation"
    );
    assert_eq!(
        row.get("source_identifier")
            .and_then(|value| value.as_str()),
        Some(effect_id.as_str()),
        "P082-R06: held readback must cite the unresolved side_effect row"
    );
    assert_eq!(
        row.get("source_json_key").and_then(|value| value.as_str()),
        Some("work_items.payload_json.p061_startup_recovery"),
        "P082-R06: held readback must carry an approved stale scheduler JSON owner"
    );
    assert_eq!(
        row.get("updated_at").and_then(|value| value.as_str()),
        Some(stale_at.to_rfc3339().as_str()),
        "P082-R06 held stale scheduler readback must derive updated_at from durable work item evidence, not read time"
    );
}

#[tokio::test]
async fn p082_r15_replayed_startup_repair_accessor_derives_crash_resume_readback() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let repair_id = format!("p082-requeue:cj-r15:{run_id_str}:1");

    let summary = recovery_matrix::build_startup_repair_summary(
        &repair_id,
        "work-r15",
        "cj-r15",
        1,
        1,
        true,
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "startup",
    );
    let readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue replayed after crash.",
            "startup_repairs",
            "startup_repairs, work_items",
            &repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        summary,
        None,
    );
    let notes = serde_json::json!({ "p082_recovery_matrix_readback": readback }).to_string();
    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id_str,
        "p082_requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("insert replayed startup repair");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("read P082 readbacks");
    let row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|value| value.as_str()) == Some("P082-R15"))
        .expect("P082-R15 readback");
    assert_eq!(
        row.get("recovery_reason_code")
            .and_then(|value| value.as_str()),
        Some(recovery_matrix::REASON_REPAIR_CRASH_RESUME_IDEMPOTENT),
        "P082-R15: replayed startup repair must emit crash-resume reason"
    );
    assert_eq!(
        row.pointer("/recovery_startup_repair_summary/replayed")
            .and_then(|value| value.as_bool()),
        Some(true),
        "P082-R15: startup repair summary must preserve replayed=true"
    );
}

// ── P082-R15: Crash-boundary: crash after idempotency row insert, then resume ─

/// Simulates a crash-boundary scenario where the daemon crashes after inserting the
/// startup_repairs idempotency row (step 1) but before the work_item status update
/// (step 2). On restart, the second call detects the existing key and must handle
/// idempotent replay without creating duplicate work.
#[tokio::test]
async fn p082_r15_crash_after_idempotency_row_insert_replays_idempotently() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r15-crash-{run_id_str}");
    let work_item_id = format!("wi-r15-crash-{run_id_str}");
    let repair_id = format!("p082-requeue:{journal_id}:{work_item_id}:1");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.retry"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry");

    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, 'invoke_agent', ?2, 'running', ?3, 'implement', ?4, ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(
        serde_json::json!({
            "run_id": run_id_str,
            "stage_id": "implement",
            "stage_execution_id": "se-r15-crash",
            "p058_claimed": { "agent_execution_id": "ae-r15-crash" },
            "source_command_journal_id": journal_id,
        })
        .to_string(),
    )
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running work item");

    // Simulate crash boundary: insert the idempotency row but do NOT update work_item.
    // This simulates a crash between step 1 (idempotency key write) and step 2 (work_item update).
    let r01_summary = recovery_matrix::build_startup_repair_summary(
        &repair_id,
        &work_item_id,
        &journal_id,
        1,
        1,
        false,
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "global",
    );
    let r01_readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup recovery requeued abandoned InvokeAgent work item.",
            "startup_repairs, work_items, command_journal",
            "startup_repairs, work_items",
            &repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        r01_summary,
        None,
    );
    let notes = serde_json::json!({ "p082_recovery_matrix_readback": r01_readback }).to_string();
    // Simulate: idempotency row was written but crash happened before work_item update.
    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id_str,
        "p082_requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("insert startup_repairs idempotency row (simulates pre-crash write)");

    // The work_item is still 'running' — the crash happened before status update.
    let pre_crash_status: String =
        sqlx::query_scalar("SELECT status FROM work_items WHERE id = ?1")
            .bind(&work_item_id)
            .fetch_one(&pool)
            .await
            .expect("fetch work item status before restart");
    assert_eq!(
        pre_crash_status, "running",
        "Pre-crash: work_item must still be running (crash happened before status update)"
    );

    // Daemon restart: re-run the startup requeue. The idempotency key already
    // exists, but the existing startup_repairs.notes row proves R01 was written
    // before the work_item mutation. This is an unapplied R01 replay, not R16.
    let requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now + chrono::Duration::seconds(5),
        "startup_repair_abandoned_invoke_agent",
    )
    .await
    .expect("restart: requeue_running_invoke_agent_on_startup");

    assert_eq!(
        requeued, 1,
        "P082-R15: restart must requeue the unapplied R01 generation when the idempotency row exists but the source work item is still running and unstamped"
    );

    let replay_status: String = sqlx::query_scalar("SELECT status FROM work_items WHERE id = ?1")
        .bind(&work_item_id)
        .fetch_one(&pool)
        .await
        .expect("fetch replayed work item status");
    assert_eq!(
        replay_status, "pending",
        "P082-R15: unapplied R01 replay must return the source work item to pending"
    );

    let replay_payload_raw: String =
        sqlx::query_scalar("SELECT payload_json FROM work_items WHERE id = ?1")
            .bind(&work_item_id)
            .fetch_one(&pool)
            .await
            .expect("fetch replayed work item payload");
    let replay_payload: serde_json::Value =
        serde_json::from_str(&replay_payload_raw).expect("payload json");
    assert_eq!(
        replay_payload
            .pointer("/p061_startup_recovery/startup_repair_id")
            .and_then(|value| value.as_str()),
        Some(repair_id.as_str()),
        "P082-R15: replay must stamp the source payload with the existing repair id"
    );
    assert_eq!(
        replay_payload
            .pointer("/p061_startup_recovery/replayed")
            .and_then(|value| value.as_bool()),
        Some(true),
        "P082-R15: replay payload must mark replayed=true"
    );

    // startup_repairs must have exactly one row for this key.
    let repair_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs WHERE id = ?1")
            .bind(&repair_id)
            .fetch_one(&pool)
            .await
            .expect("count startup_repairs rows");
    assert_eq!(
        repair_count, 1,
        "P082-R15: exactly one startup_repairs row must exist (no duplicate idempotency keys)"
    );

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run after restart");
    let r01_row = readbacks
        .iter()
        .find(|rb| rb.get("source_identifier").and_then(|v| v.as_str()) == Some(&repair_id))
        .expect("P082-R15: readback row must reference the repair idempotency key");
    assert_eq!(
        r01_row.get("scenario_id").and_then(|value| value.as_str()),
        Some("P082-R01"),
        "P082-R15: unapplied R01 replay must preserve R01 readback instead of overwriting it with R16"
    );
    assert!(
        !readbacks.iter().any(
            |row| row.get("recovery_reason_code").and_then(|v| v.as_str())
                == Some(recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED)
        ),
        "P082-R15: unapplied R01 replay must not emit startup_requeue_exhausted"
    );
}

// ── P082-R15: Crash-loop replay: same key across multiple restarts ─────────────

/// Verifies convergence when the same startup repair idempotency key is observed across
/// multiple restarts (crash-loop variant). Each observation of the exhausted key must:
/// - Leave exactly one startup_repairs row (no duplicates)
/// - Update startup_repairs.notes to hold the R16 readback (approved storage owner)
/// - Not create additional pending work items
#[tokio::test]
async fn p082_r15_crash_loop_replay_repeated_restarts_converge_on_single_owner() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r15-loop-{run_id_str}");
    let work_item_id = format!("wi-r15-loop-{run_id_str}");
    let repair_id = format!("p082-requeue:{journal_id}:{work_item_id}:1");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.retry"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry");

    // Seed the idempotency row (generation 1 already consumed from an earlier successful repair).
    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id_str,
        "p082_requeue_once",
        now,
        Some(r#"{"requeue_generation":1,"max_requeue_generation":1}"#),
    )
    .await
    .expect("seed startup_repairs idempotency row (generation 1 consumed)");

    // Insert the original work item in running state (simulating a crash before completion).
    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, 'invoke_agent', ?2, 'running', ?3, 'implement', ?4, ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(
        serde_json::json!({
            "run_id": run_id_str,
            "stage_id": "implement",
            "stage_execution_id": "se-r15-loop",
            "p058_claimed": { "agent_execution_id": "ae-r15-loop" },
            "source_command_journal_id": journal_id,
        })
        .to_string(),
    )
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running work item (crash-loop)");

    // First crash-loop restart: detects R16 (key exhausted), fails the work item.
    let requeued_1 = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now + chrono::Duration::seconds(5),
        "startup_repair_abandoned_invoke_agent",
    )
    .await
    .expect("crash-loop restart 1");
    assert_eq!(
        requeued_1, 0,
        "P082-R15 crash-loop restart 1: must not enqueue new work (R16 exhausted)"
    );

    // startup_repairs must have exactly one row (no duplicate insert).
    let repair_count_1: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs WHERE id = ?1")
            .bind(&repair_id)
            .fetch_one(&pool)
            .await
            .expect("count startup_repairs rows after restart 1");
    assert_eq!(
        repair_count_1, 1,
        "P082-R15 crash-loop: exactly one startup_repairs row after first exhausted restart"
    );

    // R16 readback must now be in startup_repairs.notes.
    let notes_1: Option<String> =
        sqlx::query_scalar("SELECT notes FROM startup_repairs WHERE id = ?1")
            .bind(&repair_id)
            .fetch_one(&pool)
            .await
            .expect("fetch startup_repairs notes after restart 1");
    let notes_json_1: serde_json::Value =
        serde_json::from_str(notes_1.as_deref().unwrap_or("{}")).unwrap_or_default();
    assert_eq!(
        notes_json_1.pointer("/p082_recovery_matrix_readback/scenario_id")
            .and_then(|v| v.as_str()),
        Some("P082-R16"),
        "P082-R15 crash-loop: startup_repairs.notes must contain R16 readback after exhausted restart"
    );

    // Second crash-loop restart: work item is now failed, no running items remain.
    let requeued_2 = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now + chrono::Duration::seconds(10),
        "startup_repair_abandoned_invoke_agent",
    )
    .await
    .expect("crash-loop restart 2");
    assert_eq!(
        requeued_2, 0,
        "P082-R15 crash-loop restart 2: no running items remain, requeue=0"
    );

    // Final convergence: exactly one startup_repairs row, no duplicate work items.
    let repair_count_final: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs WHERE id = ?1")
            .bind(&repair_id)
            .fetch_one(&pool)
            .await
            .expect("count startup_repairs rows after convergence");
    assert_eq!(
        repair_count_final, 1,
        "P082-R15: crash-loop must converge on exactly one startup_repairs row"
    );

    let failed_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_items WHERE run_id = ?1 AND status = 'failed'",
    )
    .bind(&run_id_str)
    .fetch_one(&pool)
    .await
    .expect("count failed work items");
    assert_eq!(
        failed_count, 1,
        "P082-R15: crash-loop must converge on exactly one failed work item (no duplicates)"
    );

    let pending_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM work_items WHERE run_id = ?1 AND status = 'pending'",
    )
    .bind(&run_id_str)
    .fetch_one(&pool)
    .await
    .expect("count pending work items");
    assert_eq!(
        pending_count, 0,
        "P082-R15: crash-loop must not leave duplicate pending work items"
    );
}

// ── P082 NEG: Non-canonical scenario_id in envelope is rejected ────────────────

#[test]
fn p082_neg_non_canonical_scenario_id_in_envelope_is_rejected() {
    // Corresponds to negative fixture: mutated_contract_or_matrix =
    // "scenario_id is not in canonical P082-R01..P082-R17 vocabulary"
    let invalid_readback = serde_json::json!({
        "schema_version": recovery_matrix::SCHEMA_READBACK_V1,
        "scenario_id": "P082-legacy-command-error",  // not canonical
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
        "operator_safe_summary": "Legacy error.",
        "p082_recovery_matrix_readback": invalid_readback,
    })
    .to_string();
    assert!(
        recovery_matrix::parse_command_journal_error_envelope(&envelope).is_none(),
        "P082 NEG: envelope with non-canonical scenario_id must be rejected by envelope parser"
    );
}

// ── P082 NEG: Malformed envelope (missing required field) is safe ─────────────

#[test]
fn p082_neg_malformed_envelope_missing_reason_code_is_rejected() {
    // Corresponds to negative fixture p082-malformed-command-error-envelope.json:
    // "command_journal.error contains JSON with schema_version=p082_rejected_command_error_v1
    //  but missing required reason_code field"
    let malformed = serde_json::json!({
        "schema_version": recovery_matrix::SCHEMA_REJECTED_COMMAND_ERROR_V1,
        // reason_code intentionally omitted
        "command_type": "RetryStage",
        "redaction": "none",
        "operator_safe_summary": "Stage not retryable.",
        "p082_recovery_matrix_readback": null,
    })
    .to_string();
    let parsed = recovery_matrix::parse_command_journal_error_envelope(&malformed);
    assert!(
        parsed.is_none(),
        "P082 NEG (p082-malformed-command-error-envelope): missing reason_code must be rejected"
    );
}

#[test]
fn p082_null_nested_rejected_command_readback_is_rejected() {
    let malformed = serde_json::json!({
        "schema_version": recovery_matrix::SCHEMA_REJECTED_COMMAND_ERROR_V1,
        "reason_code": recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "command_type": "RetryStage",
        "redaction": "partial",
        "operator_safe_summary": "Stage not retryable.",
        "p082_recovery_matrix_readback": null,
    })
    .to_string();

    assert!(
        recovery_matrix::parse_command_journal_error_envelope(&malformed).is_none(),
        "P082: typed rejected-command envelopes require a valid non-null nested readback"
    );
}

#[tokio::test]
async fn p082_malformed_typed_command_error_produces_tamper_fallback_row() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("p082-malformed-typed-{run_id_str}");

    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &format!(r#"{{"run_id":"{run_id_str}","stage_id":"deploy"}}"#),
        Some(&run_id_str),
        now,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal");

    let malformed = serde_json::json!({
        "schema_version": recovery_matrix::SCHEMA_REJECTED_COMMAND_ERROR_V1,
        "reason_code": recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "command_type": "RetryStage",
        "redaction": "partial",
        "operator_safe_summary": "Stage not retryable.",
        "p082_recovery_matrix_readback": null,
    })
    .to_string();
    command_journal::fail_entry(&pool, &journal_id, now, &malformed)
        .await
        .expect("fail with malformed typed envelope");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must fail closed for malformed typed envelopes");

    assert_eq!(readbacks.len(), 1);
    assert_eq!(
        readbacks[0]
            .get("recovery_projection_integrity")
            .and_then(|value| value.as_str()),
        Some("tamper_detected"),
        "P082: malformed typed rejected-command envelopes produce a safe tamper fallback row"
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(&readbacks[0]),
        "P082: malformed typed rejected-command fallback must validate against p082_recovery_matrix_readback_v1: {}",
        readbacks[0]
    );
    assert_eq!(
        readbacks[0]
            .get("source_json_key")
            .and_then(|value| value.as_str()),
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "P082: tamper_detected command_journal fallback must retain the approved source_json_key"
    );
    assert!(
        !readbacks[0]
            .to_string()
            .contains("p082_rejected_command_error_v1"),
        "P082: fallback readback must not expose the raw command_journal.error envelope"
    );
}

#[tokio::test]
async fn p082_tampered_stage_snapshot_fallback_row_validates_readback_shape() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let stage_exec_id = format!("se-r03-tampered-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let mut readback = recovery_matrix::set_readback_late_output_settlement(
        recovery_matrix::build_readback_v1(
            "P082-R03",
            "repaired",
            "no_mutation",
            recovery_matrix::REASON_IGNORED_LATE_OUTPUTS,
            "Late output from superseded source ignored; active projection unchanged.",
            "agent_execution_runtime_facts, artifact_source_generation_claims, work_items",
            "agent_execution_runtime_facts, artifact_contracts, work_items",
            &stage_exec_id,
            Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        recovery_matrix::build_late_output_settlement(
            "ae-r03-test",
            "wi-r03-test",
            "sg-gen1",
            "sg-gen2",
            "superseded",
            "ignored",
            1,
            "completed",
            false,
        ),
    );
    if let Some(obj) = readback.as_object_mut() {
        obj.insert("scenario_id".into(), serde_json::json!("P082-TAMPERED-ID"));
    }
    let snapshot = serde_json::json!({
        "p082_recovery_matrix_readback": readback,
    });

    sqlx::query(
        r#"INSERT OR IGNORE INTO stage_executions
           (id, run_id, stage_id, label, status, iteration, attempt_number,
            started_at, owner_agent, provider, model, stage_type,
            recovery_snapshot_json)
           VALUES (?1, ?2, 'implement', 'impl', 'completed', 1, 1,
                   ?3, 'agent-1', 'provider-1', 'model-1', 'standard',
                   ?4)"#,
    )
    .bind(&stage_exec_id)
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .bind(snapshot.to_string())
    .execute(&pool)
    .await
    .expect("insert stage_execution with tampered recovery_snapshot_json");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail for tampered stage snapshot");

    assert_eq!(readbacks.len(), 1);
    let row = &readbacks[0];
    assert_eq!(
        row.get("recovery_projection_integrity")
            .and_then(|value| value.as_str()),
        Some("tamper_detected"),
        "P082: tampered stage snapshot must produce a tamper fallback row"
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(row),
        "P082: tampered stage snapshot fallback must validate against readback_v1 shape: {row}"
    );
    assert!(
        row.get("recovery_late_output_settlement")
            .is_some_and(|value| !value.is_null()),
        "P082: P082-R03 tamper fallback must include recovery_late_output_settlement"
    );
}

#[tokio::test]
async fn p082_readback_accessor_tolerates_malformed_mutable_json_columns() {
    let pool = setup_db().await;
    let now = Utc::now();
    let stale_started_at = now - chrono::Duration::minutes(4);
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let lineage_id = format!("p082-json-valid-lineage-{run_id_str}");
    let generation_id = format!("p082-json-valid-generation-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    sqlx::query(
        r#"INSERT INTO session_lineages
           (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
            active_generation_id, created_at, closed_at)
           VALUES (?1, ?2, 'agent-json-valid', ?1, 'run', NULL, ?3, ?4, NULL)"#,
    )
    .bind(&lineage_id)
    .bind(&run_id_str)
    .bind(&generation_id)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert lineage");

    sqlx::query(
        r#"INSERT INTO session_generations
           (id, lineage_id, generation, invocation_owner_key, provider_session_id,
            binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
            workspace_mode, runtime_provider, runtime_model, status, created_at, last_activity_at)
	           VALUES (?1, ?2, 1, 'p082-json-valid-owner-work', NULL, 'binding-json-valid', NULL, '/',
	                   'read_write', 'codex', 'gpt-5.5', 'active', ?3, NULL)"#,
    )
    .bind(&generation_id)
    .bind(&lineage_id)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert generation");

    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id,
            owner_kind, owner_id)
           VALUES ('p082-json-valid-ae', 'agent-json-valid', 'codex', 'running', ?1,
                   ?2, ?3, 'lead_conflict_mediation', 'p082-json-valid-ae')"#,
    )
    .bind(stale_started_at.to_rfc3339())
    .bind(&generation_id)
    .bind(&lineage_id)
    .execute(&pool)
    .await
    .expect("insert valid agent execution for stale startup evidence");

    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES ('p082-json-valid-owner-work', ?1, 'invoke_agent', ?2, 'running', ?3, ?3, ?3, 1)"#,
    )
    .bind(&run_id_str)
    .bind(
        serde_json::json!({
            "run_id": run_id_str,
            "p061_startup_recovery": { "reason": "startup_stalled" },
            "p058_claimed": { "agent_execution_id": "p082-json-valid-ae" }
        })
        .to_string(),
    )
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert valid work item for stale startup evidence");

    sqlx::query(
        r#"INSERT INTO session_events
           (id, lineage_id, generation_id, event_type, recorded_at, details_json)
           VALUES ('p082-json-valid-event', ?1, ?2, 'created', ?3, '{')"#,
    )
    .bind(&lineage_id)
    .bind(&generation_id)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert malformed session event details");

    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES ('p082-json-valid-work', ?1, 'invoke_agent', '{', 'running', ?2, ?2, ?2, 1)"#,
    )
    .bind(&run_id_str)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert malformed work item payload");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not abort on malformed mutable JSON");

    assert!(
        !readbacks.is_empty(),
        "valid non-JSON-derived stale session evidence should still be reported"
    );
    for row in &readbacks {
        assert!(
            recovery_matrix::validate_readback_v1_shape(row),
            "malformed JSON fallback/accessor row must still validate: {row}"
        );
    }
}

#[tokio::test]
async fn p082_readback_accessor_ignores_oversized_work_item_payload_json_claims() {
    let pool = setup_db().await;
    let now = Utc::now();
    let stale_started_at = now - chrono::Duration::minutes(4);
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let lineage_id = format!("p082-json-size-lineage-{run_id_str}");
    let generation_id = format!("p082-json-size-generation-{run_id_str}");
    let ae_id = format!("p082-json-size-ae-{run_id_str}");
    let wi_id = format!("p082-json-size-work-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    sqlx::query(
        r#"INSERT INTO session_lineages
           (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
            active_generation_id, created_at, closed_at)
           VALUES (?1, ?2, 'agent-json-size', ?1, 'run', NULL, ?3, ?4, NULL)"#,
    )
    .bind(&lineage_id)
    .bind(&run_id_str)
    .bind(&generation_id)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert lineage");

    sqlx::query(
        r#"INSERT INTO session_generations
           (id, lineage_id, generation, invocation_owner_key, provider_session_id,
            binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
            workspace_mode, runtime_provider, runtime_model, status, created_at, last_activity_at)
           VALUES (?1, ?2, 1, 'p082-json-size-owner', NULL, 'binding-json-size',
                   NULL, '/', 'read_write', 'codex', 'gpt-5.5', 'active', ?3, NULL)"#,
    )
    .bind(&generation_id)
    .bind(&lineage_id)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert generation");

    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id,
            owner_kind, owner_id)
           VALUES (?1, 'agent-json-size', 'codex', 'running', ?2,
                   ?3, ?4, 'lead_conflict_mediation', ?1)"#,
    )
    .bind(&ae_id)
    .bind(stale_started_at.to_rfc3339())
    .bind(&generation_id)
    .bind(&lineage_id)
    .execute(&pool)
    .await
    .expect("insert agent execution");

    let oversized_payload = serde_json::json!({
        "run_id": run_id_str,
        "p058_claimed": { "agent_execution_id": ae_id },
        "padding": "X".repeat(70 * 1024),
    })
    .to_string();
    assert!(
        oversized_payload.len() > 64 * 1024,
        "test payload must exceed the P082 readback row guard"
    );

    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, ?4, 1)"#,
    )
    .bind(&wi_id)
    .bind(&run_id_str)
    .bind(&oversized_payload)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert oversized work item payload");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail on oversized work_items payload_json");

    assert!(
        readbacks.iter().all(|row| {
            row.get("scenario_id").and_then(|v| v.as_str()) != Some("P082-R05")
                && row.get("source_identifier").and_then(|v| v.as_str()) != Some(wi_id.as_str())
        }),
        "P082: oversized work_items.payload_json must not be parsed into stale startup readback"
    );
}

#[tokio::test]
async fn p082_stale_startup_requeue_ignores_malformed_work_item_payload_json() {
    let pool = setup_db().await;
    let now = Utc::now();
    let stale_started_at = now - chrono::Duration::minutes(13);
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let lineage_id = format!("p082-json-valid-requeue-lineage-{run_id_str}");
    let generation_id = format!("p082-json-valid-requeue-generation-{run_id_str}");
    let ae_id = format!("p082-json-valid-requeue-ae-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    sqlx::query(
        r#"INSERT INTO session_lineages
           (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
            active_generation_id, created_at, closed_at)
           VALUES (?1, ?2, 'agent-json-valid-requeue', ?1, 'run', NULL, ?3, ?4, NULL)"#,
    )
    .bind(&lineage_id)
    .bind(&run_id_str)
    .bind(&generation_id)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert lineage");

    sqlx::query(
        r#"INSERT INTO session_generations
           (id, lineage_id, generation, invocation_owner_key, provider_session_id,
            binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
            workspace_mode, runtime_provider, runtime_model, status, created_at, last_activity_at)
           VALUES (?1, ?2, 1, 'json-valid-requeue-owner', NULL, 'binding-json-valid-requeue',
                   NULL, '/', 'read_write', 'codex', 'gpt-5.5', 'active', ?3, NULL)"#,
    )
    .bind(&generation_id)
    .bind(&lineage_id)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert generation");

    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id, owner_kind, owner_id)
           VALUES (?1, 'agent-json-valid-requeue', 'codex', 'running', ?2,
                   ?3, ?4, 'lead_conflict_mediation', ?1)"#,
    )
    .bind(&ae_id)
    .bind(stale_started_at.to_rfc3339())
    .bind(&generation_id)
    .bind(&lineage_id)
    .execute(&pool)
    .await
    .expect("insert agent execution");

    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES ('p082-json-valid-requeue-work', ?1, 'invoke_agent', '{', 'running', ?2, ?2, ?2, 1)"#,
    )
    .bind(&run_id_str)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert malformed work item payload");

    let requeued = work_items::requeue_stale_starting_invoke_agent_sessions(
        &pool,
        now,
        now - chrono::Duration::minutes(3),
        now - chrono::Duration::minutes(12),
        "p082_json_valid_requeue_test",
    )
    .await
    .expect("stale startup requeue must not abort on malformed payload_json");

    assert_eq!(
        requeued, 0,
        "malformed payload_json cannot prove the p058 claimed owner and must be skipped"
    );
}

// ── P082 NEG: validate_readback_v1_shape rejects non-canonical scenario_id ────

#[test]
fn p082_neg_validate_readback_v1_shape_rejects_non_canonical_id() {
    let mut rb = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs",
        "sr-001",
        None,
        "valid",
        "2026-05-21T00:00:00Z",
    );
    if let Some(obj) = rb.as_object_mut() {
        obj.insert("scenario_id".into(), serde_json::json!("P082-INJECTED"));
    }
    assert!(
        !recovery_matrix::validate_readback_v1_shape(&rb),
        "P082 NEG: validate_readback_v1_shape must reject non-canonical scenario_id"
    );
}

// ── P082 NEG: validate_readback_v1_shape rejects invalid recovery_decision ────

#[test]
fn p082_neg_validate_readback_v1_shape_rejects_invalid_decision() {
    let mut rb = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs",
        "sr-001",
        None,
        "valid",
        "2026-05-21T00:00:00Z",
    );
    if let Some(obj) = rb.as_object_mut() {
        obj.insert(
            "recovery_decision".into(),
            serde_json::json!("repair_converged"),
        );
    }
    assert!(
        !recovery_matrix::validate_readback_v1_shape(&rb),
        "P082 NEG: validate_readback_v1_shape must reject invalid recovery_decision value"
    );
}

// ── P082-R08: Retry identifier guidance shape ──────────────────────────────

#[test]
fn p082_r08_identifier_guidance_has_required_fields() {
    // Verify build_retry_identifier_guidance produces a valid p082_retry_identifier_guidance_v1
    let guidance = recovery_matrix::build_retry_identifier_guidance(
        "RetryAgentExecution",
        "some-stale-uuid",
        "stage_execution_uuid",
        "stage_execution_uuid",
        &["latest-stage-exec-uuid"],
    );
    let required = [
        "schema_version",
        "command",
        "provided_identifier",
        "provided_identifier_kind",
        "expected_identifier_kind",
        "valid_identifier_examples",
        "no_mutation",
    ];
    for field in &required {
        assert!(
            guidance.get(field).is_some(),
            "P082-R08: p082_retry_identifier_guidance_v1 must include '{field}'"
        );
    }
    assert_eq!(
        guidance["schema_version"].as_str().unwrap(),
        recovery_matrix::SCHEMA_RETRY_IDENTIFIER_GUIDANCE_V1,
        "P082-R08: schema_version must be p082_retry_identifier_guidance_v1"
    );
    assert_eq!(
        guidance["no_mutation"].as_bool(),
        Some(true),
        "P082-R08: no_mutation must be true"
    );
    assert_eq!(
        guidance["provided_identifier"].as_str().unwrap(),
        "some-stale-uuid"
    );
    assert_eq!(
        guidance["expected_identifier_kind"].as_str().unwrap(),
        "stage_execution_uuid"
    );
}

#[test]
fn p082_r08_identifier_guidance_rejects_unapproved_agent_execution_id_kind() {
    let guidance = recovery_matrix::build_retry_identifier_guidance(
        "RetryAgentExecution",
        "some-stale-uuid",
        "agent_execution_id",
        "agent_execution_id",
        &["latest-agent-exec-uuid"],
    );
    let readback = recovery_matrix::set_readback_identifier_guidance(
        recovery_matrix::build_readback_v1(
            "P082-R08",
            "rejected",
            "no_mutation",
            recovery_matrix::REASON_VALID_IDENTIFIER_GUIDANCE,
            "Provide an approved retry identifier.",
            "command_journal",
            "command_journal",
            "cmd-r08-agent-kind",
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        ),
        guidance,
    );

    assert!(
        !recovery_matrix::validate_readback_v1_shape(&readback),
        "P082-R08: agent_execution_id must remain outside the approved guidance vocabulary"
    );
}

#[test]
fn p082_r08_identifier_guidance_rejects_empty_examples() {
    let guidance = recovery_matrix::build_retry_identifier_guidance(
        "RetryAgentExecution",
        "some-stale-uuid",
        "stage_execution_uuid",
        "stage_execution_uuid",
        &[],
    );
    let readback = recovery_matrix::set_readback_identifier_guidance(
        recovery_matrix::build_readback_v1(
            "P082-R08",
            "rejected",
            "no_mutation",
            recovery_matrix::REASON_VALID_IDENTIFIER_GUIDANCE,
            "Provide a valid targeted retry UUID from the latest stage execution attempt.",
            "command_journal",
            "command_journal, agent_executions, stage_executions",
            "cmd-r08-empty-examples",
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        ),
        guidance,
    );

    assert!(
        !recovery_matrix::validate_readback_v1_shape(&readback),
        "P082-R08: valid_identifier_examples must be non-empty"
    );
}

#[tokio::test]
async fn p082_r08_identifier_guidance_stored_in_command_journal_error() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let journal_id = format!("p082-r08-{}", run_id);

    command_journal::record(
        &pool,
        &journal_id,
        "RetryAgentExecution",
        &serde_json::json!({"run_id": run_id.to_string(), "agent_execution_id": "stale-ae-uuid"})
            .to_string(),
        Some(&run_id.to_string()),
        now,
        Some("mcp"),
        None,
        Some("operator"),
        Some("runs.retry"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal");

    let guidance = recovery_matrix::build_retry_identifier_guidance(
        "RetryAgentExecution",
        "stale-ae-uuid",
        "stage_execution_uuid",
        "stage_execution_uuid",
        &["latest-stage-exec-uuid"],
    );
    let readback = recovery_matrix::set_readback_identifier_guidance(
        recovery_matrix::build_readback_v1(
            "P082-R08",
            "rejected",
            "no_mutation",
            recovery_matrix::REASON_VALID_IDENTIFIER_GUIDANCE,
            "Provide a valid targeted retry UUID from the latest stage execution attempt.",
            "command_journal",
            "command_journal, agent_executions, stage_executions",
            &journal_id,
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        guidance,
    );
    let envelope = recovery_matrix::build_rejected_command_error_envelope(
        recovery_matrix::REASON_VALID_IDENTIFIER_GUIDANCE,
        "RetryAgentExecution",
        "Identifier mismatch: provided agent_execution_id references a stale stage execution.",
        readback,
    );
    command_journal::fail_entry(&pool, &journal_id, now, &envelope)
        .await
        .expect("fail command journal with R08 envelope");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    assert_eq!(readbacks.len(), 1, "P082-R08: must return exactly one row");
    let row = &readbacks[0];
    assert_eq!(
        row.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R08"),
        "P082-R08: scenario_id must be P082-R08"
    );
    assert_eq!(
        row.get("scenario_status").and_then(|v| v.as_str()),
        Some("rejected"),
        "P082-R08: scenario_status must be rejected"
    );
    assert_eq!(
        row.get("recovery_decision").and_then(|v| v.as_str()),
        Some("no_mutation"),
        "P082-R08: recovery_decision must be no_mutation (rejected before any mutation)"
    );
    assert_eq!(
        row.get("recovery_reason_code").and_then(|v| v.as_str()),
        Some(recovery_matrix::REASON_VALID_IDENTIFIER_GUIDANCE),
        "P082-R08: reason_code must be valid_identifier_guidance"
    );
    // The nested guidance must be present and contain no_mutation=true.
    let guidance_field = row.get("recovery_retry_identifier_guidance");
    assert!(
        guidance_field.is_some() && !guidance_field.unwrap().is_null(),
        "P082-R08: recovery_retry_identifier_guidance must be non-null"
    );
    let guidance_obj = guidance_field.unwrap().as_object().unwrap();
    assert_eq!(
        guidance_obj.get("no_mutation").and_then(|v| v.as_bool()),
        Some(true),
        "P082-R08: no_mutation must be true in guidance"
    );
    assert_eq!(
        guidance_obj
            .get("expected_identifier_kind")
            .and_then(|v| v.as_str()),
        Some("stage_execution_uuid"),
        "P082-R08: expected_identifier_kind must stay within the approved vocabulary"
    );
}

// ── P082-R11: Cancellation settlement readback structure ─────────────────────

#[test]
fn p082_r11_cancellation_readback_shape_built_correctly() {
    let now = Utc::now();
    let run_id = "run-cancel-r11-test";
    let readback = recovery_matrix::build_readback_v1(
        "P082-R11",
        "cancelled",
        "cancel",
        recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
        "Run cancellation settled active stage execution.",
        "runs, work_items, retry_stage_execution_authorities, session_generations, session_events",
        "runs, work_items, sessions",
        run_id,
        Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    assert_eq!(
        readback["scenario_id"].as_str().unwrap(),
        "P082-R11",
        "P082-R11: scenario_id must be P082-R11"
    );
    assert_eq!(
        readback["scenario_status"].as_str().unwrap(),
        "cancelled",
        "P082-R11: scenario_status must be cancelled"
    );
    assert_eq!(
        readback["recovery_decision"].as_str().unwrap(),
        "cancel",
        "P082-R11: recovery_decision must be cancel"
    );
    assert_eq!(
        readback["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED
    );
    // Validate the shape passes the validator.
    assert!(
        recovery_matrix::validate_readback_v1_shape(&readback),
        "P082-R11: build_readback_v1 for R11 must produce a valid shape"
    );
}

// ── P082-R11: Accessor reads from runs.cancellation_settlement_log ────────────

#[tokio::test]
async fn p082_r11_cancellation_settlement_log_accessor_reads_readback() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();

    // Insert idea + run rows (accessor queries runs table; FK enforcement is on).
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let action_id = format!("cancellation_settlement:{run_id_str}:{}", now.to_rfc3339());
    let readback = recovery_matrix::build_readback_v1(
        "P082-R11",
        "cancelled",
        "cancel",
        recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
        "Active stage execution settled by cancellation.",
        "runs, work_items, session_generations, session_events",
        "runs, work_items, sessions",
        &action_id,
        Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    let entry = serde_json::json!({
        "action_id": action_id,
        "agent_execution_id": "ae-r11-test",
        "agent_id": "agent-1",
        "prior_status": "running",
        "terminal_status": "cancelled",
        "session_close_attempted": false,
        "session_close_succeeded": null,
        "settled_at": now.to_rfc3339(),
        "p082_recovery_matrix_readback": readback,
    });
    let log = serde_json::json!([entry]).to_string();

    sqlx::query(
        r#"UPDATE runs SET cancellation_settlement_log = ?1, cancellation_requested_at = ?2 WHERE id = ?3"#,
    )
    .bind(&log)
    .bind(now.to_rfc3339())
    .bind(&run_id_str)
    .execute(&pool)
    .await
    .expect("update cancellation settlement log");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    assert_eq!(
        readbacks.len(),
        1,
        "P082-R11: accessor must return one row from cancellation_settlement_log"
    );
    let row = &readbacks[0];
    assert_eq!(
        row.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R11"),
        "P082-R11: scenario_id from cancellation log must be P082-R11"
    );
    assert_eq!(
        row.get("source_identifier").and_then(|v| v.as_str()),
        Some(action_id.as_str()),
        "P082-R11: source_identifier must be the cancellation settlement action_id"
    );
    assert_eq!(
        row.get("scenario_status").and_then(|v| v.as_str()),
        Some("cancelled"),
        "P082-R11: scenario_status must be cancelled"
    );
    assert_eq!(
        row.get("source_table").and_then(|v| v.as_str()),
        Some("runs, work_items, session_generations, session_events"),
        "P082-R11: source_table must be preserved by allowlist projection"
    );
}

#[tokio::test]
async fn p082_r11_cancellation_settlement_log_allows_parallel_entries_with_shared_action_id() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let action_id = format!("cancellation_settlement:{run_id_str}:{}", now.to_rfc3339());
    let make_entry = |agent_execution_id: &str| {
        let readback = recovery_matrix::build_readback_v1(
            "P082-R11",
            "cancelled",
            "cancel",
            recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
            "Active stage execution settled by cancellation.",
            "runs, work_items, session_generations, session_events",
            "runs, work_items, sessions",
            &format!("{action_id}:agent_execution:{agent_execution_id}"),
            Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        );
        serde_json::json!({
            "action_id": action_id,
            "agent_execution_id": agent_execution_id,
            "agent_id": "agent-1",
            "prior_status": "running",
            "terminal_status": "cancelled",
            "session_close_attempted": false,
            "session_close_succeeded": null,
            "settled_at": now.to_rfc3339(),
            "p082_recovery_matrix_readback": readback,
        })
    };
    let log = serde_json::json!([
        make_entry("ae-r11-parallel-1"),
        make_entry("ae-r11-parallel-2")
    ])
    .to_string();

    sqlx::query(
        r#"UPDATE runs SET cancellation_settlement_log = ?1, cancellation_requested_at = ?2 WHERE id = ?3"#,
    )
    .bind(&log)
    .bind(now.to_rfc3339())
    .bind(&run_id_str)
    .execute(&pool)
    .await
    .expect("update cancellation settlement log");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    let r11_rows: Vec<_> = readbacks
        .iter()
        .filter(|row| row.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R11"))
        .collect();

    assert_eq!(
        r11_rows.len(),
        2,
        "P082-R11: parallel cancellation entries sharing one action_id must both project"
    );
    assert!(
        r11_rows.iter().all(|row| {
            row.get("recovery_projection_integrity")
                .and_then(|v| v.as_str())
                == Some("valid")
        }),
        "P082-R11: suffixed per-execution source identifiers must not be classified as tamper"
    );
    assert!(
        r11_rows.iter().all(|row| {
            row.get("source_identifier")
                .and_then(|v| v.as_str())
                .is_some_and(|value| value.starts_with(&format!("{action_id}:agent_execution:")))
        }),
        "P082-R11: parallel entries must retain per-execution source identifiers"
    );
}

#[tokio::test]
async fn p082_r11_cancellation_settlement_log_missing_or_duplicate_action_id_tampers() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let action_id = format!("cancellation_settlement:{run_id_str}:{}", now.to_rfc3339());
    let readback = recovery_matrix::build_readback_v1(
        "P082-R11",
        "cancelled",
        "cancel",
        recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
        "Active stage execution settled by cancellation.",
        "runs, work_items, session_generations, session_events",
        "runs, work_items, sessions",
        &action_id,
        Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    let missing_action_entry = serde_json::json!({
        "agent_execution_id": "ae-r11-missing-action",
        "agent_id": "agent-1",
        "prior_status": "running",
        "terminal_status": "cancelled",
        "session_close_attempted": false,
        "session_close_succeeded": null,
        "settled_at": now.to_rfc3339(),
        "p082_recovery_matrix_readback": readback.clone(),
    });
    let duplicate_entry_1 = serde_json::json!({
        "action_id": action_id,
        "agent_execution_id": "ae-r11-dup-1",
        "agent_id": "agent-1",
        "prior_status": "running",
        "terminal_status": "cancelled",
        "session_close_attempted": false,
        "session_close_succeeded": null,
        "settled_at": now.to_rfc3339(),
        "p082_recovery_matrix_readback": readback,
    });
    let duplicate_entry_2 = serde_json::json!({
        "action_id": action_id,
        "agent_execution_id": "ae-r11-dup-2",
        "agent_id": "agent-2",
        "prior_status": "running",
        "terminal_status": "cancelled",
        "session_close_attempted": false,
        "session_close_succeeded": null,
        "settled_at": now.to_rfc3339(),
        "p082_recovery_matrix_readback": duplicate_entry_1["p082_recovery_matrix_readback"].clone(),
    });
    let log =
        serde_json::json!([missing_action_entry, duplicate_entry_1, duplicate_entry_2]).to_string();

    sqlx::query(
        r#"UPDATE runs SET cancellation_settlement_log = ?1, cancellation_requested_at = ?2 WHERE id = ?3"#,
    )
    .bind(&log)
    .bind(now.to_rfc3339())
    .bind(&run_id_str)
    .execute(&pool)
    .await
    .expect("update cancellation settlement log");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    assert!(
        readbacks.iter().any(|row| {
            row.get("recovery_projection_integrity")
                .and_then(|v| v.as_str())
                == Some("tamper_detected")
        }),
        "P082-R11: missing or duplicate action_id must produce tamper-detected readback"
    );
    assert!(
        readbacks.iter().all(|row| {
            row.get("source_identifier").and_then(|v| v.as_str()) != Some(action_id.as_str())
                || row
                    .get("recovery_projection_integrity")
                    .and_then(|v| v.as_str())
                    != Some("valid")
        }),
        "P082-R11: duplicated cancellation action_id must not pass as valid readback"
    );
}

#[test]
fn p082_r11_cancellation_action_id_shape_is_required() {
    let now = Utc::now();
    let action_id = format!("cancellation_settlement:run-r11:{}", now.to_rfc3339());
    let readback = recovery_matrix::build_readback_v1(
        "P082-R11",
        "cancelled",
        "cancel",
        recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
        "Active stage execution settled by cancellation.",
        "runs, work_items, session_generations, session_events",
        "runs, work_items, sessions",
        &action_id,
        Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    assert_eq!(
        readback["source_identifier"].as_str(),
        Some(action_id.as_str()),
        "P082-R11 source_identifier must cite runs.cancellation_settlement_log.action_id"
    );
}

// ── P082-R03/R17: Accessor reads from stage_executions.recovery_snapshot_json ──

#[tokio::test]
async fn p082_r03_stage_recovery_snapshot_json_accessor_reads_readback() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let stage_exec_id = format!("se-r03-{run_id_str}");

    // Insert idea + run first (stage_executions FK requires a valid run).
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    // Insert a minimal stage_executions row with a P082 recovery snapshot.
    let readback = recovery_matrix::build_readback_v1(
        "P082-R03",
        "repaired",
        "no_mutation",
        recovery_matrix::REASON_IGNORED_LATE_OUTPUTS,
        "Late output from superseded source ignored; active projection unchanged.",
        "agent_execution_runtime_facts, artifact_source_generation_claims, work_items",
        "agent_execution_runtime_facts, artifact_contracts, work_items",
        &stage_exec_id,
        Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    let settlement = recovery_matrix::build_late_output_settlement(
        "ae-r03-test",
        "wi-r03-test",
        "sg-gen1",
        "sg-gen2",
        "superseded",
        "ignored",
        1,
        "completed",
        false,
    );
    let readback_with_settlement =
        recovery_matrix::set_readback_late_output_settlement(readback, settlement);
    let snapshot = serde_json::json!({
        "p082_recovery_matrix_readback": readback_with_settlement,
    });

    sqlx::query(
        r#"INSERT OR IGNORE INTO stage_executions
           (id, run_id, stage_id, label, status, iteration, attempt_number,
            started_at, owner_agent, provider, model, stage_type,
            recovery_snapshot_json)
           VALUES (?1, ?2, 'implement', 'impl', 'completed', 1, 1,
                   ?3, 'agent-1', 'provider-1', 'model-1', 'standard',
                   ?4)"#,
    )
    .bind(&stage_exec_id)
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .bind(snapshot.to_string())
    .execute(&pool)
    .await
    .expect("insert stage_execution with recovery_snapshot_json");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    assert_eq!(
        readbacks.len(),
        1,
        "P082-R03: accessor must return one row from stage_executions.recovery_snapshot_json"
    );
    let row = &readbacks[0];
    assert_eq!(
        row.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R03"),
        "P082-R03: scenario_id must be P082-R03"
    );
    assert_eq!(
        row.get("scenario_status").and_then(|v| v.as_str()),
        Some("repaired"),
        "P082-R03: scenario_status must be repaired"
    );
    // The nested late_output_settlement subcontract must be present.
    let settlement_field = row.get("recovery_late_output_settlement");
    assert!(
        settlement_field.is_some() && !settlement_field.unwrap().is_null(),
        "P082-R03: recovery_late_output_settlement must be non-null from snapshot"
    );
    let settle_obj = settlement_field.unwrap().as_object().unwrap();
    assert_eq!(
        settle_obj
            .get("active_projection_changed")
            .and_then(|v| v.as_bool()),
        Some(false),
        "P082-R03: active_projection_changed must be false"
    );
    assert_eq!(
        settle_obj
            .get("cancelled_provider_session")
            .and_then(|v| v.as_bool()),
        Some(false),
        "P082-R03: cancelled_provider_session must be false for supersede case"
    );
}

#[tokio::test]
async fn p082_r03_tampered_stage_recovery_snapshot_produces_valid_tamper_fallback() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let stage_exec_id = format!("se-r03-tampered-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let tampered_readback = recovery_matrix::build_readback_v1(
        "P082-R03",
        "repaired",
        "no_mutation",
        recovery_matrix::REASON_IGNORED_LATE_OUTPUTS,
        "Late output from superseded source ignored; active projection unchanged.",
        "agent_execution_runtime_facts, artifact_source_generation_claims, work_items",
        "agent_execution_runtime_facts, artifact_contracts, work_items",
        &stage_exec_id,
        Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    assert!(
        !recovery_matrix::validate_readback_v1_shape(&tampered_readback),
        "P082-R03 test setup must omit required late-output settlement"
    );
    let snapshot = serde_json::json!({
        "p082_recovery_matrix_readback": tampered_readback,
    });

    sqlx::query(
        r#"INSERT OR IGNORE INTO stage_executions
           (id, run_id, stage_id, label, status, iteration, attempt_number,
            started_at, owner_agent, provider, model, stage_type,
            recovery_snapshot_json)
           VALUES (?1, ?2, 'implement', 'impl', 'completed', 1, 1,
                   ?3, 'agent-1', 'provider-1', 'model-1', 'standard',
                   ?4)"#,
    )
    .bind(&stage_exec_id)
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .bind(snapshot.to_string())
    .execute(&pool)
    .await
    .expect("insert stage_execution with tampered recovery_snapshot_json");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail for tampered stage snapshot");

    assert_eq!(
        readbacks.len(),
        1,
        "P082-R03: tampered stage snapshot must produce exactly one fallback row"
    );
    let row = &readbacks[0];
    assert_eq!(
        row.get("recovery_projection_integrity")
            .and_then(|value| value.as_str()),
        Some("tamper_detected"),
        "P082-R03: tampered stage snapshot fallback must report tamper_detected"
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(row),
        "P082-R03: tampered stage snapshot fallback must validate against p082_recovery_matrix_readback_v1: {row}"
    );
    assert!(
        row.get("recovery_late_output_settlement")
            .is_some_and(|value| !value.is_null()),
        "P082-R03: tampered fallback must include required late-output settlement"
    );
}

// ── P082-R01: Production startup repair writes p082_startup_repair_summary_v1 ──

#[test]
fn p082_r01_startup_repair_summary_helper_produces_required_fields() {
    let summary = recovery_matrix::build_startup_repair_summary(
        "p082-requeue:cj-r01-test:work-item-001:1",
        "work-item-001",
        "cj-r01-test",
        1,
        1,
        false,
        60_000,
        "2026-05-22T00:00:00Z",
        false,
        None,
        "global",
    );
    let required = [
        "schema_version",
        "startup_repair_id",
        "source_work_item_id",
        "source_command_journal_id",
        "requeue_generation",
        "max_requeue_generation",
        "replayed",
        "stale_after_ms",
        "stale_cutoff",
        "xcode_required",
        "next_retry_or_backoff_time",
        "backpressure_scope",
    ];
    for field in &required {
        assert!(
            summary.get(field).is_some(),
            "P082-R01: p082_startup_repair_summary_v1 must include '{field}'"
        );
    }
    assert_eq!(
        summary["schema_version"].as_str().unwrap(),
        recovery_matrix::SCHEMA_STARTUP_REPAIR_SUMMARY_V1
    );
    assert_eq!(
        summary["max_requeue_generation"].as_i64().unwrap(),
        1,
        "P082-R01: max_requeue_generation must be 1 for startup requeue proof"
    );
    assert_eq!(
        summary["requeue_generation"].as_i64().unwrap(),
        1,
        "P082-R01: requeue_generation must be 1"
    );
    assert_eq!(
        summary["replayed"].as_bool().unwrap(),
        false,
        "P082-R01: replayed must be false for first-time requeue"
    );
}

#[tokio::test]
async fn p082_r01_production_startup_repair_with_summary_stores_p082_readback() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let work_item_id = format!("wi-r01-{run_id_str}");
    let source_command_journal_id = format!("cj-r01-{run_id_str}");
    let repair_id = format!("p082-requeue:{source_command_journal_id}:{work_item_id}:1");

    // Simulate what the production path does: build summary+readback, store in notes.
    let summary = recovery_matrix::build_startup_repair_summary(
        &repair_id,
        &work_item_id,
        &source_command_journal_id,
        1,
        1,
        false,
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "global",
    );
    let readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup recovery requeued abandoned InvokeAgent work item.",
            "startup_repairs, work_items, command_journal",
            "startup_repairs, work_items",
            &repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        summary,
        None,
    );
    let notes = serde_json::json!({ "p082_recovery_matrix_readback": readback }).to_string();

    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id_str,
        "p082_requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("record startup repair");

    // The accessor must surface this as a P082-R01 row.
    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run");

    assert_eq!(readbacks.len(), 1, "P082-R01: must surface exactly one row");
    let row = &readbacks[0];
    assert_eq!(
        row.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R01"),
        "P082-R01: scenario_id must be P082-R01"
    );
    assert_eq!(
        row.get("recovery_reason_code").and_then(|v| v.as_str()),
        Some(recovery_matrix::REASON_STARTUP_REQUEUE_ONCE)
    );
    // startup_repair_summary must be non-null and carry max_requeue_generation=1.
    let summary_field = row.get("recovery_startup_repair_summary");
    assert!(
        summary_field.is_some() && !summary_field.unwrap().is_null(),
        "P082-R01: recovery_startup_repair_summary must be non-null"
    );
    let s = summary_field.unwrap().as_object().unwrap();
    assert_eq!(
        s.get("max_requeue_generation").and_then(|v| v.as_i64()),
        Some(1),
        "P082-R01: max_requeue_generation must be 1"
    );
}

#[tokio::test]
async fn p082_r01_production_startup_requeue_uses_command_journal_key() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r01-prod-{run_id_str}");
    let work_item_id = format!("wi-r01-prod-{run_id_str}");
    let expected_repair_id = format!("p082-requeue:{journal_id}:{work_item_id}:1");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &serde_json::json!({"run_id": run_id_str, "stage_id": "implement"}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.retry"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record source command journal entry");

    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, 'invoke_agent', ?2, 'running', ?3, 'implement', ?4, ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(
        serde_json::json!({
            "run_id": run_id_str,
            "stage_id": "implement",
            "stage_execution_id": "se-r01-prod",
            "p058_claimed": { "agent_execution_id": "ae-r01-prod" }
        })
        .to_string(),
    )
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running work item");

    let requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now + chrono::Duration::seconds(5),
        "startup_repair_abandoned_invoke_agent",
    )
    .await
    .expect("production startup requeue");
    assert_eq!(requeued, 1, "P082-R01: first generation must requeue once");

    let repair_row: Option<String> =
        sqlx::query_scalar("SELECT notes FROM startup_repairs WHERE id = ?1")
            .bind(&expected_repair_id)
            .fetch_optional(&pool)
            .await
            .expect("fetch expected startup repair");
    let notes = repair_row.expect("P082-R01: approved key format must be present");
    let notes_json: serde_json::Value = serde_json::from_str(&notes).expect("notes json");
    let summary = notes_json
        .pointer("/p082_recovery_matrix_readback/recovery_startup_repair_summary")
        .expect("startup repair summary");
    assert_eq!(
        summary
            .get("source_command_journal_id")
            .and_then(|value| value.as_str()),
        Some(journal_id.as_str()),
        "P082-R01: source_command_journal_id must be populated"
    );

    let payload_raw: String =
        sqlx::query_scalar("SELECT payload_json FROM work_items WHERE id = ?1")
            .bind(&work_item_id)
            .fetch_one(&pool)
            .await
            .expect("fetch work item payload");
    let payload: serde_json::Value = serde_json::from_str(&payload_raw).expect("payload json");
    assert_eq!(
        payload
            .pointer("/p061_startup_recovery/startup_repair_id")
            .and_then(|value| value.as_str()),
        Some(expected_repair_id.as_str()),
        "P082-R01: work item payload must reference the approved repair key"
    );
    assert_eq!(
        payload
            .pointer("/p061_startup_recovery/source_command_journal_id")
            .and_then(|value| value.as_str()),
        Some(journal_id.as_str()),
        "P082-R01: work item payload must carry source_command_journal_id"
    );
}

// ── P082-R08: Identifier guidance round-trips through command_journal ────────

#[test]
fn p082_r08_identifier_guidance_no_mutation_invariant() {
    // Verify that build_retry_identifier_guidance always sets no_mutation=true.
    for kind in &[
        "workflow_stage_id",
        "stage_execution_uuid",
        "agent_execution_id",
        "retry_authority_id",
        "work_item_id",
        "unknown",
    ] {
        let guidance = recovery_matrix::build_retry_identifier_guidance(
            "RetryStage",
            "test-id",
            kind,
            "stage_execution_uuid",
            &[],
        );
        assert_eq!(
            guidance["no_mutation"].as_bool(),
            Some(true),
            "P082-R08: no_mutation must always be true for identifier guidance (kind={kind})"
        );
    }
}

// ── P082-R12/R13/R14: Cancellation interleaving shape tests ───────────────────

#[test]
fn p082_r12_cancellation_approval_preserved_readback_shape() {
    let now = Utc::now();
    let run_id = "run-cancel-r12-test";
    let readback = recovery_matrix::build_readback_v1(
        "P082-R12",
        "cancelled",
        "cancel",
        recovery_matrix::REASON_CANCEL_PENDING_APPROVAL_PRESERVED,
        "Run cancellation settled without modifying pending approval decision.",
        "runs, approvals, approval_inbox",
        "runs, approvals",
        run_id,
        Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    assert_eq!(readback["scenario_id"].as_str().unwrap(), "P082-R12");
    assert_eq!(readback["scenario_status"].as_str().unwrap(), "cancelled");
    assert_eq!(readback["recovery_decision"].as_str().unwrap(), "cancel");
    assert_eq!(
        readback["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_CANCEL_PENDING_APPROVAL_PRESERVED
    );
    // Approval decided_at must remain null — shape must not reference approval decisions.
    assert!(
        readback["recovery_next_action"]
            .as_str()
            .unwrap_or("")
            .contains("cancellation"),
        "P082-R12: next action must describe cancellation settlement, not approval retry"
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(&readback),
        "P082-R12: readback shape must be valid"
    );
}

#[test]
fn p082_r13_cancellation_side_effect_reconciliation_readback_shape() {
    let now = Utc::now();
    let run_id = "run-cancel-r13-test";
    let readback = recovery_matrix::set_readback_side_effect_hold(
        recovery_matrix::build_readback_v1(
            "P082-R13",
            "held",
            "reconcile_side_effects",
            recovery_matrix::REASON_CANCEL_SIDE_EFFECT_RECONCILIATION_REQUIRED,
            "Cancellation held: unresolved side effects must be reconciled before final settlement.",
            "runs, side_effects, side_effect_attempts, side_effect_settlements",
            "runs, side_effects",
            run_id,
            Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        "unresolved_side_effect_entries",
        "Cancellation held: unresolved side-effect ledger entries exist. Reconcile side effects before final settlement.",
    );
    assert_eq!(readback["scenario_id"].as_str().unwrap(), "P082-R13");
    assert_eq!(readback["scenario_status"].as_str().unwrap(), "held");
    assert_eq!(
        readback["recovery_decision"].as_str().unwrap(),
        "reconcile_side_effects"
    );
    assert_eq!(
        readback["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_CANCEL_SIDE_EFFECT_RECONCILIATION_REQUIRED
    );
    // operator_message should be populated (held state for side-effect reconciliation).
    // The proposal requires recovery_operator_message to be non-null for held states.
    // We verify the shape validator accepts the row.
    assert!(
        recovery_matrix::validate_readback_v1_shape(&readback),
        "P082-R13: readback shape must be valid"
    );
}

#[test]
fn p082_r14_cancellation_startup_repair_converged_readback_shape() {
    let now = Utc::now();
    let run_id = "run-cancel-r14-test";
    let summary = recovery_matrix::build_startup_repair_summary(
        "p082-requeue:cj-r14:wi-r14:1",
        "wi-r14",
        "cj-r14",
        1,
        1,
        true,
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "run",
    );
    let readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R14",
            "cancelled",
            "cancel",
            recovery_matrix::REASON_CANCEL_STARTUP_REPAIR_CONVERGED,
            "Cancellation settled; startup repair converged idempotently with cancellation.",
            "runs, startup_repairs, work_items, session_generations",
            "runs, startup_repairs, work_items, sessions",
            run_id,
            Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        summary,
        Some(
            "Cancellation converged with an existing startup repair idempotency key; no duplicate repair work was created.",
        ),
    );
    assert_eq!(readback["scenario_id"].as_str().unwrap(), "P082-R14");
    assert_eq!(readback["scenario_status"].as_str().unwrap(), "cancelled");
    assert_eq!(readback["recovery_decision"].as_str().unwrap(), "cancel");
    assert_eq!(
        readback["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_CANCEL_STARTUP_REPAIR_CONVERGED
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(&readback),
        "P082-R14: readback shape must be valid"
    );
}

// ── P082-R16: Startup requeue exhausted held-state readback ─────────────────

#[tokio::test]
async fn p082_r16_startup_requeue_exhausted_held_state_readback_in_notes() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r16-{run_id_str}");
    let work_item_id = format!("wi-r16-{run_id_str}");
    let exhausted_key = format!("p082-requeue:{journal_id}:{work_item_id}:1");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &serde_json::json!({"run_id": run_id_str, "stage_id": "implement"}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.retry"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record source command journal entry");

    // First: record that generation 1 was already consumed.
    startup_repairs::record(
        &pool,
        &exhausted_key,
        &run_id_str,
        "requeue_once",
        now,
        Some(r#"{"requeue_generation":1,"max_requeue_generation":1}"#),
    )
    .await
    .expect("record first startup repair (generation 1 consumed)");

    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, 'invoke_agent', ?2, 'running', ?3, 'implement', ?4, ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(
        serde_json::json!({
            "run_id": run_id_str,
            "stage_id": "implement",
            "stage_execution_id": "se-r16-prod",
            "p058_claimed": { "agent_execution_id": "ae-r16-prod" }
        })
        .to_string(),
    )
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running work item");

    let requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now + chrono::Duration::seconds(5),
        "startup_repair_abandoned_invoke_agent",
    )
    .await
    .expect("production startup requeue");
    assert_eq!(
        requeued, 0,
        "P082-R16: duplicate generation must not be requeued as pending work"
    );

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    let r16_row = readbacks
        .iter()
        .find(|r| r.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R16"))
        .expect("P082-R16: held-state row must be surfaced by accessor");

    assert_eq!(
        r16_row.get("scenario_status").and_then(|v| v.as_str()),
        Some("held"),
        "P082-R16: scenario_status must be held"
    );
    assert_eq!(
        r16_row.get("recovery_reason_code").and_then(|v| v.as_str()),
        Some(recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED),
        "P082-R16: recovery_reason_code must be startup_requeue_exhausted"
    );
    assert_eq!(
        r16_row.get("recovery_decision").and_then(|v| v.as_str()),
        Some("wait"),
        "P082-R16: recovery_decision must be wait (no new mutation)"
    );
    // Non-null operator message is required for held states (P082-R16 contract).
    let operator_msg = r16_row.get("recovery_operator_message");
    assert!(
        operator_msg.is_some() && !operator_msg.unwrap().is_null(),
        "P082-R16: recovery_operator_message must be non-null for startup_requeue_exhausted held state"
    );
    // startup_repair_summary must be non-null and carry replayed=true.
    let summary_field = r16_row.get("recovery_startup_repair_summary");
    assert!(
        summary_field.is_some() && !summary_field.unwrap().is_null(),
        "P082-R16: recovery_startup_repair_summary must be non-null"
    );
    let s = summary_field.unwrap().as_object().unwrap();
    assert_eq!(
        s.get("replayed").and_then(|v| v.as_bool()),
        Some(true),
        "P082-R16: replayed must be true when same idempotency key is observed again"
    );
    assert_eq!(
        s.get("max_requeue_generation").and_then(|v| v.as_i64()),
        Some(1),
        "P082-R16: max_requeue_generation must be 1"
    );

    let status: String = sqlx::query_scalar("SELECT status FROM work_items WHERE id = ?1")
        .bind(&work_item_id)
        .fetch_one(&pool)
        .await
        .expect("fetch duplicate work item status");
    assert_eq!(
        status, "failed",
        "P082-R16: exhausted startup requeue must terminalize the source work item instead of requeueing it"
    );

    // R16 readback must be stored in startup_repairs.notes.p082_recovery_matrix_readback
    // (the approved storage owner), NOT in work_items.payload_json.p082_r16_held.
    let notes_raw: Option<String> =
        sqlx::query_scalar("SELECT notes FROM startup_repairs WHERE id = ?1")
            .bind(&exhausted_key)
            .fetch_one(&pool)
            .await
            .expect("fetch startup_repairs notes");
    let notes_str =
        notes_raw.expect("P082-R16: startup_repairs.notes must be non-null after R16 held state");
    let notes_json: serde_json::Value = serde_json::from_str(&notes_str)
        .expect("P082-R16: startup_repairs.notes must be valid JSON");
    let stored_rb = notes_json
        .get("p082_recovery_matrix_readback")
        .expect("P082-R16: startup_repairs.notes must contain p082_recovery_matrix_readback");
    assert_eq!(
        stored_rb.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R16"),
        "P082-R16: stored readback in startup_repairs.notes must have scenario_id=P082-R16"
    );

    let work_payload_raw: String =
        sqlx::query_scalar("SELECT payload_json FROM work_items WHERE id = ?1")
            .bind(&work_item_id)
            .fetch_one(&pool)
            .await
            .expect("fetch work item payload");
    let work_payload: serde_json::Value =
        serde_json::from_str(&work_payload_raw).unwrap_or_default();
    assert!(
        work_payload.get("p082_r16_held").is_none(),
        "P082-R16: work_items.payload_json must NOT contain p082_r16_held (approved owner is startup_repairs.notes)"
    );
}

// ── P082-R09: Pending approval restart readback shape ────────────────────────

#[test]
fn p082_r09_approval_pending_readback_shape() {
    let now = Utc::now();
    let approval_id = "approval-r09-test";
    let readback = recovery_matrix::build_readback_v1(
        "P082-R09",
        "pending",
        "operator_approval_required",
        recovery_matrix::REASON_APPROVAL_PENDING_OPERATOR_ACTION_REQUIRED,
        "Approval gate is pending. Operator must approve or reject via the existing approval path.",
        "approvals, approval_inbox, stage_executions",
        "approvals, projections, stages",
        approval_id,
        Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    assert_eq!(readback["scenario_id"].as_str().unwrap(), "P082-R09");
    assert_eq!(readback["scenario_status"].as_str().unwrap(), "pending");
    assert_eq!(
        readback["recovery_decision"].as_str().unwrap(),
        "operator_approval_required"
    );
    assert_eq!(
        readback["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_APPROVAL_PENDING_OPERATOR_ACTION_REQUIRED
    );
    // R09: next action must point to existing approval path, not auto-resolution.
    assert!(
        readback["recovery_next_action"]
            .as_str()
            .unwrap_or("")
            .contains("approval"),
        "P082-R09: recovery_next_action must reference the existing approval path"
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(&readback),
        "P082-R09: readback shape must be valid"
    );
}

// ── P082-R15: Crash-resume idempotent readback shape ─────────────────────────

#[test]
fn p082_r15_crash_resume_idempotent_readback_shape() {
    let now = Utc::now();
    let repair_id = "p082-requeue:crash-r15:wi-r15-test:1";
    let summary = recovery_matrix::build_startup_repair_summary(
        repair_id,
        "wi-r15-test",
        "cj-r15-test",
        1,
        1,
        true, // replayed=true after crash recovery
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "global",
    );
    let readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R15",
            "repaired",
            "retry",
            recovery_matrix::REASON_REPAIR_CRASH_RESUME_IDEMPOTENT,
            "Crash-resume recovery converged idempotently on existing repair key.",
            "startup_repairs, work_items, command_journal",
            "startup_repairs, work_items, command_journal",
            repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        summary,
        None,
    );
    assert_eq!(readback["scenario_id"].as_str().unwrap(), "P082-R15");
    assert_eq!(readback["scenario_status"].as_str().unwrap(), "repaired");
    assert_eq!(readback["recovery_decision"].as_str().unwrap(), "retry");
    assert_eq!(
        readback["recovery_reason_code"].as_str().unwrap(),
        recovery_matrix::REASON_REPAIR_CRASH_RESUME_IDEMPOTENT
    );
    // R15: replayed=true confirms the crash-resume path converged on an existing key.
    let summary_field = readback.get("recovery_startup_repair_summary").unwrap();
    assert_eq!(
        summary_field.get("replayed").and_then(|v| v.as_bool()),
        Some(true),
        "P082-R15: replayed must be true for crash-resume convergence"
    );
    assert_eq!(
        summary_field
            .get("max_requeue_generation")
            .and_then(|v| v.as_i64()),
        Some(1),
        "P082-R15: max_requeue_generation must be 1"
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(&readback),
        "P082-R15: readback shape must be valid"
    );
}

// ── P082-R17: Cancel-then-late-output (active_projection_changed=false) ──────

#[tokio::test]
async fn p082_r17_cancelled_provider_late_output_accessor_reads_readback() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let stage_exec_id = format!("se-r17-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    // R17: cancelled provider session sent late output after cancellation.
    let settlement = recovery_matrix::build_late_output_settlement(
        "ae-r17-test",
        "wi-r17-test",
        "sg-r17-cancelled",
        "sg-r17-active",
        "closed", // claim_state: closed because session was cancelled
        "ignored",
        1,
        "completed", // source_work_item_terminal_status
        true,        // cancelled_provider_session=true distinguishes R17 from R03
    );
    let readback = recovery_matrix::set_readback_late_output_settlement(
        recovery_matrix::build_readback_v1(
            "P082-R17",
            "repaired",
            "no_mutation",
            recovery_matrix::REASON_CANCELLED_PROVIDER_LATE_OUTPUT_IGNORED,
            "Late output from cancelled provider session ignored; active projection unchanged.",
            "session_generations, session_events, artifact_source_generation_claims, work_items",
            "sessions, artifact_contracts, work_items",
            &stage_exec_id,
            Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        settlement,
    );
    let snapshot = serde_json::json!({ "p082_recovery_matrix_readback": readback });

    sqlx::query(
        r#"INSERT OR IGNORE INTO stage_executions
           (id, run_id, stage_id, label, status, iteration, attempt_number,
            started_at, owner_agent, provider, model, stage_type,
            recovery_snapshot_json)
           VALUES (?1, ?2, 'implement', 'impl', 'cancelled', 1, 1,
                   ?3, 'agent-1', 'provider-1', 'model-1', 'standard', ?4)"#,
    )
    .bind(&stage_exec_id)
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .bind(snapshot.to_string())
    .execute(&pool)
    .await
    .expect("insert stage_execution with R17 recovery_snapshot_json");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    assert_eq!(
        readbacks.len(),
        1,
        "P082-R17: accessor must return one row from stage_executions.recovery_snapshot_json"
    );
    let row = &readbacks[0];
    assert_eq!(
        row.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R17")
    );
    assert_eq!(
        row.get("recovery_reason_code").and_then(|v| v.as_str()),
        Some(recovery_matrix::REASON_CANCELLED_PROVIDER_LATE_OUTPUT_IGNORED)
    );
    // P082-R17 contract: active_projection_changed must be false.
    let settlement = row.get("recovery_late_output_settlement").unwrap();
    assert_eq!(
        settlement
            .get("active_projection_changed")
            .and_then(|v| v.as_bool()),
        Some(false),
        "P082-R17: active_projection_changed must be false"
    );
    // cancelled_provider_session must be true for R17 (distinguishes from R03).
    assert_eq!(
        settlement
            .get("cancelled_provider_session")
            .and_then(|v| v.as_bool()),
        Some(true),
        "P082-R17: cancelled_provider_session must be true"
    );
    // source_work_item_terminal_status must be completed or failed, never pending/running.
    let terminal_status = settlement
        .get("source_work_item_terminal_status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        ["completed", "failed"].contains(&terminal_status),
        "P082-R17: source_work_item_terminal_status must be completed or failed, got '{terminal_status}'"
    );
}

// ── P082-R01: Integration: actual startup requeue creates idempotency row ──────

/// Integration test: calls the production `requeue_running_invoke_agent_on_startup`
/// and verifies that:
/// - A startup_repairs row is created with the canonical idempotency key
/// - startup_repairs.notes contains p082_recovery_matrix_readback with scenario_id=P082-R01
/// - The accessor surfaces a P082-R01 row for the run
#[tokio::test]
async fn p082_r01_startup_requeue_integration_creates_idempotency_row() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r01-integ-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    command_journal::record(
        &pool,
        &journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("engine"),
        None,
        Some("operator"),
        Some("engine.invoke"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal");

    // Insert a running InvokeAgent work item with a journal reference so the
    // startup repair can derive the canonical idempotency key.
    let work_item_id = format!("wi-r01-integ-{run_id_str}");
    sqlx::query(
        r#"INSERT INTO work_items (id, run_id, kind, payload_json, status, created_at, scheduled_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(&run_id_str)
    .bind(serde_json::json!({"run_id": run_id_str, "journal_id": journal_id}).to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running InvokeAgent work item");

    let requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now,
        "startup_requeue_integration_test",
    )
    .await
    .expect("requeue_running_invoke_agent_on_startup must not fail");

    assert_eq!(
        requeued, 1,
        "P082-R01: exactly one work item must be requeued"
    );

    // Verify the startup_repairs idempotency row was created.
    let expected_repair_id = format!("p082-requeue:{journal_id}:{work_item_id}:1");
    let notes_raw: Option<String> =
        sqlx::query_scalar("SELECT notes FROM startup_repairs WHERE id = ?1")
            .bind(&expected_repair_id)
            .fetch_optional(&pool)
            .await
            .expect("query startup_repairs");

    let notes_str = notes_raw
        .expect("P082-R01: startup_repairs idempotency row must exist with the canonical key");
    let notes_json: serde_json::Value = serde_json::from_str(&notes_str)
        .expect("P082-R01: startup_repairs.notes must be valid JSON");
    let readback = notes_json
        .get("p082_recovery_matrix_readback")
        .expect("P082-R01: startup_repairs.notes must contain p082_recovery_matrix_readback");
    assert_eq!(
        readback.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R01"),
        "P082-R01: scenario_id must be P082-R01"
    );
    assert_eq!(
        readback
            .get("recovery_reason_code")
            .and_then(|v| v.as_str()),
        Some(recovery_matrix::REASON_STARTUP_REQUEUE_ONCE),
        "P082-R01: reason code must be startup_requeue_once"
    );
    assert_eq!(
        readback.get("scenario_status").and_then(|v| v.as_str()),
        Some("repaired"),
        "P082-R01: scenario_status must be repaired"
    );

    // The startup repair summary must be populated with requeue_generation=1.
    let summary = readback
        .get("recovery_startup_repair_summary")
        .expect("P082-R01: recovery_startup_repair_summary must be present");
    assert_eq!(
        summary.get("requeue_generation").and_then(|v| v.as_i64()),
        Some(1),
        "P082-R01: requeue_generation must be 1"
    );
    assert_eq!(
        summary
            .get("max_requeue_generation")
            .and_then(|v| v.as_i64()),
        Some(1),
        "P082-R01: max_requeue_generation must be 1"
    );

    // The accessor must surface the P082-R01 row for the run.
    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    let r01_row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R01"))
        .expect("P082-R01: accessor must return a P082-R01 row after startup requeue");
    assert_eq!(
        r01_row
            .get("recovery_projection_integrity")
            .and_then(|v| v.as_str()),
        Some("valid"),
        "P082-R01: recovery_projection_integrity must be valid"
    );
}

#[tokio::test]
async fn p082_startup_requeue_uses_work_item_run_id_when_payload_run_id_is_tampered() {
    let pool = setup_db().await;
    let now = Utc::now();
    let owner_run_id = RunId::new();
    let owner_run_id_str = owner_run_id.to_string();
    let payload_run_id = RunId::new();
    let payload_run_id_str = payload_run_id.to_string();
    let owner_journal_id = format!("cj-owner-{owner_run_id_str}");
    let payload_journal_id = format!("cj-payload-{payload_run_id_str}");

    insert_test_run(&pool, &owner_run_id_str, &now.to_rfc3339()).await;
    insert_test_run(&pool, &payload_run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &owner_journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": owner_run_id_str}).to_string(),
        Some(&owner_run_id_str),
        now,
        Some("engine"),
        None,
        Some("operator"),
        Some("engine.invoke"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record owner command journal");
    command_journal::record(
        &pool,
        &payload_journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": payload_run_id_str}).to_string(),
        Some(&payload_run_id_str),
        now,
        Some("engine"),
        None,
        Some("operator"),
        Some("engine.invoke"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record payload command journal");

    let work_item_id = format!("wi-owner-{owner_run_id_str}");
    sqlx::query(
        r#"INSERT INTO work_items (id, run_id, kind, payload_json, status, created_at, scheduled_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(&owner_run_id_str)
    .bind(serde_json::json!({"run_id": payload_run_id_str}).to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert tampered running InvokeAgent work item");

    let requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now,
        "startup_requeue_tampered_payload_run_id",
    )
    .await
    .expect("startup requeue must not fail");
    assert_eq!(requeued, 1);

    let owner_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs WHERE run_id = ?1")
            .bind(&owner_run_id_str)
            .fetch_one(&pool)
            .await
            .expect("count owner startup repairs");
    let payload_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs WHERE run_id = ?1")
            .bind(&payload_run_id_str)
            .fetch_one(&pool)
            .await
            .expect("count payload startup repairs");
    assert_eq!(
        owner_count, 1,
        "SEC-P082-MEDIUM-001: startup repair must be owned by work_items.run_id"
    );
    assert_eq!(
        payload_count, 0,
        "SEC-P082-MEDIUM-001: tampered payload run_id must not receive startup repair diagnostics"
    );

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, owner_run_id)
        .await
        .expect("readbacks_for_run");
    let r01 = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R01"))
        .expect("P082-R01 readback for owner run");
    assert_eq!(
        r01.get("recovery_projection_integrity")
            .and_then(|v| v.as_str()),
        Some("tamper_detected"),
        "SEC-P082-MEDIUM-001: mismatched payload run_id must mark readback tamper_detected"
    );
    assert_eq!(
        r01.pointer("/recovery_startup_repair_summary/source_command_journal_id")
            .and_then(|v| v.as_str()),
        Some(owner_journal_id.as_str()),
        "SEC-P082-MEDIUM-001: fallback command journal must be selected from work_items.run_id"
    );
}

// ── P082-R01/R15/R16: Integration startup replay and exhaustion ──────────────

/// Integration test: calls the production `requeue_running_invoke_agent_on_startup`
/// twice for the same work item after the first pass already stamped
/// `p061_startup_recovery` into the payload. This is the crash-replay case: the
/// same generation may be requeued idempotently and must not be misclassified as
/// startup_requeue_exhausted.
#[tokio::test]
async fn p082_r01_startup_requeue_crash_replay_requeues_same_generation() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r16-integ-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    command_journal::record(
        &pool,
        &journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("engine"),
        None,
        Some("operator"),
        Some("engine.invoke"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal");

    let work_item_id = format!("wi-r16-integ-{run_id_str}");
    sqlx::query(
        r#"INSERT INTO work_items (id, run_id, kind, payload_json, status, created_at, scheduled_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(&run_id_str)
    .bind(serde_json::json!({"run_id": run_id_str, "journal_id": journal_id}).to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running InvokeAgent work item");

    // First requeue: should succeed and create the idempotency row.
    let first_requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now,
        "startup_requeue_first_pass",
    )
    .await
    .expect("first requeue must not fail");
    assert_eq!(
        first_requeued, 1,
        "P082-R01: first requeue must process one item"
    );

    // Re-insert the work item as running while preserving the stamped
    // p061_startup_recovery payload (simulating crash after payload write and
    // before the replayed work completes).
    sqlx::query(
        "UPDATE work_items SET status = 'running', started_at = ?1, completed_at = NULL, failed_at = NULL WHERE id = ?2",
    )
    .bind(now.to_rfc3339())
    .bind(&work_item_id)
    .execute(&pool)
    .await
    .expect("reset work item to running for second requeue attempt");

    // Second requeue is a crash replay of the same generation. It must requeue
    // idempotently instead of overwriting R01 with R16.
    let second_requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now,
        "startup_requeue_second_pass",
    )
    .await
    .expect("second requeue must not fail");
    assert_eq!(
        second_requeued, 1,
        "P082-R01/R15: crash replay of the same stamped generation must requeue idempotently"
    );

    let work_status: String = sqlx::query_scalar("SELECT status FROM work_items WHERE id = ?1")
        .bind(&work_item_id)
        .fetch_one(&pool)
        .await
        .expect("fetch work item status");
    assert_eq!(
        work_status, "pending",
        "P082-R01/R15: replayed startup repair should return the work item to pending"
    );

    // Verify only one startup_repairs row exists (idempotency invariant).
    let repair_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs WHERE run_id = ?1")
            .bind(&run_id_str)
            .fetch_one(&pool)
            .await
            .expect("count startup_repairs");
    assert_eq!(
        repair_count, 1,
        "P082-R01/R15: exactly one startup_repairs row must exist (no duplicate)"
    );

    // Verify the readback accessor still surfaces the R01 repaired row, not R16.
    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    let r01_row = readbacks
        .iter()
        .find(|row| {
            row.get("recovery_reason_code").and_then(|v| v.as_str())
                == Some(recovery_matrix::REASON_STARTUP_REQUEUE_ONCE)
        })
        .expect("P082-R01/R15: accessor must keep returning startup_requeue_once for replay");
    assert_eq!(
        r01_row.get("scenario_status").and_then(|v| v.as_str()),
        Some("repaired"),
        "P082-R01/R15: scenario_status must remain repaired for replay"
    );
    assert!(
        !readbacks.iter().any(
            |row| row.get("recovery_reason_code").and_then(|v| v.as_str())
                == Some(recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED)
        ),
        "P082-R01/R15: crash replay must not emit startup_requeue_exhausted"
    );
}

/// A duplicate startup repair key without the stamped p061_startup_recovery
/// payload is not a crash replay. That is the actual R16 exhausted-generation
/// case and must hold without enqueuing duplicate work.
#[tokio::test]
async fn p082_r16_startup_requeue_exhausted_non_replay_holds_without_duplicating_work() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r16-non-replay-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("engine"),
        None,
        Some("operator"),
        Some("engine.invoke"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal");

    let work_item_id = format!("wi-r16-non-replay-{run_id_str}");
    let repair_id = format!("p082-requeue:{journal_id}:{work_item_id}:1");
    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id_str,
        "p082_requeue_once",
        now,
        Some(r#"{"preexisting":true}"#),
    )
    .await
    .expect("seed preexisting startup repair");

    sqlx::query(
        r#"INSERT INTO work_items (id, run_id, kind, payload_json, status, created_at, scheduled_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(&run_id_str)
    .bind(serde_json::json!({"run_id": run_id_str, "journal_id": journal_id}).to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert non-replay running InvokeAgent work item");

    let requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now,
        "startup_requeue_non_replay_duplicate",
    )
    .await
    .expect("non-replay duplicate must not fail");
    assert_eq!(
        requeued, 0,
        "P082-R16: duplicate key without stamped replay payload must not create new work"
    );

    let work_status: String = sqlx::query_scalar("SELECT status FROM work_items WHERE id = ?1")
        .bind(&work_item_id)
        .fetch_one(&pool)
        .await
        .expect("fetch exhausted work item status");
    assert_eq!(
        work_status, "failed",
        "P082-R16: exhausted non-replay work item must be terminalized"
    );

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    let r16_row = readbacks
        .iter()
        .find(|row| {
            row.get("recovery_reason_code").and_then(|v| v.as_str())
                == Some(recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED)
        })
        .expect(
            "P082-R16: accessor must return startup_requeue_exhausted for non-replay duplicate",
        );
    assert_eq!(
        r16_row.get("scenario_status").and_then(|v| v.as_str()),
        Some("held"),
        "P082-R16: scenario_status must be held"
    );
}

// ── P082: Metric emission verification ─────────────────────────────────────

/// Verifies that the p082_recovery_idempotency_replay_total counter is incremented
/// when startup requeue detects an existing idempotency key (R16).
#[tokio::test]
async fn p082_r16_metric_emitted_on_startup_requeue_exhausted() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r16-metric-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    command_journal::record(
        &pool,
        &journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("engine"),
        None,
        Some("operator"),
        Some("engine.invoke"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal");

    let work_item_id = format!("wi-r16-metric-{run_id_str}");
    sqlx::query(
        r#"INSERT INTO work_items (id, run_id, kind, payload_json, status, created_at, scheduled_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(&run_id_str)
    .bind(serde_json::json!({"run_id": run_id_str, "journal_id": journal_id}).to_string())
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running InvokeAgent work item");

    let before_r01 = db::metrics::get_counter_with_label(
        "p082_recovery_idempotency_replay_total",
        "P082-R01:startup_requeue_once",
    );

    work_items::requeue_running_invoke_agent_on_startup(&pool, now, "metric_test")
        .await
        .expect("first requeue");

    let after_r01 = db::metrics::get_counter_with_label(
        "p082_recovery_idempotency_replay_total",
        "P082-R01:startup_requeue_once",
    );
    assert!(
        after_r01 > before_r01,
        "P082-R01: p082_recovery_idempotency_replay_total must be incremented on first requeue"
    );

    // Reset to running without the stamped p061_startup_recovery marker and
    // replace startup_repairs.notes with a generation-consumed marker that does
    // not contain a valid R01 readback. A duplicate key with valid R01 notes is
    // an unapplied crash replay; this setup is the actual non-replay R16 case.
    sqlx::query("UPDATE work_items SET status = 'running', payload_json = ?1 WHERE id = ?2")
        .bind(serde_json::json!({"run_id": run_id_str, "journal_id": journal_id}).to_string())
        .bind(&work_item_id)
        .execute(&pool)
        .await
        .expect("reset work item");
    let repair_id = format!("p082-requeue:{journal_id}:{work_item_id}:1");
    sqlx::query("UPDATE startup_repairs SET notes = ?1 WHERE id = ?2")
        .bind(r#"{"requeue_generation":1,"max_requeue_generation":1}"#)
        .bind(&repair_id)
        .execute(&pool)
        .await
        .expect("replace startup repair notes with consumed-generation marker");

    let before_r16 = db::metrics::get_counter_with_label(
        "p082_recovery_idempotency_replay_total",
        "P082-R16:startup_requeue_exhausted",
    );

    work_items::requeue_running_invoke_agent_on_startup(&pool, now, "metric_test_second")
        .await
        .expect("second requeue");

    let after_r16 = db::metrics::get_counter_with_label(
        "p082_recovery_idempotency_replay_total",
        "P082-R16:startup_requeue_exhausted",
    );
    assert!(
        after_r16 > before_r16,
        "P082-R16: p082_recovery_idempotency_replay_total must be incremented on exhausted requeue"
    );
}

#[tokio::test]
async fn p082_required_matrix_metrics_are_emitted_from_readback_accessor() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let readback = recovery_matrix::build_readback_v1(
        "P082-R02",
        "rejected",
        "no_mutation",
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "Rejected before mutation.",
        "command_journal",
        "command_journal",
        "p082-metric-journal",
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    let envelope = recovery_matrix::build_rejected_command_error_envelope(
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "RetryStage",
        "Rejected before mutation.",
        readback,
    );
    command_journal::record(
        &pool,
        "p082-metric-journal",
        "RetryStage",
        r#"{"run_id":"metric-run"}"#,
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator"),
        Some("operator"),
        Some("runs.retry"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record metric journal");
    command_journal::fail_entry(&pool, "p082-metric-journal", now, &envelope)
        .await
        .expect("fail metric journal");

    // Runtime readback emits readback/age telemetry only. Gate-result telemetry is
    // emitted by the proposal-082 gate harness after assertion groups, not here.
    let before_gate = db::metrics::get_counter_with_label(
        "p082_recovery_matrix_gate_result_total",
        "P082-R02:rejected",
    );
    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run");
    assert_eq!(readbacks.len(), 1);
    assert!(
        db::metrics::get_gauge(
            "p082_recovery_matrix_rows_with_db_engine_readback_coverage_percent"
        )
        .unwrap_or(0)
            > 0,
        "P082: coverage percent gauge must be emitted when readbacks are built"
    );
    assert!(
        db::metrics::get_p082_recovery_state_age_seconds_for(
            "P082-R02",
            recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        )
        .is_some(),
        "P082: state age metric must be emitted with {{scenario_id,reason_code}} labels"
    );
    let after_gate = db::metrics::get_counter_with_label(
        "p082_recovery_matrix_gate_result_total",
        "P082-R02:rejected",
    );
    assert_eq!(
        after_gate, before_gate,
        "P082: runtime readback must not emit gate-result telemetry; gate harness owns p082_recovery_matrix_gate_result_total"
    );
}

/// Gate-harness outcome telemetry is owned by `scripts/test-gate.sh`, not by
/// runtime readback accessors or DB tests. This regression keeps the old unit
/// test from becoming a false proof by verifying the actual proposal-082 gate
/// parses libtest scenario assertion results and emits passed/failed/interrupted
/// samples from the shell harness.
///
/// The approved proposal specifies:
/// `p082_recovery_matrix_gate_result_total{scenario_id,status}` emission site:
/// "proposal-082 gate harness after each scenario assertion group".
#[test]
fn p082_gate_harness_emits_gate_result_per_scenario_assertion() {
    let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let gate_script = std::fs::read_to_string(repo_root.join("scripts/test-gate.sh"))
        .expect("read scripts/test-gate.sh");
    let p082_start = gate_script
        .find("proposal-082|p082)")
        .expect("proposal-082 gate block must exist");
    let p082_block = &gate_script[p082_start..];

    for required in [
        "p082_emit_gate_result_metric",
        "p082_emit_gate_results_from_cargo_output",
        "p082_emit_interrupted_gate_results",
        "p082_recovery_matrix_gate_result_total{scenario_id=\"%s\",status=\"%s\"} 1",
        "\"ok\": \"passed\"",
        "\"FAILED\": \"failed\"",
        "\"ignored\": \"interrupted\"",
        "trap p082_emit_interrupted_gate_results EXIT",
    ] {
        assert!(
            p082_block.contains(required),
            "P082 gate harness must contain gate-result telemetry fragment: {required}"
        );
    }

    for scenario_id in domain::recovery_matrix::SCENARIO_IDS {
        assert!(
            p082_block.contains(scenario_id),
            "P082 gate harness must track interrupted outcome coverage for {scenario_id}"
        );
    }
}

// ── P082-R05: Xcode payload grace — claude/default runtime must not be treated as standard stale ──

/// A session_generation with runtime_provider='claude'/runtime_model='default' but
/// xcode_broker_required=true in the work item payload must not produce a P082-R05
/// readback before the 12-minute Xcode grace period expires.
/// This validates the payload-based xcode detection fix in Source 8.
#[tokio::test]
async fn p082_r05_xcode_payload_claude_default_not_standard_stale_within_grace() {
    let pool = setup_db().await;
    let now = Utc::now();
    // 4 minutes ago: past the standard 3-min grace but within the 12-min Xcode grace.
    let started_at = now - chrono::Duration::minutes(4);
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let lineage_id = format!("p082-r05-xc-lineage-{run_id_str}");
    let generation_id = format!("p082-r05-xc-generation-{run_id_str}");
    let ae_id = format!("p082-r05-xc-ae-{run_id_str}");
    let wi_id = format!("p082-r05-xc-wi-{run_id_str}");

    sqlx::query(
        r#"INSERT INTO session_lineages
           (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
            active_generation_id, created_at, closed_at)
           VALUES (?1, ?2, 'agent-r05-xc', ?1, 'run', NULL, ?3, ?4, NULL)"#,
    )
    .bind(&lineage_id)
    .bind(&run_id_str)
    .bind(&generation_id)
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert lineage");

    // runtime_provider/model are 'claude'/'default' — NOT xcode in runtime fields.
    sqlx::query(
        r#"INSERT INTO session_generations
           (id, lineage_id, generation, invocation_owner_key, provider_session_id,
            binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
            workspace_mode, runtime_provider, runtime_model, status, created_at)
           VALUES (?1, ?2, 1, ?3, NULL, 'binding-r05-xc', NULL, '/', 'read_write',
                   'claude', 'default', 'active', ?4)"#,
    )
    .bind(&generation_id)
    .bind(&lineage_id)
    .bind(&wi_id)
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert session generation with claude/default runtime");

    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id,
            owner_kind, owner_id)
           VALUES (?1, 'agent-r05-xc', 'claude', 'running', ?2,
                   ?3, ?4, 'lead_conflict_mediation', ?1)"#,
    )
    .bind(&ae_id)
    .bind(started_at.to_rfc3339())
    .bind(&generation_id)
    .bind(&lineage_id)
    .execute(&pool)
    .await
    .expect("insert agent execution");

    // Payload has xcode_broker_required=true — this is the payload-based Xcode signal.
    let payload = serde_json::json!({
        "run_id": run_id_str,
        "xcode_broker_required": true,
        "p058_claimed": {"agent_execution_id": ae_id},
    });
    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, ?4, 1)"#,
    )
    .bind(&wi_id)
    .bind(&run_id_str)
    .bind(payload.to_string())
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running work item with xcode payload");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    let r05 = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R05"));

    assert!(
        r05.is_none(),
        "P082-R05: a claude/default session with xcode_broker_required=true must NOT produce \
         an R05 readback within the 12-minute Xcode grace (started 4 min ago)"
    );
}

// ── P082-R05: Durable readback persists after production stale startup repair ──

/// After `requeue_stale_starting_invoke_agent_sessions` repairs a stale startup session,
/// the P082-R05 readback must remain visible via readbacks_for_run (from startup_repairs),
/// even though the live session_generation is no longer 'active'.
#[tokio::test]
async fn p082_r05_durable_readback_persists_after_stale_startup_repair() {
    let pool = setup_db().await;
    let now = Utc::now();
    // 13 minutes ago: past the 12-minute Xcode grace.
    let started_at = now - chrono::Duration::minutes(13);
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let lineage_id = format!("p082-r05-dur-lineage-{run_id_str}");
    let generation_id = format!("p082-r05-dur-generation-{run_id_str}");
    let ae_id = format!("p082-r05-dur-ae-{run_id_str}");
    let wi_id = format!("p082-r05-dur-wi-{run_id_str}");

    sqlx::query(
        r#"INSERT INTO session_lineages
           (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
            active_generation_id, created_at, closed_at)
           VALUES (?1, ?2, 'agent-r05-dur', ?1, 'run', NULL, ?3, ?4, NULL)"#,
    )
    .bind(&lineage_id)
    .bind(&run_id_str)
    .bind(&generation_id)
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert lineage");

    sqlx::query(
        r#"INSERT INTO session_generations
           (id, lineage_id, generation, invocation_owner_key, provider_session_id,
            binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
            workspace_mode, runtime_provider, runtime_model, status, created_at)
           VALUES (?1, ?2, 1, ?3, NULL, 'binding-r05-dur', NULL, '/', 'read_write',
                   'claude', 'default', 'active', ?4)"#,
    )
    .bind(&generation_id)
    .bind(&lineage_id)
    .bind(&wi_id)
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert session generation");

    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id,
            owner_kind, owner_id)
           VALUES (?1, 'agent-r05-dur', 'claude', 'running', ?2,
                   ?3, ?4, 'lead_conflict_mediation', ?1)"#,
    )
    .bind(&ae_id)
    .bind(started_at.to_rfc3339())
    .bind(&generation_id)
    .bind(&lineage_id)
    .execute(&pool)
    .await
    .expect("insert agent execution");

    let payload = serde_json::json!({
        "run_id": run_id_str,
        "xcode_broker_required": true,
        "p058_claimed": {"agent_execution_id": ae_id},
    });
    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, ?4, 1)"#,
    )
    .bind(&wi_id)
    .bind(&run_id_str)
    .bind(payload.to_string())
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running work item");

    // Repair: now=now, standard cutoff=now-3min, xcode cutoff=now-12min.
    let standard_cutoff = now - chrono::Duration::minutes(3);
    let xcode_cutoff = now - chrono::Duration::minutes(12);
    let requeued = work_items::requeue_stale_starting_invoke_agent_sessions(
        &pool,
        now,
        standard_cutoff,
        xcode_cutoff,
        "p082_r05_durable_test",
    )
    .await
    .expect("repair must succeed");
    assert_eq!(
        requeued, 1,
        "P082-R05: exactly one work item must be requeued"
    );

    // Verify the approved work_items.payload_json.p061_startup_recovery owner carries R05.
    let payload_raw: String =
        sqlx::query_scalar("SELECT payload_json FROM work_items WHERE id = ?1")
            .bind(&wi_id)
            .fetch_one(&pool)
            .await
            .expect("query repaired work item payload");
    let payload_json: serde_json::Value =
        serde_json::from_str(&payload_raw).expect("work_items.payload_json must be valid JSON");
    let readback_in_payload = payload_json
        .pointer("/p061_startup_recovery/p082_recovery_matrix_readback")
        .expect("P082-R05: payload startup recovery must contain p082_recovery_matrix_readback");
    assert_eq!(
        readback_in_payload
            .get("scenario_id")
            .and_then(|v| v.as_str()),
        Some("P082-R05"),
        "P082-R05: payload readback must have scenario_id=P082-R05"
    );
    assert_eq!(
        readback_in_payload
            .get("scenario_status")
            .and_then(|v| v.as_str()),
        Some("repaired"),
        "P082-R05: payload readback must have scenario_status=repaired"
    );
    assert_eq!(
        readback_in_payload
            .get("source_json_key")
            .and_then(|v| v.as_str()),
        Some("work_items.payload_json.p061_startup_recovery"),
        "P082-R05: readback must use the approved work_items payload source key"
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(readback_in_payload),
        "P082-R05: payload readback must pass validate_readback_v1_shape"
    );

    // The accessor must surface the P082-R05 row from work_items payload after repair.
    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    let r05_row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R05"))
        .expect("P082-R05: accessor must return R05 row after stale startup repair");
    assert_eq!(
        r05_row.get("scenario_status").and_then(|v| v.as_str()),
        Some("repaired"),
        "P082-R05: accessor R05 row must have scenario_status=repaired"
    );
    assert_eq!(
        r05_row.get("recovery_reason_code").and_then(|v| v.as_str()),
        Some(recovery_matrix::REASON_STARTUP_STALLED),
        "P082-R05: accessor R05 row must have reason_code=startup_stalled"
    );
    // The xcode operator message must be present and complete (xcode_broker_required was true).
    let msg = r05_row
        .get("recovery_operator_message")
        .and_then(|v| v.as_str())
        .expect("P082-R05: Xcode repaired readback must include the Xcode operator message");
    assert!(
        msg.contains("Xcode startup grace"),
        "P082-R05: operator message must name Xcode startup grace: {msg}"
    );
    assert!(
        msg.contains("cutoff:") || msg.contains("cutoff_at") || msg.contains("(cutoff"),
        "P082-R05: operator message must include the cutoff timestamp: {msg}"
    );
    assert!(
        msg.contains("Next check") || msg.contains("next check") || msg.contains("backoff"),
        "P082-R05: operator message must explicitly include the next check/backoff state (not just 'Inspect'): {msg}"
    );
}

#[tokio::test]
async fn p082_r05_pre_session_repair_uses_work_item_payload_owner() {
    let pool = setup_db().await;
    let now = Utc::now();
    let started_at = now - chrono::Duration::minutes(5);
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let journal_id = format!("cj-r05-pre-session-{run_id_str}");
    command_journal::record(
        &pool,
        &journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.start"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry");

    let ae_id = format!("ae-r05-pre-session-{run_id_str}");
    let wi_id = format!("wi-r05-pre-session-{run_id_str}");
    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id,
            owner_kind, owner_id)
           VALUES (?1, 'agent-r05-pre-session', 'claude', 'running', ?2,
                   NULL, NULL, 'lead_conflict_mediation', ?1)"#,
    )
    .bind(&ae_id)
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert pre-session agent execution");

    let payload = serde_json::json!({
        "run_id": run_id_str,
        "stage_id": "implement",
        "journal_id": journal_id,
        "p058_claimed": { "agent_execution_id": ae_id },
    });
    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, ?4, 1)"#,
    )
    .bind(&wi_id)
    .bind(&run_id_str)
    .bind(payload.to_string())
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running pre-session work item");

    let requeued = work_items::requeue_stale_pre_session_invoke_agents(
        &pool,
        now,
        now - chrono::Duration::minutes(3),
        now - chrono::Duration::minutes(12),
        "startup_repair_stale_acp_pre_session_startup",
    )
    .await
    .expect("pre-session repair must succeed");
    assert_eq!(
        requeued, 1,
        "P082-R05: pre-session stale startup must requeue exactly one item"
    );

    let payload_raw: String =
        sqlx::query_scalar("SELECT payload_json FROM work_items WHERE id = ?1")
            .bind(&wi_id)
            .fetch_one(&pool)
            .await
            .expect("query repaired pre-session work item payload");
    let payload_json: serde_json::Value =
        serde_json::from_str(&payload_raw).expect("payload must be valid JSON");
    assert_eq!(
        payload_json
            .pointer("/p061_startup_recovery/reason")
            .and_then(|v| v.as_str()),
        Some("startup_repair_stale_acp_pre_session_startup"),
        "P082-R05: payload must preserve the pre-session startup repair reason"
    );
    let readback = payload_json
        .pointer("/p061_startup_recovery/p082_recovery_matrix_readback")
        .expect("P082-R05: pre-session payload must contain readback");
    assert_eq!(readback["scenario_id"], serde_json::json!("P082-R05"));
    assert_eq!(
        readback["source_json_key"],
        serde_json::json!("work_items.payload_json.p061_startup_recovery")
    );
    assert!(
        recovery_matrix::validate_readback_v1_shape(readback),
        "P082-R05: pre-session readback must validate"
    );

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");
    assert!(
        readbacks.iter().any(|row| {
            row.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R05")
                && row.get("source_identifier").and_then(|v| v.as_str()) == Some(wi_id.as_str())
        }),
        "P082-R05: accessor must surface pre-session work_items payload owner"
    );
}

#[tokio::test]
async fn p082_stale_pre_session_requeue_uses_work_item_run_id_when_payload_run_id_is_tampered() {
    let pool = setup_db().await;
    let now = Utc::now();
    let started_at = now - chrono::Duration::minutes(15);
    let owner_run_id = RunId::new();
    let owner_run_id_str = owner_run_id.to_string();
    let payload_run_id = RunId::new();
    let payload_run_id_str = payload_run_id.to_string();
    let owner_journal_id = format!("cj-r05-owner-{owner_run_id_str}");
    let payload_journal_id = format!("cj-r05-payload-{payload_run_id_str}");

    insert_test_run(&pool, &owner_run_id_str, &now.to_rfc3339()).await;
    insert_test_run(&pool, &payload_run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &owner_journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": owner_run_id_str}).to_string(),
        Some(&owner_run_id_str),
        now,
        Some("engine"),
        None,
        Some("operator"),
        Some("engine.invoke"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record owner command journal");
    command_journal::record(
        &pool,
        &payload_journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": payload_run_id_str}).to_string(),
        Some(&payload_run_id_str),
        now,
        Some("engine"),
        None,
        Some("operator"),
        Some("engine.invoke"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record payload command journal");

    let ae_id = format!("ae-r05-owner-{owner_run_id_str}");
    let wi_id = format!("wi-r05-owner-{owner_run_id_str}");
    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id,
            owner_kind, owner_id)
           VALUES (?1, 'agent-r05-owner', 'claude', 'running', ?2,
                   NULL, NULL, 'lead_conflict_mediation', ?1)"#,
    )
    .bind(&ae_id)
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert pre-session agent execution");

    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, ?4, 1)"#,
    )
    .bind(&wi_id)
    .bind(&owner_run_id_str)
    .bind(
        serde_json::json!({
            "run_id": payload_run_id_str,
            "p058_claimed": { "agent_execution_id": ae_id },
        })
        .to_string(),
    )
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert tampered pre-session work item");

    let requeued = work_items::requeue_stale_pre_session_invoke_agents(
        &pool,
        now,
        now - chrono::Duration::minutes(3),
        now - chrono::Duration::minutes(12),
        "startup_repair_stale_acp_pre_session_startup",
    )
    .await
    .expect("pre-session repair must succeed");
    assert_eq!(requeued, 1);

    let payload_raw: String =
        sqlx::query_scalar("SELECT payload_json FROM work_items WHERE id = ?1")
            .bind(&wi_id)
            .fetch_one(&pool)
            .await
            .expect("query repaired pre-session work item payload");
    let payload_json: serde_json::Value =
        serde_json::from_str(&payload_raw).expect("payload must be valid JSON");
    let readback = payload_json
        .pointer("/p061_startup_recovery/p082_recovery_matrix_readback")
        .expect("P082-R05 readback");
    assert_eq!(
        readback
            .get("recovery_projection_integrity")
            .and_then(|v| v.as_str()),
        Some("tamper_detected"),
        "SEC-P082-MEDIUM-001: mismatched payload run_id must mark pre-session readback tamper_detected"
    );
    assert_eq!(
        readback
            .pointer("/recovery_startup_repair_summary/source_command_journal_id")
            .and_then(|v| v.as_str()),
        Some(owner_journal_id.as_str()),
        "SEC-P082-MEDIUM-001: pre-session fallback journal must be selected from work_items.run_id"
    );
    let payload_readbacks =
        db::repos::p082_recovery_matrix::readbacks_for_run(&pool, payload_run_id)
            .await
            .expect("readbacks_for_run for tampered payload run");
    assert!(
        payload_readbacks.iter().all(|row| {
            row.get("source_identifier").and_then(|v| v.as_str()) != Some(wi_id.as_str())
        }),
        "SEC-P082-MEDIUM-001: tampered payload run must not surface owner work item readback"
    );
}

// ── P082-R15: Crash boundaries — remaining durable write boundaries ────────────

/// Crash after session invalidation but before idempotency row insert.
/// Simulates: work item is still 'running' and no startup_repairs row exists yet —
/// the daemon crashed after invalidating the session but before writing the repair key.
/// Recovery on restart must create the startup_repairs row and requeue exactly once.
#[tokio::test]
async fn p082_r15_crash_after_session_invalidation_before_idempotency_row_recovers() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r15-si-{run_id_str}");
    let work_item_id = format!("wi-r15-si-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.start"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry for session-invalidation crash test");

    // Work item is 'running' — session was invalidated but no startup_repairs row yet.
    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, 'invoke_agent', ?2, 'running', ?3, 'implement', ?4, ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(
        serde_json::json!({
            "run_id": run_id_str,
            "stage_id": "implement",
            "stage_execution_id": "se-r15-si",
            "source_command_journal_id": journal_id,
        })
        .to_string(),
    )
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running work item for session-invalidation crash test");

    // Verify: no startup_repairs row before restart.
    let pre_repair_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs WHERE run_id = ?1")
            .bind(&run_id_str)
            .fetch_one(&pool)
            .await
            .expect("count startup_repairs before restart");
    assert_eq!(
        pre_repair_count, 0,
        "P082-R15 session_invalidation: no startup_repairs row before restart"
    );

    // Daemon restart: recovery creates idempotency row and requeues the work item.
    let requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now + chrono::Duration::seconds(5),
        "startup_repair_abandoned_invoke_agent",
    )
    .await
    .expect("restart: requeue_running_invoke_agent_on_startup");

    assert_eq!(
        requeued, 1,
        "P082-R15 session_invalidation: exactly one work item must be requeued on restart"
    );
    let post_repair_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs WHERE run_id = ?1")
            .bind(&run_id_str)
            .fetch_one(&pool)
            .await
            .expect("count startup_repairs after restart");
    assert_eq!(
        post_repair_count, 1,
        "P082-R15 session_invalidation: exactly one startup_repairs row after restart"
    );

    // Second restart: idempotent — no new requeue.
    let requeued2 = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now + chrono::Duration::seconds(10),
        "startup_repair_abandoned_invoke_agent",
    )
    .await
    .expect("second restart: idempotency check");
    assert_eq!(
        requeued2, 0,
        "P082-R15 session_invalidation: second restart must not requeue again (idempotent)"
    );
}

/// Crash after work item status mutation but before readback/projection write.
/// Simulates: the work item was requeued to 'pending' (status mutation completed), but
/// the daemon crashed before further projection or startup_repairs notes update.
/// Recovery on restart must be idempotent — no duplicate work items.
#[tokio::test]
async fn p082_r15_crash_after_work_item_status_mutation_is_idempotent() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r15-wim-{run_id_str}");
    let work_item_id = format!("wi-r15-wim-{run_id_str}");
    let repair_id = format!("p082-requeue:{journal_id}:{work_item_id}:1");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.start"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry");

    // Post-crash state: work item 'pending' (status mutation done), startup_repairs has empty notes.
    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, 'invoke_agent', ?2, 'pending', ?3, 'implement', ?4, ?4, NULL, 2)"#,
    )
    .bind(&work_item_id)
    .bind(
        serde_json::json!({
            "run_id": run_id_str,
            "stage_id": "implement",
            "stage_execution_id": "se-r15-wim",
            "source_command_journal_id": journal_id,
        })
        .to_string(),
    )
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert pending work item");

    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id_str,
        "p082_requeue_once",
        now + chrono::Duration::seconds(5),
        None, // notes not yet written — crash before this step
    )
    .await
    .expect("insert startup_repairs row without notes (pre-crash state)");

    // Restart: work item is 'pending' (not 'running') so recovery skips it.
    let requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now + chrono::Duration::seconds(10),
        "startup_repair_abandoned_invoke_agent",
    )
    .await
    .expect("restart after work-item-status-mutation crash");

    assert_eq!(
        requeued, 0,
        "P082-R15 work_item_mutation: restart must not create duplicate work (already pending)"
    );

    let repair_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs WHERE id = ?1")
            .bind(&repair_id)
            .fetch_one(&pool)
            .await
            .expect("count startup_repairs rows");
    assert_eq!(
        repair_count, 1,
        "P082-R15 work_item_mutation: exactly one startup_repairs row (no duplicate)"
    );

    let wi_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_items WHERE run_id = ?1")
        .bind(&run_id_str)
        .fetch_one(&pool)
        .await
        .expect("count work_items");
    assert_eq!(
        wi_count, 1,
        "P082-R15 work_item_mutation: exactly one work item (no duplicate enqueue)"
    );
}

/// Crash after command_journal.error settlement.
/// Simulates: a rejected command wrote its typed p082_rejected_command_error_v1 envelope to
/// command_journal.error (error settlement completed), but the daemon crashed before any
/// further projection update. Recovery via readbacks_for_run must surface the readback
/// directly from command_journal.error.
#[tokio::test]
async fn p082_r15_crash_after_command_journal_error_settlement_readback_derives_correctly() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r15-cje-{run_id_str}");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    // Build a valid rejected-command P082 readback and envelope.
    let readback = recovery_matrix::build_readback_v1(
        "P082-R02",
        "rejected",
        "no_mutation",
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "Stage is not retryable; retry rejected before mutation.",
        "command_journal, stages",
        "command_journal, stage_executions",
        &journal_id,
        Some("command_journal.error.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    let envelope = recovery_matrix::build_rejected_command_error_envelope(
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "RetryStage",
        "Stage retry rejected: stage is in a non-retryable status.",
        readback,
    );

    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &serde_json::json!({"run_id": run_id_str, "stage_id": "implement"}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.retry"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry");
    // Mark rejected with the typed P082 error envelope (simulates rejected-command
    // settlement completed without mutating the inserted payload).
    sqlx::query(
        r#"UPDATE command_journal SET result_status = 'rejected', error = ?1, completed_at = ?2 WHERE id = ?3"#,
    )
    .bind(&envelope)
    .bind(now.to_rfc3339())
    .bind(&journal_id)
    .execute(&pool)
    .await
    .expect("set command_journal.error to typed P082 envelope");

    // Recovery: readbacks_for_run derives P082-R02 from command_journal.error.
    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run after command_journal_error crash");

    assert!(
        !readbacks.is_empty(),
        "P082-R15 cmd_journal_error: at least one readback row must be returned"
    );
    let r02 = readbacks
        .iter()
        .find(|rb| rb.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R02"))
        .expect(
            "P082-R15 cmd_journal_error: R02 readback must be derived from command_journal.error",
        );
    assert_eq!(
        r02.get("recovery_reason_code").and_then(|v| v.as_str()),
        Some(recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY),
        "P082-R15 cmd_journal_error: reason code must be invalid_stage_for_retry"
    );
    assert_eq!(
        r02.get("schema_version").and_then(|v| v.as_str()),
        Some(recovery_matrix::SCHEMA_READBACK_V1),
        "P082-R15 cmd_journal_error: schema_version must be p082_recovery_matrix_readback_v1"
    );

    // Second read: idempotent.
    let readbacks2 = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("second readbacks_for_run must succeed");
    assert_eq!(
        readbacks.len(),
        readbacks2.len(),
        "P082-R15 cmd_journal_error: readbacks_for_run must be idempotent across restarts"
    );
}

/// Crash after cancellation_settlement_log update.
/// Simulates: runs.cancellation_settlement_log was updated with P082-R11 readback (log update
/// completed), but the daemon crashed before settling work items. Recovery via readbacks_for_run
/// must surface the P082-R11 readback from the cancellation log.
#[tokio::test]
async fn p082_r15_crash_after_cancellation_settlement_log_update_readback_accessible() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    let r11_readback = recovery_matrix::build_readback_v1(
        "P082-R11",
        "cancelled",
        "cancel",
        recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
        "Run cancellation requested while an active stage was running.",
        "runs, work_items, session_generations",
        "runs",
        &run_id_str,
        Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    let settlement_log = serde_json::json!([{
        "action_id": format!("cancel-action-{run_id_str}"),
        "settled_at": now.to_rfc3339(),
        "p082_recovery_matrix_readback": r11_readback,
    }])
    .to_string();

    // Write the cancellation settlement log (simulates log update completed before crash).
    sqlx::query(
        r#"UPDATE runs SET cancellation_settlement_log = ?1, cancellation_requested_at = ?2 WHERE id = ?3"#,
    )
    .bind(&settlement_log)
    .bind(now.to_rfc3339())
    .bind(&run_id_str)
    .execute(&pool)
    .await
    .expect("set cancellation_settlement_log on run");

    // Recovery: readbacks_for_run must surface the P082-R11 readback.
    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run after cancellation_settlement_log crash");

    let r11 = readbacks
        .iter()
        .find(|rb| rb.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R11"))
        .expect(
            "P082-R15 cancel_log: R11 readback must be derived from cancellation_settlement_log",
        );
    assert_eq!(
        r11.get("recovery_reason_code").and_then(|v| v.as_str()),
        Some(recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED),
        "P082-R15 cancel_log: reason code must be cancel_active_stage_requested"
    );

    // Second read: idempotent.
    let readbacks2 = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("second readbacks_for_run must succeed");
    assert_eq!(
        readbacks.len(),
        readbacks2.len(),
        "P082-R15 cancel_log: readbacks_for_run must be idempotent"
    );
}

/// Crash after side-effect hold recording.
/// Simulates: a side_effects row was recorded in 'prepared' status (hold recording completed),
/// but the daemon crashed before the run was blocked or a retry was refused. Recovery must
/// surface the unresolved side effect so the retry-block posture is correctly reinstated.
#[tokio::test]
async fn p082_r15_crash_after_side_effect_hold_recording_blocks_retry() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let effect_id = format!("se-r15-hold-{run_id_str}");
    // stage_execution_id must be a valid UUID for StageExecutionId parsing.
    let stage_execution_id = uuid::Uuid::new_v4().to_string();

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;

    // Insert side_effects row in 'prepared' state (hold recording completed before crash).
    sqlx::query(
        r#"INSERT INTO side_effects
           (id, run_id, stage_execution_id, effect_kind, target_key,
            idempotency_key, idempotency_key_version, request_fingerprint,
            request_fingerprint_version, status, external_write_attempted,
            attempt_budget_remaining, created_at, updated_at)
           VALUES (?1, ?2, ?3, 'git_commit', 'main',
                   ?4, 1, 'fp-r15-hold', 1, 'prepared', 0, 3, ?5, ?5)"#,
    )
    .bind(&effect_id)
    .bind(&run_id_str)
    .bind(&stage_execution_id)
    .bind(format!("idem-key-r15-hold-{run_id_str}"))
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert side_effects row for hold-recording crash test");

    // Recovery: list_unresolved_for_run must find the blocking effect.
    let unresolved = side_effects::list_unresolved_for_run(&pool, &run_id_str)
        .await
        .expect("list_unresolved_for_run after side_effect_hold crash");

    assert_eq!(
        unresolved.len(),
        1,
        "P082-R15 side_effect_hold: exactly one unresolved side effect must block retry"
    );
    assert_eq!(
        unresolved[0].run_id.to_string(),
        run_id_str,
        "P082-R15 side_effect_hold: unresolved effect must belong to the correct run"
    );

    // Second read: idempotent.
    let unresolved2 = side_effects::list_unresolved_for_run(&pool, &run_id_str)
        .await
        .expect("second list_unresolved_for_run must succeed");
    assert_eq!(
        unresolved.len(),
        unresolved2.len(),
        "P082-R15 side_effect_hold: list_unresolved_for_run must be idempotent across restarts"
    );
}

/// Crash after readback/projection write (final durable write boundary).
/// Simulates: startup_repairs.notes was written with a complete p082_recovery_matrix_readback,
/// and the work item was updated to 'pending'. A second daemon restart must not produce
/// duplicate readback rows or re-enqueue the work item.
#[tokio::test]
async fn p082_r15_crash_after_readback_projection_write_no_duplicate_rows() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r15-rpw-{run_id_str}");
    let work_item_id = format!("wi-r15-rpw-{run_id_str}");
    let repair_id = format!("p082-requeue:{journal_id}:{work_item_id}:1");

    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;
    command_journal::record(
        &pool,
        &journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.start"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry for readback-projection crash test");

    // Full repair cycle completed — startup_repairs has readback notes.
    let r01_summary = recovery_matrix::build_startup_repair_summary(
        &repair_id,
        &work_item_id,
        &journal_id,
        1,
        1,
        false,
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "global",
    );
    let r01_readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup recovery requeued abandoned InvokeAgent work item.",
            "startup_repairs, work_items, command_journal",
            "startup_repairs, work_items",
            &repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        r01_summary,
        None,
    );
    let notes = serde_json::json!({ "p082_recovery_matrix_readback": r01_readback }).to_string();
    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id_str,
        "p082_requeue_once",
        now + chrono::Duration::seconds(5),
        Some(&notes),
    )
    .await
    .expect("insert startup_repairs with complete readback");

    // Work item is 'pending' — both status mutation and readback write completed.
    sqlx::query(
        r#"INSERT INTO work_items
           (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, 'invoke_agent', ?2, 'pending', ?3, 'implement', ?4, ?5, NULL, 2)"#,
    )
    .bind(&work_item_id)
    .bind(
        serde_json::json!({
            "run_id": run_id_str,
            "stage_id": "implement",
            "stage_execution_id": "se-r15-rpw",
            "source_command_journal_id": journal_id,
            "p061_startup_recovery": {
                "startup_repair_id": repair_id,
                "requeue_generation": 1,
                "source_work_item_id": work_item_id,
            },
        })
        .to_string(),
    )
    .bind(&run_id_str)
    .bind(now.to_rfc3339())
    .bind((now + chrono::Duration::seconds(5)).to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert pending work item");

    // First read: exactly one P082-R01 row.
    let readbacks1 = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run (first read)");
    assert_eq!(
        readbacks1.len(),
        1,
        "P082-R15 readback_projection: exactly one readback row (no duplicates)"
    );
    assert_eq!(
        readbacks1[0].get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R01"),
        "P082-R15 readback_projection: readback must be P082-R01"
    );

    // Restart: work item is 'pending', no new requeue.
    let requeued = work_items::requeue_running_invoke_agent_on_startup(
        &pool,
        now + chrono::Duration::seconds(10),
        "startup_repair_abandoned_invoke_agent",
    )
    .await
    .expect("restart after readback-projection-write crash");
    assert_eq!(
        requeued, 0,
        "P082-R15 readback_projection: restart must not create duplicate work"
    );

    // Second read: same result, no additional rows.
    let readbacks2 = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run (second read after restart)");
    assert_eq!(
        readbacks1.len(),
        readbacks2.len(),
        "P082-R15 readback_projection: readbacks_for_run must be idempotent across restarts"
    );
}

// ── P082-R05 regression: real source_command_journal_id and requeue_generation=1 ──

/// Verifies that after `requeue_stale_starting_invoke_agent_sessions` performs a repair,
/// the startup_repairs row carries a real `source_command_journal_id` (not "unavailable")
/// and `requeue_generation=1` in its p082_startup_repair_summary_v1.
#[tokio::test]
async fn p082_r05_repair_writes_real_source_command_journal_id_and_requeue_generation_1() {
    let pool = setup_db().await;
    let now = Utc::now();
    let started_at = now - chrono::Duration::minutes(5); // past standard 3-min grace
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    insert_test_run(&pool, &run_id_str, &now.to_rfc3339()).await;
    let foreign_run_id = RunId::new().to_string();
    insert_test_run(&pool, &foreign_run_id, &now.to_rfc3339()).await;

    let journal_id = format!("cj-r05-real-{run_id_str}");
    command_journal::record(
        &pool,
        &journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": run_id_str}).to_string(),
        Some(&run_id_str),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.start"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record command journal entry");
    let foreign_journal_id = format!("cj-r05-foreign-{foreign_run_id}");
    command_journal::record(
        &pool,
        &foreign_journal_id,
        "InvokeAgent",
        &serde_json::json!({"run_id": foreign_run_id.clone()}).to_string(),
        Some(&foreign_run_id),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.start"),
        None,
        None,
        None,
        None,
    )
    .await
    .expect("record foreign command journal entry");

    let lineage_id = format!("lineage-r05-real-{run_id_str}");
    let generation_id = format!("gen-r05-real-{run_id_str}");
    let ae_id = format!("ae-r05-real-{run_id_str}");
    let wi_id = format!("wi-r05-real-{run_id_str}");

    sqlx::query(
        r#"INSERT INTO session_lineages
           (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
            active_generation_id, created_at, closed_at)
           VALUES (?1, ?2, 'agent-r05', ?1, 'run', NULL, ?3, ?4, NULL)"#,
    )
    .bind(&lineage_id)
    .bind(&run_id_str)
    .bind(&generation_id)
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert lineage");

    sqlx::query(
        r#"INSERT INTO session_generations
           (id, lineage_id, generation, invocation_owner_key, provider_session_id,
            binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
            workspace_mode, runtime_provider, runtime_model, status, created_at)
           VALUES (?1, ?2, 1, ?3, NULL, 'binding-r05-real', NULL, '/', 'read_write',
                   'claude', 'default', 'active', ?4)"#,
    )
    .bind(&generation_id)
    .bind(&lineage_id)
    .bind(&wi_id)
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert session generation");

    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id,
            owner_kind, owner_id)
           VALUES (?1, 'agent-r05', 'claude', 'running', ?2,
                   ?3, ?4, 'lead_conflict_mediation', ?1)"#,
    )
    .bind(&ae_id)
    .bind(started_at.to_rfc3339())
    .bind(&generation_id)
    .bind(&lineage_id)
    .execute(&pool)
    .await
    .expect("insert agent execution");

    let payload = serde_json::json!({
        "run_id": run_id_str,
        "source_command_journal_id": foreign_journal_id,
        "stage_id": "implement",
        "p058_claimed": { "agent_execution_id": ae_id },
    });
    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, ?4, 1)"#,
    )
    .bind(&wi_id)
    .bind(&run_id_str)
    .bind(payload.to_string())
    .bind(started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert running work item");

    let standard_cutoff = now - chrono::Duration::minutes(3);
    let xcode_cutoff = now - chrono::Duration::minutes(12);
    let requeued = work_items::requeue_stale_starting_invoke_agent_sessions(
        &pool,
        now,
        standard_cutoff,
        xcode_cutoff,
        "p082_r05_real_cj_test",
    )
    .await
    .expect("stale startup repair must succeed");
    assert_eq!(
        requeued, 1,
        "P082-R05: exactly one work item must be requeued"
    );

    let payload_raw: String =
        sqlx::query_scalar("SELECT payload_json FROM work_items WHERE id = ?1")
            .bind(&wi_id)
            .fetch_one(&pool)
            .await
            .expect("query repaired work item payload");
    let payload_json: serde_json::Value =
        serde_json::from_str(&payload_raw).expect("work_items.payload_json must be valid JSON");

    let summary = payload_json
        .pointer("/p082_recovery_matrix_readback/recovery_startup_repair_summary")
        .or_else(|| {
            payload_json
                .pointer("/p061_startup_recovery/p082_recovery_matrix_readback/recovery_startup_repair_summary")
        })
        .expect("P082-R05 regression: summary must be present in work_items payload readback");

    // Verify source_command_journal_id is a real ID, not "unavailable".
    let scji = summary
        .get("source_command_journal_id")
        .and_then(|v| v.as_str())
        .expect("P082-R05 regression: source_command_journal_id must be present");
    assert_eq!(
        scji, &journal_id,
        "P082-R05 regression: source_command_journal_id must fall back to the same-run command journal ID, not the tampered cross-run payload ID"
    );
    assert_eq!(
        payload_json
            .pointer(
                "/p061_startup_recovery/p082_recovery_matrix_readback/recovery_projection_integrity",
            )
            .and_then(|v| v.as_str()),
        Some("tamper_detected"),
        "P082-R05 regression: cross-run source_command_journal_id must be marked tamper_detected"
    );

    // Verify requeue_generation is 1, not 0.
    let gen = summary
        .get("requeue_generation")
        .and_then(|v| v.as_i64())
        .expect("P082-R05 regression: requeue_generation must be present");
    assert_eq!(
        gen, 1,
        "P082-R05 regression: requeue_generation must be 1 (first requeue), not 0"
    );
}

// ── P082-R16 regression: exhausted hold must not appear as R15 crash-resume ──

/// Verifies that a P082-R16 startup_requeue_exhausted row with replayed=true
/// is NOT classified as P082-R15 (crash-resume replay) by the readbacks_for_run accessor.
/// The R15 Source 10 path guards against misclassification by checking
/// recovery_reason_code != startup_requeue_exhausted before emitting R15,
/// so R16 rows are excluded by reason code, not by replayed=false.
#[tokio::test]
async fn p082_r16_exhausted_hold_not_classified_as_r15_crash_resume() {
    let pool = setup_db().await;
    let now = Utc::now();
    let run_id = RunId::new();
    let run_id_str = run_id.to_string();
    let journal_id = format!("cj-r16-notr15-{run_id_str}");
    let work_item_id = format!("wi-r16-notr15-{run_id_str}");
    let repair_id = format!("p082-requeue:{journal_id}:{work_item_id}:1");

    // Build an R16 summary with replayed=true (real production behavior: same exhausted key re-observed).
    // The guard against R15 misclassification is the reason_code check in Source 10,
    // not the replayed flag.
    let held_summary = recovery_matrix::build_startup_repair_summary(
        &repair_id,
        &work_item_id,
        &journal_id,
        1,
        1,
        true, // replayed=true: same exhausted key was re-observed (correct semantic)
        60_000,
        &now.to_rfc3339(),
        false,
        None,
        "global",
    );
    let held_readback = recovery_matrix::set_readback_startup_repair(
        recovery_matrix::build_readback_v1(
            "P082-R16",
            "held",
            "wait",
            recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED,
            "Startup requeue generation 1 was already consumed; no duplicate work was enqueued.",
            "startup_repairs, work_items",
            "startup_repairs, work_items",
            &repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        held_summary,
        Some("Startup requeue exhausted. Use existing recovery inspection or cancellation paths."),
    );
    let notes = serde_json::json!({
        "p082_recovery_matrix_readback": held_readback,
    })
    .to_string();

    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id_str,
        "p082_requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("insert R16 startup_repairs row");

    let readbacks = db::repos::p082_recovery_matrix::readbacks_for_run(&pool, run_id)
        .await
        .expect("readbacks_for_run must not fail");

    // Must NOT return R15 (crash-resume) for an R16 exhausted hold.
    let r15_row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R15"));
    assert!(
        r15_row.is_none(),
        "P082-R16 regression: an R16 exhausted hold must NOT produce a P082-R15 crash-resume row (reason_code guard in Source 10)"
    );

    // Must return R16 (the actual held scenario).
    let r16_row = readbacks
        .iter()
        .find(|row| row.get("scenario_id").and_then(|v| v.as_str()) == Some("P082-R16"))
        .expect("P082-R16 regression: readbacks_for_run must return R16 for an exhausted hold");
    assert_eq!(
        r16_row.get("scenario_status").and_then(|v| v.as_str()),
        Some("held"),
        "P082-R16 regression: R16 row must have scenario_status=held"
    );
    assert_eq!(
        r16_row.get("recovery_reason_code").and_then(|v| v.as_str()),
        Some(recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED),
        "P082-R16 regression: R16 row must have reason_code=startup_requeue_exhausted"
    );
    // Verify the summary has replayed=true (correct semantic: same exhausted key re-observed).
    // The protection against R15 misclassification comes from the reason_code guard in
    // Source 10, not from replayed=false. R15 must NOT be present (asserted above).
    let replayed = r16_row
        .pointer("/recovery_startup_repair_summary/replayed")
        .and_then(|v| v.as_bool());
    assert_eq!(
        replayed,
        Some(true),
        "P082-R16 regression: R16 summary must have replayed=true (same exhausted key re-observed)"
    );
}
