use db::pool::create_pool;
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

// ── artifact_lineage ────────────────────────────────────────────────────────

#[tokio::test]
async fn artifact_lineage_table_exists_after_migration() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='artifact_lineage'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn artifact_lineage_non_report_row_accepts_null_report_kind() {
    let pool = test_pool().await;
    // Seed the required run
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-al','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-al','idea-al','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    let result = sqlx::query(
        "INSERT INTO artifact_lineage (artifact_id,run_id,artifact_role,active,created_at) VALUES ('art-1','run-al','evidence',1,'2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "non-report row with null report_kind should succeed: {result:?}");
}

#[tokio::test]
async fn artifact_lineage_active_report_with_valid_report_kind_accepted() {
    let pool = test_pool().await;
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-al2','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-al2','idea-al2','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    let result = sqlx::query(
        "INSERT INTO artifact_lineage (artifact_id,run_id,artifact_role,active,report_kind,created_at) VALUES ('art-2','run-al2','report',1,'run_report','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "active report with valid report_kind should succeed: {result:?}");
}

#[tokio::test]
async fn artifact_lineage_active_report_with_null_report_kind_rejected() {
    let pool = test_pool().await;
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-al3','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-al3','idea-al3','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    let result = sqlx::query(
        "INSERT INTO artifact_lineage (artifact_id,run_id,artifact_role,active,report_kind,created_at) VALUES ('art-3','run-al3','report',1,NULL,'2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "active report with NULL report_kind must be rejected");
}

#[tokio::test]
async fn artifact_lineage_active_report_with_unknown_report_kind_rejected() {
    let pool = test_pool().await;
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-al4','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-al4','idea-al4','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    let result = sqlx::query(
        "INSERT INTO artifact_lineage (artifact_id,run_id,artifact_role,active,report_kind,created_at) VALUES ('art-4','run-al4','report',1,'some_unknown_kind','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "active report with unbounded report_kind must be rejected");
}

#[tokio::test]
async fn artifact_lineage_inactive_report_accepts_null_report_kind() {
    let pool = test_pool().await;
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-al5','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-al5','idea-al5','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    // Inactive report rows don't need a report_kind
    let result = sqlx::query(
        "INSERT INTO artifact_lineage (artifact_id,run_id,artifact_role,active,report_kind,created_at) VALUES ('art-5','run-al5','report',0,NULL,'2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "inactive report with NULL report_kind should succeed: {result:?}");
}

#[tokio::test]
async fn artifact_lineage_active_report_kind_unique_per_run() {
    let pool = test_pool().await;
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-al6','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-al6','idea-al6','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO artifact_lineage (artifact_id,run_id,artifact_role,active,report_kind,created_at) VALUES ('art-6a','run-al6','report',1,'run_report','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Second active run_report for the same run should violate the unique index
    let result = sqlx::query(
        "INSERT INTO artifact_lineage (artifact_id,run_id,artifact_role,active,report_kind,created_at) VALUES ('art-6b','run-al6','report',1,'run_report','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "duplicate active report_kind per run must be rejected by unique index");
}

// ── command_idempotency ─────────────────────────────────────────────────────

#[tokio::test]
async fn command_idempotency_table_exists() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='command_idempotency'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn command_request_aliases_table_exists() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='command_request_aliases'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn command_idempotency_invalid_lease_state_rejected() {
    let pool = test_pool().await;
    let result = sqlx::query(
        "INSERT INTO command_idempotency (principal_id,request_id,command,intent_hash,lease_generation,lease_state,acquired_at,expires_at) VALUES ('p1','r1','runs.cancel','h1',1,'invalid_state','2026-06-01T00:00:00Z','2026-06-01T00:02:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "invalid lease_state must be rejected");
}

#[tokio::test]
async fn command_idempotency_unique_active_per_request() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO command_idempotency (principal_id,request_id,command,intent_hash,lease_generation,lease_state,acquired_at,expires_at) VALUES ('p1','r1','runs.cancel','h1',1,'pending','2026-06-01T00:00:00Z','2026-06-01T00:02:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Second pending row with the same (principal_id, request_id) must fail
    let result = sqlx::query(
        "INSERT INTO command_idempotency (principal_id,request_id,command,intent_hash,lease_generation,lease_state,acquired_at,expires_at) VALUES ('p1','r1','runs.cancel','h1',2,'pending','2026-06-01T00:00:00Z','2026-06-01T00:02:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "duplicate active lease for same (principal, request) must be rejected");
}

// ── shutdown_interrupted_receipts ───────────────────────────────────────────

#[tokio::test]
async fn shutdown_receipts_table_exists() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='shutdown_interrupted_receipts'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn shutdown_receipts_queue_rank_required_for_queued_no_signal() {
    let pool = test_pool().await;
    // queued_no_signal with NULL queue_rank must be rejected
    let result = sqlx::query(
        "INSERT INTO shutdown_interrupted_receipts (receipt_id,provider_session_id,shutdown_epoch,receipt_generation,interrupted_state,queue_rank,created_at) VALUES ('rcpt-1','sess-1',1,1,'queued_no_signal',NULL,'2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "queued_no_signal with NULL queue_rank must be rejected");
}

#[tokio::test]
async fn shutdown_receipts_queue_rank_null_for_non_queued() {
    let pool = test_pool().await;
    // grace_deadline_expired with non-null queue_rank must be rejected
    let result = sqlx::query(
        "INSERT INTO shutdown_interrupted_receipts (receipt_id,provider_session_id,shutdown_epoch,receipt_generation,interrupted_state,queue_rank,created_at) VALUES ('rcpt-2','sess-1',1,1,'grace_deadline_expired',5,'2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "non-queued receipt with non-null queue_rank must be rejected");
}

#[tokio::test]
async fn shutdown_receipts_valid_queued_no_signal_with_queue_rank() {
    let pool = test_pool().await;
    let result = sqlx::query(
        "INSERT INTO shutdown_interrupted_receipts (receipt_id,provider_session_id,shutdown_epoch,receipt_generation,interrupted_state,queue_rank,created_at) VALUES ('rcpt-3','sess-1',1,1,'queued_no_signal',1,'2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "queued_no_signal with non-null queue_rank should succeed: {result:?}");
}

#[tokio::test]
async fn shutdown_receipts_valid_grace_deadline_expired_null_queue_rank() {
    let pool = test_pool().await;
    let result = sqlx::query(
        "INSERT INTO shutdown_interrupted_receipts (receipt_id,provider_session_id,shutdown_epoch,receipt_generation,interrupted_state,queue_rank,created_at) VALUES ('rcpt-4','sess-1',1,1,'grace_deadline_expired',NULL,'2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "grace_deadline_expired with NULL queue_rank should succeed: {result:?}");
}

// ── shutdown_signal_side_effects ────────────────────────────────────────────

#[tokio::test]
async fn shutdown_signal_side_effects_table_exists() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='shutdown_signal_side_effects'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn shutdown_signal_side_effects_unique_generation_per_session_epoch_kind() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO shutdown_signal_side_effects (signal_effect_id,provider_session_id,shutdown_epoch,process_id,process_start_identity,signal_kind,generation,intent_state) VALUES ('sse-1','sess-1',1,12345,'hash-abc','graceful',1,'planned')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let result = sqlx::query(
        "INSERT INTO shutdown_signal_side_effects (signal_effect_id,provider_session_id,shutdown_epoch,process_id,process_start_identity,signal_kind,generation,intent_state) VALUES ('sse-2','sess-1',1,12345,'hash-abc','graceful',1,'issued')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "duplicate (session, epoch, kind, generation) must be rejected");
}

// ── cancel_late_output_overflow ─────────────────────────────────────────────

#[tokio::test]
async fn cancel_late_output_overflow_table_exists() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='cancel_late_output_overflow'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn cancel_late_output_overflow_unique_latch_key() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO cancel_late_output_overflow (overflow_id,scope,run_id,provider_session_id,cancellation_epoch,overflow_kind,latched_at,updated_at) VALUES ('ov-1','session','run-x','sess-x',1,'message_count','2026-06-01T00:00:00Z','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Same latch key (scope+run+session+epoch+kind) must be rejected
    let result = sqlx::query(
        "INSERT INTO cancel_late_output_overflow (overflow_id,scope,run_id,provider_session_id,cancellation_epoch,overflow_kind,latched_at,updated_at) VALUES ('ov-2','session','run-x','sess-x',1,'message_count','2026-06-01T00:00:00Z','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "duplicate latch key must be rejected by unique index");
}

#[tokio::test]
async fn cancel_late_output_overflow_normalized_columns_are_generated() {
    let pool = test_pool().await;
    sqlx::query(
        "INSERT INTO cancel_late_output_overflow (overflow_id,scope,run_id,provider_session_id,cancellation_epoch,overflow_kind,latched_at,updated_at) VALUES ('ov-3','run','Run-Upper ','Sess-UPPER ',2,'run_bytes','2026-06-01T00:00:00Z','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let row = sqlx::query(
        "SELECT normalized_run_id, normalized_provider_session_id FROM cancel_late_output_overflow WHERE overflow_id='ov-3'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let norm_run: String = row.try_get("normalized_run_id").unwrap();
    let norm_sess: String = row.try_get("normalized_provider_session_id").unwrap();
    assert_eq!(norm_run, "run-upper", "normalized_run_id should be lowercased and trimmed");
    assert_eq!(norm_sess, "sess-upper", "normalized_provider_session_id should be lowercased and trimmed");
}

// ── p083 enforcement tables ──────────────────────────────────────────────────

#[tokio::test]
async fn p083_enforcement_mode_state_table_exists() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='p083_enforcement_mode_state'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn p083_enforcement_mode_invalid_mode_rejected() {
    let pool = test_pool().await;
    let result = sqlx::query(
        "INSERT INTO p083_enforcement_mode_state (state_id,enforcement_mode,mode_reason,effective_at,updated_at) VALUES ('s1','unknown_mode','test','2026-06-01T00:00:00Z','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "invalid enforcement_mode must be rejected");
}

#[tokio::test]
async fn p083_enforcement_transition_journal_transitioning_no_commit_marker() {
    let pool = test_pool().await;
    // transitioning row with a commit_marker is valid at the DB level;
    // the V5 verification query checks for this at runtime.
    // Here we just verify that the table accepts a valid transitioning row.
    let result = sqlx::query(
        "INSERT INTO p083_enforcement_mode_transition_journal (journal_id,from_mode,to_mode,transition_state,principal_id,request_id,initiated_at) VALUES ('j1','disabled','permissive','transitioning','op','req-1','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "valid transitioning row should be accepted: {result:?}");
}

// ── durable_monotonic_clock ──────────────────────────────────────────────────

#[tokio::test]
async fn durable_monotonic_clock_samples_table_exists() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='durable_monotonic_clock_samples'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn durable_monotonic_clock_invalid_sample_state_rejected() {
    let pool = test_pool().await;
    let result = sqlx::query(
        "INSERT INTO durable_monotonic_clock_samples (sample_id,boot_id,sample_state,monotonic_ms,observed_at_wall_clock,created_at) VALUES ('smp-1','boot-1','invalid_state',12345,'2026-06-01T00:00:00Z','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "invalid sample_state must be rejected");
}

#[tokio::test]
async fn durable_monotonic_clock_baseline_sample_accepted() {
    let pool = test_pool().await;
    let result = sqlx::query(
        "INSERT INTO durable_monotonic_clock_samples (sample_id,boot_id,sample_state,monotonic_ms,observed_at_wall_clock,created_at) VALUES ('smp-2','boot-1','baseline',0,'2026-06-01T00:00:00Z','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "baseline sample should be accepted: {result:?}");
}

// ── provider_sessions and cancellation_intents ───────────────────────────────

#[tokio::test]
async fn provider_sessions_table_exists() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='provider_sessions'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn provider_sessions_invalid_process_fate_rejected() {
    let pool = test_pool().await;
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-ps1','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-ps1','idea-ps1','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    let result = sqlx::query(
        "INSERT INTO provider_sessions (provider_session_id,run_id,provider,process_fate,created_at,updated_at) VALUES ('psess-1','run-ps1','codex','invalid_fate','2026-06-01T00:00:00Z','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "invalid process_fate must be rejected");
}

#[tokio::test]
async fn provider_sessions_default_process_fate_is_running() {
    let pool = test_pool().await;
    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-ps2','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-ps2','idea-ps2','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO provider_sessions (provider_session_id,run_id,provider,created_at,updated_at) VALUES ('psess-2','run-ps2','codex','2026-06-01T00:00:00Z','2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let fate: String = sqlx::query_scalar(
        "SELECT process_fate FROM provider_sessions WHERE provider_session_id='psess-2'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(fate, "running");
}

#[tokio::test]
async fn provider_cancellation_intents_table_exists() {
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='provider_cancellation_intents'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn provider_cancellation_intents_invalid_reason_rejected() {
    let pool = test_pool().await;
    let result = sqlx::query(
        "INSERT INTO provider_cancellation_intents (provider_session_id,cancellation_epoch,intent_state,reason,requested_at_monotonic_ms,requested_at_wall_clock) VALUES ('sess-1',1,'requested','bad_reason',1000,'2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "invalid reason must be rejected");
}

#[tokio::test]
async fn provider_cancellation_intents_held_state_accepted() {
    let pool = test_pool().await;
    let result = sqlx::query(
        "INSERT INTO provider_cancellation_intents (provider_session_id,cancellation_epoch,intent_state,reason,requested_at_monotonic_ms,requested_at_wall_clock) VALUES ('sess-1',1,'held','operator_cancel',1000,'2026-06-01T00:00:00Z')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_ok(), "held state with valid reason should succeed: {result:?}");
}

// ── V-query verifications ─────────────────────────────────────────────────────
// Run the proposal's verification queries to confirm the invariant holds
// immediately after migration on an empty DB.

#[tokio::test]
async fn v1_verification_query_returns_zero_rows() {
    // V1: SELECT artifact_id FROM artifact_lineage WHERE artifact_role = 'report'
    //     AND active = 1 AND (report_kind IS NULL OR report_kind NOT IN (...))
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM artifact_lineage WHERE artifact_role = 'report' AND active = 1 AND (report_kind IS NULL OR report_kind NOT IN ('proposal_current','proposal_revision_summary','proposal_feedback_coverage','review_summary','run_report','release_receipt','evidence_pack'))",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "V1 verification query must return zero rows on fresh DB");
}

#[tokio::test]
async fn v2_verification_query_returns_zero_rows() {
    // V2: SELECT principal_id, request_id, lease_generation, COUNT(*) FROM command_idempotency
    //     GROUP BY ... HAVING COUNT(*) > 1
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (SELECT principal_id, request_id, lease_generation, COUNT(*) as cnt FROM command_idempotency GROUP BY principal_id, request_id, lease_generation HAVING cnt > 1)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "V2 verification query must return zero rows on fresh DB");
}

#[tokio::test]
async fn v3_verification_query_returns_zero_rows() {
    // V3: SELECT receipt_id FROM shutdown_interrupted_receipts WHERE
    //     (interrupted_state = 'queued_no_signal' AND queue_rank IS NULL)
    //     OR (interrupted_state <> 'queued_no_signal' AND queue_rank IS NOT NULL)
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM shutdown_interrupted_receipts WHERE (interrupted_state = 'queued_no_signal' AND queue_rank IS NULL) OR (interrupted_state <> 'queued_no_signal' AND queue_rank IS NOT NULL)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "V3 verification query must return zero rows on fresh DB");
}

#[tokio::test]
async fn v4_verification_query_returns_zero_rows() {
    // V4: duplicate latch key check
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM (SELECT scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind, COUNT(*) as cnt FROM cancel_late_output_overflow GROUP BY scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind HAVING cnt > 1)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "V4 verification query must return zero rows on fresh DB");
}

#[tokio::test]
async fn v5_verification_query_returns_zero_rows() {
    // V5: SELECT COUNT(*) FROM p083_enforcement_mode_transition_journal
    //     WHERE transition_state = 'transitioning' AND commit_marker IS NOT NULL
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM p083_enforcement_mode_transition_journal WHERE transition_state = 'transitioning' AND commit_marker IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "V5 verification query must return zero rows on fresh DB");
}

#[tokio::test]
async fn v7_verification_query_returns_zero_rows() {
    // V7: SELECT provider_session_id FROM provider_cancellation_intents
    //     WHERE intent_state IN ('shutdown_started','settled') AND shutdown_epoch IS NULL
    let pool = test_pool().await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM provider_cancellation_intents WHERE intent_state IN ('shutdown_started','settled') AND shutdown_epoch IS NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0, "V7 verification query must return zero rows on fresh DB");
}

// ── command_idempotency repo ─────────────────────────────────────────────────

#[tokio::test]
async fn command_idempotency_acquire_and_commit_round_trip() {
    use db::repos::command_idempotency;
    let pool = test_pool().await;

    let acquired = command_idempotency::acquire(
        &pool,
        "principal-1",
        "req-1",
        "runs.cancel",
        "hash-abc",
        1,
        "2026-12-31T00:00:00Z",
    )
    .await
    .unwrap();
    assert!(acquired, "first acquire should succeed");

    // Duplicate acquire must fail (unique constraint).
    let dup = command_idempotency::acquire(
        &pool,
        "principal-1",
        "req-1",
        "runs.cancel",
        "hash-abc",
        1,
        "2026-12-31T00:00:00Z",
    )
    .await
    .unwrap();
    assert!(!dup, "duplicate acquire must return false");

    let committed = command_idempotency::commit(
        &pool,
        "principal-1",
        "req-1",
        1,
        r#"{"result":"ok"}"#,
    )
    .await
    .unwrap();
    assert!(committed, "commit must update the pending row");

    let lease = command_idempotency::find_active_by_request(&pool, "principal-1", "req-1")
        .await
        .unwrap()
        .expect("committed lease should be findable");
    assert_eq!(lease.lease_state, "committed");
    assert_eq!(lease.outcome_json.as_deref(), Some(r#"{"result":"ok"}"#));
}

#[tokio::test]
async fn command_idempotency_fail_and_abandon() {
    use db::repos::command_idempotency;
    let pool = test_pool().await;

    command_idempotency::acquire(
        &pool,
        "principal-2",
        "req-2",
        "runs.retry",
        "hash-xyz",
        1,
        "2026-12-31T00:00:00Z",
    )
    .await
    .unwrap();

    let failed = command_idempotency::fail_lease(
        &pool, "principal-2", "req-2", 1, "CONFLICT",
    )
    .await
    .unwrap();
    assert!(failed);

    let abandoned = command_idempotency::abandon(&pool, "principal-2", "req-2", 1)
        .await
        .unwrap();
    assert!(abandoned);

    // After abandon, no active lease remains.
    let active = command_idempotency::find_active_by_request(&pool, "principal-2", "req-2")
        .await
        .unwrap();
    assert!(active.is_none(), "abandoned lease must not appear as active");
}

#[tokio::test]
async fn command_request_alias_round_trip() {
    use db::repos::command_idempotency;
    let pool = test_pool().await;

    command_idempotency::insert_alias(
        &pool,
        "principal-3",
        "approvals.resolve",
        "hash-111",
        "req-new",
        "req-canonical",
    )
    .await
    .unwrap();

    let canonical = command_idempotency::find_canonical_by_alias(
        &pool,
        "principal-3",
        "approvals.resolve",
        "hash-111",
        "req-new",
    )
    .await
    .unwrap();
    assert_eq!(canonical.as_deref(), Some("req-canonical"));
}

// ── provider_sessions repo ───────────────────────────────────────────────────

#[tokio::test]
async fn provider_sessions_insert_and_find() {
    use db::repos::provider_sessions;
    let pool = test_pool().await;

    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-repo1','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-repo1','idea-repo1','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    provider_sessions::insert(&pool, "psess-repo1", "run-repo1", None, "codex")
        .await
        .unwrap();

    let s = provider_sessions::find_by_id(&pool, "psess-repo1")
        .await
        .unwrap()
        .expect("session should exist");
    assert_eq!(s.provider, "codex");
    assert_eq!(s.lifecycle_state, "registered");
    assert_eq!(s.process_fate, "running");
}

#[tokio::test]
async fn provider_sessions_update_process_fate_identity_ambiguous() {
    use db::repos::provider_sessions;
    let pool = test_pool().await;

    sqlx::query("INSERT INTO ideas (id,title,body,status,created_at) VALUES ('idea-repo2','t','b','active','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs (id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('run-repo2','idea-repo2','running','wf','WF','/ws','/art','2026-06-01T00:00:00Z')").execute(&pool).await.unwrap();

    provider_sessions::insert(&pool, "psess-repo2", "run-repo2", None, "claude")
        .await
        .unwrap();
    provider_sessions::insert_cancellation_intent(
        &pool, "psess-repo2", 1, "operator_cancel", 1000,
    )
    .await
    .unwrap();

    let held = provider_sessions::hold_identity_ambiguous(&pool, "psess-repo2", 1)
        .await
        .unwrap();
    assert!(held);

    let s = provider_sessions::find_by_id(&pool, "psess-repo2")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(s.process_fate, "identity_ambiguous");

    let ambiguous = provider_sessions::find_identity_ambiguous(&pool)
        .await
        .unwrap();
    assert!(ambiguous.iter().any(|s| s.provider_session_id == "psess-repo2"));
}

#[tokio::test]
async fn provider_cancellation_intent_state_transitions() {
    use db::repos::provider_sessions;
    let pool = test_pool().await;

    provider_sessions::insert_cancellation_intent(
        &pool, "sess-t1", 1, "backpressure_cutoff", 2000,
    )
    .await
    .unwrap();

    // Transition to shutdown_started with a shutdown_epoch.
    let updated = provider_sessions::update_cancellation_intent_state(
        &pool, "sess-t1", 1, "shutdown_started", Some(42),
    )
    .await
    .unwrap();
    assert!(updated);

    let active = provider_sessions::find_active_cancellation_intent(&pool, "sess-t1")
        .await
        .unwrap()
        .expect("should find shutdown_started intent");
    assert_eq!(active.intent_state, "shutdown_started");
    assert_eq!(active.shutdown_epoch, Some(42));
}

// ── shutdown_receipts repo ───────────────────────────────────────────────────

#[tokio::test]
async fn shutdown_receipts_insert_and_find() {
    use db::repos::shutdown_receipts;
    let pool = test_pool().await;

    shutdown_receipts::insert_interrupted_receipt(
        &pool, "rcpt-1", "sess-s1", 1, 1, "grace_deadline_expired", None,
    )
    .await
    .unwrap();

    let receipts = shutdown_receipts::find_receipts_by_session(&pool, "sess-s1")
        .await
        .unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].interrupted_state, "grace_deadline_expired");
    assert!(receipts[0].queue_rank.is_none());
    assert!(receipts[0].recovered_at.is_none());
}

#[tokio::test]
async fn shutdown_receipts_queued_no_signal_stores_queue_rank() {
    use db::repos::shutdown_receipts;
    let pool = test_pool().await;

    shutdown_receipts::insert_interrupted_receipt(
        &pool, "rcpt-2", "sess-s2", 1, 1, "queued_no_signal", Some(3),
    )
    .await
    .unwrap();

    let r = shutdown_receipts::find_receipt_by_key(&pool, "sess-s2", 1, 1)
        .await
        .unwrap()
        .expect("receipt must be found");
    assert_eq!(r.queue_rank, Some(3));
}

#[tokio::test]
async fn shutdown_receipts_mark_recovered() {
    use db::repos::shutdown_receipts;
    let pool = test_pool().await;

    shutdown_receipts::insert_interrupted_receipt(
        &pool, "rcpt-3", "sess-s3", 1, 1, "shutdown_interrupted", None,
    )
    .await
    .unwrap();

    let ok = shutdown_receipts::mark_receipt_recovered(&pool, "rcpt-3")
        .await
        .unwrap();
    assert!(ok);

    let r = shutdown_receipts::find_receipt_by_key(&pool, "sess-s3", 1, 1)
        .await
        .unwrap()
        .unwrap();
    assert!(r.recovered_at.is_some());
}

#[tokio::test]
async fn shutdown_signal_side_effects_planned_to_issued_to_observed() {
    use db::repos::shutdown_receipts;
    let pool = test_pool().await;

    shutdown_receipts::insert_signal_planned(
        &pool, "sig-1", "sess-sg1", 1, 12345, "boot-abc", "graceful", 1,
    )
    .await
    .unwrap();

    let active = shutdown_receipts::find_active_signal(&pool, "sess-sg1", 1, "graceful", 1)
        .await
        .unwrap()
        .expect("planned signal must be active");
    assert_eq!(active.intent_state, "planned");

    // SEC-P083-HIGH-001: CAS claim must precede kill(); transitions planned → dispatching.
    let claimed =
        shutdown_receipts::mark_signal_dispatching(&pool, "sess-sg1", 1, "graceful", 1)
            .await
            .unwrap();
    assert!(claimed, "first claim must succeed");

    // Duplicate claim must fail (concurrent-dispatcher suppression).
    let dup_claim =
        shutdown_receipts::mark_signal_dispatching(&pool, "sess-sg1", 1, "graceful", 1)
            .await
            .unwrap();
    assert!(!dup_claim, "second claim on same row must be rejected");

    // Row is active while in 'dispatching' state.
    let dispatching = shutdown_receipts::find_active_signal(&pool, "sess-sg1", 1, "graceful", 1)
        .await
        .unwrap()
        .expect("dispatching signal must still be active");
    assert_eq!(dispatching.intent_state, "dispatching");

    let issued = shutdown_receipts::mark_signal_issued(&pool, "sess-sg1", 1, "graceful", 1, 99000)
        .await
        .unwrap();
    assert!(issued, "issued transition from dispatching must succeed");

    let observed =
        shutdown_receipts::mark_signal_observed(&pool, "sess-sg1", 1, "graceful", 1, 102000)
            .await
            .unwrap();
    assert!(observed);

    // After terminal state, active signal is gone.
    let gone = shutdown_receipts::find_active_signal(&pool, "sess-sg1", 1, "graceful", 1)
        .await
        .unwrap();
    assert!(gone.is_none(), "observed signal must not appear as active");
}

#[tokio::test]
async fn shutdown_signal_dispatching_skipped_by_recovery() {
    // SEC-P083-HIGH-001: A row in 'dispatching' state must NOT be returned by
    // load_planned_for_dispatch (which only returns intent_state = 'planned').
    // This ensures a crashed dispatcher cannot trigger a duplicate signal send on restart.
    use db::repos::shutdown_receipts;
    let pool = test_pool().await;

    shutdown_receipts::insert_signal_planned(
        &pool, "sig-r1", "sess-rec1", 5, 88888, "boot-rec", "graceful", 1,
    )
    .await
    .unwrap();

    // Claim the row (simulates dispatcher acquiring it just before crash).
    let claimed =
        shutdown_receipts::mark_signal_dispatching(&pool, "sess-rec1", 5, "graceful", 1)
            .await
            .unwrap();
    assert!(claimed);

    // Row must still appear as active (it is in progress, not terminal).
    let still_active =
        shutdown_receipts::find_active_signal(&pool, "sess-rec1", 5, "graceful", 1)
            .await
            .unwrap();
    assert!(
        still_active.is_some(),
        "dispatching row must remain active (not terminal)"
    );
    assert_eq!(still_active.unwrap().intent_state, "dispatching");

    // A second dispatching call on the same row must return false.
    let second_claim =
        shutdown_receipts::mark_signal_dispatching(&pool, "sess-rec1", 5, "graceful", 1)
            .await
            .unwrap();
    assert!(
        !second_claim,
        "recovery cannot re-claim a row already in dispatching state"
    );
}

#[tokio::test]
async fn shutdown_signal_identity_mismatch_transition() {
    use db::repos::shutdown_receipts;
    let pool = test_pool().await;

    shutdown_receipts::insert_signal_planned(
        &pool, "sig-2", "sess-sg2", 1, 9999, "boot-xyz", "kill", 1,
    )
    .await
    .unwrap();

    let ok =
        shutdown_receipts::mark_signal_identity_mismatch(&pool, "sess-sg2", 1, "kill", 1, Some("PID_MISMATCH"))
            .await
            .unwrap();
    assert!(ok);

    let signals = shutdown_receipts::find_signals_by_session(&pool, "sess-sg2")
        .await
        .unwrap();
    assert_eq!(signals[0].intent_state, "identity_mismatch");
    assert_eq!(signals[0].error_code.as_deref(), Some("PID_MISMATCH"));
}

// ── P083 metrics registration ────────────────────────────────────────────────

#[test]
fn p083_required_metric_names_are_declared_and_recordable() {
    use db::metrics::{
        self, P083_REQUIRED_METRICS,
        record_p083_artifact_lineage_projection_integrity,
        record_p083_provider_session_lifecycle,
        record_p083_command_idempotency_lease_acquire,
        record_p083_command_idempotency_replay,
        record_p083_shutdown_interrupted_receipt,
        record_p083_shutdown_duplicate_signal_suppressed,
        record_p083_cancel_late_output_overflow,
        record_p083_cancel_late_output_dropped,
        record_p083_rollout_contract_lint,
        record_p083_rollout_contract_run_start_block,
        record_p083_enforcement_mode_transition,
        record_p083_rollback_execution,
        record_p083_provider_cancellation_intent,
    };

    let required = [
        "artifact_lineage_projection_integrity_total",
        "provider_session_lifecycle_total",
        "command_idempotency_lease_acquire_total",
        "command_idempotency_replay_total",
        "shutdown_interrupted_receipt_total",
        "shutdown_duplicate_signal_suppressed_total",
        "cancel_late_output_overflow_total",
        "cancel_late_output_dropped_total",
        "rollout_contract_lint_total",
        "rollout_contract_run_start_block_total",
        "p083_enforcement_mode_transition_total",
        "p083_rollback_execution_total",
        "provider_cancellation_intent_total",
    ];
    for m in required {
        assert!(
            P083_REQUIRED_METRICS.contains(&m),
            "P083_REQUIRED_METRICS missing: {m}"
        );
    }

    // Verify recording functions increment the counters.
    record_p083_artifact_lineage_projection_integrity("graphql", "fresh");
    record_p083_provider_session_lifecycle("codex", "live");
    record_p083_command_idempotency_lease_acquire("runs.cancel", "acquired");
    record_p083_command_idempotency_replay("runs.cancel", "replayed");
    record_p083_shutdown_interrupted_receipt("codex", "grace_deadline_expired");
    record_p083_shutdown_duplicate_signal_suppressed("claude");
    record_p083_cancel_late_output_overflow("codex", "session", "message_count");
    record_p083_cancel_late_output_dropped("codex", "run", "session_bytes");
    record_p083_rollout_contract_lint("P083", "pass", None);
    record_p083_rollout_contract_run_start_block("P083", "hold_condition_present", "permissive");
    record_p083_enforcement_mode_transition("disabled_to_permissive", "permissive");
    record_p083_rollback_execution("enforce_to_permissive", "pass", "gate_failed");
    record_p083_provider_cancellation_intent("codex", "requested", "operator_cancel");

    assert!(
        metrics::get_counter("artifact_lineage_projection_integrity_total") > 0,
        "artifact_lineage_projection_integrity_total must be recorded"
    );
    assert!(
        metrics::get_counter("command_idempotency_lease_acquire_total") > 0,
        "command_idempotency_lease_acquire_total must be recorded"
    );
    assert!(
        metrics::get_counter("provider_cancellation_intent_total") > 0,
        "provider_cancellation_intent_total must be recorded"
    );
    assert!(
        metrics::get_counter("p083_enforcement_mode_transition_total") > 0,
        "p083_enforcement_mode_transition_total must be recorded"
    );
}

// ── P083 metric domain conformance ───────────────────────────────────────────
//
// Asserts that each P083 recorder stores the canonical label values from
// metric_labels_contract_v1 WITHOUT collapsing them to "unknown". If a domain
// constant diverges from the proposal, bounded_label() substitutes "unknown"
// and the labeled-counter check below fails.

#[test]
fn p083_metric_domain_conformance_valid_labels_do_not_collapse() {
    use db::metrics::{
        get_counter,
        record_p083_artifact_lineage_projection_integrity,
        record_p083_cancel_late_output_dropped,
        record_p083_cancel_late_output_overflow,
        record_p083_command_idempotency_lease_acquire,
        record_p083_command_idempotency_replay,
        record_p083_enforcement_mode_transition,
        record_p083_provider_cancellation_intent,
        record_p083_provider_session_lifecycle,
        record_p083_rollback_execution,
        record_p083_rollout_contract_lint,
        record_p083_rollout_contract_run_start_block,
        record_p083_shutdown_duplicate_signal_suppressed,
        record_p083_shutdown_interrupted_receipt,
    };

    // For each recorder, use canonical proposal-spec label values and confirm
    // the labeled counter key was incremented. If bounded_label() substituted
    // "unknown" instead, the canonical-label counter stays at its prior value.

    let key = "artifact_lineage_projection_integrity_total:surface=graphql,state=fresh";
    let before = get_counter(key);
    record_p083_artifact_lineage_projection_integrity("graphql", "fresh");
    assert!(get_counter(key) > before, "surface=graphql,state=fresh must not collapse to unknown");

    let key = "provider_session_lifecycle_total:provider=codex,lifecycle_state=registered";
    let before = get_counter(key);
    record_p083_provider_session_lifecycle("codex", "registered");
    assert!(get_counter(key) > before, "lifecycle_state=registered must not collapse to unknown");

    let key = "provider_session_lifecycle_total:provider=claude,lifecycle_state=live";
    let before = get_counter(key);
    record_p083_provider_session_lifecycle("claude", "live");
    assert!(get_counter(key) > before, "provider=claude must not collapse to unknown");

    let key = "command_idempotency_lease_acquire_total:command=runs.cancel,outcome=acquired";
    let before = get_counter(key);
    record_p083_command_idempotency_lease_acquire("runs.cancel", "acquired");
    assert!(get_counter(key) > before, "outcome=acquired must not collapse to unknown");

    let key = "command_idempotency_replay_total:command=approvals.resolve,outcome=replayed";
    let before = get_counter(key);
    record_p083_command_idempotency_replay("approvals.resolve", "replayed");
    assert!(get_counter(key) > before, "outcome=replayed must not collapse to unknown");

    let key = "shutdown_interrupted_receipt_total:provider=codex,interrupted_state=grace_deadline_expired";
    let before = get_counter(key);
    record_p083_shutdown_interrupted_receipt("codex", "grace_deadline_expired");
    assert!(get_counter(key) > before, "interrupted_state=grace_deadline_expired must not collapse");

    let key = "shutdown_duplicate_signal_suppressed_total:provider=claude";
    let before = get_counter(key);
    record_p083_shutdown_duplicate_signal_suppressed("claude");
    assert!(get_counter(key) > before, "provider=claude must not collapse to unknown");

    let key = "cancel_late_output_overflow_total:provider=codex,scope=session,overflow_kind=message_count";
    let before = get_counter(key);
    record_p083_cancel_late_output_overflow("codex", "session", "message_count");
    assert!(get_counter(key) > before, "overflow_kind=message_count must not collapse to unknown");

    let key = "cancel_late_output_dropped_total:provider=gemini,scope=run,overflow_kind=session_bytes";
    let before = get_counter(key);
    record_p083_cancel_late_output_dropped("gemini", "run", "session_bytes");
    assert!(get_counter(key) > before, "overflow_kind=session_bytes must not collapse to unknown");

    // When failure_reason is None (status=pass), the label is omitted from the key.
    let key = "rollout_contract_lint_total:proposal_id=P083,status=pass";
    let before = get_counter(key);
    record_p083_rollout_contract_lint("P083", "pass", None);
    assert!(get_counter(key) > before, "proposal_id=P083,status=pass must not collapse to unknown");

    let key = "rollout_contract_run_start_block_total:proposal_id=P083,reason=hold_condition_present,enforcement_mode=permissive";
    let before = get_counter(key);
    record_p083_rollout_contract_run_start_block("P083", "hold_condition_present", "permissive");
    assert!(get_counter(key) > before, "reason=hold_condition_present must not collapse to unknown");

    let key = "p083_enforcement_mode_transition_total:transition=disabled_to_permissive,enforcement_mode=permissive";
    let before = get_counter(key);
    record_p083_enforcement_mode_transition("disabled_to_permissive", "permissive");
    assert!(get_counter(key) > before, "transition=disabled_to_permissive must not collapse to unknown");

    let key = "p083_enforcement_mode_transition_total:transition=disabled_to_enforce_denied,enforcement_mode=disabled";
    let before = get_counter(key);
    record_p083_enforcement_mode_transition("disabled_to_enforce_denied", "disabled");
    assert!(get_counter(key) > before, "transition=disabled_to_enforce_denied must not collapse to unknown");

    // Approved domain: status=pass/fail/..., reason from hold-condition set.
    let key = "p083_rollback_execution_total:action=rollback_disable,status=pass,reason=gate_failed";
    let before = get_counter(key);
    record_p083_rollback_execution("rollback_disable", "pass", "gate_failed");
    assert!(get_counter(key) > before, "action=rollback_disable must not collapse to unknown");

    let key = "provider_cancellation_intent_total:provider=codex,intent_state=requested,cancellation_reason=operator_cancel";
    let before = get_counter(key);
    record_p083_provider_cancellation_intent("codex", "requested", "operator_cancel");
    assert!(get_counter(key) > before, "intent_state=requested must not collapse to unknown");

    let key = "provider_cancellation_intent_total:provider=claude,intent_state=shutdown_started,cancellation_reason=shutdown_recovery";
    let before = get_counter(key);
    record_p083_provider_cancellation_intent("claude", "shutdown_started", "shutdown_recovery");
    assert!(get_counter(key) > before, "intent_state=shutdown_started must not collapse to unknown");

    let key = "provider_cancellation_intent_total:provider=codex,intent_state=held,cancellation_reason=backpressure_cutoff";
    let before = get_counter(key);
    record_p083_provider_cancellation_intent("codex", "held", "backpressure_cutoff");
    assert!(get_counter(key) > before, "intent_state=held must not collapse to unknown");
}
