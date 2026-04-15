use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{info, warn};

use db::repos::{runs, stages, startup_repairs};
use db::work_item::WorkItemKind;
use domain::run::Run;
use domain::stage::StageStatus;

use crate::event_bus::EventSender;
use crate::work_queue::WorkQueue;

pub struct RecoveryService {
    pool: SqlitePool,
    work_queue: WorkQueue,
    #[allow(dead_code)]
    events: EventSender,
}

pub struct RecoverySummary {
    pub runs_inspected: usize,
    pub runs_repaired: usize,
    pub work_items_requeued: usize,
}

impl RecoveryService {
    pub fn new(pool: SqlitePool, work_queue: WorkQueue, events: EventSender) -> Self {
        Self {
            pool,
            work_queue,
            events,
        }
    }

    pub async fn run_startup_repair(&self) -> Result<RecoverySummary> {
        let active_runs = runs::list_active(&self.pool).await?;
        let runs_inspected = active_runs.len();
        let mut runs_repaired = 0usize;
        let mut work_items_requeued = 0usize;

        info!(runs_inspected = %runs_inspected, "Starting startup recovery");

        for run in &active_runs {
            match self.repair_run(run).await {
                Ok(requeued) => {
                    if requeued > 0 {
                        runs_repaired += 1;
                        work_items_requeued += requeued;
                    }
                }
                Err(e) => {
                    warn!(run_id = %run.id, error = %e, "Failed to repair run during startup");
                }
            }
        }

        info!(
            runs_inspected = %runs_inspected,
            runs_repaired = %runs_repaired,
            work_items_requeued = %work_items_requeued,
            "Startup recovery complete"
        );

        Ok(RecoverySummary {
            runs_inspected,
            runs_repaired,
            work_items_requeued,
        })
    }

    async fn repair_run(&self, run: &Run) -> Result<usize> {
        let run_stages = stages::list_by_run(&self.pool, run.id).await?;
        let mut requeued = 0usize;

        // Check for stages stuck in Running state — these might be orphaned from a crash
        let running_stages: Vec<_> = run_stages
            .iter()
            .filter(|s| s.status == StageStatus::Running)
            .collect();

        let now = Utc::now();

        for stage in &running_stages {
            warn!(
                run_id = %run.id,
                stage_id = %stage.stage_id,
                "Found stage stuck in Running state during startup repair — re-enqueuing AdvanceRun"
            );
            // Mark as blocked so we can retry safely
            stages::update_status(&self.pool, stage.id, StageStatus::Blocked).await?;

            // Audit trail: record the repair action (proposal §6.3)
            let repair_id = uuid::Uuid::new_v4().to_string();
            let _ = startup_repairs::record(
                &self.pool,
                &repair_id,
                &run.id.to_string(),
                "stage_blocked",
                now,
                Some(&format!(
                    "Stage '{}' stuck in Running — marked Blocked",
                    stage.stage_id
                )),
            )
            .await;

            // Recovery recommendation: the operator should retry or cancel
            let rec_id = uuid::Uuid::new_v4().to_string();
            let _ = startup_repairs::recommend(
                &self.pool,
                &rec_id,
                &run.id.to_string(),
                Some(&stage.stage_id),
                "retry_stage",
                "Stage was stuck in Running at daemon startup — consider retry or manual review",
                now,
            )
            .await;
        }

        if !running_stages.is_empty() {
            // Re-enqueue an AdvanceRun for this run to recheck state
            self.work_queue
                .enqueue(
                    WorkItemKind::AdvanceRun,
                    Some(run.id),
                    None,
                    serde_json::json!({ "run_id": run.id.to_string(), "reason": "startup_repair" }),
                )
                .await?;
            requeued += 1;
        } else {
            // Even if no stages are stuck in Running, the run may need
            // advancement — e.g. daemon crashed after settling a stage as
            // Completed but before evaluate_and_transition ran, or after
            // a loop-back transition updated current_state but before
            // enqueuing the follow-up AdvanceRun. Unconditionally enqueue
            // an AdvanceRun for any active run. The advance handler is
            // idempotent — if nothing needs doing, it returns Ok(()).
            let has_pending_work = db::repos::work_items::list_by_run(&self.pool, run.id)
                .await
                .map(|items| {
                    items.iter().any(|w| {
                        matches!(
                            w.status,
                            db::work_item::WorkItemStatus::Pending
                                | db::work_item::WorkItemStatus::Running
                        )
                    })
                })
                .unwrap_or(false);

            if !has_pending_work {
                info!(
                    run_id = %run.id,
                    "Active run with no pending/running work — enqueuing AdvanceRun"
                );
                self.work_queue
                    .enqueue(
                        WorkItemKind::AdvanceRun,
                        Some(run.id),
                        None,
                        serde_json::json!({ "run_id": run.id.to_string(), "reason": "startup_catchup" }),
                    )
                    .await?;
                requeued += 1;
            }
        }

        Ok(requeued)
    }
}
