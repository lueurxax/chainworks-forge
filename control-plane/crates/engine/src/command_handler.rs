use anyhow::{anyhow, Result};
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::info;

use db::repos::{approvals, command_journal, projections, runs, stages};
use db::work_item::WorkItemKind;
use domain::approval::ApprovalDecision;
use domain::commands::Command;
use domain::events::DomainEvent;
use domain::ids::{ApprovalId, RunId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};

use crate::event_bus::EventSender;
use crate::work_queue::WorkQueue;

pub struct CommandHandler {
    pool: SqlitePool,
    events: EventSender,
    work_queue: WorkQueue,
}

pub enum CommandResult {
    RunStarted { run_id: RunId },
    StageApproved { approval_id: ApprovalId },
    StageRejected { approval_id: ApprovalId },
    StageRetryScheduled { run_id: RunId, stage_id: String },
    RunCancelled { run_id: RunId },
    SessionReset { run_id: RunId, stage_id: String },
}

impl CommandHandler {
    pub fn new(pool: SqlitePool, events: EventSender, work_queue: WorkQueue) -> Self {
        Self {
            pool,
            events,
            work_queue,
        }
    }

    pub async fn handle(&self, cmd: Command) -> Result<CommandResult> {
        // ── Command journal: record before execution (proposal §6.4) ────────
        let journal_id = uuid::Uuid::new_v4().to_string();
        let cmd_type = match &cmd {
            Command::StartRun(_) => "StartRun",
            Command::ApproveStage(_) => "ApproveStage",
            Command::RejectStage(_) => "RejectStage",
            Command::RetryStage(_) => "RetryStage",
            Command::CancelRun(_) => "CancelRun",
            Command::ResetSession(_) => "ResetSession",
        };
        let payload_json = serde_json::to_string(&cmd).unwrap_or_default();
        let run_id_for_journal: Option<String> = match &cmd {
            Command::StartRun(_) => None,
            Command::ApproveStage(c) => Some(c.run_id.to_string()),
            Command::RejectStage(c) => Some(c.run_id.to_string()),
            Command::RetryStage(c) => Some(c.run_id.to_string()),
            Command::CancelRun(c) => Some(c.run_id.to_string()),
            Command::ResetSession(c) => Some(c.run_id.to_string()),
        };
        let now = Utc::now();
        let _ = command_journal::record(
            &self.pool,
            &journal_id,
            cmd_type,
            &payload_json,
            run_id_for_journal.as_deref(),
            now,
        )
        .await;

        let result = self.execute_command(cmd).await;

        // Close the journal entry based on outcome.
        let completed_at = Utc::now();
        match &result {
            Ok(_) => {
                let _ =
                    command_journal::complete_entry(&self.pool, &journal_id, completed_at).await;
            }
            Err(e) => {
                let _ = command_journal::fail_entry(
                    &self.pool,
                    &journal_id,
                    completed_at,
                    &e.to_string(),
                )
                .await;
            }
        }

        result
    }

