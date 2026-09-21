//! Closed, opt-in compilation for P039. This module never creates or activates a Run.

use std::collections::BTreeSet;

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::plan::{ResolvedAgent, RunPlan};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationProfileV1 {
    pub schema_version: String,
    pub profile: String,
    pub review_state: String,
    pub approval_state: String,
    pub preparation_state: String,
    pub proposal_input: String,
    pub preparation_task: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContinuationTargetV1 {
    pub schema_version: String,
    pub profile: ContinuationProfileV1,
    pub plan: RunPlan,
}

pub fn compile_for_continuation_v1(
    workflow_path: &str,
    catalog_path: &str,
) -> Result<ContinuationTargetV1> {
    let plan = crate::compiler::compile_for_new_run_v1(workflow_path, catalog_path)?.into_plan();
    validate_continuation_target_v1(plan)
}

/// Freeze the derived policy into the actual snapshots used after a restart.
/// Current definitions remain unchanged; this produces the successor's snapshots.
pub fn freeze_continuation_target_v1(
    plan: RunPlan,
    catalog_path: &str,
) -> Result<ContinuationTargetV1> {
    let target = validate_continuation_target_v1(plan)?;
    let mut workflow: Value = serde_json::from_str(&target.plan.workflow_snapshot_json)?;
    let mut catalog: Value = serde_json::from_str(&target.plan.catalog_snapshot_json)?;
    let mut readonly = BTreeSet::new();
    for state in target.plan.states.values() {
        for agent in std::iter::once(&state.owner).chain(state.tasks.iter().map(|t| &t.agent)) {
            if !agent.worktree_write_enabled
                && agent.worktree_strategy.as_deref() == Some("shared_implementation_worktree")
            {
                readonly.insert(agent.agent_id.clone());
            }
        }
    }
    for binding in target
        .plan
        .dynamic_candidate_bindings
        .iter()
        .filter(|b| b.enabled_for_proposal_review)
    {
        let agent: ResolvedAgent = serde_json::from_str(&binding.resolved_agent_snapshot_json)?;
        readonly.insert(agent.agent_id);
    }
    // Catalog bindings are shared across states. Restricting metadata-only
    // bindings is safe; never silently change a post-gate product writer.
    for state in target.plan.states.values() {
        for agent in std::iter::once(&state.owner).chain(
            state
                .tasks
                .iter()
                .chain(&state.post_approval_tasks)
                .map(|t| &t.agent),
        ) {
            if readonly.contains(&agent.agent_id) && agent.worktree_write_enabled {
                validate_metadata_only(agent, &catalog)?;
            }
        }
    }
    for agent in catalog["agents"]
        .as_array_mut()
        .context("target_profile_invalid: agents")?
    {
        if agent["id"].as_str().is_some_and(|id| readonly.contains(id)) {
            agent["worktree_policy"] = serde_json::json!({"write_enabled":false,"strategy":"shared_implementation_worktree"});
        }
    }
    workflow["initial_state"] = Value::String(target.profile.review_state.clone());
    let compiled = crate::compiler::compile_from_snapshot_json(
        &serde_json::to_string(&workflow)?,
        &serde_json::to_string(&catalog)?,
        catalog_path,
    )?;
    validate_continuation_target_v1(compiled)
}

/// For already compiled, trusted in-process plans; never decode a caller-supplied RunPlan here.
pub fn validate_continuation_target_v1(mut plan: RunPlan) -> Result<ContinuationTargetV1> {
    use crate::snapshot_integrity::{verify_complete_pair_v1, SnapshotIntegrityV1};
    ensure!(
        matches!(
            verify_complete_pair_v1(
                Some(&plan.workflow_snapshot_json),
                Some(&plan.catalog_snapshot_json),
                Some(&plan.workflow_snapshot_hash),
                Some(&plan.catalog_snapshot_hash),
            ),
            SnapshotIntegrityV1::Verified(_)
        ),
        "target_profile_invalid: snapshot integrity"
    );
    let workflow: Value = serde_json::from_str(&plan.workflow_snapshot_json)?;
    let catalog: Value = serde_json::from_str(&plan.catalog_snapshot_json)?;
    let profile: ContinuationProfileV1 = serde_json::from_value(
        workflow
            .get("blocked_run_continuation")
            .cloned()
            .unwrap_or(Value::Null),
    )
    .context("target_profile_invalid: explicit closed profile required")?;
    ensure!(
        profile.schema_version == "blocked_run_continuation_profile_v1"
            && profile.profile == "implementation_restart_v1"
            && profile.proposal_input == "proposal_current"
            && profile.preparation_task == "freeze_approved_proposal_and_prepare_worktree"
            && workflow["workflow"]["id"] == "proposal_to_release",
        "target_profile_invalid: unsupported profile"
    );
    ensure!(
        BTreeSet::from([
            &profile.review_state,
            &profile.approval_state,
            &profile.preparation_state,
        ])
        .len()
            == 3,
        "target_profile_invalid: aliased entry states"
    );
    let review = plan
        .states
        .get(&profile.review_state)
        .context("target_profile_invalid: missing review state")?;
    ensure!(
        review.system_task.as_ref().is_some_and(
            |t| t.task_type == "proposal_review_router" && t.executor_mode == "system.routing"
        ) && review
            .dynamic_parallel
            .as_ref()
            .is_some_and(|d| d.output_contract == "proposal_review_v1")
            && !review.is_manual_gate
            && !review.is_end,
        "target_profile_invalid: review router and reviewers required"
    );
    let gate = plan
        .states
        .get(&profile.approval_state)
        .context("target_profile_invalid: missing approval state")?;
    ensure!(
        gate.is_manual_gate
            && gate.tasks.is_empty()
            && gate.post_approval_tasks.is_empty()
            && gate.transitions.len() == 2,
        "target_profile_invalid: fresh manual gate required"
    );
    let granted = gate
        .transitions
        .iter()
        .find(|t| t.condition.trim() == "approval.granted == true")
        .context("target_profile_invalid: exact grant predicate required")?;
    let rejected = gate
        .transitions
        .iter()
        .find(|t| t.condition.trim() == "approval.rejected == true")
        .context("target_profile_invalid: exact rejection predicate required")?;
    ensure!(
        granted.to == profile.preparation_state && rejected.to != profile.approval_state,
        "target_profile_invalid: approval routing"
    );
    let rejected_state = rejected.to.clone();
    let success = "proposal_review_summary.average_score >= vars.proposal_score_target and proposal_review_summary.min_individual_score >= vars.min_individual_proposal_score and proposal_review_summary.blocker_count == 0";
    let refinement = "proposal_review_summary.average_score < vars.proposal_score_target or proposal_review_summary.min_individual_score < vars.min_individual_proposal_score or proposal_review_summary.blocker_count > 0";
    ensure!(
        review.transitions.len() == 2
            && review.transitions.iter().any(|transition| {
                transition.to == profile.approval_state
                    && transition
                        .condition
                        .split_whitespace()
                        .eq(success.split_whitespace())
            })
            && review.transitions.iter().any(|transition| {
                transition.to == rejected_state
                    && transition
                        .condition
                        .split_whitespace()
                        .eq(refinement.split_whitespace())
            }),
        "target_profile_invalid: review outcome routing"
    );

    // Walk without crossing the approval gate: any reachable writer is a bypass.
    let mut pending = vec![profile.review_state.clone(), rejected_state.clone()];
    let mut pre_gate = BTreeSet::new();
    let mut gate_reached = false;
    while let Some(id) = pending.pop() {
        if id == profile.approval_state {
            gate_reached = true;
            continue;
        }
        if !pre_gate.insert(id.clone()) {
            continue;
        }
        let state = plan
            .states
            .get(&id)
            .context("target_profile_invalid: unknown transition")?;
        ensure!(
            !state.is_manual_gate
                && !state.is_end
                && state.post_approval_tasks.is_empty()
                && id != profile.preparation_state
                && !state.transitions.is_empty(),
            "target_profile_invalid: pre-approval bypass or unsupported terminal"
        );
        for agent in std::iter::once(&state.owner).chain(state.tasks.iter().map(|t| &t.agent)) {
            validate_metadata_only(agent, &catalog)?;
        }
        pending.extend(state.transitions.iter().map(|t| t.to.clone()));
    }
    ensure!(
        gate_reached && pre_gate.contains(&rejected_state),
        "target_profile_invalid: unreachable gate"
    );
    ensure!(
        reaches_review(
            &plan,
            &rejected_state,
            &profile.review_state,
            &profile.approval_state
        ),
        "target_profile_invalid: rejection must return through review"
    );
    validate_metadata_only(&plan.states[&profile.approval_state].owner, &catalog)?;

    let preparation = plan
        .states
        .get(&profile.preparation_state)
        .context("target_profile_invalid: missing preparation state")?;
    let first = preparation
        .tasks
        .first()
        .context("target_profile_invalid: missing preparation task")?;
    ensure!(
        first.task_name == profile.preparation_task
            && !first.parallel
            && first.inputs.contains(&profile.proposal_input)
            && [
                "approved_proposal",
                "implementation_plan",
                "implementation_backlog"
            ]
            .iter()
            .all(|name| first.outputs.iter().any(|output| output == name))
            && preparation
                .tasks
                .iter()
                .skip(1)
                .any(|t| t.agent.agent_id == "code_writer"),
        "target_profile_invalid: fresh preparation contracts required"
    );

    for state in plan.states.values() {
        for agent in std::iter::once(&state.owner).chain(
            state
                .tasks
                .iter()
                .chain(&state.post_approval_tasks)
                .map(|t| &t.agent),
        ) {
            validate_headless(agent, &catalog)?;
        }
    }
    let mut reviewers = 0;
    for binding in &mut plan.dynamic_candidate_bindings {
        if !binding.enabled_for_proposal_review {
            continue;
        }
        let mut agent: ResolvedAgent = serde_json::from_str(&binding.resolved_agent_snapshot_json)
            .context("target_profile_invalid: incomplete dynamic reviewer")?;
        validate_metadata_only(&agent, &catalog)?;
        validate_headless(&agent, &catalog)?;
        agent.worktree_write_enabled = false;
        agent.worktree_strategy = Some("shared_implementation_worktree".into());
        binding.resolved_agent_snapshot_json = serde_json::to_string(&agent)?;
        binding.worktree_hash = format!("{:x}", Sha256::digest(b"shared_implementation_worktree"));
        reviewers += 1;
    }
    ensure!(
        reviewers > 0,
        "target_profile_invalid: no enabled reviewers"
    );
    pre_gate.insert(profile.approval_state.clone());
    for id in pre_gate {
        let state = plan.states.get_mut(&id).expect("validated state");
        state.owner.worktree_write_enabled = false;
        state.owner.worktree_strategy = Some("shared_implementation_worktree".into());
        for task in &mut state.tasks {
            task.agent.worktree_write_enabled = false;
            task.agent.worktree_strategy = Some("shared_implementation_worktree".into());
        }
    }
    plan.initial_state = profile.review_state.clone();
    Ok(ContinuationTargetV1 {
        schema_version: "run_continuation_target_v1".into(),
        profile,
        plan,
    })
}

fn reaches_review(plan: &RunPlan, start: &str, review: &str, gate: &str) -> bool {
    let mut pending = vec![start];
    let mut seen = BTreeSet::new();
    let mut review_reached = false;
    while let Some(id) = pending.pop() {
        if id == review {
            review_reached = true;
            continue;
        }
        if id == gate {
            return false;
        }
        if !seen.insert(id) {
            continue;
        }
        if let Some(state) = plan.states.get(id) {
            pending.extend(state.transitions.iter().map(|t| t.to.as_str()));
        }
    }
    review_reached
}

fn permission<'a>(agent: &ResolvedAgent, catalog: &'a Value) -> Result<&'a Value> {
    let name = agent
        .permission_profile
        .as_deref()
        .context("target_policy_incompatible: missing permission profile")?;
    catalog["permission_profiles"]
        .get(name)
        .context("target_policy_incompatible: unknown permission profile")
}

