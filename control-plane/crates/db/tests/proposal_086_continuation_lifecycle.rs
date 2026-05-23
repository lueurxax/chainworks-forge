use db::pool::create_pool;
use db::repos::agent_work_continuations::{self, ContinuationAdmission, ContinuationClaimOutcome};
use sqlx::Row;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .expect("test shared DbWriter registration failed");
    pool
}

async fn seed_minimal_continuation(pool: &sqlx::SqlitePool, continuation_id: &str) {
    let now = "2026-05-22T00:00:00Z";
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-1','idea','body','active',?)")
        .bind(now)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at,chainworks_meta_root)
         VALUES ('run-1','idea-1','running','wf','Workflow','/tmp/ws','/tmp/art',?,'/tmp/cw-run')",
    )
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO stage_executions (id,run_id,stage_id,label,status,started_at)
         VALUES ('stage-exec-1','run-1','state_1','Stage','completed',?)",
    )
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO agent_executions (id,stage_execution_id,agent_id,provider,status,started_at,completed_at,owner_kind,owner_id)
         VALUES ('agent-exec-1','stage-exec-1','code_writer','claude','completed',?,?,'stage_execution','stage-exec-1')",
    )
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();

    let admission = ContinuationAdmission {
        continuation_id: continuation_id.to_string(),
        command_journal_id: format!("cmd-{continuation_id}"),
        run_id: "run-1".to_string(),
        stage_execution_id: "stage-exec-1".to_string(),
        agent_execution_id: "agent-exec-1".to_string(),
        mode: "live_handle_continuation".to_string(),
        trigger_kind: "operator_mcp".to_string(),
        idempotency_scope: "scope-1".to_string(),
        idempotency_key: format!("key-{continuation_id}"),
        request_fingerprint_sha256: "a".repeat(64),
        lead_decision_artifact_id: None,
        lead_decision_artifact_sha256: None,
        continuation_instruction_sha256: None,
        budget_json: None,
        caller_principal_id: "operator".to_string(),
        caller_surface: "mcp".to_string(),
        caller_principal_class: "operator".to_string(),
        caller_tool: "agents.continue_work".to_string(),
        created_at: now.to_string(),
    };
    let outcome = agent_work_continuations::admit_continuation_atomic(pool, &admission, "{}")
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        agent_work_continuations::AtomicAdmissionOutcome::Accepted
    ));
}

fn p086_lead_auto_admission(
    continuation_id: &str,
    agent_execution_id: &str,
    idempotency_key: &str,
    fingerprint_char: char,
) -> ContinuationAdmission {
    ContinuationAdmission {
        continuation_id: continuation_id.to_string(),
        command_journal_id: format!("cmd-{continuation_id}"),
        run_id: "run-1".to_string(),
        stage_execution_id: "stage-exec-1".to_string(),
        agent_execution_id: agent_execution_id.to_string(),
        mode: "live_handle_continuation".to_string(),
        trigger_kind: "lead_auto".to_string(),
        idempotency_scope: agent_execution_id.to_string(),
        idempotency_key: idempotency_key.to_string(),
        request_fingerprint_sha256: fingerprint_char.to_string().repeat(64),
        lead_decision_artifact_id: Some(format!("artifact-{continuation_id}")),
        lead_decision_artifact_sha256: Some("b".repeat(64)),
        continuation_instruction_sha256: Some("c".repeat(64)),
        budget_json: None,
        caller_principal_id: "lead_auto_agent:lead-exec-1".to_string(),
        caller_surface: "engine".to_string(),
        caller_principal_class: "agent".to_string(),
        caller_tool: "lead_auto_orchestration".to_string(),
        created_at: "2026-05-22T00:00:00Z".to_string(),
    }
}

#[tokio::test]
async fn p086_side_effect_guard_detects_unresolved_stage_effects() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-side-effect-guard").await;

    assert!(
        !agent_work_continuations::has_unresolved_side_effects_for_stage(
            &pool,
            "run-1",
            "stage-exec-1",
        )
        .await
        .unwrap(),
        "stage should start with no unresolved P078 side effects"
    );

    sqlx::query(
        "INSERT INTO side_effects
         (id, run_id, stage_execution_id, effect_kind, target_key,
          idempotency_key, request_fingerprint, status, created_at, updated_at)
         VALUES
         ('effect-1', 'run-1', 'stage-exec-1', 'git_push', 'origin/main',
          'effect-key-1', ?, 'needs_reconciliation', ?, ?)",
    )
    .bind("b".repeat(64))
    .bind("2026-05-22T00:00:00Z")
    .bind("2026-05-22T00:00:00Z")
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        agent_work_continuations::has_unresolved_side_effects_for_stage(
            &pool,
            "run-1",
            "stage-exec-1",
        )
        .await
        .unwrap(),
        "unresolved P078 side effects must block P086 continuation admission"
    );
}

