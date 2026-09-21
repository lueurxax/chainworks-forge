use db::repos::{
    run_continuation_commands::{self as commands, CommandIdentity},
    run_continuation_inputs::PreparedInput,
    run_continuation_witness as witness, run_continuations as c,
};
use domain::{
    ids::{ArtifactId, RunId},
    run::Run,
    run_carry_forward::{ContentDigest, EntryRole},
};
use sqlx::SqlitePool;

async fn open(url: &str) -> SqlitePool {
    let pool = db::pool::create_pool(url).await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    pool
}

async fn prepared(pool: &SqlitePool) -> (c::Continuation, Run, PreparedInput) {
    let source = RunId::new();
    let idea = uuid::Uuid::new_v4();
    let artifact = ArtifactId::new();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Idea','Body','2026-09-20T00:00:00Z')")
        .bind(idea.to_string()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','w','W','/source','/source/meta','2026-09-20T00:00:00Z')")
        .bind(source.to_string()).bind(idea.to_string()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,provider,created_at) VALUES (?,?,'s','writer','proposal_current','proposal_current','markdown','/source/meta/proposal.md','codex','2026-09-20T00:00:00Z')")
        .bind(artifact.to_string()).bind(source.to_string()).execute(pool).await.unwrap();
    let hash = witness::observe(&mut pool.acquire().await.unwrap(), source, None)
        .await
        .unwrap();
    let op = c::reserve_checked(
        pool,
        &c::Reservation {
            source_run_id: source.to_string(),
            caller_fingerprint: "operator".into(),
            caller_request_id: uuid::Uuid::new_v4().to_string(),
            intent_sha256: "a".repeat(64),
            plan_ref: "ignored".into(),
            plan_sha256: "b".repeat(64),
            target_ref: "ignored".into(),
            source_witness_sha256: hash.as_str()[7..].into(),
            reason: "atomicity fixture".into(),
        },
        std::path::Path::new("/owned"),
    )
    .await
    .unwrap();
    let claim = c::claim_preparation_for(pool, &op.operation_id)
        .await
        .unwrap()
        .unwrap();
    c::begin_step(
        pool,
        &op.operation_id,
        claim.generation,
        "selected_inputs_and_target",
        "{}",
        "owned",
    )
    .await
    .unwrap();
    c::finish_step(
        pool,
        &op.operation_id,
        claim.generation,
        "selected_inputs_and_target",
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let op = c::mark_prepared(
        pool,
        &op.operation_id,
        claim.generation,
        "manifest.json",
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let mut successor = db::repos::runs::find_by_id(pool, source)
        .await
        .unwrap()
        .unwrap();
    successor.id = RunId(uuid::Uuid::parse_str(&op.reserved_successor_run_id).unwrap());
    successor.status = domain::run::RunStatus::Pending;
    successor.current_state = Some("fresh_review".into());
    successor.worktree_root = Some("/owned/checkout".into());
    successor.artifact_root = "/owned/meta".into();
    let input = PreparedInput {
        input_id: uuid::Uuid::new_v4(),
        ordinal: 0,
        role: EntryRole::ExecutionSeed,
        source_kind: "artifact".into(),
        source_id: artifact.0,
        source_run_id: source,
        original_artifact_id: artifact,
        source_schema: "proposal_current".into(),
        content_sha256: ContentDigest::of(b"proposal"),
        target_logical_name: "proposal_current".into(),
        target_relative_path: "proposals/current/proposal.md".into(),
        historical_relation: serde_json::json!({"status":"unproven"}),
    };
    (op, successor, input)
}

async fn assert_uncommitted(pool: &SqlitePool, op: &c::Continuation) {
    let now = c::find(pool, &op.operation_id).await.unwrap().unwrap();
    assert_eq!(now.phase, "prepared");
    assert_eq!(now.version, op.version);
    assert!(now.successor_run_id.is_none());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT disposition FROM run_execution_fences")
            .fetch_one(pool)
            .await
            .unwrap(),
        "reserved"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuation_inputs")
            .fetch_one(pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE kind='advance_run'")
            .fetch_one(pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuation_commands")
            .fetch_one(pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM command_journal")
            .fetch_one(pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn activation_rolls_back_every_durable_boundary_before_commit() {
    let pool = open("sqlite::memory:").await;
    let (op, run, input) = prepared(&pool).await;
    let request = uuid::Uuid::new_v4().to_string();
    let intent = "e".repeat(64);
    let identity = CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &request,
        command: "runs.continuation_activate",
        intent_sha256: &intent,
    };
    for sql in [
        "CREATE TRIGGER p039_fault BEFORE INSERT ON run_continuation_inputs BEGIN SELECT RAISE(ABORT,'p039_injected_failure'); END",
        "CREATE TRIGGER p039_fault BEFORE INSERT ON runs BEGIN SELECT RAISE(ABORT,'p039_injected_failure'); END",
        "CREATE TRIGGER p039_fault BEFORE UPDATE OF installed ON run_continuation_inputs BEGIN SELECT RAISE(ABORT,'p039_injected_failure'); END",
        "CREATE TRIGGER p039_fault BEFORE INSERT ON work_items WHEN NEW.kind='advance_run' BEGIN SELECT RAISE(ABORT,'p039_injected_failure'); END",
        "CREATE TRIGGER p039_fault BEFORE UPDATE OF phase ON run_continuations WHEN NEW.phase='activated' BEGIN SELECT RAISE(ABORT,'p039_injected_failure'); END",
        "CREATE TRIGGER p039_fault BEFORE INSERT ON run_continuation_commands BEGIN SELECT RAISE(ABORT,'p039_injected_failure'); END",
    ] {
        sqlx::query(sqlx::AssertSqlSafe(sql)).execute(&pool).await.unwrap();
        let error = c::activate_checked_command(&pool, &op.operation_id, op.version, &"d".repeat(64), &run, std::slice::from_ref(&input), &identity).await.unwrap_err();
        assert!(format!("{error:#}").contains("p039_injected_failure"), "{sql}: {error:#}");
        assert_uncommitted(&pool, &op).await;
        sqlx::query("DROP TRIGGER p039_fault").execute(&pool).await.unwrap();
    }
    c::activate_checked_command(
        &pool,
        &op.operation_id,
        op.version,
        &"d".repeat(64),
        &run,
        &[input],
        &identity,
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn lost_activation_response_replays_after_database_reopen_without_ttl_or_new_work() {
    let directory = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        directory.path().join("activation.db").display()
    );
    let pool = open(&url).await;
    let (op, run, input) = prepared(&pool).await;
    let request = uuid::Uuid::new_v4().to_string();
    let intent = "e".repeat(64);
    let identity = CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &request,
        command: "runs.continuation_activate",
        intent_sha256: &intent,
    };
    let committed = c::activate_checked_command(
        &pool,
        &op.operation_id,
        op.version,
        &"d".repeat(64),
        &run,
        std::slice::from_ref(&input),
        &identity,
    )
    .await
    .unwrap();
    pool.close().await;
    let pool = open(&url).await;
    // Replay is permanent and precedes source/file validation; there is no TTL lookup.
    let replay = c::activate_checked_command(
        &pool,
        &op.operation_id,
        op.version,
        "not-revalidated-on-replay",
        &run,
        &[],
        &identity,
    )
    .await
    .unwrap();
    assert_eq!(committed.operation, replay.operation);
    assert_eq!(committed.journal_id, replay.journal_id);
    assert_eq!(
        replay.replay_of,
        Some(domain::run_carry_forward_api::ReplayOf::Accepted)
    );
    let original = commands::find(&pool, "operator", &request, identity.command, &intent)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original, committed);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE kind='advance_run'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM run_continuation_inputs WHERE installed=1"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT disposition FROM run_execution_fences")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "historical"
    );
    pool.close().await;
}

#[tokio::test]
async fn projection_failure_after_activation_keeps_canonical_links_and_rebuild_is_not_dispatch() {
    let pool = open("sqlite::memory:").await;
    let (op, run, input) = prepared(&pool).await;
    let request = uuid::Uuid::new_v4().to_string();
    let intent = "e".repeat(64);
    let identity = CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &request,
        command: "runs.continuation_activate",
        intent_sha256: &intent,
    };
    sqlx::query("CREATE TRIGGER p039_projection_failure BEFORE INSERT ON run_summaries BEGIN SELECT RAISE(ABORT,'p039_projection_unavailable'); END")
        .execute(&pool).await.unwrap();
    let accepted = c::activate_checked_command(
        &pool,
        &op.operation_id,
        op.version,
        &"d".repeat(64),
        &run,
        &[input],
        &identity,
    )
    .await
    .unwrap();
    let error = db::repos::projections::rebuild_run_summary(&pool, run.id)
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}")
            .contains("projections.rebuild_run_summary did not commit: write_failed"),
        "{error:#}"
    );
    let stale = db::repos::projections::find_run_projection(&pool, &run.id.to_string())
        .await
        .unwrap()
        .unwrap();
    assert!(!stale.projection_present);
    let incoming = c::links(&pool, &run.id.to_string())
        .await
        .unwrap()
        .incoming
        .unwrap();
    let outgoing = c::links(&pool, &op.source_run_id)
        .await
        .unwrap()
        .outgoing
        .unwrap();
    assert_eq!(incoming.operation_id, op.operation_id);
    assert_eq!(incoming.phase, "activated");
    assert_eq!(outgoing.successor_run_id, Some(run.id.to_string()));
    sqlx::query("DROP TRIGGER p039_projection_failure")
        .execute(&pool)
        .await
        .unwrap();
    db::repos::projections::rebuild_run_summary(&pool, run.id)
        .await
        .unwrap();
    let fresh = db::repos::projections::find_run_projection(&pool, &run.id.to_string())
        .await
        .unwrap()
        .unwrap();
    assert!(fresh.projection_present);
    assert!(!fresh.projection_lag);
    let replay = c::activate_checked_command(
        &pool,
        &op.operation_id,
        op.version,
        "not-revalidated-on-replay",
        &run,
        &[],
        &identity,
    )
    .await
    .unwrap();
    assert_eq!(accepted.operation, replay.operation);
    assert_eq!(accepted.journal_id, replay.journal_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE kind='advance_run'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT disposition FROM run_execution_fences")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "historical"
    );
    pool.close().await;
}

#[tokio::test]
async fn concurrent_operators_with_new_keys_create_one_live_reservation() {
    let pool = open("sqlite::memory:").await;
    let (old, _, _) = prepared(&pool).await;
    let abort = c::begin_abort(&pool, &old.operation_id, old.version)
        .await
        .unwrap();
    c::finish_abort(&pool, &old.operation_id, abort.version)
        .await
        .unwrap();
    let source = RunId(uuid::Uuid::parse_str(&old.source_run_id).unwrap());
    let hash = witness::observe(&mut pool.acquire().await.unwrap(), source, None)
        .await
        .unwrap();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
    let mut tasks = vec![];
    for caller in ["operator-a", "operator-b"] {
        let pool = pool.clone();
        let barrier = barrier.clone();
        let hash = hash.clone();
        tasks.push(tokio::spawn(async move {
            let request = c::Reservation {
                source_run_id: source.to_string(),
                caller_fingerprint: caller.into(),
                caller_request_id: uuid::Uuid::new_v4().to_string(),
                intent_sha256: "a".repeat(64),
                plan_ref: "ignored".into(),
                plan_sha256: "b".repeat(64),
                target_ref: "ignored".into(),
                source_witness_sha256: hash.as_str()[7..].into(),
                reason: "competing reservation".into(),
            };
            barrier.wait().await;
            c::reserve_checked(&pool, &request, std::path::Path::new("/owned")).await
        }));
    }
    barrier.wait().await;
    let mut accepted = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => accepted += 1,
            Err(error) => assert!(
                ["continuation_in_progress", "source_changed"]
                    .iter()
                    .any(|reason| error.to_string().contains(reason)),
                "{error:#}"
            ),
        }
    }
    assert_eq!(accepted, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM run_continuations WHERE phase<>'aborted'"
        )
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
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE status='pending'")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn explicit_reconciliation_restores_only_settled_hash_bound_complete_material() {
    let pool = open("sqlite::memory:").await;
    let (op, _, _) = prepared(&pool).await;
    let held = c::reconcile_command(
        &pool,
        &op.operation_id,
        op.version,
        Some(domain::run_carry_forward_api::ContinuationReasonCode::EffectOutcomeUnknown),
        &CommandIdentity {
            caller_fingerprint: "operator",
            caller_request_id: &uuid::Uuid::new_v4().to_string(),
            command: "runs.continuation_reconcile",
            intent_sha256: &"f".repeat(64),
        },
    )
    .await
    .unwrap();
    let version = held.operation.unwrap().version.get() as i64;
    let request = uuid::Uuid::new_v4().to_string();
    let intent = "e".repeat(64);
    let identity = CommandIdentity {
        caller_fingerprint: "operator",
        caller_request_id: &request,
        command: "runs.continuation_reconcile",
        intent_sha256: &intent,
    };
    let path = format!("/owned/{}/manifest.json", op.operation_id);
    assert!(
        c::reconcile_prepared_command(
            &pool,
            &op.operation_id,
            version,
            &path,
            &"a".repeat(64),
            &identity
        )
        .await
        .is_err(),
        "file existence without the verified effect receipt is not authority"
    );
    assert!(c::reconcile_prepared_command(
        &pool,
        &op.operation_id,
        version,
        "/unowned/manifest.json",
        &"d".repeat(64),
        &identity
    )
    .await
    .is_err());
    let restored = c::reconcile_prepared_command(
        &pool,
        &op.operation_id,
        version,
        &path,
        &"d".repeat(64),
        &identity,
    )
    .await
    .unwrap();
    assert_eq!(
        restored.operation.as_ref().unwrap().phase,
        domain::run_carry_forward_api::ContinuationPhase::Prepared
    );
    let replay = c::reconcile_prepared_command(
        &pool,
        &op.operation_id,
        version,
        &path,
        &"d".repeat(64),
        &identity,
    )
    .await
    .unwrap();
    assert_eq!(restored.operation, replay.operation);
    assert_eq!(restored.journal_id, replay.journal_id);
    let current = c::find(&pool, &op.operation_id).await.unwrap().unwrap();
    assert_eq!(current.manifest_ref.as_deref(), Some(path.as_str()));
    assert_eq!(current.generation, op.generation);
    assert!(!current.worker_claimed);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
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
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT disposition FROM run_execution_fences")
            .fetch_one(&pool)
            .await
            .unwrap(),
        "reserved"
    );
}
