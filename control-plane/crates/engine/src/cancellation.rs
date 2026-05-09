use std::time::Duration;

use acp::AcpRuntimeManager;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::sync::Arc;

use db::repos::{
    agent_executions, lead_conflict_mediations, projections, runs, scheduler, sessions, stages,
    work_items,
};
use db::write_class::WriteLane;
use db::writer::{class_a_operation, DbWriter};
use domain::agent::AgentStatus;
use domain::events::DomainEvent;
use domain::ids::RunId;
use domain::provider::InvokeAgentCapacityConfig;
use domain::run::RunStatus;
use domain::session::{SessionEvent, SessionEventType, SessionGenerationStatus};
use domain::stage::StageSettlementKind;

use crate::event_bus::EventSender;

const FINALIZE_DELAY: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancellationSettlementEntry {
    pub agent_execution_id: String,
    pub agent_id: String,
    pub prior_status: String,
    pub terminal_status: String,
    pub session_close_attempted: bool,
    pub session_close_succeeded: Option<bool>,
    pub settled_at: DateTime<Utc>,
}

pub struct BeginSettlementResult {
    pub settlement_log: String,
    pub scheduler_refresh: scheduler::RefreshQueueSummariesResult,
}

pub async fn begin_settlement(
    pool: &SqlitePool,
    run_id: RunId,
    requested_at: DateTime<Utc>,
) -> Result<String> {
    let capacity = InvokeAgentCapacityConfig::default();
    begin_settlement_with_capacity(pool, run_id, requested_at, &capacity).await
}

pub async fn begin_settlement_with_capacity(
    pool: &SqlitePool,
    run_id: RunId,
    requested_at: DateTime<Utc>,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<String> {
    let tx_started = std::time::Instant::now();
    let local_writer = DbWriter::new(pool.clone());
    let mut tx = local_writer
        .begin_immediate_transaction(
            class_a_operation(
                "cancellation.begin_settlement",
                WriteLane::CriticalBarrier,
                format!("cancellation.begin_settlement:{run_id}"),
            ),
            "cancellation.begin_settlement",
        )
        .await?;
    let result = begin_settlement_tx(
        &mut tx,
        run_id,
        requested_at,
        capacity,
        "cancellation.begin_settlement",
    )
    .await?;
    tx.commit()
        .await
        .context("commit cancellation begin settlement")?;
    db::pool::log_write_transaction("cancellation.begin_settlement", tx_started);
    projections::rebuild_all_for_run(pool, run_id).await?;
    Ok(result.settlement_log)
}

pub async fn begin_settlement_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    requested_at: DateTime<Utc>,
    capacity: &InvokeAgentCapacityConfig,
    refresh_context: &'static str,
) -> Result<BeginSettlementResult> {
    let executions_before = agent_executions::list_by_run_tx(tx, run_id).await?;

    let entries: Vec<CancellationSettlementEntry> = executions_before
        .iter()
        .filter(|exec| exec.status == AgentStatus::Running)
        .map(|exec| CancellationSettlementEntry {
            agent_execution_id: exec.id.to_string(),
            agent_id: exec.agent_id.clone(),
            prior_status: exec.status.to_string(),
            terminal_status: AgentStatus::Cancelled.to_string(),
            session_close_attempted: false,
            session_close_succeeded: None,
            settled_at: requested_at,
        })
        .collect();

    agent_executions::cancel_running_by_run_tx(tx, run_id, requested_at).await?;
    work_items::cancel_running_by_run_tx(tx, run_id, requested_at).await?;

    // REL-001 (P017 R2 audit): cascade-cancel any active lead-mediation
    // records for this run so `lead_conflict_mediations` stays consistent
    // with `agent_executions`. Without this, a mediation-owned execution
    // ending up `canceled` could leave its mediation row in `queued` /
    // `running` / `operator_confirmation_required`, splitting durable
    // mediation truth across two tables and breaking late-output,
    // resume, and operator readback invariants.
    let canceled_mediations =
        lead_conflict_mediations::cancel_active_by_run_tx(tx, &run_id.to_string(), requested_at)
            .await?;
    if canceled_mediations > 0 {
        tracing::info!(
            run_id = %run_id,
            canceled_mediations = canceled_mediations,
            "Cancelled active lead mediations as part of run cancellation cascade"
        );
    }

    for stage in stages::list_by_run_tx(tx, run_id).await? {
        if stage.status == domain::stage::StageStatus::Running {
            stages::settle_tx(tx, stage.id, StageSettlementKind::Failed, requested_at).await?;
        }
    }

    let settlement_log =
        serde_json::to_string(&entries).context("serialize cancellation settlement log")?;
    runs::mark_cancelling_tx(tx, run_id, requested_at).await?;
    runs::update_cancellation_settlement_log_tx(tx, run_id, &settlement_log).await?;
    let scheduler_refresh = scheduler::refresh_queue_summaries_for_notification_tx(
        tx,
        capacity,
        requested_at,
        refresh_context,
        0,
    )
    .await?;
    Ok(BeginSettlementResult {
        settlement_log,
        scheduler_refresh,
    })
}