#[tokio::test]
async fn p086_lead_auto_limits_count_terminal_rows_atomically() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-seed").await;
    sqlx::query("UPDATE agent_work_continuations SET status='succeeded' WHERE id='cont-seed'")
        .execute(&pool)
        .await
        .unwrap();

    let first = p086_lead_auto_admission("cont-lead-1", "agent-exec-1", "lead-key-1", 'b');
    let outcome = agent_work_continuations::admit_continuation_atomic(&pool, &first, "{}")
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        agent_work_continuations::AtomicAdmissionOutcome::Accepted
    ));
    sqlx::query("UPDATE agent_work_continuations SET status='succeeded' WHERE id='cont-lead-1'")
        .execute(&pool)
        .await
        .unwrap();

    let same_agent = p086_lead_auto_admission("cont-lead-2", "agent-exec-1", "lead-key-2", 'c');
    let outcome = agent_work_continuations::admit_continuation_atomic(&pool, &same_agent, "{}")
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        agent_work_continuations::AtomicAdmissionOutcome::LeadAutoLimitExceeded {
            scope: "agent_execution",
            current_count: 1,
            max_count: 1
        }
    ));

    for agent_execution_id in ["agent-exec-2", "agent-exec-3"] {
        sqlx::query(
            "INSERT INTO agent_executions
             (id,stage_execution_id,agent_id,provider,status,started_at,completed_at,owner_kind,owner_id)
             VALUES (?,'stage-exec-1','code_writer','claude','completed',
                     '2026-05-22T00:00:00Z','2026-05-22T00:00:00Z','stage_execution','stage-exec-1')",
        )
        .bind(agent_execution_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let second_stage =
        p086_lead_auto_admission("cont-lead-stage-2", "agent-exec-2", "lead-key-3", 'd');
    let outcome = agent_work_continuations::admit_continuation_atomic(&pool, &second_stage, "{}")
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        agent_work_continuations::AtomicAdmissionOutcome::Accepted
    ));
    sqlx::query(
        "UPDATE agent_work_continuations SET status='succeeded' WHERE id='cont-lead-stage-2'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let third_stage =
        p086_lead_auto_admission("cont-lead-stage-3", "agent-exec-3", "lead-key-4", 'e');
    let outcome = agent_work_continuations::admit_continuation_atomic(&pool, &third_stage, "{}")
        .await
        .unwrap();
    assert!(matches!(
        outcome,
        agent_work_continuations::AtomicAdmissionOutcome::LeadAutoLimitExceeded {
            scope: "stage_execution",
            current_count: 2,
            max_count: 2
        }
    ));
}

#[tokio::test]
async fn p086_claim_refuses_prompt_sent_replay_before_provider_io() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-replay").await;

    agent_work_continuations::insert_side_effect_ledger_row(
        &pool,
        "cont-replay",
        "key-cont-replay",
        "provider_send",
        3,
        "committed",
        Some(&"a".repeat(64)),
        None,
    )
    .await
    .unwrap();
    agent_work_continuations::update_continuation_status(&pool, "cont-replay", "prompt_sent", None)
        .await
        .unwrap();

    let claim = agent_work_continuations::claim_for_continuation_worker(
        &pool,
        "cont-replay",
        4242,
        "daemon-gen-1",
        "session-gen-1",
    )
    .await
    .unwrap();

    assert!(matches!(
        claim,
        ContinuationClaimOutcome::PromptAlreadySent(_)
    ));
    let status: String =
        sqlx::query_scalar("SELECT status FROM agent_work_continuations WHERE id='cont-replay'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        status, "prompt_sent",
        "claim must not rewind prompt_sent rows to queued"
    );
}

