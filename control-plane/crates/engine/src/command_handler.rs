use acp::AcpRuntimeManager;
use anyhow::{anyhow, Result};
use chrono::Utc;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

use db::repos::{
    agent_executions, agent_retry_budget_ledger, approvals, artifact_contracts, command_journal,
    ideas, projections, runs, sessions, stages, work_items,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::approval::ApprovalDecision;
use domain::commands::{CallerContext, Command};
use domain::events::DomainEvent;
use domain::ids::{ApprovalId, RunId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};
use domain::PrincipalClass;

use crate::cancellation;
use crate::event_bus::EventSender;
use crate::preflight::{
    missing_delivery_configuration_preflight, run_delivery_preflight, DeliveryPreflightResult,
};
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
    ArtifactContractOverrideCreated { override_id: String },
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

fn plan_requires_delivery_configuration(plan: &workflow::plan::RunPlan) -> bool {
    plan.states.values().any(|state| {
        state
            .tasks
            .iter()
            .chain(state.post_approval_tasks.iter())
            .any(|task| is_release_agent(&task.agent.agent_id))
    })
}

fn is_release_agent(agent_id: &str) -> bool {
    matches!(
        agent_id,
        "commit_and_push_to_github" | "build_archive_and_push_connect"
    )
}

fn find_source_invoke_work_item<'a>(
    work_items: &'a [WorkItem],
    stage_execution_id: &str,
    agent_id: &str,
    agent_execution_id: &str,
) -> Option<&'a WorkItem> {
    work_items
        .iter()
        .filter(|item| item.kind == WorkItemKind::InvokeAgent)
        .filter_map(|item| {
            let payload = serde_json::from_str::<serde_json::Value>(&item.payload_json).ok()?;
            let claimed_agent_execution_id = payload
                .pointer("/p058_claimed/agent_execution_id")
                .and_then(|value| value.as_str());
            let payload_stage_execution_id = payload
                .get("stage_execution_id")
                .and_then(|value| value.as_str());
            let payload_agent_id = payload.get("agent_id").and_then(|value| value.as_str());
            let matches = claimed_agent_execution_id == Some(agent_execution_id)
                || (payload_stage_execution_id == Some(stage_execution_id)
                    && payload_agent_id == Some(agent_id));
            matches.then_some(item)
        })
        .max_by_key(|item| item.created_at)
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
        if matches!(&cmd, Command::OverrideArtifactContract(_))
            && caller.principal_class != PrincipalClass::Operator
        {
            anyhow::bail!("forbidden: OverrideArtifactContract requires operator principal");
        }

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
            Command::OverrideArtifactContract(_) => "OverrideArtifactContract",
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
            Command::OverrideArtifactContract(c) => Some(c.run_id.to_string()),
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
            caller.request_id.as_deref(),
        )
        .await
        .map_err(|e| anyhow::anyhow!("command journal insert failed: {e}"))?;

        let result = self.execute_command(cmd, &journal_id).await;

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

    async fn execute_command(&self, cmd: Command, journal_id: &str) -> Result<CommandResult> {
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
                    } else if plan_requires_delivery_configuration(&plan) {
                        return Ok(CommandResult::StartRunBlockedByDeliveryPreflight(
                            StartRunBlockedByDeliveryPreflight {
                                delivery_preflight: missing_delivery_configuration_preflight(),
                            },
                        ));
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
                    // P050: Per-run workspace isolation. All YAML artifact paths
                    // resolve through this meta root instead of shared .chainworks/.
                    chainworks_meta_root: Some(format!(".chainworks/runs/{}", run_id)),
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

            Command::OverrideArtifactContract(c) => {
                let override_id = db::repos::artifact_contracts::create_override_and_rebuild(
                    &self.pool,
                    domain::artifact_contracts::ArtifactContractOverrideInput {
                        run_id: c.run_id,
                        contract_id: c.contract_id,
                        override_type: c.override_type,
                        from_status: c.from_status,
                        to_status: c.to_status,
                        reason: c.reason,
                        owner: "operator".to_string(),
                        source_artifacts: c.source_artifacts,
                        expires_at_stage: c.expires_at_stage,
                        journal_id: journal_id.to_string(),
                    },
                )
                .await?;
                Ok(CommandResult::ArtifactContractOverrideCreated { override_id })
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

                let mut should_enqueue_advance = false;

                // Workflow manual gates use rejection as transition evidence so
                // the state machine can route normal loopbacks such as state_6 -> state_5.
                // Non-manual stages retain the existing rejection-as-blocked behavior.
                let run_stages = stages::list_by_run(&self.pool, c.run_id).await?;
                if let Some(stage) = run_stages
                    .iter()
                    .find(|s| s.stage_id == c.stage_id && s.status == StageStatus::WaitingApproval)
                {
                    if stage.stage_type.as_deref() == Some("manual_gate") {
                        stages::settle(&self.pool, stage.id, StageSettlementKind::Completed, now)
                            .await?;
                        should_enqueue_advance = true;
                        let _ = self.events.send(DomainEvent::StageStatusChanged {
                            run_id: c.run_id,
                            stage_execution_id: stage.id,
                            status: StageStatus::Completed,
                        });
                    } else {
                        stages::update_status(&self.pool, stage.id, StageStatus::Blocked).await?;
                        let _ = self.events.send(DomainEvent::StageStatusChanged {
                            run_id: c.run_id,
                            stage_execution_id: stage.id,
                            status: StageStatus::Blocked,
                        });
                    }
                }

                let _ = self.events.send(DomainEvent::ApprovalResolved {
                    approval_id: approval.id,
                    decision: ApprovalDecision::Rejected,
                });

                if should_enqueue_advance {
                    self.work_queue
                        .enqueue(
                            WorkItemKind::AdvanceRun,
                            Some(c.run_id),
                            None,
                            serde_json::json!({ "run_id": c.run_id.to_string() }),
                        )
                        .await?;
                }

                // Refresh projections so reads reflect the rejection.
                projections::rebuild_all_for_run(&self.pool, c.run_id).await?;

                Ok(CommandResult::StageRejected {
                    approval_id: approval.id,
                })
            }

            Command::RetryStage(c) => {
                if let Some(agent_execution_id) = c.agent_execution_id {
                    return self
                        .retry_agent_execution(
                            c.run_id,
                            &c.stage_id,
                            agent_execution_id,
                            c.consume_quota_budget_now,
                            journal_id,
                        )
                        .await;
                }

                return self
                    .retry_stage_latest_attempt(
                        c.run_id,
                        &c.stage_id,
                        c.consume_quota_budget_now,
                        journal_id,
                        "operator_retry",
                    )
                    .await;
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

    async fn retry_stage_latest_attempt(
        &self,
        run_id: RunId,
        stage_id: &str,
        consume_quota_budget_now: bool,
        journal_id: &str,
        retry_reason: &str,
    ) -> Result<CommandResult> {
        let run_stages = stages::list_by_run(&self.pool, run_id).await?;
        let matching_stages = run_stages
            .iter()
            .filter(|s| s.stage_id == stage_id)
            .collect::<Vec<_>>();
        let old_stage = matching_stages
            .iter()
            .copied()
            .max_by_key(|s| s.started_at)
            .ok_or_else(|| anyhow!("Stage {} not found", stage_id))?;
        let completed_current_stage_on_blocked_run = if old_stage.status == StageStatus::Completed {
            let run = runs::find_by_id(&self.pool, run_id)
                .await?
                .ok_or_else(|| anyhow!("Run {} not found", run_id))?;
            run.status == RunStatus::Blocked
                && (run.current_state.as_deref() == Some(stage_id)
                    || old_stage.stage_id == stage_id)
        } else {
            false
        };

        if !matches!(old_stage.status, StageStatus::Failed | StageStatus::Blocked)
            && !completed_current_stage_on_blocked_run
        {
            return Err(anyhow!(
                "Stage {} latest attempt is {} and cannot be retried yet",
                stage_id,
                old_stage.status
            ));
        }
        let next_attempt_number = matching_stages
            .iter()
            .map(|s| s.attempt_number)
            .max()
            .unwrap_or(old_stage.attempt_number)
            + 1;

        let now = Utc::now();
        let new_stage = StageExecution {
            id: domain::ids::StageExecutionId::new(),
            run_id,
            stage_id: old_stage.stage_id.clone(),
            label: old_stage.label.clone(),
            status: StageStatus::Pending,
            iteration: old_stage.iteration,
            attempt_number: next_attempt_number,
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
            retry_reason: Some(retry_reason.into()),
        };
        let retry_advance_work_item_id = new_stage.id.to_string();
        let retry_invoke_work_item_id = format!("p058-invoke:{}:0", new_stage.id);
        let retry_tx_started = Instant::now();
        let mut retry_tx =
            db::pool::begin_immediate_with_retry(&self.pool, "command.RetryStage").await?;
        apply_quota_retry_budget_for_stage_tx(
            &mut retry_tx,
            run_id,
            old_stage.id,
            consume_quota_budget_now,
            journal_id,
        )
        .await?;
        stages::settle_tx(
            &mut retry_tx,
            old_stage.id,
            StageSettlementKind::Skipped,
            now,
        )
        .await?;
        stages::insert_tx(&mut retry_tx, &new_stage).await?;
        artifact_contracts::mark_active_claims_superseded_pending_retry_for_stage_tx(
            &mut retry_tx,
            run_id,
            &old_stage.id.to_string(),
            &retry_invoke_work_item_id,
            journal_id,
        )
        .await?;
        sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
            .bind(RunStatus::Running.to_string())
            .bind(stage_id)
            .bind(run_id.to_string())
            .execute(&mut *retry_tx)
            .await?;
        work_items::enqueue_tx(
            &mut retry_tx,
            &WorkItem {
                id: retry_advance_work_item_id,
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "stage_id": stage_id
                })
                .to_string(),
                status: WorkItemStatus::Pending,
                run_id: Some(run_id),
                stage_id: Some(stage_id.to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
        )
        .await?;
        retry_tx.commit().await?;
        db::pool::log_write_transaction("command.RetryStage", retry_tx_started);

        // Refresh projections so reads reflect the retry.
        projections::rebuild_all_for_run(&self.pool, run_id).await?;

        Ok(CommandResult::StageRetryScheduled {
            run_id,
            stage_id: stage_id.to_string(),
        })
    }

    async fn retry_agent_execution(
        &self,
        run_id: RunId,
        stage_id: &str,
        agent_execution_id: domain::ids::AgentExecutionId,
        consume_quota_budget_now: bool,
        journal_id: &str,
    ) -> Result<CommandResult> {
        let run = runs::find_by_id(&self.pool, run_id)
            .await?
            .ok_or_else(|| anyhow!("Run {} not found", run_id))?;
        if run.status.is_terminal() {
            return Err(anyhow!("Run {} is already in terminal state", run_id));
        }

        let target_exec = agent_executions::find_by_id(&self.pool, agent_execution_id)
            .await?
            .ok_or_else(|| anyhow!("Agent execution {} not found", agent_execution_id))?;
        let old_stage = stages::find_by_id(&self.pool, target_exec.stage_execution_id)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Stage execution {} for agent execution {} not found",
                    target_exec.stage_execution_id,
                    agent_execution_id
                )
            })?;
        if old_stage.run_id != run_id || old_stage.stage_id != stage_id {
            return Err(anyhow!(
                "Agent execution {} belongs to run {} stage {}, not run {} stage {}",
                agent_execution_id,
                old_stage.run_id,
                old_stage.stage_id,
                run_id,
                stage_id
            ));
        }

        let run_stages = stages::list_by_run(&self.pool, run_id).await?;
        let matching_stages = run_stages
            .iter()
            .filter(|s| s.stage_id == stage_id)
            .collect::<Vec<_>>();
        let latest_stage = matching_stages
            .iter()
            .copied()
            .max_by_key(|s| s.started_at)
            .ok_or_else(|| anyhow!("Stage {} not found", stage_id))?;
        if latest_stage.id != old_stage.id {
            return Err(anyhow!(
                "Agent execution {} is on stale stage execution {}; latest for {} is {}",
                agent_execution_id,
                old_stage.id,
                stage_id,
                latest_stage.id
            ));
        }

        let completed_current_stage_on_blocked_run = old_stage.status == StageStatus::Completed
            && run.status == RunStatus::Blocked
            && (run.current_state.as_deref() == Some(stage_id) || old_stage.stage_id == stage_id);
        if !matches!(old_stage.status, StageStatus::Failed | StageStatus::Blocked)
            && !completed_current_stage_on_blocked_run
        {
            return Err(anyhow!(
                "Stage {} latest attempt is {} and cannot be targeted-retried yet",
                stage_id,
                old_stage.status
            ));
        }

        let run_work_items = work_items::list_by_run(&self.pool, run_id).await?;
        let source_item = find_source_invoke_work_item(
            &run_work_items,
            &old_stage.id.to_string(),
            &target_exec.agent_id,
            &agent_execution_id.to_string(),
        )
        .ok_or_else(|| {
            anyhow!(
                "InvokeAgent work item for agent execution {} not found",
                agent_execution_id
            )
        })?;
        if matches!(
            source_item.status,
            WorkItemStatus::Pending | WorkItemStatus::Running
        ) {
            return Err(anyhow!(
                "Agent execution {} source work item {} is still {}",
                agent_execution_id,
                source_item.id,
                source_item.status
            ));
        }
        if let (Some(acp), Some(generation_id)) = (
            self.acp.as_ref(),
            target_exec.session_generation_id.as_deref(),
        ) {
            if !acp.has_live_session(generation_id, None).await {
                warn!(
                    run_id = %run_id,
                    stage_id = %stage_id,
                    agent_execution_id = %agent_execution_id,
                    generation_id = %generation_id,
                    source_work_item_id = %source_item.id,
                    source_work_item_status = %source_item.status,
                    "Targeted retry source ACP generation is no longer live; falling back to full stage retry"
                );
                return self
                    .retry_stage_latest_attempt(
                        run_id,
                        stage_id,
                        consume_quota_budget_now,
                        journal_id,
                        "operator_retry_stale_targeted_retry",
                    )
                    .await;
            }
        }

        let mut retry_payload: serde_json::Value = serde_json::from_str(&source_item.payload_json)
            .map_err(|e| {
                anyhow!(
                    "Source InvokeAgent work item {} has invalid payload: {}",
                    source_item.id,
                    e
                )
            })?;
        let next_attempt_number = matching_stages
            .iter()
            .map(|s| s.attempt_number)
            .max()
            .unwrap_or(old_stage.attempt_number)
            + 1;
        let now = Utc::now();
        let new_stage = StageExecution {
            id: domain::ids::StageExecutionId::new(),
            run_id,
            stage_id: old_stage.stage_id.clone(),
            label: old_stage.label.clone(),
            status: StageStatus::Running,
            iteration: old_stage.iteration,
            attempt_number: next_attempt_number,
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
            retry_reason: Some(format!("operator_targeted_retry:{}", target_exec.agent_id)),
        };
        let retry_work_item_id = format!(
            "p058-targeted-retry:{}:{}",
            new_stage.id, agent_execution_id
        );
        if let Some(object) = retry_payload.as_object_mut() {
            object.insert("run_id".into(), serde_json::json!(run_id.to_string()));
            object.insert("stage_id".into(), serde_json::json!(stage_id));
            object.insert(
                "stage_execution_id".into(),
                serde_json::json!(new_stage.id.to_string()),
            );
            object.remove("p058_claimed");
            object.insert(
                "targeted_retry".into(),
                serde_json::json!({
                    "journal_id": journal_id,
                    "source_stage_execution_id": old_stage.id.to_string(),
                    "source_agent_execution_id": agent_execution_id.to_string(),
                    "source_work_item_id": source_item.id,
                    "reason": "operator_targeted_retry"
                }),
            );
        } else {
            return Err(anyhow!(
                "Source InvokeAgent work item {} payload is not a JSON object",
                source_item.id
            ));
        }

        let retry_tx_started = Instant::now();
        let mut retry_tx =
            db::pool::begin_immediate_with_retry(&self.pool, "command.RetryAgentExecution").await?;
        apply_quota_retry_budget_for_stage_tx(
            &mut retry_tx,
            run_id,
            old_stage.id,
            consume_quota_budget_now,
            journal_id,
        )
        .await?;
        stages::settle_tx(
            &mut retry_tx,
            old_stage.id,
            StageSettlementKind::Skipped,
            now,
        )
        .await?;
        stages::insert_tx(&mut retry_tx, &new_stage).await?;
        sqlx::query("UPDATE runs SET status = ?1, current_state = ?2 WHERE id = ?3")
            .bind(RunStatus::Running.to_string())
            .bind(stage_id)
            .bind(run_id.to_string())
            .execute(&mut *retry_tx)
            .await?;
        work_items::enqueue_tx(
            &mut retry_tx,
            &WorkItem {
                id: retry_work_item_id,
                kind: WorkItemKind::InvokeAgent,
                payload_json: serde_json::to_string(&retry_payload)?,
                status: WorkItemStatus::Pending,
                run_id: Some(run_id),
                stage_id: Some(stage_id.to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
        )
        .await?;
        retry_tx.commit().await?;
        db::pool::log_write_transaction("command.RetryAgentExecution", retry_tx_started);

        let _ = self.events.send(DomainEvent::StageStatusChanged {
            run_id,
            stage_execution_id: old_stage.id,
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

        Ok(CommandResult::StageRetryScheduled {
            run_id,
            stage_id: stage_id.to_string(),
        })
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

async fn apply_quota_retry_budget_for_stage_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    run_id: RunId,
    stage_execution_id: domain::ids::StageExecutionId,
    consume_quota_budget_now: bool,
    journal_id: &str,
) -> Result<()> {
    let now = Utc::now();
    let ledgers =
        agent_retry_budget_ledger::list_quota_for_stage_tx(tx, run_id, stage_execution_id).await?;
    for ledger in ledgers {
        if ledger.normal_budget_consumed {
            continue;
        }
        match ledger.retry_after {
            Some(retry_after) if retry_after > now => {
                if !consume_quota_budget_now {
                    return Err(anyhow!(
                        "quota retry_after has not elapsed for stage {}; retry after {} or set consume_quota_budget_now=true",
                        stage_execution_id,
                        retry_after.to_rfc3339()
                    ));
                }
                agent_retry_budget_ledger::consume_early_quota_retry_tx(tx, &ledger.id, journal_id)
                    .await?;
            }
            _ => {
                agent_retry_budget_ledger::mark_quota_reset_elapsed_tx(tx, &ledger.id).await?;
            }
        }
    }
    Ok(())
}
