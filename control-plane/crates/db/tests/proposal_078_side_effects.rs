use chrono::{Duration, Utc};
use db::pool::create_pool;
use db::repos::side_effects::{
    executor_settle_cas, executor_start_cas, insert, list_unresolved, list_unresolved_for_run,
    list_unresolved_for_stage, mark_external_write_started, reaper_transition_cas,
    DispositionOutcome, ExecutorSettleCasParams, ExecutorStartCasParams, ReaperTransitionCasParams,
};
use domain::ids::{RunId, StageExecutionId};
use domain::side_effect::{
    EffectKind, SideEffect, SideEffectAttemptId, SideEffectId, SideEffectStatus,
};

async fn test_pool() -> sqlx::SqlitePool {
    create_pool("sqlite::memory:").await.expect("in-memory pool")
}

fn make_effect(
    pool_placeholder: &str,
    run_id: RunId,
    stage_id: StageExecutionId,
    kind: EffectKind,
    target_key: &str,
) -> SideEffect {
    let now = Utc::now();
    SideEffect {
        id: SideEffectId::from_str(&format!("eff-{pool_placeholder}")),
        run_id,
        stage_execution_id: stage_id,
        agent_execution_id: None,
        effect_kind: kind,
        target_key: target_key.to_owned(),
        idempotency_key: format!("idem-{pool_placeholder}"),
        idempotency_key_version: 1,
        request_fingerprint: format!("fp-{pool_placeholder}"),
        request_fingerprint_version: 1,
        status: SideEffectStatus::Prepared,
        owner_instance_id: None,
        lease_acquired_at: None,
        lease_renewed_at: None,
        lease_expires_at: None,
        deadline_at: Some(now + Duration::seconds(120)),
        external_write_started_at: None,
        external_write_attempted: false,
        attempt_budget_remaining: 3,
        expected_evidence_json: None,
        observed_evidence_summary_json: None,
        evidence_root: None,
        last_error_kind: None,
        last_error: None,
        settlement_txn_id: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn proposal_078_migration_046_creates_tables() {
    let pool = test_pool().await;
    // Verify tables exist by querying the sqlite schema
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'side_effect%' ORDER BY name"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(
        tables.contains(&"side_effects".to_string()),
        "side_effects table must exist; found: {:?}",
        tables
    );
    assert!(
        tables.contains(&"side_effect_attempts".to_string()),
        "side_effect_attempts table must exist"
    );
    assert!(
        tables.contains(&"side_effect_settlements".to_string()),
        "side_effect_settlements table must exist"
    );
}

#[tokio::test]
async fn proposal_078_migration_additive_existing_tables_unaffected() {
    let pool = test_pool().await;
    // Insert a run and verify it's still readable (migration was additive)
    let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM runs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(run_count, 0, "runs table must be intact");
}

#[tokio::test]
async fn proposal_078_check_constraint_rejects_unknown_effect_kind() {
    let pool = test_pool().await;
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"INSERT INTO side_effects
           (id, run_id, stage_execution_id, effect_kind, target_key,
            idempotency_key, idempotency_key_version, request_fingerprint,
            request_fingerprint_version, status, external_write_attempted,
            attempt_budget_remaining, created_at, updated_at)
           VALUES ('x', 'r', 's', 'unknown_kind_xyz', 'key', 'idem', 1, 'fp', 1,
                   'prepared', 0, 3, ?1, ?2)"#,
    )
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "CHECK constraint must reject unknown effect_kind"
    );
}

#[tokio::test]
async fn proposal_078_check_constraint_rejects_unknown_status() {
    let pool = test_pool().await;
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        r#"INSERT INTO side_effects
           (id, run_id, stage_execution_id, effect_kind, target_key,
            idempotency_key, idempotency_key_version, request_fingerprint,
            request_fingerprint_version, status, external_write_attempted,
            attempt_budget_remaining, created_at, updated_at)
           VALUES ('x', 'r', 's', 'git_push', 'key', 'idem', 1, 'fp', 1,
                   'invalid_status_xyz', 0, 3, ?1, ?2)"#,
    )
    .bind(&now)
    .bind(&now)
    .execute(&pool)
    .await;
    assert!(
        result.is_err(),
        "CHECK constraint must reject unknown status"
    );
}

