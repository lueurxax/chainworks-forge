//! P082 recovery/retry state-machine matrix: shared reason-code constants,
//! readback schema types, and envelope parsing.
//!
//! Reason codes are append-only. Do not rename or remove existing codes.

// ── Startup recovery timing constants ────────────────────────────────────────

/// Standard ACP startup grace before a provider-less startup row is stale.
pub const STANDARD_STARTUP_GRACE_SECONDS: i64 = 3 * 60;
/// Extended Xcode startup grace before a provider-less startup row is stale.
pub const XCODE_STARTUP_GRACE_SECONDS: i64 = 12 * 60;
/// Operator warning threshold for the Xcode startup grace lane.
pub const XCODE_STARTUP_GRACE_WARN_SECONDS: u64 = 12 * 60;
/// Operator critical threshold for the Xcode startup grace lane.
pub const XCODE_STARTUP_GRACE_CRITICAL_SECONDS: u64 = 15 * 60;

// ── Reason codes ─────────────────────────────────────────────────────────────

pub const REASON_RESUME_CLAIM_STATUS: &str = "resume_claim_status";
pub const REASON_STARTUP_REQUEUE_ONCE: &str = "startup_requeue_once";
pub const REASON_STARTUP_REQUEUE_EXHAUSTED: &str = "startup_requeue_exhausted";
pub const REASON_INVALID_STAGE_FOR_RETRY: &str = "invalid_stage_for_retry";
pub const REASON_IGNORED_LATE_OUTPUTS: &str = "ignored_late_outputs";
pub const REASON_DUPLICATE_OWNER_REPAIRED: &str = "duplicate_owner_repaired";
pub const REASON_STARTUP_STALLED: &str = "startup_stalled";
pub const REASON_STALE_REPAIRED: &str = "stale_repaired";
pub const REASON_NEEDS_EFFECT_RECONCILIATION: &str = "needs_effect_reconciliation";
pub const REASON_REQUIRES_EFFECT_RECONCILIATION: &str = "requires_effect_reconciliation";
pub const REASON_VALID_IDENTIFIER_GUIDANCE: &str = "valid_identifier_guidance";
pub const REASON_APPROVAL_PENDING_OPERATOR_ACTION_REQUIRED: &str =
    "approval_pending_operator_action_required";
pub const REASON_DUPLICATE_MEDIATION_OWNER_REJECTED: &str = "duplicate_mediation_owner_rejected";
pub const REASON_CANCEL_ACTIVE_STAGE_REQUESTED: &str = "cancel_active_stage_requested";
pub const REASON_CANCEL_PENDING_APPROVAL_PRESERVED: &str = "cancel_pending_approval_preserved";
pub const REASON_CANCEL_PENDING_APPROVAL_EXPIRED: &str = "cancel_pending_approval_expired";
pub const REASON_CANCEL_SIDE_EFFECT_RECONCILIATION_REQUIRED: &str =
    "cancel_side_effect_reconciliation_required";
pub const REASON_CANCEL_STARTUP_REPAIR_CONVERGED: &str = "cancel_startup_repair_converged";
pub const REASON_CANCELLED_PROVIDER_LATE_OUTPUT_IGNORED: &str =
    "cancelled_provider_late_output_ignored";
pub const REASON_REPAIR_CRASH_RESUME_IDEMPOTENT: &str = "repair_crash_resume_idempotent";

/// All canonical P082 reason codes. Append-only — removing a code is a breaking change.
pub const ALL_REASON_CODES: &[&str] = &[
    REASON_RESUME_CLAIM_STATUS,
    REASON_STARTUP_REQUEUE_ONCE,
    REASON_STARTUP_REQUEUE_EXHAUSTED,
    REASON_INVALID_STAGE_FOR_RETRY,
    REASON_IGNORED_LATE_OUTPUTS,
    REASON_DUPLICATE_OWNER_REPAIRED,
    REASON_STARTUP_STALLED,
    REASON_STALE_REPAIRED,
    REASON_NEEDS_EFFECT_RECONCILIATION,
    REASON_REQUIRES_EFFECT_RECONCILIATION,
    REASON_VALID_IDENTIFIER_GUIDANCE,
    REASON_APPROVAL_PENDING_OPERATOR_ACTION_REQUIRED,
    REASON_DUPLICATE_MEDIATION_OWNER_REJECTED,
    REASON_CANCEL_ACTIVE_STAGE_REQUESTED,
    REASON_CANCEL_PENDING_APPROVAL_PRESERVED,
    REASON_CANCEL_PENDING_APPROVAL_EXPIRED,
    REASON_CANCEL_SIDE_EFFECT_RECONCILIATION_REQUIRED,
    REASON_CANCEL_STARTUP_REPAIR_CONVERGED,
    REASON_CANCELLED_PROVIDER_LATE_OUTPUT_IGNORED,
    REASON_REPAIR_CRASH_RESUME_IDEMPOTENT,
];

// ── Approved enum vocabularies ────────────────────────────────────────────────

/// Valid `scenario_status` values per p082_recovery_matrix_readback_v1.
pub const VALID_SCENARIO_STATUSES: &[&str] = &[
    "repaired",
    "rejected",
    "held",
    "pending",
    "cancelled",
    "not_applicable",
];

/// Valid `recovery_decision` values per p082_recovery_matrix_readback_v1.
pub const VALID_RECOVERY_DECISIONS: &[&str] = &[
    "retry",
    "wait",
    "reconcile_side_effects",
    "operator_approval_required",
    "inspect_duplicate_owner",
    "cancel",
    "no_mutation",
];

/// Valid `recovery_projection_integrity` values per p082_recovery_matrix_readback_v1.
pub const VALID_PROJECTION_INTEGRITIES: &[&str] =
    &["valid", "stale", "tamper_detected", "unavailable"];

// ── Schema version constants ──────────────────────────────────────────────────

pub const SCHEMA_READBACK_V1: &str = "p082_recovery_matrix_readback_v1";
pub const SCHEMA_REJECTED_COMMAND_ERROR_V1: &str = "p082_rejected_command_error_v1";
pub const SCHEMA_RETRY_IDENTIFIER_GUIDANCE_V1: &str = "p082_retry_identifier_guidance_v1";
pub const SCHEMA_LATE_OUTPUT_SETTLEMENT_V1: &str = "p082_late_output_settlement_v1";
pub const SCHEMA_STARTUP_REPAIR_SUMMARY_V1: &str = "p082_startup_repair_summary_v1";

// ── Scenario IDs ─────────────────────────────────────────────────────────────

pub const SCENARIO_IDS: &[&str] = &[
    "P082-R01", "P082-R02", "P082-R03", "P082-R04", "P082-R05", "P082-R06", "P082-R07", "P082-R08",
    "P082-R09", "P082-R10", "P082-R11", "P082-R12", "P082-R13", "P082-R14", "P082-R15", "P082-R16",
    "P082-R17",
];

// ── Envelope parsing ──────────────────────────────────────────────────────────

/// Parse a `command_journal.error` value as a potential
/// `p082_rejected_command_error_v1` envelope.
///
/// Returns `Some(serde_json::Value)` only when the input is valid JSON,
/// has `schema_version = "p082_rejected_command_error_v1"`, all required
/// top-level fields are present, and the nested
/// `p082_recovery_matrix_readback` object validates against the full
/// readback schema contract.
///
/// Legacy plain-text errors return `None` safely without panicking.
pub fn parse_command_journal_error_envelope(error: &str) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(error).ok()?;
    let schema = v.get("schema_version")?.as_str()?;
    if schema != SCHEMA_REJECTED_COMMAND_ERROR_V1 {
        return None;
    }
    // Validate required top-level fields.
    let reason_code = v.get("reason_code")?.as_str()?;
    if !ALL_REASON_CODES.contains(&reason_code) {
        return None;
    }
    let _command_type = v.get("command_type")?.as_str()?;
    let _operator_safe_summary = v.get("operator_safe_summary")?.as_str()?;
    let _redaction = v.get("redaction")?.as_str()?;
    let readback_field = v.get("p082_recovery_matrix_readback")?;

    if readback_field.is_null() || !validate_readback_v1_shape(readback_field) {
        return None;
    }

    Some(v)
}