fn validate_metadata_only(agent: &ResolvedAgent, catalog: &Value) -> Result<()> {
    ensure!(
        !agent.worktree_write_enabled || agent.worktree_strategy.as_deref() == Some("meta_only"),
        "target_policy_incompatible: pre-gate writer"
    );
    let policy = permission(agent, catalog)?;
    let writes = policy["filesystem"]["write"]
        .as_array()
        .context("target_policy_incompatible: explicit metadata writes required")?;
    for path in writes {
        let path = path
            .as_str()
            .context("target_policy_incompatible: write path")?;
        ensure!(
            path.starts_with("${CHAINWORKS_META_ROOT:-.chainworks}/")
                && !path.split('/').any(|c| c == ".."),
            "target_policy_incompatible: pre-gate product write"
        );
    }
    for capability in ["checkout", "commit", "push"] {
        ensure!(
            policy["git"][capability] == false,
            "target_policy_incompatible: pre-gate git mutation"
        );
    }
    ensure!(
        policy["mcp"]["runtime_authority"] == false,
        "target_policy_incompatible: pre-gate runtime mutation"
    );
    let shell = policy["shell"]["allow"]
        .as_array()
        .context("target_policy_incompatible: shell policy absent")?;
    for command in shell {
        ensure!(
            matches!(
                command.as_str(),
                Some(
                    "rg" | "fd"
                        | "find"
                        | "ls"
                        | "cat"
                        | "git status"
                        | "git diff --stat"
                        | "git diff --name-only"
                )
            ),
            "target_policy_incompatible: pre-gate shell mutation"
        );
    }
    if let Some(headless) = policy.get("xcode_headless") {
        ensure!(
            headless["gates"].as_array().is_some_and(Vec::is_empty),
            "target_policy_incompatible: pre-gate Xcode execution"
        );
    }
    Ok(())
}

fn validate_headless(agent: &ResolvedAgent, catalog: &Value) -> Result<()> {
    if agent.requires_xcode_host_execution {
        let policy = permission(agent, catalog)?;
        ensure!(
            policy["xcode_headless"]["read"] == true
                && policy["xcode_headless"]["gates"]
                    .as_array()
                    .is_some_and(|gates| !gates.is_empty()),
            "target_policy_incompatible: explicit headless capability required"
        );
    }
    Ok(())
}
