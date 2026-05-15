use chrono::Utc;
use db::pool::create_pool;
use db::repos::{ideas, projections, retry_stage_execution_authorities, runs, stages};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId, StageExecutionId};
use domain::retry_authority::{
    RetryAuthorityEntryKind, RetryAuthorityState, RetryStageExecutionAuthority,
};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};

async fn setup_db() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .expect("test shared DbWriter registration failed");
    pool
}

async fn seed_run_and_stage(pool: &sqlx::SqlitePool) -> (RunId, StageExecutionId) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P091".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(
        pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-p091".into(),
            workflow_title: "P091".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: None,
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
        },
    )
    .await
    .unwrap();
    stages::insert(
        pool,
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 2,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: None,
            owner_agent: Some("code_writer".into()),
            provider: Some("junie".into()),
            model: Some("junie".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: Some("operator_retry".into()),
        },
    )
    .await
    .unwrap();
    (run_id, stage_execution_id)
}

fn authority(
    id: &str,
    run_id: RunId,
    target_stage_execution_id: StageExecutionId,
) -> RetryStageExecutionAuthority {
    let now = Utc::now();
    RetryStageExecutionAuthority {
        id: id.into(),
        run_id,
        stage_id: "implementation".into(),
        target_stage_execution_id,
        entry_kind: RetryAuthorityEntryKind::FullStageRetry,
        source_command_journal_id: Some("cmd-1".into()),
        source_retry_work_item_id: Some("work-1".into()),
        source_invoke_work_item_id: None,
        source_agent_execution_id: None,
        authority_state: RetryAuthorityState::Active,
        created_at: now,
        updated_at: now,
        terminal_reason: None,
    }
}

#[tokio::test]
async fn p091_active_retry_authority_is_schema_unique_per_run_stage() {
    let pool = setup_db().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;

    retry_stage_execution_authorities::create_active(
        &pool,
        &authority("auth-1", run_id, stage_execution_id),
    )
    .await
    .unwrap();

    let duplicate = retry_stage_execution_authorities::create_active(
        &pool,
        &authority("auth-2", run_id, stage_execution_id),
    )
    .await;
    assert!(duplicate.is_err(), "duplicate active authority must fail");

    let mut tx = db::writer::begin_repository_transaction(&pool, "p091.test")
        .await
        .unwrap();
    retry_stage_execution_authorities::supersede_active_for_stage_tx(
        &mut tx,
        run_id,
        "implementation",
        Utc::now(),
        "new_retry_superseded_previous",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    retry_stage_execution_authorities::create_active(
        &pool,
        &authority("auth-3", run_id, stage_execution_id),
    )
    .await
    .unwrap();

    let authorities = retry_stage_execution_authorities::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(authorities.len(), 2);
    assert!(authorities
        .iter()
        .any(|a| { a.id == "auth-1" && a.authority_state == RetryAuthorityState::Superseded }));
    assert!(authorities
        .iter()
        .any(|a| a.id == "auth-3" && a.authority_state == RetryAuthorityState::Active));
}

#[tokio::test]
async fn p091_recovered_orphan_authority_is_terminal_history_not_active() {
    let pool = setup_db().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;

    let mut tx = db::writer::begin_repository_transaction(&pool, "p091.test")
        .await
        .unwrap();
    retry_stage_execution_authorities::create_recovered_orphan_tx(
        &mut tx,
        "recovered-1",
        run_id,
        "implementation",
        stage_execution_id,
        "stale_retry_recovered",
        Utc::now(),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert!(
        retry_stage_execution_authorities::find_active_by_run_stage(
            &pool,
            run_id,
            "implementation"
        )
        .await
        .unwrap()
        .is_none(),
        "recovered orphan rows must not become active retry authority"
    );
    let recovered = retry_stage_execution_authorities::find_by_id(&pool, "recovered-1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        recovered.authority_state,
        RetryAuthorityState::RecoveredOrphan
    );
    assert_eq!(
        recovered.terminal_reason.as_deref(),
        Some("stale_retry_recovered")
    );
}

#[tokio::test]
async fn p091_terminal_reason_and_active_authority_project_to_stage_summary() {
    let pool = setup_db().await;
    let (run_id, stage_execution_id) = seed_run_and_stage(&pool).await;
    retry_stage_execution_authorities::create_active(
        &pool,
        &authority("auth-active", run_id, stage_execution_id),
    )
    .await
    .unwrap();

    let mut tx = db::writer::begin_repository_transaction(&pool, "p091.test")
        .await
        .unwrap();
    stages::settle_with_terminal_reason_tx(
        &mut tx,
        stage_execution_id,
        StageSettlementKind::Skipped,
        Utc::now(),
        "stale_retry_recovered",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    projections::rebuild_stage_summaries(&pool, run_id)
        .await
        .unwrap();
    let summaries = projections::list_stages_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    let summary = summaries
        .iter()
        .find(|row| row.id == stage_execution_id.to_string())
        .unwrap();

    assert_eq!(
        summary.terminal_reason.as_deref(),
        Some("stale_retry_recovered")
    );
    assert_eq!(summary.retry_authority_id.as_deref(), Some("auth-active"));
    assert!(summary.is_retry_authoritative);
    assert_eq!(summary.retry_authority_state.as_deref(), Some("active"));
}
