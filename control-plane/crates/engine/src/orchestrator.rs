use anyhow::{Context, Result};
use chrono::Utc;
use md5::Md5;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tracing::{debug, error, info, warn};

use db::repos::{
    agent_execution_runtime_facts, agent_executions, approvals, artifact_contracts, artifacts,
    closeout, ideas, lead_conflict_mediations, projections, runs, stages, work_items,
    workflow_conflicts,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{AgentFailureKind, AgentOutputSettlement, AgentStatus};
use domain::approval::{Approval, ApprovalDecision};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::artifact_contracts::{
    contract_status_allowed_values, ImplementationSelfAssessmentStatus,
};
use domain::closeout_readiness_mode::resolve_closeout_readiness_mode;
use domain::events::DomainEvent;
use domain::ids::{ApprovalId, ArtifactId, RunId};
use domain::proposal_gate_result::{ProposalGateResult, ProposalGateStatus};
use domain::run::RunStatus;
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};
use domain::workflow_conflict::{
    candidate_transition_hash, classify_workflow_conflict_reason, workflow_conflict_fingerprint,
    AdvisoryHintExtraction, CandidateTransitionEvaluation, CandidateTransitionResult,
    ImplementationHandoffStatus, WorkflowAdvisoryRejectionRecord, WorkflowConflictReason,
    WorkflowConflictRecord, WorkflowConflictStatus, WorkflowTransitionCursorRecord,
};

use crate::closeout_fingerprint::{
    build_closeout_fingerprint, resolve_closeout_worktree_truth,
    CLOSEOUT_FINGERPRINT_LATENCY_BUDGET_MS,
};
use crate::closeout_loop_budget::{
    closeout_loop_budget_exhaustion, closeout_loop_budget_remaining,
};
use crate::domain_engine::{DomainEngine, RunEvaluation};
use crate::event_bus::EventSender;
use crate::synthesizers::closeout_readiness::{
    synthesize_implementation_closeout_readiness_for_state9,
    SynthesizerInputs as CloseoutSynthesizerInputs,
};
use crate::work_queue::WorkQueue;
use db::write_class::WriteLane;
use db::writer::{class_a_operation, DbWriter};
use std::sync::Arc;

pub struct Orchestrator {
    pool: SqlitePool,
    events: EventSender,
    work_queue: WorkQueue,
    db_writer: Arc<DbWriter>,
}

impl Orchestrator {
    pub fn new(pool: SqlitePool, events: EventSender, work_queue: WorkQueue) -> Self {
        let db_writer = Arc::new(DbWriter::new(pool.clone()));
        Self::new_with_db_writer(pool, events, work_queue, db_writer)
    }

    pub fn new_with_db_writer(
        pool: SqlitePool,
        events: EventSender,
        work_queue: WorkQueue,
        db_writer: Arc<DbWriter>,
    ) -> Self {
        Self {
            pool,
            events,
            work_queue,
            db_writer,
        }
    }

