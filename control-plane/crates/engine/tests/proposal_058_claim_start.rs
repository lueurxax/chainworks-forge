use chrono::{Duration, Utc};
use db::pool::create_pool;
use db::repos::{
    agent_execution_runtime_facts, agent_executions, agent_retry_budget_ledger, artifact_contracts,
    ideas, runs, sessions, stages, work_items,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{AgentExecution, AgentOutputSettlement, AgentStatus, ArtifactSourceClaimState};
use domain::artifact_contracts::{
    ActiveArtifactGenerationInput, ArtifactSourceGenerationClaim, ArtifactSourceGenerationClaimKey,
    SourceGenerationImportDecision,
};
use domain::commands::{CallerContext, Command, RetryStageCmd};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, ArtifactId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::executor::BackgroundExecutor;
use engine::orchestrator::Orchestrator;
use engine::recovery::RecoveryService;
use engine::work_queue::WorkQueue;
use std::sync::Arc;

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

#[tokio::test]
async fn proposal_058_claim_start_precreates_execution_and_active_artifact_claim() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058".into(),
            body: "claim/start".into(),
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
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
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
        },
    )
    .await
    .unwrap();

    let now = Utc::now();
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "invoke-work-item-1".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "claude",
                "model": "sonnet",
                "prompt": "write code",
                "task_name": "code",
                "task_inputs": ["input"],
                "task_outputs": ["output"],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "code_writer",
                "worktree_write_enabled": false
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("implementation".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let claimed = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap()
        .expect("claim/start should claim InvokeAgent");

    assert_eq!(claimed.source_work_item_id, "invoke-work-item-1");
    assert_eq!(claimed.stage_execution_id, stage_execution_id);
    assert_eq!(claimed.run_id, run_id);
    assert_eq!(
        claimed.artifact_claim_key.agent_execution_id,
        claimed.agent_execution_id
    );
    let claimed_session_generation_id = claimed
        .session_generation_id
        .as_deref()
        .expect("session-scoped claim should carry a provisional generation id");
    assert_eq!(claimed_session_generation_id.len(), 36);

    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].id, claimed.agent_execution_id);
    assert_eq!(
        executions[0].session_generation_id.as_deref(),
        Some(claimed_session_generation_id)
    );
    let facts =
        agent_execution_runtime_facts::find_by_execution_id(&pool, claimed.agent_execution_id)
            .await
            .unwrap()
            .expect("claim/start should persist runtime facts");
    assert_eq!(
        facts.session_reuse_reason.as_deref(),
        Some("legacy_unknown")
    );

    let claim =
        artifact_contracts::load_source_generation_claim(&pool, &claimed.artifact_claim_key)
            .await
            .unwrap()
            .expect("active artifact claim");
    assert_eq!(claim.claim_state.to_string(), "active");
    assert_eq!(
        claim.current_session_generation_id.as_deref(),
        Some(claimed_session_generation_id)
    );

    let persisted_work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    let persisted_claimed = persisted_work_items
        .iter()
        .find(|item| item.id == claimed.work_item_id)
        .expect("claimed work item persisted");
    let persisted_payload: serde_json::Value =
        serde_json::from_str(&persisted_claimed.payload_json).unwrap();
    assert_eq!(
        persisted_payload["p058_claimed"]["agent_execution_id"],
        claimed.agent_execution_id.to_string()
    );
    assert_eq!(
        persisted_payload["p058_claimed"]["artifact_claim_key"]["source_work_item_id"],
        "invoke-work-item-1"
    );
    assert_eq!(
        persisted_payload["p058_claimed"]["session_policy_decision"]["generation"]["id"],
        claimed_session_generation_id
    );

    work_items::complete(&pool, &claimed.work_item_id)
        .await
        .unwrap();
    let settled_execution = agent_executions::find_by_id(&pool, claimed.agent_execution_id)
        .await
        .unwrap()
        .expect("preclaimed execution remains readable after work item completion");
    assert_eq!(
        settled_execution.status,
        AgentStatus::Completed,
        "completing a preclaimed InvokeAgent work item must not leave its execution running"
    );
    assert!(
        settled_execution.completed_at.is_some(),
        "completed preclaimed execution must have terminal timestamp"
    );
    let closed_claim =
        artifact_contracts::load_source_generation_claim(&pool, &claimed.artifact_claim_key)
            .await
            .unwrap()
            .expect("preclaimed artifact claim remains readable after work item completion");
    assert_eq!(
        closed_claim.claim_state.to_string(),
        "closed",
        "completing a preclaimed InvokeAgent work item must close its active source claim"
    );
    work_items::fail(&pool, &claimed.work_item_id, "late transport error")
        .await
        .unwrap();
    let terminal_work = work_items::list_by_run(&pool, run_id)
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.id == claimed.work_item_id)
        .expect("completed work item remains readable");
    assert_eq!(
        terminal_work.status,
        WorkItemStatus::Completed,
        "late failure must not overwrite terminal completed work item truth"
    );
}

