//! P082 recovery/retry state-machine matrix: shared reason-code constants,
//! readback schema types, and envelope parsing.
//!
//! Reason codes are append-only. Do not rename or remove existing codes.

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
/// top-level fields are present, and — when the nested
/// `p082_recovery_matrix_readback` object is non-null — its own
/// `schema_version`, `scenario_status`, and `recovery_decision` fields are
/// also present and within the approved vocabularies.
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

    // When the nested readback is non-null, validate its full schema contract.
    if !readback_field.is_null() && !validate_readback_v1_shape(readback_field) {
        return None;
    }

    Some(v)
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

fn object_string<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a str> {
    obj.get(key).and_then(|value| value.as_str())
}

fn object_required_string(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> bool {
    object_string(obj, key).is_some()
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
    if !validate_retry_identifier_guidance(
        obj.get("recovery_retry_identifier_guidance")
            .unwrap_or(&serde_json::Value::Null),
    ) {
        return false;
    }
    if !validate_late_output_settlement(
        obj.get("recovery_late_output_settlement")
            .unwrap_or(&serde_json::Value::Null),
    ) {
        return false;
    }
    if !validate_startup_repair_summary(
        obj.get("recovery_startup_repair_summary")
            .unwrap_or(&serde_json::Value::Null),
    ) {
        return false;
    }
    if !object_string_or_null(obj, "recovery_operator_message") {
        return false;
    }
    if status == "held"
        && matches!(
            scenario_id,
            "P082-R05" | "P082-R07" | "P082-R13" | "P082-R16"
        )
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
    if obj.get("source_table").and_then(|v| v.as_str()).is_none() {
        return false;
    }
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
    if obj.get("updated_at").and_then(|v| v.as_str()).is_none() {
        return false;
    }
    if !object_string_or_null(obj, "source_json_key") {
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
        let rb = build_readback_v1(
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
