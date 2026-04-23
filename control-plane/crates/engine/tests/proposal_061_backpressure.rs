use std::collections::BTreeMap;
use std::process::Command as ProcessCommand;
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use chrono::{Duration, Utc};
use db::pool::create_pool;
use db::repos::{
    agent_executions, approvals, artifact_contracts, ideas, runs, scheduler, stages,
    startup_repairs, work_items,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{AgentExecution, AgentOutputSettlement, AgentStatus, ArtifactSourceClaimState};
use domain::approval::{Approval, ApprovalDecision};
use domain::artifact_contracts::{
    ActiveArtifactGenerationInput, ArtifactSourceGenerationClaim, ArtifactSourceGenerationClaimKey,
    SourceGenerationImportDecision,
};
use domain::commands::{
    ApproveStageCmd, CallerContext, CancelRunCmd, Command, RejectStageCmd, ResetSessionCmd,
    RetryStageCmd, StartRunCmd,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, ApprovalId, ArtifactId, IdeaId, RunId, StageExecutionId};
use domain::provider::ProviderFamily;
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};
use engine::command_handler::CommandHandler;
use engine::executor::{
    claim_next_invoke_agent_with_start_with_capacity,
    has_capacity_eligible_pending_invoke_agent_for_start, InvokeAgentCapacityConfig,
};
use engine::host_interruption::{
    HostInterruptionEvent, HostInterruptionKind, HostInterruptionRuntimeCleanup,
    HostInterruptionService,
};
use engine::{event_bus, recovery::RecoveryService, work_queue::WorkQueue};

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf".into(),
        workflow_title: "Workflow".into(),
        workspace_root: "/tmp/workspace".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("implementation".into()),
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

