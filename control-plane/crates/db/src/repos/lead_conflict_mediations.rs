//! P017 Phase B: Repository for `lead_conflict_mediations` table.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::mediation::{LeadConflictMediationRecord, LeadMediationStatus};

pub async fn insert(pool: &SqlitePool, record: &LeadConflictMediationRecord) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "lead_conflict_mediations.insert",
        sqlx::query(
            r#"INSERT INTO lead_conflict_mediations
           (id, run_id, conflict_id, conflict_fingerprint, lead_agent_id, status,
            settlement_result, recovery_action, chosen_action, chosen_next_state_id,
            chosen_next_state_label, operator_rationale, sanitized_progress,
            validation_errors_json, cost_summary_json, metric_event_id,
            superseded_by_event_ref, agent_execution_id, confirmation_subject_id,
            created_at, updated_at, settled_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                   ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"#,
        )
        .bind(&record.id)
        .bind(&record.run_id)
        .bind(&record.conflict_id)
        .bind(&record.conflict_fingerprint)
        .bind(&record.lead_agent_id)
        .bind(record.status.to_string())
        .bind(&record.settlement_result)
        .bind(&record.recovery_action)
        .bind(&record.chosen_action)
        .bind(&record.chosen_next_state_id)
        .bind(&record.chosen_next_state_label)
        .bind(&record.operator_rationale)
        .bind(&record.sanitized_progress)
        .bind(&record.validation_errors_json)
        .bind(&record.cost_summary_json)
        .bind(&record.metric_event_id)
        .bind(&record.superseded_by_event_ref)
        .bind(&record.agent_execution_id)
        .bind(&record.confirmation_subject_id)
        .bind(record.created_at.to_rfc3339())
        .bind(record.updated_at.to_rfc3339())
        .bind(record.settled_at.map(|t| t.to_rfc3339()))
    )
    .context("insert lead_conflict_mediation")?;
    Ok(())
}

pub async fn insert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    record: &LeadConflictMediationRecord,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO lead_conflict_mediations
           (id, run_id, conflict_id, conflict_fingerprint, lead_agent_id, status,
            settlement_result, recovery_action, chosen_action, chosen_next_state_id,
            chosen_next_state_label, operator_rationale, sanitized_progress,
            validation_errors_json, cost_summary_json, metric_event_id,
            superseded_by_event_ref, agent_execution_id, confirmation_subject_id,
            created_at, updated_at, settled_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                   ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"#,
    )
    .bind(&record.id)
    .bind(&record.run_id)
    .bind(&record.conflict_id)
    .bind(&record.conflict_fingerprint)
    .bind(&record.lead_agent_id)
    .bind(record.status.to_string())
    .bind(&record.settlement_result)
    .bind(&record.recovery_action)
    .bind(&record.chosen_action)
    .bind(&record.chosen_next_state_id)
    .bind(&record.chosen_next_state_label)
    .bind(&record.operator_rationale)
    .bind(&record.sanitized_progress)
    .bind(&record.validation_errors_json)
    .bind(&record.cost_summary_json)
    .bind(&record.metric_event_id)
    .bind(&record.superseded_by_event_ref)
    .bind(&record.agent_execution_id)
    .bind(&record.confirmation_subject_id)
    .bind(record.created_at.to_rfc3339())
    .bind(record.updated_at.to_rfc3339())
    .bind(record.settled_at.map(|t| t.to_rfc3339()))
    .execute(&mut **tx)
    .await
    .context("insert lead_conflict_mediation (tx)")?;
    Ok(())
}

pub async fn find_active_for_conflict_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    conflict_fingerprint: &str,
) -> Result<Option<LeadConflictMediationRecord>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, conflict_id, conflict_fingerprint, lead_agent_id, status,
                  settlement_result, recovery_action, chosen_action, chosen_next_state_id,
                  chosen_next_state_label, operator_rationale, sanitized_progress,
                  validation_errors_json, cost_summary_json, metric_event_id,
                  superseded_by_event_ref, agent_execution_id, confirmation_subject_id,
                  created_at, updated_at, settled_at
           FROM lead_conflict_mediations
           WHERE run_id = ?1 AND conflict_fingerprint = ?2
             AND status NOT IN ('settled', 'terminal_unverifiable', 'canceled', 'superseded')
           LIMIT 1"#,
    )
    .bind(run_id)
    .bind(conflict_fingerprint)
    .fetch_optional(&mut **tx)
    .await
    .context("find active mediation for conflict (tx)")?;

    row.map(|r| parse_row(&r)).transpose()
}

pub async fn find_by_id(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<LeadConflictMediationRecord>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, conflict_id, conflict_fingerprint, lead_agent_id, status,
                  settlement_result, recovery_action, chosen_action, chosen_next_state_id,
                  chosen_next_state_label, operator_rationale, sanitized_progress,
                  validation_errors_json, cost_summary_json, metric_event_id,
                  superseded_by_event_ref, agent_execution_id, confirmation_subject_id,
                  created_at, updated_at, settled_at
           FROM lead_conflict_mediations WHERE id = ?1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("find lead_conflict_mediation by id")?;

    row.map(|r| parse_row(&r)).transpose()
}

