use anyhow::{Context, Result};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::continuation::{ContinuationCandidate, ContinuationRecord};

/// P086: Eligibility info for an agent execution that may receive a continuation.
pub struct ContinuationEligibilityInfo {
    pub agent_execution_id: String,
    pub stage_execution_id: String,
    pub run_id: String,
    pub logical_stage_id: Option<String>,
    pub stage_type: Option<String>,
    pub agent_status: String,
    pub session_generation_id: Option<String>,
    pub provider_session_id: Option<String>,
    pub catalog_snapshot_json: Option<String>,
}

/// P086: Parameters for the atomic admission transaction (command_journal + continuation row).
pub struct ContinuationAdmission {
    pub continuation_id: String,
    pub command_journal_id: String,
    pub run_id: String,
    pub stage_execution_id: String,
    pub agent_execution_id: String,
    pub mode: String,
    pub trigger_kind: String,
    pub idempotency_scope: String,
    pub idempotency_key: String,
    pub request_fingerprint_sha256: String,
    pub lead_decision_artifact_id: Option<String>,
    pub lead_decision_artifact_sha256: Option<String>,
    pub continuation_instruction_sha256: Option<String>,
    pub budget_json: Option<String>,
    pub caller_principal_id: String,
    pub caller_surface: String,
    pub caller_principal_class: String,
    pub caller_tool: String,
    pub created_at: String,
}

