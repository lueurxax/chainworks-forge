use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::{Sqlite, SqlitePool, Transaction};
use tracing::{info, warn};

use db::repos::{
    agent_execution_runtime_facts, agent_executions, runs, scheduler, stages, startup_repairs,
    work_items,
};
use db::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use domain::agent::{AgentFailureKind, AgentOutputSettlement, AgentStatus};
use domain::provider::InvokeAgentCapacityConfig;
use domain::run::Run;
use domain::stage::StageStatus;

use crate::event_bus::EventSender;
use crate::work_queue::WorkQueue;

pub struct RecoveryService {
    pool: SqlitePool,
    work_queue: WorkQueue,
    #[allow(dead_code)]
    events: EventSender,
    capacity_config: Arc<InvokeAgentCapacityConfig>,
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
    pub recovered_item_count: usize,
    pub queued_under_startup_recovery_backpressure_count: usize,
    pub oldest_recovered_queued_age_ms: Option<i64>,
    pub affected_run_count: usize,
    pub next_retry_or_backoff_time: Option<chrono::DateTime<Utc>>,
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
        Self::new_with_capacity(
            pool,
            work_queue,
            events,
            InvokeAgentCapacityConfig::default(),
        )
    }

    pub fn new_with_capacity(
        pool: SqlitePool,
        work_queue: WorkQueue,
        events: EventSender,
        capacity_config: InvokeAgentCapacityConfig,
    ) -> Self {
        Self {
            pool,
            work_queue,
            events,
            capacity_config: Arc::new(capacity_config),
        }
    }

    pub async fn run_startup_repair(&self) -> Result<RecoverySummary> {
        let active_runs = runs::list_active(&self.pool).await?;
        let runs_inspected = active_runs.len();
        let mut runs_repaired = 0usize;
        let mut work_items_requeued = 0usize;

        info!(runs_inspected = %runs_inspected, "Starting startup recovery");

        for run in &active_runs {
            let tx_started = Instant::now();
            let writer_wait_started_at = Instant::now();
            let mut tx =
                db::pool::begin_immediate_with_retry(&self.pool, "recovery.startup_repair")
                    .await
                    .context("begin startup repair transaction")?;
            let writer_wait_ms = writer_wait_started_at.elapsed().as_millis() as i64;
            let now = Utc::now();
            let repair_result = self.repair_run_tx(&mut tx, run, now).await;
            let refresh = match repair_result {
                Ok(requeued) => {
                    let refresh = scheduler::refresh_queue_summaries_for_notification_tx(
                        &mut tx,
                        &self.capacity_config,
                        now,
                        "recovery.startup_repair",
                        writer_wait_ms,
                    )
                    .await?;
                    tx.commit()
                        .await
                        .context("commit startup repair transaction")?;
                    db::pool::log_write_transaction("recovery.startup_repair", tx_started);
                    Some((requeued, refresh))
                }
                Err(error) => {
                    warn!(run_id = %run.id, error = %error, "Failed to repair run during startup");
                    None
                }
            };

            if let Some((requeued, refresh)) = refresh {
                self.work_queue.publish_scheduler_notification(refresh);
                if requeued > 0 {
                    runs_repaired += 1;
                    work_items_requeued += requeued;
                }
            }
        }

        info!(
            runs_inspected = %runs_inspected,
            runs_repaired = %runs_repaired,
            work_items_requeued = %work_items_requeued,
            "Startup recovery complete"
        );

        let readback = startup_repairs::build_startup_recovery_readback(
            &self.pool,
            work_items_requeued as i64,
            runs_repaired as i64,
            Utc::now(),
        )
        .await?;
        startup_repairs::record_startup_recovery_readback(&self.pool, &readback).await?;

        Ok(RecoverySummary {
            runs_inspected,
            runs_repaired,
            work_items_requeued,
            recovered_item_count: work_items_requeued,
            queued_under_startup_recovery_backpressure_count: readback
                .queued_under_startup_recovery_backpressure_count
                as usize,
            oldest_recovered_queued_age_ms: readback.oldest_recovered_queued_age_ms,
            affected_run_count: runs_repaired,
            next_retry_or_backoff_time: readback.next_retry_or_backoff_time,
        })
    }

    async fn repair_run_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        run: &Run,
        now: chrono::DateTime<Utc>,
    ) -> Result<usize> {
        let run_stages = stages::list_by_run_tx(tx, run.id).await?;
        let mut requeued = 0usize;

        // Check for stages stuck in Running state — these might be orphaned from a crash
        let running_stages: Vec<_> = run_stages
            .iter()
            .filter(|s| s.status == StageStatus::Running)
            .collect();

        let mut blocked_running_stages = 0usize;
        let mut stale_running_executions = 0usize;

        for stage in &running_stages {
            let requeued_preclaimed = work_items::requeue_running_preclaimed_invoke_for_stage_tx(
                tx,
                run.id,
                &stage.id.to_string(),
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

            let provenance_suffix = Self::latest_execution_provenance_suffix_tx(tx, stage.id)
                .await
                .unwrap_or_default();
            warn!(
                run_id = %run.id,
                stage_id = %stage.stage_id,
                "Found stage stuck in Running state during startup repair — re-enqueuing AdvanceRun"
            );
            // Mark as blocked so we can retry safely
            stages::update_status_tx(tx, stage.id, StageStatus::Blocked).await?;
            let drift_details = serde_json::json!({
                "source": "startup_repair",
                "reason": "stage_stuck_running",
                "stage_execution_id": stage.id.to_string(),
                "stage_id": stage.stage_id,
                "action": "marked_blocked_for_operator_retry",
            });
            runs::update_drift_detection_tx(tx, run.id, now, &drift_details.to_string()).await?;

            // Audit trail: record the repair action (proposal §6.3)
            let repair_id = uuid::Uuid::new_v4().to_string();
            startup_repairs::record_tx(
                &mut **tx,
                &repair_id,
                &run.id.to_string(),
                "stage_blocked",
                now,
                Some(&format!(
                    "Stage '{}' stuck in Running — marked Blocked{}",
                    stage.stage_id, provenance_suffix
                )),
            )
            .await?;

            // Recovery recommendation: the operator should retry or cancel
            let rec_id = uuid::Uuid::new_v4().to_string();
            startup_repairs::recommend_tx(
                &mut **tx,
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
            .await?;
            blocked_running_stages += 1;
        }

        for stage in run_stages
            .iter()
            .filter(|stage| stage.status != StageStatus::Running)
        {
            let executions = agent_executions::find_by_stage_tx(tx, stage.id).await?;
            let running_execution_ids = executions
                .iter()
                .filter(|execution| execution.status == AgentStatus::Running)
                .map(|execution| execution.id)
                .collect::<Vec<_>>();
            if running_execution_ids.is_empty() {
                continue;
            }

            for execution_id in &running_execution_ids {
                agent_executions::update_completed_tx(
                    tx,
                    *execution_id,
                    AgentStatus::Cancelled,
                    now,
                )
                .await?;
            }
            let cancelled_work_items =
                work_items::cancel_pending_or_running_invoke_by_stage_execution_tx(
                    tx,
                    run.id,
                    &stage.id.to_string(),
                    now,
                    "stale_stage_execution_startup_repair",
                )
                .await?;
            let repair_id = uuid::Uuid::new_v4().to_string();
            startup_repairs::record_tx(
                &mut **tx,
                &repair_id,
                &run.id.to_string(),
                "stale_running_agent_execution_cancelled",
                now,
                Some(&format!(
                    "Cancelled {} stale Running agent execution(s) attached to non-running stage '{}' ({:?}); cancelled {} matching InvokeAgent work item(s)",
                    running_execution_ids.len(),
                    stage.stage_id,
                    stage.status,
                    cancelled_work_items
                )),
            )
            .await?;
            stale_running_executions += running_execution_ids.len();
        }

        if blocked_running_stages > 0 || stale_running_executions > 0 {
            // Re-enqueue an AdvanceRun for this run to recheck state
            let item = advance_run_work_item(run.id, "startup_repair", now);
            work_items::enqueue_tx(tx, &item).await?;
            requeued += 1;
        } else {
            // Even if no stages are stuck in Running, the run may need
            // advancement — e.g. daemon crashed after settling a stage as
            // Completed but before evaluate_and_transition ran, or after
            // a loop-back transition updated current_state but before
            // enqueuing the follow-up AdvanceRun. Unconditionally enqueue
            // an AdvanceRun for any active run. The advance handler is
            // idempotent — if nothing needs doing, it returns Ok(()).
            let has_pending_work = work_items::has_pending_or_running_by_run_tx(tx, run.id).await?;

            if !has_pending_work {
                info!(
                    run_id = %run.id,
                    "Active run with no pending/running work — enqueuing AdvanceRun"
                );
                let item = advance_run_work_item(run.id, "startup_catchup", now);
                work_items::enqueue_tx(tx, &item).await?;
                requeued += 1;
            }
        }

        Ok(requeued)
    }

    async fn latest_execution_provenance_suffix_tx(
        tx: &mut Transaction<'_, Sqlite>,
        stage_execution_id: domain::ids::StageExecutionId,
    ) -> Option<String> {
        let executions = agent_executions::find_by_stage_tx(tx, stage_execution_id)
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

fn advance_run_work_item(
    run_id: domain::ids::RunId,
    reason: &str,
    now: chrono::DateTime<Utc>,
) -> WorkItem {
    WorkItem {
        id: uuid::Uuid::new_v4().to_string(),
        kind: WorkItemKind::AdvanceRun,
        payload_json: serde_json::json!({
            "run_id": run_id.to_string(),
            "reason": reason,
        })
        .to_string(),
        status: WorkItemStatus::Pending,
        run_id: Some(run_id),
        stage_id: None,
        created_at: now,
        scheduled_at: now,
        attempt_count: 0,
        last_error: None,
    }
}