fn sanitize_p082_json(value: serde_json::Value) -> (serde_json::Value, bool) {
    match value {
        serde_json::Value::String(raw) => {
            let sanitized = crate::error_sanitizer::sanitize_error_for_storage(&raw, 2048);
            let changed = sanitized != raw;
            (serde_json::Value::String(sanitized), changed)
        }
        serde_json::Value::Array(items) => {
            let mut changed = false;
            let items = items
                .into_iter()
                .map(|item| {
                    let (sanitized, item_changed) = sanitize_p082_json(item);
                    changed |= item_changed;
                    sanitized
                })
                .collect();
            (serde_json::Value::Array(items), changed)
        }
        serde_json::Value::Object(map) => {
            let mut changed = false;
            let mut sanitized_map = serde_json::Map::new();
            for (key, value) in map {
                let (sanitized, value_changed) = sanitize_p082_json(value);
                changed |= value_changed;
                sanitized_map.insert(key, sanitized);
            }
            if changed
                && sanitized_map
                    .get("diagnostic_redaction")
                    .and_then(|value| value.as_str())
                    == Some("none")
            {
                sanitized_map.insert(
                    "diagnostic_redaction".to_string(),
                    serde_json::Value::String("partial".to_string()),
                );
            }
            (serde_json::Value::Object(sanitized_map), changed)
        }
        other => (other, false),
    }
}

/// Build a `p082_rejected_command_error_v1` JSON string suitable for writing
/// to `command_journal.error`.
///
/// `reason_code` must be one of the canonical P082 reason codes.
/// `operator_safe_summary` must not contain raw command payloads, auth material,
/// provider transcripts, or raw stderr.
pub fn build_rejected_command_error_envelope(
    reason_code: &str,
    command_type: &str,
    operator_safe_summary: &str,
    readback: serde_json::Value,
) -> String {
    let reason_code = if ALL_REASON_CODES.contains(&reason_code) {
        reason_code
    } else {
        REASON_RESUME_CLAIM_STATUS
    };
    let command_type = crate::error_sanitizer::sanitize_error_for_storage(command_type, 128);
    let operator_safe_summary =
        crate::error_sanitizer::sanitize_error_for_storage(operator_safe_summary, 2048);
    let (readback, _) = sanitize_p082_json(readback);
    let readback = if validate_readback_v1_shape(&readback) {
        readback
    } else {
        serde_json::Value::Null
    };
    serde_json::json!({
        "schema_version": SCHEMA_REJECTED_COMMAND_ERROR_V1,
        "reason_code": reason_code,
        "command_type": command_type,
        "redaction": "partial",
        "operator_safe_summary": operator_safe_summary,
        "p082_recovery_matrix_readback": readback,
    })
    .to_string()
}

/// Extend a `p082_recovery_matrix_readback_v1` object with non-null R07
/// side-effect hold fields.
///
/// Sets `recovery_side_effect_blocking_status` and `recovery_operator_message`
/// to the provided non-null values. Required for R07 held-state readbacks where
/// these fields must be non-null per the P082 contract (null is only valid when
/// the scenario is not side-effect-owned).
///
/// Returns the value unchanged if it is not a JSON object (e.g. a validation-error
/// sentinel returned by `build_readback_v1`).
pub fn set_readback_side_effect_hold(
    readback: serde_json::Value,
    blocking_status: &str,
    operator_message: &str,
) -> serde_json::Value {
    match readback {
        serde_json::Value::Object(mut m) => {
            // Do not modify validation-error sentinels returned by build_readback_v1.
            // These have a schema_version ending in "_VALIDATION_ERROR" and represent
            // programming errors, not real readback rows.
            let is_sentinel = m
                .get("schema_version")
                .and_then(|v| v.as_str())
                .map(|s| s.ends_with("_VALIDATION_ERROR"))
                .unwrap_or(false);
            if is_sentinel {
                return serde_json::Value::Object(m);
            }
            m.insert(
                "recovery_side_effect_blocking_status".into(),
                serde_json::Value::String(blocking_status.to_string()),
            );
            m.insert(
                "recovery_operator_message".into(),
                serde_json::Value::String(operator_message.to_string()),
            );
            serde_json::Value::Object(m)
        }
        other => other,
    }
}

/// Build a `p082_retry_identifier_guidance_v1` nested subcontract object.
///
/// `no_mutation` must always be `true` for R08 identifier mismatch rows — the
/// command was rejected before any mutation.  `valid_identifier_examples` should
/// list 1-3 valid identifiers of the expected kind for the caller's context, e.g.
/// actual stage_execution IDs from the run, or the current retry_authority_id.
pub fn build_retry_identifier_guidance(
    command: &str,
    provided_identifier: &str,
    provided_identifier_kind: &str,
    expected_identifier_kind: &str,
    valid_identifier_examples: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": SCHEMA_RETRY_IDENTIFIER_GUIDANCE_V1,
        "command": command,
        "provided_identifier": provided_identifier,
        "provided_identifier_kind": provided_identifier_kind,
        "expected_identifier_kind": expected_identifier_kind,
        "valid_identifier_examples": valid_identifier_examples,
        "no_mutation": true,
    })
}

/// Extend a `p082_recovery_matrix_readback_v1` object with a non-null
/// `recovery_retry_identifier_guidance` subcontract (R08).
///
/// Returns the value unchanged if it is not a JSON object or is a validation-error
/// sentinel.
pub fn set_readback_identifier_guidance(
    readback: serde_json::Value,
    guidance: serde_json::Value,
) -> serde_json::Value {
    match readback {
        serde_json::Value::Object(mut m) => {
            let is_sentinel = m
                .get("schema_version")
                .and_then(|v| v.as_str())
                .map(|s| s.ends_with("_VALIDATION_ERROR"))
                .unwrap_or(false);
            if is_sentinel {
                return serde_json::Value::Object(m);
            }
            m.insert("recovery_retry_identifier_guidance".into(), guidance);
            serde_json::Value::Object(m)
        }
        other => other,
    }
}

/// Build a `p082_late_output_settlement_v1` nested subcontract object.
///
/// `active_projection_changed` must be `false` — late output from superseded or
/// cancelled sources must not mutate the active projection.
pub fn build_late_output_settlement(
    source_agent_execution_id: &str,
    source_work_item_id: &str,
    source_session_generation_id: &str,
    active_session_generation_id: &str,
    claim_state: &str,
    output_settlement: &str,
    ignored_late_output_count: i64,
    source_work_item_terminal_status: &str,
    cancelled_provider_session: bool,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": SCHEMA_LATE_OUTPUT_SETTLEMENT_V1,
        "source_agent_execution_id": source_agent_execution_id,
        "source_work_item_id": source_work_item_id,
        "source_session_generation_id": source_session_generation_id,
        "active_session_generation_id": active_session_generation_id,
        "claim_state": claim_state,
        "output_settlement": output_settlement,
        "ignored_late_output_count": ignored_late_output_count,
        "source_work_item_terminal_status": source_work_item_terminal_status,
        "active_projection_changed": false,
        "cancelled_provider_session": cancelled_provider_session,
    })
}

/// Extend a `p082_recovery_matrix_readback_v1` object with a non-null
/// `recovery_late_output_settlement` subcontract (R03/R17).
///
/// Returns the value unchanged if it is not a JSON object or is a validation-error
/// sentinel.
pub fn set_readback_late_output_settlement(
    readback: serde_json::Value,
    settlement: serde_json::Value,
) -> serde_json::Value {
    match readback {
        serde_json::Value::Object(mut m) => {
            let is_sentinel = m
                .get("schema_version")
                .and_then(|v| v.as_str())
                .map(|s| s.ends_with("_VALIDATION_ERROR"))
                .unwrap_or(false);
            if is_sentinel {
                return serde_json::Value::Object(m);
            }
            m.insert("recovery_late_output_settlement".into(), settlement);
            serde_json::Value::Object(m)
        }
        other => other,
    }
}

/// Build a `p082_startup_repair_summary_v1` nested subcontract object.
///
/// `max_requeue_generation` must be 1 for P082 startup requeue proof.
/// `replayed` is true when this is a crash-resume path reusing an existing key.
pub fn build_startup_repair_summary(
    startup_repair_id: &str,
    source_work_item_id: &str,
    source_command_journal_id: &str,
    requeue_generation: i64,
    max_requeue_generation: i64,
    replayed: bool,
    stale_after_ms: i64,
    stale_cutoff: &str,
    xcode_required: bool,
    next_retry_or_backoff_time: Option<&str>,
    backpressure_scope: &str,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": SCHEMA_STARTUP_REPAIR_SUMMARY_V1,
        "startup_repair_id": startup_repair_id,
        "source_work_item_id": source_work_item_id,
        "source_command_journal_id": source_command_journal_id,
        "requeue_generation": requeue_generation,
        "max_requeue_generation": max_requeue_generation,
        "replayed": replayed,
        "stale_after_ms": stale_after_ms,
        "stale_cutoff": stale_cutoff,
        "xcode_required": xcode_required,
        "next_retry_or_backoff_time": next_retry_or_backoff_time,
        "backpressure_scope": backpressure_scope,
    })
}

