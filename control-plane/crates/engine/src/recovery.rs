use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{info, warn};

use db::repos::{
    agent_execution_runtime_facts, agent_executions, artifact_contracts, projections, runs, stages,
    startup_repairs, work_items, workflow_conflicts,
};
use db::work_item::{WorkItemKind, WorkItemStatus};
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
    pub agent_executions_settled: u64,
    pub recovered_item_count: i64,
    pub queued_under_startup_recovery_backpressure_count: i64,
    pub oldest_recovered_queued_age_ms: Option<i64>,
    pub affected_run_count: i64,
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

async fn stage_has_pending_or_running_invoke_work(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
    stage_execution_id: domain::ids::StageExecutionId,
) -> Result<bool> {
    let items = work_items::list_by_run(pool, run_id).await?;
    let stage_execution_id = stage_execution_id.to_string();
    Ok(items.iter().any(|item| {
        item.kind == WorkItemKind::InvokeAgent
            && matches!(
                item.status,
                WorkItemStatus::Pending | WorkItemStatus::Running
            )
            && serde_json::from_str::<serde_json::Value>(&item.payload_json)
                .ok()
                .and_then(|payload| {
                    payload
                        .get("stage_execution_id")
                        .and_then(|value| value.as_str())
                        .map(str::to_string)
                })
                .as_deref()
                == Some(stage_execution_id.as_str())
    }))
}