#[tokio::test]
async fn proposal_078_idempotency_key_unique_constraint() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    let mut eff1 = make_effect("001", run_id, stage_id, EffectKind::GitCommit, "target-1");
    eff1.idempotency_key = "shared-idem-key".to_owned();
    insert(&pool, &eff1).await.unwrap();

    let mut eff2 = make_effect("002", run_id, stage_id, EffectKind::GitPush, "target-2");
    eff2.idempotency_key = "shared-idem-key".to_owned(); // same key
    let result = insert(&pool, &eff2).await;
    assert!(
        result.is_err(),
        "unique constraint on idempotency_key must be enforced"
    );
}

#[tokio::test]
async fn proposal_078_insert_and_find_roundtrip() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect("rtrip", run_id, stage_id, EffectKind::GitPush, "refs/heads/main");

    insert(&pool, &eff).await.unwrap();
    let loaded = db::repos::side_effects::find_by_id(&pool, &eff.id)
        .await
        .unwrap()
        .expect("effect must be findable");

    assert_eq!(loaded.id.to_string(), eff.id.to_string());
    assert_eq!(loaded.status, SideEffectStatus::Prepared);
    assert_eq!(loaded.effect_kind, EffectKind::GitPush);
}

#[tokio::test]
async fn proposal_078_list_unresolved_for_run() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let other_run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    let eff1 = make_effect("lr1", run_id, stage_id, EffectKind::GitCommit, "t1");
    let eff2 = make_effect("lr2", run_id, stage_id, EffectKind::GitPush, "t2");
    let eff_other = make_effect("lro", other_run_id, stage_id, EffectKind::BuildArchive, "t3");

    insert(&pool, &eff1).await.unwrap();
    insert(&pool, &eff2).await.unwrap();
    insert(&pool, &eff_other).await.unwrap();

    let unresolved = list_unresolved_for_run(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert_eq!(unresolved.len(), 2, "must return 2 effects for run");

    let other_unresolved = list_unresolved_for_run(&pool, &other_run_id.to_string())
        .await
        .unwrap();
    assert_eq!(other_unresolved.len(), 1, "must return 1 effect for other run");
}

#[tokio::test]
async fn proposal_078_executor_start_cas_prepared_to_executing() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect("cas-start", run_id, stage_id, EffectKind::GitCommit, "target");
    insert(&pool, &eff).await.unwrap();

    let now = Utc::now();
    let attempt_id = SideEffectAttemptId::new();
    let params = ExecutorStartCasParams {
        effect_id: &eff.id,
        owner_instance_id: "inst-1",
        attempt_id: &attempt_id,
        lease_acquired_at: now,
        lease_expires_at: now + Duration::seconds(30),
        deadline_at: None,
        now,
    };

    let won = executor_start_cas(&pool, &params).await.unwrap();
    assert!(won, "first executor_start_cas must succeed");

    let loaded = db::repos::side_effects::find_by_id(&pool, &eff.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.status, SideEffectStatus::Executing);
    assert_eq!(loaded.owner_instance_id.as_deref(), Some("inst-1"));
}

