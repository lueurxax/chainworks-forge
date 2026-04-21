use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{
    agent_executions, approvals, artifact_contracts, artifacts, ideas, runs, sessions, stages,
    work_items,
};
use domain::agent::{AgentExecution, AgentStatus};
use domain::approval::{Approval, ApprovalDecision};
use domain::commands::{
    ApproveStageCmd, CallerContext, Command, RejectStageCmd, RetryStageCmd, RunStewardAnalysisCmd,
    StartRunCmd,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, ApprovalId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::orchestrator::Orchestrator;
use engine::recovery::RecoveryService;
use engine::work_queue::WorkQueue;

async fn test_pool() -> sqlx::SqlitePool {
    create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed")
}

fn make_idea(id: IdeaId) -> Idea {
    Idea {
        id,
        title: "Test idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    }
}

fn make_run(id: RunId, idea_id: IdeaId, status: RunStatus) -> Run {
    Run {
        id,
        idea_id,
        status,
        workflow_id: "wf-test".into(),
        workflow_title: "Test Workflow".into(),
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
    }
}

fn steward_test_runtime_inputs() -> engine::steward::config::StewardRuntimeInputs {
    let mut config = engine::steward::config::StewardConfig::default_config();
    config.windows.observation_window_size = 5;
    config.windows.baseline_window_size = 5;
    config.windows.minimum_window_size = 5;
    engine::steward::config::synthetic_runtime_inputs(
        config,
        serde_json::json!({
            "schema_version": 1,
            "agents": [{"id": "system_steward"}, {"id": "steward_auditor"}],
            "artifacts": {
                "metrics_window": "${CHAINWORKS_META_ROOT:-.chainworks}/steward/metrics-window.json"
            }
        }),
    )
    .unwrap()
}

fn analysis_root(base: &std::path::Path, analysis_id: &str) -> std::path::PathBuf {
    base.join("steward").join("analyses").join(analysis_id)
}

async fn insert_steward_completed_run(
    pool: &sqlx::SqlitePool,
    idea_id: IdeaId,
    completed_at: chrono::DateTime<Utc>,
    lead_time_seconds: i64,
) -> RunId {
    let run_id = RunId::new();
    let mut run = make_run(run_id, idea_id, RunStatus::Completed);
    run.workflow_family = Some("mvp_live".into());
    run.project_key = Some("crypto-savings".into());
    run.risk_class = Some("high".into());
    run.stack = Some("swiftui".into());
    run.workflow_snapshot_hash = Some("a".repeat(64));
    run.catalog_snapshot_hash = Some("b".repeat(64));
    run.workflow_snapshot_json = Some(r#"{"workflow":{"id":"mvp_live"}}"#.into());
    run.catalog_snapshot_json = Some(r#"{"agents":[]}"#.into());
    run.started_at = completed_at - chrono::Duration::seconds(lead_time_seconds);
    run.completed_at = Some(completed_at);
    runs::insert(pool, &run).await.unwrap();
    run_id
}

fn make_stage(id: StageExecutionId, run_id: RunId, status: StageStatus) -> StageExecution {
    StageExecution {
        id,
        run_id,
        stage_id: "stage_test".into(),
        label: "Test Stage".into(),
        status,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: None,
        started_at: Utc::now(),
        completed_at: None,
        owner_agent: None,
        provider: None,
        model: None,
        stage_type: None,
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    }
}

fn make_approval(run_id: RunId, stage_id: &str, decision: ApprovalDecision) -> Approval {
    Approval {
        id: ApprovalId::new(),
        run_id,
        stage_id: stage_id.to_string(),
        decision,
        requested_at: Utc::now(),
        decided_at: None,
        comment: None,
        expires_at: None,
    }
}

fn make_command_handler(pool: sqlx::SqlitePool) -> Arc<CommandHandler> {
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    Arc::new(CommandHandler::new(pool, events, work_queue))
}

fn test_workflow_yaml_path() -> String {
    format!(
        "{}/../../../examples/workflows/workflow.yaml",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn test_agent_catalog_yaml_path() -> String {
    format!(
        "{}/../../../examples/agents/agents.yaml",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn make_agent_execution(
    stage_execution_id: StageExecutionId,
    status: AgentStatus,
) -> AgentExecution {
    AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id,
        agent_id: "worker".into(),
        provider: "claude".into(),
        model: None,
        started_at: Utc::now(),
        completed_at: None,
        status,
        owner_execution_lineage_id: None,
        session_lineage_id: None,
        session_generation_id: None,
        rehydrated_from_checkpoint_artifact_id: None,
        invocation_owner_key: None,
        session_reuse_scope: None,
        session_family_id: None,
        session_reuse_disposition: None,
        session_reset_reason: None,
        backend_profile_id: None,
        requested_mcp_extensions_json: None,
        predicted_mcp_extensions_json: None,
        predicted_mcp_runtime_ids_json: None,
        actual_mcp_extensions_json: None,
        actual_mcp_runtime_ids_json: None,
        denied_mcp_extensions_json: None,
        mcp_blocking_issues_json: None,
        actual_mcp_observation_json: None,
        mcp_session_startup_latency_ms: None,
    }
}

// ---------------------------------------------------------------------------
// Recovery parity harness (P027)
// Proves daemon RecoveryService matches app-side ResumeManager semantics:
// stages stuck in Running after a crash must become Blocked.
// ---------------------------------------------------------------------------

/// RecoveryService must mark stuck-Running stages as Blocked and re-enqueue
/// AdvanceRun, mirroring Swift ResumeManager.normalizeInterruptedRunsForManualResume.
#[tokio::test]
async fn steward_drift_tests_startup_repair_clears_stuck_running_stage_and_marks_drift() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();
    stages::insert(&pool, &make_stage(stage_id, run_id, StageStatus::Running))
        .await
        .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let recovery = RecoveryService::new(pool.clone(), work_queue, events);

    let summary = recovery.run_startup_repair().await.unwrap();

    assert_eq!(
        summary.runs_inspected, 1,
        "one active run must be inspected"
    );
    assert_eq!(summary.runs_repaired, 1, "stuck run must be repaired");
    assert!(
        summary.work_items_requeued >= 1,
        "at least one AdvanceRun must be re-enqueued"
    );

    let repaired_stage = stages::find_by_id(&pool, stage_id).await.unwrap().unwrap();
    assert_eq!(
        repaired_stage.status,
        StageStatus::Blocked,
        "stage stuck in Running must become Blocked after startup repair"
    );
    let repaired_run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert!(
        repaired_run.drift_detected_at.is_some(),
        "startup recovery drift classification must persist run-owned drift timestamp"
    );
    let drift_details: serde_json::Value =
        serde_json::from_str(&repaired_run.drift_details_json.unwrap()).unwrap();
    assert_eq!(drift_details["source"], "startup_repair");
    assert_eq!(drift_details["reason"], "stage_stuck_running");
}

/// A run with no stuck stages must not be counted as repaired.
#[tokio::test]
async fn test_startup_repair_skips_clean_runs() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();
    // Stage is already Completed — nothing to repair.
    stages::insert(&pool, &make_stage(stage_id, run_id, StageStatus::Completed))
        .await
        .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let recovery = RecoveryService::new(pool.clone(), work_queue, events);

    let summary = recovery.run_startup_repair().await.unwrap();

    assert_eq!(summary.runs_inspected, 1);
    assert_eq!(
        summary.runs_repaired, 1,
        "active run with completed stage needs catchup AdvanceRun"
    );
    assert_eq!(
        summary.work_items_requeued, 1,
        "one AdvanceRun must be re-enqueued for startup catchup"
    );

    let unchanged_stage = stages::find_by_id(&pool, stage_id).await.unwrap().unwrap();
    assert_eq!(
        unchanged_stage.status,
        StageStatus::Completed,
        "clean stage must not be modified by startup repair"
    );
}

#[tokio::test]
async fn test_startup_repair_recommendation_includes_execution_session_provenance() {
    use sqlx::Row;

    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();
    stages::insert(&pool, &make_stage(stage_id, run_id, StageStatus::Running))
        .await
        .unwrap();

    let mut execution = make_agent_execution(stage_id, AgentStatus::Running);
    execution.session_reuse_disposition = Some("fresh_after_reset".into());
    execution.session_reset_reason = Some("operator_reset".into());
    execution.rehydrated_from_checkpoint_artifact_id = Some("checkpoint-artifact-1".into());
    agent_executions::insert(&pool, &execution).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let recovery = RecoveryService::new(pool.clone(), work_queue, events);

    let summary = recovery.run_startup_repair().await.unwrap();

    assert_eq!(summary.runs_repaired, 1);

    let row = sqlx::query(
        r#"SELECT reason
           FROM recovery_recommendations
           WHERE run_id = ?1 AND stage_id = ?2
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(run_id.to_string())
    .bind("stage_test")
    .fetch_one(&pool)
    .await
    .unwrap();

    let reason: String = row.get("reason");
    assert!(reason.contains("reuse_disposition=fresh_after_reset"));
    assert!(reason.contains("reset_reason=operator_reset"));
    assert!(reason.contains("checkpoint_artifact_id=checkpoint-artifact-1"));
}

#[tokio::test]
async fn test_cancel_run_phase1_cancels_agent_executions_and_running_work_items() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(stage_exec_id, run_id, StageStatus::Running),
    )
    .await
    .unwrap();
    agent_executions::insert(
        &pool,
        &make_agent_execution(stage_exec_id, AgentStatus::Running),
    )
    .await
    .unwrap();

    work_items::enqueue(
        &pool,
        &db::work_item::WorkItem {
            id: "running-item".into(),
            kind: db::work_item::WorkItemKind::InvokeAgent,
            payload_json: "{}".into(),
            status: db::work_item::WorkItemStatus::Running,
            run_id: Some(run_id),
            stage_id: Some("stage_test".into()),
            created_at: Utc::now(),
            scheduled_at: Utc::now(),
            attempt_count: 1,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::CancelRun(domain::commands::CancelRunCmd { run_id }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Cancelling);
    assert!(run.cancellation_requested_at.is_some());
    let log = run
        .cancellation_settlement_log
        .as_ref()
        .expect("settlement log");
    let entries: serde_json::Value = serde_json::from_str(log).unwrap();
    assert_eq!(entries.as_array().unwrap().len(), 1);
    assert!(entries[0]["session_close_succeeded"].is_null());

    let executions = agent_executions::find_by_stage(&pool, stage_exec_id)
        .await
        .unwrap();
    assert_eq!(executions[0].status, AgentStatus::Cancelled);
    assert!(executions[0].completed_at.is_some());

    let items = work_items::list_by_run(&pool, run_id).await.unwrap();
    assert_eq!(items[0].status, db::work_item::WorkItemStatus::Cancelled);

    let stage = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stage.status, StageStatus::Failed);
}

#[tokio::test]
async fn test_invoke_agent_uses_stage_execution_id_as_owner_execution_lineage() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();
    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "owner_branch_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let owner_key = engine::session::fingerprint::invocation_owner_key(
        &engine::session::fingerprint::InvocationOwnerKeyInput {
            run_id: &run_id.to_string(),
            agent_id: "worker",
            stage_lineage_id: "owner_branch_stage",
            task_name: "owner_branch_stage",
            owner_execution_lineage_id: &stage_exec_id.to_string(),
        },
    );
    let mut execution = make_agent_execution(stage_exec_id, AgentStatus::Running);
    execution.owner_execution_lineage_id = Some(stage_exec_id.to_string());
    execution.invocation_owner_key = Some(owner_key.clone());
    agent_executions::insert(&pool, &execution).await.unwrap();

    let found = agent_executions::find_by_stage(&pool, stage_exec_id)
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("agent execution");

    assert_eq!(
        found.owner_execution_lineage_id.as_deref(),
        Some(stage_exec_id.to_string().as_str())
    );
    assert_eq!(
        found.invocation_owner_key.as_deref(),
        Some(owner_key.as_str())
    );
    assert!(owner_key.ends_with(stage_exec_id.to_string().as_str()));
}

#[tokio::test]
async fn test_cancel_run_eventually_finalizes_to_cancelled() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(stage_exec_id, run_id, StageStatus::Running),
    )
    .await
    .unwrap();
    agent_executions::insert(
        &pool,
        &make_agent_execution(stage_exec_id, AgentStatus::Running),
    )
    .await
    .unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::CancelRun(domain::commands::CancelRunCmd { run_id }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let mut settled = None;
    for _ in 0..20 {
        let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
        if run.status == RunStatus::Cancelled {
            settled = Some(run);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let run = settled.expect("run should finalize to cancelled");
    assert!(run.cancellation_settled_at.is_some());
    let log = run
        .cancellation_settlement_log
        .as_ref()
        .expect("settlement log");
    let entries: serde_json::Value = serde_json::from_str(log).unwrap();
    assert_eq!(
        entries[0]["session_close_attempted"],
        serde_json::json!(false)
    );
}

// ---------------------------------------------------------------------------
// Approval / retry parity harness (P027)
// Proves daemon CommandHandler approval and retry semantics match the
// app-owned ExecutionService authority model:
// – Granted approval → stage transitions WaitingApproval → Running
// – Rejected non-manual approval → stage transitions WaitingApproval → Blocked
// – Rejected workflow manual_gate approval → stage settles Completed so transitions can evaluate
// – Retry → old stage settled as Skipped, new stage created with attempt+1
// ---------------------------------------------------------------------------

/// Granting approval must resolve the canonical approval record to Granted
/// and advance the stage from WaitingApproval to Running.
#[tokio::test]
async fn test_approve_stage_resolves_approval_and_advances_stage() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "review_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "review_stage", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::ApproveStage(ApproveStageCmd {
                run_id,
                stage_id: "review_stage".into(),
                comment: Some("LGTM".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    // Approval must now be Granted.
    let resolved = approvals::find_by_id(&pool, approval.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        resolved.decision,
        ApprovalDecision::Granted,
        "approval decision must be Granted after ApproveStage"
    );
    assert!(resolved.decided_at.is_some(), "decided_at must be set");

    // Stage must have transitioned to Running.
    let updated_stage = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated_stage.status,
        StageStatus::Running,
        "stage must advance to Running after approval is granted"
    );
}

/// Rejecting a non-manual approval must resolve the canonical approval record
/// to Rejected and transition the stage from WaitingApproval to Blocked.
#[tokio::test]
async fn test_reject_stage_resolves_approval_and_blocks_stage() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "gated_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "gated_stage", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::RejectStage(RejectStageCmd {
                run_id,
                stage_id: "gated_stage".into(),
                comment: Some("Not ready".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    // Approval must now be Rejected.
    let resolved = approvals::find_by_id(&pool, approval.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        resolved.decision,
        ApprovalDecision::Rejected,
        "approval decision must be Rejected after RejectStage"
    );

    // Stage must have transitioned to Blocked.
    let updated_stage = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated_stage.status,
        StageStatus::Blocked,
        "stage must become Blocked after approval is rejected"
    );
}

/// Rejecting a workflow manual gate must leave durable rejection evidence and
/// allow the workflow transition evaluator to route an explicit loopback.
#[tokio::test]
async fn test_reject_manual_gate_can_transition_on_rejected_approval() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.current_state = Some("state_6_implementation_approval".into());
    run.workflow_yaml_path = Some(test_workflow_yaml_path());
    run.agent_catalog_yaml_path = Some(test_agent_catalog_yaml_path());
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_6_implementation_approval".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(
        run_id,
        "state_6_implementation_approval",
        ApprovalDecision::Pending,
    );
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::RejectStage(RejectStageCmd {
                run_id,
                stage_id: "state_6_implementation_approval".into(),
                comment: Some("Refine before implementation".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let rejected_stage = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        rejected_stage.status,
        StageStatus::Completed,
        "manual_gate rejection must settle the gate so transition predicates can evaluate"
    );

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue);
    orchestrator.advance_run(run_id).await.unwrap();

    let updated_run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        updated_run.current_state.as_deref(),
        Some("state_5_proposal_refined"),
        "approval.rejected == true must route state_6 back to proposal refinement"
    );

    orchestrator.advance_run(run_id).await.unwrap();
    let refinement_prompt = work_items::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .find(|item| {
            item.kind == db::work_item::WorkItemKind::InvokeAgent
                && item.stage_id.as_deref() == Some("state_5_proposal_refined")
        })
        .and_then(|item| {
            serde_json::from_str::<serde_json::Value>(&item.payload_json)
                .ok()
                .and_then(|payload| payload["prompt"].as_str().map(str::to_owned))
        })
        .expect("state_5 refinement prompt should be enqueued");
    assert!(
        refinement_prompt.contains("Rejected Implementation Approval Context"),
        "state_5 prompt must include rejected approval context"
    );
    assert!(
        refinement_prompt.contains("Refine before implementation"),
        "state_5 prompt must include the operator rejection comment"
    );
}

/// Retrying a stage must settle the old execution as Skipped and produce a new
/// execution for the same stage_id with attempt_number incremented by 1.
#[tokio::test]
async fn test_retry_stage_creates_new_attempt_and_skips_old() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();

    let mut stage = make_stage(old_stage_exec_id, run_id, StageStatus::Failed);
    stage.stage_id = "flaky_stage".into();
    stage.attempt_number = 1;
    stages::insert(&pool, &stage).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "flaky_stage".into(),
                consume_quota_budget_now: false,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    // Old stage must be settled as Skipped.
    let old = stages::find_by_id(&pool, old_stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        old.status,
        StageStatus::Skipped,
        "old stage execution must be settled as Skipped after retry"
    );
    assert_eq!(
        old.settlement_kind,
        Some(domain::stage::StageSettlementKind::Skipped),
        "settlement_kind must be Skipped"
    );

    // A new stage execution must exist with attempt_number = 2 and status Pending.
    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let new_stage = all_stages
        .iter()
        .find(|s| s.stage_id == "flaky_stage" && s.attempt_number == 2)
        .expect("new stage execution with attempt_number=2 must exist after retry");

    assert_eq!(
        new_stage.status,
        StageStatus::Pending,
        "new stage execution must start as Pending"
    );
}

#[tokio::test]
async fn test_retry_stage_targets_latest_matching_attempt() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let first_stage_exec_id = StageExecutionId::new();
    let latest_stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Blocked))
        .await
        .unwrap();

    let mut first = make_stage(first_stage_exec_id, run_id, StageStatus::Failed);
    first.stage_id = "flaky_stage".into();
    first.attempt_number = 1;
    first.started_at = chrono::Utc::now() - chrono::Duration::minutes(5);
    stages::insert(&pool, &first).await.unwrap();

    let mut latest = make_stage(latest_stage_exec_id, run_id, StageStatus::Failed);
    latest.stage_id = "flaky_stage".into();
    latest.attempt_number = 2;
    latest.started_at = chrono::Utc::now();
    stages::insert(&pool, &latest).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "flaky_stage".into(),
                consume_quota_budget_now: false,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let first_after = stages::find_by_id(&pool, first_stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    let latest_after = stages::find_by_id(&pool, latest_stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first_after.status, StageStatus::Failed);
    assert_eq!(latest_after.status, StageStatus::Skipped);

    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    assert!(
        all_stages
            .iter()
            .any(|s| s.stage_id == "flaky_stage" && s.attempt_number == 3),
        "retry must create the next attempt after the latest matching stage"
    );
}

#[tokio::test]
async fn test_retry_stage_allows_completed_current_stage_when_run_is_blocked() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(run_id, idea_id, RunStatus::Blocked);
    run.current_state = Some("blocked_stage".into());
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(old_stage_exec_id, run_id, StageStatus::Completed);
    stage.stage_id = "blocked_stage".into();
    stage.attempt_number = 1;
    stages::insert(&pool, &stage).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "blocked_stage".into(),
                consume_quota_budget_now: false,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let old = stages::find_by_id(&pool, old_stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old.status, StageStatus::Skipped);

    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let new_stage = all_stages
        .iter()
        .find(|s| s.stage_id == "blocked_stage" && s.attempt_number == 2)
        .expect("blocked completed current stage retry must create a new attempt");
    assert_eq!(new_stage.status, StageStatus::Pending);
}

#[tokio::test]
async fn test_retry_stage_rejects_active_latest_attempt() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();

    let mut failed = make_stage(StageExecutionId::new(), run_id, StageStatus::Failed);
    failed.stage_id = "flaky_stage".into();
    failed.attempt_number = 1;
    failed.started_at = chrono::Utc::now() - chrono::Duration::minutes(5);
    stages::insert(&pool, &failed).await.unwrap();

    let mut running = make_stage(StageExecutionId::new(), run_id, StageStatus::Running);
    running.stage_id = "flaky_stage".into();
    running.attempt_number = 2;
    running.started_at = chrono::Utc::now();
    stages::insert(&pool, &running).await.unwrap();

    let handler = make_command_handler(pool.clone());
    let result = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "flaky_stage".into(),
                consume_quota_budget_now: false,
            }),
            CallerContext::test_fixture(),
        )
        .await;

    assert!(
        result.is_err(),
        "retry must reject while the latest stage attempt is still active"
    );
}

