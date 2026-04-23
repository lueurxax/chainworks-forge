use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::ids::RunId;
use domain::workflow_conflict::{
    ImplementationHandoffStatus, WorkflowAdvisoryRejectionRecord, WorkflowConflictRecord,
    WorkflowConflictStatus, WorkflowTransitionCursorRecord,
};

pub async fn upsert_conflict_by_fingerprint(
    pool: &SqlitePool,
    record: &WorkflowConflictRecord,
) -> Result<WorkflowConflictRecord> {
    let mut tx = pool.begin().await?;
    let stored = upsert_conflict_by_fingerprint_tx(&mut tx, record).await?;
    tx.commit().await?;
    Ok(stored)
}

pub async fn upsert_conflict_by_fingerprint_tx(
    tx: &mut Transaction<'_, Sqlite>,
    record: &WorkflowConflictRecord,
) -> Result<WorkflowConflictRecord> {
    let existing = sqlx::query(
        r#"SELECT record_json
           FROM workflow_conflicts
           WHERE run_id = ?1 AND current_state_id = ?2 AND conflict_fingerprint = ?3"#,
    )
    .bind(&record.run_id)
    .bind(&record.current_state_id)
    .bind(&record.conflict_fingerprint)
    .fetch_optional(&mut **tx)
    .await
    .context("find workflow conflict by fingerprint")?;

    let stored = if let Some(row) = existing {
        let raw: String = row.get("record_json");
        let existing_record: WorkflowConflictRecord =
            serde_json::from_str(&raw).context("decode existing workflow conflict record")?;
        let mut updated = record.clone();
        updated.conflict_id = existing_record.conflict_id;
        updated.created_at = existing_record.created_at;
        write_conflict_update_tx(tx, &updated).await?;
        updated
    } else {
        write_conflict_insert_tx(tx, record).await?;
        record.clone()
    };

    Ok(stored)
}

pub async fn insert_advisory_rejection(
    pool: &SqlitePool,
    record: &WorkflowAdvisoryRejectionRecord,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO workflow_advisory_rejections
           (rejection_id, run_id, stage_execution_id, lineage_id, current_state_id,
            selected_transition_id, selected_next_state_id, advisory_next_stage_hint,
            advisory_next_action, advisory_hint_hash, advisory_hint_provenance_json,
            graph_membership_result, created_at, record_json)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
    )
    .bind(&record.rejection_id)
    .bind(&record.run_id)
    .bind(&record.stage_execution_id)
    .bind(&record.lineage_id)
    .bind(&record.current_state_id)
    .bind(&record.selected_transition_id)
    .bind(&record.selected_next_state_id)
    .bind(&record.advisory_next_stage_hint)
    .bind(&record.advisory_next_action)
    .bind(&record.advisory_hint_hash)
    .bind(serde_json::to_string(&record.advisory_hint_provenance)?)
    .bind(&record.graph_membership_result)
    .bind(record.created_at.to_rfc3339())
    .bind(serde_json::to_string(record)?)
    .execute(pool)
    .await
    .context("insert workflow advisory rejection")?;
    Ok(())
}