async fn run_has_pending_or_running_work(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> Result<bool> {
    let items = work_items::list_by_run(pool, run_id).await?;
    Ok(items.iter().any(|item| {
        matches!(
            item.status,
            WorkItemStatus::Pending | WorkItemStatus::Running
        )
    }))
}

impl RecoveryService {
    pub fn new(pool: SqlitePool, work_queue: WorkQueue, events: EventSender) -> Self {
        Self {
            pool,
            work_queue,
            events,
        }
    }

    pub fn new_with_capacity(
        pool: SqlitePool,
        _work_queue: WorkQueue,
        events: EventSender,
        invoke_agent_capacity: InvokeAgentCapacityConfig,
    ) -> Self {
        let work_queue = WorkQueue::with_events_and_capacity(
            pool.clone(),
            events.clone(),
            invoke_agent_capacity,
        );
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
        let now = Utc::now();
        let agent_executions_settled =
            work_items::settle_terminal_preclaimed_invoke_agent_executions(&self.pool, now).await?;

        info!(runs_inspected = %runs_inspected, "Starting startup recovery");
        if agent_executions_settled > 0 {
            warn!(
                settled = agent_executions_settled,
                "Startup recovery settled terminal preclaimed InvokeAgent executions"
            );
        }
        let requeued_steward_analyses = work_items::requeue_running_steward_analysis_on_startup(
            &self.pool,
            now,
            "startup_repair_abandoned_steward_analysis",
        )
        .await?;
        if requeued_steward_analyses > 0 {
            work_items_requeued += requeued_steward_analyses as usize;
            warn!(
                requeued = requeued_steward_analyses,
                "Startup recovery requeued abandoned StewardAnalysis work items"
            );
        }
        let requeued_invoke_agents = work_items::requeue_running_invoke_agent_on_startup(
            &self.pool,
            now,
            "startup_repair_abandoned_invoke_agent",
        )
        .await?;
        if requeued_invoke_agents > 0 {
            work_items_requeued += requeued_invoke_agents as usize;
            warn!(
                requeued = requeued_invoke_agents,
                "Startup recovery requeued abandoned InvokeAgent work items"
            );
        }

        for run in &active_runs {
            let mut repaired_run = false;
            match artifact_contracts::repair_contract_status_normalization_and_rebuild(
                &self.pool, run.id,
            )
            .await
            {
                Ok(repaired_contracts) => {
                    if repaired_contracts > 0 {
                        repaired_run = true;
                        info!(
                            run_id = %run.id,
                            repaired_contracts = repaired_contracts,
                            "Startup recovery repaired legacy artifact contract status normalization"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        run_id = %run.id,
                        error = %e,
                        "Failed to repair artifact contract normalization during startup"
                    );
                }
            }
            if let Err(e) = rebuild_operator_read_projections(&self.pool, run.id).await {
                warn!(
                    run_id = %run.id,
                    error = %e,
                    "Failed to rebuild operator read projections during startup"
                );
            }
            match self.repair_run(run).await {
                Ok(requeued) => {
                    if requeued > 0 {
                        repaired_run = true;
                        work_items_requeued += requeued;
                    }
                }
                Err(e) => {
                    warn!(run_id = %run.id, error = %e, "Failed to repair run during startup");
                }
            }
            if repaired_run {
                runs_repaired += 1;
            }
        }

        info!(
            runs_inspected = %runs_inspected,
            runs_repaired = %runs_repaired,
            work_items_requeued = %work_items_requeued,
            "Startup recovery complete"
        );

        let readback = if work_items_requeued > 0 {
            self.work_queue.refresh_scheduler_projection().await?;
            let readback = startup_repairs::build_startup_recovery_readback(
                &self.pool,
                work_items_requeued as i64,
                runs_repaired as i64,
                Utc::now(),
            )
            .await?;
            startup_repairs::record_startup_recovery_readback(&self.pool, &readback).await?;
            Some(readback)
        } else {
            None
        };

        let reported_runs_repaired = readback
            .as_ref()
            .map(|readback| readback.affected_run_count.max(runs_repaired as i64) as usize)
            .unwrap_or(runs_repaired);

        Ok(RecoverySummary {
            runs_inspected,
            runs_repaired: reported_runs_repaired,
            work_items_requeued,
            agent_executions_settled,
            recovered_item_count: readback
                .as_ref()
                .map(|readback| readback.recovered_item_count)
                .unwrap_or(work_items_requeued as i64),
            queued_under_startup_recovery_backpressure_count: readback
                .as_ref()
                .map(|readback| readback.queued_under_startup_recovery_backpressure_count)
                .unwrap_or(0),
            oldest_recovered_queued_age_ms: readback
                .as_ref()
                .and_then(|readback| readback.oldest_recovered_queued_age_ms),
            affected_run_count: readback
                .as_ref()
                .map(|readback| readback.affected_run_count)
                .unwrap_or(runs_repaired as i64),
            next_retry_or_backoff_time: readback
                .as_ref()
                .and_then(|readback| readback.next_retry_or_backoff_time),
        })
    }

    async fn repair_run(&self, run: &Run) -> Result<usize> {
        let run_stages = stages::list_by_run(&self.pool, run.id).await?;
        let mut requeued = 0usize;
        let now = Utc::now();

        for stage in run_stages
            .iter()
            .filter(|stage| stage.status != StageStatus::Running)
        {
            let executions = agent_executions::find_by_stage(&self.pool, stage.id).await?;
            if !executions
                .iter()
                .any(|execution| execution.status == AgentStatus::Running)
            {
                continue;
            }

            let tx_started = std::time::Instant::now();
            let mut tx = db::pool::begin_immediate_with_retry(
                &self.pool,
                "recovery.clear_stale_stage_execution",
            )
            .await?;
            let cancelled_executions =
                agent_executions::cancel_running_by_stage_tx(&mut tx, stage.id, now).await?;
            let cancelled_work_items =
                work_items::cancel_pending_or_running_invoke_by_stage_execution_tx(
                    &mut tx,
                    run.id,
                    &stage.id.to_string(),
                    now,
                    "stale_stage_execution_startup_repair",
                )
                .await?;
            tx.commit().await?;
            db::pool::log_write_transaction("recovery.clear_stale_stage_execution", tx_started);

            warn!(
                run_id = %run.id,
                stage_id = %stage.stage_id,
                stage_execution_id = %stage.id,
                cancelled_executions = cancelled_executions,
                cancelled_work_items = cancelled_work_items,
                "Startup repair cleared stale running InvokeAgent state for non-running stage"
            );
        }

        // Check for stages stuck in Running state — these might be orphaned from a crash
        let running_stages: Vec<_> = run_stages
            .iter()
            .filter(|s| s.status == StageStatus::Running)
            .collect();

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
            if stage_has_pending_or_running_invoke_work(&self.pool, run.id, stage.id).await? {
                info!(
                    run_id = %run.id,
                    stage_id = %stage.stage_id,
                    stage_execution_id = %stage.id,
                    "Startup repair left running stage open because recovered InvokeAgent work is queued"
                );
                continue;
            }

            let executions = agent_executions::find_by_stage(&self.pool, stage.id).await?;
            if executions.is_empty() {
                if !run_has_pending_or_running_work(&self.pool, run.id).await? {
                    info!(
                        run_id = %run.id,
                        stage_id = %stage.stage_id,
                        stage_execution_id = %stage.id,
                        "Startup repair kickstarting empty running stage"
                    );
                    self.work_queue
                        .enqueue(
                            WorkItemKind::AdvanceRun,
                            Some(run.id),
                            None,
                            serde_json::json!({
                                "run_id": run.id.to_string(),
                                "reason": "startup_empty_running_stage_kickstart",
                                "stage_execution_id": stage.id.to_string(),
                                "stage_id": stage.stage_id,
                            }),
                        )
                        .await?;
                    requeued += 1;
                }
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
            if self.transition_cursor_blocks_startup_catchup(run).await? {
                let cancelled = work_items::cancel_pending_or_running_advance_by_run(
                    &self.pool,
                    run.id,
                    now,
                    "startup_repair_transition_cursor_parked",
                )
                .await?;
                if cancelled > 0 {
                    warn!(
                        run_id = %run.id,
                        cancelled = %cancelled,
                        "Startup recovery cancelled stale AdvanceRun work items for transition cursor parked run"
                    );
                }
                return Ok(requeued);
            }

            let requeued_running_advance = work_items::requeue_running_advance_by_run(
                &self.pool,
                run.id,
                now,
                "startup_repair_abandoned_advance_run",
            )
            .await?;
            if requeued_running_advance > 0 {
                info!(
                    run_id = %run.id,
                    requeued = %requeued_running_advance,
                    "Startup recovery requeued abandoned AdvanceRun work items"
                );
                requeued += requeued_running_advance as usize;
            }

            // Even if no stages are stuck in Running, the run may need
            // advancement — e.g. daemon crashed after settling a stage as
            // Completed but before evaluate_and_transition ran, or after
            // a loop-back transition updated current_state but before
            // enqueuing the follow-up AdvanceRun. Unconditionally enqueue
            // an AdvanceRun for any active run. The advance handler is
            // idempotent — if nothing needs doing, it returns Ok(()).
            let has_pending_work = run_has_pending_or_running_work(&self.pool, run.id)
                .await
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

    async fn transition_cursor_blocks_startup_catchup(&self, run: &Run) -> Result<bool> {
        let Some(cursor) = workflow_conflicts::get_transition_cursor(&self.pool, run.id).await?
        else {
            return Ok(false);
        };
        let blocks_catchup = matches!(
            cursor.resume_policy.as_str(),
            "await_conflict_resolution" | "terminal_failure"
        );
        if blocks_catchup {
            info!(
                run_id = %run.id,
                current_state = %cursor.current_state_id,
                cursor_status = %cursor.cursor_status,
                resume_policy = %cursor.resume_policy,
                conflict_id = ?cursor.conflict_id,
                "Startup recovery leaving run parked at workflow transition cursor"
            );
        }
        Ok(blocks_catchup)
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

async fn rebuild_operator_read_projections(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> Result<()> {
    projections::rebuild_run_summary(pool, run_id).await?;
    projections::rebuild_stage_summaries(pool, run_id).await?;
    projections::rebuild_approval_inbox(pool, run_id).await?;
    Ok(())
}
