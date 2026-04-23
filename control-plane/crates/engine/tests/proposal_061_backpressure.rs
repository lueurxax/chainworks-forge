use std::collections::HashMap;

use chrono::{Duration, Utc};
use db::pool::create_pool;
use db::repos::{agent_executions, ideas, runs, stages, work_items};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{AgentExecution, AgentStatus};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::executor::{
    claim_next_invoke_agent_with_start_with_capacity,
    has_capacity_eligible_pending_invoke_agent_for_start, InvokeAgentCapacityConfig,
};

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
        actual_xcode_runtime_observation_json: None,
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
        &make_running_execution(running_gemini_stage, "gemini"),
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
            "gemini",
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
        max_active_total: 6,
        max_active_per_run: 10,
        provider_caps: HashMap::from([("gemini".into(), 1), ("codex".into(), 3)]),
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
        max_active_total: 6,
        max_active_per_run: 10,
        provider_caps: HashMap::from([("gemini".into(), 1), ("codex".into(), 3)]),
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
