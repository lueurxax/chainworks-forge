use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{approvals, ideas, runs, stages};
use domain::approval::{Approval, ApprovalDecision};
use domain::commands::{ApproveStageCmd, Command, RejectStageCmd, RetryStageCmd};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ApprovalId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::recovery::RecoveryService;
use engine::work_queue::WorkQueue;

async fn test_pool() -> sqlx::SqlitePool {
    create_pool("sqlite::memory:").await.expect("in-memory pool failed")
}

fn make_idea(id: IdeaId) -> Idea {
    Idea {
        id,
        title: "Test idea".into(),
        body: "body".into(),
        workspace_root_path: None,
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
    }
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

// ---------------------------------------------------------------------------
// Recovery parity harness (P027)
// Proves daemon RecoveryService matches app-side ResumeManager semantics:
// stages stuck in Running after a crash must become Blocked.
// ---------------------------------------------------------------------------

/// RecoveryService must mark stuck-Running stages as Blocked and re-enqueue
/// AdvanceRun, mirroring Swift ResumeManager.normalizeInterruptedRunsForManualResume.
#[tokio::test]
async fn test_startup_repair_clears_stuck_running_stage() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();
    stages::insert(&pool, &make_stage(stage_id, run_id, StageStatus::Running)).await.unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let recovery = RecoveryService::new(pool.clone(), work_queue, events);

    let summary = recovery.run_startup_repair().await.unwrap();

    assert_eq!(summary.runs_inspected, 1, "one active run must be inspected");
    assert_eq!(summary.runs_repaired, 1, "stuck run must be repaired");
    assert!(summary.work_items_requeued >= 1, "at least one AdvanceRun must be re-enqueued");

    let repaired_stage = stages::find_by_id(&pool, stage_id).await.unwrap().unwrap();
    assert_eq!(
        repaired_stage.status,
        StageStatus::Blocked,
        "stage stuck in Running must become Blocked after startup repair"
    );
}

/// A run with no stuck stages must not be counted as repaired.
#[tokio::test]
async fn test_startup_repair_skips_clean_runs() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();
    // Stage is already Completed — nothing to repair.
    stages::insert(
        &pool,
        &make_stage(stage_id, run_id, StageStatus::Completed),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(64);
    let work_queue = WorkQueue::new(pool.clone());
    let recovery = RecoveryService::new(pool.clone(), work_queue, events);

    let summary = recovery.run_startup_repair().await.unwrap();

    assert_eq!(summary.runs_inspected, 1);
    assert_eq!(summary.runs_repaired, 0, "no repair needed for clean run");
    assert_eq!(summary.work_items_requeued, 0);

    let unchanged_stage = stages::find_by_id(&pool, stage_id).await.unwrap().unwrap();
    assert_eq!(
        unchanged_stage.status,
        StageStatus::Completed,
        "clean stage must not be modified by startup repair"
    );
}

// ---------------------------------------------------------------------------
// Approval / retry parity harness (P027)
// Proves daemon CommandHandler approval and retry semantics match the
// app-owned ExecutionService authority model:
// – Granted approval → stage transitions WaitingApproval → Running
// – Rejected approval → stage transitions WaitingApproval → Blocked
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
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "review_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "review_stage", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::ApproveStage(ApproveStageCmd {
            run_id,
            stage_id: "review_stage".into(),
            comment: Some("LGTM".into()),
        }))
        .await
        .unwrap();

    // Approval must now be Granted.
    let resolved = approvals::find_by_id(&pool, approval.id).await.unwrap().unwrap();
    assert_eq!(
        resolved.decision,
        ApprovalDecision::Granted,
        "approval decision must be Granted after ApproveStage"
    );
    assert!(resolved.decided_at.is_some(), "decided_at must be set");

    // Stage must have transitioned to Running.
    let updated_stage = stages::find_by_id(&pool, stage_exec_id).await.unwrap().unwrap();
    assert_eq!(
        updated_stage.status,
        StageStatus::Running,
        "stage must advance to Running after approval is granted"
    );
}

/// Rejecting approval must resolve the canonical approval record to Rejected
/// and transition the stage from WaitingApproval to Blocked.
#[tokio::test]
async fn test_reject_stage_resolves_approval_and_blocks_stage() {
    let pool = test_pool().await;

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();

    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();

    let mut stage = make_stage(stage_exec_id, run_id, StageStatus::WaitingApproval);
    stage.stage_id = "gated_stage".into();
    stages::insert(&pool, &stage).await.unwrap();

    let approval = make_approval(run_id, "gated_stage", ApprovalDecision::Pending);
    approvals::insert(&pool, &approval).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::RejectStage(RejectStageCmd {
            run_id,
            stage_id: "gated_stage".into(),
            comment: Some("Not ready".into()),
        }))
        .await
        .unwrap();

    // Approval must now be Rejected.
    let resolved = approvals::find_by_id(&pool, approval.id).await.unwrap().unwrap();
    assert_eq!(
        resolved.decision,
        ApprovalDecision::Rejected,
        "approval decision must be Rejected after RejectStage"
    );

    // Stage must have transitioned to Blocked.
    let updated_stage = stages::find_by_id(&pool, stage_exec_id).await.unwrap().unwrap();
    assert_eq!(
        updated_stage.status,
        StageStatus::Blocked,
        "stage must become Blocked after approval is rejected"
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
    runs::insert(&pool, &make_run(run_id, idea_id, RunStatus::Running)).await.unwrap();

    let mut stage = make_stage(old_stage_exec_id, run_id, StageStatus::Failed);
    stage.stage_id = "flaky_stage".into();
    stage.attempt_number = 1;
    stages::insert(&pool, &stage).await.unwrap();

    let handler = make_command_handler(pool.clone());
    handler
        .handle(Command::RetryStage(RetryStageCmd {
            run_id,
            stage_id: "flaky_stage".into(),
        }))
        .await
        .unwrap();

    // Old stage must be settled as Skipped.
    let old = stages::find_by_id(&pool, old_stage_exec_id).await.unwrap().unwrap();
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