#[tokio::test]
async fn proposal_078_executor_start_cas_race_one_winner() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect("cas-race", run_id, stage_id, EffectKind::GitPush, "race-target");
    insert(&pool, &eff).await.unwrap();

    let now = Utc::now();
    let attempt_a = SideEffectAttemptId::new();
    let attempt_b = SideEffectAttemptId::new();

    let params_a = ExecutorStartCasParams {
        effect_id: &eff.id,
        owner_instance_id: "inst-a",
        attempt_id: &attempt_a,
        lease_acquired_at: now,
        lease_expires_at: now + Duration::seconds(30),
        deadline_at: None,
        now,
    };
    let params_b = ExecutorStartCasParams {
        effect_id: &eff.id,
        owner_instance_id: "inst-b",
        attempt_id: &attempt_b,
        lease_acquired_at: now,
        lease_expires_at: now + Duration::seconds(30),
        deadline_at: None,
        now,
    };

    let won_a = executor_start_cas(&pool, &params_a).await.unwrap();
    let won_b = executor_start_cas(&pool, &params_b).await.unwrap();

    // Exactly one must win
    let winners = [won_a, won_b].into_iter().filter(|&w| w).count();
    assert_eq!(
        winners, 1,
        "exactly one executor_start_cas must win; won_a={won_a} won_b={won_b}"
    );
}

#[tokio::test]
async fn proposal_078_external_write_attempted_at_most_once() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect(
        "ext-write",
        run_id,
        stage_id,
        EffectKind::GitPush,
        "ext-write-target",
    );
    insert(&pool, &eff).await.unwrap();

    let now = Utc::now();
    let expires = now + Duration::seconds(30);
    let attempt_id = SideEffectAttemptId::new();

    // Must be in executing state before mark_external_write_started is valid.
    let start_params = ExecutorStartCasParams {
        effect_id: &eff.id,
        owner_instance_id: "inst-ext-write",
        attempt_id: &attempt_id,
        lease_acquired_at: now,
        lease_expires_at: expires,
        deadline_at: None,
        now,
    };
    let started = executor_start_cas(&pool, &start_params).await.unwrap();
    assert!(started, "executor_start_cas must succeed before external write");

    // First mark: succeeds (status=executing, correct owner, not yet attempted)
    let ok1 = mark_external_write_started(&pool, &eff.id, "inst-ext-write", now)
        .await
        .unwrap();
    assert!(ok1, "first mark_external_write_started must succeed");

    // Second mark: fails (already attempted)
    let ok2 = mark_external_write_started(&pool, &eff.id, "inst-ext-write", now)
        .await
        .unwrap();
    assert!(
        !ok2,
        "second mark_external_write_started must fail — at most one write attempt allowed"
    );

    let loaded = db::repos::side_effects::find_by_id(&pool, &eff.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        loaded.external_write_attempted,
        "external_write_attempted must be true"
    );
}

#[tokio::test]
async fn proposal_078_mark_external_write_started_requires_executing_state() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect(
        "ext-write-state",
        run_id,
        stage_id,
        EffectKind::GitPush,
        "ext-write-state-target",
    );
    insert(&pool, &eff).await.unwrap();

    let now = Utc::now();
    // Effect is still in prepared state — must fail closed.
    let ok = mark_external_write_started(&pool, &eff.id, "inst-any", now)
        .await
        .unwrap();
    assert!(
        !ok,
        "mark_external_write_started must fail when effect is in prepared (not executing) state"
    );
}

#[tokio::test]
async fn proposal_078_reaper_transition_cas_executing_to_needs_reconciliation() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect("reaper", run_id, stage_id, EffectKind::GitCommit, "reaper-tgt");
    insert(&pool, &eff).await.unwrap();

    let now = Utc::now();
    let attempt_id = SideEffectAttemptId::new();
    let start_params = ExecutorStartCasParams {
        effect_id: &eff.id,
        owner_instance_id: "inst-reaper",
        attempt_id: &attempt_id,
        lease_acquired_at: now,
        lease_expires_at: now - Duration::seconds(1), // expired
        deadline_at: None,
        now,
    };
    // Start with an already-expired lease
    executor_start_cas(&pool, &start_params).await.unwrap();

    let loaded = db::repos::side_effects::find_by_id(&pool, &eff.id)
        .await
        .unwrap()
        .unwrap();

    let reaper_params = ReaperTransitionCasParams {
        effect_id: &eff.id,
        observed_status: SideEffectStatus::Executing,
        observed_owner: Some("inst-reaper"),
        observed_lease_renewed_at: loaded.lease_renewed_at,
        observed_updated_at: loaded.updated_at,
        now,
        last_error_kind: "lease_expired",
        last_error: "watchdog: lease expired",
    };

    let transitioned = reaper_transition_cas(&pool, &reaper_params).await.unwrap();
    assert!(transitioned, "reaper_transition_cas must succeed for expired lease");

    let after = db::repos::side_effects::find_by_id(&pool, &eff.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        after.status,
        SideEffectStatus::NeedsReconciliation,
        "status must be needs_reconciliation after watchdog reaper"
    );
}