fn make_stage(
    run_id: RunId,
    stage_execution_id: StageExecutionId,
    stage_id: &str,
) -> StageExecution {
    StageExecution {
        id: stage_execution_id,
        run_id,
        stage_id: stage_id.into(),
        label: stage_id.into(),
        status: StageStatus::Running,
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

fn make_running_execution(stage_execution_id: StageExecutionId, provider: &str) -> AgentExecution {
    AgentExecution {
        id: AgentExecutionId::new(),
        stage_execution_id,
        agent_id: format!("{provider}_agent"),
        provider: provider.into(),
        model: Some("default".into()),
        status: AgentStatus::Running,
        started_at: Utc::now(),
        completed_at: None,
        owner_execution_lineage_id: Some(stage_execution_id.to_string()),
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

fn make_invoke_work_item(
    id: &str,
    run_id: RunId,
    stage_execution_id: StageExecutionId,
    stage_id: &str,
    provider: &str,
    scheduled_offset_seconds: i64,
) -> WorkItem {
    let now = Utc::now();
    WorkItem {
        id: id.into(),
        kind: WorkItemKind::InvokeAgent,
        payload_json: serde_json::json!({
            "stage_id": stage_id,
            "stage_execution_id": stage_execution_id.to_string(),
            "agent_id": format!("{provider}_agent"),
            "provider": provider,
            "model": "default",
            "prompt": "execute",
            "task_name": stage_id,
            "task_inputs": [],
            "task_outputs": [],
            "declared_outputs": [],
            "requested_mcp_server_ids": [],
            "session_reuse_scope": "same_agent_family_within_run",
            "session_family_id": format!("{provider}_agent"),
            "worktree_write_enabled": false
        })
        .to_string(),
        status: WorkItemStatus::Pending,
        run_id: Some(run_id),
        stage_id: Some(stage_id.into()),
        created_at: now + Duration::seconds(scheduled_offset_seconds),
        scheduled_at: now + Duration::seconds(scheduled_offset_seconds),
        attempt_count: 0,
        last_error: None,
    }
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

#[derive(Default)]
struct RecordingRuntimeCleanup {
    closed_generations: Mutex<Vec<String>>,
    fail_generation: Option<String>,
}

#[async_trait::async_trait]
impl HostInterruptionRuntimeCleanup for RecordingRuntimeCleanup {
    async fn close_session_generation(&self, generation_id: &str) -> anyhow::Result<()> {
        self.closed_generations
            .lock()
            .unwrap()
            .push(generation_id.to_string());
        if self.fail_generation.as_deref() == Some(generation_id) {
            anyhow::bail!("cleanup failed for {generation_id}");
        }
        Ok(())
    }
}

const COMMAND_LATENCY_SAMPLE_COUNT: usize = 20;
const COMMAND_LATENCY_P95_LIMIT: StdDuration = StdDuration::from_secs(2);
const COMMAND_LATENCY_HARD_LIMIT: StdDuration = StdDuration::from_secs(5);

async fn seed_active_fake_agents(pool: &sqlx::SqlitePool, count: usize) -> IdeaId {
    let idea_id = IdeaId::new();
    let now = Utc::now();
    ideas::insert(
        pool,
        &Idea {
            id: idea_id,
            title: "P061 active fake agents".into(),
            body: "latency pressure fixture".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();

    let providers = ["claude", "gemini", "codex", "auggie", "junie"];
    for index in 0..count {
        let run_id = RunId::new();
        let stage_execution_id = StageExecutionId::new();
        runs::insert(pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        stages::insert(
            pool,
            &make_stage(run_id, stage_execution_id, "active_fake_agent"),
        )
        .await
        .unwrap();
        let execution =
            make_running_execution(stage_execution_id, providers[index % providers.len()]);
        agent_executions::insert(pool, &execution).await.unwrap();
    }

    idea_id
}

async fn insert_pending_approval(
    pool: &sqlx::SqlitePool,
    idea_id: IdeaId,
    stage_id: &str,
) -> (RunId, ApprovalId) {
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let approval_id = ApprovalId::new();
    let now = Utc::now();
    runs::insert(pool, &make_run(run_id, idea_id))
        .await
        .unwrap();

    let mut stage = make_stage(run_id, stage_execution_id, stage_id);
    stage.status = StageStatus::WaitingApproval;
    stages::insert(pool, &stage).await.unwrap();
    approvals::insert(
        pool,
        &Approval {
            id: approval_id,
            run_id,
            stage_id: stage_id.into(),
            decision: ApprovalDecision::Pending,
            requested_at: now,
            decided_at: None,
            comment: None,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    (run_id, approval_id)
}

async fn insert_failed_retry_stage(
    pool: &sqlx::SqlitePool,
    idea_id: IdeaId,
    stage_id: &str,
) -> RunId {
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    runs::insert(pool, &make_run(run_id, idea_id))
        .await
        .unwrap();

    let mut stage = make_stage(run_id, stage_execution_id, stage_id);
    stage.status = StageStatus::Failed;
    stage.completed_at = Some(Utc::now());
    stages::insert(pool, &stage).await.unwrap();

    run_id
}

async fn insert_cancellable_run(pool: &sqlx::SqlitePool, idea_id: IdeaId, stage_id: &str) -> RunId {
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    runs::insert(pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(pool, &make_stage(run_id, stage_execution_id, stage_id))
        .await
        .unwrap();
    run_id
}

fn p95_latency(mut samples: Vec<StdDuration>) -> StdDuration {
    assert!(!samples.is_empty());
    samples.sort();
    let index = ((samples.len() * 95).div_ceil(100)).saturating_sub(1);
    samples[index]
}

fn assert_command_latency_bounds(command: &str, samples: Vec<StdDuration>) {
    let worst = samples
        .iter()
        .copied()
        .max()
        .expect("latency samples should not be empty");
    let p95 = p95_latency(samples);

    assert!(
        p95 < COMMAND_LATENCY_P95_LIMIT,
        "{command} p95 latency under 20 active fake agents was {p95:?}, expected below {:?}",
        COMMAND_LATENCY_P95_LIMIT
    );
    assert!(
        worst <= COMMAND_LATENCY_HARD_LIMIT,
        "{command} single-command latency under 20 active fake agents was {worst:?}, expected at or below {:?}",
        COMMAND_LATENCY_HARD_LIMIT
    );
}

#[tokio::test]
async fn approve_retry_cancel_p95_latency_stays_below_two_seconds_under_twenty_active_fake_agents()
{
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = seed_active_fake_agents(&pool, 20).await;
    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(128),
        WorkQueue::new(pool.clone()),
    );

    let mut approve_samples = Vec::with_capacity(COMMAND_LATENCY_SAMPLE_COUNT);
    let mut retry_samples = Vec::with_capacity(COMMAND_LATENCY_SAMPLE_COUNT);
    let mut cancel_samples = Vec::with_capacity(COMMAND_LATENCY_SAMPLE_COUNT);

    for index in 0..COMMAND_LATENCY_SAMPLE_COUNT {
        let approval_stage_id = format!("approval_latency_{index}");
        let (approve_run_id, _) = insert_pending_approval(&pool, idea_id, &approval_stage_id).await;
        let started = Instant::now();
        handler
            .handle(
                Command::ApproveStage(ApproveStageCmd {
                    run_id: approve_run_id,
                    stage_id: approval_stage_id,
                    comment: Some("latency proof".into()),
                }),
                CallerContext::test_fixture(),
            )
            .await
            .unwrap();
        approve_samples.push(started.elapsed());

        let retry_stage_id = format!("retry_latency_{index}");
        let retry_run_id = insert_failed_retry_stage(&pool, idea_id, &retry_stage_id).await;
        let started = Instant::now();
        handler
            .handle(
                Command::RetryStage(RetryStageCmd {
                    run_id: retry_run_id,
                    stage_id: retry_stage_id,
                    agent_execution_id: None,
                    consume_quota_budget_now: false,
                }),
                CallerContext::test_fixture(),
            )
            .await
            .unwrap();
        retry_samples.push(started.elapsed());

        let cancel_run_id =
            insert_cancellable_run(&pool, idea_id, &format!("cancel_latency_{index}")).await;
        let started = Instant::now();
        handler
            .handle(
                Command::CancelRun(CancelRunCmd {
                    run_id: cancel_run_id,
                }),
                CallerContext::test_fixture(),
            )
            .await
            .unwrap();
        cancel_samples.push(started.elapsed());
    }

    assert_command_latency_bounds("ApproveStage", approve_samples);
    assert_command_latency_bounds("RetryStage", retry_samples);
    assert_command_latency_bounds("CancelRun", cancel_samples);
}

#[tokio::test]
async fn command_handler_refreshes_scheduler_projection_with_configured_capacity() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = seed_active_fake_agents(&pool, 0).await;
    let approval_stage_id = "approval_with_custom_capacity";
    let (approval_run_id, _) = insert_pending_approval(&pool, idea_id, approval_stage_id).await;

    let running_run_id = RunId::new();
    let running_stage_execution_id = StageExecutionId::new();
    runs::insert(&pool, &make_run(running_run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(running_run_id, running_stage_execution_id, "running_codex"),
    )
    .await
    .unwrap();
    agent_executions::insert(
        &pool,
        &make_running_execution(running_stage_execution_id, "codex"),
    )
    .await
    .unwrap();

    let queued_run_id = RunId::new();
    let queued_stage_execution_id = StageExecutionId::new();
    runs::insert(&pool, &make_run(queued_run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(queued_run_id, queued_stage_execution_id, "queued_codex"),
    )
    .await
    .unwrap();
    work_items::enqueue(
        &pool,
        &make_invoke_work_item(
            "queued-codex-custom-capacity",
            queued_run_id,
            queued_stage_execution_id,
            "queued_codex",
            "codex",
            -300,
        ),
    )
    .await
    .unwrap();

    let capacity = InvokeAgentCapacityConfig {
        global_active_agent_executions: 20,
        per_run_active_agent_executions: 10,
        provider_caps: BTreeMap::from([(ProviderFamily::Codex, 1)]),
    };
    let handler = CommandHandler::new_with_capacity(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
        capacity,
    );

    handler
        .handle(
            Command::ApproveStage(ApproveStageCmd {
                run_id: approval_run_id,
                stage_id: approval_stage_id.into(),
                comment: Some("custom capacity projection proof".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let summaries = scheduler::list_queue_summaries(&pool).await.unwrap();
    assert!(
        summaries.iter().any(|summary| {
            summary.provider_family.as_deref() == Some("codex")
                && summary.top_reason == "provider_capacity"
                && summary.queued_count == 1
        }),
        "command-triggered scheduler refresh should use configured provider caps: {summaries:?}"
    );
}

#[tokio::test]
async fn cancel_run_closes_journal_with_cancellation_settlement_and_scheduler_refresh() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 cancel".into(),
            body: "write coordination".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(run_id, stage_execution_id, "implementation"),
    )
    .await
    .unwrap();
    let running_execution = make_running_execution(stage_execution_id, "claude");
    let running_execution_id = running_execution.id;
    agent_executions::insert(&pool, &running_execution)
        .await
        .unwrap();
    let mut running_work = make_invoke_work_item(
        "running-cancelled-invoke",
        run_id,
        stage_execution_id,
        "implementation",
        "claude",
        -30,
    );
    running_work.status = WorkItemStatus::Running;
    work_items::enqueue(&pool, &running_work).await.unwrap();

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let commanded = handler
        .handle(
            Command::CancelRun(CancelRunCmd { run_id }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let journal_row: (String, Option<String>) =
        sqlx::query_as("SELECT result_status, completed_at FROM command_journal WHERE id = ?1")
            .bind(&commanded.journal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(journal_row.0, "completed");
    assert!(
        journal_row.1.is_some(),
        "CancelRun journal entry should close in the cancellation write transaction"
    );

    let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Cancelling);
    assert!(run.cancellation_requested_at.is_some());
    assert!(
        run.cancellation_settlement_log
            .as_deref()
            .unwrap_or_default()
            .contains(&running_execution_id.to_string()),
        "CancelRun settlement log should be written before commit"
    );

    let execution = agent_executions::find_by_id(&pool, running_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(execution.status, AgentStatus::Cancelled);

    let stage = stages::find_by_id(&pool, stage_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stage.status, StageStatus::Failed);
    assert_eq!(stage.settlement_kind, Some(StageSettlementKind::Failed));

    let work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    let cancelled_work = work_items
        .iter()
        .find(|item| item.id == "running-cancelled-invoke")
        .expect("running InvokeAgent work item should remain as cancellation audit evidence");
    assert_eq!(cancelled_work.status, WorkItemStatus::Cancelled);

    assert!(
        scheduler::latest_health_snapshot(&pool)
            .await
            .unwrap()
            .is_some(),
        "CancelRun command write unit must refresh scheduler health"
    );
}

#[tokio::test]
async fn reset_session_closes_journal_with_repair_wake_and_scheduler_refresh() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 reset".into(),
            body: "write coordination".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(run_id, stage_execution_id, "implementation"),
    )
    .await
    .unwrap();

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let commanded = handler
        .handle(
            Command::ResetSession(ResetSessionCmd {
                run_id,
                stage_id: "implementation".into(),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let journal_row: (String, Option<String>) =
        sqlx::query_as("SELECT result_status, completed_at FROM command_journal WHERE id = ?1")
            .bind(&commanded.journal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(journal_row.0, "completed");
    assert!(
        journal_row.1.is_some(),
        "ResetSession journal entry should close in the reset write transaction"
    );

    let stage = stages::find_by_id(&pool, stage_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stage.status, StageStatus::Pending);

    let work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    assert!(
        work_items.iter().any(|item| {
            item.kind == WorkItemKind::StartupRepair
                && item.stage_id.as_deref() == Some("implementation")
                && item.status == WorkItemStatus::Pending
        }),
        "ResetSession must enqueue StartupRepair inside the command write unit"
    );
    assert!(
        scheduler::latest_health_snapshot(&pool)
            .await
            .unwrap()
            .is_some(),
        "ResetSession command write unit must refresh scheduler health"
    );
}

#[tokio::test]
async fn startup_repair_blocks_stale_running_stage_enqueues_wake_and_scheduler_refresh() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 startup repair".into(),
            body: "write coordination".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(run_id, stage_execution_id, "implementation"),
    )
    .await
    .unwrap();

    let recovery = RecoveryService::new(
        pool.clone(),
        WorkQueue::new(pool.clone()),
        event_bus::new_bus(16),
    );
    let summary = recovery.run_startup_repair().await.unwrap();
    assert_eq!(summary.runs_inspected, 1);
    assert_eq!(summary.runs_repaired, 1);
    assert_eq!(summary.work_items_requeued, 1);

    let stage = stages::find_by_id(&pool, stage_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stage.status, StageStatus::Blocked);

    let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert!(
        run.drift_details_json
            .as_deref()
            .unwrap_or_default()
            .contains("stage_stuck_running"),
        "startup repair should record drift facts inside the repair transaction"
    );

    let work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    let advance_run_count = work_items
        .iter()
        .filter(|item| item.kind == WorkItemKind::AdvanceRun)
        .count();
    assert_eq!(advance_run_count, 1);
    assert!(
        work_items.iter().any(|item| {
            item.kind == WorkItemKind::AdvanceRun
                && item.status == WorkItemStatus::Pending
                && item.payload_json.contains("startup_repair")
        }),
        "startup repair must enqueue AdvanceRun inside the repair write unit"
    );

    let repair_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(repair_count, 1);
    let recommendation_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM recovery_recommendations")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(recommendation_count, 1);
    assert!(
        scheduler::latest_health_snapshot(&pool)
            .await
            .unwrap()
            .is_some(),
        "StartupRepair write unit must refresh scheduler health before commit"
    );

    let second = recovery.run_startup_repair().await.unwrap();
    assert_eq!(second.runs_inspected, 1);
    assert_eq!(
        second.work_items_requeued, 0,
        "repeated startup repair should be idempotent while the repair wake is pending"
    );
    let repeated_repair_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM startup_repairs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(repeated_repair_count, 1);
}

#[tokio::test]
async fn startup_repair_readback_counts_requeued_invoke_backpressure() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 startup recovery readback".into(),
            body: "readback".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(run_id, stage_execution_id, "implementation"),
    )
    .await
    .unwrap();

    let mut invoke = make_invoke_work_item(
        "startup-requeued-invoke",
        run_id,
        stage_execution_id,
        "implementation",
        "codex_cli",
        -600,
    );
    let mut payload: serde_json::Value = serde_json::from_str(&invoke.payload_json).unwrap();
    payload["p058_claimed"] = serde_json::json!({
        "agent_execution_id": AgentExecutionId::new().to_string()
    });
    invoke.payload_json = payload.to_string();
    invoke.status = WorkItemStatus::Running;
    work_items::enqueue(&pool, &invoke).await.unwrap();

    let recovery = RecoveryService::new(
        pool.clone(),
        WorkQueue::new(pool.clone()),
        event_bus::new_bus(16),
    );
    let summary = recovery.run_startup_repair().await.unwrap();

    assert_eq!(summary.runs_inspected, 1);
    assert_eq!(summary.runs_repaired, 1);
    assert_eq!(summary.recovered_item_count, 1);
    assert_eq!(summary.affected_run_count, 1);
    assert_eq!(
        summary.queued_under_startup_recovery_backpressure_count, 1,
        "startup-requeued InvokeAgent work should be reported separately from ordinary queued work"
    );
    assert!(
        summary.oldest_recovered_queued_age_ms.unwrap_or_default() >= 590_000,
        "oldest recovered queued age should preserve the requeued item's schedule age"
    );
    assert!(summary.next_retry_or_backoff_time.is_some());

    let summaries = scheduler::list_queue_summaries_by_run(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert!(
        summaries.iter().any(
            |summary| summary.top_reason == "startup_recovery_backpressure"
                && summary.queued_count == 1
        ),
        "scheduler summaries must expose startup recovery backpressure as a top reason"
    );

    let readback = startup_repairs::latest_startup_recovery_readback(&pool)
        .await
        .unwrap()
        .expect("startup recovery readback should be persisted");
    assert_eq!(readback.recovered_item_count, 1);
    assert_eq!(readback.affected_run_count, 1);
    assert_eq!(readback.queued_under_startup_recovery_backpressure_count, 1);
    assert!(readback.next_retry_or_backoff_time.is_some());
}

#[tokio::test]
async fn retry_stage_capacity_refresh_clears_superseded_invoke_backpressure() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_execution_id = StageExecutionId::new();
    let old_agent_execution_id = AgentExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 retry".into(),
            body: "write coordination".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();

    let mut old_stage = make_stage(run_id, old_stage_execution_id, "implementation");
    old_stage.status = StageStatus::Failed;
    old_stage.completed_at = Some(Utc::now());
    stages::insert(&pool, &old_stage).await.unwrap();

    let mut running_execution = make_running_execution(old_stage_execution_id, "claude");
    running_execution.id = old_agent_execution_id;
    agent_executions::insert(&pool, &running_execution)
        .await
        .unwrap();
    work_items::enqueue(
        &pool,
        &make_invoke_work_item(
            "old-pending-invoke",
            run_id,
            old_stage_execution_id,
            "implementation",
            "claude",
            -300,
        ),
    )
    .await
    .unwrap();

    scheduler::refresh_queue_summaries(
        &pool,
        &domain::provider::InvokeAgentCapacityConfig::default(),
    )
    .await
    .unwrap();
    assert!(
        !scheduler::list_queue_summaries(&pool)
            .await
            .unwrap()
            .is_empty(),
        "precondition: stale pending InvokeAgent should be visible before retry"
    );

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let commanded = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                agent_execution_id: None,
                consume_quota_budget_now: false,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let journal_row: (String, Option<String>) =
        sqlx::query_as("SELECT result_status, completed_at FROM command_journal WHERE id = ?1")
            .bind(&commanded.journal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(journal_row.0, "completed");
    assert!(
        journal_row.1.is_some(),
        "RetryStage journal entry should close in the retry write transaction"
    );

    let old_stage_after = stages::find_by_id(&pool, old_stage_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_stage_after.status, StageStatus::Skipped);
    assert_eq!(
        old_stage_after.settlement_kind,
        Some(StageSettlementKind::Skipped)
    );

    let old_execution_after = agent_executions::find_by_id(&pool, old_agent_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_execution_after.status, AgentStatus::Cancelled);

    let work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    let old_work_item = work_items
        .iter()
        .find(|item| item.id == "old-pending-invoke")
        .expect("old pending invoke work item must still be retained as audit evidence");
    assert_eq!(old_work_item.status, WorkItemStatus::Cancelled);
    assert_eq!(
        old_work_item.last_error.as_deref(),
        Some("superseded_by_retry")
    );

    assert!(
        scheduler::list_queue_summaries(&pool)
            .await
            .unwrap()
            .is_empty(),
        "RetryStage transaction should clear stale InvokeAgent queue summaries"
    );
}

#[tokio::test]
async fn retry_stage_injected_crashes_roll_back_and_startup_repair_clears_stale_running_executions()
{
    let failpoints = [
        "record_journal",
        "apply_quota_budget",
        "cancel_agent_executions",
        "cancel_work_items",
        "settle_old_stage",
        "insert_new_stage",
        "supersede_artifact_claims",
        "enqueue_retry_wake",
        "refresh_scheduler",
        "complete_journal",
    ];

    for failpoint in failpoints {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let old_stage_execution_id = StageExecutionId::new();
        let old_agent_execution_id = AgentExecutionId::new();

        ideas::insert(
            &pool,
            &Idea {
                id: idea_id,
                title: format!("P061 retry failpoint {failpoint}"),
                body: "write coordination failure injection".into(),
                workspace_root_path: None,
                project_key: None,
                status: IdeaStatus::Active,
                created_at: Utc::now(),
                archived_at: None,
            },
        )
        .await
        .unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let mut old_stage = make_stage(run_id, old_stage_execution_id, "implementation");
        old_stage.status = StageStatus::Failed;
        old_stage.completed_at = Some(Utc::now());
        stages::insert(&pool, &old_stage).await.unwrap();

        let mut running_execution = make_running_execution(old_stage_execution_id, "claude");
        running_execution.id = old_agent_execution_id;
        agent_executions::insert(&pool, &running_execution)
            .await
            .unwrap();

        let mut running_work = make_invoke_work_item(
            "old-running-invoke",
            run_id,
            old_stage_execution_id,
            "implementation",
            "claude",
            -300,
        );
        running_work.status = WorkItemStatus::Running;
        work_items::enqueue(&pool, &running_work).await.unwrap();

        let injected_failpoint = failpoint.to_string();
        let handler = CommandHandler::new(
            pool.clone(),
            event_bus::new_bus(16),
            WorkQueue::new(pool.clone()),
        )
        .with_retry_stage_failure_injection(Arc::new(move |step| {
            if step == injected_failpoint {
                anyhow::bail!("injected RetryStage failure after {step}");
            }
            Ok(())
        }));

        let err = match handler
            .handle(
                Command::RetryStage(RetryStageCmd {
                    run_id,
                    stage_id: "implementation".into(),
                    agent_execution_id: None,
                    consume_quota_budget_now: false,
                }),
                CallerContext::test_fixture(),
            )
            .await
        {
            Ok(_) => panic!(
                "injected RetryStage failure at {failpoint} should abort the write transaction"
            ),
            Err(error) => error,
        };
        assert!(
            err.to_string().contains("injected RetryStage failure"),
            "unexpected injected failure for {failpoint}: {err}"
        );

        let stages_after_failure = stages::list_by_run(&pool, run_id).await.unwrap();
        assert_eq!(
            stages_after_failure.len(),
            1,
            "RetryStage failure at {failpoint} should roll back the new stage attempt"
        );

        let recovery = RecoveryService::new(
            pool.clone(),
            WorkQueue::new(pool.clone()),
            event_bus::new_bus(16),
        );
        recovery.run_startup_repair().await.unwrap();

        let running_execution_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*)
               FROM agent_executions ae
               INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
               WHERE se.run_id = ?1 AND ae.status = ?2"#,
        )
        .bind(run_id.to_string())
        .bind(AgentStatus::Running.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            running_execution_count, 0,
            "startup repair should clear stale Running executions after RetryStage crash point {failpoint}"
        );

        let old_work_item = work_items::list_by_run(&pool, run_id)
            .await
            .unwrap()
            .into_iter()
            .find(|item| item.id == "old-running-invoke")
            .expect("old running invoke item should remain as audit evidence");
        assert_eq!(
            old_work_item.status,
            WorkItemStatus::Cancelled,
            "startup repair should cancel exact stale InvokeAgent work after RetryStage crash point {failpoint}"
        );
        assert_eq!(
            old_work_item.last_error.as_deref(),
            Some("stale_stage_execution_startup_repair")
        );
    }
}

#[tokio::test]
async fn approve_and_reject_stage_close_journal_with_stage_mutation_and_scheduler_refresh() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let approve_run_id = RunId::new();
    let reject_run_id = RunId::new();
    let approve_stage_execution_id = StageExecutionId::new();
    let reject_stage_execution_id = StageExecutionId::new();
    let approve_id = ApprovalId::new();
    let reject_id = ApprovalId::new();
    let now = Utc::now();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 approvals".into(),
            body: "write coordination".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(approve_run_id, idea_id))
        .await
        .unwrap();
    runs::insert(&pool, &make_run(reject_run_id, idea_id))
        .await
        .unwrap();

    let mut approve_stage = make_stage(approve_run_id, approve_stage_execution_id, "approval");
    approve_stage.status = StageStatus::WaitingApproval;
    stages::insert(&pool, &approve_stage).await.unwrap();
    approvals::insert(
        &pool,
        &Approval {
            id: approve_id,
            run_id: approve_run_id,
            stage_id: "approval".into(),
            decision: ApprovalDecision::Pending,
            requested_at: now,
            decided_at: None,
            comment: None,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    let mut reject_stage = make_stage(reject_run_id, reject_stage_execution_id, "rejection");
    reject_stage.status = StageStatus::WaitingApproval;
    stages::insert(&pool, &reject_stage).await.unwrap();
    approvals::insert(
        &pool,
        &Approval {
            id: reject_id,
            run_id: reject_run_id,
            stage_id: "rejection".into(),
            decision: ApprovalDecision::Pending,
            requested_at: now,
            decided_at: None,
            comment: None,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let approved = handler
        .handle(
            Command::ApproveStage(ApproveStageCmd {
                run_id: approve_run_id,
                stage_id: "approval".into(),
                comment: Some("ship it".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();
    let rejected = handler
        .handle(
            Command::RejectStage(RejectStageCmd {
                run_id: reject_run_id,
                stage_id: "rejection".into(),
                comment: Some("needs changes".into()),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    for journal_id in [&approved.journal_id, &rejected.journal_id] {
        let row: (String, Option<String>) =
            sqlx::query_as("SELECT result_status, completed_at FROM command_journal WHERE id = ?1")
                .bind(journal_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row.0, "completed");
        assert!(
            row.1.is_some(),
            "approval commands must close the journal row in the command write transaction"
        );
    }

    let approved_approval = approvals::find_by_id(&pool, approve_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(approved_approval.decision, ApprovalDecision::Granted);
    let approved_stage = stages::find_by_id(&pool, approve_stage_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(approved_stage.status, StageStatus::Running);

    let rejected_approval = approvals::find_by_id(&pool, reject_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rejected_approval.decision, ApprovalDecision::Rejected);
    let rejected_stage = stages::find_by_id(&pool, reject_stage_execution_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rejected_stage.status, StageStatus::Blocked);

    let approve_items = work_items::list_by_run(&pool, approve_run_id)
        .await
        .unwrap();
    assert!(
        approve_items
            .iter()
            .any(|item| item.kind == WorkItemKind::AdvanceRun
                && item.status == WorkItemStatus::Pending),
        "ApproveStage must enqueue the AdvanceRun wake inside the command write unit"
    );
    assert!(
        scheduler::latest_health_snapshot(&pool)
            .await
            .unwrap()
            .is_some(),
        "approval command write units must refresh scheduler health"
    );
}

#[tokio::test]
async fn start_run_closes_journal_with_run_wake_and_scheduler_refresh() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let now = Utc::now();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 start".into(),
            body: "write coordination".into(),
            workspace_root_path: None,
            project_key: Some("p061".into()),
            status: IdeaStatus::Draft,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();

    let repo = tempfile::tempdir().unwrap();
    ProcessCommand::new("git")
        .args(["init", "--initial-branch", "main"])
        .current_dir(repo.path())
        .output()
        .expect("git init should run");
    ProcessCommand::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo.path())
        .output()
        .expect("git config user.email should run");
    ProcessCommand::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo.path())
        .output()
        .expect("git config user.name should run");
    std::fs::write(repo.path().join("README.md"), "initial\n").unwrap();
    ProcessCommand::new("git")
        .args(["add", "README.md"])
        .current_dir(repo.path())
        .output()
        .expect("git add should run");
    ProcessCommand::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(repo.path())
        .output()
        .expect("git commit should run");
    let worktrees = tempfile::tempdir().unwrap();
    let delivery_configuration_json = format!(
        r#"{{
            "repo_identifier":"repo-p061",
            "repo_root":"{}",
            "base_branch":"main",
            "worktree_base_path":"{}",
            "target_branch":"cw/p061",
            "release_target_id":"sandbox-target",
            "release_mode":"sandbox"
        }}"#,
        repo.path().display(),
        worktrees.path().display()
    );

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let commanded = handler
        .handle(
            Command::StartRun(StartRunCmd {
                idea_id,
                workflow_id: "wf-p061".into(),
                workflow_title: "P061 Workflow".into(),
                workspace_root: "/tmp/p061-workspace".into(),
                artifact_root: "/tmp/p061-artifacts".into(),
                delivery_configuration_json: Some(delivery_configuration_json),
                workflow_yaml_path: test_workflow_yaml_path(),
                agent_catalog_yaml_path: test_agent_catalog_yaml_path(),
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let run_id = match commanded.result {
        engine::command_handler::CommandResult::RunStarted { run_id } => run_id,
        _ => panic!("expected StartRun to start a run"),
    };
    let journal_row: (String, Option<String>) =
        sqlx::query_as("SELECT result_status, completed_at FROM command_journal WHERE id = ?1")
            .bind(&commanded.journal_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(journal_row.0, "completed");
    assert!(
        journal_row.1.is_some(),
        "StartRun journal entry should close in the run creation write transaction"
    );

    let run = runs::find_by_id(&pool, run_id).await.unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Pending);
    assert_eq!(run.project_key.as_deref(), Some("p061"));

    let idea = ideas::find_by_id(&pool, idea_id).await.unwrap().unwrap();
    assert_eq!(idea.status, IdeaStatus::Active);

    let work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    assert!(
        work_items
            .iter()
            .any(|item| item.kind == WorkItemKind::AdvanceRun
                && item.status == WorkItemStatus::Pending),
        "StartRun must enqueue the initial AdvanceRun wake inside the command write unit"
    );
    assert!(
        scheduler::latest_health_snapshot(&pool)
            .await
            .unwrap()
            .is_some(),
        "StartRun command write unit must refresh scheduler health"
    );
}

#[tokio::test]
async fn invoke_agent_claim_skips_provider_at_capacity_and_claims_next_eligible_provider() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let running_gemini_stage = StageExecutionId::new();
    let pending_gemini_stage = StageExecutionId::new();
    let pending_codex_stage = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061".into(),
            body: "backpressure".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    for (stage_execution_id, stage_id) in [
        (running_gemini_stage, "running_gemini"),
        (pending_gemini_stage, "pending_gemini"),
        (pending_codex_stage, "pending_codex"),
    ] {
        stages::insert(&pool, &make_stage(run_id, stage_execution_id, stage_id))
            .await
            .unwrap();
    }
    agent_executions::insert(
        &pool,
        &make_running_execution(running_gemini_stage, "gemini_acp"),
    )
    .await
    .unwrap();
    work_items::enqueue(
        &pool,
        &make_invoke_work_item(
            "gemini-pending",
            run_id,
            pending_gemini_stage,
            "pending_gemini",
            "gemini_cli_acp",
            -2,
        ),
    )
    .await
    .unwrap();
    work_items::enqueue(
        &pool,
        &make_invoke_work_item(
            "codex-pending",
            run_id,
            pending_codex_stage,
            "pending_codex",
            "codex",
            -1,
        ),
    )
    .await
    .unwrap();

    let capacity = InvokeAgentCapacityConfig {
        global_active_agent_executions: 6,
        per_run_active_agent_executions: 10,
        provider_caps: BTreeMap::from([(ProviderFamily::Gemini, 1), (ProviderFamily::Codex, 3)]),
    };

    let claimed = claim_next_invoke_agent_with_start_with_capacity(&pool, &capacity)
        .await
        .unwrap()
        .expect("codex should still be claimable while gemini is at cap");

    assert_eq!(claimed.work_item_id, "codex-pending");

    let pending = work_items::list_by_status(&pool, WorkItemStatus::Pending)
        .await
        .unwrap();
    assert!(
        pending.iter().any(|item| item.id == "gemini-pending"),
        "provider-capped Gemini item should remain pending"
    );
    assert!(
        !pending.iter().any(|item| item.id == "codex-pending"),
        "eligible Codex item should be claimed"
    );

    let codex_executions = agent_executions::find_by_stage(&pool, pending_codex_stage)
        .await
        .unwrap();
    assert_eq!(codex_executions.len(), 1);
    assert_eq!(codex_executions[0].provider, "codex");
    assert_eq!(codex_executions[0].status, AgentStatus::Running);

    let summaries = scheduler::list_queue_summaries(&pool).await.unwrap();
    assert!(
        summaries.iter().any(|summary| {
            summary.provider_family.as_deref() == Some("gemini")
                && summary.top_reason == "provider_capacity"
                && summary.global_queue_depth == 1
        }),
        "claim/start must refresh scheduler projection in the same write unit for remaining backpressured work: {summaries:?}"
    );
}

#[tokio::test]
async fn invoke_agent_capacity_precheck_reports_when_all_pending_work_is_blocked() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let running_gemini_stage = StageExecutionId::new();
    let pending_gemini_stage = StageExecutionId::new();
    let pending_codex_stage = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061".into(),
            body: "capacity precheck".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    for (stage_execution_id, stage_id) in [
        (running_gemini_stage, "running_gemini"),
        (pending_gemini_stage, "pending_gemini"),
        (pending_codex_stage, "pending_codex"),
    ] {
        stages::insert(&pool, &make_stage(run_id, stage_execution_id, stage_id))
            .await
            .unwrap();
    }
    agent_executions::insert(
        &pool,
        &make_running_execution(running_gemini_stage, "gemini"),
    )
    .await
    .unwrap();

    let capacity = InvokeAgentCapacityConfig {
        global_active_agent_executions: 6,
        per_run_active_agent_executions: 10,
        provider_caps: BTreeMap::from([(ProviderFamily::Gemini, 1), (ProviderFamily::Codex, 3)]),
    };

    work_items::enqueue(
        &pool,
        &make_invoke_work_item(
            "gemini-pending",
            run_id,
            pending_gemini_stage,
            "pending_gemini",
            "gemini",
            -2,
        ),
    )
    .await
    .unwrap();

    assert!(
        !has_capacity_eligible_pending_invoke_agent_for_start(&pool, &capacity)
            .await
            .unwrap(),
        "precheck should report no eligible work when every pending item is capacity blocked"
    );

    work_items::enqueue(
        &pool,
        &make_invoke_work_item(
            "codex-pending",
            run_id,
            pending_codex_stage,
            "pending_codex",
            "codex",
            -1,
        ),
    )
    .await
    .unwrap();

    assert!(
        has_capacity_eligible_pending_invoke_agent_for_start(&pool, &capacity)
            .await
            .unwrap(),
        "precheck should report eligible work when a later candidate can run"
    );
}

#[tokio::test]
async fn invoke_agent_claim_prefers_least_recently_served_run_within_candidate_window() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let recently_served_run_id = RunId::new();
    let unserved_run_id = RunId::new();
    let recent_stage_execution_id = StageExecutionId::new();
    let unserved_stage_execution_id = StageExecutionId::new();
    let now = Utc::now();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 fairness".into(),
            body: "least recently served run wins".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(recently_served_run_id, idea_id))
        .await
        .unwrap();
    runs::insert(&pool, &make_run(unserved_run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(
            recently_served_run_id,
            recent_stage_execution_id,
            "recent_run_stage",
        ),
    )
    .await
    .unwrap();
    stages::insert(
        &pool,
        &make_stage(
            unserved_run_id,
            unserved_stage_execution_id,
            "unserved_run_stage",
        ),
    )
    .await
    .unwrap();
    scheduler::upsert_service_state(
        &pool,
        &scheduler::SchedulerServiceState {
            scope: "run".into(),
            scope_id: recently_served_run_id.to_string(),
            last_served_at: Some(now),
            last_claimed_work_item_id: Some("recent-previous".into()),
            updated_at: now,
        },
    )
    .await
    .unwrap();
    work_items::enqueue(
        &pool,
        &make_invoke_work_item(
            "recent-run-older-work",
            recently_served_run_id,
            recent_stage_execution_id,
            "recent_run_stage",
            "codex",
            -5,
        ),
    )
    .await
    .unwrap();
    work_items::enqueue(
        &pool,
        &make_invoke_work_item(
            "unserved-run-newer-work",
            unserved_run_id,
            unserved_stage_execution_id,
            "unserved_run_stage",
            "codex",
            -1,
        ),
    )
    .await
    .unwrap();

    let claimed = claim_next_invoke_agent_with_start_with_capacity(
        &pool,
        &InvokeAgentCapacityConfig::default(),
    )
    .await
    .unwrap()
    .expect("one InvokeAgent should be claimable");

    assert_eq!(claimed.work_item_id, "unserved-run-newer-work");
    let service_state = scheduler::get_service_state(&pool, "run", &unserved_run_id.to_string())
        .await
        .unwrap()
        .expect("claim/start must persist run service state");
    assert_eq!(
        service_state.last_claimed_work_item_id.as_deref(),
        Some("unserved-run-newer-work")
    );
}

#[tokio::test]
async fn work_queue_refresh_publishes_scheduler_backpressure_domain_event_on_transition() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let events = event_bus::new_bus(16);
    let mut rx = events.subscribe();
    let work_queue = WorkQueue::with_events(pool.clone(), events);
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    work_items::enqueue(
        &pool,
        &make_invoke_work_item(
            "old-codex-pending",
            run_id,
            stage_execution_id,
            "implementation",
            "codex",
            -360,
        ),
    )
    .await
    .unwrap();

    work_queue
        .refresh_scheduler_projection_with_capacity(
            &domain::provider::InvokeAgentCapacityConfig::default(),
        )
        .await
        .unwrap();
    assert!(
        rx.try_recv().is_err(),
        "first high-pressure snapshot should only arm pending_active"
    );

    work_queue
        .refresh_scheduler_projection_with_capacity(
            &domain::provider::InvokeAgentCapacityConfig::default(),
        )
        .await
        .unwrap();

    let event = rx.recv().await.unwrap();
    let domain::events::DomainEvent::SchedulerBackpressureChanged {
        run_id: event_run_id,
        provider_family,
        top_reason,
        queued_count,
        global_queue_depth,
        state,
        ..
    } = event
    else {
        panic!("expected scheduler backpressure event");
    };
    assert_eq!(event_run_id, Some(run_id.to_string()));
    assert_eq!(provider_family, Some("codex".into()));
    assert_eq!(top_reason, "queued");
    assert_eq!(queued_count, 1);
    assert_eq!(global_queue_depth, 1);
    assert_eq!(state, "active");
}

#[tokio::test]
async fn host_interruption_records_epoch_cancels_execution_and_requeues_invoke_work() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let now = Utc::now();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 host interruption".into(),
            body: "host interruption retry".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(run_id, stage_execution_id, "implementation"),
    )
    .await
    .unwrap();

    let mut execution = make_running_execution(stage_execution_id, "codex_cli");
    execution.started_at = now - Duration::seconds(45);
    execution.session_generation_id = Some("generation-cleanup-1".into());
    let execution_id = execution.id;
    agent_executions::insert(&pool, &execution).await.unwrap();

    let mut work_item = make_invoke_work_item(
        "codex-running",
        run_id,
        stage_execution_id,
        "implementation",
        "codex_cli",
        -40,
    );
    work_item.status = WorkItemStatus::Running;
    work_items::enqueue(&pool, &work_item).await.unwrap();

    let runtime_cleanup = Arc::new(RecordingRuntimeCleanup::default());
    let service = HostInterruptionService::with_capacity_config_and_runtime_cleanup(
        pool.clone(),
        WorkQueue::new(pool.clone()),
        domain::provider::InvokeAgentCapacityConfig::default(),
        runtime_cleanup.clone(),
    );
    let summary = service
        .record_and_requeue(HostInterruptionEvent {
            kind: HostInterruptionKind::SystemSleep,
            started_at: now - Duration::seconds(60),
            ended_at: Some(now),
            monotonic_gap_ms: None,
            wall_clock_gap_ms: Some(60_000),
            details_json: Some(r#"{"source":"test"}"#.into()),
        })
        .await
        .unwrap();

    assert_eq!(summary.affected_executions, 1);
    assert_eq!(summary.cancelled_executions, 1);
    assert_eq!(summary.retries_enqueued, 1);
    assert_eq!(summary.retries_missing_work_item, 0);
    assert_eq!(summary.runtime_cleanup_attempted, 1);
    assert_eq!(summary.runtime_cleanup_succeeded, 1);
    assert_eq!(summary.runtime_cleanup_failed, 0);
    assert_eq!(
        runtime_cleanup
            .closed_generations
            .lock()
            .unwrap()
            .as_slice(),
        ["generation-cleanup-1"]
    );

    let stored_execution = agent_executions::find_by_id(&pool, execution_id)
        .await
        .unwrap()
        .expect("execution should remain queryable");
    assert_eq!(stored_execution.status, AgentStatus::Cancelled);

    let pending = work_items::list_by_status(&pool, WorkItemStatus::Pending)
        .await
        .unwrap();
    let retry_item = pending
        .iter()
        .find(|item| item.id == "codex-running")
        .expect("host interruption should requeue the running InvokeAgent work item");
    let retry_payload: serde_json::Value = serde_json::from_str(&retry_item.payload_json).unwrap();
    assert_eq!(
        retry_payload["host_interruption_retry"]["stage_execution_id"],
        stage_execution_id.to_string()
    );
    assert!(retry_payload.get("p058_claimed").is_none());

    let readback = scheduler::list_host_interruption_epochs_by_run(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert_eq!(readback.len(), 1);
    assert_eq!(readback[0].epoch.id, summary.epoch_id);
    assert_eq!(readback[0].affected_executions.len(), 1);
    assert_eq!(
        readback[0].affected_executions[0].action,
        "recovering_from_system_sleep"
    );
    assert_eq!(readback[0].affected_executions[0].previous_status, "running");
    assert_eq!(readback[0].affected_executions[0].settlement_status, "retry_enqueued");
    assert_eq!(readback[0].affected_executions[0].cleanup_status, "succeeded");
    assert_eq!(
        readback[0].affected_executions[0].quota_budget_effect,
        "not_consumed"
    );
    assert!(
        readback[0].affected_executions[0]
            .retry_enqueued_at
            .is_some()
    );
    assert_eq!(
        readback[0].affected_executions[0]
            .provider_family
            .as_deref(),
        Some("codex")
    );

    let health = scheduler::latest_health_snapshot(&pool)
        .await
        .unwrap()
        .expect("host interruption should refresh scheduler health");
    assert_eq!(
        health.last_host_interruption_epoch_id.as_deref(),
        Some(summary.epoch_id.as_str())
    );
}

#[tokio::test]
async fn host_interruption_late_output_from_superseded_attempt_cannot_promote_over_retry_generation(
) {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let old_agent_execution_id = AgentExecutionId::new();
    let now = Utc::now();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 host late output".into(),
            body: "host retry late output settlement".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(run_id, stage_execution_id, "implementation"),
    )
    .await
    .unwrap();

    let mut old_execution = make_running_execution(stage_execution_id, "codex_cli");
    old_execution.id = old_agent_execution_id;
    old_execution.started_at = now - Duration::seconds(45);
    old_execution.session_generation_id = Some("generation-old".into());
    agent_executions::insert(&pool, &old_execution)
        .await
        .unwrap();

    let source_work_item_id = "codex-running-late-output";
    let mut work_item = make_invoke_work_item(
        source_work_item_id,
        run_id,
        stage_execution_id,
        "implementation",
        "codex_cli",
        -40,
    );
    work_item.status = WorkItemStatus::Running;
    work_items::enqueue(&pool, &work_item).await.unwrap();

    let old_claim_key = ArtifactSourceGenerationClaimKey {
        run_id,
        stage_execution_id,
        agent_execution_id: old_agent_execution_id,
        source_work_item_id: source_work_item_id.into(),
    };
    artifact_contracts::insert_source_generation_claim(
        &pool,
        ArtifactSourceGenerationClaim {
            key: old_claim_key.clone(),
            current_session_generation_id: Some("generation-old".into()),
            claim_state: ArtifactSourceClaimState::Active,
            superseding_work_item_id: None,
            superseded_by_agent_execution_id: None,
            supersession_journal_id: None,
            superseded_at: None,
            closed_at: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    let service = HostInterruptionService::new(pool.clone(), WorkQueue::new(pool.clone()));
    let summary = service
        .record_and_requeue(HostInterruptionEvent {
            kind: HostInterruptionKind::SystemSleep,
            started_at: now - Duration::seconds(60),
            ended_at: Some(now),
            monotonic_gap_ms: None,
            wall_clock_gap_ms: Some(60_000),
            details_json: Some(r#"{"source":"test"}"#.into()),
        })
        .await
        .unwrap();
    assert_eq!(summary.retries_enqueued, 1);

    let old_claim = artifact_contracts::load_source_generation_claim(&pool, &old_claim_key)
        .await
        .unwrap()
        .expect("old host-interrupted claim should remain as supersession evidence");
    assert_eq!(
        old_claim.claim_state,
        ArtifactSourceClaimState::SupersededPendingRetry
    );
    assert_eq!(
        old_claim.superseding_work_item_id.as_deref(),
        Some(source_work_item_id)
    );

    let new_agent_execution_id = AgentExecutionId::new();
    let mut new_execution = make_running_execution(stage_execution_id, "codex_cli");
    new_execution.id = new_agent_execution_id;
    new_execution.session_generation_id = Some("generation-new".into());
    agent_executions::insert(&pool, &new_execution)
        .await
        .unwrap();
    let new_claim_key = ArtifactSourceGenerationClaimKey {
        run_id,
        stage_execution_id,
        agent_execution_id: new_agent_execution_id,
        source_work_item_id: source_work_item_id.into(),
    };
    artifact_contracts::insert_source_generation_claim(
        &pool,
        ArtifactSourceGenerationClaim {
            key: new_claim_key.clone(),
            current_session_generation_id: Some("generation-new".into()),
            claim_state: ArtifactSourceClaimState::Active,
            superseding_work_item_id: None,
            superseded_by_agent_execution_id: None,
            supersession_journal_id: None,
            superseded_at: None,
            closed_at: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await
    .unwrap();

    let new_decision = artifact_contracts::import_generation_with_claim_cas(
        &pool,
        &new_claim_key,
        "generation-new",
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS".into(),
            generation_id: "new-retry-generation".into(),
            source_agent_execution_id: Some(new_agent_execution_id.to_string()),
            source_stage_execution_id: Some(stage_execution_id.to_string()),
            source_session_generation_id: Some("generation-new".into()),
            source_work_item_id: Some(source_work_item_id.into()),
            supersedes_generation_id: None,
            output_settlement: AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(new_decision, SourceGenerationImportDecision::Activated);

    let late_decision = artifact_contracts::import_generation_with_claim_cas(
        &pool,
        &old_claim_key,
        "generation-old",
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS".into(),
            generation_id: "late-old-generation".into(),
            source_agent_execution_id: Some(old_agent_execution_id.to_string()),
            source_stage_execution_id: Some(stage_execution_id.to_string()),
            source_session_generation_id: Some("generation-old".into()),
            source_work_item_id: Some(source_work_item_id.into()),
            supersedes_generation_id: None,
            output_settlement: AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        late_decision,
        SourceGenerationImportDecision::IgnoredLateOutputs
    );

    let active_generation_id: String = sqlx::query_scalar(
        "SELECT generation_id FROM active_artifact_contracts WHERE run_id = ?1 AND contract_id = ?2",
    )
    .bind(run_id.to_string())
    .bind("prepush_review_v1")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active_generation_id, "new-retry-generation",
        "late output from the host-interrupted attempt must not replace the retry generation"
    );

    let late_row: (i64, i64, String) = sqlx::query_as(
        "SELECT valid, source_generation_verified, output_settlement FROM artifact_contract_generations WHERE generation_id = ?1",
    )
    .bind("late-old-generation")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(late_row.0, 0);
    assert_eq!(late_row.1, 0);
    assert_eq!(
        late_row.2,
        AgentOutputSettlement::IgnoredLateOutputs.to_string()
    );
}

#[tokio::test]
async fn host_interruption_requires_runtime_cleanup_before_retry_enqueue() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let now = Utc::now();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 host cleanup".into(),
            body: "cleanup before retry".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(run_id, stage_execution_id, "implementation"),
    )
    .await
    .unwrap();

    let mut execution = make_running_execution(stage_execution_id, "codex_cli");
    execution.started_at = now - Duration::seconds(45);
    execution.session_generation_id = Some("generation-cleanup-fails".into());
    let execution_id = execution.id;
    agent_executions::insert(&pool, &execution).await.unwrap();

    let mut work_item = make_invoke_work_item(
        "codex-running-cleanup-fails",
        run_id,
        stage_execution_id,
        "implementation",
        "codex_cli",
        -40,
    );
    work_item.status = WorkItemStatus::Running;
    work_items::enqueue(&pool, &work_item).await.unwrap();

    let runtime_cleanup = Arc::new(RecordingRuntimeCleanup {
        closed_generations: Mutex::new(Vec::new()),
        fail_generation: Some("generation-cleanup-fails".into()),
    });
    let service = HostInterruptionService::with_capacity_config_and_runtime_cleanup(
        pool.clone(),
        WorkQueue::new(pool.clone()),
        domain::provider::InvokeAgentCapacityConfig::default(),
        runtime_cleanup.clone(),
    );

    let summary = service
        .record_and_requeue(HostInterruptionEvent {
            kind: HostInterruptionKind::SystemSleep,
            started_at: now - Duration::seconds(60),
            ended_at: Some(now),
            monotonic_gap_ms: None,
            wall_clock_gap_ms: Some(60_000),
            details_json: Some(r#"{"source":"test"}"#.into()),
        })
        .await
        .expect("host interruption should record cleanup failure");

    assert_eq!(summary.runtime_cleanup_attempted, 1);
    assert_eq!(summary.runtime_cleanup_succeeded, 0);
    assert_eq!(summary.runtime_cleanup_failed, 1);
    assert_eq!(summary.cancelled_executions, 1);
    assert_eq!(summary.retries_enqueued, 0);
    assert_eq!(summary.retries_deferred_cleanup_failed, 1);
    assert_eq!(
        runtime_cleanup
            .closed_generations
            .lock()
            .unwrap()
            .as_slice(),
        ["generation-cleanup-fails"]
    );

    let stored_execution = agent_executions::find_by_id(&pool, execution_id)
        .await
        .unwrap()
        .expect("execution should remain queryable");
    assert_eq!(
        stored_execution.status,
        AgentStatus::Cancelled,
        "cleanup failure should not block host interruption settlement"
    );

    let pending = work_items::list_by_status(&pool, WorkItemStatus::Pending)
        .await
        .unwrap();
    assert!(
        !pending
            .iter()
            .any(|item| item.id == "codex-running-cleanup-fails"),
        "cleanup failure must defer retry enqueue until cleanup succeeds"
    );
    let epochs = scheduler::list_host_interruption_epochs_by_run(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert!(
        !epochs.is_empty(),
        "cleanup failure should not block durable host interruption epoch creation"
    );
    let affected = &epochs[0].affected_executions[0];
    assert_eq!(affected.previous_status, "running");
    assert_eq!(affected.cleanup_status, "failed");
    assert_eq!(affected.settlement_status, "retry_deferred_cleanup_failed");
    assert_eq!(affected.quota_budget_effect, "not_consumed");
    assert_eq!(affected.retry_enqueued_at, None);
}

#[tokio::test]
async fn host_interruption_retry_does_not_consume_provider_quota_budget() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let now = Utc::now();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P061 host quota exemption".into(),
            body: "host retry should not spend provider quota budget".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    stages::insert(
        &pool,
        &make_stage(run_id, stage_execution_id, "implementation"),
    )
    .await
    .unwrap();

    let mut execution = make_running_execution(stage_execution_id, "codex_cli");
    execution.started_at = now - Duration::seconds(45);
    let execution_id = execution.id;
    agent_executions::insert(&pool, &execution).await.unwrap();

    let mut work_item = make_invoke_work_item(
        "codex-running-quota-exempt",
        run_id,
        stage_execution_id,
        "implementation",
        "codex_cli",
        -40,
    );
    work_item.status = WorkItemStatus::Running;
    work_items::enqueue(&pool, &work_item).await.unwrap();

    let service = HostInterruptionService::new(pool.clone(), WorkQueue::new(pool.clone()));
    let summary = service
        .record_and_requeue(HostInterruptionEvent {
            kind: HostInterruptionKind::SystemSleep,
            started_at: now - Duration::seconds(60),
            ended_at: Some(now),
            monotonic_gap_ms: None,
            wall_clock_gap_ms: Some(60_000),
            details_json: Some(r#"{"source":"test"}"#.into()),
        })
        .await
        .unwrap();

    assert_eq!(summary.affected_executions, 1);
    assert_eq!(summary.retries_enqueued, 1);

    let quota_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_retry_budget_ledger WHERE run_id = ?1 AND stage_execution_id = ?2",
    )
    .bind(run_id.to_string())
    .bind(stage_execution_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        quota_rows, 0,
        "host-interruption retries must not create or consume provider quota retry-budget rows"
    );

    let stored_execution = agent_executions::find_by_id(&pool, execution_id)
        .await
        .unwrap()
        .expect("execution should remain queryable");
    assert_eq!(stored_execution.status, AgentStatus::Cancelled);
}
