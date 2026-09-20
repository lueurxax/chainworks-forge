use chrono::Utc;
use db::repos::xcode_effect_attempts::{self as journal, DispatchDecision};
use db::writer::{begin_repository_transaction, register_shared_writer, DbWriter};
use domain::xcode_effect::*;
use engine::{event_bus::new_bus, recovery::RecoveryService, work_queue::WorkQueue};
use sqlx::SqlitePool;
use std::sync::Arc;
use uuid::Uuid;

struct Fixture {
    pool: SqlitePool,
    writer: Arc<DbWriter>,
    recovery: RecoveryService,
}

impl Fixture {
    async fn new() -> Self {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = Arc::new(DbWriter::new(pool.clone()));
        register_shared_writer(&pool, writer.clone()).await.unwrap();
        let recovery = RecoveryService::new_with_db_writer(
            pool.clone(),
            WorkQueue::new(pool.clone()),
            new_bus(16),
            writer.clone(),
        );
        Self {
            pool,
            writer,
            recovery,
        }
    }

    async fn seed(&self, count: u32) -> Vec<StoredAttempt> {
        let mut attempts = Vec::new();
        for n in 0..count {
            let intent = NormalizedIntent {
                run_id: Uuid::new_v4(),
                owner_lineage: format!("owner-{n}"),
                invocation_id: Uuid::new_v4(),
                operation_key: Uuid::new_v4(),
                project_key: ProjectKey {
                    version: 1,
                    uid: 501,
                    canonical_path: format!("/fixture/{n}.xcodeproj"),
                    device: "7".into(),
                    inode: (1000 + n).to_string(),
                },
                operation: "workspace_open".into(),
                target_digest: "a".repeat(64),
                binding_digest: None,
                request_digest: "b".repeat(64),
                origin: EffectOrigin::Lifecycle,
                effect_class: EffectClass::Mutate,
                result_schema: SchemaIdentity {
                    contract_id: "fixture.open".into(),
                    version: 1,
                    manifest_digest: "c".repeat(64),
                    schema_digest: "d".repeat(64),
                },
            };
            let tx = begin_repository_transaction(&self.pool, "test_xcode_recovery_seed")
                .await
                .unwrap();
            let prepared = journal::prepare(tx, &intent, Utc::now()).await.unwrap();
            let tx = begin_repository_transaction(&self.pool, "test_xcode_recovery_seed")
                .await
                .unwrap();
            let DispatchDecision::Dispatch(dispatched) = journal::dispatch(
                tx,
                &intent.owner_lineage,
                prepared.nonce,
                &intent.request_digest,
                prepared.revision,
                Utc::now(),
            )
            .await
            .unwrap() else {
                panic!("first dispatch");
            };
            attempts.push(dispatched);
        }
        attempts
    }

    async fn holds(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn close(self) {
        self.writer.shutdown().await;
        self.pool.close().await;
    }
}

#[tokio::test]
async fn startup_xcode_recovery_is_empty_without_attempts() {
    let f = Fixture::new().await;
    assert_eq!(
        f.recovery
            .recover_xcode_effects_before_admission()
            .await
            .unwrap(),
        0
    );
    assert_eq!(f.holds().await, 0);
    f.close().await;
}

#[tokio::test]
async fn startup_xcode_recovery_batches_101_attempts_and_preserves_holds() {
    let f = Fixture::new().await;
    let attempts = f.seed(101).await;
    assert_eq!(
        f.recovery
            .recover_xcode_effects_before_admission()
            .await
            .unwrap(),
        101
    );
    assert_eq!(f.holds().await, 101);
    for before in &attempts {
        let saved = journal::get(&f.pool, &before.intent.owner_lineage, before.attempt_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.state, AttemptState::Unknown);
        assert_eq!(saved.uncertainty_reason, Some(UncertaintyReason::Restart));
        assert_eq!(saved.revision, 2);
        assert_eq!(saved.nonce, before.nonce);
    }
    assert_eq!(
        f.recovery
            .recover_xcode_effects_before_admission()
            .await
            .unwrap(),
        0
    );
    assert_eq!(f.holds().await, 101);
    f.close().await;
}

#[tokio::test]
async fn startup_xcode_recovery_stops_on_failed_batch_without_partial_transition() {
    let f = Fixture::new().await;
    let attempts = f.seed(2).await;
    let mut tx = begin_repository_transaction(&f.pool, "test_xcode_recovery_fault")
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER reject_xcode_recovery BEFORE UPDATE OF state ON xcode_effect_attempts WHEN NEW.state = 'unknown' AND NEW.attempt_id = (SELECT attempt_id FROM xcode_effect_attempts ORDER BY sequence LIMIT 1 OFFSET 1) BEGIN SELECT RAISE(ABORT, 'fixture recovery fault'); END")
        .execute(&mut **tx).await.unwrap();
    tx.commit().await.unwrap();
    let recovered = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        f.recovery.recover_xcode_effects_before_admission(),
    )
    .await
    .expect("failed recovery must not retry");
    assert!(format!("{:#}", recovered.unwrap_err()).contains("fixture recovery fault"));
    assert_eq!(f.holds().await, 2);
    for before in attempts {
        let saved = journal::get(&f.pool, &before.intent.owner_lineage, before.attempt_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.state, AttemptState::Dispatched);
        assert_eq!(saved.revision, 1);
    }
    f.close().await;
}