#[tokio::test]
async fn proposal_058_claim_start_without_session_scope_does_not_fabricate_generation() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 no session scope".into(),
            body: "claim/start without reuse".into(),
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
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "docs".into(),
            label: "Docs".into(),
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
        },
    )
    .await
    .unwrap();

    let now = Utc::now();
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "invoke-work-item-no-session".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "docs",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "docs_guardian",
                "provider": "gemini",
                "model": "gemini-2.5-flash",
                "prompt": "check docs",
                "task_name": "docs",
                "task_inputs": [],
                "task_outputs": [],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": null,
                "session_family_id": null,
                "worktree_write_enabled": false
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("docs".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let claimed = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap()
        .expect("claim/start should claim non-session InvokeAgent");
    assert!(claimed.session_generation_id.is_none());

    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].id, claimed.agent_execution_id);
    assert!(executions[0].session_lineage_id.is_none());
    assert!(executions[0].session_generation_id.is_none());
    assert!(executions[0].session_reuse_disposition.is_none());

    let claim =
        artifact_contracts::load_source_generation_claim(&pool, &claimed.artifact_claim_key)
            .await
            .unwrap()
            .expect("active artifact claim");
    assert_eq!(claim.claim_state.to_string(), "active");
    assert!(claim.current_session_generation_id.is_none());

    let persisted_work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    let persisted_claimed = persisted_work_items
        .iter()
        .find(|item| item.id == claimed.work_item_id)
        .expect("claimed work item persisted");
    assert_eq!(persisted_claimed.status, WorkItemStatus::Running);
    let persisted_started_at: Option<String> =
        sqlx::query_scalar("SELECT started_at FROM work_items WHERE id = ?1")
            .bind(&claimed.work_item_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(persisted_started_at.is_some());
    let persisted_payload: serde_json::Value =
        serde_json::from_str(&persisted_claimed.payload_json).unwrap();
    assert_eq!(
        persisted_payload["p058_claimed"]["agent_execution_id"],
        claimed.agent_execution_id.to_string()
    );
    assert!(persisted_payload["p058_claimed"]
        .get("session_generation_id")
        .is_none());
    assert!(persisted_payload["p058_claimed"]
        .get("session_policy_decision")
        .is_none());
}