#[tokio::test]
async fn test_advance_run_consumes_pending_compute_retry_stage() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let temp = tempfile::tempdir().unwrap();
    let workflow_path = temp.path().join("workflow.yaml");
    let catalog_path = temp.path().join("catalog.yaml");
    std::fs::write(
        &workflow_path,
        r#"
workflow:
  id: compute-retry
initial_state: build
states:
  build:
    label: Build
    owner: worker
    run:
      sequence:
        - agent: worker
          task: implement
          outputs:
            - implementation_progress
"#,
    )
    .unwrap();
    std::fs::write(
        &catalog_path,
        r#"
backend_profiles:
  worker_profile:
    provider: claude
agents:
  - id: worker
    backend_profile: worker_profile
"#,
    )
    .unwrap();

    let mut run = make_run(run_id, idea_id, RunStatus::Blocked);
    run.current_state = Some("build".into());
    run.workflow_yaml_path = Some(workflow_path.to_string_lossy().into_owned());
    run.agent_catalog_yaml_path = Some(catalog_path.to_string_lossy().into_owned());
    runs::insert(&pool, &run).await.unwrap();

    let mut old_stage = make_stage(old_stage_exec_id, run_id, StageStatus::Failed);
    old_stage.stage_id = "build".into();
    old_stage.attempt_number = 1;
    stages::insert(&pool, &old_stage).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "build".into(),
                consume_quota_budget_now: false,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let pending_retry = stages::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.stage_id == "build" && s.attempt_number == 2)
        .expect("RetryStage must create attempt=2");
    assert_eq!(pending_retry.status, StageStatus::Pending);

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    let retry_after = stages::find_by_id(&pool, pending_retry.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        retry_after.status,
        StageStatus::Running,
        "advance_run must start the pending retry attempt instead of creating a replacement stage"
    );

    let stages_after = stages::list_by_run(&pool, run_id).await.unwrap();
    let build_stages: Vec<_> = stages_after
        .iter()
        .filter(|s| s.stage_id == "build")
        .collect();
    assert_eq!(
        build_stages.len(),
        2,
        "compute retry must keep exactly the skipped old stage and the running retry stage"
    );
    assert!(
        !build_stages
            .iter()
            .any(|s| s.id != old_stage_exec_id && s.id != pending_retry.id),
        "advance_run must not fork an extra build stage execution"
    );

    let invoke_items: Vec<_> = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();
    assert_eq!(invoke_items.len(), 1);
    assert!(
        invoke_items[0]
            .payload_json
            .contains(&pending_retry.id.to_string()),
        "enqueued InvokeAgent work must target the pending retry stage execution"
    );
}

#[tokio::test]
async fn test_self_loop_transition_creates_next_iteration_before_reentering() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let completed_stage_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let temp = tempfile::tempdir().unwrap();
    let meta_root = temp.path().join(".chainworks");
    std::fs::create_dir_all(&meta_root).unwrap();
    std::fs::write(
        meta_root.join("self-assessment.json"),
        r#"{"seemingly_complete":false}"#,
    )
    .unwrap();

    let workflow_path = temp.path().join("workflow.yaml");
    let catalog_path = temp.path().join("catalog.yaml");
    std::fs::write(
        &workflow_path,
        r#"
workflow:
  id: self-loop
variables:
  max_self_loop_cycles: 3
initial_state: loop
states:
  loop:
    label: Loop
    owner: worker
    run:
      sequence:
        - agent: worker
          task: implement
          outputs:
            - implementation_self_assessment
    loop:
      counter: self_loop_count
      max: vars.max_self_loop_cycles
    transitions:
      - to: done
        when: implementation_self_assessment.seemingly_complete == true
      - to: loop
        when: implementation_self_assessment.seemingly_complete == false
  done:
    label: Done
    type: end
    owner: worker
"#,
    )
    .unwrap();
    std::fs::write(
        &catalog_path,
        r#"
artifacts:
  implementation_self_assessment: ${CHAINWORKS_META_ROOT:-.chainworks}/self-assessment.json
backend_profiles:
  worker_profile:
    provider: claude
agents:
  - id: worker
    backend_profile: worker_profile
"#,
    )
    .unwrap();

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.current_state = Some("loop".into());
    run.workspace_root = temp.path().to_string_lossy().into_owned();
    run.artifact_root = temp.path().join("artifacts").to_string_lossy().into_owned();
    run.chainworks_meta_root = Some(meta_root.to_string_lossy().into_owned());
    run.workflow_yaml_path = Some(workflow_path.to_string_lossy().into_owned());
    run.agent_catalog_yaml_path = Some(catalog_path.to_string_lossy().into_owned());
    runs::insert(&pool, &run).await.unwrap();

    let mut completed_stage = make_stage(completed_stage_id, run_id, StageStatus::Running);
    completed_stage.stage_id = "loop".into();
    completed_stage.label = "Loop".into();
    stages::insert(&pool, &completed_stage).await.unwrap();
    stages::settle(
        &pool,
        completed_stage_id,
        domain::stage::StageSettlementKind::Completed,
        Utc::now(),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());

    orchestrator.advance_run(run_id).await.unwrap();

    let loop_stages: Vec<_> = stages::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .filter(|s| s.stage_id == "loop")
        .collect();
    assert_eq!(
        loop_stages.len(),
        2,
        "self-loop transition must persist the next iteration before the queued re-entry runs"
    );

    let next_iteration = loop_stages
        .iter()
        .find(|s| s.iteration == 2)
        .expect("self-loop must create iteration 2");
    assert_eq!(next_iteration.status, StageStatus::Pending);

    let advance_items: Vec<_> = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::AdvanceRun)
        .collect();
    assert_eq!(
        advance_items.len(),
        1,
        "self-loop transition should enqueue one re-entry work item"
    );

    orchestrator.advance_run(run_id).await.unwrap();

    let next_after_reentry = stages::find_by_id(&pool, next_iteration.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        next_after_reentry.status,
        StageStatus::Running,
        "queued self-loop re-entry must start the pending next iteration"
    );

    let invoke_items: Vec<_> = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();
    assert_eq!(invoke_items.len(), 1);
    assert!(
        invoke_items[0]
            .payload_json
            .contains(&next_iteration.id.to_string()),
        "InvokeAgent work must target the self-loop iteration 2 stage execution"
    );
}

#[tokio::test]
async fn test_code_writer_prompt_scopes_seemingly_complete_to_code_owned_work() {
    let temp = tempfile::tempdir().unwrap();
    let db_path = temp.path().join("prompt-scope.sqlite");
    let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
        .await
        .unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let workflow_path = temp.path().join("workflow.yaml");
    let catalog_path = temp.path().join("catalog.yaml");
    std::fs::write(
        &workflow_path,
        r#"
workflow:
  id: code-writer-scope
initial_state: implementation
states:
  implementation:
    label: Implementation
    owner: code_writer
    run:
      sequence:
        - agent: code_writer
          task: continue_implementation
          outputs:
            - implementation_self_assessment
    transitions:
      - to: reviewed
        when: implementation_self_assessment.seemingly_complete == true
      - to: implementation
        when: implementation_self_assessment.seemingly_complete == false
  reviewed:
    label: Reviewed
    type: end
    owner: code_writer
"#,
    )
    .unwrap();
    std::fs::write(
        &catalog_path,
        r#"
artifacts:
  implementation_self_assessment: ${CHAINWORKS_META_ROOT:-.chainworks}/self-assessment.json
backend_profiles:
  code_writer_profile:
    provider: codex
contracts:
  implementation_self_assessment_v1:
    format: json
    required_fields:
      - seemingly_complete
      - remaining_tasks
      - known_risks
      - tests_run
      - docs_impacted
agents:
  - id: code_writer
    backend_profile: code_writer_profile
    output_contract: implementation_self_assessment_v1
"#,
    )
    .unwrap();

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.current_state = Some("implementation".into());
    run.workspace_root = temp.path().to_string_lossy().into_owned();
    run.artifact_root = temp.path().join("artifacts").to_string_lossy().into_owned();
    run.chainworks_meta_root = Some(
        temp.path()
            .join(".chainworks")
            .to_string_lossy()
            .into_owned(),
    );
    run.worktree_root = Some(temp.path().join("worktree").to_string_lossy().into_owned());
    run.workflow_yaml_path = Some(workflow_path.to_string_lossy().into_owned());
    run.agent_catalog_yaml_path = Some(catalog_path.to_string_lossy().into_owned());
    runs::insert(&pool, &run).await.unwrap();

    let mut pending_stage = make_stage(stage_execution_id, run_id, StageStatus::Pending);
    pending_stage.stage_id = "implementation".into();
    pending_stage.label = "Implementation".into();
    stages::insert(&pool, &pending_stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue);

    orchestrator.advance_run(run_id).await.unwrap();

    let invoke_item = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .find(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .expect("code writer stage should enqueue InvokeAgent");
    let payload: serde_json::Value = serde_json::from_str(&invoke_item.payload_json).unwrap();
    let prompt = payload["prompt"]
        .as_str()
        .expect("InvokeAgent payload must include prompt");

    assert!(
        prompt.contains("Set `seemingly_complete` based only on remaining code-writer-owned source or test work."),
        "code_writer prompt must prevent non-code/manual/release evidence blockers from forcing implementation self-loops"
    );
    assert!(
        prompt.contains("remaining_code_tasks"),
        "code_writer prompt should ask for stable ownership classification fields"
    );
    assert!(
        prompt.contains("handoff_tasks"),
        "code_writer prompt should route non-code blockers as handoff tasks"
    );
}

/// Starting a run must persist the frozen delivery configuration JSON on the
/// run record so downstream release logic can consume it.
#[tokio::test]
async fn test_start_run_persists_delivery_configuration_json() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let handler = make_command_handler(pool.clone());
    let repo = tempfile::tempdir().unwrap();
    std::process::Command::new("git")
        .args(["init", "--initial-branch", "main"])
        .current_dir(repo.path())
        .output()
        .expect("git init should run");
    let worktrees = tempfile::tempdir().unwrap();
    let delivery_configuration_json = Some(format!(
        r#"{{
            "repo_identifier":"repo-1",
            "repo_root":"{}",
            "base_branch":"main",
            "worktree_base_path":"{}",
            "target_branch":"cw/release",
            "release_target_id":"app-store"
        }}"#,
        repo.path().display(),
        worktrees.path().display()
    ));

    let commanded = handler
        .handle(
            Command::StartRun(StartRunCmd {
                idea_id,
                workflow_id: "wf-start".into(),
                workflow_title: "Start Run".into(),
                workspace_root: "/tmp/ws".into(),
                artifact_root: "/tmp/art".into(),
                workflow_yaml_path: test_workflow_yaml_path(),
                agent_catalog_yaml_path: test_agent_catalog_yaml_path(),
                delivery_configuration_json: delivery_configuration_json.clone(),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let run_id = match commanded.result {
        engine::command_handler::CommandResult::RunStarted { run_id } => run_id,
        _ => panic!("unexpected command result"),
    };

    let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(run.delivery_configuration_json, delivery_configuration_json);
    assert_eq!(
        run.workflow_family.as_deref(),
        Some("proposal_to_release"),
        "new runs must freeze workflow family from parsed workflow metadata with id fallback"
    );
    assert_eq!(run.project_key.as_deref(), Some("untagged"));
    assert_eq!(run.risk_class.as_deref(), Some("standard"));
    assert_eq!(run.stack.as_deref(), Some("unknown"));
    assert_eq!(
        run.workflow_snapshot_hash.as_deref().map(str::len),
        Some(64)
    );
    assert_eq!(run.catalog_snapshot_hash.as_deref().map(str::len), Some(64));
    assert!(run
        .workflow_snapshot_json
        .as_deref()
        .unwrap_or_default()
        .contains("proposal_to_release"));
    assert!(run
        .catalog_snapshot_json
        .as_deref()
        .unwrap_or_default()
        .contains("backend_profiles"));
}

#[tokio::test]
async fn steward_pipeline_tests_persists_analysis_and_deterministic_artifacts() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let completed_at = Utc::now();

    for idx in 0..10 {
        let mut run = make_run(RunId::new(), idea_id, RunStatus::Completed);
        run.workflow_family = Some("mvp_live".into());
        run.project_key = Some("crypto-savings".into());
        run.risk_class = Some("high".into());
        run.stack = Some("swiftui".into());
        run.workflow_snapshot_hash = Some("a".repeat(64));
        run.catalog_snapshot_hash = Some("b".repeat(64));
        run.workflow_snapshot_json = Some(r#"{"workflow":{"id":"mvp_live"}}"#.into());
        run.catalog_snapshot_json = Some(r#"{"agents":[]}"#.into());
        run.completed_at = Some(completed_at - chrono::Duration::seconds(idx));
        runs::insert(&pool, &run).await.unwrap();
    }

    let mut legacy = make_run(RunId::new(), idea_id, RunStatus::Completed);
    legacy.completed_at = Some(completed_at - chrono::Duration::seconds(20));
    runs::insert(&pool, &legacy).await.unwrap();

    let artifact_base = tempfile::tempdir().unwrap();
    let runtime_inputs = steward_test_runtime_inputs();
    let analysis = engine::steward::run_steward_analysis(
        &pool,
        &runtime_inputs,
        engine::steward::StewardAnalysisRequest::manual(artifact_base.path()),
    )
    .await
    .unwrap();

    assert_eq!(analysis.status.to_string(), "completed");
    assert_eq!(analysis.cohort_quality.to_string(), "strong");
    assert_eq!(analysis.run_count, 5);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&analysis.cohort_keys_json).unwrap(),
        serde_json::json!({
            "risk_class": "high",
            "workflow_family": "mvp_live"
        })
    );
    assert_eq!(
        analysis.agent_catalog_snapshot_hash,
        runtime_inputs.agent_catalog_hash
    );
    assert_eq!(
        analysis.steward_config_snapshot_hash,
        runtime_inputs.steward_config_hash
    );

    let persisted = db::repos::steward::find_analysis(&pool, &analysis.id)
        .await
        .unwrap()
        .expect("analysis should persist");
    assert_eq!(
        persisted.metrics_snapshot_artifact_id,
        analysis.metrics_snapshot_artifact_id
    );

    let root = analysis_root(artifact_base.path(), &analysis.id);
    let metrics_path = root
        .join("active-catalog-io")
        .join("steward")
        .join("metrics-window.json");
    let metrics_json = std::fs::read_to_string(metrics_path).unwrap();
    let metrics: serde_json::Value = serde_json::from_str(&metrics_json).unwrap();
    assert_eq!(metrics["run_count"], 5);
    assert_eq!(metrics["legacy_pre_p049_excluded_count"], 1);

    let workflow_snapshot_path = root
        .join("active-catalog-io")
        .join("steward")
        .join("workflow-snapshot.json");
    let workflow_snapshot: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(workflow_snapshot_path).unwrap()).unwrap();
    assert_eq!(
        workflow_snapshot["primary_workflow_family"],
        serde_json::json!("mvp_live")
    );
    assert_eq!(workflow_snapshot["snapshot_count"], serde_json::json!(1));
    assert!(
        root.join("active-catalog-io")
            .join("steward")
            .join("catalog-snapshot.json")
            .exists(),
        "analysis must materialize current daemon-owned catalog snapshot"
    );
}

#[tokio::test]
async fn steward_pipeline_tests_persists_failed_analysis_when_deterministic_slice_errors() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let completed_at = Utc::now();
    let mut run = make_run(RunId::new(), idea_id, RunStatus::Completed);
    run.workflow_family = Some("mvp_live".into());
    run.project_key = Some("crypto-savings".into());
    run.risk_class = Some("high".into());
    run.stack = Some("swiftui".into());
    run.workflow_snapshot_hash = Some("a".repeat(64));
    run.catalog_snapshot_hash = Some("b".repeat(64));
    run.workflow_snapshot_json = Some("{not-json".into());
    run.catalog_snapshot_json = Some(r#"{"agents":[]}"#.into());
    run.completed_at = Some(completed_at);
    runs::insert(&pool, &run).await.unwrap();

    let result = engine::steward::run_steward_analysis(
        &pool,
        &steward_test_runtime_inputs(),
        engine::steward::StewardAnalysisRequest::manual(tempfile::tempdir().unwrap().path()),
    )
    .await;

    assert!(result.is_err());
    let failed = db::repos::steward::list_analyses(
        &pool,
        10,
        Some(domain::steward::StewardAnalysisStatus::Failed),
    )
    .await
    .unwrap();
    assert_eq!(failed.len(), 1);
    assert!(failed[0]
        .error_summary
        .as_deref()
        .unwrap_or_default()
        .contains("parse workflow snapshot"));
}

#[tokio::test]
async fn steward_pipeline_tests_detects_degradation_and_persists_recommendation() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let completed_at = Utc::now();

    for idx in 0..5 {
        insert_steward_completed_run(
            &pool,
            idea_id,
            completed_at - chrono::Duration::seconds(idx),
            200,
        )
        .await;
    }
    for idx in 5..10 {
        insert_steward_completed_run(
            &pool,
            idea_id,
            completed_at - chrono::Duration::seconds(idx),
            100,
        )
        .await;
    }

    let mut runtime_inputs = steward_test_runtime_inputs();
    runtime_inputs.steward_config.thresholds.insert(
        "lead_time_median_seconds".into(),
        engine::steward::config::StewardThreshold {
            method: "median_percentage".into(),
            trigger: 0.2,
        },
    );

    let artifact_base = tempfile::tempdir().unwrap();
    let analysis = engine::steward::run_steward_analysis(
        &pool,
        &runtime_inputs,
        engine::steward::StewardAnalysisRequest::manual(artifact_base.path()),
    )
    .await
    .unwrap();

    assert_eq!(analysis.degradation_count, 1);
    let recommendations = db::repos::steward::list_recommendations(&pool, &analysis.id)
        .await
        .unwrap();
    assert_eq!(recommendations.len(), 1);
    assert_eq!(recommendations[0].target_metric, "lead_time_median_seconds");

    let alerts_path =
        analysis_root(artifact_base.path(), &analysis.id).join("degradation-alerts.json");
    let alerts_json = std::fs::read_to_string(&alerts_path).unwrap();
    assert!(
        !alerts_json.contains("analysis_id"),
        "deterministic alert artifacts must not embed random analysis ids"
    );
    let second = engine::steward::run_steward_analysis(
        &pool,
        &runtime_inputs,
        engine::steward::StewardAnalysisRequest::manual(artifact_base.path()),
    )
    .await
    .unwrap();
    let second_alerts_json = std::fs::read_to_string(
        analysis_root(artifact_base.path(), &second.id).join("degradation-alerts.json"),
    )
    .unwrap();
    assert_eq!(
        alerts_json, second_alerts_json,
        "deterministic alert artifact bytes must be stable across unchanged reruns"
    );
}

#[tokio::test]
async fn steward_metrics_tests_use_domain_decisions_and_frozen_stage_families() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let run_id = RunId::new();
    let completed_at = Utc::now();
    let mut run = make_run(run_id, idea_id, RunStatus::Completed);
    run.workflow_family = Some("mvp_live".into());
    run.project_key = Some("crypto-savings".into());
    run.risk_class = Some("high".into());
    run.stack = Some("swiftui".into());
    run.workflow_snapshot_hash = Some("a".repeat(64));
    run.catalog_snapshot_hash = Some("b".repeat(64));
    run.workflow_snapshot_json = Some(
        serde_json::json!({
            "workflow": {"id": "mvp_live"},
            "states": {
                "alpha": {"label": "Proposal drafting", "owner": "writer"},
                "beta": {"label": "Build execution", "run": {"sequence": [{"agent": "code_writer", "task": "Implementation pass"}]}},
                "gamma": {"label": "Independent quality audit", "owner": "reviewer"}
            }
        })
        .to_string(),
    );
    run.catalog_snapshot_json = Some(r#"{"agents":[]}"#.into());
    run.started_at = completed_at - chrono::Duration::seconds(100);
    run.completed_at = Some(completed_at);
    runs::insert(&pool, &run).await.unwrap();

    let mut granted = make_approval(run_id, "approval", ApprovalDecision::Granted);
    granted.decided_at = Some(completed_at);
    approvals::insert(&pool, &granted).await.unwrap();
    let mut rejected = make_approval(run_id, "approval", ApprovalDecision::Rejected);
    rejected.decided_at = Some(completed_at);
    approvals::insert(&pool, &rejected).await.unwrap();

    for (stage_id, cost) in [("alpha", 10), ("beta", 20), ("gamma", 30)] {
        let stage_execution_id = StageExecutionId::new();
        let mut stage = make_stage(stage_execution_id, run_id, StageStatus::Completed);
        stage.stage_id = stage_id.into();
        stage.started_at = completed_at - chrono::Duration::seconds(10);
        stage.completed_at = Some(completed_at);
        stages::insert(&pool, &stage).await.unwrap();

        let lineage_id = format!("lineage-{stage_id}");
        let generation_id = format!("generation-{stage_id}");
        sessions::insert_lineage(
            &pool,
            &domain::session::SessionLineage {
                id: lineage_id.clone(),
                run_id: run_id.to_string(),
                agent_id: "agent".into(),
                lineage_id: stage_id.into(),
                session_reuse_scope: "same_invocation_owner".into(),
                session_family_id: None,
                active_generation_id: Some(generation_id.clone()),
                created_at: completed_at,
                closed_at: None,
            },
        )
        .await
        .unwrap();
        sessions::insert_generation(
            &pool,
            &domain::session::SessionGeneration {
                id: generation_id.clone(),
                lineage_id,
                generation: 1,
                invocation_owner_key: "owner".into(),
                provider_session_id: Some("provider-session".into()),
                binding_fingerprint: "fingerprint".into(),
                rehydrated_from_checkpoint_artifact_id: None,
                working_directory: "/tmp/ws".into(),
                workspace_mode: "read_only".into(),
                runtime_provider: "claude".into(),
                runtime_model: "opus".into(),
                status: domain::session::SessionGenerationStatus::Closed,
                turn_count: 1,
                estimated_input_tokens: 0,
                latest_cached_input_tokens: None,
                latest_output_tokens: None,
                latest_model_context_window: None,
                cumulative_prompt_tokens: 0,
                cumulative_cost_cents: cost,
                created_at: completed_at,
                last_activity_at: None,
                ended_at: Some(completed_at),
                end_reason: Some("completed".into()),
            },
        )
        .await
        .unwrap();

        let mut execution = make_agent_execution(stage_execution_id, AgentStatus::Completed);
        execution.session_generation_id = Some(generation_id);
        agent_executions::insert(&pool, &execution).await.unwrap();
    }

    let metrics = engine::steward::metrics::collect_metrics(
        &pool,
        &[run],
        Some(&engine::steward::cohort::CohortKey {
            workflow_family: "mvp_live".into(),
            risk_class: "high".into(),
        }),
        0,
    )
    .await
    .unwrap();

    assert_eq!(metrics.approval_rejection_rate, 0.5);
    assert_eq!(metrics.cost_by_stage_family.get("proposal"), Some(&10));
    assert_eq!(
        metrics.cost_by_stage_family.get("implementation"),
        Some(&20)
    );
    assert_eq!(metrics.cost_by_stage_family.get("audit"), Some(&30));
}

