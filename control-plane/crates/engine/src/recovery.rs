use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{info, warn};

use db::repos::{
    agent_execution_runtime_facts, agent_executions, runs, stages, startup_repairs, work_items,
};
use db::work_item::WorkItemKind;
use domain::agent::{AgentFailureKind, AgentOutputSettlement};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_058_recovery_action_uses_failure_kind_and_output_settlement() {
        let mut facts = domain::agent::AgentExecutionRuntimeFacts::defaults_for(
            domain::ids::AgentExecutionId::new(),
            Utc::now(),
        );
        facts.failure_kind = Some(AgentFailureKind::ProviderQuota);
        facts.output_settlement = AgentOutputSettlement::MissingRequiredOutputs;

        assert_eq!(
            recovery_action_from_runtime_facts(Some(&facts)),
            ("wait_until_retry_after", "provider_quota_retry_budget")
        );

        facts.failure_kind = None;
        facts.output_settlement = AgentOutputSettlement::ValidOutputsFromFailedExecution;
        assert_eq!(
            recovery_action_from_runtime_facts(Some(&facts)),
            (
                "accept_or_retry_degraded_outputs",
                "valid_outputs_from_failed_execution"
            )
        );
    }
}

pub struct RecoverySummary {
    pub runs_inspected: usize,
    pub runs_repaired: usize,
    pub work_items_requeued: usize,
}

pub async fn persist_failed_stage_recovery_snapshot(
    pool: &SqlitePool,
    stage_execution_id: domain::ids::StageExecutionId,
    failed_at: chrono::DateTime<Utc>,
) -> Result<String> {
    let snapshot = match stages::find_by_id(pool, stage_execution_id).await? {
        Some(stage) => {
            let executions = agent_executions::find_by_stage(pool, stage_execution_id).await?;
            let latest_execution = executions.last();
            let runtime_facts = match latest_execution {
                Some(execution) => {
                    agent_execution_runtime_facts::find_by_execution_id(pool, execution.id).await?
                }
                None => None,
            };
            let (action, reason) = recovery_action_from_runtime_facts(runtime_facts.as_ref());
            serde_json::json!({
                "status": "available",
                "action": action,
                "reason": reason,
                "stage_execution_id": stage_execution_id.to_string(),
                "run_id": stage.run_id.to_string(),
                "stage_id": stage.stage_id,
                "failed_at": failed_at,
                "latest_agent_execution_id": latest_execution.map(|execution| execution.id.to_string()),
                "latest_agent_status": latest_execution.map(|execution| execution.status.to_string()),
                "failure_kind": runtime_facts.as_ref().and_then(|facts| facts.failure_kind.as_ref()).map(ToString::to_string),
                "output_settlement": runtime_facts.as_ref().map(|facts| facts.output_settlement.to_string()),
                "retry_after": runtime_facts.as_ref().and_then(|facts| facts.retry_after.map(|dt| dt.to_rfc3339())),
                "validation_failure_present": stage.validation_failure_json.is_some(),
            })
        }
        None => serde_json::json!({
            "status": "unavailable",
            "reason": "stage_execution_not_found",
            "stage_execution_id": stage_execution_id.to_string(),
            "failed_at": failed_at,
        }),
    };
    let encoded = serde_json::to_string_pretty(&snapshot)?;
    stages::update_recovery_snapshot_json(pool, stage_execution_id, &encoded).await?;
    Ok(encoded)
}

