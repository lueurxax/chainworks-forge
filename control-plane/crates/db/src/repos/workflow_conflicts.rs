use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use domain::ids::RunId;
use domain::workflow_conflict::{
    ImplementationHandoffStatus, WorkflowAdvisoryRejectionRecord, WorkflowConflictRecord,
    WorkflowConflictStatus, WorkflowTransitionCursorRecord,
};

#[derive(Clone, Debug, PartialEq)]
pub struct WorkflowConflictMetricEvent {
    pub event_id: String,
    /// Owning run id. Nullable since migration 031 to support
    /// daemon-level events that fire before a run row exists (e.g.
    /// Phase C compile-fail outcome).
    pub run_id: Option<String>,
    pub conflict_id: Option<String>,
    pub metric_name: String,
    pub labels_json: serde_json::Value,
    pub value: f64,
    pub unit: String,
    pub occurred_at: DateTime<Utc>,
}

pub async fn upsert_conflict_by_fingerprint(
    pool: &SqlitePool,
    record: &WorkflowConflictRecord,
) -> Result<WorkflowConflictRecord> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "workflow_conflicts.upsert_conflict_by_fingerprint",
    )
    .await?;
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

    supersede_current_blocking_conflicts_tx(tx, &stored).await?;

    // OPS-003 (P017 R5): emit `workflow_conflict_current_total` per
    // upsert keyed by (reason, status) so rollout dashboards can count
    // current conflicts by classification. Bounded label cardinality:
    // reason ∈ enum (8), status ∈ enum (6) ⇒ 48 unique label tuples max.
    record_workflow_conflict_current_tx(tx, &stored, stored.updated_at).await?;

    Ok(stored)
}

pub async fn insert_advisory_rejection(
    pool: &SqlitePool,
    record: &WorkflowAdvisoryRejectionRecord,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "workflow_conflicts.insert_advisory_rejection",
    )
    .await?;
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
    .execute(&mut **tx)
    .await
    .context("insert workflow advisory rejection")?;
    // OPS-003 (P017 R5): every durable advisory rejection record emits
    // `advisory_rejection_total`. When the advisory hint was rejected
    // because the next-stage hint was absent from the graph, also emit
    // `invalid_next_stage_hint_non_blocking_total` so dashboards can
    // distinguish total volume from invalid-hint volume.
    record_advisory_rejection_tx(
        &mut tx,
        &record.run_id,
        &record.current_state_id,
        &record.graph_membership_result,
        record.created_at,
    )
    .await?;
    if record.graph_membership_result == "absent_from_graph" {
        record_invalid_next_stage_hint_non_blocking_tx(
            &mut tx,
            &record.run_id,
            &record.current_state_id,
            record.advisory_next_action.as_deref(),
            record.created_at,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn get_current_blocking_conflict(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<WorkflowConflictRecord>> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "workflow_conflicts.get_current_blocking_conflict",
    )
    .await?;
    let record = get_current_blocking_conflict_tx(&mut tx, run_id).await?;
    tx.commit().await?;
    Ok(record)
}

pub async fn get_current_blocking_conflict_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
) -> Result<Option<WorkflowConflictRecord>> {
    let statuses = current_blocking_statuses();
    assert_eq!(
        statuses.len(),
        3,
        "workflow_conflicts SQL expects exactly three current blocking statuses"
    );
    let row = sqlx::query(
        r#"SELECT record_json
           FROM workflow_conflicts
           WHERE run_id = ?1 AND status IN (?2, ?3, ?4)
           ORDER BY updated_at DESC, created_at DESC
           LIMIT 1"#,
    )
    .bind(run_id.to_string())
    .bind(statuses[0].to_string())
    .bind(statuses[1].to_string())
    .bind(statuses[2].to_string())
    .fetch_optional(&mut **tx)
    .await
    .context("get current blocking workflow conflict")?;

    row.map(|row| decode_conflict_row(&row)).transpose()
}

