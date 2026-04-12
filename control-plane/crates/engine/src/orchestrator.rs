use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{info, warn};

use db::repos::{approvals, runs, stages};
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

    async fn advance_run_workflow(
        &self,
        run_id: RunId,
        run: &domain::run::Run,
    ) -> Result<()> {
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
        if let Some(stage) = current_stage {
            match stage.status {
                StageStatus::Running => {
                    // For multi-task stages (fan-out), check if ALL InvokeAgent
                    // work items for this stage have completed.
                    let work_items = db::repos::work_items::list_by_run(&self.pool, run_id).await?;
                    let stage_invokes: Vec<_> = work_items
                        .iter()
                        .filter(|w| {
                            w.kind == db::work_item::WorkItemKind::InvokeAgent
                                && w.stage_id.as_deref() == Some(&current_state_id)
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

                    if total > 0 && completed + failed == total {
                        // All tasks finished — settle stage.
                        let now = Utc::now();
                        let (kind, status) = if failed > 0 {
                            (domain::stage::StageSettlementKind::Failed, StageStatus::Failed)
                        } else {
                            (domain::stage::StageSettlementKind::Completed, StageStatus::Completed)
                        };
                        info!(
                            run_id = %run_id,
                            state = %current_state_id,
                            total = total,
                            completed = completed,
                            failed = failed,
                            "All tasks finished — settling stage"
                        );
                        stages::settle(&self.pool, stage.id, kind, now).await?;
                        let _ = self.events.send(DomainEvent::StageStatusChanged {
                            run_id,
                            stage_execution_id: stage.id,
                            status: status.clone(),
                        });
                        if status == StageStatus::Completed {
                            return self
                                .evaluate_and_transition(run_id, &current_state_id, &plan, &all_stages)
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
                _ => {}
            }
        }

        // ── Case 2: no stage yet — lazy creation ────────────────────────

        // End state — mark run complete
        if state.is_end {
            info!(run_id = %run_id, state = %current_state_id, "Reached end state");
            // Create a stage to record the end state execution
            self.create_stage_for_state(run_id, &current_state_id, state).await?;
            let now = Utc::now();
            // Settle it immediately
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
            let _ = self.events.send(DomainEvent::RunStatusChanged {
                run_id,
                status: RunStatus::Completed,
            });
            return Ok(());
        }

        // Manual gate — create stage as WaitingApproval + Approval record
        if state.is_manual_gate {
            info!(run_id = %run_id, state = %current_state_id, "Entering manual gate");
            let stage = self.create_stage_for_state(run_id, &current_state_id, state).await?;
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

        // Regular compute state — create stage and enqueue tasks.
        info!(run_id = %run_id, state = %current_state_id, provider = %state.owner.provider, "Entering compute state");
        let stage = self.create_stage_for_state(run_id, &current_state_id, state).await?;
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

        if state.tasks.is_empty() {
            // No tasks defined — run the owner agent as a single task
            let prompt = build_task_prompt_for_owner(state);
            self.enqueue_invoke_agent(run_id, &stage, &state.owner, &prompt, 0, 1).await?;
        } else {
            // Fan-out: enqueue one InvokeAgent per task.
            // Parallel tasks run concurrently (executor spawns tokio tasks).
            // All tasks are enqueued at once — the executor picks them up.
            let total = state.tasks.len();
            for (i, task) in state.tasks.iter().enumerate() {
                let prompt = build_task_prompt(task);
                info!(
                    run_id = %run_id,
                    task = %task.task_name,
                    agent = %task.agent.agent_id,
                    provider = %task.agent.provider,
                    parallel = task.parallel,
                    index = i,
                    total = total,
                    "Enqueuing task"
                );
                self.enqueue_invoke_agent(run_id, &stage, &task.agent, &prompt, i, total).await?;
            }
        }

        Ok(())
    }

    /// Enqueue a single InvokeAgent work item for a task.
    async fn enqueue_invoke_agent(
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
                    "agent_id": agent.agent_id,
                    "provider": agent.provider,
                    "model": agent.model,
                    "effort": agent.effort,
                    "prompt": prompt,
                    "task_index": task_index,
                    "total_tasks": total_tasks,
                }),
            )
            .await
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
        let iteration = all_stages
            .iter()
            .filter(|s| s.stage_id == state_id)
            .count() as i64
            + 1;

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

        // Check loop: if this state has a loop config, check if we've exceeded max.
        if let Some(lc) = &state.loop_config {
            let iterations = all_stages
                .iter()
                .filter(|s| s.stage_id == current_state_id)
                .count() as u64;
            if iterations >= lc.max {
                info!(
                    run_id = %run_id,
                    state = current_state_id,
                    iterations = iterations,
                    max = lc.max,
                    "Loop budget exhausted"
                );
                // Skip the loop-back transition, fall through to non-loop transitions
                // by filtering out transitions that point to already-visited states
                // that would form a loop.
            }
        }

        // Fetch run for condition evaluation context.
        let run = runs::find_by_id(&self.pool, run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        // Evaluate transitions — find the first that matches.
        for transition in &state.transitions {
            let matches = self.evaluate_condition(
                &transition.condition,
                &run,
                plan,
            ).await;
            if matches {
                // Check loop guard: if transitioning back to current state and loop exhausted
                if transition.to == *current_state_id {
                    if let Some(lc) = &state.loop_config {
                        let iterations = all_stages
                            .iter()
                            .filter(|s| s.stage_id == current_state_id)
                            .count() as u64;
                        if iterations >= lc.max {
                            continue; // skip this loop-back
                        }
                    }
                }

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
    /// Matches Swift's `TransitionEvaluator` (ARCH-031 canonical patterns).
    ///
    /// Supported:
    /// - `"true"` → always true
    /// - `exists('artifact_name')` → check filesystem via artifact_paths map
    /// - `approval.granted == true` → check if approval was granted
    /// - `expr and expr`, `expr or expr` → logical connectives
    /// - Anything else → default true (with warning)
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

        // Handle `and` / `or` connectives
        if let Some(pos) = trimmed.find(" and ") {
            let lhs = &trimmed[..pos];
            let rhs = &trimmed[pos + 5..];
            return Box::pin(self.evaluate_condition(lhs, run, plan)).await
                && Box::pin(self.evaluate_condition(rhs, run, plan)).await;
        }
        if let Some(pos) = trimmed.find(" or ") {
            let lhs = &trimmed[..pos];
            let rhs = &trimmed[pos + 4..];
            return Box::pin(self.evaluate_condition(lhs, run, plan)).await
                || Box::pin(self.evaluate_condition(rhs, run, plan)).await;
        }

        // exists('artifact_name') — resolve path from catalog artifacts map,
        // then check if the file exists on disk (matches Swift TransitionEvaluator).
        if trimmed.starts_with("exists(") && trimmed.ends_with(')') {
            let artifact_name = trimmed
                .trim_start_matches("exists(")
                .trim_end_matches(')')
                .trim_matches('\'')
                .trim_matches('"');

            if let Some(path_template) = plan.artifact_paths.get(artifact_name) {
                // Check workspace-relative path first (canonical location from YAML)
                let resolved = resolve_path_template(path_template, &run.workspace_root);
                if std::path::Path::new(&resolved).exists() {
                    info!(artifact = artifact_name, path = %resolved, "exists() = true (workspace)");
                    return true;
                }
                // Fallback: check artifact_root (agents may write there instead)
                let art_path = format!("{}/{}", run.artifact_root, artifact_name);
                if std::path::Path::new(&art_path).exists() {
                    info!(artifact = artifact_name, path = %art_path, "exists() = true (artifact_root)");
                    return true;
                }
                // Fallback: check artifact_root with run_id subdirectory
                let art_run_path = format!("{}/{}/{}", run.artifact_root, run.id, artifact_name);
                if std::path::Path::new(&art_run_path).exists() {
                    info!(artifact = artifact_name, path = %art_run_path, "exists() = true (artifact_root/run_id)");
                    return true;
                }
                info!(
                    artifact = artifact_name,
                    workspace_path = %resolved,
                    artifact_root_path = %art_path,
                    "exists() = false"
                );
                return false;
            }
            // Artifact name not in catalog — check if it exists as a bare filename
            // in the workspace (fallback)
            warn!(
                artifact = artifact_name,
                "Artifact not found in catalog artifact_paths — defaulting to true"
            );
            return true;
        }

        if trimmed == "approval.granted == true" {
            let approvals = approvals::list_by_run(&self.pool, run.id)
                .await
                .unwrap_or_default();
            return approvals
                .iter()
                .any(|a| a.decision == ApprovalDecision::Granted);
        }

        // Unrecognized expression — default to true for forward progress
        warn!(
            condition = trimmed,
            "Unrecognized transition condition — defaulting to true"
        );
        true
    }

    // =====================================================================
    // Legacy flat-stage orchestration (no YAML workflow)
    // =====================================================================

    async fn advance_run_flat(
        &self,
        run_id: RunId,
        run: &domain::run::Run,
    ) -> Result<()> {
        let all_stages = stages::list_by_run(&self.pool, run_id).await?;
        let all_approvals = approvals::list_by_run(&self.pool, run_id).await?;

        let evaluation = DomainEngine::evaluate_run(run, &all_stages, &all_approvals);

        match evaluation {
            RunEvaluation::Terminal => {}

            RunEvaluation::Complete => {
                info!(run_id = %run_id, "Run complete, marking completed");
                let now = Utc::now();
                runs::mark_completed(&self.pool, run_id, now).await?;
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
                let stage = all_stages
                    .iter()
                    .find(|s| s.id == next_stage_id)
                    .cloned();

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
/// Build prompt for a specific task (agent-level prompt + task name).
fn build_task_prompt(task: &workflow::plan::CompiledTask) -> String {
    let agent_prompt = task.agent.prompt.as_deref().unwrap_or("");
    let task_desc = format!("Execute task: {}", task.task_name);
    if agent_prompt.is_empty() {
        task_desc
    } else {
        format!("## System Instructions\n{agent_prompt}\n\n---\n\n{task_desc}")
    }
}

/// Build prompt for the owner agent when no explicit tasks are defined.
fn build_task_prompt_for_owner(state: &workflow::plan::CompiledState) -> String {
    let agent_prompt = state.owner.prompt.as_deref().unwrap_or("");
    let task_context = format!("Execute state '{}' (label: {}).", state.id, state.label);
    if agent_prompt.is_empty() {
        task_context
    } else {
        format!("## System Instructions\n{agent_prompt}\n\n---\n\n{task_context}")
    }
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