#[tokio::test]
async fn proposal_058_reclaimed_null_scope_payload_clears_legacy_fake_generation() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();
    let agent_execution_id = AgentExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 legacy no session scope".into(),
            body: "claim/start legacy fake generation".into(),
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
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "docs".into(),
            label: "Docs".into(),
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
        },
    )
    .await
    .unwrap();

    let claim_key = ArtifactSourceGenerationClaimKey {
        run_id,
        stage_execution_id,
        agent_execution_id,
        source_work_item_id: "legacy-preclaimed-no-session".into(),
    };
    let now = Utc::now();
    agent_executions::insert(
        &pool,
        &AgentExecution {
            id: agent_execution_id,
            stage_execution_id,
            agent_id: "docs_guardian".into(),
            provider: "gemini".into(),
            model: Some("gemini-2.5-flash".into()),
            status: AgentStatus::Running,
            started_at: now,
            completed_at: None,
            session_lineage_id: Some("fake-generation".into()),
            session_generation_id: Some("fake-generation".into()),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("owner".into()),
            session_reuse_scope: None,
            session_family_id: None,
            session_reuse_disposition: Some("fresh".into()),
            session_reset_reason: None,
            owner_execution_lineage_id: Some(stage_execution_id.to_string()),
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
            actual_xcode_runtime_observation_json: None,
        },
    )
    .await
    .unwrap();
    artifact_contracts::insert_source_generation_claim(
        &pool,
        ArtifactSourceGenerationClaim {
            key: claim_key.clone(),
            current_session_generation_id: Some("fake-generation".into()),
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
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "legacy-preclaimed-no-session".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "docs",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "docs_guardian",
                "provider": "gemini",
                "model": "gemini-2.5-flash",
                "prompt": "check docs",
                "task_name": "docs",
                "task_inputs": [],
                "task_outputs": [],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": null,
                "session_family_id": null,
                "worktree_write_enabled": false,
                "p058_claimed": {
                    "agent_execution_id": agent_execution_id.to_string(),
                    "artifact_claim_key": claim_key,
                    "session_generation_id": "fake-generation",
                    "session_policy_decision": {
                        "generation": { "id": "fake-generation" }
                    }
                }
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("docs".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let claimed = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap()
        .expect("legacy preclaim should be reclaimed");
    assert!(claimed.session_generation_id.is_none());

    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert!(executions[0].session_lineage_id.is_none());
    assert!(executions[0].session_generation_id.is_none());
    assert!(executions[0].session_reuse_disposition.is_none());

    let claim =
        artifact_contracts::load_source_generation_claim(&pool, &claimed.artifact_claim_key)
            .await
            .unwrap()
            .expect("claim remains active until output import closes it");
    assert!(claim.current_session_generation_id.is_none());

    let persisted_work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    let persisted_claimed = persisted_work_items
        .iter()
        .find(|item| item.id == claimed.work_item_id)
        .expect("claimed work item persisted");
    let persisted_payload: serde_json::Value =
        serde_json::from_str(&persisted_claimed.payload_json).unwrap();
    assert!(persisted_payload["p058_claimed"]
        .get("session_generation_id")
        .is_none());
    assert!(persisted_payload["p058_claimed"]
        .get("session_policy_decision")
        .is_none());
}

#[tokio::test]
async fn proposal_058_startup_recovery_requeues_preclaimed_invoke_without_new_execution() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 recovery".into(),
            body: "claim/start crash".into(),
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
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
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
        },
    )
    .await
    .unwrap();

    let now = Utc::now();
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "invoke-work-item-recovery".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "claude",
                "model": "sonnet",
                "prompt": "write code",
                "task_name": "code",
                "task_inputs": ["input"],
                "task_outputs": ["output"],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "code_writer",
                "worktree_write_enabled": false
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("implementation".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let claimed_before_crash = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap()
        .expect("claim/start should preclaim execution before crash");

    let recovery = RecoveryService::new(
        pool.clone(),
        WorkQueue::new(pool.clone()),
        event_bus::new_bus(16),
    );
    let summary = recovery.run_startup_repair().await.unwrap();
    assert_eq!(summary.work_items_requeued, 1);

    let stage_after_repair = stages::find_by_id(&pool, stage_execution_id)
        .await
        .unwrap()
        .expect("stage remains present");
    assert_eq!(stage_after_repair.status, StageStatus::Running);

    let pending = work_items::list_by_status(&pool, WorkItemStatus::Pending)
        .await
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "invoke-work-item-recovery");

    let claimed_after_recovery = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap()
        .expect("recovery should reclaim durable preclaimed InvokeAgent");
    assert_eq!(
        claimed_after_recovery.agent_execution_id,
        claimed_before_crash.agent_execution_id
    );
    assert_eq!(
        claimed_after_recovery.artifact_claim_key,
        claimed_before_crash.artifact_claim_key
    );
    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].id, claimed_before_crash.agent_execution_id);
}

