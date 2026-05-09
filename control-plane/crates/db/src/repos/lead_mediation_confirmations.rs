//! P017 Phase B: Repository for `lead_mediation_confirmations` table.
//! Separate store from stage approvals per the frozen approval-mediation contract.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::mediation::{LeadMediationConfirmation, MediationConfirmationStatus};

pub async fn insert(pool: &SqlitePool, record: &LeadMediationConfirmation) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "lead_mediation_confirmations.insert",
        sqlx::query(
            r#"INSERT INTO lead_mediation_confirmations
           (id, mediation_record_id, run_id, conflict_id, conflict_fingerprint,
            status, suggested_action, requested_at, deadline_at, readback_ref,
            idempotency_scope_key, resolved_at, resolved_by_principal_id,
            resolution_decision, resolution_comment)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
        )
        .bind(&record.id)
        .bind(&record.mediation_record_id)
        .bind(&record.run_id)
        .bind(&record.conflict_id)
        .bind(&record.conflict_fingerprint)
        .bind(record.status.to_string())
        .bind(&record.suggested_action)
        .bind(record.requested_at.to_rfc3339())
        .bind(record.deadline_at.map(|t| t.to_rfc3339()))
        .bind(&record.readback_ref)
        .bind(&record.idempotency_scope_key)
        .bind(record.resolved_at.map(|t| t.to_rfc3339()))
        .bind(&record.resolved_by_principal_id)
        .bind(&record.resolution_decision)
        .bind(&record.resolution_comment)
    )
    .context("insert lead_mediation_confirmation")?;
    Ok(())
}

