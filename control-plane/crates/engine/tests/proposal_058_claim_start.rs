use chrono::{Duration, Utc};
use db::pool::create_pool;
use db::repos::{
    agent_execution_runtime_facts, agent_executions, agent_retry_budget_ledger, artifact_contracts,
    escalation as escalation_repo, ideas, runs, sessions, stages, work_items,
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
use domain::mediation::OwnerKind;
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::executor::BackgroundExecutor;
use engine::orchestrator::Orchestrator;
use engine::recovery::RecoveryService;
use engine::work_queue::WorkQueue;
use std::collections::HashMap;
use std::sync::Arc;

async fn setup_memory_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    let writer = Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .unwrap();
    pool
}

async fn setup_file_backed_pool(path: &str) -> sqlx::SqlitePool {
    let pool = create_pool(path).await.unwrap();
    let writer = Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, writer)
        .await
        .unwrap();
    pool
}

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
        review_routing_json: None,
        closeout_readiness_mode: None,
    }
}

fn test_workflow_yaml_path() -> String {
    format!(
        "{}/../../../examples/workflows/full-mvp-live.yaml",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn test_agent_catalog_yaml_path() -> String {
    format!(
        "{}/../../../examples/agents/agents.yaml",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[tokio::test]
async fn proposal_058_claim_start_precreates_execution_and_active_artifact_claim() {
    let pool = setup_memory_pool().await;
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
    let pool = setup_memory_pool().await;
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
    let pool = setup_memory_pool().await;
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
        owner_kind: OwnerKind::StageExecution,
        owner_id: stage_execution_id.to_string(),
        stage_execution_id: Some(stage_execution_id),
        agent_execution_id,
        source_work_item_id: "legacy-preclaimed-no-session".into(),
    };
    let now = Utc::now();
    agent_executions::insert(
        &pool,
        &AgentExecution {
            id: agent_execution_id,
            stage_execution_id: Some(stage_execution_id),
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
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
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
async fn proposal_058_startup_recovery_requeues_preclaimed_invoke_with_fresh_execution() {
    let pool = setup_memory_pool().await;
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

    let payload_after_recovery: serde_json::Value =
        serde_json::from_str(&pending[0].payload_json).unwrap();
    assert!(
        payload_after_recovery.get("p058_claimed").is_none(),
        "startup recovery must clear stale claim/session ownership before retry"
    );

    let old_execution =
        agent_executions::find_by_id(&pool, claimed_before_crash.agent_execution_id)
            .await
            .unwrap()
            .expect("abandoned preclaimed execution remains auditable");
    assert_eq!(
        old_execution.status,
        AgentStatus::Cancelled,
        "startup recovery must terminalize abandoned preclaimed executions"
    );
    assert!(old_execution.completed_at.is_some());

    let old_claim = artifact_contracts::load_source_generation_claim(
        &pool,
        &claimed_before_crash.artifact_claim_key,
    )
    .await
    .unwrap()
    .expect("abandoned source-generation claim remains auditable");
    assert_eq!(
        old_claim.claim_state,
        ArtifactSourceClaimState::SupersededPendingRetry
    );
    assert_eq!(
        old_claim.superseding_work_item_id.as_deref(),
        Some("invoke-work-item-recovery")
    );

    let claimed_after_recovery = engine::executor::claim_next_invoke_agent_with_start(&pool)
        .await
        .unwrap()
        .expect("recovery should start a fresh InvokeAgent execution");
    assert_ne!(
        claimed_after_recovery.agent_execution_id, claimed_before_crash.agent_execution_id,
        "recovery retry must not bind to the crashed ACP/session execution"
    );
    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(executions.len(), 2);
    assert!(executions.iter().any(|execution| execution.id
        == claimed_before_crash.agent_execution_id
        && execution.status == AgentStatus::Cancelled));
    assert!(executions.iter().any(|execution| execution.id
        == claimed_after_recovery.agent_execution_id
        && execution.status == AgentStatus::Running));
}

#[tokio::test]
async fn proposal_058_xcode_mcp_invoke_claim_respects_configured_xcode_capacity() {
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 Xcode single flight".into(),
            body: "startup retry modal guard".into(),
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
    for item_id in ["xcode-invoke-1", "xcode-invoke-2", "xcode-invoke-3"] {
        work_items::enqueue(
            &pool,
            &WorkItem {
                id: item_id.into(),
                kind: WorkItemKind::InvokeAgent,
                payload_json: serde_json::json!({
                    "stage_id": "implementation",
                    "stage_execution_id": stage_execution_id.to_string(),
                    "agent_id": item_id,
                    "provider": "claude",
                    "model": "sonnet",
                    "prompt": "use Xcode MCP",
                    "task_name": item_id,
                    "task_inputs": ["input"],
                    "task_outputs": ["output"],
                    "declared_outputs": [],
                    "requested_mcp_server_ids": ["xcode"],
                    "xcode_broker_required": true,
                    "session_reuse_scope": "same_agent_family_within_run",
                    "session_family_id": item_id,
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
    }

    let capacity = engine::executor::InvokeAgentCapacityConfig {
        max_active_total: 10,
        max_active_per_run: 10,
        max_active_xcode_mcp: 2,
        provider_caps: HashMap::from([("claude".to_string(), 10)]),
    };

    let claimed =
        engine::executor::claim_next_invoke_agent_with_start_with_capacity(&pool, &capacity)
            .await
            .unwrap()
            .expect("first Xcode MCP invocation should start");
    assert_eq!(claimed.source_work_item_id, "xcode-invoke-1");

    let second =
        engine::executor::claim_next_invoke_agent_with_start_with_capacity(&pool, &capacity)
            .await
            .unwrap()
            .expect("second Xcode MCP invocation should start while below configured cap");
    assert_eq!(second.source_work_item_id, "xcode-invoke-2");

    let third =
        engine::executor::claim_next_invoke_agent_with_start_with_capacity(&pool, &capacity)
            .await
            .unwrap();
    assert!(
        third.is_none(),
        "a third Xcode MCP invocation must wait once the configured cap is reached"
    );
}

#[tokio::test]
async fn proposal_090_junie_preflight_running_does_not_consume_provider_capacity_until_launch() {
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let first_stage_execution_id = StageExecutionId::new();
    let second_stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P090 preflight capacity".into(),
            body: "preflight must not consume Junie provider capacity".into(),
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
        (first_stage_execution_id, "implementation_a"),
        (second_stage_execution_id, "implementation_b"),
    ] {
        stages::insert(
            &pool,
            &StageExecution {
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
            },
        )
        .await
        .unwrap();
    }

    let preflight_execution_id = AgentExecutionId::new();
    let now = Utc::now();
    agent_executions::insert(
        &pool,
        &AgentExecution {
            id: preflight_execution_id,
            stage_execution_id: Some(first_stage_execution_id),
            agent_id: "code_writer".into(),
            provider: "junie".into(),
            model: Some("junie".into()),
            status: AgentStatus::Running,
            started_at: now,
            completed_at: None,
            session_lineage_id: Some("junie-preflight".into()),
            session_generation_id: Some("junie-preflight".into()),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("code_writer".into()),
            session_reuse_scope: None,
            session_family_id: None,
            session_reuse_disposition: Some("fresh".into()),
            session_reset_reason: None,
            owner_execution_lineage_id: Some(first_stage_execution_id.to_string()),
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
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
        },
    )
    .await
    .unwrap();
    let mut preflight_facts =
        domain::agent::AgentExecutionRuntimeFacts::defaults_for(preflight_execution_id, now);
    preflight_facts.runtime_preflight_phase = Some("preflight_running".into());
    preflight_facts.runtime_preflight_provider_launched = Some(false);
    preflight_facts.runtime_preflight_attempt_count = Some(1);
    agent_execution_runtime_facts::upsert(&pool, &preflight_facts)
        .await
        .unwrap();

    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "junie-invoke-after-preflight".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation_b",
                "stage_execution_id": second_stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "junie",
                "model": "junie",
                "prompt": "continue",
                "task_name": "code_writer",
                "task_inputs": ["input"],
                "task_outputs": ["output"],
                "declared_outputs": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "code_writer",
                "worktree_write_enabled": true
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("implementation_b".into()),
            created_at: Utc::now(),
            scheduled_at: Utc::now(),
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();

    let capacity = engine::executor::InvokeAgentCapacityConfig {
        max_active_total: 10,
        max_active_per_run: 10,
        max_active_xcode_mcp: 10,
        provider_caps: HashMap::from([("junie".to_string(), 1)]),
    };

    let claimed =
        engine::executor::claim_next_invoke_agent_with_start_with_capacity(&pool, &capacity)
            .await
            .unwrap()
            .expect("preflight-only Junie execution must not consume provider capacity");
    assert_eq!(claimed.source_work_item_id, "junie-invoke-after-preflight");
}

#[tokio::test]
async fn proposal_058_startup_repair_settles_terminal_preclaimed_invoke_execution() {
    let pool = setup_memory_pool().await;
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
    let pool = setup_file_backed_pool(&format!("sqlite://{}", db_path.to_string_lossy())).await;
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
    if let Some(non_invoke_item) = queue.claim_next().await.unwrap() {
        assert_eq!(
            non_invoke_item.kind,
            WorkItemKind::AdvanceRun,
            "generic queue claim must not pick up InvokeAgent fallback items"
        );
    }
    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert!(executions.is_empty());
}

#[tokio::test]
async fn proposal_058_explicit_null_session_reuse_scope_claims_as_no_reuse() {
    let pool = setup_memory_pool().await;
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
async fn proposal_058_declared_output_claim_gets_durable_generation_without_reuse_scope() {
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 declared output lineage".into(),
            body: "declared outputs need repair-capable session identity".into(),
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
            stage_id: "state_9_implementation_reviewed".into(),
            label: "Implementation reviewed".into(),
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
            id: "declared-output-null-reuse-invoke".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "state_9_implementation_reviewed",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "security_checker",
                "provider": "claude",
                "model": "sonnet",
                "prompt": "check security",
                "task_name": "check_implementation_security",
                "task_inputs": ["approved_proposal", "changed_files_manifest"],
                "task_outputs": ["security_report"],
                "declared_outputs": [
                    {
                        "output_name": "security_report",
                        "target_path": "/tmp/workspace/.chainworks/runs/run/security/report.json"
                    }
                ],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": null,
                "session_family_id": null,
                "worktree_write_enabled": false
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("state_9_implementation_reviewed".into()),
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
        .expect("declared-output invocation should be claimed");
    let claimed_generation = claimed
        .session_generation_id
        .as_deref()
        .expect("declared outputs need a durable generation for output repair");
    assert_eq!(claimed_generation.len(), 36);

    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].session_reuse_scope, None);
    assert_eq!(
        executions[0].session_generation_id.as_deref(),
        Some(claimed_generation)
    );

    let claim =
        artifact_contracts::load_source_generation_claim(&pool, &claimed.artifact_claim_key)
            .await
            .unwrap()
            .expect("active artifact claim");
    assert_eq!(
        claim.current_session_generation_id.as_deref(),
        Some(claimed_generation)
    );

    let persisted_work_items = work_items::list_by_run(&pool, run_id).await.unwrap();
    let persisted_claimed = persisted_work_items
        .iter()
        .find(|item| item.id == claimed.work_item_id)
        .expect("claimed work item persisted");
    let persisted_payload: serde_json::Value =
        serde_json::from_str(&persisted_claimed.payload_json).unwrap();
    assert_eq!(
        persisted_payload["p058_claimed"]["session_policy_decision"]["generation"]["id"],
        claimed_generation
    );
}

#[tokio::test]
async fn proposal_058_production_executor_fails_sessionless_invoke_before_processing() {
    let tempdir = tempfile::tempdir().unwrap();
    let db_path = tempdir.path().join("sessionless-production-invoke.sqlite");
    let pool = setup_file_backed_pool(&format!("sqlite://{}", db_path.to_string_lossy())).await;
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
    let pool = setup_file_backed_pool(&format!("sqlite://{}", db_path.to_string_lossy())).await;
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
        owner_kind: OwnerKind::StageExecution,
        owner_id: stage_execution_id.to_string(),
        stage_execution_id: Some(stage_execution_id),
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
    let pool = setup_memory_pool().await;
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
            stage_execution_id: Some(old_stage_execution_id),
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
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
        },
    )
    .await
    .unwrap();

    let old_claim_key = ArtifactSourceGenerationClaimKey {
        run_id,
        owner_kind: OwnerKind::StageExecution,
        owner_id: old_stage_execution_id.to_string(),
        stage_execution_id: Some(old_stage_execution_id),
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
                operator_instruction: None,
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
async fn proposal_078_retry_release_stage_requires_effect_reconciliation_before_state_changes() {
    std::env::set_var("CHAINWORKS_P078_HEURISTIC_RETRY_GUARD_ENABLED", "1");
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_execution_id = StageExecutionId::new();
    let old_agent_execution_id = AgentExecutionId::new();
    let source_work_item_id = "release-invoke-work-item".to_string();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P078 retry".into(),
            body: "release guard".into(),
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
            stage_id: "commit_and_push_to_github".into(),
            label: "Commit and push".into(),
            status: StageStatus::Failed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_agent: Some("commit_and_push_to_github".into()),
            provider: Some("claude".into()),
            model: Some("sonnet".into()),
            stage_type: Some("release".into()),
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
            stage_execution_id: Some(old_stage_execution_id),
            agent_id: "commit_and_push_to_github".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_execution_lineage_id: Some(old_stage_execution_id.to_string()),
            session_lineage_id: Some("release-lineage-1".into()),
            session_generation_id: Some("release-generation-1".into()),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("release-owner-key".into()),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("commit_and_push_to_github".into()),
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
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
        },
    )
    .await
    .unwrap();

    let old_claim_key = ArtifactSourceGenerationClaimKey {
        run_id,
        owner_kind: OwnerKind::StageExecution,
        owner_id: old_stage_execution_id.to_string(),
        stage_execution_id: Some(old_stage_execution_id),
        agent_execution_id: old_agent_execution_id,
        source_work_item_id: source_work_item_id.clone(),
    };
    artifact_contracts::insert_source_generation_claim(
        &pool,
        ArtifactSourceGenerationClaim {
            key: old_claim_key.clone(),
            current_session_generation_id: Some("release-generation-1".into()),
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
    let result = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "commit_and_push_to_github".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: None,
            }),
            CallerContext::test_fixture(),
        )
        .await;
    let err = match result {
        Ok(_) => panic!("release retry must fail closed before state changes"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("requires_effect_reconciliation"),
        "release retry must fail closed with a typed reconciliation error: {err}"
    );

    let stages_after = stages::list_by_run(&pool, run_id).await.unwrap();
    assert_eq!(
        stages_after.len(),
        1,
        "release retry must not create a replacement stage"
    );
    assert_eq!(stages_after[0].id, old_stage_execution_id);
    assert_eq!(stages_after[0].status, StageStatus::Failed);

    let claim = artifact_contracts::load_source_generation_claim(&pool, &old_claim_key)
        .await
        .unwrap()
        .expect("old release claim remains addressable");
    assert_eq!(claim.claim_state, ArtifactSourceClaimState::Active);
    assert!(
        claim.superseding_work_item_id.is_none(),
        "release retry preflight must not supersede artifact claims"
    );

    let pending = work_items::list_by_status(&pool, WorkItemStatus::Pending)
        .await
        .unwrap();
    assert!(
        pending.is_empty(),
        "release retry preflight must not enqueue retry work"
    );
}

#[tokio::test]
async fn proposal_078_retry_manual_release_gate_checks_post_approval_release_tasks() {
    std::env::set_var("CHAINWORKS_P078_HEURISTIC_RETRY_GUARD_ENABLED", "1");
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P078 manual release retry".into(),
            body: "post approval release guard".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    let mut run = make_run(run_id, idea_id);
    run.status = RunStatus::Blocked;
    run.current_state = Some("state_11_manual_release".into());
    run.workflow_yaml_path = Some(test_workflow_yaml_path());
    run.agent_catalog_yaml_path = Some(test_agent_catalog_yaml_path());
    runs::insert(&pool, &run).await.unwrap();
    stages::insert(
        &pool,
        &StageExecution {
            id: old_stage_execution_id,
            run_id,
            stage_id: "state_11_manual_release".into(),
            label: "Manual release".into(),
            status: StageStatus::Failed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_agent: Some("lead_orchestrator".into()),
            provider: Some("claude".into()),
            model: Some("sonnet".into()),
            stage_type: Some("manual_gate".into()),
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        },
    )
    .await
    .unwrap();

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let result = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "state_11_manual_release".into(),
                consume_quota_budget_now: false,
                agent_execution_id: None,
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: None,
            }),
            CallerContext::test_fixture(),
        )
        .await;
    let err = match result {
        Ok(_) => panic!("manual release retry must fail closed before state changes"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("requires_effect_reconciliation"),
        "manual release retry must be guarded by post_approval release tasks: {err}"
    );
    let stages_after = stages::list_by_run(&pool, run_id).await.unwrap();
    assert_eq!(
        stages_after.len(),
        1,
        "manual release retry must not create a replacement stage"
    );
    assert_eq!(stages_after[0].id, old_stage_execution_id);
    assert_eq!(stages_after[0].status, StageStatus::Failed);
}

#[tokio::test]
async fn proposal_078_targeted_release_retry_records_failed_journal_entry() {
    std::env::set_var("CHAINWORKS_P078_HEURISTIC_RETRY_GUARD_ENABLED", "1");
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let old_stage_execution_id = StageExecutionId::new();
    let old_agent_execution_id = AgentExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P078 targeted retry".into(),
            body: "targeted release audit".into(),
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
            stage_id: "state_11_manual_release".into(),
            label: "Manual release".into(),
            status: StageStatus::Failed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_agent: Some("lead_orchestrator".into()),
            provider: Some("claude".into()),
            model: Some("sonnet".into()),
            stage_type: Some("manual_gate".into()),
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
            stage_execution_id: Some(old_stage_execution_id),
            agent_id: "commit_and_push_to_github".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_execution_lineage_id: Some(old_stage_execution_id.to_string()),
            session_lineage_id: Some("release-lineage-1".into()),
            session_generation_id: Some("release-generation-1".into()),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: Some("release-owner-key".into()),
            session_reuse_scope: Some("same_agent_family_within_run".into()),
            session_family_id: Some("commit_and_push_to_github".into()),
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
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
        },
    )
    .await
    .unwrap();

    let handler = CommandHandler::new(
        pool.clone(),
        event_bus::new_bus(16),
        WorkQueue::new(pool.clone()),
    );
    let result = handler
        .handle(
            Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id: "state_11_manual_release".into(),
                consume_quota_budget_now: false,
                agent_execution_id: Some(old_agent_execution_id),
                legacy_discovery_override_policy: None,
                legacy_discovery_override_reason: None,
                operator_instruction: None,
            }),
            CallerContext::test_fixture(),
        )
        .await;
    let err = match result {
        Ok(_) => panic!("targeted release retry must fail closed before state changes"),
        Err(err) => err,
    };

    assert!(
        err.to_string().contains("requires_effect_reconciliation"),
        "targeted release retry must be guarded: {err}"
    );
    let failed_journal_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM command_journal WHERE command_type = 'RetryStage' AND result_status = 'failed' AND error LIKE '%requires_effect_reconciliation%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        failed_journal_count, 1,
        "targeted release retry denial must leave a failed command journal entry"
    );
}

