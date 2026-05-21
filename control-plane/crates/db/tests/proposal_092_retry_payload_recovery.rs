use chrono::Utc;
use db::pool::create_pool;
use db::repos::{ideas, retry_payload_recovery_events, runs};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId, StageExecutionId};
use domain::retry_authority::RetryPayloadRecoveryEvent;
use domain::run::{Run, RunStatus};

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

async fn seed_run(pool: &sqlx::SqlitePool) -> RunId {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P092".into(),
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
            workflow_id: "wf-p092".into(),
            workflow_title: "P092".into(),
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
    run_id
}

fn recovery_event(
    run_id: RunId,
    target_stage_execution_id: StageExecutionId,
) -> RetryPayloadRecoveryEvent {
    let now = Utc::now();
    RetryPayloadRecoveryEvent {
        idempotency_key: "p092:run:invoke:auth:agent".into(),
        run_id,
        invoke_work_item_id: "invoke-p092".into(),
        retry_authority_id: Some("auth-p092".into()),
        target_stage_execution_id: Some(target_stage_execution_id),
        completed_agent_execution_id: Some("agent-p092".into()),
        reason_code: "valid_retry_invoke_completion_recovered".into(),
        mode: "diagnostic".into(),
        repaired: false,
        current_json: serde_json::json!({
            "run_id": run_id.to_string(),
            "target_stage_execution_id": target_stage_execution_id.to_string(),
            "retry_authority_id": "auth-p092",
            "completed_agent_execution_id": "agent-p092",
            "invoke_work_item_id": "invoke-p092"
        }),
        provenance_json: Some(serde_json::json!({
            "source_stage_execution_id": "old-stage",
            "source_agent_execution_id": "old-agent"
        })),
        repaired_fields_json: Some(serde_json::json!(["target_stage_execution_id"])),
        diagnostic_json: Some(serde_json::json!({"would_repair": true})),
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn p092_recovery_event_upsert_promotes_diagnostic_to_enforced_without_duplicate() {
    let pool = setup_db().await;
    let run_id = seed_run(&pool).await;
    let target_stage_execution_id = StageExecutionId::new();
    let mut event = recovery_event(run_id, target_stage_execution_id);

    retry_payload_recovery_events::upsert(&pool, &event)
        .await
        .unwrap();
    event.mode = "enforce".into();
    event.repaired = true;
    event.repaired_fields_json = Some(serde_json::json!([
        "target_stage_execution_id",
        "source_agent_execution_id"
    ]));
    event.updated_at = Utc::now();
    retry_payload_recovery_events::upsert(&pool, &event)
        .await
        .unwrap();

    let events = retry_payload_recovery_events::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    let readback = events[0].readback_json();
    assert_eq!(readback["mode"], serde_json::json!("enforce"));
    assert_eq!(readback["repaired"], serde_json::json!(true));
    assert_eq!(
        readback["current"]["target_stage_execution_id"],
        serde_json::json!(target_stage_execution_id.to_string())
    );
    assert_eq!(readback["unknown_reason_code"], serde_json::json!(false));
}

#[tokio::test]
async fn p092_recovery_event_unknown_reason_round_trips() {
    let pool = setup_db().await;
    let run_id = seed_run(&pool).await;
    let target_stage_execution_id = StageExecutionId::new();
    let mut event = recovery_event(run_id, target_stage_execution_id);
    event.idempotency_key = "p092:unknown".into();
    event.reason_code = "future_reason".into();

    retry_payload_recovery_events::upsert(&pool, &event)
        .await
        .unwrap();
    let events = retry_payload_recovery_events::list_by_run(&pool, run_id)
        .await
        .unwrap();

    let readback = events[0].readback_json();
    assert_eq!(readback["reason_code"], serde_json::json!("future_reason"));
    assert_eq!(readback["unknown_reason_code"], serde_json::json!(true));
}