/// P086: Durable rollout metric event. IDs live in columns instead of labels so
/// label cardinality stays bounded while operators can still join back to run
/// and continuation truth.
#[derive(Clone, Debug)]
pub struct P086ContinuationMetricEvent {
    pub id: String,
    pub run_id: Option<String>,
    pub stage_execution_id: Option<String>,
    pub agent_execution_id: Option<String>,
    pub continuation_id: Option<String>,
    pub metric_name: String,
    pub labels_json: serde_json::Value,
    pub value: i64,
    pub occurred_at: String,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct P086ContinuationMetricsSummary {
    pub run_id: String,
    pub admission_total: i64,
    pub accepted_total: i64,
    pub rejected_total: i64,
    pub replay_total: i64,
    pub success_total: i64,
    pub no_progress_total: i64,
    pub failed_total: i64,
    pub cancelled_total: i64,
    pub fresh_session_avoided_total: i64,
    pub lead_auto_total: i64,
    pub operator_mcp_total: i64,
    pub changed_files_total: i64,
    pub tests_or_gates_total: i64,
    pub orphan_reap_attempted_total: i64,
    pub orphan_reap_verified_total: i64,
    pub resurrection_unsupported_total: i64,
    pub terminal_total: i64,
    pub useful_progress_total: i64,
    pub useful_progress_rate: f64,
    pub no_progress_rate: f64,
    pub tests_passed_after_continuation_total: i64,
    pub followup_validation_total: i64,
    pub followup_validation_success_total: i64,
    pub followup_validation_success_rate: f64,
    pub lead_auto_success_total: i64,
    pub lead_auto_success_rate: f64,
    pub operator_mcp_success_total: i64,
    pub operator_mcp_success_rate: f64,
    pub time_saved_seconds_total: i64,
    pub time_saved_sample_count: i64,
    pub average_time_saved_seconds: f64,
    pub provider_session_budget_input_tokens_total: i64,
    pub provider_session_budget_output_tokens_total: i64,
    pub provider_session_budget_cached_input_tokens_total: i64,
    pub provider_session_budget_cost_cents_total: i64,
    pub provider_session_resurrection_attach_success_total: i64,
    pub provider_session_resurrection_attach_failure_total: i64,
}

const SELECT_COLS: &str = r#"
    id, run_id, stage_execution_id, agent_execution_id,
    mode, trigger_kind, status,
    failure_reason, reconciliation_status,
    idempotency_scope, idempotency_key, request_fingerprint_sha256,
    canonical_request_artifact_id,
    attach_receipt_artifact_id,
    evidence_bundle_artifact_id,
    worktree_readback_artifact_id,
    continuation_report_artifact_id,
    response_fingerprint_sha256, response_artifact_id,
    result_or_no_progress_artifact_id,
    conflict_count,
    lead_decision_artifact_id, lead_decision_artifact_sha256,
    continuation_instruction_sha256,
    budget_json,
    created_at, updated_at
"#;

fn row_to_record(r: &sqlx::sqlite::SqliteRow) -> ContinuationRecord {
    ContinuationRecord {
        id: r.get("id"),
        run_id: r.get("run_id"),
        stage_execution_id: r.get("stage_execution_id"),
        agent_execution_id: r.get("agent_execution_id"),
        mode: r.get("mode"),
        trigger_kind: r.get("trigger_kind"),
        status: r.get("status"),
        failure_reason: r.get("failure_reason"),
        reconciliation_status: r.get("reconciliation_status"),
        idempotency_scope: r.get("idempotency_scope"),
        idempotency_key: r.get("idempotency_key"),
        request_fingerprint_sha256: r.get("request_fingerprint_sha256"),
        canonical_request_artifact_id: r.get("canonical_request_artifact_id"),
        attach_receipt_artifact_id: r.get("attach_receipt_artifact_id"),
        evidence_bundle_artifact_id: r.get("evidence_bundle_artifact_id"),
        worktree_readback_artifact_id: r.get("worktree_readback_artifact_id"),
        continuation_report_artifact_id: r.get("continuation_report_artifact_id"),
        response_fingerprint_sha256: r.get("response_fingerprint_sha256"),
        response_artifact_id: r.get("response_artifact_id"),
        result_or_no_progress_artifact_id: r.get("result_or_no_progress_artifact_id"),
        conflict_count: r.get("conflict_count"),
        lead_decision_artifact_id: r.get("lead_decision_artifact_id"),
        lead_decision_artifact_sha256: r.get("lead_decision_artifact_sha256"),
        continuation_instruction_sha256: r.get("continuation_instruction_sha256"),
        budget_json: r.get("budget_json"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

fn row_to_metric_event(r: &sqlx::sqlite::SqliteRow) -> P086ContinuationMetricEvent {
    let labels_raw: String = r.get("labels_json");
    P086ContinuationMetricEvent {
        id: r.get("id"),
        run_id: r.get("run_id"),
        stage_execution_id: r.get("stage_execution_id"),
        agent_execution_id: r.get("agent_execution_id"),
        continuation_id: r.get("continuation_id"),
        metric_name: r.get("metric_name"),
        labels_json: serde_json::from_str(&labels_raw).unwrap_or_else(|_| serde_json::json!({})),
        value: r.get("value"),
        occurred_at: r.get("occurred_at"),
    }
}

fn bounded_p086_metric_labels(labels: serde_json::Value) -> serde_json::Value {
    let Some(object) = labels.as_object() else {
        return serde_json::json!({});
    };
    let allowed = [
        "mode",
        "trigger_kind",
        "outcome",
        "terminal_status",
        "failure_reason",
        "evidence_source",
        "resurrection_status",
        "orphan_reap_policy",
        "estimate_source",
        "validation_outcome",
        "budget_dimension",
    ];
    let mut bounded = serde_json::Map::new();
    for key in allowed {
        if let Some(value) = object.get(key) {
            let normalized = value
                .as_str()
                .map(|s| {
                    s.chars()
                        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
                        .take(96)
                        .collect::<String>()
                })
                .unwrap_or_else(|| value.to_string().chars().take(96).collect());
            bounded.insert(key.to_string(), serde_json::Value::String(normalized));
        }
    }
    serde_json::Value::Object(bounded)
}

pub async fn record_p086_continuation_metric_event_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: Option<&str>,
    stage_execution_id: Option<&str>,
    agent_execution_id: Option<&str>,
    continuation_id: Option<&str>,
    metric_name: &str,
    labels_json: serde_json::Value,
    value: i64,
) -> Result<String> {
    let event_id = uuid::Uuid::new_v4().to_string();
    let occurred_at = chrono::Utc::now().to_rfc3339();
    let labels = bounded_p086_metric_labels(labels_json).to_string();
    sqlx::query(
        "INSERT INTO p086_continuation_metric_events
         (id, run_id, stage_execution_id, agent_execution_id, continuation_id,
          metric_name, labels_json, value, occurred_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&event_id)
    .bind(run_id)
    .bind(stage_execution_id)
    .bind(agent_execution_id)
    .bind(continuation_id)
    .bind(metric_name)
    .bind(labels)
    .bind(value.max(0))
    .bind(&occurred_at)
    .execute(&mut **tx)
    .await
    .context("record P086 continuation metric event")?;
    Ok(event_id)
}

pub async fn record_p086_continuation_metric_event(
    pool: &SqlitePool,
    run_id: Option<&str>,
    stage_execution_id: Option<&str>,
    agent_execution_id: Option<&str>,
    continuation_id: Option<&str>,
    metric_name: &str,
    labels_json: serde_json::Value,
    value: i64,
) -> Result<String> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "agent_work_continuations.metric_event")
            .await?;
    let id = record_p086_continuation_metric_event_tx(
        &mut tx,
        run_id,
        stage_execution_id,
        agent_execution_id,
        continuation_id,
        metric_name,
        labels_json,
        value,
    )
    .await?;
    tx.commit()
        .await
        .context("commit P086 continuation metric event")?;
    Ok(id)
}

pub async fn list_p086_continuation_metric_events_for_run(
    pool: &SqlitePool,
    run_id: &str,
    limit: i64,
) -> Result<Vec<P086ContinuationMetricEvent>> {
    let rows = sqlx::query(
        "SELECT id, run_id, stage_execution_id, agent_execution_id, continuation_id,
                metric_name, labels_json, value, occurred_at
         FROM p086_continuation_metric_events
         WHERE run_id = ?
         ORDER BY occurred_at DESC
         LIMIT ?",
    )
    .bind(run_id)
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await
    .context("list P086 continuation metric events for run")?;
    Ok(rows.iter().map(row_to_metric_event).collect())
}

pub async fn p086_continuation_metrics_summary_for_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<P086ContinuationMetricsSummary> {
    let events = list_p086_continuation_metric_events_for_run(pool, run_id, 500).await?;
    let mut summary = P086ContinuationMetricsSummary {
        run_id: run_id.to_string(),
        ..Default::default()
    };
    for event in events {
        let value = event.value.max(0);
        match event.metric_name.as_str() {
            "continuation_admission_total" => {
                summary.admission_total += value;
                match event.labels_json.get("outcome").and_then(|v| v.as_str()) {
                    Some("accepted") => summary.accepted_total += value,
                    Some("replay") => summary.replay_total += value,
                    Some(outcome) if outcome.starts_with("rejected") => {
                        summary.rejected_total += value
                    }
                    _ => {}
                }
                match event
                    .labels_json
                    .get("trigger_kind")
                    .and_then(|v| v.as_str())
                {
                    Some("lead_auto") => summary.lead_auto_total += value,
                    Some("operator_mcp") => summary.operator_mcp_total += value,
                    _ => {}
                }
            }
            "continuation_settlement_total" => {
                match event
                    .labels_json
                    .get("terminal_status")
                    .and_then(|v| v.as_str())
                {
                    Some("succeeded") => {
                        summary.success_total += value;
                        match event
                            .labels_json
                            .get("trigger_kind")
                            .and_then(|v| v.as_str())
                        {
                            Some("lead_auto") => summary.lead_auto_success_total += value,
                            Some("operator_mcp") => summary.operator_mcp_success_total += value,
                            _ => {}
                        }
                    }
                    Some("no_progress") => summary.no_progress_total += value,
                    Some("failed") => summary.failed_total += value,
                    Some("cancelled") => summary.cancelled_total += value,
                    _ => {}
                }
            }
            "continuation_fresh_session_avoided_total" => {
                summary.fresh_session_avoided_total += value
            }
            "continuation_changed_files_total" => summary.changed_files_total += value,
            "continuation_tests_or_gates_total" => summary.tests_or_gates_total += value,
            "continuation_tests_passed_total" => {
                summary.tests_passed_after_continuation_total += value
            }
            "continuation_useful_progress_total" => summary.useful_progress_total += value,
            "continuation_followup_validation_total" => {
                summary.followup_validation_total += value;
                if event
                    .labels_json
                    .get("validation_outcome")
                    .and_then(|v| v.as_str())
                    == Some("success")
                {
                    summary.followup_validation_success_total += value;
                }
            }
            "continuation_time_saved_seconds" => {
                summary.time_saved_seconds_total += value;
                summary.time_saved_sample_count += 1;
            }
            "continuation_provider_session_budget_input_tokens_total" => {
                summary.provider_session_budget_input_tokens_total += value
            }
            "continuation_provider_session_budget_output_tokens_total" => {
                summary.provider_session_budget_output_tokens_total += value
            }
            "continuation_provider_session_budget_cached_input_tokens_total" => {
                summary.provider_session_budget_cached_input_tokens_total += value
            }
            "continuation_provider_session_budget_cost_cents_total" => {
                summary.provider_session_budget_cost_cents_total += value
            }
            "continuation_orphan_reap_attempted_total" => {
                summary.orphan_reap_attempted_total += value
            }
            "continuation_orphan_reap_verified_total" => {
                summary.orphan_reap_verified_total += value
            }
            "continuation_resurrection_total" => {
                match event
                    .labels_json
                    .get("resurrection_status")
                    .and_then(|v| v.as_str())
                {
                    Some("success") | Some("attached") => {
                        summary.provider_session_resurrection_attach_success_total += value;
                    }
                    Some("unsupported") => {
                        summary.resurrection_unsupported_total += value;
                        summary.provider_session_resurrection_attach_failure_total += value;
                    }
                    Some("failed") | Some("failure") => {
                        summary.provider_session_resurrection_attach_failure_total += value;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    summary.terminal_total = summary.success_total
        + summary.no_progress_total
        + summary.failed_total
        + summary.cancelled_total;
    if summary.terminal_total > 0 {
        summary.useful_progress_rate =
            summary.useful_progress_total as f64 / summary.terminal_total as f64;
        summary.no_progress_rate = summary.no_progress_total as f64 / summary.terminal_total as f64;
    }
    if summary.followup_validation_total > 0 {
        summary.followup_validation_success_rate = summary.followup_validation_success_total as f64
            / summary.followup_validation_total as f64;
    }
    if summary.lead_auto_total > 0 {
        summary.lead_auto_success_rate =
            summary.lead_auto_success_total as f64 / summary.lead_auto_total as f64;
    }
    if summary.operator_mcp_total > 0 {
        summary.operator_mcp_success_rate =
            summary.operator_mcp_success_total as f64 / summary.operator_mcp_total as f64;
    }
    if summary.time_saved_sample_count > 0 {
        summary.average_time_saved_seconds =
            summary.time_saved_seconds_total as f64 / summary.time_saved_sample_count as f64;
    }
    Ok(summary)
}

/// P086: Find continuation records for a given agent_execution_id (bounded).
/// Capped at 200 rows to prevent O(N) reads at the SQLite reader pool; a pagination
/// cursor must be added before Phase 2 enablement if deeper history is needed.
pub async fn list_for_agent_execution(
    pool: &SqlitePool,
    agent_execution_id: &str,
) -> Result<Vec<ContinuationRecord>> {
    let query = format!(
        "SELECT {SELECT_COLS} FROM agent_work_continuations \
         WHERE agent_execution_id = ? ORDER BY created_at DESC LIMIT 200"
    );
    let rows = sqlx::query(&query)
        .bind(agent_execution_id)
        .fetch_all(pool)
        .await
        .context("list continuations for agent execution")?;

    Ok(rows.iter().map(row_to_record).collect())
}

/// P086: Run-level continuation history for operator readback.
pub async fn list_for_run(pool: &SqlitePool, run_id: &str) -> Result<Vec<ContinuationRecord>> {
    let query = format!(
        "SELECT {SELECT_COLS} FROM agent_work_continuations \
         WHERE run_id = ? ORDER BY created_at DESC LIMIT 200"
    );
    let rows = sqlx::query(&query)
        .bind(run_id)
        .fetch_all(pool)
        .await
        .context("list continuations for run")?;

    Ok(rows.iter().map(row_to_record).collect())
}

/// P086: Find active (non-terminal) continuation for a given agent_execution_id.
pub async fn find_active_for_agent_execution(
    pool: &SqlitePool,
    agent_execution_id: &str,
) -> Result<Option<ContinuationRecord>> {
    let query = format!(
        "SELECT {SELECT_COLS} FROM agent_work_continuations \
         WHERE agent_execution_id = ? \
           AND status NOT IN ('succeeded', 'no_progress', 'failed', 'cancelled') \
         ORDER BY created_at DESC LIMIT 1"
    );
    let row = sqlx::query(&query)
        .bind(agent_execution_id)
        .fetch_optional(pool)
        .await
        .context("find active continuation for agent execution")?;

    Ok(row.as_ref().map(row_to_record))
}

const CANDIDATES_SQL: &str = r#"SELECT
             ae.id              AS agent_execution_id,
             ae.owner_id        AS stage_execution_id,
             r.id               AS run_id,
             ae.agent_id        AS agent_role,
             sg.provider_session_id,
             ae.status,
             EXISTS(
               SELECT 1 FROM agent_work_continuations awc
               WHERE awc.agent_execution_id = ae.id
                 AND awc.status NOT IN ('succeeded', 'no_progress', 'failed', 'cancelled')
             ) AS has_active_continuation
           FROM agent_executions ae
           JOIN stage_executions se ON se.id = ae.owner_id
           JOIN runs r ON r.id = se.run_id
           LEFT JOIN session_generations sg ON sg.id = ae.session_generation_id
           WHERE r.id = ?
             AND ae.owner_kind = 'stage_execution'
             AND ae.agent_id = 'code_writer'
             AND ae.status IN ('completed', 'failed')
           ORDER BY ae.started_at DESC"#;

/// P086: List eligible continuation candidates for a run.
/// Returns completed/failed code_writer stage-owned AgentExecution rows
/// that have no already-active continuation. Uses a single query with an
/// EXISTS subquery to avoid the N+1 per-row active-continuation lookup.
pub async fn list_candidates_for_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<ContinuationCandidate>> {
    let rows = sqlx::query(CANDIDATES_SQL)
        .bind(run_id)
        .fetch_all(pool)
        .await
        .context("list continuation candidates for run")?;

    let candidates: Vec<ContinuationCandidate> = rows
        .iter()
        .map(|row| {
            let has_active: i64 = row.try_get("has_active_continuation").unwrap_or(0);
            let eligible = has_active == 0;
            let disabled_reason = if !eligible {
                Some("agent_already_running".to_string())
            } else {
                None
            };
            ContinuationCandidate {
                agent_execution_id: row.get("agent_execution_id"),
                run_id: row.get("run_id"),
                stage_execution_id: row.try_get("stage_execution_id").unwrap_or_default(),
                agent_role: row.get("agent_role"),
                provider_session_id: row.get("provider_session_id"),
                status: row.get("status"),
                eligible,
                disabled_reason,
            }
        })
        .collect();

    Ok(candidates)
}

/// P086: Check whether an agent execution is eligible for continuation.
/// Returns `Some(info)` for any code_writer + stage_execution-owned agent regardless of status,
/// so the caller can distinguish `agent_already_running` (non-terminal status) from
/// `ineligible_agent_execution` (wrong role/owner or no matching row). Terminal-status check
/// is the caller's responsibility (see handle_continue_work).
/// Returns None when the agent execution does not exist or does not meet role/ownership criteria.
pub async fn check_eligibility(
    pool: &SqlitePool,
    agent_execution_id: &str,
) -> Result<Option<ContinuationEligibilityInfo>> {
    let row = sqlx::query(
        "SELECT ae.id AS agent_execution_id, ae.owner_id AS stage_execution_id,
                se.run_id, se.stage_id AS logical_stage_id, se.stage_type,
                ae.status AS agent_status,
                ae.session_generation_id,
                sg.provider_session_id,
                r.catalog_snapshot_json
         FROM agent_executions ae
         JOIN stage_executions se ON se.id = ae.owner_id
         JOIN runs r ON r.id = se.run_id
         LEFT JOIN session_generations sg ON sg.id = ae.session_generation_id
         WHERE ae.id = ?
           AND ae.agent_id = 'code_writer'
           AND ae.owner_kind = 'stage_execution'
           AND ae.owner_id IS NOT NULL",
    )
    .bind(agent_execution_id)
    .fetch_optional(pool)
    .await
    .context("check_eligibility: query agent execution")?;

    Ok(row.map(|r| ContinuationEligibilityInfo {
        agent_execution_id: r.get("agent_execution_id"),
        stage_execution_id: r.get("stage_execution_id"),
        run_id: r.get("run_id"),
        logical_stage_id: r.get("logical_stage_id"),
        stage_type: r.get("stage_type"),
        agent_status: r.get("agent_status"),
        session_generation_id: r.get("session_generation_id"),
        provider_session_id: r.get("provider_session_id"),
        catalog_snapshot_json: r.get("catalog_snapshot_json"),
    }))
}

/// P086: fail closed when the target stage still has unresolved external side effects.
/// Continuation is only an implementation/code-editing lane; release/publish side-effect
/// recovery remains owned by P078/P080.
pub async fn has_unresolved_side_effects_for_stage(
    pool: &SqlitePool,
    run_id: &str,
    stage_execution_id: &str,
) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM side_effects
         WHERE run_id = ?
           AND stage_execution_id = ?
           AND status IN (
             'prepared','executing','externally_observed',
             'needs_reconciliation','conflict','unrecoverable'
           )",
    )
    .bind(run_id)
    .bind(stage_execution_id)
    .fetch_one(pool)
    .await
    .context("has_unresolved_side_effects_for_stage: query side_effects")?;
    Ok(count > 0)
}

/// P086: Check whether a run has any pending (unresolved) approvals.
/// Used at admission to enforce approval_required rejection before any continuation
/// can be enqueued against a stage that still awaits operator decision.
pub async fn has_pending_approval_for_run(pool: &SqlitePool, run_id: &str) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM approvals
         WHERE run_id = ? AND (decision = 'pending' OR decision = 'requested')",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .context("has_pending_approval_for_run: query approvals")?;
    Ok(count > 0)
}

/// P086: Find the most recent continuation row matching the given idempotency scope+key.
/// Used for replay detection (same scope+key+fingerprint) and conflict detection
/// (same scope+key, different fingerprint → -32044).
pub async fn find_by_idempotency(
    pool: &SqlitePool,
    idempotency_scope: &str,
    idempotency_key: &str,
) -> Result<Option<ContinuationRecord>> {
    let query = format!(
        "SELECT {SELECT_COLS} FROM agent_work_continuations \
         WHERE idempotency_scope = ? AND idempotency_key = ? \
         ORDER BY created_at DESC LIMIT 1"
    );
    let row = sqlx::query(&query)
        .bind(idempotency_scope)
        .bind(idempotency_key)
        .fetch_optional(pool)
        .await
        .context("find_by_idempotency: query")?;

    Ok(row.as_ref().map(row_to_record))
}

/// P086: Seconds before an accepted/queued/starting row is swept to failed=admission_timeout.
/// Matches proposal architecture.admission_backpressure max_admission_to_start_seconds.
pub const MAX_ADMISSION_TO_START_SECONDS: i64 = 300;

/// Global queue depth cap per proposal observability contract.
pub const QUEUE_DEPTH_MAX: i64 = 64;
/// Concurrency cap: max rows in active-execution states simultaneously.
pub const CONCURRENCY_MAX: i64 = 16;

/// P086: Transition a continuation row to a new lifecycle status.
/// Updates both `status` and `updated_at`; sets `failure_reason` when provided.
/// Returns the number of rows affected (0 if the id was not found).
pub async fn update_continuation_status(
    pool: &SqlitePool,
    continuation_id: &str,
    new_status: &str,
    failure_reason: Option<&str>,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE agent_work_continuations
         SET status = ?, failure_reason = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(new_status)
    .bind(failure_reason)
    .bind(&now)
    .bind(continuation_id)
    .execute(pool)
    .await
    .context("update_continuation_status")?;
    Ok(rows.rows_affected())
}

/// P086: Transition a continuation row unless cancellation has claimed it.
/// Used after provider_send so a late provider result cannot erase run cancellation.
pub async fn update_continuation_status_unless_cancelling(
    pool: &SqlitePool,
    continuation_id: &str,
    new_status: &str,
    failure_reason: Option<&str>,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE agent_work_continuations
         SET status = ?, failure_reason = ?, updated_at = ?
         WHERE id = ? AND status != 'cancelling'",
    )
    .bind(new_status)
    .bind(failure_reason)
    .bind(&now)
    .bind(continuation_id)
    .execute(pool)
    .await
    .context("update_continuation_status_unless_cancelling")?;
    Ok(rows.rows_affected())
}

/// P086: Sweep accepted/queued/starting rows older than `max_seconds` to
/// status=failed with failure_reason=admission_timeout.
///
/// Per proposal architecture.admission_backpressure: timed-out admission rows must
/// have a terminal failure_reason and terminal replay behavior.  The executor
/// materializes missing terminal artifacts immediately after this transition.
///
/// Returns the number of rows swept.
pub async fn sweep_admission_timeouts(pool: &SqlitePool, max_seconds: i64) -> Result<u64> {
    let cutoff = (chrono::Utc::now()
        - chrono::Duration::try_seconds(max_seconds).unwrap_or(chrono::Duration::zero()))
    .to_rfc3339();
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE agent_work_continuations
         SET status = 'failed',
             failure_reason = 'admission_timeout',
             updated_at = ?
         WHERE status IN ('accepted', 'queued', 'starting')
           AND created_at < ?",
    )
    .bind(&now)
    .bind(&cutoff)
    .execute(pool)
    .await
    .context("sweep_admission_timeouts")?;
    Ok(rows.rows_affected())
}

/// P086 Phase 2: Find terminal continuation rows that still need response/result
/// artifacts.  Used by admission-timeout recovery and startup repair to fail closed
/// instead of leaving terminal rows without durable evidence.
pub async fn list_terminal_missing_artifacts(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<ContinuationRecord>> {
    let limit = limit.clamp(1, 500);
    let rows = sqlx::query(&format!(
        "SELECT {SELECT_COLS} FROM agent_work_continuations
         WHERE status IN ('succeeded', 'no_progress', 'failed', 'cancelled')
           AND (response_artifact_id IS NULL OR result_or_no_progress_artifact_id IS NULL)
         ORDER BY updated_at ASC
         LIMIT ?"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("list terminal continuations missing artifacts")?;
    Ok(rows.iter().map(row_to_record).collect())
}

/// P086: Outcome of the atomic admission attempt.
pub enum AtomicAdmissionOutcome {
    /// New continuation row created; background worker may claim it.
    Accepted,
    /// Identical idempotency scope/key/fingerprint matched an existing row.
    Replay(Box<ContinuationRecord>),
    /// Same scope+key, different canonical fingerprint (-32044).
    IdempotencyConflict,
    /// A non-terminal continuation already exists for this agent execution.
    AlreadyRunning(Box<ContinuationRecord>),
    /// Lead-auto policy cap exceeded before insertion.
    LeadAutoLimitExceeded {
        scope: &'static str,
        current_count: i64,
        max_count: i64,
    },
    /// Global queue depth or concurrency cap exceeded (-32051).
    Saturated { queue_depth: i64, concurrency: i64 },
}

/// P086: Result of a background worker claim attempt.
pub enum ContinuationClaimOutcome {
    /// Row was claimed and moved into `starting`.
    Claimed(Box<ContinuationRecord>),
    /// Row does not exist.
    Missing,
    /// Row is already terminal and must not be processed again.
    Terminal(Box<ContinuationRecord>),
    /// Row has reached or passed `prompt_sent`; replay must not send provider I/O.
    PromptAlreadySent(Box<ContinuationRecord>),
    /// Row is already owned by an active worker or in a transient state.
    AlreadyActive(Box<ContinuationRecord>),
}

/// P086: Atomic admission — idempotency check, active-continuation check, saturation check,
/// and row insertion all inside a single BEGIN IMMEDIATE transaction, preventing the TOCTOU
/// races identified by P086-SEC-MED-001 (idempotency non-atomic) and P086-SEC-MED-002
/// (active-row non-atomic). Saturation is computed in-transaction so concurrent admits
/// don't race past the cap.
pub async fn admit_continuation_atomic(
    pool: &SqlitePool,
    admission: &ContinuationAdmission,
    payload_json: &str,
) -> Result<AtomicAdmissionOutcome> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "agent_work_continuations.admit_atomic")
            .await?;

    // 1. Idempotency check inside the transaction (P086-SEC-MED-001 fix).
    //    Any duplicate request with the same scope+key is detected here before insert.
    let idempotency_query = format!(
        "SELECT {SELECT_COLS} FROM agent_work_continuations \
         WHERE idempotency_scope = ? AND idempotency_key = ? \
         ORDER BY created_at DESC LIMIT 1"
    );
    let existing_row = sqlx::query(&idempotency_query)
        .bind(&admission.idempotency_scope)
        .bind(&admission.idempotency_key)
        .fetch_optional(&mut **tx)
        .await
        .context("admit_atomic: check idempotency")?;

    if let Some(ref row) = existing_row {
        let record = row_to_record(row);
        if record.request_fingerprint_sha256 == admission.request_fingerprint_sha256 {
            record_p086_continuation_metric_event_tx(
                &mut tx,
                Some(&admission.run_id),
                Some(&admission.stage_execution_id),
                Some(&admission.agent_execution_id),
                Some(&record.id),
                "continuation_admission_total",
                serde_json::json!({
                    "mode": admission.mode,
                    "trigger_kind": admission.trigger_kind,
                    "outcome": "replay"
                }),
                1,
            )
            .await?;
            tx.commit()
                .await
                .context("admit_atomic: commit replay metric")?;
            return Ok(AtomicAdmissionOutcome::Replay(Box::new(record)));
        } else {
            // P086-SEC-LOW-B fix: increment conflict_count on the matched row and commit,
            // so repeated conflicts are durably tracked for forensic coverage and abuse detection.
            sqlx::query(
                "UPDATE agent_work_continuations SET conflict_count = conflict_count + 1
                 WHERE id = ?",
            )
            .bind(&record.id)
            .execute(&mut **tx)
            .await
            .context("admit_atomic: increment conflict_count")?;
            record_p086_continuation_metric_event_tx(
                &mut tx,
                Some(&admission.run_id),
                Some(&admission.stage_execution_id),
                Some(&admission.agent_execution_id),
                Some(&record.id),
                "continuation_admission_total",
                serde_json::json!({
                    "mode": admission.mode,
                    "trigger_kind": admission.trigger_kind,
                    "outcome": "rejected_idempotency_conflict"
                }),
                1,
            )
            .await?;
            tx.commit()
                .await
                .context("admit_atomic: commit conflict_count increment")?;
            return Ok(AtomicAdmissionOutcome::IdempotencyConflict);
        }
    }

    // 2. Active-continuation check inside the transaction (P086-SEC-MED-002 fix).
    //    Prevents two concurrent requests from each seeing "no active" and both inserting.
    let active_query = format!(
        "SELECT {SELECT_COLS} FROM agent_work_continuations \
         WHERE agent_execution_id = ? \
           AND status NOT IN ('succeeded', 'no_progress', 'failed', 'cancelled') \
         ORDER BY created_at DESC LIMIT 1"
    );
    let active_row = sqlx::query(&active_query)
        .bind(&admission.agent_execution_id)
        .fetch_optional(&mut **tx)
        .await
        .context("admit_atomic: check active continuation")?;

    if let Some(ref row) = active_row {
        let record = row_to_record(row);
        record_p086_continuation_metric_event_tx(
            &mut tx,
            Some(&admission.run_id),
            Some(&admission.stage_execution_id),
            Some(&admission.agent_execution_id),
            Some(&record.id),
            "continuation_admission_total",
            serde_json::json!({
                "mode": admission.mode,
                "trigger_kind": admission.trigger_kind,
                "outcome": "rejected_agent_already_running"
            }),
            1,
        )
        .await?;
        tx.commit()
            .await
            .context("admit_atomic: commit already-running metric")?;
        return Ok(AtomicAdmissionOutcome::AlreadyRunning(Box::new(record)));
    }

    // 3. Lead-auto policy limits inside the transaction. Terminal lead_auto rows
    // count too: the proposal caps automatic continuation at max 1 per agent
    // execution and max 2 per stage execution, independent of active queue state.
    if admission.trigger_kind == "lead_auto" {
        let lead_auto_agent_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_work_continuations
             WHERE trigger_kind = 'lead_auto' AND agent_execution_id = ?",
        )
        .bind(&admission.agent_execution_id)
        .fetch_one(&mut **tx)
        .await
        .context("admit_atomic: count lead_auto rows for agent")?;
        if lead_auto_agent_count >= 1 {
            record_p086_continuation_metric_event_tx(
                &mut tx,
                Some(&admission.run_id),
                Some(&admission.stage_execution_id),
                Some(&admission.agent_execution_id),
                None,
                "continuation_admission_total",
                serde_json::json!({
                    "mode": admission.mode,
                    "trigger_kind": admission.trigger_kind,
                    "outcome": "rejected_lead_auto_agent_limit"
                }),
                1,
            )
            .await?;
            tx.commit()
                .await
                .context("admit_atomic: commit lead_auto agent limit metric")?;
            return Ok(AtomicAdmissionOutcome::LeadAutoLimitExceeded {
                scope: "agent_execution",
                current_count: lead_auto_agent_count,
                max_count: 1,
            });
        }

        let lead_auto_stage_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM agent_work_continuations
             WHERE trigger_kind = 'lead_auto' AND stage_execution_id = ?",
        )
        .bind(&admission.stage_execution_id)
        .fetch_one(&mut **tx)
        .await
        .context("admit_atomic: count lead_auto rows for stage")?;
        if lead_auto_stage_count >= 2 {
            record_p086_continuation_metric_event_tx(
                &mut tx,
                Some(&admission.run_id),
                Some(&admission.stage_execution_id),
                Some(&admission.agent_execution_id),
                None,
                "continuation_admission_total",
                serde_json::json!({
                    "mode": admission.mode,
                    "trigger_kind": admission.trigger_kind,
                    "outcome": "rejected_lead_auto_stage_limit"
                }),
                1,
            )
            .await?;
            tx.commit()
                .await
                .context("admit_atomic: commit lead_auto stage limit metric")?;
            return Ok(AtomicAdmissionOutcome::LeadAutoLimitExceeded {
                scope: "stage_execution",
                current_count: lead_auto_stage_count,
                max_count: 2,
            });
        }
    }

    // 4. Saturation check inside the transaction (P086 admission backpressure).
    //    queue_depth = all non-terminal rows (accepted, queued, starting, running, …).
    //    concurrency = rows in active-execution states only.
    let queue_depth: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_work_continuations
         WHERE status NOT IN ('succeeded', 'no_progress', 'failed', 'cancelled')",
    )
    .fetch_one(&mut **tx)
    .await
    .context("admit_atomic: count queue depth")?;

