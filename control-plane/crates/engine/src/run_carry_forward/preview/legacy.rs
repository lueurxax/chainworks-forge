use super::*;
use workflow::definition::WorkflowFile;

// These are the two reviewed legacy families, not caller-selected definitions.
const LEGACY: [&str; 2] = [
    include_str!("../../../../../../examples/workflows/workflow.yaml"),
    include_str!("../../../../../../examples/workflows/full-mvp-live.yaml"),
];
const FRONTIER: [&str; 4] = [
    "state_7_implementation_started",
    "state_8_implementation_continued",
    "state_9_implementation_reviewed",
    "state_10_implementation_refined",
];

pub(super) fn compile_source(source: &Run) -> anyhow::Result<RunPlan> {
    use workflow::snapshot_integrity::{verify_complete_pair_v1, SnapshotIntegrityV1};
    ensure!(
        matches!(
            verify_complete_pair_v1(
                source.workflow_snapshot_json.as_deref(),
                source.catalog_snapshot_json.as_deref(),
                source.workflow_snapshot_hash.as_deref(),
                source.catalog_snapshot_hash.as_deref()
            ),
            SnapshotIntegrityV1::Verified(_)
        ),
        "unsupported_frontier: unverified source snapshot"
    );
    let workflow = source
        .workflow_snapshot_json
        .as_deref()
        .context("unsupported_frontier")?;
    let catalog = source
        .catalog_snapshot_json
        .as_deref()
        .context("unsupported_frontier")?;
    ensure!(
        workflow.len() <= 4 * 1024 * 1024 && catalog.len() <= 8 * 1024 * 1024,
        "continuation_budget_exceeded: snapshots"
    );
    // Legacy external-skill snapshots which cannot be recompiled without present-day
    // files are deliberately not upgraded or granted compatibility permissions.
    workflow::compiler::compile_from_snapshot_json(
        workflow,
        catalog,
        source
            .agent_catalog_yaml_path
            .as_deref()
            .unwrap_or("/unavailable/source-catalog.yaml"),
    )
    .context("unsupported_frontier: source frozen compiler")
}

fn topology(workflow: &WorkflowFile) -> anyhow::Result<Value> {
    let mut states = serde_json::to_value(&workflow.states)?;
    for state in states
        .as_object_mut()
        .context("unsupported_frontier")?
        .values_mut()
    {
        state
            .as_object_mut()
            .context("unsupported_frontier")?
            .remove("label");
    }
    Ok(json!({"initial_state": workflow.initial_state, "states": states}))
}