/// Batch-fetch the current blocking conflict for each run_id in the list.
/// Returns a HashMap keyed by run_id string. Runs without a blocking conflict
/// are absent from the map. Designed for the runs-list query to avoid N+1 lookups.
pub async fn get_blocking_conflicts_for_runs(
    pool: &SqlitePool,
    run_ids: &[String],
) -> Result<std::collections::HashMap<String, WorkflowConflictRecord>> {
    if run_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let statuses = current_blocking_statuses();
    // Build a single query using a VALUES list for the run_id IN clause.
    // SQLite supports parameterized IN only by repeating ? placeholders.
    let placeholders = run_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let status_start = run_ids.len() + 1;
    let sql = format!(
        "SELECT run_id, record_json FROM workflow_conflicts \
         WHERE run_id IN ({}) AND status IN (?{}, ?{}, ?{}) \
         ORDER BY run_id, updated_at DESC, created_at DESC",
        placeholders,
        status_start,
        status_start + 1,
        status_start + 2,
    );
    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()));
    for id in run_ids {
        q = q.bind(id);
    }
    q = q.bind(statuses[0].to_string());
    q = q.bind(statuses[1].to_string());
    q = q.bind(statuses[2].to_string());
    let rows = q
        .fetch_all(pool)
        .await
        .context("batch get blocking workflow conflicts")?;

    let mut map: std::collections::HashMap<String, WorkflowConflictRecord> =
        std::collections::HashMap::new();
    for row in &rows {
        let run_id: String = row.try_get("run_id").context("run_id column")?;
        if map.contains_key(&run_id) {
            continue; // Keep only the first (most-recently-updated) row per run.
        }
        let record = decode_conflict_row(row)?;
        map.insert(run_id, record);
    }
    Ok(map)
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
    crate::execute_repository_write!(
        pool,
        "workflow_conflicts.upsert_implementation_handoff_status",
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
    )
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
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "workflow_conflicts.upsert_transition_cursor",
    )
    .await?;
    upsert_transition_cursor_tx(&mut tx, cursor).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn upsert_transition_cursor_tx(
    tx: &mut Transaction<'_, Sqlite>,
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
    .execute(&mut **tx)
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

pub async fn list_metric_events_for_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<WorkflowConflictMetricEvent>> {
    let rows = sqlx::query(
        r#"SELECT event_id, run_id, conflict_id, metric_name, labels_json,
                  value, unit, occurred_at
           FROM workflow_conflict_metric_events
           WHERE run_id = ?1
           ORDER BY occurred_at ASC, event_id ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await
    .context("list workflow conflict metric events for run")?;

    rows.iter().map(decode_metric_event_row).collect()
}

pub async fn record_recovery_action_chosen_tx(
    tx: &mut Transaction<'_, Sqlite>,
    conflict: &WorkflowConflictRecord,
    action_class: &str,
    source_surface: &str,
    result: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(conflict.run_id.clone()),
            conflict_id: Some(conflict.conflict_id.clone()),
            metric_name: "recovery_action_chosen_total".to_string(),
            labels_json: serde_json::json!({
                "conflict_reason": conflict.reason.to_string(),
                "action_class": action_class,
                "source_surface": source_surface,
                "result": result,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

pub async fn record_phase_c_validation_outcome_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    outcome: &str,
    source: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: None,
            metric_name: "phase_c_validation_outcome_total".to_string(),
            labels_json: serde_json::json!({
                "outcome": outcome,
                "source": source,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-001 (P017 R2 audit): record one `lead_mediation_attempt_total`
/// metric event per mediation-owned `agent_executions` completion.
///
/// `result` summarises the per-attempt outcome — one of:
/// - `validated_awaiting_confirmation`
/// - `lead_output_validation_failed`
/// - `agent_failed`
/// - `cancelled`
/// - `other`
///
/// `attempt_number` is the durable count of mediation-owned executions
/// for this mediation at the point the attempt completes (1, 2, …).
/// `mediation_record_id` keeps attempts grouped per-mediation so the
/// metric remains bounded label-cardinality wise.
pub async fn record_lead_mediation_attempt_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    conflict_id: Option<&str>,
    mediation_record_id: &str,
    lead_agent_id: &str,
    result: &str,
    attempt_number: i64,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: conflict_id.map(|s| s.to_string()),
            metric_name: "lead_mediation_attempt_total".to_string(),
            labels_json: serde_json::json!({
                "result": result,
                "lead_agent_id": lead_agent_id,
                "mediation_record_id": mediation_record_id,
                "attempt_number": attempt_number,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-001 (P017 R2 audit): record one `external_catalog_warning_total`
/// metric event per external-catalog warning decision (e.g. operator
/// attestation that a legacy/external catalog is being kept opt-in for
/// the warning window before Phase C fail-closed enforcement).
///
/// `warning_kind` is the typed warning code (e.g.
/// `P017_PHASE_C_EXTERNAL_CATALOG_UNDISCOVERED`).
/// `decision` is one of `enabled`, `waived`, or `denied`.
/// `source_surface` indicates where the decision was recorded (e.g.
/// `legacy_discovery_override`, `phase_c_inventory`).
pub async fn record_external_catalog_warning_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    warning_kind: &str,
    decision: &str,
    source_surface: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: None,
            metric_name: "external_catalog_warning_total".to_string(),
            labels_json: serde_json::json!({
                "warning_kind": warning_kind,
                "decision": decision,
                "source_surface": source_surface,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-002 (P017 R4 audit): emit `phase_c_validation_outcome_total` for
/// the **fail-closed compile** path. Run id is None because this fires
/// when `workflow::compiler::compile()` returns Err and the run row
/// never gets inserted.
///
/// Migration 031 made `workflow_conflict_metric_events.run_id` NULL-able
/// specifically to support this daemon-level event without an FK
/// violation.
pub async fn record_phase_c_validation_failure_tx(
    tx: &mut Transaction<'_, Sqlite>,
    failure_kind: &str,
    workflow_path: Option<&str>,
    catalog_path: Option<&str>,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: None,
            conflict_id: None,
            metric_name: "phase_c_validation_outcome_total".to_string(),
            labels_json: serde_json::json!({
                "outcome": "fail",
                "source": "compile",
                "failure_kind": failure_kind,
                "workflow_path": workflow_path,
                "catalog_path": catalog_path,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

pub async fn record_phase_c_validation_failure(
    pool: &SqlitePool,
    failure_kind: &str,
    workflow_path: Option<&str>,
    catalog_path: Option<&str>,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "workflow_conflicts.record_phase_c_validation_failure",
    )
    .await?;
    record_phase_c_validation_failure_tx(
        &mut tx,
        failure_kind,
        workflow_path,
        catalog_path,
        occurred_at,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// OPS-002: emit `duplicate_mediation_session_total` when the
/// orchestrator finds an existing active mediation while attempting to
/// create a new one for the same conflict (resume idempotency).
pub async fn record_duplicate_mediation_session_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    conflict_id: &str,
    mediation_record_id: &str,
    detection_source: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: Some(conflict_id.to_string()),
            metric_name: "duplicate_mediation_session_total".to_string(),
            labels_json: serde_json::json!({
                "mediation_record_id": mediation_record_id,
                "detection_source": detection_source,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-002: emit `report_readback_completeness` to record whether a
/// composed `workflow_conflict` readback exposes all expected fields
/// (current conflict, history, advisory rejections, lead owner, valid
/// action class, terminal failure reason) for a given run.
///
/// `value` is a ratio in [0, 1] — fraction of expected fields present.
pub async fn record_report_readback_completeness_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    conflict_id: Option<&str>,
    expected_fields: &[&str],
    present_fields: &[&str],
    surface: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    let ratio = if expected_fields.is_empty() {
        1.0
    } else {
        present_fields.len() as f64 / expected_fields.len() as f64
    };
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: conflict_id.map(|s| s.to_string()),
            metric_name: "report_readback_completeness".to_string(),
            labels_json: serde_json::json!({
                "surface": surface,
                "expected_fields": expected_fields,
                "present_fields": present_fields,
            }),
            value: ratio,
            unit: "ratio".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-002: emit `phase_c_lead_inventory_external_catalog_total` when the
/// daemon evaluates an executable catalog's external-catalog inventory
/// and reaches an enforcement decision.
pub async fn record_phase_c_lead_inventory_external_catalog_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: Option<&str>,
    inventory_result: &str,
    enforcement_decision: &str,
    catalog_path: Option<&str>,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: run_id.map(|s| s.to_string()),
            conflict_id: None,
            metric_name: "phase_c_lead_inventory_external_catalog_total".to_string(),
            labels_json: serde_json::json!({
                "inventory_result": inventory_result,
                "enforcement_decision": enforcement_decision,
                "catalog_path": catalog_path,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-002: emit `mediation_late_output_ignored_total` when a mediation
/// completion arrives after the mediation is already terminal and the
/// late output is ignored.
pub async fn record_mediation_late_output_ignored_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    conflict_id: Option<&str>,
    mediation_record_id: &str,
    reason: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: conflict_id.map(|s| s.to_string()),
            metric_name: "mediation_late_output_ignored_total".to_string(),
            labels_json: serde_json::json!({
                "mediation_record_id": mediation_record_id,
                "reason": reason,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-003 (P017 R5): emit `advisory_rejection_total` per durable
/// advisory rejection record. Bounded labels: `current_state_id`,
/// `graph_membership_result`.
pub async fn record_advisory_rejection_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    current_state_id: &str,
    graph_membership_result: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: None,
            metric_name: "advisory_rejection_total".to_string(),
            labels_json: serde_json::json!({
                "current_state_id": current_state_id,
                "graph_membership_result": graph_membership_result,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-003: emit `invalid_next_stage_hint_non_blocking_total` when an
/// advisory next-stage hint is rejected as non-blocking.
pub async fn record_invalid_next_stage_hint_non_blocking_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    current_state_id: &str,
    advisory_next_action: Option<&str>,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: None,
            metric_name: "invalid_next_stage_hint_non_blocking_total".to_string(),
            labels_json: serde_json::json!({
                "current_state_id": current_state_id,
                "advisory_next_action": advisory_next_action,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-003: emit `workflow_conflict_current_total` per `(reason, status)`
/// when a blocking conflict is upserted. Lets dashboards count current
/// conflicts by reason and status.
pub async fn record_workflow_conflict_current_tx(
    tx: &mut Transaction<'_, Sqlite>,
    record: &WorkflowConflictRecord,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(record.run_id.clone()),
            conflict_id: Some(record.conflict_id.clone()),
            metric_name: "workflow_conflict_current_total".to_string(),
            labels_json: serde_json::json!({
                "reason": record.reason.to_string(),
                "status": record.status.to_string(),
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-003: emit `terminal_unverifiable_total` when a conflict
/// transitions to `TerminalUnverifiable`, labeled by failure reason.
pub async fn record_terminal_unverifiable_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    conflict_id: &str,
    terminal_failure_reason: Option<&str>,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: Some(conflict_id.to_string()),
            metric_name: "terminal_unverifiable_total".to_string(),
            labels_json: serde_json::json!({
                "terminal_failure_reason": terminal_failure_reason,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-002: emit `mediation_retry_budget_exhausted_total` when an
/// owner-aware retry budget for a mediation is exhausted.
pub async fn record_mediation_retry_budget_exhausted_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    mediation_record_id: &str,
    provider_profile_id: Option<&str>,
    conflict_reason: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(run_id.to_string()),
            conflict_id: None,
            metric_name: "mediation_retry_budget_exhausted_total".to_string(),
            labels_json: serde_json::json!({
                "mediation_record_id": mediation_record_id,
                "provider_profile_id": provider_profile_id,
                "conflict_reason": conflict_reason,
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-001: emit the Phase B dogfood mediation completion rate as a runtime
/// metric event, keyed by the workflow/conflict dimensions promised by P017.
pub async fn record_phase_b_dogfood_mediation_completion_rate_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: Option<&str>,
    workflow_id: &str,
    conflict_reason: &str,
    completion_rate: f64,
    sample_size: i64,
    evidence_source: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: run_id.map(ToString::to_string),
            conflict_id: None,
            metric_name: "phase_b_dogfood_mediation_completion_rate".to_string(),
            labels_json: serde_json::json!({
                "workflow_id": workflow_id,
                "conflict_reason": conflict_reason,
                "sample_size": sample_size,
                "evidence_source": evidence_source,
            }),
            value: completion_rate,
            unit: "ratio".to_string(),
            occurred_at,
        },
    )
    .await
}

/// OPS-001: emit the Phase B dogfood operator-guidance sufficiency total as
/// a runtime metric event, keyed by the action/result dimensions in P017.
pub async fn record_phase_b_dogfood_operator_guidance_sufficient_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: Option<&str>,
    action_class: &str,
    result: &str,
    sufficient_count: i64,
    evidence_source: &str,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: run_id.map(ToString::to_string),
            conflict_id: None,
            metric_name: "phase_b_dogfood_operator_guidance_sufficient_total".to_string(),
            labels_json: serde_json::json!({
                "action_class": action_class,
                "result": result,
                "evidence_source": evidence_source,
            }),
            value: sufficient_count as f64,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

pub async fn find_conflict_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    conflict_id: &str,
) -> Result<Option<WorkflowConflictRecord>> {
    let row = sqlx::query(
        r#"SELECT record_json
           FROM workflow_conflicts
           WHERE conflict_id = ?1"#,
    )
    .bind(conflict_id)
    .fetch_optional(&mut **tx)
    .await
    .context("find workflow conflict by id")?;

    row.map(|row| decode_conflict_row(&row)).transpose()
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
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "workflow_conflicts.transition_conflict_status",
    )
    .await?;
    let record = transition_conflict_status_tx(
        &mut tx,
        conflict_id,
        status,
        transitioned_at,
        resolution_record_json,
        terminal_failure_reason,
        superseded_by_conflict_id,
    )
    .await?;
    tx.commit().await?;
    Ok(record)
}

pub async fn transition_conflict_status_tx(
    tx: &mut Transaction<'_, Sqlite>,
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
    .fetch_one(&mut **tx)
    .await
    .context("find workflow conflict for status transition")?;
    let mut record = decode_conflict_row(&row)?;

    record.status = status;
    record.updated_at = transitioned_at;
    record.resolved_at = if matches!(
        record.status,
        WorkflowConflictStatus::Resolved | WorkflowConflictStatus::TerminalUnverifiable
    ) {
        Some(transitioned_at)
    } else {
        record.resolved_at
    };
    record.resolution_record_json = resolution_record_json;
    record.terminal_failure_reason = terminal_failure_reason;
    record.superseded_by_conflict_id = superseded_by_conflict_id;

    write_conflict_update_tx(tx, &record).await?;
    record_terminal_metric_events_tx(tx, &record, transitioned_at).await?;
    Ok(record)
}

fn current_blocking_statuses() -> [WorkflowConflictStatus; 3] {
    [
        WorkflowConflictStatus::Unresolved,
        WorkflowConflictStatus::LeadMediationPending,
        WorkflowConflictStatus::OperatorConfirmationRequired,
    ]
}

fn is_current_blocking_status(status: &WorkflowConflictStatus) -> bool {
    current_blocking_statuses().contains(status)
}

async fn supersede_current_blocking_conflicts_tx(
    tx: &mut Transaction<'_, Sqlite>,
    current: &WorkflowConflictRecord,
) -> Result<()> {
    if !is_current_blocking_status(&current.status) {
        return Ok(());
    }

    let statuses = current_blocking_statuses();
    assert_eq!(
        statuses.len(),
        3,
        "workflow_conflicts SQL expects exactly three current blocking statuses"
    );
    let rows = sqlx::query(
        r#"SELECT record_json
           FROM workflow_conflicts
           WHERE run_id = ?1
             AND current_state_id = ?2
             AND conflict_id != ?3
             AND status IN (?4, ?5, ?6)"#,
    )
    .bind(&current.run_id)
    .bind(&current.current_state_id)
    .bind(&current.conflict_id)
    .bind(statuses[0].to_string())
    .bind(statuses[1].to_string())
    .bind(statuses[2].to_string())
    .fetch_all(&mut **tx)
    .await
    .context("find superseded current workflow conflicts")?;

    for row in rows {
        let mut superseded = decode_conflict_row(&row)?;
        superseded.status = WorkflowConflictStatus::Superseded;
        superseded.updated_at = current.updated_at;
        superseded.superseded_by_conflict_id = Some(current.conflict_id.clone());
        write_conflict_update_tx(tx, &superseded).await?;
    }

    Ok(())
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

async fn record_terminal_metric_events_tx(
    tx: &mut Transaction<'_, Sqlite>,
    record: &WorkflowConflictRecord,
    occurred_at: DateTime<Utc>,
) -> Result<()> {
    if !matches!(
        record.status,
        WorkflowConflictStatus::Resolved
            | WorkflowConflictStatus::Superseded
            | WorkflowConflictStatus::TerminalUnverifiable
    ) {
        return Ok(());
    }

    // OPS-003 (P017 R5): emit `terminal_unverifiable_total` whenever a
    // conflict transitions into the terminal-unverifiable state, with
    // bounded `terminal_failure_reason` label.
    if matches!(record.status, WorkflowConflictStatus::TerminalUnverifiable) {
        record_terminal_unverifiable_tx(
            tx,
            &record.run_id,
            &record.conflict_id,
            record.terminal_failure_reason.as_deref(),
            occurred_at,
        )
        .await?;
    }

    let resolution_mode = resolution_mode(record);
    let action_class = action_class(record);
    let elapsed_seconds =
        (occurred_at - record.created_at).num_milliseconds().max(0) as f64 / 1000.0;

    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(record.run_id.clone()),
            conflict_id: Some(record.conflict_id.clone()),
            metric_name: "workflow_conflict_time_to_resolution_seconds".to_string(),
            labels_json: serde_json::json!({
                "conflict_reason": record.reason.to_string(),
                "resolution_mode": resolution_mode,
            }),
            value: elapsed_seconds,
            unit: "seconds".to_string(),
            occurred_at,
        },
    )
    .await?;

    insert_metric_event_tx(
        tx,
        &WorkflowConflictMetricEvent {
            event_id: Uuid::new_v4().to_string(),
            run_id: Some(record.run_id.clone()),
            conflict_id: Some(record.conflict_id.clone()),
            metric_name: "conflict_reason_to_action_outcome_total".to_string(),
            labels_json: serde_json::json!({
                "conflict_reason": record.reason.to_string(),
                "action_class": action_class,
                "terminal_status": record.status.to_string(),
            }),
            value: 1.0,
            unit: "count".to_string(),
            occurred_at,
        },
    )
    .await
}

fn resolution_mode(record: &WorkflowConflictRecord) -> &'static str {
    match record.status {
        WorkflowConflictStatus::Resolved => "graph_settlement",
        WorkflowConflictStatus::Superseded => "superseded",
        WorkflowConflictStatus::TerminalUnverifiable => "terminal_unverifiable",
        WorkflowConflictStatus::LeadMediationPending => "lead_mediation",
        WorkflowConflictStatus::OperatorConfirmationRequired => "operator_confirmation",
        WorkflowConflictStatus::Unresolved => "unresolved",
    }
}

fn action_class(record: &WorkflowConflictRecord) -> String {
    record
        .resolution_record_json
        .as_ref()
        .and_then(|json| {
            json.get("action_class")
                .or_else(|| json.get("recovery_action"))
                .or_else(|| json.get("selected_action"))
                .and_then(|value| value.as_str())
        })
        .unwrap_or_else(|| match record.status {
            WorkflowConflictStatus::Resolved => "graph_settlement",
            WorkflowConflictStatus::Superseded => "superseded",
            WorkflowConflictStatus::TerminalUnverifiable => "manual_or_clone_fallback",
            WorkflowConflictStatus::LeadMediationPending => "lead_mediation",
            WorkflowConflictStatus::OperatorConfirmationRequired => "operator_confirmation",
            WorkflowConflictStatus::Unresolved => "unresolved",
        })
        .to_string()
}

async fn insert_metric_event_tx(
    tx: &mut Transaction<'_, Sqlite>,
    event: &WorkflowConflictMetricEvent,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO workflow_conflict_metric_events
           (event_id, run_id, conflict_id, metric_name, labels_json,
            value, unit, occurred_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
    )
    .bind(&event.event_id)
    .bind(&event.run_id)
    .bind(&event.conflict_id)
    .bind(&event.metric_name)
    .bind(serde_json::to_string(&event.labels_json)?)
    .bind(event.value)
    .bind(&event.unit)
    .bind(event.occurred_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .context("insert workflow conflict metric event")?;
    Ok(())
}

fn decode_metric_event_row(row: &sqlx::sqlite::SqliteRow) -> Result<WorkflowConflictMetricEvent> {
    let labels_raw: String = row.get("labels_json");
    let occurred_raw: String = row.get("occurred_at");
    Ok(WorkflowConflictMetricEvent {
        event_id: row.get("event_id"),
        run_id: row.get("run_id"),
        conflict_id: row.get("conflict_id"),
        metric_name: row.get("metric_name"),
        labels_json: serde_json::from_str(&labels_raw).context("decode metric labels_json")?,
        value: row.get("value"),
        unit: row.get("unit"),
        occurred_at: DateTime::parse_from_rfc3339(&occurred_raw)
            .context("parse metric occurred_at")?
            .with_timezone(&Utc),
    })
}

/// Update the lead_agent_id, mediation_record_id, and status on a conflict record
/// to link it to a newly created mediation. Used by the orchestrator when Phase B
/// mediation is initiated for an eligible conflict.
pub async fn update_mediation_pointer(
    pool: &SqlitePool,
    conflict_id: &str,
    lead_agent_id: &str,
    mediation_record_id: &str,
    status: WorkflowConflictStatus,
    now: DateTime<Utc>,
) -> Result<()> {
    let row = sqlx::query(r#"SELECT record_json FROM workflow_conflicts WHERE conflict_id = ?1"#)
        .bind(conflict_id)
        .fetch_one(pool)
        .await
        .context("find workflow conflict for mediation pointer update")?;

    let mut record = decode_conflict_row(&row)?;
    record.lead_agent_id = Some(lead_agent_id.to_string());
    record.mediation_record_id = Some(mediation_record_id.to_string());
    record.status = status;
    record.updated_at = now;

    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "workflow_conflicts.update_mediation_pointer",
    )
    .await?;
    write_conflict_update_tx(&mut tx, &record).await?;
    tx.commit().await?;
    Ok(())
}

/// Transaction variant of update_mediation_pointer for use inside an
/// existing IMMEDIATE transaction (MF-PRE-ENABLE-002).
pub async fn update_mediation_pointer_tx(
    tx: &mut Transaction<'_, Sqlite>,
    conflict_id: &str,
    lead_agent_id: &str,
    mediation_record_id: &str,
    status: WorkflowConflictStatus,
    now: DateTime<Utc>,
) -> Result<()> {
    let row = sqlx::query(r#"SELECT record_json FROM workflow_conflicts WHERE conflict_id = ?1"#)
        .bind(conflict_id)
        .fetch_one(&mut **tx)
        .await
        .context("find workflow conflict for mediation pointer update (tx)")?;

    let mut record = decode_conflict_row(&row)?;
    record.lead_agent_id = Some(lead_agent_id.to_string());
    record.mediation_record_id = Some(mediation_record_id.to_string());
    record.status = status;
    record.updated_at = now;

    write_conflict_update_tx(tx, &record).await?;
    Ok(())
}

fn decode_conflict_row(row: &sqlx::sqlite::SqliteRow) -> Result<WorkflowConflictRecord> {
    let raw: String = row.get("record_json");
    serde_json::from_str(&raw).context("decode WorkflowConflictRecord")
}

fn decode_rejection_row(row: &sqlx::sqlite::SqliteRow) -> Result<WorkflowAdvisoryRejectionRecord> {
    let raw: String = row.get("record_json");
    serde_json::from_str(&raw).context("decode WorkflowAdvisoryRejectionRecord")
}
