//! P082: MCP operator readback lane proof.
//!
//! Verifies that runs.get includes both p082_recovery_matrix_readback (singular)
//! and p082_recovery_matrix_readbacks (plural), and that reports.get includes
//! p082_recovery_matrix_readbacks (plural) but NOT p082_recovery_matrix_readback
//! (singular).

use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{artifact_contracts, artifacts, ideas, runs, startup_repairs};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::artifact_contracts::{ActiveArtifactGenerationInput, ArtifactContractOverrideInput};
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
        workspace_root: "cw-test/ws".into(),
        artifact_root: "cw-test/artifacts".into(),
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

fn p082_startup_repair_summary(
    repair_id: &str,
    source_work_item_id: &str,
    source_command_journal_id: &str,
) -> serde_json::Value {
    domain::recovery_matrix::build_startup_repair_summary(
        repair_id,
        source_work_item_id,
        source_command_journal_id,
        1,
        1,
        false,
        180_000,
        "2026-05-21T10:03:00Z",
        false,
        None,
        "run",
    )
}

fn p082_attach_startup_summary(
    readback: serde_json::Value,
    repair_id: &str,
    source_work_item_id: &str,
    source_command_journal_id: &str,
) -> serde_json::Value {
    domain::recovery_matrix::set_readback_startup_repair(
        readback,
        p082_startup_repair_summary(repair_id, source_work_item_id, source_command_journal_id),
        None,
    )
}

