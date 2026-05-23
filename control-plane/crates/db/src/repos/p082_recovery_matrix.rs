//! P082: Shared durable readback accessor for the recovery/retry state-machine matrix.
//!
//! All MCP lanes (runs.get, reports.get, report://{run_id}, run report JSON, and
//! release receipt diagnostics) must call `readbacks_for_run` rather than
//! re-implementing their own queries.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use sqlx::{Row, SqlitePool};

/// Maximum byte size per readback row to guard against unbounded JSON parsing
/// (SEC-P082-003).
const MAX_READBACK_ROW_BYTES: usize = 64 * 1024;

/// Maximum number of readback rows returned per run (SEC-P082-003).
const MAX_READBACK_ROWS: usize = 1_000;

/// Maximum byte length per individual string field within a readback row.
/// Values that exceed this are replaced with a length marker (SEC-P082-001).
const MAX_STRING_FIELD_BYTES: usize = 2048;

/// Allowlisted top-level field names for `p082_recovery_matrix_readback_v1`.
/// Unknown keys are stripped before the rows are returned to callers (SEC-P082-001).
const READBACK_ALLOWLIST: &[&str] = &[
    "schema_version",
    "scenario_id",
    "scenario_status",
    "recovery_decision",
    "recovery_reason_code",
    "recovery_next_action",
    "recovery_hold_conditions",
    "recovery_side_effect_blocking_status",
    "recovery_retry_identifier_guidance",
    "recovery_late_output_settlement",
    "recovery_startup_repair_summary",
    "recovery_operator_message",
    "recovery_projection_integrity",
    "source_table",
    "source_repository",
    "source_identifier",
    "source_json_key",
    "updated_at",
    "diagnostic_redaction",
];

/// Allowlisted field names for `p082_retry_identifier_guidance_v1` nested subcontract.
const RETRY_IDENTIFIER_GUIDANCE_ALLOWLIST: &[&str] = &[
    "schema_version",
    "command",
    "provided_identifier",
    "provided_identifier_kind",
    "expected_identifier_kind",
    "valid_identifier_examples",
    "no_mutation",
];

/// Allowlisted field names for `p082_late_output_settlement_v1` nested subcontract.
const LATE_OUTPUT_SETTLEMENT_ALLOWLIST: &[&str] = &[
    "schema_version",
    "source_agent_execution_id",
    "source_work_item_id",
    "source_session_generation_id",
    "active_session_generation_id",
    "claim_state",
    "output_settlement",
    "ignored_late_output_count",
    "source_work_item_terminal_status",
    "active_projection_changed",
    "cancelled_provider_session",
];

/// Allowlisted field names for `p082_startup_repair_summary_v1` nested subcontract.
const STARTUP_REPAIR_SUMMARY_ALLOWLIST: &[&str] = &[
    "schema_version",
    "startup_repair_id",
    "source_work_item_id",
    "source_command_journal_id",
    "requeue_generation",
    "max_requeue_generation",
    "replayed",
    "stale_after_ms",
    "stale_cutoff",
    "xcode_required",
    "next_retry_or_backoff_time",
    "backpressure_scope",
];

fn sanitize_string(value: String) -> serde_json::Value {
    let lower = value.to_ascii_lowercase();
    if value.len() > MAX_STRING_FIELD_BYTES {
        return serde_json::Value::String(format!("[redacted: {} bytes]", value.len()));
    }
    // Auth material / secret patterns
    if lower.contains("bearer ")
        || lower.contains("access_token")
        || lower.contains("auth_token")
        || lower.contains("secret")
        || lower.contains("sk-")
        || lower.contains("/.ssh/")
    {
        return serde_json::Value::String("[redacted]".to_string());
    }
    // Absolute filesystem paths — the proposal requires operator-safe paths relative
    // to the run meta-root only. Absolute paths can expose home directory layout and
    // local filesystem structure (SEC-P082-HIGH-2 fix).
    if value.starts_with("/Users/")
        || value.starts_with("/private/")
        || value.starts_with("/var/")
        || value.starts_with("/tmp/")
        || value.starts_with("/home/")
        || value.starts_with("/root/")
        || value.starts_with("/opt/")
        || lower.contains("/.chainworks/runs/")
    {
        return serde_json::Value::String("[redacted]".to_string());
    }
    serde_json::Value::String(value)
}

