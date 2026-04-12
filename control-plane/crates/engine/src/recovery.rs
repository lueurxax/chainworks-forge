use anyhow::Result;
use sqlx::SqlitePool;
use tracing::{info, warn};

use db::repos::{runs, stages};
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

        // Enqueue a global startup repair work item for any remaining stale state
        self.work_queue
            .enqueue(
                WorkItemKind::StartupRepair,
                None,
                None,
                serde_json::json!({ "reason": "daemon_startup" }),
            )
            .await?;

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

        for stage in &running_stages {
            warn!(
                run_id = %run.id,
                stage_id = %stage.stage_id,
                "Found stage stuck in Running state during startup repair — re-enqueuing AdvanceRun"
            );
            // Mark as blocked so we can retry safely
            stages::update_status(&self.pool, stage.id, StageStatus::Blocked).await?;
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
        }

        Ok(requeued)
    }
}
