//! P082: MCP operator readback lane proof.
//!
//! Verifies that runs.get includes both p082_recovery_matrix_readback (singular)
//! and p082_recovery_matrix_readbacks (plural), and that reports.get includes
//! p082_recovery_matrix_readbacks (plural) but NOT p082_recovery_matrix_readback
//! (singular).

use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{artifacts, ideas, runs};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ArtifactId, IdeaId, RunId};
use domain::run::{Run, RunStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::work_queue::WorkQueue;
use mcp_server::tools::{reports, runs as mcp_runs};

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    pool
}

fn command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
    let events = event_bus::new_bus(64);
    CommandHandler::new(pool.clone(), events, WorkQueue::new(pool))
}

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-p082".into(),
        workflow_title: "P082 Readback Test".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("implement".into()),
        workflow_yaml_path: None,
        agent_catalog_yaml_path: None,
        worktree_root: None,
        base_branch: None,
        base_revision: None,
        target_branch: None,
        delivery_configuration_json: None,
        delivery_preflight_json: None,
        workflow_family: None,
        project_key: None,
        risk_class: None,
        stack: None,
        workflow_snapshot_hash: None,
        catalog_snapshot_hash: None,
        workflow_snapshot_json: None,
        catalog_snapshot_json: None,
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: None,
        review_routing_json: None,
        closeout_readiness_mode: None,
    }
}

async fn seed_run(pool: &sqlx::SqlitePool) -> RunId {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P082 readback".into(),
            body: "P082 readback test".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    run_id
}

/// P082: runs.get must include both singular and plural P082 readback fields.
#[tokio::test]
async fn p082_runs_get_includes_singular_and_plural_readback() {
    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    let result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("runs.get must succeed");

    // Singular: must be present (null when no rows, but key must exist)
    assert!(
        result.get("p082_recovery_matrix_readback").is_some(),
        "P082: runs.get must include p082_recovery_matrix_readback (singular, may be null)"
    );

    // Plural: must be present (empty array when no rows, but key must exist)
    assert!(
        result.get("p082_recovery_matrix_readbacks").is_some(),
        "P082: runs.get must include p082_recovery_matrix_readbacks (plural)"
    );

    // Plural must be an array
    assert!(
        result["p082_recovery_matrix_readbacks"].is_array(),
        "P082: p082_recovery_matrix_readbacks must be an array"
    );
}