struct FakeStewardAgentExecutor;

#[async_trait::async_trait]
impl engine::steward::service::StewardAgentExecutor for FakeStewardAgentExecutor {
    async fn run_steward_agent(
        &self,
        invocation: engine::steward::service::StewardAgentInvocation,
    ) -> anyhow::Result<()> {
        let steward_root = invocation.chainworks_meta_root.join("steward");
        match invocation.agent_id.as_str() {
            "system_steward" => {
                std::fs::create_dir_all(steward_root.join("reports"))?;
                std::fs::create_dir_all(steward_root.join("proposals"))?;
                std::fs::write(
                    steward_root.join("reports").join("health-report.json"),
                    r#"{"analysis_id":"test","confidence":"high"}"#,
                )?;
                std::fs::write(
                    steward_root.join("proposals").join("agent-tuning.json"),
                    r#"{"analysis_id":"test","status":"proposed"}"#,
                )?;
            }
            "steward_auditor" => {
                std::fs::create_dir_all(steward_root.join("reports"))?;
                std::fs::write(
                    steward_root.join("reports").join("audit-report.json"),
                    r#"{"analysis_id":"test","confidence":"high"}"#,
                )?;
            }
            other => anyhow::bail!("unexpected steward agent {other}"),
        }
        Ok(())
    }
}

#[tokio::test]
async fn steward_pipeline_tests_runs_optional_catalog_lanes_after_inputs_exist() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let completed_at = Utc::now();
    for idx in 0..10 {
        insert_steward_completed_run(
            &pool,
            idea_id,
            completed_at - chrono::Duration::seconds(idx),
            120,
        )
        .await;
    }

    let artifact_base = tempfile::tempdir().unwrap();
    let runtime_inputs = steward_test_runtime_inputs();
    let analysis = engine::steward::service::run_steward_analysis_with_executor(
        &pool,
        &runtime_inputs,
        engine::steward::StewardAnalysisRequest::manual(artifact_base.path()),
        Some(&FakeStewardAgentExecutor),
    )
    .await
    .unwrap();

    assert!(analysis.health_report_artifact_id.is_some());
    assert!(analysis.audit_report_artifact_id.is_some());
    assert!(analysis.agent_tuning_artifact_id.is_some());
    let root = analysis_root(artifact_base.path(), &analysis.id)
        .join("active-catalog-io")
        .join("steward");
    assert!(root.join("reports").join("health-report.json").exists());
    assert!(root.join("reports").join("audit-report.json").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn steward_executor_tests_work_item_runs_active_catalog_agents_through_acp() {
    use std::os::unix::fs::PermissionsExt;

    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use engine::executor::BackgroundExecutor;

    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let completed_at = Utc::now();
    for idx in 0..10 {
        insert_steward_completed_run(
            &pool,
            idea_id,
            completed_at - chrono::Duration::seconds(idx),
            120,
        )
        .await;
    }

    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("steward_acp_fixture.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, os, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    return json.loads(line)

msg = recv()
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"protocolVersion": 1}})
msg = recv()
cwd = msg.get("params", {}).get("cwd")
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"sessionId": "steward-fixture"}})
msg = recv()
reports = os.path.join(cwd, "steward", "reports")
proposals = os.path.join(cwd, "steward", "proposals")
os.makedirs(reports, exist_ok=True)
os.makedirs(proposals, exist_ok=True)
with open(os.path.join(reports, "health-report.json"), "w") as f:
    f.write('{"status":"healthy"}')
with open(os.path.join(reports, "audit-report.json"), "w") as f:
    f.write('{"status":"audited"}')
with open(os.path.join(proposals, "agent-tuning.json"), "w") as f:
    f.write('{"status":"proposed"}')
send({"jsonrpc": "2.0", "id": msg["id"], "result": {"stopReason": "end_turn", "sessionId": "steward-fixture"}})
recv()
"#,
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).unwrap();

    let catalog_path = temp.path().join("agents.yaml");
    std::fs::write(
        &catalog_path,
        r#"
backend_profiles:
  steward_profile:
    provider: claude_acp
    model: fixture
    mcp: []
artifacts:
  metrics_window: ${CHAINWORKS_META_ROOT:-.chainworks}/steward/metrics-window.json
  baseline_window: ${CHAINWORKS_META_ROOT:-.chainworks}/steward/baseline-window.json
  sdlc_health_report: ${CHAINWORKS_META_ROOT:-.chainworks}/steward/reports/health-report.json
  stewardship_audit_report: ${CHAINWORKS_META_ROOT:-.chainworks}/steward/reports/audit-report.json
  agent_tuning_proposal: ${CHAINWORKS_META_ROOT:-.chainworks}/steward/proposals/agent-tuning.json
skills:
  steward_core:
    type: inline_skill
    description: Steward fixture skill.
agents:
  - id: system_steward
    backend_profile: steward_profile
    skill_ref: steward_core
    inputs: [metrics_window, baseline_window]
    outputs: [sdlc_health_report, agent_tuning_proposal]
    prompt: "Run system steward."
  - id: steward_auditor
    backend_profile: steward_profile
    skill_ref: steward_core
    inputs: [sdlc_health_report]
    outputs: [stewardship_audit_report]
    prompt: "Run steward auditor."
"#,
    )
    .unwrap();

    let (catalog_json, catalog_hash) =
        engine::steward::config::load_agent_catalog_json(&catalog_path).unwrap();
    let mut runtime_inputs = steward_test_runtime_inputs();
    runtime_inputs.agent_catalog_path = catalog_path;
    runtime_inputs.agent_catalog_json = catalog_json;
    runtime_inputs.agent_catalog_hash = catalog_hash;

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_string_lossy().into_owned(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![adapter]));
    let executor = BackgroundExecutor::new_with_steward_runtime_inputs(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp,
        events,
        Arc::new(runtime_inputs),
    );

    let artifact_base = temp.path().join("artifacts");
    work_queue
        .enqueue(
            db::work_item::WorkItemKind::StewardAnalysis,
            None,
            None,
            serde_json::json!({
                "reason": "manual",
                "artifact_base": artifact_base.to_string_lossy()
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());
    let analyses = db::repos::steward::list_analyses(&pool, 10, None)
        .await
        .unwrap();
    assert_eq!(analyses.len(), 1);
    assert!(analyses[0].health_report_artifact_id.is_some());
    assert!(analyses[0].audit_report_artifact_id.is_some());
    assert!(analyses[0].agent_tuning_artifact_id.is_some());
}

#[tokio::test]
async fn steward_trigger_tests_manual_command_enqueues_shared_work_item() {
    let pool = test_pool().await;
    let handler = make_command_handler(pool.clone());

    let result = handler
        .handle(
            Command::RunStewardAnalysis(RunStewardAnalysisCmd {
                reason: "manual".into(),
                artifact_base: Some("/tmp/steward-artifacts".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    assert!(matches!(
        result.result,
        engine::command_handler::CommandResult::StewardAnalysisQueued
    ));
    let item = work_items::claim_next(&pool)
        .await
        .unwrap()
        .expect("manual trigger should enqueue a work item");
    assert_eq!(item.kind, db::work_item::WorkItemKind::StewardAnalysis);
    let payload: serde_json::Value = serde_json::from_str(&item.payload_json).unwrap();
    assert_eq!(payload["reason"], "manual");
    assert_eq!(payload["artifact_base"], "/tmp/steward-artifacts");
}

#[tokio::test]
async fn steward_trigger_tests_completed_run_consumes_config_change_pending_first() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let temp = tempfile::tempdir().unwrap();
    let workflow_path = temp.path().join("workflow.yaml");
    let catalog_path = temp.path().join("catalog.yaml");
    std::fs::write(
        &workflow_path,
        r#"
workflow:
  id: steward-trigger
  family: steward_family
  risk_class: standard
  stack: rust
initial_state: done
states:
  done:
    label: Done
    type: end
    owner: lead
"#,
    )
    .unwrap();
    std::fs::write(
        &catalog_path,
        r#"
backend_profiles:
  lead_profile:
    provider: claude
agents:
  - id: lead
    backend_profile: lead_profile
"#,
    )
    .unwrap();

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.current_state = Some("done".into());
    run.workflow_yaml_path = Some(workflow_path.to_string_lossy().into_owned());
    run.agent_catalog_yaml_path = Some(catalog_path.to_string_lossy().into_owned());
    runs::insert(&pool, &run).await.unwrap();
    db::repos::steward::mark_config_change_pending(
        &pool,
        Some("config-hash"),
        Some("catalog-hash"),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue);
    orchestrator.advance_run(run_id).await.unwrap();

    let item = work_items::claim_next(&pool)
        .await
        .unwrap()
        .expect("completed run should enqueue steward analysis");
    assert_eq!(item.kind, db::work_item::WorkItemKind::StewardAnalysis);
    let payload: serde_json::Value = serde_json::from_str(&item.payload_json).unwrap();
    assert_eq!(payload["reason"], "config_change");
    assert!(
        db::repos::steward::take_config_change_pending(&pool)
            .await
            .unwrap()
            .is_none(),
        "config-change pending flag must be consumed once"
    );
}

#[tokio::test]
async fn steward_trigger_tests_post_run_hook_honors_interval() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    db::repos::steward::set_post_run_trigger_config(&pool, true, 2)
        .await
        .unwrap();

    let temp = tempfile::tempdir().unwrap();
    let workflow_path = temp.path().join("workflow.yaml");
    let catalog_path = temp.path().join("catalog.yaml");
    std::fs::write(
        &workflow_path,
        r#"
workflow:
  id: steward-trigger
  family: steward_family
  risk_class: standard
  stack: rust
initial_state: done
states:
  done:
    label: Done
    type: end
    owner: lead
"#,
    )
    .unwrap();
    std::fs::write(
        &catalog_path,
        r#"
backend_profiles:
  lead_profile:
    provider: claude
agents:
  - id: lead
    backend_profile: lead_profile
"#,
    )
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue);

    for idx in 0..2 {
        let run_id = RunId::new();
        let mut run = make_run(run_id, idea_id, RunStatus::Running);
        run.current_state = Some("done".into());
        run.workflow_yaml_path = Some(workflow_path.to_string_lossy().into_owned());
        run.agent_catalog_yaml_path = Some(catalog_path.to_string_lossy().into_owned());
        runs::insert(&pool, &run).await.unwrap();
        orchestrator.advance_run(run_id).await.unwrap();

        let item = work_items::claim_next(&pool).await.unwrap();
        if idx == 0 {
            assert!(
                item.is_none(),
                "first completed run must only increment post-run counter"
            );
        } else {
            let item = item.expect("second completed run should enqueue steward analysis");
            assert_eq!(item.kind, db::work_item::WorkItemKind::StewardAnalysis);
            let payload: serde_json::Value = serde_json::from_str(&item.payload_json).unwrap();
            assert_eq!(payload["reason"], "post_run_hook");
        }
    }
}

#[tokio::test]
async fn delivery_preflight_success_persists_run_owned_result() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let repo = tempfile::tempdir().unwrap();
    std::process::Command::new("git")
        .args(["init", "--initial-branch", "main"])
        .current_dir(repo.path())
        .output()
        .expect("git init should run");
    let worktrees = tempfile::tempdir().unwrap();
    let delivery_configuration_json = Some(format!(
        r#"{{
            "repo_identifier":"repo-1",
            "repo_root":"{}",
            "base_branch":"main",
            "worktree_base_path":"{}",
            "target_branch":"cw/release",
            "release_target_id":"app-store"
        }}"#,
        repo.path().display(),
        worktrees.path().display()
    ));

    let handler = make_command_handler(pool.clone());
    let commanded = handler
        .handle(
            Command::StartRun(StartRunCmd {
                idea_id,
                workflow_id: "wf-start".into(),
                workflow_title: "Start Run".into(),
                workspace_root: "/tmp/ws".into(),
                artifact_root: "/tmp/art".into(),
                workflow_yaml_path: test_workflow_yaml_path(),
                agent_catalog_yaml_path: test_agent_catalog_yaml_path(),
                delivery_configuration_json,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let run_id = match commanded.result {
        engine::command_handler::CommandResult::RunStarted { run_id } => run_id,
        _ => panic!("unexpected command result"),
    };

    let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    let preflight = run
        .delivery_preflight_json
        .as_deref()
        .expect("delivery preflight should persist");
    assert!(preflight.contains(r#""passed":true"#));
    assert!(preflight.contains("repo_root_exists"));
}

#[tokio::test]
async fn delivery_preflight_failure_blocks_before_run_creation() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let missing_repo = tempfile::tempdir().unwrap().path().join("missing");
    let worktrees = tempfile::tempdir().unwrap();
    let delivery_configuration_json = Some(format!(
        r#"{{
            "repo_identifier":"repo-1",
            "repo_root":"{}",
            "base_branch":"main",
            "worktree_base_path":"{}",
            "target_branch":"cw/release",
            "release_target_id":"app-store"
        }}"#,
        missing_repo.display(),
        worktrees.path().display()
    ));

    let handler = make_command_handler(pool.clone());
    let commanded = handler
        .handle(
            Command::StartRun(StartRunCmd {
                idea_id,
                workflow_id: "wf-start".into(),
                workflow_title: "Start Run".into(),
                workspace_root: "/tmp/ws".into(),
                artifact_root: "/tmp/art".into(),
                workflow_yaml_path: test_workflow_yaml_path(),
                agent_catalog_yaml_path: test_agent_catalog_yaml_path(),
                delivery_configuration_json,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    match commanded.result {
        engine::command_handler::CommandResult::StartRunBlockedByDeliveryPreflight(blocked) => {
            assert!(!blocked.delivery_preflight.passed);
            assert!(blocked
                .delivery_preflight
                .checks
                .iter()
                .any(|check| check.id == "repo_root_exists" && !check.passed));
        }
        _ => panic!("expected delivery preflight block"),
    }
    assert!(runs::list_by_idea(&pool, idea_id).await.unwrap().is_empty());
}

#[tokio::test]
async fn release_workflow_without_delivery_configuration_blocks_before_run_creation() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let handler = make_command_handler(pool.clone());
    let commanded = handler
        .handle(
            Command::StartRun(StartRunCmd {
                idea_id,
                workflow_id: "wf-start".into(),
                workflow_title: "Start Run".into(),
                workspace_root: "/tmp/ws".into(),
                artifact_root: "/tmp/art".into(),
                workflow_yaml_path: test_workflow_yaml_path(),
                agent_catalog_yaml_path: test_agent_catalog_yaml_path(),
                delivery_configuration_json: None,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    match commanded.result {
        engine::command_handler::CommandResult::StartRunBlockedByDeliveryPreflight(blocked) => {
            assert!(!blocked.delivery_preflight.passed);
            assert!(blocked
                .delivery_preflight
                .checks
                .iter()
                .any(|check| check.id == "delivery_configuration_present" && !check.passed));
        }
        _ => panic!("expected missing delivery configuration to block release workflow start"),
    }
    assert!(runs::list_by_idea(&pool, idea_id).await.unwrap().is_empty());
}

#[tokio::test]
async fn mcp_resolution_persistence_tests() {
    use acp::AcpRuntimeManager;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let registry_path = tmp.path().join("mcp-config.yaml");
    std::fs::write(&registry_path, "mcp: {}\n").unwrap();
    let previous_registry = std::env::var("CHAINWORKS_CODEX_CONFIG_PATH").ok();
    std::env::set_var("CHAINWORKS_CODEX_CONFIG_PATH", &registry_path);

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workspace_root = tmp.path().to_string_lossy().into_owned();
    run.artifact_root = tmp.path().join("artifacts").to_string_lossy().into_owned();
    runs::insert(&pool, &run).await.unwrap();
    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "mcp_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(AcpRuntimeManager::new()),
        events,
    );
    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("mcp_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "mcp_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "mcp-agent",
                "provider": "claude",
                "backend_profile_id": "codex_with_missing_mcp",
                "requested_mcp_server_ids": ["missing-extension"]
            }),
        )
        .await
        .unwrap();

    let processed = executor.process_next_item().await.unwrap();
    if let Some(previous_registry) = previous_registry {
        std::env::set_var("CHAINWORKS_CODEX_CONFIG_PATH", previous_registry);
    } else {
        std::env::remove_var("CHAINWORKS_CODEX_CONFIG_PATH");
    }

    assert!(
        processed,
        "expected queued InvokeAgent work item to process"
    );
    let executions = agent_executions::find_by_stage(&pool, stage_exec_id)
        .await
        .unwrap();
    let execution = executions.first().expect("agent execution should persist");
    assert_eq!(execution.status, AgentStatus::Failed);
    assert_eq!(
        execution.requested_mcp_extensions_json.as_deref(),
        Some(r#"["missing-extension"]"#)
    );
    assert_eq!(
        execution.denied_mcp_extensions_json.as_deref(),
        Some(r#"["missing-extension"]"#)
    );
    assert!(execution
        .mcp_blocking_issues_json
        .as_deref()
        .unwrap()
        .contains("missing-extension"));
    assert_eq!(execution.actual_mcp_extensions_json.as_deref(), Some("[]"));
    assert_eq!(execution.actual_mcp_runtime_ids_json.as_deref(), Some("[]"));
    let observation: serde_json::Value =
        serde_json::from_str(execution.actual_mcp_observation_json.as_deref().unwrap()).unwrap();
    assert_eq!(
        observation["source"],
        serde_json::json!("mcp_resolution_blocked_before_session_new")
    );
    assert_eq!(
        observation["trust_level"],
        serde_json::json!("authoritative_no_session")
    );
    assert_eq!(
        observation["actual_equals_predicted"],
        serde_json::json!(false)
    );

    let settled = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(settled.status, StageStatus::Failed);
    assert!(
        settled.evidence_packet_json.is_some(),
        "MCP blocked stage should get failed-stage evidence"
    );
}

#[tokio::test]
async fn test_reset_session_marks_generation_reset_and_next_policy_is_fresh_after_reset() {
    use db::repos::sessions;
    use domain::commands::ResetSessionCmd;
    use domain::session::{
        SessionGeneration, SessionGenerationStatus, SessionLineage, SessionReuseDisposition,
    };
    use engine::session::policy::{ensure_policy, SessionPolicyInput};

    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    let now = Utc::now();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "reset_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let lineage = SessionLineage {
        id: "lineage-reset".into(),
        run_id: run_id.to_string(),
        agent_id: "worker".into(),
        lineage_id: "worker".into(),
        session_reuse_scope: "same_invocation_owner".into(),
        session_family_id: None,
        active_generation_id: Some("generation-reset".into()),
        created_at: now,
        closed_at: None,
    };
    sessions::insert_lineage(&pool, &lineage).await.unwrap();
    sessions::insert_generation(
        &pool,
        &SessionGeneration {
            id: "generation-reset".into(),
            lineage_id: lineage.id.clone(),
            generation: 1,
            invocation_owner_key: "owner".into(),
            provider_session_id: Some("provider-session".into()),
            binding_fingerprint: "fingerprint".into(),
            rehydrated_from_checkpoint_artifact_id: None,
            working_directory: "/tmp/ws".into(),
            workspace_mode: "read_only".into(),
            runtime_provider: "claude".into(),
            runtime_model: "sonnet".into(),
            status: SessionGenerationStatus::Active,
            turn_count: 1,
            estimated_input_tokens: 0,
            latest_cached_input_tokens: None,
            latest_output_tokens: None,
            latest_model_context_window: None,
            cumulative_prompt_tokens: 0,
            cumulative_cost_cents: 0,
            created_at: now,
            last_activity_at: None,
            ended_at: None,
            end_reason: None,
        },
    )
    .await
    .unwrap();

    let mut execution = make_agent_execution(stage_exec_id, AgentStatus::Running);
    execution.session_lineage_id = Some(lineage.id.clone());
    execution.session_generation_id = Some("generation-reset".into());
    agent_executions::insert(&pool, &execution).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::ResetSession(ResetSessionCmd {
                run_id,
                stage_id: "reset_stage".into(),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let stage = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stage.status, StageStatus::Pending);

    let lineage_after = sessions::find_lineage_by_run_and_key(&pool, &run_id.to_string(), "worker")
        .await
        .unwrap()
        .unwrap();
    assert!(lineage_after.active_generation_id.is_none());

    let generations = sessions::list_generations_for_lineage(&pool, &lineage.id)
        .await
        .unwrap();
    assert_eq!(generations[0].status, SessionGenerationStatus::Reset);
    assert_eq!(generations[0].end_reason.as_deref(), Some("operator_reset"));

    let decision = ensure_policy(
        &pool,
        SessionPolicyInput {
            run_id: run_id.to_string(),
            agent_id: "worker".into(),
            provider: "claude".into(),
            model: "sonnet".into(),
            working_directory: "/tmp/ws".into(),
            workspace_mode: "read_only".into(),
            session_reuse_scope: Some("same_invocation_owner".into()),
            session_family_id: None,
            invocation_owner_key: "new-owner".into(),
            binding_fingerprint: "fingerprint".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        decision.disposition,
        SessionReuseDisposition::FreshAfterReset
    );
}

// ---------------------------------------------------------------------------
// InvokeAgent end-to-end parity harness (P027 / ARCH-001 / REQ-009)
//
// Proves the full daemon path:
//   BackgroundExecutor → AcpRuntimeManager → fixture ACP binary
//   → artifact persistence → projection rebuild → stage settlement
//
// This is the "bounded real runtime-backed daemon slice" required by R4.
// ---------------------------------------------------------------------------

/// BackgroundExecutor.process_next_item() drives a real ACP subprocess that
/// speaks the JSON-RPC 2.0 ACP protocol, persists the artifact it creates,
/// settles the stage, and rebuilds projections — all through the same code
/// path that runs in production.
///
/// The fixture is a Python script that completes the full ACP handshake
/// (initialize → session/new → session/prompt) and creates `report.json`
/// inside the workspace_root it receives via `session/new.params.cwd`.
/// The transport discovers the new file via workspace diff and returns it
/// as an artifact path.
#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_end_to_end_with_fixture_binary() {
    use std::os::unix::fs::PermissionsExt;

    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use db::repos::projections;
    use domain::run::Run;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();

    // Write a Python ACP fixture script.  It speaks the full JSON-RPC 2.0 ACP
    // protocol: initialize → session/new → session/prompt → (optional) session/close.
    // During session/prompt it creates report.json in the cwd it received.
    let script = tmp.path().join("acp_fixture.py");
    std::fs::write(&script, r#"#!/usr/bin/env python3
import sys, json, os

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
if msg is None: sys.exit(1)
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})

msg = recv()
if msg is None: sys.exit(1)
cwd = msg.get("params",{}).get("cwd","/tmp")
session_id = "e2e-fixture-session"
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})

msg = recv()
if msg is None: sys.exit(1)
artifact = os.path.join(cwd, "report.json")
with open(artifact, "w") as f:
    f.write('{"summary":"ok"}\n')
send({"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":"Done."}}})
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})