#[tokio::test]
async fn p086_terminal_settlement_persists_artifact_ids_and_response_hash() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-terminal").await;

    agent_work_continuations::settle_with_artifacts(
        &pool,
        "cont-terminal",
        "succeeded",
        None,
        "response-artifact-id",
        &"b".repeat(64),
        "result-artifact-id",
    )
    .await
    .unwrap();

    let row = sqlx::query(
        "SELECT status, response_artifact_id, response_fingerprint_sha256, result_or_no_progress_artifact_id
         FROM agent_work_continuations WHERE id='cont-terminal'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.get::<String, _>("status"), "succeeded");
    assert_eq!(
        row.get::<String, _>("response_artifact_id"),
        "response-artifact-id"
    );
    assert_eq!(
        row.get::<String, _>("response_fingerprint_sha256"),
        "b".repeat(64)
    );
    assert_eq!(
        row.get::<String, _>("result_or_no_progress_artifact_id"),
        "result-artifact-id"
    );
}

#[tokio::test]
async fn p086_durable_metric_events_are_recorded_and_summarized() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-metrics").await;

    agent_work_continuations::record_p086_continuation_metric_event(
        &pool,
        Some("run-1"),
        Some("stage-exec-1"),
        Some("agent-exec-1"),
        Some("cont-metrics"),
        "continuation_settlement_total",
        serde_json::json!({
            "mode": "live_handle_continuation",
            "trigger_kind": "operator_mcp",
            "terminal_status": "succeeded"
        }),
        1,
    )
    .await
    .unwrap();
    agent_work_continuations::record_p086_continuation_metric_event(
        &pool,
        Some("run-1"),
        Some("stage-exec-1"),
        Some("agent-exec-1"),
        Some("cont-metrics"),
        "continuation_fresh_session_avoided_total",
        serde_json::json!({"outcome": "reused_existing_session"}),
        1,
    )
    .await
    .unwrap();
    agent_work_continuations::record_p086_continuation_metric_event(
        &pool,
        Some("run-1"),
        Some("stage-exec-1"),
        Some("agent-exec-1"),
        Some("cont-metrics"),
        "continuation_changed_files_total",
        serde_json::json!({"terminal_status": "succeeded"}),
        3,
    )
    .await
    .unwrap();
    agent_work_continuations::record_p086_continuation_metric_event(
        &pool,
        Some("run-1"),
        Some("stage-exec-1"),
        Some("agent-exec-1"),
        Some("cont-metrics"),
        "continuation_tests_or_gates_total",
        serde_json::json!({"terminal_status": "succeeded"}),
        2,
    )
    .await
    .unwrap();
    agent_work_continuations::record_p086_continuation_metric_event(
        &pool,
        Some("run-1"),
        Some("stage-exec-1"),
        Some("agent-exec-1"),
        Some("cont-metrics"),
        "continuation_tests_passed_total",
        serde_json::json!({"terminal_status": "succeeded"}),
        1,
    )
    .await
    .unwrap();
    agent_work_continuations::record_p086_continuation_metric_event(
        &pool,
        Some("run-1"),
        Some("stage-exec-1"),
        Some("agent-exec-1"),
        Some("cont-metrics"),
        "continuation_useful_progress_total",
        serde_json::json!({"terminal_status": "succeeded"}),
        1,
    )
    .await
    .unwrap();
    agent_work_continuations::record_p086_continuation_metric_event(
        &pool,
        Some("run-1"),
        Some("stage-exec-1"),
        Some("agent-exec-1"),
        Some("cont-metrics"),
        "continuation_followup_validation_total",
        serde_json::json!({"validation_outcome": "success"}),
        1,
    )
    .await
    .unwrap();
    agent_work_continuations::record_p086_continuation_metric_event(
        &pool,
        Some("run-1"),
        Some("stage-exec-1"),
        Some("agent-exec-1"),
        Some("cont-metrics"),
        "continuation_time_saved_seconds",
        serde_json::json!({"estimate_source": "fresh_retry_estimate_seconds"}),
        120,
    )
    .await
    .unwrap();
    for (metric_name, budget_dimension, value) in [
        (
            "continuation_provider_session_budget_input_tokens_total",
            "input_tokens",
            100,
        ),
        (
            "continuation_provider_session_budget_output_tokens_total",
            "output_tokens",
            40,
        ),
        (
            "continuation_provider_session_budget_cached_input_tokens_total",
            "cached_input_tokens",
            20,
        ),
        (
            "continuation_provider_session_budget_cost_cents_total",
            "cost_cents",
            7,
        ),
    ] {
        agent_work_continuations::record_p086_continuation_metric_event(
            &pool,
            Some("run-1"),
            Some("stage-exec-1"),
            Some("agent-exec-1"),
            Some("cont-metrics"),
            metric_name,
            serde_json::json!({"budget_dimension": budget_dimension}),
            value,
        )
        .await
        .unwrap();
    }
    agent_work_continuations::record_p086_continuation_metric_event(
        &pool,
        Some("run-1"),
        Some("stage-exec-1"),
        Some("agent-exec-1"),
        Some("cont-metrics"),
        "continuation_resurrection_total",
        serde_json::json!({"resurrection_status": "unsupported"}),
        1,
    )
    .await
    .unwrap();

    let events =
        agent_work_continuations::list_p086_continuation_metric_events_for_run(&pool, "run-1", 20)
            .await
            .unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.metric_name == "continuation_admission_total"),
        "admission metric must be emitted by atomic admission"
    );
    assert!(
        events
            .iter()
            .any(|event| event.metric_name == "continuation_settlement_total"),
        "settlement metric must be durable"
    );

    let summary =
        agent_work_continuations::p086_continuation_metrics_summary_for_run(&pool, "run-1")
            .await
            .unwrap();
    assert_eq!(summary.admission_total, 1);
    assert_eq!(summary.accepted_total, 1);
    assert_eq!(summary.success_total, 1);
    assert_eq!(summary.fresh_session_avoided_total, 1);
    assert_eq!(summary.changed_files_total, 3);
    assert_eq!(summary.terminal_total, 1);
    assert_eq!(summary.useful_progress_total, 1);
    assert_eq!(summary.useful_progress_rate, 1.0);
    assert_eq!(summary.no_progress_rate, 0.0);
    assert_eq!(summary.tests_or_gates_total, 2);
    assert_eq!(summary.tests_passed_after_continuation_total, 1);
    assert_eq!(summary.followup_validation_total, 1);
    assert_eq!(summary.followup_validation_success_total, 1);
    assert_eq!(summary.followup_validation_success_rate, 1.0);
    assert_eq!(summary.operator_mcp_success_total, 1);
    assert_eq!(summary.operator_mcp_success_rate, 1.0);
    assert_eq!(summary.time_saved_seconds_total, 120);
    assert_eq!(summary.time_saved_sample_count, 1);
    assert_eq!(summary.average_time_saved_seconds, 120.0);
    assert_eq!(summary.provider_session_budget_input_tokens_total, 100);
    assert_eq!(summary.provider_session_budget_output_tokens_total, 40);
    assert_eq!(
        summary.provider_session_budget_cached_input_tokens_total,
        20
    );
    assert_eq!(summary.provider_session_budget_cost_cents_total, 7);
    assert_eq!(
        summary.provider_session_resurrection_attach_success_total,
        0
    );
    assert_eq!(
        summary.provider_session_resurrection_attach_failure_total,
        1
    );
    assert_eq!(summary.resurrection_unsupported_total, 1);
}