    async fn begin_orchestrator_transaction(
        &self,
        operation_name: &'static str,
        idempotency_key: impl Into<String>,
    ) -> Result<sqlx::Transaction<'_, sqlx::Sqlite>> {
        self.db_writer
            .begin_immediate_transaction(
                class_a_operation(operation_name, WriteLane::CriticalBarrier, idempotency_key),
                operation_name,
            )
            .await
    }

    async fn enqueue_steward_analysis(&self, completed_run_id: Option<RunId>) -> Result<()> {
        let pending_config_change =
            db::repos::steward::take_config_change_pending(&self.pool).await?;
        let reason = if pending_config_change.is_some() {
            db::repos::steward::reset_completed_run_counter(&self.pool).await?;
            "config_change"
        } else {
            let (enabled, run_interval) =
                db::repos::steward::post_run_trigger_config(&self.pool).await?;
            if !enabled {
                return Ok(());
            }
            let completed_count =
                db::repos::steward::increment_completed_run_counter(&self.pool).await?;
            if completed_count < run_interval {
                return Ok(());
            }
            db::repos::steward::reset_completed_run_counter(&self.pool).await?;
            "post_run_hook"
        };
        self.work_queue
            .enqueue(
                WorkItemKind::StewardAnalysis,
                None,
                None,
                serde_json::json!({
                    "reason": reason,
                    "completed_run_id": completed_run_id.map(|id| id.to_string()),
                }),
            )
            .await
    }

    pub async fn advance_run(&self, run_id: RunId) -> Result<()> {
        let run = match runs::find_by_id(&self.pool, run_id).await? {
            Some(r) => r,
            None => {
                warn!(run_id = %run_id, "advance_run: run not found");
                return Ok(());
            }
        };

        if run.status.is_terminal() {
            projections::rebuild_all_for_run(&self.pool, run_id).await?;
            return Ok(());
        }

        // ── Workflow-driven state machine ────────────────────────────────
        if run.workflow_yaml_path.is_some() && run.agent_catalog_yaml_path.is_some() {
            return self.advance_run_workflow(run_id, &run).await;
        }

        // ── Legacy flat-stage orchestration ──────────────────────────────
        self.advance_run_flat(run_id, &run).await
    }

    // =====================================================================
    // Workflow-driven state machine (matches Swift WorkflowOrchestrator)
    // =====================================================================

    async fn mark_run_completed_and_refresh(
        &self,
        run_id: RunId,
        completed_at: chrono::DateTime<Utc>,
    ) -> Result<()> {
        runs::mark_completed(&self.pool, run_id, completed_at).await?;
        projections::rebuild_all_for_run(&self.pool, run_id).await?;
        Ok(())
    }

    async fn advance_run_workflow(&self, run_id: RunId, run: &domain::run::Run) -> Result<()> {
        let workflow_path = run.workflow_yaml_path.as_deref().unwrap();
        let catalog_path = run.agent_catalog_yaml_path.as_deref().unwrap();
        let plan = match (
            run.workflow_snapshot_json.as_deref(),
            run.catalog_snapshot_json.as_deref(),
        ) {
            (Some(workflow_snapshot), Some(catalog_snapshot))
                if !workflow_snapshot.trim().is_empty() && !catalog_snapshot.trim().is_empty() =>
            {
                match workflow::compiler::compile_from_snapshot_json(
                    workflow_snapshot,
                    catalog_snapshot,
                    catalog_path,
                ) {
                    Ok(plan) => plan,
                    Err(snapshot_error) => {
                        warn!(
                            run_id = %run_id,
                            error = %snapshot_error,
                            "Failed to compile workflow from frozen snapshots; falling back to workflow files"
                        );
                        workflow::compiler::compile(workflow_path, catalog_path)?
                    }
                }
            }
            _ => workflow::compiler::compile(workflow_path, catalog_path)?,
        };

        let current_state_id = run
            .current_state
            .clone()
            .unwrap_or_else(|| plan.initial_state.clone());

        let state = match plan.states.get(&current_state_id) {
            Some(s) => s,
            None => {
                warn!(run_id = %run_id, state = %current_state_id, "State not in plan");
                return Ok(());
            }
        };

        let all_stages = stages::list_by_run(&self.pool, run_id).await?;

        // Find the latest stage for the current state (highest iteration).
        let current_stage = all_stages
            .iter()
            .filter(|s| s.stage_id == current_state_id)
            .last();

        // ── Case 1: stage in progress — wait or check task completion ──
        //
        // IMPORTANT: A stage that is Completed from a **previous** iteration
        // after a cross-state loop-back must NOT be re-evaluated. Instead we
        // fall through to Case 2 (lazy creation) so a new stage is created for
        // the next iteration. Without this check, loop-backs (e.g. state_5→state_4)
        // cause an infinite advance_run cycle because the orchestrator sees the
        // old Completed stage and immediately calls evaluate_and_transition,
        // which transitions again, etc. Same-state self-loops are handled at
        // transition time by pre-creating the next Pending iteration.
        //
        // We detect "stale" completed stages by checking whether any LATER state
        // has a stage (meaning the workflow already moved past this state and
        // looped back). If so, this Completed stage belongs to a prior cycle.
        // Stale detection: a terminal stage from a prior loop iteration must
        // not be re-evaluated — we need to create a new stage instead.
        // Stale detection applies to any terminal status (Completed/Failed/etc)
        // if the workflow has already moved past this state and looped back.
        let stage_is_stale = current_stage
            .filter(|s| {
                matches!(
                    s.status,
                    StageStatus::Completed
                        | StageStatus::Failed
                        | StageStatus::Blocked
                        | StageStatus::Skipped
                )
            })
            .map(|terminal_stage| {
                // If any other stage (different state_id) was started AFTER
                // this one, the workflow has moved past this state and looped back.
                all_stages.iter().any(|other| {
                    other.stage_id != current_state_id
                        && other.started_at > terminal_stage.started_at
                })
            })
            .unwrap_or(false);

        if let Some(stage) = current_stage {
            // BUG 4 fix: Deduplication guard. When multiple AdvanceRun items
            // fire concurrently (e.g. 5 parallel tasks each enqueue one), the
            // first one advances the state. The remaining ones find a Running
            // or WaitingApproval stage that is NOT stale — just return early.
            //
            // P044 carve-out: A manual_gate stage that just transitioned to
            // Running after approval may have zero InvokeAgent work items yet.
            // The post-approval kickstart (below) needs to fire in that case,
            // so we must NOT skip when is_manual_gate + Running + no invokes.
            //
            // P044 refinement: When all enqueued InvokeAgent items for the
            // stage are settled (Completed/Failed), we must NOT skip — phase
            // advancement and stage settlement happen in the match block below.
            if !stage_is_stale
                && matches!(
                    stage.status,
                    StageStatus::Running | StageStatus::WaitingApproval
                )
            {
                let should_skip = if stage.status == StageStatus::Running {
                    let se_id_str = stage.id.to_string();
                    let work_items = db::repos::work_items::list_by_run(&self.pool, run_id)
                        .await
                        .unwrap_or_default();
                    let invokes: Vec<_> = work_items
                        .iter()
                        .filter(|w| {
                            w.kind == db::work_item::WorkItemKind::InvokeAgent
                                && payload_matches_stage_execution(&w.payload_json, &se_id_str)
                        })
                        .collect();
                    if invokes.is_empty() {
                        // No tasks enqueued yet. For manual_gate Running stages
                        // this is the post-approval kickstart moment — must
                        // fall through. For non-manual-gate Running without
                        // invokes (rare, shouldn't normally happen), fall
                        // through too so the match block can handle it.
                        false
                    } else {
                        // Tasks exist: skip only if any are still in flight.
                        // Once all are Completed/Failed, phase advancement
                        // or stage settlement needs the logic below.
                        invokes.iter().any(|w| {
                            matches!(
                                w.status,
                                db::work_item::WorkItemStatus::Pending
                                    | db::work_item::WorkItemStatus::Running
                            )
                        })
                    }
                } else {
                    // WaitingApproval — always skip (waiting for operator).
                    true
                };

                if should_skip {
                    debug!(
                        run_id = %run_id,
                        state = %current_state_id,
                        status = ?stage.status,
                        "Stage already active — skipping redundant AdvanceRun"
                    );
                    return Ok(());
                }
            }

            if stage_is_stale {
                info!(
                    run_id = %run_id,
                    state = %current_state_id,
                    iteration = stage.iteration,
                    "Stale terminal stage from prior loop iteration — creating new stage"
                );
                // Fall through to Case 2 (lazy creation)
            } else {
                match stage.status {
                    StageStatus::Running => {
                        // For multi-task stages (fan-out), check if ALL InvokeAgent
                        // work items for THIS SPECIFIC stage execution have completed.
                        // We filter by stage_execution_id (UUID) from each item's payload
                        // rather than stage_id (logical name) to avoid cross-iteration
                        // contamination — e.g. state_4 iter1 items being counted with
                        // state_4 iter2 items.
                        let se_id_str = stage.id.to_string();
                        let work_items =
                            db::repos::work_items::list_by_run(&self.pool, run_id).await?;
                        let stage_invokes: Vec<_> = work_items
                            .iter()
                            .filter(|w| {
                                w.kind == db::work_item::WorkItemKind::InvokeAgent
                                    && payload_matches_stage_execution(&w.payload_json, &se_id_str)
                            })
                            .collect();
                        let total = stage_invokes.len();
                        let completed_work_items = stage_invokes
                            .iter()
                            .filter(|w| w.status == db::work_item::WorkItemStatus::Completed)
                            .count();
                        let failed_work_items = stage_invokes
                            .iter()
                            .filter(|w| w.status == db::work_item::WorkItemStatus::Failed)
                            .count();
                        let settled_work_items = completed_work_items + failed_work_items;
                        let failed_agent_executions =
                            db::repos::agent_executions::find_by_stage(&self.pool, stage.id)
                                .await?
                                .iter()
                                .filter(|execution| execution.status == AgentStatus::Failed)
                                .count();
                        let failed = failed_work_items + failed_agent_executions;
                        let completed = total.saturating_sub(failed);

                        // Determine if this is a post-approval context (manual_gate
                        // with a Granted approval) so we use the right task list.
                        let is_post_approval = state.is_manual_gate && {
                            let stage_approvals = approvals::list_by_run(&self.pool, run_id)
                                .await
                                .unwrap_or_default();
                            stage_approvals.iter().any(|a| {
                                a.stage_id == current_state_id
                                    && a.decision == ApprovalDecision::Granted
                            })
                        };
                        let effective = effective_tasks(state, is_post_approval);

                        // ── Empty-running kickstart ─────────────────────────
                        // Retry and approval commands can put an existing stage
                        // execution into Running before InvokeAgent work exists.
                        // Treat Running+0 invokes as a resumable start edge,
                        // otherwise the scheduler waits forever for work that
                        // was never enqueued.
                        if total == 0 {
                            info!(
                                run_id = %run_id,
                                state = %current_state_id,
                                post_approval = is_post_approval,
                                "Kickstarting running stage with no InvokeAgent work"
                            );

                            if state.dynamic_parallel.is_some() {
                                if self
                                    .execute_system_routing_if_applicable(run_id, run, stage, &plan)
                                    .await?
                                {
                                    let updated_stage = stages::list_by_run(&self.pool, run_id)
                                        .await?
                                        .into_iter()
                                        .find(|s| s.id == stage.id);
                                    if let Some(ref s) = updated_stage {
                                        if matches!(
                                            s.status,
                                            StageStatus::Failed | StageStatus::Blocked
                                        ) {
                                            return Ok(());
                                        }
                                    }
                                }

                                if let Some(ref dp) = state.dynamic_parallel {
                                    let artifact_path = format!(
                                        "{}/routing/agent-selection-plan.v1.json",
                                        run.artifact_root.trim_end_matches('/')
                                    );
                                    if std::path::Path::new(&artifact_path).exists() {
                                        let idea_opt = ideas::find_by_id(&self.pool, run.idea_id)
                                            .await
                                            .ok()
                                            .flatten();
                                        self.materialize_dynamic_parallel(
                                            run_id,
                                            run,
                                            stage,
                                            &plan,
                                            dp,
                                            idea_opt.as_ref(),
                                        )
                                        .await?;
                                        return Ok(());
                                    }
                                }
                            }

                            let idea_opt = ideas::find_by_id(&self.pool, run.idea_id)
                                .await
                                .ok()
                                .flatten();
                            let effective_total = effective.len();
                            if effective_total == 0 {
                                let prompt = build_task_prompt_for_owner(
                                    state,
                                    &plan,
                                    run,
                                    idea_opt.as_ref(),
                                );
                                self.enqueue_invoke_agent_for_owner(
                                    run_id,
                                    stage,
                                    &state.owner,
                                    &prompt,
                                    0,
                                    1,
                                )
                                .await?;
                                return Ok(());
                            }

                            let approval_rejection_context = self
                                .approval_rejection_context_for_state(run_id, &current_state_id)
                                .await?;
                            for (i, task) in effective.iter().enumerate() {
                                if task.phase != 0 {
                                    continue;
                                }
                                if self
                                    .block_code_writer_handoff_if_unavailable(
                                        run_id,
                                        &current_state_id,
                                        stage,
                                        task,
                                        &plan,
                                        run,
                                    )
                                    .await?
                                {
                                    return Ok(());
                                }
                                let prompt = build_task_prompt(
                                    task,
                                    &plan,
                                    run,
                                    idea_opt.as_ref(),
                                    None,
                                    approval_rejection_context.as_deref(),
                                );
                                self.enqueue_invoke_agent(
                                    run_id,
                                    stage,
                                    task,
                                    &prompt,
                                    i,
                                    effective_total,
                                    &plan,
                                    run,
                                )
                                .await?;
                            }
                            return Ok(());
                        }

                        if total > 0 && settled_work_items == total {
                            // All enqueued tasks finished. Generalized N-phase
                            // gating: determine which phase just completed, then
                            // check if a subsequent phase exists and needs enqueuing.

                            // Determine the current (just-completed) phase from
                            // the work items that were enqueued.
                            // P060: dynamic_parallel work items carry an explicit
                            // p060_dynamic_phase field; use it when present instead
                            // of looking up through the effective task list.
                            let current_phase: u32 = stage_invokes
                                .iter()
                                .filter_map(|w| {
                                    let v =
                                        serde_json::from_str::<serde_json::Value>(&w.payload_json)
                                            .ok()?;
                                    // P060: explicit dynamic phase takes precedence.
                                    if let Some(dp) =
                                        v.get("p060_dynamic_phase").and_then(|p| p.as_u64())
                                    {
                                        return Some(dp as u32);
                                    }
                                    v.get("task_index")?.as_u64().and_then(|idx| {
                                        effective.get(idx as usize).map(|t| t.phase)
                                    })
                                })
                                .max()
                                .unwrap_or(0);

                            // Find the next phase (if any) that hasn't been enqueued.
                            let next_phase: Option<u32> = effective
                                .iter()
                                .map(|t| t.phase)
                                .filter(|&p| p > current_phase)
                                .min();

                            // Check if any tasks from the next phase are already enqueued.
                            // P060: also check p060_dynamic_phase for dynamic work items.
                            let next_phase_already_enqueued = next_phase.map_or(true, |np| {
                                stage_invokes.iter().any(|w| {
                                    let v =
                                        serde_json::from_str::<serde_json::Value>(&w.payload_json)
                                            .ok();
                                    let v = match v {
                                        Some(v) => v,
                                        None => return false,
                                    };
                                    // P060: check explicit dynamic phase.
                                    if let Some(dp) =
                                        v.get("p060_dynamic_phase").and_then(|p| p.as_u64())
                                    {
                                        return dp as u32 == np;
                                    }
                                    v.get("task_index")
                                        .and_then(|ti| ti.as_u64())
                                        .map(|idx| {
                                            effective
                                                .get(idx as usize)
                                                .map_or(false, |t| t.phase == np)
                                        })
                                        .unwrap_or(false)
                                })
                            });

                            if let Some(np) = next_phase {
                                if !next_phase_already_enqueued {
                                    // Current phase complete — enqueue next phase.
                                    if failed > 0 {
                                        warn!(
                                            run_id = %run_id,
                                            state = %current_state_id,
                                            phase = current_phase,
                                            failed = failed,
                                            "Phase {} had failures — skipping phase {}, settling as Failed",
                                            current_phase, np
                                        );
                                    } else {
                                        info!(
                                            run_id = %run_id,
                                            state = %current_state_id,
                                            completed_phase = current_phase,
                                            next_phase = np,
                                            "Phase {} complete — enqueuing phase {} tasks",
                                            current_phase, np
                                        );
                                        let idea_opt = ideas::find_by_id(&self.pool, run.idea_id)
                                            .await
                                            .ok()
                                            .flatten();
                                        let effective_total = effective.len();
                                        let approval_rejection_context = self
                                            .approval_rejection_context_for_state(
                                                run_id,
                                                &current_state_id,
                                            )
                                            .await?;
                                        for (i, task) in effective.iter().enumerate() {
                                            if task.phase != np {
                                                continue;
                                            }
                                            if self
                                                .block_code_writer_handoff_if_unavailable(
                                                    run_id,
                                                    &current_state_id,
                                                    stage,
                                                    task,
                                                    &plan,
                                                    run,
                                                )
                                                .await?
                                            {
                                                return Ok(());
                                            }
                                            let mut prompt = build_task_prompt(
                                                task,
                                                &plan,
                                                run,
                                                idea_opt.as_ref(),
                                                None,
                                                approval_rejection_context.as_deref(),
                                            );
                                            // P060: If the then-task declares selected_outputs_from,
                                            // resolve selected reviewer artifacts and inject paths.
                                            if task.selected_outputs_from.is_some() {
                                                if let Ok(Some((_bundle, file_paths))) = self
                                                    .resolve_selected_outputs_for_task(
                                                        run_id, run, stage, task,
                                                    )
                                                    .await
                                                {
                                                    if !file_paths.is_empty() {
                                                        prompt.push_str(
                                                            "\n\n### Selected Reviewer Artifacts\n",
                                                        );
                                                        prompt.push_str("The following reviewer outputs were selected by the routing algorithm. ");
                                                        prompt.push_str(
                                                            "Aggregate only these artifacts:\n",
                                                        );
                                                        for path in &file_paths {
                                                            prompt.push_str(&format!(
                                                                "- `{}`\n",
                                                                path
                                                            ));
                                                        }
                                                    }
                                                }
                                            }
                                            self.enqueue_invoke_agent(
                                                run_id,
                                                stage,
                                                task,
                                                &prompt,
                                                i,
                                                effective_total,
                                                &plan,
                                                run,
                                            )
                                            .await?;
                                        }
                                        return Ok(()); // wait for next phase to complete
                                    }
                                }
                            }

                            // All phases finished — settle stage.
                            let now = Utc::now();
                            let (kind, settle_status) = if failed > 0 {
                                (
                                    domain::stage::StageSettlementKind::Failed,
                                    StageStatus::Failed,
                                )
                            } else {
                                (
                                    domain::stage::StageSettlementKind::Completed,
                                    StageStatus::Completed,
                                )
                            };
                            info!(
                                run_id = %run_id,
                                state = %current_state_id,
                                total = total,
                                completed = completed,
                                failed = failed,
                                "All tasks finished — settling stage"
                            );
                            if kind == domain::stage::StageSettlementKind::Failed {
                                if self
                                    .schedule_auto_contract_output_retry_for_stage(
                                        run_id, run, stage,
                                    )
                                    .await?
                                {
                                    return Ok(());
                                }
                                crate::recovery::persist_failed_stage_recovery_snapshot(
                                    &self.pool, stage.id, now,
                                )
                                .await?;
                                if let Some(evidence_artifact) =
                                    crate::evidence::build_failed_stage_evidence_for_latest_execution(
                                        &self.pool, run, stage, now,
                                    )
                                    .await?
                                {
                                    let _ = self.events.send(DomainEvent::ArtifactCreated {
                                        run_id,
                                        artifact_id: evidence_artifact.id,
                                    });
                                }
                            }
                            stages::settle(&self.pool, stage.id, kind, now).await?;
                            let _ = self.events.send(DomainEvent::StageStatusChanged {
                                run_id,
                                stage_execution_id: stage.id,
                                status: settle_status.clone(),
                            });
                            if settle_status == StageStatus::Completed {
                                // P044 §3h: End states with tasks complete the run
                                // directly after their tasks finish. We must not
                                // evaluate transitions — state_12 declares a
                                // `when: "true"` self-transition that would loop
                                // forever otherwise.
                                if state.is_end {
                                    info!(
                                        run_id = %run_id,
                                        state = %current_state_id,
                                        "End state with tasks complete — marking run Completed"
                                    );
                                    self.mark_run_completed_and_refresh(run_id, now).await?;
                                    self.enqueue_steward_analysis(Some(run_id)).await?;
                                    self.cleanup_worktree_if_needed(&run).await;
                                    let _ = self.events.send(DomainEvent::RunStatusChanged {
                                        run_id,
                                        status: RunStatus::Completed,
                                    });
                                    return Ok(());
                                }
                                // P077: Synthesize and persist closeout readiness before
                                // transition evaluation so implementation_closeout_readiness_v1
                                // guards can read an active generation from closeout_gate_generations.
                                self.synthesize_and_persist_closeout_readiness_if_needed(
                                    run_id,
                                    &current_state_id,
                                    state,
                                    run,
                                    &plan,
                                    &all_stages,
                                )
                                .await?;
                                return self
                                    .evaluate_and_transition(
                                        run_id,
                                        &current_state_id,
                                        &plan,
                                        &all_stages,
                                    )
                                    .await;
                            }
                            // Failed — run blocked
                            runs::update_status(&self.pool, run_id, RunStatus::Blocked).await?;
                            return Ok(());
                        }

                        // Tasks still running — wait
                        return Ok(());
                    }
                    StageStatus::WaitingApproval => {
                        return Ok(()); // wait for approval
                    }
                    StageStatus::Completed => {
                        // P077: synthesize closeout readiness before transition evaluation.
                        self.synthesize_and_persist_closeout_readiness_if_needed(
                            run_id,
                            &current_state_id,
                            state,
                            run,
                            &plan,
                            &all_stages,
                        )
                        .await?;
                        // Stage done — evaluate transitions
                        return self
                            .evaluate_and_transition(run_id, &current_state_id, &plan, &all_stages)
                            .await;
                    }
                    StageStatus::Failed | StageStatus::Blocked => {
                        if state.is_end
                            && all_stages.iter().any(|candidate| {
                                candidate.stage_id == current_state_id
                                    && candidate.status == StageStatus::Completed
                            })
                        {
                            let now = Utc::now();
                            info!(
                                run_id = %run_id,
                                state = %current_state_id,
                                "End state has a prior completed attempt — marking run Completed despite later failed retry"
                            );
                            self.mark_run_completed_and_refresh(run_id, now).await?;
                            self.enqueue_steward_analysis(Some(run_id)).await?;
                            self.cleanup_worktree_if_needed(&run).await;
                            let _ = self.events.send(DomainEvent::RunStatusChanged {
                                run_id,
                                status: RunStatus::Completed,
                            });
                            return Ok(());
                        }
                        // Stage failed — update run status
                        if run.status != RunStatus::Blocked {
                            runs::update_status(&self.pool, run_id, RunStatus::Blocked).await?;
                            let _ = self.events.send(DomainEvent::RunStatusChanged {
                                run_id,
                                status: RunStatus::Blocked,
                            });
                        }
                        return Ok(());
                    }
                    StageStatus::Skipped => {
                        // Skipped (e.g. retry) — re-evaluate
                        return self
                            .evaluate_and_transition(run_id, &current_state_id, &plan, &all_stages)
                            .await;
                    }
                    StageStatus::Pending => {
                        // P044 §3g: A retried manual_gate stage was inserted as
                        // Pending by RetryStage. We must restore it to
                        // WaitingApproval with a fresh Approval record on the
                        // same stage execution instead of falling through to
                        // Case 2, which would fork lineage by creating another
                        // stage via create_stage_for_state.
                        if state.is_manual_gate {
                            info!(
                                run_id = %run_id,
                                state = %current_state_id,
                                stage_execution_id = %stage.id,
                                attempt = stage.attempt_number,
                                "Retried manual gate — restoring to WaitingApproval with fresh approval"
                            );
                            stages::update_status(
                                &self.pool,
                                stage.id,
                                StageStatus::WaitingApproval,
                            )
                            .await?;

                            let approval = Approval {
                                id: ApprovalId::new(),
                                run_id,
                                stage_id: current_state_id.clone(),
                                decision: ApprovalDecision::Requested,
                                requested_at: Utc::now(),
                                decided_at: None,
                                comment: None,
                                expires_at: None,
                            };
                            approvals::insert(&self.pool, &approval).await?;

                            let _ = self.events.send(DomainEvent::StageStatusChanged {
                                run_id,
                                stage_execution_id: stage.id,
                                status: StageStatus::WaitingApproval,
                            });
                            let _ = self.events.send(DomainEvent::ApprovalRequested {
                                run_id,
                                approval_id: approval.id,
                                stage_id: current_state_id.clone(),
                            });

                            if run.status != RunStatus::WaitingApproval {
                                runs::update_status(&self.pool, run_id, RunStatus::WaitingApproval)
                                    .await?;
                                let _ = self.events.send(DomainEvent::RunStatusChanged {
                                    run_id,
                                    status: RunStatus::WaitingApproval,
                                });
                            }
                            projections::rebuild_run_summary(&self.pool, run_id).await?;
                            projections::rebuild_stage_summaries(&self.pool, run_id).await?;
                            projections::rebuild_approval_inbox(&self.pool, run_id).await?;
                            return Ok(());
                        }
                        // Non-manual-gate Pending stages are left to fall
                        // through to Case 2 (preserves existing retry
                        // semantics for compute states, which are not in
                        // P044's scope).
                    }
                    _ => {}
                }
            } // end if !stage_is_stale
        }

        // ── Case 2: no stage yet — lazy creation ────────────────────────

        // End state — mark run complete (or fall through if it has tasks)
        if state.is_end {
            if state.tasks.is_empty() {
                // Bare end state — settle immediately (no tasks to run).
                info!(run_id = %run_id, state = %current_state_id, "Reached bare end state");
                self.create_stage_for_state(run_id, &current_state_id, state)
                    .await?;
                let now = Utc::now();
                let end_stage = stages::list_by_run(&self.pool, run_id)
                    .await?
                    .into_iter()
                    .find(|s| s.stage_id == current_state_id)
                    .unwrap();
                stages::settle(
                    &self.pool,
                    end_stage.id,
                    domain::stage::StageSettlementKind::Completed,
                    now,
                )
                .await?;
                self.mark_run_completed_and_refresh(run_id, now).await?;
                self.enqueue_steward_analysis(Some(run_id)).await?;
                // Worktree cleanup on completion (Proposal 007).
                self.cleanup_worktree_if_needed(&run).await;
                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id,
                    status: RunStatus::Completed,
                });
                return Ok(());
            }
            // End state with tasks — fall through to regular compute-state
            // handling. evaluate_and_transition will see is_end and mark
            // the run completed after tasks finish.
            info!(run_id = %run_id, state = %current_state_id, "End state with tasks — entering compute path");
        }

        // Manual gate — create stage as WaitingApproval + Approval record
        if state.is_manual_gate {
            info!(run_id = %run_id, state = %current_state_id, "Entering manual gate");
            let stage = self
                .create_stage_for_state(run_id, &current_state_id, state)
                .await?;
            stages::update_status(&self.pool, stage.id, StageStatus::WaitingApproval).await?;

            let approval = Approval {
                id: ApprovalId::new(),
                run_id,
                stage_id: current_state_id.clone(),
                decision: ApprovalDecision::Requested,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            };
            approvals::insert(&self.pool, &approval).await?;

            let _ = self.events.send(DomainEvent::StageStatusChanged {
                run_id,
                stage_execution_id: stage.id,
                status: StageStatus::WaitingApproval,
            });
            let _ = self.events.send(DomainEvent::ApprovalRequested {
                run_id,
                approval_id: approval.id,
                stage_id: current_state_id.clone(),
            });

            if run.status != RunStatus::WaitingApproval {
                runs::update_status(&self.pool, run_id, RunStatus::WaitingApproval).await?;
                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id,
                    status: RunStatus::WaitingApproval,
                });
            }
            projections::rebuild_run_summary(&self.pool, run_id).await?;
            projections::rebuild_stage_summaries(&self.pool, run_id).await?;
            projections::rebuild_approval_inbox(&self.pool, run_id).await?;
            return Ok(());
        }

        if self
            .blocked_implementation_review_available(run_id, state)
            .await?
        {
            let stage = self
                .create_stage_for_state(run_id, &current_state_id, state)
                .await?;
            stages::update_status(&self.pool, stage.id, StageStatus::Running).await?;
            let _ = self.events.send(DomainEvent::StageStatusChanged {
                run_id,
                stage_execution_id: stage.id,
                status: StageStatus::Running,
            });
            if self
                .synthesize_blocked_implementation_review_if_needed(
                    run_id, &stage, state, &plan, run,
                )
                .await?
            {
                let now = Utc::now();
                stages::settle(
                    &self.pool,
                    stage.id,
                    domain::stage::StageSettlementKind::Completed,
                    now,
                )
                .await?;
                let _ = self.events.send(DomainEvent::StageStatusChanged {
                    run_id,
                    stage_execution_id: stage.id,
                    status: StageStatus::Completed,
                });
                let all_stages = stages::list_by_run(&self.pool, run_id).await?;
                return self
                    .evaluate_and_transition(run_id, &current_state_id, &plan, &all_stages)
                    .await;
            }
        }

        // ── Worktree provisioning (Proposal 007) ────────────────────────
        // If any agent in this state needs a real git worktree (dedicated or
        // shared — NOT meta_only), provision one after the run-start rollout
        // contract preflight has had a chance to fail closed.
        let needs_git_worktree = {
            let needs_wt = |a: &workflow::plan::ResolvedAgent| -> bool {
                a.worktree_write_enabled && a.worktree_strategy.as_deref() != Some("meta_only")
            };
            state.tasks.iter().any(|t| needs_wt(&t.agent))
                || state.post_approval_tasks.iter().any(|t| needs_wt(&t.agent))
                || needs_wt(&state.owner)
        };
        let implementation_run_start_task = state
            .tasks
            .iter()
            .chain(state.post_approval_tasks.iter())
            .find(|task| is_code_writer_implementation_task(task));
        // Re-bind `run` as mutable reference so we can refresh it after provisioning.
        let mut run = run.clone();

        // Regular compute state — start an existing pending retry attempt, or
        // lazily create the first stage execution when no attempt exists.
        let stage = if let Some(pending_stage) =
            current_stage.filter(|s| !stage_is_stale && s.status == StageStatus::Pending)
        {
            info!(
                run_id = %run_id,
                state = %current_state_id,
                stage_execution_id = %pending_stage.id,
                attempt = pending_stage.attempt_number,
                provider = %state.owner.provider,
                "Entering compute state from pending retry stage"
            );
            pending_stage.clone()
        } else {
            info!(run_id = %run_id, state = %current_state_id, provider = %state.owner.provider, "Entering compute state");
            self.create_stage_for_state(run_id, &current_state_id, state)
                .await?
        };

        if let Some(task) = implementation_run_start_task {
            if self
                .block_implementation_run_start_if_rollout_contract_hold(
                    run_id,
                    &current_state_id,
                    &stage,
                    task,
                    &plan,
                    &run,
                )
                .await?
            {
                return Ok(());
            }
        }

        stages::update_status(&self.pool, stage.id, StageStatus::Running).await?;

        let _ = self.events.send(DomainEvent::StageStatusChanged {
            run_id,
            stage_execution_id: stage.id,
            status: StageStatus::Running,
        });

        if !matches!(run.status, RunStatus::Running) {
            runs::update_status(&self.pool, run_id, RunStatus::Running).await?;
            let _ = self.events.send(DomainEvent::RunStatusChanged {
                run_id,
                status: RunStatus::Running,
            });
        }

        if needs_git_worktree && run.worktree_root.is_none() {
            let idea_opt_for_slug = ideas::find_by_id(&self.pool, run.idea_id)
                .await
                .ok()
                .flatten();
            let idea_title = idea_opt_for_slug
                .as_ref()
                .map(|i| i.title.as_str())
                .unwrap_or("untitled");

            // Extract base_branch from the first agent with a worktree_policy in the catalog.
            let base_branch = self.resolve_base_branch_from_catalog(&run);

            info!(
                run_id = %run_id,
                state = %current_state_id,
                base_branch = ?base_branch,
                "Provisioning worktree for write-enabled state"
            );
            match crate::worktree::WorktreeProvisioner::provision(
                &run.workspace_root,
                run_id,
                idea_title,
                base_branch.as_deref(),
            )
            .await
            {
                Ok(result) => {
                    runs::update_worktree_fields(
                        &self.pool,
                        run_id,
                        &result.worktree_root,
                        &result.base_branch,
                        &result.base_revision,
                        &result.target_branch,
                    )
                    .await?;
                    // Re-read run so prompt building sees worktree_root.
                    run = runs::find_by_id(&self.pool, run_id).await?.ok_or_else(|| {
                        anyhow::anyhow!("Run vanished after provisioning: {}", run_id)
                    })?;
                    info!(
                        run_id = %run_id,
                        worktree_root = %result.worktree_root,
                        target_branch = %result.target_branch,
                        "Worktree provisioned"
                    );
                }
                Err(e) => {
                    error!(
                        run_id = %run_id,
                        state = %current_state_id,
                        error = %e,
                        "Worktree provisioning failed — blocking run"
                    );
                    runs::update_status(&self.pool, run_id, RunStatus::Blocked).await?;
                    let _ = self.events.send(DomainEvent::RunStatusChanged {
                        run_id,
                        status: RunStatus::Blocked,
                    });
                    return Ok(());
                }
            }
        }

        if self
            .synthesize_blocked_implementation_review_if_needed(run_id, &stage, state, &plan, &run)
            .await?
        {
            let now = Utc::now();
            stages::settle(
                &self.pool,
                stage.id,
                domain::stage::StageSettlementKind::Completed,
                now,
            )
            .await?;
            let _ = self.events.send(DomainEvent::StageStatusChanged {
                run_id,
                stage_execution_id: stage.id,
                status: StageStatus::Completed,
            });
            let all_stages = stages::list_by_run(&self.pool, run_id).await?;
            return self
                .evaluate_and_transition(run_id, &current_state_id, &plan, &all_stages)
                .await;
        }

        // Fetch the originating idea so we can inject its title+body into
        // prompts that consume `input.idea`. Without this, the agent only
        // sees a placeholder line ("path not defined in catalog") and has no
        // access to what the user actually asked for.
        let idea_opt = ideas::find_by_id(&self.pool, run.idea_id)
            .await
            .ok()
            .flatten();

        // Proposal 007 §7.7: Validate worktree readiness before write-enabled execution.
        // Also validates for release agents that need worktree (strategy=dedicated,
        // even if write_enabled=false — they read from the worktree to commit/push).
        let any_agent_needs_worktree = needs_git_worktree
            || state.tasks.iter().any(|t| {
                t.agent.worktree_strategy.as_deref() == Some("dedicated")
                    || t.agent.worktree_strategy.as_deref()
                        == Some("shared_implementation_worktree")
            })
            || state.post_approval_tasks.iter().any(|t| {
                t.agent.worktree_strategy.as_deref() == Some("dedicated")
                    || t.agent.worktree_strategy.as_deref()
                        == Some("shared_implementation_worktree")
            })
            || matches!(
                state.owner.worktree_strategy.as_deref(),
                Some("dedicated") | Some("shared_implementation_worktree")
            );

        if any_agent_needs_worktree {
            if let Err(e) = crate::worktree::RepoSafetyGuard::validate_worktree_ready(
                run.worktree_root.as_deref(),
            ) {
                error!(
                    run_id = %run_id,
                    state = %current_state_id,
                    error = %e,
                    "RepoSafetyGuard failed — blocking run"
                );
                runs::update_status(&self.pool, run_id, RunStatus::Blocked).await?;
                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id,
                    status: RunStatus::Blocked,
                });
                return Ok(());
            }
        }

        // Proposal 007: gather source context (changed files manifest) for
        // write-enabled states so implementation agents see what's already changed.
        let source_ctx = if needs_git_worktree {
            if let (Some(wt), Some(bb)) = (run.worktree_root.as_deref(), run.base_branch.as_deref())
            {
                crate::worktree::build_source_context(wt, bb).await.ok()
            } else {
                None
            }
        } else {
            None
        };

        // P060: Execute system routing before dispatching agent tasks.
        // If routing is applicable and was executed (success or handled failure),
        // the routing outcome is persisted. On failure, the stage is settled and
        // we return early. On success, we continue with normal task dispatch.
        if self
            .execute_system_routing_if_applicable(run_id, &run, &stage, &plan)
            .await?
        {
            // Check if routing failed (stage settled as Failed/Blocked).
            let updated_stage = stages::list_by_run(&self.pool, run_id)
                .await?
                .into_iter()
                .find(|s| s.id == stage.id);
            if let Some(ref s) = updated_stage {
                if matches!(s.status, StageStatus::Failed | StageStatus::Blocked) {
                    return Ok(());
                }
            }
            // Routing succeeded — continue with normal task dispatch.
        }

        // P060: If routing succeeded and the state has dynamic_parallel,
        // materialize selected reviewers instead of dispatching static phase 0 tasks.
        if state.dynamic_parallel.is_some() {
            if let Some(ref dp) = state.dynamic_parallel {
                // Check if the AgentSelectionPlanV1 artifact exists (routing must have run).
                let artifact_path = format!(
                    "{}/routing/agent-selection-plan.v1.json",
                    run.artifact_root.trim_end_matches('/')
                );
                if std::path::Path::new(&artifact_path).exists() {
                    let idea_opt = ideas::find_by_id(&self.pool, run.idea_id)
                        .await
                        .ok()
                        .flatten();
                    let enqueued = self
                        .materialize_dynamic_parallel(
                            run_id,
                            &run,
                            &stage,
                            &plan,
                            dp,
                            idea_opt.as_ref(),
                        )
                        .await?;
                    if enqueued > 0 {
                        return Ok(());
                    }
                    // If enqueued == 0, all tasks were already materialized (resume).
                    // Fall through — the phase advancement logic will handle completion
                    // checks and then-block dispatch.
                    return Ok(());
                }
                // No artifact yet — routing hasn't run or failed. Fall through to
                // normal dispatch (which may include the system.routing task).
            }
        }

        if state.tasks.is_empty() {
            // No tasks defined — run the owner agent as a single task
            let prompt = build_task_prompt_for_owner(state, &plan, &run, idea_opt.as_ref());
            self.enqueue_invoke_agent_for_owner(run_id, &stage, &state.owner, &prompt, 0, 1)
                .await?;
        } else {
            // Phase-aware enqueuing: phase 0 tasks (parallel + initial sequence)
            // are enqueued immediately. Phase 1 tasks (`then` blocks — sequential
            // after parallel) are deferred until all phase 0 tasks complete.
            // This prevents aggregators from racing with the tasks they aggregate.
            let phase0_tasks: Vec<_> = state
                .tasks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.phase == 0)
                .collect();
            let total = state.tasks.len();
            let approval_rejection_context = self
                .approval_rejection_context_for_state(run_id, &current_state_id)
                .await?;
            for (i, task) in &phase0_tasks {
                if self
                    .block_code_writer_handoff_if_unavailable(
                        run_id,
                        &current_state_id,
                        &stage,
                        task,
                        &plan,
                        &run,
                    )
                    .await?
                {
                    return Ok(());
                }
                let prompt = build_task_prompt(
                    task,
                    &plan,
                    &run,
                    idea_opt.as_ref(),
                    source_ctx.as_ref(),
                    approval_rejection_context.as_deref(),
                );
                info!(
                    run_id = %run_id,
                    task = %task.task_name,
                    agent = %task.agent.agent_id,
                    provider = %task.agent.provider,
                    parallel = task.parallel,
                    phase = task.phase,
                    index = i,
                    total = total,
                    "Enqueuing task (phase 0)"
                );
                self.enqueue_invoke_agent(run_id, &stage, task, &prompt, *i, total, &plan, &run)
                    .await?;
            }
        }

        Ok(())
    }

    async fn synthesize_blocked_implementation_review_if_needed(
        &self,
        run_id: RunId,
        stage: &StageExecution,
        state: &workflow::plan::CompiledState,
        plan: &workflow::plan::RunPlan,
        run: &domain::run::Run,
    ) -> Result<bool> {
        if !state_produces_implementation_review(state) {
            return Ok(false);
        }

        let Some(active) = artifact_contracts::find_active_implementation_self_assessment_summary(
            &self.pool, run_id,
        )
        .await?
        else {
            return Ok(false);
        };
        if active.summary.status != ImplementationSelfAssessmentStatus::Blocked {
            return Ok(false);
        }

        let target_path = plan
            .artifact_paths
            .get("implementation_review_summary")
            .map(|template| {
                resolve_path_template(
                    template,
                    &run.workspace_root,
                    run.chainworks_meta_root.as_deref(),
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "{}/review/implementation-summary.json",
                    run.artifact_root.trim_end_matches('/')
                )
            });
        if let Some(parent) = std::path::Path::new(&target_path).parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "create implementation review summary directory {}",
                    parent.display()
                )
            })?;
        }

        let implementation_self_assessment_summary = serde_json::to_value(&active.summary)
            .context("serialize implementation self-assessment summary for release hold")?;
        let payload = serde_json::json!({
            "status": "release_evidence_blocked",
            "open_blockers": 0,
            "must_fix": [],
            "recommended_next_step": "hold_release_until_verification_green",
            "source": "implementation_self_assessment_v2",
            "implementation_self_assessment_status": active.summary.status.as_str(),
            "verification_green": active.summary.verification_green,
            "implementation_self_assessment_summary": implementation_self_assessment_summary,
            "owner_class_counts": &active.summary.owner_class_counts,
            "target_stage_summaries": &active.summary.target_stage_summaries,
            "remaining_code_tasks": &active.summary.remaining_code_tasks,
            "handoff_tasks": &active.summary.handoff_tasks,
            "known_risks": &active.summary.known_risks,
            "tests_run": &active.summary.tests_run,
            "docs_impacted": &active.summary.docs_impacted,
            "validation_errors": &active.summary.validation_errors,
            "warnings": &active.summary.warnings
        });
        let bytes = serde_json::to_vec_pretty(&payload)
            .context("serialize blocked implementation review summary")?;
        std::fs::write(&target_path, &bytes)
            .with_context(|| format!("write implementation review summary {target_path}"))?;

        let artifact = Artifact {
            id: domain::ids::ArtifactId::new(),
            run_id,
            stage_id: stage.stage_id.clone(),
            agent_id: "lead_orchestrator".to_string(),
            name: "implementation_review_summary".to_string(),
            contract_id: "implementation_review_summary_v1".to_string(),
            format: ArtifactFormat::Json,
            file_path: target_path,
            checksum_sha256: None,
            size_bytes: Some(bytes.len() as i64),
            provider: "engine".to_string(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(&self.pool, &artifact).await?;
        artifact_contracts::upsert_generation_and_rebuild(
            &self.pool,
            domain::artifact_contracts::ActiveArtifactGenerationInput {
                run_id,
                artifact_id: artifact.id,
                contract_id: artifact.contract_id.clone(),
                canonical_path: artifact.name.clone(),
                raw_path: artifact.file_path.clone(),
                raw_status: "release_evidence_blocked".to_string(),
                generation_id: format!("engine-synthesized:{}:{}", stage.id, artifact.id),
                source_agent_execution_id: None,
                source_stage_execution_id: Some(stage.id.to_string()),
                source_session_generation_id: None,
                source_work_item_id: None,
                supersedes_generation_id: None,
                output_settlement: AgentOutputSettlement::None,
                partial: false,
                warnings: vec![
                    "implementation_review_summary was synthesized from active implementation_self_assessment_v2 release-hold truth"
                        .to_string(),
                ],
            },
        )
        .await?;
        let _ = self.events.send(DomainEvent::ArtifactCreated {
            run_id,
            artifact_id: artifact.id,
        });
        Ok(true)
    }

    async fn blocked_implementation_review_available(
        &self,
        run_id: RunId,
        state: &workflow::plan::CompiledState,
    ) -> Result<bool> {
        if !state_produces_implementation_review(state) {
            return Ok(false);
        }
        let Some(active) = artifact_contracts::find_active_implementation_self_assessment_summary(
            &self.pool, run_id,
        )
        .await?
        else {
            return Ok(false);
        };
        Ok(active.summary.status == ImplementationSelfAssessmentStatus::Blocked)
    }

    /// P077: Synthesize implementation_closeout_readiness_v1 and atomically persist
    /// both proposal_gate_result_v1 and implementation_closeout_readiness_v1 into
    /// closeout_gate_generations before transition evaluation fires.
    ///
    /// Called at every stage-completion point for states whose transitions reference
    /// implementation_closeout_readiness_v1.decision. The transaction deactivates
    /// the previous active generation, so this is safe to call on re-entry.
    ///
    /// On DB failure the error is logged and the function returns Ok(()); the
    /// transition guard will simply not match and the run stays in this state until
    /// the next AdvanceRun trigger, which retries the synthesis.
    async fn synthesize_and_persist_closeout_readiness_if_needed(
        &self,
        run_id: RunId,
        current_state_id: &str,
        state: &workflow::plan::CompiledState,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
        all_stages: &[StageExecution],
    ) -> Result<()> {
        if !state_evaluates_closeout_readiness(state) {
            return Ok(());
        }

        let run_id_str = run_id.to_string();

        // Read the active proposal gate result (if the operator has already settled one).
        // If none, use MissingDefinition — synthesizer routes to AwaitGateDefinition.
        let gate_result = match closeout::find_active_gate_generation(&self.pool, &run_id_str).await
        {
            Ok(Some(row)) => ProposalGateResult {
                gate_id: format!("p077:{}", &row.generation_id),
                proposal_id: "077".to_string(),
                run_id: run_id_str.clone(),
                stage_id: current_state_id.to_string(),
                status: row
                    .status
                    .parse::<ProposalGateStatus>()
                    .unwrap_or(ProposalGateStatus::Invalid),
                generation_id: row.generation_id,
                diagnostic_reason: row.diagnostic_reason,
                executor_version: None,
                evidence_digest: None,
                exit_code: None,
                elapsed_ms: None,
                settled_at: chrono::Utc::now(),
                authorization_lineage: None,
                failure_classification: None,
            },
            _ => ProposalGateResult::missing_definition(
                format!("gate-missing:{}", uuid::Uuid::new_v4()),
                &run_id_str,
                "077",
                current_state_id,
                "no_proposal_gate_settled",
            ),
        };

        // Resolve the closeout readiness mode from the frozen run column.
        let has_enforcement_migration =
            closeout::has_enforcement_migration_record(&self.pool, &run_id_str)
                .await
                .unwrap_or(false);
        let mode_result = resolve_closeout_readiness_mode(
            run.closeout_readiness_mode.as_deref(),
            has_enforcement_migration,
        );

        // Load the active implementation self-assessment (optional input).
        let self_assessment_opt =
            artifact_contracts::find_active_implementation_self_assessment_summary(
                &self.pool, run_id,
            )
            .await
            .ok()
            .flatten();
        let self_assessment_ref = self_assessment_opt.as_ref().map(|a| &a.summary);

        // P077 BLK-006: source controlled_reports_green from active artifact contracts.
        let controlled_reports_green =
            closeout::compute_controlled_reports_green(&self.pool, &run_id_str)
                .await
                .ok()
                .flatten();
        let accepted_risks = closeout::find_active_accepted_risks(&self.pool, &run_id_str)
            .await
            .unwrap_or_default();

        // P077 BLK-011: read the prior blocker_digest so the synthesizer can detect
        // soft-convergence (repeated identical blockers without diff or gate progress).
        let prior_blocker_digest = closeout::find_active_blocker_digest(&self.pool, &run_id_str)
            .await
            .ok()
            .flatten();
        let upstream_generation_ids =
            closeout::list_closeout_fingerprint_source_generation_ids(&self.pool, &run_id_str)
                .await
                .unwrap_or_default();
        let worktree_truth = resolve_closeout_worktree_truth(run).await;
        if let Some(reason) = worktree_truth.diagnostic_reason.as_deref() {
            warn!(
                run_id = %run_id,
                state = %current_state_id,
                reason,
                "P077: current worktree fingerprint truth unavailable; closeout will fail closed"
            );
        }
        let closeout_fingerprint = build_closeout_fingerprint(
            run,
            current_state_id,
            worktree_truth.worktree_head.clone(),
            worktree_truth.dirty_or_changed_file_digest.clone(),
            upstream_generation_ids,
            worktree_truth.latency_ms,
        );
        let fingerprint_latency_exceeded = worktree_truth.unavailable
            || worktree_truth.latency_exceeded
            || worktree_truth.latency_ms > CLOSEOUT_FINGERPRINT_LATENCY_BUDGET_MS;
        let loop_budget_remaining =
            closeout_loop_budget_remaining(plan, all_stages, "state_10_implementation_refined");

        let inputs = CloseoutSynthesizerInputs {
            run_id: &run_id_str,
            stage_id: current_state_id,
            gate_result: &gate_result,
            mode_result: &mode_result,
            self_assessment: self_assessment_ref,
            accepted_risks: &accepted_risks,
            loop_budget_remaining,
            fingerprint: Some(closeout_fingerprint),
            fingerprint_latency_exceeded,
            // Sourced from active audit/docs/security/prepush/tests artifact contracts.
            // None means at least one controlled report is missing — synthesizer fails
            // closed in enforcement mode while advisory mode is unaffected.
            controlled_reports_green,
            previous_blocker_digest: prior_blocker_digest.as_deref(),
        };

        let synth_result = synthesize_implementation_closeout_readiness_for_state9(inputs);

        let tx_inputs = closeout::CloseoutTransactionInputs {
            gate_result: &gate_result,
            readiness: &synth_result.readiness,
            accepted_risks: &accepted_risks,
            blocker_digest: synth_result.current_blocker_digest.as_deref(),
        };

        match closeout::execute_closeout_transaction_with_projection_rebuild(&self.pool, tx_inputs)
            .await
        {
            Ok(tx_result) => {
                info!(
                    run_id = %run_id,
                    state = %current_state_id,
                    decision = %tx_result.readiness_decision,
                    "P077: closeout readiness synthesized and persisted"
                );
            }
            Err(e) => {
                error!(
                    run_id = %run_id,
                    state = %current_state_id,
                    error = %e,
                    "P077: failed to persist closeout readiness — guard will not fire this cycle"
                );
            }
        }

        Ok(())
    }

    async fn block_implementation_run_start_if_rollout_contract_hold(
        &self,
        run_id: RunId,
        current_state_id: &str,
        stage: &StageExecution,
        task: &workflow::plan::CompiledTask,
        plan: &workflow::plan::RunPlan,
        run: &domain::run::Run,
    ) -> Result<bool> {
        let persisted_artifacts = artifacts::list_by_run(&self.pool, run_id).await?;
        let (approved_proposal_present, approved_proposal_artifact) = self
            .ensure_implementation_handoff_artifact(
                "approved_proposal",
                run,
                plan,
                stage,
                &persisted_artifacts,
            )
            .await?;

        let preflight =
            crate::rollout_contract_preflight::implementation_run_start_rollout_contract_preflight(
                &self.pool,
                run,
                approved_proposal_artifact.as_ref(),
            )
            .await?;
        if preflight.action
            != crate::rollout_contract_preflight::RolloutContractPreflightAction::Hold
        {
            return Ok(false);
        }

        let required_artifacts: Vec<String> = task
            .inputs
            .iter()
            .filter(|name| !is_inline_runtime_input(name))
            .cloned()
            .collect();
        let available_artifacts = approved_proposal_present
            .then(|| "approved_proposal".to_string())
            .into_iter()
            .collect::<Vec<_>>();
        let missing_artifacts = if required_artifacts
            .iter()
            .any(|artifact| artifact == "approved_proposal")
            && !approved_proposal_present
        {
            vec!["approved_proposal".to_string()]
        } else {
            Vec::new()
        };
        let now = Utc::now();
        let handoff_status = ImplementationHandoffStatus {
            schema_version: ImplementationHandoffStatus::SCHEMA_VERSION.to_string(),
            run_id: run_id.to_string(),
            current_state_id: current_state_id.to_string(),
            task_name: task.task_name.clone(),
            required_input_artifacts: required_artifacts,
            available_input_artifacts: available_artifacts,
            missing_input_artifacts: missing_artifacts.clone(),
            approved_proposal_present,
            approved_proposal_artifact_id: approved_proposal_artifact
                .as_ref()
                .map(|artifact| artifact.id.to_string()),
            approved_proposal_digest: approved_proposal_artifact
                .as_ref()
                .and_then(|artifact| artifact.checksum_sha256.clone()),
            worktree_root: run.worktree_root.clone(),
            workspace_root: run.workspace_root.clone(),
            artifact_root: run.artifact_root.clone(),
            code_writer_start_status: "blocked_before_code".to_string(),
            status: "blocked_before_code".to_string(),
            missing_handoff_outputs: missing_artifacts,
            last_handoff_agent_execution_id: None,
            retryable_from: Some(format!("rollout_contract_preflight:{current_state_id}")),
            blocked_before_code_reason: Some("rollout_contract_preflight_hold".to_string()),
            updated_at: now,
        };
        workflow_conflicts::upsert_implementation_handoff_status(&self.pool, &handoff_status)
            .await?;
        stages::update_status(&self.pool, stage.id, StageStatus::Blocked).await?;
        if run.status != RunStatus::Blocked {
            runs::update_status(&self.pool, run_id, RunStatus::Blocked).await?;
        }
        let _ = self.events.send(DomainEvent::StageStatusChanged {
            run_id,
            stage_execution_id: stage.id,
            status: StageStatus::Blocked,
        });
        let _ = self.events.send(DomainEvent::RunStatusChanged {
            run_id,
            status: RunStatus::Blocked,
        });
        warn!(
            run_id = %run_id,
            state = current_state_id,
            task = %task.task_name,
            rollout_contract_check_id = %preflight.check.id,
            failure_reasons = ?preflight.check.failure_reasons,
            "Rollout contract preflight held code_writer before enqueue"
        );
        Ok(true)
    }

    async fn block_code_writer_handoff_if_unavailable(
        &self,
        run_id: RunId,
        current_state_id: &str,
        stage: &StageExecution,
        task: &workflow::plan::CompiledTask,
        plan: &workflow::plan::RunPlan,
        run: &domain::run::Run,
    ) -> Result<bool> {
        if !is_code_writer_implementation_task(task) {
            return Ok(false);
        }

        let required_artifacts: Vec<String> = task
            .inputs
            .iter()
            .filter(|name| !is_inline_runtime_input(name))
            .cloned()
            .collect();
        let mut available_artifacts = Vec::new();
        let mut missing_artifacts = Vec::new();
        let mut persisted_artifacts = artifacts::list_by_run(&self.pool, run_id).await?;
        let mut approved_proposal_artifact_id = None;
        let mut approved_proposal_digest = None;

        for artifact_name in &required_artifacts {
            let (artifact_available, ensured_artifact) = self
                .ensure_implementation_handoff_artifact(
                    artifact_name,
                    run,
                    plan,
                    stage,
                    &persisted_artifacts,
                )
                .await?;
            if artifact_available {
                if let Some(artifact) = ensured_artifact {
                    if artifact.name == "approved_proposal" {
                        approved_proposal_artifact_id = Some(artifact.id.to_string());
                        approved_proposal_digest = artifact.checksum_sha256.clone();
                    }
                    if !persisted_artifacts
                        .iter()
                        .any(|existing| existing.id == artifact.id)
                    {
                        persisted_artifacts.push(artifact);
                    }
                }
                available_artifacts.push(artifact_name.clone());
            } else {
                missing_artifacts.push(artifact_name.clone());
            }
        }

        let now = Utc::now();
        let code_writer_start_status = if missing_artifacts.is_empty() {
            "not_queued"
        } else {
            "blocked_before_code"
        };
        let status = if required_artifacts.is_empty() {
            "not_required"
        } else if missing_artifacts.is_empty() {
            "ready"
        } else {
            "blocked_before_code"
        };
        let handoff_status = ImplementationHandoffStatus {
            schema_version: ImplementationHandoffStatus::SCHEMA_VERSION.to_string(),
            run_id: run_id.to_string(),
            current_state_id: current_state_id.to_string(),
            task_name: task.task_name.clone(),
            required_input_artifacts: required_artifacts.clone(),
            available_input_artifacts: available_artifacts.clone(),
            missing_input_artifacts: missing_artifacts.clone(),
            approved_proposal_present: available_artifacts
                .iter()
                .any(|artifact| artifact == "approved_proposal"),
            approved_proposal_artifact_id,
            approved_proposal_digest,
            worktree_root: run.worktree_root.clone(),
            workspace_root: run.workspace_root.clone(),
            artifact_root: run.artifact_root.clone(),
            code_writer_start_status: code_writer_start_status.to_string(),
            status: status.to_string(),
            missing_handoff_outputs: missing_artifacts.clone(),
            last_handoff_agent_execution_id: None,
            retryable_from: Some(format!("implementation_handoff:{current_state_id}")),
            blocked_before_code_reason: (!missing_artifacts.is_empty())
                .then(|| "implementation_handoff_unavailable".to_string()),
            updated_at: now,
        };
        workflow_conflicts::upsert_implementation_handoff_status(&self.pool, &handoff_status)
            .await?;

        if missing_artifacts.is_empty() {
            return Ok(false);
        }

        self.persist_implementation_handoff_unavailable_conflict(
            run_id,
            current_state_id,
            stage,
            &required_artifacts,
            &missing_artifacts,
            now,
        )
        .await?;
        stages::update_status(&self.pool, stage.id, StageStatus::Blocked).await?;
        if run.status != RunStatus::Blocked {
            runs::update_status(&self.pool, run_id, RunStatus::Blocked).await?;
        }
        let _ = self.events.send(DomainEvent::StageStatusChanged {
            run_id,
            stage_execution_id: stage.id,
            status: StageStatus::Blocked,
        });
        let _ = self.events.send(DomainEvent::RunStatusChanged {
            run_id,
            status: RunStatus::Blocked,
        });
        warn!(
            run_id = %run_id,
            state = current_state_id,
            task = %task.task_name,
            missing_artifacts = ?missing_artifacts,
            "Implementation handoff unavailable — code_writer not enqueued"
        );
        Ok(true)
    }

    async fn ensure_implementation_handoff_artifact(
        &self,
        artifact_name: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
        stage: &StageExecution,
        persisted_artifacts: &[Artifact],
    ) -> Result<(bool, Option<Artifact>)> {
        if let Some(artifact) = persisted_artifacts
            .iter()
            .rev()
            .find(|artifact| artifact.name == artifact_name)
        {
            return Ok((true, Some(artifact.clone())));
        }

        if artifact_name == "approved_proposal" {
            if let Some(artifact) = self
                .snapshot_approved_proposal_handoff_artifact(run, plan, stage, persisted_artifacts)
                .await?
            {
                return Ok((true, Some(artifact)));
            }
        }

        match artifact_contracts::active_contract_exists_result(&self.pool, run.id, artifact_name)
            .await
        {
            Ok(artifact_contracts::CanonicalContractField::Resolved(_)) => return Ok((true, None)),
            Ok(artifact_contracts::CanonicalContractField::MissingControlled { .. }) => {
                return Ok((false, None))
            }
            Ok(artifact_contracts::CanonicalContractField::UncontrolledAlias) => {}
            Err(error) => {
                if artifact_contracts::contract_id_for_alias(artifact_name).is_some() {
                    warn!(
                        run_id = %run.id,
                        artifact_name,
                        error = %error,
                        "Controlled implementation handoff artifact lookup failed"
                    );
                    return Ok((false, None));
                }
            }
        }

        let Some(path_template) = plan.artifact_paths.get(artifact_name) else {
            return Ok((false, None));
        };
        let resolved = resolve_path_template(
            path_template,
            &run.workspace_root,
            run.chainworks_meta_root.as_deref(),
        );
        if std::path::Path::new(&resolved).exists() {
            return Ok((true, None));
        }
        Ok((false, None))
    }

    async fn snapshot_approved_proposal_handoff_artifact(
        &self,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
        stage: &StageExecution,
        persisted_artifacts: &[Artifact],
    ) -> Result<Option<Artifact>> {
        let Some(path_template) = plan.artifact_paths.get("approved_proposal") else {
            return Ok(None);
        };
        let target_path = resolve_path_template(
            path_template,
            &run.workspace_root,
            run.chainworks_meta_root.as_deref(),
        );
        let target_path = std::path::PathBuf::from(target_path);
        if !target_path.exists() {
            let Some(source) = persisted_artifacts
                .iter()
                .rev()
                .find(|artifact| artifact.name == "proposal_current")
            else {
                return Ok(None);
            };
            let source_path = Self::absolute_artifact_path(&source.file_path, &run.workspace_root);
            let source_data = std::fs::read(&source_path).with_context(|| {
                format!(
                    "read proposal_current before approved_proposal snapshot {}",
                    source_path.display()
                )
            })?;
            let rollout_contract_failures =
                crate::rollout_contract_preflight::approved_proposal_rollout_contract_lint_failures(
                    &source_data,
                    &source_path,
                    &run.workspace_root,
                )?;
            if !rollout_contract_failures.is_empty() {
                warn!(
                    run_id = %run.id,
                    stage_id = %stage.stage_id,
                    failure_reasons = ?rollout_contract_failures,
                    "approved_proposal snapshot blocked by rollout_contract_v1 invariant"
                );
                return Ok(None);
            }
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "create approved_proposal handoff directory {}",
                        parent.display()
                    )
                })?;
            }
            std::fs::write(&target_path, &source_data).with_context(|| {
                format!(
                    "write approved_proposal handoff artifact {}",
                    target_path.display()
                )
            })?;
        }

        let data = std::fs::read(&target_path).with_context(|| {
            format!(
                "read engine-owned approved_proposal handoff artifact {}",
                target_path.display()
            )
        })?;
        let rollout_contract_failures =
            crate::rollout_contract_preflight::approved_proposal_rollout_contract_lint_failures(
                &data,
                &target_path,
                &run.workspace_root,
            )?;
        if !rollout_contract_failures.is_empty() {
            warn!(
                run_id = %run.id,
                stage_id = %stage.stage_id,
                failure_reasons = ?rollout_contract_failures,
                "approved_proposal artifact registration blocked by rollout_contract_v1 invariant"
            );
            return Ok(None);
        }
        let digest = Sha256::digest(&data);
        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id: run.id,
            stage_id: stage.stage_id.clone(),
            agent_id: "engine".to_string(),
            name: "approved_proposal".to_string(),
            contract_id: "approved_proposal".to_string(),
            format: ArtifactFormat::Markdown,
            file_path: Self::workspace_relative_artifact_path(&target_path, &run.workspace_root)
                .unwrap_or_else(|| target_path.to_string_lossy().into_owned()),
            checksum_sha256: Some(format!("{digest:x}")),
            size_bytes: Some(data.len() as i64),
            provider: "engine".to_string(),
            model: None,
            created_at: Utc::now(),
            is_pinned: true,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(&self.pool, &artifact).await?;
        Ok(Some(artifact))
    }

    fn absolute_artifact_path(file_path: &str, workspace_root: &str) -> std::path::PathBuf {
        let path = std::path::PathBuf::from(file_path);
        if path.is_absolute() {
            path
        } else {
            std::path::Path::new(workspace_root).join(path)
        }
    }

    fn workspace_relative_artifact_path(
        path: &std::path::Path,
        workspace_root: &str,
    ) -> Option<String> {
        if !path.is_absolute() {
            return Some(path.to_string_lossy().into_owned());
        }
        let workspace_root = std::path::Path::new(workspace_root);
        let relative = path.strip_prefix(workspace_root).ok()?;
        Some(relative.to_string_lossy().into_owned())
    }

    async fn persist_implementation_handoff_unavailable_conflict(
        &self,
        run_id: RunId,
        current_state_id: &str,
        stage: &StageExecution,
        required_artifacts: &[String],
        missing_artifacts: &[String],
        now: chrono::DateTime<Utc>,
    ) -> Result<()> {
        let candidate = CandidateTransitionEvaluation {
            transition_id: format!("{current_state_id}__implementation_handoff"),
            from_state_id: current_state_id.to_string(),
            to_state_id: current_state_id.to_string(),
            condition_expression_id: Some("implementation_handoff.required_inputs".to_string()),
            result: CandidateTransitionResult::MissingInput,
            required_artifacts: required_artifacts.to_vec(),
            missing_artifacts: missing_artifacts.to_vec(),
            missing_fields: Vec::new(),
            source_artifact_ids: Vec::new(),
            source_agent_execution_id: None,
            sanitized_diagnostic: Some(format!(
                "Implementation handoff is missing required input artifact(s): {}",
                missing_artifacts.join(", ")
            )),
        };
        let candidates = vec![candidate];
        let candidate_hash = candidate_transition_hash(&candidates);
        let reason = WorkflowConflictReason::ImplementationHandoffUnavailable;
        let fingerprint = workflow_conflict_fingerprint(
            &run_id.to_string(),
            current_state_id,
            &reason,
            &candidate_hash,
            &[],
        );
        let stage_execution_id = stage.id.to_string();
        let record = WorkflowConflictRecord {
            conflict_id: uuid::Uuid::new_v4().to_string(),
            conflict_fingerprint: fingerprint,
            run_id: run_id.to_string(),
            stage_execution_id: Some(stage_execution_id.clone()),
            lineage_id: Some(stage_execution_id),
            current_state_id: current_state_id.to_string(),
            operator_label: workflow_conflict_operator_label(&reason).to_string(),
            reason,
            status: WorkflowConflictStatus::Unresolved,
            candidate_transitions: candidates,
            candidate_transition_hash: candidate_hash,
            advisory_evidence_refs: Vec::new(),
            lead_agent_id: None,
            mediation_record_id: None,
            created_at: now,
            updated_at: now,
            resolved_at: None,
            superseded_by_conflict_id: None,
            resolution_record_json: None,
            terminal_failure_reason: None,
            diagnostic_redaction_tier: "operator_safe".to_string(),
        };
        let stored =
            workflow_conflicts::upsert_conflict_by_fingerprint(&self.pool, &record).await?;
        self.record_workflow_transition_cursor(WorkflowTransitionCursorRecord {
            schema_version: WorkflowTransitionCursorRecord::SCHEMA_VERSION.to_string(),
            run_id: run_id.to_string(),
            current_state_id: current_state_id.to_string(),
            cursor_status: "awaiting_conflict_resolution".to_string(),
            resume_policy: "await_conflict_resolution".to_string(),
            selected_transition_id: None,
            selected_next_state_id: None,
            conflict_id: Some(stored.conflict_id),
            conflict_fingerprint: Some(stored.conflict_fingerprint),
            candidate_transition_hash: Some(stored.candidate_transition_hash),
            terminal_failure_reason: None,
            updated_at: stored.updated_at,
        })
        .await?;
        Ok(())
    }

    async fn approval_rejection_context_for_state(
        &self,
        run_id: RunId,
        current_state_id: &str,
    ) -> Result<Option<String>> {
        if current_state_id != "state_5_proposal_refined" {
            return Ok(None);
        }

        let approvals = approvals::list_by_run(&self.pool, run_id).await?;
        let Some(rejection) = approvals
            .iter()
            .filter(|approval| {
                approval.stage_id == "state_6_implementation_approval"
                    && approval.decision == ApprovalDecision::Rejected
            })
            .filter(|approval| {
                approval
                    .comment
                    .as_deref()
                    .map(|comment| !comment.trim().is_empty())
                    .unwrap_or(false)
            })
            .max_by_key(|approval| approval.decided_at.unwrap_or(approval.requested_at))
        else {
            return Ok(None);
        };

        Ok(Some(format!(
            "### Rejected Implementation Approval Context\n\
             The previous state_6_implementation_approval gate was rejected. \
             Use the operator comment below together with the proposal and review \
             input artifacts when refining the proposal.\n\
             - stage_id: {}\n\
             - decided_at: {}\n\
             - comment: {}",
            rejection.stage_id,
            rejection
                .decided_at
                .unwrap_or(rejection.requested_at)
                .to_rfc3339(),
            rejection.comment.as_deref().unwrap_or_default().trim()
        )))
    }

    async fn workflow_conflict_resolution_context_for_proposal_writer(
        &self,
        run_id: RunId,
        stage: &StageExecution,
        task: &workflow::plan::CompiledTask,
    ) -> Result<Option<String>> {
        if task.agent.agent_id != "proposal_writer"
            || !task
                .outputs
                .iter()
                .any(|output| output == "proposal_current")
        {
            return Ok(None);
        }
        let Some(cursor) = workflow_conflicts::get_transition_cursor(&self.pool, run_id).await?
        else {
            return Ok(None);
        };
        if cursor.cursor_status != "operator_transition_selected"
            || cursor.resume_policy != "continue_from_selected_transition"
            || cursor.current_state_id != stage.stage_id
            || cursor.selected_next_state_id.as_deref() != Some(stage.stage_id.as_str())
        {
            return Ok(None);
        }
        let Some(conflict_id) = cursor.conflict_id.as_deref() else {
            return Ok(None);
        };
        let history = workflow_conflicts::list_conflict_history_for_run(&self.pool, run_id).await?;
        let Some(conflict) = history
            .iter()
            .rev()
            .find(|conflict| conflict.conflict_id == conflict_id)
        else {
            return Ok(None);
        };
        let resolution_reason = conflict
            .resolution_record_json
            .as_ref()
            .and_then(|record| record.get("resolution_reason"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim();
        if resolution_reason.is_empty() {
            return Ok(None);
        }

        Ok(Some(format!(
            "\n\n### Mandatory Workflow Conflict Resolution\n\
             A human operator explicitly selected one more proposal refinement after a workflow conflict. \
             This instruction is authoritative for this proposal_writer turn; do not return a stale proposal revision.\n\
             - conflict_id: `{}`\n\
             - candidate_transition_hash: `{}`\n\
             - selected_transition_id: `{}`\n\
             - selected_next_state_id: `{}`\n\
             - resolution_reason: `{}`\n\n\
             Required machine acknowledgement in `proposal_revision_summary`:\n\
             `workflow_conflict_resolution` must be an object with exactly this `conflict_id`, \
             `candidate_transition_hash`, `selected_transition_id`, `selected_next_state_id`, \
             the full `resolution_reason`, `applied: true`, and a non-empty `applied_changes` array. \
             If you cannot apply the instruction, say so in the required output instead of reusing old revision content.",
            conflict.conflict_id,
            cursor
                .candidate_transition_hash
                .as_deref()
                .unwrap_or(conflict.candidate_transition_hash.as_str()),
            cursor.selected_transition_id.as_deref().unwrap_or(""),
            cursor.selected_next_state_id.as_deref().unwrap_or(""),
            resolution_reason
        )))
    }

    /// Enqueue a single InvokeAgent work item for a task.
    async fn enqueue_invoke_agent(
        &self,
        run_id: RunId,
        stage: &StageExecution,
        task: &workflow::plan::CompiledTask,
        prompt: &str,
        task_index: usize,
        total_tasks: usize,
        plan: &workflow::plan::RunPlan,
        run: &domain::run::Run,
    ) -> Result<()> {
        let mut prompt = prompt.to_string();
        if let Some(context) = self
            .workflow_conflict_resolution_context_for_proposal_writer(run_id, stage, task)
            .await?
        {
            prompt.push_str(&context);
        }
        let declared_outputs = build_declared_outputs(task, plan, run);
        let mut agent = task.agent.clone();
        let provider_health_fallback = self
            .apply_run_local_provider_health_fallback(
                run_id,
                run,
                &mut agent,
                &task.outputs,
                task.agent.output_contract.as_deref(),
            )
            .await?;
        let work_item_id = format!("p058-invoke:{}:{}", stage.id, task_index);
        self.work_queue
            .enqueue_with_id(
                work_item_id,
                WorkItemKind::InvokeAgent,
                Some(run_id),
                Some(stage.stage_id.clone()),
                serde_json::json!({
                    "run_id": run_id.to_string(),
                    "stage_id": stage.stage_id,
                    "stage_execution_id": stage.id.to_string(),
                    "task_name": task.task_name,
                    "task_inputs": task.inputs,
                    "task_outputs": task.outputs,
                    "agent_id": agent.agent_id,
                    "backend_profile_id": agent.backend_profile_id,
                    "provider": agent.provider,
                    "model": agent.model,
                    "effort": agent.effort,
                    "max_turns": agent.max_turns,
                    "temperature": agent.temperature,
                    "permission_profile": agent.permission_profile,
                    "skill_ref": agent.skill_ref,
                    "skill_role": agent.skill_role,
                    "skill_snapshot_hash": agent.skill_snapshot_hash,
                    "requested_mcp_server_ids": agent.requested_mcp_server_ids,
                    "xcode_broker_required": agent.xcode_broker_required,
                    "xcode_shim_injection_signal": agent.xcode_shim_injection_signal,
                    "requires_xcode_host_execution": agent.requires_xcode_host_execution,
                    "output_contract": agent.output_contract,
                    "prompt": prompt,
                    "task_index": task_index,
                    "total_tasks": total_tasks,
                    "worktree_write_enabled": agent.worktree_write_enabled,
                    "worktree_strategy": effective_worktree_strategy_for_task(task),
                    "legacy_broad_discovery_policy": plan.legacy_broad_discovery_policy,
                    "session_reuse_scope": agent.session_reuse_scope,
                    "session_family_id": agent.session_family_id,
                    "declared_outputs": declared_outputs,
                    "provider_health_fallback": provider_health_fallback,
                    "stage_degraded_output_policy": plan
                        .states
                        .get(&stage.stage_id)
                        .map(|state| state.degraded_output_policy.clone())
                        .unwrap_or_default(),
                }),
            )
            .await?;
        if is_code_writer_implementation_task(task) {
            self.mark_code_writer_start_status_queued(run_id).await?;
        }
        Ok(())
    }

    async fn apply_run_local_provider_health_fallback(
        &self,
        run_id: RunId,
        run: &domain::run::Run,
        agent: &mut workflow::plan::ResolvedAgent,
        task_outputs: &[String],
        output_contract: Option<&str>,
    ) -> Result<Option<serde_json::Value>> {
        if !is_health_fallback_eligible_task(&agent.agent_id, task_outputs, output_contract)
            || !is_health_fallback_source_provider(&agent.provider)
        {
            return Ok(None);
        }
        let source_provider = agent.provider.clone();

        let executions = agent_executions::list_by_run(&self.pool, run_id).await?;
        let runtime_facts = agent_execution_runtime_facts::list_by_run(&self.pool, run_id).await?;
        let fact_by_execution: std::collections::HashMap<_, _> = runtime_facts
            .iter()
            .map(|facts| (facts.agent_execution_id, facts))
            .collect();
        let unhealthy_source = executions.iter().rev().find(|exec| {
            exec.agent_id == agent.agent_id
                && exec.status == AgentStatus::Failed
                && same_provider_family_for_health_fallback(&exec.provider, &source_provider)
                && fact_by_execution
                    .get(&exec.id)
                    .map(|facts| provider_health_fallback_failure(facts))
                    .unwrap_or(false)
        });
        let Some(source) = unhealthy_source else {
            return Ok(None);
        };

        let Some(catalog_json) = run.catalog_snapshot_json.as_deref() else {
            return Ok(None);
        };
        let catalog: serde_json::Value = serde_json::from_str(catalog_json)
            .context("parse catalog_snapshot_json for provider health fallback")?;
        let Some(profiles) = catalog
            .get("backend_profiles")
            .and_then(serde_json::Value::as_object)
        else {
            return Ok(None);
        };
        let Some((fallback_profile_id, profile)) = run_local_health_fallback_profile_candidates(
            &agent.agent_id,
            task_outputs,
            output_contract,
            &source_provider,
        )
        .into_iter()
        .find_map(|candidate| {
            profiles
                .get(candidate)
                .and_then(serde_json::Value::as_object)
                .map(|profile| (candidate, profile))
        }) else {
            return Ok(None);
        };
        let Some(provider) = profile.get("provider").and_then(serde_json::Value::as_str) else {
            return Ok(None);
        };
        if provider == agent.provider {
            return Ok(None);
        }

        let from_provider = agent.provider.clone();
        let from_backend_profile_id = agent.backend_profile_id.clone();
        agent.backend_profile_id = Some(fallback_profile_id.to_string());
        agent.provider = provider.to_string();
        agent.model = profile
            .get("model")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        agent.effort = profile
            .get("effort")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
        agent.max_turns = profile
            .get("max_turns")
            .and_then(serde_json::Value::as_u64)
            .and_then(|turns| u32::try_from(turns).ok());

        warn!(
            run_id = %run_id,
            agent_id = %agent.agent_id,
            failed_agent_execution_id = %source.id,
            from_provider = %from_provider,
            to_provider = %agent.provider,
            to_backend_profile_id = %fallback_profile_id,
            "Applying run-local provider health fallback after prior provider output failure"
        );

        Ok(Some(serde_json::json!({
            "reason": "prior_run_local_provider_output_failure",
            "failed_agent_execution_id": source.id.to_string(),
            "from_provider": from_provider,
            "to_provider": agent.provider,
            "from_backend_profile_id": from_backend_profile_id,
            "to_backend_profile_id": fallback_profile_id,
        })))
    }

    async fn schedule_auto_contract_output_retry_for_stage(
        &self,
        run_id: RunId,
        run: &domain::run::Run,
        stage: &StageExecution,
    ) -> Result<bool> {
        let executions = agent_executions::find_by_stage(&self.pool, stage.id).await?;
        let runtime_facts = agent_execution_runtime_facts::list_by_run(&self.pool, run_id).await?;
        let facts_by_execution: std::collections::HashMap<_, _> = runtime_facts
            .iter()
            .map(|facts| (facts.agent_execution_id, facts))
            .collect();
        let work_items_for_run = work_items::list_by_run(&self.pool, run_id).await?;
        let matching_stages = stages::list_by_run(&self.pool, run_id).await?;

        for execution in executions
            .iter()
            .filter(|execution| execution.status == AgentStatus::Failed)
        {
            let Some(facts) = facts_by_execution.get(&execution.id) else {
                continue;
            };
            if !provider_health_fallback_failure(facts) {
                continue;
            }

            let retry_reason = auto_contract_output_retry_reason(&execution.agent_id);
            if stage.retry_reason.as_deref() == Some(retry_reason.as_str())
                || matching_stages.iter().any(|candidate| {
                    candidate.stage_id == stage.stage_id
                        && candidate.iteration == stage.iteration
                        && candidate.retry_reason.as_deref() == Some(retry_reason.as_str())
                })
            {
                continue;
            }

            let stage_execution_id = stage.id.to_string();
            let agent_execution_id = execution.id.to_string();
            let Some(source_item) = work_items_for_run
                .iter()
                .filter(|item| item.kind == WorkItemKind::InvokeAgent)
                .filter(|item| {
                    matches!(
                        item.status,
                        WorkItemStatus::Completed | WorkItemStatus::Failed
                    )
                })
                .find(|item| {
                    work_item_matches_agent_execution(
                        item,
                        &stage_execution_id,
                        &execution.agent_id,
                        &agent_execution_id,
                    )
                })
            else {
                continue;
            };

            let mut retry_payload: serde_json::Value =
                match serde_json::from_str(&source_item.payload_json) {
                    Ok(payload) => payload,
                    Err(error) => {
                        warn!(
                            run_id = %run_id,
                            stage_id = %stage.stage_id,
                            agent_id = %execution.agent_id,
                            source_work_item_id = %source_item.id,
                            error = %error,
                            "Auto contract output retry skipped because source payload is invalid"
                        );
                        continue;
                    }
                };
            let source_provider = retry_payload
                .get("provider")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(execution.provider.as_str())
                .to_string();
            let task_outputs: Vec<String> = retry_payload
                .get("task_outputs")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let output_contract = retry_payload
                .get("output_contract")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned);
            if !is_health_fallback_eligible_task(
                &execution.agent_id,
                &task_outputs,
                output_contract.as_deref(),
            ) || !is_health_fallback_source_provider(&source_provider)
            {
                continue;
            }

            let Some(catalog_json) = run.catalog_snapshot_json.as_deref() else {
                continue;
            };
            let catalog: serde_json::Value = serde_json::from_str(catalog_json)
                .context("parse catalog_snapshot_json for auto contract output retry")?;
            let Some(profiles) = catalog
                .get("backend_profiles")
                .and_then(serde_json::Value::as_object)
            else {
                continue;
            };
            let Some((fallback_profile_id, profile)) =
                run_local_health_fallback_profile_candidates(
                    &execution.agent_id,
                    &task_outputs,
                    output_contract.as_deref(),
                    &source_provider,
                )
                .into_iter()
                .find_map(|candidate| {
                    profiles
                        .get(candidate)
                        .and_then(serde_json::Value::as_object)
                        .map(|profile| (candidate, profile))
                })
            else {
                continue;
            };
            let Some(fallback_provider) =
                profile.get("provider").and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            if same_provider_family_for_health_fallback(&source_provider, fallback_provider) {
                continue;
            }

            let next_attempt_number = matching_stages
                .iter()
                .filter(|candidate| candidate.stage_id == stage.stage_id)
                .map(|candidate| candidate.attempt_number)
                .max()
                .unwrap_or(stage.attempt_number)
                + 1;
            let now = Utc::now();
            let new_stage = StageExecution {
                id: domain::ids::StageExecutionId::new(),
                run_id,
                stage_id: stage.stage_id.clone(),
                label: stage.label.clone(),
                status: StageStatus::Running,
                iteration: stage.iteration,
                attempt_number: next_attempt_number,
                settlement_kind: None,
                started_at: now,
                completed_at: None,
                owner_agent: stage.owner_agent.clone(),
                provider: stage.provider.clone(),
                model: stage.model.clone(),
                stage_type: stage.stage_type.clone(),
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: Some(retry_reason.clone()),
            };
            let retry_work_item_id = format!(
                "auto-contract-output-retry:{}:{}",
                new_stage.id, execution.id
            );
            let from_backend_profile_id = retry_payload
                .get("backend_profile_id")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned);

            let Some(object) = retry_payload.as_object_mut() else {
                continue;
            };
            object.insert("run_id".into(), serde_json::json!(run_id.to_string()));
            object.insert("stage_id".into(), serde_json::json!(stage.stage_id));
            object.insert(
                "stage_execution_id".into(),
                serde_json::json!(new_stage.id.to_string()),
            );
            object.remove("p058_claimed");
            object.insert(
                "provider".into(),
                serde_json::json!(fallback_provider.to_string()),
            );
            object.insert(
                "backend_profile_id".into(),
                serde_json::json!(fallback_profile_id),
            );
            object.insert(
                "model".into(),
                profile
                    .get("model")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null),
            );
            if let Some(effort) = profile.get("effort").cloned() {
                object.insert("effort".into(), effort);
            }
            if let Some(max_turns) = profile.get("max_turns").cloned() {
                object.insert("max_turns".into(), max_turns);
            }
            object.insert(
                "targeted_retry".into(),
                serde_json::json!({
                    "source_stage_execution_id": stage.id.to_string(),
                    "source_agent_execution_id": execution.id.to_string(),
                    "source_work_item_id": source_item.id,
                    "reason": "auto_contract_output_retry",
                    "provider_fallback": {
                        "reason": "source_contract_outputs_missing",
                        "from_backend_profile_id": from_backend_profile_id,
                        "from_provider": source_provider.clone(),
                        "to_backend_profile_id": fallback_profile_id,
                        "to_provider": fallback_provider,
                    }
                }),
            );

            let tx_started = std::time::Instant::now();
            let mut tx = self
                .begin_orchestrator_transaction(
                    "orchestrator.AutoContractOutputRetry",
                    format!("orchestrator.AutoContractOutputRetry:{}", stage.id),
                )
                .await?;
            stages::settle_tx(&mut tx, stage.id, StageSettlementKind::Skipped, now).await?;
            stages::insert_tx(&mut tx, &new_stage).await?;
            sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
                .bind(RunStatus::Running.to_string())
                .bind(stage.stage_id.as_str())
                .bind(run_id.to_string())
                .execute(&mut *tx)
                .await?;
            work_items::enqueue_tx(
                &mut tx,
                &WorkItem {
                    id: retry_work_item_id,
                    kind: WorkItemKind::InvokeAgent,
                    payload_json: serde_json::to_string(&retry_payload)?,
                    status: WorkItemStatus::Pending,
                    run_id: Some(run_id),
                    stage_id: Some(stage.stage_id.clone()),
                    created_at: now,
                    scheduled_at: now,
                    attempt_count: 0,
                    last_error: None,
                },
            )
            .await?;
            tx.commit().await?;
            db::pool::log_write_transaction("orchestrator.AutoContractOutputRetry", tx_started);

            warn!(
                run_id = %run_id,
                stage_id = %stage.stage_id,
                source_stage_execution_id = %stage.id,
                retry_stage_execution_id = %new_stage.id,
                agent_id = %execution.agent_id,
                from_provider = %source_provider,
                to_provider = %fallback_provider,
                to_backend_profile_id = %fallback_profile_id,
                "Scheduled auto targeted retry after missing required outputs"
            );
            let _ = self.events.send(DomainEvent::StageStatusChanged {
                run_id,
                stage_execution_id: stage.id,
                status: StageStatus::Skipped,
            });
            let _ = self.events.send(DomainEvent::StageStatusChanged {
                run_id,
                stage_execution_id: new_stage.id,
                status: StageStatus::Running,
            });
            let _ = self.events.send(DomainEvent::RunStatusChanged {
                run_id,
                status: RunStatus::Running,
            });
            projections::rebuild_all_for_run(&self.pool, run_id).await?;
            return Ok(true);
        }

        Ok(false)
    }

    async fn mark_code_writer_start_status_queued(&self, run_id: RunId) -> Result<()> {
        let Some(mut status) =
            workflow_conflicts::get_implementation_handoff_status(&self.pool, run_id).await?
        else {
            return Ok(());
        };
        if status.status == "blocked_before_code" {
            return Ok(());
        }
        status.code_writer_start_status = "queued".to_string();
        status.updated_at = Utc::now();
        workflow_conflicts::upsert_implementation_handoff_status(&self.pool, &status).await
    }

    async fn enqueue_invoke_agent_for_owner(
        &self,
        run_id: RunId,
        stage: &StageExecution,
        agent: &workflow::plan::ResolvedAgent,
        prompt: &str,
        task_index: usize,
        total_tasks: usize,
    ) -> Result<()> {
        let work_item_id = format!("p058-invoke:{}:{}", stage.id, task_index);
        self.work_queue
            .enqueue_with_id(
                work_item_id,
                WorkItemKind::InvokeAgent,
                Some(run_id),
                Some(stage.stage_id.clone()),
                serde_json::json!({
                    "run_id": run_id.to_string(),
                    "stage_id": stage.stage_id,
                    "stage_execution_id": stage.id.to_string(),
                    "task_name": stage.stage_id,
                    "task_inputs": Vec::<String>::new(),
                    "task_outputs": Vec::<String>::new(),
                    "agent_id": agent.agent_id,
                    "backend_profile_id": agent.backend_profile_id,
                    "provider": agent.provider,
                    "model": agent.model,
                    "effort": agent.effort,
                    "max_turns": agent.max_turns,
                    "temperature": agent.temperature,
                    "permission_profile": agent.permission_profile,
                    "skill_ref": agent.skill_ref,
                    "skill_role": agent.skill_role,
                    "skill_snapshot_hash": agent.skill_snapshot_hash,
                    "requested_mcp_server_ids": agent.requested_mcp_server_ids,
                    "xcode_broker_required": agent.xcode_broker_required,
                    "xcode_shim_injection_signal": agent.xcode_shim_injection_signal,
                    "requires_xcode_host_execution": agent.requires_xcode_host_execution,
                    "output_contract": agent.output_contract,
                    "prompt": prompt,
                    "task_index": task_index,
                    "total_tasks": total_tasks,
                    "worktree_write_enabled": agent.worktree_write_enabled,
                    "worktree_strategy": agent.worktree_strategy,
                    "legacy_broad_discovery_policy": workflow::plan::LegacyBroadDiscoveryPolicy::Disabled,
                    "session_reuse_scope": agent.session_reuse_scope,
                    "session_family_id": agent.session_family_id,
                    "declared_outputs": Vec::<crate::contracts::DeclaredOutput>::new(),
                }),
            )
            .await
    }

    /// Resolve the base_branch from the first agent with a worktree_policy
    /// in the catalog. Falls back to None (which the provisioner treats as "main").
    fn resolve_base_branch_from_catalog(&self, run: &domain::run::Run) -> Option<String> {
        let catalog_path = run.agent_catalog_yaml_path.as_ref()?;
        let catalog = workflow::catalog::load(catalog_path).ok()?;
        let agents = catalog.agents.as_ref()?;
        for agent in agents {
            if let Some(ref wp) = agent.worktree_policy {
                if let Some(ref bb) = wp.base_branch {
                    // Resolve ${VAR:-default} patterns without path-normalizing branch names.
                    let resolved = resolve_scalar_template(bb);
                    return Some(resolved);
                }
            }
        }
        None
    }

    /// Clean up the worktree if one was provisioned for this run.
    /// Best-effort: logs warnings on failure, never propagates errors.
    async fn cleanup_worktree_if_needed(&self, run: &domain::run::Run) {
        if let Some(ref wt) = run.worktree_root {
            if let Err(e) =
                crate::worktree::WorktreeProvisioner::cleanup(wt, &run.workspace_root).await
            {
                warn!(
                    run_id = %run.id,
                    worktree = %wt,
                    error = %e,
                    "Worktree cleanup failed — manual removal may be needed"
                );
            }
        }
    }

    /// P060: Execute the deterministic proposal review routing algorithm.
    ///
    /// Runs in-process (no agent/provider invocation), persists a SystemExecution
    /// and RoutingReceipt, and writes an AgentSelectionPlanV1 artifact on success
    /// or sets stage validation failure JSON on failure.
    ///
    /// Returns `Ok(true)` if routing was executed (success or failure handled),
    /// `Ok(false)` if routing was not applicable (legacy mode or no bindings),
    /// and `Err` on infrastructure failure.
    async fn execute_system_routing_if_applicable(
        &self,
        run_id: RunId,
        run: &domain::run::Run,
        stage: &StageExecution,
        plan: &workflow::plan::RunPlan,
    ) -> Result<bool> {
        // Check if dynamic routing is requested.
        let routing_options: domain::routing::ReviewRoutingOptions = match &run.review_routing_json
        {
            Some(json) => match serde_json::from_str(json) {
                Ok(opts) => opts,
                Err(e) => {
                    warn!(run_id = %run_id, error = %e, "Invalid review_routing_json — using legacy_fixed routing mode");
                    domain::routing::ReviewRoutingOptions {
                        mode: domain::routing::ReviewRoutingMode::LegacyFixed,
                        ..Default::default()
                    }
                }
            },
            None => {
                warn!(
                    run_id = %run_id,
                    "Missing review_routing_json on dynamic routing workflow — using legacy_fixed routing mode"
                );
                domain::routing::ReviewRoutingOptions {
                    mode: domain::routing::ReviewRoutingMode::LegacyFixed,
                    ..Default::default()
                }
            }
        };

        // P060 Phase 3 / OPS-001: feature-flag cutover.
        // `domain::routing::resolve_effective_routing_mode` consults the
        // daemon-level `CHAINWORKS_P060_ROUTING_MODE_OVERRIDE` env var so
        // operators can force every run into a specific mode for staged
        // rollout / emergency rollback.
        let resolution = domain::routing::resolve_effective_routing_mode(&routing_options.mode);
        match &resolution {
            domain::routing::EffectiveRoutingModeResolution::OverriddenByEnv { from, to } => {
                info!(
                    run_id = %run_id,
                    run_mode = %from,
                    override_mode = %to,
                    "P060 cutover flag: overriding per-run routing mode"
                );
            }
            domain::routing::EffectiveRoutingModeResolution::OverrideUnrecognized {
                raw,
                per_run,
            } => {
                warn!(
                    run_id = %run_id,
                    raw = %raw,
                    per_run = %per_run,
                    "P060 cutover flag: unrecognized value, falling back to per-run mode"
                );
            }
            domain::routing::EffectiveRoutingModeResolution::UsedPerRunMode(_) => {}
        }
        let effective_mode = resolution.effective();

        let is_legacy = effective_mode == domain::routing::ReviewRoutingMode::LegacyFixed;

        // Shadow dynamic mode: run the algorithm, persist evidence, but
        // DO NOT take over dispatch. The legacy fixed quartet still
        // drives reviewer invocations. This lets operators A/B compare
        // the dynamic plan against actual fixed output before cutover.
        let is_shadow = effective_mode == domain::routing::ReviewRoutingMode::ShadowDynamic;

        // Check if the plan has dynamic candidate bindings.
        if plan.dynamic_candidate_bindings.is_empty() {
            info!(run_id = %run_id, "P060: No dynamic candidate bindings in plan — skipping routing");
            return Ok(false);
        }

        info!(
            run_id = %run_id,
            stage = %stage.stage_id,
            mode = %effective_mode,
            shadow = is_shadow,
            candidates = plan.dynamic_candidate_bindings.len(),
            "P060: Executing system routing for proposal review"
        );

        // Build proposal fingerprint from plan metadata.
        let fingerprint = self.build_proposal_fingerprint(run, plan).await;

        // Build input snapshot hashes.
        let routing_metadata_hash = {
            let mut hasher = Sha256::new();
            for b in &plan.dynamic_candidate_bindings {
                hasher.update(b.routing_metadata_hash.as_bytes());
            }
            format!("{:x}", hasher.finalize())
        };
        let candidate_binding_hash = {
            let mut hasher = Sha256::new();
            for b in &plan.dynamic_candidate_bindings {
                hasher.update(b.binding_id.as_bytes());
                hasher.update(b.agent_id.as_bytes());
            }
            format!("{:x}", hasher.finalize())
        };
        let evidence_hash = {
            let mut hasher = Sha256::new();
            for er in &fingerprint.evidence_refs {
                hasher.update(er.hash.as_bytes());
            }
            format!("{:x}", hasher.finalize())
        };
        let override_hash = if routing_options.force_include.is_empty()
            && routing_options.force_exclude.is_empty()
        {
            None
        } else {
            let mut hasher = Sha256::new();
            for inc in &routing_options.force_include {
                hasher.update(inc.as_bytes());
            }
            for exc in &routing_options.force_exclude {
                hasher.update(exc.as_bytes());
            }
            Some(format!("{:x}", hasher.finalize()))
        };

        let input_hashes = domain::routing::InputSnapshotHashes {
            workflow_snapshot_hash: plan.workflow_snapshot_hash.clone(),
            catalog_snapshot_hash: plan.catalog_snapshot_hash.clone(),
            routing_metadata_hash,
            candidate_binding_hash,
            evidence_hash,
            override_hash,
        };

        // Execute the routing algorithm, or synthesize a fixed-quartet plan for
        // rollback mode so dynamic workflows still dispatch reviewers.
        let outcome = if is_legacy {
            crate::proposal_review_router::build_legacy_fixed_routing_outcome(
                run_id,
                &stage.stage_id,
                stage.attempt_number,
                &plan.dynamic_candidate_bindings,
                &fingerprint,
                &input_hashes,
            )
        } else {
            crate::proposal_review_router::route_proposal_reviewers(
                run_id,
                &stage.stage_id,
                stage.attempt_number,
                &plan.dynamic_candidate_bindings,
                &fingerprint,
                &routing_options,
                &input_hashes,
            )
        };

        match outcome {
            crate::proposal_review_router::RoutingOutcome::Success {
                plan: selection_plan,
                receipt,
                system_execution,
            } => {
                info!(
                    run_id = %run_id,
                    selected = selection_plan.selected_agents.len(),
                    plan_hash = %selection_plan.plan_hash,
                    "P060: Routing succeeded"
                );

                self.persist_routing_success_artifact(
                    run_id,
                    run,
                    stage,
                    &selection_plan,
                    &receipt,
                    &system_execution,
                    if is_shadow {
                        "agent-selection-plan.shadow.v1.json"
                    } else {
                        "agent-selection-plan.v1.json"
                    },
                    if is_shadow {
                        "agent_selection_plan_shadow_v1"
                    } else {
                        "agent_selection_plan_v1"
                    },
                )
                .await?;

                // Emit routing completed event. The status label
                // distinguishes a real production routing run from a
                // shadow comparison so operators can filter dashboards.
                let event_status = if is_shadow {
                    "shadow_succeeded".to_string()
                } else {
                    "succeeded".to_string()
                };
                let _ = self.events.send(DomainEvent::RoutingCompleted {
                    run_id,
                    stage_id: stage.stage_id.clone(),
                    system_execution_id: system_execution.id,
                    receipt_id: receipt.receipt_id,
                    status: event_status,
                    plan_hash: Some(selection_plan.plan_hash.clone()),
                });

                if is_shadow {
                    let legacy_outcome =
                        crate::proposal_review_router::build_legacy_fixed_routing_outcome(
                            run_id,
                            &stage.stage_id,
                            stage.attempt_number,
                            &plan.dynamic_candidate_bindings,
                            &fingerprint,
                            &input_hashes,
                        );
                    let crate::proposal_review_router::RoutingOutcome::Success {
                        plan: legacy_plan,
                        receipt: legacy_receipt,
                        system_execution: legacy_system_execution,
                    } = legacy_outcome
                    else {
                        anyhow::bail!(
                            "P060: shadow_dynamic failed to synthesize legacy fixed dispatch plan"
                        );
                    };
                    self.persist_routing_success_artifact(
                        run_id,
                        run,
                        stage,
                        &legacy_plan,
                        &legacy_receipt,
                        &legacy_system_execution,
                        "agent-selection-plan.v1.json",
                        "agent_selection_plan_v1",
                    )
                    .await?;
                    info!(
                        run_id = %run_id,
                        dynamic_plan_hash = %selection_plan.plan_hash,
                        legacy_plan_hash = %legacy_plan.plan_hash,
                        "P060: Shadow routing recorded — dispatching legacy fixed quartet"
                    );
                    return Ok(true);
                }

                Ok(true)
            }
            crate::proposal_review_router::RoutingOutcome::Failure {
                failure_kind,
                receipt,
                system_execution,
                validation_failure_json,
            } => {
                warn!(
                    run_id = %run_id,
                    failure = %failure_kind,
                    shadow = is_shadow,
                    "P060: Routing failed"
                );

                // Persist system execution and routing receipt regardless
                // of mode — operators need the diagnostic in shadow runs too.
                db::repos::system_executions::insert(&self.pool, &system_execution).await?;
                db::repos::routing_receipts::insert(&self.pool, &receipt).await?;

                // P060 Phase 3: in shadow mode, a routing failure must NOT
                // block the run. Record the dynamic failure as observational
                // evidence, then materialize a legacy fixed plan so the
                // migrated dynamic workflow still dispatches reviewers.
                if is_shadow {
                    let _ = self.events.send(DomainEvent::RoutingCompleted {
                        run_id,
                        stage_id: stage.stage_id.clone(),
                        system_execution_id: system_execution.id,
                        receipt_id: receipt.receipt_id,
                        status: "shadow_failed".to_string(),
                        plan_hash: None,
                    });
                    let legacy_outcome =
                        crate::proposal_review_router::build_legacy_fixed_routing_outcome(
                            run_id,
                            &stage.stage_id,
                            stage.attempt_number,
                            &plan.dynamic_candidate_bindings,
                            &fingerprint,
                            &input_hashes,
                        );
                    let crate::proposal_review_router::RoutingOutcome::Success {
                        plan: legacy_plan,
                        receipt: legacy_receipt,
                        system_execution: legacy_system_execution,
                    } = legacy_outcome
                    else {
                        anyhow::bail!(
                            "P060: shadow_dynamic failure fallback failed to synthesize legacy fixed dispatch plan"
                        );
                    };
                    self.persist_routing_success_artifact(
                        run_id,
                        run,
                        stage,
                        &legacy_plan,
                        &legacy_receipt,
                        &legacy_system_execution,
                        "agent-selection-plan.v1.json",
                        "agent_selection_plan_v1",
                    )
                    .await?;
                    info!(
                        run_id = %run_id,
                        failure = %failure_kind,
                        legacy_plan_hash = %legacy_plan.plan_hash,
                        "P060: Shadow routing failure recorded — dispatching legacy fixed quartet"
                    );
                    return Ok(true);
                }

                // Production dynamic mode: the routing failure is a
                // hard stage-level conflict. Set validation failure JSON
                // and block the run.
                stages::update_validation_failure_json(
                    &self.pool,
                    stage.id,
                    &validation_failure_json,
                )
                .await?;

                let now = Utc::now();
                stages::settle(
                    &self.pool,
                    stage.id,
                    domain::stage::StageSettlementKind::Failed,
                    now,
                )
                .await?;
                let _ = self.events.send(DomainEvent::StageStatusChanged {
                    run_id,
                    stage_execution_id: stage.id,
                    status: StageStatus::Failed,
                });
                runs::update_status(&self.pool, run_id, RunStatus::Blocked).await?;
                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id,
                    status: RunStatus::Blocked,
                });

                Ok(true)
            }
        }
    }

    /// P060: Build a ProposalFingerprint from the run context and plan metadata.
    async fn build_proposal_fingerprint(
        &self,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> crate::proposal_review_router::ProposalFingerprint {
        // Try to read the proposal artifact to compute MD5 and extract evidence.
        let proposal_text = if let Some(template) = plan.artifact_paths.get("proposal_current") {
            let path = resolve_path_template(
                template,
                &run.workspace_root,
                run.chainworks_meta_root.as_deref(),
            );
            match std::fs::read(&path) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(_) => String::new(),
            }
        } else {
            String::new()
        };
        let proposal_md5 = if proposal_text.is_empty() {
            "unavailable".to_string()
        } else {
            format!("{:x}", Md5::digest(proposal_text.as_bytes()))
        };
        let proposal_lower = proposal_text.to_lowercase();

        // Extract stacks, surfaces, and risks from plan metadata.
        let mut stacks = plan
            .stack
            .as_ref()
            .map(|s| vec![s.clone()])
            .unwrap_or_default();
        if proposal_lower.contains(".rs")
            || proposal_lower.contains("rust")
            || proposal_lower.contains("cargo")
            || proposal_lower.contains("tokio")
            || proposal_lower.contains("control-plane")
        {
            stacks.push("rust-backend".to_string());
        }
        if proposal_lower.contains("swift")
            || proposal_lower.contains("swiftui")
            || proposal_lower.contains("appkit")
            || proposal_lower.contains("macos")
            || proposal_lower.contains("xcode")
        {
            stacks.push("apple-client".to_string());
            stacks.push("macos".to_string());
        }
        if proposal_lower.contains("graphql")
            || proposal_lower.contains("mcp")
            || proposal_lower.contains("yaml")
            || proposal_lower.contains("artifact contract")
        {
            stacks.push("shared-api".to_string());
        }
        stacks.sort();
        stacks.dedup();

        // Derive surfaces from workflow family and risk class.
        let mut surfaces = Vec::new();
        if let Some(ref family) = plan.workflow_family {
            // Common surface names derived from workflow family.
            if family.contains("macos") || family.contains("swift") {
                surfaces.push("macos".to_string());
            }
            if family.contains("swiftui") {
                surfaces.push("swiftui".to_string());
            }
        }
        for (needle, surface) in [
            ("graphql", "api-contract"),
            ("mcp", "api-contract"),
            ("schema", "api-contract"),
            ("yaml", "api-contract"),
            ("migration", "migration"),
            ("sqlite", "persistence"),
            ("database", "persistence"),
            ("queue", "background-work"),
            ("worker", "background-work"),
            ("retry", "background-work"),
            ("cancellation", "concurrency"),
            ("backpressure", "concurrency"),
            ("ui", "ui"),
            ("swiftui", "ui"),
            ("navigation", "navigation"),
            ("telemetry", "telemetry"),
            ("feature flag", "feature-flag"),
            ("rollout", "rollout"),
        ] {
            if proposal_lower.contains(needle) {
                surfaces.push(surface.to_string());
            }
        }
        surfaces.sort();
        surfaces.dedup();

        let mut risks = plan
            .risk_class
            .as_ref()
            .filter(|r| *r != "standard")
            .map(|r| vec![r.clone()])
            .unwrap_or_default();
        for (needle, risk) in [
            ("idempot", "idempotency"),
            ("data loss", "data-loss"),
            ("duplicate", "idempotency"),
            ("availability", "availability-sensitive"),
            ("security", "security-sensitive"),
            ("secret", "security-sensitive"),
            ("privacy", "privacy-sensitive"),
            ("backward", "backward-compatibility"),
            ("compatibility", "backward-compatibility"),
            ("operator", "operability-sensitive"),
            ("rollback", "operability-sensitive"),
            ("latency", "latency-sensitive"),
        ] {
            if proposal_lower.contains(needle) {
                risks.push(risk.to_string());
            }
        }
        risks.sort();
        risks.dedup();

        let strong_keywords = [
            "retry",
            "idempotency",
            "timeout",
            "deadline",
            "cancellation",
            "backpressure",
            "queue",
            "worker",
            "recovery",
            "resume",
            "shutdown",
            "graphql",
            "mcp",
            "yaml",
            "telemetry",
            "rollout",
            "security",
            "auth",
            "swiftui",
            "macos",
            "rust-backend",
        ]
        .into_iter()
        .filter(|keyword| proposal_lower.contains(keyword))
        .map(str::to_string)
        .collect::<Vec<_>>();

        let mut repo_signals = Vec::new();
        if proposal_lower.contains("control-plane/") || proposal_lower.contains("crates/") {
            repo_signals.push("rust-backend".to_string());
        }
        if proposal_lower.contains("graphql-server") || proposal_lower.contains("mcp-server") {
            repo_signals.push("api-contract".to_string());
        }
        if proposal_lower.contains("chainworks forge/") {
            repo_signals.push("macos".to_string());
            repo_signals.push("ui".to_string());
        }
        repo_signals.sort();
        repo_signals.dedup();

        let mut evidence_refs = Vec::new();
        for tag in stacks
            .iter()
            .chain(surfaces.iter())
            .chain(risks.iter())
            .chain(strong_keywords.iter())
        {
            evidence_refs.push(domain::routing::RoutingEvidenceRef {
                evidence_id: format!("proposal:{}", tag),
                evidence_type: "proposal_text_match".to_string(),
                hash: format!("{:x}", Sha256::digest(tag.as_bytes())),
                normalized_value: Some(tag.clone()),
                path: Some("proposal_current".to_string()),
                symbol: None,
                span: None,
            });
        }

        crate::proposal_review_router::ProposalFingerprint {
            proposal_md5,
            stacks,
            surfaces,
            risks,
            strong_keywords,
            repo_signals,
            cross_stack_dependencies: Vec::new(),
            baseline_gaps: Vec::new(),
            evidence_refs,
        }
    }

    async fn persist_routing_success_artifact(
        &self,
        run_id: RunId,
        run: &domain::run::Run,
        stage: &StageExecution,
        selection_plan: &domain::routing::AgentSelectionPlanV1,
        receipt: &domain::routing::RoutingReceipt,
        system_execution: &domain::routing::SystemExecution,
        artifact_file_name: &str,
        artifact_name: &str,
    ) -> Result<()> {
        db::repos::system_executions::insert(&self.pool, system_execution).await?;
        db::repos::routing_receipts::insert(&self.pool, receipt).await?;

        let artifact_path = format!(
            "{}/routing/{artifact_file_name}",
            run.artifact_root.trim_end_matches('/')
        );
        if let Some(parent) = std::path::Path::new(&artifact_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let plan_json = serde_json::to_vec_pretty(selection_plan)?;
        std::fs::write(&artifact_path, &plan_json)?;

        let artifact = domain::artifact::Artifact {
            id: domain::ids::ArtifactId::new(),
            run_id,
            stage_id: stage.stage_id.clone(),
            agent_id: "proposal_review_router".to_string(),
            name: artifact_name.to_string(),
            contract_id: "agent_selection_plan_v1".to_string(),
            format: domain::artifact::ArtifactFormat::Json,
            file_path: artifact_path,
            checksum_sha256: None,
            size_bytes: Some(plan_json.len() as i64),
            provider: "system.routing".to_string(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(&self.pool, &artifact).await?;

        Ok(())
    }

    /// P060: Materialize dynamic_parallel reviewers from an AgentSelectionPlanV1
    /// artifact. For each selected agent, looks up the corresponding
    /// CompiledDynamicAgentBinding, deserializes the frozen ResolvedAgent, and
    /// enqueues an InvokeAgent work item. Uses DynamicMaterializationRecord for
    /// idempotency — duplicate resume/retry cannot create duplicate reviewer
    /// executions.
    ///
    /// Returns the number of dynamic tasks enqueued (0 if all were already
    /// materialized from a prior attempt).
    async fn materialize_dynamic_parallel(
        &self,
        run_id: RunId,
        run: &domain::run::Run,
        stage: &StageExecution,
        plan: &workflow::plan::RunPlan,
        dynamic_parallel: &workflow::plan::CompiledDynamicParallel,
        idea_opt: Option<&domain::idea::Idea>,
    ) -> Result<usize> {
        // Read the AgentSelectionPlanV1 artifact from disk.
        let artifact_path = format!(
            "{}/routing/agent-selection-plan.v1.json",
            run.artifact_root.trim_end_matches('/')
        );
        let plan_bytes = std::fs::read(&artifact_path).context(
            "P060: Failed to read AgentSelectionPlanV1 artifact for dynamic_parallel materialization",
        )?;
        let selection_plan: domain::routing::AgentSelectionPlanV1 =
            serde_json::from_slice(&plan_bytes)
                .context("P060: Failed to parse AgentSelectionPlanV1 artifact")?;

        // Build a lookup from agent_id → CompiledDynamicAgentBinding.
        let binding_map: std::collections::HashMap<
            &str,
            &domain::routing::CompiledDynamicAgentBinding,
        > = plan
            .dynamic_candidate_bindings
            .iter()
            .map(|b| (b.agent_id.as_str(), b))
            .collect();

        let total_selected = selection_plan.selected_agents.len();
        // Total tasks = dynamic reviewers + then-block tasks from state.tasks.
        let then_task_count = plan
            .states
            .get(&stage.stage_id)
            .map(|s| s.tasks.iter().filter(|t| t.phase > 0).count())
            .unwrap_or(0);
        let total_tasks = total_selected + then_task_count;
        let mut enqueued = 0;

        for (idx, selected_agent) in selection_plan.selected_agents.iter().enumerate() {
            let binding = match binding_map.get(selected_agent.agent_id.as_str()) {
                Some(b) => *b,
                None => {
                    warn!(
                        run_id = %run_id,
                        agent_id = %selected_agent.agent_id,
                        "P060: Selected agent not found in dynamic candidate bindings — skipping"
                    );
                    continue;
                }
            };

            // Check idempotency — skip if already materialized.
            let materialization_epoch = dynamic_materialization_epoch(stage);
            if db::repos::dynamic_materialization::is_materialized(
                &self.pool,
                run_id,
                &stage.stage_id,
                materialization_epoch,
                &selection_plan.plan_hash,
                &binding.binding_id,
            )
            .await?
            {
                info!(
                    run_id = %run_id,
                    agent_id = %selected_agent.agent_id,
                    binding_id = %binding.binding_id,
                    "P060: Already materialized — skipping (idempotent)"
                );
                continue;
            }

            // Deserialize the frozen ResolvedAgent from the binding.
            let resolved_agent: workflow::plan::ResolvedAgent =
                serde_json::from_str(&binding.resolved_agent_snapshot_json).context(format!(
                    "P060: Failed to deserialize ResolvedAgent for {}",
                    selected_agent.agent_id
                ))?;

            // Build a CompiledTask for this dynamic reviewer.
            let output_name = p060_dynamic_review_output_name(&selected_agent.agent_id);
            let mut output_schemas = std::collections::HashMap::new();
            output_schemas.insert(
                output_name.clone(),
                p060_dynamic_review_output_schema(&dynamic_parallel.output_contract),
            );
            let mut task = workflow::plan::CompiledTask {
                agent: resolved_agent,
                task_name: format!("dynamic_review_{}", selected_agent.agent_id),
                inputs: dynamic_parallel.inputs.clone(),
                outputs: vec![output_name],
                output_policies: std::collections::HashMap::new(),
                output_schemas,
                parallel: true,
                phase: 0,
                selected_outputs_from: None,
            };
            let output_contract = task.agent.output_contract.clone();
            let provider_health_fallback = self
                .apply_run_local_provider_health_fallback(
                    run_id,
                    run,
                    &mut task.agent,
                    &task.outputs,
                    output_contract.as_deref(),
                )
                .await?;

            // Build prompt for this reviewer.
            let prompt = build_task_prompt(&task, plan, run, idea_opt, None, None);

            // Record materialization for idempotency.
            let work_item_id = format!(
                "p060-dynamic:{}:{}:{}",
                stage.id, selection_plan.plan_hash, idx
            );
            let mat_record = domain::routing::DynamicMaterializationRecord {
                id: domain::ids::DynamicMaterializationId::new(),
                run_id,
                stage_id: stage.stage_id.clone(),
                attempt_id: materialization_epoch,
                phase_id: "dynamic_parallel".into(),
                plan_hash: selection_plan.plan_hash.clone(),
                binding_id: binding.binding_id.clone(),
                agent_execution_id: work_item_id.clone(),
                idempotency_key: format!(
                    "{}:{}:{}:{}:dynamic_parallel:{}:{}",
                    run_id,
                    stage.stage_id,
                    materialization_epoch,
                    selection_plan.plan_hash,
                    binding.binding_id,
                    idx,
                ),
                created_at: Utc::now(),
            };
            db::repos::dynamic_materialization::insert_idempotent(&self.pool, &mat_record).await?;

            info!(
                run_id = %run_id,
                agent_id = %selected_agent.agent_id,
                provider = %task.agent.provider,
                score = selected_agent.score,
                mandatory = selected_agent.mandatory,
                index = idx,
                total = total_selected,
                "P060: Materializing dynamic reviewer (phase 0)"
            );

            // Enqueue via the standard InvokeAgent path, with a p060_dynamic_phase
            // marker in the payload so phase advancement logic recognizes these.
            let declared_outputs = build_declared_outputs(&task, plan, run);
            self.work_queue
                .enqueue_with_id(
                    work_item_id,
                    WorkItemKind::InvokeAgent,
                    Some(run_id),
                    Some(stage.stage_id.clone()),
                    serde_json::json!({
                        "run_id": run_id.to_string(),
                        "stage_id": stage.stage_id,
                        "stage_execution_id": stage.id.to_string(),
                        "task_name": task.task_name,
                        "task_inputs": task.inputs,
                        "task_outputs": task.outputs,
                        "agent_id": task.agent.agent_id,
                        "backend_profile_id": task.agent.backend_profile_id,
                        "provider": task.agent.provider,
                        "model": task.agent.model,
                        "effort": task.agent.effort,
                        "max_turns": task.agent.max_turns,
                        "temperature": task.agent.temperature,
                        "permission_profile": task.agent.permission_profile,
                        "skill_ref": task.agent.skill_ref,
                        "skill_role": task.agent.skill_role,
                        "skill_snapshot_hash": task.agent.skill_snapshot_hash,
                        "requested_mcp_server_ids": task.agent.requested_mcp_server_ids,
                        "xcode_broker_required": task.agent.xcode_broker_required,
                        "xcode_shim_injection_signal": task.agent.xcode_shim_injection_signal,
                        "requires_xcode_host_execution": task.agent.requires_xcode_host_execution,
                        "output_contract": task.agent.output_contract,
                        "prompt": prompt,
                        "task_index": idx,
                        "total_tasks": total_tasks,
                        "worktree_write_enabled": task.agent.worktree_write_enabled,
                        "worktree_strategy": task.agent.worktree_strategy,
                        "legacy_broad_discovery_policy": plan.legacy_broad_discovery_policy,
                        "session_reuse_scope": task.agent.session_reuse_scope,
                        "session_family_id": task.agent.session_family_id,
                        "declared_outputs": declared_outputs,
                        "provider_health_fallback": provider_health_fallback,
                        "p060_dynamic_phase": 0,
                        "p060_dispatch_mode": selection_plan.mode.to_string(),
                        "p060_plan_hash": selection_plan.plan_hash,
                        "p060_binding_id": binding.binding_id,
                    }),
                )
                .await?;

            enqueued += 1;
        }

        info!(
            run_id = %run_id,
            stage = %stage.stage_id,
            enqueued = enqueued,
            total_selected = total_selected,
            "P060: Dynamic parallel materialization complete"
        );

        Ok(enqueued)
    }

    /// P060: Resolve selected reviewer outputs for a then-task that declares
    /// `selected_outputs_from`. Reads the AgentSelectionPlanV1 artifact,
    /// queries stage artifacts, filters to selected reviewers, and returns
    /// a `ReviewCorpusBundleV2` and the resolved artifact file paths.
    ///
    /// Returns `None` if the task doesn't declare `selected_outputs_from`
    /// or the selection plan artifact doesn't exist.
    async fn resolve_selected_outputs_for_task(
        &self,
        run_id: RunId,
        run: &domain::run::Run,
        stage: &StageExecution,
        task: &workflow::plan::CompiledTask,
    ) -> Result<Option<(domain::routing::ReviewCorpusBundleV2, Vec<String>)>> {
        let sof = match &task.selected_outputs_from {
            Some(sof) => sof,
            None => return Ok(None),
        };

        // Read the AgentSelectionPlanV1 artifact from disk.
        let artifact_path = format!(
            "{}/routing/agent-selection-plan.v1.json",
            run.artifact_root.trim_end_matches('/')
        );
        let plan_bytes = match std::fs::read(&artifact_path) {
            Ok(b) => b,
            Err(_) => {
                warn!(
                    run_id = %run_id,
                    "P060: AgentSelectionPlanV1 artifact not found — cannot resolve selected_outputs_from"
                );
                return Ok(None);
            }
        };
        let selection_plan: domain::routing::AgentSelectionPlanV1 =
            serde_json::from_slice(&plan_bytes)
                .context("P060: Failed to parse AgentSelectionPlanV1 for selected_outputs_from")?;

        // Query stage artifacts from the DB.
        let stage_artifacts = artifacts::list_by_stage(&self.pool, run_id, &stage.stage_id).await?;

        // Convert to AvailableArtifact for the resolver.
        let available: Vec<crate::proposal_review_router::AvailableArtifact> = stage_artifacts
            .iter()
            .map(|a| crate::proposal_review_router::AvailableArtifact {
                artifact_id: a.id.to_string(),
                agent_id: a.agent_id.clone(),
                contract_id: a.contract_id.clone(),
                file_path: a.file_path.clone(),
            })
            .collect();

        let result = crate::proposal_review_router::resolve_selected_outputs(
            &selection_plan,
            &available,
            &sof.output_contract,
        );

        let file_paths: Vec<String> = result
            .selected_review_artifacts
            .iter()
            .map(|a| a.file_path.clone())
            .collect();

        let bundle = domain::routing::ReviewCorpusBundleV2 {
            selected_review_artifacts: result
                .selected_review_artifacts
                .iter()
                .map(|a| a.artifact_id.clone())
                .collect(),
            selected_reviewer_ids: result.selected_reviewer_ids,
            reviewer_count: result.reviewer_count,
            selection_plan_hash: result.selection_plan_hash,
            selection_plan: selection_plan.clone(),
            legacy_fixed_mode: result.legacy_fixed_mode,
        };

        info!(
            run_id = %run_id,
            stage = %stage.stage_id,
            selected_count = bundle.reviewer_count,
            plan_hash = %bundle.selection_plan_hash,
            ignored = result.ignored_artifacts.len(),
            "P060: Resolved selected_outputs_from for aggregation"
        );

        // Write the ReviewCorpusBundleV2 as a metadata artifact.
        let bundle_path = format!(
            "{}/routing/review-corpus-bundle.v2.json",
            run.artifact_root.trim_end_matches('/')
        );
        if let Some(parent) = std::path::Path::new(&bundle_path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bundle_json = serde_json::to_vec_pretty(&bundle)?;
        std::fs::write(&bundle_path, &bundle_json)?;

        let bundle_artifact = domain::artifact::Artifact {
            id: domain::ids::ArtifactId::new(),
            run_id,
            stage_id: stage.stage_id.clone(),
            agent_id: "system.aggregation".to_string(),
            name: "review_corpus_bundle_v2".to_string(),
            contract_id: "review_corpus_bundle_v2".to_string(),
            format: domain::artifact::ArtifactFormat::Json,
            file_path: bundle_path,
            checksum_sha256: None,
            size_bytes: Some(bundle_json.len() as i64),
            provider: "system".to_string(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(&self.pool, &bundle_artifact).await?;

        Ok(Some((bundle, file_paths)))
    }

    /// Create a StageExecution for the given state.
    async fn create_stage_for_state(
        &self,
        run_id: RunId,
        state_id: &str,
        state: &workflow::plan::CompiledState,
    ) -> Result<StageExecution> {
        let now = Utc::now();
        db::repos::artifact_contracts::expire_overrides_for_stage(&self.pool, run_id, state_id)
            .await?;

        // Determine iteration: count how many stages for this state_id already exist.
        let all_stages = stages::list_by_run(&self.pool, run_id).await?;
        let iteration = all_stages.iter().filter(|s| s.stage_id == state_id).count() as i64 + 1;

        let stage = StageExecution {
            id: domain::ids::StageExecutionId::new(),
            run_id,
            stage_id: state_id.to_string(),
            label: state.label.clone(),
            status: StageStatus::Pending,
            iteration,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: None,
            owner_agent: Some(state.owner.agent_id.clone()),
            provider: Some(state.owner.provider.clone()),
            model: state.owner.model.clone(),
            stage_type: state.state_type.clone(),
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        stages::insert(&self.pool, &stage).await?;
        Ok(stage)
    }

    /// Evaluate transition conditions and advance to the next state.
    async fn evaluate_and_transition(
        &self,
        run_id: RunId,
        current_state_id: &str,
        plan: &workflow::plan::RunPlan,
        all_stages: &[StageExecution],
    ) -> Result<()> {
        let state = match plan.states.get(current_state_id) {
            Some(s) => s,
            None => return Ok(()),
        };

        // Fetch run for condition evaluation context.
        let run = runs::find_by_id(&self.pool, run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        if state.is_end {
            let now = Utc::now();
            self.mark_run_completed_and_refresh(run_id, now).await?;
            self.enqueue_steward_analysis(Some(run_id)).await?;
            let _ = self.events.send(DomainEvent::RunStatusChanged {
                run_id,
                status: RunStatus::Completed,
            });
            return Ok(());
        }

        // Evaluate transitions against compiled graph truth. P017 keeps the
        // existing transition action path, but records a typed candidate result
        // before choosing a graph-authoritative transition.
        let mut candidate_evaluations = Vec::new();
        for (transition_index, transition) in state.transitions.iter().enumerate() {
            let mut evaluation = self
                .evaluate_transition_candidate(
                    transition_index,
                    current_state_id,
                    transition,
                    &run,
                    plan,
                )
                .await;
            if evaluation.result == CandidateTransitionResult::Matched {
                if let Some(exhaustion) =
                    closeout_loop_budget_exhaustion(plan, all_stages, &transition.to)
                {
                    debug!(
                        run_id = %run_id,
                        from = current_state_id,
                        to = %transition.to,
                        counter = %exhaustion.counter,
                        iterations = exhaustion.iterations,
                        max = exhaustion.max,
                        "Loop budget exhausted — blocking transition into loop state"
                    );
                    evaluation.result = CandidateTransitionResult::NotMatched;
                    evaluation.sanitized_diagnostic = Some(format!(
                        "Loop budget exhausted for {}: {}/{} iterations",
                        exhaustion.counter, exhaustion.iterations, exhaustion.max
                    ));
                }
            }
            candidate_evaluations.push(evaluation);
        }
        self.apply_implementation_review_refinement_guard(
            run_id,
            current_state_id,
            state,
            plan,
            all_stages,
            &mut candidate_evaluations,
        )
        .await?;

        if let Some(aggregate_diagnostic) = self
            .proposal_review_summary_transition_truth_conflict(&candidate_evaluations, &run, plan)
            .await?
        {
            warn!(
                run_id = %run_id,
                state = current_state_id,
                diagnostic = %aggregate_diagnostic,
                "Aggregate transition truth conflicted — run blocked"
            );
            annotate_aggregate_transition_truth_conflict(
                &mut candidate_evaluations,
                &aggregate_diagnostic,
            );
            self.record_workflow_conflict_and_block(
                run_id,
                current_state_id,
                all_stages,
                candidate_evaluations,
                plan,
                &run,
                Some(WorkflowConflictReason::AggregateTransitionTruthConflicted),
            )
            .await?;
            return Ok(());
        }

        let matched_transition_indexes: Vec<usize> = candidate_evaluations
            .iter()
            .enumerate()
            .filter_map(|(index, evaluation)| {
                (evaluation.result == CandidateTransitionResult::Matched).then_some(index)
            })
            .collect();

        if matched_transition_indexes.len() > 1 {
            warn!(
                run_id = %run_id,
                state = current_state_id,
                matched_transition_count = matched_transition_indexes.len(),
                candidate_transitions = ?candidate_evaluations,
                "Multiple declarative transitions matched without tie-break — run blocked"
            );
            self.record_workflow_conflict_and_block(
                run_id,
                current_state_id,
                all_stages,
                candidate_evaluations,
                plan,
                &run,
                None,
            )
            .await?;
            return Ok(());
        }

        if let Some(transition_index) = matched_transition_indexes.first().copied() {
            let transition = &state.transitions[transition_index];
            let selected_transition_id =
                transition_id_for(current_state_id, &transition.to, transition_index);
            info!(
                run_id = %run_id,
                from = current_state_id,
                to = %transition.to,
                condition = %transition.condition,
                "Transition matched"
            );
            self.record_advisory_rejections_for_selected_transition(
                run_id,
                current_state_id,
                &selected_transition_id,
                &transition.to,
                plan,
                all_stages,
                &run,
            )
            .await?;
            self.resolve_current_workflow_conflict_for_selected_transition(
                run_id,
                current_state_id,
                &selected_transition_id,
                &transition.to,
            )
            .await?;
            self.record_workflow_transition_cursor(WorkflowTransitionCursorRecord {
                schema_version: WorkflowTransitionCursorRecord::SCHEMA_VERSION.to_string(),
                run_id: run_id.to_string(),
                current_state_id: current_state_id.to_string(),
                cursor_status: "graph_transition_selected".to_string(),
                resume_policy: "continue_from_selected_transition".to_string(),
                selected_transition_id: Some(selected_transition_id.clone()),
                selected_next_state_id: Some(transition.to.clone()),
                conflict_id: None,
                conflict_fingerprint: None,
                candidate_transition_hash: Some(candidate_transition_hash(&candidate_evaluations)),
                terminal_failure_reason: None,
                updated_at: Utc::now(),
            })
            .await?;
            if transition.to == current_state_id {
                let target_state = plan.states.get(&transition.to).ok_or_else(|| {
                    anyhow::anyhow!("Transition target state not found: {}", transition.to)
                })?;
                let next_stage = self
                    .create_stage_for_state(run_id, &transition.to, target_state)
                    .await?;
                info!(
                    run_id = %run_id,
                    state = %transition.to,
                    stage_execution_id = %next_stage.id,
                    iteration = next_stage.iteration,
                    "Self-loop transition created next pending stage iteration"
                );
            }
            runs::update_current_state(&self.pool, run_id, &transition.to).await?;
            db::repos::artifact_contracts::expire_overrides_for_stage(
                &self.pool,
                run_id,
                &transition.to,
            )
            .await?;
            db::repos::projections::rebuild_all_for_run(&self.pool, run_id).await?;

            // Re-enter advance_run for the new state
            self.work_queue
                .enqueue(
                    WorkItemKind::AdvanceRun,
                    Some(run_id),
                    None,
                    serde_json::json!({
                        "run_id": run_id.to_string(),
                        "reason": "state_transition",
                        "from": current_state_id,
                        "to": transition.to,
                    }),
                )
                .await?;
            return Ok(());
        }

        // No transition matched — check if run should complete or block.
        if state.transitions.is_empty() || state.is_end {
            let now = Utc::now();
            self.mark_run_completed_and_refresh(run_id, now).await?;
            self.enqueue_steward_analysis(Some(run_id)).await?;
            let _ = self.events.send(DomainEvent::RunStatusChanged {
                run_id,
                status: RunStatus::Completed,
            });
        } else {
            info!(
                run_id = %run_id,
                state = current_state_id,
                candidate_transitions = ?candidate_evaluations,
                "No transition matched — run blocked"
            );
            self.record_workflow_conflict_and_block(
                run_id,
                current_state_id,
                all_stages,
                candidate_evaluations,
                plan,
                &run,
                None,
            )
            .await?;
        }

        Ok(())
    }

    async fn apply_implementation_review_refinement_guard(
        &self,
        run_id: RunId,
        current_state_id: &str,
        state: &workflow::plan::CompiledState,
        plan: &workflow::plan::RunPlan,
        all_stages: &[StageExecution],
        candidate_evaluations: &mut [CandidateTransitionEvaluation],
    ) -> Result<()> {
        if current_state_id != "state_9_implementation_reviewed" {
            return Ok(());
        }

        let Some(release_index) = state
            .transitions
            .iter()
            .position(|transition| transition.to == "state_11_manual_release")
        else {
            return Ok(());
        };
        if candidate_evaluations
            .get(release_index)
            .map(|candidate| candidate.result.clone())
            != Some(CandidateTransitionResult::Matched)
        {
            return Ok(());
        }

        let target_status = plan
            .variables
            .get("implementation_review_target_status")
            .and_then(|value| value.as_str())
            .unwrap_or("code_complete");
        let status = db::repos::artifact_contracts::canonical_contract_field_result(
            &self.pool,
            run_id,
            "implementation_review_summary",
            "status",
        )
        .await?;
        let status_label = match &status {
            db::repos::artifact_contracts::CanonicalContractField::Resolved(value) => value
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| value.to_string()),
            db::repos::artifact_contracts::CanonicalContractField::MissingControlled {
                contract_id,
            } => format!("missing:{contract_id}"),
            db::repos::artifact_contracts::CanonicalContractField::UncontrolledAlias => {
                "uncontrolled_alias".to_string()
            }
        };
        if matches!(
            status,
            db::repos::artifact_contracts::CanonicalContractField::Resolved(ref value)
                if value.as_str() == Some(target_status)
        ) {
            return Ok(());
        }

        let Some(refine_index) = state
            .transitions
            .iter()
            .position(|transition| transition.to == "state_10_implementation_refined")
        else {
            candidate_evaluations[release_index].result = CandidateTransitionResult::NotMatched;
            candidate_evaluations[release_index].sanitized_diagnostic = Some(format!(
                "implementation_review_summary.status={status_label} cannot enter manual release, and no refinement transition exists"
            ));
            return Ok(());
        };

        candidate_evaluations[release_index].result = CandidateTransitionResult::NotMatched;
        candidate_evaluations[release_index].sanitized_diagnostic = Some(format!(
            "implementation_review_summary.status={status_label} routes back to implementation refinement, not manual release"
        ));

        if let Some(exhaustion) =
            closeout_loop_budget_exhaustion(plan, all_stages, &state.transitions[refine_index].to)
        {
            candidate_evaluations[refine_index].result = CandidateTransitionResult::NotMatched;
            candidate_evaluations[refine_index].sanitized_diagnostic = Some(format!(
                "implementation_review_summary.status={status_label} requires refinement, but loop budget exhausted for {}: {}/{} iterations",
                exhaustion.counter, exhaustion.iterations, exhaustion.max
            ));
            return Ok(());
        }

        candidate_evaluations[refine_index].result = CandidateTransitionResult::Matched;
        candidate_evaluations[refine_index].sanitized_diagnostic = Some(format!(
            "implementation_review_summary.status={status_label} requires continued implementation refinement"
        ));
        Ok(())
    }

    async fn record_workflow_conflict_and_block(
        &self,
        run_id: RunId,
        current_state_id: &str,
        all_stages: &[StageExecution],
        candidate_evaluations: Vec<CandidateTransitionEvaluation>,
        plan: &workflow::plan::RunPlan,
        run: &domain::run::Run,
        reason_override: Option<WorkflowConflictReason>,
    ) -> Result<()> {
        let reason = reason_override.unwrap_or_else(|| {
            classify_workflow_conflict_reason(&candidate_evaluations)
                .unwrap_or(WorkflowConflictReason::NoDeclarativeTransitionMatched)
        });
        let candidate_hash = candidate_transition_hash(&candidate_evaluations);
        let advisory_evidence_refs = self
            .collect_blocking_advisory_evidence_refs(run, plan)
            .await?;
        let fingerprint = workflow_conflict_fingerprint(
            &run_id.to_string(),
            current_state_id,
            &reason,
            &candidate_hash,
            &advisory_evidence_refs,
        );
        let stage_execution_id = all_stages
            .iter()
            .filter(|stage| stage.stage_id == current_state_id)
            .max_by_key(|stage| (stage.iteration, stage.attempt_number, stage.started_at))
            .map(|stage| stage.id.to_string());
        let now = Utc::now();
        let status = initial_workflow_conflict_status(&reason);
        let terminal_failure_reason = initial_workflow_conflict_terminal_failure_reason(&reason);
        let record = WorkflowConflictRecord {
            conflict_id: uuid::Uuid::new_v4().to_string(),
            conflict_fingerprint: fingerprint,
            run_id: run_id.to_string(),
            stage_execution_id: stage_execution_id.clone(),
            lineage_id: stage_execution_id,
            current_state_id: current_state_id.to_string(),
            operator_label: workflow_conflict_operator_label(&reason).to_string(),
            reason,
            status,
            candidate_transitions: candidate_evaluations,
            candidate_transition_hash: candidate_hash,
            advisory_evidence_refs,
            lead_agent_id: None,
            mediation_record_id: None,
            created_at: now,
            updated_at: now,
            resolved_at: None,
            superseded_by_conflict_id: None,
            resolution_record_json: None,
            terminal_failure_reason,
            diagnostic_redaction_tier: "operator_safe".to_string(),
        };
        let stored =
            workflow_conflicts::upsert_conflict_by_fingerprint(&self.pool, &record).await?;
        info!(
            run_id = %run_id,
            state = current_state_id,
            conflict_id = %stored.conflict_id,
            conflict_fingerprint = %stored.conflict_fingerprint,
            reason = %stored.reason,
            "Persisted workflow conflict record"
        );

        if let Ok(Some(run)) = runs::find_by_id(&self.pool, run_id).await {
            if run.status != RunStatus::Blocked {
                runs::update_status(&self.pool, run_id, RunStatus::Blocked).await?;
                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id,
                    status: RunStatus::Blocked,
                });
            }
        }
        // P017 Phase B: attempt mediation initiation for eligible conflicts.
        let mut effective_status = stored.status.clone();
        if crate::mediation::feature_flag::is_phase_b_mediation_enabled()
            && !stored.status.is_terminal_or_operator()
        {
            match self
                .try_initiate_mediation(&stored, run_id, current_state_id, now)
                .await
            {
                Ok(Some(mediation_id)) => {
                    effective_status = WorkflowConflictStatus::LeadMediationPending;
                    info!(
                        run_id = %run_id,
                        conflict_id = %stored.conflict_id,
                        mediation_id = %mediation_id,
                        "Phase B mediation initiated for conflict"
                    );
                }
                Ok(None) => {
                    debug!(
                        run_id = %run_id,
                        conflict_id = %stored.conflict_id,
                        "Phase B lead resolution failed closed; no mediation initiated"
                    );
                }
                Err(e) => {
                    warn!(
                        run_id = %run_id,
                        conflict_id = %stored.conflict_id,
                        error = %e,
                        "Phase B mediation initiation failed; conflict remains unresolved"
                    );
                }
            }
        }

        self.record_workflow_transition_cursor(WorkflowTransitionCursorRecord {
            schema_version: WorkflowTransitionCursorRecord::SCHEMA_VERSION.to_string(),
            run_id: run_id.to_string(),
            current_state_id: current_state_id.to_string(),
            cursor_status: if effective_status == WorkflowConflictStatus::TerminalUnverifiable {
                "terminal_unverifiable".to_string()
            } else if effective_status == WorkflowConflictStatus::LeadMediationPending {
                "lead_mediation_pending".to_string()
            } else {
                "awaiting_conflict_resolution".to_string()
            },
            resume_policy: if effective_status == WorkflowConflictStatus::TerminalUnverifiable {
                "terminal_failure".to_string()
            } else if effective_status == WorkflowConflictStatus::LeadMediationPending {
                "await_mediation_settlement".to_string()
            } else {
                "await_conflict_resolution".to_string()
            },
            selected_transition_id: None,
            selected_next_state_id: None,
            conflict_id: Some(stored.conflict_id.clone()),
            conflict_fingerprint: Some(stored.conflict_fingerprint.clone()),
            candidate_transition_hash: Some(stored.candidate_transition_hash.clone()),
            terminal_failure_reason: stored.terminal_failure_reason.clone(),
            updated_at: stored.updated_at,
        })
        .await?;
        Ok(())
    }

    /// P017 Phase B: Attempt to initiate lead mediation for an eligible conflict.
    /// Returns Some(mediation_id) if mediation was created, None if lead resolution
    /// failed closed (which is a normal, expected outcome).
    async fn try_initiate_mediation(
        &self,
        conflict: &WorkflowConflictRecord,
        run_id: RunId,
        _current_state_id: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<String>> {
        use crate::mediation::lead_resolver::{LeadResolution, PhaseBLeadResolver};

        // MF-PRE-ENABLE-002: Wrap find-active, insert, and update-pointer in a
        // single IMMEDIATE transaction to prevent orphaned mediation records.
        // Lead resolution (file I/O) happens outside the tx since it's read-only.

        // Attempt Phase B lead resolution from the versioned compatibility map.
        // If the map file doesn't exist or no match is found, resolution fails closed.
        let resolver_path =
            "docs/reference/workflow-conflict-evidence/phase-0-phase-b-lead-resolver.json";
        let resolver = match PhaseBLeadResolver::load_from_file(resolver_path) {
            Ok(r) => r,
            Err(e) => {
                debug!(
                    run_id = %run_id,
                    error = %e,
                    "Phase B lead resolver map not available; mediation cannot start"
                );
                return Ok(None);
            }
        };

        // Resolve using the run's workflow and catalog paths.
        let run = runs::find_by_id(&self.pool, run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;
        let workflow_path = run.workflow_yaml_path.as_deref().unwrap_or("");
        let catalog_path = run.agent_catalog_yaml_path.as_deref().unwrap_or("");

        let lead_agent_id = match resolver.resolve(workflow_path, catalog_path) {
            LeadResolution::Resolved { lead_agent_id, .. } => lead_agent_id,
            LeadResolution::FailedClosed { reason } => {
                debug!(
                    run_id = %run_id,
                    conflict_id = %conflict.conflict_id,
                    reason = %reason,
                    "Phase B lead resolution failed closed"
                );
                return Ok(None);
            }
        };

        // Begin IMMEDIATE transaction for the idempotency check + insert + pointer update.
        let mut tx = self
            .begin_orchestrator_transaction(
                "mediation.try_initiate",
                format!("mediation.try_initiate:{run_id}:{}", conflict.conflict_id),
            )
            .await?;

        // Check for existing active mediation for this conflict fingerprint (idempotent).
        if let Some(existing) = lead_conflict_mediations::find_active_for_conflict_tx(
            &mut tx,
            &run_id.to_string(),
            &conflict.conflict_fingerprint,
        )
        .await?
        {
            // OPS-002 (P017 R4): emit duplicate_mediation_session_total when
            // resume / retry / orchestrator-replay attempts to create a new
            // mediation for a conflict that already has an active one.
            // Detection source `try_initiate` distinguishes this from
            // settlement-side or readback-side detections that may exist
            // in future production callers.
            let now = Utc::now();
            let _ = db::repos::workflow_conflicts::record_duplicate_mediation_session_tx(
                &mut tx,
                &run_id.to_string(),
                &conflict.conflict_id,
                &existing.id,
                "try_initiate",
                now,
            )
            .await;
            tx.commit().await.ok();
            return Ok(Some(existing.id));
        }

        // Create the mediation record inside the transaction.
        let mediation_id = uuid::Uuid::new_v4().to_string();
        let mediation = domain::mediation::LeadConflictMediationRecord {
            id: mediation_id.clone(),
            run_id: run_id.to_string(),
            conflict_id: conflict.conflict_id.clone(),
            conflict_fingerprint: conflict.conflict_fingerprint.clone(),
            lead_agent_id: lead_agent_id.clone(),
            status: domain::mediation::LeadMediationStatus::Pending,
            settlement_result: None,
            recovery_action: None,
            chosen_action: None,
            chosen_next_state_id: None,
            chosen_next_state_label: None,
            operator_rationale: None,
            sanitized_progress: Some("Queued for lead mediation".to_string()),
            validation_errors_json: None,
            cost_summary_json: None,
            metric_event_id: None,
            superseded_by_event_ref: None,
            agent_execution_id: None,
            confirmation_subject_id: None,
            created_at: now,
            updated_at: now,
            settled_at: None,
        };
        lead_conflict_mediations::insert_tx(&mut tx, &mediation).await?;

        // Update conflict record with mediation pointer and lead_mediation_pending status.
        workflow_conflicts::update_mediation_pointer_tx(
            &mut tx,
            &conflict.conflict_id,
            &lead_agent_id,
            &mediation_id,
            WorkflowConflictStatus::LeadMediationPending,
            now,
        )
        .await?;

        // Commit the atomic find+insert+pointer-update.
        tx.commit().await?;

        // Enqueue InvokeAgent work item for the lead agent with owner-aware payload.
        // MC-002: If enqueue fails after the mediation record was committed,
        // transition the orphaned Pending mediation to terminal_unverifiable
        // so it doesn't permanently block new mediation for the same fingerprint.
        if let Err(enqueue_err) = self
            .enqueue_mediation_invoke_agent(run_id, &run, &mediation_id, conflict, &lead_agent_id)
            .await
        {
            tracing::error!(
                run_id = %run_id,
                mediation_id = %mediation_id,
                error = %enqueue_err,
                "Failed to enqueue mediation work item; transitioning orphaned mediation to terminal"
            );
            // Best-effort recovery: mark the orphaned mediation as terminal.
            let recovery_now = chrono::Utc::now();
            if let Ok(mut recovery_tx) = self
                .begin_orchestrator_transaction(
                    "mediation.orphan_recovery",
                    format!("mediation.orphan_recovery:{mediation_id}"),
                )
                .await
            {
                let _ = db::repos::lead_conflict_mediations::update_status_tx(
                    &mut recovery_tx,
                    &mediation_id,
                    "terminal_unverifiable",
                    Some("enqueue_failure"),
                    Some("clone_or_manual_fallback"),
                    recovery_now,
                )
                .await;
                let _ = recovery_tx.commit().await;
            }
            return Err(enqueue_err);
        }

        Ok(Some(mediation_id))
    }

    /// P017 Phase B: Enqueue an InvokeAgent work item for the resolved lead agent,
    /// with owner_kind=lead_conflict_mediation and the mediation record as owner_id.
    async fn enqueue_mediation_invoke_agent(
        &self,
        run_id: RunId,
        run: &domain::run::Run,
        mediation_id: &str,
        conflict: &WorkflowConflictRecord,
        lead_agent_id: &str,
    ) -> Result<()> {
        // Look up the lead agent's config from the catalog.
        let catalog_path = run
            .agent_catalog_yaml_path
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Run has no agent catalog path"))?;
        let catalog = workflow::catalog::load(catalog_path)?;
        let agents = catalog
            .agents
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Agent catalog has no agents list"))?;
        let lead_agent = agents
            .iter()
            .find(|a| a.id == lead_agent_id)
            .ok_or_else(|| {
                anyhow::anyhow!("Lead agent '{}' not found in catalog", lead_agent_id)
            })?;

        // Resolve provider from backend profile or agent entry.
        let provider = resolve_agent_provider(lead_agent, &catalog);
        let model = resolve_agent_model(lead_agent, &catalog);
        let effort = resolve_agent_effort(lead_agent, &catalog);
        let lead_resolution_contract_id = lead_agent
            .lead_resolution_contract
            .as_deref()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Lead agent '{}' missing lead_resolution_contract",
                    lead_agent_id
                )
            })?;
        let lead_resolution_contract = catalog
            .contracts
            .as_ref()
            .and_then(|contracts| contracts.get(lead_resolution_contract_id))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Lead agent '{}' references missing lead_resolution_contract '{}'",
                    lead_agent_id,
                    lead_resolution_contract_id
                )
            })?;
        let lead_resolution_target_path = catalog
            .artifacts
            .as_ref()
            .and_then(|artifacts| artifacts.get("lead_resolution"))
            .cloned()
            .unwrap_or_else(|| {
                "${CHAINWORKS_META_ROOT:-.chainworks}/mediation/lead-resolution.json".to_string()
            });
        let lead_resolution_output_schema = serde_json::json!({
            "contract_id": lead_resolution_contract_id,
            "format": lead_resolution_contract
                .format
                .clone()
                .unwrap_or_else(|| "json".to_string()),
            "human_format": lead_resolution_contract.human_format.clone(),
            "machine_format": lead_resolution_contract.machine_format.clone(),
            "validation_mode": lead_resolution_contract.validation_mode.clone(),
            "normalized_artifact_name": lead_resolution_contract.normalized_artifact_name.clone(),
            "raw_artifact_name": lead_resolution_contract.raw_artifact_name.clone(),
            "required_fields": lead_resolution_contract.required_fields.clone(),
        });
        let declared_outputs = serde_json::json!([{
            "output_name": "lead_resolution",
            "target_path": lead_resolution_target_path,
            "schema": lead_resolution_output_schema,
            "reuse_policy": serde_json::Value::Null,
            "companion_output_name": serde_json::Value::Null,
            "companion_path": serde_json::Value::Null,
        }]);

        let prompt = format!(
            "You are the system lead agent mediating workflow conflict {}. \
             Conflict reason: {}. Current state: {}. \
             Analyze the conflict and propose a resolution. Return the required \
             LeadResolutionContract as CHAINWORKS_OUTPUT for lead_resolution.",
            conflict.conflict_id, conflict.reason, conflict.current_state_id,
        );

        let work_item_id = format!("p017-mediation:{}:0", mediation_id);
        self.work_queue
            .enqueue_with_id(
                work_item_id,
                WorkItemKind::InvokeAgent,
                Some(run_id),
                Some(conflict.current_state_id.clone()),
                serde_json::json!({
                    "run_id": run_id.to_string(),
                    "stage_id": conflict.current_state_id,
                    "stage_execution_id": serde_json::Value::Null,
                    "task_name": format!("mediation_{}", mediation_id),
                    "task_inputs": Vec::<String>::new(),
                    "task_outputs": ["lead_resolution"],
                    "declared_outputs": declared_outputs,
                    "agent_id": lead_agent_id,
                    "provider": provider,
                    "model": model,
                    "effort": effort,
                    "output_contract": lead_resolution_contract_id,
                    "prompt": prompt,
                    "task_index": 0,
                    "total_tasks": 1,
                    // P017: Owner-aware execution identity fields.
                    "owner_kind": "lead_conflict_mediation",
                    "owner_id": mediation_id,
                    "origin_stage_id": conflict.current_state_id,
                    "origin_stage_execution_id": conflict.stage_execution_id,
                    "mediation_record_id": mediation_id,
                    "conflict_fingerprint": conflict.conflict_fingerprint,
                }),
            )
            .await?;

        // Update mediation status to queued.
        let now = chrono::Utc::now();
        let mut tx = self
            .begin_orchestrator_transaction(
                "mediation.update_status_queued",
                format!("mediation.update_status_queued:{mediation_id}"),
            )
            .await?;
        db::repos::lead_conflict_mediations::update_status_tx(
            &mut tx,
            mediation_id,
            "queued",
            None,
            None,
            now,
        )
        .await?;
        tx.commit().await?;

        info!(
            run_id = %run_id,
            mediation_id = %mediation_id,
            lead_agent_id = %lead_agent_id,
            "Enqueued lead agent invocation for mediation"
        );
        Ok(())
    }

    async fn record_workflow_transition_cursor(
        &self,
        cursor: WorkflowTransitionCursorRecord,
    ) -> Result<()> {
        workflow_conflicts::upsert_transition_cursor(&self.pool, &cursor).await
    }

    async fn record_advisory_rejections_for_selected_transition(
        &self,
        run_id: RunId,
        current_state_id: &str,
        selected_transition_id: &str,
        selected_next_state_id: &str,
        plan: &workflow::plan::RunPlan,
        all_stages: &[StageExecution],
        run: &domain::run::Run,
    ) -> Result<()> {
        let stage_execution_id = latest_stage_execution_id_for_state(all_stages, current_state_id);
        for advisory in self.collect_transition_advisories(run, plan).await? {
            let Some(next_stage_hint) = advisory.next_stage_hint.as_deref() else {
                continue;
            };
            if next_stage_hint == selected_next_state_id {
                continue;
            }

            let graph_membership_result =
                advisory_graph_membership_result(plan, &advisory, Some(selected_next_state_id));
            let advisory_hint_hash = sha256_prefixed_json(&serde_json::json!({
                "schema": "workflow_advisory_hint_v1",
                "source_artifact_id": advisory.source_artifact_id.clone(),
                "next_stage": advisory.next_stage_hint.clone(),
                "next_action": advisory.next_action.clone(),
                "graph_membership_result": graph_membership_result,
            }));
            let mut provenance = Vec::new();
            if let Some(next_stage) = advisory.next_stage_hint.clone() {
                provenance.push(AdvisoryHintExtraction {
                    source_artifact_id: advisory.source_artifact_id.clone(),
                    source_agent_execution_id: advisory.source_agent_execution_id.clone(),
                    advisory_path: "$.next_stage".to_string(),
                    raw_value_hash: sha256_prefixed_json(&next_stage),
                    redacted_value: Some(next_stage),
                    graph_membership_result: graph_membership_result.to_string(),
                    superseded_by_projection: advisory.superseded_by_projection,
                    included_in_candidate_transition_hash: true,
                });
            }
            if let Some(next_action) = advisory.next_action.clone() {
                provenance.push(AdvisoryHintExtraction {
                    source_artifact_id: advisory.source_artifact_id.clone(),
                    source_agent_execution_id: advisory.source_agent_execution_id.clone(),
                    advisory_path: "$.next_action".to_string(),
                    raw_value_hash: sha256_prefixed_json(&next_action),
                    redacted_value: Some(next_action),
                    graph_membership_result: graph_membership_result.to_string(),
                    superseded_by_projection: advisory.superseded_by_projection,
                    included_in_candidate_transition_hash: true,
                });
            }

            let record = WorkflowAdvisoryRejectionRecord {
                rejection_id: uuid::Uuid::new_v4().to_string(),
                run_id: run_id.to_string(),
                stage_execution_id: stage_execution_id.clone(),
                lineage_id: stage_execution_id.clone(),
                current_state_id: current_state_id.to_string(),
                selected_transition_id: selected_transition_id.to_string(),
                selected_next_state_id: selected_next_state_id.to_string(),
                advisory_next_stage_hint: advisory.next_stage_hint.clone(),
                advisory_next_action: advisory.next_action.clone(),
                advisory_hint_hash,
                advisory_hint_provenance: provenance,
                graph_membership_result: graph_membership_result.to_string(),
                created_at: Utc::now(),
            };
            workflow_conflicts::insert_advisory_rejection(&self.pool, &record).await?;
            info!(
                run_id = %run_id,
                state = current_state_id,
                selected_transition_id = selected_transition_id,
                selected_next_state_id = selected_next_state_id,
                advisory_next_stage_hint = ?record.advisory_next_stage_hint,
                graph_membership_result = graph_membership_result,
                "Persisted workflow advisory rejection"
            );
        }
        Ok(())
    }

    async fn resolve_current_workflow_conflict_for_selected_transition(
        &self,
        run_id: RunId,
        current_state_id: &str,
        selected_transition_id: &str,
        selected_next_state_id: &str,
    ) -> Result<()> {
        let Some(conflict) =
            workflow_conflicts::get_current_blocking_conflict(&self.pool, run_id).await?
        else {
            return Ok(());
        };
        if conflict.current_state_id != current_state_id {
            return Ok(());
        }

        workflow_conflicts::transition_conflict_status(
            &self.pool,
            &conflict.conflict_id,
            WorkflowConflictStatus::Resolved,
            Utc::now(),
            Some(serde_json::json!({
                "resolution_kind": "graph_authoritative_transition_selected",
                "selected_transition_id": selected_transition_id,
                "selected_next_state_id": selected_next_state_id,
            })),
            None,
            None,
        )
        .await?;
        info!(
            run_id = %run_id,
            state = current_state_id,
            conflict_id = %conflict.conflict_id,
            selected_transition_id = selected_transition_id,
            selected_next_state_id = selected_next_state_id,
            "Resolved current workflow conflict before graph-authoritative transition"
        );
        Ok(())
    }

    async fn collect_blocking_advisory_evidence_refs(
        &self,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> Result<Vec<String>> {
        let mut refs = std::collections::BTreeSet::new();
        for advisory in self.collect_transition_advisories(run, plan).await? {
            let graph_membership_result = advisory_graph_membership_result(plan, &advisory, None);
            for evidence_ref in advisory_evidence_refs_for_hint(&advisory, graph_membership_result)
            {
                refs.insert(evidence_ref);
            }
        }
        Ok(refs.into_iter().collect())
    }

    async fn collect_transition_advisories(
        &self,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> Result<Vec<TransitionAdvisoryHint>> {
        let mut advisories = Vec::new();
        let mut seen_sources = std::collections::BTreeSet::new();

        if let Some(projection) =
            artifact_contracts::find_run_state_projection(&self.pool, run.id).await?
        {
            if let Some(items) = projection
                .active_index_json
                .get("advisory_artifacts")
                .and_then(|value| value.as_array())
            {
                for item in items {
                    let Some(path) = item.get("advisory_path").and_then(|value| value.as_str())
                    else {
                        continue;
                    };
                    if !seen_sources.insert(format!("projection:{path}")) {
                        continue;
                    }
                    let Some(json) = read_json_file(path) else {
                        continue;
                    };
                    if let Some(hint) = advisory_hint_from_json(
                        item.get("advisory_id")
                            .and_then(|value| value.as_str())
                            .unwrap_or(path),
                        item.get("source_agent_execution_id")
                            .and_then(|value| value.as_str())
                            .map(str::to_string),
                        true,
                        &json,
                    ) {
                        advisories.push(hint);
                    }
                }
            }
        }

        for (artifact_name, path_template) in &plan.artifact_paths {
            let path = resolve_path_template(
                path_template,
                &run.workspace_root,
                run.chainworks_meta_root.as_deref(),
            );
            if !seen_sources.insert(format!("plan:{path}")) {
                continue;
            }
            let Some(json) = read_json_file(&path) else {
                continue;
            };
            if let Some(hint) = advisory_hint_from_json(artifact_name, None, false, &json) {
                advisories.push(hint);
            }
        }

        Ok(advisories)
    }

    async fn proposal_review_summary_transition_truth_conflict(
        &self,
        candidate_evaluations: &[CandidateTransitionEvaluation],
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> Result<Option<String>> {
        if !candidate_evaluations.iter().any(|candidate| {
            candidate
                .required_artifacts
                .iter()
                .any(|artifact| artifact == "proposal_review_summary")
        }) {
            return Ok(None);
        }

        let pass = self
            .read_artifact_field("proposal_review_summary", "pass", run, plan)
            .await
            .and_then(|value| value.as_bool());
        let blocker_count = self
            .read_artifact_field("proposal_review_summary", "blocker_count", run, plan)
            .await
            .and_then(|value| json_value_to_u64(&value));
        let blocking_issues = self
            .read_artifact_field("proposal_review_summary", "blocking_issues", run, plan)
            .await;
        let required_changes = self
            .read_artifact_field("proposal_review_summary", "required_changes", run, plan)
            .await;
        let decision = self
            .read_artifact_field("proposal_review_summary", "decision", run, plan)
            .await
            .and_then(|value| value.as_str().map(str::to_string));

        let has_blocker_count = blocker_count.is_some_and(|count| count > 0);
        let has_blocking_issues = json_value_has_entries(blocking_issues.as_ref());
        let has_required_changes = json_value_has_entries(required_changes.as_ref());
        let has_blocking_evidence =
            has_blocker_count || has_blocking_issues || has_required_changes;

        if pass == Some(true) && has_blocking_evidence {
            return Ok(Some(
                "proposal_review_summary_v1 has pass=true while blocker evidence is non-empty"
                    .to_string(),
            ));
        }

        let explicitly_no_blocking_issues = blocking_issues
            .as_ref()
            .is_some_and(json_value_is_empty_collection);
        let explicitly_no_required_changes = required_changes
            .as_ref()
            .is_some_and(json_value_is_empty_collection);
        if pass == Some(false)
            && blocker_count == Some(0)
            && explicitly_no_blocking_issues
            && explicitly_no_required_changes
        {
            return Ok(Some(
                "proposal_review_summary_v1 has pass=false while blocker evidence is explicitly empty"
                    .to_string(),
            ));
        }

        if let Some(decision) = decision.as_deref() {
            let decision = decision.to_ascii_lowercase();
            let decision_passes = decision.contains("pass")
                || decision.contains("approve")
                || decision.contains("approved");
            let decision_blocks = decision.contains("fail")
                || decision.contains("block")
                || decision.contains("revise")
                || decision.contains("changes_required");
            if decision_passes && (pass == Some(false) || has_blocking_evidence) {
                return Ok(Some(
                    "proposal_review_summary_v1 decision indicates pass while authoritative fields block"
                        .to_string(),
                ));
            }
            if decision_blocks && pass == Some(true) && !has_blocking_evidence {
                return Ok(Some(
                    "proposal_review_summary_v1 decision indicates blocking while authoritative fields pass"
                        .to_string(),
                ));
            }
        }

        Ok(None)
    }

    async fn evaluate_transition_candidate(
        &self,
        transition_index: usize,
        current_state_id: &str,
        transition: &workflow::plan::CompiledTransition,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> CandidateTransitionEvaluation {
        let condition = self
            .evaluate_condition_classified(&transition.condition, run, plan, current_state_id)
            .await;
        CandidateTransitionEvaluation {
            transition_id: transition_id_for(current_state_id, &transition.to, transition_index),
            from_state_id: current_state_id.to_string(),
            to_state_id: transition.to.clone(),
            condition_expression_id: Some(format!("transition_condition_{}", transition_index)),
            result: condition.result,
            required_artifacts: condition.required_artifacts,
            missing_artifacts: condition.missing_artifacts,
            missing_fields: condition.missing_fields,
            source_artifact_ids: condition.source_artifact_ids,
            source_agent_execution_id: None,
            sanitized_diagnostic: condition.sanitized_diagnostic,
        }
    }

    async fn evaluate_condition_classified(
        &self,
        condition: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
        current_state_id: &str,
    ) -> ClassifiedConditionEvaluation {
        let trimmed = condition.trim().trim_matches('"');

        if trimmed == "true" || trimmed == "'true'" {
            return ClassifiedConditionEvaluation::matched();
        }
        if trimmed == "false" || trimmed == "'false'" {
            return ClassifiedConditionEvaluation::not_matched();
        }

        if let Some(split) = split_connective(trimmed, " and ") {
            let left =
                Box::pin(self.evaluate_condition_classified(split.0, run, plan, current_state_id))
                    .await;
            let right =
                Box::pin(self.evaluate_condition_classified(split.1, run, plan, current_state_id))
                    .await;
            return ClassifiedConditionEvaluation::combine_and(left, right);
        }
        if let Some(split) = split_connective(trimmed, " or ") {
            let left =
                Box::pin(self.evaluate_condition_classified(split.0, run, plan, current_state_id))
                    .await;
            let right =
                Box::pin(self.evaluate_condition_classified(split.1, run, plan, current_state_id))
                    .await;
            return ClassifiedConditionEvaluation::combine_or(left, right);
        }

        if trimmed.starts_with("exists(") && trimmed.ends_with(')') {
            let artifact_name = trimmed[7..trimmed.len() - 1]
                .trim_matches('\'')
                .trim_matches('"');
            return self
                .evaluate_artifact_exists_classified(artifact_name, run, plan)
                .await;
        }

        if trimmed == "approval.granted == true" {
            let approvals = approvals::list_by_run(&self.pool, run.id)
                .await
                .unwrap_or_default();
            return if approvals
                .iter()
                .any(|a| a.stage_id == current_state_id && a.decision == ApprovalDecision::Granted)
            {
                ClassifiedConditionEvaluation::matched()
            } else {
                ClassifiedConditionEvaluation::not_matched()
            };
        }
        if trimmed == "approval.rejected == true" {
            let approvals = approvals::list_by_run(&self.pool, run.id)
                .await
                .unwrap_or_default();
            return if approvals
                .iter()
                .any(|a| a.stage_id == current_state_id && a.decision == ApprovalDecision::Rejected)
            {
                ClassifiedConditionEvaluation::matched()
            } else {
                ClassifiedConditionEvaluation::not_matched()
            };
        }

        for (op_str, op) in &[
            (" <= ", CompOp::Le),
            (" >= ", CompOp::Ge),
            (" != ", CompOp::Ne),
            (" == ", CompOp::Eq),
            (" < ", CompOp::Lt),
            (" > ", CompOp::Gt),
        ] {
            if let Some(pos) = trimmed.find(op_str) {
                let lhs = trimmed[..pos].trim();
                let rhs = trimmed[pos + op_str.len()..].trim();
                if let Some((artifact_name, field_name)) = artifact_field_ref(lhs) {
                    return self
                        .evaluate_artifact_field_comparison_classified(
                            artifact_name,
                            field_name,
                            *op,
                            rhs,
                            run,
                            plan,
                        )
                        .await;
                }
                let lv = self.resolve_value(lhs, run, plan).await;
                let rv = self.resolve_value(rhs, run, plan).await;
                return if apply_comparison(&lv, *op, &rv) {
                    ClassifiedConditionEvaluation::matched()
                } else {
                    ClassifiedConditionEvaluation::not_matched()
                };
            }
        }

        ClassifiedConditionEvaluation::invalid_expression(format!(
            "Unsupported transition condition: {trimmed}"
        ))
    }

    async fn evaluate_artifact_exists_classified(
        &self,
        artifact_name: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> ClassifiedConditionEvaluation {
        match db::repos::artifact_contracts::active_contract_exists_result(
            &self.pool,
            run.id,
            artifact_name,
        )
        .await
        {
            Ok(db::repos::artifact_contracts::CanonicalContractField::Resolved(_)) => {
                return ClassifiedConditionEvaluation::matched()
                    .with_required_artifact(artifact_name)
                    .with_source_artifact(artifact_name);
            }
            Ok(db::repos::artifact_contracts::CanonicalContractField::MissingControlled {
                contract_id,
            }) => {
                return ClassifiedConditionEvaluation::missing_input(format!(
                    "Controlled artifact {contract_id} is missing canonical DB truth"
                ))
                .with_required_artifact(artifact_name)
                .with_missing_artifact(artifact_name);
            }
            Ok(db::repos::artifact_contracts::CanonicalContractField::UncontrolledAlias) => {}
            Err(error) => {
                if let Some(contract_id) =
                    db::repos::artifact_contracts::contract_id_for_alias(artifact_name)
                {
                    return ClassifiedConditionEvaluation::missing_input(format!(
                        "Controlled artifact {contract_id} lookup failed: {error}"
                    ))
                    .with_required_artifact(artifact_name)
                    .with_missing_artifact(artifact_name);
                }
            }
        }

        let Some(path_template) = plan.artifact_paths.get(artifact_name) else {
            return ClassifiedConditionEvaluation::invalid_expression(format!(
                "Artifact {artifact_name} is not declared by the workflow/catalog contract"
            ))
            .with_required_artifact(artifact_name);
        };

        let resolved = resolve_path_template(
            path_template,
            &run.workspace_root,
            run.chainworks_meta_root.as_deref(),
        );
        if std::path::Path::new(&resolved).exists() {
            return ClassifiedConditionEvaluation::matched()
                .with_required_artifact(artifact_name)
                .with_source_artifact(artifact_name);
        }
        if run.chainworks_meta_root.is_none() {
            for suffix in &[
                artifact_name.to_string(),
                format!("{}/{}", run.id, artifact_name),
            ] {
                let path = format!("{}/{}", run.artifact_root, suffix);
                if std::path::Path::new(&path).exists() {
                    return ClassifiedConditionEvaluation::matched()
                        .with_required_artifact(artifact_name)
                        .with_source_artifact(artifact_name);
                }
            }
        }

        ClassifiedConditionEvaluation::missing_input(format!(
            "Declared artifact {artifact_name} is absent"
        ))
        .with_required_artifact(artifact_name)
        .with_missing_artifact(artifact_name)
    }

    async fn evaluate_artifact_field_comparison_classified(
        &self,
        artifact_name: &str,
        field_name: &str,
        op: CompOp,
        rhs: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> ClassifiedConditionEvaluation {
        let field_ref = format!("{artifact_name}.{field_name}");
        match db::repos::artifact_contracts::canonical_contract_field_result(
            &self.pool,
            run.id,
            artifact_name,
            field_name,
        )
        .await
        {
            Ok(db::repos::artifact_contracts::CanonicalContractField::Resolved(value)) => {
                let rv = self.resolve_value(rhs, run, plan).await;
                return if apply_comparison(&value, op, &rv) {
                    ClassifiedConditionEvaluation::matched()
                } else {
                    ClassifiedConditionEvaluation::not_matched()
                }
                .with_required_artifact(artifact_name)
                .with_source_artifact(artifact_name);
            }
            Ok(db::repos::artifact_contracts::CanonicalContractField::MissingControlled {
                contract_id,
            }) => {
                if is_implementation_self_assessment_alias(artifact_name) {
                    if let Some(value) = self
                        .active_implementation_self_assessment_summary_field(run.id, field_name)
                        .await
                    {
                        let rv = self.resolve_value(rhs, run, plan).await;
                        return if apply_comparison(&value, op, &rv) {
                            ClassifiedConditionEvaluation::matched()
                        } else {
                            ClassifiedConditionEvaluation::not_matched()
                        }
                        .with_required_artifact(artifact_name)
                        .with_source_artifact(artifact_name);
                    }
                }
                return ClassifiedConditionEvaluation::missing_input(format!(
                    "Controlled artifact field {contract_id}.{field_name} is missing canonical DB truth"
                ))
                .with_required_artifact(artifact_name)
                .with_missing_field(&field_ref);
            }
            Ok(db::repos::artifact_contracts::CanonicalContractField::UncontrolledAlias) => {}
            Err(error) => {
                if let Some(contract_id) =
                    db::repos::artifact_contracts::contract_id_for_alias(artifact_name)
                {
                    return ClassifiedConditionEvaluation::missing_input(format!(
                        "Controlled artifact field {contract_id}.{field_name} lookup failed: {error}"
                    ))
                    .with_required_artifact(artifact_name)
                    .with_missing_field(&field_ref);
                }
            }
        }

        if !plan.artifact_paths.contains_key(artifact_name) {
            return ClassifiedConditionEvaluation::invalid_expression(format!(
                "Artifact field {field_ref} references an undeclared artifact"
            ))
            .with_required_artifact(artifact_name);
        }

        let Some(value) = self
            .read_artifact_field(artifact_name, field_name, run, plan)
            .await
        else {
            let exists = self
                .evaluate_artifact_exists_classified(artifact_name, run, plan)
                .await;
            return if exists.result == CandidateTransitionResult::MissingInput
                && exists.missing_artifacts.iter().any(|a| a == artifact_name)
            {
                exists
            } else {
                ClassifiedConditionEvaluation::missing_input(format!(
                    "Declared artifact field {field_ref} is absent"
                ))
                .with_required_artifact(artifact_name)
                .with_missing_field(&field_ref)
            };
        };

        let rv = self.resolve_value(rhs, run, plan).await;
        if apply_comparison(&value, op, &rv) {
            ClassifiedConditionEvaluation::matched()
        } else {
            ClassifiedConditionEvaluation::not_matched()
        }
        .with_required_artifact(artifact_name)
        .with_source_artifact(artifact_name)
    }

    /// Resolve a value reference to a JSON Value.
    /// Supports: `vars.name`, `artifact.field`, literals (int/float/string/bool).
    async fn resolve_value(
        &self,
        ref_str: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> serde_json::Value {
        let trimmed = ref_str.trim();

        // vars.* → plan variables
        if let Some(var_name) = trimmed.strip_prefix("vars.") {
            return plan
                .variables
                .get(var_name)
                .cloned()
                .unwrap_or(serde_json::Value::Null);
        }

        // artifact.field → read JSON file, extract field
        if trimmed.contains('.') && !trimmed.starts_with("vars.") {
            let parts: Vec<&str> = trimmed.splitn(2, '.').collect();
            if parts.len() == 2 {
                let artifact_name = parts[0];
                let field_name = parts[1];
                if artifact_name == "implementation_self_assessment_v2"
                    || artifact_name == "implementation_self_assessment"
                {
                    if let Some(val) = self
                        .read_artifact_field(artifact_name, field_name, run, plan)
                        .await
                    {
                        return val;
                    }
                }
                let controlled_contract_id =
                    db::repos::artifact_contracts::contract_id_for_alias(artifact_name);
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let can_block_in_place =
                        handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread;
                    if can_block_in_place {
                        if let Ok(result) = tokio::task::block_in_place(|| {
                            handle.block_on(
                                db::repos::artifact_contracts::canonical_contract_field_result(
                                    &self.pool,
                                    run.id,
                                    artifact_name,
                                    field_name,
                                ),
                            )
                        }) {
                            match result {
                                db::repos::artifact_contracts::CanonicalContractField::Resolved(value) => {
                                    return value;
                                }
                                db::repos::artifact_contracts::CanonicalContractField::MissingControlled {
                                    contract_id,
                                } => {
                                    warn!(
                                        artifact = artifact_name,
                                        field = field_name,
                                        contract_id = %contract_id,
                                        "P057-controlled artifact field missing canonical DB truth; raw artifact fallback disabled"
                                    );
                                    return serde_json::Value::Null;
                                }
                                db::repos::artifact_contracts::CanonicalContractField::UncontrolledAlias => {}
                            }
                        } else if let Some(contract_id) = controlled_contract_id {
                            warn!(
                                artifact = artifact_name,
                                field = field_name,
                                contract_id = %contract_id,
                                "P057-controlled artifact lookup failed; raw artifact fallback disabled"
                            );
                            return serde_json::Value::Null;
                        }
                    } else if let Some(contract_id) = controlled_contract_id {
                        warn!(
                            artifact = artifact_name,
                            field = field_name,
                            contract_id = %contract_id,
                            "P057-controlled artifact lookup unavailable on current-thread runtime; raw artifact fallback disabled"
                        );
                        return serde_json::Value::Null;
                    }
                } else if let Some(contract_id) = controlled_contract_id {
                    warn!(
                        artifact = artifact_name,
                        field = field_name,
                        contract_id = %contract_id,
                        "P057-controlled artifact lookup unavailable outside Tokio runtime; raw artifact fallback disabled"
                    );
                    return serde_json::Value::Null;
                }
                if let Some(val) = self
                    .read_artifact_field(artifact_name, field_name, run, plan)
                    .await
                {
                    return val;
                }
            }
        }

        // Literal: true/false
        if trimmed == "true" {
            return serde_json::Value::Bool(true);
        }
        if trimmed == "false" {
            return serde_json::Value::Bool(false);
        }

        // Literal: number
        if let Ok(n) = trimmed.parse::<i64>() {
            return serde_json::json!(n);
        }
        if let Ok(f) = trimmed.parse::<f64>() {
            return serde_json::json!(f);
        }

        // Literal: quoted string
        if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
            || (trimmed.starts_with('"') && trimmed.ends_with('"'))
        {
            return serde_json::Value::String(trimmed[1..trimmed.len() - 1].to_string());
        }

        // Bare string
        serde_json::Value::String(trimmed.to_string())
    }

    /// Read a field from a JSON artifact file on disk.
    async fn read_artifact_field(
        &self,
        artifact_name: &str,
        field_name: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> Option<serde_json::Value> {
        if is_implementation_self_assessment_alias(artifact_name) {
            return self
                .active_implementation_self_assessment_summary_field(run.id, field_name)
                .await;
        }

        // Find the artifact file path
        let path = if let Some(template) = plan.artifact_paths.get(artifact_name) {
            let resolved = resolve_path_template(
                template,
                &run.workspace_root,
                run.chainworks_meta_root.as_deref(),
            );
            if std::path::Path::new(&resolved).exists() {
                resolved
            } else if run.chainworks_meta_root.is_none() {
                // Legacy fallback: try artifact_root (only for pre-P050 runs).
                // P050: Post-P050 runs must NOT fall back to shared artifact_root.
                let alt = format!("{}/{}", run.artifact_root, artifact_name);
                if std::path::Path::new(&alt).exists() {
                    alt
                } else {
                    let alt2 = format!("{}/{}/{}", run.artifact_root, run.id, artifact_name);
                    if std::path::Path::new(&alt2).exists() {
                        alt2
                    } else {
                        return None;
                    }
                }
            } else {
                // Post-P050 run: no artifact_root fallback — canonical path is the only truth.
                return None;
            }
        } else {
            return None;
        };

        // Read and parse JSON
        let content = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        extract_json_field(&json, field_name)
    }

    async fn active_implementation_self_assessment_summary_field(
        &self,
        run_id: RunId,
        field_name: &str,
    ) -> Option<serde_json::Value> {
        let active = artifact_contracts::find_active_implementation_self_assessment_summary(
            &self.pool, run_id,
        )
        .await
        .ok()
        .flatten()?;
        let summary_json = serde_json::to_value(&active.summary).ok()?;
        if field_name == "seemingly_complete" {
            return extract_json_field(&summary_json, "implementation_complete");
        }
        if field_name == "blocking_remaining_code_tasks" {
            return extract_json_field(&summary_json, "blocking_remaining_code_task_count");
        }
        extract_json_field(&summary_json, field_name)
    }

    // =====================================================================
    // Legacy flat-stage orchestration (no YAML workflow)
    // =====================================================================

    async fn advance_run_flat(&self, run_id: RunId, run: &domain::run::Run) -> Result<()> {
        let all_stages = stages::list_by_run(&self.pool, run_id).await?;
        let all_approvals = approvals::list_by_run(&self.pool, run_id).await?;

        let evaluation = DomainEngine::evaluate_run(run, &all_stages, &all_approvals);

        match evaluation {
            RunEvaluation::Terminal => {}

            RunEvaluation::Complete => {
                info!(run_id = %run_id, "Run complete, marking completed");
                let now = Utc::now();
                self.mark_run_completed_and_refresh(run_id, now).await?;
                self.enqueue_steward_analysis(Some(run_id)).await?;
                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id,
                    status: RunStatus::Completed,
                });
            }

            RunEvaluation::Failed => {
                info!(run_id = %run_id, "All stages terminal, none succeeded — marking failed");
                runs::update_status(&self.pool, run_id, RunStatus::Failed).await?;
                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id,
                    status: RunStatus::Failed,
                });
            }

            RunEvaluation::WaitingApproval { stage_id } => {
                info!(run_id = %run_id, stage_id = %stage_id, "Run waiting for approval");
                if run.status != RunStatus::WaitingApproval {
                    runs::update_status(&self.pool, run_id, RunStatus::WaitingApproval).await?;
                    let _ = self.events.send(DomainEvent::RunStatusChanged {
                        run_id,
                        status: RunStatus::WaitingApproval,
                    });
                }
            }

            RunEvaluation::Blocked { reason } => {
                info!(run_id = %run_id, reason = %reason, "Run blocked");
                if run.status == RunStatus::Cancelling {
                    let now = Utc::now();
                    runs::mark_cancelled(&self.pool, run_id, now).await?;
                    let _ = self.events.send(DomainEvent::RunStatusChanged {
                        run_id,
                        status: RunStatus::Cancelled,
                    });
                } else if run.status != RunStatus::Blocked {
                    runs::update_status(&self.pool, run_id, RunStatus::Blocked).await?;
                    let _ = self.events.send(DomainEvent::RunStatusChanged {
                        run_id,
                        status: RunStatus::Blocked,
                    });
                }
            }

            RunEvaluation::CanAdvance { next_stage_id } => {
                let stage = all_stages.iter().find(|s| s.id == next_stage_id).cloned();

                if let Some(stage) = stage {
                    info!(run_id = %run_id, stage_id = %stage.stage_id, "Activating next stage");
                    stages::update_status(&self.pool, stage.id, StageStatus::Running).await?;
                    let _ = self.events.send(DomainEvent::StageStatusChanged {
                        run_id,
                        stage_execution_id: stage.id,
                        status: StageStatus::Running,
                    });

                    if !matches!(run.status, RunStatus::Running) {
                        runs::update_status(&self.pool, run_id, RunStatus::Running).await?;
                        let _ = self.events.send(DomainEvent::RunStatusChanged {
                            run_id,
                            status: RunStatus::Running,
                        });
                    }

                    // Use per-stage provider if available, else env default.
                    let provider = stage.provider.clone().unwrap_or_else(|| {
                        std::env::var("CHAINWORKS_DEFAULT_PROVIDER")
                            .unwrap_or_else(|_| "claude".to_string())
                    });
                    let agent_id = stage
                        .owner_agent
                        .clone()
                        .unwrap_or_else(|| stage.stage_id.clone());

                    self.work_queue
                        .enqueue_with_id(
                            format!("p058-invoke:{}:0", stage.id),
                            WorkItemKind::InvokeAgent,
                            Some(run_id),
                            Some(stage.stage_id.clone()),
                            serde_json::json!({
                                "run_id": run_id.to_string(),
                                "stage_id": stage.stage_id,
                                "stage_execution_id": stage.id.to_string(),
                                "agent_id": agent_id.clone(),
                                "provider": provider,
                                "session_reuse_scope": "same_agent_family_within_run",
                                "session_family_id": agent_id,
                                "declared_outputs": Vec::<crate::contracts::DeclaredOutput>::new(),
                            }),
                        )
                        .await?;
                }
            }
        }

        Ok(())
    }
}

/// Agent-authored transition hints are advisory evidence only. These values
/// are compared against the selected graph transition and persisted as
/// non-blocking rejection history when they diverge.
#[derive(Clone, Debug)]
struct TransitionAdvisoryHint {
    source_artifact_id: String,
    source_agent_execution_id: Option<String>,
    next_stage_hint: Option<String>,
    next_action: Option<String>,
    superseded_by_projection: bool,
}

fn transition_id_for(current_state_id: &str, to_state_id: &str, transition_index: usize) -> String {
    format!("{current_state_id}__to__{to_state_id}__{transition_index}")
}

fn latest_stage_execution_id_for_state(
    all_stages: &[StageExecution],
    current_state_id: &str,
) -> Option<String> {
    all_stages
        .iter()
        .filter(|stage| stage.stage_id == current_state_id)
        .max_by_key(|stage| (stage.iteration, stage.attempt_number, stage.started_at))
        .map(|stage| stage.id.to_string())
}

fn read_json_file(path: &str) -> Option<serde_json::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn advisory_hint_from_json(
    source_artifact_id: &str,
    source_agent_execution_id: Option<String>,
    superseded_by_projection: bool,
    json: &serde_json::Value,
) -> Option<TransitionAdvisoryHint> {
    let next_stage_hint = json
        .get("next_stage")
        .or_else(|| json.get("nextStage"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);
    let next_action = json
        .get("next_action")
        .or_else(|| json.get("nextAction"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string);

    if next_stage_hint.is_none() && next_action.is_none() {
        return None;
    }

    Some(TransitionAdvisoryHint {
        source_artifact_id: source_artifact_id.to_string(),
        source_agent_execution_id,
        next_stage_hint,
        next_action,
        superseded_by_projection,
    })
}

fn advisory_graph_membership_result(
    plan: &workflow::plan::RunPlan,
    advisory: &TransitionAdvisoryHint,
    selected_next_state_id: Option<&str>,
) -> &'static str {
    let Some(next_stage_hint) = advisory.next_stage_hint.as_deref() else {
        return "no_next_stage_hint";
    };
    if !plan.states.contains_key(next_stage_hint) {
        return "absent_from_graph";
    }
    if selected_next_state_id == Some(next_stage_hint) {
        "graph_state_selected"
    } else {
        "graph_state_not_selected"
    }
}

fn advisory_evidence_refs_for_hint(
    advisory: &TransitionAdvisoryHint,
    graph_membership_result: &str,
) -> Vec<String> {
    let mut refs = Vec::new();
    if let Some(next_stage) = advisory.next_stage_hint.as_deref() {
        refs.push(format!(
            "{}:$.next_stage:{}:{}",
            advisory.source_artifact_id,
            graph_membership_result,
            sha256_prefixed_json(next_stage)
        ));
    }
    if let Some(next_action) = advisory.next_action.as_deref() {
        refs.push(format!(
            "{}:$.next_action:{}",
            advisory.source_artifact_id,
            sha256_prefixed_json(next_action)
        ));
    }
    refs
}

fn sha256_prefixed_json<T: serde::Serialize + ?Sized>(value: &T) -> String {
    let json = serde_json::to_vec(value).expect("workflow advisory hash payload should serialize");
    let digest = Sha256::digest(json);
    format!("sha256:{digest:x}")
}

// ---------------------------------------------------------------------------
// Expression evaluation helpers (match Swift TransitionEvaluator)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum CompOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone)]
struct ClassifiedConditionEvaluation {
    result: CandidateTransitionResult,
    required_artifacts: Vec<String>,
    missing_artifacts: Vec<String>,
    missing_fields: Vec<String>,
    source_artifact_ids: Vec<String>,
    sanitized_diagnostic: Option<String>,
}

impl ClassifiedConditionEvaluation {
    fn matched() -> Self {
        Self::new(CandidateTransitionResult::Matched, None)
    }

    fn not_matched() -> Self {
        Self::new(CandidateTransitionResult::NotMatched, None)
    }

    fn missing_input(diagnostic: String) -> Self {
        Self::new(CandidateTransitionResult::MissingInput, Some(diagnostic))
    }

    fn invalid_expression(diagnostic: String) -> Self {
        Self::new(
            CandidateTransitionResult::InvalidExpression,
            Some(diagnostic),
        )
    }

    fn new(result: CandidateTransitionResult, sanitized_diagnostic: Option<String>) -> Self {
        Self {
            result,
            required_artifacts: Vec::new(),
            missing_artifacts: Vec::new(),
            missing_fields: Vec::new(),
            source_artifact_ids: Vec::new(),
            sanitized_diagnostic,
        }
    }

    fn with_required_artifact(mut self, artifact_name: &str) -> Self {
        push_unique(&mut self.required_artifacts, artifact_name);
        self
    }

    fn with_missing_artifact(mut self, artifact_name: &str) -> Self {
        push_unique(&mut self.missing_artifacts, artifact_name);
        self
    }

    fn with_missing_field(mut self, field_ref: &str) -> Self {
        push_unique(&mut self.missing_fields, field_ref);
        self
    }

    fn with_source_artifact(mut self, artifact_name: &str) -> Self {
        push_unique(&mut self.source_artifact_ids, artifact_name);
        self
    }

    fn combine_and(left: Self, right: Self) -> Self {
        let result = dominant_result_for_all(&[&left, &right]).unwrap_or_else(|| {
            if left.result == CandidateTransitionResult::Matched
                && right.result == CandidateTransitionResult::Matched
            {
                CandidateTransitionResult::Matched
            } else {
                CandidateTransitionResult::NotMatched
            }
        });
        Self::combine_with_result(result, left, right)
    }

    fn combine_or(left: Self, right: Self) -> Self {
        let result = dominant_result_for_all(&[&left, &right]).unwrap_or_else(|| {
            if left.result == CandidateTransitionResult::Matched
                || right.result == CandidateTransitionResult::Matched
            {
                CandidateTransitionResult::Matched
            } else {
                CandidateTransitionResult::NotMatched
            }
        });
        Self::combine_with_result(result, left, right)
    }

    fn combine_with_result(result: CandidateTransitionResult, left: Self, right: Self) -> Self {
        let mut combined = Self::new(
            result,
            left.sanitized_diagnostic
                .clone()
                .or_else(|| right.sanitized_diagnostic.clone()),
        );
        for value in left
            .required_artifacts
            .iter()
            .chain(right.required_artifacts.iter())
        {
            push_unique(&mut combined.required_artifacts, value);
        }
        for value in left
            .missing_artifacts
            .iter()
            .chain(right.missing_artifacts.iter())
        {
            push_unique(&mut combined.missing_artifacts, value);
        }
        for value in left
            .missing_fields
            .iter()
            .chain(right.missing_fields.iter())
        {
            push_unique(&mut combined.missing_fields, value);
        }
        for value in left
            .source_artifact_ids
            .iter()
            .chain(right.source_artifact_ids.iter())
        {
            push_unique(&mut combined.source_artifact_ids, value);
        }
        combined
    }
}

fn dominant_result_for_all(
    evaluations: &[&ClassifiedConditionEvaluation],
) -> Option<CandidateTransitionResult> {
    for result in [
        CandidateTransitionResult::EvaluationError,
        CandidateTransitionResult::InvalidExpression,
        CandidateTransitionResult::MissingInput,
    ] {
        if evaluations
            .iter()
            .any(|evaluation| evaluation.result == result)
        {
            return Some(result);
        }
    }
    None
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn workflow_conflict_operator_label(reason: &WorkflowConflictReason) -> &'static str {
    match reason {
        WorkflowConflictReason::InvalidNextStageHint => "Invalid next-stage advisory hint",
        WorkflowConflictReason::NoDeclarativeTransitionMatched => {
            "No declarative workflow transition matched"
        }
        WorkflowConflictReason::MultipleDeclarativeTransitionsMatchedWithoutTieBreak => {
            "Multiple workflow transitions matched without a tie-break"
        }
        WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition => {
            "Required transition artifact or field is missing"
        }
        WorkflowConflictReason::AggregateTransitionTruthConflicted => {
            "Aggregate transition truth is conflicted"
        }
        WorkflowConflictReason::WorkflowConflictUnverifiable => {
            "Workflow transition outcome is unverifiable"
        }
        WorkflowConflictReason::ImplementationHandoffUnavailable => {
            "Implementation handoff is unavailable"
        }
    }
}

fn initial_workflow_conflict_status(reason: &WorkflowConflictReason) -> WorkflowConflictStatus {
    match reason {
        WorkflowConflictReason::AggregateTransitionTruthConflicted => {
            WorkflowConflictStatus::OperatorConfirmationRequired
        }
        WorkflowConflictReason::WorkflowConflictUnverifiable => {
            WorkflowConflictStatus::TerminalUnverifiable
        }
        _ => WorkflowConflictStatus::Unresolved,
    }
}

fn initial_workflow_conflict_terminal_failure_reason(
    reason: &WorkflowConflictReason,
) -> Option<String> {
    match reason {
        WorkflowConflictReason::WorkflowConflictUnverifiable => Some(
            "Workflow transition outcome could not be verified from declared graph inputs"
                .to_string(),
        ),
        _ => None,
    }
}

fn annotate_aggregate_transition_truth_conflict(
    candidate_evaluations: &mut [CandidateTransitionEvaluation],
    diagnostic: &str,
) {
    for candidate in candidate_evaluations.iter_mut().filter(|candidate| {
        candidate
            .required_artifacts
            .iter()
            .any(|artifact| artifact == "proposal_review_summary")
    }) {
        candidate.result = CandidateTransitionResult::EvaluationError;
        candidate.sanitized_diagnostic = Some(diagnostic.to_string());
    }
}

fn json_value_to_u64(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(number) => number.as_u64(),
        serde_json::Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn json_value_has_entries(value: Option<&serde_json::Value>) -> bool {
    match value {
        Some(serde_json::Value::Array(values)) => !values.is_empty(),
        Some(serde_json::Value::Object(values)) => !values.is_empty(),
        Some(serde_json::Value::String(value)) => !value.trim().is_empty(),
        _ => false,
    }
}

fn json_value_is_empty_collection(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(values) => values.is_empty(),
        serde_json::Value::Object(values) => values.is_empty(),
        serde_json::Value::String(value) => value.trim().is_empty(),
        _ => false,
    }
}

/// Split on a connective keyword, respecting parentheses depth.
fn split_connective<'a>(expr: &'a str, connective: &str) -> Option<(&'a str, &'a str)> {
    let mut depth = 0i32;
    let conn_len = connective.len();
    let bytes = expr.as_bytes();
    let conn_bytes = connective.as_bytes();

    for i in 0..bytes.len() {
        if bytes[i] == b'(' {
            depth += 1;
        } else if bytes[i] == b')' {
            depth -= 1;
        }

        if depth == 0 && i + conn_len <= bytes.len() && &bytes[i..i + conn_len] == conn_bytes {
            let lhs = expr[..i].trim();
            let rhs = expr[i + conn_len..].trim();
            if !lhs.is_empty() && !rhs.is_empty() {
                return Some((lhs, rhs));
            }
        }
    }
    None
}

fn artifact_field_ref(ref_str: &str) -> Option<(&str, &str)> {
    if ref_str.starts_with("vars.") || ref_str.starts_with("approval.") {
        return None;
    }
    let (artifact_name, field_name) = ref_str.split_once('.')?;
    if artifact_name.is_empty() || field_name.is_empty() {
        return None;
    }
    Some((artifact_name, field_name))
}

/// Apply a comparison operator to two JSON values.
fn apply_comparison(lhs: &serde_json::Value, op: CompOp, rhs: &serde_json::Value) -> bool {
    // Try numeric comparison first
    let ln = to_f64(lhs);
    let rn = to_f64(rhs);
    if let (Some(l), Some(r)) = (ln, rn) {
        return match op {
            CompOp::Eq => (l - r).abs() < f64::EPSILON,
            CompOp::Ne => (l - r).abs() >= f64::EPSILON,
            CompOp::Lt => l < r,
            CompOp::Le => l <= r,
            CompOp::Gt => l > r,
            CompOp::Ge => l >= r,
        };
    }

    // String/bool equality
    match op {
        CompOp::Eq => lhs == rhs,
        CompOp::Ne => lhs != rhs,
        _ => false, // non-numeric values can't be ordered
    }
}

fn to_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// Return the effective task list for a state, accounting for post-approval tasks.
/// For manual_gate states with a granted approval, use `post_approval_tasks` when non-empty;
/// otherwise fall back to the regular `tasks` list.
fn effective_tasks<'a>(
    state: &'a workflow::plan::CompiledState,
    is_post_approval: bool,
) -> &'a [workflow::plan::CompiledTask] {
    if is_post_approval && !state.post_approval_tasks.is_empty() {
        &state.post_approval_tasks
    } else {
        &state.tasks
    }
}

fn state_produces_implementation_review(state: &workflow::plan::CompiledState) -> bool {
    state.tasks.iter().any(|task| {
        task.outputs
            .iter()
            .any(|output| output == "implementation_review_summary")
    }) || state.transitions.iter().any(|transition| {
        transition
            .condition
            .contains("implementation_review_summary.")
    })
}

/// P077: Returns true if any of the state's transitions evaluate
/// implementation_closeout_readiness_v1 fields (e.g. `.decision`).
/// Used to detect states that require the closeout synthesis to run before
/// transition evaluation so the active closeout_gate_generations entry exists.
fn state_evaluates_closeout_readiness(state: &workflow::plan::CompiledState) -> bool {
    state.transitions.iter().any(|t| {
        t.condition
            .contains("implementation_closeout_readiness_v1.")
    })
}

fn is_code_writer_implementation_task(task: &workflow::plan::CompiledTask) -> bool {
    task.agent.agent_id == "code_writer"
        && matches!(
            task.task_name.as_str(),
            "start_implementation"
                | "initial_implementation"
                | "continue_implementation"
                | "refine_implementation"
                | "refine_implementation_from_findings"
        )
}

fn is_health_fallback_eligible_task(
    agent_id: &str,
    task_outputs: &[String],
    output_contract: Option<&str>,
) -> bool {
    output_contract == Some("proposal_review_v1")
        || (agent_id == "lead_orchestrator"
            && task_outputs
                .iter()
                .any(|output| output == "proposal_review_summary"))
        || (agent_id == "proposal_writer"
            && task_outputs
                .iter()
                .any(|output| output == "proposal_current"))
        || (agent_id == "docs_guardian" && output_contract == Some("docs_report_v1"))
        || (agent_id == "security_checker" && output_contract == Some("security_report_v1"))
        || (agent_id == "prepush_code_reviewer" && output_contract == Some("prepush_review_v1"))
}

fn is_health_fallback_source_provider(provider: &str) -> bool {
    matches!(
        provider,
        "claude" | "claude_acp" | "gemini" | "gemini_acp" | "codex" | "codex_acp"
    )
}

fn same_provider_family_for_health_fallback(left: &str, right: &str) -> bool {
    matches!(
        (left, right),
        ("claude", "claude")
            | ("claude", "claude_acp")
            | ("claude_acp", "claude")
            | ("claude_acp", "claude_acp")
            | ("gemini", "gemini")
            | ("gemini", "gemini_acp")
            | ("gemini_acp", "gemini")
            | ("gemini_acp", "gemini_acp")
    )
}

fn provider_health_fallback_failure(facts: &domain::agent::AgentExecutionRuntimeFacts) -> bool {
    matches!(
        facts.failure_kind.as_ref(),
        Some(AgentFailureKind::ProviderQuota)
            | Some(AgentFailureKind::MissingRequiredOutputs)
            | Some(AgentFailureKind::ProviderTimeout)
            | Some(AgentFailureKind::TransportClosed)
            | Some(AgentFailureKind::TransportEpipe)
            | Some(AgentFailureKind::TransportProtocolError)
    ) || facts.output_settlement == AgentOutputSettlement::MissingRequiredOutputs
}

fn run_local_health_fallback_profile_candidates(
    agent_id: &str,
    task_outputs: &[String],
    output_contract: Option<&str>,
    source_provider: &str,
) -> Vec<&'static str> {
    if agent_id == "lead_orchestrator"
        && task_outputs
            .iter()
            .any(|output| output == "proposal_review_summary")
    {
        return vec!["codex_writer_high", "codex_architect_high"];
    }
    if agent_id == "proposal_writer"
        && task_outputs
            .iter()
            .any(|output| output == "proposal_current")
    {
        if matches!(source_provider, "codex" | "codex_acp") {
            return vec!["claude_writer_high", "claude_product_high"];
        }
        return vec!["codex_writer_high", "codex_architect_high"];
    }
    if output_contract == Some("proposal_review_v1") {
        if matches!(source_provider, "gemini" | "gemini_acp") {
            let design_reviewer =
                agent_id.contains("ui") || agent_id.contains("ux") || agent_id.contains("macos");
            if design_reviewer {
                return vec![
                    "claude_design_medium",
                    "claude_product_high",
                    "codex_architect_high",
                ];
            }
            return vec!["claude_product_high", "codex_architect_high"];
        }
        if matches!(source_provider, "codex" | "codex_acp") {
            let design_reviewer =
                agent_id.contains("ui") || agent_id.contains("ux") || agent_id.contains("macos");
            if design_reviewer {
                return vec!["claude_design_medium", "claude_product_high"];
            }
            return vec!["claude_product_high", "claude_design_medium"];
        }
        return vec!["codex_architect_high", "codex_writer_high"];
    }
    if agent_id == "docs_guardian" && output_contract == Some("docs_report_v1") {
        if matches!(source_provider, "gemini" | "gemini_acp") {
            return vec![
                "claude_docs_medium",
                "claude_design_medium",
                "codex_architect_high",
            ];
        }
        return vec![
            "gemini_docs_flash",
            "claude_docs_medium",
            "codex_architect_high",
        ];
    }
    if agent_id == "security_checker" && output_contract == Some("security_report_v1") {
        if matches!(source_provider, "claude" | "claude_acp") {
            return vec![
                "codex_architect_high",
                "codex_audit_high",
                "codex_writer_high",
            ];
        }
        return vec!["claude_security_high", "claude_product_high"];
    }
    if agent_id == "prepush_code_reviewer" && output_contract == Some("prepush_review_v1") {
        if matches!(source_provider, "claude" | "claude_acp") {
            return vec!["codex_architect_high", "codex_writer_high"];
        }
        return vec!["claude_prepush_medium", "claude_product_high"];
    }
    Vec::new()
}

