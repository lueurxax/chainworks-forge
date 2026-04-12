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

        // ── Case 1: stage in progress — wait ────────────────────────────
        if let Some(stage) = current_stage {
            match stage.status {
                StageStatus::Running | StageStatus::WaitingApproval => {
                    return Ok(()); // still in progress
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

        // Regular compute state — create stage, enqueue InvokeAgent
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

        self.work_queue
            .enqueue(
                WorkItemKind::InvokeAgent,
                Some(run_id),
                Some(stage.stage_id.clone()),
                serde_json::json!({
                    "run_id": run_id.to_string(),
                    "stage_id": stage.stage_id,
                    "stage_execution_id": stage.id.to_string(),
                    "agent_id": state.owner.agent_id,
                    "provider": state.owner.provider,
                }),
            )
            .await?;

        Ok(())
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

        // Evaluate transitions — find the first that matches.
        for transition in &state.transitions {
            let matches = self
                .evaluate_condition(&transition.condition, run_id, all_stages)
                .await;
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

    /// Simple condition evaluator for transition `when` expressions.
    ///
    /// Supported patterns:
    /// - `"true"` → always true
    /// - `exists('artifact_name')` → check if an artifact with that name exists
    /// - `approval.granted == true` → check if approval for current stage was granted
    /// - Anything else → treated as true (with a warning)
    async fn evaluate_condition(
        &self,
        condition: &str,
        run_id: RunId,
        _all_stages: &[StageExecution],
    ) -> bool {
        let trimmed = condition.trim().trim_matches('"');

        if trimmed == "true" {
            return true;
        }

        if trimmed.starts_with("exists(") {
            // exists('artifact_name') — check artifacts table
            let artifact_name = trimmed
                .trim_start_matches("exists(")
                .trim_end_matches(')')
                .trim_matches('\'')
                .trim_matches('"');
            let artifacts = db::repos::artifacts::list_by_run(&self.pool, run_id)
                .await
                .unwrap_or_default();
            return artifacts.iter().any(|a| a.name == artifact_name);
        }

        if trimmed.contains("approval.granted == true") {
            let approvals = approvals::list_by_run(&self.pool, run_id)
                .await
                .unwrap_or_default();
            return approvals
                .iter()
                .any(|a| a.decision == ApprovalDecision::Granted);
        }

        // For complex conditions (score comparisons, etc.) — default to true
        // so the first matching transition fires. The state machine will
        // refine this as we add a proper expression evaluator.
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