#[tokio::test]
async fn proposal_078_reaper_and_executor_settle_are_mutually_exclusive() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect("mutex", run_id, stage_id, EffectKind::GitPush, "mutex-target");
    insert(&pool, &eff).await.unwrap();

    let now = Utc::now();
    let expires = now + Duration::seconds(30);
    let attempt_id = SideEffectAttemptId::new();

    let start_params = ExecutorStartCasParams {
        effect_id: &eff.id,
        owner_instance_id: "inst-mutex",
        attempt_id: &attempt_id,
        lease_acquired_at: now,
        lease_expires_at: expires,
        deadline_at: None,
        now,
    };
    let won = executor_start_cas(&pool, &start_params).await.unwrap();
    assert!(won);

    let loaded = db::repos::side_effects::find_by_id(&pool, &eff.id)
        .await
        .unwrap()
        .unwrap();

    let settle_params = ExecutorSettleCasParams {
        effect_id: &eff.id,
        owner_instance_id: "inst-mutex",
        settlement_attempt_id: &attempt_id,
        observed_lease_renewed_at: loaded.lease_renewed_at.unwrap(),
        new_status: SideEffectStatus::Settled,
        observed_evidence_summary_json: None,
        settlement_txn_id: "txn-settle",
        last_error_kind: None,
        last_error: None,
        now,
        settlement_source: "executor",
        receipt_artifact_id: None,
        decision_json: None,
        decision_json_hash: None,
        disposition_id: None,
    };

    let reaper_params = ReaperTransitionCasParams {
        effect_id: &eff.id,
        observed_status: SideEffectStatus::Executing,
        observed_owner: Some("inst-mutex"),
        observed_lease_renewed_at: loaded.lease_renewed_at,
        observed_updated_at: loaded.updated_at,
        now,
        last_error_kind: "lease_expired",
        last_error: "watchdog: simulated race",
    };

    // Apply settle first
    let settled = executor_settle_cas(&pool, &settle_params).await.unwrap();
    assert!(settled, "executor settle must succeed");

    // Reaper trying the same row must lose
    let reaped = reaper_transition_cas(&pool, &reaper_params).await.unwrap();
    assert!(
        !reaped,
        "reaper_transition_cas must fail after executor already settled"
    );

    let after = db::repos::side_effects::find_by_id(&pool, &eff.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        after.status,
        SideEffectStatus::Settled,
        "status must remain settled"
    );
}