#[tokio::test]
async fn proposal_058_startup_repair_settles_terminal_preclaimed_invoke_execution() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 terminal settlement".into(),
            body: "terminal InvokeAgent work item settles its preclaimed execution".into(),
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
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
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
        },
    )
    .await
    .unwrap();

    let now = Utc::now();
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "invoke-terminal-before-repair".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "claude",
                "model": "sonnet",
                "declared_outputs": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "code_writer"
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("implementation".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let claimed = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap()
        .expect("claim/start should preclaim execution");
    work_items::fail(
        &pool,
        "invoke-terminal-before-repair",
        "provider startup failed",
    )
    .await
    .unwrap();

    let recovery = RecoveryService::new(
        pool.clone(),
        WorkQueue::new(pool.clone()),
        event_bus::new_bus(16),
    );
    let summary = recovery.run_startup_repair().await.unwrap();
    assert_eq!(summary.agent_executions_settled, 1);

    let execution = agent_executions::find_by_id(&pool, claimed.agent_execution_id)
        .await
        .unwrap()
        .expect("preclaimed execution remains addressable");
    assert_eq!(execution.status, AgentStatus::Failed);
    assert!(
        execution.completed_at.is_some(),
        "terminal preclaimed execution must receive completed_at"
    );
}

#[tokio::test]
async fn proposal_058_sessionless_invoke_agent_fails_closed_before_execution_creation() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("sessionless-invoke.sqlite");
    let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
        .await
        .unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 sessionless".into(),
            body: "fallback removal".into(),
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
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
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
        },
    )
    .await
    .unwrap();

    let now = Utc::now() - Duration::hours(1);
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "legacy-sessionless-invoke".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "claude",
                "model": "sonnet"
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("implementation".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let claimed = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap();
    assert!(claimed.is_none());

    let failed = work_items::list_by_status(&pool, WorkItemStatus::Failed)
        .await
        .unwrap();
    let all_work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    assert_eq!(failed.len(), 1, "all work items: {all_work_items:?}");
    assert_eq!(failed[0].id, "legacy-sessionless-invoke");
    assert!(failed[0]
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("session_reuse_scope"));
    let queue = WorkQueue::new(pool.clone());
    assert!(
        queue.claim_next().await.unwrap().is_none(),
        "generic queue claim must not pick up InvokeAgent fallback items"
    );
    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert!(executions.is_empty());
}

#[tokio::test]
async fn proposal_058_explicit_null_session_reuse_scope_claims_as_no_reuse() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 null reuse".into(),
            body: "explicit null session reuse scope".into(),
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
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
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
        },
    )
    .await
    .unwrap();

    let now = Utc::now();
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "explicit-null-reuse-invoke".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "docs_guardian",
                "provider": "gemini",
                "model": "flash",
                "session_reuse_scope": null,
                "session_family_id": null,
                "declared_outputs": []
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("implementation".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let claimed = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap()
        .expect("explicit null session_reuse_scope should mean no reuse, not legacy missing");

    assert_eq!(claimed.source_work_item_id, "explicit-null-reuse-invoke");
    let failed = work_items::list_by_status(&pool, WorkItemStatus::Failed)
        .await
        .unwrap();
    assert!(
        failed.is_empty(),
        "explicit null reuse must not fail closed"
    );
    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].session_reuse_scope, None);
}

#[tokio::test]
async fn proposal_058_production_executor_fails_sessionless_invoke_before_processing() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("sessionless-production-invoke.sqlite");
    let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
        .await
        .unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 production sessionless".into(),
            body: "production fallback removal".into(),
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
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
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
        },
    )
    .await
    .unwrap();

    let now = Utc::now() - Duration::hours(1);
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "legacy-sessionless-production-invoke".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "claude",
                "model": "sonnet"
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("implementation".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let queue = WorkQueue::new(pool.clone());
    let events = event_bus::new_bus(16);
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        queue,
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    let processed = executor.process_next_item().await.unwrap();
    assert!(!processed);

    let failed = work_items::list_by_status(&pool, WorkItemStatus::Failed)
        .await
        .unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].id, "legacy-sessionless-production-invoke");
    assert!(failed[0]
        .last_error
        .as_deref()
        .unwrap_or_default()
        .contains("session_reuse_scope"));

    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert!(executions.is_empty());
}