pub async fn insert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    record: &LeadMediationConfirmation,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO lead_mediation_confirmations
           (id, mediation_record_id, run_id, conflict_id, conflict_fingerprint,
            status, suggested_action, requested_at, deadline_at, readback_ref,
            idempotency_scope_key, resolved_at, resolved_by_principal_id,
            resolution_decision, resolution_comment)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
    )
    .bind(&record.id)
    .bind(&record.mediation_record_id)
    .bind(&record.run_id)
    .bind(&record.conflict_id)
    .bind(&record.conflict_fingerprint)
    .bind(record.status.to_string())
    .bind(&record.suggested_action)
    .bind(record.requested_at.to_rfc3339())
    .bind(record.deadline_at.map(|t| t.to_rfc3339()))
    .bind(&record.readback_ref)
    .bind(&record.idempotency_scope_key)
    .bind(record.resolved_at.map(|t| t.to_rfc3339()))
    .bind(&record.resolved_by_principal_id)
    .bind(&record.resolution_decision)
    .bind(&record.resolution_comment)
    .execute(&mut **tx)
    .await
    .context("insert lead_mediation_confirmation (tx)")?;
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<LeadMediationConfirmation>> {
    let row = sqlx::query(
        r#"SELECT id, mediation_record_id, run_id, conflict_id, conflict_fingerprint,
                  status, suggested_action, requested_at, deadline_at, readback_ref,
                  idempotency_scope_key, resolved_at, resolved_by_principal_id,
                  resolution_decision, resolution_comment
           FROM lead_mediation_confirmations WHERE id = ?1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("find mediation confirmation by id")?;

    row.map(|r| parse_row(&r)).transpose()
}

pub async fn find_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<Option<LeadMediationConfirmation>> {
    let row = sqlx::query(
        r#"SELECT id, mediation_record_id, run_id, conflict_id, conflict_fingerprint,
                  status, suggested_action, requested_at, deadline_at, readback_ref,
                  idempotency_scope_key, resolved_at, resolved_by_principal_id,
                  resolution_decision, resolution_comment
           FROM lead_mediation_confirmations WHERE id = ?1"#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .context("find mediation confirmation by id (tx)")?;

    row.map(|r| parse_row(&r)).transpose()
}

/// List all pending mediation confirmations. This is the canonical source
/// for the mediation portion of the `approvals.list` mixed inbox.
pub async fn list_pending(pool: &SqlitePool) -> Result<Vec<LeadMediationConfirmation>> {
    let rows = sqlx::query(
        r#"SELECT id, mediation_record_id, run_id, conflict_id, conflict_fingerprint,
                  status, suggested_action, requested_at, deadline_at, readback_ref,
                  idempotency_scope_key, resolved_at, resolved_by_principal_id,
                  resolution_decision, resolution_comment
           FROM lead_mediation_confirmations
           WHERE status = 'pending'
           ORDER BY requested_at ASC"#,
    )
    .fetch_all(pool)
    .await
    .context("list pending mediation confirmations")?;

    rows.iter().map(|r| parse_row(r)).collect()
}

/// Resolve a pending confirmation within an existing transaction.
/// Returns rows_affected so callers can distinguish success (1) from
/// concurrent no-op (0) when the CAS guard (status='pending') blocks.
pub async fn resolve_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    decision: &str,
    comment: Option<&str>,
    principal_id: &str,
    now: DateTime<Utc>,
) -> Result<u64> {
    let result = sqlx::query(
        r#"UPDATE lead_mediation_confirmations
           SET status = 'resolved', resolution_decision = ?1, resolution_comment = ?2,
               resolved_by_principal_id = ?3, resolved_at = ?4
           WHERE id = ?5 AND status = 'pending'"#,
    )
    .bind(decision)
    .bind(comment)
    .bind(principal_id)
    .bind(now.to_rfc3339())
    .bind(id)
    .execute(&mut **tx)
    .await
    .context("resolve mediation confirmation")?;
    Ok(result.rows_affected())
}

/// Find pending confirmations past their deadline. The caller is responsible
/// for processing each one atomically (expire + settle in a single tx).
pub async fn find_pending_past_deadline(
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> Result<Vec<PendingExpiredConfirmation>> {
    let now_str = now.to_rfc3339();
    let rows = sqlx::query(
        r#"SELECT id, mediation_record_id FROM lead_mediation_confirmations
           WHERE status = 'pending' AND deadline_at IS NOT NULL AND deadline_at < ?1"#,
    )
    .bind(&now_str)
    .fetch_all(pool)
    .await
    .context("find pending past-deadline confirmations")?;

    Ok(rows
        .iter()
        .map(|r| PendingExpiredConfirmation {
            id: r.get("id"),
            mediation_record_id: r.get("mediation_record_id"),
        })
        .collect())
}

/// Expire a single confirmation within an existing transaction.
/// Used by the watchdog to atomically expire + settle in one tx.
pub async fn expire_one_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    now: DateTime<Utc>,
) -> Result<u64> {
    let result = sqlx::query(
        r#"UPDATE lead_mediation_confirmations
           SET status = 'expired', resolved_at = ?1
           WHERE id = ?2 AND status = 'pending'"#,
    )
    .bind(now.to_rfc3339())
    .bind(id)
    .execute(&mut **tx)
    .await
    .context("expire single confirmation (tx)")?;
    Ok(result.rows_affected())
}

/// Result of find_pending_past_deadline with the fields needed for settlement.
pub struct PendingExpiredConfirmation {
    pub id: String,
    pub mediation_record_id: String,
}

fn parse_row(r: &sqlx::sqlite::SqliteRow) -> Result<LeadMediationConfirmation> {
    let status_str: String = r.get("status");
    let status: MediationConfirmationStatus =
        status_str.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let requested_at = DateTime::parse_from_rfc3339(r.get::<String, _>("requested_at").as_str())
        .context("parse requested_at")?
        .with_timezone(&Utc);
    let deadline_at = r
        .get::<Option<String>, _>("deadline_at")
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .context("parse deadline_at")
                .map(|dt| dt.with_timezone(&Utc))
        })
        .transpose()?;
    let resolved_at = r
        .get::<Option<String>, _>("resolved_at")
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .context("parse resolved_at")
                .map(|dt| dt.with_timezone(&Utc))
        })
        .transpose()?;

    Ok(LeadMediationConfirmation {
        id: r.get("id"),
        mediation_record_id: r.get("mediation_record_id"),
        run_id: r.get("run_id"),
        conflict_id: r.get("conflict_id"),
        conflict_fingerprint: r.get("conflict_fingerprint"),
        status,
        suggested_action: r.get("suggested_action"),
        requested_at,
        deadline_at,
        readback_ref: r.get("readback_ref"),
        idempotency_scope_key: r.get("idempotency_scope_key"),
        resolved_at,
        resolved_by_principal_id: r.get("resolved_by_principal_id"),
        resolution_decision: r.get("resolution_decision"),
        resolution_comment: r.get("resolution_comment"),
    })
}