async fn seed_p082_readback_for_reason(
    pool: &sqlx::SqlitePool,
    run_id: RunId,
    index: usize,
    reason_code: &str,
) {
    let repair_id = format!("p082-reason-{index:02}-{run_id}");
    let readback = p082_attach_startup_summary(
        domain::recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "wait",
            reason_code,
            "Operator readback coverage for canonical P082 recovery reason code.",
            "startup_repairs",
            "startup_repairs",
            &repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T10:03:01Z",
        ),
        &repair_id,
        &format!("wi-reason-{index:02}"),
        &format!("cj-reason-{index:02}"),
    );
    assert!(
        domain::recovery_matrix::validate_readback_v1_shape(&readback),
        "seed reason readback must validate: {readback}"
    );
    let notes = serde_json::json!({
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();
    startup_repairs::record(
        pool,
        &repair_id,
        &run_id.to_string(),
        "p082_reason_coverage",
        Utc::now() + chrono::Duration::seconds(index as i64),
        Some(&notes),
    )
    .await
    .expect("seed P082 reason readback");
}

fn p082_reason_set(readbacks: &[serde_json::Value]) -> std::collections::BTreeSet<String> {
    readbacks
        .iter()
        .filter_map(|readback| {
            readback
                .get("recovery_reason_code")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .collect()
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

#[tokio::test]
async fn p082_runs_get_singular_matches_latest_plural_dynamic_readback() {
    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
    let stale_at = Utc::now() - chrono::Duration::minutes(5);
    let work_item_id = format!("wi-p082-dynamic-r06-{run_id}");
    let side_effect_id = format!("se-p082-dynamic-r06-{run_id}");
    let stage_execution_id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, ?4, 1)"#,
    )
    .bind(&work_item_id)
    .bind(run_id.to_string())
    .bind(serde_json::json!({"run_id": run_id.to_string()}).to_string())
    .bind(stale_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert stale running InvokeAgent work item");
    sqlx::query(
        r#"INSERT INTO side_effects
           (id, run_id, stage_execution_id, effect_kind, target_key,
            idempotency_key, idempotency_key_version, request_fingerprint,
            request_fingerprint_version, status, external_write_attempted,
            attempt_budget_remaining, created_at, updated_at)
           VALUES (?1, ?2, ?3, 'git_commit', 'p082-dynamic-r06',
                   ?4, 1, 'fp-p082-dynamic-r06', 1, 'prepared', 0, 3, ?5, ?5)"#,
    )
    .bind(&side_effect_id)
    .bind(run_id.to_string())
    .bind(stage_execution_id)
    .bind(format!("idem-p082-dynamic-r06-{run_id}"))
    .bind(Utc::now().to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert unresolved side-effect evidence for held R06");

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
        .expect("runs.get must include singular p082 readback");
    let plural = result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|value| value.as_array())
        .expect("runs.get must include plural p082 readbacks");
    let latest = plural
        .iter()
        .filter(|row| {
            row.get("scenario_status").and_then(|value| value.as_str()) != Some("not_applicable")
        })
        .last()
        .expect("dynamic stale work item must produce a latest P082 row");

    assert_eq!(
        latest.get("scenario_id").and_then(|value| value.as_str()),
        Some("P082-R06"),
        "P082 dynamic setup must produce the stale-work R06 readback"
    );
    assert_eq!(
        singular, latest,
        "P082 runs.get singular readback must be selected from the same plural readback snapshot"
    );
}

#[tokio::test]
async fn p082_runs_cancel_rejects_missing_caller_request_id_before_mutation() {
    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    let err = mcp_runs::execute(
        "runs.cancel",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect_err("runs.cancel must reject missing caller_request_id");

    assert!(
        err.to_string().contains("Missing 'caller_request_id'"),
        "missing caller_request_id must be rejected before CancelRun mutation, got {err}"
    );
    let journal_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM command_journal")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        journal_count, 0,
        "missing idempotency_key must not write a CancelRun command journal entry"
    );
}

#[tokio::test]
async fn p082_mcp_readbacks_cover_all_recovery_reason_codes() {
    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    for (index, reason_code) in domain::recovery_matrix::ALL_REASON_CODES.iter().enumerate() {
        seed_p082_readback_for_reason(&pool, run_id, index, reason_code).await;
    }

    let expected: std::collections::BTreeSet<String> = domain::recovery_matrix::ALL_REASON_CODES
        .iter()
        .map(|reason| (*reason).to_string())
        .collect();

    let runs_result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("runs.get must succeed");
    let runs_readbacks = runs_result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|value| value.as_array())
        .expect("runs.get p082_recovery_matrix_readbacks");
    assert_eq!(
        p082_reason_set(runs_readbacks),
        expected,
        "runs.get must expose every canonical P082 recovery reason code"
    );

    let reports_result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("reports.get must succeed");
    let reports_readbacks = reports_result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|value| value.as_array())
        .expect("reports.get p082_recovery_matrix_readbacks");
    assert_eq!(
        p082_reason_set(reports_readbacks),
        expected,
        "reports.get must expose every canonical P082 recovery reason code"
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

    assert!(
        result.get("p082_recovery_matrix_readbacks").is_some(),
        "P082: reports.get must include result-level p082_recovery_matrix_readbacks"
    );
    assert!(
        result.get("p082_recovery_matrix_readback").is_none(),
        "P082: reports.get must not include result-level singular p082_recovery_matrix_readback"
    );

    // reports.get returns an object with a reports array plus result-level P082 diagnostics.
    let reports_array = result
        .get("reports")
        .and_then(|value| value.as_array())
        .expect("P082: reports.get must return a reports array");

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
    let repair_id = format!("p082-requeue:cj-mcp-test:{run_id}:1");
    let readback = p082_attach_startup_summary(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled; startup_repairs row created with requeue_generation=1.",
            "startup_repairs",
            "startup_repairs, work_items",
            &repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T10:03:01Z",
        ),
        &repair_id,
        "wi-mcp-test",
        "cj-mcp-test",
    );

    // Embed the readback in the startup_repair notes JSON.
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();

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
        .get("reports")
        .and_then(|value| value.as_array())
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

#[tokio::test]
async fn p082_runs_get_singular_is_latest_row_from_same_plural_snapshot() {
    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
    let now = Utc::now();
    let stale_started_at = now - chrono::Duration::minutes(4);
    let lineage_id = format!("p082-mcp-r05-lineage-{run_id}");
    let generation_id = format!("p082-mcp-r05-generation-{run_id}");
    let owner_key = format!("p082-mcp-r05-owner-{run_id}");

    sqlx::query(
        r#"INSERT INTO session_lineages
           (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
            active_generation_id, created_at, closed_at)
           VALUES (?1, ?2, 'agent-r05-mcp', ?1, 'run', NULL, ?3, ?4, NULL)"#,
    )
    .bind(&lineage_id)
    .bind(run_id.to_string())
    .bind(&generation_id)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert session lineage");

    sqlx::query(
        r#"INSERT INTO session_generations
           (id, lineage_id, generation, invocation_owner_key, provider_session_id,
            binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
            workspace_mode, runtime_provider, runtime_model, status, created_at, last_activity_at)
           VALUES (?1, ?2, 1, ?3, NULL, 'binding-r05-mcp', NULL, '/', 'read_write',
                   'codex', 'gpt-5.5', 'active', ?4, NULL)"#,
    )
    .bind(&generation_id)
    .bind(&lineage_id)
    .bind(&owner_key)
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert stale session generation");

    sqlx::query(
        r#"INSERT INTO agent_executions
           (id, agent_id, provider, status, started_at,
            session_generation_id, session_lineage_id,
            owner_kind, owner_id)
           VALUES ('p082-mcp-r05-ae', 'agent-r05-mcp', 'codex', 'running', ?1,
                   ?2, ?3, 'lead_conflict_mediation', 'p082-mcp-r05-ae')"#,
    )
    .bind(stale_started_at.to_rfc3339())
    .bind(&generation_id)
    .bind(&lineage_id)
    .execute(&pool)
    .await
    .expect("insert stale startup agent execution");

    sqlx::query(
        r#"INSERT INTO work_items
           (id, run_id, kind, payload_json, status, created_at, scheduled_at, started_at, attempt_count)
           VALUES (?1, ?2, 'invoke_agent', ?3, 'running', ?4, ?4, ?4, 1)"#,
    )
    .bind(&owner_key)
    .bind(run_id.to_string())
    .bind(
        serde_json::json!({
            "run_id": run_id.to_string(),
            "p061_startup_recovery": { "reason": "startup_stalled" },
            "p058_claimed": { "agent_execution_id": "p082-mcp-r05-ae" }
        })
        .to_string(),
    )
    .bind(stale_started_at.to_rfc3339())
    .execute(&pool)
    .await
    .expect("insert stale startup work item owner");

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
        .expect("runs.get must include singular p082 readback");
    let plural = result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|value| value.as_array())
        .expect("runs.get must include plural p082 readbacks");
    let latest = plural
        .iter()
        .filter(|row| {
            row.get("scenario_status").and_then(|value| value.as_str()) != Some("not_applicable")
        })
        .last()
        .expect("dynamic stale startup row must be present");

    assert_eq!(
        singular, latest,
        "P082: runs.get singular readback must be selected from the same plural snapshot"
    );
    assert_eq!(
        singular.get("scenario_id").and_then(|value| value.as_str()),
        Some("P082-R05"),
        "P082: dynamic stale startup fixture must exercise R05"
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
        None,
        None,
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
    sqlx::query(
        r#"UPDATE command_journal SET result_status = 'rejected', error = ?1, completed_at = ?2 WHERE id = ?3"#,
    )
    .bind(&envelope)
    .bind(now.to_rfc3339())
    .bind(&journal_id)
    .execute(&pool)
    .await
    .expect("reject command journal entry with p082 envelope");

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
        None,
        None,
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
    let repair_id = format!("p082-requeue:cj-sec-mcp:{run_id}:1");
    let readback = p082_attach_startup_summary(
        recovery_matrix::build_readback_v1(
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
            "2026-05-21T10:03:01Z",
        ),
        &repair_id,
        "wi-sec-mcp",
        "cj-sec-mcp",
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
    let parity_repair_id = format!("p082-requeue:cj-parity:{run_id}:1");
    let r01_readback = p082_attach_startup_summary(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs, work_items",
            &parity_repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-22T00:00:00Z",
        ),
        &parity_repair_id,
        "wi-parity",
        "cj-parity",
    );
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": r01_readback,
    })
    .to_string();
    startup_repairs::record(
        &pool,
        &parity_repair_id,
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
        None,
        None,
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
        .get("reports")
        .and_then(|value| value.as_array())
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
    let authz_repair_id = format!("p082-requeue:cj-agent-authz:{run_id}:1");
    let readback = p082_attach_startup_summary(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs, work_items",
            &authz_repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T10:03:01Z",
        ),
        &authz_repair_id,
        "wi-agent-authz",
        "cj-agent-authz",
    );
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();
    startup_repairs::record(
        &pool,
        &authz_repair_id,
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
    let agent_reports_repair_id = format!("p082-requeue:cj-agent-reports:{run_id}:1");
    let readback = p082_attach_startup_summary(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs, work_items",
            &agent_reports_repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T10:03:01Z",
        ),
        &agent_reports_repair_id,
        "wi-agent-reports",
        "cj-agent-reports",
    );
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();
    startup_repairs::record(
        &pool,
        &agent_reports_repair_id,
        &run_id.to_string(),
        "requeue_once",
        chrono::Utc::now(),
        Some(&notes),
    )
    .await
    .expect("seed startup_repair");

    let agent = auth::Principal::new("test-agent", auth::PrincipalClass::Agent);
    let error = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &agent,
    )
    .await
    .expect_err("reports.get must reject Agent principal before reading report lanes")
    .to_string();
    assert!(
        error.contains("requires Operator"),
        "P082 SEC-HIGH-1: Agent principal must be denied before reports.get lanes load; got {error}"
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
    let readback = p082_attach_startup_summary(
        recovery_matrix::build_readback_v1(
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
        ),
        &repair_id,
        "wi-run-report",
        "cj-run-report",
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
    let reports_get_metric_before = db::metrics::get_counter_with_label(
        "p082_recovery_reason_readback_total",
        &format!(
            "{}:reports.get",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE
        ),
    );
    let run_report_metric_before = db::metrics::get_counter_with_label(
        "p082_recovery_reason_readback_total",
        &format!(
            "{}:run_report",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE
        ),
    );
    let result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("reports.get must succeed");
    let reports_array = result["reports"]
        .as_array()
        .expect("reports.get returns array");
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
        readbacks[0]
            .get("scenario_id")
            .and_then(|value| value.as_str()),
        Some("P082-R01")
    );
    assert!(
        db::metrics::get_counter_with_label(
            "p082_recovery_reason_readback_total",
            &format!(
                "{}:reports.get",
                recovery_matrix::REASON_STARTUP_REQUEUE_ONCE
            ),
        ) > reports_get_metric_before,
        "P082: reports.get must emit the container-level recovery reason lane metric"
    );
    assert!(
        db::metrics::get_counter_with_label(
            "p082_recovery_reason_readback_total",
            &format!(
                "{}:run_report",
                recovery_matrix::REASON_STARTUP_REQUEUE_ONCE
            ),
        ) > run_report_metric_before,
        "P082: embedded run_report artifact must emit its own recovery reason lane metric"
    );
}

