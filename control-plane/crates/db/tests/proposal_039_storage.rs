use sqlx::SqlitePool;

async fn pool() -> SqlitePool {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES ('00000000-0000-4000-8000-000000000039','Idea','Body','2026-09-20')").execute(&pool).await.unwrap();
    insert_run(&pool, "source", "blocked").await.unwrap();
    pool
}

async fn insert_run(pool: &SqlitePool, id: &str, status: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,'00000000-0000-4000-8000-000000000039',?,'workflow','Workflow','/repo','/meta','2026-09-20')")
        .bind(id).bind(status).execute(pool).await?;
    Ok(())
}

async fn operation(
    pool: &SqlitePool,
    id: &str,
    source: &str,
    successor: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES (?,'ContinueBlocked','{}','2026-09-20')")
        .bind(id).execute(pool).await?;
    sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,'00000000-0000-4000-8000-000000000039',?,'operator',?,?,'implementation_restart_v1','plan.json',?,'target.json',?,?, '2026-09-20','2026-09-20','2026-09-21')")
        .bind(id).bind(source).bind(successor).bind(id).bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64)).bind(id).execute(pool).await?;
    Ok(())
}

async fn fence(pool: &SqlitePool, id: &str, source: &str) {
    sqlx::query("INSERT INTO run_execution_fences(source_run_id,operation_id,generation,disposition,created_at) VALUES (?,?,1,'reserved','2026-09-20')")
        .bind(source).bind(id).execute(pool).await.unwrap();
}