pub async fn find_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
) -> Result<Option<LeadConflictMediationRecord>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, conflict_id, conflict_fingerprint, lead_agent_id, status,
                  settlement_result, recovery_action, chosen_action, chosen_next_state_id,
                  chosen_next_state_label, operator_rationale, sanitized_progress,
                  validation_errors_json, cost_summary_json, metric_event_id,
                  superseded_by_event_ref, agent_execution_id, confirmation_subject_id,
                  created_at, updated_at, settled_at
           FROM lead_conflict_mediations WHERE id = ?1"#,
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
    .context("find lead_conflict_mediation by id (tx)")?;

    row.map(|r| parse_row(&r)).transpose()
}

pub async fn find_active_for_conflict(
    pool: &SqlitePool,
    run_id: &str,
    conflict_fingerprint: &str,
) -> Result<Option<LeadConflictMediationRecord>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, conflict_id, conflict_fingerprint, lead_agent_id, status,
                  settlement_result, recovery_action, chosen_action, chosen_next_state_id,
                  chosen_next_state_label, operator_rationale, sanitized_progress,
                  validation_errors_json, cost_summary_json, metric_event_id,
                  superseded_by_event_ref, agent_execution_id, confirmation_subject_id,
                  created_at, updated_at, settled_at
           FROM lead_conflict_mediations
           WHERE run_id = ?1 AND conflict_fingerprint = ?2
             AND status NOT IN ('settled', 'terminal_unverifiable', 'canceled', 'superseded')
           LIMIT 1"#,
    )
    .bind(run_id)
    .bind(conflict_fingerprint)
    .fetch_optional(pool)
    .await
    .context("find active mediation for conflict")?;

    row.map(|r| parse_row(&r)).transpose()
}

pub async fn list_by_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<LeadConflictMediationRecord>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, conflict_id, conflict_fingerprint, lead_agent_id, status,
                  settlement_result, recovery_action, chosen_action, chosen_next_state_id,
                  chosen_next_state_label, operator_rationale, sanitized_progress,
                  validation_errors_json, cost_summary_json, metric_event_id,
                  superseded_by_event_ref, agent_execution_id, confirmation_subject_id,
                  created_at, updated_at, settled_at
           FROM lead_conflict_mediations WHERE run_id = ?1 ORDER BY created_at ASC"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("list mediations by run")?;

    rows.iter().map(|r| parse_row(r)).collect()
}

/// Update mediation status within an existing transaction.
/// Returns rows_affected so callers can distinguish success (1) from
/// concurrent no-op (0) when the terminal-status guard blocks the update.
pub async fn update_status_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    status: &str,
    settlement_result: Option<&str>,
    recovery_action: Option<&str>,
    now: DateTime<Utc>,
) -> Result<u64> {
    let settled_at = if matches!(
        status,
        "settled" | "terminal_unverifiable" | "canceled" | "superseded"
    ) {
        Some(now.to_rfc3339())
    } else {
        None
    };

    // BLK-002: Terminal-status guard prevents TOCTOU race where expiry
    // could overwrite a concurrent settlement. Only update if the mediation
    // is not already in a terminal state.
    let result = sqlx::query(
        r#"UPDATE lead_conflict_mediations
           SET status = ?1, settlement_result = COALESCE(?2, settlement_result),
               recovery_action = COALESCE(?3, recovery_action),
               updated_at = ?4, settled_at = COALESCE(?5, settled_at)
           WHERE id = ?6
             AND status NOT IN ('settled', 'terminal_unverifiable', 'canceled', 'superseded')"#,
    )
    .bind(status)
    .bind(settlement_result)
    .bind(recovery_action)
    .bind(now.to_rfc3339())
    .bind(settled_at)
    .bind(id)
    .execute(&mut **tx)
    .await
    .context("update mediation status")?;

    let rows = result.rows_affected();
    if rows == 0 {
        tracing::warn!(
            mediation_id = %id,
            target_status = %status,
            "update_status_tx: no rows updated — mediation already in terminal state"
        );
    }

    Ok(rows)
}

