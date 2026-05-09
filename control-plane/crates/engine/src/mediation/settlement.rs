//! P017 Phase B: MediationSettlementService.
//!
//! Engine-owned boundary for all mediation settlement operations.
//! Allowed entrypoints:
//!   - ResolveLeadMediationConfirmation command path
//!   - Engine/orchestrator auto-settle for valid no-confirmation outcomes
//!   - Engine recovery/repair for stale, canceled, duplicate, and ignored-late-output
//!
//! Forbidden entrypoints:
//!   - GraphQL mutation or resolver direct write
//!   - MCP server direct repository write
//!   - DB repository calling TransitionAuthorityResolver
//!   - ApproveStage/RejectStage reused as shortcut

use anyhow::Result;
use chrono::{DateTime, Utc};
use db::write_class::WriteLane;
use db::writer::{class_a_operation, DbWriter};
use sqlx::{Sqlite, SqlitePool, Transaction};

// ── Transaction-accepting settlement functions ────────────────────────
// These are the canonical settlement entrypoints used by the command handler
// within its existing IMMEDIATE transaction.

/// Settle a mediation as confirmed by operator (tx variant).
/// Returns `SettlementOutcome` with `rows_affected` so callers can detect
/// concurrent settlement (rows_affected == 0).
pub async fn settle_confirmed_tx(
    tx: &mut Transaction<'_, Sqlite>,
    mediation_record_id: &str,
    now: DateTime<Utc>,
) -> Result<SettlementOutcome> {
    let rows = db::repos::lead_conflict_mediations::update_status_tx(
        tx,
        mediation_record_id,
        "settled",
        Some("confirmed_by_operator"),
        None,
        now,
    )
    .await?;

    Ok(SettlementOutcome {
        mediation_record_id: mediation_record_id.to_string(),
        settlement_result: "confirmed_by_operator".to_string(),
        recovery_action: None,
        rows_affected: rows,
    })
}

/// Settle a mediation as rejected (clone/manual fallback) (tx variant).
pub async fn settle_rejected_clone_manual_tx(
    tx: &mut Transaction<'_, Sqlite>,
    mediation_record_id: &str,
    now: DateTime<Utc>,
) -> Result<SettlementOutcome> {
    let rows = db::repos::lead_conflict_mediations::update_status_tx(
        tx,
        mediation_record_id,
        "terminal_unverifiable",
        Some("rejected_clone_manual"),
        Some("clone_or_manual_fallback"),
        now,
    )
    .await?;

    Ok(SettlementOutcome {
        mediation_record_id: mediation_record_id.to_string(),
        settlement_result: "rejected_clone_manual".to_string(),
        recovery_action: Some("clone_or_manual_fallback".to_string()),
        rows_affected: rows,
    })
}

/// Settle a mediation due to deadline expiry (tx variant).
pub async fn settle_expired_tx(
    tx: &mut Transaction<'_, Sqlite>,
    mediation_record_id: &str,
    now: DateTime<Utc>,
) -> Result<SettlementOutcome> {
    let rows = db::repos::lead_conflict_mediations::update_status_tx(
        tx,
        mediation_record_id,
        "terminal_unverifiable",
        Some("confirmation_deadline_expired"),
        Some("clone_or_manual_fallback"),
        now,
    )
    .await?;

    Ok(SettlementOutcome {
        mediation_record_id: mediation_record_id.to_string(),
        settlement_result: "confirmation_deadline_expired".to_string(),
        recovery_action: Some("clone_or_manual_fallback".to_string()),
        rows_affected: rows,
    })
}

// ── Pool-based service (delegates to tx variants) ─────────────────────

/// Engine-owned settlement service. All mediation settlements pass through here.
pub struct MediationSettlementService {
    db_writer: DbWriter,
}

impl MediationSettlementService {
    pub fn new(pool: SqlitePool) -> Self {
        let db_writer = DbWriter::new(pool.clone());
        Self { db_writer }
    }

    async fn begin_transaction(
        &self,
        operation_name: &'static str,
        mediation_record_id: &str,
    ) -> Result<db::writer::QueuedTransaction> {
        self.db_writer
            .begin_immediate_transaction(
                class_a_operation(
                    operation_name,
                    WriteLane::CriticalBarrier,
                    format!("{operation_name}:{mediation_record_id}"),
                ),
                operation_name,
            )
            .await
    }

    /// Settle a mediation as confirmed by operator.
    pub async fn settle_confirmed(
        &self,
        mediation_record_id: &str,
        now: DateTime<Utc>,
    ) -> Result<SettlementOutcome> {
        let mut tx = self
            .begin_transaction("mediation.settle_confirmed", mediation_record_id)
            .await?;
        let outcome = settle_confirmed_tx(&mut tx, mediation_record_id, now).await?;
        tx.commit().await?;
        Ok(outcome)
    }

    /// Settle a mediation as rejected (clone/manual fallback).
    pub async fn settle_rejected_clone_manual(
        &self,
        mediation_record_id: &str,
        now: DateTime<Utc>,
    ) -> Result<SettlementOutcome> {
        let mut tx = self
            .begin_transaction(
                "mediation.settle_rejected_clone_manual",
                mediation_record_id,
            )
            .await?;
        let outcome = settle_rejected_clone_manual_tx(&mut tx, mediation_record_id, now).await?;
        tx.commit().await?;
        Ok(outcome)
    }

    /// Settle a mediation due to deadline expiry.
    pub async fn settle_expired(
        &self,
        mediation_record_id: &str,
        now: DateTime<Utc>,
    ) -> Result<SettlementOutcome> {
        let mut tx = self
            .begin_transaction("mediation.settle_expired", mediation_record_id)
            .await?;
        let outcome = settle_expired_tx(&mut tx, mediation_record_id, now).await?;
        tx.commit().await?;
        Ok(outcome)
    }
}

/// Result of a settlement operation.
pub struct SettlementOutcome {
    pub mediation_record_id: String,
    pub settlement_result: String,
    pub recovery_action: Option<String>,
    /// Number of rows affected by the status update. 0 means the mediation
    /// was already in a terminal state (concurrent settlement race).
    pub rows_affected: u64,
}
