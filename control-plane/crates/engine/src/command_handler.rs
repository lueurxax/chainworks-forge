use acp::AcpRuntimeManager;
use anyhow::{anyhow, Result};
use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{info, warn};

use db::repos::{
    agent_executions, approvals, command_journal, ideas, projections, runs, sessions, stages,
};
use db::work_item::WorkItemKind;
use domain::approval::ApprovalDecision;
use domain::commands::{CallerContext, Command};
use domain::events::DomainEvent;
use domain::ids::{ApprovalId, RunId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};

use crate::cancellation;
use crate::event_bus::EventSender;
use crate::preflight::{run_delivery_preflight, DeliveryPreflightResult};
use crate::work_queue::WorkQueue;

pub struct CommandHandler {
    pool: SqlitePool,
    events: EventSender,
    work_queue: WorkQueue,
    acp: Option<Arc<AcpRuntimeManager>>,
}

pub enum CommandResult {
    RunStarted { run_id: RunId },
    StartRunBlockedByDeliveryPreflight(StartRunBlockedByDeliveryPreflight),
    StageApproved { approval_id: ApprovalId },
    StageRejected { approval_id: ApprovalId },
    StageRetryScheduled { run_id: RunId, stage_id: String },
    RunCancelled { run_id: RunId },
    SessionReset { run_id: RunId, stage_id: String },
    StewardAnalysisQueued,
}

pub struct StartRunBlockedByDeliveryPreflight {
    pub delivery_preflight: DeliveryPreflightResult,
}

/// P029: Wrapper that pairs the command result with the journal audit ID.
/// `CommandHandler::handle` returns this instead of bare `CommandResult`.
pub struct Commanded {
    pub result: CommandResult,
    pub journal_id: String,
}

impl CommandHandler {
    pub fn new(pool: SqlitePool, events: EventSender, work_queue: WorkQueue) -> Self {
        Self {
            pool,
            events,
            work_queue,
            acp: None,
        }
    }

    pub fn new_with_acp(
        pool: SqlitePool,
        events: EventSender,
        work_queue: WorkQueue,
        acp: Arc<AcpRuntimeManager>,
    ) -> Self {
        Self {
            pool,
            events,
            work_queue,
            acp: Some(acp),
        }
    }

    pub async fn handle(&self, cmd: Command, caller: CallerContext) -> Result<Commanded> {
        // ── Command journal: record before execution (proposal §6.4) ────────
        let journal_id = uuid::Uuid::new_v4().to_string();
        let cmd_type = match &cmd {
            Command::StartRun(_) => "StartRun",
            Command::ApproveStage(_) => "ApproveStage",
            Command::RejectStage(_) => "RejectStage",
            Command::RetryStage(_) => "RetryStage",
            Command::CancelRun(_) => "CancelRun",
            Command::ResetSession(_) => "ResetSession",
            Command::RunStewardAnalysis(_) => "RunStewardAnalysis",
        };
        let raw = serde_json::to_string(&cmd).unwrap_or_default();
        let payload_json = crate::command_journal_redact::redact_for_journal(&cmd, &raw);
        let run_id_for_journal: Option<String> = match &cmd {
            Command::StartRun(_) => None,
            Command::ApproveStage(c) => Some(c.run_id.to_string()),
            Command::RejectStage(c) => Some(c.run_id.to_string()),
            Command::RetryStage(c) => Some(c.run_id.to_string()),
            Command::CancelRun(c) => Some(c.run_id.to_string()),
            Command::ResetSession(c) => Some(c.run_id.to_string()),
            Command::RunStewardAnalysis(_) => None,
        };
        let now = Utc::now();
        let principal_class_str = caller.principal_class.to_string();
        // INSERT is mandatory — fail closed (P029 §P2-005)
        command_journal::record(
            &self.pool,
            &journal_id,
            cmd_type,
            &payload_json,
            run_id_for_journal.as_deref(),
            now,
            Some(&caller.surface.to_string()),
            Some(&caller.principal_id),
            Some(&principal_class_str),
            Some(&caller.caller_tool),
        )
        .await
        .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;

        let result = self.execute_command(cmd).await;

        // Completion/failure are best-effort — log errors but don't fail the command
        let completed_at = Utc::now();
        match &result {
            Ok(_) => {
                if let Err(e) =
                    command_journal::complete_entry(&self.pool, &journal_id, completed_at).await
                {
                    tracing::error!(journal_id = %journal_id, error = %e, "Failed to close journal entry");
                }
            }
            Err(e) => {
                if let Err(e2) = command_journal::fail_entry(
                    &self.pool,
                    &journal_id,
                    completed_at,
                    &e.to_string(),
                )
                .await
                {
                    tracing::error!(journal_id = %journal_id, error = %e2, "Failed to record journal failure");
                }
            }
        }

        result.map(|r| Commanded {
            result: r,
            journal_id: journal_id.clone(),
        })
    }