/// Transition every non-terminal mediation for `run_id` to `canceled`.
///
/// REL-001 (P017 R2 audit): the run cancellation cascade must keep
/// `lead_conflict_mediations` truth synchronized with `agent_executions`
/// when a run is cancelled. Otherwise a mediation can linger in
/// `queued` / `running` / `operator_confirmation_required` while the
/// owning `agent_execution` is already `canceled`, which splits durable
/// truth between the two tables and corrupts late-output, resume, and
/// operator readback semantics.
///
/// Runs in the same transaction as `agent_executions::cancel_running_by_run_tx`
/// so the two updates are atomic.
///
/// Idempotent: rows already in a terminal state are skipped by the
/// `status NOT IN (...)` guard. Returns the number of mediations that
/// were transitioned (0 = nothing was active).
pub async fn cancel_active_by_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    now: DateTime<Utc>,
) -> Result<u64> {
    let now_str = now.to_rfc3339();
    let result = sqlx::query(
        r#"UPDATE lead_conflict_mediations
           SET status = 'canceled',
               settlement_result = COALESCE(settlement_result, 'cancelled'),
               recovery_action = COALESCE(recovery_action, 'run_cancelled'),
               updated_at = ?1,
               settled_at = ?1
           WHERE run_id = ?2
             AND status NOT IN ('settled', 'terminal_unverifiable', 'canceled', 'superseded')"#,
    )
    .bind(&now_str)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .context("cancel active mediations by run (tx)")?;
    Ok(result.rows_affected())
}

pub async fn update_after_lead_output_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    status: &str,
    settlement_result: Option<&str>,
    recovery_action: Option<&str>,
    chosen_action: Option<&str>,
    chosen_next_state_id: Option<&str>,
    chosen_next_state_label: Option<&str>,
    sanitized_progress: Option<&str>,
    validation_errors_json: Option<&str>,
    confirmation_subject_id: Option<&str>,
    now: DateTime<Utc>,
) -> Result<u64> {
    let settled_at = if matches!(
        status,
        "settled" | "terminal_unverifiable" | "canceled" | "superseded"
    ) {
        Some(now.to_rfc3339())
    } else {
        None
    };

    let result = sqlx::query(
        r#"UPDATE lead_conflict_mediations
           SET status = ?1,
               settlement_result = COALESCE(?2, settlement_result),
               recovery_action = COALESCE(?3, recovery_action),
               chosen_action = COALESCE(?4, chosen_action),
               chosen_next_state_id = COALESCE(?5, chosen_next_state_id),
               chosen_next_state_label = COALESCE(?6, chosen_next_state_label),
               sanitized_progress = COALESCE(?7, sanitized_progress),
               validation_errors_json = COALESCE(?8, validation_errors_json),
               confirmation_subject_id = COALESCE(?9, confirmation_subject_id),
               updated_at = ?10,
               settled_at = COALESCE(?11, settled_at)
           WHERE id = ?12
             AND status NOT IN ('settled', 'terminal_unverifiable', 'canceled', 'superseded')"#,
    )
    .bind(status)
    .bind(settlement_result)
    .bind(recovery_action)
    .bind(chosen_action)
    .bind(chosen_next_state_id)
    .bind(chosen_next_state_label)
    .bind(sanitized_progress)
    .bind(validation_errors_json)
    .bind(confirmation_subject_id)
    .bind(now.to_rfc3339())
    .bind(settled_at)
    .bind(id)
    .execute(&mut **tx)
    .await
    .context("update mediation after lead output")?;

    Ok(result.rows_affected())
}

fn parse_row(r: &sqlx::sqlite::SqliteRow) -> Result<LeadConflictMediationRecord> {
    let status_str: String = r.get("status");
    let status: LeadMediationStatus = status_str.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let created_at = DateTime::parse_from_rfc3339(r.get::<String, _>("created_at").as_str())
        .context("parse created_at")?
        .with_timezone(&Utc);
    let updated_at = DateTime::parse_from_rfc3339(r.get::<String, _>("updated_at").as_str())
        .context("parse updated_at")?
        .with_timezone(&Utc);
    let settled_at = r
        .get::<Option<String>, _>("settled_at")
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .context("parse settled_at")
                .map(|dt| dt.with_timezone(&Utc))
        })
        .transpose()?;

    Ok(LeadConflictMediationRecord {
        id: r.get("id"),
        run_id: r.get("run_id"),
        conflict_id: r.get("conflict_id"),
        conflict_fingerprint: r.get("conflict_fingerprint"),
        lead_agent_id: r.get("lead_agent_id"),
        status,
        settlement_result: r.get("settlement_result"),
        recovery_action: r.get("recovery_action"),
        chosen_action: r.get("chosen_action"),
        chosen_next_state_id: r.get("chosen_next_state_id"),
        chosen_next_state_label: r.get("chosen_next_state_label"),
        operator_rationale: r.get("operator_rationale"),
        sanitized_progress: r.get("sanitized_progress"),
        validation_errors_json: r.get("validation_errors_json"),
        cost_summary_json: r.get("cost_summary_json"),
        metric_event_id: r.get("metric_event_id"),
        superseded_by_event_ref: r.get("superseded_by_event_ref"),
        agent_execution_id: r.get("agent_execution_id"),
        confirmation_subject_id: r.get("confirmation_subject_id"),
        created_at,
        updated_at,
        settled_at,
    })
}