/// Extend a `p082_recovery_matrix_readback_v1` object with a non-null
/// `recovery_startup_repair_summary` and optional `recovery_operator_message`
/// (R01/R05/R14/R16).
///
/// Returns the value unchanged if it is not a JSON object or is a validation-error
/// sentinel.
pub fn set_readback_startup_repair(
    readback: serde_json::Value,
    summary: serde_json::Value,
    operator_message: Option<&str>,
) -> serde_json::Value {
    match readback {
        serde_json::Value::Object(mut m) => {
            let is_sentinel = m
                .get("schema_version")
                .and_then(|v| v.as_str())
                .map(|s| s.ends_with("_VALIDATION_ERROR"))
                .unwrap_or(false);
            if is_sentinel {
                return serde_json::Value::Object(m);
            }
            m.insert("recovery_startup_repair_summary".into(), summary);
            if let Some(msg) = operator_message {
                m.insert(
                    "recovery_operator_message".into(),
                    serde_json::Value::String(msg.to_string()),
                );
            }
            serde_json::Value::Object(m)
        }
        other => other,
    }
}

/// Build a minimal `p082_recovery_matrix_readback_v1` object.
///
/// Returns a validation-error sentinel object (with `schema_version =
/// "p082_recovery_matrix_readback_v1_VALIDATION_ERROR"`) when:
/// - `scenario_status`, `recovery_decision`, or `recovery_projection_integrity`
///   are outside the approved vocabulary, or
/// - `recovery_next_action` is empty for a non-`not_applicable` status.
///
/// Validation is enforced in both debug and release builds so malformed rows
/// are rejected fail-closed rather than persisted. Optional subcontract fields
/// default to JSON null; arrays default to [].
pub fn build_readback_v1(
    scenario_id: &str,
    scenario_status: &str,
    recovery_decision: &str,
    recovery_reason_code: &str,
    recovery_next_action: &str,
    source_table: &str,
    source_repository: &str,
    source_identifier: &str,
    source_json_key: Option<&str>,
    recovery_projection_integrity: &str,
    updated_at: &str,
) -> serde_json::Value {
    if !VALID_SCENARIO_STATUSES.contains(&scenario_status) {
        return serde_json::json!({
            "schema_version": "p082_recovery_matrix_readback_v1_VALIDATION_ERROR",
            "error": format!("invalid scenario_status '{scenario_status}'"),
        });
    }
    if !VALID_RECOVERY_DECISIONS.contains(&recovery_decision) {
        return serde_json::json!({
            "schema_version": "p082_recovery_matrix_readback_v1_VALIDATION_ERROR",
            "error": format!("invalid recovery_decision '{recovery_decision}'"),
        });
    }
    if !VALID_PROJECTION_INTEGRITIES.contains(&recovery_projection_integrity) {
        return serde_json::json!({
            "schema_version": "p082_recovery_matrix_readback_v1_VALIDATION_ERROR",
            "error": format!("invalid recovery_projection_integrity '{recovery_projection_integrity}'"),
        });
    }
    if scenario_status != "not_applicable" && recovery_next_action.is_empty() {
        return serde_json::json!({
            "schema_version": "p082_recovery_matrix_readback_v1_VALIDATION_ERROR",
            "error": format!("recovery_next_action must be non-empty for scenario_status '{scenario_status}'"),
        });
    }
    if !ALL_REASON_CODES.contains(&recovery_reason_code) {
        return serde_json::json!({
            "schema_version": "p082_recovery_matrix_readback_v1_VALIDATION_ERROR",
            "error": format!("unknown reason_code '{recovery_reason_code}'"),
        });
    }
    serde_json::json!({
        "schema_version": SCHEMA_READBACK_V1,
        "scenario_id": scenario_id,
        "scenario_status": scenario_status,
        "recovery_decision": recovery_decision,
        "recovery_reason_code": recovery_reason_code,
        "recovery_next_action": recovery_next_action,
        "recovery_hold_conditions": [],
        "recovery_side_effect_blocking_status": null,
        "recovery_retry_identifier_guidance": null,
        "recovery_late_output_settlement": null,
        "recovery_startup_repair_summary": null,
        "recovery_operator_message": null,
        "recovery_projection_integrity": recovery_projection_integrity,
        "source_table": source_table,
        "source_repository": source_repository,
        "source_identifier": source_identifier,
        "source_json_key": source_json_key,
        "updated_at": updated_at,
        "diagnostic_redaction": "none",
    })
}

const VALID_DIAGNOSTIC_REDACTIONS: &[&str] = &["none", "partial", "full"];
const SOURCE_KEY_COMMAND_JOURNAL_ERROR: &str =
    "command_journal.error.p082_recovery_matrix_readback";
const SOURCE_KEY_STARTUP_REPAIRS_NOTES: &str =
    "startup_repairs.notes.p082_recovery_matrix_readback";
const SOURCE_KEY_WORK_ITEMS_STARTUP_RECOVERY: &str =
    "work_items.payload_json.p061_startup_recovery";
const SOURCE_KEY_STAGE_EXECUTIONS_RECOVERY_SNAPSHOT: &str =
    "stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback";
const SOURCE_KEY_RUNS_CANCELLATION_SETTLEMENT_LOG: &str =
    "runs.cancellation_settlement_log.p082_recovery_matrix_readback";
const SOURCE_KEY_RETRY_PAYLOAD_RECOVERY_EVENTS_DIAGNOSTIC: &str =
    "retry_payload_recovery_events.diagnostic_json.p082_recovery_matrix_readback";
const SOURCE_KEY_SESSION_EVENTS_DETAILS: &str =
    "session_events.details_json.p082_recovery_matrix_readback";
const SOURCE_KEY_LEAD_CONFLICT_MEDIATIONS_VALIDATION_ERRORS: &str =
    "lead_conflict_mediations.validation_errors_json.p082_recovery_matrix_readback";
const SOURCE_KEY_WORKFLOW_CONFLICTS_RECORD: &str =
    "workflow_conflicts.record_json.p082_recovery_matrix_readback";
const APPROVED_JSON_SOURCE_KEYS: &[&str] = &[
    SOURCE_KEY_COMMAND_JOURNAL_ERROR,
    SOURCE_KEY_STARTUP_REPAIRS_NOTES,
    SOURCE_KEY_WORK_ITEMS_STARTUP_RECOVERY,
    SOURCE_KEY_STAGE_EXECUTIONS_RECOVERY_SNAPSHOT,
    SOURCE_KEY_RUNS_CANCELLATION_SETTLEMENT_LOG,
    SOURCE_KEY_RETRY_PAYLOAD_RECOVERY_EVENTS_DIAGNOSTIC,
    SOURCE_KEY_SESSION_EVENTS_DETAILS,
    SOURCE_KEY_LEAD_CONFLICT_MEDIATIONS_VALIDATION_ERRORS,
    SOURCE_KEY_WORKFLOW_CONFLICTS_RECORD,
];
const VALID_IDENTIFIER_KINDS: &[&str] = &[
    "workflow_stage_id",
    "stage_execution_uuid",
    "retry_authority_id",
    "work_item_id",
];
const VALID_PROVIDED_IDENTIFIER_KINDS: &[&str] = &[
    "workflow_stage_id",
    "stage_execution_uuid",
    "retry_authority_id",
    "work_item_id",
    "unknown",
];
const VALID_LATE_OUTPUT_CLAIM_STATES: &[&str] = &["superseded", "closed", "ignored"];
const VALID_LATE_OUTPUT_SETTLEMENTS: &[&str] = &["ignored", "quarantined"];
const VALID_LATE_OUTPUT_TERMINAL_STATUSES: &[&str] = &["completed", "failed"];
const REQUIRED_READBACK_V1_FIELDS: &[&str] = &[
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

fn object_string<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a str> {
    obj.get(key).and_then(|value| value.as_str())
}

fn object_required_string(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    object_string(obj, key).is_some_and(|value| !value.is_empty())
}

fn object_string_or_null(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    match obj.get(key) {
        Some(value) => value.is_null() || value.as_str().is_some(),
        None => false,
    }
}

fn object_required_i64(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    obj.get(key).and_then(|value| value.as_i64()).is_some()
}

fn object_required_bool(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    obj.get(key).and_then(|value| value.as_bool()).is_some()
}

fn object_string_array(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    match obj.get(key).and_then(|value| value.as_array()) {
        Some(items) => items.iter().all(|item| item.as_str().is_some()),
        None => false,
    }
}

fn scenario_requires_retry_identifier_guidance(scenario_id: &str) -> bool {
    matches!(scenario_id, "P082-R08")
}

fn scenario_requires_late_output_settlement(scenario_id: &str) -> bool {
    matches!(scenario_id, "P082-R03" | "P082-R17")
}

fn scenario_requires_startup_repair_summary(scenario_id: &str) -> bool {
    matches!(
        scenario_id,
        "P082-R01" | "P082-R05" | "P082-R14" | "P082-R15" | "P082-R16"
    )
}

fn scenario_allows_side_effect_blocking_status(scenario_id: &str) -> bool {
    matches!(scenario_id, "P082-R07" | "P082-R13")
}

fn allowed_json_source_keys(
    scenario_id: &str,
    reason_code: &str,
    integrity: &str,
    source_table: &str,
) -> Option<&'static [&'static str]> {
    match scenario_id {
        "P082-R01" | "P082-R15" | "P082-R16" => Some(&[SOURCE_KEY_STARTUP_REPAIRS_NOTES]),
        "P082-R02" => {
            let legacy_plain_text_fallback =
                reason_code == REASON_RESUME_CLAIM_STATUS && integrity == "unavailable";
            if legacy_plain_text_fallback {
                None
            } else {
                Some(&[SOURCE_KEY_COMMAND_JOURNAL_ERROR])
            }
        }
        "P082-R03" | "P082-R17" => Some(&[
            SOURCE_KEY_STAGE_EXECUTIONS_RECOVERY_SNAPSHOT,
            SOURCE_KEY_SESSION_EVENTS_DETAILS,
        ]),
        "P082-R04" => Some(&[SOURCE_KEY_SESSION_EVENTS_DETAILS]),
        "P082-R05" => Some(&[SOURCE_KEY_WORK_ITEMS_STARTUP_RECOVERY]),
        "P082-R06" => Some(&[
            SOURCE_KEY_WORK_ITEMS_STARTUP_RECOVERY,
            SOURCE_KEY_STARTUP_REPAIRS_NOTES,
        ]),
        "P082-R07" | "P082-R08" => Some(&[SOURCE_KEY_COMMAND_JOURNAL_ERROR]),
        "P082-R09" => {
            if source_table == "stage_executions" {
                Some(&[SOURCE_KEY_STAGE_EXECUTIONS_RECOVERY_SNAPSHOT])
            } else {
                Some(&[SOURCE_KEY_STAGE_EXECUTIONS_RECOVERY_SNAPSHOT])
            }
        }
        "P082-R10" => Some(&[SOURCE_KEY_LEAD_CONFLICT_MEDIATIONS_VALIDATION_ERRORS]),
        "P082-R11" | "P082-R12" | "P082-R13" | "P082-R14" => {
            Some(&[SOURCE_KEY_RUNS_CANCELLATION_SETTLEMENT_LOG])
        }
        _ => None,
    }
}