pub async fn get_current_blocking_conflict(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<WorkflowConflictRecord>> {
    let statuses: Vec<String> = current_blocking_statuses()
        .iter()
        .map(ToString::to_string)
        .collect();
    let row = sqlx::query(
        r#"SELECT record_json
           FROM workflow_conflicts
           WHERE run_id = ?1 AND status IN (?2, ?3, ?4)
           ORDER BY updated_at DESC, created_at DESC
           LIMIT 1"#,
    )
    .bind(run_id.to_string())
    .bind(&statuses[0])
    .bind(&statuses[1])
    .bind(&statuses[2])
    .fetch_optional(pool)
    .await
    .context("get current blocking workflow conflict")?;

    row.map(|row| decode_conflict_row(&row)).transpose()
}

pub async fn list_conflict_history_for_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<WorkflowConflictRecord>> {
    let rows = sqlx::query(
        r#"SELECT record_json
           FROM workflow_conflicts
           WHERE run_id = ?1
           ORDER BY created_at ASC, updated_at ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await
    .context("list workflow conflict history for run")?;

    rows.iter().map(decode_conflict_row).collect()
}

pub async fn list_advisory_rejections_for_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<WorkflowAdvisoryRejectionRecord>> {
    let rows = sqlx::query(
        r#"SELECT record_json
           FROM workflow_advisory_rejections
           WHERE run_id = ?1
           ORDER BY created_at ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await
    .context("list workflow advisory rejections for run")?;

    rows.iter().map(decode_rejection_row).collect()
}

pub async fn upsert_implementation_handoff_status(
    pool: &SqlitePool,
    status: &ImplementationHandoffStatus,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO implementation_handoff_statuses
           (run_id, current_state_id, task_name, code_writer_start_status,
            status, updated_at, record_json)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
           ON CONFLICT(run_id) DO UPDATE SET
             current_state_id = excluded.current_state_id,
             task_name = excluded.task_name,
             code_writer_start_status = excluded.code_writer_start_status,
             status = excluded.status,
             updated_at = excluded.updated_at,
             record_json = excluded.record_json"#,
    )
    .bind(&status.run_id)
    .bind(&status.current_state_id)
    .bind(&status.task_name)
    .bind(&status.code_writer_start_status)
    .bind(&status.status)
    .bind(status.updated_at.to_rfc3339())
    .bind(serde_json::to_string(status)?)
    .execute(pool)
    .await
    .context("upsert implementation handoff status")?;
    Ok(())
}

pub async fn get_implementation_handoff_status(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<ImplementationHandoffStatus>> {
    let row = sqlx::query(
        r#"SELECT record_json
           FROM implementation_handoff_statuses
           WHERE run_id = ?1"#,
    )
    .bind(run_id.to_string())
    .fetch_optional(pool)
    .await
    .context("get implementation handoff status")?;

    row.map(|row| {
        let raw: String = row.get("record_json");
        serde_json::from_str(&raw).context("decode ImplementationHandoffStatus")
    })
    .transpose()
}

pub async fn upsert_transition_cursor(
    pool: &SqlitePool,
    cursor: &WorkflowTransitionCursorRecord,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO workflow_transition_cursors
           (run_id, current_state_id, cursor_status, resume_policy, selected_transition_id,
            selected_next_state_id, conflict_id, conflict_fingerprint, candidate_transition_hash,
            terminal_failure_reason, updated_at, record_json)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
           ON CONFLICT(run_id) DO UPDATE SET
             current_state_id = excluded.current_state_id,
             cursor_status = excluded.cursor_status,
             resume_policy = excluded.resume_policy,
             selected_transition_id = excluded.selected_transition_id,
             selected_next_state_id = excluded.selected_next_state_id,
             conflict_id = excluded.conflict_id,
             conflict_fingerprint = excluded.conflict_fingerprint,
             candidate_transition_hash = excluded.candidate_transition_hash,
             terminal_failure_reason = excluded.terminal_failure_reason,
             updated_at = excluded.updated_at,
             record_json = excluded.record_json"#,
    )
    .bind(&cursor.run_id)
    .bind(&cursor.current_state_id)
    .bind(&cursor.cursor_status)
    .bind(&cursor.resume_policy)
    .bind(&cursor.selected_transition_id)
    .bind(&cursor.selected_next_state_id)
    .bind(&cursor.conflict_id)
    .bind(&cursor.conflict_fingerprint)
    .bind(&cursor.candidate_transition_hash)
    .bind(&cursor.terminal_failure_reason)
    .bind(cursor.updated_at.to_rfc3339())
    .bind(serde_json::to_string(cursor)?)
    .execute(pool)
    .await
    .context("upsert workflow transition cursor")?;
    Ok(())
}

pub async fn get_transition_cursor(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<WorkflowTransitionCursorRecord>> {
    let row = sqlx::query(
        r#"SELECT record_json
           FROM workflow_transition_cursors
           WHERE run_id = ?1"#,
    )
    .bind(run_id.to_string())
    .fetch_optional(pool)
    .await
    .context("get workflow transition cursor")?;

    row.map(|row| {
        let raw: String = row.get("record_json");
        serde_json::from_str(&raw).context("decode WorkflowTransitionCursorRecord")
    })
    .transpose()
}

pub async fn transition_conflict_status(
    pool: &SqlitePool,
    conflict_id: &str,
    status: WorkflowConflictStatus,
    transitioned_at: DateTime<Utc>,
    resolution_record_json: Option<serde_json::Value>,
    terminal_failure_reason: Option<String>,
    superseded_by_conflict_id: Option<String>,
) -> Result<WorkflowConflictRecord> {
    let row = sqlx::query(
        r#"SELECT record_json
           FROM workflow_conflicts
           WHERE conflict_id = ?1"#,
    )
    .bind(conflict_id)
    .fetch_one(pool)
    .await
    .context("find workflow conflict for status transition")?;
    let mut record = decode_conflict_row(&row)?;

    record.status = status;
    record.updated_at = transitioned_at;
    record.resolved_at = if matches!(record.status, WorkflowConflictStatus::Resolved) {
        Some(transitioned_at)
    } else {
        record.resolved_at
    };
    record.resolution_record_json = resolution_record_json;
    record.terminal_failure_reason = terminal_failure_reason;
    record.superseded_by_conflict_id = superseded_by_conflict_id;

    write_conflict_update_pool(pool, &record).await?;
    Ok(record)
}

fn current_blocking_statuses() -> [WorkflowConflictStatus; 3] {
    [
        WorkflowConflictStatus::Unresolved,
        WorkflowConflictStatus::LeadMediationPending,
        WorkflowConflictStatus::OperatorConfirmationRequired,
    ]
}

async fn write_conflict_insert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    record: &WorkflowConflictRecord,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO workflow_conflicts
           (conflict_id, conflict_fingerprint, run_id, stage_execution_id, lineage_id,
            current_state_id, reason, operator_label, status, candidate_transitions_json,
            candidate_transition_hash, advisory_evidence_refs_json, lead_agent_id,
            mediation_record_id, created_at, updated_at, resolved_at,
            superseded_by_conflict_id, resolution_record_json, terminal_failure_reason,
            diagnostic_redaction_tier, record_json)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"#,
    )
    .bind(&record.conflict_id)
    .bind(&record.conflict_fingerprint)
    .bind(&record.run_id)
    .bind(&record.stage_execution_id)
    .bind(&record.lineage_id)
    .bind(&record.current_state_id)
    .bind(record.reason.to_string())
    .bind(&record.operator_label)
    .bind(record.status.to_string())
    .bind(serde_json::to_string(&record.candidate_transitions)?)
    .bind(&record.candidate_transition_hash)
    .bind(serde_json::to_string(&record.advisory_evidence_refs)?)
    .bind(&record.lead_agent_id)
    .bind(&record.mediation_record_id)
    .bind(record.created_at.to_rfc3339())
    .bind(record.updated_at.to_rfc3339())
    .bind(record.resolved_at.map(|dt| dt.to_rfc3339()))
    .bind(&record.superseded_by_conflict_id)
    .bind(record.resolution_record_json.as_ref().map(ToString::to_string))
    .bind(&record.terminal_failure_reason)
    .bind(&record.diagnostic_redaction_tier)
    .bind(serde_json::to_string(record)?)
    .execute(&mut **tx)
    .await
    .context("insert workflow conflict")?;
    Ok(())
}

async fn write_conflict_update_tx(
    tx: &mut Transaction<'_, Sqlite>,
    record: &WorkflowConflictRecord,
) -> Result<()> {
    sqlx::query(conflict_update_sql())
        .bind(&record.conflict_fingerprint)
        .bind(&record.run_id)
        .bind(&record.stage_execution_id)
        .bind(&record.lineage_id)
        .bind(&record.current_state_id)
        .bind(record.reason.to_string())
        .bind(&record.operator_label)
        .bind(record.status.to_string())
        .bind(serde_json::to_string(&record.candidate_transitions)?)
        .bind(&record.candidate_transition_hash)
        .bind(serde_json::to_string(&record.advisory_evidence_refs)?)
        .bind(&record.lead_agent_id)
        .bind(&record.mediation_record_id)
        .bind(record.updated_at.to_rfc3339())
        .bind(record.resolved_at.map(|dt| dt.to_rfc3339()))
        .bind(&record.superseded_by_conflict_id)
        .bind(
            record
                .resolution_record_json
                .as_ref()
                .map(ToString::to_string),
        )
        .bind(&record.terminal_failure_reason)
        .bind(&record.diagnostic_redaction_tier)
        .bind(serde_json::to_string(record)?)
        .bind(&record.conflict_id)
        .execute(&mut **tx)
        .await
        .context("update workflow conflict")?;
    Ok(())
}

async fn write_conflict_update_pool(
    pool: &SqlitePool,
    record: &WorkflowConflictRecord,
) -> Result<()> {
    sqlx::query(conflict_update_sql())
        .bind(&record.conflict_fingerprint)
        .bind(&record.run_id)
        .bind(&record.stage_execution_id)
        .bind(&record.lineage_id)
        .bind(&record.current_state_id)
        .bind(record.reason.to_string())
        .bind(&record.operator_label)
        .bind(record.status.to_string())
        .bind(serde_json::to_string(&record.candidate_transitions)?)
        .bind(&record.candidate_transition_hash)
        .bind(serde_json::to_string(&record.advisory_evidence_refs)?)
        .bind(&record.lead_agent_id)
        .bind(&record.mediation_record_id)
        .bind(record.updated_at.to_rfc3339())
        .bind(record.resolved_at.map(|dt| dt.to_rfc3339()))
        .bind(&record.superseded_by_conflict_id)
        .bind(
            record
                .resolution_record_json
                .as_ref()
                .map(ToString::to_string),
        )
        .bind(&record.terminal_failure_reason)
        .bind(&record.diagnostic_redaction_tier)
        .bind(serde_json::to_string(record)?)
        .bind(&record.conflict_id)
        .execute(pool)
        .await
        .context("update workflow conflict")?;
    Ok(())
}

fn conflict_update_sql() -> &'static str {
    r#"UPDATE workflow_conflicts
       SET conflict_fingerprint = ?1,
           run_id = ?2,
           stage_execution_id = ?3,
           lineage_id = ?4,
           current_state_id = ?5,
           reason = ?6,
           operator_label = ?7,
           status = ?8,
           candidate_transitions_json = ?9,
           candidate_transition_hash = ?10,
           advisory_evidence_refs_json = ?11,
           lead_agent_id = ?12,
           mediation_record_id = ?13,
           updated_at = ?14,
           resolved_at = ?15,
           superseded_by_conflict_id = ?16,
           resolution_record_json = ?17,
           terminal_failure_reason = ?18,
           diagnostic_redaction_tier = ?19,
           record_json = ?20
       WHERE conflict_id = ?21"#
}

fn decode_conflict_row(row: &sqlx::sqlite::SqliteRow) -> Result<WorkflowConflictRecord> {
    let raw: String = row.get("record_json");
    serde_json::from_str(&raw).context("decode WorkflowConflictRecord")
}

fn decode_rejection_row(row: &sqlx::sqlite::SqliteRow) -> Result<WorkflowAdvisoryRejectionRecord> {
    let raw: String = row.get("record_json");
    serde_json::from_str(&raw).context("decode WorkflowAdvisoryRejectionRecord")
}