fn auto_contract_output_retry_reason(agent_id: &str) -> String {
    format!("auto_contract_output_retry:{agent_id}")
}

fn work_item_matches_agent_execution(
    item: &db::work_item::WorkItem,
    stage_execution_id: &str,
    agent_id: &str,
    agent_execution_id: &str,
) -> bool {
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(&item.payload_json) else {
        return false;
    };
    let claimed_agent_execution_id = payload
        .pointer("/p058_claimed/agent_execution_id")
        .and_then(serde_json::Value::as_str);
    let payload_stage_execution_id = payload
        .get("stage_execution_id")
        .and_then(serde_json::Value::as_str);
    let payload_agent_id = payload.get("agent_id").and_then(serde_json::Value::as_str);
    claimed_agent_execution_id == Some(agent_execution_id)
        || (payload_stage_execution_id == Some(stage_execution_id)
            && payload_agent_id == Some(agent_id))
}

fn is_inline_runtime_input(input_name: &str) -> bool {
    matches!(input_name, "input.idea" | "idea" | "input.file" | "file")
}

fn is_implementation_self_assessment_alias(artifact_name: &str) -> bool {
    matches!(
        artifact_name,
        "implementation_self_assessment_v2" | "implementation_self_assessment"
    )
}

fn extract_json_field(json: &serde_json::Value, field_name: &str) -> Option<serde_json::Value> {
    if let Some(value) = json.get(field_name) {
        return Some(value.clone());
    }

    let mut current = json;
    for part in field_name.split('.') {
        current = current.get(part)?;
    }
    Some(current.clone())
}