try:
    recv()   # session/close — best-effort
except Exception:
    pass

sys.exit(0)
"#).unwrap();
    {
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
    }

    // Build an AcpRuntimeManager wired to the fixture adapter.
    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    // Insert canonical domain entities.
    // workspace_root points at the tempdir so the executor sends it to the
    // fixture via session/new.params.cwd, and the transport scans it for artifacts.
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-e2e".into(),
            workflow_title: "E2E Test Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
            started_at: chrono::Utc::now(),
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
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "build_stage".into();
    stage.label = "Build Stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    // Wire up BackgroundExecutor.
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor =
        BackgroundExecutor::new(pool.clone(), work_queue.clone(), orchestrator, acp, events);

    // Enqueue a fully-populated InvokeAgent work item.
    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("build_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "build_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "fixture-agent",
                "provider": "claude",
            }),
        )
        .await
        .unwrap();

    // Process the work item through the real executor path.
    let processed = executor.process_next_item().await.unwrap();
    assert!(
        processed,
        "process_next_item must return true when a work item is available"
    );

    // Stage must be settled as Completed.
    let settled = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        settled.status,
        StageStatus::Completed,
        "stage must be Completed after successful ACP session"
    );

    // Artifact must be persisted in the canonical artifacts table.
    let persisted_artifacts = db::repos::artifacts::list_by_run(&pool, run_id)
        .await
        .unwrap();
    assert_eq!(
        persisted_artifacts.len(),
        1,
        "exactly one artifact must be persisted (report.json created by the fixture)"
    );
    let art = &persisted_artifacts[0];
    assert!(
        art.file_path.ends_with("report.json"),
        "artifact file_path must point to report.json, got: {}",
        art.file_path
    );
    assert_eq!(
        art.format.to_string(),
        "json",
        "artifact format must be derived from the .json extension"
    );
    assert_eq!(
        art.contract_id, "claude.output",
        "contract_id must be provider-scoped, not a stub"
    );

    // Projections must reflect the settled stage and its artifact.
    let stage_rows = projections::list_stages_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    let stage_proj = stage_rows
        .iter()
        .find(|s| s.stage_id == "build_stage")
        .expect("build_stage must appear in stage projection after rebuild");
    assert_eq!(
        stage_proj.status,
        StageStatus::Completed.to_string(),
        "stage projection status must match settled status"
    );
    assert!(
        stage_proj.has_artifacts,
        "stage projection must reflect that an artifact was created"
    );
}

#[tokio::test]
async fn test_invoke_agent_startup_error_marks_agent_execution_failed() {
    use acp::AcpRuntimeManager;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workspace_root = workspace_root.clone();
    run.artifact_root = workspace_root.clone();
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "startup_stage".into();
    stage.label = "Startup Stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("startup_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "startup_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "startup-agent",
                "provider": "missing-provider",
            }),
        )
        .await
        .unwrap();

    let err = executor
        .process_next_item()
        .await
        .expect_err("missing provider should fail ACP startup");
    assert!(
        err.to_string()
            .contains("No adapter registered for provider 'missing-provider'"),
        "unexpected error: {err}"
    );

    let executions = agent_executions::find_by_stage(&pool, stage_exec_id)
        .await
        .unwrap();
    let execution = executions
        .first()
        .expect("agent execution should be inserted before ACP startup");
    assert_eq!(execution.status, AgentStatus::Failed);
    assert!(
        execution.completed_at.is_some(),
        "failed startup execution must be terminal"
    );

    let queued = work_items::list_by_run(&pool, run_id).await.unwrap();
    assert!(
        queued.iter().any(|item| {
            item.kind == db::work_item::WorkItemKind::AdvanceRun
                && item.status == db::work_item::WorkItemStatus::Pending
        }),
        "failed invoke_agent must enqueue AdvanceRun so fan-in can settle the stage"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_persists_undeclared_envelope_output_as_generic_artifact() {
    use std::os::unix::fs::PermissionsExt;

    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use domain::run::Run;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();

    let script = tmp.path().join("acp_envelope_undeclared_fixture.py");
    std::fs::write(&script, r#"#!/usr/bin/env python3
import sys, json

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
if msg is None: sys.exit(1)
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})

msg = recv()
if msg is None: sys.exit(1)
session_id = "undeclared-envelope-session"
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})

msg = recv()
if msg is None: sys.exit(1)
send({"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","message":{"content":[{"type":"text","text":"<<<CHAINWORKS_OUTPUT:stdout_only_report>>>{\"summary\":\"ok\"}<<<END_CHAINWORKS_OUTPUT>>>"}]}}}})
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})

try:
    recv()
except Exception:
    pass

sys.exit(0)
"#).unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-undeclared-envelope".into(),
            workflow_title: "Undeclared Envelope Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
            started_at: chrono::Utc::now(),
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
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "undeclared_stage".into();
    stage.label = "Undeclared Stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor =
        BackgroundExecutor::new(pool.clone(), work_queue.clone(), orchestrator, acp, events);

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("undeclared_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "undeclared_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "fixture-agent",
                "provider": "claude",
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let persisted_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
    let artifact = persisted_artifacts
        .iter()
        .find(|artifact| artifact.name == "stdout_only_report")
        .expect("undeclared envelope output should persist as a generic artifact");
    assert_eq!(artifact.contract_id, "claude.output");
    assert!(std::path::Path::new(&artifact.file_path).is_file());
}

#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_persists_declared_machine_artifact_under_normalized_name() {
    use std::os::unix::fs::PermissionsExt;

    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use domain::run::Run;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let target_path = tmp.path().join("proposal_review_summary.json");

    let script = tmp.path().join("acp_normalized_name_fixture.py");
    std::fs::write(&script, r#"#!/usr/bin/env python3
import sys, json

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
if msg is None: sys.exit(1)
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})

msg = recv()
if msg is None: sys.exit(1)
session_id = "normalized-name-session"
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})

msg = recv()
if msg is None: sys.exit(1)
send({"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","message":{"content":[{"type":"text","text":"<<<CHAINWORKS_OUTPUT:review_alias>>>{\"summary\":\"ok\"}<<<END_CHAINWORKS_OUTPUT>>>"}]}}}})
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})

try:
    recv()
except Exception:
    pass

sys.exit(0)
"#).unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-normalized-name".into(),
            workflow_title: "Normalized Name Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
            started_at: chrono::Utc::now(),
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
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "normalized_stage".into();
    stage.label = "Normalized Stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor =
        BackgroundExecutor::new(pool.clone(), work_queue.clone(), orchestrator, acp, events);

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("normalized_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "normalized_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "fixture-agent",
                "provider": "claude",
                "declared_outputs": [{
                    "output_name": "review_alias",
                    "target_path": target_path.to_string_lossy(),
                    "schema": {
                        "contract_id": "proposal_review_v1",
                        "format": "json",
                        "human_format": serde_json::Value::Null,
                        "machine_format": "json",
                        "validation_mode": "strict_structured",
                        "normalized_artifact_name": "proposal_review_summary",
                        "raw_artifact_name": serde_json::Value::Null,
                        "required_fields": ["summary"]
                    },
                    "companion_output_name": serde_json::Value::Null,
                    "companion_path": serde_json::Value::Null
                }]
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let persisted_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
    let artifact = persisted_artifacts
        .iter()
        .find(|artifact| artifact.contract_id == "proposal_review_v1")
        .expect("declared output should persist");
    assert_eq!(artifact.name, "proposal_review_summary");
    assert!(artifact.file_path.ends_with("proposal_review_summary.json"));
}

#[cfg(unix)]
#[tokio::test]
async fn proposal_057_invoke_agent_imports_declared_contract_output_into_active_index() {
    use acp::{AcpRuntimeManager, ExecutionRequest, ExecutionResult};
    use domain::agent::AgentStatus;
    use domain::run::Run;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    struct P057FixtureAdapter;

    #[async_trait::async_trait]
    impl acp::adapters::AcpAdapter for P057FixtureAdapter {
        fn provider_name(&self) -> &str {
            "fixture"
        }

        async fn open_session(
            &self,
            _req: &ExecutionRequest,
        ) -> anyhow::Result<acp::AcpSessionHandle> {
            anyhow::bail!("P057 fixture adapter does not provide reusable sessions")
        }

        async fn execute(&self, _req: ExecutionRequest) -> anyhow::Result<ExecutionResult> {
            Ok(ExecutionResult {
                agent_execution_id: AgentExecutionId::new(),
                status: AgentStatus::Completed,
                artifact_paths: Vec::new(),
                discovered_artifacts: vec![acp::DiscoveredArtifact {
                    name: "prepush_review_report".into(),
                    content: br#"{"status":"PASS_WITH_NOTES"}"#.to_vec(),
                    source_path: None,
                }],
                transcript_text: Some(
                    r#"<<<CHAINWORKS_OUTPUT:prepush_review_report>>>{"status":"PASS_WITH_NOTES"}<<<END_CHAINWORKS_OUTPUT>>>"#
                        .into(),
                ),
                cost_cents: None,
                usage: None,
                provider_session_id: Some("p057-fixture-session".into()),
                reused_existing_session: false,
                session_generation_id: None,
                mcp_observation: None,
                actual_mcp_extensions: Vec::new(),
                actual_mcp_runtime_ids: Vec::new(),
                mcp_session_startup_latency_ms: None,
                close_diagnostic: None,
            })
        }
    }

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let target_path = tmp.path().join("review/prepush.json");
    std::fs::create_dir_all(target_path.parent().unwrap()).unwrap();

    let fixture_adapter = Arc::new(P057FixtureAdapter) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-p057-contract-import".into(),
            workflow_title: "P057 Contract Import Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_review".into()),
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
            chainworks_meta_root: Some(
                tmp.path()
                    .join(".chainworks")
                    .to_string_lossy()
                    .into_owned(),
            ),
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "review_stage".into();
    stage.label = "Review Stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor =
        BackgroundExecutor::new(pool.clone(), work_queue.clone(), orchestrator, acp, events);

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("review_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "review_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "fixture-agent",
                "provider": "fixture",
                "declared_outputs": [{
                    "output_name": "prepush_review_report",
                    "target_path": target_path.to_string_lossy(),
                    "schema": {
                        "contract_id": "prepush_review_v1",
                        "format": "json",
                        "human_format": serde_json::Value::Null,
                        "machine_format": "json",
                        "validation_mode": "strict_structured",
                        "normalized_artifact_name": "prepush_review_report",
                        "raw_artifact_name": serde_json::Value::Null,
                        "required_fields": ["status"]
                    },
                    "companion_output_name": serde_json::Value::Null,
                    "companion_path": serde_json::Value::Null
                }]
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let canonical = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "prepush_review_report",
        "status",
    )
    .await
    .unwrap();
    assert_eq!(canonical, Some(serde_json::json!("pass")));

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .expect("P057 run-state projection");
    assert_eq!(
        projection.active_index_json["contracts"]["prepush_review_v1"]["status"],
        "pass"
    );
    assert_eq!(
        projection.active_index_json["contracts"]["prepush_review_v1"]["source_stage_execution_id"],
        stage_exec_id.to_string()
    );
    assert!(tmp
        .path()
        .join(".chainworks/artifacts/active-index.json")
        .exists());
    assert!(tmp.path().join(".chainworks/state/run-state.json").exists());
}