#[tokio::test]
async fn p082_reports_get_run_report_artifact_empty_for_agent_and_observer() {
    use db::repos::startup_repairs;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let now = chrono::Utc::now();
    let repair_id = format!("p082-requeue:cj-run-report-authz:{run_id}:1");
    let readback = p082_attach_startup_summary(
        recovery_matrix::build_readback_v1(
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
        ),
        &repair_id,
        "wi-run-report-authz",
        "cj-run-report-authz",
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
            file_path: "/tmp/p082-run-report-agent.json".to_string(),
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

    for principal_class in [auth::PrincipalClass::Agent, auth::PrincipalClass::Observer] {
        let principal = auth::Principal::new("test-non-operator", principal_class);
        let error = reports::execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .expect_err("reports.get must reject non-Operator principal before reading report lanes")
        .to_string();
        assert!(
            error.contains("requires Operator"),
            "P082 SEC-HIGH-1: non-Operator run_report artifacts must not be reachable; got {error}"
        );
    }
}

/// P082 SEC-P082-SEC-001: Embedded absolute filesystem paths in allowed readback fields
/// must be redacted before Operator-lane exposure (runs.get, reports.get, run_report artifact).
/// Covers top-level fields, string arrays, nested subcontract strings, and file:// URIs.
#[tokio::test]
async fn p082_runs_get_redacts_embedded_absolute_paths_in_allowed_fields() {
    use db::repos::startup_repairs;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    // Build a readback with embedded absolute paths in top-level allowed fields,
    // string arrays, and nested subcontract values.
    let path_repair_id = format!("p082-requeue:cj-path-sec:{run_id}:1");
    let mut readback = p082_attach_startup_summary(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            // Embedded path in recovery_next_action (allowed field)
            "Inspect /Users/alice/Documents/run-output.txt for details.",
            "startup_repairs",
            "startup_repairs, work_items",
            &path_repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T10:03:01Z",
        ),
        &path_repair_id,
        "wi-path-sec",
        "cj-path-sec",
    )
    .as_object()
    .cloned()
    .unwrap();

    // Embed an absolute path inside recovery_hold_conditions (string array)
    readback.insert(
        "recovery_hold_conditions".to_string(),
        serde_json::json!([
            "Resolve /home/user/project/lock before retrying.",
            "Check path=/var/run/chainworks/state for stale locks.",
        ]),
    );
    // Embed a file:// URI inside recovery_operator_message (allowed field)
    readback.insert(
        "recovery_operator_message".to_string(),
        serde_json::json!("See file:///private/var/log/chainworks.log for context."),
    );

    let readback_val = serde_json::Value::Object(readback);
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback_val,
    })
    .to_string();

    let repair_id = format!("p082-requeue:cj-path-sec:{run_id}:1");
    startup_repairs::record(
        &pool,
        &repair_id,
        &run_id.to_string(),
        "p082_requeue_once",
        Utc::now(),
        Some(&notes),
    )
    .await
    .expect("seed startup repair with embedded paths");

    // runs.get: verify Operator lane does not expose any absolute paths
    let result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("runs.get must succeed");

    let result_str = serde_json::to_string(&result).unwrap();
    for forbidden in &[
        "/Users/",
        "/home/",
        "/private/",
        "/tmp/",
        "/var/",
        "/root/",
        "file://",
    ] {
        assert!(
            !result_str.contains(forbidden),
            "P082 SEC-P082-SEC-001: runs.get must not expose embedded absolute path fragment '{}' to Operator",
            forbidden
        );
    }
    // P082-R01 row must still be present (redaction must not drop the row)
    let plural = result
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("p082_recovery_matrix_readbacks must be present");
    assert_eq!(
        plural.len(),
        1,
        "P082 SEC: sanitized row must still be returned"
    );

    // reports.get: same guarantee for the mcp_execution_truth lane
    let reports_result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("reports.get must succeed");

    let reports_str = serde_json::to_string(&reports_result).unwrap();
    for forbidden in &[
        "/Users/",
        "/home/",
        "/private/",
        "/tmp/",
        "/var/",
        "/root/",
        "file://",
    ] {
        assert!(
            !reports_str.contains(forbidden),
            "P082 SEC-P082-SEC-001: reports.get must not expose embedded absolute path fragment '{}'",
            forbidden
        );
    }
}

