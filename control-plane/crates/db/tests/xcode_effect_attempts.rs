use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use db::repos::xcode_effect_attempts::{self as journal, DispatchDecision, JournalError};
use db::writer::{
    begin_repository_transaction, register_shared_writer, DbWriter, QueuedTransaction,
};
use domain::xcode_effect::*;
use sqlx::SqlitePool;
use tempfile::TempDir;
use uuid::Uuid;

struct Fixture {
    _dir: TempDir,
    url: String,
    pool: SqlitePool,
    writer: Arc<DbWriter>,
}

impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let url = format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("journal.db").display()
        );
        let pool = db::pool::create_pool(&url).await.unwrap();
        let writer = Arc::new(DbWriter::new(pool.clone()));
        register_shared_writer(&pool, writer.clone()).await.unwrap();
        Self {
            _dir: dir,
            url,
            pool,
            writer,
        }
    }

    async fn tx(&self) -> QueuedTransaction {
        begin_repository_transaction(&self.pool, "test_xcode_effect_journal")
            .await
            .unwrap()
    }

    async fn reopen(&mut self) {
        self.writer.shutdown().await;
        self.pool.close().await;
        self.pool = db::pool::create_pool(&self.url).await.unwrap();
        self.writer = Arc::new(DbWriter::new(self.pool.clone()));
        register_shared_writer(&self.pool, self.writer.clone())
            .await
            .unwrap();
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

fn now(n: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + n, 0).unwrap()
}

fn intent() -> NormalizedIntent {
    NormalizedIntent {
        run_id: Uuid::new_v4(),
        owner_lineage: "fixture-owner".into(),
        invocation_id: Uuid::new_v4(),
        project_key: ProjectKey {
            version: 1,
            uid: 501,
            canonical_path: "/fixture/App.xcodeproj".into(),
            device: "7".into(),
            inode: "9007199254740993".into(),
        },
        operation_key: Uuid::new_v4(),
        operation: "fixture.mutate".into(),
        target_digest: "a".repeat(64),
        binding_digest: Some("b".repeat(64)),
        request_digest: "c".repeat(64),
        origin: EffectOrigin::Mcp,
        effect_class: EffectClass::Mutate,
        result_schema: SchemaIdentity {
            contract_id: "fixture.result".into(),
            version: 1,
            manifest_digest: "d".repeat(64),
            schema_digest: "e".repeat(64),
        },
    }
}

fn result(i: &NormalizedIntent) -> HistoricalResult {
    HistoricalResult {
        schema_version: 1,
        schema: i.result_schema.clone(),
        payload: HistoricalPayload::Result {
            summary: RedactedText::new("Fixture applied").unwrap(),
        },
        result_digest: "f".repeat(64),
    }
}

fn error(i: &NormalizedIntent, code: EffectErrorCode) -> HistoricalResult {
    HistoricalResult {
        payload: HistoricalPayload::Error {
            code,
            message: RedactedText::new("Fixture error").unwrap(),
        },
        ..result(i)
    }
}

fn proof() -> SettlementProof {
    SettlementProof {
        evidence_ref: "fixture/evidence/settled".into(),
        active_operation_check_ref: "fixture/evidence/no-active-operation".into(),
        no_operation_in_flight: true,
    }
}

async fn prepared(f: &Fixture, i: &NormalizedIntent) -> StoredAttempt {
    journal::prepare(f.tx().await, i, now(0)).await.unwrap()
}

async fn dispatched(f: &Fixture, i: &NormalizedIntent) -> StoredAttempt {
    let a = prepared(f, i).await;
    match journal::dispatch(
        f.tx().await,
        &i.owner_lineage,
        a.nonce,
        &i.request_digest,
        a.revision,
        now(1),
    )
    .await
    .unwrap()
    {
        DispatchDecision::Dispatch(a) => a,
        _ => panic!("first dispatch must win"),
    }
}

#[tokio::test]
async fn prepare_dedupes_across_invocations_and_preserves_original_identity() {
    let f = Fixture::new().await;
    let mut i = intent();
    let a = prepared(&f, &i).await;
    i.invocation_id = Uuid::new_v4();
    let b = journal::prepare(f.tx().await, &i, now(1)).await.unwrap();
    assert_eq!(a, b);
    assert_eq!(a.nonce, a.attempt_id);
    assert_eq!(a.state, AttemptState::Prepared);
    assert_eq!(a.revision, 0);
    assert_eq!(f.holds().await, 0);
    f.close().await;
}