    async fn execute_command(&self, cmd: Command) -> Result<CommandResult> {
        match cmd {
            Command::StartRun(c) => {
                let now = Utc::now();
                let run_id = RunId::new();
                // Compile the plan early to fail fast on invalid YAML before
                // persisting anything.
                let plan =
                    workflow::compiler::compile(&c.workflow_yaml_path, &c.agent_catalog_yaml_path)?;
                let delivery_preflight_json =
                    if let Some(delivery_configuration_json) = &c.delivery_configuration_json {
                        let delivery_config: domain::run::DeliveryConfiguration =
                            serde_json::from_str(delivery_configuration_json)?;
                        let preflight = run_delivery_preflight(&delivery_config);
                        if !preflight.passed {
                            return Ok(CommandResult::StartRunBlockedByDeliveryPreflight(
                                StartRunBlockedByDeliveryPreflight {
                                    delivery_preflight: preflight,
                                },
                            ));
                        }
                        Some(serde_json::to_string(&preflight)?)
                    } else {
                        None
                    };
                let idea = ideas::find_by_id(&self.pool, c.idea_id)
                    .await?
                    .ok_or_else(|| anyhow!("Idea {} not found", c.idea_id))?;
                let project_key = idea
                    .project_key
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .unwrap_or("untagged")
                    .to_string();

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
                    cancellation_settlement_log: None,
                    current_state: Some(plan.initial_state),
                    workflow_yaml_path: Some(c.workflow_yaml_path),
                    agent_catalog_yaml_path: Some(c.agent_catalog_yaml_path),
                    // Worktree fields — provisioned later by the orchestrator
                    // when the first write-enabled implementation state is entered.
                    worktree_root: None,
                    base_branch: None,
                    base_revision: None,
                    target_branch: None,
                    delivery_configuration_json: c.delivery_configuration_json.clone(),
                    delivery_preflight_json,
                    workflow_family: plan.workflow_family.clone(),
                    project_key: Some(project_key),
                    risk_class: plan.risk_class.clone(),
                    stack: plan.stack.clone(),
                    workflow_snapshot_hash: Some(plan.workflow_snapshot_hash.clone()),
                    catalog_snapshot_hash: Some(plan.catalog_snapshot_hash.clone()),
                    workflow_snapshot_json: Some(plan.workflow_snapshot_json.clone()),
                    catalog_snapshot_json: Some(plan.catalog_snapshot_json.clone()),
                    drift_detected_at: None,
                    drift_details_json: None,
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
                        // P044 §3d: If post-approval tasks exist, set stage to Running
                        // so the orchestrator can enqueue them. Otherwise settle as Completed.
                        let has_post_tasks = self
                            .check_has_post_approval_tasks(c.run_id, &c.stage_id)
                            .await;
                        if has_post_tasks {
                            stages::update_status(&self.pool, stage.id, StageStatus::Running)
                                .await?;
                            let _ = self.events.send(DomainEvent::StageStatusChanged {
                                run_id: c.run_id,
                                stage_execution_id: stage.id,
                                status: StageStatus::Running,
                            });
                        } else {
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
                        }
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
                stages::settle(&self.pool, old_stage.id, StageSettlementKind::Skipped, now).await?;

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
                    validation_failure_json: None,
                    evidence_packet_json: None,
                    recovery_snapshot_json: None,
                    retry_reason: Some("operator_retry".into()),
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
                cancellation::begin_settlement(&self.pool, c.run_id, now).await?;

                // Worktree cleanup on cancel (Proposal 007).
                if let Some(ref wt) = run.worktree_root {
                    if let Err(e) =
                        crate::worktree::WorktreeProvisioner::cleanup(wt, &run.workspace_root).await
                    {
                        tracing::warn!(
                            run_id = %c.run_id,
                            worktree = %wt,
                            error = %e,
                            "Worktree cleanup on cancel failed"
                        );
                    }
                }

                let _ = self.events.send(DomainEvent::RunStatusChanged {
                    run_id: c.run_id,
                    status: RunStatus::Cancelling,
                });

                cancellation::spawn_finalize_settlement(
                    self.pool.clone(),
                    self.events.clone(),
                    self.acp.clone(),
                    c.run_id,
                );

                Ok(CommandResult::RunCancelled { run_id: c.run_id })
            }

            Command::RunStewardAnalysis(c) => {
                let artifact_base = c
                    .artifact_base
                    .or_else(|| std::env::var("CHAINWORKS_META_ROOT").ok())
                    .unwrap_or_else(|| ".chainworks".into());
                self.work_queue
                    .enqueue(
                        WorkItemKind::StewardAnalysis,
                        None,
                        None,
                        serde_json::json!({
                            "reason": c.reason,
                            "artifact_base": artifact_base,
                        }),
                    )
                    .await?;
                Ok(CommandResult::StewardAnalysisQueued)
            }

            Command::ResetSession(c) => {
                // Mark the stage as requiring a reset by setting it to Pending
                let run_stages = stages::list_by_run(&self.pool, c.run_id).await?;
                if let Some(stage) = run_stages.iter().find(|s| s.stage_id == c.stage_id) {
                    let executions = agent_executions::find_by_stage(&self.pool, stage.id).await?;
                    for execution in executions {
                        if let Some(ref generation_id) = execution.session_generation_id {
                            sessions::end_generation(
                                &self.pool,
                                generation_id,
                                domain::session::SessionGenerationStatus::Reset,
                                "operator_reset",
                                Utc::now(),
                            )
                            .await?;
                            if let Some(ref lineage_id) = execution.session_lineage_id {
                                sessions::set_active_generation(&self.pool, lineage_id, None)
                                    .await?;
                                sessions::insert_event(
                                    &self.pool,
                                    &domain::session::SessionEvent {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        lineage_id: lineage_id.clone(),
                                        generation_id: generation_id.clone(),
                                        event_type:
                                            domain::session::SessionEventType::OperatorReset,
                                        recorded_at: Utc::now(),
                                        details_json: Some(
                                            serde_json::json!({ "reason": "operator_reset" })
                                                .to_string(),
                                        ),
                                    },
                                )
                                .await?;
                            }
                            if let Some(acp) = &self.acp {
                                let _ = acp.close_session(generation_id).await;
                            }
                        }
                    }
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

    /// P044 §3d helper: Check whether the workflow plan for the given run has
    /// `post_approval_tasks` on the state identified by `stage_id`.
    ///
    /// Returns `false` on any error (run not found, missing paths, plan compile
    /// failure, state not found) so that the caller falls back to the existing
    /// "settle as Completed" behaviour.
    async fn check_has_post_approval_tasks(&self, run_id: RunId, stage_id: &str) -> bool {
        let run = match runs::find_by_id(&self.pool, run_id).await {
            Ok(Some(r)) => r,
            _ => {
                warn!(run_id = %run_id, "check_has_post_approval_tasks: run not found");
                return false;
            }
        };

        let workflow_path = match run.workflow_yaml_path.as_deref() {
            Some(p) => p,
            None => return false,
        };
        let catalog_path = match run.agent_catalog_yaml_path.as_deref() {
            Some(p) => p,
            None => return false,
        };

        let plan = match workflow::compiler::compile(workflow_path, catalog_path) {
            Ok(p) => p,
            Err(e) => {
                warn!(
                    run_id = %run_id,
                    stage_id = %stage_id,
                    error = %e,
                    "check_has_post_approval_tasks: failed to compile plan"
                );
                return false;
            }
        };

        match plan.states.get(stage_id) {
            Some(state) => !state.post_approval_tasks.is_empty(),
            None => {
                warn!(
                    run_id = %run_id,
                    stage_id = %stage_id,
                    "check_has_post_approval_tasks: state not found in plan"
                );
                false
            }
        }
    }
}