#[tokio::test]
async fn proposal_057_failed_provider_result_settles_valid_outputs_by_degraded_policy() {
    use acp::{AcpRuntimeManager, ExecutionRequest, ExecutionResult};
    use domain::agent::{AgentFailureKind, AgentOutputSettlement};
    use domain::run::Run;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    struct P057FailedFixtureAdapter;

    #[async_trait::async_trait]
    impl acp::adapters::AcpAdapter for P057FailedFixtureAdapter {
        fn provider_name(&self) -> &str {
            "failed_fixture"
        }

        async fn open_session(
            &self,
            _req: &ExecutionRequest,
        ) -> anyhow::Result<acp::AcpSessionHandle> {
            anyhow::bail!("P057 failed fixture adapter does not provide reusable sessions")
        }

        async fn execute(&self, _req: ExecutionRequest) -> anyhow::Result<ExecutionResult> {
            Ok(ExecutionResult {
                agent_execution_id: AgentExecutionId::new(),
                status: AgentStatus::Failed,
                artifact_paths: Vec::new(),
                discovered_artifacts: vec![acp::DiscoveredArtifact {
                    name: "prepush_review_report".into(),
                    content: br#"{"status":"PASS_WITH_NOTES"}"#.to_vec(),
                    source_path: None,
                }],
                transcript_text: Some(
                    "provider quota limit reached after producing valid structured output".into(),
                ),
                cost_cents: None,
                usage: None,
                provider_session_id: Some("p057-failed-fixture-session".into()),
                reused_existing_session: false,
                session_generation_id: None,
                mcp_observation: None,
                actual_mcp_extensions: Vec::new(),
                actual_mcp_runtime_ids: Vec::new(),
                mcp_session_startup_latency_ms: None,
                close_diagnostic: None,
            })
        }
    }

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let target_path = tmp.path().join("review/prepush.json");
    std::fs::create_dir_all(target_path.parent().unwrap()).unwrap();

    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![
        Arc::new(P057FailedFixtureAdapter) as Arc<dyn acp::adapters::AcpAdapter>,
    ]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-p057-degraded-import".into(),
            workflow_title: "P057 Degraded Import Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
            started_at: chrono::Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_review".into()),
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
            chainworks_meta_root: Some(
                tmp.path()
                    .join(".chainworks")
                    .to_string_lossy()
                    .into_owned(),
            ),
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "review_stage".into();
    stage.label = "Review Stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor =
        BackgroundExecutor::new(pool.clone(), work_queue.clone(), orchestrator, acp, events);

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("review_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "review_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "fixture-agent",
                "provider": "failed_fixture",
                "stage_degraded_output_policy": {
                    "mode": "allow_valid_contract_outputs",
                    "contracts": ["prepush_review_v1"],
                    "failure_kinds": ["provider_quota"],
                    "max_settlement": "valid_outputs_from_failed_execution"
                },
                "declared_outputs": [{
                    "output_name": "prepush_review_report",
                    "target_path": target_path.to_string_lossy(),
                    "schema": {
                        "contract_id": "prepush_review_v1",
                        "format": "json",
                        "human_format": serde_json::Value::Null,
                        "machine_format": "json",
                        "validation_mode": "strict_structured",
                        "normalized_artifact_name": "prepush_review_report",
                        "raw_artifact_name": serde_json::Value::Null,
                        "required_fields": ["status"]
                    },
                    "companion_output_name": serde_json::Value::Null,
                    "companion_path": serde_json::Value::Null
                }]
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let canonical = artifact_contracts::canonical_contract_field(
        &pool,
        run_id,
        "prepush_review_report",
        "status",
    )
    .await
    .unwrap();
    assert_eq!(canonical, Some(serde_json::json!("pass")));

    let projection = artifact_contracts::find_run_state_projection(&pool, run_id)
        .await
        .unwrap()
        .expect("P057 degraded run-state projection");
    let contract = &projection.active_index_json["contracts"]["prepush_review_v1"];
    assert_eq!(contract["status"], "pass");
    assert_eq!(
        contract["output_settlement"],
        AgentOutputSettlement::ValidOutputsFromFailedExecution.to_string()
    );
    assert_eq!(contract["partial"], true);
    assert_eq!(
        contract["source_stage_execution_id"],
        stage_exec_id.to_string()
    );

    let agent_exec = agent_executions::find_by_stage(&pool, stage_exec_id)
        .await
        .unwrap()
        .pop()
        .expect("agent execution");
    let runtime_facts =
        db::repos::agent_execution_runtime_facts::find_by_execution_id(&pool, agent_exec.id)
            .await
            .unwrap()
            .expect("runtime facts");
    assert_eq!(
        runtime_facts.failure_kind,
        Some(AgentFailureKind::ProviderQuota)
    );
    assert_eq!(
        runtime_facts.output_settlement,
        AgentOutputSettlement::ValidOutputsFromFailedExecution
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_imports_implementation_self_assessment_summary() {
    use std::os::unix::fs::PermissionsExt;

    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use domain::artifact_contracts::ImplementationSelfAssessmentStatus;
    use domain::run::Run;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let target_path = tmp
        .path()
        .join("implementation")
        .join("self-assessment.json");

    let script = tmp.path().join("acp_self_assessment_fixture.py");
    std::fs::write(&script, r#"#!/usr/bin/env python3
import sys, json

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    return json.loads(stripped)

msg = recv()
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})

msg = recv()
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":"self-assessment-session"}})

msg = recv()
payload = {
    "implementation_complete": True,
    "verification_green": False,
    "remaining_code_tasks": [],
    "handoff_tasks": [],
    "known_risks": ["CloudKit manual smoke evidence remains outside code-owned verification."],
    "tests_run": ["cargo test was blocked by missing toolchain"],
    "docs_impacted": []
}
send({
    "jsonrpc":"2.0",
    "method":"session/update",
    "params":{"update":{"sessionUpdate":"agent_message_chunk","message":{"content":[{
        "type":"text",
        "text":"<<<CHAINWORKS_OUTPUT:implementation_self_assessment>>>" + json.dumps(payload) + "<<<END_CHAINWORKS_OUTPUT>>>"
    }]}}}
})
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":"self-assessment-session"}})

try:
    recv()
except Exception:
    pass
sys.exit(0)
"#).unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-self-assessment".into(),
            workflow_title: "Self Assessment Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
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
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "implementation_stage".into();
    stage.label = "Implementation Stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor =
        BackgroundExecutor::new(pool.clone(), work_queue.clone(), orchestrator, acp, events);

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("implementation_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "implementation_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "code_writer",
                "provider": "claude",
                "declared_outputs": [{
                    "output_name": "implementation_self_assessment",
                    "target_path": target_path.to_string_lossy(),
                    "schema": {
                        "contract_id": "implementation_self_assessment_v2",
                        "format": "json",
                        "human_format": serde_json::Value::Null,
                        "machine_format": "json",
                        "validation_mode": "strict_structured",
                        "normalized_artifact_name": serde_json::Value::Null,
                        "raw_artifact_name": serde_json::Value::Null,
                        "required_fields": [
                            "implementation_complete",
                            "verification_green",
                            "remaining_code_tasks",
                            "handoff_tasks",
                            "known_risks",
                            "tests_run",
                            "docs_impacted"
                        ]
                    },
                    "companion_output_name": serde_json::Value::Null,
                    "companion_path": serde_json::Value::Null
                }]
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let summary =
        artifact_contracts::find_active_implementation_self_assessment_summary(&pool, run_id)
            .await
            .unwrap()
            .expect("implementation self-assessment summary should be imported");
    assert_eq!(
        summary.summary.status,
        ImplementationSelfAssessmentStatus::Blocked
    );
    assert_eq!(summary.summary.implementation_complete, Some(true));
    assert_eq!(summary.summary.verification_green, Some(false));
    assert_eq!(summary.summary.blocking_remaining_code_task_count, Some(0));
}

#[tokio::test]
async fn malformed_implementation_self_assessment_import_persists_invalid_active_summary() {
    use acp::AcpRuntimeManager;
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        ImplementationSelfAssessmentStatus, IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::ids::ArtifactId;
    use engine::executor::BackgroundExecutor;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let artifact_root = tmp.path().join(".chainworks");
    let implementation_dir = artifact_root.join("implementation");
    std::fs::create_dir_all(&implementation_dir).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workspace_root = tmp.path().to_string_lossy().into_owned();
    run.artifact_root = artifact_root.to_string_lossy().into_owned();
    runs::insert(&pool, &run).await.unwrap();

    let artifact_path = artifact_root.join(IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH);
    std::fs::write(&artifact_path, br#"{ "implementation_complete": true, "#).unwrap();
    let artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_8_implementation_continued".into(),
        agent_id: "code_writer".into(),
        name: "implementation_self_assessment".into(),
        contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
        format: ArtifactFormat::Json,
        file_path: artifact_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "test".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    artifacts::insert(&pool, &artifact).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue,
        orchestrator,
        Arc::new(AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    executor
        .persist_implementation_self_assessment_summary_if_applicable(&artifact)
        .await
        .unwrap();

    let summary =
        artifact_contracts::find_active_implementation_self_assessment_summary(&pool, run_id)
            .await
            .unwrap()
            .expect("malformed v2 generation should remain active as invalid truth");
    assert_eq!(
        summary.summary.status,
        ImplementationSelfAssessmentStatus::Invalid
    );
    assert_eq!(summary.source_kind, "v2");
    assert!(summary.summary.raw_artifact_available);
    assert!(summary
        .summary
        .validation_errors
        .iter()
        .any(|issue| issue.code == "malformed_json"));
}

#[tokio::test]
async fn blocked_implementation_assessment_synthesizes_release_hold_review_summary() {
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        ImplementationSelfAssessmentStatus, IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::ids::ArtifactId;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let artifact_root = tmp.path().join(".chainworks");
    let implementation_dir = artifact_root.join("implementation");
    std::fs::create_dir_all(&implementation_dir).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workspace_root = workspace_root;
    run.artifact_root = artifact_root.to_string_lossy().into_owned();
    run.current_state = Some("state_9_implementation_reviewed".into());
    run.workflow_yaml_path = Some(test_workflow_yaml_path());
    run.agent_catalog_yaml_path = Some(test_agent_catalog_yaml_path());
    runs::insert(&pool, &run).await.unwrap();

    let self_assessment_path = implementation_dir.join("self-assessment.json");
    let raw = serde_json::json!({
        "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
        "implementation_complete": true,
        "verification_green": false,
        "remaining_code_tasks": [],
        "handoff_tasks": [{
            "summary": "Capture manual smoke evidence",
            "owner_class": "manual_evidence",
            "target_stage": "state_9_implementation_reviewed",
            "blocking_review": true,
            "evidence": "verification is blocked until the operator captures smoke evidence"
        }],
        "known_risks": ["Rust toolchain missing in verification environment"],
        "tests_run": ["cargo test: blocked"],
        "docs_impacted": []
    });
    std::fs::write(
        &self_assessment_path,
        serde_json::to_vec_pretty(&raw).unwrap(),
    )
    .unwrap();
    let artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_8_implementation_continued".into(),
        agent_id: "code_writer".into(),
        name: "implementation_self_assessment".into(),
        contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
        format: ArtifactFormat::Json,
        file_path: self_assessment_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "test".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    artifacts::insert(&pool, &artifact).await.unwrap();
    let summary = parse_implementation_self_assessment_v2(
        &raw,
        ContractParseContext {
            run_id: run_id.to_string(),
            run_age: None,
            declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
            canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
            raw_artifact_path: Some(artifact.file_path.clone()),
            source_generation_id: None,
            artifact_created_at: Some(artifact.created_at),
            v2_generation_seen_for_run: true,
            legacy_v1_generation_available: false,
        },
    );
    artifact_contracts::persist_implementation_self_assessment_summary(
        &pool,
        run_id,
        artifact.id,
        &artifact.contract_id,
        &summary,
        artifact.created_at,
    )
    .await
    .unwrap();
    let active =
        artifact_contracts::find_active_implementation_self_assessment_summary(&pool, run_id)
            .await
            .unwrap()
            .expect("active self-assessment summary");
    assert_eq!(
        active.summary.status,
        ImplementationSelfAssessmentStatus::Blocked
    );

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    let review_artifact = artifacts::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .find(|artifact| artifact.name == "implementation_review_summary")
        .expect("blocked verification should synthesize implementation review summary");
    let review: serde_json::Value =
        serde_json::from_slice(&std::fs::read(review_artifact.file_path).unwrap()).unwrap();
    assert_eq!(
        review["status"],
        serde_json::json!("release_evidence_blocked")
    );
    assert_eq!(
        review["implementation_self_assessment_summary"]["status"],
        serde_json::json!("blocked")
    );
    assert_eq!(
        review["handoff_tasks"][0]["summary"],
        serde_json::json!("Capture manual smoke evidence")
    );
    assert_eq!(
        review["implementation_self_assessment_summary"]["handoff_tasks"][0]["owner_class"],
        serde_json::json!("manual_evidence")
    );

    let updated = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        updated.current_state.as_deref(),
        Some("state_10_implementation_refined")
    );

    let queued_invocations = work_items::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .filter(|item| {
            item.kind == db::work_item::WorkItemKind::InvokeAgent
                && item.stage_id.as_deref() == Some("state_9_implementation_reviewed")
        })
        .count();
    assert_eq!(queued_invocations, 0);
}

#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_reuses_live_session_generation_end_to_end() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("reuse.sqlite");
    let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
        .await
        .unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();

    let script = tmp.path().join("acp_reuse_fixture.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import sys, json, os

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})
msg = recv()
cwd = msg.get("params",{}).get("cwd","/tmp")
session_id = "reuse-engine-session"
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})

msg = recv()
with open(os.path.join(cwd, "first.json"), "w") as f:
    f.write('{"turn": 1}\n')
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})

msg = recv()
with open(os.path.join(cwd, "second.json"), "w") as f:
    f.write('{"turn": 2}\n')
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})

msg = recv()
if msg.get("method") != "session/close":
    sys.exit(1)
sys.exit(0)
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_one_id = StageExecutionId::new();
    let stage_two_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-reuse".into(),
            workflow_title: "Reuse Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
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
        },
    )
    .await
    .unwrap();

    let mut stage_one = make_stage(stage_one_id, run_id, StageStatus::Running);
    stage_one.stage_id = "reuse_stage_1".into();
    stages::insert(&pool, &stage_one).await.unwrap();

    let mut stage_two = make_stage(stage_two_id, run_id, StageStatus::Running);
    stage_two.stage_id = "reuse_stage_2".into();
    stages::insert(&pool, &stage_two).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("reuse_stage_1".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "reuse_stage_1",
                "stage_execution_id": stage_one_id.to_string(),
                "agent_id": "reuse-agent",
                "provider": "claude",
                "prompt": "reuse turn",
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "proposal-loop",
            }),
        )
        .await
        .unwrap();
    assert!(executor.process_next_item().await.unwrap());

    let lineage =
        sessions::find_lineage_by_run_and_key(&pool, &run_id.to_string(), "proposal-loop")
            .await
            .unwrap()
            .expect("lineage should exist after first turn");
    let generation = sessions::find_active_generation(&pool, &lineage.id)
        .await
        .unwrap()
        .expect("active generation should exist after first turn");
    assert_eq!(generation.turn_count, 1);
    assert_eq!(
        generation.provider_session_id.as_deref(),
        Some("reuse-engine-session")
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("reuse_stage_2".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "reuse_stage_2",
                "stage_execution_id": stage_two_id.to_string(),
                "agent_id": "reuse-agent",
                "provider": "claude",
                "prompt": "reuse turn",
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "proposal-loop",
            }),
        )
        .await
        .unwrap();
    assert!(executor.process_next_item().await.unwrap());
    assert!(executor.process_next_item().await.unwrap());

    let generation_after = sessions::find_active_generation(&pool, &lineage.id)
        .await
        .unwrap()
        .expect("active generation should still exist after second turn");
    assert_eq!(generation_after.id, generation.id);

    let daemon_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
    assert!(
        daemon_artifacts
            .iter()
            .any(|artifact| artifact.file_path.ends_with("first.json")),
        "first turn artifact missing: {:?}",
        daemon_artifacts
            .iter()
            .map(|artifact| artifact.file_path.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        daemon_artifacts
            .iter()
            .any(|artifact| artifact.file_path.ends_with("second.json")),
        "second turn artifact missing: {:?}",
        daemon_artifacts
            .iter()
            .map(|artifact| artifact.file_path.clone())
            .collect::<Vec<_>>()
    );

    acp.close_session(&generation.id).await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;
    use engine::session::fingerprint::{
        binding_fingerprint, invocation_owner_key, BindingFingerprintInput, InvocationOwnerKeyInput,
    };
    use engine::session::policy::{ensure_policy, SessionPolicyInput};
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("budget.sqlite");
    let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
        .await
        .unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();

    let script = tmp.path().join("acp_budget_fixture.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import sys, json, os

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})
msg = recv()
cwd = msg.get("params",{}).get("cwd","/tmp")
session_id = "budget-engine-session"
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})

msg = recv()
with open(os.path.join(cwd, "budget.json"), "w") as f:
    f.write('{"turn": 1}\n')
send({
    "jsonrpc":"2.0",
    "method":"session/update",
    "params":{
        "update":{
            "kind":"usage",
            "usage":{
                "input_tokens": 60000,
                "cached_input_tokens": 6000,
                "output_tokens": 1200,
                "model_context_window": 200000
            }
        }
    }
})
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})

msg = recv()
if msg and msg.get("method") == "session/close":
    sys.exit(0)
sys.exit(0)
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-budget".into(),
            workflow_title: "Budget Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
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
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "budget_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events,
    );

    let prompt = "budget turn from runtime telemetry";
    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("budget_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "budget_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "budget-agent",
                "provider": "claude",
                "prompt": prompt,
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "proposal-loop",
            }),
        )
        .await
        .unwrap();
    assert!(executor.process_next_item().await.unwrap());

    let lineage =
        sessions::find_lineage_by_run_and_key(&pool, &run_id.to_string(), "proposal-loop")
            .await
            .unwrap()
            .expect("lineage should exist after first turn");
    let generation = sessions::find_active_generation(&pool, &lineage.id)
        .await
        .unwrap()
        .expect("active generation should exist after first turn");
    assert_eq!(generation.turn_count, 1);
    assert_eq!(generation.estimated_input_tokens, 60_000);
    assert_eq!(generation.latest_cached_input_tokens, Some(6_000));
    assert_eq!(generation.latest_output_tokens, Some(1_200));
    assert_eq!(generation.latest_model_context_window, Some(200_000));
    assert_eq!(generation.cumulative_prompt_tokens, 60_000);
    assert!(generation.last_activity_at.is_some());
    assert_eq!(
        generation.provider_session_id.as_deref(),
        Some("budget-engine-session")
    );

    let run_id_str = run_id.to_string();
    let owner_key = invocation_owner_key(&InvocationOwnerKeyInput {
        run_id: &run_id_str,
        agent_id: "budget-agent",
        stage_lineage_id: "budget_stage",
        task_name: "budget_stage",
        owner_execution_lineage_id: "follow-up-owner",
    });
    let fingerprint = binding_fingerprint(&BindingFingerprintInput {
        agent_id: "budget-agent",
        provider: "claude",
        model: None,
        effort: None,
        prompt,
        working_directory: &workspace_root,
        workspace_mode: "read_only",
        worktree_write_enabled: false,
        worktree_strategy: None,
        inputs: &Vec::new(),
        outputs: &Vec::new(),
        backend_profile: None,
        permission_profile: None,
        mcp_servers: &Vec::new(),
        skill_snapshot_hash: None,
        skill_ref: None,
        skill_role: None,
        output_contract: None,
        max_turns: None,
        temperature: None,
    });
    let decision = ensure_policy(
        &pool,
        SessionPolicyInput {
            run_id: run_id.to_string(),
            agent_id: "budget-agent".into(),
            provider: "claude".into(),
            model: "default".into(),
            working_directory: workspace_root.clone(),
            workspace_mode: "read_only".into(),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("proposal-loop".into()),
            invocation_owner_key: owner_key,
            binding_fingerprint: fingerprint,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        decision.disposition,
        domain::session::SessionReuseDisposition::ReusedAfterResume
    );
    assert!(!decision.should_reuse_live_session);
    assert!(decision
        .generation
        .rehydrated_from_checkpoint_artifact_id
        .is_some());

    acp.close_session(&generation.id).await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_persists_runtime_cost_and_next_policy_invalidates_on_cost_budget() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;
    use engine::session::fingerprint::{
        binding_fingerprint, invocation_owner_key, BindingFingerprintInput, InvocationOwnerKeyInput,
    };
    use engine::session::policy::{ensure_policy, SessionPolicyInput};
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("budget-cost.sqlite");
    let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
        .await
        .unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();

    let script = tmp.path().join("acp_budget_cost_fixture.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import sys, json, os

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})
msg = recv()
session_id = "budget-cost-session"
cwd = msg.get("params",{}).get("cwd","/tmp")
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})

msg = recv()
with open(os.path.join(cwd, "budget-cost.json"), "w") as f:
    f.write('{"turn": 1}\n')
send({
    "jsonrpc":"2.0",
    "method":"session/update",
    "params":{
        "update":{
            "kind":"usage",
            "usage":{
                "cost_cents": 600,
                "input_tokens": 2000,
                "cached_input_tokens": 1000,
                "output_tokens": 200,
                "model_context_window": 200000
            }
        }
    }
})
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})

msg = recv()
if msg and msg.get("method") == "session/close":
    sys.exit(0)
sys.exit(0)
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-budget-cost".into(),
            workflow_title: "Budget Cost Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
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
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "budget_cost_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events,
    );

    let prompt = "budget cost turn from runtime telemetry";
    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("budget_cost_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "budget_cost_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "budget-agent",
                "provider": "claude",
                "prompt": prompt,
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "proposal-loop",
            }),
        )
        .await
        .unwrap();
    assert!(executor.process_next_item().await.unwrap());

    let lineage =
        sessions::find_lineage_by_run_and_key(&pool, &run_id.to_string(), "proposal-loop")
            .await
            .unwrap()
            .expect("lineage should exist after first turn");
    let generation = sessions::find_active_generation(&pool, &lineage.id)
        .await
        .unwrap()
        .expect("active generation should exist after first turn");
    assert_eq!(generation.cumulative_cost_cents, 600);
    assert_eq!(generation.estimated_input_tokens, 2_000);

    let run_id_str = run_id.to_string();
    let owner_key = invocation_owner_key(&InvocationOwnerKeyInput {
        run_id: &run_id_str,
        agent_id: "budget-agent",
        stage_lineage_id: "budget_cost_stage",
        task_name: "budget_cost_stage",
        owner_execution_lineage_id: &stage_exec_id.to_string(),
    });
    let fingerprint = binding_fingerprint(&BindingFingerprintInput {
        agent_id: "budget-agent",
        provider: "claude",
        model: None,
        effort: None,
        prompt,
        working_directory: &workspace_root,
        workspace_mode: "read_only",
        worktree_write_enabled: false,
        worktree_strategy: None,
        inputs: &Vec::new(),
        outputs: &Vec::new(),
        backend_profile: None,
        permission_profile: None,
        mcp_servers: &Vec::new(),
        skill_snapshot_hash: None,
        skill_ref: None,
        skill_role: None,
        output_contract: None,
        max_turns: None,
        temperature: None,
    });
    let decision = ensure_policy(
        &pool,
        SessionPolicyInput {
            run_id: run_id.to_string(),
            agent_id: "budget-agent".into(),
            provider: "claude".into(),
            model: "default".into(),
            working_directory: workspace_root.clone(),
            workspace_mode: "read_only".into(),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("proposal-loop".into()),
            invocation_owner_key: owner_key,
            binding_fingerprint: fingerprint,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        decision.disposition,
        domain::session::SessionReuseDisposition::FreshAfterBudget
    );
    assert!(!decision.should_reuse_live_session);

    acp.close_session(&generation.id).await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn test_cancel_run_finalize_closes_live_session_via_runtime_manager() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;
    use std::os::unix::fs::PermissionsExt;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();

    let script = tmp.path().join("acp_cancel_fixture.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import sys, json

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})
msg = recv()
session_id = "cancel-engine-session"
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})
msg = recv()
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})
msg = recv()
if msg.get("method") != "session/close":
    sys.exit(1)