pub(super) async fn validate_frontier(
    pool: &SqlitePool,
    source: &Run,
    plan: &RunPlan,
    observation: &db::repos::run_continuation_witness::Observation,
) -> anyhow::Result<(Uuid, String)> {
    ensure!(
        source.workflow_id == "proposal_to_release"
            && FRONTIER.contains(&source.current_state.as_deref().unwrap_or_default()),
        "unsupported_frontier"
    );
    let frozen: WorkflowFile =
        serde_json::from_str(&plan.workflow_snapshot_json).context("unsupported_frontier")?;
    ensure!(
        frozen.workflow.as_ref().and_then(|w| w.id.as_deref()) == Some("proposal_to_release"),
        "unsupported_frontier"
    );
    let topology = topology(&frozen)?;
    let incoming: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM run_continuations WHERE successor_run_id=? AND phase='activated')")
        .bind(source.id.to_string()).fetch_one(pool).await?;
    if incoming {
        let target = workflow::carry_forward::validate_continuation_target_v1(plan.clone())
            .context("unsupported_frontier: successor policy")?;
        ensure!(
            plan.initial_state == target.profile.review_state,
            "unsupported_frontier: successor entry"
        );
    }
    let mut known = false;
    for fixture in LEGACY {
        let mut expected: WorkflowFile = serde_yaml::from_str(fixture)?;
        if incoming {
            expected.initial_state = "state_4_proposal_reviewed".into();
        }
        if topology == self::topology(&expected)? {
            known = true;
            break;
        }
    }
    ensure!(known, "unsupported_frontier: unknown frozen topology");
    let gate = plan
        .states
        .get("state_6_implementation_approval")
        .context("unsupported_frontier")?;
    let prepare = plan
        .states
        .get(FRONTIER[0])
        .context("unsupported_frontier")?;
    let review = plan
        .states
        .get(FRONTIER[2])
        .context("unsupported_frontier")?;
    ensure!(
        gate.is_manual_gate
            && gate.tasks.is_empty()
            && gate.post_approval_tasks.is_empty()
            && gate
                .transitions
                .iter()
                .any(|t| t.to == prepare.id && t.condition == "approval.granted == true")
            && prepare.tasks.len() == 2
            && prepare.tasks[0].task_name == "freeze_approved_proposal_and_prepare_worktree"
            && prepare.tasks[0].agent.agent_id == "lead_orchestrator"
            && prepare.tasks[1].agent.agent_id == "code_writer"
            && prepare.tasks[1].agent.worktree_write_enabled
            && review
                .tasks
                .iter()
                .any(|t| t.agent.agent_id == "proposal_implementation_auditor")
            && review
                .tasks
                .iter()
                .any(|t| t.agent.agent_id == "security_checker"),
        "unsupported_frontier: compiled roles"
    );
    // No historical entry into release, including a run subsequently moved back to implementation.
    let mut before_release = BTreeSet::new();
    let mut pending = vec![plan.initial_state.clone()];
    while let Some(id) = pending.pop() {
        if id == "state_11_manual_release" || !before_release.insert(id.clone()) {
            continue;
        }
        let state = plan.states.get(&id).context("unsupported_frontier")?;
        pending.extend(state.transitions.iter().map(|t| t.to.clone()));
    }
    ensure!(
        observation
            .stage_executions
            .iter()
            .all(|row| row["stage_id"]
                .as_str()
                .is_some_and(|id| before_release.contains(id))),
        "unsupported_frontier: release or unknown stage history"
    );
    let current = source
        .current_state
        .as_deref()
        .context("unsupported_frontier")?;
    let mut candidates = observation
        .stage_executions
        .iter()
        .filter(|row| row["stage_id"] == current)
        .collect::<Vec<_>>();
    ensure!(
        !candidates.is_empty(),
        "unsupported_frontier: no canonical stage"
    );
    candidates.sort_by_key(|row| (row["iteration"].as_i64(), row["attempt_number"].as_i64()));
    let stage = candidates.pop().context("unsupported_frontier")?;
    let iteration = stage["iteration"]
        .as_i64()
        .context("unsupported_frontier")?;
    let attempt = stage["attempt_number"]
        .as_i64()
        .context("unsupported_frontier")?;
    ensure!(
        iteration >= 0
            && attempt > 0
            && candidates.last().is_none_or(|other| (
                other["iteration"].as_i64(),
                other["attempt_number"].as_i64()
            ) != (Some(iteration), Some(attempt))),
        "unsupported_frontier: ambiguous canonical stage"
    );
    ensure!(
        ["blocked", "failed", "completed"].contains(&stage["status"].as_str().unwrap_or_default()),
        "source_busy: stage"
    );
    let stage_id = Uuid::parse_str(stage["id"].as_str().context("unsupported_frontier")?)
        .context("unsupported_frontier: stage UUID")?;
    let transition = observation
        .transition_cursor
        .as_ref()
        .map(|row| canonical_digest(row))
        .transpose()?;
    let cursor = serde_json::to_string(
        &json!({"current_state":current,"stage_execution_id":stage_id,"iteration":iteration,"attempt_number":attempt,"transition":transition}),
    )?;
    ensure!(
        cursor.len() <= 4096,
        "continuation_budget_exceeded: source cursor"
    );
    let released: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM side_effects WHERE run_id=?1) OR EXISTS(SELECT 1 FROM approvals WHERE run_id=?1 AND stage_id='state_11_manual_release')")
        .bind(source.id.to_string()).fetch_one(pool).await?;
    ensure!(!released, "unsupported_frontier: release admitted");
    // Admission needs an actual historical decision, independently of its optional
    // proposal binding. This grant never authorizes the successor.
    let granted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM approvals WHERE run_id=? AND stage_id='state_6_implementation_approval' AND decision='granted' AND decided_at IS NOT NULL)")
        .bind(source.id.to_string()).fetch_one(pool).await?;
    ensure!(
        granted,
        "unsupported_frontier: historical implementation grant missing"
    );
    Ok((stage_id, cursor))
}