/// P082: reports.get mcp_execution_truth report must include plural but NOT singular P082 readback field.
#[tokio::test]
async fn p082_reports_get_includes_plural_not_singular() {
    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    let result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("reports.get must succeed");

    // reports.get returns an array of report objects
    let reports_array = result
        .as_array()
        .expect("P082: reports.get must return an array");

    // Find the mcp_execution_truth report — this is where P082 readbacks live
    let mcp_truth = reports_array
        .iter()
        .find(|r| r["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("P082: reports.get must include mcp_execution_truth report");

    // Plural: must be present inside mcp_execution_truth
    assert!(
        mcp_truth.get("p082_recovery_matrix_readbacks").is_some(),
        "P082: reports.get mcp_execution_truth must include p082_recovery_matrix_readbacks (plural)"
    );

    // Plural must be an array
    assert!(
        mcp_truth["p082_recovery_matrix_readbacks"].is_array(),
        "P082: reports.get p082_recovery_matrix_readbacks must be an array"
    );

    // Singular: must NOT be present inside mcp_execution_truth
    assert!(
        mcp_truth.get("p082_recovery_matrix_readback").is_none(),
        "P082: reports.get mcp_execution_truth must NOT include singular p082_recovery_matrix_readback"
    );
}

/// P082: When a startup_repair row carries a valid p082_recovery_matrix_readback in its notes,
/// runs.get must return a non-null singular readback and a non-empty plural array.
#[tokio::test]
async fn p082_runs_get_returns_non_empty_readback_when_startup_repair_row_exists() {
    use db::repos::startup_repairs;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    // Build a valid P082 readback for P082-R01 (startup_requeue_once).
    let readback = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled; startup_repairs row created with requeue_generation=1.",
        "startup_repairs",
        "startup_repairs, work_items",
        &format!("p082-requeue:cj-mcp-test:{run_id}:1"),
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T10:03:01Z",
    );

    // Embed the readback in the startup_repair notes JSON.
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();

    let repair_id = format!("p082-requeue:cj-mcp-test:{run_id}:1");
    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id.to_string(),
        "requeue_once",
        Utc::now(),
        Some(&notes),
    )
    .await
    .expect("seed startup_repair with P082 readback");

    // runs.get must return a non-null singular readback.
    let result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("runs.get must succeed");

    let singular = result
        .get("p082_recovery_matrix_readback")
        .expect("P082: runs.get must include p082_recovery_matrix_readback");
    assert!(
        !singular.is_null(),
        "P082: singular p082_recovery_matrix_readback must be non-null when a startup_repair row exists"
    );
    assert_eq!(
        singular.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R01"),
        "P082: singular readback must have scenario_id=P082-R01"
    );
    assert_eq!(
        singular.get("scenario_status").and_then(|v| v.as_str()),
        Some("repaired"),
        "P082: singular readback must have scenario_status=repaired (approved vocabulary)"
    );
    assert_eq!(
        singular.get("recovery_decision").and_then(|v| v.as_str()),
        Some("retry"),
        "P082: singular readback must have recovery_decision=retry (approved vocabulary)"
    );
    assert_eq!(
        singular
            .get("recovery_reason_code")
            .and_then(|v| v.as_str()),
        Some("startup_requeue_once"),
        "P082: singular readback must have correct reason_code"
    );

    // Plural array must have exactly one element.
    let plural = result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("P082: p082_recovery_matrix_readbacks must be an array");
    assert_eq!(
        plural.len(),
        1,
        "P082: plural readbacks must contain exactly one row when one startup_repair row exists"
    );

    // reports.get mcp_execution_truth must also expose the non-empty plural.
    let reports_result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("reports.get must succeed");

    let reports_array = reports_result
        .as_array()
        .expect("reports.get returns array");
    let mcp_truth = reports_array
        .iter()
        .find(|r| r["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("P082: mcp_execution_truth report must exist");

    let report_readbacks = mcp_truth
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("P082: mcp_execution_truth must have p082_recovery_matrix_readbacks array");
    assert_eq!(
        report_readbacks.len(),
        1,
        "P082: reports.get mcp_execution_truth p082_recovery_matrix_readbacks must be non-empty"
    );
}

/// P082: When a rejected command_journal entry carries a p082_rejected_command_error_v1 envelope,
/// runs.get must return a non-null singular readback with recovery_decision=no_mutation.
#[tokio::test]
async fn p082_runs_get_returns_readback_from_command_journal_rejected_envelope() {
    use db::repos::command_journal;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
    let now = Utc::now();

    // Write a rejected command with a p082_rejected_command_error_v1 envelope.
    let journal_id = format!("p082-mcp-rejected-{run_id}");
    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &serde_json::json!({"run_id": run_id.to_string(), "stage_id": "implement"}).to_string(),
        Some(&run_id.to_string()),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.retry"),
        None,
    )
    .await
    .expect("record command journal entry");

    let readback = recovery_matrix::build_readback_v1(
        "P082-R02",
        "rejected",
        "no_mutation",
        recovery_matrix::REASON_INVALID_STAGE_FOR_RETRY,
        "Stage is not in a retryable status. No mutation was performed.",
        "command_journal",
        "command_journal, stages",
        &journal_id,
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
    command_journal::fail_entry(&pool, &journal_id, now, &envelope)
        .await
        .expect("fail command journal entry with p082 envelope");

    // runs.get must return a non-null singular readback from the rejected command.
    let result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("runs.get must succeed");

    let singular = result
        .get("p082_recovery_matrix_readback")
        .expect("P082: runs.get must include p082_recovery_matrix_readback");
    assert!(
        !singular.is_null(),
        "P082: singular must be non-null when command_journal has rejected P082 envelope"
    );
    assert_eq!(
        singular.get("scenario_id").and_then(|v| v.as_str()),
        Some("P082-R02"),
        "P082: readback from rejected command must have scenario_id=P082-R02"
    );
    assert_eq!(
        singular.get("recovery_decision").and_then(|v| v.as_str()),
        Some("no_mutation"),
        "P082: rejected command readback must have recovery_decision=no_mutation"
    );
    assert_eq!(
        singular
            .get("recovery_reason_code")
            .and_then(|v| v.as_str()),
        Some("invalid_stage_for_retry"),
        "P082: rejected command readback must have correct reason_code"
    );

    // Verify command_journal.payload_json was not mutated.
    let payload: Option<String> =
        sqlx::query_scalar("SELECT payload_json FROM command_journal WHERE id = ?1")
            .bind(&journal_id)
            .fetch_optional(&pool)
            .await
            .expect("fetch payload_json");
    let payload_str = payload.expect("payload_json must exist");
    let payload_v: serde_json::Value = serde_json::from_str(&payload_str).unwrap();
    assert_eq!(
        payload_v.get("schema_version"),
        None,
        "P082: command_journal.payload_json must not contain schema_version (not mutated for readback)"
    );
}

/// P082: Legacy plain-text command_journal.error is surfaced as a safe fallback row
/// with recovery_projection_integrity=unavailable, not silently dropped.
#[tokio::test]
async fn p082_runs_get_includes_legacy_fallback_row_with_unavailable_integrity() {
    use db::repos::command_journal;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
    let now = Utc::now();

    // Write a failed command with a legacy plain-text error (not a P082 envelope)
    let journal_id = format!("p082-mcp-legacy-{run_id}");
    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &serde_json::json!({"run_id": run_id.to_string(), "stage_id": "deploy"}).to_string(),
        Some(&run_id.to_string()),
        now,
        Some("mcp"),
        Some("operator-1"),
        Some("operator"),
        Some("runs.retry"),
        None,
    )
    .await
    .expect("record command journal entry");

    // Legacy plain-text error (pre-P082 format — no schema_version)
    command_journal::fail_entry(
        &pool,
        &journal_id,
        now,
        "Stage deploy is not retryable: current status is running",
    )
    .await
    .expect("fail command journal with legacy plain text");

    // runs.get must include the fallback row with unavailable integrity
    let result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("runs.get must succeed");

    let plural = result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("P082: p082_recovery_matrix_readbacks must be an array");

    assert_eq!(
        plural.len(),
        1,
        "P082: legacy plain-text error must surface as one fallback row in p082_recovery_matrix_readbacks"
    );

    let row = &plural[0];
    assert_eq!(
        row.get("recovery_projection_integrity")
            .and_then(|v| v.as_str()),
        Some("unavailable"),
        "P082: legacy fallback row must have recovery_projection_integrity=unavailable"
    );
    assert_eq!(
        row.get("scenario_status").and_then(|v| v.as_str()),
        Some("held"),
        "P082: legacy fallback row must have scenario_status=held"
    );
    // Raw error text must not appear in the surfaced row
    let row_str = row.to_string();
    assert!(
        !row_str.contains("not retryable"),
        "P082: legacy fallback row must not expose raw error text"
    );
    assert!(
        !row_str.contains("current status is running"),
        "P082: legacy fallback row must not expose raw error detail"
    );
}

/// P082: Injected sensitive keys in durable readback rows are stripped before
/// being returned to MCP callers (SEC-P082-001 regression test).
#[tokio::test]
async fn p082_runs_get_strips_injected_sensitive_keys_from_startup_repair_notes() {
    use db::repos::startup_repairs;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    // Build a readback with injected sensitive keys
    let readback = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs, work_items",
        &format!("p082-requeue:cj-sec-mcp:{run_id}:1"),
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T10:03:01Z",
    );

    let mut readback_obj = readback.as_object().cloned().unwrap();
    readback_obj.insert(
        "access_token".to_string(),
        serde_json::json!("leaked-bearer-token"),
    );
    readback_obj.insert(
        "raw_stderr".to_string(),
        serde_json::json!("sensitive provider output"),
    );
    let injected_readback = serde_json::Value::Object(readback_obj);

    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": injected_readback,
    })
    .to_string();

    let repair_id = format!("p082-requeue:cj-sec-mcp:{run_id}:1");
    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id.to_string(),
        "requeue_once",
        Utc::now(),
        Some(&notes),
    )
    .await
    .expect("seed startup_repair with injected readback");

    let result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("runs.get must succeed");

    let plural = result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("P082: p082_recovery_matrix_readbacks must be an array");

    assert_eq!(plural.len(), 1, "P082: must return exactly one row");
    let row_str = serde_json::to_string(&plural[0]).unwrap();

    // SEC-P082-001: injected keys must be stripped
    assert!(
        !row_str.contains("access_token"),
        "P082 SEC-P082-001: access_token must be stripped from MCP response"
    );
    assert!(
        !row_str.contains("leaked-bearer-token"),
        "P082 SEC-P082-001: bearer token value must be stripped from MCP response"
    );
    assert!(
        !row_str.contains("raw_stderr"),
        "P082 SEC-P082-001: raw_stderr must be stripped from MCP response"
    );
    // Legitimate field must survive
    assert!(
        row_str.contains("\"scenario_id\":\"P082-R01\""),
        "P082: scenario_id must survive allowlist projection in MCP response"
    );
}

