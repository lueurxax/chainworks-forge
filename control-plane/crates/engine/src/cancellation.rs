use std::time::Duration;

use acp::AcpRuntimeManager;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::sync::Arc;

use db::repos::{
    agent_executions, agent_work_continuations, approvals, lead_conflict_mediations, projections,
    runs, scheduler, sessions, side_effects, stages, work_items,
};
use db::write_class::WriteLane;
use db::writer::class_a_operation;
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
    /// P082-R11/R12/R13/R14: typed readback for this cancellation settlement entry.
    /// Written when the cancellation interacts with retry work, approvals, side effects,
    /// or startup repairs. Null for simple running-execution cancellations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p082_recovery_matrix_readback: Option<serde_json::Value>,
}

pub struct BeginSettlementResult {
    pub settlement_log: String,
    pub scheduler_refresh: scheduler::RefreshQueueSummariesResult,
}

fn p082_cancellation_entry_from_readback(
    run_id: RunId,
    requested_at: DateTime<Utc>,
    readback: serde_json::Value,
) -> CancellationSettlementEntry {
    CancellationSettlementEntry {
        agent_execution_id: format!("p082-cancellation-readback:{run_id}"),
        agent_id: "p082-cancellation".to_string(),
        prior_status: "not_applicable".to_string(),
        terminal_status: readback
            .get("scenario_status")
            .and_then(|value| value.as_str())
            .unwrap_or("held")
            .to_string(),
        session_close_attempted: false,
        session_close_succeeded: None,
        settled_at: requested_at,
        p082_recovery_matrix_readback: Some(readback),
    }
}

pub fn p082_cancel_side_effect_reconciliation_readback(
    run_id: RunId,
    requested_at: DateTime<Utc>,
) -> serde_json::Value {
    domain::recovery_matrix::set_readback_side_effect_hold(
        domain::recovery_matrix::build_readback_v1(
            "P082-R13",
            "held",
            "reconcile_side_effects",
            domain::recovery_matrix::REASON_CANCEL_SIDE_EFFECT_RECONCILIATION_REQUIRED,
            "Cancellation held: unresolved side effects must be reconciled before final settlement.",
            "runs, side_effects, side_effect_attempts, side_effect_settlements",
            "runs, side_effects",
            &run_id.to_string(),
            Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
            "valid",
            &requested_at.to_rfc3339(),
        ),
        "unresolved_side_effect_entries",
        "Cancellation held: unresolved side-effect ledger entries exist. Reconcile side effects before final settlement.",
    )
}

pub fn p082_cancellation_settlement_log_for_readback(
    run_id: RunId,
    requested_at: DateTime<Utc>,
    readback: serde_json::Value,
) -> Result<String> {
    serde_json::to_string(&vec![p082_cancellation_entry_from_readback(
        run_id,
        requested_at,
        readback,
    )])
    .context("serialize P082 cancellation settlement readback")
}

async fn p082_cancellation_readback_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    requested_at: DateTime<Utc>,
) -> Result<serde_json::Value> {
    let run_id_string = run_id.to_string();
    if !side_effects::list_unresolved_for_run_tx(tx, &run_id_string)
        .await?
        .is_empty()
    {
        return Ok(p082_cancel_side_effect_reconciliation_readback(
            run_id,
            requested_at,
        ));
    }

    let approvals = approvals::list_by_run_tx(tx, run_id).await?;
    if approvals.iter().any(|approval| {
        matches!(
            approval.decision.to_string().as_str(),
            "pending" | "requested"
        )
    }) {
        return Ok(domain::recovery_matrix::build_readback_v1(
            "P082-R12",
            "cancelled",
            "cancel",
            domain::recovery_matrix::REASON_CANCEL_PENDING_APPROVAL_PRESERVED,
            "Run cancellation settled without modifying pending approval decision.",
            "runs, approvals, approval_inbox",
            "runs, approvals",
            &run_id_string,
            Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
            "valid",
            &requested_at.to_rfc3339(),
        ));
    }

    let startup_repair_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM startup_repairs
           WHERE run_id = ?1"#,
    )
    .bind(&run_id_string)
    .fetch_one(&mut **tx)
    .await
    .context("count startup repairs for P082 cancellation readback")?;
    if startup_repair_count > 0 {
        return Ok(domain::recovery_matrix::build_readback_v1(
            "P082-R14",
            "cancelled",
            "cancel",
            domain::recovery_matrix::REASON_CANCEL_STARTUP_REPAIR_CONVERGED,
            "Cancellation settled; startup repair converged idempotently with cancellation.",
            "runs, startup_repairs, work_items, session_generations",
            "runs, startup_repairs, work_items, sessions",
            &run_id_string,
            Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
            "valid",
            &requested_at.to_rfc3339(),
        ));
    }

    Ok(domain::recovery_matrix::build_readback_v1(
        "P082-R11",
        "cancelled",
        "cancel",
        domain::recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
        "Run cancellation settled active stage execution. Provider session terminalization evidence in session_generations/session_events.",
        "runs, work_items, retry_stage_execution_authorities, session_generations, session_events",
        "runs, work_items, sessions",
        &run_id_string,
        Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
        "valid",
        &requested_at.to_rfc3339(),
    ))
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
    let db_writer = db::writer::shared_writer_for(pool)
        .await
        .ok_or_else(|| anyhow::anyhow!("P075 shared DbWriter is not registered"))?;
    let mut tx = db_writer
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
    let p082_readback = p082_cancellation_readback_tx(tx, run_id, requested_at).await?;

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
            p082_recovery_matrix_readback: Some(p082_readback.clone()),
        })
        .collect();
    let entries = if entries.is_empty()
        && p082_readback
            .get("scenario_id")
            .and_then(|value| value.as_str())
            != Some("P082-R11")
    {
        vec![p082_cancellation_entry_from_readback(
            run_id,
            requested_at,
            p082_readback,
        )]
    } else {
        entries
    };

    agent_executions::cancel_running_by_run_tx(tx, run_id, requested_at).await?;
    work_items::cancel_running_by_run_tx(tx, run_id, requested_at).await?;
    let expired_approvals =
        approvals::expire_pending_by_run_tx(tx, run_id, requested_at, Some("run_cancelled".into()))
            .await?;
    if expired_approvals > 0 {
        tracing::info!(
            run_id = %run_id,
            expired_approvals = expired_approvals,
            "Expired pending approvals as part of run cancellation cascade"
        );
    }
    let cancelled_continuations = agent_work_continuations::mark_active_for_run_cancelling_tx(
        tx,
        &run_id.to_string(),
        &requested_at.to_rfc3339(),
    )
    .await?;
    if cancelled_continuations > 0 {
        tracing::info!(
            run_id = %run_id,
            cancelled_continuations = cancelled_continuations,
            "Marked active P086 continuations as cancelling during run cancellation cascade"
        );
    }

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