#[tokio::test]
async fn proposal_058_retry_stage_requires_explicit_quota_budget_before_reset() {
    let pool = setup_memory_pool().await;
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
            stage_execution_id: Some(old_stage_execution_id),
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
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
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
                operator_instruction: None,
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
                operator_instruction: None,
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

// ── HIGH-001 regression: payload backend_profile_id drives escalation resolution ──

/// P058 HIGH-001: claim_next_invoke_agent_with_start must use the payload's
/// backend_profile_id (the actual invoked agent's profile) when resolving the
/// escalation policy, NOT the stage owner's backend_profile from the frozen plan.
///
/// Setup:
/// - Stage owner "owner_agent" has backend_profile "owner_profile". No escalation policy
///   applies to "owner_profile".
/// - Escalation policy "task_profile_escalation" applies to backend_profile "task_profile"
///   (enabled_default = true).
/// - InvokeAgent payload carries backend_profile_id = "task_profile".
///
/// Expected: agent_execution.escalation_policy_id = Some("task_profile_escalation").
/// Pre-fix behavior would have read "owner_profile" from stage owner → no policy match
/// → null escalation fields (bypassing attribution audit invariant).
#[tokio::test]
async fn p058_high_001_claim_uses_payload_backend_profile_for_escalation() {
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    // Minimal workflow JSON: workflow id required; state "task_state" owned by "owner_agent".
    let workflow_json = r#"{
        "initial_state": "task_state",
        "workflow": {"id": "test_workflow_p058_high001"},
        "states": {
            "task_state": {
                "label": "Task State",
                "owner": "owner_agent",
                "type": "end"
            }
        }
    }"#;

    // Minimal catalog: owner_agent uses owner_profile (no policy for this profile).
    // Escalation policy binds to "task_profile" (a different profile) with enabled_default=true.
    // lead_agent is required by the compiler (exactly one system_role=lead).
    let catalog_json = r#"{
        "backend_profiles": {
            "owner_profile": {"provider": "claude"},
            "task_profile":  {"provider": "claude"},
            "lead_profile":  {"provider": "claude"}
        },
        "permission_profiles": {
            "lead_perm": {}
        },
        "contracts": {
            "lead_contract": {"format": "json"}
        },
        "agents": [
            {"id": "owner_agent", "backend_profile": "owner_profile"},
            {
                "id": "lead_agent",
                "system_role": "lead",
                "backend_profile": "lead_profile",
                "permission_profile": "lead_perm",
                "lead_resolution_contract": "lead_contract"
            }
        ],
        "escalation_policies": [
            {
                "policy_id": "task_profile_escalation",
                "schema_version": "escalation_policy_v1",
                "enabled_default": true,
                "applies_to": {"backend_profile_id": "task_profile"},
                "max_chain_attempts": 3,
                "max_chain_wall_clock_seconds": 1800,
                "triggers": ["contract_output_failure"],
                "tiers": [
                    {"tier_id": "retry_tier", "kind": "same_backend_retry", "max_attempts": 2}
                ]
            }
        ]
    }"#;

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 HIGH-001".into(),
            body: "payload backend_profile escalation resolution".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();

    // Run with frozen workflow + catalog snapshots so compile_run_plan_from_snapshot succeeds.
    let mut run = make_run(run_id, idea_id);
    run.workflow_snapshot_json = Some(workflow_json.into());
    run.catalog_snapshot_json = Some(catalog_json.into());
    runs::insert(&pool, &run).await.unwrap();

    stages::insert(
        &pool,
        &domain::stage::StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "task_state".into(),
            label: "Task State".into(),
            status: domain::stage::StageStatus::Running,
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
    // InvokeAgent payload carries backend_profile_id = "task_profile" — different from
    // stage owner's "owner_profile". The fix (HIGH-001) ensures this payload value is used
    // for escalation resolution so the policy "task_profile_escalation" is found.
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "high001-invoke-work-item".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "task_state",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "task_agent",
                "provider": "claude",
                "model": "sonnet",
                "prompt": "do the task",
                "task_name": "task",
                "task_inputs": [],
                "task_outputs": [],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "task_agent",
                "worktree_write_enabled": false,
                "backend_profile_id": "task_profile"
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("task_state".into()),
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
        .expect("HIGH-001: claim should succeed");

    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(
        executions.len(),
        1,
        "exactly one execution should be created"
    );

    let exec = &executions[0];
    assert_eq!(exec.id, claimed.agent_execution_id);

    // Core HIGH-001 assertion: escalation policy must be attributed from the task's
    // backend_profile_id ("task_profile"), not the stage owner's ("owner_profile").
    assert_eq!(
        exec.escalation_policy_id.as_deref(),
        Some("task_profile_escalation"),
        "HIGH-001: escalation_policy_id must match the policy bound to the payload's \
         backend_profile_id ('task_profile'), not the stage owner's 'owner_profile'"
    );
    assert!(
        exec.escalation_policy_hash.is_some(),
        "HIGH-001: escalation_policy_hash must be set when policy is resolved"
    );
    assert!(
        exec.escalation_ledger_id.is_some(),
        "HIGH-001: escalation_ledger_id must be set when policy is resolved"
    );
    assert_eq!(
        exec.escalation_tier_id.as_deref(),
        Some("retry_tier"),
        "HIGH-001: first tier from the policy must be set"
    );
}