pub fn spawn_finalize_settlement(
    pool: SqlitePool,
    events: EventSender,
    acp: Option<Arc<AcpRuntimeManager>>,
    run_id: RunId,
) {
    tokio::spawn(async move {
        tokio::time::sleep(FINALIZE_DELAY).await;
        if let Err(error) = finalize_settlement(&pool, &events, acp.as_ref(), run_id).await {
            tracing::warn!(run_id = %run_id, error = %error, "finalize settlement failed");
        }
    });
}

async fn finalize_settlement(
    pool: &SqlitePool,
    events: &EventSender,
    acp: Option<&Arc<AcpRuntimeManager>>,
    run_id: RunId,
) -> Result<()> {
    let run = match runs::find_by_id(pool, run_id).await? {
        Some(run) => run,
        None => return Ok(()),
    };

    if run.status != RunStatus::Cancelling {
        return Ok(());
    }

    let log = run.cancellation_settlement_log.as_deref().unwrap_or("[]");
    let mut entries: Vec<CancellationSettlementEntry> =
        serde_json::from_str(log).context("parse cancellation settlement log")?;
    let executions = agent_executions::list_by_run(pool, run_id).await?;

    for entry in &mut entries {
        let generation_id = executions
            .iter()
            .find(|execution| execution.id.to_string() == entry.agent_execution_id)
            .and_then(|execution| execution.session_generation_id.clone());

        match (acp, generation_id.as_deref()) {
            (Some(runtime), Some(generation_id)) => {
                entry.session_close_attempted = true;
                let close_result = runtime.close_session(generation_id).await;
                entry.session_close_succeeded = Some(close_result.is_ok());
                if close_result.is_ok() {
                    sessions::end_generation(
                        pool,
                        generation_id,
                        SessionGenerationStatus::Closed,
                        "cancelled",
                        Utc::now(),
                    )
                    .await?;
                    if let Some(execution) = executions
                        .iter()
                        .find(|execution| execution.id.to_string() == entry.agent_execution_id)
                    {
                        if let Some(lineage_id) = execution.session_lineage_id.as_deref() {
                            sessions::insert_event(
                                pool,
                                &SessionEvent {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    lineage_id: lineage_id.to_string(),
                                    generation_id: generation_id.to_string(),
                                    event_type: SessionEventType::Closed,
                                    recorded_at: Utc::now(),
                                    details_json: Some(
                                        serde_json::json!({ "reason": "cancelled" }).to_string(),
                                    ),
                                },
                            )
                            .await?;
                        }
                    }
                }
            }
            _ => {
                entry.session_close_attempted = false;
                entry.session_close_succeeded = Some(false);
            }
        }
        entry.settled_at = Utc::now();
    }

    let settled_at = Utc::now();
    let settlement_log =
        serde_json::to_string(&entries).context("serialize finalized cancellation log")?;
    runs::finalize_cancellation(pool, run_id, settled_at, &settlement_log).await?;
    projections::rebuild_all_for_run(pool, run_id).await?;
    let _ = events.send(DomainEvent::RunStatusChanged {
        run_id,
        status: RunStatus::Cancelled,
    });
    Ok(())
}