#[tokio::test]
async fn proposal_058_production_loop_claims_pending_invoke_agent_items() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("production-loop-invoke.sqlite");
    let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
        .await
        .unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 production loop".into(),
            body: "production loop claims invoke_agent".into(),
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
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
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
        },
    )
    .await
    .unwrap();

    let now = Utc::now() - Duration::minutes(1);
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "production-loop-invoke".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "fixture-missing",
                "model": "fixture",
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "code_writer",
                "declared_outputs": []
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("implementation".into()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let queue = WorkQueue::new(pool.clone());
    let events = event_bus::new_bus(16);
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        queue.clone(),
    ));
    let executor = Arc::new(BackgroundExecutor::new(
        pool.clone(),
        queue,
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    ));
    let handle = Arc::clone(&executor).start();

    let status = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let items = work_items::list_by_run(&pool, run_id).await.unwrap();
            let item = items
                .iter()
                .find(|item| item.id == "production-loop-invoke")
                .unwrap();
            if matches!(
                item.status,
                WorkItemStatus::Completed | WorkItemStatus::Failed | WorkItemStatus::Cancelled
            ) {
                return item.status.clone();
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("production executor loop must claim InvokeAgent work without a non-invoke wake item");

    handle.abort();

    assert_ne!(status, WorkItemStatus::Pending);
    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(
        executions.len(),
        1,
        "claim/start must create the AgentExecution before provider execution"
    );
    assert_eq!(
        executions[0].status,
        AgentStatus::Failed,
        "ACP startup errors must not leave precreated AgentExecution rows running"
    );
    assert!(
        executions[0].completed_at.is_some(),
        "failed startup execution must be terminal"
    );
    let session_generation_id = executions[0]
        .session_generation_id
        .as_deref()
        .expect("preclaimed execution must be rebound to real session generation before ACP");
    let generation = sessions::find_generation_by_id(&pool, session_generation_id)
        .await
        .unwrap()
        .expect("session generation referenced by preclaimed execution must exist");
    assert_eq!(generation.runtime_provider, "fixture-missing");
    assert_eq!(generation.runtime_model, "fixture");
    let claim_key = ArtifactSourceGenerationClaimKey {
        run_id,
        stage_execution_id,
        agent_execution_id: executions[0].id,
        source_work_item_id: "production-loop-invoke".into(),
    };
    let claim = artifact_contracts::load_source_generation_claim(&pool, &claim_key)
        .await
        .unwrap()
        .expect("preclaimed artifact claim remains addressable");
    assert_eq!(
        claim.current_session_generation_id.as_deref(),
        Some(session_generation_id),
        "artifact claim CAS token must track the real session generation, not the preclaim placeholder"
    );
}