#[tokio::test]
async fn p086_worker_can_attach_canonical_request_artifact_id() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-request-artifact").await;

    agent_work_continuations::set_canonical_request_artifact(
        &pool,
        "cont-request-artifact",
        "canonical-request-artifact-id",
    )
    .await
    .unwrap();

    let artifact_id: String = sqlx::query_scalar(
        "SELECT canonical_request_artifact_id
         FROM agent_work_continuations
         WHERE id = 'cont-request-artifact'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(artifact_id, "canonical-request-artifact-id");
}

#[tokio::test]
async fn p086_lists_terminal_rows_missing_artifacts_for_repair() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-missing-artifacts").await;

    agent_work_continuations::update_continuation_status(
        &pool,
        "cont-missing-artifacts",
        "failed",
        Some("admission_timeout"),
    )
    .await
    .unwrap();

    let rows = agent_work_continuations::list_terminal_missing_artifacts(&pool, 10)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "cont-missing-artifacts");
    assert_eq!(rows[0].status, "failed");
}

#[tokio::test]
async fn p086_side_effect_lookup_and_worker_release_are_durable() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-release").await;

    let claim = agent_work_continuations::claim_for_continuation_worker(
        &pool,
        "cont-release",
        4242,
        "daemon-gen-release",
        "session-gen-release",
    )
    .await
    .unwrap();
    assert!(matches!(claim, ContinuationClaimOutcome::Claimed(_)));

    assert!(!agent_work_continuations::has_side_effect_ledger_row(
        &pool,
        "cont-release",
        "provider_send"
    )
    .await
    .unwrap());

    agent_work_continuations::insert_side_effect_ledger_row(
        &pool,
        "cont-release",
        "key-cont-release",
        "provider_send",
        3,
        "committed",
        Some(&"a".repeat(64)),
        None,
    )
    .await
    .unwrap();
    assert!(agent_work_continuations::has_side_effect_ledger_row(
        &pool,
        "cont-release",
        "provider_send"
    )
    .await
    .unwrap());

    let released = agent_work_continuations::release_supervised_worker(
        &pool,
        "cont-release",
        "completed",
        Some(r#"{"reason":"test"}"#),
    )
    .await
    .unwrap();
    assert_eq!(released, 1);
}