    async fn execute_command(&self, cmd: Command) -> Result<CommandResult> {
        match cmd {
            Command::StartRun(c) => {
                let now = Utc::now();
                let run_id = RunId::new();
                // If YAML paths are provided, compile the plan early to
                // fail fast on invalid YAML before persisting anything.
                let initial_state = if let (Some(wf), Some(ac)) =
                    (&c.workflow_yaml_path, &c.agent_catalog_yaml_path)
                {
                    let plan = workflow::compiler::compile(wf, ac)?;
                    Some(plan.initial_state)
                } else {
                    None
                };

                let run = Run {
                    id: run_id,
                    idea_id: c.idea_id,
                    status: RunStatus::Pending,
                    workflow_id: c.workflow_id,
                    workflow_title: c.workflow_title,
                    workspace_root: c.workspace_root,
                    artifact_root: c.artifact_root,
                    started_at: now,
                    completed_at: None,
                    cancellation_requested_at: None,
                    cancellation_settled_at: None,
                    current_state: initial_state,
                    workflow_yaml_path: c.workflow_yaml_path,
                    agent_catalog_yaml_path: c.agent_catalog_yaml_path,
                };
                runs::insert(&self.pool, &run).await?;
                // Activate the idea when its first run starts.
                db::repos::ideas::update_status(
                    &self.pool,
                    c.idea_id,
                    domain::idea::IdeaStatus::Active,
                )
                .await?;
                info!(run_id = %run_id, "Run started");
                let _ = self.events.send(DomainEvent::RunStarted {
                    run_id,
                    idea_id: run.idea_id,
                });
                self.work_queue
                    .enqueue(
                        WorkItemKind::AdvanceRun,
                        Some(run_id),
                        None,
                        serde_json::json!({ "run_id": run_id.to_string() }),
                    )
                    .await?;
                Ok(CommandResult::RunStarted { run_id })
            }

            Command::ApproveStage(c) => {
                let pending = approvals::list_by_run(&self.pool, c.run_id).await?;
                let approval = pending
                    .into_iter()
                    .find(|a| {
                        a.stage_id == c.stage_id
                            && matches!(
                                a.decision,
                                ApprovalDecision::Pending | ApprovalDecision::Requested
                            )
                    })
                    .ok_or_else(|| anyhow!("No pending approval for stage {}", c.stage_id))?;

                let now = Utc::now();
                approvals::resolve(
                    &self.pool,
                    approval.id,
                    ApprovalDecision::Granted,
                    now,
                    c.comment,
                )
                .await?;

                // For manual_gate stages (workflow-driven), approval completes the
                // gate — settle as Completed so the orchestrator can transition.
                // For regular stages, set to Running so InvokeAgent can proceed.
                let run_stages = stages::list_by_run(&self.pool, c.run_id).await?;
                if let Some(stage) = run_stages
                    .iter()
                    .find(|s| s.stage_id == c.stage_id && s.status == StageStatus::WaitingApproval)
                {
                    if stage.stage_type.as_deref() == Some("manual_gate") {
                        stages::settle(
                            &self.pool,
                            stage.id,
                            StageSettlementKind::Completed,
                            now,
                        )
                        .await?;
                        let _ = self.events.send(DomainEvent::StageStatusChanged {
                            run_id: c.run_id,
                            stage_execution_id: stage.id,
                            status: StageStatus::Completed,
                        });
                    } else {
                        stages::update_status(&self.pool, stage.id, StageStatus::Running).await?;
                        let _ = self.events.send(DomainEvent::StageStatusChanged {
                            run_id: c.run_id,
                            stage_execution_id: stage.id,
                            status: StageStatus::Running,
                        });
                    }
                }

                let _ = self.events.send(DomainEvent::ApprovalResolved {
                    approval_id: approval.id,
                    decision: ApprovalDecision::Granted,
                });

                self.work_queue
                    .enqueue(
                        WorkItemKind::AdvanceRun,
                        Some(c.run_id),
                        None,
                        serde_json::json!({ "run_id": c.run_id.to_string() }),
                    )
                    .await?;

                // Refresh projections so reads reflect the resolved approval.
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::StageApproved {
                    approval_id: approval.id,
                })
            }

            Command::RejectStage(c) => {
                let pending = approvals::list_by_run(&self.pool, c.run_id).await?;
                let approval = pending
                    .into_iter()
                    .find(|a| {
                        a.stage_id == c.stage_id
                            && matches!(
                                a.decision,
                                ApprovalDecision::Pending | ApprovalDecision::Requested
                            )
                    })
                    .ok_or_else(|| anyhow!("No pending approval for stage {}", c.stage_id))?;

                let now = Utc::now();
                approvals::resolve(
                    &self.pool,
                    approval.id,
                    ApprovalDecision::Rejected,
                    now,
                    c.comment,
                )
                .await?;

                // Update stage to Blocked
                let run_stages = stages::list_by_run(&self.pool, c.run_id).await?;
                if let Some(stage) = run_stages
                    .iter()
                    .find(|s| s.stage_id == c.stage_id && s.status == StageStatus::WaitingApproval)
                {
                    stages::update_status(&self.pool, stage.id, StageStatus::Blocked).await?;
                    let _ = self.events.send(DomainEvent::StageStatusChanged {
                        run_id: c.run_id,
                        stage_execution_id: stage.id,
                        status: StageStatus::Blocked,
                    });
                }

                let _ = self.events.send(DomainEvent::ApprovalResolved {
                    approval_id: approval.id,
                    decision: ApprovalDecision::Rejected,
                });

                // Refresh projections so reads reflect the rejection.
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::StageRejected {
                    approval_id: approval.id,
                })
            }