/// P082 release_receipt lane: reports.get delivery_receipt artifact must include
/// p082_recovery_matrix_readbacks and no recovery command affordances.
#[tokio::test]
async fn p082_release_receipt_lane_includes_readbacks_and_no_command_affordances() {
    use db::repos::startup_repairs;
    use domain::recovery_matrix;

    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    // Seed a P082-R01 readback via startup_repairs.
    let readback = p082_attach_startup_summary(
        recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue for release_receipt lane test.",
            "startup_repairs",
            "startup_repairs, work_items",
            &format!("p082-requeue:cj-release:{run_id}:1"),
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T10:03:01Z",
        ),
        &format!("p082-requeue:cj-release:{run_id}:1"),
        "wi-release",
        "cj-release",
    );
    let notes = serde_json::json!({
        "requeue_generation": 1,
        "max_requeue_generation": 1,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string();
    startup_repairs::record(
        &pool,
        &format!("p082-requeue:cj-release:{run_id}:1"),
        &run_id.to_string(),
        "requeue_once",
        Utc::now(),
        Some(&notes),
    )
    .await
    .expect("seed startup repair for release_receipt test");

    // Seed a delivery_receipt artifact.
    let artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_release".into(),
        agent_id: "release-agent".into(),
        name: "delivery_receipt".into(),
        contract_id: "delivery_receipt".into(),
        format: ArtifactFormat::Json,
        file_path: "/tmp/p082-release-receipt-test.json".into(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "claude".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
        agent_execution_id: None,
    };
    artifacts::insert(&pool, &artifact)
        .await
        .expect("insert delivery_receipt artifact");

    let reports_get_metric_before = db::metrics::get_counter_with_label(
        "p082_recovery_reason_readback_total",
        &format!(
            "{}:reports.get",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE
        ),
    );
    let release_receipt_metric_before = db::metrics::get_counter_with_label(
        "p082_recovery_reason_readback_total",
        &format!(
            "{}:release_receipt",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE
        ),
    );
    let result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .expect("reports.get must succeed");

    let reports_array = result
        .get("reports")
        .and_then(|v| v.as_array())
        .expect("reports.get must return a reports array");

    let delivery = reports_array
        .iter()
        .find(|r| r["name"] == serde_json::json!("delivery_receipt"))
        .expect("P082: delivery_receipt report must be present");

    // Plural readbacks field must be present and non-empty.
    let readbacks = delivery
        .get("p082_recovery_matrix_readbacks")
        .and_then(|v| v.as_array())
        .expect("P082: delivery_receipt must include p082_recovery_matrix_readbacks array");
    assert!(
        !readbacks.is_empty(),
        "P082: delivery_receipt p082_recovery_matrix_readbacks must be non-empty when a startup_repair row exists"
    );

    // No recovery command affordances: the readback must not expose mutation commands.
    let delivery_str = serde_json::to_string(delivery).unwrap();
    for affordance in &["RetryStage", "CancelRun", "RetryRun"] {
        assert!(
            !delivery_str.contains(affordance),
            "P082: delivery_receipt must not expose recovery command affordance '{affordance}'"
        );
    }
    assert!(
        db::metrics::get_counter_with_label(
            "p082_recovery_reason_readback_total",
            &format!(
                "{}:reports.get",
                recovery_matrix::REASON_STARTUP_REQUEUE_ONCE
            ),
        ) > reports_get_metric_before,
        "P082: reports.get must emit the container-level recovery reason lane metric for release receipt payloads"
    );
    assert!(
        db::metrics::get_counter_with_label(
            "p082_recovery_reason_readback_total",
            &format!(
                "{}:release_receipt",
                recovery_matrix::REASON_STARTUP_REQUEUE_ONCE
            ),
        ) > release_receipt_metric_before,
        "P082: embedded release receipt artifact must emit its own recovery reason lane metric"
    );
}

/// SEC-HIGH-002: Agent and Observer principals must not receive sensitive Run fields
/// (absolute paths, delivery/snapshot configs, operator-only overrides) on runs.get.
#[tokio::test]
async fn sec_high_002_runs_get_redacts_sensitive_fields_for_non_operator() {
    use db::repos::{ideas, runs};
    use domain::idea::{Idea, IdeaStatus};
    use domain::run::RunStatus;

    let pool = test_pool().await;
    let handler = command_handler(pool.clone());

    // Seed a run with sensitive fields populated.
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "SEC-HIGH-002 redaction test".into(),
            body: "sensitive run fields must be redacted".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: chrono::Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(
        &pool,
        &domain::run::Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-sec-high-002".into(),
            workflow_title: "SEC-HIGH-002 test".into(),
            workspace_root: "/Users/user/Documents/SecretProject".into(),
            artifact_root: "/Users/user/.chainworks/artifacts/secret".into(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: Some("secret settlement log".into()),
            current_state: Some("implement".into()),
            workflow_yaml_path: Some("/Users/user/workflows/secret.yaml".into()),
            agent_catalog_yaml_path: Some("/Users/user/agents/secret.yaml".into()),
            worktree_root: Some("/Users/user/.chainworks/worktrees/secret".into()),
            base_branch: Some("main".into()),
            base_revision: Some("abc123".into()),
            target_branch: Some("cw/secret/abc123".into()),
            delivery_configuration_json: Some("{\"secret\":\"delivery\"}".into()),
            delivery_preflight_json: Some("{\"secret\":\"preflight\"}".into()),
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: Some("sha256-secret-hash".into()),
            catalog_snapshot_hash: Some("sha256-catalog-hash".into()),
            workflow_snapshot_json: Some("{\"secret\":\"workflow\"}".into()),
            catalog_snapshot_json: Some("{\"secret\":\"catalog\"}".into()),
            drift_detected_at: None,
            drift_details_json: Some("{\"secret\":\"drift\"}".into()),
            chainworks_meta_root: Some("/Users/user/.chainworks/runs/secret".into()),
            review_routing_json: Some("{\"secret\":\"routing\"}".into()),
            closeout_readiness_mode: None,
        },
    )
    .await
    .unwrap();

    let sensitive_fields = [
        "workspace_root",
        "artifact_root",
        "workflow_yaml_path",
        "agent_catalog_yaml_path",
        "worktree_root",
        "base_branch",
        "base_revision",
        "target_branch",
        "delivery_configuration_json",
        "delivery_preflight_json",
        "workflow_snapshot_json",
        "catalog_snapshot_json",
        "workflow_snapshot_hash",
        "catalog_snapshot_hash",
        "drift_details_json",
        "chainworks_meta_root",
        "review_routing_json",
        "cancellation_settlement_log",
        "operator_overrides",
        "legacy_discovery_overrides",
        "retry_authority",
        "retry_authority_history",
        "p091_orphan_repair_readback",
    ];

    for class in [auth::PrincipalClass::Agent, auth::PrincipalClass::Observer] {
        let principal = auth::Principal::new("test-non-op", class.clone());
        let result = mcp_runs::execute(
            "runs.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .expect("runs.get must succeed");

        for field in &sensitive_fields {
            assert!(
                result.get(*field).is_none(),
                "SEC-HIGH-002: {:?} principal must not see '{}' in runs.get result",
                class,
                field
            );
        }

        // Safe fields must still be present.
        assert!(
            result.get("id").is_some(),
            "SEC-HIGH-002: 'id' must remain for {:?}",
            class
        );
        assert!(
            result.get("status").is_some(),
            "SEC-HIGH-002: 'status' must remain for {:?}",
            class
        );
        assert!(
            result.get("workflow_title").is_some(),
            "SEC-HIGH-002: 'workflow_title' must remain for {:?}",
            class
        );
        // p082 readbacks must be null/empty for non-Operator.
        assert!(
            result
                .get("p082_recovery_matrix_readback")
                .map_or(true, |v| v.is_null()),
            "SEC-HIGH-002: {:?} must receive null p082_recovery_matrix_readback",
            class
        );
    }

    // Operator must still see sensitive fields.
    let operator = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
    let op_result = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &operator,
    )
    .await
    .expect("runs.get must succeed for Operator");
    assert!(
        op_result.get("workspace_root").is_some(),
        "SEC-HIGH-002: Operator must still see workspace_root"
    );
    assert!(
        op_result.get("delivery_configuration_json").is_some(),
        "SEC-HIGH-002: Operator must still see delivery_configuration_json"
    );
    assert!(
        op_result.get("cancellation_settlement_log").is_some(),
        "SEC-HIGH-002: Operator must still see cancellation_settlement_log"
    );
}

/// SEC-P082-HIGH-001: reports.get is broadly readable, but the
/// canonical_artifact_contracts system report contains operator-only routing
/// and override state. Non-Operator principals must not receive that report.
#[tokio::test]
async fn sec_high_001_reports_get_omits_canonical_artifact_contracts_for_non_operator() {
    let pool = test_pool().await;
    let run_id = seed_run(&pool).await;
    let handler = command_handler(pool.clone());

    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "/Users/user/private/review/prepush.json".into(),
            raw_status: "block".into(),
            generation_id: "sec-high-001-gen".into(),
            source_agent_execution_id: None,
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();
    artifact_contracts::create_override_and_rebuild(
        &pool,
        ArtifactContractOverrideInput {
            run_id,
            contract_id: "prepush_review_v1".into(),
            override_type: "status".into(),
            from_status: "block".into(),
            to_status: "pass".into(),
            reason: "operator verified sensitive override".into(),
            owner: "operator".into(),
            source_artifacts: vec![],
            expires_at_stage: "state_11_manual_release".into(),
            journal_id: "sec-high-001-journal".into(),
        },
    )
    .await
    .unwrap();

    for class in [auth::PrincipalClass::Agent, auth::PrincipalClass::Observer] {
        let principal = auth::Principal::new("test-non-op", class.clone());
        let error = reports::execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .expect_err("reports.get must reject non-operators before sensitive report lanes load")
        .to_string();
        assert!(
            error.contains("requires Operator"),
            "SEC-P082-HIGH-001: {:?} must be denied before canonical_artifact_contracts can load; got {error}",
            class
        );
    }

    let operator = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
    let op_result = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &operator,
    )
    .await
    .expect("reports.get must succeed for Operator");
    let canonical = op_result
        .get("reports")
        .and_then(serde_json::Value::as_array)
        .or_else(|| op_result.as_array())
        .unwrap()
        .iter()
        .find(|report| report["report_kind"] == "canonical_artifact_contracts")
        .expect("Operator must retain canonical_artifact_contracts diagnostics");
    assert_eq!(
        canonical["operator_overrides"][0]["reason"],
        "operator verified sensitive override"
    );
}