/// Returns true if any string value in `v` (recursively) is a redaction marker
/// (starts with "[redacted"). Used to update `diagnostic_redaction` after projection.
fn value_contains_redaction_marker(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::String(s) => s.starts_with("[redacted"),
        serde_json::Value::Array(items) => items.iter().any(value_contains_redaction_marker),
        serde_json::Value::Object(m) => m.values().any(value_contains_redaction_marker),
        _ => false,
    }
}

fn sanitize_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => sanitize_string(s),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sanitize_value).collect())
        }
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(key, value)| (key, sanitize_value(value)))
                .collect(),
        ),
        other => other,
    }
}

/// Recursively sanitize a nested subcontract object: strip unknown keys, cap
/// string lengths, and redact path/secret-looking strings at every depth.
/// Non-object, non-null values are replaced with null (tamper fallback).
/// This is the fix for SEC-P082-001: nested subcontracts must be recursively
/// sanitized, not passed through unchanged.
fn sanitize_nested_subcontract(val: serde_json::Value, allowlist: &[&str]) -> serde_json::Value {
    match val {
        serde_json::Value::Null => serde_json::Value::Null,
        serde_json::Value::Object(map) => {
            let projected = map
                .into_iter()
                .filter(|(k, _)| allowlist.contains(&k.as_str()))
                .map(|(k, v)| (k, sanitize_value(v)))
                .collect();
            serde_json::Value::Object(projected)
        }
        // Non-object, non-null value for a subcontract field: tamper fallback — replace with null.
        _ => serde_json::Value::Null,
    }
}

/// Strip unknown keys from a readback object, cap string field lengths at
/// `MAX_STRING_FIELD_BYTES`, and recursively sanitize nested subcontract objects
/// (SEC-P082-001 / HIGH-1 fix).
///
/// Nested fields `recovery_retry_identifier_guidance`, `recovery_late_output_settlement`,
/// and `recovery_startup_repair_summary` are passed through `sanitize_nested_subcontract`
/// so that injected keys (e.g. `access_token`, `raw_stderr`, absolute paths) cannot
/// leak through MCP/report/release lanes.
///
/// Sets `diagnostic_redaction` to `"partial"` when any string was replaced with a
/// redaction marker or when injected unknown keys were stripped from the top-level object.
fn allowlist_project(obj: serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    let original_key_count = obj.len();
    let mut projected: serde_json::Map<String, serde_json::Value> = obj
        .into_iter()
        .filter(|(k, _)| READBACK_ALLOWLIST.contains(&k.as_str()))
        .map(|(k, v)| {
            let v = match k.as_str() {
                "recovery_retry_identifier_guidance" => {
                    sanitize_nested_subcontract(v, RETRY_IDENTIFIER_GUIDANCE_ALLOWLIST)
                }
                "recovery_late_output_settlement" => {
                    sanitize_nested_subcontract(v, LATE_OUTPUT_SETTLEMENT_ALLOWLIST)
                }
                "recovery_startup_repair_summary" => {
                    sanitize_nested_subcontract(v, STARTUP_REPAIR_SUMMARY_ALLOWLIST)
                }
                _ => sanitize_value(v),
            };
            (k, v)
        })
        .collect();

    // Upgrade diagnostic_redaction to "partial" when:
    // - Unknown keys were stripped from the top-level object (injected fields), or
    // - Any string value was replaced with a [redacted...] marker.
    let keys_stripped = projected.len() < original_key_count;
    let string_redacted = projected.values().any(value_contains_redaction_marker);
    if (keys_stripped || string_redacted)
        && projected
            .get("diagnostic_redaction")
            .and_then(|v| v.as_str())
            == Some("none")
    {
        projected.insert(
            "diagnostic_redaction".to_string(),
            serde_json::Value::String("partial".to_string()),
        );
    }

    serde_json::Value::Object(projected)
}