#[tokio::test]
async fn startup_xcode_recovery_rejects_a_closed_writer_without_losing_holds() {
    let f = Fixture::new().await;
    let attempts = f.seed(1).await;
    f.writer.shutdown().await;
    let error = f
        .recovery
        .recover_xcode_effects_before_admission()
        .await
        .unwrap_err();
    assert!(error.to_string().contains("shutdown_admission_denied"));
    let before = &attempts[0];
    let saved = journal::get(&f.pool, &before.intent.owner_lineage, before.attempt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.state, AttemptState::Dispatched);
    assert_eq!(saved.revision, 1);
    assert_eq!(f.holds().await, 1);
    f.pool.close().await;
}

#[tokio::test]
async fn startup_xcode_recovery_resumes_after_a_later_batch_failure_without_revisiting_history() {
    let f = Fixture::new().await;
    let attempts = f.seed(101).await;
    let mut tx = begin_repository_transaction(&f.pool, "test_xcode_recovery_fault")
        .await
        .unwrap();
    sqlx::query("CREATE TRIGGER reject_later_recovery BEFORE UPDATE OF state ON xcode_effect_attempts WHEN NEW.state = 'unknown' AND NEW.attempt_id = (SELECT attempt_id FROM xcode_effect_attempts ORDER BY sequence LIMIT 1 OFFSET 100) BEGIN SELECT RAISE(ABORT, 'fixture later batch fault'); END")
        .execute(&mut **tx).await.unwrap();
    tx.commit().await.unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        f.recovery.recover_xcode_effects_before_admission(),
    )
    .await
    .expect("failed later batch must stop recovery");
    assert!(format!("{:#}", result.unwrap_err()).contains("fixture later batch fault"));
    assert_eq!(f.holds().await, 101);
    for (index, before) in attempts.iter().enumerate() {
        let saved = journal::get(&f.pool, &before.intent.owner_lineage, before.attempt_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            saved.state,
            if index < 100 {
                AttemptState::Unknown
            } else {
                AttemptState::Dispatched
            }
        );
        assert_eq!(saved.revision, if index < 100 { 2 } else { 1 });
    }

    let mut tx = begin_repository_transaction(&f.pool, "test_xcode_recovery_fault_removed")
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER reject_later_recovery")
        .execute(&mut **tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        f.recovery
            .recover_xcode_effects_before_admission()
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        f.recovery
            .recover_xcode_effects_before_admission()
            .await
            .unwrap(),
        0
    );
    for before in attempts {
        let saved = journal::get(&f.pool, &before.intent.owner_lineage, before.attempt_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.state, AttemptState::Unknown);
        assert_eq!(saved.revision, 2);
        assert_eq!(saved.nonce, before.nonce);
    }
    assert_eq!(f.holds().await, 101);
    f.close().await;
}

#[tokio::test]
async fn live_startup_repair_leaves_dispatched_xcode_attempts_untouched() {
    let f = Fixture::new().await;
    let attempts = f.seed(1).await;
    f.recovery.run_startup_repair().await.unwrap();
    let before = &attempts[0];
    let saved = journal::get(&f.pool, &before.intent.owner_lineage, before.attempt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.state, AttemptState::Dispatched);
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.uncertainty_reason, None);
    assert_eq!(f.holds().await, 1);
    f.close().await;
}