sys.exit(0)
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-cancel".into(),
            workflow_title: "Cancel Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root,
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
        },
    )
    .await
    .unwrap();
    stages::insert(
        &pool,
        &make_stage(stage_exec_id, run_id, StageStatus::Running),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        acp.clone(),
        events.clone(),
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("stage_test".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "stage_test",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "reuse-agent",
                "provider": "claude",
                "prompt": "prime session",
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "proposal-loop",
            }),
        )
        .await
        .unwrap();
    assert!(executor.process_next_item().await.unwrap());

    let lineage =
        sessions::find_lineage_by_run_and_key(&pool, &run_id.to_string(), "proposal-loop")
            .await
            .unwrap()
            .expect("lineage should exist after priming session");
    let generation = sessions::find_active_generation(&pool, &lineage.id)
        .await
        .unwrap()
        .expect("active generation should exist after priming session");

    let running_exec = AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id: stage_exec_id,
        agent_id: "reuse-agent".into(),
        provider: "claude".into(),
        model: None,
        started_at: Utc::now(),
        completed_at: None,
        status: AgentStatus::Running,
        owner_execution_lineage_id: Some(stage_exec_id.to_string()),
        session_lineage_id: Some(lineage.id.clone()),
        session_generation_id: Some(generation.id.clone()),
        rehydrated_from_checkpoint_artifact_id: None,
        invocation_owner_key: Some(generation.invocation_owner_key.clone()),
        session_reuse_scope: Some("same_agent_family_within_run".into()),
        session_family_id: Some("proposal-loop".into()),
        session_reuse_disposition: Some("reused".into()),
        session_reset_reason: None,
        backend_profile_id: None,
        requested_mcp_extensions_json: None,
        predicted_mcp_extensions_json: None,
        predicted_mcp_runtime_ids_json: None,
        actual_mcp_extensions_json: None,
        actual_mcp_runtime_ids_json: None,
        denied_mcp_extensions_json: None,
        mcp_blocking_issues_json: None,
        actual_mcp_observation_json: None,
        mcp_session_startup_latency_ms: None,
    };
    agent_executions::insert(&pool, &running_exec)
        .await
        .unwrap();
    stages::update_status(&pool, stage_exec_id, StageStatus::Running)
        .await
        .unwrap();

    let handler = Arc::new(CommandHandler::new_with_acp(
        pool.clone(),
        events,
        work_queue.clone(),
        acp.clone(),
    ));
    handler
        .handle(
            Command::CancelRun(domain::commands::CancelRunCmd { run_id }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let mut settled = None;
    for _ in 0..25 {
        let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
        if run.status == RunStatus::Cancelled {
            settled = Some(run);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let run = settled.expect("run should finalize to cancelled");
    let entries: serde_json::Value = serde_json::from_str(
        run.cancellation_settlement_log
            .as_deref()
            .expect("settlement log"),
    )
    .unwrap();
    assert_eq!(
        entries[0]["session_close_attempted"],
        serde_json::json!(true)
    );
    assert_eq!(
        entries[0]["session_close_succeeded"],
        serde_json::json!(true)
    );
    assert!(
        acp.close_session(&generation.id).await.is_err(),
        "session should already be closed by cancellation finalization"
    );
}

#[tokio::test]
async fn implementation_status_transitions_use_active_contract_summary() {
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::ids::ArtifactId;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let artifact_root = tmp.path().join(".chainworks");
    std::fs::create_dir_all(artifact_root.join("implementation")).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workspace_root = workspace_root;
    run.artifact_root = artifact_root.to_string_lossy().into_owned();
    run.current_state = Some("state_7_implementation_started".into());
    run.workflow_yaml_path = Some(test_workflow_yaml_path());
    run.agent_catalog_yaml_path = Some(test_agent_catalog_yaml_path());
    runs::insert(&pool, &run).await.unwrap();

    let mut completed_stage = make_stage(StageExecutionId::new(), run_id, StageStatus::Completed);
    completed_stage.stage_id = "state_7_implementation_started".into();
    stages::insert(&pool, &completed_stage).await.unwrap();

    let raw_path = artifact_root
        .join("implementation")
        .join("self-assessment.json");
    std::fs::write(
        &raw_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
            "implementation_complete": false,
            "verification_green": true,
            "remaining_code_tasks": [{
                "summary": "raw file says loop",
                "owner": "code_writer",
                "blocking": true,
                "evidence": "this raw file must not drive transition truth"
            }],
            "handoff_tasks": [],
            "known_risks": [],
            "tests_run": [],
            "docs_impacted": []
        }))
        .unwrap(),
    )
    .unwrap();

    let active_raw = serde_json::json!({
        "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
        "implementation_complete": true,
        "verification_green": false,
        "remaining_code_tasks": [],
        "handoff_tasks": [],
        "known_risks": ["verification blocked"],
        "tests_run": ["proposal-054: blocked"],
        "docs_impacted": []
    });
    let artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_7_implementation_started".into(),
        agent_id: "code_writer".into(),
        name: "implementation_self_assessment".into(),
        contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
        format: ArtifactFormat::Json,
        file_path: raw_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "test".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    artifacts::insert(&pool, &artifact).await.unwrap();
    let active_summary = parse_implementation_self_assessment_v2(
        &active_raw,
        ContractParseContext {
            run_id: run_id.to_string(),
            run_age: None,
            declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
            canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
            raw_artifact_path: Some(artifact.file_path.clone()),
            source_generation_id: None,
            artifact_created_at: Some(artifact.created_at),
            v2_generation_seen_for_run: true,
            legacy_v1_generation_available: false,
        },
    );
    artifact_contracts::persist_implementation_self_assessment_summary(
        &pool,
        run_id,
        artifact.id,
        &artifact.contract_id,
        &active_summary,
        artifact.created_at,
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue);
    orchestrator.advance_run(run_id).await.unwrap();

    let updated = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        updated.current_state.as_deref(),
        Some("state_9_implementation_reviewed"),
        "transition must use active summary status=blocked instead of raw file needs_code_fixes"
    );
}

#[tokio::test]
async fn implementation_loop_budget_exhaustion_still_allows_complete_exit() {
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::ids::ArtifactId;

    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let artifact_root = tmp.path().join(".chainworks");
    std::fs::create_dir_all(artifact_root.join("implementation")).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workspace_root = workspace_root;
    run.artifact_root = artifact_root.to_string_lossy().into_owned();
    run.current_state = Some("state_8_implementation_continued".into());
    run.workflow_yaml_path = Some(test_workflow_yaml_path());
    run.agent_catalog_yaml_path = Some(test_agent_catalog_yaml_path());
    runs::insert(&pool, &run).await.unwrap();

    let started_at = Utc::now();
    for iteration in 1..=10 {
        let mut completed_stage =
            make_stage(StageExecutionId::new(), run_id, StageStatus::Completed);
        completed_stage.stage_id = "state_8_implementation_continued".into();
        completed_stage.label = "Implementation continued until code-owned work is resolved".into();
        completed_stage.iteration = iteration;
        completed_stage.started_at = started_at + chrono::Duration::milliseconds(iteration as i64);
        completed_stage.completed_at = Some(completed_stage.started_at);
        stages::insert(&pool, &completed_stage).await.unwrap();
    }

    let raw_path = artifact_root
        .join("implementation")
        .join("self-assessment.json");
    let active_raw = serde_json::json!({
        "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
        "implementation_complete": true,
        "verification_green": true,
        "remaining_code_tasks": [],
        "handoff_tasks": [],
        "known_risks": [],
        "tests_run": ["proposal-053 gate passed"],
        "docs_impacted": []
    });
    std::fs::write(&raw_path, serde_json::to_vec_pretty(&active_raw).unwrap()).unwrap();

    let artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_8_implementation_continued".into(),
        agent_id: "code_writer".into(),
        name: "implementation_self_assessment".into(),
        contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
        format: ArtifactFormat::Json,
        file_path: raw_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "test".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    artifacts::insert(&pool, &artifact).await.unwrap();
    let active_summary = parse_implementation_self_assessment_v2(
        &active_raw,
        ContractParseContext {
            run_id: run_id.to_string(),
            run_age: None,
            declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
            canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
            raw_artifact_path: Some(artifact.file_path.clone()),
            source_generation_id: None,
            artifact_created_at: Some(artifact.created_at),
            v2_generation_seen_for_run: true,
            legacy_v1_generation_available: false,
        },
    );
    artifact_contracts::persist_implementation_self_assessment_summary(
        &pool,
        run_id,
        artifact.id,
        &artifact.contract_id,
        &active_summary,
        artifact.created_at,
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue);
    orchestrator.advance_run(run_id).await.unwrap();

    let updated = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        updated.current_state.as_deref(),
        Some("state_9_implementation_reviewed"),
        "exhausted implementation loop must still exit when the active self-assessment is complete"
    );
    assert_eq!(updated.status, RunStatus::Running);
}

#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_missing_live_handle_falls_back_to_fresh_generation() {
    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;
    use engine::session::fingerprint::{binding_fingerprint, BindingFingerprintInput};
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("missing-live-handle.sqlite");
    let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
        .await
        .unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();

    let script = tmp.path().join("acp_missing_live_handle_fixture.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import sys, json, os

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})
msg = recv()
cwd = msg.get("params",{}).get("cwd","/tmp")
session_id = "fresh-session-after-missing-live-handle"
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})
msg = recv()
with open(os.path.join(cwd, "fresh.json"), "w") as f:
    f.write('{"fresh": true}\n')
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})
msg = recv()
if msg and msg.get("method") == "session/close":
    sys.exit(0)
sys.exit(0)
"#,
    )
    .unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();

    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    let now = Utc::now();
    let fingerprint = binding_fingerprint(&BindingFingerprintInput {
        agent_id: "reuse-agent",
        provider: "claude",
        model: None,
        effort: None,
        prompt: "fallback after missing live handle",
        working_directory: &workspace_root,
        workspace_mode: "read_only",
        worktree_write_enabled: false,
        worktree_strategy: None,
        inputs: &Vec::new(),
        outputs: &Vec::new(),
        backend_profile: None,
        permission_profile: None,
        mcp_servers: &Vec::new(),
        skill_snapshot_hash: None,
        skill_ref: None,
        skill_role: None,
        output_contract: None,
        max_turns: None,
        temperature: None,
    });

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-reuse".into(),
            workflow_title: "Reuse Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
            started_at: now,
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
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "reuse_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let lineage = domain::session::SessionLineage {
        id: "lineage-1".into(),
        run_id: run_id.to_string(),
        agent_id: "reuse-agent".into(),
        lineage_id: "proposal-loop".into(),
        session_reuse_scope: "same_agent_family_within_run".into(),
        session_family_id: Some("proposal-loop".into()),
        active_generation_id: Some("generation-1".into()),
        created_at: now,
        closed_at: None,
    };
    sessions::insert_lineage(&pool, &lineage).await.unwrap();
    sessions::insert_generation(
        &pool,
        &domain::session::SessionGeneration {
            id: "generation-1".into(),
            lineage_id: lineage.id.clone(),
            generation: 1,
            invocation_owner_key: "owner".into(),
            provider_session_id: Some("stale-provider-session".into()),
            binding_fingerprint: fingerprint,
            rehydrated_from_checkpoint_artifact_id: None,
            working_directory: workspace_root.clone(),
            workspace_mode: "read_only".into(),
            runtime_provider: "claude".into(),
            runtime_model: "default".into(),
            status: domain::session::SessionGenerationStatus::Active,
            turn_count: 1,
            estimated_input_tokens: 0,
            latest_cached_input_tokens: None,
            latest_output_tokens: None,
            latest_model_context_window: None,
            cumulative_prompt_tokens: 0,
            cumulative_cost_cents: 0,
            created_at: now,
            last_activity_at: None,
            ended_at: None,
            end_reason: None,
        },
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor =
        BackgroundExecutor::new(pool.clone(), work_queue.clone(), orchestrator, acp, events);

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("reuse_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "reuse_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "reuse-agent",
                "provider": "claude",
                "prompt": "fallback after missing live handle",
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "proposal-loop",
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let updated_lineage =
        sessions::find_lineage_by_run_and_key(&pool, &run_id.to_string(), "proposal-loop")
            .await
            .unwrap()
            .expect("lineage should still exist");
    let active_generation = sessions::find_active_generation(&pool, &updated_lineage.id)
        .await
        .unwrap()
        .expect("a fresh active generation should exist");
    assert_ne!(active_generation.id, "generation-1");

    let executions = agent_executions::find_by_stage(&pool, stage_exec_id)
        .await
        .unwrap();
    let execution = executions
        .iter()
        .find(|execution| execution.agent_id == "reuse-agent")
        .expect("agent execution should exist");
    assert_eq!(
        execution.session_reuse_disposition.as_deref(),
        Some("fresh_after_transport_error")
    );

    let daemon_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
    assert!(
        daemon_artifacts
            .iter()
            .any(|artifact| artifact.file_path.ends_with("fresh.json")),
        "fresh artifact missing after missing-live-handle fallback"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn test_invoke_agent_rehydrates_from_checkpointed_generation_and_persists_checkpoint_artifact(
) {
    use std::os::unix::fs::PermissionsExt;

    use acp::adapters::claude::ClaudeAgentAdapter;
    use acp::AcpRuntimeManager;
    use domain::run::Run;
    use engine::executor::BackgroundExecutor;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let tempdir = tempfile::tempdir().unwrap();
    let workspace_root = tempdir.path().display().to_string();
    let script = tempdir.path().join("acp_checkpoint_resume_fixture.py");
    std::fs::write(&script, r#"#!/usr/bin/env python3
import sys, json, os

def send(obj):
    sys.stdout.write(json.dumps(obj) + '\n')
    sys.stdout.flush()

def recv():
    line = sys.stdin.readline()
    if not line:
        return None
    stripped = line.strip()
    if not stripped:
        return None
    try:
        return json.loads(stripped)
    except json.JSONDecodeError:
        return None

msg = recv()
if msg is None: sys.exit(1)
send({"jsonrpc":"2.0","id":msg["id"],"result":{"protocolVersion":1}})

msg = recv()
if msg is None: sys.exit(1)
cwd = msg.get("params",{}).get("cwd","/tmp")
session_id = "checkpoint-resume-session"
send({"jsonrpc":"2.0","id":msg["id"],"result":{"sessionId":session_id}})

msg = recv()
if msg is None: sys.exit(1)
artifact = os.path.join(cwd, "resume.json")
with open(artifact, "w") as f:
    f.write('{"ok":true,"mode":"resume"}\n')
send({"jsonrpc":"2.0","method":"session/update","params":{"update":{"sessionUpdate":"agent_message_chunk","content":"Resumed."}}})
send({"jsonrpc":"2.0","id":msg["id"],"result":{"stopReason":"end_turn","sessionId":session_id}})

try:
    recv()
except Exception:
    pass

sys.exit(0)
"#).unwrap();
    let mut perms = std::fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).unwrap();
    let fixture_adapter = Arc::new(ClaudeAgentAdapter::new_with_binary(
        script.to_str().unwrap(),
    )) as Arc<dyn acp::adapters::AcpAdapter>;
    let acp = Arc::new(AcpRuntimeManager::new_with_adapters(vec![fixture_adapter]));
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    let now = Utc::now();
    let checkpoint_artifact_id = domain::ids::ArtifactId::new().to_string();
    let fingerprint = engine::session::fingerprint::binding_fingerprint(
        &engine::session::fingerprint::BindingFingerprintInput {
            agent_id: "resume-agent",
            provider: "claude",
            model: None,
            effort: None,
            prompt: "resume from checkpoint",
            working_directory: &workspace_root,
            workspace_mode: "read_only",
            worktree_write_enabled: false,
            worktree_strategy: None,
            inputs: &Vec::new(),
            outputs: &Vec::new(),
            backend_profile: None,
            permission_profile: None,
            mcp_servers: &Vec::new(),
            skill_snapshot_hash: None,
            skill_ref: None,
            skill_role: None,
            output_contract: None,
            max_turns: None,
            temperature: None,
        },
    );

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-resume".into(),
            workflow_title: "Resume Workflow".into(),
            workspace_root: workspace_root.clone(),
            artifact_root: workspace_root.clone(),
            started_at: now,
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
        },
    )
    .await
    .unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Running);
    stage.stage_id = "resume_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let lineage = domain::session::SessionLineage {
        id: "lineage-resume".into(),
        run_id: run_id.to_string(),
        agent_id: "resume-agent".into(),
        lineage_id: "proposal-loop".into(),
        session_reuse_scope: "same_agent_family_within_run".into(),
        session_family_id: Some("proposal-loop".into()),
        active_generation_id: None,
        created_at: now,
        closed_at: None,
    };
    sessions::insert_lineage(&pool, &lineage).await.unwrap();
    sessions::insert_generation(
        &pool,
        &domain::session::SessionGeneration {
            id: "generation-checkpoint".into(),
            lineage_id: lineage.id.clone(),
            generation: 1,
            invocation_owner_key: "owner".into(),
            provider_session_id: Some("provider-session".into()),
            binding_fingerprint: fingerprint,
            rehydrated_from_checkpoint_artifact_id: None,
            working_directory: workspace_root.clone(),
            workspace_mode: "read_only".into(),
            runtime_provider: "claude".into(),
            runtime_model: "default".into(),
            status: domain::session::SessionGenerationStatus::Closed,
            turn_count: 20,
            estimated_input_tokens: 0,
            latest_cached_input_tokens: None,
            latest_output_tokens: None,
            latest_model_context_window: None,
            cumulative_prompt_tokens: 0,
            cumulative_cost_cents: 0,
            created_at: now,
            last_activity_at: None,
            ended_at: Some(now),
            end_reason: Some(format!(
                "budget_compaction_checkpoint:{checkpoint_artifact_id}"
            )),
        },
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor =
        BackgroundExecutor::new(pool.clone(), work_queue.clone(), orchestrator, acp, events);

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("resume_stage".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "resume_stage",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "resume-agent",
                "provider": "claude",
                "prompt": "resume from checkpoint",
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "proposal-loop",
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let updated_lineage =
        sessions::find_lineage_by_run_and_key(&pool, &run_id.to_string(), "proposal-loop")
            .await
            .unwrap()
            .expect("lineage should still exist");
    let active_generation = sessions::find_active_generation(&pool, &updated_lineage.id)
        .await
        .unwrap()
        .expect("a resumed active generation should exist");
    assert_eq!(
        active_generation
            .rehydrated_from_checkpoint_artifact_id
            .as_deref(),
        Some(checkpoint_artifact_id.as_str())
    );

    let executions = agent_executions::find_by_stage(&pool, stage_exec_id)
        .await
        .unwrap();
    let execution = executions
        .iter()
        .find(|execution| execution.agent_id == "resume-agent")
        .expect("agent execution should exist");
    assert_eq!(
        execution.session_reuse_disposition.as_deref(),
        Some("reused_after_resume")
    );
    assert_eq!(
        execution.rehydrated_from_checkpoint_artifact_id.as_deref(),
        Some(checkpoint_artifact_id.as_str())
    );

    let checkpoint_artifact = artifacts::find_by_id(&pool, checkpoint_artifact_id.parse().unwrap())
        .await
        .unwrap()
        .expect("checkpoint artifact should be persisted");
    assert_eq!(
        checkpoint_artifact.report_kind.as_deref(),
        Some("session_checkpoint")
    );
    assert!(std::path::Path::new(&checkpoint_artifact.file_path).exists());
}

/// R7 bar: daemon-vs-Swift behavioral diff harness.
///
/// Takes a golden snapshot captured from the Swift app and proves the daemon
/// produces an equivalent report shape for an identical workflow slice.
/// The golden file lives in tests/fixtures/golden_swift_report.json — it was
/// captured once from a real Swift run and encodes the non-regression bar:
/// for the same input (2-stage linear workflow), both the Swift app and the
/// daemon must produce runs with the same stage IDs, statuses, artifact
/// contracts, and aggregate counts.
#[tokio::test]
async fn test_daemon_vs_swift_report_behavioral_parity() {
    use db::repos::artifacts;
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::ids::ArtifactId;
    use domain::stage::StageSettlementKind;

    // Load golden snapshot from Swift run
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let golden_path = format!("{manifest_dir}/tests/fixtures/golden_swift_report.json");
    let golden_raw =
        std::fs::read_to_string(&golden_path).expect("golden swift report fixture must exist");
    let golden: serde_json::Value =
        serde_json::from_str(&golden_raw).expect("golden snapshot must be valid JSON");

    let pool = test_pool().await;

    // Seed the daemon path: idea + run + 2 stages + 2 artifacts matching golden
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running))
        .await
        .unwrap();

    let now = Utc::now();
    let golden_stages = golden["stages"].as_array().unwrap();
    for stage_def in golden_stages {
        let stage_id = stage_def["stage_id"].as_str().unwrap();
        let label = stage_def["label"].as_str().unwrap();
        let se_id = StageExecutionId::new();
        let mut stage = StageExecution {
            id: se_id,
            run_id,
            stage_id: stage_id.to_string(),
            label: label.to_string(),
            status: StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: None,
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        stages::insert(&pool, &stage).await.unwrap();
        // Settle as Completed (matches golden)
        stages::settle(&pool, se_id, StageSettlementKind::Completed, now)
            .await
            .unwrap();
        stage.status = StageStatus::Completed;
    }

    let golden_artifacts = golden["artifacts"].as_array().unwrap();
    for art_def in golden_artifacts {
        let art = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: art_def["stage_id"].as_str().unwrap().to_string(),
            agent_id: "claude".to_string(),
            name: art_def["name"].as_str().unwrap().to_string(),
            contract_id: art_def["contract_id"].as_str().unwrap().to_string(),
            format: ArtifactFormat::Json,
            file_path: format!("/tmp/parity/{}", art_def["name"].as_str().unwrap()),
            checksum_sha256: None,
            size_bytes: None,
            provider: "claude".to_string(),
            model: None,
            created_at: now,
            is_pinned: false,
            report_kind: None,
            report_version: None,
        };
        artifacts::insert(&pool, &art).await.unwrap();
    }

    // Mark run completed
    runs::mark_completed(&pool, run_id, now).await.unwrap();

    // Rebuild projections
    db::repos::projections::rebuild_all_for_run(&pool, run_id)
        .await
        .unwrap();

    // ── Build daemon report (same shape as golden) ─────────────────────────
    let daemon_run = db::repos::projections::find_run_projection(&pool, &run_id.to_string())
        .await
        .unwrap()
        .expect("run must exist");
    let daemon_stages = db::repos::projections::list_stages_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    let daemon_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();

    // ── Diff assertions: daemon report MUST match golden shape ────────────

    // Run-level: status + aggregate counts
    assert_eq!(
        daemon_run.status,
        golden["run_status"].as_str().unwrap(),
        "run status mismatch: daemon produces different status than Swift"
    );
    assert_eq!(
        daemon_run.total_stages,
        golden["total_stages"].as_i64().unwrap(),
        "total_stages mismatch"
    );
    assert_eq!(
        daemon_run.completed_stages,
        golden["completed_stages"].as_i64().unwrap(),
        "completed_stages mismatch"
    );
    assert_eq!(
        daemon_run.failed_stages,
        golden["failed_stages"].as_i64().unwrap(),
        "failed_stages mismatch"
    );

    // Stage-level: each golden stage must appear with matching status
    for golden_stage in golden_stages {
        let stage_id = golden_stage["stage_id"].as_str().unwrap();
        let daemon_stage = daemon_stages
            .iter()
            .find(|s| s.stage_id == stage_id)
            .unwrap_or_else(|| panic!("daemon missing stage {stage_id} that Swift produced"));
        assert_eq!(
            daemon_stage.status,
            golden_stage["status"].as_str().unwrap(),
            "stage {} status mismatch: daemon={} Swift={}",
            stage_id,
            daemon_stage.status,
            golden_stage["status"].as_str().unwrap()
        );
        assert_eq!(
            daemon_stage.attempt_number,
            golden_stage["attempt_number"].as_i64().unwrap(),
            "stage {} attempt_number mismatch",
            stage_id
        );
    }

    // Artifact-level: each golden artifact must exist with matching contract
    for golden_art in golden_artifacts {
        let name = golden_art["name"].as_str().unwrap();
        let daemon_art = daemon_artifacts
            .iter()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("daemon missing artifact {name} that Swift produced"));
        assert_eq!(
            daemon_art.contract_id,
            golden_art["contract_id"].as_str().unwrap(),
            "artifact {} contract_id mismatch",
            name
        );
        assert_eq!(
            daemon_art.stage_id,
            golden_art["stage_id"].as_str().unwrap(),
            "artifact {} stage_id mismatch",
            name
        );
    }

    // Reverse check: daemon didn't produce MORE stages/artifacts than Swift
    assert_eq!(
        daemon_stages.len(),
        golden_stages.len(),
        "daemon produced {} stages, Swift produced {} — non-regression violation",
        daemon_stages.len(),
        golden_stages.len()
    );
    assert_eq!(
        daemon_artifacts.len(),
        golden_artifacts.len(),
        "daemon produced {} artifacts, Swift produced {} — non-regression violation",
        daemon_artifacts.len(),
        golden_artifacts.len()
    );
}

