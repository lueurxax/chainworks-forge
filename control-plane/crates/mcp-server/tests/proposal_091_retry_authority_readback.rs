use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    ideas, retry_payload_recovery_events, retry_stage_execution_authorities, runs, stages,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId, StageExecutionId};
use domain::retry_authority::{
    RetryAuthorityEntryKind, RetryAuthorityState, RetryPayloadRecoveryEvent,
    RetryStageExecutionAuthority,
};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::work_queue::WorkQueue;
use mcp_server::tools::{reports, runs as mcp_runs};

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    pool
}

fn command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
    let events = event_bus::new_bus(64);
    CommandHandler::new(pool.clone(), events, WorkQueue::new(pool))
}

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Ready,
        workflow_id: "wf-p091".into(),
        workflow_title: "P091".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("implement".into()),
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
    }
}

async fn seed_retry_authority(pool: &sqlx::SqlitePool) -> (RunId, String) {
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P091 idea".into(),
            body: "Retry authority readback".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(pool, &make_run(run_id, idea_id))
        .await
        .unwrap();

    let now = Utc::now();
    let target_stage_execution_id = StageExecutionId::new();
    stages::insert(
        pool,
        &StageExecution {
            id: target_stage_execution_id,
            run_id,
            stage_id: "implement".into(),
            label: "Implement".into(),
            status: StageStatus::Running,
            iteration: 0,
            attempt_number: 2,
            settlement_kind: None,
            started_at: now,
            completed_at: None,
            owner_agent: Some("code_writer".into()),
            provider: Some("junie".into()),
            model: None,
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: Some("operator retry".into()),
        },
    )
    .await
    .unwrap();
    retry_stage_execution_authorities::create_active(
        pool,
        &RetryStageExecutionAuthority {
            id: "p091-active-authority".into(),
            run_id,
            stage_id: "implement".into(),
            target_stage_execution_id,
            entry_kind: RetryAuthorityEntryKind::FullStageRetry,
            source_command_journal_id: Some("journal-1".into()),
            source_retry_work_item_id: Some("retry-work-1".into()),
            source_invoke_work_item_id: Some("invoke-work-1".into()),
            source_agent_execution_id: Some("agent-exec-1".into()),
            authority_state: RetryAuthorityState::Active,
            created_at: now,
            updated_at: now,
            terminal_reason: None,
        },
    )
    .await
    .unwrap();
    retry_payload_recovery_events::upsert(
        pool,
        &RetryPayloadRecoveryEvent {
            idempotency_key: "p092:mcp-readback".into(),
            run_id,
            invoke_work_item_id: "invoke-work-1".into(),
            retry_authority_id: Some("p091-active-authority".into()),
            target_stage_execution_id: Some(target_stage_execution_id),
            completed_agent_execution_id: Some("agent-exec-1".into()),
            reason_code: "valid_retry_invoke_completion_recovered".into(),
            mode: "diagnostic".into(),
            repaired: false,
            current_json: serde_json::json!({
                "run_id": run_id.to_string(),
                "target_stage_execution_id": target_stage_execution_id.to_string(),
                "retry_authority_id": "p091-active-authority",
                "completed_agent_execution_id": "agent-exec-1",
                "invoke_work_item_id": "invoke-work-1"
            }),
            provenance_json: Some(serde_json::json!({
                "source_agent_execution_id": "old-agent"
            })),
            repaired_fields_json: Some(serde_json::json!(["target_stage_execution_id"])),
            diagnostic_json: Some(serde_json::json!({"would_repair": true})),
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();
    retry_payload_recovery_events::upsert(
        pool,
        &RetryPayloadRecoveryEvent {
            idempotency_key: "p092:mcp-missing-authority".into(),
            run_id,
            invoke_work_item_id: "invoke-missing-authority".into(),
            retry_authority_id: None,
            target_stage_execution_id: Some(target_stage_execution_id),
            completed_agent_execution_id: None,
            reason_code: "retry_authority_missing_for_targeted_invoke".into(),
            mode: "enforce".into(),
            repaired: false,
            current_json: serde_json::json!({
                "run_id": run_id.to_string(),
                "target_stage_execution_id": target_stage_execution_id.to_string(),
                "retry_authority_id": null,
                "invoke_work_item_id": "invoke-missing-authority"
            }),
            provenance_json: Some(serde_json::json!({
                "payload_retry_authority_id": "stale-auth"
            })),
            repaired_fields_json: Some(serde_json::json!([])),
            diagnostic_json: Some(serde_json::json!({"fail_closed": true})),
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    sqlx::query(
        r#"INSERT INTO p091_orphan_repair_passes
           (id, mode, disabled, run_id, candidates_total, excluded_total,
            would_repair_total, repaired_total, disabled_total,
            bounded_samples_json, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"#,
    )
    .bind("p091-readback-pass")
    .bind("diagnostic")
    .bind(0_i64)
    .bind(run_id.to_string())
    .bind(2_i64)
    .bind(1_i64)
    .bind(1_i64)
    .bind(0_i64)
    .bind(0_i64)
    .bind(
        r#"[{"stage_execution_id":"sample","reason":"settled_sibling_without_live_retry_driver"}]"#,
    )
    .bind(now.to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    (run_id, "p091-active-authority".into())
}

#[tokio::test]
async fn retry_authority_history_and_current_readback_include_active_authority() {
    let pool = test_pool().await;
    let (run_id, authority_id) = seed_retry_authority(&pool).await;
    let handler = command_handler(pool.clone());
    let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

    let report_payload = reports::execute(
        "reports.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .unwrap();
    let reports = report_payload["reports"].as_array().expect("reports array");
    let mcp_truth = reports
        .iter()
        .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
        .expect("mcp execution truth report");
    assert_eq!(mcp_truth["retryAuthority"]["id"], authority_id);
    assert_eq!(mcp_truth["retryAuthorityHistory"][0]["id"], authority_id);
    assert_eq!(
        mcp_truth["retryAuthority"]["retryPayloadRecovery"]["reason_code"],
        "valid_retry_invoke_completion_recovered"
    );
    let report_missing = mcp_truth["retryAuthorityHistory"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["authority_state"] == serde_json::json!("missing_authority"))
        .expect("report missing-authority history row");
    assert_eq!(
        report_missing["retryPayloadRecovery"]["reason_code"],
        "retry_authority_missing_for_targeted_invoke"
    );
    assert_eq!(
        mcp_truth["p091OrphanRepairReadback"]["latest_pass"]["candidates_total"],
        2
    );

    let run_payload = mcp_runs::execute(
        "runs.get",
        serde_json::json!({ "run_id": run_id.to_string() }),
        &pool,
        &handler,
        &principal,
    )
    .await
    .unwrap();
    assert_eq!(run_payload["retry_authority"]["id"], authority_id);
    assert_eq!(
        run_payload["retry_authority_history"][0]["id"],
        authority_id
    );
    assert_eq!(
        run_payload["retry_authority"]["retry_payload_recovery"]["current"]["invoke_work_item_id"],
        "invoke-work-1"
    );
    let run_missing = run_payload["retry_authority_history"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["authority_state"] == serde_json::json!("missing_authority"))
        .expect("runs.get missing-authority history row");
    assert_eq!(
        run_missing["retry_payload_recovery"]["current"]["retry_authority_id"],
        serde_json::Value::Null
    );
    assert_eq!(
        run_payload["p091_orphan_repair_readback"]["latest_pass"]["would_repair_total"],
        1
    );
}