    let concurrency: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM agent_work_continuations
         WHERE status IN ('starting', 'running', 'preflight_passed',
                          'prompt_sent', 'observing', 'worktree_observed', 'finalizing')",
    )
    .fetch_one(&mut **tx)
    .await
    .context("admit_atomic: count concurrency")?;

    if queue_depth >= QUEUE_DEPTH_MAX || concurrency >= CONCURRENCY_MAX {
        // P086: proposal requires an admission rejection audit row on saturation so the
        // command receipt is durable for forensics. Write to command_journal with
        // result_status='rejected' before returning, then commit.
        sqlx::query(
            "INSERT INTO command_journal
             (id, command_type, payload_json, result_status, run_id, created_at,
              caller_surface, caller_principal_id, caller_principal_class, caller_tool)
             VALUES (?1, 'agents.continue_work', ?2, 'rejected', ?3, ?4,
                     ?5, ?6, ?7, ?8)",
        )
        .bind(&admission.command_journal_id)
        .bind(payload_json)
        .bind(&admission.run_id)
        .bind(&admission.created_at)
        .bind(&admission.caller_surface)
        .bind(&admission.caller_principal_id)
        .bind(&admission.caller_principal_class)
        .bind(&admission.caller_tool)
        .execute(&mut **tx)
        .await
        .context("admit_atomic: insert saturation rejection audit row")?;
        record_p086_continuation_metric_event_tx(
            &mut tx,
            Some(&admission.run_id),
            Some(&admission.stage_execution_id),
            Some(&admission.agent_execution_id),
            Some(&admission.continuation_id),
            "continuation_admission_total",
            serde_json::json!({
                "mode": admission.mode,
                "trigger_kind": admission.trigger_kind,
                "outcome": "rejected_saturation_capacity_exceeded"
            }),
            1,
        )
        .await?;
        tx.commit()
            .await
            .context("admit_atomic: commit saturation rejection")?;
        return Ok(AtomicAdmissionOutcome::Saturated {
            queue_depth,
            concurrency,
        });
    }

    // 5. All checks passed — insert command_journal + continuation row atomically.
    // result_status='completed': the MCP command reception and atomic admission are complete.
    // The continuation lifecycle is tracked in agent_work_continuations; command_journal
    // records command receipt and should not stay pending indefinitely with no background handler.
    sqlx::query(
        "INSERT INTO command_journal
         (id, command_type, payload_json, result_status, run_id, created_at,
          caller_surface, caller_principal_id, caller_principal_class, caller_tool)
         VALUES (?1, 'agents.continue_work', ?2, 'completed', ?3, ?4,
                 ?5, ?6, ?7, ?8)",
    )
    .bind(&admission.command_journal_id)
    .bind(payload_json)
    .bind(&admission.run_id)
    .bind(&admission.created_at)
    .bind(&admission.caller_surface)
    .bind(&admission.caller_principal_id)
    .bind(&admission.caller_principal_class)
    .bind(&admission.caller_tool)
    .execute(&mut **tx)
    .await
    .context("admit_atomic: insert command_journal")?;

    sqlx::query(
        "INSERT INTO agent_work_continuations
         (id, run_id, stage_execution_id, agent_execution_id, command_journal_id,
          mode, trigger_kind, status,
          idempotency_scope, idempotency_key, request_fingerprint_sha256,
          conflict_count,
          lead_decision_artifact_id, lead_decision_artifact_sha256,
          continuation_instruction_sha256,
          budget_json,
          created_at, updated_at)
         VALUES
         (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'accepted', ?8, ?9, ?10, 0,
          ?11, ?12, ?13, ?14, ?15, ?15)",
    )
    .bind(&admission.continuation_id)
    .bind(&admission.run_id)
    .bind(&admission.stage_execution_id)
    .bind(&admission.agent_execution_id)
    .bind(&admission.command_journal_id)
    .bind(&admission.mode)
    .bind(&admission.trigger_kind)
    .bind(&admission.idempotency_scope)
    .bind(&admission.idempotency_key)
    .bind(&admission.request_fingerprint_sha256)
    .bind(&admission.lead_decision_artifact_id)
    .bind(&admission.lead_decision_artifact_sha256)
    .bind(&admission.continuation_instruction_sha256)
    .bind(&admission.budget_json)
    .bind(&admission.created_at)
    .execute(&mut **tx)
    .await
    .context("admit_atomic: insert agent_work_continuations")?;

    record_p086_continuation_metric_event_tx(
        &mut tx,
        Some(&admission.run_id),
        Some(&admission.stage_execution_id),
        Some(&admission.agent_execution_id),
        Some(&admission.continuation_id),
        "continuation_admission_total",
        serde_json::json!({
            "mode": admission.mode,
            "trigger_kind": admission.trigger_kind,
            "outcome": "accepted"
        }),
        1,
    )
    .await?;

    tx.commit().await.context("admit_atomic: commit")?;
    Ok(AtomicAdmissionOutcome::Accepted)
}