fn validate_source_json_key(
    scenario_id: &str,
    reason_code: &str,
    integrity: &str,
    source_table: &str,
    value: &serde_json::Value,
) -> bool {
    let actual = match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(value) => Some(value.as_str()),
        _ => return false,
    };
    if let Some(actual) = actual {
        if !APPROVED_JSON_SOURCE_KEYS.contains(&actual) {
            return false;
        }
    }
    match allowed_json_source_keys(scenario_id, reason_code, integrity, source_table) {
        Some(expected) => actual
            .map(|actual| expected.contains(&actual))
            .unwrap_or(false),
        None => actual.is_none(),
    }
}

fn validate_iso8601_timestamp(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value).is_ok()
}

fn validate_retry_identifier_guidance(value: &serde_json::Value) -> bool {
    if value.is_null() {
        return true;
    }
    let Some(obj) = value.as_object() else {
        return false;
    };
    if object_string(obj, "schema_version") != Some(SCHEMA_RETRY_IDENTIFIER_GUIDANCE_V1) {
        return false;
    }
    for field in ["command", "provided_identifier"] {
        if !object_required_string(obj, field) {
            return false;
        }
    }
    let Some(provided_kind) = object_string(obj, "provided_identifier_kind") else {
        return false;
    };
    if !VALID_PROVIDED_IDENTIFIER_KINDS.contains(&provided_kind) {
        return false;
    }
    let Some(expected_kind) = object_string(obj, "expected_identifier_kind") else {
        return false;
    };
    if !VALID_IDENTIFIER_KINDS.contains(&expected_kind) {
        return false;
    }
    if !object_string_array(obj, "valid_identifier_examples") {
        return false;
    }
    if obj
        .get("valid_identifier_examples")
        .and_then(|value| value.as_array())
        .map(|examples| examples.is_empty())
        != Some(false)
    {
        return false;
    }
    obj.get("no_mutation").and_then(|value| value.as_bool()) == Some(true)
}

fn validate_late_output_settlement(value: &serde_json::Value) -> bool {
    if value.is_null() {
        return true;
    }
    let Some(obj) = value.as_object() else {
        return false;
    };
    if object_string(obj, "schema_version") != Some(SCHEMA_LATE_OUTPUT_SETTLEMENT_V1) {
        return false;
    }
    for field in [
        "source_agent_execution_id",
        "source_work_item_id",
        "source_session_generation_id",
        "active_session_generation_id",
    ] {
        if !object_required_string(obj, field) {
            return false;
        }
    }
    let Some(claim_state) = object_string(obj, "claim_state") else {
        return false;
    };
    if !VALID_LATE_OUTPUT_CLAIM_STATES.contains(&claim_state) {
        return false;
    }
    let Some(output_settlement) = object_string(obj, "output_settlement") else {
        return false;
    };
    if !VALID_LATE_OUTPUT_SETTLEMENTS.contains(&output_settlement) {
        return false;
    }
    if !object_required_i64(obj, "ignored_late_output_count") {
        return false;
    }
    let Some(terminal_status) = object_string(obj, "source_work_item_terminal_status") else {
        return false;
    };
    if !VALID_LATE_OUTPUT_TERMINAL_STATUSES.contains(&terminal_status) {
        return false;
    }
    if obj
        .get("active_projection_changed")
        .and_then(|value| value.as_bool())
        != Some(false)
    {
        return false;
    }
    object_required_bool(obj, "cancelled_provider_session")
}

fn validate_startup_repair_summary(value: &serde_json::Value) -> bool {
    if value.is_null() {
        return true;
    }
    let Some(obj) = value.as_object() else {
        return false;
    };
    if object_string(obj, "schema_version") != Some(SCHEMA_STARTUP_REPAIR_SUMMARY_V1) {
        return false;
    }
    for field in [
        "startup_repair_id",
        "source_work_item_id",
        "source_command_journal_id",
        "stale_cutoff",
        "backpressure_scope",
    ] {
        if !object_required_string(obj, field) {
            return false;
        }
    }
    for field in [
        "requeue_generation",
        "max_requeue_generation",
        "stale_after_ms",
    ] {
        if !object_required_i64(obj, field) {
            return false;
        }
    }
    if obj
        .get("max_requeue_generation")
        .and_then(|value| value.as_i64())
        != Some(1)
    {
        return false;
    }
    if !object_required_bool(obj, "replayed") || !object_required_bool(obj, "xcode_required") {
        return false;
    }
    object_string_or_null(obj, "next_retry_or_backoff_time")
}