fn recovery_action_from_runtime_facts(
    facts: Option<&domain::agent::AgentExecutionRuntimeFacts>,
) -> (&'static str, &'static str) {
    let Some(facts) = facts else {
        return ("retry_stage", "stage_settled_failed");
    };
    match (&facts.failure_kind, &facts.output_settlement) {
        (Some(AgentFailureKind::ProviderQuota), _) => {
            ("wait_until_retry_after", "provider_quota_retry_budget")
        }
        (Some(AgentFailureKind::ProviderPermissionRequired), _) => {
            ("authorize_provider", "provider_permission_required")
        }
        (Some(AgentFailureKind::McpPermissionModalStall), _) => {
            ("authorize_xcode", "mcp_permission_modal_stall")
        }
        (Some(AgentFailureKind::MissingRequiredOutputs), _)
        | (_, AgentOutputSettlement::MissingRequiredOutputs) => {
            ("inspect_outputs_then_retry", "missing_required_outputs")
        }
        (Some(AgentFailureKind::InvalidOutputContract), _)
        | (_, AgentOutputSettlement::InvalidRequiredOutputs) => {
            ("inspect_contract_then_retry", "invalid_required_outputs")
        }
        (_, AgentOutputSettlement::ValidOutputsFromFailedExecution) => (
            "accept_or_retry_degraded_outputs",
            "valid_outputs_from_failed_execution",
        ),
        (Some(AgentFailureKind::SupersededByRetry), _) => {
            ("inspect_retry_successor", "superseded_by_retry")
        }
        _ => ("retry_stage", "runtime_failure"),
    }
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
        let mut blocked_running_stages = 0usize;

        for stage in &running_stages {
            let requeued_preclaimed = work_items::requeue_running_preclaimed_invoke_for_stage(
                &self.pool,
                run.id,
                stage.id,
                &stage.stage_id,
            )
            .await?;
            if requeued_preclaimed > 0 {
                info!(
                    run_id = %run.id,
                    stage_id = %stage.stage_id,
                    requeued = %requeued_preclaimed,
                    "Requeued preclaimed P058 InvokeAgent work item during startup repair"
                );
                requeued += requeued_preclaimed;
                continue;
            }

            let provenance_suffix = self
                .latest_execution_provenance_suffix(stage.id)
                .await
                .unwrap_or_default();
            warn!(
                run_id = %run.id,
                stage_id = %stage.stage_id,
                "Found stage stuck in Running state during startup repair — re-enqueuing AdvanceRun"
            );
            // Mark as blocked so we can retry safely
            stages::update_status(&self.pool, stage.id, StageStatus::Blocked).await?;
            let drift_details = serde_json::json!({
                "source": "startup_repair",
                "reason": "stage_stuck_running",
                "stage_execution_id": stage.id.to_string(),
                "stage_id": stage.stage_id,
                "action": "marked_blocked_for_operator_retry",
            });
            runs::update_drift_detection(&self.pool, run.id, now, &drift_details.to_string())
                .await?;

            // Audit trail: record the repair action (proposal §6.3)
            let repair_id = uuid::Uuid::new_v4().to_string();
            let _ = startup_repairs::record(
                &self.pool,
                &repair_id,
                &run.id.to_string(),
                "stage_blocked",
                now,
                Some(&format!(
                    "Stage '{}' stuck in Running — marked Blocked{}",
                    stage.stage_id, provenance_suffix
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
                &format!(
                    "Stage was stuck in Running at daemon startup — consider retry or manual review{}",
                    provenance_suffix
                ),
                now,
            )
            .await;
            blocked_running_stages += 1;
        }

        if blocked_running_stages > 0 {
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

    async fn latest_execution_provenance_suffix(
        &self,
        stage_execution_id: domain::ids::StageExecutionId,
    ) -> Option<String> {
        let executions = agent_executions::find_by_stage(&self.pool, stage_execution_id)
            .await
            .ok()?;
        let execution = executions.last()?;
        let mut details = Vec::new();

        if let Some(disposition) = execution.session_reuse_disposition.as_deref() {
            details.push(format!("reuse_disposition={disposition}"));
        }
        if let Some(reason) = execution.session_reset_reason.as_deref() {
            details.push(format!("reset_reason={reason}"));
        }
        if let Some(checkpoint_id) = execution.rehydrated_from_checkpoint_artifact_id.as_deref() {
            details.push(format!("checkpoint_artifact_id={checkpoint_id}"));
        }

        if details.is_empty() {
            None
        } else {
            Some(format!(
                "; latest execution provenance: {}",
                details.join(", ")
            ))
        }
    }
}