#[tokio::test]
async fn proposal_078_settle_blocks_second_settlement() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect(
        "double-settle",
        run_id,
        stage_id,
        EffectKind::ConnectUpload,
        "ds-target",
    );
    insert(&pool, &eff).await.unwrap();

    let now = Utc::now();
    let expires = now + Duration::seconds(30);
    let attempt_id = SideEffectAttemptId::new();

    let start_params = ExecutorStartCasParams {
        effect_id: &eff.id,
        owner_instance_id: "inst-ds",
        attempt_id: &attempt_id,
        lease_acquired_at: now,
        lease_expires_at: expires,
        deadline_at: None,
        now,
    };
    executor_start_cas(&pool, &start_params).await.unwrap();

    let loaded = db::repos::side_effects::find_by_id(&pool, &eff.id)
        .await
        .unwrap()
        .unwrap();

    let p1_attempt = attempt_id.clone();
    let p1_eff = eff.id.clone();
    let lrn = loaded.lease_renewed_at.unwrap();
    let settle1 = ExecutorSettleCasParams {
        effect_id: &p1_eff,
        owner_instance_id: "inst-ds",
        settlement_attempt_id: &p1_attempt,
        observed_lease_renewed_at: lrn,
        new_status: SideEffectStatus::Settled,
        observed_evidence_summary_json: None,
        settlement_txn_id: "txn-1",
        last_error_kind: None,
        last_error: None,
        now,
        settlement_source: "executor",
        receipt_artifact_id: None,
        decision_json: None,
        decision_json_hash: None,
        disposition_id: None,
    };
    let ok1 = executor_settle_cas(&pool, &settle1).await.unwrap();
    assert!(ok1, "first settle must succeed");

    let p2_attempt = attempt_id.clone();
    let p2_eff = eff.id.clone();
    let settle2 = ExecutorSettleCasParams {
        effect_id: &p2_eff,
        owner_instance_id: "inst-ds",
        settlement_attempt_id: &p2_attempt,
        observed_lease_renewed_at: lrn,
        new_status: SideEffectStatus::Settled,
        observed_evidence_summary_json: None,
        settlement_txn_id: "txn-2",
        last_error_kind: None,
        last_error: None,
        now,
        settlement_source: "executor",
        receipt_artifact_id: None,
        decision_json: None,
        decision_json_hash: None,
        disposition_id: None,
    };
    let ok2 = executor_settle_cas(&pool, &settle2).await.unwrap();
    assert!(!ok2, "second settle must fail (settlement already exists)");
}

#[tokio::test]
async fn proposal_078_disposition_id_idempotency() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect(
        "disp-idem",
        run_id,
        stage_id,
        EffectKind::GitPush,
        "disp-target",
    );
    // Manually insert as needs_reconciliation
    {
        let now = Utc::now();
        let mut e = eff.clone();
        e.status = SideEffectStatus::NeedsReconciliation;
        e.created_at = now;
        e.updated_at = now;
        insert(&pool, &e).await.unwrap();
    }

    let decision = r#"{"verified": true, "note": "manual check"}"#;
    let hash = format!("{:016x}", {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        decision.hash(&mut h);
        h.finish()
    });

    // First apply: should succeed
    let outcome1 = db::repos::side_effects::apply_operator_disposition(
        &pool,
        &eff.id,
        SideEffectStatus::Reconciled,
        "mcp_operator",
        "disp-abc-123",
        decision,
        &hash,
        "operator-1",
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(outcome1, DispositionOutcome::Applied);

    // Second apply with same id + same payload: idempotent success
    let outcome2 = db::repos::side_effects::apply_operator_disposition(
        &pool,
        &eff.id,
        SideEffectStatus::Reconciled,
        "mcp_operator",
        "disp-abc-123",
        decision,
        &hash,
        "operator-1",
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(
        outcome2,
        DispositionOutcome::AlreadyApplied,
        "same disposition_id + same payload must be idempotent"
    );
}

#[tokio::test]
async fn proposal_078_disposition_id_mismatch_rejected() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let eff = make_effect("disp-mm", run_id, stage_id, EffectKind::GitCommit, "mm-target");
    {
        let now = Utc::now();
        let mut e = eff.clone();
        e.status = SideEffectStatus::NeedsReconciliation;
        e.created_at = now;
        e.updated_at = now;
        insert(&pool, &e).await.unwrap();
    }

    let decision1 = r#"{"verified": true}"#;
    let hash1 = format!("{:016x}", {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        decision1.hash(&mut h);
        h.finish()
    });

    db::repos::side_effects::apply_operator_disposition(
        &pool,
        &eff.id,
        SideEffectStatus::Reconciled,
        "mcp_operator",
        "disp-shared-id",
        decision1,
        &hash1,
        "op",
        Utc::now(),
    )
    .await
    .unwrap();

    // Now try to reuse disposition_id with different payload
    let decision2 = r#"{"verified": false, "changed": "payload"}"#;
    let hash2 = format!("{:016x}", {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        decision2.hash(&mut h);
        h.finish()
    });

    // Need a new effect in needs_reconciliation since first is now reconciled
    let eff2 = make_effect("disp-mm2", run_id, stage_id, EffectKind::GitPush, "mm2");
    {
        let now = Utc::now();
        let mut e = eff2.clone();
        e.status = SideEffectStatus::NeedsReconciliation;
        e.created_at = now;
        e.updated_at = now;
        insert(&pool, &e).await.unwrap();
    }

    let outcome = db::repos::side_effects::apply_operator_disposition(
        &pool,
        &eff2.id,
        SideEffectStatus::Reconciled,
        "mcp_operator",
        "disp-shared-id",
        decision2,
        &hash2,
        "op",
        Utc::now(),
    )
    .await
    .unwrap();

    assert_eq!(
        outcome,
        DispositionOutcome::PayloadMismatch,
        "changed decision_json for same disposition_id must be rejected"
    );
}