fn push_valid_projected_readback(
    readbacks: &mut Vec<serde_json::Value>,
    readback: serde_json::Value,
) {
    if domain::recovery_matrix::validate_readback_v1_shape(&readback) {
        if let Some(obj) = readback.as_object().cloned() {
            readbacks.push(allowlist_project(obj));
        }
    }
}

fn parse_utc_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

/// Collect all `p082_recovery_matrix_readback_v1` rows for a run from the two
/// existing durable owners:
///
/// 1. `startup_repairs.notes.p082_recovery_matrix_readback` — startup/crash repair rows.
/// 2. `command_journal.error` — parsed as `p082_rejected_command_error_v1` for
///    rejection rows.
///
/// Rows are bounded to `MAX_READBACK_ROWS` and each JSON fragment is bounded to
/// `MAX_READBACK_ROW_BYTES`. Legacy plain-text `command_journal.error` values are
/// skipped safely without panicking. Unknown JSON keys are stripped via allowlist
/// projection. Only rows whose `schema_version` equals
/// `p082_recovery_matrix_readback_v1` are returned; validation-error sentinels and
/// unknown schema versions are silently excluded.
///
/// Result is sorted by `updated_at` ASC, then `scenario_id` ASC (ties).
pub async fn readbacks_for_run(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> Result<Vec<serde_json::Value>> {
    use domain::recovery_matrix;
    let mut readbacks: Vec<serde_json::Value> = Vec::new();

    // Source 1: startup_repairs.notes.p082_recovery_matrix_readback
    // SQL-side LENGTH guard prevents oversized rows from being fully loaded into memory
    // before the in-memory size check (SEC-P082-MEDIUM-2 fix).
    let repairs = sqlx::query(
        r#"SELECT notes, repaired_at
           FROM startup_repairs
           WHERE run_id = ?1
             AND (notes IS NULL OR LENGTH(notes) <= ?2)
           ORDER BY repaired_at ASC
           LIMIT ?3"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROW_BYTES as i64)
    .bind(MAX_READBACK_ROWS as i64)
    .fetch_all(pool)
    .await?;

    for row in repairs {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let notes_raw: Option<String> = row.try_get("notes").unwrap_or(None);
        let repaired_at: String = row.try_get("repaired_at").unwrap_or_default();
        if let Some(notes_str) = notes_raw {
            if let Ok(notes_json) = serde_json::from_str::<serde_json::Value>(&notes_str) {
                if let Some(readback) = notes_json.get("p082_recovery_matrix_readback") {
                    if readback.is_null() {
                        continue;
                    }
                    // Full shape validation before exposing to callers (SEC-P082-MEDIUM-1 fix).
                    // Rows that fail validation emit a tamper_detected fallback instead of
                    // passing potentially malformed data through operator-facing lanes.
                    if recovery_matrix::validate_readback_v1_shape(readback) {
                        if let Some(obj) = readback.as_object().cloned() {
                            readbacks.push(allowlist_project(obj));
                        }
                    } else {
                        // Shape validation failed — emit a sanitized tamper_detected row.
                        let fallback = recovery_matrix::build_readback_v1(
                            "P082-R01",
                            "held",
                            "wait",
                            recovery_matrix::REASON_RESUME_CLAIM_STATUS,
                            "Startup repair readback shape validation failed; tamper or schema drift detected.",
                            "startup_repairs",
                            "startup_repairs",
                            &format!("startup-repair-tampered-shape:{run_id}"),
                            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                            "tamper_detected",
                            &repaired_at,
                        );
                        if let Some(obj) = fallback.as_object().cloned() {
                            readbacks.push(allowlist_project(obj));
                        }
                    }
                }
            }
        }
    }

    // Source 2: command_journal.error as p082_rejected_command_error_v1.
    // Also emits a safe fallback row (recovery_projection_integrity=unavailable)
    // for legacy plain-text or non-P082 JSON errors, per P082 backward-compat rule.
    // SQL-side LENGTH guard (SEC-P082-MEDIUM-2 fix).
    let rejected = sqlx::query(
        r#"SELECT id, error, created_at
           FROM command_journal
           WHERE run_id = ?1
             AND result_status = 'failed'
             AND error IS NOT NULL
             AND LENGTH(error) <= ?2
           ORDER BY created_at ASC
           LIMIT ?3"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROW_BYTES as i64)
    .bind(MAX_READBACK_ROWS as i64)
    .fetch_all(pool)
    .await?;

    for row in rejected {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let error_raw: Option<String> = row.try_get("error").unwrap_or(None);
        let journal_id: String = row.try_get("id").unwrap_or_default();
        let created_at: String = row.try_get("created_at").unwrap_or_default();
        if let Some(error_str) = error_raw {
            if error_str.len() > MAX_READBACK_ROW_BYTES {
                continue;
            }
            if let Some(envelope) =
                recovery_matrix::parse_command_journal_error_envelope(&error_str)
            {
                if let Some(readback) = envelope.get("p082_recovery_matrix_readback") {
                    if !readback.is_null()
                        && readback.get("schema_version").and_then(|v| v.as_str())
                            == Some(recovery_matrix::SCHEMA_READBACK_V1)
                    {
                        if let Some(obj) = readback.as_object().cloned() {
                            readbacks.push(allowlist_project(obj));
                        }
                    }
                }
            } else {
                // Legacy plain-text or non-P082 JSON: surface a safe fallback row with
                // recovery_projection_integrity=unavailable. Raw error text is NOT exposed.
                // scenario_id uses P082-R02 (rejected command context) as the closest
                // canonical fit; recovery_projection_integrity=unavailable signals that the
                // typed readback envelope is absent. This preserves the canonical vocabulary.
                let fallback = recovery_matrix::build_readback_v1(
                    "P082-R02",
                    "held",
                    "wait",
                    recovery_matrix::REASON_RESUME_CLAIM_STATUS,
                    "Legacy rejection record exists. Inspect command journal for recovery details; no P082 typed readback is available.",
                    "command_journal",
                    "command_journal",
                    &journal_id,
                    None,
                    "unavailable",
                    &created_at,
                );
                if let Some(obj) = fallback.as_object().cloned() {
                    readbacks.push(allowlist_project(obj));
                }
            }
        }
    }

    // Source 3: runs.cancellation_settlement_log (R11/R12/R13/R14)
    // Each entry in the JSON array may carry p082_recovery_matrix_readback.
    // SQL-side LENGTH guard prevents loading oversized columns.
    let cancel_rows = sqlx::query(
        r#"SELECT cancellation_settlement_log, cancellation_requested_at
           FROM runs
           WHERE id = ?1
             AND cancellation_settlement_log IS NOT NULL
             AND LENGTH(cancellation_settlement_log) <= ?2"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROW_BYTES as i64)
    .fetch_all(pool)
    .await?;

    for row in cancel_rows {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let log_raw: Option<String> = row.try_get("cancellation_settlement_log").unwrap_or(None);
        let requested_at: Option<String> = row.try_get("cancellation_requested_at").unwrap_or(None);
        if let Some(log_str) = log_raw {
            if log_str.len() > MAX_READBACK_ROW_BYTES {
                continue;
            }
            if let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(&log_str) {
                for entry in entries {
                    if readbacks.len() >= MAX_READBACK_ROWS {
                        break;
                    }
                    if let Some(rb) = entry.get("p082_recovery_matrix_readback") {
                        if rb.is_null() {
                            continue;
                        }
                        if recovery_matrix::validate_readback_v1_shape(rb) {
                            if let Some(obj) = rb.as_object().cloned() {
                                readbacks.push(allowlist_project(obj));
                            }
                        } else {
                            let ts = requested_at.as_deref().unwrap_or("");
                            let fallback = recovery_matrix::build_readback_v1(
                                "P082-R11",
                                "held",
                                "wait",
                                recovery_matrix::REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
                                "Cancellation settlement readback shape validation failed; tamper or schema drift detected.",
                                "runs",
                                "runs",
                                &run_id.to_string(),
                                Some("runs.cancellation_settlement_log.p082_recovery_matrix_readback"),
                                "tamper_detected",
                                ts,
                            );
                            if let Some(obj) = fallback.as_object().cloned() {
                                readbacks.push(allowlist_project(obj));
                            }
                        }
                    }
                }
            }
        }
    }

    // Source 4: stage_executions.recovery_snapshot_json (R03/R09/R17)
    // Each stage execution may carry p082_recovery_matrix_readback in its snapshot.
    // SQL-side LENGTH guard prevents loading oversized columns.
    let snapshot_rows = sqlx::query(
        r#"SELECT id, recovery_snapshot_json, started_at
           FROM stage_executions
           WHERE run_id = ?1
             AND recovery_snapshot_json IS NOT NULL
             AND LENGTH(recovery_snapshot_json) <= ?2
           ORDER BY started_at ASC
           LIMIT ?3"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROW_BYTES as i64)
    .bind(MAX_READBACK_ROWS as i64)
    .fetch_all(pool)
    .await?;

    for row in snapshot_rows {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let snapshot_raw: Option<String> = row.try_get("recovery_snapshot_json").unwrap_or(None);
        let stage_id: String = row.try_get("id").unwrap_or_default();
        let started_at: String = row.try_get("started_at").unwrap_or_default();
        if let Some(snap_str) = snapshot_raw {
            if snap_str.len() > MAX_READBACK_ROW_BYTES {
                continue;
            }
            if let Ok(snap_json) = serde_json::from_str::<serde_json::Value>(&snap_str) {
                if let Some(rb) = snap_json.get("p082_recovery_matrix_readback") {
                    if rb.is_null() {
                        continue;
                    }
                    if recovery_matrix::validate_readback_v1_shape(rb) {
                        if let Some(obj) = rb.as_object().cloned() {
                            readbacks.push(allowlist_project(obj));
                        }
                    } else {
                        let fallback = recovery_matrix::build_readback_v1(
                            "P082-R03",
                            "held",
                            "wait",
                            recovery_matrix::REASON_IGNORED_LATE_OUTPUTS,
                            "Stage recovery snapshot readback shape validation failed; tamper or schema drift detected.",
                            "stage_executions",
                            "stage_executions",
                            &stage_id,
                            Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
                            "tamper_detected",
                            &started_at,
                        );
                        if let Some(obj) = fallback.as_object().cloned() {
                            readbacks.push(allowlist_project(obj));
                        }
                    }
                }
            }
        }
    }

    // Source 5: approvals table (R09).
    //
    // Pending approvals are already durable run truth. The P082 accessor projects a
    // diagnostic row from that owner so restart readback can show that the approval
    // was preserved and still requires the existing human approval path. This is
    // read-only and does not synthesize an approval decision.
    let approval_rows = sqlx::query(
        r#"SELECT id, stage_id, requested_at
           FROM approvals
           WHERE run_id = ?1
             AND decision IN ('pending', 'requested')
           ORDER BY requested_at ASC
           LIMIT ?2"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROWS as i64)
    .fetch_all(pool)
    .await?;

    for row in approval_rows {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let approval_id: String = row.try_get("id").unwrap_or_default();
        let requested_at: String = row.try_get("requested_at").unwrap_or_default();
        let readback = recovery_matrix::build_readback_v1(
            "P082-R09",
            "pending",
            "operator_approval_required",
            recovery_matrix::REASON_APPROVAL_PENDING_OPERATOR_ACTION_REQUIRED,
            "Pending approval was preserved across recovery; use the existing approval path.",
            "approvals, approval_inbox, stage_executions",
            "approvals, approval_inbox, stage_executions",
            &approval_id,
            None,
            "valid",
            &requested_at,
        );
        push_valid_projected_readback(&mut readbacks, readback);
    }

    // Source 6: lead_conflict_mediations table (R10).
    //
    // Active duplicate mediation owners are prevented by a partial unique index, so
    // durable duplicate evidence appears as a second row that has been superseded,
    // canceled, or otherwise terminalized. A grouped readback makes that evidence
    // visible without relaxing the unique-owner invariant.
    let mediation_rows = sqlx::query(
        r#"SELECT conflict_fingerprint,
                  MIN(id) AS source_id,
                  COUNT(*) AS mediation_count,
                  MAX(updated_at) AS updated_at
           FROM lead_conflict_mediations
           WHERE run_id = ?1
           GROUP BY conflict_fingerprint
           HAVING COUNT(*) > 1
           ORDER BY updated_at ASC
           LIMIT ?2"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROWS as i64)
    .fetch_all(pool)
    .await?;

    for row in mediation_rows {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let source_id: String = row.try_get("source_id").unwrap_or_default();
        let updated_at: String = row.try_get("updated_at").unwrap_or_default();
        let readback = recovery_matrix::build_readback_v1(
            "P082-R10",
            "rejected",
            "inspect_duplicate_owner",
            recovery_matrix::REASON_DUPLICATE_MEDIATION_OWNER_REJECTED,
            "Duplicate mediation owner evidence was preserved; inspect the surviving mediation owner.",
            "lead_conflict_mediations, lead_mediation_confirmations, workflow_conflicts",
            "lead_conflict_mediations, lead_mediation_confirmations, workflow_conflicts",
            &source_id,
            None,
            "valid",
            &updated_at,
        );
        push_valid_projected_readback(&mut readbacks, readback);
    }

    // Source 7: session_lineages/session_generations (R04).
    //
    // A duplicate active invocation_owner_key means more than one active session
    // generation claims the same owner. This diagnostic row is intentionally
    // read-only: the repair path remains the existing session terminalization flow.
    let duplicate_session_rows = sqlx::query(
        r#"SELECT sg.invocation_owner_key,
                  MIN(sg.id) AS source_id,
                  COUNT(*) AS generation_count,
                  MAX(sg.created_at) AS updated_at
           FROM session_generations sg
           INNER JOIN session_lineages sl ON sl.id = sg.lineage_id
           WHERE sl.run_id = ?1
             AND sg.status = 'active'
           GROUP BY sg.invocation_owner_key
           HAVING COUNT(*) > 1
           ORDER BY updated_at ASC
           LIMIT ?2"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROWS as i64)
    .fetch_all(pool)
    .await?;

    for row in duplicate_session_rows {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let source_id: String = row.try_get("source_id").unwrap_or_default();
        let updated_at: String = row.try_get("updated_at").unwrap_or_default();
        let readback = recovery_matrix::build_readback_v1(
            "P082-R04",
            "held",
            "inspect_duplicate_owner",
            recovery_matrix::REASON_DUPLICATE_OWNER_REPAIRED,
            "Duplicate active session owner detected; inspect and terminalize the duplicate owner through existing recovery paths.",
            "session_lineages, session_generations, session_events, work_items",
            "session_lineages, session_generations, session_events, work_items",
            &source_id,
            None,
            "stale",
            &updated_at,
        );
        push_valid_projected_readback(&mut readbacks, readback);
    }

    // Source 8: stale active session generations without a provider session (R05).
    //
    // The accessor reports stale startup evidence from existing session rows. It
    // does not perform the requeue; startup recovery remains the mutation owner.
    let startup_rows = sqlx::query(
        r#"SELECT sg.id, sg.invocation_owner_key, sg.runtime_provider, sg.runtime_model,
                  sg.created_at
           FROM session_generations sg
           INNER JOIN session_lineages sl ON sl.id = sg.lineage_id
           WHERE sl.run_id = ?1
             AND sg.status = 'active'
             AND sg.provider_session_id IS NULL
             AND sg.last_activity_at IS NULL
           ORDER BY sg.created_at ASC
           LIMIT ?2"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROWS as i64)
    .fetch_all(pool)
    .await?;

    let now = Utc::now();
    for row in startup_rows {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let generation_id: String = row.try_get("id").unwrap_or_default();
        let owner_key: String = row.try_get("invocation_owner_key").unwrap_or_default();
        let runtime_provider: String = row.try_get("runtime_provider").unwrap_or_default();
        let runtime_model: String = row.try_get("runtime_model").unwrap_or_default();
        let created_at: String = row.try_get("created_at").unwrap_or_default();
        let xcode_required = runtime_provider.to_ascii_lowercase().contains("xcode")
            || runtime_model.to_ascii_lowercase().contains("xcode");
        let stale_after = if xcode_required {
            Duration::minutes(12)
        } else {
            Duration::minutes(3)
        };
        let Some(created_at_dt) = parse_utc_rfc3339(&created_at) else {
            continue;
        };
        if now.signed_duration_since(created_at_dt) < stale_after {
            continue;
        }
        let stale_after_ms = stale_after.num_milliseconds();
        // stale_cutoff is the deadline when the grace period expired, not the session start.
        // created_at_dt + stale_after = the timestamp at which the session became stale.
        let stale_cutoff_dt = created_at_dt + stale_after;
        let stale_cutoff = stale_cutoff_dt.to_rfc3339();
        let repair_id = format!("p082-stale-startup:{generation_id}");
        let summary = recovery_matrix::build_startup_repair_summary(
            &repair_id,
            &owner_key,
            "unavailable",
            0,
            1,
            false,
            stale_after_ms,
            &stale_cutoff,
            xcode_required,
            None,
            "startup",
        );
        let operator_message = if xcode_required {
            format!(
                "Xcode startup grace exceeded the 12 minute window; session generation {generation_id} has no provider session. Grace cutoff: {stale_cutoff}. Inspect Xcode broker/session startup."
            )
        } else {
            format!(
                "ACP startup grace exceeded; session generation {generation_id} has no provider session or activity. Grace cutoff: {stale_cutoff}."
            )
        };
        let readback = recovery_matrix::set_readback_startup_repair(
            recovery_matrix::build_readback_v1(
                "P082-R05",
                "held",
                "wait",
                recovery_matrix::REASON_STARTUP_STALLED,
                "Stale ACP startup detected; startup recovery owns any requeue decision.",
                "work_items, session_generations, session_events, startup_recovery_readbacks",
                "work_items, sessions, startup_repairs",
                &generation_id,
                None,
                "stale",
                &now.to_rfc3339(),
            ),
            summary,
            Some(&operator_message),
        );
        push_valid_projected_readback(&mut readbacks, readback);
    }

    // Source 9: running InvokeAgent work without a durable executor claim (R06).
    let stale_work_rows = sqlx::query(
        r#"SELECT id, created_at, started_at
           FROM work_items
           WHERE run_id = ?1
             AND kind = 'invoke_agent'
             AND status = 'running'
             AND json_extract(payload_json, '$.p058_claimed.agent_execution_id') IS NULL
           ORDER BY COALESCE(started_at, created_at) ASC
           LIMIT ?2"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROWS as i64)
    .fetch_all(pool)
    .await?;

    for row in stale_work_rows {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let work_item_id: String = row.try_get("id").unwrap_or_default();
        let created_at: String = row.try_get("created_at").unwrap_or_default();
        let started_at: Option<String> = row.try_get("started_at").unwrap_or(None);
        let evidence_at = started_at.as_deref().unwrap_or(&created_at);
        let Some(evidence_at_dt) = parse_utc_rfc3339(evidence_at) else {
            continue;
        };
        if now.signed_duration_since(evidence_at_dt) < Duration::minutes(3) {
            continue;
        }
        let readback = recovery_matrix::build_readback_v1(
            "P082-R06",
            "held",
            "wait",
            recovery_matrix::REASON_STALE_REPAIRED,
            "Running InvokeAgent work item has no durable executor owner; repair must use an explicit recorded transition.",
            "work_items, startup_repairs, side_effects",
            "work_items, startup_repairs, side_effects",
            &work_item_id,
            None,
            "stale",
            &now.to_rfc3339(),
        );
        push_valid_projected_readback(&mut readbacks, readback);
    }

    // Source 10: startup repair summaries that explicitly replayed an existing
    // idempotency key (R15 crash-resume proof).
    let replay_rows = sqlx::query(
        r#"SELECT id, notes, repaired_at
           FROM startup_repairs
           WHERE run_id = ?1
             AND notes IS NOT NULL
             AND LENGTH(notes) <= ?2
           ORDER BY repaired_at ASC
           LIMIT ?3"#,
    )
    .bind(run_id.to_string())
    .bind(MAX_READBACK_ROW_BYTES as i64)
    .bind(MAX_READBACK_ROWS as i64)
    .fetch_all(pool)
    .await?;

    for row in replay_rows {
        if readbacks.len() >= MAX_READBACK_ROWS {
            break;
        }
        let repair_id: String = row.try_get("id").unwrap_or_default();
        let notes_raw: Option<String> = row.try_get("notes").unwrap_or(None);
        let repaired_at: String = row.try_get("repaired_at").unwrap_or_default();
        let Some(notes_str) = notes_raw else {
            continue;
        };
        let Ok(notes_json) = serde_json::from_str::<serde_json::Value>(&notes_str) else {
            continue;
        };
        let Some(summary) = notes_json
            .get("p082_recovery_matrix_readback")
            .and_then(|rb| rb.get("recovery_startup_repair_summary"))
            .filter(|summary| {
                summary.get("replayed").and_then(|value| value.as_bool()) == Some(true)
            })
            .cloned()
        else {
            continue;
        };
        let readback = recovery_matrix::set_readback_startup_repair(
            recovery_matrix::build_readback_v1(
                "P082-R15",
                "repaired",
                "retry",
                recovery_matrix::REASON_REPAIR_CRASH_RESUME_IDEMPOTENT,
                "Crash-resume replay reused an existing repair idempotency key without duplicate mutation.",
                "startup_repairs, retry_payload_recovery_events, side_effects, runs, command_journal",
                "startup_repairs, work_items, command_journal",
                &repair_id,
                Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                "valid",
                &repaired_at,
            ),
            summary,
            None,
        );
        push_valid_projected_readback(&mut readbacks, readback);
    }

    // Sort: updated_at ASC, then scenario_id ASC (ties — last scenario_id becomes singular)
    readbacks.sort_by(|a, b| {
        let at_a = a.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
        let at_b = b.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
        let id_a = a.get("scenario_id").and_then(|v| v.as_str()).unwrap_or("");
        let id_b = b.get("scenario_id").and_then(|v| v.as_str()).unwrap_or("");
        at_a.cmp(at_b).then(id_a.cmp(id_b))
    });

    Ok(readbacks)
}

/// Latest-row selection for the singular `p082_recovery_matrix_readback` field on
/// `runs.get`. Returns the last non-`not_applicable` row after sorting by
/// `updated_at` ASC, `scenario_id` ASC. Returns `null` when no applicable row
/// exists.
pub async fn latest_readback_for_run(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> Result<serde_json::Value> {
    let readbacks = readbacks_for_run(pool, run_id).await?;
    let singular = readbacks
        .iter()
        .filter(|rb| rb.get("scenario_status").and_then(|v| v.as_str()) != Some("not_applicable"))
        .last()
        .cloned();
    Ok(singular.unwrap_or(serde_json::Value::Null))
}

/// Emits `p082_recovery_reason_readback_total{reason_code:lane}` for each readback row.
/// Callers pass the MCP lane name ("mcp", "reports.get", "report_resource",
/// "run_report", "release_receipt") so the metric carries the correct label.
pub fn emit_readback_lane_metrics(readbacks: &[serde_json::Value], lane: &str) {
    for row in readbacks {
        if let Some(reason_code) = row.get("recovery_reason_code").and_then(|v| v.as_str()) {
            crate::metrics::increment_counter_with_label(
                "p082_recovery_reason_readback_total",
                &format!("{reason_code}:{lane}"),
            );
        }
    }
}