/// Check if an InvokeAgent work item's payload belongs to a specific stage execution.
/// Parses the `stage_execution_id` field from the JSON payload and compares it.
fn payload_matches_stage_execution(payload_json: &str, expected_se_id: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(payload_json)
        .ok()
        .and_then(|v| {
            v.get("stage_execution_id")?
                .as_str()
                .map(|s| s == expected_se_id)
        })
        .unwrap_or(false)
}

/// Build prompt for a specific task. Mirrors Swift `RuntimeSessionBridge.buildTaskDirective`:
/// - Agent system prompt
/// - Task name
/// - Input artifacts with resolved filesystem paths
/// - Required outputs with resolved target paths (so the agent writes directly to canonical locations)
/// - Workspace root
/// - Boundaries (no shell redirection into artifact_root; use explicit absolute paths)
/// Returns `true` when the task declares `input.idea` (with or without the
/// `input.` prefix) as one of its inputs. Used to decide whether to inline the
/// raw idea content into the prompt.
fn task_uses_idea_input(task: &workflow::plan::CompiledTask) -> bool {
    task.inputs.iter().any(|i| {
        let name = i.as_str();
        matches!(name, "input.idea" | "idea" | "input.file" | "file")
    })
}

fn build_declared_outputs(
    task: &workflow::plan::CompiledTask,
    plan: &workflow::plan::RunPlan,
    run: &domain::run::Run,
) -> Vec<crate::contracts::DeclaredOutput> {
    task.outputs
        .iter()
        .map(|output_name| {
            let schema = task.output_schemas.get(output_name).cloned();
            let machine_artifact_name = schema
                .as_ref()
                .and_then(|schema| schema.normalized_artifact_name.as_deref())
                .unwrap_or(output_name.as_str());
            let path_artifact_name = if plan.artifact_paths.contains_key(output_name) {
                output_name.as_str()
            } else {
                machine_artifact_name
            };
            let target_path =
                p060_dynamic_review_target_path(output_name, schema.as_ref(), plan, run, task)
                    .unwrap_or_else(|| {
                        resolved_artifact_path_for_task(path_artifact_name, plan, run, task)
                    });
            let companion_output_name = schema
                .as_ref()
                .filter(|schema| {
                    crate::contracts::validation_mode(schema) == "structured_with_human_companion"
                })
                .and_then(|schema| schema.raw_artifact_name.clone());
            let companion_path = companion_output_name
                .as_ref()
                .and_then(|name| plan.artifact_paths.get(name))
                .map(|template| {
                    let resolved = resolve_path_template(
                        template,
                        &run.workspace_root,
                        run.chainworks_meta_root.as_deref(),
                    );
                    let mr_abs = run.chainworks_meta_root.as_ref().map(|mr| {
                        if mr.starts_with('/') {
                            mr.clone()
                        } else {
                            format!("{}/{}", run.workspace_root, mr)
                        }
                    });
                    normalize_path_for_worktree(
                        &resolved,
                        &run.workspace_root,
                        run.worktree_root.as_deref(),
                        task.agent.worktree_write_enabled,
                        mr_abs.as_deref(),
                    )
                });

            crate::contracts::DeclaredOutput {
                output_name: output_name.clone(),
                target_path,
                schema,
                reuse_policy: task.output_policies.get(output_name).map(|policy| {
                    match policy.reuse_policy {
                        workflow::plan::OutputReusePolicy::MustProduce => {
                            domain::discovery::OutputReusePolicy::MustProduce
                        }
                        workflow::plan::OutputReusePolicy::AllowUnchangedExisting => {
                            domain::discovery::OutputReusePolicy::AllowUnchangedExisting
                        }
                    }
                }),
                companion_output_name,
                companion_path,
            }
        })
        .collect()
}