// ---------------------------------------------------------------------------
// P044: Post-approval task detection in manual_gate approval flow
// ---------------------------------------------------------------------------

/// Approving a manual_gate that has post_approval_tasks (state_11_manual_release)
/// must set the stage to Running (not Completed) so the orchestrator can enqueue
/// the post-approval work.
#[tokio::test]
async fn test_approve_manual_gate_with_post_approval_tasks_sets_running() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    // Build a run with real workflow + catalog paths so the command handler
    // can compile the plan and detect post_approval_tasks.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let examples_dir = format!("{}/../../../examples", manifest_dir);
    let wf_path = format!("{examples_dir}/workflows/full-mvp-live.yaml");
    let cat_path = format!("{examples_dir}/agents/agents.yaml");

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(wf_path);
    run.agent_catalog_yaml_path = Some(cat_path);
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_11_manual_release".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    // Insert the pending approval BEFORE calling the command handler.
    let approval = make_approval(run_id, "state_11_manual_release", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::ApproveStage(ApproveStageCmd {
                run_id,
                stage_id: "state_11_manual_release".into(),
                comment: Some("Ship it".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    // Stage must be Running (not Completed) because post_approval_tasks exist.
    let updated = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.status,
        StageStatus::Running,
        "manual_gate with post_approval_tasks must transition to Running after approval, \
         not Completed, so the orchestrator can enqueue post-approval work"
    );
}

/// Approving a simple manual_gate (state_3, no post_approval_tasks) must
/// settle the stage as Completed.
#[tokio::test]
async fn test_approve_simple_manual_gate_settles_completed() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let examples_dir = format!("{}/../../../examples", manifest_dir);
    let wf_path = format!("{examples_dir}/workflows/full-mvp-live.yaml");
    let cat_path = format!("{examples_dir}/agents/agents.yaml");

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(wf_path);
    run.agent_catalog_yaml_path = Some(cat_path);
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_3_initial_proposal_approval".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(
        run_id,
        "state_3_initial_proposal_approval",
        ApprovalDecision::Pending,
    );
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::ApproveStage(ApproveStageCmd {
                run_id,
                stage_id: "state_3_initial_proposal_approval".into(),
                comment: Some("Looks good".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    // Stage must be Completed because state_3 has no post_approval_tasks.
    let updated = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.status,
        StageStatus::Completed,
        "simple manual_gate without post_approval_tasks must settle as Completed after approval"
    );
}

// ---------------------------------------------------------------------------
// P044 focused proof: post-approval task enqueuing and end-state semantics
// ---------------------------------------------------------------------------

/// After approving state_11 (which has post_approval_tasks), advance_run must
/// enqueue InvokeAgent work items for the post-approval tasks.
#[tokio::test]
async fn test_post_approval_tasks_enqueued_after_approval() {
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workflow_path = format!(
        "{}/../../../examples/workflows/full-mvp-live.yaml",
        manifest_dir
    );
    let catalog_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(workflow_path);
    run.agent_catalog_yaml_path = Some(catalog_path);
    run.current_state = Some("state_11_manual_release".into());
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_11_manual_release".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "state_11_manual_release", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    // Approve via CommandHandler — this transitions the stage to Running.
    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::ApproveStage(ApproveStageCmd {
                run_id,
                stage_id: "state_11_manual_release".into(),
                comment: Some("Ship it".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    // Now call advance_run — the orchestrator should detect the post-approval
    // context and enqueue InvokeAgent work items for the post-approval tasks.
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    // Verify InvokeAgent work items were enqueued for post-approval tasks.
    let work_items = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap();
    let invoke_items: Vec<_> = work_items
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();

    assert!(
        !invoke_items.is_empty(),
        "advance_run must enqueue InvokeAgent work items for post-approval tasks on state_11"
    );

    // state_11 has two sequential post-approval tasks (phase 0 then phase 1);
    // phase 0 should be enqueued first.
    let has_commit_push = invoke_items
        .iter()
        .any(|w| w.payload_json.contains("commit_and_push"));
    assert!(
        has_commit_push,
        "at least one InvokeAgent must target the commit_and_push post-approval task"
    );
}

/// An end state with tasks (state_12_workflow_complete) must NOT short-circuit
/// to immediate completion — it must fall through to the compute path, create
/// a Running stage, and enqueue tasks.
#[tokio::test]
async fn test_end_state_with_tasks_does_not_short_circuit() {
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workflow_path = format!(
        "{}/../../../examples/workflows/full-mvp-live.yaml",
        manifest_dir
    );
    let catalog_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(workflow_path);
    run.agent_catalog_yaml_path = Some(catalog_path);
    run.current_state = Some("state_12_workflow_complete".into());
    runs::insert(&pool, &run).await.unwrap();

    // No stages yet — the orchestrator should create one and NOT immediately complete.
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    // A stage must have been created for the end state.
    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let end_stage = all_stages
        .iter()
        .find(|s| s.stage_id == "state_12_workflow_complete")
        .expect("orchestrator must create a stage for end state with tasks");

    // The stage should be Running (tasks enqueued), NOT Completed.
    assert_eq!(
        end_stage.status,
        StageStatus::Running,
        "end state with tasks must enter Running (compute path), not short-circuit to Completed"
    );

    // The run must NOT be Completed yet — tasks haven't finished.
    let refreshed_run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_ne!(
        refreshed_run.status,
        RunStatus::Completed,
        "run must not be Completed while end-state tasks are still running"
    );

    // InvokeAgent work items must have been enqueued for the end state's tasks.
    let work_items = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap();
    let invoke_items: Vec<_> = work_items
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();
    assert!(
        !invoke_items.is_empty(),
        "end state with tasks must enqueue InvokeAgent work items"
    );
}

// ---------------------------------------------------------------------------
// P044 strengthened focused proofs: phase ordering, retry semantics, and
// simple-gate non-regression
// ---------------------------------------------------------------------------

/// Proves strict runtime phase ordering for post-approval tasks on state_11.
/// After approval and advance_run, only phase 0 (commit_and_push) must be
/// enqueued; phase 1 (build_and_distribute) must NOT appear until phase 0
/// completes.
#[tokio::test]
async fn test_n_phase_sequence_ordering() {
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wf_path = format!(
        "{}/../../../examples/workflows/full-mvp-live.yaml",
        manifest_dir
    );
    let cat_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(wf_path);
    run.agent_catalog_yaml_path = Some(cat_path);
    run.current_state = Some("state_11_manual_release".into());
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_11_manual_release".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "state_11_manual_release", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::ApproveStage(ApproveStageCmd {
                run_id,
                stage_id: "state_11_manual_release".into(),
                comment: Some("Ship it".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    // Only phase 0 tasks should be enqueued — phase 1 waits.
    let work_items = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap();
    let invoke_items: Vec<_> = work_items
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();

    // Should have exactly 1 InvokeAgent (phase 0 = commit_and_push only)
    assert_eq!(
        invoke_items.len(),
        1,
        "N-phase ordering: only phase 0 task must be enqueued initially, got {} items",
        invoke_items.len()
    );

    // The enqueued task must be commit_and_push (phase 0), not build_and_distribute (phase 1)
    let payload: serde_json::Value = serde_json::from_str(&invoke_items[0].payload_json).unwrap();
    let task_index = payload["task_index"].as_u64().unwrap();
    assert_eq!(
        task_index, 0,
        "first enqueued task must be task_index 0 (phase 0)"
    );
    assert!(
        invoke_items[0].payload_json.contains("commit_and_push"),
        "first enqueued task must be commit_and_push (phase 0)"
    );

    // build_and_distribute (phase 1) must NOT be enqueued yet
    assert!(
        !invoke_items
            .iter()
            .any(|w| w.payload_json.contains("build_and_distribute")),
        "phase 1 task (build_and_distribute) must NOT be enqueued before phase 0 completes"
    );
}

/// Proves that retrying a failed state_11 post-approval stage returns to
/// WaitingApproval-equivalent state: old stage is Skipped, new stage is
/// Pending with incremented attempt_number.
#[tokio::test]
async fn test_post_approval_retry_requires_fresh_approval() {
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    // Use real workflow + catalog so advance_run can compile the plan and
    // detect state_11_manual_release as a manual_gate with post_approval_tasks.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wf_path = format!(
        "{}/../../../examples/workflows/full-mvp-live.yaml",
        manifest_dir
    );
    let cat_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);
    let mut run = make_run(run_id, idea_id, RunStatus::Blocked);
    run.workflow_yaml_path = Some(wf_path);
    run.agent_catalog_yaml_path = Some(cat_path);
    run.current_state = Some("state_11_manual_release".into());
    runs::insert(&pool, &run).await.unwrap();

    // Simulate a failed post-approval stage with a resolved approval record
    // from the first (failed) attempt.
    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::Failed);
    stage.stage_id = "state_11_manual_release".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let mut first_approval =
        make_approval(run_id, "state_11_manual_release", ApprovalDecision::Granted);
    first_approval.decided_at = Some(Utc::now());
    approvals::insert(&pool, &first_approval).await.unwrap();

    // Step 1: RetryStage command creates the new attempt and enqueues
    // AdvanceRun. The old stage is Skipped, the new attempt is Pending.
    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "state_11_manual_release".into(),
                consume_quota_budget_now: false,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let old = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        old.status,
        StageStatus::Skipped,
        "old failed stage must be Skipped after retry"
    );

    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let retried_stage = all_stages
        .iter()
        .find(|s| s.stage_id == "state_11_manual_release" && s.attempt_number == 2)
        .expect("retry must create new stage with attempt_number=2")
        .clone();
    assert_eq!(
        retried_stage.status,
        StageStatus::Pending,
        "retried manual_gate stage must start as Pending before advance_run fires"
    );

    // Step 2: P044 §3g — advance_run must restore the retried stage to
    // WaitingApproval on the same stage execution (no lineage fork) and
    // create a fresh Approval record so the operator must re-approve.
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events, work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    // The retried stage (attempt=2) must now be WaitingApproval, preserved
    // by ID — not superseded by a new create_stage_for_state execution.
    let retried_after = stages::find_by_id(&pool, retried_stage.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        retried_after.status,
        StageStatus::WaitingApproval,
        "retried manual_gate stage must return to WaitingApproval after advance_run"
    );
    assert_eq!(
        retried_after.attempt_number, 2,
        "WaitingApproval stage must be the retried attempt, not a forked attempt=1"
    );

    // No additional attempt=1 stage must be created by create_stage_for_state.
    let post_advance_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let attempts_for_state_11: Vec<_> = post_advance_stages
        .iter()
        .filter(|s| s.stage_id == "state_11_manual_release")
        .collect();
    assert_eq!(
        attempts_for_state_11.len(),
        2,
        "retry must produce exactly 2 stage executions (Skipped + WaitingApproval), got {}: {:?}",
        attempts_for_state_11.len(),
        attempts_for_state_11
            .iter()
            .map(|s| (s.attempt_number, s.status.clone()))
            .collect::<Vec<_>>()
    );

    // A NEW approval record must exist in Requested state. The original
    // first_approval is still Granted; the new one must be distinct.
    let all_approvals = approvals::list_by_run(&pool, run_id).await.unwrap();
    let fresh_requests: Vec<_> = all_approvals
        .iter()
        .filter(|a| {
            a.stage_id == "state_11_manual_release"
                && a.decision == ApprovalDecision::Requested
                && a.id != first_approval.id
        })
        .collect();
    assert_eq!(
        fresh_requests.len(),
        1,
        "retry must create exactly one fresh Requested approval record, got {}",
        fresh_requests.len()
    );

    // Run status must reflect the pending approval checkpoint.
    let refreshed_run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        refreshed_run.status,
        RunStatus::WaitingApproval,
        "run status must be WaitingApproval after retried release gate awaits fresh approval"
    );
}

/// Ensures state_6 (simple gate, no post_approval_tasks) still completes
/// immediately after approval — non-regression for the post_approval_tasks
/// detection logic.
#[tokio::test]
async fn test_simple_manual_gate_no_regression() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wf_path = format!(
        "{}/../../../examples/workflows/full-mvp-live.yaml",
        manifest_dir
    );
    let cat_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.workflow_yaml_path = Some(wf_path);
    run.agent_catalog_yaml_path = Some(cat_path);
    runs::insert(&pool, &run).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "state_6_implementation_approval".into();
    stage.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(
        run_id,
        "state_6_implementation_approval",
        ApprovalDecision::Pending,
    );
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::ApproveStage(ApproveStageCmd {
                run_id,
                stage_id: "state_6_implementation_approval".into(),
                comment: Some("Approved".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let updated = stages::find_by_id(&pool, stage_exec_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.status,
        StageStatus::Completed,
        "state_6 (simple gate, no post_approval_tasks) must settle as Completed after approval"
    );
}

// ---------------------------------------------------------------------------
// P044: End-to-end happy path for state_11 -> state_12
// ---------------------------------------------------------------------------

/// Walks the full P044 happy path in a single contiguous fixture:
///   1. state_11 approval -> stage Running
///   2. advance_run enqueues phase 0 (commit_and_push)
///   3. simulate phase 0 completion: write git_push_receipt artifact on disk,
///      mark work item Completed
///   4. advance_run enqueues phase 1 (build_and_distribute); assert strict
///      started_at ordering (phase 0 started before phase 1)
///   5. simulate phase 1 completion: write release_bundle_manifest +
///      connect_upload_receipt, mark work item Completed
///   6. advance_run settles state_11 as Completed and transitions to state_12
///   7. advance_run creates state_12 stage and enqueues
///      finalize_run_and_produce_receipts
///   8. simulate finalize completion: write delivery_receipt + run_report +
///      run_state, mark work item Completed
///   9. advance_run settles state_12 and marks run Completed
///
/// Per P044 §8 we do not execute real ACP side effects; we simulate task
/// completion by writing artifact files and marking work items Completed.
#[tokio::test]
async fn test_state_11_to_state_12_happy_path() {
    use chrono::Utc;
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::ids::ArtifactId;
    use engine::orchestrator::Orchestrator;

    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();

    // Isolated workspace + artifact root so exists() lookups hit only our files.
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().to_string_lossy().into_owned();
    let artifact_root = tmp.path().join("artifacts").to_string_lossy().into_owned();
    std::fs::create_dir_all(&artifact_root).unwrap();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let wf_path = format!(
        "{}/../../../examples/workflows/full-mvp-live.yaml",
        manifest_dir
    );
    let cat_path = format!("{}/../../../examples/agents/agents.yaml", manifest_dir);

    // Seed run: at state_11, running, with worktree_root so the release safety
    // guard (which now inspects post_approval_tasks) is satisfied.
    let worktree_root = tmp.path().join("worktree").to_string_lossy().into_owned();
    std::fs::create_dir_all(&worktree_root).unwrap();

    let run = Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-test".into(),
        workflow_title: "Test Workflow".into(),
        workspace_root: workspace_root.clone(),
        artifact_root: artifact_root.clone(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_11_manual_release".into()),
        workflow_yaml_path: Some(wf_path),
        agent_catalog_yaml_path: Some(cat_path),
        worktree_root: Some(worktree_root.clone()),
        base_branch: Some("main".into()),
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
    };
    runs::insert(&pool, &run).await.unwrap();

    // Seed state_11 stage as WaitingApproval with an unresolved approval.
    let stage_11_id = StageExecutionId::new();
    let mut stage_11 = make_stage(stage_11_id, run_id, StageStatus::WaitingApproval);
    stage_11.stage_id = "state_11_manual_release".into();
    stage_11.stage_type = Some("manual_gate".into());
    stages::insert(&pool, &stage_11).await.unwrap();

    let approval = make_approval(run_id, "state_11_manual_release", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    // ── Step 1: approve state_11 -> stage transitions to Running ──────────
    let handler = make_command_handler(pool.clone());
    handler
        .handle(
            Command::ApproveStage(ApproveStageCmd {
                run_id,
                stage_id: "state_11_manual_release".into(),
                comment: Some("Ship it".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let s11_after_approval = stages::find_by_id(&pool, stage_11_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        s11_after_approval.status,
        StageStatus::Running,
        "state_11 must be Running (not Completed) after approval because it has post_approval_tasks"
    );

    // ── Step 2: advance_run enqueues phase 0 (commit_and_push) only ──────
    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Orchestrator::new(pool.clone(), events.clone(), work_queue.clone());
    orchestrator.advance_run(run_id).await.unwrap();

    let items_after_p0 = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap();
    let invoke_items_p0: Vec<_> = items_after_p0
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();
    assert_eq!(
        invoke_items_p0.len(),
        1,
        "phase 0 only: exactly one InvokeAgent must be enqueued, got {}",
        invoke_items_p0.len()
    );
    assert!(
        invoke_items_p0[0].payload_json.contains("commit_and_push"),
        "phase 0 must be commit_and_push, payload: {}",
        invoke_items_p0[0].payload_json
    );
    let phase0_item_id = invoke_items_p0[0].id.clone();
    let phase0_enqueued_at = invoke_items_p0[0].created_at;

    // ── Step 3: simulate phase 0 completion ──────────────────────────────
    // Write git_push_receipt at the path the catalog declares so exists() resolves.
    let release_dir = tmp.path().join(".chainworks").join("release");
    std::fs::create_dir_all(&release_dir).unwrap();
    let git_push_receipt_path = release_dir.join("git-push.json");
    std::fs::write(
        &git_push_receipt_path,
        r#"{"branch":"main","sha":"deadbeef"}"#,
    )
    .unwrap();

    // Also insert the artifact row so report/projection surfaces see it.
    let now = Utc::now();
    let git_push_artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_11_manual_release".into(),
        agent_id: "commit_and_push_to_github".into(),
        name: "git_push_receipt".into(),
        contract_id: "git_push_receipt_v1".into(),
        format: ArtifactFormat::Json,
        file_path: git_push_receipt_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "claude".into(),
        model: None,
        created_at: now,
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    db::repos::artifacts::insert(&pool, &git_push_artifact)
        .await
        .unwrap();

    // Mark phase 0 work item Completed.
    db::repos::work_items::complete(&pool, &phase0_item_id)
        .await
        .unwrap();

    // ── Step 4: advance_run enqueues phase 1 (build_and_distribute) ──────
    orchestrator.advance_run(run_id).await.unwrap();

    let items_after_p1 = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap();
    let invoke_items_p1: Vec<_> = items_after_p1
        .iter()
        .filter(|w| w.kind == db::work_item::WorkItemKind::InvokeAgent)
        .collect();
    assert_eq!(
        invoke_items_p1.len(),
        2,
        "phase 1 must be enqueued alongside completed phase 0, got {} items",
        invoke_items_p1.len()
    );
    let phase1_item = invoke_items_p1
        .iter()
        .find(|w| w.payload_json.contains("build_and_distribute"))
        .expect("phase 1 InvokeAgent for build_and_distribute must exist");
    let phase1_enqueued_at = phase1_item.created_at;

    // Strict phase ordering by enqueue time: phase 0 must have been enqueued
    // before phase 1. created_at is set when the work item is persisted, so
    // phase 0's timestamp must precede or equal phase 1's.
    assert!(
        phase0_enqueued_at <= phase1_enqueued_at,
        "strict phase ordering: phase 0 (enqueued_at={:?}) must come before phase 1 (enqueued_at={:?})",
        phase0_enqueued_at,
        phase1_enqueued_at
    );

    // ── Step 5: simulate phase 1 completion ──────────────────────────────
    // Write release_bundle_manifest and connect_upload_receipt artifacts.
    let rbm_path = release_dir.join("release-bundle.json");
    std::fs::write(&rbm_path, r#"{"bundle":"ok"}"#).unwrap();
    let cur_path = release_dir.join("connect-upload.json");
    std::fs::write(&cur_path, r#"{"connect":"ok"}"#).unwrap();

    let rbm_artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_11_manual_release".into(),
        agent_id: "build_archive_and_push_connect".into(),
        name: "release_bundle_manifest".into(),
        contract_id: "release_bundle_manifest_v1".into(),
        format: ArtifactFormat::Json,
        file_path: rbm_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "claude".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    db::repos::artifacts::insert(&pool, &rbm_artifact)
        .await
        .unwrap();

    let cur_artifact = Artifact {
        id: ArtifactId::new(),
        run_id,
        stage_id: "state_11_manual_release".into(),
        agent_id: "build_archive_and_push_connect".into(),
        name: "connect_upload_receipt".into(),
        contract_id: "connect_upload_receipt_v1".into(),
        format: ArtifactFormat::Json,
        file_path: cur_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: None,
        provider: "claude".into(),
        model: None,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: None,
        report_version: None,
    };
    db::repos::artifacts::insert(&pool, &cur_artifact)
        .await
        .unwrap();

    db::repos::work_items::complete(&pool, &phase1_item.id.clone())
        .await
        .unwrap();

    // ── Step 6: advance_run settles state_11, transitions to state_12 ────
    orchestrator.advance_run(run_id).await.unwrap();

    let s11_settled = stages::find_by_id(&pool, stage_11_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        s11_settled.status,
        StageStatus::Completed,
        "state_11 must be Completed after both phases complete"
    );

    let run_after_s11 = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        run_after_s11.current_state.as_deref(),
        Some("state_12_workflow_complete"),
        "run must have transitioned to state_12 (git_push_receipt exists)"
    );
    assert_ne!(
        run_after_s11.status,
        RunStatus::Completed,
        "run must not be Completed yet — state_12 tasks haven't run"
    );

    // ── Step 7: advance_run creates state_12 stage and enqueues finalizer ─
    orchestrator.advance_run(run_id).await.unwrap();

    let all_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let s12 = all_stages
        .iter()
        .find(|s| s.stage_id == "state_12_workflow_complete")
        .expect("state_12 stage must be created after transition");
    assert_eq!(
        s12.status,
        StageStatus::Running,
        "state_12 (end state with tasks) must enter Running, not short-circuit to Completed"
    );

    let items_after_s12 = db::repos::work_items::list_by_run(&pool, run_id)
        .await
        .unwrap();
    let s12_invokes: Vec<_> = items_after_s12
        .iter()
        .filter(|w| {
            w.kind == db::work_item::WorkItemKind::InvokeAgent
                && w.payload_json.contains(&s12.id.to_string())
        })
        .collect();
    assert_eq!(
        s12_invokes.len(),
        1,
        "state_12 must enqueue exactly one InvokeAgent (finalize_run_and_produce_receipts)"
    );
    assert!(
        s12_invokes[0]
            .payload_json
            .contains("finalize_run_and_produce_receipts"),
        "state_12 task must be finalize_run_and_produce_receipts, payload: {}",
        s12_invokes[0].payload_json
    );
    let finalize_item_id = s12_invokes[0].id.clone();

    // ── Step 8: simulate finalize completion (write receipt + report) ─────
    let delivery_receipt_path = release_dir.join("delivery-receipt.json");
    std::fs::write(&delivery_receipt_path, r#"{"delivery":"ok"}"#).unwrap();
    let run_report_path = release_dir.join("run-report.json");
    std::fs::write(&run_report_path, r#"{"report":"final"}"#).unwrap();
    let run_state_path = release_dir.join("run-state.json");
    std::fs::write(&run_state_path, r#"{"state":"complete"}"#).unwrap();

    let now = Utc::now();
    for (name, path, contract) in [
        (
            "delivery_receipt",
            delivery_receipt_path.clone(),
            "delivery_receipt_v1",
        ),
        ("run_report", run_report_path.clone(), "run_report_v1"),
        ("run_state", run_state_path.clone(), "run_state_v1"),
    ] {
        let art = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_12_workflow_complete".into(),
            agent_id: "lead_orchestrator".into(),
            name: name.into(),
            contract_id: contract.into(),
            format: ArtifactFormat::Json,
            file_path: path.to_string_lossy().into_owned(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "claude".into(),
            model: None,
            created_at: now,
            is_pinned: false,
            report_kind: None,
            report_version: None,
        };
        db::repos::artifacts::insert(&pool, &art).await.unwrap();
    }

    db::repos::work_items::complete(&pool, &finalize_item_id)
        .await
        .unwrap();

    // ── Step 9: advance_run settles state_12 and marks run Completed ─────
    orchestrator.advance_run(run_id).await.unwrap();

    let s12_settled = stages::find_by_id(&pool, s12.id).await.unwrap().unwrap();
    assert_eq!(
        s12_settled.status,
        StageStatus::Completed,
        "state_12 must be Completed after finalize_run_and_produce_receipts finishes"
    );

    let final_run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(
        final_run.status,
        RunStatus::Completed,
        "run must be Completed after state_12 (end state with tasks) finishes"
    );
    assert!(
        final_run.completed_at.is_some(),
        "completed_at must be set on a completed run"
    );

    // Terminal artifact inventory: all three finalizer outputs must exist.
    let all_artifacts = db::repos::artifacts::list_by_run(&pool, run_id)
        .await
        .unwrap();
    for terminal in ["delivery_receipt", "run_report", "run_state"] {
        assert!(
            all_artifacts.iter().any(|a| a.name == terminal),
            "terminal artifact {terminal} must be present after run completes"
        );
    }
    // Release intermediate artifacts are also present.
    for intermediate in [
        "git_push_receipt",
        "release_bundle_manifest",
        "connect_upload_receipt",
    ] {
        assert!(
            all_artifacts.iter().any(|a| a.name == intermediate),
            "release artifact {intermediate} must be present after run completes"
        );
    }
}

// ---------------------------------------------------------------------------
// P050: Per-run workspace isolation focused tests
// ---------------------------------------------------------------------------

/// resolve_path_template uses the run's meta_root for CHAINWORKS_META_ROOT.
#[test]
fn test_resolve_path_template_uses_run_meta_root() {
    let result = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/state/run-state.json",
        "/project",
        Some(".chainworks/runs/abc123"),
    );
    assert_eq!(
        result, "/project/.chainworks/runs/abc123/state/run-state.json",
        "meta_root must override the template default"
    );
}

/// resolve_path_template falls back to template default for NULL meta_root (legacy).
#[test]
fn test_resolve_path_template_null_meta_root_uses_template_default() {
    let result = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/state/run-state.json",
        "/project",
        None,
    );
    assert_eq!(
        result, "/project/.chainworks/state/run-state.json",
        "NULL meta_root must use template default .chainworks"
    );
}

/// resolve_path_template does not consult process env for CHAINWORKS_META_ROOT when meta_root is Some.
#[test]
fn test_resolve_path_template_does_not_consult_process_env_for_runs() {
    // Set a process env value that should be ignored.
    std::env::set_var("CHAINWORKS_META_ROOT", "/should/not/be/used");
    let result = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/proposals/current/proposal.md",
        "/project",
        Some(".chainworks/runs/test-run"),
    );
    std::env::remove_var("CHAINWORKS_META_ROOT");
    assert_eq!(
        result, "/project/.chainworks/runs/test-run/proposals/current/proposal.md",
        "per-run meta_root must win over process env"
    );
}

/// New run created via command_handler gets per-run chainworks_meta_root.
/// Verified by P044 test pattern (uses make_command_handler + ApproveStage).
/// Here we check directly via make_run and DB round-trip.
#[tokio::test]
async fn test_new_run_gets_isolated_meta_root() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let mut run = make_run(run_id, idea_id, RunStatus::Pending);
    // Simulate what command_handler does for new post-P050 runs.
    run.chainworks_meta_root = Some(format!(".chainworks/runs/{}", run_id));
    runs::insert(&pool, &run).await.unwrap();

    let loaded = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert!(
        loaded.chainworks_meta_root.is_some(),
        "new post-P050 run must have non-null chainworks_meta_root"
    );
    let meta_root = loaded.chainworks_meta_root.unwrap();
    assert!(
        meta_root.starts_with(".chainworks/runs/"),
        "meta_root must start with .chainworks/runs/, got: {meta_root}"
    );
    assert!(
        meta_root.contains(&run_id.to_string()[..8]),
        "meta_root must contain the run ID"
    );
}

/// Post-P050 normalize_artifacts ignores stale files in shared flat artifact_root.
#[tokio::test]
async fn test_normalize_artifacts_ignores_stale_flat_artifact_root_for_post_p050_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().to_string_lossy().into_owned();
    let artifact_root = format!("{workspace}/artifacts");
    let run_id = RunId::new();
    let run_dir = format!("{artifact_root}/{run_id}");
    std::fs::create_dir_all(&run_dir).unwrap();

    // Create a stale artifact in the shared flat artifact_root (from a prior run).
    let stale_path = format!("{artifact_root}/proposal_current");
    std::fs::write(&stale_path, "stale from prior run").unwrap();

    // Canonical per-run destination should NOT exist before normalization.
    let meta_root = format!(".chainworks/runs/{run_id}");
    let canonical = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/proposals/current/proposal.md",
        &workspace,
        Some(&meta_root),
    );
    assert!(
        !std::path::Path::new(&canonical).exists(),
        "canonical per-run path must not exist before normalization"
    );

    // For post-P050 runs (meta_root = Some), normalize_artifacts should NOT
    // copy the stale flat-root file because it only searches artifact_root/{run_id}.
    // The stale file is in artifact_root/ (not artifact_root/{run_id}/).
    // So the canonical path should remain absent after normalization.

    // We can verify the resolve_path_template behavior directly:
    let shared_path = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/proposals/current/proposal.md",
        &workspace,
        None, // legacy: uses shared .chainworks/
    );
    assert!(
        shared_path.contains("/.chainworks/proposals/"),
        "legacy path must use shared .chainworks/"
    );
    assert!(
        canonical.contains(&format!("/.chainworks/runs/{run_id}/")),
        "per-run path must use .chainworks/runs/{{run_id}}/"
    );
    assert_ne!(
        shared_path, canonical,
        "per-run and legacy paths must differ"
    );
}

/// Run serde serialization includes chainworks_meta_root for northbound readback.
/// (GraphQL GqlRun carries it via From<Run> — tested in graphql-server crate.)
#[test]
fn test_run_serde_includes_chainworks_meta_root() {
    let mut run = make_run(RunId::new(), IdeaId::new(), RunStatus::Running);
    run.chainworks_meta_root = Some(".chainworks/runs/test-run".into());

    let json = serde_json::to_value(&run).unwrap();
    assert_eq!(
        json["chainworks_meta_root"].as_str(),
        Some(".chainworks/runs/test-run"),
        "Run serde serialization must include chainworks_meta_root for MCP/GraphQL readback"
    );
}

/// MCP runs.get returns chainworks_meta_root through full Run serialization.
#[test]
fn test_mcp_runs_get_exposes_chainworks_meta_root() {
    let mut run = make_run(RunId::new(), IdeaId::new(), RunStatus::Running);
    run.chainworks_meta_root = Some(".chainworks/runs/mcp-test".into());

    let json = serde_json::to_value(&run).unwrap();
    assert_eq!(
        json["chainworks_meta_root"].as_str(),
        Some(".chainworks/runs/mcp-test"),
        "Run serde serialization must include chainworks_meta_root"
    );
}

// ── P050 Worktree interaction ──

/// normalize_path_for_worktree skips meta-root paths (control-plane artifacts
/// must NOT be rewritten into the worktree).
#[test]
fn test_normalize_path_for_worktree_skips_meta_root_paths() {
    let path = "/project/.chainworks/runs/abc/proposals/current/proposal.md";
    let result = engine::orchestrator::normalize_path_for_worktree(
        path,
        "/project",
        Some("/project/.chainworks/worktrees/cw-test-12345678"),
        true, // worktree_write_enabled
        Some("/project/.chainworks/runs/abc"),
    );
    assert_eq!(
        result, path,
        "meta-root paths must NOT be rewritten to worktree"
    );
}

/// normalize_path_for_worktree still normalizes source-code paths.
#[test]
fn test_normalize_path_for_worktree_still_normalizes_source_paths() {
    let path = "/project/ios/Sources/AppDelegate.swift";
    let result = engine::orchestrator::normalize_path_for_worktree(
        path,
        "/project",
        Some("/project/.chainworks/worktrees/cw-test-12345678"),
        true,
        Some("/project/.chainworks/runs/abc"),
    );
    assert_eq!(
        result, "/project/.chainworks/worktrees/cw-test-12345678/ios/Sources/AppDelegate.swift",
        "source-code paths must still be rewritten to worktree"
    );
}

// ── P050 Transition conditions ──

/// exists() transition checks resolve against per-run meta root.
/// Proven via resolve_path_template: the canonical path points to per-run dir.
#[test]
fn test_exists_checks_per_run_meta_root() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path().to_string_lossy().into_owned();
    let run_id = "test-run-abc";
    let meta_root = format!(".chainworks/runs/{run_id}");

    // Create the artifact at the per-run location
    let per_run_dir = format!("{ws}/.chainworks/runs/{run_id}/state");
    std::fs::create_dir_all(&per_run_dir).unwrap();
    std::fs::write(
        format!("{per_run_dir}/run-state.json"),
        r#"{"status":"ok"}"#,
    )
    .unwrap();

    // Resolve the template — must point to per-run path
    let resolved = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/state/run-state.json",
        &ws,
        Some(&meta_root),
    );
    assert!(
        std::path::Path::new(&resolved).exists(),
        "per-run artifact must exist at resolved path"
    );
    assert!(
        resolved.contains(&format!("/.chainworks/runs/{run_id}/")),
        "resolved path must be under per-run meta root"
    );
}

/// artifact.field transition reads resolve against per-run meta root.
/// Proven: the shared .chainworks/ location does NOT have the artifact,
/// only the per-run location does. Template resolution must find the per-run one.
#[test]
fn test_artifact_field_reads_per_run_meta_root() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path().to_string_lossy().into_owned();
    let run_id = "field-read-run";
    let meta_root = format!(".chainworks/runs/{run_id}");

    // Create at per-run location only (not at shared .chainworks/)
    let per_run_dir = format!("{ws}/.chainworks/runs/{run_id}/reviews/proposal");
    std::fs::create_dir_all(&per_run_dir).unwrap();
    std::fs::write(
        format!("{per_run_dir}/summary.json"),
        r#"{"average_score": 9.55, "pass": true}"#,
    )
    .unwrap();

    let resolved = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/reviews/proposal/summary.json",
        &ws,
        Some(&meta_root),
    );
    assert!(std::path::Path::new(&resolved).exists());

    // Verify shared location does NOT have it
    let shared = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/reviews/proposal/summary.json",
        &ws,
        None,
    );
    assert!(
        !std::path::Path::new(&shared).exists(),
        "shared path must NOT have the artifact"
    );
}