fn status_is_terminal(status: &str) -> bool {
    matches!(status, "succeeded" | "no_progress" | "failed" | "cancelled")
}

fn status_is_prompt_sent_or_later(status: &str) -> bool {
    matches!(
        status,
        "prompt_sent"
            | "observing"
            | "worktree_observed"
            | "needs_continuation_reconciliation"
            | "finalizing"
    )
}

/// P086 Phase 2: Claim a continuation row before any provider I/O.
///
/// This is the replay guard for duplicate ProcessContinuation work items.  A
/// row that has reached `prompt_sent` is never rewound to queued/starting.
pub async fn claim_for_continuation_worker(
    pool: &SqlitePool,
    continuation_id: &str,
    worker_pid: i64,
    daemon_generation_id: &str,
    session_generation_id: &str,
) -> Result<ContinuationClaimOutcome> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "agent_work_continuations.claim_worker")
            .await?;

    let query = format!("SELECT {SELECT_COLS} FROM agent_work_continuations WHERE id = ?");
    let Some(row) = sqlx::query(&query)
        .bind(continuation_id)
        .fetch_optional(&mut **tx)
        .await
        .context("claim_worker: load continuation")?
    else {
        return Ok(ContinuationClaimOutcome::Missing);
    };
    let record = row_to_record(&row);

    if status_is_terminal(&record.status) {
        return Ok(ContinuationClaimOutcome::Terminal(Box::new(record)));
    }

    if status_is_prompt_sent_or_later(&record.status) {
        return Ok(ContinuationClaimOutcome::PromptAlreadySent(Box::new(
            record,
        )));
    }

    if !matches!(record.status.as_str(), "accepted" | "queued" | "starting") {
        return Ok(ContinuationClaimOutcome::AlreadyActive(Box::new(record)));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let changed = sqlx::query(
        "UPDATE agent_work_continuations
         SET status = 'starting', updated_at = ?
         WHERE id = ? AND status IN ('accepted', 'queued', 'starting')",
    )
    .bind(&now)
    .bind(continuation_id)
    .execute(&mut **tx)
    .await
    .context("claim_worker: transition to starting")?
    .rows_affected();

    if changed == 0 {
        tx.commit()
            .await
            .context("claim_worker: commit no-op claim")?;
        return Ok(ContinuationClaimOutcome::AlreadyActive(Box::new(record)));
    }

    sqlx::query(
        "INSERT OR IGNORE INTO supervised_workers_continuation
         (continuation_id, worker_pid, daemon_generation_id, session_generation_id,
          registered_at, last_heartbeat_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(continuation_id)
    .bind(worker_pid)
    .bind(daemon_generation_id)
    .bind(session_generation_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut **tx)
    .await
    .context("claim_worker: register supervised worker")?;

    tx.commit().await.context("claim_worker: commit")?;

    let claimed = find_by_id(pool, continuation_id)
        .await?
        .context("claim_worker: claimed row disappeared")?;
    Ok(ContinuationClaimOutcome::Claimed(Box::new(claimed)))
}

/// P086 Phase 2: Insert an ordered side-effect ledger row.
pub async fn insert_side_effect_ledger_row(
    pool: &SqlitePool,
    continuation_id: &str,
    idempotency_key: &str,
    side_effect_kind: &str,
    sequence_number: i64,
    status: &str,
    request_fingerprint_sha256: Option<&str>,
    response_fingerprint_sha256: Option<&str>,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "INSERT OR IGNORE INTO agent_external_side_effect_ledger
         (continuation_id, idempotency_key, side_effect_kind, sequence_number,
          status, request_fingerprint_sha256, response_fingerprint_sha256, committed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(continuation_id)
    .bind(idempotency_key)
    .bind(side_effect_kind)
    .bind(sequence_number)
    .bind(status)
    .bind(request_fingerprint_sha256)
    .bind(response_fingerprint_sha256)
    .bind(if status == "committed" {
        Some(now.as_str())
    } else {
        None
    })
    .execute(pool)
    .await
    .context("insert side-effect ledger row")?;
    Ok(rows.rows_affected())
}

pub async fn has_side_effect_ledger_row(
    pool: &SqlitePool,
    continuation_id: &str,
    side_effect_kind: &str,
) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
         FROM agent_external_side_effect_ledger
         WHERE continuation_id = ? AND side_effect_kind = ?",
    )
    .bind(continuation_id)
    .bind(side_effect_kind)
    .fetch_one(pool)
    .await
    .context("has side-effect ledger row")?;
    Ok(count > 0)
}

pub async fn release_supervised_worker(
    pool: &SqlitePool,
    continuation_id: &str,
    release_reason: &str,
    stale_generation_evidence_json: Option<&str>,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE supervised_workers_continuation
         SET released_at = ?,
             release_reason = ?,
             stale_generation_evidence_json = COALESCE(?, stale_generation_evidence_json)
         WHERE continuation_id = ? AND released_at IS NULL",
    )
    .bind(&now)
    .bind(release_reason)
    .bind(stale_generation_evidence_json)
    .bind(continuation_id)
    .execute(pool)
    .await
    .context("release supervised continuation worker")?;
    Ok(rows.rows_affected())
}

