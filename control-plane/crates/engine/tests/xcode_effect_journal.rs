use std::sync::Arc;

use acp::xcode_effect_journal::{XcodeDispatchDecision, XcodeEffectJournal, XcodeJournalError};
use db::writer::DbWriter;
use domain::xcode_effect::*;
use engine::xcode_effect_journal::DbXcodeEffectJournal;
use sqlx::SqlitePool;
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn frozen_permissions_are_loaded_by_exact_profile_without_live_catalog_fallback() {
    use engine::xcode_effect_journal::frozen_xcode_permission_policy;
    let catalog = serde_json::json!({"permission_profiles":{"VERIFY":{"xcode_headless":{"read":true,"gates":["fast"]}}}}).to_string();
    assert_eq!(
        frozen_xcode_permission_policy(Some(&catalog), Some("VERIFY")).unwrap()["xcode_headless"]
            ["gates"],
        serde_json::json!(["fast"])
    );
    assert!(frozen_xcode_permission_policy(None, Some("VERIFY")).is_err());
    assert!(frozen_xcode_permission_policy(Some(&catalog), Some("CODE_WRITE")).is_err());
    assert!(frozen_xcode_permission_policy(Some(&catalog), None).is_err());
}

#[tokio::test]
async fn coordinator_hold_readback_covers_path_and_inode_aliases_and_fails_when_db_unavailable() {
    use acp::xcode_coordinator::ProjectHoldCheck;
    let f = Fixture::new().await;
    let checker = engine::xcode_effect_journal::DbXcodeProjectHoldCheck(f.pool.clone());
    assert!(!checker.is_held(&f.intent.project_key).await.unwrap());
    f.dispatched().await;
    assert!(checker.is_held(&f.intent.project_key).await.unwrap());
    let mut alias = f.intent.project_key.clone();
    alias.canonical_path = "/fixture/Alias.xcodeproj".into();
    assert!(checker.is_held(&alias).await.unwrap());
    alias.inode = "99".into();
    assert!(!checker.is_held(&alias).await.unwrap());
    f.writer.shutdown().await;
    f.pool.close().await;
    assert!(checker.is_held(&f.intent.project_key).await.is_err());
}

struct Fixture {
    _dir: TempDir,
    pool: SqlitePool,
    writer: Arc<DbWriter>,
    journal: Arc<dyn XcodeEffectJournal>,
    intent: NormalizedIntent,
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
        let intent = NormalizedIntent {
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
        };
        let journal = Arc::new(DbXcodeEffectJournal::new(
            pool.clone(),
            writer.clone(),
            intent.run_id,
            intent.owner_lineage.clone(),
        ));
        Self {
            _dir: dir,
            pool,
            writer,
            journal,
            intent,
        }
    }

    fn scoped(&self, run_id: Uuid, owner: &str) -> DbXcodeEffectJournal {
        DbXcodeEffectJournal::new(
            self.pool.clone(),
            self.writer.clone(),
            run_id,
            owner.to_owned(),
        )
    }

    async fn holds(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM xcode_project_holds")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn attempts(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM xcode_effect_attempts")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn dispatched(&self) -> StoredAttempt {
        let prepared = self.journal.prepare(&self.intent).await.unwrap();
        match self
            .journal
            .dispatch(prepared.nonce, &self.intent.request_digest, 0)
            .await
            .unwrap()
        {
            XcodeDispatchDecision::Dispatch(attempt) => attempt,
            XcodeDispatchDecision::Existing(_) => panic!("first dispatch must win"),
        }
    }

    async fn close(self) {
        self.writer.shutdown().await;
        self.pool.close().await;
    }
}

fn revision(attempt: &StoredAttempt) -> AttemptRevision {
    AttemptRevision {
        attempt_id: attempt.attempt_id,
        revision: attempt.revision,
    }
}

fn outcome(intent: &NormalizedIntent, code: Option<EffectErrorCode>) -> HistoricalResult {
    HistoricalResult {
        schema_version: 1,
        schema: intent.result_schema.clone(),
        payload: match code {
            Some(code) => HistoricalPayload::Error {
                code,
                message: RedactedText::new("Fixture error").unwrap(),
            },
            None => HistoricalPayload::Result {
                summary: RedactedText::new("Fixture applied").unwrap(),
            },
        },
        result_digest: "f".repeat(64),
    }
}

fn success(intent: &NormalizedIntent) -> Completion {
    Completion::Succeeded {
        outcome: outcome(intent, None),
        proof: SettlementProof {
            evidence_ref: "fixture/evidence/settled".into(),
            active_operation_check_ref: "fixture/evidence/no-active-operation".into(),
            no_operation_in_flight: true,
        },
    }
}