/// P082 cross-surface parity: runs.get p082_recovery_matrix_readbacks and reports.get
/// mcp_execution_truth p082_recovery_matrix_readbacks must agree on row count and scenario IDs.
#[tokio::test]
async fn p082_runs_get_and_reports_get_readbacks_parity() {
    use db::repos::startup_repairs;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    // Seed two rows: one startup repair (R01) and one rejected command (R07).
    let now = chrono::Utc::now();

    // R01 row via startup_repairs.notes
    let r01_readback = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs, work_items",
        &format!("p082-requeue:cj-parity:{run_id}:1"),
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-22T00:00:00Z",
    );
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": r01_readback,
    })
    .to_string();
    startup_repairs::record(
        &pool,
        &format!("p082-requeue:cj-parity:{run_id}:1"),
        &run_id.to_string(),
        "requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("seed R01 startup repair");

    // R07 row via command_journal.error
    use db::repos::command_journal;
    let journal_id = format!("p082-parity-r07-{run_id}");
    command_journal::record(
        &pool,
        &journal_id,
        "RetryStage",
        &serde_json::json!({"run_id": run_id.to_string(), "stage_id": "release"}).to_string(),
        Some(&run_id.to_string()),
        now,
        Some("mcp"),
        None,
        Some("operator"),
        Some("runs.retry"),
        None,
    )
    .await
    .expect("record command journal");
    let r07_readback = recovery_matrix::set_readback_side_effect_hold(
        recovery_matrix::build_readback_v1(
            "P082-R07",
            "held",
            "reconcile_side_effects",
            recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION,
            "Reconcile unresolved side effects before retrying.",
            "side_effects, command_journal",
            "side_effects, command_journal",
            &journal_id,
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        ),
        "unresolved_side_effect_entries",
        "Retry blocked: unresolved side-effect ledger entries exist.",
    );
    let envelope = recovery_matrix::build_rejected_command_error_envelope(
        recovery_matrix::REASON_REQUIRES_EFFECT_RECONCILIATION,
        "RetryStage",
        "Retry blocked due to side effects.",
        r07_readback,
    );
    command_journal::fail_entry(&pool, &journal_id, now, &envelope)
        .await
        .expect("fail command journal with R07 envelope");

    // Query runs.get
    let runs_result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("runs.get must succeed");

    let runs_plural = runs_result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("runs.get must have p082_recovery_matrix_readbacks");

    // Query reports.get
    let reports_result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("reports.get must succeed");

    let reports_array = reports_result
        .as_array()
        .expect("reports.get returns array");
    let mcp_truth = reports_array
        .iter()
        .find(|r| r["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("mcp_execution_truth report must exist");

    let reports_plural = mcp_truth
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("reports.get mcp_execution_truth must have p082_recovery_matrix_readbacks");

    // Parity: both lanes must return the same number of rows.
    assert_eq!(
        runs_plural.len(),
        reports_plural.len(),
        "P082 parity: runs.get and reports.get mcp_execution_truth must have the same number of readback rows"
    );
    assert_eq!(
        runs_plural.len(),
        2,
        "P082 parity: both lanes must return exactly 2 rows (R01 + R07)"
    );

    // Parity: scenario_ids in both lanes must agree.
    let mut runs_ids: Vec<&str> = runs_plural
        .iter()
        .filter_map(|r| r.get("scenario_id").and_then(|v| v.as_str()))
        .collect();
    let mut reports_ids: Vec<&str> = reports_plural
        .iter()
        .filter_map(|r| r.get("scenario_id").and_then(|v| v.as_str()))
        .collect();
    runs_ids.sort_unstable();
    reports_ids.sort_unstable();
    assert_eq!(
        runs_ids, reports_ids,
        "P082 parity: scenario_ids from runs.get and reports.get must match"
    );
    assert!(
        runs_ids.contains(&"P082-R01"),
        "P082 parity: P082-R01 must be present in both lanes"
    );
    assert!(
        runs_ids.contains(&"P082-R07"),
        "P082 parity: P082-R07 must be present in both lanes"
    );

    // Lane contract: reports.get must NOT have singular p082_recovery_matrix_readback.
    assert!(
        mcp_truth.get("p082_recovery_matrix_readback").is_none(),
        "P082 parity: reports.get must NOT expose singular p082_recovery_matrix_readback"
    );
    // Lane contract: runs.get MUST have singular p082_recovery_matrix_readback (latest applicable).
    assert!(
        runs_result.get("p082_recovery_matrix_readback").is_some(),
        "P082 parity: runs.get must include singular p082_recovery_matrix_readback"
    );
}

/// P082 SEC-P082-HIGH-1: Agent principal must receive null/empty for p082 readback fields on runs.get.
/// Recovery readbacks contain session/work-item identifiers and operator messages that must
/// not be exposed to lower-privilege principals.
#[tokio::test]
async fn p082_runs_get_agent_principal_receives_null_and_empty_readbacks() {
    use db::repos::startup_repairs;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());

    // Seed a valid P082 startup repair row so Operator would see real data.
    let readback = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs, work_items",
        &format!("p082-requeue:cj-agent-authz:{run_id}:1"),
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T10:03:01Z",
    );
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();
    startup_repairs::record(
        &pool,
        &format!("p082-requeue:cj-agent-authz:{run_id}:1"),
        &run_id.to_string(),
        "requeue_once",
        chrono::Utc::now(),
        Some(&notes),
    )
    .await
    .expect("seed startup_repair");

    // Agent principal: p082 readbacks must be null/empty.
    let agent = auth::Principal::new("test-agent", auth::PrincipalClass::Agent);
    let agent_result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &agent,
    )
    .await
    .expect("runs.get must succeed for Agent principal");

    assert!(
        agent_result
            .get("p082_recovery_matrix_readback")
            .map_or(true, |v| v.is_null()),
        "P082 SEC-HIGH-1: Agent principal must receive null for p082_recovery_matrix_readback"
    );
    let agent_plural = agent_result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array());
    assert!(
        agent_plural.map_or(true, |a| a.is_empty()),
        "P082 SEC-HIGH-1: Agent principal must receive empty array for p082_recovery_matrix_readbacks"
    );

    // Observer principal: same gating.
    let observer = auth::Principal::new("test-observer", auth::PrincipalClass::Observer);
    let observer_result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &observer,
    )
    .await
    .expect("runs.get must succeed for Observer principal");

    assert!(
        observer_result
            .get("p082_recovery_matrix_readback")
            .map_or(true, |v| v.is_null()),
        "P082 SEC-HIGH-1: Observer principal must receive null for p082_recovery_matrix_readback"
    );
    let observer_plural = observer_result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array());
    assert!(
        observer_plural.map_or(true, |a| a.is_empty()),
        "P082 SEC-HIGH-1: Observer principal must receive empty array for p082_recovery_matrix_readbacks"
    );

    // Operator principal: must still receive real data.
    let operator = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
    let operator_result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &operator,
    )
    .await
    .expect("runs.get must succeed for Operator principal");

    let operator_singular = operator_result
        .get("p082_recovery_matrix_readback")
        .expect("Operator must see p082_recovery_matrix_readback");
    assert!(
        !operator_singular.is_null(),
        "P082 SEC-HIGH-1: Operator must receive non-null p082_recovery_matrix_readback"
    );
    let operator_plural = operator_result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("Operator must see p082_recovery_matrix_readbacks array");
    assert!(
        !operator_plural.is_empty(),
        "P082 SEC-HIGH-1: Operator must receive non-empty p082_recovery_matrix_readbacks"
    );
}

