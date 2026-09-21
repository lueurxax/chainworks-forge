use db::repos::run_continuations::{reserve, Reservation};
use domain::{
    commands::{CallerContext, CancelRunCmd, Command, RetryRunCmd},
    ids::RunId,
};
use engine::{
    command_handler::CommandHandler, event_bus, orchestrator::Orchestrator, work_queue::WorkQueue,
};
use sqlx::SqlitePool;

async fn source() -> (SqlitePool, RunId) {
    let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    let idea = uuid::Uuid::new_v4().to_string();
    let run = RunId::new();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Source','Body','2026-09-20T00:00:00Z')").bind(&idea).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','workflow','Workflow','/unused-p039-fixture','/unused-p039-fixture/meta','2026-09-20T00:00:00Z')")
        .bind(run.to_string()).bind(idea).execute(&pool).await.unwrap();
    reserve(
        &pool,
        &Reservation {
            source_run_id: run.to_string(),
            caller_fingerprint: "operator".into(),
            caller_request_id: uuid::Uuid::new_v4().to_string(),
            intent_sha256: "a".repeat(64),
            plan_ref: "plan.json".into(),
            plan_sha256: "b".repeat(64),
            target_ref: "target.json".into(),
            source_witness_sha256: "c".repeat(64),
            reason: "fixture".into(),
        },
    )
    .await
    .unwrap();
    (pool, run)
}

#[tokio::test]
async fn source_commands_stop_before_journal_or_retry_idempotency_writes() {
    let (pool, run) = source().await;
    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    for command in [
        Command::CancelRun(CancelRunCmd {
            run_id: run,
            request_id: Some(uuid::Uuid::new_v4().to_string()),
        }),
        Command::RetryRun(RetryRunCmd {
            run_id: run,
            request_id: uuid::Uuid::new_v4().to_string(),
        }),
    ] {
        let result = handler
            .handle(
                command,
                CallerContext::mcp("operator", &domain::PrincipalClass::Operator, "runs.cancel"),
            )
            .await;
        assert!(result
            .err()
            .expect("source mutation must be denied")
            .to_string()
            .contains("continuation_in_progress"));
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM command_journal")
                .fetch_one(&pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM command_idempotency")
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
}

#[tokio::test]
async fn orchestrator_rejects_fenced_source_before_workflow_or_flat_execution() {
    let (pool, run) = source().await;
    let orchestrator = Orchestrator::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    assert!(orchestrator
        .advance_run(run)
        .await
        .unwrap_err()
        .to_string()
        .contains("continuation_in_progress"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM stage_executions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn every_run_scoped_recovery_command_stops_before_business_admission() {
    use serde_json::json;
    let (pool, run) = source().await;
    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let key = uuid::Uuid::now_v7().to_string();
    for (name, mut fields) in [
        ("ApproveStage", json!({"stage_id":"gate"})),
        ("RejectStage", json!({"stage_id":"gate"})),
        ("RetryStage", json!({"stage_id":"implementation"})),
        (
            "ConsumeProviderQuotaHold",
            json!({"stage_id":"implementation","reason":"operator request"}),
        ),
        (
            "ResumeEscalationDeadline",
            json!({"escalation_ledger_id":"ledger","reason":"operator request","idempotency_key":key}),
        ),
        (
            "ResumeEscalationChain",
            json!({"escalation_ledger_id":"ledger","target_tier_id":"tier","reason":"operator request","idempotency_key":key}),
        ),
        (
            "ResolveWorkflowConflictTransition",
            json!({"conflict_id":"conflict","selected_transition_id":"transition","resolution_reason":"operator request"}),
        ),
        (
            "RetrofitCatalogSnapshot",
            json!({"expected_catalog_snapshot_hash":"old","reason":"operator request"}),
        ),
        (
            "MainSyncRequest",
            json!({"trigger_reason":"operator_request","idempotency_key":key}),
        ),
        ("MainSyncRetry", json!({"idempotency_key":key})),
        (
            "MainSyncSetRunOverride",
            json!({"mode":"automatic","reason":"operator request"}),
        ),
        (
            "MainSyncRepairState",
            json!({"recovery_note":"operator request"}),
        ),
        (
            "MainSyncRecordRecoveryDecision",
            json!({"decision":"retry_sync","summary":"operator request"}),
        ),
        (
            "ResolveApproval",
            json!({"approval_id":uuid::Uuid::new_v4(),"decision":"approved","stage_id":"gate"}),
        ),
    ] {
        fields["run_id"] = json!(run);
        let command: Command = serde_json::from_value(json!({name:fields})).unwrap();
        let error = handler
            .handle(command, CallerContext::test_fixture())
            .await
            .err()
            .expect("reserved source command must fail");
        assert!(
            error.to_string().contains("continuation_in_progress"),
            "{name}: {error}"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM stage_executions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn start_run_cannot_probe_files_or_create_a_sibling_for_a_reserved_idea() {
    let (pool, run) = source().await;
    let source = db::repos::runs::find_by_id(&pool, run)
        .await
        .unwrap()
        .unwrap();
    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let command = Command::StartRun(domain::commands::StartRunCmd {
        idea_id: source.idea_id,
        workflow_id: "new".into(),
        workflow_title: "New".into(),
        workspace_root: "/must-not-probe".into(),
        artifact_root: "/must-not-probe/meta".into(),
        workflow_yaml_path: "/must-not-open/workflow.yaml".into(),
        agent_catalog_yaml_path: "/must-not-open/catalog.yaml".into(),
        delivery_configuration_json: None,
        review_routing_json: None,
        rollout_contract_preflight_policy_json: None,
        closeout_readiness_mode: None,
    });
    let error = handler
        .handle(command, CallerContext::test_fixture())
        .await
        .err()
        .expect("reserved idea must fail");
    assert!(
        error.to_string().contains("continuation_in_progress"),
        "{error}"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn actual_startup_recovery_keeps_reserved_source_historical_and_never_requeues_it() {
    assert!(
        std::env::var_os("CHAINWORKS_TOOLCHAIN_HOME").is_none(),
        "offline gate must not sweep a configured toolchain home"
    );
    let (pool, run) = source().await;
    db::repos::run_continuations::recover_preparations(&pool)
        .await
        .unwrap();
    let before = db::repos::runs::find_by_id(&pool, run)
        .await
        .unwrap()
        .unwrap();
    let service = engine::recovery::RecoveryService::new(
        pool.clone(),
        WorkQueue::new(pool.clone()),
        event_bus::new_bus(16),
    );
    let summary = service.run_startup_repair().await.unwrap();
    assert_eq!(summary.runs_inspected, 0);
    assert_eq!(summary.runs_repaired, 0);
    assert_eq!(summary.work_items_requeued, 0);
    assert_eq!(
        service
            .run_p092_retry_payload_recovery_for_active_runs()
            .await
            .unwrap(),
        0
    );
    let after = db::repos::runs::find_by_id(&pool, run)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE run_id=?")
            .bind(run.to_string())
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM provider_sessions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert!(
        db::repos::run_continuations::check_source_guard(&pool, &run.to_string())
            .await
            .is_err()
    );
}