#[tokio::test]
async fn prepare_dispatch_unknown_keeps_durable_hold_and_readback() {
    let f = Fixture::new().await;
    let prepared = f.journal.prepare(&f.intent).await.unwrap();
    assert_eq!(
        (prepared.state, prepared.revision),
        (AttemptState::Prepared, 0)
    );
    assert_eq!(f.holds().await, 0);
    let dispatched = f.dispatched().await;
    assert_eq!(
        (dispatched.state, dispatched.revision),
        (AttemptState::Dispatched, 1)
    );
    assert_eq!(f.holds().await, 1);
    let unknown = f
        .journal
        .complete(
            revision(&dispatched),
            &Completion::Unknown {
                reason: UncertaintyReason::TransportLost,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (unknown.state, unknown.revision),
        (AttemptState::Unknown, 2)
    );
    assert_eq!(
        unknown.uncertainty_reason,
        Some(UncertaintyReason::TransportLost)
    );
    assert!(unknown.completed_at.is_none());
    assert_eq!(f.holds().await, 1);
    assert_eq!(
        f.journal.get(unknown.attempt_id).await.unwrap(),
        Some(unknown.clone())
    );
    assert!(
        matches!(f.journal.dispatch(unknown.nonce, &f.intent.request_digest, 0).await.unwrap(),
        XcodeDispatchDecision::Existing(a) if a.state == AttemptState::Unknown && a.revision == 2)
    );
    f.close().await;
}

#[tokio::test]
async fn prepare_duplicate_preserves_original_invocation_and_nonce() {
    let f = Fixture::new().await;
    let original = f.journal.prepare(&f.intent).await.unwrap();
    let mut repeated = f.intent.clone();
    repeated.invocation_id = Uuid::new_v4();
    let duplicate = f.journal.prepare(&repeated).await.unwrap();
    assert_eq!(duplicate, original);
    assert_eq!(f.attempts().await, 1);
    repeated.request_digest = "1".repeat(64);
    assert!(matches!(
        f.journal.prepare(&repeated).await,
        Err(XcodeJournalError::IntentConflict)
    ));
    assert_eq!(f.attempts().await, 1);
    f.close().await;
}

#[tokio::test]
async fn concurrent_same_nonce_dispatch_returns_exactly_one_dispatch() {
    let f = Fixture::new().await;
    let a = f.journal.prepare(&f.intent).await.unwrap();
    let (left, right) = tokio::join!(
        f.journal.dispatch(a.nonce, &f.intent.request_digest, 0),
        f.journal.dispatch(a.nonce, &f.intent.request_digest, 0),
    );
    let mut sends = 0;
    let mut existing = 0;
    for result in [left, right] {
        match result.unwrap() {
            XcodeDispatchDecision::Dispatch(_) => sends += 1,
            XcodeDispatchDecision::Existing(a) => {
                existing += 1;
                assert_eq!((a.state, a.revision), (AttemptState::Dispatched, 1));
            }
        }
    }
    assert_eq!((sends, existing), (1, 1));
    assert_eq!(f.holds().await, 1);
    assert_eq!(
        f.journal.get(a.attempt_id).await.unwrap().unwrap().revision,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn prepare_rejects_intents_outside_fixed_run_or_owner_before_writing() {
    let f = Fixture::new().await;
    let mut other_run = f.intent.clone();
    other_run.run_id = Uuid::new_v4();
    let mut other_owner = f.intent.clone();
    other_owner.owner_lineage = "other-owner".into();
    for intent in [other_run, other_owner] {
        assert!(matches!(
            f.journal.prepare(&intent).await,
            Err(XcodeJournalError::ScopeMismatch)
        ));
    }
    assert_eq!(f.attempts().await, 0);
    f.close().await;
}

#[tokio::test]
async fn foreign_run_or_owner_cannot_read_dispatch_cancel_or_complete_attempt() {
    let f = Fixture::new().await;
    let a = f.journal.prepare(&f.intent).await.unwrap();
    let strangers = [
        f.scoped(Uuid::new_v4(), &f.intent.owner_lineage),
        f.scoped(f.intent.run_id, "other-owner"),
    ];
    for journal in &strangers {
        assert!(journal.get(a.attempt_id).await.unwrap().is_none());
        assert!(matches!(
            journal.dispatch(a.nonce, &f.intent.request_digest, 0).await,
            Err(XcodeJournalError::NotFound)
        ));
        assert!(matches!(
            journal
                .cancel(
                    revision(&a),
                    &outcome(&f.intent, Some(EffectErrorCode::CancelledBeforeDispatch))
                )
                .await,
            Err(XcodeJournalError::NotFound)
        ));
    }
    assert_eq!(
        f.journal.get(a.attempt_id).await.unwrap().unwrap().state,
        AttemptState::Prepared
    );
    let dispatched = f.dispatched().await;
    for journal in &strangers {
        assert!(journal.get(a.attempt_id).await.unwrap().is_none());
        assert!(matches!(
            journal
                .complete(revision(&dispatched), &success(&f.intent))
                .await,
            Err(XcodeJournalError::NotFound)
        ));
    }
    assert_eq!(
        f.journal.get(a.attempt_id).await.unwrap().unwrap().revision,
        1
    );
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

#[tokio::test]
async fn prepare_same_owner_key_in_other_run_does_not_return_foreign_attempt() {
    let f = Fixture::new().await;
    let original = f.journal.prepare(&f.intent).await.unwrap();
    let mut other = f.intent.clone();
    other.run_id = Uuid::new_v4();
    let journal = f.scoped(other.run_id, &other.owner_lineage);
    assert!(matches!(
        journal.prepare(&other).await,
        Err(XcodeJournalError::IntentConflict)
    ));
    assert!(journal.get(original.attempt_id).await.unwrap().is_none());
    assert_eq!(f.attempts().await, 1);
    f.close().await;
}

#[tokio::test]
async fn missing_attempt_is_none_for_get_and_not_found_for_mutations() {
    let f = Fixture::new().await;
    let id = Uuid::new_v4();
    let a = AttemptRevision {
        attempt_id: id,
        revision: 0,
    };
    assert!(f.journal.get(id).await.unwrap().is_none());
    assert!(matches!(
        f.journal.dispatch(id, &f.intent.request_digest, 0).await,
        Err(XcodeJournalError::NotFound)
    ));
    assert!(matches!(
        f.journal.complete(a, &success(&f.intent)).await,
        Err(XcodeJournalError::NotFound)
    ));
    assert!(matches!(
        f.journal
            .cancel(
                a,
                &outcome(&f.intent, Some(EffectErrorCode::CancelledBeforeDispatch))
            )
            .await,
        Err(XcodeJournalError::NotFound)
    ));
    assert_eq!(f.attempts().await, 0);
    f.close().await;
}

#[tokio::test]
async fn corrupt_stored_intent_is_not_treated_as_missing_or_dispatchable() {
    let f = Fixture::new().await;
    let a = f.journal.prepare(&f.intent).await.unwrap();
    // Corrupt only this disposable fixture to exercise repository decode failure.
    sqlx::raw_sql(
        "DROP TRIGGER xcode_effect_identity_immutable; DROP TRIGGER xcode_effect_transition_guard;",
    )
    .execute(&f.pool)
    .await
    .unwrap();
    sqlx::query("UPDATE xcode_effect_attempts SET intent_json = json_set(intent_json, '$.run_id', 'not-a-uuid') WHERE attempt_id = ?")
        .bind(a.attempt_id.to_string()).execute(&f.pool).await.unwrap();
    assert!(matches!(
        f.journal.get(a.attempt_id).await,
        Err(XcodeJournalError::CorruptRecord)
    ));
    assert!(matches!(
        f.journal
            .dispatch(a.nonce, &f.intent.request_digest, 0)
            .await,
        Err(XcodeJournalError::CorruptRecord)
    ));
    assert_eq!(f.holds().await, 0);
    f.close().await;
}

#[tokio::test]
async fn stopped_writer_denies_new_intent_and_dispatch_without_pool_fallback() {
    let f = Fixture::new().await;
    let a = f.journal.prepare(&f.intent).await.unwrap();
    f.writer.shutdown().await;
    let mut other = f.intent.clone();
    other.operation_key = Uuid::new_v4();
    assert!(matches!(
        f.journal.prepare(&other).await,
        Err(XcodeJournalError::StorageUnavailable)
    ));
    assert!(matches!(
        f.journal
            .dispatch(a.nonce, &f.intent.request_digest, 0)
            .await,
        Err(XcodeJournalError::StorageUnavailable)
    ));
    assert_eq!(f.attempts().await, 1);
    assert_eq!(f.holds().await, 0);
    assert_eq!(
        f.journal.get(a.attempt_id).await.unwrap().unwrap().state,
        AttemptState::Prepared
    );
    f.close().await;
}

#[tokio::test]
async fn completion_storage_failure_retains_dispatch_fence_and_hold() {
    let f = Fixture::new().await;
    let a = f.dispatched().await;
    sqlx::raw_sql("CREATE TRIGGER fixture_reject_completion BEFORE UPDATE ON xcode_effect_attempts WHEN NEW.state = 'succeeded' BEGIN SELECT RAISE(ABORT, 'fixture_storage_failure'); END;")
        .execute(&f.pool).await.unwrap();
    assert!(matches!(
        f.journal.complete(revision(&a), &success(&f.intent)).await,
        Err(XcodeJournalError::StorageUnavailable)
    ));
    let saved = f.journal.get(a.attempt_id).await.unwrap().unwrap();
    assert_eq!((saved.state, saved.revision), (AttemptState::Dispatched, 1));
    assert!(saved.outcome.is_none());
    assert_eq!(f.holds().await, 1);
    assert!(matches!(
        f.journal
            .dispatch(a.nonce, &f.intent.request_digest, 0)
            .await
            .unwrap(),
        XcodeDispatchDecision::Existing(_)
    ));
    f.close().await;
}

#[tokio::test]
async fn hold_digest_revision_and_transition_errors_remain_distinct() {
    let f = Fixture::new().await;
    let a = f.dispatched().await;
    let mut conflicting = f.intent.clone();
    conflicting.operation_key = Uuid::new_v4();
    assert!(matches!(
        f.journal.prepare(&conflicting).await,
        Err(XcodeJournalError::ProjectHeld)
    ));
    assert!(matches!(
        f.journal.dispatch(a.nonce, &"1".repeat(64), 1).await,
        Err(XcodeJournalError::IntentConflict)
    ));
    let stale = AttemptRevision {
        attempt_id: a.attempt_id,
        revision: 0,
    };
    assert!(matches!(
        f.journal.complete(stale, &success(&f.intent)).await,
        Err(XcodeJournalError::RevisionConflict)
    ));
    assert!(matches!(
        f.journal
            .cancel(
                revision(&a),
                &outcome(&f.intent, Some(EffectErrorCode::CancelledBeforeDispatch))
            )
            .await,
        Err(XcodeJournalError::InvalidTransition)
    ));
    assert_eq!(f.holds().await, 1);
    f.close().await;
}

#[tokio::test]
async fn complete_duplicate_cannot_resettle_or_authorize_resend() {
    let f = Fixture::new().await;
    let a = f.dispatched().await;
    let terminal = f
        .journal
        .complete(revision(&a), &success(&f.intent))
        .await
        .unwrap();
    assert_eq!(
        (terminal.state, terminal.revision),
        (AttemptState::Succeeded, 2)
    );
    assert_eq!(f.holds().await, 0);
    assert!(matches!(
        f.journal.complete(revision(&a), &success(&f.intent)).await,
        Err(XcodeJournalError::RevisionConflict)
    ));
    assert!(
        matches!(f.journal.dispatch(a.nonce, &f.intent.request_digest, 0).await.unwrap(),
        XcodeDispatchDecision::Existing(saved) if saved == terminal)
    );
    f.close().await;
}

#[tokio::test]
async fn cancel_duplicate_cannot_resettle_or_authorize_dispatch() {
    let f = Fixture::new().await;
    let a = f.journal.prepare(&f.intent).await.unwrap();
    let cancelled = outcome(&f.intent, Some(EffectErrorCode::CancelledBeforeDispatch));
    let terminal = f.journal.cancel(revision(&a), &cancelled).await.unwrap();
    assert_eq!(
        (terminal.state, terminal.revision),
        (AttemptState::Failed, 1)
    );
    assert!(terminal.dispatched_at.is_none());
    assert_eq!(f.holds().await, 0);
    assert!(matches!(
        f.journal.cancel(revision(&a), &cancelled).await,
        Err(XcodeJournalError::RevisionConflict)
    ));
    assert!(
        matches!(f.journal.dispatch(a.nonce, &f.intent.request_digest, 0).await.unwrap(),
        XcodeDispatchDecision::Existing(saved) if saved == terminal)
    );
    f.close().await;
}

#[tokio::test]
async fn invalid_input_is_typed_and_does_not_write() {
    let f = Fixture::new().await;
    let mut invalid = f.intent.clone();
    invalid.request_digest = "invalid".into();
    assert!(matches!(
        f.journal.prepare(&invalid).await,
        Err(XcodeJournalError::InvalidInput(InputError {
            field: "request_digest",
            code: InputReasonCode::InvalidDigest
        }))
    ));
    assert_eq!(f.attempts().await, 0);
    f.close().await;
}