#[tokio::test]
async fn same_key_rejects_every_semantically_changed_intent_field() {
    let f = Fixture::new().await;
    let i = intent();
    prepared(&f, &i).await;
    let mut variants = Vec::new();
    let mut v = i.clone();
    v.request_digest = "1".repeat(64);
    variants.push(v);
    let mut v = i.clone();
    v.target_digest = "1".repeat(64);
    variants.push(v);
    let mut v = i.clone();
    v.binding_digest = Some("1".repeat(64));
    variants.push(v);
    let mut v = i.clone();
    v.run_id = Uuid::new_v4();
    variants.push(v);
    let mut v = i.clone();
    v.operation = "fixture.other".into();
    variants.push(v);
    let mut v = i.clone();
    v.origin = EffectOrigin::Shim;
    variants.push(v);
    let mut v = i.clone();
    v.effect_class = EffectClass::Execute;
    variants.push(v);
    let mut v = i.clone();
    v.result_schema.version = 2;
    variants.push(v);
    for changed in variants {
        assert!(matches!(
            journal::prepare(f.tx().await, &changed, now(1)).await,
            Err(JournalError::IntentConflict)
        ));
    }
    f.close().await;
}

#[tokio::test]
async fn concurrent_same_nonce_dispatch_has_one_winner_and_durable_hold() {
    let f = Fixture::new().await;
    let i = intent();
    let a = prepared(&f, &i).await;
    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut tasks = Vec::new();
    for _ in 0..8 {
        let pool = f.pool.clone();
        let i = i.clone();
        let a = a.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let tx = begin_repository_transaction(&pool, "test_xcode_effect_journal")
                .await
                .unwrap();
            journal::dispatch(tx, &i.owner_lineage, a.nonce, &i.request_digest, 0, now(1))
                .await
                .unwrap()
        }));
    }
    let mut winners = 0;
    for t in tasks {
        if matches!(t.await.unwrap(), DispatchDecision::Dispatch(_)) {
            winners += 1;
        }
    }
    assert_eq!(winners, 1);
    let saved = journal::get(&f.pool, &i.owner_lineage, a.attempt_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.state, AttemptState::Dispatched);
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.dispatched_at, Some(now(1)));
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

#[tokio::test]
async fn held_project_denies_new_key_and_preexisting_nonce_across_owners() {
    let f = Fixture::new().await;
    let i = intent();
    let mut other = i.clone();
    other.operation_key = Uuid::new_v4();
    other.owner_lineage = "other-owner".into();
    let b = prepared(&f, &other).await;
    dispatched(&f, &i).await;
    assert!(matches!(
        journal::dispatch(
            f.tx().await,
            &other.owner_lineage,
            b.nonce,
            &other.request_digest,
            0,
            now(2)
        )
        .await,
        Err(JournalError::ProjectHeld)
    ));
    other.operation_key = Uuid::new_v4();
    assert!(matches!(
        journal::prepare(f.tx().await, &other, now(2)).await,
        Err(JournalError::ProjectHeld)
    ));
    other.project_key.inode = "12".into();
    assert!(matches!(
        journal::prepare(f.tx().await, &other, now(2)).await,
        Err(JournalError::ProjectHeld)
    ));
    other.project_key.canonical_path = "/fixture/Other.xcodeproj".into();
    prepared(&f, &other).await;
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

#[tokio::test]
async fn reopened_database_recovers_dispatched_with_cas_without_replay() {
    let mut f = Fixture::new().await;
    let i = intent();
    let a = dispatched(&f, &i).await;
    f.reopen().await;
    let expected = [AttemptRevision {
        attempt_id: a.attempt_id,
        revision: a.revision,
    }];
    let recovered = journal::recover_dispatched(f.tx().await, &expected, now(2))
        .await
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].state, AttemptState::Unknown);
    assert_eq!(recovered[0].revision, 2);
    assert_eq!(
        recovered[0].uncertainty_reason,
        Some(UncertaintyReason::Restart)
    );
    assert!(recovered[0].completed_at.is_none());
    assert_eq!(f.holds().await, 1);
    assert!(matches!(
        journal::dispatch(
            f.tx().await,
            &i.owner_lineage,
            a.nonce,
            &i.request_digest,
            2,
            now(3)
        )
        .await
        .unwrap(),
        DispatchDecision::Existing(_)
    ));
    let mut retry = i.clone();
    retry.operation_key = Uuid::new_v4();
    assert!(matches!(
        journal::prepare(f.tx().await, &retry, now(3)).await,
        Err(JournalError::ProjectHeld)
    ));
    assert!(matches!(
        journal::recover_dispatched(f.tx().await, &expected, now(3)).await,
        Err(JournalError::RevisionConflict)
    ));
    f.close().await;
}