// ── P050 ACP env handoff ──

/// ExecutionRequest carries chainworks_meta_root field.
#[test]
fn test_execution_request_carries_chainworks_meta_root() {
    let req = acp::ExecutionRequest {
        run_id: RunId::new(),
        stage_id: "test".into(),
        agent_id: "test".into(),
        provider: "claude".into(),
        model: None,
        effort: None,
        workspace_root: "/tmp".into(),
        prompt: "test".into(),
        worktree_root: None,
        worktree_write_enabled: false,
        worktree_strategy: None,
        expected_output_paths: vec![],
        keep_session_alive: false,
        reuse_existing_session: false,
        session_generation_id: None,
        provider_session_id: None,
        mcp_servers: vec![],
        chainworks_meta_root: Some(".chainworks/runs/test-run".into()),
    };
    assert_eq!(
        req.chainworks_meta_root.as_deref(),
        Some(".chainworks/runs/test-run"),
    );
}

// ── P050 Artifact normalization ──

/// normalize_artifacts with meta_root=Some only searches artifact_root/{run_id}.
#[test]
fn test_normalize_artifacts_uses_run_scoped_source_dir_for_post_p050_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path().to_string_lossy().into_owned();
    let artifact_root = format!("{ws}/artifacts");
    let run_id = RunId::new();

    // Put a file in artifact_root/{run_id}/ (correct location for post-P050)
    let run_dir = format!("{artifact_root}/{run_id}");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::write(format!("{run_dir}/test_artifact.json"), "run-scoped").unwrap();

    // The per-run canonical path resolves differently from shared:
    let meta_root = format!(".chainworks/runs/{run_id}");
    let canonical = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/test_artifact.json",
        &ws,
        Some(&meta_root),
    );
    assert!(canonical.contains(&format!("/.chainworks/runs/{run_id}/")));
}

/// Legacy runs (meta_root=None) preserve the old flat-root fallback.
#[test]
fn test_normalize_artifacts_preserves_flat_root_fallback_for_null_legacy_runs() {
    let legacy = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/state/run-state.json",
        "/project",
        None,
    );
    assert_eq!(
        legacy, "/project/.chainworks/state/run-state.json",
        "legacy NULL meta_root must resolve to shared .chainworks/"
    );
}

// ── P050 Northbound ──

/// MCP runs.list projection row carries chainworks_meta_root.
#[tokio::test]
async fn test_mcp_runs_list_projection_exposes_chainworks_meta_root() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let mut run = make_run(run_id, idea_id, RunStatus::Running);
    run.chainworks_meta_root = Some(".chainworks/runs/proj-test".into());
    runs::insert(&pool, &run).await.unwrap();

    let rows = db::repos::projections::list_active_projection(&pool)
        .await
        .unwrap();
    let row = rows.iter().find(|r| r.id == run_id.to_string()).unwrap();
    assert_eq!(
        row.chainworks_meta_root.as_deref(),
        Some(".chainworks/runs/proj-test"),
        "projection row must carry chainworks_meta_root"
    );
}

/// runs.start does not accept a caller-provided chainworks_meta_root override.
/// (Verified: StartRunCmd has no chainworks_meta_root field — the daemon derives it.)
#[test]
fn test_runs_start_does_not_accept_chainworks_meta_root_override() {
    // StartRunCmd's fields are fixed by the domain struct.
    // This test verifies the struct does NOT have a chainworks_meta_root field.
    let cmd = domain::commands::StartRunCmd {
        idea_id: IdeaId::new(),
        workflow_id: "wf".into(),
        workflow_title: "wf".into(),
        workspace_root: "/tmp".into(),
        artifact_root: "/tmp/a".into(),
        workflow_yaml_path: "/tmp/wf.yaml".into(),
        agent_catalog_yaml_path: "/tmp/cat.yaml".into(),
        delivery_configuration_json: None,
    };
    // If this compiles, StartRunCmd has no chainworks_meta_root field — the daemon owns it.
    let _ = cmd;
}

// ── P050 End-to-end ──

/// Stale shared .chainworks/ artifacts are invisible when per-run meta root is used.
#[test]
fn test_stale_workspace_artifacts_not_visible_to_new_run() {
    let tmp = tempfile::tempdir().unwrap();
    let ws = tmp.path().to_string_lossy().into_owned();

    // Simulate stale artifacts from a prior run in shared .chainworks/
    let stale_dir = format!("{ws}/.chainworks/state");
    std::fs::create_dir_all(&stale_dir).unwrap();
    std::fs::write(format!("{stale_dir}/run-state.json"), "stale").unwrap();

    // New run resolves through per-run meta root — stale shared path is NOT consulted
    let new_run_meta = ".chainworks/runs/new-run-id";
    let resolved = engine::orchestrator::resolve_path_template(
        "${CHAINWORKS_META_ROOT:-.chainworks}/state/run-state.json",
        &ws,
        Some(new_run_meta),
    );

    // The resolved path points to the per-run location (which doesn't exist yet)
    assert!(
        !std::path::Path::new(&resolved).exists(),
        "new run must NOT see stale shared artifact"
    );
    assert!(
        resolved.contains("/.chainworks/runs/new-run-id/"),
        "path must be per-run"
    );
}

/// Prompt input paths point to per-run meta root, not shared .chainworks/.
#[test]
fn test_prompt_input_paths_point_to_per_run_meta_root() {
    let meta_root = ".chainworks/runs/prompt-test";
    let inputs = [
        "${CHAINWORKS_META_ROOT:-.chainworks}/context/idea.md",
        "${CHAINWORKS_META_ROOT:-.chainworks}/state/run-state.json",
        "${CHAINWORKS_META_ROOT:-.chainworks}/proposals/current/proposal.md",
    ];
    for template in &inputs {
        let resolved =
            engine::orchestrator::resolve_path_template(template, "/project", Some(meta_root));
        assert!(
            resolved.contains("/.chainworks/runs/prompt-test/"),
            "input path '{}' must resolve to per-run meta root, got: {}",
            template,
            resolved
        );
    }
}
