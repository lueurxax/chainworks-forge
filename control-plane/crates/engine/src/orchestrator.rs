use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{debug, error, info, warn};

use db::repos::{approvals, ideas, runs, stages};
use db::work_item::WorkItemKind;
use domain::approval::{Approval, ApprovalDecision};
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
        // IMPORTANT: A stage that is Completed from a **previous** iteration of
        // the same state (loop-back) must NOT be re-evaluated. Instead we fall
        // through to Case 2 (lazy creation) so a new stage is created for the
        // next iteration. Without this check, loop-backs (e.g. state_5→state_4)
        // cause an infinite advance_run cycle because the orchestrator sees the
        // old Completed stage and immediately calls evaluate_and_transition,
        // which transitions again, etc.
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
                                let prompt =
                                    build_task_prompt(task, &plan, run, idea_opt.as_ref(), None);
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

        // Regular compute state — create stage and enqueue tasks.
        info!(run_id = %run_id, state = %current_state_id, provider = %state.owner.provider, "Entering compute state");
        let stage = self
            .create_stage_for_state(run_id, &current_state_id, state)
            .await?;
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
            for (i, task) in &phase0_tasks {
                let prompt =
                    build_task_prompt(task, &plan, &run, idea_opt.as_ref(), source_ctx.as_ref());
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
        self.work_queue
            .enqueue(
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
                    "output_contract": task.agent.output_contract,
                    "prompt": prompt,
                    "task_index": task_index,
                    "total_tasks": total_tasks,
                    "worktree_write_enabled": task.agent.worktree_write_enabled,
                    "worktree_strategy": task.agent.worktree_strategy,
                    "session_reuse_scope": task.agent.session_reuse_scope,
                    "session_family_id": task.agent.session_family_id,
                    "declared_outputs": declared_outputs,
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
        self.work_queue
            .enqueue(
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
                    // Resolve ${VAR:-default} patterns.
                    let resolved = resolve_path_template(bb, &run.workspace_root);
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

        // Check loop budget. For cross-state loops (e.g. state_5→state_4),
        // the loop_config lives on state_5 and the transition target is state_4.
        // When budget is exhausted, we must skip ALL transitions — not just
        // self-transitions — because the single `to: state_4, when: "true"`
        // transition IS the loop, even though `target != current`.
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
                "Loop budget exhausted — skipping all transitions, blocking run"
            );
            // Fall through to the "no transition matched" handler below
            // which will block the run.
        }

        // Fetch run for condition evaluation context.
        let run = runs::find_by_id(&self.pool, run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        // Evaluate transitions — find the first that matches.
        // Skip entirely if loop budget is exhausted.
        for transition in &state.transitions {
            if loop_budget_exhausted {
                break; // don't evaluate any transitions when loop is done
            }

            let matches = self
                .evaluate_condition(&transition.condition, &run, plan)
                .await;
            if matches {
                info!(
                    run_id = %run_id,
                    from = current_state_id,
                    to = %transition.to,
                    condition = %transition.condition,
                    "Transition matched"
                );
                runs::update_current_state(&self.pool, run_id, &transition.to).await?;
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
    /// - `approval.granted == true` → check approvals
    /// - `artifact.field {==,!=,<,<=,>,>=} value` → read JSON artifact field
    /// - `vars.name` → runtime variable from plan
    /// - `expr and expr`, `expr or expr` → logical connectives
    async fn evaluate_condition(
        &self,
        condition: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
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
            return Box::pin(self.evaluate_condition(split.0, run, plan)).await
                && Box::pin(self.evaluate_condition(split.1, run, plan)).await;
        }
        if let Some(split) = split_connective(trimmed, " or ") {
            return Box::pin(self.evaluate_condition(split.0, run, plan)).await
                || Box::pin(self.evaluate_condition(split.1, run, plan)).await;
        }

        // exists('artifact_name')
        if trimmed.starts_with("exists(") && trimmed.ends_with(')') {
            let artifact_name = trimmed[7..trimmed.len() - 1]
                .trim_matches('\'')
                .trim_matches('"');
            return self.check_artifact_exists(artifact_name, run, plan);
        }

        // approval.granted == true
        if trimmed == "approval.granted == true" {
            let approvals = approvals::list_by_run(&self.pool, run.id)
                .await
                .unwrap_or_default();
            return approvals
                .iter()
                .any(|a| a.decision == ApprovalDecision::Granted);
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
                let lv = self.resolve_value(lhs, run, plan);
                let rv = self.resolve_value(rhs, run, plan);
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
    fn check_artifact_exists(
        &self,
        artifact_name: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> bool {
        if let Some(path_template) = plan.artifact_paths.get(artifact_name) {
            let resolved = resolve_path_template(path_template, &run.workspace_root);
            if std::path::Path::new(&resolved).exists() {
                info!(artifact = artifact_name, path = %resolved, "exists() = true");
                return true;
            }
            // Fallback: artifact_root
            for suffix in &[
                artifact_name.to_string(),
                format!("{}/{}", run.id, artifact_name),
            ] {
                let path = format!("{}/{}", run.artifact_root, suffix);
                if std::path::Path::new(&path).exists() {
                    info!(artifact = artifact_name, path = %path, "exists() = true (artifact_root)");
                    return true;
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
    fn resolve_value(
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
                if let Some(val) = self.read_artifact_field(artifact_name, field_name, run, plan) {
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
    fn read_artifact_field(
        &self,
        artifact_name: &str,
        field_name: &str,
        run: &domain::run::Run,
        plan: &workflow::plan::RunPlan,
    ) -> Option<serde_json::Value> {
        // Find the artifact file path
        let path = if let Some(template) = plan.artifact_paths.get(artifact_name) {
            let resolved = resolve_path_template(template, &run.workspace_root);
            if std::path::Path::new(&resolved).exists() {
                resolved
            } else {
                // Try artifact_root
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
            }
        } else {
            return None;
        };

        // Read and parse JSON
        let content = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        // Extract field (supports nested with dot notation)
        let val = json.get(field_name).cloned();
        if val.is_some() {
            return val;
        }

        // Try nested path: "a.b" → json["a"]["b"]
        let mut current = &json;
        for part in field_name.split('.') {
            current = current.get(part)?;
        }
        Some(current.clone())
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
                        .enqueue(
                            WorkItemKind::InvokeAgent,
                            Some(run_id),
                            Some(stage.stage_id.clone()),
                            serde_json::json!({
                                "run_id": run_id.to_string(),
                                "stage_id": stage.stage_id,
                                "stage_execution_id": stage.id.to_string(),
                                "agent_id": agent_id,
                                "provider": provider,
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
            let target_path =
                resolved_artifact_path_for_task(machine_artifact_name, plan, run, task);
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
                    let resolved = resolve_path_template(template, &run.workspace_root);
                    normalize_path_for_worktree(
                        &resolved,
                        &run.workspace_root,
                        run.worktree_root.as_deref(),
                        task.agent.worktree_write_enabled,
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
        .map(|template| resolve_path_template(template, &run.workspace_root))
        .unwrap_or_else(|| format!("{}/{}", run.artifact_root, artifact_name));
    normalize_path_for_worktree(
        &resolved,
        &run.workspace_root,
        run.worktree_root.as_deref(),
        task.agent.worktree_write_enabled,
    )
}

fn build_task_prompt(
    task: &workflow::plan::CompiledTask,
    plan: &workflow::plan::RunPlan,
    run: &domain::run::Run,
    idea: Option<&domain::idea::Idea>,
    source_ctx: Option<&crate::worktree::SourceContext>,
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
                let resolved = resolve_path_template(template, &run.workspace_root);
                let normalized = normalize_path_for_worktree(
                    &resolved,
                    &run.workspace_root,
                    wt_root,
                    wt_enabled,
                );
                parts.push(format!("- `{input_name}` → `{normalized}`"));
            } else {
                parts.push(format!("- `{input_name}` (path not defined in catalog)"));
            }
        }
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
                            .map(|template| resolve_path_template(template, &run.workspace_root))
                            .map(|resolved| {
                                normalize_path_for_worktree(
                                    &resolved,
                                    &run.workspace_root,
                                    wt_root,
                                    wt_enabled,
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
        parts.push(String::from(
            "- Use explicit absolute paths from the workspace root above.\n\
             - Write required outputs to the canonical paths listed in Required Outputs.\n\
             - Do not rely on implicit working directory.\n\
             - Do not perform git operations unless the task explicitly requests them.",
        ));
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
        parts.push(String::from("\n### Available Artifact Paths"));
        parts.push(String::from(
            "Reference these when producing outputs (write to canonical paths):",
        ));
        for (name, template) in plan.artifact_paths.iter().take(15) {
            let resolved = resolve_path_template(template, &run.workspace_root);
            let normalized = normalize_path_for_worktree(
                &resolved,
                &run.workspace_root,
                owner_wt_root,
                owner_wt_enabled,
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
             the final CHAINWORKS_OUTPUT envelope.",
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

fn normalize_path_for_worktree(
    path: &str,
    workspace_root: &str,
    worktree_root: Option<&str>,
    worktree_write_enabled: bool,
) -> String {
    if !worktree_write_enabled {
        return path.to_string();
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

pub fn resolve_path_template(template: &str, workspace_root: &str) -> String {
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
            std::env::var(var_name).unwrap_or_else(|_| default_val.to_string())
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