#[tokio::test]
async fn proposal_058_retry_stage_supersedes_old_claim_before_retry_work_is_claimed() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_execution_id = StageExecutionId::new();
    let old_agent_execution_id = AgentExecutionId::new();
    let source_work_item_id = "old-invoke-work-item".to_string();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 retry".into(),
            body: "supersession".into(),
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
        &StageExecution {
            id: old_stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
            status: StageStatus::Failed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        },
    )
    .await
    .unwrap();
    agent_executions::insert(
        &pool,
        &AgentExecution {
            id: old_agent_execution_id,
            stage_execution_id: old_stage_execution_id,
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_execution_lineage_id: Some(old_stage_execution_id.to_string()),
            session_lineage_id: Some("lineage-1".into()),
            session_generation_id: Some("generation-1".into()),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("owner-key".into()),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("code_writer".into()),
            session_reuse_disposition: Some("fresh".into()),
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
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: None,
        },
    )
    .await
    .unwrap();

    let old_claim_key = ArtifactSourceGenerationClaimKey {
        run_id,
        stage_execution_id: old_stage_execution_id,
        agent_execution_id: old_agent_execution_id,
        source_work_item_id: source_work_item_id.clone(),
    };
    artifact_contracts::insert_source_generation_claim(
        &pool,
        ArtifactSourceGenerationClaim {
            key: old_claim_key.clone(),
            current_session_generation_id: Some("generation-1".into()),
            claim_state: ArtifactSourceClaimState::Active,
            superseding_work_item_id: None,
            superseded_by_agent_execution_id: None,
            supersession_journal_id: None,
            superseded_at: None,
            closed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
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
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();

    let claim = artifact_contracts::load_source_generation_claim(&pool, &old_claim_key)
        .await
        .unwrap()
        .expect("old claim remains as pending supersession evidence");
    assert_eq!(
        claim.claim_state,
        ArtifactSourceClaimState::SupersededPendingRetry
    );
    assert_eq!(
        claim.supersession_journal_id.as_deref(),
        Some(commanded.journal_id.as_str())
    );
    let superseding_id = claim
        .superseding_work_item_id
        .as_deref()
        .expect("retry superseding item id");
    assert!(
        superseding_id.starts_with("p058-invoke:"),
        "superseding id must identify the retry InvokeAgent work item, got {superseding_id}"
    );
    let pending = work_items::list_by_status(&pool, WorkItemStatus::Pending)
        .await
        .unwrap();
    assert!(
        !pending.iter().any(|item| item.id == superseding_id),
        "retry claim must not store the AdvanceRun work item id as superseding_work_item_id"
    );

    let decision = artifact_contracts::import_generation_with_claim_cas(
        &pool,
        &old_claim_key,
        "generation-1",
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS".into(),
            generation_id: "late-old-generation".into(),
            source_agent_execution_id: Some(old_agent_execution_id.to_string()),
            source_stage_execution_id: Some(old_stage_execution_id.to_string()),
            source_session_generation_id: Some("generation-1".into()),
            source_work_item_id: Some(source_work_item_id),
            supersedes_generation_id: None,
            output_settlement: AgentOutputSettlement::ValidOutputsFromCompletedExecution,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(decision, SourceGenerationImportDecision::IgnoredLateOutputs);
}

#[tokio::test]
async fn proposal_058_retry_stage_requires_explicit_quota_budget_before_reset() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_execution_id = StageExecutionId::new();
    let old_agent_execution_id = AgentExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 quota retry".into(),
            body: "quota budget".into(),
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
        &StageExecution {
            id: old_stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
            status: StageStatus::Failed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        },
    )
    .await
    .unwrap();
    agent_executions::insert(
        &pool,
        &AgentExecution {
            id: old_agent_execution_id,
            stage_execution_id: old_stage_execution_id,
            agent_id: "code_writer".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_execution_lineage_id: Some(old_stage_execution_id.to_string()),
            session_lineage_id: Some("lineage-1".into()),
            session_generation_id: Some("generation-1".into()),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("owner-key".into()),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("code_writer".into()),
            session_reuse_disposition: Some("fresh".into()),
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
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: None,
        },
    )
    .await
    .unwrap();
    let retry_after = Utc::now() + Duration::hours(1);
    let ledger = agent_retry_budget_ledger::upsert_quota_failure(
        &pool,
        run_id,
        old_stage_execution_id,
        old_agent_execution_id,
        Some(retry_after),
    )
    .await
    .unwrap();

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let blocked = match handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
            }),
            CallerContext::test_fixture(),
        )
        .await
    {
        Ok(_) => panic!("quota retry before reset should require explicit budget consumption"),
        Err(error) => error,
    };
    assert!(
        blocked
            .to_string()
            .contains("quota retry_after has not elapsed"),
        "unexpected quota retry error: {blocked}"
    );
    let pending_after_block = work_items::list_by_status(&pool, WorkItemStatus::Pending)
        .await
        .unwrap();
    assert!(pending_after_block.is_empty());

    let commanded = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "implementation".into(),
                consume_quota_budget_now: true,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
            }),
            CallerContext::test_fixture(),
        )
        .await
        .unwrap();
    assert!(matches!(
        commanded.result,
        engine::command_handler::CommandResult::StageRetryScheduled { .. }
    ));

    let rows = sqlx::query(
        "SELECT normal_budget_consumed, early_retry_journal_id, state FROM agent_retry_budget_ledger WHERE id = ?1",
    )
    .bind(&ledger.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    use sqlx::Row;
    assert_eq!(rows[0].get::<i64, _>("normal_budget_consumed"), 1);
    assert_eq!(
        rows[0].get::<Option<String>, _>("early_retry_journal_id"),
        Some(commanded.journal_id)
    );
    assert_eq!(rows[0].get::<String, _>("state"), "early_retry_consumed");
}
