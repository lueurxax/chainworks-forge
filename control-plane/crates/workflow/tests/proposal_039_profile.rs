use serde_json::{json, Value};
use std::{path::PathBuf, sync::OnceLock};
use workflow::{carry_forward::validate_continuation_target_v1, plan::RunPlan};

const REVIEW: &str = "state_4_proposal_reviewed";
const GATE: &str = "state_6_implementation_approval";
const PREPARE: &str = "state_7_implementation_started";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn base() -> RunPlan {
    static PLAN: OnceLock<RunPlan> = OnceLock::new();
    PLAN.get_or_init(|| {
        workflow::compiler::compile_for_new_run_v1(
            root()
                .join("examples/workflows/workflow.yaml")
                .to_str()
                .unwrap(),
            root().join("examples/agents/agents.yaml").to_str().unwrap(),
        )
        .unwrap()
        .into_plan()
    })
    .clone()
}

fn with_edit(edit: impl FnOnce(&mut Value, &mut Value)) -> RunPlan {
    let baseline = base();
    let mut workflow: Value = serde_json::from_str(&baseline.workflow_snapshot_json).unwrap();
    let mut catalog: Value = serde_json::from_str(&baseline.catalog_snapshot_json).unwrap();
    workflow["blocked_run_continuation"] = json!({
        "schema_version": "blocked_run_continuation_profile_v1",
        "profile": "implementation_restart_v1",
        "review_state": REVIEW,
        "approval_state": GATE,
        "preparation_state": PREPARE,
        "proposal_input": "proposal_current",
        "preparation_task": "freeze_approved_proposal_and_prepare_worktree"
    });
    edit(&mut workflow, &mut catalog);
    workflow::compiler::compile_from_snapshot_json(
        &serde_json::to_string(&workflow).unwrap(),
        &serde_json::to_string(&catalog).unwrap(),
        root().join("examples/agents/agents.yaml").to_str().unwrap(),
    )
    .unwrap()
}

#[test]
fn continuation_enters_fresh_review_and_freezes_all_review_roots() {
    let plan = with_edit(|_, _| {});
    let old_workflow = plan.workflow_snapshot_json.clone();
    let old_catalog = plan.catalog_snapshot_json.clone();
    let target = validate_continuation_target_v1(plan).unwrap();
    assert_eq!(target.plan.initial_state, REVIEW);
    assert_eq!(target.profile.approval_state, GATE);
    assert_eq!(target.plan.workflow_snapshot_json, old_workflow);
    assert_eq!(target.plan.catalog_snapshot_json, old_catalog);
    for id in [REVIEW, "state_5_proposal_refined", GATE] {
        let state = &target.plan.states[id];
        for agent in std::iter::once(&state.owner).chain(state.tasks.iter().map(|t| &t.agent)) {
            assert!(!agent.worktree_write_enabled);
            assert_eq!(
                agent.worktree_strategy.as_deref(),
                Some("shared_implementation_worktree")
            );
        }
    }
    let mut reviewers = 0;
    for binding in target
        .plan
        .dynamic_candidate_bindings
        .iter()
        .filter(|b| b.enabled_for_proposal_review)
    {
        let agent: workflow::plan::ResolvedAgent =
            serde_json::from_str(&binding.resolved_agent_snapshot_json).unwrap();
        assert_eq!(
            agent.worktree_strategy.as_deref(),
            Some("shared_implementation_worktree")
        );
        assert!(!agent.worktree_write_enabled);
        reviewers += 1;
    }
    assert!(reviewers > 0);
}

#[test]
fn frozen_successor_retains_derived_policy_after_snapshot_recompile() {
    let original = with_edit(|_, _| {});
    let original_workflow = original.workflow_snapshot_json.clone();
    let original_catalog = original.catalog_snapshot_json.clone();
    let target = workflow::carry_forward::freeze_continuation_target_v1(
        original.clone(),
        root().join("examples/agents/agents.yaml").to_str().unwrap(),
    )
    .unwrap();
    let reloaded = workflow::compiler::compile_from_snapshot_json(
        &target.plan.workflow_snapshot_json,
        &target.plan.catalog_snapshot_json,
        root().join("examples/agents/agents.yaml").to_str().unwrap(),
    )
    .unwrap();
    assert_eq!(reloaded.initial_state, REVIEW);
    for id in [REVIEW, "state_5_proposal_refined", GATE] {
        let state = &reloaded.states[id];
        for agent in std::iter::once(&state.owner).chain(state.tasks.iter().map(|t| &t.agent)) {
            assert!(!agent.worktree_write_enabled);
            assert_eq!(
                agent.worktree_strategy.as_deref(),
                Some("shared_implementation_worktree")
            );
        }
    }
    for binding in reloaded
        .dynamic_candidate_bindings
        .iter()
        .filter(|b| b.enabled_for_proposal_review)
    {
        let agent: workflow::plan::ResolvedAgent =
            serde_json::from_str(&binding.resolved_agent_snapshot_json).unwrap();
        assert!(!agent.worktree_write_enabled);
        assert_eq!(
            agent.worktree_strategy.as_deref(),
            Some("shared_implementation_worktree")
        );
    }
    let before_writer = original.states[PREPARE]
        .tasks
        .iter()
        .find(|t| t.agent.agent_id == "code_writer")
        .unwrap();
    let after_writer = reloaded.states[PREPARE]
        .tasks
        .iter()
        .find(|t| t.agent.agent_id == "code_writer")
        .unwrap();
    assert_eq!(
        before_writer.agent.worktree_write_enabled,
        after_writer.agent.worktree_write_enabled
    );
    assert_eq!(
        before_writer.agent.worktree_strategy,
        after_writer.agent.worktree_strategy
    );
    assert_eq!(original.workflow_snapshot_json, original_workflow);
    assert_eq!(original.catalog_snapshot_json, original_catalog);
    assert_ne!(
        target.plan.workflow_snapshot_hash,
        original.workflow_snapshot_hash
    );
    assert_ne!(
        target.plan.catalog_snapshot_hash,
        original.catalog_snapshot_hash
    );
    assert_eq!(
        target.plan.workflow_snapshot_hash,
        reloaded.workflow_snapshot_hash
    );
    assert_eq!(
        target.plan.catalog_snapshot_hash,
        reloaded.catalog_snapshot_hash
    );
}