fn dynamic_materialization_epoch(stage: &StageExecution) -> i64 {
    stage
        .iteration
        .saturating_mul(1_000_000)
        .saturating_add(stage.attempt_number)
}

fn p060_dynamic_review_output_name(agent_id: &str) -> String {
    let suffix = agent_id
        .strip_prefix("proposal_reviewer_")
        .unwrap_or(agent_id)
        .replace('-', "_");
    format!("proposal_review_{suffix}")
}

fn p060_dynamic_review_output_schema(contract_id: &str) -> workflow::plan::OutputSchema {
    workflow::plan::OutputSchema {
        contract_id: contract_id.to_string(),
        format: "json".to_string(),
        human_format: None,
        machine_format: Some("json".to_string()),
        validation_mode: Some("strict_structured".to_string()),
        normalized_artifact_name: None,
        raw_artifact_name: None,
        required_fields: vec![
            "agent_id".to_string(),
            "role".to_string(),
            "score".to_string(),
            "decision".to_string(),
            "verdict".to_string(),
            "summary".to_string(),
            "issues".to_string(),
            "blocking_issues".to_string(),
            "non_blocking_issues".to_string(),
            "suggestions".to_string(),
            "assumptions".to_string(),
        ],
    }
}

fn p060_dynamic_review_target_path(
    output_name: &str,
    schema: Option<&workflow::plan::OutputSchema>,
    plan: &workflow::plan::RunPlan,
    run: &domain::run::Run,
    task: &workflow::plan::CompiledTask,
) -> Option<String> {
    if plan.artifact_paths.contains_key(output_name) {
        return None;
    }
    if schema.map(|schema| schema.contract_id.as_str()) != Some("proposal_review_v1") {
        return None;
    }
    let suffix = output_name.strip_prefix("proposal_review_")?;
    if suffix.is_empty() {
        return None;
    }

    let file_stem = suffix.replace('_', "-");
    let template =
        format!("${{CHAINWORKS_META_ROOT:-.chainworks}}/reviews/proposal/{file_stem}.json");
    let resolved = resolve_path_template(
        &template,
        &run.workspace_root,
        run.chainworks_meta_root.as_deref(),
    );
    Some(normalize_resolved_artifact_path_for_task(
        &resolved, run, task,
    ))
}