#[tokio::test]
async fn cancellation_before_dispatch_cannot_be_replayed() {
    let f = Fixture::new().await;
    let i = intent();
    let a = prepared(&f, &i).await;
    let cancelled = journal::cancel(
        f.tx().await,
        &i.owner_lineage,
        a.attempt_id,
        0,
        &error(&i, EffectErrorCode::CancelledBeforeDispatch),
        now(1),
    )
    .await
    .unwrap();
    assert_eq!(cancelled.state, AttemptState::Failed);
    assert!(cancelled.dispatched_at.is_none());
    assert!(matches!(
        cancelled.outcome.unwrap().payload,
        HistoricalPayload::Error {
            code: EffectErrorCode::CancelledBeforeDispatch,
            ..
        }
    ));
    assert_eq!(f.holds().await, 0);
    assert!(matches!(
        journal::dispatch(
            f.tx().await,
            &i.owner_lineage,
            a.nonce,
            &i.request_digest,
            1,
            now(2)
        )
        .await
        .unwrap(),
        DispatchDecision::Existing(_)
    ));
    let mut next = i.clone();
    next.operation_key = Uuid::new_v4();
    let b = dispatched(&f, &next).await;
    assert!(matches!(
        journal::cancel(
            f.tx().await,
            &i.owner_lineage,
            b.attempt_id,
            b.revision,
            &error(&next, EffectErrorCode::CancelledBeforeDispatch),
            now(2)
        )
        .await,
        Err(JournalError::InvalidTransition)
    ));
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

#[tokio::test]
async fn completion_persistence_failure_keeps_the_dispatch_fence_and_hold() {
    let mut f = Fixture::new().await;
    let i = intent();
    let a = dispatched(&f, &i).await;
    sqlx::query("CREATE TRIGGER reject_outcome BEFORE UPDATE OF outcome_json ON xcode_effect_attempts WHEN NEW.outcome_json IS NOT NULL BEGIN SELECT RAISE(ABORT, 'fixture disk failure'); END")
        .execute(&f.pool).await.unwrap();
    assert!(journal::complete(
        f.tx().await,
        &i.owner_lineage,
        a.attempt_id,
        a.revision,
        &Completion::Succeeded {
            outcome: result(&i),
            proof: proof()
        },
        now(2)
    )
    .await
    .is_err());
    f.reopen().await;
    assert_eq!(
        journal::get(&f.pool, &i.owner_lineage, a.attempt_id)
            .await
            .unwrap()
            .unwrap(),
        a
    );
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

#[tokio::test]
async fn dispatch_hold_insertion_failure_rolls_back_revision_and_state() {
    let f = Fixture::new().await;
    let i = intent();
    let a = prepared(&f, &i).await;
    sqlx::query("CREATE TRIGGER reject_hold BEFORE INSERT ON xcode_project_holds BEGIN SELECT RAISE(ABORT, 'fixture hold failure'); END")
        .execute(&f.pool).await.unwrap();
    assert!(journal::dispatch(
        f.tx().await,
        &i.owner_lineage,
        a.nonce,
        &i.request_digest,
        0,
        now(1)
    )
    .await
    .is_err());
    assert_eq!(
        journal::get(&f.pool, &i.owner_lineage, a.attempt_id)
            .await
            .unwrap()
            .unwrap(),
        a
    );
    assert_eq!(f.holds().await, 0);
    f.close().await;
}

#[tokio::test]
async fn proven_terminal_outcomes_release_hold_and_repeated_commit_returns_history() {
    let f = Fixture::new().await;
    for success in [true, false] {
        let i = intent();
        let a = dispatched(&f, &i).await;
        let c = if success {
            Completion::Succeeded {
                outcome: result(&i),
                proof: proof(),
            }
        } else {
            Completion::Failed {
                outcome: error(&i, EffectErrorCode::TerminalFailure),
                proof: proof(),
            }
        };
        let done = journal::complete(f.tx().await, &i.owner_lineage, a.attempt_id, 1, &c, now(2))
            .await
            .unwrap();
        assert_eq!(
            done.state,
            if success {
                AttemptState::Succeeded
            } else {
                AttemptState::Failed
            }
        );
        assert_eq!(done.completed_at, Some(now(2)));
        assert_eq!(f.holds().await, 0);
        assert!(
            matches!(journal::dispatch(f.tx().await, &i.owner_lineage, a.nonce, &i.request_digest, 0, now(3)).await.unwrap(), DispatchDecision::Existing(ref existing) if existing == &done)
        );
    }
    f.close().await;
}

#[tokio::test]
async fn uncertain_partial_outcome_requires_evidence_bearing_reconciliation() {
    let f = Fixture::new().await;
    let i = intent();
    let a = dispatched(&f, &i).await;
    let unknown = journal::complete(
        f.tx().await,
        &i.owner_lineage,
        a.attempt_id,
        1,
        &Completion::Unknown {
            reason: UncertaintyReason::PartialEffect,
        },
        now(2),
    )
    .await
    .unwrap();
    assert_eq!(unknown.state, AttemptState::Unknown);
    assert_eq!(f.holds().await, 1);
    let mut r = Reconciliation {
        operator_id: "fixture-operator".into(),
        inspected_state: RedactedText::new("Partial effect inspected").unwrap(),
        proof: proof(),
        disposition: ReconciliationDisposition::Partial,
        outcome: error(&i, EffectErrorCode::ReconciledPartial),
    };
    r.proof.no_operation_in_flight = false;
    assert!(matches!(
        journal::reconcile(f.tx().await, &i.owner_lineage, a.attempt_id, 2, &r, now(3)).await,
        Err(JournalError::InvalidInput(_))
    ));
    assert_eq!(f.holds().await, 1);
    r.proof.no_operation_in_flight = true;
    let done = journal::reconcile(f.tx().await, &i.owner_lineage, a.attempt_id, 2, &r, now(3))
        .await
        .unwrap();
    assert_eq!(done.state, AttemptState::Reconciled);
    assert_eq!(done.reconciliation, Some(r));
    assert_eq!(
        done.reconciliation_evidence_ref.as_deref(),
        Some("fixture/evidence/settled")
    );
    assert_eq!(f.holds().await, 0);
    assert!(matches!(
        journal::reconcile(
            f.tx().await,
            &i.owner_lineage,
            a.attempt_id,
            2,
            done.reconciliation.as_ref().unwrap(),
            now(4)
        )
        .await,
        Err(JournalError::RevisionConflict)
    ));
    f.close().await;
}

#[tokio::test]
async fn invalid_inputs_do_not_write_and_only_lifecycle_open_can_omit_binding() {
    let f = Fixture::new().await;
    let mut i = intent();
    i.binding_digest = None;
    assert!(
        matches!(journal::prepare(f.tx().await, &i, now(0)).await, Err(JournalError::InvalidInput(e)) if e.code == InputReasonCode::BindingRequired)
    );
    i.origin = EffectOrigin::Lifecycle;
    i.operation = "workspace_open".into();
    prepared(&f, &i).await;
    let cases: Vec<(NormalizedIntent, InputReasonCode)> = vec![
        (
            {
                let mut v = intent();
                v.request_digest = "bad".into();
                v
            },
            InputReasonCode::InvalidDigest,
        ),
        (
            {
                let mut v = intent();
                v.target_digest = "A".repeat(64);
                v
            },
            InputReasonCode::InvalidDigest,
        ),
        (
            {
                let mut v = intent();
                v.operation_key = Uuid::nil();
                v
            },
            InputReasonCode::InvalidUuid,
        ),
        (
            {
                let mut v = intent();
                v.owner_lineage = " ".into();
                v
            },
            InputReasonCode::InvalidText,
        ),
        (
            {
                let mut v = intent();
                v.project_key.canonical_path = "/fixture/../outside".into();
                v
            },
            InputReasonCode::InvalidProjectKey,
        ),
        (
            {
                let mut v = intent();
                v.project_key.inode = "01".into();
                v
            },
            InputReasonCode::InvalidProjectKey,
        ),
        (
            {
                let mut v = intent();
                v.result_schema.version = 0;
                v
            },
            InputReasonCode::InvalidSchema,
        ),
    ];
    for (bad, code) in cases {
        assert!(
            matches!(journal::prepare(f.tx().await, &bad, now(0)).await, Err(JournalError::InvalidInput(e)) if e.code == code)
        );
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM xcode_effect_attempts")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    f.close().await;
}

#[tokio::test]
async fn revisions_digest_scope_and_clock_checks_fail_closed() {
    let f = Fixture::new().await;
    let i = intent();
    let a = prepared(&f, &i).await;
    assert!(matches!(
        journal::dispatch(
            f.tx().await,
            &i.owner_lineage,
            a.nonce,
            &i.request_digest,
            9,
            now(1)
        )
        .await,
        Err(JournalError::RevisionConflict)
    ));
    assert!(matches!(
        journal::dispatch(
            f.tx().await,
            &i.owner_lineage,
            a.nonce,
            &"1".repeat(64),
            0,
            now(1)
        )
        .await,
        Err(JournalError::IntentConflict)
    ));
    assert!(matches!(
        journal::dispatch(
            f.tx().await,
            "other-owner",
            a.nonce,
            &i.request_digest,
            0,
            now(1)
        )
        .await,
        Err(JournalError::NotFound)
    ));
    assert!(
        matches!(journal::dispatch(f.tx().await, &i.owner_lineage, a.nonce, &i.request_digest, 0, now(-1)).await, Err(JournalError::InvalidInput(e)) if e.code == InputReasonCode::InvalidTimestamp)
    );
    assert!(journal::get(&f.pool, "other-owner", a.attempt_id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(f.holds().await, 0);
    f.close().await;
}

#[tokio::test]
async fn historical_schema_survives_reopen_and_unrelated_manifest_change() {
    let mut f = Fixture::new().await;
    let i = intent();
    let a = dispatched(&f, &i).await;
    let saved = journal::complete(
        f.tx().await,
        &i.owner_lineage,
        a.attempt_id,
        1,
        &Completion::Succeeded {
            outcome: result(&i),
            proof: proof(),
        },
        now(2),
    )
    .await
    .unwrap();
    let mut later = intent();
    later.result_schema.manifest_digest = "2".repeat(64);
    later.result_schema.version = 2;
    prepared(&f, &later).await;
    f.reopen().await;
    assert_eq!(
        journal::get(&f.pool, &i.owner_lineage, a.attempt_id)
            .await
            .unwrap()
            .unwrap(),
        saved
    );
    let page = journal::list(&f.pool, &i.owner_lineage, 1, None)
        .await
        .unwrap();
    assert_eq!(page.items, vec![saved]);
    let next = journal::list(&f.pool, &i.owner_lineage, 1, page.next_cursor.as_deref())
        .await
        .unwrap();
    assert_eq!(next.items.len(), 1);
    assert_eq!(next.items[0].intent, later);
    assert!(next.next_cursor.is_none());
    assert!(
        journal::list(&f.pool, "other-owner", 1, page.next_cursor.as_deref())
            .await
            .is_err()
    );
    assert!(journal::list(&f.pool, &i.owner_lineage, 0, None)
        .await
        .is_err());
    f.close().await;
}

#[test]
fn bounded_redacted_text_rejects_oversize_and_unknown_envelope_fields() {
    let s = RedactedText::new("Authorization: Bearer fixture-secret").unwrap();
    assert!(!serde_json::to_string(&s)
        .unwrap()
        .contains("fixture-secret"));
    assert!(RedactedText::new(&"x".repeat(513)).is_err());
    assert!(serde_json::from_str::<RedactedText>(&format!("\"{}\"", "x".repeat(513))).is_err());
    let mut value = serde_json::to_value(result(&intent())).unwrap();
    value["raw_payload"] = serde_json::json!({"anything": true});
    assert!(serde_json::from_value::<HistoricalResult>(value).is_err());
}

#[tokio::test]
async fn hold_blocks_both_path_replacement_and_device_inode_alias() {
    for replace_inode in [false, true] {
        let f = Fixture::new().await;
        let i = intent();
        let mut alias = i.clone();
        alias.operation_key = Uuid::new_v4();
        if replace_inode {
            alias.project_key.inode = "123".into();
        } else {
            alias.project_key.canonical_path = "/alias/App.xcodeproj".into();
        }
        let waiting = prepared(&f, &alias).await;
        dispatched(&f, &i).await;
        assert!(matches!(
            journal::dispatch(
                f.tx().await,
                &alias.owner_lineage,
                waiting.nonce,
                &alias.request_digest,
                0,
                now(2)
            )
            .await,
            Err(JournalError::ProjectHeld)
        ));
        alias.operation_key = Uuid::new_v4();
        assert!(matches!(
            journal::prepare(f.tx().await, &alias, now(2)).await,
            Err(JournalError::ProjectHeld)
        ));
        assert_eq!(f.holds().await, 1);
        f.close().await;
    }
}

#[tokio::test]
async fn recovery_batch_with_stale_revision_rolls_back_every_attempt() {
    let f = Fixture::new().await;
    let i = intent();
    let a = dispatched(&f, &i).await;
    let mut j = intent();
    j.project_key.canonical_path = "/fixture/B.xcodeproj".into();
    j.project_key.inode = "99".into();
    let b = dispatched(&f, &j).await;
    let expected = [
        AttemptRevision {
            attempt_id: a.attempt_id,
            revision: 1,
        },
        AttemptRevision {
            attempt_id: b.attempt_id,
            revision: 0,
        },
    ];
    assert!(matches!(
        journal::recover_dispatched(f.tx().await, &expected, now(2)).await,
        Err(JournalError::RevisionConflict)
    ));
    assert_eq!(
        journal::get(&f.pool, &i.owner_lineage, a.attempt_id)
            .await
            .unwrap()
            .unwrap(),
        a
    );
    assert_eq!(
        journal::get(&f.pool, &j.owner_lineage, b.attempt_id)
            .await
            .unwrap()
            .unwrap(),
        b
    );
    assert_eq!(f.holds().await, 2);
    f.close().await;
}

#[tokio::test]
async fn hold_release_failure_rolls_back_success_outcome_and_retains_fence() {
    let f = Fixture::new().await;
    let i = intent();
    let a = dispatched(&f, &i).await;
    sqlx::query("CREATE TRIGGER reject_release BEFORE DELETE ON xcode_project_holds BEGIN SELECT RAISE(ABORT, 'fixture release failure'); END")
        .execute(&f.pool).await.unwrap();
    assert!(journal::complete(
        f.tx().await,
        &i.owner_lineage,
        a.attempt_id,
        1,
        &Completion::Succeeded {
            outcome: result(&i),
            proof: proof()
        },
        now(2)
    )
    .await
    .is_err());
    assert_eq!(
        journal::get(&f.pool, &i.owner_lineage, a.attempt_id)
            .await
            .unwrap()
            .unwrap(),
        a
    );
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

#[tokio::test]
async fn invalid_completion_evidence_schema_or_result_shape_never_releases_hold() {
    let f = Fixture::new().await;
    let i = intent();
    let a = dispatched(&f, &i).await;
    let mut unproven = proof();
    unproven.no_operation_in_flight = false;
    let mut no_evidence = proof();
    no_evidence.evidence_ref.clear();
    let mut changed_schema = result(&i);
    changed_schema.schema.version = 2;
    let cases = [
        Completion::Succeeded {
            outcome: result(&i),
            proof: unproven,
        },
        Completion::Succeeded {
            outcome: result(&i),
            proof: no_evidence,
        },
        Completion::Succeeded {
            outcome: changed_schema,
            proof: proof(),
        },
        Completion::Succeeded {
            outcome: error(&i, EffectErrorCode::TerminalFailure),
            proof: proof(),
        },
        Completion::Failed {
            outcome: result(&i),
            proof: proof(),
        },
        Completion::Failed {
            outcome: error(&i, EffectErrorCode::OutcomeUnknown),
            proof: proof(),
        },
    ];
    for c in cases {
        assert!(matches!(
            journal::complete(f.tx().await, &i.owner_lineage, a.attempt_id, 1, &c, now(2)).await,
            Err(JournalError::InvalidInput(_))
        ));
        assert_eq!(
            journal::get(&f.pool, &i.owner_lineage, a.attempt_id)
                .await
                .unwrap()
                .unwrap(),
            a
        );
        assert_eq!(f.holds().await, 1);
    }
    f.close().await;
}

#[tokio::test]
async fn database_constraints_reject_unknown_states_and_unsettled_hold_removal() {
    let f = Fixture::new().await;
    let i = intent();
    let a = dispatched(&f, &i).await;
    assert!(sqlx::query("DELETE FROM xcode_project_holds")
        .execute(&f.pool)
        .await
        .is_err());
    assert!(sqlx::query("DELETE FROM xcode_effect_attempts")
        .execute(&f.pool)
        .await
        .is_err());
    assert!(sqlx::query(
        "UPDATE xcode_effect_attempts SET state = 'prepared', revision = revision + 1"
    )
    .execute(&f.pool)
    .await
    .is_err());
    assert!(sqlx::query(
        "UPDATE xcode_effect_attempts SET state = 'invented', revision = revision + 1"
    )
    .execute(&f.pool)
    .await
    .is_err());
    assert!(sqlx::query(
        "UPDATE xcode_effect_attempts SET intent_json = '{}' WHERE attempt_id = ?"
    )
    .bind(a.attempt_id.to_string())
    .execute(&f.pool)
    .await
    .is_err());
    assert_eq!(
        journal::get(&f.pool, &i.owner_lineage, a.attempt_id)
            .await
            .unwrap()
            .unwrap(),
        a
    );
    f.close().await;
}

#[tokio::test]
async fn file_backed_mutation_requires_existing_writer_admission() {
    let dir = tempfile::tempdir().unwrap();
    let url = format!(
        "sqlite://{}?mode=rwc",
        dir.path().join("unregistered.db").display()
    );
    let pool = db::pool::create_pool(&url).await.unwrap();
    assert!(
        begin_repository_transaction(&pool, "test_xcode_effect_journal")
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM xcode_effect_attempts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    pool.close().await;
}

#[tokio::test]
async fn independent_writer_queues_cannot_dispatch_conflicting_aliases() {
    let f = Fixture::new().await;
    let i = intent();
    let a = prepared(&f, &i).await;
    let mut j = i.clone();
    j.operation_key = Uuid::new_v4();
    j.project_key.canonical_path = "/alias/App.xcodeproj".into();
    let b = prepared(&f, &j).await;
    let second_pool = db::pool::create_pool(&f.url).await.unwrap();
    let second_writer = Arc::new(DbWriter::new(second_pool.clone()));
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let mut tasks = Vec::new();
    for (writer, i, a) in [(f.writer.clone(), i, a), (second_writer.clone(), j, b)] {
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let op = db::writer::class_a_operation(
                "test_xcode_effect_journal",
                db::write_class::WriteLane::CriticalBarrier,
                a.attempt_id.to_string(),
            );
            let tx = writer
                .begin_immediate_transaction(op, "test_xcode_effect_journal")
                .await
                .unwrap();
            journal::dispatch(tx, &i.owner_lineage, a.nonce, &i.request_digest, 0, now(1)).await
        }));
    }
    let mut winners = 0;
    let mut held = 0;
    for t in tasks {
        match t.await.unwrap() {
            Ok(DispatchDecision::Dispatch(_)) => winners += 1,
            Err(JournalError::ProjectHeld) => held += 1,
            other => panic!("unexpected concurrent decision: {other:?}"),
        }
    }
    assert_eq!((winners, held), (1, 1));
    assert_eq!(f.holds().await, 1);
    second_writer.shutdown().await;
    second_pool.close().await;
    f.close().await;
}

#[tokio::test]
async fn deferred_commit_failure_preserves_outcome_and_hold_after_reopen() {
    let mut f = Fixture::new().await;
    let i = intent();
    let a = dispatched(&f, &i).await;
    sqlx::query("CREATE TABLE commit_failure (attempt_id TEXT REFERENCES xcode_effect_attempts(attempt_id) DEFERRABLE INITIALLY DEFERRED)")
        .execute(&f.pool).await.unwrap();
    let mut tx = f.tx().await;
    sqlx::query("INSERT INTO commit_failure VALUES ('absent-attempt')")
        .execute(&mut **tx)
        .await
        .unwrap();
    assert!(matches!(
        journal::complete(
            tx,
            &i.owner_lineage,
            a.attempt_id,
            1,
            &Completion::Succeeded {
                outcome: result(&i),
                proof: proof()
            },
            now(2)
        )
        .await,
        Err(JournalError::Storage(_))
    ));
    f.reopen().await;
    assert_eq!(
        journal::get(&f.pool, &i.owner_lineage, a.attempt_id)
            .await
            .unwrap()
            .unwrap(),
        a
    );
    assert_eq!(f.holds().await, 1);
    let failed_rows: i64 = sqlx::query_scalar("SELECT count(*) FROM commit_failure")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    assert_eq!(failed_rows, 0);
    f.close().await;
}

#[tokio::test]
async fn applied_and_not_applied_reconciliation_preserve_distinct_historical_outcomes() {
    let f = Fixture::new().await;
    for disposition in [
        ReconciliationDisposition::Applied,
        ReconciliationDisposition::NotApplied,
    ] {
        let i = intent();
        let a = dispatched(&f, &i).await;
        journal::recover_dispatched(
            f.tx().await,
            &[AttemptRevision {
                attempt_id: a.attempt_id,
                revision: 1,
            }],
            now(2),
        )
        .await
        .unwrap();
        let outcome = if disposition == ReconciliationDisposition::Applied {
            result(&i)
        } else {
            error(&i, EffectErrorCode::ReconciledNotApplied)
        };
        let r = Reconciliation {
            operator_id: "fixture-operator".into(),
            inspected_state: RedactedText::new("Inspected fixture state").unwrap(),
            proof: proof(),
            disposition,
            outcome,
        };
        let done = journal::reconcile(f.tx().await, &i.owner_lineage, a.attempt_id, 2, &r, now(3))
            .await
            .unwrap();
        assert_eq!(done.outcome, Some(r.outcome));
        assert_eq!(done.revision, 3);
        assert_eq!(done.reconciliation.unwrap().disposition, disposition);
        assert_eq!(f.holds().await, 0);
    }
    f.close().await;
}

#[tokio::test]
async fn simultaneous_prepares_return_one_durable_nonce_and_creation_sequence() {
    let f = Fixture::new().await;
    let i = intent();
    let barrier = Arc::new(tokio::sync::Barrier::new(4));
    let mut tasks = Vec::new();
    for _ in 0..4 {
        let pool = f.pool.clone();
        let i = i.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let tx = begin_repository_transaction(&pool, "test_xcode_effect_journal")
                .await
                .unwrap();
            journal::prepare(tx, &i, now(0)).await.unwrap()
        }));
    }
    let first = tasks.remove(0).await.unwrap();
    for t in tasks {
        assert_eq!(t.await.unwrap(), first);
    }
    let page = journal::list(&f.pool, &i.owner_lineage, 100, None)
        .await
        .unwrap();
    assert_eq!(page.items, vec![first]);
    f.close().await;
}

#[tokio::test]
async fn project_key_validates_u64_decimal_identity_without_lossy_coercion() {
    let f = Fixture::new().await;
    let mut i = intent();
    i.project_key.inode = "18446744073709551616".into();
    assert!(matches!(journal::prepare(f.tx().await, &i, now(0)).await,
        Err(JournalError::InvalidInput(e)) if e.code == InputReasonCode::InvalidProjectKey));
    i.project_key.inode = "18446744073709551615".into();
    let saved = prepared(&f, &i).await;
    assert_eq!(saved.intent.project_key.inode, "18446744073709551615");
    f.close().await;
}

#[tokio::test]
async fn project_key_at_path_byte_limit_survives_json_escaping() {
    let f = Fixture::new().await;
    let mut i = intent();
    i.project_key.canonical_path = format!("/{}", "\"".repeat(2047));
    let saved = prepared(&f, &i).await;
    assert_eq!(
        journal::get(&f.pool, &i.owner_lineage, saved.attempt_id)
            .await
            .unwrap()
            .unwrap(),
        saved
    );
    f.close().await;
}

#[tokio::test]
async fn recovery_candidates_are_bounded_ordered_and_only_dispatched() {
    let f = Fixture::new().await;
    assert!(journal::list_dispatched_revisions(&f.pool, 100)
        .await
        .unwrap()
        .is_empty());
    let mut expected = Vec::new();
    for n in 0..3 {
        let mut i = intent();
        i.project_key.canonical_path = format!("/fixture/{n}.xcodeproj");
        i.project_key.inode = (1000 + n).to_string();
        let a = dispatched(&f, &i).await;
        expected.push(AttemptRevision {
            attempt_id: a.attempt_id,
            revision: 1,
        });
    }
    prepared(&f, &intent()).await;
    let first = journal::list_dispatched_revisions(&f.pool, 2)
        .await
        .unwrap();
    assert_eq!(first, expected[..2]);
    journal::recover_dispatched(f.tx().await, &first, now(2))
        .await
        .unwrap();
    assert_eq!(
        journal::list_dispatched_revisions(&f.pool, 100)
            .await
            .unwrap(),
        expected[2..]
    );
    assert_eq!(f.holds().await, 3);
    f.close().await;
}

#[tokio::test]
async fn recovery_candidate_query_rejects_unbounded_limits() {
    let f = Fixture::new().await;
    for limit in [0, 101, u32::MAX] {
        assert!(
            matches!(journal::list_dispatched_revisions(&f.pool, limit).await,
            Err(JournalError::InvalidInput(e)) if e.code == InputReasonCode::InvalidLimit)
        );
    }
    f.close().await;
}