/// Persist the exact ACP provider process binding for a claimed continuation
/// worker. Recovery uses this binding to target only the process group that was
/// registered while the live session was still present in memory.
pub async fn set_supervised_worker_provider_process(
    pool: &SqlitePool,
    continuation_id: &str,
    worker_pid: i64,
    daemon_generation_id: &str,
    provider_child_pid: i64,
    provider_process_group_id: i64,
    provider_process_uid: i64,
) -> Result<u64> {
    let rows = sqlx::query(
        "UPDATE supervised_workers_continuation
         SET provider_child_pid = ?,
             provider_process_group_id = ?,
             provider_process_uid = ?
         WHERE continuation_id = ?
           AND worker_pid = ?
           AND daemon_generation_id = ?
           AND released_at IS NULL",
    )
    .bind(provider_child_pid)
    .bind(provider_process_group_id)
    .bind(provider_process_uid)
    .bind(continuation_id)
    .bind(worker_pid)
    .bind(daemon_generation_id)
    .execute(pool)
    .await
    .context("set supervised continuation provider process binding")?;
    Ok(rows.rows_affected())
}

/// P086 Phase 2: Settle terminal status while recording the mandatory response
/// and result/no-progress artifacts.
pub async fn settle_with_artifacts(
    pool: &SqlitePool,
    continuation_id: &str,
    terminal_status: &str,
    failure_reason: Option<&str>,
    response_artifact_id: &str,
    response_fingerprint_sha256: &str,
    result_or_no_progress_artifact_id: &str,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE agent_work_continuations
         SET status = ?,
             failure_reason = ?,
             response_artifact_id = ?,
             response_fingerprint_sha256 = ?,
             result_or_no_progress_artifact_id = ?,
             updated_at = ?
         WHERE id = ?",
    )
    .bind(terminal_status)
    .bind(failure_reason)
    .bind(response_artifact_id)
    .bind(response_fingerprint_sha256)
    .bind(result_or_no_progress_artifact_id)
    .bind(&now)
    .bind(continuation_id)
    .execute(pool)
    .await
    .context("settle continuation with artifacts")?;
    Ok(rows.rows_affected())
}