#[tokio::test]
async fn p086_evidence_artifact_ids_are_readback_fields() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-evidence-fields").await;

    agent_work_continuations::set_evidence_artifact_ids(
        &pool,
        "cont-evidence-fields",
        Some("attach-artifact"),
        Some("evidence-artifact"),
        Some("worktree-artifact"),
        Some("report-artifact"),
    )
    .await
    .unwrap();

    let record = agent_work_continuations::find_by_id(&pool, "cont-evidence-fields")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        record.attach_receipt_artifact_id.as_deref(),
        Some("attach-artifact")
    );
    assert_eq!(
        record.evidence_bundle_artifact_id.as_deref(),
        Some("evidence-artifact")
    );
    assert_eq!(
        record.worktree_readback_artifact_id.as_deref(),
        Some("worktree-artifact")
    );
    assert_eq!(
        record.continuation_report_artifact_id.as_deref(),
        Some("report-artifact")
    );
}

#[tokio::test]
async fn p086_stale_supervised_workers_are_detected_and_released() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-stale-worker").await;

    let claim = agent_work_continuations::claim_for_continuation_worker(
        &pool,
        "cont-stale-worker",
        4242,
        "daemon-old",
        "session-gen-stale",
    )
    .await
    .unwrap();
    assert!(matches!(claim, ContinuationClaimOutcome::Claimed(_)));

    let stale = agent_work_continuations::list_stale_supervised_workers(
        &pool,
        "daemon-current",
        "2099-01-01T00:00:00Z",
        10,
    )
    .await
    .unwrap();
    assert_eq!(stale.len(), 1);
    assert_eq!(stale[0].continuation_id, "cont-stale-worker");

    agent_work_continuations::release_supervised_worker(
        &pool,
        "cont-stale-worker",
        "stale_generation",
        Some(r#"{"outcome":"released_stale_generation"}"#),
    )
    .await
    .unwrap();
    let stale_after = agent_work_continuations::list_stale_supervised_workers(
        &pool,
        "daemon-current",
        "2099-01-01T00:00:00Z",
        10,
    )
    .await
    .unwrap();
    assert!(stale_after.is_empty());
}

#[tokio::test]
async fn p086_post_provider_settlement_does_not_overwrite_cancelling() {
    let pool = test_pool().await;
    seed_minimal_continuation(&pool, "cont-cancel-race").await;

    agent_work_continuations::update_continuation_status(
        &pool,
        "cont-cancel-race",
        "cancelling",
        Some("run_cancel_requested"),
    )
    .await
    .unwrap();

    let changed = agent_work_continuations::update_continuation_status_unless_cancelling(
        &pool,
        "cont-cancel-race",
        "observing",
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        changed, 0,
        "post-provider status transitions must not erase cancelling"
    );

    let settled = agent_work_continuations::settle_with_artifacts_unless_cancelling(
        &pool,
        "cont-cancel-race",
        "succeeded",
        None,
        "response-artifact",
        &"b".repeat(64),
        "result-artifact",
    )
    .await
    .unwrap();
    assert_eq!(
        settled, 0,
        "late provider terminal settlement must not erase cancelling"
    );

    let record = agent_work_continuations::find_by_id(&pool, "cont-cancel-race")
        .await
        .unwrap()
        .expect("record");
    assert_eq!(record.status, "cancelling");
    assert_eq!(
        record.failure_reason.as_deref(),
        Some("run_cancel_requested")
    );
}