#[test]
fn ordinary_workflow_without_opt_in_cannot_be_a_continuation_target() {
    let plan = with_edit(|wf, _| {
        wf.as_object_mut()
            .unwrap()
            .remove("blocked_run_continuation");
    });
    let error = validate_continuation_target_v1(plan).unwrap_err();
    assert!(error.to_string().contains("target_profile_invalid"));
}

#[test]
fn checked_in_target_opts_in_without_changing_ordinary_run_entry() {
    let original = base();
    assert_eq!(original.initial_state, "state_1_idea_received");
    let target = validate_continuation_target_v1(original).unwrap();
    assert_eq!(target.profile.profile, "implementation_restart_v1");
    assert_eq!(target.plan.initial_state, REVIEW);
}

#[test]
fn profile_unknown_version_fields_or_entry_are_rejected() {
    for (key, value) in [
        ("schema_version", "future"),
        ("profile", "arbitrary_jump"),
        ("review_state", PREPARE),
        ("extra", "override"),
    ] {
        let plan = with_edit(|wf, _| wf["blocked_run_continuation"][key] = json!(value));
        assert!(validate_continuation_target_v1(plan).is_err(), "{key}");
    }
}

#[test]
fn review_to_code_bypass_cannot_inherit_historical_approval() {
    let plan = with_edit(|wf, _| {
        wf["states"][REVIEW]["transitions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"to": PREPARE, "when": "true"}));
    });
    assert!(validate_continuation_target_v1(plan)
        .unwrap_err()
        .to_string()
        .contains("target_profile_invalid"));
}

#[test]
fn review_success_and_refinement_predicates_are_not_replaceable_by_unconditional_edges() {
    for index in [0, 1] {
        let plan = with_edit(|wf, _| {
            wf["states"][REVIEW]["transitions"][index]["when"] = json!("true");
        });
        assert!(validate_continuation_target_v1(plan)
            .unwrap_err()
            .to_string()
            .contains("target_profile_invalid: review outcome routing"));
    }
}

#[test]
fn gate_requires_granted_transition_and_rejected_review_loop() {
    for (index, field, value) in [(0, "when", "true"), (1, "to", PREPARE)] {
        let plan =
            with_edit(|wf, _| wf["states"][GATE]["transitions"][index][field] = json!(value));
        assert!(validate_continuation_target_v1(plan).is_err());
    }
}

#[test]
fn refinement_cannot_skip_a_new_review_after_rejection() {
    let plan = with_edit(|wf, _| {
        wf["states"]["state_5_proposal_refined"]["transitions"]
            .as_array_mut()
            .unwrap()
            .push(json!({"to": GATE, "when": "true"}));
    });
    assert!(validate_continuation_target_v1(plan).is_err());
}

#[test]
fn pre_gate_permission_cannot_write_product_code_even_if_worktree_flag_is_false() {
    let plan = with_edit(|_, cat| {
        cat["permission_profiles"]["RO_REVIEW"]["filesystem"]["write"] =
            json!(["${CHAINWORKS_REPO_ROOT:-.}/**"]);
    });
    assert!(validate_continuation_target_v1(plan)
        .unwrap_err()
        .to_string()
        .contains("target_policy_incompatible"));
}

#[test]
fn dynamic_reviewer_with_writer_authority_is_rejected() {
    let plan = with_edit(|_, cat| {
        let agent = cat["agents"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|a| a["routing"]["enabled_for_proposal_review"] == true)
            .unwrap();
        agent["worktree_policy"] = json!({"strategy":"dedicated", "write_enabled":true});
    });
    assert!(validate_continuation_target_v1(plan).is_err());
}

#[test]
fn legacy_xcode_policy_without_headless_grant_is_not_current_authority() {
    let plan = with_edit(|_, cat| {
        cat["permission_profiles"]["CODE_WRITE"]
            .as_object_mut()
            .unwrap()
            .remove("xcode_headless");
    });
    assert!(validate_continuation_target_v1(plan)
        .unwrap_err()
        .to_string()
        .contains("target_policy_incompatible"));
}

#[test]
fn missing_preparation_output_cannot_be_supplied_by_historical_seed() {
    let plan = with_edit(|wf, _| {
        wf["states"][PREPARE]["run"]["sequence"][0]["outputs"] = json!(["run_state"])
    });
    assert!(validate_continuation_target_v1(plan).is_err());
}