#[tokio::test]
async fn migration_preserves_legacy_rows_and_enforces_foreign_keys() {
    let pool = pool().await;
    let legacy: (String, Option<String>) =
        sqlx::query_as("SELECT status, workflow_snapshot_hash FROM runs WHERE id='source'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(legacy, ("blocked".into(), None));
    assert!(operation(&pool, "missing", "missing-source", "reserved")
        .await
        .is_err());
    operation(&pool, "op", "source", "reserved").await.unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM run_continuations WHERE successor_run_id IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn duplicate_source_or_idea_reservation_is_rejected() {
    let pool = pool().await;
    operation(&pool, "op", "source", "reserved").await.unwrap();
    assert!(operation(&pool, "sibling", "source", "reserved-2")
        .await
        .is_err());
    assert!(insert_run(&pool, "start-race", "pending").await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn fenced_source_rejects_business_writes_but_retains_historical_reads() {
    let pool = pool().await;
    operation(&pool, "op", "source", "reserved").await.unwrap();
    fence(&pool, "op", "source").await;
    for sql in [
        "UPDATE runs SET status='running' WHERE id='source'",
        "INSERT INTO work_items(id,kind,payload_json,run_id,created_at,scheduled_at) VALUES ('work','advance_run','{}','source','now','now')",
        "INSERT INTO stage_executions(id,run_id,stage_id,label,started_at) VALUES ('stage','source','s','S','now')",
        "INSERT INTO approvals(id,run_id,stage_id,requested_at) VALUES ('approval','source','s','now')",
        "INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,provider,created_at) VALUES ('artifact','source','s','a','name','contract','md','/old','codex','now')",
    ] {
        let error = sqlx::query(sql).execute(&pool).await.expect_err(sql);
        assert!(error.to_string().contains("continuation_in_progress"), "{error}");
    }
    let status: String = sqlx::query_scalar("SELECT status FROM runs WHERE id='source'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "blocked");
}

#[tokio::test]
async fn fence_cannot_be_released_until_explicit_abort_settles() {
    let pool = pool().await;
    operation(&pool, "op", "source", "reserved").await.unwrap();
    fence(&pool, "op", "source").await;
    assert!(sqlx::query("DELETE FROM run_execution_fences")
        .execute(&pool)
        .await
        .is_err());
    assert!(sqlx::query(
        "UPDATE run_continuations SET phase='aborted',version=2 WHERE operation_id='op'"
    )
    .execute(&pool)
    .await
    .is_err());
    sqlx::query("UPDATE run_continuations SET phase='aborting',version=2,generation=2 WHERE operation_id='op'").execute(&pool).await.unwrap();
    sqlx::query("UPDATE run_continuations SET phase='aborted',version=3 WHERE operation_id='op'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM run_execution_fences")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM runs WHERE id='source'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "blocked"
    );
}

#[tokio::test]
async fn stale_step_generation_and_unknown_effects_cannot_authorize_abort() {
    let pool = pool().await;
    operation(&pool, "op", "source", "reserved").await.unwrap();
    fence(&pool, "op", "source").await;
    sqlx::query("INSERT INTO run_continuation_steps(operation_id,step_key,generation,status,intent_json,destination_identity,updated_at) VALUES ('op','copy',1,'dispatching','{}','owned','now')").execute(&pool).await.unwrap();
    sqlx::query("UPDATE run_continuations SET phase='aborting',version=2,generation=2 WHERE operation_id='op'").execute(&pool).await.unwrap();
    assert!(sqlx::query("UPDATE run_continuation_steps SET status='verified',receipt_sha256=? WHERE operation_id='op'").bind("d".repeat(64)).execute(&pool).await.is_err());
    assert!(sqlx::query(
        "UPDATE run_continuations SET phase='aborted',version=3 WHERE operation_id='op'"
    )
    .execute(&pool)
    .await
    .is_err());
}

#[tokio::test]
async fn reservation_is_atomic_replays_original_identity_and_never_creates_a_run() {
    use db::repos::run_continuations::{reserve, Reservation};
    let pool = pool().await;
    let request = Reservation {
        source_run_id: "source".into(),
        caller_fingerprint: "operator".into(),
        caller_request_id: uuid::Uuid::new_v4().to_string(),
        intent_sha256: "a".repeat(64),
        plan_ref: "plan.json".into(),
        plan_sha256: "b".repeat(64),
        target_ref: "target.json".into(),
        source_witness_sha256: "c".repeat(64),
        reason: "current policy".into(),
    };
    let first = reserve(&pool, &request).await.unwrap();
    let replay = reserve(&pool, &request).await.unwrap();
    assert_eq!(first.operation_id, replay.operation_id);
    assert_eq!(first.journal_id, replay.journal_id);
    let receipt = db::repos::run_continuation_commands::find(
        &pool,
        &request.caller_fingerprint,
        &request.caller_request_id,
        "runs.continue_blocked",
        &request.intent_sha256,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(receipt.operation.as_ref().unwrap().version.get(), 1);
    assert_eq!(
        receipt
            .operation
            .as_ref()
            .unwrap()
            .operation_id
            .as_uuid()
            .to_string(),
        first.operation_id
    );
    cassert_receipt_replay(&receipt);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_execution_fences")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE kind='prepare_run_continuation' AND run_id IS NULL").fetch_one(&pool).await.unwrap(), 1);
    let mut conflict = request.clone();
    conflict.intent_sha256 = "d".repeat(64);
    assert!(reserve(&pool, &conflict)
        .await
        .unwrap_err()
        .to_string()
        .contains("idempotency_conflict"));
}

fn cassert_receipt_replay(receipt: &domain::run_carry_forward_api::RunContinuationCommandResultV1) {
    let replay = receipt.replayed().unwrap();
    assert_eq!(receipt.operation, replay.operation);
    assert_eq!(receipt.journal_id, replay.journal_id);
    assert_eq!(
        replay.replay_of,
        Some(domain::run_carry_forward_api::ReplayOf::Accepted)
    );
}

#[tokio::test]
async fn abort_command_replays_original_outcome_without_releasing_active_worker() {
    use db::repos::{run_continuation_commands::CommandIdentity, run_continuations as c};
    let pool = pool().await;
    let op = c::reserve(&pool, &reservation()).await.unwrap();
    let claim = c::claim_preparation_for(&pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    let id = CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &uuid::Uuid::new_v4().to_string(),
        command: "runs.continuation_abort",
        intent_sha256: &"e".repeat(64),
    };
    let accepted = c::abort_command(&pool, &op.operation_id, claim.version, &id)
        .await
        .unwrap();
    assert_eq!(
        accepted.operation.as_ref().unwrap().phase,
        domain::run_carry_forward_api::ContinuationPhase::Aborting
    );
    let held = c::find(&pool, &op.operation_id).await.unwrap().unwrap();
    assert!(held.worker_claimed);
    assert!(c::check_source_guard(&pool, "source").await.is_err());
    c::settle_worker(&pool, &op.operation_id, claim.generation)
        .await
        .unwrap();
    let held = c::find(&pool, &op.operation_id).await.unwrap().unwrap();
    c::finish_abort(&pool, &op.operation_id, held.version)
        .await
        .unwrap();
    let replay = c::abort_command(&pool, &op.operation_id, claim.version, &id)
        .await
        .unwrap();
    assert_eq!(
        accepted.operation, replay.operation,
        "replay must not substitute the later aborted phase"
    );
    assert_eq!(accepted.journal_id, replay.journal_id);
}

#[tokio::test]
async fn canonical_busy_work_prevents_reservation_without_partial_rows() {
    use db::repos::run_continuations::{reserve, Reservation};
    let pool = pool().await;
    sqlx::query("INSERT INTO work_items(id,kind,payload_json,run_id,created_at,scheduled_at) VALUES ('work','advance_run','{}','source','now','later')").execute(&pool).await.unwrap();
    let request = Reservation {
        source_run_id: "source".into(),
        caller_fingerprint: "operator".into(),
        caller_request_id: uuid::Uuid::new_v4().to_string(),
        intent_sha256: "a".repeat(64),
        plan_ref: "plan.json".into(),
        plan_sha256: "b".repeat(64),
        target_ref: "target.json".into(),
        source_witness_sha256: "c".repeat(64),
        reason: "current policy".into(),
    };
    assert!(reserve(&pool, &request)
        .await
        .unwrap_err()
        .to_string()
        .contains("source_busy"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn preparation_claim_is_single_flight_and_restart_never_repeats_dispatched_step() {
    use db::repos::run_continuations as c;
    let pool = pool().await;
    let request = c::Reservation {
        source_run_id: "source".into(),
        caller_fingerprint: "operator".into(),
        caller_request_id: uuid::Uuid::new_v4().to_string(),
        intent_sha256: "a".repeat(64),
        plan_ref: "plan.json".into(),
        plan_sha256: "b".repeat(64),
        target_ref: "target.json".into(),
        source_witness_sha256: "c".repeat(64),
        reason: "current policy".into(),
    };
    let op = c::reserve(&pool, &request).await.unwrap();
    assert!(c::claim_preparation_for(&pool, "missing")
        .await
        .unwrap()
        .is_none());
    let claim = c::claim_preparation(&pool).await.unwrap().unwrap();
    assert_eq!(claim.operation_id, op.operation_id);
    assert!(c::claim_preparation(&pool).await.unwrap().is_none());
    c::begin_step(
        &pool,
        &claim.operation_id,
        claim.generation,
        "copy",
        "{}",
        "owned",
    )
    .await
    .unwrap();
    c::recover_preparations(&pool).await.unwrap();
    assert!(c::claim_preparation(&pool).await.unwrap().is_none());
    let held = c::find(&pool, &op.operation_id).await.unwrap().unwrap();
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(c::finish_step(
        &pool,
        &op.operation_id,
        claim.generation,
        "copy",
        &"d".repeat(64)
    )
    .await
    .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM run_continuation_steps")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "unknown"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_execution_fences")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn abort_revokes_worker_generation_before_any_fence_release() {
    use db::repos::run_continuations as c;
    let pool = pool().await;
    let request = c::Reservation {
        source_run_id: "source".into(),
        caller_fingerprint: "operator".into(),
        caller_request_id: uuid::Uuid::new_v4().to_string(),
        intent_sha256: "a".repeat(64),
        plan_ref: "plan.json".into(),
        plan_sha256: "b".repeat(64),
        target_ref: "target.json".into(),
        source_witness_sha256: "c".repeat(64),
        reason: "current policy".into(),
    };
    let op = c::reserve(&pool, &request).await.unwrap();
    let claim = c::claim_preparation(&pool).await.unwrap().unwrap();
    let abort = c::begin_abort(&pool, &op.operation_id, claim.version)
        .await
        .unwrap();
    assert_eq!(abort.phase, "aborting");
    assert_eq!(abort.generation, claim.generation + 1);
    assert!(c::begin_step(
        &pool,
        &op.operation_id,
        claim.generation,
        "copy",
        "{}",
        "owned"
    )
    .await
    .is_err());
    assert!(c::finish_abort(&pool, &op.operation_id, abort.version)
        .await
        .is_err());
    c::settle_worker(&pool, &op.operation_id, claim.generation)
        .await
        .unwrap();
    let settled = c::find(&pool, &op.operation_id).await.unwrap().unwrap();
    c::finish_abort(&pool, &op.operation_id, settled.version)
        .await
        .unwrap();
    c::check_source_guard(&pool, "source").await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "blocked"
    );
}

fn reservation() -> db::repos::run_continuations::Reservation {
    db::repos::run_continuations::Reservation {
        source_run_id: "source".into(),
        caller_fingerprint: "operator".into(),
        caller_request_id: uuid::Uuid::new_v4().to_string(),
        intent_sha256: "a".repeat(64),
        plan_ref: "plan.json".into(),
        plan_sha256: "b".repeat(64),
        target_ref: "target.json".into(),
        source_witness_sha256: "c".repeat(64),
        reason: "current policy".into(),
    }
}

#[tokio::test]
async fn explicit_reconciliation_records_a_hold_without_retry_or_releasing_unknown_owner() {
    use db::repos::{run_continuation_commands as receipt, run_continuations as c};
    let pool = pool().await;
    let op = c::reserve(&pool, &reservation()).await.unwrap();
    let claim = c::claim_preparation(&pool).await.unwrap().unwrap();
    c::begin_step(
        &pool,
        &op.operation_id,
        claim.generation,
        "git",
        "{}",
        "owned",
    )
    .await
    .unwrap();
    c::recover_preparations(&pool).await.unwrap();
    let before = c::find(&pool, &op.operation_id).await.unwrap().unwrap();
    let request = uuid::Uuid::new_v4().to_string();
    let intent = "f".repeat(64);
    let identity = receipt::CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &request,
        command: "runs.continuation_reconcile",
        intent_sha256: &intent,
    };
    let accepted = c::reconcile_command(
        &pool,
        &op.operation_id,
        before.version,
        Some(domain::run_carry_forward_api::ContinuationReasonCode::EffectOutcomeUnknown),
        &identity,
    )
    .await
    .unwrap();
    let replay = c::reconcile_command(
        &pool,
        &op.operation_id,
        before.version,
        Some(domain::run_carry_forward_api::ContinuationReasonCode::EffectOutcomeUnknown),
        &identity,
    )
    .await
    .unwrap();
    assert_eq!(accepted.operation, replay.operation);
    assert_eq!(accepted.journal_id, replay.journal_id);
    let after = c::find(&pool, &op.operation_id).await.unwrap().unwrap();
    assert_eq!(after.phase, "needs_reconciliation");
    assert!(after.worker_claimed);
    assert_eq!(after.generation, before.generation);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM run_continuation_steps")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "unknown"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_items WHERE status IN ('pending','running')"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert!(c::check_source_guard(&pool, "source").await.is_err());
}

#[tokio::test]
async fn ordinary_queue_does_not_claim_preparation_or_stall_other_ideas() {
    let pool = pool().await;
    db::repos::run_continuations::reserve(&pool, &reservation())
        .await
        .unwrap();
    sqlx::query("UPDATE work_items SET scheduled_at='2020-01-01T00:00:00Z'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(db::repos::work_items::claim_next_non_invoke(&pool)
        .await
        .unwrap()
        .is_none());
    sqlx::query("INSERT INTO work_items(id,kind,payload_json,created_at,scheduled_at) VALUES ('other','rebuild_projection','{}','2020-01-01T00:00:00Z','2020-01-01T00:00:00Z')").execute(&pool).await.unwrap();
    assert_eq!(
        db::repos::work_items::claim_next_non_invoke(&pool)
            .await
            .unwrap()
            .unwrap()
            .id,
        "other"
    );
}

#[tokio::test]
async fn activation_creates_exactly_one_successor_and_advance_with_historical_source() {
    use db::repos::run_continuations as c;
    let pool = pool().await;
    let op = c::reserve(&pool, &reservation()).await.unwrap();
    let claim = c::claim_preparation(&pool).await.unwrap().unwrap();
    assert!(c::mark_prepared(
        &pool,
        &op.operation_id,
        claim.generation,
        "manifest.json",
        &"d".repeat(64)
    )
    .await
    .is_err());
    c::begin_step(
        &pool,
        &op.operation_id,
        claim.generation,
        "materialize",
        "{}",
        "owned",
    )
    .await
    .unwrap();
    c::finish_step(
        &pool,
        &op.operation_id,
        claim.generation,
        "materialize",
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let prepared = c::mark_prepared(
        &pool,
        &op.operation_id,
        claim.generation,
        "manifest.json",
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let request = uuid::Uuid::new_v4().to_string();
    let intent = "e".repeat(64);
    let identity = db::repos::run_continuation_commands::CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &request,
        command: "runs.continuation_reconcile",
        intent_sha256: &intent,
    };
    let inspected =
        c::reconcile_command(&pool, &op.operation_id, prepared.version, None, &identity)
            .await
            .unwrap();
    assert_eq!(
        inspected.operation.unwrap().version.get(),
        prepared.version as u64
    );
    let run: domain::run::Run = serde_json::from_value(serde_json::json!({
        "id":op.reserved_successor_run_id,"idea_id":op.idea_id,"status":"pending",
        "workflow_id":"proposal_to_release","workflow_title":"Current policy",
        "workspace_root":"/repo","artifact_root":"/new/meta","worktree_root":"/new/checkout",
        "chainworks_meta_root":"/new/meta","started_at":"2026-09-20T00:00:00Z",
        "current_state":"fresh_review"
    }))
    .unwrap();
    let activation_request = uuid::Uuid::new_v4().to_string();
    let identity = db::repos::run_continuation_commands::CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &activation_request,
        command: "runs.continuation_activate",
        intent_sha256: &"e".repeat(64),
    };
    let accepted = c::activate_command(
        &pool,
        &op.operation_id,
        prepared.version,
        &"d".repeat(64),
        &run,
        &identity,
    )
    .await
    .unwrap();
    let replay = c::activate_command(
        &pool,
        &op.operation_id,
        prepared.version,
        &"d".repeat(64),
        &run,
        &identity,
    )
    .await
    .unwrap();
    assert_eq!(accepted.operation, replay.operation);
    assert_eq!(accepted.journal_id, replay.journal_id);
    assert_eq!(
        replay.status,
        domain::run_carry_forward_api::ContinuationCommandStatus::Replayed
    );
    let activated = c::find(&pool, &op.operation_id).await.unwrap().unwrap();
    assert_eq!(activated.phase, "activated");
    assert!(c::activate(
        &pool,
        &op.operation_id,
        prepared.version,
        &"d".repeat(64),
        &run
    )
    .await
    .is_err());
    assert!(c::begin_abort(&pool, &op.operation_id, activated.version)
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE kind='advance_run'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM runs WHERE id='source'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "blocked"
    );
    assert!(c::check_source_guard(&pool, "source")
        .await
        .unwrap_err()
        .to_string()
        .contains("source_continued"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM approvals")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    let initial_links = c::links(&pool, &run.id.to_string()).await.unwrap();
    assert_eq!(
        initial_links.incoming.unwrap().operation_id,
        op.operation_id
    );
    assert!(initial_links.outgoing.is_none());
    sqlx::query("UPDATE work_items SET status='completed' WHERE kind='advance_run'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE runs SET status='blocked' WHERE id=?")
        .bind(run.id.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let mut next = reservation();
    next.source_run_id = run.id.to_string();
    let next_op = c::reserve(&pool, &next).await.unwrap();
    c::recover_preparations(&pool).await.unwrap();
    let links = c::links(&pool, &run.id.to_string()).await.unwrap();
    assert_eq!(links.incoming.unwrap().operation_id, op.operation_id);
    assert_eq!(
        links.outgoing.as_ref().unwrap().operation_id,
        next_op.operation_id
    );
    assert_eq!(
        links.outgoing.as_ref().unwrap().phase,
        "needs_reconciliation"
    );
    let aborting = c::begin_abort(
        &pool,
        &next_op.operation_id,
        links.outgoing.unwrap().version,
    )
    .await
    .unwrap();
    c::finish_abort(&pool, &next_op.operation_id, aborting.version)
        .await
        .unwrap();
    let links = c::links(&pool, &run.id.to_string()).await.unwrap();
    assert_eq!(links.incoming.unwrap().operation_id, op.operation_id);
    assert!(links.outgoing.is_none());
    assert_eq!(
        c::find(&pool, &next_op.operation_id)
            .await
            .unwrap()
            .unwrap()
            .phase,
        "aborted"
    );
}