/// Validate all required fields of a `p082_recovery_matrix_readback_v1` object.
///
/// Returns `true` only when every required field is present with an accepted value.
/// Used by the DB accessor to reject tampered or incomplete rows before exposing them
/// through MCP/report/release readback lanes (SEC-P082-001 / MEDIUM-1 fix).
pub fn validate_readback_v1_shape(rb: &serde_json::Value) -> bool {
    let Some(obj) = rb.as_object() else {
        return false;
    };
    if !REQUIRED_READBACK_V1_FIELDS
        .iter()
        .all(|field| obj.contains_key(*field))
    {
        return false;
    }
    if obj.get("schema_version").and_then(|v| v.as_str()) != Some(SCHEMA_READBACK_V1) {
        return false;
    }
    let Some(scenario_id) = obj.get("scenario_id").and_then(|v| v.as_str()) else {
        return false;
    };
    if !SCENARIO_IDS.contains(&scenario_id) {
        return false;
    }
    let Some(status) = obj.get("scenario_status").and_then(|v| v.as_str()) else {
        return false;
    };
    if !VALID_SCENARIO_STATUSES.contains(&status) {
        return false;
    }
    let Some(decision) = obj.get("recovery_decision").and_then(|v| v.as_str()) else {
        return false;
    };
    if !VALID_RECOVERY_DECISIONS.contains(&decision) {
        return false;
    }
    let Some(reason_code) = obj.get("recovery_reason_code").and_then(|v| v.as_str()) else {
        return false;
    };
    if !ALL_REASON_CODES.contains(&reason_code) {
        return false;
    }
    // recovery_next_action must be non-empty for non-not_applicable statuses.
    if status != "not_applicable" {
        match obj.get("recovery_next_action").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => {}
            _ => return false,
        }
    } else if obj
        .get("recovery_next_action")
        .and_then(|v| v.as_str())
        .is_none()
    {
        return false;
    }
    if !object_string_array(obj, "recovery_hold_conditions") {
        return false;
    }
    if !object_string_or_null(obj, "recovery_side_effect_blocking_status") {
        return false;
    }
    let side_effect_blocking_status = obj
        .get("recovery_side_effect_blocking_status")
        .unwrap_or(&serde_json::Value::Null);
    if !scenario_allows_side_effect_blocking_status(scenario_id)
        && !side_effect_blocking_status.is_null()
    {
        return false;
    }
    if scenario_allows_side_effect_blocking_status(scenario_id) && status == "held" {
        match side_effect_blocking_status.as_str() {
            Some(value) if !value.is_empty() => {}
            _ => return false,
        }
    }
    let Some(retry_identifier_guidance) = obj.get("recovery_retry_identifier_guidance") else {
        return false;
    };
    if scenario_requires_retry_identifier_guidance(scenario_id)
        && retry_identifier_guidance.is_null()
    {
        return false;
    }
    if !scenario_requires_retry_identifier_guidance(scenario_id)
        && !retry_identifier_guidance.is_null()
    {
        return false;
    }
    if !validate_retry_identifier_guidance(retry_identifier_guidance) {
        return false;
    }
    let Some(late_output_settlement) = obj.get("recovery_late_output_settlement") else {
        return false;
    };
    if scenario_requires_late_output_settlement(scenario_id) && late_output_settlement.is_null() {
        return false;
    }
    if !scenario_requires_late_output_settlement(scenario_id) && !late_output_settlement.is_null() {
        return false;
    }
    if !validate_late_output_settlement(late_output_settlement) {
        return false;
    }
    let Some(startup_repair_summary) = obj.get("recovery_startup_repair_summary") else {
        return false;
    };
    if scenario_requires_startup_repair_summary(scenario_id) && startup_repair_summary.is_null() {
        return false;
    }
    if !scenario_requires_startup_repair_summary(scenario_id) && !startup_repair_summary.is_null() {
        return false;
    }
    if !validate_startup_repair_summary(startup_repair_summary) {
        return false;
    }
    if !object_string_or_null(obj, "recovery_operator_message") {
        return false;
    }
    let xcode_startup_grace = scenario_id == "P082-R05"
        && startup_repair_summary
            .as_object()
            .and_then(|summary| summary.get("xcode_required"))
            .and_then(|value| value.as_bool())
            == Some(true);
    if ((status == "held"
        && matches!(
            scenario_id,
            "P082-R05" | "P082-R07" | "P082-R13" | "P082-R16"
        ))
        || xcode_startup_grace)
        && object_string(obj, "recovery_operator_message")
            .map(str::is_empty)
            .unwrap_or(true)
    {
        return false;
    }
    let Some(integrity) = obj
        .get("recovery_projection_integrity")
        .and_then(|v| v.as_str())
    else {
        return false;
    };
    if !VALID_PROJECTION_INTEGRITIES.contains(&integrity) {
        return false;
    }
    let Some(source_table) = obj.get("source_table").and_then(|v| v.as_str()) else {
        return false;
    };
    if obj
        .get("source_repository")
        .and_then(|v| v.as_str())
        .is_none()
    {
        return false;
    }
    if obj
        .get("source_identifier")
        .and_then(|v| v.as_str())
        .is_none()
    {
        return false;
    }
    let Some(updated_at) = obj.get("updated_at").and_then(|v| v.as_str()) else {
        return false;
    };
    if !validate_iso8601_timestamp(updated_at) {
        return false;
    }
    let Some(source_json_key) = obj.get("source_json_key") else {
        return false;
    };
    if !validate_source_json_key(
        scenario_id,
        reason_code,
        integrity,
        source_table,
        source_json_key,
    ) {
        return false;
    }
    match obj.get("diagnostic_redaction").and_then(|v| v.as_str()) {
        Some(s) if VALID_DIAGNOSTIC_REDACTIONS.contains(&s) => {}
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn startup_summary() -> serde_json::Value {
        build_startup_repair_summary(
            "p082-requeue:cj-001:wi-001:1",
            "wi-001",
            "cj-001",
            1,
            1,
            false,
            60_000,
            "2026-05-21T00:00:00Z",
            false,
            None,
            "global",
        )
    }

    fn late_output_settlement(cancelled_provider_session: bool) -> serde_json::Value {
        build_late_output_settlement(
            "ae-001",
            "wi-001",
            "sg-old",
            "sg-active",
            if cancelled_provider_session {
                "closed"
            } else {
                "superseded"
            },
            "ignored",
            1,
            "completed",
            cancelled_provider_session,
        )
    }

    fn identifier_guidance() -> serde_json::Value {
        build_retry_identifier_guidance(
            "RetryAgentExecution",
            "stage-abc-wrong-kind",
            "stage_execution_uuid",
            "stage_execution_uuid",
            &["stage-exec-001"],
        )
    }

    #[test]
    fn all_reason_codes_present_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for code in ALL_REASON_CODES {
            assert!(seen.insert(*code), "duplicate reason code: {code}");
        }
        // Minimum required count
        assert!(
            ALL_REASON_CODES.len() >= 19,
            "expected at least 19 reason codes"
        );
    }

    #[test]
    fn all_scenario_ids_present_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for id in SCENARIO_IDS {
            assert!(seen.insert(*id), "duplicate scenario id: {id}");
        }
        assert_eq!(SCENARIO_IDS.len(), 17, "expected exactly 17 scenario IDs");
    }

    #[test]
    fn parse_valid_envelope() {
        let envelope = build_rejected_command_error_envelope(
            REASON_INVALID_STAGE_FOR_RETRY,
            "RetryStage",
            "Stage is not in a retryable status.",
            serde_json::json!({
                "schema_version": SCHEMA_READBACK_V1,
                "scenario_id": "P082-R02",
                "scenario_status": "rejected",
                "recovery_decision": "no_mutation",
                "recovery_reason_code": REASON_INVALID_STAGE_FOR_RETRY,
                "recovery_next_action": "No mutation was performed. Inspect the stage status and retry when eligible.",
                "recovery_hold_conditions": [],
                "recovery_side_effect_blocking_status": null,
                "recovery_retry_identifier_guidance": null,
                "recovery_late_output_settlement": null,
                "recovery_startup_repair_summary": null,
                "recovery_operator_message": null,
                "recovery_projection_integrity": "valid",
                "source_table": "command_journal",
                "source_repository": "command_journal",
                "source_identifier": "cmd-001",
                "source_json_key": "command_journal.error.p082_recovery_matrix_readback",
                "updated_at": "2026-05-21T00:00:00Z",
                "diagnostic_redaction": "none",
            }),
        );
        let parsed = parse_command_journal_error_envelope(&envelope);
        assert!(parsed.is_some(), "valid envelope must parse successfully");
        let v = parsed.unwrap();
        assert_eq!(
            v["reason_code"].as_str().unwrap(),
            REASON_INVALID_STAGE_FOR_RETRY
        );
    }

    #[test]
    fn build_rejected_command_error_envelope_sanitizes_summary_and_readback_strings() {
        let readback = build_readback_v1(
            "P082-R02",
            "rejected",
            "no_mutation",
            REASON_INVALID_STAGE_FOR_RETRY,
            "No mutation was performed; api_key: abc access_token xyz sk-test must not leak.",
            "command_journal",
            "command_journal",
            "cmd-002",
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        );
        let envelope = build_rejected_command_error_envelope(
            REASON_INVALID_STAGE_FOR_RETRY,
            "RetryStage",
            "Rejected retry with password: hunter2 secret abc123 authorization: Bearer auth123",
            readback,
        );
        for leaked in ["sk-test", "abc", "xyz", "hunter2", "abc123", "auth123"] {
            assert!(
                !envelope.contains(leaked),
                "P082 rejected-command envelope leaked {leaked}: {envelope}"
            );
        }
        let parsed = parse_command_journal_error_envelope(&envelope)
            .expect("sanitized envelope should still parse");
        assert_eq!(
            parsed["p082_recovery_matrix_readback"]["diagnostic_redaction"],
            serde_json::json!("partial")
        );
    }

    #[test]
    fn parse_legacy_plain_text_error_is_safe() {
        let plain = "stage is not retryable: status=completed";
        assert!(
            parse_command_journal_error_envelope(plain).is_none(),
            "plain-text error must return None without panicking"
        );
    }

    #[test]
    fn parse_malformed_json_envelope_is_safe() {
        let bad = r#"{"schema_version":"p082_rejected_command_error_v1"}"#;
        assert!(
            parse_command_journal_error_envelope(bad).is_none(),
            "envelope missing required fields must return None"
        );
    }

    #[test]
    fn build_readback_v1_has_all_required_fields() {
        let rb = build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs, work_items",
            "startup-repair-001",
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        );
        assert_eq!(rb["schema_version"], SCHEMA_READBACK_V1);
        assert_eq!(rb["scenario_id"], "P082-R01");
        assert_eq!(rb["recovery_reason_code"], REASON_STARTUP_REQUEUE_ONCE);
    }

    #[test]
    fn validate_readback_v1_shape_rejects_empty_startup_source_command_journal_id() {
        let summary = build_startup_repair_summary(
            "p082-requeue:cj-001:wi-001:1",
            "wi-001",
            "",
            1,
            1,
            false,
            60_000,
            "2026-05-21T00:00:00Z",
            false,
            None,
            "global",
        );
        let rb = set_readback_startup_repair(
            build_readback_v1(
                "P082-R01",
                "repaired",
                "retry",
                REASON_STARTUP_REQUEUE_ONCE,
                "Startup requeue scheduled.",
                "startup_repairs",
                "startup_repairs, work_items, command_journal",
                "startup-repair-001",
                Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            ),
            summary,
            None,
        );

        assert!(
            !validate_readback_v1_shape(&rb),
            "P082 startup summary must reject empty source_command_journal_id"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_omitted_optional_subcontract_fields() {
        for omitted_key in [
            "recovery_retry_identifier_guidance",
            "recovery_late_output_settlement",
            "recovery_startup_repair_summary",
            "source_json_key",
        ] {
            let mut rb = build_readback_v1(
                "P082-R02",
                "rejected",
                "no_mutation",
                REASON_INVALID_STAGE_FOR_RETRY,
                "Rejected command did not mutate state.",
                "command_journal",
                "command_journal",
                "cmd-001",
                Some("command_journal.error.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            );
            rb.as_object_mut().unwrap().remove(omitted_key);
            assert!(
                !validate_readback_v1_shape(&rb),
                "p082_recovery_matrix_readback_v1 must reject omitted top-level field {omitted_key}"
            );
        }
    }

    #[test]
    fn validate_readback_v1_shape_requires_xcode_r05_message_for_repaired_rows() {
        let summary = build_startup_repair_summary(
            "p082-requeue:cj-r05:wi-r05:1",
            "wi-r05",
            "cj-r05",
            1,
            1,
            false,
            720_000,
            "2026-05-21T00:12:00Z",
            true,
            None,
            "startup",
        );
        let rb = set_readback_startup_repair(
            build_readback_v1(
                "P082-R05",
                "repaired",
                "retry",
                REASON_STARTUP_STALLED,
                "Xcode startup stale repair converged.",
                "work_items",
                "work_items, sessions, startup_repairs",
                "wi-r05",
                Some("work_items.payload_json.p061_startup_recovery"),
                "valid",
                "2026-05-21T00:13:00Z",
            ),
            summary,
            None,
        );

        assert!(
            !validate_readback_v1_shape(&rb),
            "P082-R05 xcode_required=true rows must include a non-empty recovery_operator_message even after repair"
        );
    }

    #[test]
    fn valid_scenario_statuses_are_correct() {
        let expected = [
            "repaired",
            "rejected",
            "held",
            "pending",
            "cancelled",
            "not_applicable",
        ];
        for s in &expected {
            assert!(
                VALID_SCENARIO_STATUSES.contains(s),
                "VALID_SCENARIO_STATUSES missing '{s}'"
            );
        }
        // Approved contract never uses these legacy values
        for bad in &["resolved", "active"] {
            assert!(
                !VALID_SCENARIO_STATUSES.contains(bad),
                "VALID_SCENARIO_STATUSES must not contain legacy value '{bad}'"
            );
        }
    }

    #[test]
    fn valid_recovery_decisions_are_correct() {
        let expected = [
            "retry",
            "wait",
            "reconcile_side_effects",
            "operator_approval_required",
            "inspect_duplicate_owner",
            "cancel",
            "no_mutation",
        ];
        for d in &expected {
            assert!(
                VALID_RECOVERY_DECISIONS.contains(d),
                "VALID_RECOVERY_DECISIONS missing '{d}'"
            );
        }
        // Approved contract never uses these legacy values
        for bad in &["repair_converged", "held", "cancelled"] {
            assert!(
                !VALID_RECOVERY_DECISIONS.contains(bad),
                "VALID_RECOVERY_DECISIONS must not contain legacy value '{bad}'"
            );
        }
    }

    #[test]
    fn valid_projection_integrities_include_tamper_detected() {
        assert!(
            VALID_PROJECTION_INTEGRITIES.contains(&"tamper_detected"),
            "VALID_PROJECTION_INTEGRITIES must include tamper_detected"
        );
        for v in &["valid", "stale", "unavailable"] {
            assert!(
                VALID_PROJECTION_INTEGRITIES.contains(v),
                "VALID_PROJECTION_INTEGRITIES missing '{v}'"
            );
        }
    }

    #[test]
    fn parse_envelope_with_invalid_nested_status_returns_none() {
        // Build an envelope where the nested readback has a legacy `resolved` status.
        let invalid_readback = serde_json::json!({
            "schema_version": SCHEMA_READBACK_V1,
            "scenario_id": "P082-R01",
            "scenario_status": "resolved",  // invalid
            "recovery_decision": "retry",
            "recovery_reason_code": REASON_STARTUP_REQUEUE_ONCE,
            "recovery_next_action": "Startup requeue scheduled.",
            "recovery_hold_conditions": [],
            "recovery_side_effect_blocking_status": null,
            "recovery_retry_identifier_guidance": null,
            "recovery_late_output_settlement": null,
            "recovery_startup_repair_summary": null,
            "recovery_operator_message": null,
            "recovery_projection_integrity": "valid",
            "source_table": "startup_repairs",
            "source_repository": "startup_repairs",
            "source_identifier": "sr-001",
            "source_json_key": null,
            "updated_at": "2026-05-21T00:00:00Z",
            "diagnostic_redaction": "none",
        });
        let envelope = serde_json::json!({
            "schema_version": SCHEMA_REJECTED_COMMAND_ERROR_V1,
            "reason_code": REASON_STARTUP_REQUEUE_ONCE,
            "command_type": "StartupRepair",
            "redaction": "none",
            "operator_safe_summary": "Startup repair requeue.",
            "p082_recovery_matrix_readback": invalid_readback,
        })
        .to_string();
        let parsed = parse_command_journal_error_envelope(&envelope);
        assert!(
            parsed.is_none(),
            "envelope with nested readback using legacy scenario_status must return None"
        );
    }

    #[test]
    fn set_readback_side_effect_hold_sets_non_null_r07_fields() {
        let base = build_readback_v1(
            "P082-R07",
            "held",
            "reconcile_side_effects",
            REASON_REQUIRES_EFFECT_RECONCILIATION,
            "Reconcile unresolved side effects before retrying.",
            "side_effects, command_journal",
            "side_effects, command_journal",
            "cmd-r07-1",
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        );
        // Before patching, these are null by default.
        assert!(base["recovery_side_effect_blocking_status"].is_null());
        assert!(base["recovery_operator_message"].is_null());

        let patched = set_readback_side_effect_hold(
            base,
            "unresolved_side_effect_entries",
            "Retry blocked: unresolved side-effect ledger entries exist.",
        );
        assert_eq!(
            patched["recovery_side_effect_blocking_status"]
                .as_str()
                .unwrap(),
            "unresolved_side_effect_entries",
            "P082-R07: recovery_side_effect_blocking_status must be non-null for held state"
        );
        assert_eq!(
            patched["recovery_operator_message"].as_str().unwrap(),
            "Retry blocked: unresolved side-effect ledger entries exist.",
            "P082-R07: recovery_operator_message must be non-null for held state"
        );
        // Other fields must be preserved.
        assert_eq!(patched["scenario_id"].as_str().unwrap(), "P082-R07");
        assert_eq!(patched["scenario_status"].as_str().unwrap(), "held");
        assert_eq!(
            patched["schema_version"].as_str().unwrap(),
            SCHEMA_READBACK_V1
        );
    }

    #[test]
    fn set_readback_side_effect_hold_passes_through_sentinel() {
        // Validation-error sentinels (non-object) must be passed through unchanged.
        let sentinel = serde_json::json!({
            "schema_version": "p082_recovery_matrix_readback_v1_VALIDATION_ERROR",
            "error": "invalid scenario_status 'bad'",
        });
        let result = set_readback_side_effect_hold(sentinel.clone(), "status", "msg");
        assert_eq!(result, sentinel, "sentinel must be returned unchanged");
    }

    #[test]
    fn parse_envelope_with_non_canonical_scenario_id_returns_none() {
        let invalid_readback = serde_json::json!({
            "schema_version": SCHEMA_READBACK_V1,
            "scenario_id": "P082-legacy-command-error",  // not in canonical vocabulary
            "scenario_status": "held",
            "recovery_decision": "wait",
            "recovery_reason_code": REASON_RESUME_CLAIM_STATUS,
            "recovery_next_action": "Inspect command journal.",
            "recovery_hold_conditions": [],
            "recovery_side_effect_blocking_status": null,
            "recovery_retry_identifier_guidance": null,
            "recovery_late_output_settlement": null,
            "recovery_startup_repair_summary": null,
            "recovery_operator_message": null,
            "recovery_projection_integrity": "unavailable",
            "source_table": "command_journal",
            "source_repository": "command_journal",
            "source_identifier": "cj-001",
            "source_json_key": null,
            "updated_at": "2026-05-21T00:00:00Z",
            "diagnostic_redaction": "none",
        });
        let envelope = serde_json::json!({
            "schema_version": SCHEMA_REJECTED_COMMAND_ERROR_V1,
            "reason_code": REASON_RESUME_CLAIM_STATUS,
            "command_type": "RetryStage",
            "redaction": "none",
            "operator_safe_summary": "Legacy error.",
            "p082_recovery_matrix_readback": invalid_readback,
        })
        .to_string();
        assert!(
            parse_command_journal_error_envelope(&envelope).is_none(),
            "envelope with non-canonical scenario_id must return None"
        );
    }

    #[test]
    fn parse_envelope_with_empty_next_action_for_non_not_applicable_returns_none() {
        let invalid_readback = serde_json::json!({
            "schema_version": SCHEMA_READBACK_V1,
            "scenario_id": "P082-R02",
            "scenario_status": "rejected",
            "recovery_decision": "no_mutation",
            "recovery_reason_code": REASON_INVALID_STAGE_FOR_RETRY,
            "recovery_next_action": "",  // must not be empty for non-not_applicable
            "recovery_hold_conditions": [],
            "recovery_side_effect_blocking_status": null,
            "recovery_retry_identifier_guidance": null,
            "recovery_late_output_settlement": null,
            "recovery_startup_repair_summary": null,
            "recovery_operator_message": null,
            "recovery_projection_integrity": "valid",
            "source_table": "command_journal",
            "source_repository": "command_journal",
            "source_identifier": "cj-001",
            "source_json_key": null,
            "updated_at": "2026-05-21T00:00:00Z",
            "diagnostic_redaction": "none",
        });
        let envelope = serde_json::json!({
            "schema_version": SCHEMA_REJECTED_COMMAND_ERROR_V1,
            "reason_code": REASON_INVALID_STAGE_FOR_RETRY,
            "command_type": "RetryStage",
            "redaction": "none",
            "operator_safe_summary": "Stage not retryable.",
            "p082_recovery_matrix_readback": invalid_readback,
        })
        .to_string();
        assert!(
            parse_command_journal_error_envelope(&envelope).is_none(),
            "envelope with empty recovery_next_action for non-not_applicable status must return None"
        );
    }

    #[test]
    fn validate_readback_v1_shape_accepts_valid_row() {
        let rb = set_readback_startup_repair(
            build_readback_v1(
                "P082-R01",
                "repaired",
                "retry",
                REASON_STARTUP_REQUEUE_ONCE,
                "Startup requeue scheduled.",
                "startup_repairs",
                "startup_repairs, work_items",
                "sr-001",
                Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            ),
            startup_summary(),
            None,
        );
        assert!(
            validate_readback_v1_shape(&rb),
            "validate_readback_v1_shape must accept a fully populated valid readback"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_non_canonical_scenario_id() {
        let mut rb = build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs",
            "sr-001",
            None,
            "valid",
            "2026-05-21T00:00:00Z",
        );
        if let Some(obj) = rb.as_object_mut() {
            obj.insert("scenario_id".into(), serde_json::json!("P082-tampered-id"));
        }
        assert!(
            !validate_readback_v1_shape(&rb),
            "validate_readback_v1_shape must reject non-canonical scenario_id"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_missing_updated_at() {
        let mut rb = build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs",
            "sr-001",
            None,
            "valid",
            "2026-05-21T00:00:00Z",
        );
        if let Some(obj) = rb.as_object_mut() {
            obj.remove("updated_at");
        }
        assert!(
            !validate_readback_v1_shape(&rb),
            "validate_readback_v1_shape must reject row missing updated_at"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_non_iso_updated_at() {
        let rb = set_readback_startup_repair(
            build_readback_v1(
                "P082-R01",
                "repaired",
                "retry",
                REASON_STARTUP_REQUEUE_ONCE,
                "Startup requeue scheduled.",
                "startup_repairs",
                "startup_repairs",
                "sr-001",
                Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                "valid",
                "not-a-timestamp",
            ),
            startup_summary(),
            None,
        );
        assert!(
            !validate_readback_v1_shape(&rb),
            "validate_readback_v1_shape must reject non-ISO/RFC3339 updated_at values"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_missing_required_startup_summary() {
        let rb = build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled.",
            "startup_repairs",
            "startup_repairs",
            "sr-001",
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        );
        assert!(
            !validate_readback_v1_shape(&rb),
            "P082-R01 must fail closed when recovery_startup_repair_summary is null"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_missing_required_late_output_settlement() {
        let rb = build_readback_v1(
            "P082-R03",
            "repaired",
            "no_mutation",
            REASON_IGNORED_LATE_OUTPUTS,
            "Late output was ignored.",
            "agent_execution_runtime_facts, artifact_source_generation_claims, work_items",
            "agent_execution_runtime_facts, artifact_contracts, work_items",
            "stage-exec-001",
            Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        );
        assert!(
            !validate_readback_v1_shape(&rb),
            "P082-R03 must fail closed when recovery_late_output_settlement is null"
        );
    }

    #[test]
    fn validate_readback_v1_shape_accepts_required_late_output_settlement() {
        let rb = set_readback_late_output_settlement(
            build_readback_v1(
                "P082-R17",
                "repaired",
                "no_mutation",
                REASON_CANCELLED_PROVIDER_LATE_OUTPUT_IGNORED,
                "Late output from cancelled provider session was ignored.",
                "agent_execution_runtime_facts, artifact_source_generation_claims, work_items",
                "agent_execution_runtime_facts, artifact_contracts, work_items",
                "stage-exec-001",
                Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            ),
            late_output_settlement(true),
        );
        assert!(
            validate_readback_v1_shape(&rb),
            "P082-R17 must validate when recovery_late_output_settlement is present and valid"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_cancelled_late_output_terminal_status() {
        let settlement = build_late_output_settlement(
            "ae-001",
            "wi-001",
            "sg-old",
            "sg-active",
            "closed",
            "ignored",
            1,
            "cancelled",
            true,
        );
        let rb = set_readback_late_output_settlement(
            build_readback_v1(
                "P082-R17",
                "repaired",
                "no_mutation",
                REASON_CANCELLED_PROVIDER_LATE_OUTPUT_IGNORED,
                "Late output from cancelled provider session was ignored.",
                "agent_execution_runtime_facts, artifact_source_generation_claims, work_items",
                "agent_execution_runtime_facts, artifact_contracts, work_items",
                "stage-exec-001",
                Some("stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            ),
            settlement,
        );

        assert!(
            !validate_readback_v1_shape(&rb),
            "P082-R17 settlement must use completed/failed source work item status, not cancelled"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_missing_required_identifier_guidance() {
        let rb = build_readback_v1(
            "P082-R08",
            "rejected",
            "no_mutation",
            REASON_VALID_IDENTIFIER_GUIDANCE,
            "Use the expected identifier kind.",
            "command_journal",
            "command_journal, agent_executions, stage_executions",
            "cmd-001",
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        );
        assert!(
            !validate_readback_v1_shape(&rb),
            "P082-R08 must fail closed when recovery_retry_identifier_guidance is null"
        );
    }

    #[test]
    fn validate_readback_v1_shape_accepts_required_identifier_guidance() {
        let rb = set_readback_identifier_guidance(
            build_readback_v1(
                "P082-R08",
                "rejected",
                "no_mutation",
                REASON_VALID_IDENTIFIER_GUIDANCE,
                "Use the expected identifier kind.",
                "command_journal",
                "command_journal, agent_executions, stage_executions",
                "cmd-001",
                Some("command_journal.error.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            ),
            identifier_guidance(),
        );
        assert!(
            validate_readback_v1_shape(&rb),
            "P082-R08 must validate when recovery_retry_identifier_guidance is present and valid"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_side_effect_status_on_unrelated_scenario() {
        let mut rb = set_readback_startup_repair(
            build_readback_v1(
                "P082-R01",
                "repaired",
                "retry",
                REASON_STARTUP_REQUEUE_ONCE,
                "Startup requeue scheduled.",
                "startup_repairs",
                "startup_repairs, work_items",
                "sr-001",
                Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            ),
            startup_summary(),
            None,
        );
        rb["recovery_side_effect_blocking_status"] =
            serde_json::json!("unresolved_side_effect_entries");

        assert!(
            !validate_readback_v1_shape(&rb),
            "side-effect blocking status must be null outside side-effect scenarios"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_identifier_guidance_on_unrelated_scenario() {
        let mut rb = build_readback_v1(
            "P082-R02",
            "rejected",
            "no_mutation",
            REASON_RESUME_CLAIM_STATUS,
            "Retry command was rejected before mutation.",
            "command_journal",
            "command_journal",
            "cmd-001",
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        );
        rb["recovery_retry_identifier_guidance"] = identifier_guidance();

        assert!(
            !validate_readback_v1_shape(&rb),
            "retry identifier guidance must be null outside identifier-guidance scenarios"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_late_output_settlement_on_unrelated_scenario() {
        let mut rb = set_readback_startup_repair(
            build_readback_v1(
                "P082-R01",
                "repaired",
                "retry",
                REASON_STARTUP_REQUEUE_ONCE,
                "Startup requeue scheduled.",
                "startup_repairs",
                "startup_repairs, work_items",
                "sr-001",
                Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            ),
            startup_summary(),
            None,
        );
        rb["recovery_late_output_settlement"] = late_output_settlement(false);

        assert!(
            !validate_readback_v1_shape(&rb),
            "late-output settlement must be null outside late-output scenarios"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_startup_summary_on_unrelated_scenario() {
        let mut rb = build_readback_v1(
            "P082-R02",
            "rejected",
            "no_mutation",
            REASON_RESUME_CLAIM_STATUS,
            "Retry command was rejected before mutation.",
            "command_journal",
            "command_journal",
            "cmd-001",
            Some("command_journal.error.p082_recovery_matrix_readback"),
            "valid",
            "2026-05-21T00:00:00Z",
        );
        rb["recovery_startup_repair_summary"] = startup_summary();

        assert!(
            !validate_readback_v1_shape(&rb),
            "startup repair summary must be null outside startup/crash scenarios"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_null_json_source_key_for_json_owner() {
        let rb = build_readback_v1(
            "P082-R02",
            "rejected",
            "no_mutation",
            REASON_INVALID_STAGE_FOR_RETRY,
            "Stage is not retryable.",
            "command_journal",
            "command_journal, stages",
            "cmd-001",
            None,
            "valid",
            "2026-05-21T00:00:00Z",
        );
        assert!(
            !validate_readback_v1_shape(&rb),
            "command_journal.error-backed P082-R02 rows must carry the approved source_json_key"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_wrong_json_source_key_for_json_owner() {
        let rb = set_readback_startup_repair(
            build_readback_v1(
                "P082-R01",
                "repaired",
                "retry",
                REASON_STARTUP_REQUEUE_ONCE,
                "Startup requeue scheduled.",
                "startup_repairs",
                "startup_repairs",
                "sr-001",
                Some("command_journal.error.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            ),
            startup_summary(),
            None,
        );
        assert!(
            !validate_readback_v1_shape(&rb),
            "startup_repairs.notes-backed P082-R01 rows must carry the startup_repairs source_json_key"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_null_source_json_key_for_pending_approval_row() {
        let rb = build_readback_v1(
            "P082-R09",
            "pending",
            "operator_approval_required",
            REASON_APPROVAL_PENDING_OPERATOR_ACTION_REQUIRED,
            "Pending approval was preserved across recovery.",
            "approvals, approval_inbox, stage_executions",
            "approvals, approval_inbox, stage_executions",
            "approval-001",
            None,
            "valid",
            "2026-05-21T00:00:00Z",
        );
        assert!(
            !validate_readback_v1_shape(&rb),
            "P082-R09 rows must carry the approved stage recovery snapshot source_json_key"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_wrong_json_source_key_for_duplicate_owner_rows() {
        for (scenario_id, reason_code, expected_key) in [
            (
                "P082-R04",
                REASON_DUPLICATE_OWNER_REPAIRED,
                "session_events.details_json.p082_recovery_matrix_readback",
            ),
            (
                "P082-R10",
                REASON_DUPLICATE_MEDIATION_OWNER_REJECTED,
                "lead_conflict_mediations.validation_errors_json.p082_recovery_matrix_readback",
            ),
        ] {
            let rb = build_readback_v1(
                scenario_id,
                "rejected",
                "inspect_duplicate_owner",
                reason_code,
                "Duplicate owner evidence preserved.",
                "owner_table",
                "owner_table",
                "owner-001",
                Some("command_journal.error.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-21T00:00:00Z",
            );
            assert!(
                !validate_readback_v1_shape(&rb),
                "{scenario_id} must reject wrong source_json_key; expected {expected_key}"
            );
        }
    }

    #[test]
    fn validate_readback_v1_shape_requires_r06_approved_owner_key() {
        let rb = build_readback_v1(
            "P082-R06",
            "held",
            "wait",
            REASON_NEEDS_EFFECT_RECONCILIATION,
            "Stale scheduler ownership is held for reconciliation.",
            "work_items, startup_repairs, side_effects",
            "work_items, startup_repairs, side_effects",
            "se-001",
            None,
            "stale",
            "2026-05-21T00:00:00Z",
        );
        assert!(
            !validate_readback_v1_shape(&rb),
            "P082-R06 held rows must not pass with source_json_key=null"
        );
    }

    #[test]
    fn validate_readback_v1_shape_accepts_null_source_json_key_for_legacy_error_fallback() {
        let rb = build_readback_v1(
            "P082-R02",
            "held",
            "wait",
            REASON_RESUME_CLAIM_STATUS,
            "Legacy rejection record exists; no P082 typed readback is available.",
            "command_journal",
            "command_journal",
            "cmd-legacy",
            None,
            "unavailable",
            "2026-05-21T00:00:00Z",
        );
        assert!(
            validate_readback_v1_shape(&rb),
            "legacy plain-text command_journal.error fallback rows may keep source_json_key null"
        );
    }

    #[test]
    fn validate_readback_v1_shape_rejects_empty_next_action_for_non_not_applicable() {
        let rb = build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            REASON_STARTUP_REQUEUE_ONCE,
            "", // empty — must be rejected for repaired status
            "startup_repairs",
            "startup_repairs",
            "sr-001",
            None,
            "valid",
            "2026-05-21T00:00:00Z",
        );
        // build_readback_v1 returns a sentinel for empty next_action on non-not_applicable status
        assert!(
            !validate_readback_v1_shape(&rb),
            "validate_readback_v1_shape must reject row with empty recovery_next_action for non-not_applicable status"
        );
    }

    #[test]
    fn parse_envelope_with_invalid_nested_decision_returns_none() {
        let invalid_readback = serde_json::json!({
            "schema_version": SCHEMA_READBACK_V1,
            "scenario_id": "P082-R01",
            "scenario_status": "repaired",
            "recovery_decision": "repair_converged",  // invalid
            "recovery_reason_code": REASON_STARTUP_REQUEUE_ONCE,
            "recovery_next_action": "Startup requeue scheduled.",
            "recovery_hold_conditions": [],
            "recovery_side_effect_blocking_status": null,
            "recovery_retry_identifier_guidance": null,
            "recovery_late_output_settlement": null,
            "recovery_startup_repair_summary": null,
            "recovery_operator_message": null,
            "recovery_projection_integrity": "valid",
            "source_table": "startup_repairs",
            "source_repository": "startup_repairs",
            "source_identifier": "sr-001",
            "source_json_key": null,
            "updated_at": "2026-05-21T00:00:00Z",
            "diagnostic_redaction": "none",
        });
        let envelope = serde_json::json!({
            "schema_version": SCHEMA_REJECTED_COMMAND_ERROR_V1,
            "reason_code": REASON_STARTUP_REQUEUE_ONCE,
            "command_type": "StartupRepair",
            "redaction": "none",
            "operator_safe_summary": "Startup repair requeue.",
            "p082_recovery_matrix_readback": invalid_readback,
        })
        .to_string();
        let parsed = parse_command_journal_error_envelope(&envelope);
        assert!(
            parsed.is_none(),
            "envelope with nested readback using legacy recovery_decision must return None"
        );
    }
}
