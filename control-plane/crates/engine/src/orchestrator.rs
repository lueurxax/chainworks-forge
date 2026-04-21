use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{debug, error, info, warn};

use db::repos::{approvals, artifact_contracts, artifacts, ideas, runs, stages};
use db::work_item::WorkItemKind;
use domain::approval::{Approval, ApprovalDecision};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::artifact_contracts::ImplementationSelfAssessmentStatus;
use domain::events::DomainEvent;
use domain::ids::{ApprovalId, RunId};
use domain::run::RunStatus;
use domain::stage::{StageExecution, StageStatus};

use crate::domain_engine::{DomainEngine, RunEvaluation};
use crate::event_bus::EventSender;
use crate::work_queue::WorkQueue;

pub struct Orchestrator {
    pool: SqlitePool,
    events: EventSender,
    work_queue: WorkQueue,
}

impl Orchestrator {
    pub fn new(pool: SqlitePool, events: EventSender, work_queue: WorkQueue) -> Self {
        Self {
            pool,
            events,
            work_queue,
        }
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

    async fn advance_run_workflow(&self, run_id: RunId, run: &domain::run::Run) -> Result<()> {
        let plan = workflow::compiler::compile(
            run.workflow_yaml_path.as_deref().unwrap(),
            run.agent_catalog_yaml_path.as_deref().unwrap(),
        )?;

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
        // Stale detection: a Completed stage from a prior loop iteration must
        // not be re-evaluated — we need to create a new stage instead.
        // We use iteration count: if the stage's iteration is less than the
        // total number of stages for that state_id, it's from a prior cycle.
        let stage_is_stale = current_stage
            .filter(|s| s.status == StageStatus::Completed)
            .map(|completed_stage| {
                // If any other stage (different state_id) was started AFTER
                // this one, the workflow has moved past this state and looped back.
                all_stages.iter().any(|other| {
                    other.stage_id != current_state_id
                        && other.started_at > completed_stage.started_at
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
                    "Stale completed stage from prior loop iteration — creating new stage"
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
                        let completed = stage_invokes
                            .iter()
                            .filter(|w| w.status == db::work_item::WorkItemStatus::Completed)
                            .count();
                        let failed = stage_invokes
                            .iter()
                            .filter(|w| w.status == db::work_item::WorkItemStatus::Failed)
                            .count();

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

                        // ── Post-approval kickstart ─────────────────────────
                        // When a manual_gate stage transitions to Running after
                        // approval but has zero InvokeAgent work items yet,
                        // enqueue phase 0 from the post-approval task list.
                        if total == 0 && is_post_approval {
                            info!(
                                run_id = %run_id,
                                state = %current_state_id,
                                "Enqueuing post-approval phase 0 tasks"
                            );
                            let idea_opt = ideas::find_by_id(&self.pool, run.idea_id)
                                .await
                                .ok()
                                .flatten();
                            let effective_total = effective.len();
                            for (i, task) in
                                effective.iter().enumerate().filter(|(_, t)| t.phase == 0)
                            {
                                let approval_rejection_context = self
                                    .approval_rejection_context_for_state(run_id, &current_state_id)
                                    .await?;
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

                        if total > 0 && completed + failed == total {
                            // All enqueued tasks finished. Generalized N-phase
                            // gating: determine which phase just completed, then
                            // check if a subsequent phase exists and needs enqueuing.

                            // Determine the current (just-completed) phase from
                            // the work items that were enqueued.
                            let current_phase: u32 = stage_invokes
                                .iter()
                                .filter_map(|w| {
                                    serde_json::from_str::<serde_json::Value>(&w.payload_json)
                                        .ok()
                                        .and_then(|v| v.get("task_index")?.as_u64())
                                        .and_then(|idx| {
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
                            let next_phase_already_enqueued = next_phase.map_or(true, |np| {
                                stage_invokes.iter().any(|w| {
                                    serde_json::from_str::<serde_json::Value>(&w.payload_json)
                                        .ok()
                                        .and_then(|v| v.get("task_index")?.as_u64())
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
                                    runs::mark_completed(&self.pool, run_id, now).await?;
                                    self.enqueue_steward_analysis(Some(run_id)).await?;
                                    self.cleanup_worktree_if_needed(&run).await;
                                    let _ = self.events.send(DomainEvent::RunStatusChanged {
                                        run_id,
                                        status: RunStatus::Completed,
                                    });
                                    return Ok(());
                                }
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
                        // Stage done — evaluate transitions
                        return self
                            .evaluate_and_transition(run_id, &current_state_id, &plan, &all_stages)
                            .await;
                    }
                    StageStatus::Failed | StageStatus::Blocked => {
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
                runs::mark_completed(&self.pool, run_id, now).await?;
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
        // shared — NOT meta_only), provision one before creating the stage.
        let needs_git_worktree = {
            let needs_wt = |a: &workflow::plan::ResolvedAgent| -> bool {
                a.worktree_write_enabled && a.worktree_strategy.as_deref() != Some("meta_only")
            };
            state.tasks.iter().any(|t| needs_wt(&t.agent))
                || state.post_approval_tasks.iter().any(|t| needs_wt(&t.agent))
                || needs_wt(&state.owner)
        };
        // Re-bind `run` as mutable reference so we can refresh it after provisioning.
        let mut run = run.clone();
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
        };
        artifacts::insert(&self.pool, &artifact).await?;
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
        let declared_outputs = build_declared_outputs(task, plan, run);
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
                    "task_index": task_index,
                    "total_tasks": total_tasks,
                    "worktree_write_enabled": task.agent.worktree_write_enabled,
                    "worktree_strategy": effective_worktree_strategy_for_task(task),
                    "session_reuse_scope": task.agent.session_reuse_scope,
                    "session_family_id": task.agent.session_family_id,
                    "declared_outputs": declared_outputs,
                    "stage_degraded_output_policy": plan
                        .states
                        .get(&stage.stage_id)
                        .map(|state| state.degraded_output_policy.clone())
                        .unwrap_or_default(),
                }),
            )
            .await
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

        // Check loop budget. Budget exhaustion must prevent another loop body
        // from being scheduled, but it must still allow an already-complete
        // loop state to take its exit transition. This matters for states like
        // state_8 where the agent can report `complete` on the final allowed
        // iteration. Cross-state loops such as state_5 -> state_4 are modeled
        // as unconditional `true` transitions from a loop-configured state, so
        // those are still treated as budget-consuming loop transitions.
        let loop_budget_exhausted = state.loop_config.as_ref().map_or(false, |lc| {
            let iterations = all_stages
                .iter()
                .filter(|s| s.stage_id == current_state_id)
                .count() as u64;
            iterations >= lc.max
        });

        if loop_budget_exhausted {
            info!(
                run_id = %run_id,
                state = current_state_id,
                "Loop budget exhausted — skipping loop transitions"
            );
            // Fall through to transition evaluation. Only transitions that
            // would consume another loop cycle are skipped below.
        }

        // Fetch run for condition evaluation context.
        let run = runs::find_by_id(&self.pool, run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        // Evaluate transitions — find the first that matches.
        for transition in &state.transitions {
            if loop_budget_exhausted
                && transition_consumes_loop_budget(current_state_id, transition)
            {
                debug!(
                    run_id = %run_id,
                    from = current_state_id,
                    to = %transition.to,
                    condition = %transition.condition,
                    "Skipping budget-consuming loop transition"
                );
                continue;
            }

            let matches = self
                .evaluate_condition(&transition.condition, &run, plan, current_state_id)
                .await;
            if matches {
                info!(
                    run_id = %run_id,
                    from = current_state_id,
                    to = %transition.to,
                    condition = %transition.condition,
                    "Transition matched"
                );
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
        }

        // No transition matched — check if run should complete or block.
        if state.transitions.is_empty() || state.is_end {
            let now = Utc::now();
            runs::mark_completed(&self.pool, run_id, now).await?;
            self.enqueue_steward_analysis(Some(run_id)).await?;
            let _ = self.events.send(DomainEvent::RunStatusChanged {
                run_id,
                status: RunStatus::Completed,
            });
        } else {
            info!(
                run_id = %run_id,
                state = current_state_id,
                "No transition matched — run blocked"
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
        }

        Ok(())
    }

    /// Condition evaluator for transition `when` expressions.
    /// Matches Swift `TransitionEvaluator` (ARCH-031 canonical patterns).
    ///
    /// Supported:
    /// - `"true"` / `"false"` → literals
    /// - `exists('artifact_name')` → check filesystem
    /// - `approval.granted == true` → check granted approvals
    /// - `approval.rejected == true` → check rejected approvals
    /// - `artifact.field {==,!=,<,<=,>,>=} value` → read JSON artifact field
    /// - `vars.name` → runtime variable from plan
    /// - `expr and expr`, `expr or expr` → logical connectives
    async fn evaluate_condition(
        &self,
        condition: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
        current_state_id: &str,
    ) -> bool {
        let trimmed = condition.trim().trim_matches('"');

        if trimmed == "true" || trimmed == "'true'" {
            return true;
        }
        if trimmed == "false" || trimmed == "'false'" {
            return false;
        }

        // Handle `and` / `or` connectives (top-level split)
        if let Some(split) = split_connective(trimmed, " and ") {
            return Box::pin(self.evaluate_condition(split.0, run, plan, current_state_id)).await
                && Box::pin(self.evaluate_condition(split.1, run, plan, current_state_id)).await;
        }
        if let Some(split) = split_connective(trimmed, " or ") {
            return Box::pin(self.evaluate_condition(split.0, run, plan, current_state_id)).await
                || Box::pin(self.evaluate_condition(split.1, run, plan, current_state_id)).await;
        }

        // exists('artifact_name')
        if trimmed.starts_with("exists(") && trimmed.ends_with(')') {
            let artifact_name = trimmed[7..trimmed.len() - 1]
                .trim_matches('\'')
                .trim_matches('"');
            return self.check_artifact_exists(artifact_name, run, plan).await;
        }

        // approval.granted == true / approval.rejected == true
        if trimmed == "approval.granted == true" {
            let approvals = approvals::list_by_run(&self.pool, run.id)
                .await
                .unwrap_or_default();
            return approvals.iter().any(|a| {
                a.stage_id == current_state_id && a.decision == ApprovalDecision::Granted
            });
        }
        if trimmed == "approval.rejected == true" {
            let approvals = approvals::list_by_run(&self.pool, run.id)
                .await
                .unwrap_or_default();
            return approvals.iter().any(|a| {
                a.stage_id == current_state_id && a.decision == ApprovalDecision::Rejected
            });
        }

        // Comparison expressions: lhs op rhs
        // Try operators in order: <=, >=, !=, ==, <, >
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
                let lv = self.resolve_value(lhs, run, plan).await;
                let rv = self.resolve_value(rhs, run, plan).await;
                let result = apply_comparison(&lv, *op, &rv);
                info!(
                    lhs = lhs,
                    rhs = rhs,
                    op = ?op,
                    lhs_val = ?lv,
                    rhs_val = ?rv,
                    result = result,
                    "Transition comparison"
                );
                return result;
            }
        }

        // Unrecognized — fail closed (false), not open
        warn!(
            condition = trimmed,
            "Unrecognized transition condition — returning false"
        );
        false
    }

    /// Check if an artifact exists on the filesystem.
    async fn check_artifact_exists(
        &self,
        artifact_name: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> bool {
        match db::repos::artifact_contracts::active_contract_exists_result(
            &self.pool,
            run.id,
            artifact_name,
        )
        .await
        {
            Ok(db::repos::artifact_contracts::CanonicalContractField::Resolved(_)) => {
                info!(
                    artifact = artifact_name,
                    "P057-controlled exists() = true from active SQLite contract"
                );
                return true;
            }
            Ok(db::repos::artifact_contracts::CanonicalContractField::MissingControlled {
                contract_id,
            }) => {
                warn!(
                    artifact = artifact_name,
                    contract_id = %contract_id,
                    "P057-controlled exists() missing active SQLite contract; raw artifact fallback disabled"
                );
                return false;
            }
            Ok(db::repos::artifact_contracts::CanonicalContractField::UncontrolledAlias) => {}
            Err(error) => {
                if let Some(contract_id) =
                    db::repos::artifact_contracts::contract_id_for_alias(artifact_name)
                {
                    warn!(
                        artifact = artifact_name,
                        contract_id = %contract_id,
                        error = %error,
                        "P057-controlled exists() lookup failed; raw artifact fallback disabled"
                    );
                    return false;
                }
            }
        }
        if let Some(path_template) = plan.artifact_paths.get(artifact_name) {
            let resolved = resolve_path_template(
                path_template,
                &run.workspace_root,
                run.chainworks_meta_root.as_deref(),
            );
            if std::path::Path::new(&resolved).exists() {
                info!(artifact = artifact_name, path = %resolved, "exists() = true");
                return true;
            }
            // Fallback: artifact_root — only for legacy runs without per-run meta root.
            // P050: Post-P050 runs must NOT fall back to shared artifact_root because
            // stale files from prior runs would pollute transition conditions.
            if run.chainworks_meta_root.is_none() {
                for suffix in &[
                    artifact_name.to_string(),
                    format!("{}/{}", run.id, artifact_name),
                ] {
                    let path = format!("{}/{}", run.artifact_root, suffix);
                    if std::path::Path::new(&path).exists() {
                        info!(artifact = artifact_name, path = %path, "exists() = true (artifact_root legacy fallback)");
                        return true;
                    }
                }
            }
            info!(artifact = artifact_name, "exists() = false");
            return false;
        }
        warn!(
            artifact = artifact_name,
            "Artifact not in catalog — returning true"
        );
        true
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
        if artifact_name == "implementation_self_assessment_v2"
            || artifact_name == "implementation_self_assessment"
        {
            let active = artifact_contracts::find_active_implementation_self_assessment_summary(
                &self.pool, run.id,
            )
            .await
            .ok()
            .flatten()?;
            let summary_json = serde_json::to_value(&active.summary).ok()?;
            return extract_json_field(&summary_json, field_name);
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
                runs::mark_completed(&self.pool, run_id, now).await?;
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

/// Resolve `${VAR:-default}` patterns in artifact path templates.
/// Falls back to the default value if the env var is not set.
/// Also resolves bare `.` as workspace_root.
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

fn transition_consumes_loop_budget(
    current_state_id: &str,
    transition: &workflow::plan::CompiledTransition,
) -> bool {
    if transition.to == current_state_id {
        return true;
    }

    let condition = transition
        .condition
        .trim()
        .trim_matches('"')
        .trim_matches('\'');
    condition == "true"
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
            let target_path = resolved_artifact_path_for_task(path_artifact_name, plan, run, task);
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
                companion_output_name,
                companion_path,
            }
        })
        .collect()
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

    // Required outputs with resolved target paths — agent must write here
    if !task.outputs.is_empty() {
        parts.push(String::from("\n### Required Outputs"));
        parts.push(String::from(
            "Write each output to its canonical path below. \
             Create parent directories if missing.",
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
                 - Write required outputs to the canonical paths listed in Required Outputs.\n\
                 - Do not rely on implicit working directory."
            ));
        } else {
            parts.push(String::from(
                "- Use explicit absolute paths from the workspace root above.\n\
                 - Write required outputs to the canonical paths listed in Required Outputs.\n\
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
                 - Write required outputs to the canonical paths listed in Required Outputs.\n\
                 - Do not rely on implicit working directory.\n\
                 - Do not perform git operations unless the task explicitly requests them.",
            ));
        } else {
            parts.push(String::from(
                "- Use explicit absolute paths from the workspace root above.\n\
                 - Write required outputs to the canonical paths listed in Required Outputs.\n\
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
        && (task_name == "initial_implementation" || task_name == "continue_implementation")
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
            "Never emit shell commands that write required outputs into the run \
             artifact directory. Required outputs must be returned only through \
             the final JSON object envelope.",
        ));
        parts.push(String::from(
            "Use exactly this final response shape, with no surrounding prose, \
             markdown, or code fences:\n\
             {\"CHAINWORKS_OUTPUT\":{\"<canonical path from Required Outputs>\":{...}}}",
        ));
        parts.push(String::from(
            "Use the exact canonical output paths from Required Outputs as \
             `CHAINWORKS_OUTPUT` keys. Do not use output names as keys unless \
             a canonical path is unavailable.",
        ));
        parts.push(String::from(
            "Set `seemingly_complete` based only on remaining code-writer-owned \
             source or test work.",
        ));
        parts.push(String::from(
            "If the code changes and code-owned verification for the approved \
             proposal are done, set `seemingly_complete` to true even when manual \
             evidence, release evidence, documentation-only work, CloudKit \
             signed-in smoke checks, calendar/go-no-go decisions, or other \
             operator/ops tasks remain.",
        ));
        parts.push(String::from(
            "Do not make cosmetic polishing edits or rerun already-green tests \
             solely to avoid returning `seemingly_complete: true`.",
        ));
        parts.push(String::from(
            "Put non-code blockers into `remaining_tasks` or `known_risks` as \
             handoff tasks with owner labels.",
        ));
        parts.push(String::from(
            "When useful, include optional JSON fields `remaining_code_tasks`, \
             `handoff_tasks`, `blocked_by_non_code_evidence`, and \
             `verification_green` in the implementation self-assessment.",
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
    use domain::ids::{IdeaId, RunId};
    use domain::run::{Run, RunStatus};
    use std::collections::HashMap;
    use workflow::plan::{CompiledTask, OutputSchema, ResolvedAgent, RunPlan};

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
            workflow_snapshot_hash: "workflow".into(),
            catalog_snapshot_hash: "catalog".into(),
            workflow_snapshot_json: "{}".into(),
            catalog_snapshot_json: "{}".into(),
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
            },
            task_name: "review_proposal_as_product_owner".into(),
            inputs: Vec::new(),
            outputs: vec!["proposal_review_po".into()],
            output_schemas,
            parallel: true,
            phase: 0,
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

        assert!(
            !orchestrator
                .evaluate_condition(
                    "exists('prepush_review_report')",
                    &run,
                    &plan,
                    "state_8_implementation"
                )
                .await,
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

        assert!(
            orchestrator
                .evaluate_condition(
                    "exists('prepush_review_report')",
                    &run,
                    &plan,
                    "state_8_implementation"
                )
                .await,
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
}