/// P082 SEC-P082-HIGH-1: Agent principal must receive empty readbacks on reports.get.
#[tokio::test]
async fn p082_reports_get_agent_principal_receives_empty_readbacks() {
    use db::repos::startup_repairs;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());

    // Seed a valid P082 startup repair row.
    let readback = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs, work_items",
        &format!("p082-requeue:cj-agent-reports:{run_id}:1"),
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        "2026-05-21T10:03:01Z",
    );
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();
    startup_repairs::record(
        &pool,
        &format!("p082-requeue:cj-agent-reports:{run_id}:1"),
        &run_id.to_string(),
        "requeue_once",
        chrono::Utc::now(),
        Some(&notes),
    )
    .await
    .expect("seed startup_repair");

    let agent = auth::Principal::new("test-agent", auth::PrincipalClass::Agent);
    let result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &agent,
    )
    .await
    .expect("reports.get must succeed for Agent principal");

    let reports_array = result.as_array().expect("reports.get returns array");
    let mcp_truth = reports_array
        .iter()
        .find(|r| r["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("mcp_execution_truth report must exist");

    let readbacks = mcp_truth
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("p082_recovery_matrix_readbacks must be an array");
    assert!(
        readbacks.is_empty(),
        "P082 SEC-HIGH-1: Agent principal must receive empty p082_recovery_matrix_readbacks in reports.get"
    );
}

#[tokio::test]
async fn p082_reports_get_run_report_artifact_includes_plural_readbacks() {
    use db::repos::startup_repairs;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let now = chrono::Utc::now();
    let repair_id = format!("p082-requeue:cj-run-report:{run_id}:1");
    let readback = recovery_matrix::build_readback_v1(
        "P082-R01",
        "repaired",
        "retry",
        recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
        "Startup requeue scheduled.",
        "startup_repairs",
        "startup_repairs, work_items",
        &repair_id,
        Some("startup_repairs.notes.p082_recovery_matrix_readback"),
        "valid",
        &now.to_rfc3339(),
    );
    let notes = serde_json::json!({
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();
    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id.to_string(),
        "p082_requeue_once",
        now,
        Some(&notes),
    )
    .await
    .expect("seed startup repair");

    artifacts::insert(
        &pool,
        &Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_12_workflow_complete".to_string(),
            agent_id: "lead_orchestrator".to_string(),
            name: "run_report".to_string(),
            contract_id: "run_report_v1".to_string(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/p082-run-report.json".to_string(),
            checksum_sha256: None,
            size_bytes: Some(2),
            provider: "system".to_string(),
            model: None,
            created_at: now,
            is_pinned: false,
            report_kind: Some("run_report".to_string()),
            report_version: Some(1),
            agent_execution_id: None,
        },
    )
    .await
    .expect("insert run_report artifact");

    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
    let result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("reports.get must succeed");
    let reports_array = result.as_array().expect("reports.get returns array");
    let run_report = reports_array
        .iter()
        .find(|report| report["name"] == serde_json::json!("run_report"))
        .expect("reports.get must include the generated run_report artifact");
    let readbacks = run_report
        .get("p082_recovery_matrix_readbacks")
        .and_then(|value| value.as_array())
        .expect("run_report artifact must include p082_recovery_matrix_readbacks");
    assert_eq!(
        readbacks.len(),
        1,
        "P082: run_report artifact lane must expose the same plural readback contract"
    );
    assert_eq!(
        readbacks[0].get("scenario_id").and_then(|value| value.as_str()),
        Some("P082-R01")
    );
}
