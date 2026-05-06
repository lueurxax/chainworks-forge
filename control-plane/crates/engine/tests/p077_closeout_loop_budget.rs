use std::collections::HashMap;

use chrono::Utc;
use domain::ids::{RunId, StageExecutionId};
use domain::stage::{StageExecution, StageStatus};
use engine::closeout_loop_budget::closeout_loop_budget_remaining;
use std::collections::HashMap as StdHashMap;
use workflow::plan::{
    CompiledLoop, CompiledState, CompiledTask, DegradedOutputPolicy, LegacyBroadDiscoveryPolicy,
    ResolvedAgent, RunPlan,
};

fn test_plan(loop_max: Option<u64>) -> RunPlan {
    let task = CompiledTask {
        task_name: "review".into(),
        agent: ResolvedAgent {
            agent_id: "reviewer".into(),
            backend_profile_id: None,
            provider: "codex".into(),
            model: None,
            effort: None,
            max_turns: None,
            temperature: None,
            prompt: None,
            permission_profile: None,
            skill_ref: None,
            skill_role: None,
            skill_snapshot_hash: None,
            requested_mcp_server_ids: Vec::new(),
            resolved_skill: None,
            output_contract: None,
            worktree_write_enabled: false,
            worktree_strategy: None,
            session_reuse_scope: None,
            session_family_id: None,
            xcode_broker_required: false,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            xcode_prompt_lint_warnings: Vec::new(),
            toolchain_cache_policy: None,
        },
        inputs: Vec::new(),
        outputs: Vec::new(),
        output_policies: StdHashMap::new(),
        output_schemas: StdHashMap::new(),
        parallel: false,
        phase: 0,
        selected_outputs_from: None,
    };
    let mut states = HashMap::new();
    states.insert(
        "state_10_implementation_refined".into(),
        CompiledState {
            id: "state_10_implementation_refined".into(),
            label: "Refine".into(),
            state_type: None,
            owner: task.agent.clone(),
            is_manual_gate: false,
            is_end: false,
            tasks: vec![task],
            post_approval_tasks: Vec::new(),
            transitions: Vec::new(),
            loop_config: loop_max.map(|max| CompiledLoop {
                counter: "implementation_refine".into(),
                max,
            }),
            degraded_output_policy: DegradedOutputPolicy::default(),
            dynamic_parallel: None,
            system_task: None,
        },
    );

    RunPlan {
        initial_state: "state_10_implementation_refined".into(),
        states,
        variables: HashMap::new(),
        artifact_paths: HashMap::new(),
        workflow_family: None,
        risk_class: None,
        stack: None,
        legacy_broad_discovery_policy: LegacyBroadDiscoveryPolicy::Disabled,
        workflow_snapshot_hash: "workflow".into(),
        catalog_snapshot_hash: "catalog".into(),
        workflow_snapshot_json: "{}".into(),
        catalog_snapshot_json: "{}".into(),
        dynamic_candidate_bindings: Vec::new(),
        run_plan_snapshot_format_version: None,
        closeout_readiness_mode: None,
    }
}

fn stage(stage_id: &str, iteration: i64) -> StageExecution {
    StageExecution {
        id: StageExecutionId::new(),
        run_id: RunId::new(),
        stage_id: stage_id.into(),
        label: stage_id.into(),
        status: StageStatus::Completed,
        iteration,
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
    }
}

#[test]
fn closeout_loop_budget_remaining_is_false_when_refine_loop_is_exhausted() {
    let plan = test_plan(Some(2));
    let stages = vec![
        stage("state_10_implementation_refined", 1),
        stage("state_10_implementation_refined", 2),
    ];

    assert!(!closeout_loop_budget_remaining(
        &plan,
        &stages,
        "state_10_implementation_refined"
    ));
}

#[test]
fn closeout_loop_budget_remaining_is_true_without_loop_config() {
    let plan = test_plan(None);
    let stages = vec![stage("state_10_implementation_refined", 12)];

    assert!(closeout_loop_budget_remaining(
        &plan,
        &stages,
        "state_10_implementation_refined"
    ));
}