#[tokio::test]
async fn p058_claim_uses_quota_free_escalation_backend_before_provider_quota_wait() {
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let quota_stage_execution_id = StageExecutionId::new();
    let stage_execution_id = StageExecutionId::new();

    let workflow_json = r#"{
        "initial_state": "impl_state",
        "workflow": {"id": "test_workflow_p058_quota_escalation"},
        "states": {
            "impl_state": {
                "label": "Impl",
                "owner": "code_writer",
                "type": "end"
            }
        }
    }"#;

    let catalog_json = r#"{
        "backend_profiles": {
            "claude_builder_high": {
                "provider": "claude_acp",
                "model": "sonnet",
                "effort": "high",
                "max_turns": 24
            },
            "codex_builder_high": {
                "provider": "codex_acp",
                "model": "gpt-5.5",
                "effort": "high",
                "max_turns": 24
            },
            "lead_profile": {"provider": "claude"}
        },
        "permission_profiles": {
            "lead_perm": {}
        },
        "contracts": {
            "lead_contract": {"format": "json"}
        },
        "agents": [
            {"id": "code_writer", "backend_profile": "claude_builder_high"},
            {
                "id": "lead_agent",
                "system_role": "lead",
                "backend_profile": "lead_profile",
                "permission_profile": "lead_perm",
                "lead_resolution_contract": "lead_contract"
            }
        ],
        "escalation_policies": [
            {
                "policy_id": "code_writer_quota_escalation",
                "schema_version": "escalation_policy_v1",
                "enabled_default": true,
                "applies_to": {"backend_profile_id": "claude_builder_high"},
                "max_chain_attempts": 3,
                "max_chain_wall_clock_seconds": 1800,
                "triggers": ["provider_quota_exhausted"],
                "tiers": [
                    {
                        "tier_id": "claude_builder_same_backend",
                        "kind": "backend_profile",
                        "backend_profile_id": "claude_builder_high",
                        "max_attempts": 1
                    },
                    {
                        "tier_id": "codex_builder_fallback",
                        "kind": "backend_profile",
                        "backend_profile_id": "codex_builder_high",
                        "max_attempts": 1
                    },
                    {"tier_id": "human_pause", "kind": "pause"}
                ]
            }
        ]
    }"#;

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058 quota fallback".into(),
            body: "quota-aware escalation before provider wait".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();

    let mut run = make_run(run_id, idea_id);
    run.workflow_snapshot_json = Some(workflow_json.into());
    run.catalog_snapshot_json = Some(catalog_json.into());
    runs::insert(&pool, &run).await.unwrap();

    stages::insert(
        &pool,
        &StageExecution {
            id: quota_stage_execution_id,
            run_id,
            stage_id: "impl_state".into(),
            label: "Impl quota source".into(),
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
    stages::insert(
        &pool,
        &StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "impl_state".into(),
            label: "Impl".into(),
            status: StageStatus::Running,
            iteration: 2,
            attempt_number: 2,
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

    let quota_agent_execution_id = AgentExecutionId::new();
    agent_executions::insert(
        &pool,
        &AgentExecution {
            id: quota_agent_execution_id,
            stage_execution_id: Some(quota_stage_execution_id),
            agent_id: "code_writer".into(),
            provider: "claude_acp".into(),
            model: Some("sonnet".into()),
            status: AgentStatus::Failed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            owner_execution_lineage_id: Some(quota_stage_execution_id.to_string()),
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: None,
            session_reuse_scope: None,
            session_family_id: None,
            session_reuse_disposition: None,
            session_reset_reason: None,
            backend_profile_id: Some("claude_builder_high".into()),
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
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
            escalation_policy_id: None,
            escalation_policy_hash: None,
            escalation_tier_id: None,
            escalation_tier_kind_raw: None,
            escalation_trigger_raw: None,
            escalation_digest_version: None,
            escalation_ledger_id: None,
        },
    )
    .await
    .unwrap();
    agent_retry_budget_ledger::upsert_quota_failure(
        &pool,
        run_id,
        quota_stage_execution_id,
        quota_agent_execution_id,
        Some(Utc::now() + Duration::hours(1)),
    )
    .await
    .unwrap();

    let now = Utc::now();
    work_items::enqueue(
        &pool,
        &WorkItem {
            id: "quota-fallback-invoke-work-item".into(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "impl_state",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "claude_acp",
                "backend_profile_id": "claude_builder_high",
                "model": "sonnet",
                "prompt": "implement",
                "task_name": "impl",
                "task_inputs": [],
                "task_outputs": [],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "code_writer",
                "worktree_write_enabled": false
            })
            .to_string(),
            status: WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("impl_state".into()),
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
        .expect("quota-aware escalation should choose a free fallback backend before waiting");

    let execution = agent_executions::find_by_id(&pool, claimed.agent_execution_id)
        .await
        .unwrap()
        .expect("claimed execution exists");
    assert_eq!(execution.provider, "codex");
    assert_eq!(execution.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(
        execution.backend_profile_id.as_deref(),
        Some("codex_builder_high")
    );
    assert_eq!(
        execution.escalation_policy_id.as_deref(),
        Some("code_writer_quota_escalation")
    );
    assert_eq!(
        execution.escalation_tier_id.as_deref(),
        Some("codex_builder_fallback")
    );
    assert_eq!(
        execution.escalation_trigger_raw.as_deref(),
        Some("provider_quota_exhausted")
    );
}

/// P058 Phase 1: claim path writes escalation_execution_metadata row when a policy applies.
/// Verifies the runtime metadata writer added in the refine pass: prior to this fix,
/// insert_execution_metadata_tx was only called from db-layer tests, never from the executor.
#[tokio::test]
async fn p058_claim_writes_execution_metadata_row() {
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    let workflow_json = r#"{
        "initial_state": "impl_state",
        "workflow": {"id": "test_workflow_p058_meta"},
        "states": {
            "impl_state": {
                "label": "Impl",
                "owner": "impl_agent",
                "type": "end"
            }
        }
    }"#;

    let catalog_json = r#"{
        "backend_profiles": {
            "impl_profile": {"provider": "claude"},
            "lead_profile":  {"provider": "claude"}
        },
        "permission_profiles": {
            "lead_perm": {}
        },
        "contracts": {
            "lead_contract": {"format": "json"}
        },
        "agents": [
            {"id": "impl_agent", "backend_profile": "impl_profile"},
            {
                "id": "lead_agent",
                "system_role": "lead",
                "backend_profile": "lead_profile",
                "permission_profile": "lead_perm",
                "lead_resolution_contract": "lead_contract"
            }
        ],
        "escalation_policies": [
            {
                "policy_id": "impl_escalation",
                "schema_version": "escalation_policy_v1",
                "enabled_default": true,
                "applies_to": {"agent_id": "impl_agent"},
                "max_chain_attempts": 3,
                "max_chain_wall_clock_seconds": 1800,
                "triggers": ["contract_output_failure"],
                "tiers": [
                    {"tier_id": "retry_tier", "kind": "same_backend_retry", "max_attempts": 2}
                ]
            }
        ]
    }"#;

    ideas::insert(
        &pool,
        &domain::idea::Idea {
            id: idea_id,
            title: "P058 metadata row test".into(),
            body: "proves executor writes execution_metadata".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();

    let mut run = make_run(run_id, idea_id);
    run.workflow_snapshot_json = Some(workflow_json.into());
    run.catalog_snapshot_json = Some(catalog_json.into());
    runs::insert(&pool, &run).await.unwrap();

    stages::insert(
        &pool,
        &domain::stage::StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "impl_state".into(),
            label: "Impl".into(),
            status: domain::stage::StageStatus::Running,
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
        &db::work_item::WorkItem {
            id: "meta-test-work-item".into(),
            kind: db::work_item::WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "impl_state",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "impl_agent",
                "provider": "claude",
                "model": "sonnet",
                "prompt": "implement",
                "task_name": "impl",
                "task_inputs": [],
                "task_outputs": [],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "impl_agent",
                "worktree_write_enabled": false
            })
            .to_string(),
            status: db::work_item::WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("impl_state".into()),
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
        .expect("claim should succeed");

    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(executions.len(), 1);
    let exec = &executions[0];
    assert_eq!(exec.id, claimed.agent_execution_id);

    let ledger_id = exec
        .escalation_ledger_id
        .as_deref()
        .expect("escalation_ledger_id must be set when policy applies");

    // Core assertion: executor must have written an execution_metadata row.
    let metas = escalation_repo::find_execution_metadata_by_ledger(&pool, ledger_id)
        .await
        .unwrap();
    assert_eq!(
        metas.len(),
        1,
        "executor must write exactly one execution_metadata row per claim when a policy applies"
    );
    let meta = &metas[0];
    assert_eq!(meta.agent_execution_id, exec.id);
    assert_eq!(meta.escalation_ledger_id, ledger_id);
    assert_eq!(
        meta.tier_id,
        exec.escalation_tier_id.as_deref().unwrap_or("")
    );
    assert_eq!(meta.tier_attempt_index, 0);
}

#[tokio::test]
async fn p058_startup_recovery_force_detaches_running_escalation_execution() {
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    let workflow_json = r#"{
        "initial_state": "impl_state",
        "workflow": {"id": "test_workflow_p058_shutdown"},
        "states": {
            "impl_state": {
                "label": "Impl",
                "owner": "impl_agent",
                "type": "end"
            }
        }
    }"#;

    let catalog_json = r#"{
        "backend_profiles": {
            "impl_profile": {"provider": "claude"},
            "lead_profile":  {"provider": "claude"}
        },
        "permission_profiles": {
            "lead_perm": {}
        },
        "contracts": {
            "lead_contract": {"format": "json"}
        },
        "agents": [
            {"id": "impl_agent", "backend_profile": "impl_profile"},
            {
                "id": "lead_agent",
                "system_role": "lead",
                "backend_profile": "lead_profile",
                "permission_profile": "lead_perm",
                "lead_resolution_contract": "lead_contract"
            }
        ],
        "escalation_policies": [
            {
                "policy_id": "impl_escalation",
                "schema_version": "escalation_policy_v1",
                "enabled_default": true,
                "applies_to": {"agent_id": "impl_agent"},
                "max_chain_attempts": 3,
                "max_chain_wall_clock_seconds": 1800,
                "triggers": ["contract_output_failure"],
                "tiers": [
                    {"tier_id": "retry_tier", "kind": "same_backend_retry", "max_attempts": 2}
                ]
            }
        ]
    }"#;

    ideas::insert(
        &pool,
        &domain::idea::Idea {
            id: idea_id,
            title: "P058 shutdown drain replay".into(),
            body: "proves recovery force-detach for abandoned escalation provider sessions".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();

    let mut run = make_run(run_id, idea_id);
    run.workflow_snapshot_json = Some(workflow_json.into());
    run.catalog_snapshot_json = Some(catalog_json.into());
    runs::insert(&pool, &run).await.unwrap();

    stages::insert(
        &pool,
        &domain::stage::StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "impl_state".into(),
            label: "Impl".into(),
            status: domain::stage::StageStatus::Running,
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
        &db::work_item::WorkItem {
            id: "shutdown-drain-work-item".into(),
            kind: db::work_item::WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "impl_state",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "impl_agent",
                "provider": "claude",
                "model": "sonnet",
                "prompt": "implement",
                "task_name": "impl",
                "task_inputs": [],
                "task_outputs": [],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": "same_agent_family_within_run",
                "session_family_id": "impl_agent",
                "worktree_write_enabled": false
            })
            .to_string(),
            status: db::work_item::WorkItemStatus::Pending,
            run_id: Some(run_id),
            stage_id: Some("impl_state".into()),
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
        .expect("claim should leave a running provider session");
    let execution_before = agent_executions::find_by_id(&pool, claimed.agent_execution_id)
        .await
        .unwrap()
        .expect("claimed execution exists before recovery");
    assert_eq!(execution_before.status, AgentStatus::Running);
    assert!(
        execution_before.escalation_ledger_id.is_some(),
        "claim must stamp escalation ledger before recovery"
    );
    let recoverable_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM agent_executions ae
           INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
           LEFT JOIN escalation_execution_metadata em ON em.agent_execution_id = ae.id
           WHERE se.run_id = ?
             AND se.status = 'running'
             AND ae.status = 'running'
             AND ae.escalation_ledger_id IS NOT NULL"#,
    )
    .bind(run_id.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recoverable_count, 1);

    let recovery = RecoveryService::new(
        pool.clone(),
        WorkQueue::new(pool.clone()),
        event_bus::new_bus(16),
    );
    let _summary = recovery.run_startup_repair().await.unwrap();
    let pending_invokes = work_items::list_by_status(&pool, WorkItemStatus::Pending)
        .await
        .unwrap()
        .into_iter()
        .filter(|item| item.kind == WorkItemKind::InvokeAgent)
        .count();
    assert_eq!(
        pending_invokes, 0,
        "P058 startup recovery must not re-launch an already force-detached escalation execution"
    );

    let execution = agent_executions::find_by_id(&pool, claimed.agent_execution_id)
        .await
        .unwrap()
        .expect("claimed execution remains auditable");
    assert_eq!(execution.status, AgentStatus::Failed);
    assert!(execution.completed_at.is_some());

    let facts =
        agent_execution_runtime_facts::find_by_execution_id(&pool, claimed.agent_execution_id)
            .await
            .unwrap()
            .expect("runtime facts are written for shutdown drain replay");
    assert_eq!(
        facts.supervision_classification.as_deref(),
        Some("shutdown_drain_timeout")
    );
    assert_eq!(
        facts
            .failure_kind
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("transport_closed")
    );

    let stage_after = stages::find_by_id(&pool, stage_execution_id)
        .await
        .unwrap()
        .expect("stage remains present");
    assert_eq!(stage_after.status, StageStatus::Failed);

    let run_after = runs::find_by_id(&pool, run_id)
        .await
        .unwrap()
        .expect("run remains present");
    assert_eq!(run_after.status, RunStatus::Blocked);

    let ledger_id = execution
        .escalation_ledger_id
        .as_deref()
        .expect("claimed execution has escalation ledger");
    let ledger = escalation_repo::find_ledger_by_id(&pool, ledger_id)
        .await
        .unwrap()
        .expect("ledger remains present");
    assert_eq!(ledger.status_raw, "paused");
    assert_eq!(
        ledger.pause_reason_raw.as_deref(),
        Some("provider_session_force_detached")
    );
    let events = escalation_repo::find_events_by_ledger(&pool, ledger_id)
        .await
        .unwrap();
    assert!(events.iter().any(|event| {
        event.event_kind_raw == "escalation.provider_session_force_detached"
            && event.pause_reason_raw.as_deref() == Some("provider_session_force_detached")
    }));
}

/// Atomicity regression: two concurrent claimers racing on the same InvokeAgent item must
/// produce exactly one agent_execution row and one escalation_execution_metadata row.
/// The loser's CAS miss must leave no orphan rows (the fix for the two-transaction gap).
#[tokio::test]
async fn proposal_058_claim_start_concurrent_claimers_leave_exactly_one_agent_execution() {
    let pool = setup_memory_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_execution_id = StageExecutionId::new();

    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P058".into(),
            body: "concurrent claim atomicity".into(),
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
        &pool,
        &Run {
            id: run_id,
            idea_id,
            status: domain::run::RunStatus::Running,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp".into(),
            artifact_root: "/tmp".into(),
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
            review_routing_json: None,
            closeout_readiness_mode: None,
        },
    )
    .await
    .unwrap();
    stages::insert(
        &pool,
        &domain::stage::StageExecution {
            id: stage_execution_id,
            run_id,
            stage_id: "implementation".into(),
            label: "Implementation".into(),
            status: domain::stage::StageStatus::Running,
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
        &db::work_item::WorkItem {
            id: "invoke-atomic-cas-1".into(),
            kind: db::work_item::WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "stage_id": "implementation",
                "stage_execution_id": stage_execution_id.to_string(),
                "agent_id": "code_writer",
                "provider": "claude",
                "model": "sonnet",
                "prompt": "write code",
                "task_name": "code",
                "task_inputs": [],
                "task_outputs": [],
                "declared_outputs": [],
                "requested_mcp_server_ids": [],
                "session_reuse_scope": "none",
                "session_family_id": "code_writer",
                "worktree_write_enabled": false
            })
            .to_string(),
            status: db::work_item::WorkItemStatus::Pending,
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

    // Race two concurrent claimers. SQLite IMMEDIATE transactions serialize them, so exactly
    // one CAS wins and the other returns Ok(None) without writing any rows.
    let pool1 = pool.clone();
    let pool2 = pool.clone();
    let (r1, r2) = tokio::join!(
        engine::executor::claim_next_invoke_agent_with_start(&pool1),
        engine::executor::claim_next_invoke_agent_with_start(&pool2),
    );
    let r1 = r1.unwrap();
    let r2 = r2.unwrap();

    let wins = [r1.is_some(), r2.is_some()].iter().filter(|&&b| b).count();
    assert_eq!(
        wins, 1,
        "exactly one claimer must win the CAS; got {} winners (two=double-insert, zero=lost win)",
        wins
    );

    let executions = agent_executions::find_by_stage(&pool, stage_execution_id)
        .await
        .unwrap();
    assert_eq!(
        executions.len(),
        1,
        "exactly one agent_execution must be created; {} found — CAS atomicity broken if != 1",
        executions.len()
    );
}
