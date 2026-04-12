use anyhow::Result;
use sqlx::SqlitePool;
use tracing::{info, warn};

use db::repos::{approvals, runs, stages};
use db::work_item::WorkItemKind;
use domain::events::DomainEvent;
use domain::ids::RunId;
use domain::run::RunStatus;
use domain::stage::StageStatus;

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

        let all_stages = stages::list_by_run(&self.pool, run_id).await?;
        let all_approvals = approvals::list_by_run(&self.pool, run_id).await?;

        let evaluation = DomainEngine::evaluate_run(&run, &all_stages, &all_approvals);

        match evaluation {
            RunEvaluation::Terminal => {
                // nothing to do
            }

            RunEvaluation::Complete => {
                info!(run_id = %run_id, "Run complete, marking completed");
                let now = chrono::Utc::now();
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
                // If we're Cancelling, settle as Cancelled
                if run.status == RunStatus::Cancelling {
                    let now = chrono::Utc::now();
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
                // Find the stage and activate it
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

                    // Update run status to Running if needed
                    if !matches!(run.status, RunStatus::Running) {
                        runs::update_status(&self.pool, run_id, RunStatus::Running).await?;
                        let _ = self.events.send(DomainEvent::RunStatusChanged {
                            run_id,
                            status: RunStatus::Running,
                        });
                    }

                    // Enqueue agent invocation for this stage.
                    // Provider is sourced from CHAINWORKS_DEFAULT_PROVIDER so
                    // a deployment can wire a specific runtime without code changes.
                    let default_provider = std::env::var("CHAINWORKS_DEFAULT_PROVIDER")
                        .unwrap_or_else(|_| "claude".to_string());
                    self.work_queue
                        .enqueue(
                            WorkItemKind::InvokeAgent,
                            Some(run_id),
                            Some(stage.stage_id.clone()),
                            serde_json::json!({
                                "run_id": run_id.to_string(),
                                "stage_id": stage.stage_id,
                                "stage_execution_id": stage.id.to_string(),
                                "agent_id": stage.stage_id,
                                "provider": default_provider,
                            }),
                        )
                        .await?;
                }
            }
        }

        Ok(())
    }
}