            Command::RetryStage(c) => {
                let run_stages = stages::list_by_run(&self.pool, c.run_id).await?;
                let old_stage = run_stages
                    .iter()
                    .find(|s| s.stage_id == c.stage_id)
                    .ok_or_else(|| anyhow!("Stage {} not found", c.stage_id))?;

                let now = Utc::now();
                // Mark old stage as skipped
                stages::settle(&self.pool, old_stage.id, StageSettlementKind::Skipped, now)
                    .await?;

                // Create new stage execution with attempt+1
                let new_stage = StageExecution {
                    id: domain::ids::StageExecutionId::new(),
                    run_id: c.run_id,
                    stage_id: old_stage.stage_id.clone(),
                    label: old_stage.label.clone(),
                    status: StageStatus::Pending,
                    iteration: old_stage.iteration,
                    attempt_number: old_stage.attempt_number + 1,
                    settlement_kind: None,
                    started_at: now,
                    completed_at: None,
                    owner_agent: old_stage.owner_agent.clone(),
                    provider: old_stage.provider.clone(),
                    model: old_stage.model.clone(),
                    stage_type: old_stage.stage_type.clone(),
                };
                stages::insert(&self.pool, &new_stage).await?;

                self.work_queue
                    .enqueue(
                        WorkItemKind::AdvanceRun,
                        Some(c.run_id),
                        Some(c.stage_id.clone()),
                        serde_json::json!({ "run_id": c.run_id.to_string(), "stage_id": c.stage_id }),
                    )
                    .await?;

                // Refresh projections so reads reflect the retry.
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::StageRetryScheduled {
                    run_id: c.run_id,
                    stage_id: c.stage_id,
                })
            }

            Command::CancelRun(c) => {
                let run = runs::find_by_id(&self.pool, c.run_id)
                    .await?
                    .ok_or_else(|| anyhow!("Run {} not found", c.run_id))?;

                if run.status.is_terminal() {
                    return Err(anyhow!("Run {} is already in terminal state", c.run_id));
                }

                let now = Utc::now();
                runs::mark_cancelling(&self.pool, c.run_id, now).await?;

                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id: c.run_id,
                    status: RunStatus::Cancelling,
                });

                // Refresh projections so reads reflect cancelling status.
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::RunCancelled { run_id: c.run_id })
            }

            Command::ResetSession(c) => {
                // Mark the stage as requiring a reset by setting it to Pending
                let run_stages = stages::list_by_run(&self.pool, c.run_id).await?;
                if let Some(stage) = run_stages.iter().find(|s| s.stage_id == c.stage_id) {
                    stages::update_status(&self.pool, stage.id, StageStatus::Pending).await?;
                }

                // Enqueue a repair work item
                self.work_queue
                    .enqueue(
                        WorkItemKind::StartupRepair,
                        Some(c.run_id),
                        Some(c.stage_id.clone()),
                        serde_json::json!({ "run_id": c.run_id.to_string(), "stage_id": c.stage_id }),
                    )
                    .await?;

                // Refresh projections so reads reflect the reset.
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::SessionReset {
                    run_id: c.run_id,
                    stage_id: c.stage_id,
                })
            }
        }
    }
}