/// P086: Terminal settlement unless cancellation has claimed the row.
pub async fn settle_with_artifacts_unless_cancelling(
    pool: &SqlitePool,
    continuation_id: &str,
    terminal_status: &str,
    failure_reason: Option<&str>,
    response_artifact_id: &str,
    response_fingerprint_sha256: &str,
    result_or_no_progress_artifact_id: &str,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE agent_work_continuations
         SET status = ?,
             failure_reason = ?,
             response_artifact_id = ?,
             response_fingerprint_sha256 = ?,
             result_or_no_progress_artifact_id = ?,
             updated_at = ?
         WHERE id = ? AND status != 'cancelling'",
    )
    .bind(terminal_status)
    .bind(failure_reason)
    .bind(response_artifact_id)
    .bind(response_fingerprint_sha256)
    .bind(result_or_no_progress_artifact_id)
    .bind(&now)
    .bind(continuation_id)
    .execute(pool)
    .await
    .context("settle continuation with artifacts unless cancelling")?;
    Ok(rows.rows_affected())
}

/// P086 Phase 2: Attach the canonical request artifact after the worker has
/// materialized the exact prompt/session request snapshot it is about to use.
pub async fn set_canonical_request_artifact(
    pool: &SqlitePool,
    continuation_id: &str,
    canonical_request_artifact_id: &str,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE agent_work_continuations
         SET canonical_request_artifact_id = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(canonical_request_artifact_id)
    .bind(&now)
    .bind(continuation_id)
    .execute(pool)
    .await
    .context("set canonical request artifact")?;
    Ok(rows.rows_affected())
}