#[tokio::test]
async fn proposal_078_list_unresolved_bounded_by_100() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    // Insert 5 effects
    for i in 0..5 {
        let eff = make_effect(
            &format!("bound-{i}"),
            run_id,
            stage_id,
            EffectKind::GitCommit,
            &format!("tgt-{i}"),
        );
        insert(&pool, &eff).await.unwrap();
    }

    let all = list_unresolved(&pool, 100).await.unwrap();
    assert_eq!(all.len(), 5);

    let limited = list_unresolved(&pool, 3).await.unwrap();
    assert_eq!(limited.len(), 3, "list_unresolved must respect limit");
}

/// HIGH-001 regression: a disposition_id used for effect A must return PayloadMismatch
/// (not AlreadyApplied) when replayed against a different effect B.
/// AlreadyApplied for the wrong effect would falsify reconciliation audit trail.
#[tokio::test]
async fn proposal_078_disposition_id_cross_effect_replay_returns_payload_mismatch() {
    let pool = test_pool().await;
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    let hash = "deadbeef0123456789abcdef01234567";

    // Effect A: settle with disposition_id="shared-disp-id"
    let eff_a = make_effect("cross-a", run_id, stage_id, EffectKind::GitCommit, "tgt-a");
    {
        let now = Utc::now();
        let mut e = eff_a.clone();
        e.status = SideEffectStatus::NeedsReconciliation;
        e.created_at = now;
        e.updated_at = now;
        insert(&pool, &e).await.unwrap();
    }
    let decision_a = r#"{"schema_version":"side_effect_decision_v1","decision":"reconciled","operator_notes":"ok"}"#;
    let outcome_a = db::repos::side_effects::apply_operator_disposition(
        &pool,
        &eff_a.id,
        SideEffectStatus::Reconciled,
        "mcp_operator",
        "shared-disp-id",
        decision_a,
        hash,
        "op",
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(outcome_a, DispositionOutcome::Applied);

    // Effect B: attempt to reuse "shared-disp-id" — must NOT return AlreadyApplied.
    let eff_b = make_effect("cross-b", run_id, stage_id, EffectKind::GitPush, "tgt-b");
    {
        let now = Utc::now();
        let mut e = eff_b.clone();
        e.status = SideEffectStatus::NeedsReconciliation;
        e.created_at = now;
        e.updated_at = now;
        insert(&pool, &e).await.unwrap();
    }
    let decision_b = r#"{"schema_version":"side_effect_decision_v1","decision":"reconciled","operator_notes":"same decision"}"#;
    let outcome_b = db::repos::side_effects::apply_operator_disposition(
        &pool,
        &eff_b.id,
        SideEffectStatus::Reconciled,
        "mcp_operator",
        "shared-disp-id",
        decision_b,
        hash,
        "op",
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(
        outcome_b,
        DispositionOutcome::PayloadMismatch,
        "disposition_id already used for effect A must return PayloadMismatch for effect B, not AlreadyApplied"
    );
}