fn resolved_artifact_path_for_task(
    artifact_name: &str,
    plan: &workflow::plan::RunPlan,
    run: &domain::run::Run,
    task: &workflow::plan::CompiledTask,
) -> String {
    let resolved = plan
        .artifact_paths
        .get(artifact_name)
        .map(|template| {
            resolve_path_template(
                template,
                &run.workspace_root,
                run.chainworks_meta_root.as_deref(),
            )
        })
        .unwrap_or_else(|| format!("{}/{}", run.artifact_root, artifact_name));
    normalize_resolved_artifact_path_for_task(&resolved, run, task)
}

fn normalize_resolved_artifact_path_for_task(
    resolved: &str,
    run: &domain::run::Run,
    task: &workflow::plan::CompiledTask,
) -> String {
    let meta_abs = run.chainworks_meta_root.as_ref().map(|mr| {
        if mr.starts_with('/') {
            mr.clone()
        } else {
            format!("{}/{}", run.workspace_root, mr)
        }
    });
    normalize_path_for_worktree(
        &resolved,
        &run.workspace_root,
        run.worktree_root.as_deref(),
        task.agent.worktree_write_enabled,
        meta_abs.as_deref(),
    )
}

fn build_task_prompt(
    task: &workflow::plan::CompiledTask,
    plan: &workflow::plan::RunPlan,
    run: &domain::run::Run,
    idea: Option<&domain::idea::Idea>,
    source_ctx: Option<&crate::worktree::SourceContext>,
    approval_rejection_context: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    let agent_prompt = task.agent.prompt.as_deref().unwrap_or("").trim();
    if !agent_prompt.is_empty() {
        parts.push(format!("## System Instructions\n{agent_prompt}"));
        parts.push(String::from("---"));
    }

    // Inject resolved skill content (matches Swift RuntimeSessionBridge line 501-505).
    // Position: after system instructions, before task heading.
    if let Some(skill) = &task.agent.resolved_skill {
        if !skill.injected_content.trim().is_empty() {
            parts.push(String::new());
            parts.push(skill.injected_content.clone());
        }
    }

    parts.push(format!("## Task: {}", task.task_name));
    // P050: make the per-run meta root explicit because read-only worktree
    // agents otherwise tend to resolve `.chainworks/runs/...` relative to the
    // implementation worktree.
    let meta_root_abs = run.chainworks_meta_root.as_ref().map(|mr| {
        if mr.starts_with('/') {
            mr.clone()
        } else {
            format!("{}/{}", run.workspace_root, mr)
        }
    });
    if let Some(ref meta_root_abs) = meta_root_abs {
        parts.push(format!("Run meta-root (absolute): {}", meta_root_abs));
    }

    // Proposal 007: write-enabled agents see worktree root as primary path.
    if task.agent.worktree_write_enabled {
        if let Some(ref wt) = run.worktree_root {
            parts.push(format!("Worktree root: {}", wt));
            parts.push(format!(
                "Workspace root (read-only): {}",
                run.workspace_root
            ));
        } else {
            parts.push(format!("Workspace root: {}", run.workspace_root));
        }
    } else if task_reads_implementation_worktree(task) {
        if let Some(ref wt) = run.worktree_root {
            parts.push(format!("Implementation worktree root: {}", wt));
            parts.push(format!(
                "Workspace root (baseline only): {}",
                run.workspace_root
            ));
        } else {
            parts.push(format!("Workspace root: {}", run.workspace_root));
        }
    } else {
        parts.push(format!("Workspace root: {}", run.workspace_root));
    }

    // When the task consumes `input.idea`, inline the idea title + body so the
    // agent actually sees what the user asked for. Otherwise the placeholder
    // line below ("path not defined in catalog") leaves the agent guessing.
    if let Some(idea) = idea {
        if task_uses_idea_input(task) {
            parts.push(String::from("\n### Idea"));
            parts.push(format!("Title: {}", idea.title));
            parts.push(String::from("\nBody:"));
            parts.push(idea.body.clone());
            parts.push(String::from(
                "\nUse this idea (title and body) as the authoritative source for \
                 normalization. When the body references specific files, paths, \
                 proposal names, or artifacts, those references take precedence \
                 over any other candidates found in the workspace.",
            ));
        }
    }

    // Input artifacts with resolved paths.
    // Proposal 007: normalize paths to worktree for write-enabled agents.
    let wt_enabled = task.agent.worktree_write_enabled;
    let wt_root = run.worktree_root.as_deref();
    if !task.inputs.is_empty() {
        parts.push(String::from("\n### Input Artifacts"));
        for input_name in &task.inputs {
            if let Some(template) = plan.artifact_paths.get(input_name) {
                let resolved = resolve_path_template(
                    template,
                    &run.workspace_root,
                    run.chainworks_meta_root.as_deref(),
                );
                let normalized = normalize_path_for_worktree(
                    &resolved,
                    &run.workspace_root,
                    wt_root,
                    wt_enabled,
                    meta_root_abs.as_deref(),
                );
                parts.push(format!("- `{input_name}` → `{normalized}`"));
            } else {
                parts.push(format!("- `{input_name}` (path not defined in catalog)"));
            }
        }
    }

    if let Some(context) = approval_rejection_context {
        parts.push(String::new());
        parts.push(context.to_string());
    }

    // Required outputs with resolved target paths. Agents return these through
    // CHAINWORKS_OUTPUT; the engine validates and materializes canonical files.
    if !task.outputs.is_empty() {
        parts.push(String::from("\n### Required Outputs"));
        parts.push(String::from(
            "Return each required output through the final `CHAINWORKS_OUTPUT` \
             object using the canonical path keys below; the engine will \
             materialize canonical files after contract validation.",
        ));
        parts.push(String::from(
            "Tool stdout is not an output channel. Only the final assistant \
             message is settled for `CHAINWORKS_OUTPUT`. Do not call shell \
             `echo`, `printf`, or file-writing commands to return \
             `CHAINWORKS_OUTPUT`.",
        ));
        for output_name in &task.outputs {
            let normalized = resolved_artifact_path_for_task(output_name, plan, run, task);
            parts.push(format!("- `{output_name}` → `{normalized}`"));
            if let Some(schema) = task.output_schemas.get(output_name) {
                if crate::contracts::validation_mode(schema) == "structured_with_human_companion" {
                    if let Some(raw_name) = schema.raw_artifact_name.as_ref() {
                        let companion_path = plan
                            .artifact_paths
                            .get(raw_name)
                            .map(|template| {
                                resolve_path_template(
                                    template,
                                    &run.workspace_root,
                                    run.chainworks_meta_root.as_deref(),
                                )
                            })
                            .map(|resolved| {
                                normalize_path_for_worktree(
                                    &resolved,
                                    &run.workspace_root,
                                    wt_root,
                                    wt_enabled,
                                    meta_root_abs.as_deref(),
                                )
                            });
                        if let Some(companion_path) = companion_path {
                            parts
                                .push(format!("- `{}` companion → `{}`", raw_name, companion_path));
                        }
                    }
                }
            }
        }
    }

    // Output contracts — schema each output must conform to.
    // Matches Swift RuntimeSessionBridge "Structured Output Requirements" block.
    if !task.output_schemas.is_empty() {
        parts.push(String::from("\n### Structured Output Requirements"));
        parts.push(String::from(
            "CRITICAL: Each required output file must contain exactly one \
             top-level JSON object and nothing else.\n\
             - When returning outputs through `CHAINWORKS_OUTPUT`, the value \
               for each canonical path is treated as that output file content.\n\
             - Tool stdout is not an output channel; only the final assistant \
               message is settled.\n\
             - Do not call shell `echo`, `printf`, or file-writing commands \
               to return `CHAINWORKS_OUTPUT`.\n\
             - Do NOT wrap the JSON in code fences (```​ or ```json).\n\
             - Do NOT emit markdown, prose, or companion files unless they \
               are explicitly listed as required outputs.\n\
             - If you want to explain your work, put the explanation inside \
               JSON fields required by the contract.\n\
             - Every listed field below MUST be present in the JSON, with \
               its correct type.",
        ));
        // Sort for deterministic prompt output
        let mut names: Vec<&String> = task.output_schemas.keys().collect();
        names.sort();
        for output_name in names {
            let schema = &task.output_schemas[output_name];
            parts.push(format!(
                "\n#### `{}` → contract `{}` ({})",
                output_name, schema.contract_id, schema.format
            ));
            if crate::contracts::validation_mode(schema) == "structured_with_human_companion" {
                if let Some(raw_name) = schema.raw_artifact_name.as_deref() {
                    parts.push(format!(
                        "Human companion required: `{}` ({})",
                        raw_name,
                        crate::contracts::human_format(schema).unwrap_or("markdown")
                    ));
                }
            }
            parts.push(String::from("Required fields:"));
            for field in &schema.required_fields {
                parts.push(format!("- `{}`", field));
            }
            if schema.required_fields.iter().any(|field| field == "status") {
                if let Some(allowed_values) = contract_status_allowed_values(&schema.contract_id) {
                    parts.push(format!(
                        "Allowed values for `status`: {}.",
                        allowed_values
                            .iter()
                            .map(|value| format!("`{value}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            if schema.contract_id == "implementation_self_assessment_v2" {
                parts.push(String::from(
                    "Required nested shapes for `implementation_self_assessment_v2`:\n\
                     - `remaining_code_tasks` must be an array of objects: \
                     `{ \"summary\": string, \"owner\": string, \"blocking\": boolean, \"evidence\": string }`.\n\
                     - `handoff_tasks` must be an array of objects: \
                     `{ \"summary\": string, \"owner_class\": one of [\"docs\", \"manual_evidence\", \"release\", \"ops\", \"product\", \"human_operator\", \"unknown\"], \"target_stage\": string, \"blocking_review\": boolean, \"evidence\": string }`.\n\
                     - `known_risks`, `tests_run`, and `docs_impacted` must be arrays of strings.\n\
                     - Do not use strings in `remaining_code_tasks`; do not use booleans in `docs_impacted`; do not invent owner classes such as `docs-agent`, `operator`, `security-owner`, or `release-owner`.",
                ));
            }
        }
    }

    // Boundaries (subset of Swift's buildBoundaryBlock)
    parts.push(String::from("\n### Boundaries"));
    if task.agent.worktree_write_enabled {
        if let Some(ref wt) = run.worktree_root {
            parts.push(format!(
                "- This agent has write access to the worktree root: {wt}\n\
                 - Do NOT write files outside the worktree root.\n\
                 - Read source from the worktree, not the original workspace.\n\
                 - Do not commit, push, or modify git state.\n\
                 - Do not write run artifact outputs directly into the run meta-root; return them through `CHAINWORKS_OUTPUT`.\n\
                 - Do not rely on implicit working directory."
            ));
        } else {
            parts.push(String::from(
                "- Use explicit absolute paths from the workspace root above.\n\
                 - Return required outputs through `CHAINWORKS_OUTPUT`; the engine materializes canonical files.\n\
                 - Do not rely on implicit working directory.\n\
                 - Do not perform git operations unless the task explicitly requests them.",
            ));
        }
    } else {
        if task_reads_implementation_worktree(task) && run.worktree_root.is_some() {
            parts.push(String::from(
                "- Read source from the implementation worktree, not the original workspace.\n\
                 - Treat `Run meta-root (absolute)` as the only valid base for run artifacts; do not use `.chainworks/runs/...` relative to the implementation worktree.\n\
                 - Use meta-root input and output paths exactly as listed above.\n\
                 - Return required outputs through `CHAINWORKS_OUTPUT`; the engine materializes canonical files.\n\
                 - Do not rely on implicit working directory.\n\
                 - Do not perform git operations unless the task explicitly requests them.",
            ));
        } else {
            parts.push(String::from(
                "- Use explicit absolute paths from the workspace root above.\n\
                 - Return required outputs through `CHAINWORKS_OUTPUT`; the engine materializes canonical files.\n\
                 - Do not rely on implicit working directory.\n\
                 - Do not perform git operations unless the task explicitly requests them.",
            ));
        }
    }

    // ── Task-specific guidance (matching Swift RuntimeSessionBridge) ─────
    append_task_specific_guidance(
        &mut parts,
        &task.task_name,
        &task.agent.agent_id,
        run,
        source_ctx,
    );

    parts.join("\n")
}

fn effective_worktree_strategy_for_task(task: &workflow::plan::CompiledTask) -> Option<String> {
    task.agent.worktree_strategy.clone().or_else(|| {
        task_reads_implementation_worktree(task).then_some("shared_implementation_worktree".into())
    })
}

/// P017: Resolve the provider for an agent entry from its backend profile.
fn resolve_agent_provider(
    agent: &workflow::catalog::AgentEntry,
    catalog: &workflow::catalog::AgentCatalogFile,
) -> String {
    catalog
        .backend_profiles
        .as_ref()
        .and_then(|profiles| profiles.get(&agent.backend_profile))
        .map(|bp| bp.provider.clone())
        .unwrap_or_else(|| "claude".to_string())
}

/// P017: Resolve the model for an agent entry from its backend profile.
fn resolve_agent_model(
    agent: &workflow::catalog::AgentEntry,
    catalog: &workflow::catalog::AgentCatalogFile,
) -> Option<String> {
    catalog
        .backend_profiles
        .as_ref()
        .and_then(|profiles| profiles.get(&agent.backend_profile))
        .and_then(|bp| bp.model.clone())
}

/// P017: Resolve the effort for an agent entry from its backend profile.
fn resolve_agent_effort(
    agent: &workflow::catalog::AgentEntry,
    catalog: &workflow::catalog::AgentCatalogFile,
) -> Option<String> {
    catalog
        .backend_profiles
        .as_ref()
        .and_then(|profiles| profiles.get(&agent.backend_profile))
        .and_then(|bp| bp.effort.clone())
}

fn task_reads_implementation_worktree(task: &workflow::plan::CompiledTask) -> bool {
    if task.agent.worktree_write_enabled || task.agent.worktree_strategy.is_some() {
        return false;
    }
    matches!(
        task.agent.agent_id.as_str(),
        "security_checker" | "proposal_implementation_auditor" | "prepush_code_reviewer"
    )
}

/// Build prompt for the owner agent when no explicit tasks are defined.
fn build_task_prompt_for_owner(
    state: &workflow::plan::CompiledState,
    plan: &workflow::plan::RunPlan,
    run: &domain::run::Run,
    idea: Option<&domain::idea::Idea>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    let agent_prompt = state.owner.prompt.as_deref().unwrap_or("").trim();
    if !agent_prompt.is_empty() {
        parts.push(format!("## System Instructions\n{agent_prompt}"));
        parts.push(String::from("---"));
    }

    // Inject resolved skill content for the owner agent.
    if let Some(skill) = &state.owner.resolved_skill {
        if !skill.injected_content.trim().is_empty() {
            parts.push(String::new());
            parts.push(skill.injected_content.clone());
        }
    }

    parts.push(format!("## State: {} — {}", state.id, state.label));
    // Proposal 007: write-enabled owner agents see worktree root.
    if state.owner.worktree_write_enabled {
        if let Some(ref wt) = run.worktree_root {
            parts.push(format!("Worktree root: {}", wt));
            parts.push(format!(
                "Workspace root (read-only): {}",
                run.workspace_root
            ));
        } else {
            parts.push(format!("Workspace root: {}", run.workspace_root));
        }
    } else {
        parts.push(format!("Workspace root: {}", run.workspace_root));
    }

    // Owner-only states (typically the very first state) still need the idea
    // context so the agent knows what the user submitted.
    if let Some(idea) = idea {
        parts.push(String::from("\n### Idea"));
        parts.push(format!("Title: {}", idea.title));
        parts.push(String::from("\nBody:"));
        parts.push(idea.body.clone());
        parts.push(String::from(
            "\nUse this idea (title and body) as the authoritative source. \
             When the body references specific files, paths, proposal names, \
             or artifacts, those references take precedence over any other \
             candidates found in the workspace.",
        ));
    }

    if !plan.artifact_paths.is_empty() {
        let owner_wt_enabled = state.owner.worktree_write_enabled;
        let owner_wt_root = run.worktree_root.as_deref();
        let owner_meta_abs = run.chainworks_meta_root.as_ref().map(|mr| {
            if mr.starts_with('/') {
                mr.clone()
            } else {
                format!("{}/{}", run.workspace_root, mr)
            }
        });
        parts.push(String::from("\n### Available Artifact Paths"));
        parts.push(String::from(
            "Reference these when producing outputs (write to canonical paths):",
        ));
        for (name, template) in plan.artifact_paths.iter().take(15) {
            let resolved = resolve_path_template(
                template,
                &run.workspace_root,
                run.chainworks_meta_root.as_deref(),
            );
            let normalized = normalize_path_for_worktree(
                &resolved,
                &run.workspace_root,
                owner_wt_root,
                owner_wt_enabled,
                owner_meta_abs.as_deref(),
            );
            parts.push(format!("- `{name}` → `{normalized}`"));
        }
        if plan.artifact_paths.len() > 15 {
            parts.push(format!("...and {} more", plan.artifact_paths.len() - 15));
        }
    }

    parts.join("\n")
}

/// Normalize a resolved path for a write-enabled agent: replace workspace_root
/// prefix with worktree_root so the agent reads/writes in the isolated worktree.
/// Matches Swift `RuntimeSessionBridge.normalizedAttachmentContent`.
/// Append task-specific guidance to prompt parts.
/// Matches Swift `RuntimeSessionBridge` task-specific hints for freeze_proposal,
/// initial_implementation, and continue_implementation tasks.
fn append_task_specific_guidance(
    parts: &mut Vec<String>,
    task_name: &str,
    agent_id: &str,
    run: &domain::run::Run,
    source_ctx: Option<&crate::worktree::SourceContext>,
) {
    if task_name == "freeze_proposal_and_provision_worktree" {
        parts.push(String::new());
        parts.push(String::from("### Task-Specific Guidance"));
        parts.push(String::from(
            "The dedicated worktree has already been provisioned by the engine. \
             Do not spend your turn re-provisioning or narrating setup steps.",
        ));
        parts.push(String::from(
            "Freeze `proposal_current` into `approved_proposal` and treat it as \
             the frozen implementation source of truth.",
        ));
        parts.push(String::from(
            "Do not fabricate or emit `approved_proposal` for an applicable proposal \
             unless `proposal_current` already contains a valid strict `rollout_contract_v1`. \
             If it is missing or invalid, surface that proposal refinement is required.",
        ));
        parts.push(String::from(
            "Use `proposal_review_summary` as the implementation gate verdict \
             and planning context.",
        ));
        parts.push(String::from(
            "Treat `run_state` as persisted workflow context, not unquestionable authority. \
             If it contains stale stage identifiers or outdated next-step truth, correct it \
             to match the current workflow before returning it.",
        ));
        parts.push(String::from(
            "Return `implementation_plan`, `implementation_backlog`, and `run_state` \
             together with `approved_proposal` in the final response envelope.",
        ));
        parts.push(String::from(
            "Do not stop after read-only analysis. The task is incomplete until all \
             required implementation-start outputs are present and non-empty.",
        ));
    }

    if agent_id == "code_writer"
        && matches!(
            task_name,
            "start_implementation" | "initial_implementation" | "continue_implementation"
        )
    {
        parts.push(String::new());
        parts.push(String::from("### Task-Specific Guidance"));
        parts.push(String::from(
            "Treat the provided worktree/project roots and canonical input artifact \
             paths as the authoritative starting point for implementation.",
        ));
        if let Some(ref wt) = run.worktree_root {
            parts.push(format!("Implementation worktree: {wt}"));
        }
        parts.push(String::from(
            "Do not re-discover repository structure unless a referenced path is \
             missing or clearly stale.",
        ));
        parts.push(String::from(
            "If a referenced path has drifted, do one brief remap and continue. \
             Do not spend the turn on broad search churn.",
        ));
        parts.push(String::from(
            "Prefer moving directly from the approved plan/backlog into concrete \
             edits and tests instead of repeated search/read passes.",
        ));
        parts.push(String::from(
            "Before any `apply_patch` or edit, re-read the target file from the \
             current worktree path so your patch context matches the live file, \
             not the handoff snapshot.",
        ));
        parts.push(String::from(
            "Keep patches narrow. Prefer small hunks with minimal surrounding \
             context instead of large anchored rewrites.",
        ));
        parts.push(String::from(
            "If `apply_patch` verification fails, do not retry the same patch \
             blindly. Re-read the file, regenerate the hunk against the live \
             contents, and continue with the smallest viable edit.",
        ));
        parts.push(String::from(
            "Do not write run artifact outputs directly into the run meta-root \
             with shell commands. Required outputs must be returned through the \
             final `CHAINWORKS_OUTPUT` JSON object so the engine can validate and \
             materialize them.",
        ));
        parts.push(String::from(
            "Use the exact canonical output paths from Required Outputs as \
             `CHAINWORKS_OUTPUT` keys. Output-name keys are accepted only as \
             fallback when a canonical path is unavailable.",
        ));
        parts.push(String::from(
            "Each `CHAINWORKS_OUTPUT` value must be the full JSON object for that \
             output contract, including every field listed in Structured Output \
             Requirements. For `implementation_self_assessment_v2`, use \
             `implementation_complete`, `verification_green`, \
             `remaining_code_tasks`, `handoff_tasks`, `known_risks`, \
             `tests_run`, and `docs_impacted`; do not use legacy self-assessment \
             field names.",
        ));

        // Source context: changed files manifest (Proposal 007 SourceContextBuilder).
        if let Some(ctx) = source_ctx {
            if !ctx.changed_files.is_empty() {
                parts.push(String::new());
                parts.push(String::from(
                    "### Changed Files (from prior implementation passes)",
                ));
                for f in &ctx.changed_files {
                    parts.push(format!("- {f}"));
                }
                if !ctx.diff_summary.is_empty() {
                    parts.push(String::new());
                    parts.push(format!("Diff summary:\n```\n{}\n```", ctx.diff_summary));
                }
            }
        }
    }
}

/// Normalize a path for worktree agents.
/// P050: meta-root paths (control-plane artifacts) are NOT rewritten into the worktree.
pub fn normalize_path_for_worktree(
    path: &str,
    workspace_root: &str,
    worktree_root: Option<&str>,
    worktree_write_enabled: bool,
    meta_root_absolute: Option<&str>,
) -> String {
    if !worktree_write_enabled {
        return path.to_string();
    }
    // P050: Meta-root paths are control-plane artifacts, not source code.
    // Do not rewrite them into the worktree.
    if let Some(mr) = meta_root_absolute {
        if path.starts_with(mr) {
            return path.to_string();
        }
    }
    let Some(wt) = worktree_root else {
        return path.to_string();
    };
    if wt == workspace_root || wt.is_empty() {
        return path.to_string();
    }
    if path.starts_with(workspace_root) {
        return path.replacen(workspace_root, wt, 1);
    }
    path.to_string()
}

/// Resolve `${VAR:-default}` patterns in artifact path templates.
///
/// P050: When `meta_root` is `Some(val)`, `${CHAINWORKS_META_ROOT:-.chainworks}`
/// resolves to `val` instead of consulting the process env or using the template
/// default. This provides per-run isolation without changing YAML templates.
pub fn resolve_path_template(
    template: &str,
    workspace_root: &str,
    meta_root: Option<&str>,
) -> String {
    let mut result = template.to_string();

    // Resolve ${VAR:-default} patterns
    while let Some(start) = result.find("${") {
        let Some(end) = result[start..].find('}') else {
            break;
        };
        let end = start + end;
        let inner = &result[start + 2..end]; // VAR:-default
        let resolved = if let Some(colon_pos) = inner.find(":-") {
            let var_name = &inner[..colon_pos];
            let default_val = &inner[colon_pos + 2..];
            // P050: For CHAINWORKS_META_ROOT, use per-run override if available.
            // Do NOT consult process env for run artifact resolution.
            if var_name == "CHAINWORKS_META_ROOT" {
                if let Some(mr) = meta_root {
                    mr.to_string()
                } else {
                    // Legacy run (NULL meta_root): use template default
                    default_val.to_string()
                }
            } else {
                std::env::var(var_name).unwrap_or_else(|_| default_val.to_string())
            }
        } else {
            std::env::var(inner).unwrap_or_default()
        };
        result = format!("{}{}{}", &result[..start], resolved, &result[end + 1..]);
    }

    // If the path starts with "." make it relative to workspace_root
    if result.starts_with("./") || result == "." {
        result = format!("{}/{}", workspace_root, result.trim_start_matches("./"));
    } else if !result.starts_with('/') {
        result = format!("{}/{}", workspace_root, result);
    }

    result
}

/// Resolve `${VAR:-default}` patterns for non-path scalar templates.
///
/// Used for values such as git branch names where artifact path normalization
/// would corrupt a valid default like `main` into `<workspace>/main`.
pub fn resolve_scalar_template(template: &str) -> String {
    let mut result = template.to_string();

    while let Some(start) = result.find("${") {
        let Some(end) = result[start..].find('}') else {
            break;
        };
        let end = start + end;
        let inner = &result[start + 2..end];
        let resolved = if let Some(colon_pos) = inner.find(":-") {
            let var_name = &inner[..colon_pos];
            let default_val = &inner[colon_pos + 2..];
            std::env::var(var_name).unwrap_or_else(|_| default_val.to_string())
        } else {
            std::env::var(inner).unwrap_or_default()
        };
        result = format!("{}{}{}", &result[..start], resolved, &result[end + 1..]);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{agent_execution_runtime_facts, ideas, runs, stages};
    use domain::agent::{AgentExecutionRuntimeFacts, AgentFailureKind};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
    use domain::run::{Run, RunStatus};
    use domain::stage::StageStatus;
    use std::collections::HashMap;
    use workflow::plan::{
        CompiledLoop, CompiledState, CompiledTask, CompiledTransition, DegradedOutputPolicy,
        OutputSchema, ResolvedAgent, RunPlan,
    };

    #[test]
    fn approved_proposal_snapshot_path_is_workspace_relative() {
        let stored = Orchestrator::workspace_relative_artifact_path(
            std::path::Path::new(
                "/workspace/.chainworks/runs/run-1/proposals/approved/proposal.md",
            ),
            "/workspace",
        );

        assert_eq!(
            stored.as_deref(),
            Some(".chainworks/runs/run-1/proposals/approved/proposal.md")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn approved_proposal_snapshot_requires_valid_rollout_contract() {
        let pool = test_pool().await;
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let tmp = tempfile::tempdir().unwrap();
        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.workspace_root = tmp.path().to_string_lossy().into_owned();
        run.chainworks_meta_root = Some(format!(".chainworks/runs/{run_id}"));

        let source_rel = format!(".chainworks/runs/{run_id}/proposals/current/proposal.json");
        let source_path = tmp.path().join(&source_rel);
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::write(
            &source_path,
            br#"{"proposal_id":"P999","title":"Missing rollout contract"}"#,
        )
        .unwrap();

        let stage = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "state_5_implementation_ready".into(),
            label: "Implementation ready".into(),
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
        };
        let mut plan = test_plan();
        plan.artifact_paths.insert(
            "approved_proposal".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/proposals/approved/proposal.json".into(),
        );
        let source_artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_4_proposal_reviewed".into(),
            agent_id: "proposal_writer".into(),
            name: "proposal_current".into(),
            contract_id: "proposal_current".into(),
            format: ArtifactFormat::Json,
            file_path: source_rel,
            checksum_sha256: None,
            size_bytes: None,
            provider: "test".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        };

        let snapshot = orchestrator
            .snapshot_approved_proposal_handoff_artifact(&run, &plan, &stage, &[source_artifact])
            .await
            .unwrap();

        assert!(
            snapshot.is_none(),
            "approved_proposal must not be created without a valid rollout_contract_v1"
        );
        assert!(
            !tmp.path()
                .join(format!(
                    ".chainworks/runs/{run_id}/proposals/approved/proposal.json"
                ))
                .exists(),
            "invalid proposal_current must not be copied into approved_proposal"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn approved_proposal_registration_requires_valid_rollout_contract() {
        let pool = test_pool().await;
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let tmp = tempfile::tempdir().unwrap();
        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.workspace_root = tmp.path().to_string_lossy().into_owned();
        run.chainworks_meta_root = Some(format!(".chainworks/runs/{run_id}"));

        let target_path = tmp.path().join(format!(
            ".chainworks/runs/{run_id}/proposals/approved/proposal.json"
        ));
        std::fs::create_dir_all(target_path.parent().unwrap()).unwrap();
        std::fs::write(
            &target_path,
            br#"{"proposal_id":"P999","title":"Existing invalid approved proposal"}"#,
        )
        .unwrap();

        let stage = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "state_5_implementation_ready".into(),
            label: "Implementation ready".into(),
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
        };
        let mut plan = test_plan();
        plan.artifact_paths.insert(
            "approved_proposal".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/proposals/approved/proposal.json".into(),
        );

        let snapshot = orchestrator
            .snapshot_approved_proposal_handoff_artifact(&run, &plan, &stage, &[])
            .await
            .unwrap();

        assert!(
            snapshot.is_none(),
            "existing approved_proposal file must not be registered without a valid rollout_contract_v1"
        );
    }

    fn test_run(run_id: RunId) -> Run {
        Run {
            id: run_id,
            idea_id: IdeaId::new(),
            status: RunStatus::Running,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/workspace".into(),
            artifact_root: "/artifact-root".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("review".into()),
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
            chainworks_meta_root: Some(format!(".chainworks/runs/{run_id}")),
            review_routing_json: None,
            closeout_readiness_mode: None,
        }
    }

    fn test_plan() -> RunPlan {
        let mut artifact_paths = HashMap::new();
        artifact_paths.insert(
            "proposal_review_po".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/reviews/proposal/product-owner.json".into(),
        );
        RunPlan {
            initial_state: "review".into(),
            states: HashMap::new(),
            variables: HashMap::new(),
            artifact_paths,
            workflow_family: None,
            risk_class: None,
            stack: None,
            legacy_broad_discovery_policy: workflow::plan::LegacyBroadDiscoveryPolicy::Disabled,
            workflow_snapshot_hash: "workflow".into(),
            catalog_snapshot_hash: "catalog".into(),
            workflow_snapshot_json: "{}".into(),
            catalog_snapshot_json: "{}".into(),
            dynamic_candidate_bindings: Vec::new(),
            run_plan_snapshot_format_version: None,
            closeout_readiness_mode: None,
        }
    }

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    fn test_idea(id: IdeaId) -> Idea {
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

    fn reviewer_task() -> CompiledTask {
        let mut output_schemas = HashMap::new();
        output_schemas.insert(
            "proposal_review_po".into(),
            OutputSchema {
                contract_id: "proposal_review_v1".into(),
                format: "json".into(),
                human_format: None,
                machine_format: Some("json".into()),
                validation_mode: Some("strict_structured".into()),
                normalized_artifact_name: Some("proposal_review_normalized".into()),
                raw_artifact_name: Some("proposal_review_raw".into()),
                required_fields: vec!["agent_id".into(), "verdict".into()],
            },
        );

        CompiledTask {
            agent: ResolvedAgent {
                agent_id: "proposal_reviewer_product_owner".into(),
                backend_profile_id: None,
                provider: "claude".into(),
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
                output_contract: Some("proposal_review_v1".into()),
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
            task_name: "review_proposal_as_product_owner".into(),
            inputs: Vec::new(),
            outputs: vec!["proposal_review_po".into()],
            output_policies: HashMap::new(),
            output_schemas,
            parallel: true,
            phase: 0,
            selected_outputs_from: None,
        }
    }

    #[tokio::test]
    async fn run_local_provider_health_fallback_uses_codex_after_claude_output_failure() {
        let pool = test_pool().await;
        let events = crate::event_bus::new_bus(64);
        let orchestrator =
            Orchestrator::new(pool.clone(), events.clone(), WorkQueue::new(pool.clone()));
        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.catalog_snapshot_json = Some(
            serde_json::json!({
                "backend_profiles": {
                    "claude_product_high": {
                        "provider": "claude_acp",
                        "model": "opus",
                        "effort": "high",
                        "max_turns": 14
                    },
                    "codex_architect_high": {
                        "provider": "codex_acp",
                        "model": "gpt-5.5",
                        "effort": "xhigh",
                        "max_turns": 16
                    }
                }
            })
            .to_string(),
        );
        ideas::insert(&pool, &test_idea(run.idea_id)).await.unwrap();
        runs::insert(&pool, &run).await.unwrap();

        let stage_id = StageExecutionId::new();
        let stage = StageExecution {
            id: stage_id,
            run_id,
            stage_id: "review".into(),
            label: "review".into(),
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
        };
        stages::insert(&pool, &stage).await.unwrap();

        let failed_exec_id = AgentExecutionId::new();
        sqlx::query(
            r#"INSERT INTO agent_executions
               (id, stage_execution_id, agent_id, provider, provider_family, model, status,
                started_at, completed_at, owner_kind, owner_id)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'failed', ?7, ?8, 'stage_execution', ?2)"#,
        )
        .bind(failed_exec_id.to_string())
        .bind(stage_id.to_string())
        .bind("proposal_reviewer_product_owner")
        .bind("claude_acp")
        .bind("claude")
        .bind("opus")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        let mut facts = AgentExecutionRuntimeFacts::defaults_for(failed_exec_id, Utc::now());
        facts.failure_kind = Some(AgentFailureKind::ProviderQuota);
        facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
        agent_execution_runtime_facts::upsert(&pool, &facts)
            .await
            .unwrap();

        let task = reviewer_task();
        let mut agent = task.agent.clone();
        agent.backend_profile_id = Some("claude_product_high".into());
        agent.provider = "claude_acp".into();
        agent.model = Some("opus".into());
        let output_contract = agent.output_contract.clone();

        let fallback = orchestrator
            .apply_run_local_provider_health_fallback(
                run_id,
                &run,
                &mut agent,
                &task.outputs,
                output_contract.as_deref(),
            )
            .await
            .unwrap()
            .expect("provider health fallback should apply");

        assert_eq!(agent.provider, "codex_acp");
        assert_eq!(
            agent.backend_profile_id.as_deref(),
            Some("codex_architect_high")
        );
        assert_eq!(
            fallback["reason"],
            serde_json::json!("prior_run_local_provider_output_failure")
        );
        assert_eq!(
            fallback["failed_agent_execution_id"],
            serde_json::json!(failed_exec_id.to_string())
        );
    }

    #[tokio::test]
    async fn run_local_provider_health_fallback_uses_design_profile_after_gemini_ui_failure() {
        let pool = test_pool().await;
        let events = crate::event_bus::new_bus(64);
        let orchestrator =
            Orchestrator::new(pool.clone(), events.clone(), WorkQueue::new(pool.clone()));
        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.catalog_snapshot_json = Some(
            serde_json::json!({
                "backend_profiles": {
                    "gemini_review_pro": {
                        "provider": "gemini_acp",
                        "model": "gemini-2.5-pro",
                        "effort": "medium",
                        "max_turns": 12
                    },
                    "claude_design_medium": {
                        "provider": "claude_acp",
                        "model": "opus",
                        "effort": "medium",
                        "max_turns": 12
                    },
                    "codex_architect_high": {
                        "provider": "codex_acp",
                        "model": "gpt-5.5",
                        "effort": "xhigh",
                        "max_turns": 16
                    }
                }
            })
            .to_string(),
        );
        ideas::insert(&pool, &test_idea(run.idea_id)).await.unwrap();
        runs::insert(&pool, &run).await.unwrap();

        let stage_id = StageExecutionId::new();
        let stage = StageExecution {
            id: stage_id,
            run_id,
            stage_id: "review".into(),
            label: "review".into(),
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
        };
        stages::insert(&pool, &stage).await.unwrap();

        let failed_exec_id = AgentExecutionId::new();
        sqlx::query(
            r#"INSERT INTO agent_executions
               (id, stage_execution_id, agent_id, provider, provider_family, model, status,
                started_at, completed_at, owner_kind, owner_id)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'failed', ?7, ?8, 'stage_execution', ?2)"#,
        )
        .bind(failed_exec_id.to_string())
        .bind(stage_id.to_string())
        .bind("proposal_reviewer_ui")
        .bind("gemini_acp")
        .bind("gemini")
        .bind("gemini-2.5-pro")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        let mut facts = AgentExecutionRuntimeFacts::defaults_for(failed_exec_id, Utc::now());
        facts.failure_kind = Some(AgentFailureKind::MissingRequiredOutputs);
        facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
        agent_execution_runtime_facts::upsert(&pool, &facts)
            .await
            .unwrap();

        let mut task = reviewer_task();
        task.agent.agent_id = "proposal_reviewer_ui".into();
        task.agent.backend_profile_id = Some("gemini_review_pro".into());
        task.agent.provider = "gemini_acp".into();
        task.agent.model = Some("gemini-2.5-pro".into());
        let output_contract = task.agent.output_contract.clone();

        let fallback = orchestrator
            .apply_run_local_provider_health_fallback(
                run_id,
                &run,
                &mut task.agent,
                &task.outputs,
                output_contract.as_deref(),
            )
            .await
            .unwrap()
            .expect("provider health fallback should apply");

        assert_eq!(task.agent.provider, "claude_acp");
        assert_eq!(
            task.agent.backend_profile_id.as_deref(),
            Some("claude_design_medium")
        );
        assert_eq!(fallback["from_provider"], serde_json::json!("gemini_acp"));
        assert_eq!(fallback["to_provider"], serde_json::json!("claude_acp"));
    }

    #[tokio::test]
    async fn auto_contract_output_retry_schedules_targeted_fallback_before_stage_blocks() {
        let pool = test_pool().await;
        let events = crate::event_bus::new_bus(64);
        let orchestrator =
            Orchestrator::new(pool.clone(), events.clone(), WorkQueue::new(pool.clone()));
        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.catalog_snapshot_json = Some(
            serde_json::json!({
                "backend_profiles": {
                    "claude_product_high": {
                        "provider": "claude_acp",
                        "model": "sonnet",
                        "effort": "medium",
                        "max_turns": 12
                    },
                    "codex_architect_high": {
                        "provider": "codex_acp",
                        "model": "gpt-5.5",
                        "effort": "xhigh",
                        "max_turns": 16
                    }
                }
            })
            .to_string(),
        );
        ideas::insert(&pool, &test_idea(run.idea_id)).await.unwrap();
        runs::insert(&pool, &run).await.unwrap();

        let stage_id = StageExecutionId::new();
        let stage = StageExecution {
            id: stage_id,
            run_id,
            stage_id: "review".into(),
            label: "review".into(),
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
        };
        stages::insert(&pool, &stage).await.unwrap();

        let failed_exec_id = AgentExecutionId::new();
        sqlx::query(
            r#"INSERT INTO agent_executions
               (id, stage_execution_id, agent_id, provider, provider_family, model, status,
                started_at, completed_at, owner_kind, owner_id)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'failed', ?7, ?8, 'stage_execution', ?2)"#,
        )
        .bind(failed_exec_id.to_string())
        .bind(stage_id.to_string())
        .bind("proposal_reviewer_product_owner")
        .bind("claude_acp")
        .bind("claude")
        .bind("sonnet")
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        let mut facts = AgentExecutionRuntimeFacts::defaults_for(failed_exec_id, Utc::now());
        facts.failure_kind = Some(AgentFailureKind::MissingRequiredOutputs);
        facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;
        agent_execution_runtime_facts::upsert(&pool, &facts)
            .await
            .unwrap();

        db::repos::work_items::enqueue(
            &pool,
            &db::work_item::WorkItem {
                id: format!("p058-invoke:{stage_id}:0"),
                kind: db::work_item::WorkItemKind::InvokeAgent,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "stage_id": "review",
                    "stage_execution_id": stage_id.to_string(),
                    "agent_id": "proposal_reviewer_product_owner",
                    "provider": "claude_acp",
                    "backend_profile_id": "claude_product_high",
                    "model": "sonnet",
                    "effort": "medium",
                    "max_turns": 12,
                    "output_contract": "proposal_review_v1",
                    "task_outputs": ["proposal_review_po"],
                    "p058_claimed": {
                        "agent_execution_id": failed_exec_id.to_string()
                    }
                })
                .to_string(),
                status: db::work_item::WorkItemStatus::Completed,
                run_id: Some(run_id),
                stage_id: Some("review".into()),
                created_at: Utc::now(),
                scheduled_at: Utc::now(),
                attempt_count: 1,
                last_error: None,
            },
        )
        .await
        .unwrap();

        let scheduled = orchestrator
            .schedule_auto_contract_output_retry_for_stage(run_id, &run, &stage)
            .await
            .unwrap();

        assert!(scheduled);
        let stages = stages::list_by_run(&pool, run_id).await.unwrap();
        let retry_stage = stages
            .iter()
            .find(|candidate| {
                candidate.retry_reason.as_deref().is_some_and(|reason| {
                    reason == "auto_contract_output_retry:proposal_reviewer_product_owner"
                })
            })
            .expect("auto retry stage should be created");
        assert_eq!(retry_stage.status, StageStatus::Running);
        let old_stage = stages
            .iter()
            .find(|candidate| candidate.id == stage_id)
            .unwrap();
        assert_eq!(old_stage.status, StageStatus::Skipped);

        let retry_invokes: Vec<_> = db::repos::work_items::list_by_run(&pool, run_id)
            .await
            .unwrap()
            .into_iter()
            .filter(|item| {
                item.kind == db::work_item::WorkItemKind::InvokeAgent
                    && item.status == db::work_item::WorkItemStatus::Pending
            })
            .collect();
        assert_eq!(retry_invokes.len(), 1);
        let payload: serde_json::Value =
            serde_json::from_str(&retry_invokes[0].payload_json).unwrap();
        assert_eq!(payload["provider"], serde_json::json!("codex_acp"));
        assert_eq!(
            payload["backend_profile_id"],
            serde_json::json!("codex_architect_high")
        );
        assert_eq!(
            payload.pointer("/targeted_retry/provider_fallback/reason"),
            Some(&serde_json::json!("source_contract_outputs_missing"))
        );
    }

    fn compiled_state(
        id: &str,
        transitions: Vec<CompiledTransition>,
        is_end: bool,
    ) -> CompiledState {
        let task = reviewer_task();
        CompiledState {
            id: id.into(),
            label: id.into(),
            state_type: None,
            owner: task.agent.clone(),
            is_manual_gate: false,
            is_end,
            tasks: vec![task],
            post_approval_tasks: Vec::new(),
            transitions,
            loop_config: None,
            degraded_output_policy: DegradedOutputPolicy::default(),
            dynamic_parallel: None,
            system_task: None,
        }
    }

    fn implementation_security_task() -> CompiledTask {
        let mut task = reviewer_task();
        task.agent.agent_id = "security_checker".into();
        task.agent.permission_profile = Some("RO_VERIFY".into());
        task.agent.output_contract = Some("security_report_v1".into());
        task.task_name = "check_implementation_security".into();
        task.inputs = vec!["approved_proposal".into(), "changed_files_manifest".into()];
        task.outputs = vec!["security_report".into()];
        task.output_schemas.clear();
        task
    }

    fn code_writer_task(task_name: &str) -> CompiledTask {
        let mut task = reviewer_task();
        task.agent.agent_id = "code_writer".into();
        task.agent.worktree_write_enabled = true;
        task.agent.worktree_strategy = Some("shared_implementation_worktree".into());
        task.task_name = task_name.into();
        task.inputs = vec!["approved_proposal".into()];
        task.outputs = vec!["implementation_summary".into()];
        task.output_schemas.clear();
        task
    }

    #[test]
    fn p084_run_start_guard_covers_initial_and_refinement_code_writer_tasks() {
        for task_name in [
            "start_implementation",
            "initial_implementation",
            "continue_implementation",
            "refine_implementation",
            "refine_implementation_from_findings",
        ] {
            assert!(
                is_code_writer_implementation_task(&code_writer_task(task_name)),
                "P084 rollout preflight must cover code_writer task {task_name}"
            );
        }
    }

    #[test]
    fn implementation_review_readonly_tasks_use_implementation_worktree_strategy() {
        let task = implementation_security_task();

        assert_eq!(
            effective_worktree_strategy_for_task(&task).as_deref(),
            Some("shared_implementation_worktree"),
            "read-only implementation review agents must inspect the implementation worktree"
        );
    }

    #[test]
    fn p060_dynamic_review_outputs_are_unique_but_keep_proposal_review_contract() {
        let output_name = p060_dynamic_review_output_name("proposal_reviewer_rust_architect");
        assert_eq!(output_name, "proposal_review_rust_architect");

        let schema = p060_dynamic_review_output_schema("proposal_review_v1");
        assert_eq!(schema.contract_id, "proposal_review_v1");
        assert_eq!(schema.validation_mode.as_deref(), Some("strict_structured"));
        assert!(
            schema.normalized_artifact_name.is_none(),
            "dynamic reviewer outputs must not collapse to the shared proposal_review_normalized path"
        );
        assert!(schema
            .required_fields
            .iter()
            .any(|field| field == "verdict"));
    }

    #[test]
    fn p060_dynamic_materialization_epoch_changes_on_loop_reentry() {
        let run_id = RunId::new();
        let mut first_review = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "state_4_proposal_reviewed".into(),
            label: "Proposal reviewed".into(),
            status: StageStatus::Running,
            iteration: 8,
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
        };
        let first_epoch = dynamic_materialization_epoch(&first_review);

        first_review.id = StageExecutionId::new();
        first_review.iteration = 10;
        first_review.attempt_number = 1;

        assert_ne!(
            first_epoch,
            dynamic_materialization_epoch(&first_review),
            "re-entering the same dynamic review stage after proposal refinement must not reuse the prior materialization idempotency key"
        );
    }

    #[test]
    fn implementation_summary_orchestrator_does_not_switch_to_worktree() {
        let mut task = reviewer_task();
        task.agent.agent_id = "lead_orchestrator".into();
        task.task_name = "aggregate_implementation_reviews".into();
        task.inputs = vec![
            "security_report".into(),
            "docs_report".into(),
            "audit_report".into(),
            "prepush_review_report".into(),
            "implementation_review_summary".into(),
        ];

        assert_eq!(
            effective_worktree_strategy_for_task(&task),
            None,
            "orchestrator summary tasks read artifacts but do not inspect implementation source"
        );
    }

    #[test]
    fn implementation_review_prompt_points_source_reads_at_worktree() {
        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.worktree_root = Some("/workspace/.chainworks/worktrees/implementation".into());
        let mut plan = test_plan();
        plan.artifact_paths.insert(
            "approved_proposal".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/proposals/approved/proposal.md".into(),
        );
        plan.artifact_paths.insert(
            "changed_files_manifest".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/implementation/changed-files.json".into(),
        );
        let task = implementation_security_task();

        let prompt = build_task_prompt(&task, &plan, &run, None, None, None);

        assert!(prompt.contains(
            "Implementation worktree root: /workspace/.chainworks/worktrees/implementation"
        ));
        assert!(prompt.contains(&format!(
            "Run meta-root (absolute): /workspace/.chainworks/runs/{run_id}"
        )));
        assert!(prompt.contains("Read source from the implementation worktree"));
        assert!(prompt
            .contains("do not use `.chainworks/runs/...` relative to the implementation worktree"));
        assert!(prompt.contains(&format!(
            "/workspace/.chainworks/runs/{run_id}/implementation/changed-files.json"
        )));
    }

    #[test]
    fn code_writer_prompt_uses_envelope_output_contract_not_direct_meta_writes() {
        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.worktree_root = Some("/workspace/.chainworks/worktrees/implementation".into());
        let mut plan = test_plan();
        plan.artifact_paths.insert(
            "implementation_progress".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/implementation/progress.json".into(),
        );
        plan.artifact_paths.insert(
            "implementation_self_assessment".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/implementation/self-assessment.json".into(),
        );
        plan.artifact_paths.insert(
            "tests_result".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/implementation/tests-result.json".into(),
        );
        let mut task = reviewer_task();
        task.agent.agent_id = "code_writer".into();
        task.agent.worktree_write_enabled = true;
        task.agent.output_contract = Some("implementation_self_assessment_v2".into());
        task.task_name = "continue_implementation".into();
        task.outputs = vec![
            "implementation_progress".into(),
            "implementation_self_assessment".into(),
            "tests_result".into(),
        ];
        task.output_schemas.clear();
        task.output_schemas.insert(
            "implementation_progress".into(),
            OutputSchema {
                contract_id: "implementation_progress".into(),
                format: "json".into(),
                human_format: None,
                machine_format: Some("json".into()),
                validation_mode: Some("strict_structured".into()),
                normalized_artifact_name: None,
                raw_artifact_name: None,
                required_fields: vec![
                    "status".into(),
                    "current_phase".into(),
                    "completed_items".into(),
                    "deferred_items".into(),
                    "notes".into(),
                ],
            },
        );
        task.output_schemas.insert(
            "implementation_self_assessment".into(),
            OutputSchema {
                contract_id: "implementation_self_assessment_v2".into(),
                format: "json".into(),
                human_format: None,
                machine_format: Some("json".into()),
                validation_mode: Some("strict_structured".into()),
                normalized_artifact_name: None,
                raw_artifact_name: None,
                required_fields: vec![
                    "status".into(),
                    "implementation_complete".into(),
                    "verification_green".into(),
                    "remaining_code_tasks".into(),
                    "handoff_tasks".into(),
                    "known_risks".into(),
                    "tests_run".into(),
                    "docs_impacted".into(),
                ],
            },
        );
        task.output_schemas.insert(
            "tests_result".into(),
            OutputSchema {
                contract_id: "tests_result_v1".into(),
                format: "json".into(),
                human_format: None,
                machine_format: Some("json".into()),
                validation_mode: Some("strict_structured".into()),
                normalized_artifact_name: None,
                raw_artifact_name: None,
                required_fields: vec!["status".into(), "summary".into()],
            },
        );

        let prompt = build_task_prompt(&task, &plan, &run, None, None, None);

        assert!(
            prompt.contains("Return each required output through the final `CHAINWORKS_OUTPUT`")
        );
        assert!(prompt.contains("the engine will materialize canonical files"));
        assert!(prompt.contains("Tool stdout is not an output channel"));
        assert!(prompt.contains("Only the final assistant message is settled"));
        assert!(prompt.contains("Do not call shell `echo`"));
        assert!(prompt.contains("Do not write run artifact outputs directly"));
        assert!(prompt.contains("implementation_complete"));
        assert!(prompt.contains("remaining_code_tasks"));
        assert!(!prompt.contains("Write each output to its canonical path below"));
        assert!(!prompt
            .contains("Write required outputs to the canonical paths listed in Required Outputs"));
        assert!(!prompt.contains("<canonical path from Required Outputs>"));
        assert!(!prompt.contains("seemingly_complete"));
        assert!(!prompt.contains("remaining_tasks"));
    }

    #[test]
    fn declared_output_target_path_uses_task_alias_not_normalized_identity() {
        let run_id = RunId::new();
        let run = test_run(run_id);
        let plan = test_plan();
        let task = reviewer_task();

        let declared_outputs = build_declared_outputs(&task, &plan, &run);

        assert_eq!(declared_outputs.len(), 1);
        let declared = &declared_outputs[0];
        assert_eq!(declared.output_name, "proposal_review_po");
        assert_eq!(
            declared
                .schema
                .as_ref()
                .and_then(|schema| schema.normalized_artifact_name.as_deref()),
            Some("proposal_review_normalized")
        );
        assert_eq!(
            declared.target_path,
            format!("/workspace/.chainworks/runs/{run_id}/reviews/proposal/product-owner.json")
        );
    }

    #[test]
    fn p060_dynamic_product_owner_output_uses_legacy_artifact_path() {
        let run_id = RunId::new();
        let run = test_run(run_id);
        let plan = test_plan();
        let mut task = reviewer_task();
        let output_name = p060_dynamic_review_output_name("proposal_reviewer_product_owner");
        task.outputs = vec![output_name.clone()];
        task.output_schemas.clear();
        task.output_schemas.insert(
            output_name.clone(),
            p060_dynamic_review_output_schema("proposal_review_v1"),
        );

        let declared_outputs = build_declared_outputs(&task, &plan, &run);

        assert_eq!(declared_outputs.len(), 1);
        let declared = &declared_outputs[0];
        assert_eq!(declared.output_name, "proposal_review_product_owner");
        assert_eq!(
            declared.target_path,
            format!("/workspace/.chainworks/runs/{run_id}/reviews/proposal/product-owner.json"),
            "dynamic P060 product-owner reviews must validate the artifact path written by the existing reviewer catalog"
        );
    }

    #[test]
    fn implementation_review_prompts_include_allowed_status_enums_for_each_role() {
        let run_id = RunId::new();
        let run = test_run(run_id);
        let mut plan = test_plan();
        let fixtures = [
            (
                "proposal_implementation_auditor",
                "audit_report",
                "audit_report_v1",
                vec!["implemented", "needs_code_fixes", "invalid", "unknown"],
            ),
            (
                "security_checker",
                "security_report",
                "security_report_v1",
                vec!["pass", "block", "invalid", "unknown"],
            ),
            (
                "docs_guardian",
                "docs_report",
                "docs_report_v1",
                vec!["pass", "not_needed", "block", "invalid", "unknown"],
            ),
            (
                "prepush_code_reviewer",
                "prepush_review_report",
                "prepush_review_v1",
                vec!["pass", "block", "invalid", "unknown"],
            ),
            (
                "lead_orchestrator",
                "implementation_review_summary",
                "implementation_review_summary_v1",
                vec![
                    "code_complete",
                    "needs_code_fixes",
                    "release_evidence_blocked",
                    "invalid",
                ],
            ),
            (
                "code_writer",
                "implementation_self_assessment",
                "implementation_self_assessment_v2",
                vec![
                    "complete",
                    "needs_code_fixes",
                    "blocked",
                    "handoff_required",
                    "unknown",
                    "invalid",
                ],
            ),
        ];

        for (agent_id, output_name, contract_id, allowed_values) in fixtures {
            plan.artifact_paths.insert(
                output_name.to_string(),
                format!("${{CHAINWORKS_META_ROOT:-.chainworks}}/{output_name}.json"),
            );
            let mut task = reviewer_task();
            task.agent.agent_id = agent_id.to_string();
            task.agent.output_contract = Some(contract_id.to_string());
            task.outputs = vec![output_name.to_string()];
            task.output_schemas.clear();
            task.output_schemas.insert(
                output_name.to_string(),
                OutputSchema {
                    contract_id: contract_id.to_string(),
                    format: "json".to_string(),
                    human_format: None,
                    machine_format: Some("json".to_string()),
                    validation_mode: Some("strict_structured".to_string()),
                    normalized_artifact_name: Some(output_name.to_string()),
                    raw_artifact_name: None,
                    required_fields: vec!["status".to_string()],
                },
            );

            let prompt = build_task_prompt(&task, &plan, &run, None, None, None);
            assert!(
                prompt.contains("Allowed values for `status`:"),
                "{agent_id} prompt should state allowed status values"
            );
            for allowed in allowed_values {
                assert!(
                    prompt.contains(&format!("`{allowed}`")),
                    "{agent_id} prompt should include canonical status `{allowed}`"
                );
            }
        }
    }

    #[test]
    fn scalar_template_resolution_does_not_path_normalize_branch_names() {
        assert_eq!(
            resolve_scalar_template("${CHAINWORKS_BASE_BRANCH:-main}"),
            "main"
        );
        assert_eq!(
            resolve_scalar_template("release/candidate"),
            "release/candidate"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn proposal_057_controlled_artifact_does_not_fall_back_to_raw_file_truth() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let tmp = tempfile::tempdir().unwrap();
        let run_id = RunId::new();
        let idea_id = IdeaId::new();
        db::repos::ideas::insert(
            &pool,
            &domain::idea::Idea {
                id: idea_id,
                title: "Idea".into(),
                body: "Body".into(),
                workspace_root_path: None,
                project_key: None,
                status: domain::idea::IdeaStatus::Active,
                created_at: Utc::now(),
                archived_at: None,
            },
        )
        .await
        .unwrap();
        let mut run = test_run(run_id);
        run.idea_id = idea_id;
        run.workspace_root = tmp.path().to_string_lossy().into_owned();
        run.chainworks_meta_root = Some(tmp.path().to_string_lossy().into_owned());
        db::repos::runs::insert(&pool, &run).await.unwrap();
        let raw_path = tmp.path().join("review/prepush.json");
        std::fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
        std::fs::write(&raw_path, r#"{"status":"pass"}"#).unwrap();

        let mut plan = test_plan();
        plan.artifact_paths.insert(
            "prepush_review_report".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/review/prepush.json".into(),
        );

        let missing = orchestrator
            .resolve_value("prepush_review_report.status", &run, &plan)
            .await;
        assert_eq!(
            missing,
            serde_json::Value::Null,
            "P057-controlled aliases must fail closed when SQLite has no active contract"
        );

        db::repos::artifact_contracts::upsert_generation_and_rebuild(
            &pool,
            domain::artifact_contracts::ActiveArtifactGenerationInput {
                run_id,
                artifact_id: domain::ids::ArtifactId::new(),
                contract_id: "prepush_review_v1".into(),
                canonical_path: "review/prepush.json".into(),
                raw_path: "review/prepush.json".into(),
                raw_status: "PASS_WITH_NOTES".into(),
                generation_id: "gen-1".into(),
                source_agent_execution_id: None,
                source_stage_execution_id: None,
                source_session_generation_id: None,
                source_work_item_id: None,
                supersedes_generation_id: None,
                output_settlement: domain::agent::AgentOutputSettlement::None,
                partial: false,
                warnings: vec![],
            },
        )
        .await
        .unwrap();
        let canonical = orchestrator
            .resolve_value("prepush_review_report.status", &run, &plan)
            .await;
        assert_eq!(canonical, serde_json::json!("pass"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn proposal_057_controlled_exists_uses_active_contract_truth_not_raw_files() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let tmp = tempfile::tempdir().unwrap();
        let run_id = RunId::new();
        let idea_id = IdeaId::new();
        db::repos::ideas::insert(
            &pool,
            &domain::idea::Idea {
                id: idea_id,
                title: "Idea".into(),
                body: "Body".into(),
                workspace_root_path: None,
                project_key: None,
                status: domain::idea::IdeaStatus::Active,
                created_at: Utc::now(),
                archived_at: None,
            },
        )
        .await
        .unwrap();
        let mut run = test_run(run_id);
        run.idea_id = idea_id;
        run.workspace_root = tmp.path().to_string_lossy().into_owned();
        run.chainworks_meta_root = Some(tmp.path().to_string_lossy().into_owned());
        db::repos::runs::insert(&pool, &run).await.unwrap();

        let raw_path = tmp.path().join("review/prepush.json");
        std::fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
        std::fs::write(&raw_path, r#"{"status":"pass"}"#).unwrap();

        let mut plan = test_plan();
        plan.artifact_paths.insert(
            "prepush_review_report".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/review/prepush.json".into(),
        );

        let missing = orchestrator
            .evaluate_condition_classified(
                "exists('prepush_review_report')",
                &run,
                &plan,
                "state_8_implementation",
            )
            .await;
        assert_eq!(
            missing.result,
            CandidateTransitionResult::MissingInput,
            "P057-controlled exists() must fail closed when only raw file truth exists"
        );

        db::repos::artifact_contracts::upsert_generation_and_rebuild(
            &pool,
            domain::artifact_contracts::ActiveArtifactGenerationInput {
                run_id,
                artifact_id: domain::ids::ArtifactId::new(),
                contract_id: "prepush_review_v1".into(),
                canonical_path: "review/prepush.json".into(),
                raw_path: "review/prepush.json".into(),
                raw_status: "PASS_WITH_NOTES".into(),
                generation_id: "gen-exists".into(),
                source_agent_execution_id: None,
                source_stage_execution_id: None,
                source_session_generation_id: None,
                source_work_item_id: None,
                supersedes_generation_id: None,
                output_settlement: domain::agent::AgentOutputSettlement::None,
                partial: false,
                warnings: vec![],
            },
        )
        .await
        .unwrap();

        let resolved = orchestrator
            .evaluate_condition_classified(
                "exists('prepush_review_report')",
                &run,
                &plan,
                "state_8_implementation",
            )
            .await;
        assert_eq!(
            resolved.result,
            CandidateTransitionResult::Matched,
            "P057-controlled exists() should read active SQLite contract truth"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn proposal_057_controlled_artifact_fails_closed_without_async_lookup() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool),
        );
        let tmp = tempfile::tempdir().unwrap();
        let mut run = test_run(RunId::new());
        run.workspace_root = tmp.path().to_string_lossy().into_owned();
        run.chainworks_meta_root = Some(tmp.path().to_string_lossy().into_owned());

        let raw_path = tmp.path().join("review/prepush.json");
        std::fs::create_dir_all(raw_path.parent().unwrap()).unwrap();
        std::fs::write(&raw_path, r#"{"status":"pass"}"#).unwrap();

        let mut plan = test_plan();
        plan.artifact_paths.insert(
            "prepush_review_report".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/review/prepush.json".into(),
        );

        assert_eq!(
            orchestrator
                .resolve_value("prepush_review_report.status", &run, &plan)
                .await,
            serde_json::Value::Null
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn proposal_017_exists_unknown_artifact_fails_closed() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool),
        );
        let run = test_run(RunId::new());
        let plan = test_plan();

        let evaluation = orchestrator
            .evaluate_condition_classified("exists('unknown_artifact')", &run, &plan, "review")
            .await;
        assert_eq!(
            evaluation.result,
            CandidateTransitionResult::InvalidExpression,
            "P017 requires unknown catalog artifact references to fail closed"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_stale_failed_stage_reentry_is_ignored() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let now = Utc::now();
        let idea = domain::idea::Idea {
            id: IdeaId::new(),
            title: "Stale failed stage".into(),
            body: "re-entry should ignore old failed stages if a newer stage from another state exists".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        };
        db::repos::ideas::insert(&pool, &idea).await.unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let wf_path = temp_dir.path().join("wf.yaml");
        let cat_path = temp_dir.path().join("catalog.yaml");
        std::fs::write(
            &wf_path,
            "
workflow:
  id: test_wf
initial_state: state_9
states:
  state_9:
    label: Review
    owner: reviewer
    transitions:
      - to: state_10
        when: 'false'
  state_10:
    label: Refine
    owner: refiner
    transitions:
      - to: state_9
        when: 'true'
",
        )
        .unwrap();
        std::fs::write(
            &cat_path,
            "
agents:
  - id: reviewer
    backend_profile: reviewer_profile
    system_role: lead
    permission_profile: default
    lead_resolution_contract: proposal_review_v1
  - id: refiner
    backend_profile: refiner_profile
    permission_profile: default
backend_profiles:
  reviewer_profile:
    provider: codex
  refiner_profile:
    provider: codex
contracts:
  proposal_review_v1:
    format: json
permission_profiles:
  default: {}
",
        )
        .unwrap();

        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.idea_id = idea.id;
        run.current_state = Some("state_9".into());
        run.workflow_yaml_path = Some(wf_path.to_str().unwrap().into());
        run.agent_catalog_yaml_path = Some(cat_path.to_str().unwrap().into());
        db::repos::runs::insert(&pool, &run).await.unwrap();

        // 1. Failed stage for state_9 (old)
        let failed_state_9 = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "state_9".into(),
            label: "Review".into(),
            status: StageStatus::Failed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now - chrono::Duration::minutes(10),
            completed_at: Some(now - chrono::Duration::minutes(9)),
            owner_agent: Some("reviewer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        db::repos::stages::insert(&pool, &failed_state_9)
            .await
            .unwrap();

        // 2. Successful stage for state_10 (more recent)
        let completed_state_10 = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "state_10".into(),
            label: "Refine".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now - chrono::Duration::minutes(5),
            completed_at: Some(now - chrono::Duration::minutes(4)),
            owner_agent: Some("refiner".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        db::repos::stages::insert(&pool, &completed_state_10)
            .await
            .unwrap();

        // Advance workflow
        orchestrator.advance_run(run_id).await.unwrap();

        // Check if a NEW stage for state_9 was created
        let all_stages = db::repos::stages::list_by_run(&pool, run_id).await.unwrap();
        let state_9_stages: Vec<_> = all_stages
            .iter()
            .filter(|s| s.stage_id == "state_9")
            .collect();

        assert_eq!(
            state_9_stages.len(),
            2,
            "Should have created a new stage for state_9 because the old one is stale"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn loop_budget_allows_final_cross_state_review_after_refinement() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let now = Utc::now();
        let idea = domain::idea::Idea {
            id: IdeaId::new(),
            title: "Loop budget".into(),
            body: "final refinement should still be reviewed".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        };
        db::repos::ideas::insert(&pool, &idea).await.unwrap();

        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.idea_id = idea.id;
        run.current_state = Some("refine".into());
        db::repos::runs::insert(&pool, &run).await.unwrap();

        let mut plan = test_plan();
        let mut refine = compiled_state(
            "refine",
            vec![CompiledTransition {
                to: "review".into(),
                condition: "true".into(),
            }],
            false,
        );
        refine.loop_config = Some(CompiledLoop {
            counter: "proposal_revision_count".into(),
            max: 2,
        });
        plan.states.insert("refine".into(), refine);
        plan.states
            .insert("review".into(), compiled_state("review", Vec::new(), false));

        let first = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "refine".into(),
            label: "Refine".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now - chrono::Duration::seconds(10),
            completed_at: Some(now - chrono::Duration::seconds(9)),
            owner_agent: Some("proposal_writer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        let latest = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "refine".into(),
            label: "Refine".into(),
            status: StageStatus::Completed,
            iteration: 2,
            attempt_number: 3,
            settlement_kind: None,
            started_at: now,
            completed_at: Some(now),
            owner_agent: Some("proposal_writer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        db::repos::stages::insert(&pool, &first).await.unwrap();
        db::repos::stages::insert(&pool, &latest).await.unwrap();

        orchestrator
            .evaluate_and_transition(run_id, "refine", &plan, &[first, latest])
            .await
            .unwrap();

        let stored = db::repos::runs::find_by_id(&pool, run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.current_state.as_deref(), Some("review"));
        assert!(
            db::repos::workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
                .await
                .unwrap()
                .is_none(),
            "final allowed refinement must transition to review, not block before review"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn loop_budget_blocks_entering_exhausted_loop_state() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let now = Utc::now();
        let idea = domain::idea::Idea {
            id: IdeaId::new(),
            title: "Loop budget".into(),
            body: "do not enter exhausted loop".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        };
        db::repos::ideas::insert(&pool, &idea).await.unwrap();

        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.idea_id = idea.id;
        run.current_state = Some("review".into());
        db::repos::runs::insert(&pool, &run).await.unwrap();

        let mut plan = test_plan();
        plan.states.insert(
            "review".into(),
            compiled_state(
                "review",
                vec![CompiledTransition {
                    to: "refine".into(),
                    condition: "true".into(),
                }],
                false,
            ),
        );
        let mut refine = compiled_state("refine", Vec::new(), false);
        refine.loop_config = Some(CompiledLoop {
            counter: "proposal_revision_count".into(),
            max: 2,
        });
        plan.states.insert("refine".into(), refine);

        let review = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "review".into(),
            label: "Review".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: Some(now),
            owner_agent: Some("lead_orchestrator".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        let skipped_retry = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "refine".into(),
            label: "Refine".into(),
            status: StageStatus::Skipped,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now - chrono::Duration::seconds(20),
            completed_at: Some(now - chrono::Duration::seconds(19)),
            owner_agent: Some("proposal_writer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: Some("retry superseded prior attempt".into()),
        };
        let first = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "refine".into(),
            label: "Refine".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 2,
            settlement_kind: None,
            started_at: now - chrono::Duration::seconds(18),
            completed_at: Some(now - chrono::Duration::seconds(17)),
            owner_agent: Some("proposal_writer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        let second = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "refine".into(),
            label: "Refine".into(),
            status: StageStatus::Completed,
            iteration: 2,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now - chrono::Duration::seconds(10),
            completed_at: Some(now - chrono::Duration::seconds(9)),
            owner_agent: Some("proposal_writer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        for stage in [&review, &skipped_retry, &first, &second] {
            db::repos::stages::insert(&pool, stage).await.unwrap();
        }

        orchestrator
            .evaluate_and_transition(
                run_id,
                "review",
                &plan,
                &[review, skipped_retry, first, second],
            )
            .await
            .unwrap();

        let stored = db::repos::runs::find_by_id(&pool, run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, RunStatus::Blocked);
        assert_eq!(stored.current_state.as_deref(), Some("review"));
        let conflict = db::repos::workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
            .await
            .unwrap()
            .expect("exhausted loop entry should record a blocking conflict");
        assert_eq!(conflict.candidate_transitions.len(), 1);
        assert_eq!(
            conflict.candidate_transitions[0].result,
            CandidateTransitionResult::NotMatched
        );
        assert!(conflict.candidate_transitions[0]
            .sanitized_diagnostic
            .as_deref()
            .is_some_and(|diagnostic| diagnostic.contains("Loop budget exhausted")));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn proposal_017_orchestrator_persists_no_match_workflow_conflict() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let tmp = tempfile::tempdir().unwrap();
        let now = Utc::now();
        let idea = domain::idea::Idea {
            id: IdeaId::new(),
            title: "P017".into(),
            body: "workflow conflict runtime wiring".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        };
        db::repos::ideas::insert(&pool, &idea).await.unwrap();

        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.idea_id = idea.id;
        run.workspace_root = tmp.path().to_string_lossy().into_owned();
        let meta_root = tmp.path().join(".chainworks/runs").join(run_id.to_string());
        run.chainworks_meta_root = Some(meta_root.to_string_lossy().into_owned());
        let advisory_path = meta_root.join("reviews/proposal/product-owner.json");
        std::fs::create_dir_all(advisory_path.parent().unwrap()).unwrap();
        std::fs::write(
            &advisory_path,
            r#"{"next_stage":"state_3_proposal_drafted","next_action":"revise_proposal"}"#,
        )
        .unwrap();
        db::repos::runs::insert(&pool, &run).await.unwrap();

        let stage = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "review".into(),
            label: "Review".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: Some(now),
            owner_agent: Some("proposal_reviewer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        db::repos::stages::insert(&pool, &stage).await.unwrap();

        let mut plan = test_plan();
        plan.states.insert(
            "review".into(),
            compiled_state(
                "review",
                vec![CompiledTransition {
                    to: "refine".into(),
                    condition: "false".into(),
                }],
                false,
            ),
        );
        plan.states
            .insert("refine".into(), compiled_state("refine", Vec::new(), true));

        orchestrator
            .evaluate_and_transition(run_id, "review", &plan, &[stage.clone()])
            .await
            .unwrap();

        let blocked = db::repos::workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
            .await
            .unwrap()
            .expect("blocking workflow conflict should be persisted");
        assert_eq!(
            blocked.reason,
            WorkflowConflictReason::NoDeclarativeTransitionMatched
        );
        assert_eq!(blocked.current_state_id, "review");
        assert_eq!(blocked.stage_execution_id, Some(stage.id.to_string()));
        assert_eq!(blocked.candidate_transitions.len(), 1);
        assert_eq!(
            blocked.candidate_transitions[0].result,
            CandidateTransitionResult::NotMatched
        );
        assert!(
            blocked
                .advisory_evidence_refs
                .iter()
                .any(|evidence_ref| evidence_ref
                    .starts_with("proposal_review_po:$.next_stage:absent_from_graph:sha256:")),
            "blocking conflicts should retain invalid advisory next_stage provenance"
        );
        assert!(
            blocked
                .advisory_evidence_refs
                .iter()
                .any(|evidence_ref| evidence_ref
                    .starts_with("proposal_review_po:$.next_action:sha256:")),
            "blocking conflicts should retain advisory next_action provenance"
        );
        assert_eq!(
            db::repos::runs::find_by_id(&pool, run_id)
                .await
                .unwrap()
                .unwrap()
                .status,
            RunStatus::Blocked
        );

        orchestrator
            .evaluate_and_transition(run_id, "review", &plan, &[stage])
            .await
            .unwrap();
        let history = db::repos::workflow_conflicts::list_conflict_history_for_run(&pool, run_id)
            .await
            .unwrap();
        assert_eq!(
            history.len(),
            1,
            "repeated blocking evaluations should upsert the same fingerprint"
        );
        assert_eq!(history[0].conflict_id, blocked.conflict_id);
        assert_eq!(
            history[0].advisory_evidence_refs,
            blocked.advisory_evidence_refs
        );
        let cursor = db::repos::workflow_conflicts::get_transition_cursor(&pool, run_id)
            .await
            .unwrap()
            .expect("blocking conflict should anchor transition cursor");
        assert_eq!(cursor.current_state_id, "review");
        assert_eq!(cursor.cursor_status, "awaiting_conflict_resolution");
        assert_eq!(cursor.resume_policy, "await_conflict_resolution");
        assert_eq!(
            cursor.conflict_id.as_deref(),
            Some(blocked.conflict_id.as_str())
        );
        assert_eq!(
            cursor.conflict_fingerprint.as_deref(),
            Some(blocked.conflict_fingerprint.as_str())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn proposal_017_orchestrator_records_non_blocking_advisory_rejection() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let tmp = tempfile::tempdir().unwrap();
        let now = Utc::now();
        let idea = domain::idea::Idea {
            id: IdeaId::new(),
            title: "P017".into(),
            body: "workflow advisory rejection runtime wiring".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        };
        db::repos::ideas::insert(&pool, &idea).await.unwrap();

        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.idea_id = idea.id;
        run.workspace_root = tmp.path().to_string_lossy().into_owned();
        run.chainworks_meta_root = Some(tmp.path().to_string_lossy().into_owned());
        db::repos::runs::insert(&pool, &run).await.unwrap();

        let stage = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "review".into(),
            label: "Review".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: Some(now),
            owner_agent: Some("proposal_reviewer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        db::repos::stages::insert(&pool, &stage).await.unwrap();

        let raw_run_state_path = tmp.path().join("state/run-state.json");
        std::fs::create_dir_all(raw_run_state_path.parent().unwrap()).unwrap();
        std::fs::write(
            &raw_run_state_path,
            r#"{"next_stage":"state_3_proposal_drafted","next_action":"revise_proposal"}"#,
        )
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        db::repos::artifact_contracts::record_run_state_advisory_tx(
            &mut tx,
            domain::artifact_contracts::ActiveArtifactGenerationInput {
                run_id,
                artifact_id: domain::ids::ArtifactId::new(),
                contract_id: "run_state_projection_v1".into(),
                canonical_path: "run_state.json".into(),
                raw_path: raw_run_state_path.to_string_lossy().into_owned(),
                raw_status: "superseded_advisory".into(),
                generation_id: "advisory-run-state".into(),
                source_agent_execution_id: Some("agent-exec-review".into()),
                source_stage_execution_id: Some(stage.id.to_string()),
                source_session_generation_id: None,
                source_work_item_id: None,
                supersedes_generation_id: None,
                output_settlement: domain::agent::AgentOutputSettlement::None,
                partial: false,
                warnings: vec![],
            },
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let mut plan = test_plan();
        plan.states.insert(
            "review".into(),
            compiled_state(
                "review",
                vec![CompiledTransition {
                    to: "refine".into(),
                    condition: "true".into(),
                }],
                false,
            ),
        );
        plan.states
            .insert("refine".into(), compiled_state("refine", Vec::new(), true));

        orchestrator
            .evaluate_and_transition(run_id, "review", &plan, &[stage.clone()])
            .await
            .unwrap();

        let rejections =
            db::repos::workflow_conflicts::list_advisory_rejections_for_run(&pool, run_id)
                .await
                .unwrap();
        assert_eq!(rejections.len(), 1);
        let rejection = &rejections[0];
        assert_eq!(rejection.stage_execution_id, Some(stage.id.to_string()));
        assert_eq!(rejection.selected_next_state_id, "refine");
        assert_eq!(
            rejection.advisory_next_stage_hint.as_deref(),
            Some("state_3_proposal_drafted")
        );
        assert_eq!(
            rejection.advisory_next_action.as_deref(),
            Some("revise_proposal")
        );
        assert_eq!(rejection.graph_membership_result, "absent_from_graph");
        assert!(rejection
            .advisory_hint_provenance
            .iter()
            .any(|hint| hint.advisory_path == "$.next_stage"
                && hint.superseded_by_projection
                && hint.source_agent_execution_id.as_deref() == Some("agent-exec-review")));
        assert!(
            db::repos::workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
                .await
                .unwrap()
                .is_none(),
            "a rejected advisory hint must not block graph-authoritative advancement"
        );
        assert_eq!(
            db::repos::runs::find_by_id(&pool, run_id)
                .await
                .unwrap()
                .unwrap()
                .current_state
                .as_deref(),
            Some("refine")
        );
        let cursor = db::repos::workflow_conflicts::get_transition_cursor(&pool, run_id)
            .await
            .unwrap()
            .expect("legal graph transition should settle through cursor");
        assert_eq!(cursor.current_state_id, "review");
        assert_eq!(cursor.cursor_status, "graph_transition_selected");
        assert_eq!(cursor.resume_policy, "continue_from_selected_transition");
        assert_eq!(cursor.selected_next_state_id.as_deref(), Some("refine"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn proposal_017_orchestrator_blocks_conflicted_proposal_review_summary() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let tmp = tempfile::tempdir().unwrap();
        let now = Utc::now();
        let idea = domain::idea::Idea {
            id: IdeaId::new(),
            title: "P017".into(),
            body: "aggregate transition truth conflict".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        };
        db::repos::ideas::insert(&pool, &idea).await.unwrap();

        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.idea_id = idea.id;
        run.workspace_root = tmp.path().to_string_lossy().into_owned();
        run.chainworks_meta_root = Some(tmp.path().to_string_lossy().into_owned());
        db::repos::runs::insert(&pool, &run).await.unwrap();

        let stage = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "review".into(),
            label: "Review".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: Some(now),
            owner_agent: Some("lead_orchestrator".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        db::repos::stages::insert(&pool, &stage).await.unwrap();

        let summary_path = tmp.path().join("reviews/proposal/summary.json");
        std::fs::create_dir_all(summary_path.parent().unwrap()).unwrap();
        std::fs::write(
            &summary_path,
            r#"{"pass":true,"blocker_count":1,"blocking_issues":[{"id":"B-1"}],"required_changes":["fix authority"],"decision":"pass"}"#,
        )
        .unwrap();

        let mut plan = test_plan();
        plan.artifact_paths.insert(
            "proposal_review_summary".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/reviews/proposal/summary.json".into(),
        );
        plan.states.insert(
            "review".into(),
            compiled_state(
                "review",
                vec![
                    CompiledTransition {
                        to: "approved".into(),
                        condition: "proposal_review_summary.pass == true".into(),
                    },
                    CompiledTransition {
                        to: "refine".into(),
                        condition: "proposal_review_summary.pass == false".into(),
                    },
                ],
                false,
            ),
        );
        plan.states.insert(
            "approved".into(),
            compiled_state("approved", Vec::new(), true),
        );
        plan.states
            .insert("refine".into(), compiled_state("refine", Vec::new(), true));

        orchestrator
            .evaluate_and_transition(run_id, "review", &plan, &[stage])
            .await
            .unwrap();

        let blocked = db::repos::workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
            .await
            .unwrap()
            .expect("conflicted aggregate truth should block graph advancement");
        assert_eq!(
            blocked.reason,
            WorkflowConflictReason::AggregateTransitionTruthConflicted
        );
        assert_eq!(
            blocked.status,
            WorkflowConflictStatus::OperatorConfirmationRequired
        );
        assert!(blocked.candidate_transitions.iter().any(|candidate| {
            candidate.result == CandidateTransitionResult::EvaluationError
                && candidate
                    .sanitized_diagnostic
                    .as_deref()
                    .is_some_and(|diagnostic| diagnostic.contains("pass=true"))
        }));
        let stored_run = db::repos::runs::find_by_id(&pool, run_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored_run.status, RunStatus::Blocked);
        assert_eq!(stored_run.current_state.as_deref(), Some("review"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn proposal_017_orchestrator_persists_terminal_unverifiable_conflict_history() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let now = Utc::now();
        let idea = domain::idea::Idea {
            id: IdeaId::new(),
            title: "P017".into(),
            body: "terminal unverifiable workflow conflict".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        };
        db::repos::ideas::insert(&pool, &idea).await.unwrap();

        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.idea_id = idea.id;
        db::repos::runs::insert(&pool, &run).await.unwrap();

        let stage = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "review".into(),
            label: "Review".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: Some(now),
            owner_agent: Some("proposal_reviewer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        db::repos::stages::insert(&pool, &stage).await.unwrap();

        let mut plan = test_plan();
        plan.states.insert(
            "review".into(),
            compiled_state(
                "review",
                vec![CompiledTransition {
                    to: "refine".into(),
                    condition: "exists('unknown_artifact')".into(),
                }],
                false,
            ),
        );
        plan.states
            .insert("refine".into(), compiled_state("refine", Vec::new(), true));

        orchestrator
            .evaluate_and_transition(run_id, "review", &plan, &[stage])
            .await
            .unwrap();

        assert!(
            db::repos::workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
                .await
                .unwrap()
                .is_none(),
            "terminal_unverifiable conflicts remain history, not current unresolved conflicts"
        );
        let history = db::repos::workflow_conflicts::list_conflict_history_for_run(&pool, run_id)
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(
            history[0].reason,
            WorkflowConflictReason::WorkflowConflictUnverifiable
        );
        assert_eq!(
            history[0].status,
            WorkflowConflictStatus::TerminalUnverifiable
        );
        assert!(history[0].terminal_failure_reason.is_some());
        assert_eq!(
            db::repos::runs::find_by_id(&pool, run_id)
                .await
                .unwrap()
                .unwrap()
                .status,
            RunStatus::Blocked
        );
        let cursor = db::repos::workflow_conflicts::get_transition_cursor(&pool, run_id)
            .await
            .unwrap()
            .expect("terminal unverifiable conflict should settle cursor");
        assert_eq!(cursor.current_state_id, "review");
        assert_eq!(cursor.cursor_status, "terminal_unverifiable");
        assert_eq!(cursor.resume_policy, "terminal_failure");
        assert_eq!(
            cursor.conflict_id.as_deref(),
            Some(history[0].conflict_id.as_str())
        );
        assert!(cursor.terminal_failure_reason.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn proposal_017_orchestrator_resolves_current_conflict_on_later_legal_transition() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool.clone()),
        );
        let now = Utc::now();
        let idea = domain::idea::Idea {
            id: IdeaId::new(),
            title: "P017".into(),
            body: "workflow conflict resolution runtime wiring".into(),
            workspace_root_path: None,
            project_key: None,
            status: domain::idea::IdeaStatus::Active,
            created_at: now,
            archived_at: None,
        };
        db::repos::ideas::insert(&pool, &idea).await.unwrap();

        let run_id = RunId::new();
        let mut run = test_run(run_id);
        run.idea_id = idea.id;
        db::repos::runs::insert(&pool, &run).await.unwrap();

        let stage = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: "review".into(),
            label: "Review".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at: now,
            completed_at: Some(now),
            owner_agent: Some("proposal_reviewer".into()),
            provider: Some("codex".into()),
            model: Some("test".into()),
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        };
        db::repos::stages::insert(&pool, &stage).await.unwrap();

        let mut blocking_plan = test_plan();
        blocking_plan.states.insert(
            "review".into(),
            compiled_state(
                "review",
                vec![CompiledTransition {
                    to: "refine".into(),
                    condition: "false".into(),
                }],
                false,
            ),
        );
        blocking_plan
            .states
            .insert("refine".into(), compiled_state("refine", Vec::new(), true));
        orchestrator
            .evaluate_and_transition(run_id, "review", &blocking_plan, &[stage.clone()])
            .await
            .unwrap();
        let blocked = db::repos::workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
            .await
            .unwrap()
            .expect("blocking conflict should exist before legal transition");

        let mut resolving_plan = blocking_plan;
        resolving_plan.states.insert(
            "review".into(),
            compiled_state(
                "review",
                vec![CompiledTransition {
                    to: "refine".into(),
                    condition: "true".into(),
                }],
                false,
            ),
        );
        orchestrator
            .evaluate_and_transition(run_id, "review", &resolving_plan, &[stage])
            .await
            .unwrap();

        assert!(
            db::repos::workflow_conflicts::get_current_blocking_conflict(&pool, run_id)
                .await
                .unwrap()
                .is_none(),
            "legal graph advancement should resolve the current blocking conflict"
        );
        let history = db::repos::workflow_conflicts::list_conflict_history_for_run(&pool, run_id)
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].conflict_id, blocked.conflict_id);
        assert_eq!(history[0].status, WorkflowConflictStatus::Resolved);
        assert_eq!(
            history[0]
                .resolution_record_json
                .as_ref()
                .and_then(|value| value.get("selected_next_state_id"))
                .and_then(|value| value.as_str()),
            Some("refine")
        );
        assert_eq!(
            db::repos::runs::find_by_id(&pool, run_id)
                .await
                .unwrap()
                .unwrap()
                .current_state
                .as_deref(),
            Some("refine")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn proposal_017_candidate_transition_evaluation_classifies_unknown_and_missing_inputs() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let events = crate::event_bus::new_bus(16);
        let orchestrator = Orchestrator::new(
            pool.clone(),
            events,
            crate::work_queue::WorkQueue::new(pool),
        );
        let tmp = tempfile::tempdir().unwrap();
        let mut run = test_run(RunId::new());
        run.workspace_root = tmp.path().to_string_lossy().into_owned();
        run.chainworks_meta_root = Some(tmp.path().to_string_lossy().into_owned());
        let mut plan = test_plan();
        plan.artifact_paths.insert(
            "proposal_review_summary".into(),
            "${CHAINWORKS_META_ROOT:-.chainworks}/reviews/proposal/summary.json".into(),
        );

        let unknown_exists = orchestrator
            .evaluate_transition_candidate(
                0,
                "review",
                &CompiledTransition {
                    to: "refine".into(),
                    condition: "exists('unknown_artifact')".into(),
                },
                &run,
                &plan,
            )
            .await;
        assert_eq!(
            unknown_exists.result,
            CandidateTransitionResult::InvalidExpression
        );
        assert_eq!(
            unknown_exists.required_artifacts,
            vec!["unknown_artifact".to_string()]
        );

        let unknown_field = orchestrator
            .evaluate_transition_candidate(
                1,
                "review",
                &CompiledTransition {
                    to: "refine".into(),
                    condition: "unknown_artifact.pass == true".into(),
                },
                &run,
                &plan,
            )
            .await;
        assert_eq!(
            unknown_field.result,
            CandidateTransitionResult::InvalidExpression
        );

        let declared_absent = orchestrator
            .evaluate_transition_candidate(
                2,
                "review",
                &CompiledTransition {
                    to: "refine".into(),
                    condition: "exists('proposal_review_summary')".into(),
                },
                &run,
                &plan,
            )
            .await;
        assert_eq!(
            declared_absent.result,
            CandidateTransitionResult::MissingInput
        );
        assert_eq!(
            declared_absent.missing_artifacts,
            vec!["proposal_review_summary".to_string()]
        );

        let summary_path = tmp.path().join("reviews/proposal/summary.json");
        std::fs::create_dir_all(summary_path.parent().unwrap()).unwrap();
        std::fs::write(&summary_path, r#"{"summary":"needs refinement"}"#).unwrap();
        let missing_field = orchestrator
            .evaluate_transition_candidate(
                3,
                "review",
                &CompiledTransition {
                    to: "refine".into(),
                    condition: "proposal_review_summary.pass == false".into(),
                },
                &run,
                &plan,
            )
            .await;
        assert_eq!(
            missing_field.result,
            CandidateTransitionResult::MissingInput
        );
        assert_eq!(
            missing_field.missing_fields,
            vec!["proposal_review_summary.pass".to_string()]
        );

        std::fs::write(&summary_path, r#"{"pass":false}"#).unwrap();
        let matched = orchestrator
            .evaluate_transition_candidate(
                4,
                "review",
                &CompiledTransition {
                    to: "refine".into(),
                    condition: "proposal_review_summary.pass == false".into(),
                },
                &run,
                &plan,
            )
            .await;
        assert_eq!(matched.result, CandidateTransitionResult::Matched);
        assert_eq!(
            matched.source_artifact_ids,
            vec!["proposal_review_summary".to_string()]
        );
    }
}