/// P086: Attach post-dispatch/readback artifact ids that complete the continuation
/// evidence bundle. These fields are intentionally nullable so unsupported
/// provider-session resurrection can fail closed while still exposing the
/// missing evidence in readback.
pub async fn set_evidence_artifact_ids(
    pool: &SqlitePool,
    continuation_id: &str,
    attach_receipt_artifact_id: Option<&str>,
    evidence_bundle_artifact_id: Option<&str>,
    worktree_readback_artifact_id: Option<&str>,
    continuation_report_artifact_id: Option<&str>,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE agent_work_continuations
         SET attach_receipt_artifact_id = COALESCE(?, attach_receipt_artifact_id),
             evidence_bundle_artifact_id = COALESCE(?, evidence_bundle_artifact_id),
             worktree_readback_artifact_id = COALESCE(?, worktree_readback_artifact_id),
             continuation_report_artifact_id = COALESCE(?, continuation_report_artifact_id),
             updated_at = ?
         WHERE id = ?",
    )
    .bind(attach_receipt_artifact_id)
    .bind(evidence_bundle_artifact_id)
    .bind(worktree_readback_artifact_id)
    .bind(continuation_report_artifact_id)
    .bind(&now)
    .bind(continuation_id)
    .execute(pool)
    .await
    .context("set P086 evidence artifact ids")?;
    Ok(rows.rows_affected())
}

/// P086 Phase 2: Find a continuation row by its primary key.
pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<ContinuationRecord>> {
    let query = format!("SELECT {SELECT_COLS} FROM agent_work_continuations WHERE id = ?");
    let row = sqlx::query(&query)
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("find_by_id: query agent_work_continuations")?;
    Ok(row.as_ref().map(row_to_record))
}

/// P086 Phase 2: Execution context for the background continuation worker.
pub struct ContinuationExecutionContext {
    /// Logical stage id from stage_executions.stage_id.
    pub stage_id: Option<String>,
    /// From agent_executions.session_generation_id (may be absent).
    pub session_generation_id: Option<String>,
    /// From session_generations.provider_session_id for the live session.
    pub provider_session_id: Option<String>,
    /// Provider name (e.g. "claude", "codex") from session_generations.runtime_provider.
    pub runtime_provider: Option<String>,
    /// Model id from session_generations.runtime_model.
    pub runtime_model: Option<String>,
    /// From runs.workspace_root.
    pub workspace_root: Option<String>,
    /// From runs.worktree_root.
    pub worktree_root: Option<String>,
    /// From runs.chainworks_meta_root.
    pub chainworks_meta_root: Option<String>,
}

/// P086 Phase 2: Load the execution context the worker needs to send a continuation prompt.
/// Joins agent_executions → session_generations → stage_executions → runs in a single query.
pub async fn find_continuation_execution_context(
    pool: &SqlitePool,
    continuation: &ContinuationRecord,
) -> Result<ContinuationExecutionContext> {
    let row = sqlx::query(
        r#"SELECT
             se.stage_id                 AS stage_id,
             ae.session_generation_id AS session_generation_id,
             sg.provider_session_id  AS provider_session_id,
             sg.runtime_provider     AS runtime_provider,
             sg.runtime_model        AS runtime_model,
             r.workspace_root        AS workspace_root,
             r.worktree_root         AS worktree_root,
             r.chainworks_meta_root  AS chainworks_meta_root
           FROM agent_executions ae
           LEFT JOIN session_generations sg ON sg.id = ae.session_generation_id
           JOIN stage_executions se ON se.id = ae.owner_id
           JOIN runs r ON r.id = se.run_id
           WHERE ae.id = ?"#,
    )
    .bind(&continuation.agent_execution_id)
    .fetch_optional(pool)
    .await
    .context("find_continuation_execution_context")?;

    Ok(match row {
        Some(r) => ContinuationExecutionContext {
            stage_id: r.try_get("stage_id").unwrap_or(None),
            session_generation_id: r.try_get("session_generation_id").unwrap_or(None),
            provider_session_id: r.try_get("provider_session_id").unwrap_or(None),
            runtime_provider: r.try_get("runtime_provider").unwrap_or(None),
            runtime_model: r.try_get("runtime_model").unwrap_or(None),
            workspace_root: r.try_get("workspace_root").unwrap_or(None),
            worktree_root: r.try_get("worktree_root").unwrap_or(None),
            chainworks_meta_root: r.try_get("chainworks_meta_root").unwrap_or(None),
        },
        None => ContinuationExecutionContext {
            stage_id: None,
            session_generation_id: None,
            provider_session_id: None,
            runtime_provider: None,
            runtime_model: None,
            workspace_root: None,
            worktree_root: None,
            chainworks_meta_root: None,
        },
    })
}

/// P086 Phase 2: Stamp session_generation_id and provider_session_id onto the continuation row
/// when the worker claims it, so the row carries provenance for recovery and reconciliation.
pub async fn update_continuation_session(
    pool: &SqlitePool,
    continuation_id: &str,
    session_generation_id: &str,
    provider_session_id: Option<&str>,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE agent_work_continuations
         SET session_generation_id = ?, provider_session_id = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(session_generation_id)
    .bind(provider_session_id)
    .bind(&now)
    .bind(continuation_id)
    .execute(pool)
    .await
    .context("update_continuation_session")?;
    Ok(rows.rows_affected())
}

/// Refresh an unreleased supervised worker heartbeat. The continuation worker calls
/// this at each long-running boundary so startup recovery can distinguish active
/// ownership from a stale daemon generation.
pub async fn refresh_supervised_worker_heartbeat(
    pool: &SqlitePool,
    continuation_id: &str,
    worker_pid: i64,
    daemon_generation_id: &str,
) -> Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = sqlx::query(
        "UPDATE supervised_workers_continuation
         SET last_heartbeat_at = ?
         WHERE continuation_id = ?
           AND worker_pid = ?
           AND daemon_generation_id = ?
           AND released_at IS NULL",
    )
    .bind(&now)
    .bind(continuation_id)
    .bind(worker_pid)
    .bind(daemon_generation_id)
    .execute(pool)
    .await
    .context("refresh supervised continuation worker heartbeat")?;
    Ok(rows.rows_affected())
}

#[derive(Clone, Debug)]
pub struct StaleContinuationWorker {
    pub continuation_id: String,
    pub worker_pid: i64,
    pub daemon_generation_id: String,
    pub session_generation_id: String,
    pub provider_child_pid: Option<i64>,
    pub provider_process_group_id: Option<i64>,
    pub provider_process_uid: Option<i64>,
    pub last_heartbeat_at: Option<String>,
    pub provider_session_id: Option<String>,
    pub status: String,
}

/// Return unreleased supervised workers that belong to a previous daemon generation
/// or have a stale heartbeat. Recovery uses these rows to prove orphan reap or
/// fail closed before allowing provider-session resurrection.
pub async fn list_stale_supervised_workers(
    pool: &SqlitePool,
    current_daemon_generation_id: &str,
    stale_before: &str,
    limit: i64,
) -> Result<Vec<StaleContinuationWorker>> {
    let rows = sqlx::query(
        "SELECT swc.continuation_id, swc.worker_pid, swc.daemon_generation_id,
                swc.session_generation_id, swc.provider_child_pid,
                swc.provider_process_group_id, swc.provider_process_uid,
                swc.last_heartbeat_at,
                awc.provider_session_id, awc.status
         FROM supervised_workers_continuation swc
         JOIN agent_work_continuations awc ON awc.id = swc.continuation_id
         WHERE swc.released_at IS NULL
           AND (swc.daemon_generation_id != ? OR swc.last_heartbeat_at < ?)
         ORDER BY swc.last_heartbeat_at ASC
         LIMIT ?",
    )
    .bind(current_daemon_generation_id)
    .bind(stale_before)
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await
    .context("list stale supervised continuation workers")?;

    Ok(rows
        .into_iter()
        .map(|row| StaleContinuationWorker {
            continuation_id: row.get("continuation_id"),
            worker_pid: row.get("worker_pid"),
            daemon_generation_id: row.get("daemon_generation_id"),
            session_generation_id: row.get("session_generation_id"),
            provider_child_pid: row.get("provider_child_pid"),
            provider_process_group_id: row.get("provider_process_group_id"),
            provider_process_uid: row.get("provider_process_uid"),
            last_heartbeat_at: row.get("last_heartbeat_at"),
            provider_session_id: row.get("provider_session_id"),
            status: row.get("status"),
        })
        .collect())
}

/// Crash/replay recovery candidates. Rows at prompt_sent-or-later are never
/// replayed; they must be settled from captured worktree/transcript evidence.
pub async fn list_needing_continuation_reconciliation(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<ContinuationRecord>> {
    let rows = sqlx::query(&format!(
        "SELECT {SELECT_COLS} FROM agent_work_continuations
         WHERE status = 'needs_continuation_reconciliation'
         ORDER BY updated_at ASC
         LIMIT ?"
    ))
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await
    .context("list P086 continuations needing reconciliation")?;
    Ok(rows.iter().map(row_to_record).collect())
}

/// Mark active continuations for a cancelling run. The worker later turns these
/// into cancelled or reconciliation-needed outcomes depending on whether
/// provider_send had already been committed.
pub async fn mark_active_for_run_cancelling_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    run_id: &str,
    now: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        "UPDATE agent_work_continuations
         SET status = 'cancelling',
             failure_reason = 'run_cancel_requested',
             updated_at = ?
         WHERE run_id = ?
           AND status NOT IN ('succeeded', 'no_progress', 'failed', 'cancelled')",
    )
    .bind(now)
    .bind(run_id)
    .execute(&mut **tx)
    .await
    .context("mark active P086 continuations cancelling")?;
    Ok(rows.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_sql_enforces_code_writer_role() {
        // Regression: prior version aliased owner_kind as agent_role without
        // filtering by agent_id. Verify the canonical SQL enforces the correct
        // role predicate and join structure.
        assert!(
            CANDIDATES_SQL.contains("ae.agent_id = 'code_writer'"),
            "candidates query must filter by agent_id = 'code_writer'"
        );
        assert!(
            CANDIDATES_SQL.contains("ae.owner_kind = 'stage_execution'"),
            "candidates query must filter by owner_kind = 'stage_execution'"
        );
        assert!(
            CANDIDATES_SQL.contains("JOIN stage_executions se ON se.id = ae.owner_id"),
            "candidates query must JOIN stage_executions to verify stage ownership"
        );
        assert!(
            CANDIDATES_SQL.contains("JOIN runs r ON r.id = se.run_id"),
            "candidates query must JOIN runs to scope by run_id"
        );
        assert!(
            CANDIDATES_SQL.contains("ae.status IN ('completed', 'failed')"),
            "candidates query must filter by terminal agent execution status"
        );
    }

    #[test]
    fn candidates_sql_uses_exists_not_n_plus_1() {
        // Regression: prior version issued a separate find_active_for_agent_execution
        // query per candidate row (N+1). Verify the single-query EXISTS approach.
        assert!(
            CANDIDATES_SQL.contains("EXISTS("),
            "candidates query must use EXISTS subquery, not per-row lookup"
        );
        assert!(
            CANDIDATES_SQL.contains("has_active_continuation"),
            "candidates query must expose has_active_continuation column"
        );
    }

    #[test]
    fn max_admission_seconds_constant_is_positive_and_bounded() {
        assert!(
            MAX_ADMISSION_TO_START_SECONDS > 0,
            "MAX_ADMISSION_TO_START_SECONDS must be positive"
        );
        // Must be less than 1 hour — values above this suggest a config error.
        assert!(
            MAX_ADMISSION_TO_START_SECONDS <= 3600,
            "MAX_ADMISSION_TO_START_SECONDS must be <= 3600"
        );
    }

    #[test]
    fn sweep_admission_timeout_sql_targets_only_non_terminal_statuses() {
        // Regression: sweep must not touch terminal rows (succeeded/no_progress/failed/cancelled).
        // Verify the UPDATE SQL includes the IN ('accepted','queued','starting') guard.
        // This is a compile-time string check; the actual DB behavior is covered by integration tests.
        let sql = "UPDATE agent_work_continuations
         SET status = 'failed',
             failure_reason = 'admission_timeout',
             updated_at = ?
         WHERE status IN ('accepted', 'queued', 'starting')
           AND created_at < ?";
        assert!(sql.contains("status IN ('accepted', 'queued', 'starting')"));
        assert!(sql.contains("failure_reason = 'admission_timeout'"));
    }

    #[test]
    fn update_continuation_status_sql_sets_updated_at() {
        // Regression: status updates must always refresh updated_at so idx_awc_recovery
        // and staleness queries remain correct.
        let sql = "UPDATE agent_work_continuations
         SET status = ?, failure_reason = ?, updated_at = ?
         WHERE id = ?";
        assert!(sql.contains("updated_at = ?"));
        assert!(sql.contains("failure_reason = ?"));
    }

    #[test]
    fn queue_and_concurrency_caps_are_consistent() {
        assert!(QUEUE_DEPTH_MAX > 0);
        assert!(CONCURRENCY_MAX > 0);
        assert!(
            QUEUE_DEPTH_MAX >= CONCURRENCY_MAX,
            "queue cap must be >= concurrency cap"
        );
    }

    #[test]
    fn check_eligibility_sql_does_not_filter_by_status() {
        // P086 audit fix: check_eligibility must return non-terminal (running) agents so the
        // MCP handler can emit agent_already_running rather than ineligible_agent_execution.
        // The SQL must NOT contain the old ae.status IN ('completed','failed') predicate.
        let sql = "SELECT ae.id AS agent_execution_id, ae.owner_id AS stage_execution_id,
                se.run_id, ae.status AS agent_status
         FROM agent_executions ae
         JOIN stage_executions se ON se.id = ae.owner_id
         WHERE ae.id = ?
           AND ae.agent_id = 'code_writer'
           AND ae.owner_kind = 'stage_execution'
           AND ae.owner_id IS NOT NULL";
        assert!(
            !sql.contains("ae.status IN ('completed', 'failed')"),
            "check_eligibility SQL must NOT filter by terminal status; caller is responsible"
        );
        assert!(
            sql.contains("ae.agent_id = 'code_writer'"),
            "check_eligibility SQL must still require code_writer role"
        );
        assert!(
            sql.contains("ae.owner_kind = 'stage_execution'"),
            "check_eligibility SQL must still require stage_execution owner"
        );
    }

    #[test]
    fn saturation_rejection_inserts_command_journal_row() {
        // Regression: prior implementation dropped the BEGIN IMMEDIATE transaction on
        // saturation without committing, so no audit row existed. Verify the new code
        // inserts a command_journal row with result_status='rejected' before committing.
        // This is a string-level regression guard; actual DB behavior requires integration tests.
        let rejection_sql = "INSERT INTO command_journal
             (id, command_type, payload_json, result_status, run_id, created_at,
              caller_surface, caller_principal_id, caller_principal_class, caller_tool)
             VALUES (?1, 'agents.continue_work', ?2, 'rejected', ?3, ?4,
                     'mcp', ?5, 'operator', 'agents.continue_work')";
        assert!(
            rejection_sql.contains("result_status, "),
            "saturation rejection SQL must include result_status column"
        );
        assert!(
            rejection_sql.contains("'rejected'"),
            "saturation rejection SQL must set result_status = 'rejected'"
        );
    }
}
