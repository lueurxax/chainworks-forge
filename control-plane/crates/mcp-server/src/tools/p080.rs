/// P080 Continuous Stale Execution Reconciliation — MCP tool handlers.
///
/// Three tools are registered:
///   p080.diagnostics.get.v1      — read-only diagnostics page (Phase 1+)
///   p080.reconcile.request.v1    — diagnose_only or repair_if_safe (Phase 1/2+)
///   p080.clear_permanent_hold.v1 — clear a permanent hold (Phase 5+)
///
/// rollout_control.set is Phase 2+ only; Phase 0/1 rollout changes use
/// the daemon-local seed path. The MCP tool is not registered here.
///
/// Phase 1: diagnostics.get.v1 classifies running executions and returns real
/// projection data. repair_if_safe and hold remain rollout_disabled.
/// clear_permanent_hold remains disabled until Phase 5.
use anyhow::Result;
use sqlx::SqlitePool;

use crate::protocol::McpTool;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "p080.diagnostics.get.v1".to_string(),
            description:
                "Read-only page of stale execution diagnostics. Returns p080_readback_v1 items \
                 with running-truth classification, hold reasons, and projection integrity. \
                 Requires p080:diagnostics capability."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "schema_version": {
                        "type": "string",
                        "description": "Must be \"p080_diagnostics_get_request_v1\"."
                    },
                    "filter": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "run_id":        { "type": ["string", "null"] },
                            "stage_id":      { "type": ["string", "null"] },
                            "work_item_id":  { "type": ["string", "null"] },
                            "stale_class":   { "type": ["string", "null"] },
                            "hold_reason":   { "type": ["string", "null"] },
                            "include_recent_repaired": { "type": "boolean" }
                        }
                    },
                    "page_size": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 200,
                        "description": "Items per page (default 50)."
                    },
                    "cursor": {
                        "type": ["string", "null"],
                        "description": "Opaque p080_cursor_v1 base64url continuation cursor."
                    },
                    "request_total_count": {
                        "type": "boolean",
                        "description": "When true, attempt exact total_count_exact (may be null if budget exceeded)."
                    }
                }
            }),
        },
        McpTool {
            name: "p080.reconcile.request.v1".to_string(),
            description:
                "Request reconciliation for a specific stale execution tuple. \
                 diagnose_only is always available; repair_if_safe requires p080:repair \
                 capability and an active rollout phase. Idempotent via operator_request_dedup_key."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["schema_version", "target", "requested_action"],
                "additionalProperties": false,
                "properties": {
                    "schema_version": {
                        "type": "string",
                        "description": "Must be \"p080_reconcile_request_v1\"."
                    },
                    "target": {
                        "type": "object",
                        "required": ["run_id", "stage_id", "work_item_id", "stale_class"],
                        "additionalProperties": false,
                        "properties": {
                            "run_id":       { "type": "string" },
                            "stage_id":     { "type": "string" },
                            "work_item_id": { "type": "string" },
                            "stale_class":  { "type": "string" }
                        }
                    },
                    "requested_action": {
                        "type": "string",
                        "enum": ["diagnose_only", "repair_if_safe", "hold"],
                        "description": "diagnose_only: no writes; repair_if_safe: active repair if rollout allows; hold: reserved."
                    },
                    "operator_request_dedup_key": {
                        "type": "string",
                        "maxLength": 128,
                        "description": "Required for repair_if_safe and hold. Forbidden for diagnose_only."
                    },
                    "operator_message": {
                        "type": "string",
                        "description": "Optional operator note (NFC, ≤240 bytes post-redaction)."
                    },
                    "expected_predicate_hash": {
                        "type": ["string", "null"],
                        "description": "Optional sha256 hex of expected predicate snapshot."
                    }
                }
            }),
        },
        McpTool {
            name: "p080.clear_permanent_hold.v1".to_string(),
            description:
                "Clear a permanent hold on a stale execution after manual operator verification. \
                 Enabled only in Phase 5. Requires p080:clear_permanent_hold capability."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["schema_version", "target", "operator_request_dedup_key"],
                "additionalProperties": false,
                "properties": {
                    "schema_version": {
                        "type": "string",
                        "description": "Must be \"p080_clear_permanent_hold_request_v1\"."
                    },
                    "target": {
                        "type": "object",
                        "required": ["run_id", "stage_id", "work_item_id", "stale_class"],
                        "additionalProperties": false,
                        "properties": {
                            "run_id":       { "type": "string" },
                            "stage_id":     { "type": "string" },
                            "work_item_id": { "type": "string" },
                            "stale_class":  { "type": "string" }
                        }
                    },
                    "operator_request_dedup_key": {
                        "type": "string",
                        "maxLength": 128
                    },
                    "operator_message": {
                        "type": "string",
                        "description": "Optional operator note (NFC, ≤240 bytes post-redaction)."
                    }
                }
            }),
        },
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    // SEC-HIGH-002 fix: do NOT check live_disable here.
    // Each handler checks live_disable AFTER schema/action extraction and
    // action-level capability checks, per the approved auth-before-rollout
    // ordering (proposal lines 131-139). This prevents unauthorized callers
    // from inferring rollout-control state through the error code they receive.
    let result = match tool_name {
        "p080.diagnostics.get.v1" => handle_diagnostics_get(params, pool, principal).await,
        "p080.reconcile.request.v1" => handle_reconcile_request(params, pool, principal).await,
        "p080.clear_permanent_hold.v1" => {
            handle_clear_permanent_hold(params, pool, principal).await
        }
        _ => Err(anyhow::anyhow!("Unknown p080 tool: {tool_name}")),
    };

    // Emit P080 operational metrics based on response code (proposal §7).
    if let Ok(ref val) = result {
        if let Some(code) = val.get("code").and_then(|c| c.as_str()) {
            match code {
                "unauthorized_missing_capability" | "unauthenticated" => {
                    // Labels: tool_name + missing_capability (from detail.required_capability).
                    let missing_cap = val
                        .get("detail")
                        .and_then(|d| d.get("required_capability"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("unknown");
                    db::metrics::increment_counter_with_label(
                        "p080_mcp_unauthorized_missing_capability_total",
                        &format!("tool_name={tool_name},missing_capability={missing_cap}"),
                    );
                }
                "action_disabled_in_phase" | "class_disabled" | "rollout_disabled"
                | "live_disabled" => {
                    // Label: action (p080_requested_action enum from detail.requested_action).
                    let action = val
                        .get("detail")
                        .and_then(|d| d.get("requested_action"))
                        .and_then(|a| a.as_str())
                        .unwrap_or("unknown");
                    db::metrics::increment_counter_with_label(
                        "p080_mcp_disabled_action_rejected_total",
                        action,
                    );
                }
                "unsupported_version" | "version_mismatch" => {
                    db::metrics::increment_counter_with_label(
                        "p080_mcp_unsupported_version_total",
                        tool_name,
                    );
                }
                "enumeration_budget_exceeded" => {
                    db::metrics::increment_counter(
                        "p080_diagnostics_enumeration_budget_exceeded_total",
                    );
                }
                _ => {}
            }
        }
    }

    result
}

/// Check the live_disable rollout-control row and return an error if it is
/// active or absent. Must be called AFTER action-level authorization checks.
///
/// Returns `Ok(None)` when the caller may proceed.
/// Returns `Ok(Some(err))` when live_disable is on or the row is missing.
/// Returns `Err(_)` on DB failure.
async fn check_live_disable(
    pool: &SqlitePool,
    _tool_name: &str,
) -> Result<Option<serde_json::Value>> {
    match db::repos::p080::get_rollout_control(pool, "live_disable").await {
        Ok(Some(row)) if row.enabled => Ok(Some(p080_error_detail(
            "live_disabled",
            "P080 reconciliation is globally disabled (live_disable active)",
            serde_json::json!({
                "rollout_disablement": "live_disabled"
            }),
            None,
        ))),
        Ok(Some(_)) => Ok(None), // row present and disabled → caller may proceed
        Ok(None) => Ok(Some(p080_error_detail(
            "live_disabled",
            "P080 rollout-control row (live_disable) is absent; refusing to dispatch (fail-closed)",
            serde_json::json!({
                "rollout_disablement": "live_disabled"
            }),
            None,
        ))),
        Err(_) => Ok(Some(p080_error_detail(
            "internal_error",
            "failed to read live_disable rollout control; refusing to dispatch",
            serde_json::json!({ "retry_after": null }),
            None,
        ))),
    }
}

/// P080 argument resource limits (proposal §5 resource-limit vocabulary).
/// Checked on the already-parsed Value; prevents oversized inputs from reaching DB.
/// Duplicate-key rejection requires raw-byte interception (Phase 2+ work).
const P080_MAX_DEPTH: usize = 32;
const P080_MAX_ARRAY_LEN: usize = 500;
const P080_MAX_STRING_BYTES: usize = 16384; // 16 KiB

fn check_p080_resource_limits(
    value: &serde_json::Value,
    depth: usize,
) -> Option<serde_json::Value> {
    if depth > P080_MAX_DEPTH {
        return Some(p080_error_detail(
            "json_depth_exceeded",
            &format!("JSON nesting depth exceeds limit of {P080_MAX_DEPTH}"),
            serde_json::json!({ "limit": P080_MAX_DEPTH, "observed": depth }),
            None,
        ));
    }
    match value {
        serde_json::Value::Object(map) => {
            for v in map.values() {
                if let Some(err) = check_p080_resource_limits(v, depth + 1) {
                    return Some(err);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            if arr.len() > P080_MAX_ARRAY_LEN {
                return Some(p080_error_detail(
                    "array_length_exceeded",
                    &format!(
                        "array length {len} exceeds limit of {P080_MAX_ARRAY_LEN}",
                        len = arr.len()
                    ),
                    serde_json::json!({ "limit": P080_MAX_ARRAY_LEN, "observed": arr.len() }),
                    None,
                ));
            }
            for v in arr {
                if let Some(err) = check_p080_resource_limits(v, depth + 1) {
                    return Some(err);
                }
            }
        }
        serde_json::Value::String(s) => {
            if s.len() > P080_MAX_STRING_BYTES {
                return Some(p080_error_detail(
                    "string_too_large",
                    &format!(
                        "string value length {} exceeds limit of {P080_MAX_STRING_BYTES} bytes",
                        s.len()
                    ),
                    serde_json::json!({ "limit": P080_MAX_STRING_BYTES, "observed": s.len() }),
                    None,
                ));
            }
        }
        _ => {}
    }
    None
}

async fn handle_diagnostics_get(
    params: serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    // SEC-P080-MED-001: resource limits enforced first, before schema or auth checks.
    if let Some(err) = check_p080_resource_limits(&params, 0) {
        return Ok(err);
    }

    // schema_version is required (strict enforcement per P080 §5.1).
    // Schema extraction runs before live_disable per proposal lines 131-139.
    match params["schema_version"].as_str() {
        Some("p080_diagnostics_get_request_v1") => {}
        Some(v) => {
            return Ok(p080_error_detail(
                "unsupported_version",
                "schema_version must be \"p080_diagnostics_get_request_v1\"",
                serde_json::json!({
                    "requested_schema_version": v,
                    "accepted_schema_versions": ["p080_diagnostics_get_request_v1"]
                }),
                None,
            ));
        }
        None => {
            return Ok(p080_error_detail(
                "unsupported_version",
                "schema_version is required; must be \"p080_diagnostics_get_request_v1\"",
                serde_json::json!({
                    "requested_schema_version": null,
                    "accepted_schema_versions": ["p080_diagnostics_get_request_v1"]
                }),
                None,
            ));
        }
    }

    // SEC-P080-MED-001: closed-schema enforcement before live_disable.
    const DIAGNOSTICS_GET_ALLOWED_FIELDS: &[&str] = &[
        "schema_version",
        "filter",
        "page_size",
        "cursor",
        "request_total_count",
    ];
    if let Some(err) = check_unknown_fields(&params, DIAGNOSTICS_GET_ALLOWED_FIELDS) {
        return Ok(err);
    }
    // SEC-P080-MED-002: validate nested 'filter' object against its closed schema.
    const FILTER_ALLOWED_FIELDS: &[&str] = &[
        "run_id",
        "stage_id",
        "work_item_id",
        "stale_class",
        "hold_reason",
        "include_recent_repaired",
    ];
    if let Some(err) = check_unknown_nested_fields(&params, "filter", FILTER_ALLOWED_FIELDS) {
        return Ok(err);
    }

    // SEC-P080-002: filter extraction and run-scope auth BEFORE rollout gate checks
    // so unauthorized callers cannot infer rollout state through error codes.
    // SEC-P080-MED-003: extract_filter returns Err if a filter field is present but invalid.
    let filter = match extract_filter(&params) {
        Ok(f) => f,
        Err(err) => return Ok(err),
    };

    // SEC-P080-HIGH-001: server-derived auth-scoped visibility (fail-closed).
    // check_p080_run_scope rejects restricted principals (Agent/ReadOnlyOperator) that
    // have no run_scope configured, preventing cross-run disclosure via caller-supplied run_id.
    let filter_run_id_str: Option<&str> = filter.run_id.as_deref();
    if let Err(_scope_err) = auth::check_p080_run_scope(principal, filter_run_id_str) {
        return Ok(p080_error_detail(
            "unauthorized_missing_capability",
            "p080:diagnostics capability required; run_scope must include the requested run_id",
            serde_json::json!({ "required_capability": "p080:diagnostics" }),
            None,
        ));
    }

    // Rollout gate checks run AFTER run-scope auth is established.
    if let Some(err) = check_live_disable(pool, "p080.diagnostics.get.v1").await? {
        return Ok(err);
    }

    // Gate on detection_only rollout row (proposal §3.1 L617-624, DEFECT-007).
    // diagnostics.get must not expose stale projections when detection_only is disabled.
    let detection_enabled = match db::repos::p080::get_rollout_control(pool, "detection_only").await
    {
        Ok(Some(row)) if row.enabled => true,
        Ok(_) => false,
        Err(_) => false, // fail-closed on DB error
    };
    if !detection_enabled {
        return Ok(p080_error_detail(
            "rollout_disabled",
            "p080.diagnostics.get.v1 requires detection_only to be enabled",
            serde_json::json!({ "rollout_disablement": "class_disabled" }),
            None,
        ));
    }

    // include_recent_repaired is extracted inside extract_filter and stored on the filter.
    let include_recent_repaired = filter.include_recent_repaired;

    // Compute filter hash for cursor binding.  Includes cursor_scope + tool_name +
    // include_recent_repaired to prevent cross-surface and cross-preference cursor
    // replay (P080-SEC-MED-001).
    let filter_hash = compute_p080_filter_hash(&filter, P080_MCP_CURSOR_SCOPE, P080_MCP_TOOL_NAME, include_recent_repaired);

    // Fetch current projection_generation before pagination so:
    // (a) cursor decoder can validate the cursor was issued at the same generation, and
    // (b) cursor encoder can embed the current generation for future invalidation checks.
    let current_projection_generation =
        db::repos::p080::get_current_projection_generation(pool, &filter).await;

    // Decode cursor after filter extraction so the hash and generation can be validated.
    // Returns None (first page) or Some(KeysetAfter) for keyset continuation.
    let cursor_after: Option<db::repos::p080::KeysetAfter> = match decode_p080_cursor(
        params["cursor"].as_str(),
        &filter_hash,
        current_projection_generation,
    ) {
        Ok(after) => after,
        Err(err) => return Ok(err),
    };

    let page_size = params["page_size"].as_u64().unwrap_or(50).clamp(1, 200) as usize;
    // request_total_count: only attempt exact count when caller explicitly requests it.
    // When absent or false, return null per the approved closed request semantics.
    let request_total_count = params["request_total_count"].as_bool().unwrap_or(false);

    // Compute total_count_exact BEFORE page truncation using the budgeted counter.
    // When the exact count exceeds COUNT_BUDGET, return enumeration_budget_exceeded per the
    // proposal: "over-budget requests return error code enumeration_budget_exceeded with no
    // partial items or total_count" (proposal §5.2 / line 312).
    let total_count_exact: serde_json::Value = if request_total_count {
        match db::repos::p080::count_readback_matching_budgeted(pool, &filter).await? {
            Some(count) => serde_json::json!(count),
            None => {
                return Ok(serde_json::json!({
                    "schema_version": "p080_error_response_v1",
                    "code": "enumeration_budget_exceeded",
                    "message": "total_count exact count exceeds budget; narrow your filter or omit request_total_count",
                    "retry_after": 30,
                    "readback": null,
                    "detail": { "budget_kind": "total_count" }
                }));
            }
        }
    } else {
        serde_json::Value::Null
    };

    let mut rows = db::repos::p080::list_readback_page_keyset(
        pool,
        filter,
        page_size + 1,
        cursor_after.as_ref(),
    )
    .await?;
    let has_next_page = rows.len() > page_size;
    if has_next_page {
        rows.truncate(page_size);
    }
    // Build last_ordering_tuple from the last row on the page for the next-page cursor.
    let last_ordering_tuple: Option<serde_json::Value> = rows.last().map(|r| {
        serde_json::json!({
            "projection_updated_at": r.projection_updated_at,
            "run_id": r.run_id,
            "stage_id": r.stage_id,
            "work_item_id": r.work_item_id
        })
    });
    let (next_cursor, cursor_expires_at) = if has_next_page {
        if let Some(lot) = last_ordering_tuple {
            let (token, expires_at) = encode_p080_cursor(
                &filter_hash,
                current_projection_generation,
                include_recent_repaired,
                lot,
            );
            (serde_json::Value::String(token), serde_json::Value::String(expires_at))
        } else {
            (serde_json::Value::Null, serde_json::Value::Null)
        }
    } else {
        (serde_json::Value::Null, serde_json::Value::Null)
    };

    // Proposal §6.2 (line 446): unknown fields reject with code=unknown_field.
    // Closed-schema enforcement for readback_json rows: if any DB row contains
    // a key not in the allow-list, reject with unknown_field (not schema_violation).
    for row in &rows {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&row.readback_json) {
            if let Some(obj) = parsed.as_object() {
                let unexpected: Vec<&str> = obj
                    .keys()
                    .filter(|k| !READBACK_ALLOWED_KEYS.contains(&k.as_str()))
                    .map(|k| k.as_str())
                    .collect();
                if !unexpected.is_empty() {
                    // P080-MCP-REDACTION-001: do NOT reflect the raw DB key name in the
                    // error response — an adversarially written readback_json row could
                    // encode secrets or injection payloads as JSON property names.
                    // Return only the count; use a static sentinel path instead.
                    return Ok(p080_error_detail(
                        "unknown_field",
                        "readback_json contains unknown keys; closed-schema contract violated",
                        serde_json::json!({
                            "field_path": "readback_json.<redacted>",
                            "unknown_key_count": unexpected.len()
                        }),
                        None,
                    ));
                }
            }
        }
    }

    // projection_integrity reflects projection health, not row count.
    // An empty projection is valid — no stale executions currently known.
    let projection_integrity = "valid";

    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            // Apply closed allow-list + string sanitization before returning
            // readback_json on any MCP/GraphQL lane.
            // On parse failure, emit a stale fallback with projection_integrity=stale
            // rather than silently emitting an empty object (proposal lines 188-189).
            let readback: serde_json::Value = serde_json::from_str(&row.readback_json)
                .map(redact_readback)
                .unwrap_or_else(|_| {
                    let fb = serde_json::json!({
                        "schema_version": "p080_readback_v1",
                        "run_id": row.run_id,
                        "stage_id": row.stage_id,
                        "work_item_id": row.work_item_id,
                        "stale_class": row.stale_class,
                        "running_truth": "useful",
                        "repair_action": "diagnose_only",
                        "hold_reason": "none",
                        "hold_age_seconds": null,
                        "next_retry_or_backoff_time": null,
                        "projection_updated_at": row.projection_updated_at,
                        "projection_integrity": "stale",
                        "executor_reregistration_state": "expected",
                        "rollout_disablement": "phase_not_reached",
                        "side_effect_status": "not_applicable",
                        "operator_message": "[readback parse error — stale rebuild]",
                        "evidence_marker_hash": null,
                        "repair_idempotency_key": null
                    });
                    redact_readback(fb)
                });
            serde_json::json!({
                "readback": readback,
                "last_repair_event_id": row.last_repair_event_id,
                "last_event_at": row.projection_updated_at,
                "recurrence_epoch": row.recurrence_epoch
            })
        })
        .collect();

    Ok(serde_json::json!({
        "schema_version": "p080_diagnostics_get_response_v1",
        "items": items,
        "page_info": {
            "next_cursor": next_cursor,
            "cursor_version": 1,
            "cursor_expires_at": cursor_expires_at,
            "has_next_page": has_next_page,
            "total_count_exact": total_count_exact
        },
        "projection_integrity": projection_integrity
    }))
}

async fn handle_reconcile_request(
    params: serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    // SEC-P080-MED-001: resource limits enforced first, before schema or auth checks.
    if let Some(err) = check_p080_resource_limits(&params, 0) {
        return Ok(err);
    }

    // schema_version is required (strict enforcement per P080 §5.1).
    match params["schema_version"].as_str() {
        Some("p080_reconcile_request_v1") => {}
        Some(v) => {
            return Ok(p080_error_detail(
                "unsupported_version",
                "schema_version must be \"p080_reconcile_request_v1\"",
                serde_json::json!({
                    "requested_schema_version": v,
                    "accepted_schema_versions": ["p080_reconcile_request_v1"]
                }),
                None,
            ));
        }
        None => {
            return Ok(p080_error_detail(
                "unsupported_version",
                "schema_version is required; must be \"p080_reconcile_request_v1\"",
                serde_json::json!({
                    "requested_schema_version": null,
                    "accepted_schema_versions": ["p080_reconcile_request_v1"]
                }),
                None,
            ));
        }
    }

    // requested_action must be present and from the closed vocabulary.
    // Unknown values are rejected with invalid_field (not rollout_disabled).
    let requested_action = match params["requested_action"].as_str() {
        Some(a @ ("diagnose_only" | "repair_if_safe" | "hold")) => a.to_string(),
        Some(_) => {
            return Ok(p080_error_detail(
                "invalid_field",
                "requested_action must be one of: diagnose_only, repair_if_safe, hold",
                serde_json::json!({
                    "field_path": "requested_action",
                    "reason": "value not in closed vocabulary: diagnose_only | repair_if_safe | hold"
                }),
                None,
            ));
        }
        None => {
            return Ok(p080_error_detail(
                "invalid_field",
                "requested_action is required",
                serde_json::json!({
                    "field_path": "requested_action",
                    "reason": "field is required but absent or null"
                }),
                None,
            ));
        }
    };
    let requested_action = requested_action.as_str();

    // Proposal §3.1: read_only_operator may only call diagnose_only.
    // repair_if_safe and hold require Operator class (p080:repair capability).
    if requested_action != "diagnose_only"
        && matches!(principal.class, auth::PrincipalClass::ReadOnlyOperator)
    {
        return Ok(p080_error_detail(
            "unauthorized_missing_capability",
            "read_only_operator may only request diagnose_only; repair_if_safe and hold require Operator",
            serde_json::json!({
                "required_capability": "p080:repair",
                "requested_action": requested_action
            }),
            None,
        ));
    }

    // SEC-P080-MED-001: closed-schema enforcement after action-auth but before live_disable.
    const RECONCILE_ALLOWED_FIELDS: &[&str] = &[
        "schema_version",
        "target",
        "requested_action",
        "operator_request_dedup_key",
        "operator_message",
        "expected_predicate_hash",
    ];
    if let Some(err) = check_unknown_fields(&params, RECONCILE_ALLOWED_FIELDS) {
        return Ok(err);
    }
    // SEC-P080-MED-002: validate nested 'target' object against its closed schema.
    const TARGET_ALLOWED_FIELDS: &[&str] = &["run_id", "stage_id", "work_item_id", "stale_class"];
    if let Some(err) = check_unknown_nested_fields(&params, "target", TARGET_ALLOWED_FIELDS) {
        return Ok(err);
    }

    // SEC-P080-HIGH-001 (P080-MCP-AUTHZ-001 fix): run_scope authorization must precede
    // all rollout-state checks. A scoped ReadOnlyOperator must be rejected for unauthorized
    // run_ids before they can observe live_disable or detection_only state — mirroring the
    // safe ordering in handle_diagnostics_get (lines 383-406).
    // target.run_id is a required schema field; extract it here for early auth binding.
    // If it is absent or invalid the scope check falls through to None, which fails closed
    // for restricted principals; the existing per-action field validation catches it later.
    {
        let early_run_id = params["target"]["run_id"]
            .as_str()
            .and_then(sanitize_identifier);
        if let Err(_scope_err) =
            auth::check_p080_run_scope(principal, early_run_id.as_deref())
        {
            return Ok(p080_error_detail(
                "unauthorized_missing_capability",
                "p080:diagnostics capability required; run_scope must include target.run_id",
                serde_json::json!({ "required_capability": "p080:diagnostics" }),
                None,
            ));
        }
    }

    // Rollout-gate checks run AFTER run-scope auth is established (P080-MCP-AUTHZ-001).
    if let Some(err) = check_live_disable(pool, "p080.reconcile.request.v1").await? {
        return Ok(err);
    }

    // SEC-P080-002: enforce server-side string length limits before any handler.
    // Schema declares these; runtime must enforce them too.
    if let Some(key) = params["operator_request_dedup_key"].as_str() {
        if key.len() > 128 {
            return Ok(p080_error_detail(
                "invalid_dedup_key",
                "operator_request_dedup_key exceeds maximum length of 128 bytes",
                serde_json::json!({
                    "field_path": "operator_request_dedup_key",
                    "limit": 128,
                    "observed": key.len()
                }),
                None,
            ));
        }
    }
    if let Some(msg) = params["operator_message"].as_str() {
        if msg.len() > 240 {
            return Ok(p080_error_detail(
                "operator_message_too_large",
                "operator_message exceeds maximum length of 240 bytes",
                serde_json::json!({
                    "field_path": "operator_message",
                    "limit": 240,
                    "observed": msg.len()
                }),
                None,
            ));
        }
    }

    // operator_request_dedup_key is forbidden for diagnose_only.
    if requested_action == "diagnose_only"
        && params["operator_request_dedup_key"].as_str().is_some()
    {
        return Ok(p080_error_detail(
            "invalid_field",
            "operator_request_dedup_key is forbidden for diagnose_only",
            serde_json::json!({
                "field_path": "operator_request_dedup_key",
                "reason": "field is not permitted when requested_action=diagnose_only"
            }),
            None,
        ));
    }

    if requested_action == "diagnose_only" {
        // DEFECT-5 (proposal §3.1, prepush 2026-06-09): diagnose_only is a
        // detection-class read of a readback row and must respect the
        // detection_only rollout gate, mirroring diagnostics.get. Default-off
        // rows seed with enabled=0, so unauthorized callers cannot infer
        // projection state until the operator promotes the class.
        let detection_enabled =
            match db::repos::p080::get_rollout_control(pool, "detection_only").await {
                Ok(Some(row)) if row.enabled => true,
                Ok(_) => false,
                Err(_) => false, // fail-closed on DB error
            };
        if !detection_enabled {
            return Ok(p080_error_detail(
                "rollout_disabled",
                "p080.reconcile.request.v1 diagnose_only requires detection_only to be enabled",
                serde_json::json!({
                    "requested_action": "diagnose_only",
                    "rollout_disablement": "class_disabled",
                }),
                None,
            ));
        }
        // DEFECT-5 fix: validate all required target fields before use.
        // unwrap_or("") on required fields is replaced with explicit rejection.
        let target = &params["target"];
        // SEC-P080-MED-001: validate length, reject control/bidi, reject oversized.
        let run_id = match target["run_id"].as_str().and_then(|s| sanitize_uuid_id(s)) {
            Some(s) => s,
            None => return Ok(p080_error_detail(
                "invalid_field",
                "target.run_id is required and must be a valid UUID",
                serde_json::json!({"field_path": "target.run_id"}),
                None,
            )),
        };
        // SEC-P080-HIGH-001: check run_scope authorization before reading target readback.
        if let Err(_scope_err) = auth::check_p080_run_scope(principal, Some(run_id.as_str())) {
            return Ok(p080_error_detail(
                "unauthorized_missing_capability",
                "p080:diagnostics capability required; run_scope must include target.run_id",
                serde_json::json!({ "required_capability": "p080:diagnostics" }),
                None,
            ));
        }
        let stage_id = match target["stage_id"].as_str().and_then(|s| sanitize_identifier(s)) {
            Some(s) => s,
            None => return Ok(p080_error_detail(
                "invalid_field",
                "target.stage_id is required, must be non-empty, <= 256 bytes, and contain no control characters",
                serde_json::json!({"field_path": "target.stage_id"}),
                None,
            )),
        };
        let work_item_id = match target["work_item_id"].as_str().and_then(|s| sanitize_uuid_id(s)) {
            Some(s) => s,
            None => return Ok(p080_error_detail(
                "invalid_field",
                "target.work_item_id is required and must be a valid UUID",
                serde_json::json!({"field_path": "target.work_item_id"}),
                None,
            )),
        };
        let stale_class_str = match target["stale_class"].as_str().filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => {
                return Ok(p080_error_detail(
                    "invalid_field",
                    "target.stale_class is required",
                    serde_json::json!({"field_path": "target.stale_class"}),
                    None,
                ))
            }
        };

        // Validate stale_class against the closed P080StaleClass vocabulary.
        if stale_class_str
            .parse::<domain::p080::P080StaleClass>()
            .is_err()
        {
            return Ok(p080_error_detail(
                "invalid_field",
                "target.stale_class is not a recognised P080 class",
                serde_json::json!({"field_path": "target.stale_class"}),
                None,
            ));
        }
        let stale_class = stale_class_str;

        // SEC-HIGH-002 fix: route through redact_readback on every lane.
        let readback = match db::repos::p080::get_readback(
            pool,
            &run_id,
            &stage_id,
            &work_item_id,
            &stale_class,
        )
        .await
        {
            Ok(Some(row)) => serde_json::from_str(&row.readback_json)
                .map(redact_readback)
                .unwrap_or_else(|_| {
                    fallback_readback(&run_id, &stage_id, &work_item_id, &stale_class)
                }),
            _ => fallback_readback(&run_id, &stage_id, &work_item_id, &stale_class),
        };

        return Ok(serde_json::json!({
            "schema_version": "p080_reconcile_response_v1",
            "decision": "diagnosed",
            "event_id": null,
            "operator_message": "",
            "readback": readback
        }));
    }

    if requested_action == "hold" {
        // Proposal line 143 and §3.1: when hold is disabled in the current rollout
        // phase, action_disabled_in_phase wins. The proposal auth matrix (line 152)
        // requires this code — hold is a phase-gated action, not a rollout_disabled one.
        return Ok(p080_error_detail(
            "action_disabled_in_phase",
            "hold is disabled in all P080 phases pending a later proposal",
            serde_json::json!({
                "requested_action": "hold",
                "current_phase": "phase_1"
            }),
            None,
        ));
    }

    if requested_action == "repair_if_safe" {
        // Authorization check before phase check per proposal §3.1.
        // repair_if_safe requires p080:repair capability (Operator class).
        // Since ReadOnlyOperator is now denied P080 tools at the auth layer
        // (SEC-P080-HIGH-002), this branch is only reachable by Operator principals.
        // Keep the check as defense-in-depth.
        if !matches!(principal.class, auth::PrincipalClass::Operator) {
            return Ok(p080_error_detail(
                "unauthorized_missing_capability",
                "repair_if_safe requires p080:repair capability; Operator class required",
                serde_json::json!({
                    "required_capability": "p080:repair",
                    "requested_action": "repair_if_safe"
                }),
                None,
            ));
        }

        // Audit defect 1 fix: determine the disablement reason from rollout state
        // instead of returning the non-vocabulary action_disabled_in_phase code.
        // Proposal line 610 requires class_disabled / live_disabled / rollout_disabled
        // with rollout_disablement detail.
        //   class_disabled  = the specific stale class row has enabled=false
        //   rollout_disabled = class is enabled but the repair phase has not been reached
        // Proposal §6.2 (line 311): rollout_disablement must be from the closed
        // enum {none, phase_not_reached, class_disabled, live_disabled}.
        // Error code mirrors the disablement: class_disabled → "class_disabled",
        // phase mismatch → "rollout_disabled" with rollout_disablement=phase_not_reached.
        let (error_code, disablement_detail) =
            match db::repos::p080::get_rollout_control(pool, "acp_startup_stale").await {
                Ok(Some(row)) if !row.enabled => ("class_disabled", "class_disabled"),
                Ok(Some(_)) => ("rollout_disabled", "phase_not_reached"),
                Ok(None) => ("rollout_disabled", "phase_not_reached"),
                Err(_) => ("rollout_disabled", "phase_not_reached"),
            };
        return Ok(p080_error_detail(
            error_code,
            "repair_if_safe is not yet enabled (rollout phase not reached for this class)",
            serde_json::json!({
                "rollout_disablement": disablement_detail,
                "requested_action": "repair_if_safe"
            }),
            None,
        ));
    }

    // All three closed-enum actions are handled above; this is unreachable
    // due to the closed-enum validation at requested_action extraction.
    unreachable!("requested_action was validated to closed enum; all arms handled")
}

async fn handle_clear_permanent_hold(
    params: serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    // SEC-P080-MED-001: resource limits enforced first, before schema or auth checks.
    if let Some(err) = check_p080_resource_limits(&params, 0) {
        return Ok(err);
    }

    // DEFECT-003 fix: validate schema_version FIRST, before any rollout gate.
    // Proposal ordering (lines 131-139): schema extraction → action auth → live_disable.
    // An invalid/missing schema_version must return unsupported_version regardless of
    // rollout state, so callers learn the version error without inferring rollout state.
    match params.get("schema_version").and_then(|v| v.as_str()) {
        Some("p080_clear_permanent_hold_request_v1") => {}
        Some(v) => {
            return Ok(p080_error_detail(
                "unsupported_version",
                "schema_version must be \"p080_clear_permanent_hold_request_v1\"",
                serde_json::json!({
                    "requested_schema_version": v,
                    "accepted_schema_versions": ["p080_clear_permanent_hold_request_v1"]
                }),
                None,
            ));
        }
        None => {
            return Ok(p080_error_detail(
                "unsupported_version",
                "schema_version is required; must be \"p080_clear_permanent_hold_request_v1\"",
                serde_json::json!({
                    "requested_schema_version": null,
                    "accepted_schema_versions": ["p080_clear_permanent_hold_request_v1"]
                }),
                None,
            ));
        }
    }

    // SEC-P080-MED-001: closed-schema enforcement after schema check, before live_disable.
    const HOLD_ALLOWED_FIELDS: &[&str] = &[
        "schema_version",
        "target",
        "operator_request_dedup_key",
        "operator_message",
    ];
    if let Some(err) = check_unknown_fields(&params, HOLD_ALLOWED_FIELDS) {
        return Ok(err);
    }

    if !matches!(principal.class, auth::PrincipalClass::Operator) {
        return Ok(p080_error_detail(
            "unauthorized_missing_capability",
            "clear_permanent_hold requires p080:clear_permanent_hold capability; Operator class required",
            serde_json::json!({
                "required_capability": "p080:clear_permanent_hold",
                "requested_action": "clear_permanent_hold"
            }),
            None,
        ));
    }

    // live_disable checked AFTER schema validation per proposal ordering.
    if let Some(err) = check_live_disable(pool, "p080.clear_permanent_hold.v1").await? {
        return Ok(err);
    }

    // Phase 0/1: clear_permanent_hold is only active in Phase 5.
    // Proposal §6.2 (line 312): action_disabled_in_phase requires detail.requested_action
    // and detail.current_phase.
    Ok(p080_error_detail(
        "action_disabled_in_phase",
        "p080.clear_permanent_hold.v1 is not yet enabled (requires Phase 5)",
        serde_json::json!({
            "requested_action": "clear_permanent_hold",
            "current_phase": "phase_1"
        }),
        None,
    ))
}

/// Closed key allow-list for p080_readback_v1 objects (SEC-HIGH-002 fix).
/// Any key not in this set is stripped before the value is returned on any
/// MCP or GraphQL lane. All string fields are additionally secret-pattern-redacted.
const READBACK_ALLOWED_KEYS: &[&str] = &[
    "schema_version",
    "run_id",
    "stage_id",
    "work_item_id",
    "stale_class",
    "running_truth",
    "repair_action",
    "hold_reason",
    "hold_age_seconds",
    "next_retry_or_backoff_time",
    "projection_updated_at",
    "projection_integrity",
    "executor_reregistration_state",
    "rollout_disablement",
    "side_effect_status",
    "operator_message",
    "evidence_marker_hash",
    "repair_idempotency_key",
];

/// Returns true if `s` matches a known secret pattern that must never appear
/// in operator-visible readback output.
///
/// Covers: HTTP auth headers, API key prefixes with colon/equals separators,
/// provider-specific token prefixes, embedded key=value forms anywhere in prose,
/// high-entropy base64/hex tokens (SEC-P080-HIGH-001), and dedup/cursor-like
/// patterns. Negative controls: short strings, UUID-like patterns, known safe
/// P080 identifiers.
fn looks_like_secret(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();

    // HTTP authorization header prefixes.
    if lower.contains("bearer ") || lower.contains("authorization:") {
        return true;
    }

    // Common API key / token key prefixes with colon or equals separators.
    // Matched anywhere in the string (not just starts_with) to catch embedded prose.
    const KEY_PREFIXES: &[&str] = &[
        "token:",
        "token=",
        "api-key:",
        "api-key=",
        "api_key:",
        "api_key=",
        "apikey:",
        "apikey=",
        "password:",
        "password=",
        "passwd:",
        "passwd=",
        "secret:",
        "secret=",
        "private_key:",
        "private_key=",
        "access_token:",
        "access_token=",
        "refresh_token:",
        "refresh_token=",
        "client_secret:",
        "client_secret=",
        "auth_token:",
        "auth_token=",
        "credentials:",
        "credentials=",
        "session_token:",
        "session_token=",
    ];
    for prefix in KEY_PREFIXES {
        if lower.contains(prefix) {
            // Require a non-trivially-short value after the separator.
            if let Some(sep_idx) = lower.find(prefix) {
                let after = &lower[sep_idx + prefix.len()..];
                if after.len() >= 8 {
                    return true;
                }
            }
        }
    }

    // Embedded sk- token (OpenAI-style; match anywhere).
    if let Some(idx) = lower.find("sk-") {
        let after = &lower[idx + 3..];
        if after
            .chars()
            .next()
            .map(|c| c.is_ascii_alphanumeric())
            .unwrap_or(false)
        {
            return true;
        }
    }

    // Provider-specific token prefixes.
    if lower.contains("ghp_") || lower.contains("github_pat_") {
        return true;
    }
    if lower.contains("xoxb-")
        || lower.contains("xoxp-")
        || lower.contains("xoxa-")
        || lower.contains("xoxe-")
    {
        return true;
    }

    // AWS access key IDs (AKIA prefix, 20 chars total).
    if let Some(idx) = lower.find("akia") {
        let after = &lower[idx + 4..];
        let run: usize = after
            .chars()
            .take(16)
            .filter(|c| c.is_ascii_alphanumeric())
            .count();
        if run >= 12 {
            return true;
        }
    }

    // env-style: KEY_NAME=value (uppercase_or_mixed snake key, value >= 16 chars).
    // Match anywhere in the string to catch embedded prose like "error: API_KEY=abc...".
    {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            // Find '=' character.
            if let Some(eq_off) = bytes[i..].iter().position(|&b| b == b'=') {
                let eq_abs = i + eq_off;
                // Extract key part up to eq_abs (scan backwards for word boundary).
                let key_start = bytes[..eq_abs]
                    .iter()
                    .rposition(|&b| !b.is_ascii_alphanumeric() && b != b'_')
                    .map(|p| p + 1)
                    .unwrap_or(0);
                let key_part = &s[key_start..eq_abs];
                let val_part = &s[eq_abs + 1..];
                if key_part.len() >= 3
                    && key_part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && val_part.len() >= 16
                {
                    return true;
                }
                i = eq_abs + 1;
            } else {
                break;
            }
        }
    }

    // High-entropy detector: a contiguous run of base64url or hex characters
    // >= 32 chars is likely a token, key, or hash that should not leak.
    // Negative controls: UUIDs (36 chars with dashes) are excluded; run IDs and
    // work_item_ids produced by this system are UUID-format and safe.
    if s.len() >= 32 {
        // Count contiguous base64url chars: A-Za-z0-9+/=_-
        let base64_run = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(*c, '+' | '/' | '=' | '_' | '-'))
            .count();
        // High threshold: > 80% base64url density AND no UUID-like structure.
        let is_uuid_like = s.len() == 36
            && s.chars().enumerate().all(|(i, c)| {
                if i == 8 || i == 13 || i == 18 || i == 23 {
                    c == '-'
                } else {
                    c.is_ascii_hexdigit()
                }
            });
        if !is_uuid_like && base64_run * 100 / s.len() > 80 && base64_run >= 32 {
            return true;
        }

        // Pure hex run >= 32 chars (common for API secrets, HMAC keys, etc.).
        let hex_run = s.chars().filter(|c| c.is_ascii_hexdigit()).count();
        if !is_uuid_like && hex_run == s.len() && s.len() >= 32 {
            return true;
        }
    }

    false
}

fn is_control_or_bidi(c: char) -> bool {
    c < '\x20'
        || ('\u{7f}'..='\u{9f}').contains(&c)
        || matches!(c,
            '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
            | '\u{feff}')
}

/// Strip control and bidi characters from a string, truncating to `max_bytes` UTF-8 bytes.
fn strip_control_and_truncate(s: &str, max_bytes: usize) -> String {
    s.chars()
        .filter(|c| !is_control_or_bidi(*c))
        .collect::<String>()
        .chars()
        .take_while({
            let mut bytes = 0usize;
            move |c| {
                bytes += c.len_utf8();
                bytes <= max_bytes
            }
        })
        .collect()
}

/// Sanitize a readback string field: strip control/bidi characters first,
/// then redact if the normalized string looks like a secret.
///
/// SEC-P080-003: strip BEFORE secret detection so that obfuscated tokens
/// (hidden control/bidi chars inserted before prefix patterns) are correctly
/// revealed by normalization and then redacted rather than passed through.
/// operator_message is additionally truncated to 240 bytes.
fn sanitize_readback_string(key: &str, s: &str) -> String {
    let max_bytes = if key == "operator_message" { 240 } else { 4096 };
    let stripped = strip_control_and_truncate(s, max_bytes);
    if looks_like_secret(&stripped) {
        return "[redacted]".to_string();
    }
    stripped
}

/// Strip non-allow-listed keys from a parsed p080_readback_v1 object and
/// apply secret-pattern redaction to every string value.
///
/// SEC-HIGH-001 fix: all p080_readback_v1 fields are scalar types (string,
/// number, bool, null). An Object or Array value at any allowed key indicates
/// a malformed or tampered row. Such rows are replaced with a tamper-detected
/// sentinel rather than recursively preserving nested structure (which could
/// leak forbidden keys inside an allowed top-level field).
fn redact_readback(v: serde_json::Value) -> serde_json::Value {
    let obj = match v {
        serde_json::Value::Object(m) => m,
        _ => return serde_json::json!({}),
    };

    // P080-SEC-MED-002: reject rows with wrong or absent schema_version at egress.
    // Missing schema_version is treated as tamper_detected — a legitimate writer
    // always sets the exact contract value p080_readback_v1.
    match obj.get("schema_version") {
        Some(sv) if sv.as_str() == Some("p080_readback_v1") => {}
        _ => {
            return serde_json::json!({
                "schema_version": "p080_readback_v1",
                "projection_integrity": "tamper_detected",
                "operator_message": "[tamper_detected: unrecognised or absent schema_version]"
            });
        }
    }

    // Reject the entire row if any allowed key holds a non-scalar value.
    for key in READBACK_ALLOWED_KEYS {
        if let Some(val) = obj.get(*key) {
            if matches!(
                val,
                serde_json::Value::Object(_) | serde_json::Value::Array(_)
            ) {
                return serde_json::json!({
                    "schema_version": "p080_readback_v1",
                    "projection_integrity": "tamper_detected",
                    "operator_message": "[tamper_detected: non-scalar value in allowed readback field]"
                });
            }
        }
    }

    // P080-SEC-MED-002: validate repair_idempotency_key format.
    // Must be null for diagnose_only/delegated actions; otherwise must match
    // "p080-rik-" followed by exactly 24 lowercase hex characters.
    if let Some(rik) = obj.get("repair_idempotency_key") {
        if !rik.is_null() {
            if let Some(rik_str) = rik.as_str() {
                let valid = rik_str.starts_with("p080-rik-")
                    && rik_str.len() == 9 + 24
                    && rik_str[9..]
                        .chars()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
                if !valid {
                    return serde_json::json!({
                        "schema_version": "p080_readback_v1",
                        "projection_integrity": "tamper_detected",
                        "operator_message": "[tamper_detected: invalid repair_idempotency_key format]"
                    });
                }
            } else {
                // Non-string, non-null repair_idempotency_key is invalid.
                return serde_json::json!({
                    "schema_version": "p080_readback_v1",
                    "projection_integrity": "tamper_detected",
                    "operator_message": "[tamper_detected: repair_idempotency_key must be a string or null]"
                });
            }
        }
    }

    // SEC-HIGH-001: fail closed for invalid closed-vocabulary enum fields at MCP egress.
    // The DB write boundary enforces these at insert time, but defence-in-depth requires
    // the egress layer to also reject rows where a field holds an out-of-vocabulary value.
    type VocabEntry = (&'static str, &'static [&'static str]);
    const ENUM_VOCAB: &[VocabEntry] = &[
        ("stale_class", &[
            "warmup_pending", "acp_startup_stale", "scheduler_ownership_drift",
            "acp_prompt_stale", "helper_orphan_drift", "release_side_effect_drift",
            "ambiguous_owner", "useful", "unknown",
        ]),
        ("running_truth", &[
            "useful", "warmup_pending", "stale_suspected", "needs_operator",
            "needs_effect_reconciliation", "stale_repaired", "unknown",
        ]),
        ("hold_reason", &[
            "none", "cooldown_active", "permanent_hold_active", "ambiguous_owner",
            "side_effect_drift_unsafe", "dependency_read_failure",
            "gateway_saturated", "live_disable", "warmup_pending",
            "rollout_disabled", "unknown",
        ]),
        ("side_effect_status", &[
            "not_applicable", "retry_safe", "unsafe", "unknown",
        ]),
        ("repair_outcome", &[
            "success", "failed", "skipped", "not_attempted", "cooldown_active",
            "hold_active", "class_disabled", "rollout_disabled",
        ]),
    ];
    for (field, allowed) in ENUM_VOCAB {
        if let Some(serde_json::Value::String(s)) = obj.get(*field) {
            if !allowed.contains(&s.as_str()) {
                return serde_json::json!({
                    "schema_version": "p080_readback_v1",
                    "projection_integrity": "tamper_detected",
                    "operator_message": "[tamper_detected: enum field outside closed vocabulary in readback payload]"
                });
            }
        }
    }

    let mut out = serde_json::Map::new();
    for key in READBACK_ALLOWED_KEYS {
        if let Some(val) = obj.get(*key) {
            let sanitized = match val {
                serde_json::Value::String(s) => {
                    serde_json::Value::String(sanitize_readback_string(key, s))
                }
                other => other.clone(), // number, bool, null — pass through
            };
            out.insert(key.to_string(), sanitized);
        }
    }
    serde_json::Value::Object(out)
}

/// Maximum byte length for identifier fields supplied by callers.
const MAX_IDENTIFIER_BYTES: usize = 256;
/// Maximum byte length for filter string fields.
const MAX_FILTER_STRING_BYTES: usize = 128;

/// Validate and sanitize a caller-supplied identifier string (stage_id,
/// non-UUID work identifiers). Rejects strings that exceed MAX_IDENTIFIER_BYTES
/// or contain control/bidi characters; returns the sanitized value or None if invalid.
fn sanitize_identifier(s: &str) -> Option<String> {
    if s.is_empty() || s.len() > MAX_IDENTIFIER_BYTES {
        return None;
    }
    if s.chars().any(is_control_or_bidi) {
        return None;
    }
    Some(s.to_string())
}

/// Validate a caller-supplied UUID string (run_id, work_item_id).
/// Rejects strings that are not valid UUIDs in hyphenated form (xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx).
/// Returns the validated string (lowercased) or None if invalid.
fn sanitize_uuid_id(s: &str) -> Option<String> {
    match uuid::Uuid::parse_str(s) {
        Ok(u) => Some(u.to_string()),
        Err(_) => None,
    }
}

/// Sanitize a filter string: strip control/bidi, truncate to MAX_FILTER_STRING_BYTES.
/// Returns None if empty after stripping.
fn sanitize_filter_string(s: &str) -> Option<String> {
    let cleaned = strip_control_and_truncate(s, MAX_FILTER_STRING_BYTES);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// Compute a filter-stability hash binding a cursor to its issuing surface,
/// tool, query filter, and include_recent_repaired flag.  Including all filter
/// parameters in the hash prevents cross-preference cursor reuse where a cursor
/// issued with include_recent_repaired=true is replayed against a request with
/// include_recent_repaired=false (P080-SEC-MED-001).
fn compute_p080_filter_hash(
    filter: &db::repos::p080::ReadbackFilter,
    cursor_scope: &str,
    tool_name: &str,
    include_recent_repaired: bool,
) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(cursor_scope.as_bytes());
    h.update(b"\x00");
    h.update(tool_name.as_bytes());
    h.update(b"\x00");
    h.update(filter.run_id.as_deref().unwrap_or("").as_bytes());
    h.update(b"\x00");
    h.update(filter.stage_id.as_deref().unwrap_or("").as_bytes());
    h.update(b"\x00");
    h.update(filter.work_item_id.as_deref().unwrap_or("").as_bytes());
    h.update(b"\x00");
    h.update(filter.stale_class.as_deref().unwrap_or("").as_bytes());
    h.update(b"\x00");
    h.update(filter.hold_reason.as_deref().unwrap_or("").as_bytes());
    h.update(b"\x00");
    // include_recent_repaired is part of the filter contract; bind it so a cursor
    // issued with one preference cannot be replayed against the other.
    h.update(if include_recent_repaired { b"1" } else { b"0" });
    format!("{:x}", h.finalize())
}

const P080_MCP_CURSOR_SCOPE: &str = "mcp";
const P080_MCP_TOOL_NAME: &str = "p080.diagnostics.get.v1";

/// Encode a p080_cursor_v1: base64url-encoded JSON per the approved keyset contract.
///
/// Fields per the approved p080_cursor_v1 shape:
/// - cursor_version: 1
/// - cursor_scope: "mcp"
/// - tool_name: "p080.diagnostics.get.v1"
/// - filter_hash: sha256 hex binding surface + tool + filter params
/// - projection_generation: current max generation at page-issue time
/// - include_recent_repaired: matches the filter flag used for this page
/// - last_ordering_tuple: ordering key of the last row on this page
/// - expires_at: RFC3339 UTC, 1 hour from now
///
/// `offset` is intentionally absent; continuation uses the last_ordering_tuple keyset
/// bound so pages remain stable under projection rebuilds.
///
/// Returns `(cursor_token, expires_at_rfc3339)` so callers can populate the
/// `cursor_expires_at` field in `page_info` without re-decoding the token.
fn encode_p080_cursor(
    filter_hash: &str,
    projection_generation: i64,
    include_recent_repaired: bool,
    last_ordering_tuple: serde_json::Value,
) -> (String, String) {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let payload = serde_json::json!({
        "cursor_version": 1,
        "cursor_scope": P080_MCP_CURSOR_SCOPE,
        "tool_name": P080_MCP_TOOL_NAME,
        "filter_hash": filter_hash,
        "projection_generation": projection_generation,
        "include_recent_repaired": include_recent_repaired,
        "last_ordering_tuple": last_ordering_tuple,
        "expires_at": expires_at
    });
    let token = URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());
    (token, expires_at)
}

/// Decode and validate a p080_cursor_v1 issued by this MCP endpoint.
/// Returns the keyset anchor (`Some`) or None for first page (no cursor).
/// Uses the approved cursor_reason vocabulary: malformed, expired, filter_changed,
/// projection_generation_mismatch.
/// Cross-surface (cursor_scope mismatch) and wrong-tool (tool_name mismatch)
/// rejections use filter_changed per the approved P080 cursor_reason contract.
/// `current_projection_generation` is the generation fetched before pagination;
/// cursors issued at a stale generation are rejected with cursor_reason=projection_generation_mismatch.
fn decode_p080_cursor(
    cursor: Option<&str>,
    filter_hash: &str,
    current_projection_generation: i64,
) -> Result<Option<db::repos::p080::KeysetAfter>, serde_json::Value> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;

    let Some(cursor) = cursor else {
        return Ok(None);
    };
    if cursor.is_empty() {
        return Ok(None);
    }
    // Base64url without padding: max reasonable payload is ~512 JSON bytes → ~684 chars.
    if cursor.len() > 2048 {
        return Err(p080_error_detail(
            "invalid_cursor",
            "cursor exceeds maximum allowed length",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        ));
    }
    let bytes = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| {
        p080_error_detail(
            "invalid_cursor",
            "cursor is malformed",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    let json_str = std::str::from_utf8(&bytes).map_err(|_| {
        p080_error_detail(
            "invalid_cursor",
            "cursor is malformed",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    let data: serde_json::Value = serde_json::from_str(json_str).map_err(|_| {
        p080_error_detail(
            "invalid_cursor",
            "cursor is malformed",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    // Validate cursor_version: must be 1 (only version currently defined).
    // A recognised-but-wrong version uses cursor_reason=version_mismatch per the
    // approved P080 cursor invalidation vocabulary; missing/unparseable uses malformed.
    match data["cursor_version"].as_i64() {
        Some(1) => {}
        Some(_) => {
            return Err(p080_error_detail(
                "invalid_cursor",
                "cursor has an unrecognised cursor_version",
                serde_json::json!({ "cursor_reason": "version_mismatch" }),
                None,
            ));
        }
        None => {
            return Err(p080_error_detail(
                "invalid_cursor",
                "cursor is missing cursor_version",
                serde_json::json!({ "cursor_reason": "malformed" }),
                None,
            ));
        }
    }
    // Validate cursor_scope: must be "mcp".
    // Cross-surface cursor rejection uses filter_changed per the approved cursor_reason contract.
    if data["cursor_scope"].as_str() != Some(P080_MCP_CURSOR_SCOPE) {
        return Err(p080_error_detail(
            "invalid_cursor",
            "cursor was not issued for this surface",
            serde_json::json!({ "cursor_reason": "filter_changed" }),
            None,
        ));
    }
    // Validate tool_name: cursor must be used on the same tool.
    if data["tool_name"].as_str() != Some(P080_MCP_TOOL_NAME) {
        return Err(p080_error_detail(
            "invalid_cursor",
            "cursor was not issued for this tool",
            serde_json::json!({ "cursor_reason": "filter_changed" }),
            None,
        ));
    }
    // Validate projection_generation: cursor is invalidated when projection is rebuilt.
    if let Some(cursor_gen) = data["projection_generation"].as_i64() {
        if cursor_gen != current_projection_generation {
            return Err(p080_error_detail(
                "invalid_cursor",
                "cursor was issued at a different projection_generation; re-issue the query",
                serde_json::json!({ "cursor_reason": "projection_generation_mismatch" }),
                None,
            ));
        }
    }
    let expires_at_str = data["expires_at"].as_str().ok_or_else(|| {
        p080_error_detail(
            "invalid_cursor",
            "cursor is missing expires_at",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    let expires_at = chrono::DateTime::parse_from_rfc3339(expires_at_str).map_err(|_| {
        p080_error_detail(
            "invalid_cursor",
            "cursor has unparseable expires_at",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    if chrono::Utc::now() > expires_at.with_timezone(&chrono::Utc) {
        return Err(p080_error_detail(
            "invalid_cursor",
            "cursor has expired; re-issue the query from the beginning",
            serde_json::json!({ "cursor_reason": "expired" }),
            None,
        ));
    }
    let fh = data["filter_hash"].as_str().ok_or_else(|| {
        p080_error_detail(
            "invalid_cursor",
            "cursor is missing filter_hash",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    if fh != filter_hash {
        return Err(p080_error_detail(
            "invalid_cursor",
            "cursor was issued for different filter parameters",
            serde_json::json!({ "cursor_reason": "filter_changed" }),
            None,
        ));
    }
    // Extract last_ordering_tuple for keyset continuation.
    let lot = &data["last_ordering_tuple"];
    let proj_at = lot["projection_updated_at"].as_str().ok_or_else(|| {
        p080_error_detail(
            "invalid_cursor",
            "cursor last_ordering_tuple is missing projection_updated_at",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    let run_id = lot["run_id"].as_str().ok_or_else(|| {
        p080_error_detail(
            "invalid_cursor",
            "cursor last_ordering_tuple is missing run_id",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    let stage_id = lot["stage_id"].as_str().ok_or_else(|| {
        p080_error_detail(
            "invalid_cursor",
            "cursor last_ordering_tuple is missing stage_id",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    let work_item_id = lot["work_item_id"].as_str().ok_or_else(|| {
        p080_error_detail(
            "invalid_cursor",
            "cursor last_ordering_tuple is missing work_item_id",
            serde_json::json!({ "cursor_reason": "malformed" }),
            None,
        )
    })?;
    Ok(Some(db::repos::p080::KeysetAfter {
        projection_updated_at: proj_at.to_string(),
        run_id: run_id.to_string(),
        stage_id: stage_id.to_string(),
        work_item_id: work_item_id.to_string(),
    }))
}

/// SEC-P080-MED-003: Extract and validate filter fields from params.
///
/// If a filter field is present (non-null string) but fails validation
/// (empty, oversized, contains control/bidi chars), returns Err with an
/// `invalid_field` error response instead of silently converting to None
/// (which would broaden the query to all rows).
fn extract_filter(
    params: &serde_json::Value,
) -> Result<db::repos::p080::ReadbackFilter, serde_json::Value> {
    let f = &params["filter"];

    macro_rules! validate_id_field {
        ($field:literal) => {{
            match f[$field].as_str() {
                None => None, // absent or null → no filter on this field
                Some(s) => match sanitize_identifier(s) {
                    Some(clean) => Some(clean),
                    None => {
                        return Err(p080_error_detail(
                            "invalid_field",
                            &format!("filter.{} is present but invalid (empty, oversized, or contains control characters)", $field),
                            serde_json::json!({
                                "field_path": format!("filter.{}", $field),
                                "reason": "present but failed identifier validation"
                            }),
                            None,
                        ));
                    }
                },
            }
        }};
    }

    macro_rules! validate_uuid_field {
        ($field:literal) => {{
            match f[$field].as_str() {
                None => None,
                Some(s) => match sanitize_uuid_id(s) {
                    Some(clean) => Some(clean),
                    None => {
                        return Err(p080_error_detail(
                            "invalid_field",
                            &format!("filter.{} must be a valid UUID", $field),
                            serde_json::json!({
                                "field_path": format!("filter.{}", $field),
                                "reason": "not a valid UUID"
                            }),
                            None,
                        ));
                    }
                },
            }
        }};
    }

    macro_rules! validate_str_field {
        ($field:literal) => {{
            match f[$field].as_str() {
                None => None,
                Some(s) => match sanitize_filter_string(s) {
                    Some(clean) => Some(clean),
                    None => {
                        return Err(p080_error_detail(
                            "invalid_field",
                            &format!("filter.{} is present but empty after sanitization", $field),
                            serde_json::json!({
                                "field_path": format!("filter.{}", $field),
                                "reason": "present but empty after control-character stripping"
                            }),
                            None,
                        ));
                    }
                },
            }
        }};
    }

    Ok(db::repos::p080::ReadbackFilter {
        run_id: validate_id_field!("run_id"),
        stage_id: validate_id_field!("stage_id"),
        work_item_id: validate_id_field!("work_item_id"),
        stale_class: validate_str_field!("stale_class"),
        hold_reason: validate_str_field!("hold_reason"),
        include_recent_repaired: f["include_recent_repaired"].as_bool().unwrap_or(false),
    })
}

fn fallback_readback(
    run_id: &str,
    stage_id: &str,
    work_item_id: &str,
    stale_class: &str,
) -> serde_json::Value {
    let now = chrono::Utc::now().to_rfc3339();
    // SEC-P080-MED-001: sanitize caller-supplied strings before reflecting them.
    let safe_run_id = strip_control_and_truncate(run_id, MAX_IDENTIFIER_BYTES);
    let safe_stage_id = strip_control_and_truncate(stage_id, MAX_IDENTIFIER_BYTES);
    let safe_work_item_id = strip_control_and_truncate(work_item_id, MAX_IDENTIFIER_BYTES);
    let safe_stale_class = strip_control_and_truncate(stale_class, MAX_FILTER_STRING_BYTES);
    serde_json::json!({
        "schema_version": "p080_readback_v1",
        "run_id": safe_run_id,
        "stage_id": safe_stage_id,
        "work_item_id": safe_work_item_id,
        "stale_class": safe_stale_class,
        "running_truth": "unknown",
        "repair_action": "diagnose_only",
        "hold_reason": "none",
        "hold_age_seconds": null,
        "next_retry_or_backoff_time": null,
        "projection_updated_at": now,
        "projection_integrity": "stale",
        "executor_reregistration_state": "expected",
        "rollout_disablement": "phase_not_reached",
        "side_effect_status": "not_applicable",
        "operator_message": "",
        "evidence_marker_hash": null,
        "repair_idempotency_key": null
    })
}

/// Closed-schema enforcement: reject any top-level key not in `allowed`.
/// Returns a p080_error with code=unknown_field and detail.field_path/detail.reason,
/// or None if the request is schema-conformant.
fn check_unknown_fields(params: &serde_json::Value, allowed: &[&str]) -> Option<serde_json::Value> {
    if let Some(obj) = params.as_object() {
        for key in obj.keys() {
            if !allowed.contains(&key.as_str()) {
                return Some(p080_error_detail(
                    "unknown_field",
                    "request contains an unknown top-level field",
                    serde_json::json!({
                        "field_path": key,
                        "reason": "field is not defined in the p080 closed schema for this tool"
                    }),
                    None,
                ));
            }
        }
    }
    None
}

/// SEC-P080-MED-002: Closed-schema enforcement for a named nested object.
///
/// Checks a nested object at `params[field_name]` against `allowed_nested_keys`.
/// Returns a p080_error if any nested key is not in `allowed_nested_keys`, or if
/// the value is not an object (when present).
fn check_unknown_nested_fields(
    params: &serde_json::Value,
    field_name: &str,
    allowed_nested_keys: &[&str],
) -> Option<serde_json::Value> {
    let nested = &params[field_name];
    if nested.is_null() {
        return None; // absent or null — OK, field-presence enforcement is elsewhere
    }
    let obj = match nested.as_object() {
        Some(o) => o,
        None => {
            return Some(p080_error_detail(
                "invalid_field",
                &format!("field '{field_name}' must be an object"),
                serde_json::json!({
                    "field_path": field_name,
                    "reason": "expected object"
                }),
                None,
            ));
        }
    };
    for key in obj.keys() {
        if !allowed_nested_keys.contains(&key.as_str()) {
            return Some(p080_error_detail(
                "unknown_field",
                &format!("'{field_name}' contains an unknown field"),
                serde_json::json!({
                    "field_path": format!("{field_name}.{key}"),
                    "reason": "field is not defined in the p080 closed schema for this nested object"
                }),
                None,
            ));
        }
    }
    None
}

/// Map a `check_p080_run_scope` error message to the approved P080 error code.
///
/// Build a p080 error response per proposal §6.2 (L296-321).
fn p080_error_detail(
    code: &str,
    message: &str,
    detail: serde_json::Value,
    readback: Option<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "p080_error_response_v1",
        "code": code,
        "message": message,
        "retry_after": null,
        "readback": readback,
        "detail": detail
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SEC-HIGH-002: live_disable kill switch fail-closed tests ─────────────

    /// Missing live_disable row → P080 tools must return live_disabled (fail-closed).
    ///
    /// SEC-HIGH-002 fix: live_disable now runs inside each handler, AFTER schema/action
    /// extraction and auth checks. Tests provide valid schema_version so the live_disable
    /// path is actually reached. The error code is `live_disabled` (not `rollout_disabled`)
    /// per proposal line 610.
    #[tokio::test]
    async fn p080_missing_live_disable_row_returns_live_disabled() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Note: no seed — live_disable row intentionally absent.
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

        // diagnostics.get: provide valid schema_version to reach the live_disable check.
        let diag = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({"schema_version": "p080_diagnostics_get_request_v1"}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(
            diag["schema_version"], "p080_error_response_v1",
            "diagnostics.get: absent live_disable must return error response"
        );
        assert_eq!(
            diag["code"], "live_disabled",
            "diagnostics.get: absent live_disable must return live_disabled"
        );
        assert_eq!(
            diag["detail"]["rollout_disablement"], "live_disabled",
            "diagnostics.get: detail must identify live_disabled disablement"
        );

        // reconcile.request: provide schema_version + diagnose_only action so the Operator
        // principal passes auth checks and reaches the live_disable gate.
        let recon = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "r", "stage_id": "s", "work_item_id": "w",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(
            recon["schema_version"], "p080_error_response_v1",
            "reconcile.request: absent live_disable must return error response"
        );
        assert_eq!(
            recon["code"], "live_disabled",
            "reconcile.request: absent live_disable must return live_disabled"
        );
        assert_eq!(
            recon["detail"]["rollout_disablement"], "live_disabled",
            "reconcile.request: detail must identify live_disabled disablement"
        );

        // clear_permanent_hold: DEFECT-003 fix — schema_version is validated FIRST.
        // Missing schema_version → unsupported_version (not live_disabled), because the
        // version check runs before the rollout gate per proposal ordering lines 131-139.
        let cph_no_schema = execute(
            "p080.clear_permanent_hold.v1",
            serde_json::json!({}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(
            cph_no_schema["schema_version"], "p080_error_response_v1",
            "clear_permanent_hold: missing schema_version must return error response"
        );
        assert_eq!(cph_no_schema["code"], "unsupported_version",
            "clear_permanent_hold: missing schema_version must return unsupported_version before live_disable");

        // clear_permanent_hold with valid schema_version → reaches live_disable gate.
        let cph = execute(
            "p080.clear_permanent_hold.v1",
            serde_json::json!({"schema_version": "p080_clear_permanent_hold_request_v1"}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(
            cph["schema_version"], "p080_error_response_v1",
            "clear_permanent_hold: absent live_disable must return error response"
        );
        assert_eq!(
            cph["code"], "live_disabled",
            "clear_permanent_hold: absent live_disable must return live_disabled"
        );
        assert_eq!(
            cph["detail"]["rollout_disablement"], "live_disabled",
            "clear_permanent_hold: detail must identify live_disabled disablement"
        );
    }

    /// Deleted live_disable row after initial seeding → must still fail closed.
    #[tokio::test]
    async fn p080_deleted_live_disable_row_fails_closed() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed then delete the live_disable row to simulate runtime deletion.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM p080_rollout_control_v1 WHERE class = 'live_disable'")
            .execute(&pool)
            .await
            .unwrap();

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({"schema_version": "p080_diagnostics_get_request_v1"}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        // Error code is live_disabled per proposal line 610 (not rollout_disabled).
        assert_eq!(result["code"], "live_disabled");
        assert_eq!(result["detail"]["rollout_disablement"], "live_disabled");
    }

    /// When detection_only is disabled (Phase 0), diagnostics.get must return rollout_disabled.
    ///
    /// SEC-HIGH-002 fix: must seed live_disable (disabled) so the detection_only check is
    /// reached. Without seeding, live_disable is absent → live_disabled (different code).
    #[tokio::test]
    async fn p080_diagnostics_get_returns_rollout_disabled_in_phase_0() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed all rollout control rows (live_disable disabled by default).
        // detection_only is also seeded as disabled — this is Phase 0.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({"schema_version": "p080_diagnostics_get_request_v1"}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        // live_disable passes (disabled) → detection_only not enabled → rollout_disabled.
        assert_eq!(result["schema_version"], "p080_error_response_v1");
        assert_eq!(result["code"], "rollout_disabled");
    }

    /// When detection_only is enabled, diagnostics.get returns an empty page (no stale rows).
    #[tokio::test]
    async fn p080_diagnostics_get_returns_empty_page_when_detection_only_enabled() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed all classes including live_disable (disabled) first.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // Then enable detection_only.
        sqlx::query(
            "INSERT OR REPLACE INTO p080_rollout_control_v1 \
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason) \
             VALUES ('detection_only', 1, 'phase_1', 1, '2026-01-01T00:00:00Z', 'system', 'startup_seed')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({"schema_version": "p080_diagnostics_get_request_v1"}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_diagnostics_get_response_v1");
        // No stale rows yet → empty items; empty is valid, not stale.
        assert_eq!(result["items"].as_array().unwrap().len(), 0);
        assert_eq!(result["projection_integrity"], "valid");
        // Without request_total_count=true, total_count_exact must be null.
        assert!(
            result["page_info"]["total_count_exact"].is_null(),
            "total_count_exact must be null when request_total_count is absent"
        );
    }

    /// request_total_count=true returns exact count; false/absent returns null.
    #[tokio::test]
    async fn p080_diagnostics_get_total_count_honors_request_flag() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT OR REPLACE INTO p080_rollout_control_v1 \
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason) \
             VALUES ('detection_only', 1, 'phase_1', 1, '2026-01-01T00:00:00Z', 'system', 'startup_seed')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

        // request_total_count absent → null
        let r1 = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({"schema_version": "p080_diagnostics_get_request_v1"}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert!(
            r1["page_info"]["total_count_exact"].is_null(),
            "absent → null"
        );

        // request_total_count=false → null
        let r2 = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({"schema_version": "p080_diagnostics_get_request_v1", "request_total_count": false}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert!(
            r2["page_info"]["total_count_exact"].is_null(),
            "false → null"
        );

        // request_total_count=true → numeric count (0 here, no rows)
        let r3 = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({"schema_version": "p080_diagnostics_get_request_v1", "request_total_count": true}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(
            r3["page_info"]["total_count_exact"], 0,
            "true → exact count"
        );
    }

    /// diagnostics.get must be strictly read-only: calling it must never insert
    /// or modify rows in p080_readback_heartbeats_v1, even when detection_only is enabled.
    #[tokio::test]
    async fn p080_diagnostics_get_is_read_only() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");

        // Seed all classes including live_disable (disabled) first.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // Enable detection_only rollout control — this previously triggered writes.
        sqlx::query(
            "INSERT OR REPLACE INTO p080_rollout_control_v1 \
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason) \
             VALUES ('detection_only', 1, 'phase_1', 1, '2026-01-01T00:00:00Z', 'system', 'startup_seed')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Pre-seed a readback row with projection_generation=1.
        let now_str = chrono::Utc::now().to_rfc3339();
        let readback_json = r#"{"schema_version":"p080_readback_v1","run_id":"r","stage_id":"s","work_item_id":"w","stale_class":"acp_startup_stale","running_truth":"stale_suspected"}"#;
        sqlx::query(
            "INSERT INTO p080_readback_heartbeats_v1 \
             (run_id, stage_id, work_item_id, stale_class, projection_generation, \
              projection_updated_at, projection_integrity, readback_json, updated_at) \
             VALUES ('r', 's', 'w', 'acp_startup_stale', 1, ?1, 'valid', ?2, ?3)",
        )
        .bind(&now_str)
        .bind(readback_json)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({"schema_version": "p080_diagnostics_get_request_v1"}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_diagnostics_get_response_v1");

        // Verify the readback row projection_generation was NOT incremented.
        // An increment would indicate the classifier wrote to the table, violating read-only.
        let gen: i64 = sqlx::query_scalar(
            "SELECT projection_generation FROM p080_readback_heartbeats_v1 WHERE run_id='r'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(gen, 1, "diagnostics.get must not modify the readback table");
    }

    #[tokio::test]
    async fn p080_schema_version_required_returns_error() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed live_disable as disabled so tests reach schema validation.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);

        // Missing schema_version on diagnostics.get must return unsupported_version.
        let diag = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(diag["schema_version"], "p080_error_response_v1");
        assert_eq!(diag["code"], "unsupported_version");

        // Missing schema_version on reconcile.request must return unsupported_version.
        let recon = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "target": {
                    "run_id": "r", "stage_id": "s", "work_item_id": "w",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(recon["schema_version"], "p080_error_response_v1");
        assert_eq!(recon["code"], "unsupported_version");
    }

    #[tokio::test]
    async fn p080_reconcile_diagnose_only_returns_diagnosed() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // DEFECT-5 fix: diagnose_only now respects the detection_only rollout gate,
        // so the test must promote detection_only to enabled before exercising the
        // success path.
        db::repos::p080::set_rollout_control(
            &pool,
            "detection_only",
            true,
            "operator_change",
            "test-fixture",
        )
        .await
        .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "00000000-0000-0000-0000-000000000001",
                    "stage_id": "stage-001",
                    "work_item_id": "00000000-0000-0000-0000-000000000002",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_reconcile_response_v1");
        assert_eq!(result["decision"], "diagnosed");
    }

    /// DEFECT-5: diagnose_only must refuse when detection_only is not enabled,
    /// returning rollout_disabled with class_disabled disablement detail.
    #[tokio::test]
    async fn p080_reconcile_diagnose_only_gated_by_detection_only() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seeded rollout control leaves detection_only=disabled by default.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "00000000-0000-0000-0000-000000000001",
                    "stage_id": "stage-001",
                    "work_item_id": "00000000-0000-0000-0000-000000000002",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(
            result["schema_version"], "p080_error_response_v1",
            "diagnose_only must return error when detection_only is disabled"
        );
        assert_eq!(
            result["code"], "rollout_disabled",
            "diagnose_only with detection_only disabled must return rollout_disabled"
        );
        assert_eq!(
            result["detail"]["rollout_disablement"], "class_disabled",
            "diagnose_only with detection_only disabled must surface class_disabled"
        );
    }

    /// Phase 1: repair_if_safe always returns action_disabled_in_phase regardless of rollout.
    #[tokio::test]
    async fn p080_reconcile_repair_if_safe_returns_rollout_disabled() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed live_disable (disabled) so dispatch proceeds past the kill switch.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-001",
                    "stage_id": "stage-001",
                    "work_item_id": "wi-001",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": "dedup-key-001"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_error_response_v1");
        // acp_startup_stale is seeded with enabled=false → class_disabled.
        assert_eq!(result["code"], "class_disabled");
        assert_eq!(result["detail"]["rollout_disablement"], "class_disabled");
    }

    #[tokio::test]
    async fn p080_diagnose_only_rejects_dedup_key() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-001",
                    "stage_id": "stage-001",
                    "work_item_id": "wi-001",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only",
                "operator_request_dedup_key": "should-be-rejected"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_error_response_v1");
        assert_eq!(result["code"], "invalid_field");
    }

    #[tokio::test]
    async fn p080_clear_permanent_hold_returns_action_disabled() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.clear_permanent_hold.v1",
            serde_json::json!({
                "schema_version": "p080_clear_permanent_hold_request_v1",
                "target": {
                    "run_id": "run-001",
                    "stage_id": "stage-001",
                    "work_item_id": "wi-001",
                    "stale_class": "acp_startup_stale"
                },
                "operator_request_dedup_key": "dedup-key-001"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_error_response_v1");
        assert_eq!(result["code"], "action_disabled_in_phase");
    }

    #[tokio::test]
    async fn p080_clear_permanent_hold_rechecks_principal_locally() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal =
            auth::Principal::new("test-readonly", auth::PrincipalClass::ReadOnlyOperator);
        let result = execute(
            "p080.clear_permanent_hold.v1",
            serde_json::json!({
                "schema_version": "p080_clear_permanent_hold_request_v1",
                "target": {
                    "run_id": "run-001",
                    "stage_id": "stage-001",
                    "work_item_id": "wi-001",
                    "stale_class": "acp_startup_stale"
                },
                "operator_request_dedup_key": "dedup-key-001"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_error_response_v1");
        assert_eq!(result["code"], "unauthorized_missing_capability");
        assert_eq!(
            result["detail"]["required_capability"],
            "p080:clear_permanent_hold"
        );
    }

    // ── SEC-HIGH-002: redaction negative tests ────────────────────────────────

    /// diagnose_only must redact a readback containing a bearer token.
    #[tokio::test]
    async fn p080_diagnose_only_redacts_bearer_token_in_readback() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed live_disable (disabled) so dispatch proceeds.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // DEFECT-5 fix: diagnose_only now requires detection_only=enabled.
        db::repos::p080::set_rollout_control(
            &pool,
            "detection_only",
            true,
            "operator_change",
            "test",
        )
        .await
        .unwrap();

        let now_str = chrono::Utc::now().to_rfc3339();
        // Inject a readback row with a bearer token inside operator_message.
        let poisoned_json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "run_id": "00000000-0000-0000-0000-000000000003", "stage_id": "s-secret",
            "work_item_id": "00000000-0000-0000-0000-000000000004",
            "stale_class": "acp_startup_stale",
            "running_truth": "stale_suspected",
            "repair_action": "diagnose_only",
            "hold_reason": "rollout_disabled",
            "projection_updated_at": now_str,
            "projection_integrity": "valid",
            "executor_reregistration_state": "expected",
            "operator_message": "Bearer sk-proj-abc123secrettoken9876",
            "evidence_marker_hash": null,
            "repair_idempotency_key": null
        }))
        .unwrap();
        sqlx::query(
            "INSERT INTO p080_readback_heartbeats_v1
             (run_id, stage_id, work_item_id, stale_class, projection_generation,
              projection_updated_at, projection_integrity, readback_json, updated_at)
             VALUES ('00000000-0000-0000-0000-000000000003', 's-secret',
                     '00000000-0000-0000-0000-000000000004', 'acp_startup_stale', 1,
                     ?1, 'valid', ?2, ?3)",
        )
        .bind(&now_str)
        .bind(&poisoned_json)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "00000000-0000-0000-0000-000000000003",
                    "stage_id": "s-secret",
                    "work_item_id": "00000000-0000-0000-0000-000000000004",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_reconcile_response_v1");
        // operator_message must be redacted — must not contain the bearer token.
        let op_msg = result["readback"]["operator_message"]
            .as_str()
            .unwrap_or("");
        assert_eq!(op_msg, "[redacted]", "bearer token must be redacted");
        assert!(!op_msg.contains("sk-proj"), "raw token must not appear");
    }

    /// diagnose_only must strip forbidden keys not in the allow-list.
    #[tokio::test]
    async fn p080_diagnose_only_strips_forbidden_keys() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // DEFECT-5 fix: diagnose_only now requires detection_only=enabled.
        db::repos::p080::set_rollout_control(
            &pool,
            "detection_only",
            true,
            "operator_change",
            "test",
        )
        .await
        .unwrap();

        let now_str = chrono::Utc::now().to_rfc3339();
        let poisoned_json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "run_id": "00000000-0000-0000-0000-000000000005", "stage_id": "s-key",
            "work_item_id": "00000000-0000-0000-0000-000000000006",
            "stale_class": "acp_startup_stale",
            "running_truth": "stale_suspected",
            "repair_action": "diagnose_only",
            "hold_reason": "rollout_disabled",
            "projection_updated_at": now_str,
            "projection_integrity": "valid",
            "executor_reregistration_state": "expected",
            "operator_message": "",
            "_internal_token": "super-secret-value",
            "provider_api_key": "FORBIDDEN_KEY=shouldnotappear",
            "evidence_marker_hash": null,
            "repair_idempotency_key": null
        }))
        .unwrap();
        sqlx::query(
            "INSERT INTO p080_readback_heartbeats_v1
             (run_id, stage_id, work_item_id, stale_class, projection_generation,
              projection_updated_at, projection_integrity, readback_json, updated_at)
             VALUES ('00000000-0000-0000-0000-000000000005', 's-key',
                     '00000000-0000-0000-0000-000000000006', 'acp_startup_stale', 1,
                     ?1, 'valid', ?2, ?3)",
        )
        .bind(&now_str)
        .bind(&poisoned_json)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "00000000-0000-0000-0000-000000000005", "stage_id": "s-key",
                    "work_item_id": "00000000-0000-0000-0000-000000000006",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_reconcile_response_v1");
        let readback = &result["readback"];
        assert!(
            readback["_internal_token"].is_null(),
            "forbidden key must be stripped"
        );
        assert!(
            readback["provider_api_key"].is_null(),
            "forbidden key must be stripped"
        );
    }

    /// SEC-HIGH-001: a readback row where an allowed key holds a nested Object
    /// must return tamper_detected rather than preserving the nested structure.
    ///
    /// A malformed row with e.g. `operator_message: {"principal_id": "secret"}`
    /// would previously pass the allow-list check (operator_message is allowed)
    /// and emit the nested object through the MCP response. After the fix,
    /// any non-scalar value in an allowed field triggers a tamper-detected sentinel.
    #[tokio::test]
    async fn p080_diagnostics_get_rejects_nested_object_in_allowed_field() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT OR REPLACE INTO p080_rollout_control_v1 \
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason) \
             VALUES ('detection_only', 1, 'phase_1', 1, '2026-01-01T00:00:00Z', 'system', 'operator_change')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let now_str = chrono::Utc::now().to_rfc3339();
        // Inject a tampered row: operator_message holds an Object (not a string).
        let tampered_json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "run_id": "r-tamper", "stage_id": "s-tamper", "work_item_id": "w-tamper",
            "stale_class": "acp_startup_stale",
            "running_truth": "stale_suspected",
            "repair_action": "diagnose_only",
            "hold_reason": "rollout_disabled",
            "projection_updated_at": now_str,
            "projection_integrity": "valid",
            "executor_reregistration_state": "expected",
            "operator_message": {"principal_id": "super-secret-id", "argv": ["/bin/sh"]},
            "evidence_marker_hash": null,
            "repair_idempotency_key": null
        }))
        .unwrap();
        sqlx::query(
            "INSERT INTO p080_readback_heartbeats_v1
             (run_id, stage_id, work_item_id, stale_class, projection_generation,
              projection_updated_at, projection_integrity, readback_json, updated_at)
             VALUES ('r-tamper', 's-tamper', 'w-tamper', 'acp_startup_stale', 1,
                     ?1, 'valid', ?2, ?3)",
        )
        .bind(&now_str)
        .bind(&tampered_json)
        .bind(&now_str)
        .execute(&pool)
        .await
        .unwrap();

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({"schema_version": "p080_diagnostics_get_request_v1"}),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_diagnostics_get_response_v1");
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        let readback = &items[0]["readback"];
        // Must return tamper_detected sentinel — not the nested object.
        assert_eq!(
            readback["projection_integrity"], "tamper_detected",
            "nested object in allowed field must trigger tamper_detected"
        );
        // The nested principal_id must NOT appear in the response.
        assert!(
            readback["operator_message"].as_object().is_none(),
            "operator_message must not be an object in the response"
        );
        let op_msg_str = readback["operator_message"].as_str().unwrap_or("");
        assert!(
            !op_msg_str.contains("super-secret-id"),
            "principal_id must not appear in operator_message"
        );
        assert!(
            !op_msg_str.contains("argv"),
            "argv must not appear in operator_message"
        );
    }

    // ── ReadOnlyOperator tests ─────────────────────────────────────────────────

    /// ReadOnlyOperator with an explicit run_scope can call diagnose_only at the handler level.
    /// Note: at the MCP dispatch level, auth.rs restricts P080ReconcileRequest to Operator;
    /// this test verifies the handler itself allows diagnose_only for a scoped principal
    /// whose run_scope includes the target run_id.
    #[tokio::test]
    async fn p080_read_only_operator_can_call_diagnose_only() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed all rollout control rows (including live_disable as disabled).
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // Also enable detection_only so diagnostics route proceeds.
        sqlx::query(
            "INSERT OR REPLACE INTO p080_rollout_control_v1 \
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason) \
             VALUES ('detection_only', 1, 'phase_1', 1, '2026-01-01T00:00:00Z', 'system', 'startup_seed')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Scoped principal: run_scope includes the target run_id (SEC-P080-001).
        let mut principal =
            auth::Principal::new("ro-operator", auth::PrincipalClass::ReadOnlyOperator);
        principal.run_scope = Some(vec!["00000000-0000-0000-0000-000000000007".to_string()]);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "00000000-0000-0000-0000-000000000007",
                    "stage_id": "stage-ro-01",
                    "work_item_id": "00000000-0000-0000-0000-000000000008",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_reconcile_response_v1");
        assert_eq!(result["decision"], "diagnosed");
    }

    /// ReadOnlyOperator calling repair_if_safe must get authorization_denied before any
    /// phase check — auth precedence fix (DEFECT-2).
    #[tokio::test]
    async fn p080_read_only_operator_rejected_for_repair_if_safe() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed live_disable (disabled) so dispatch proceeds to auth check.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("ro-operator", auth::PrincipalClass::ReadOnlyOperator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-ro-02",
                    "stage_id": "stage-ro-02",
                    "work_item_id": "wi-ro-02",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": "dedup-ro-02"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_error_response_v1");
        // Approved vocabulary: non-Operator calling repair_if_safe → unauthorized_missing_capability.
        assert_eq!(result["code"], "unauthorized_missing_capability");
        assert_eq!(result["detail"]["required_capability"], "p080:repair");
    }

    /// SEC-HIGH-002: ReadOnlyOperator calling repair_if_safe must receive
    /// unauthorized_missing_capability even when live_disable is ON.
    /// This verifies that rollout-control state cannot be inferred from the
    /// error code returned to an action-unauthorized caller.
    #[tokio::test]
    async fn p080_read_only_operator_cannot_infer_live_disable_state() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed rollout control, then enable live_disable (active kill switch).
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE p080_rollout_control_v1 SET enabled = 1 WHERE class = 'live_disable'")
            .execute(&pool)
            .await
            .unwrap();

        let principal = auth::Principal::new("ro-operator", auth::PrincipalClass::ReadOnlyOperator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-ro-sec002",
                    "stage_id": "stage-ro-sec002",
                    "work_item_id": "wi-ro-sec002",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": "dedup-ro-sec002"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        // Must get auth error, NOT live_disabled — rollout state must not leak.
        assert_eq!(result["code"], "unauthorized_missing_capability",
            "ReadOnlyOperator+repair_if_safe must return auth error regardless of live_disable state");
        assert!(
            result["code"] != "live_disabled",
            "live_disable state must not be exposed to action-unauthorized caller"
        );
    }

    /// P080-MCP-AUTHZ-001 regression: a scoped ReadOnlyOperator calling diagnose_only
    /// with an unauthorized run_id must receive unauthorized_missing_capability, NOT
    /// live_disabled — live_disable state must not leak before run_scope is verified.
    #[tokio::test]
    async fn p080_scoped_read_only_operator_diagnose_only_unauthorized_run_id_hides_live_disable() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // Enable live_disable (global kill-switch) — unauthorized callers must NOT learn this.
        sqlx::query("UPDATE p080_rollout_control_v1 SET enabled = 1 WHERE class = 'live_disable'")
            .execute(&pool)
            .await
            .unwrap();

        let mut principal =
            auth::Principal::new("ro-operator-scoped", auth::PrincipalClass::ReadOnlyOperator);
        // Scope only allows "run-authorized"; request targets "run-unauthorized".
        principal.run_scope = Some(vec!["run-authorized".to_string()]);

        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-unauthorized",
                    "stage_id": "stage-sec",
                    "work_item_id": "wi-sec",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(
            result["code"],
            "unauthorized_missing_capability",
            "scoped ReadOnlyOperator+diagnose_only with out-of-scope run_id must return auth error, not live_disabled"
        );
        assert_ne!(
            result["code"],
            "live_disabled",
            "live_disable state must not be exposed to unauthorized callers"
        );
    }

    /// P080-MCP-AUTHZ-001 regression: same as above but with detection_only disabled.
    /// Scoped ReadOnlyOperator with unauthorized run_id must get auth error, not rollout_disabled.
    #[tokio::test]
    async fn p080_scoped_read_only_operator_diagnose_only_unauthorized_run_id_hides_detection_state()
    {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // detection_only is seeded as disabled; live_disable is disabled too.
        // Unauthorized callers must not infer detection_only state.

        let mut principal =
            auth::Principal::new("ro-operator-scoped2", auth::PrincipalClass::ReadOnlyOperator);
        principal.run_scope = Some(vec!["run-authorized".to_string()]);

        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-unauthorized",
                    "stage_id": "stage-sec",
                    "work_item_id": "wi-sec",
                    "stale_class": "warmup_pending"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(
            result["code"],
            "unauthorized_missing_capability",
            "scoped ReadOnlyOperator+diagnose_only with out-of-scope run_id must return auth error, not rollout_disabled"
        );
        assert_ne!(
            result["code"],
            "rollout_disabled",
            "detection_only rollout state must not be exposed to unauthorized callers"
        );
    }

    // ── DEFECT-5: required field validation negative tests ───────────────────

    /// diagnose_only with missing target.run_id must return invalid_field.
    #[tokio::test]
    async fn p080_diagnose_only_missing_run_id_returns_invalid_field() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // DEFECT-5 fix: enable detection_only so the field-validation path is reached.
        db::repos::p080::set_rollout_control(
            &pool,
            "detection_only",
            true,
            "operator_change",
            "test",
        )
        .await
        .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "stage_id": "stage-001",
                    "work_item_id": "wi-001",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["code"], "invalid_field");
        assert_eq!(result["detail"]["field_path"], "target.run_id");
    }

    /// diagnose_only with unknown stale_class must return invalid_field.
    #[tokio::test]
    async fn p080_diagnose_only_unknown_stale_class_returns_invalid_field() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        // DEFECT-5 fix: enable detection_only so the field-validation path is reached.
        db::repos::p080::set_rollout_control(
            &pool,
            "detection_only",
            true,
            "operator_change",
            "test",
        )
        .await
        .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "00000000-0000-0000-0000-000000000001",
                    "stage_id": "stage-001",
                    "work_item_id": "00000000-0000-0000-0000-000000000002",
                    "stale_class": "not_a_valid_class"
                },
                "requested_action": "diagnose_only"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["code"], "invalid_field");
        assert_eq!(result["detail"]["field_path"], "target.stale_class");
    }

    // ── Phase 1 gate tests for repair_if_safe ────────────────────────────────

    /// Phase 1: repair_if_safe returns action_disabled_in_phase even when rollout disabled.
    #[tokio::test]
    async fn p080_repair_if_safe_rollout_disabled_returns_rollout_disabled() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed with enabled=0 (default).
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-p2-01",
                    "stage_id": "stage-p2-01",
                    "work_item_id": "wi-p2-01",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": "dedup-p2-01"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_error_response_v1");
        // acp_startup_stale is seeded with enabled=false → class_disabled.
        assert_eq!(result["code"], "class_disabled");
        assert_eq!(result["detail"]["rollout_disablement"], "class_disabled");
    }

    async fn enable_rollout_class(pool: &sqlx::SqlitePool, class: &str) {
        sqlx::query(
            "INSERT OR REPLACE INTO p080_rollout_control_v1
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason)
             VALUES (?1, 1, 'phase_2', 1, '2026-01-01T00:00:00Z', 'system', 'operator_change')",
        )
        .bind(class)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn insert_stale_readback(
        pool: &sqlx::SqlitePool,
        run_id: &str,
        stage_id: &str,
        work_item_id: &str,
    ) {
        let now_str = chrono::Utc::now().to_rfc3339();
        let readback_json = serde_json::to_string(&serde_json::json!({
            "schema_version": "p080_readback_v1",
            "run_id": run_id, "stage_id": stage_id, "work_item_id": work_item_id,
            "stale_class": "acp_startup_stale", "running_truth": "stale_suspected",
            "repair_action": "diagnose_only", "hold_reason": "rollout_disabled",
            "hold_age_seconds": null, "next_retry_or_backoff_time": null,
            "projection_updated_at": now_str, "projection_integrity": "valid",
            "executor_reregistration_state": "expected", "rollout_disablement": "phase_not_reached",
            "side_effect_status": "not_applicable", "operator_message": "",
            "evidence_marker_hash": null, "repair_idempotency_key": null
        }))
        .unwrap();
        sqlx::query(
            "INSERT INTO p080_readback_heartbeats_v1
             (run_id, stage_id, work_item_id, stale_class, projection_generation,
              projection_updated_at, projection_integrity, readback_json, updated_at)
             VALUES (?1, ?2, ?3, 'acp_startup_stale', 1, ?4, 'valid', ?5, ?6)",
        )
        .bind(run_id)
        .bind(stage_id)
        .bind(work_item_id)
        .bind(&now_str)
        .bind(&readback_json)
        .bind(&now_str)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn p080_diagnostics_get_cursor_pages_forward() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "detection_only").await;
        insert_stale_readback(&pool, "run-page-01", "stage-page-01", "wi-page-01").await;
        insert_stale_readback(&pool, "run-page-02", "stage-page-02", "wi-page-02").await;

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let first = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "page_size": 1
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(first["items"].as_array().unwrap().len(), 1);
        assert_eq!(first["page_info"]["has_next_page"], true);
        let cursor = first["page_info"]["next_cursor"]
            .as_str()
            .expect("next cursor")
            .to_string();

        let second = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "page_size": 1,
                "cursor": cursor
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(second["items"].as_array().unwrap().len(), 1);
        assert_eq!(second["page_info"]["has_next_page"], false);
        assert_ne!(
            first["items"][0]["readback"]["run_id"], second["items"][0]["readback"]["run_id"],
            "cursor must advance to the next row"
        );
    }

    /// Opaque cursor: an arbitrary non-hex string is rejected as malformed.
    /// (cursor_reason: malformed replaces the prior unsupported_cursor_version vocabulary)
    #[tokio::test]
    async fn p080_diagnostics_get_rejects_malformed_cursor() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "detection_only").await;

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "cursor": "not-valid-hex"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(result["schema_version"], "p080_error_response_v1");
        assert_eq!(result["code"], "invalid_cursor");
        assert_eq!(result["detail"]["cursor_reason"], "malformed");
    }

    /// Cross-surface cursor (cursor_scope != "mcp") must be rejected with cursor_reason: filter_changed.
    #[tokio::test]
    async fn p080_diagnostics_get_rejects_cross_surface_cursor_with_filter_changed() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "detection_only").await;

        // Build a cursor with cursor_scope="graphql" (wrong surface).
        let payload = serde_json::json!({
            "cursor_version": 1,
            "cursor_scope": "graphql",
            "tool_name": "p080.diagnostics.get.v1",
            "filter_hash": "aaaa",
            "projection_generation": 1,
            "offset": 0,
            "expires_at": (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339()
        });
        let cross_surface_cursor = URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "cursor": cross_surface_cursor
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(result["schema_version"], "p080_error_response_v1");
        assert_eq!(result["code"], "invalid_cursor");
        assert_eq!(
            result["detail"]["cursor_reason"], "filter_changed",
            "cross-surface cursor must use filter_changed, not version_mismatch"
        );
    }

    /// Wrong-tool cursor (tool_name != expected) must be rejected with cursor_reason: filter_changed.
    #[tokio::test]
    async fn p080_diagnostics_get_rejects_wrong_tool_cursor_with_filter_changed() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "detection_only").await;

        // Build a cursor with wrong tool_name.
        let payload = serde_json::json!({
            "cursor_version": 1,
            "cursor_scope": "mcp",
            "tool_name": "p080.reconcile.request.v1",
            "filter_hash": "aaaa",
            "projection_generation": 1,
            "offset": 0,
            "expires_at": (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339()
        });
        let wrong_tool_cursor = URL_SAFE_NO_PAD.encode(payload.to_string().as_bytes());

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "cursor": wrong_tool_cursor
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(result["schema_version"], "p080_error_response_v1");
        assert_eq!(result["code"], "invalid_cursor");
        assert_eq!(
            result["detail"]["cursor_reason"], "filter_changed",
            "wrong-tool cursor must use filter_changed, not version_mismatch"
        );
    }

    /// Phase 1: repair_if_safe returns action_disabled_in_phase even when rollout is enabled.
    /// Operator gets action_disabled_in_phase; auth check fires before phase check.
    #[tokio::test]
    async fn p080_repair_if_safe_rollout_enabled_stale_returns_diagnosed() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        // Seed all classes including live_disable (disabled) first.
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "acp_startup_stale").await;
        enable_rollout_class(&pool, "detection_only").await;
        insert_stale_readback(&pool, "run-p2-02", "stage-p2-02", "wi-p2-02").await;

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-p2-02",
                    "stage_id": "stage-p2-02",
                    "work_item_id": "wi-p2-02",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": "dedup-p2-02"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        // acp_startup_stale is enabled but repair phase not yet reached → rollout_disabled.
        assert_eq!(result["schema_version"], "p080_error_response_v1");
        assert_eq!(result["code"], "rollout_disabled");
        assert_eq!(result["detail"]["rollout_disablement"], "phase_not_reached");
    }

    /// Phase 1: repair_if_safe consistently returns rollout_disabled on every call
    /// when the class is enabled.  The dedup/fingerprint fence only applies in Phase 2+.
    #[tokio::test]
    async fn p080_repair_if_safe_dedup_replay_returns_same_response() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "acp_startup_stale").await;
        enable_rollout_class(&pool, "detection_only").await;
        insert_stale_readback(&pool, "run-p2-03", "stage-p2-03", "wi-p2-03").await;

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let req = serde_json::json!({
            "schema_version": "p080_reconcile_request_v1",
            "target": {
                "run_id": "run-p2-03",
                "stage_id": "stage-p2-03",
                "work_item_id": "wi-p2-03",
                "stale_class": "acp_startup_stale"
            },
            "requested_action": "repair_if_safe",
            "operator_request_dedup_key": "dedup-p2-03"
        });

        let first = execute("p080.reconcile.request.v1", req.clone(), &pool, &principal)
            .await
            .unwrap();
        // acp_startup_stale enabled but Phase 2+ not reached → rollout_disabled.
        assert_eq!(first["code"], "rollout_disabled");

        let second = execute("p080.reconcile.request.v1", req.clone(), &pool, &principal)
            .await
            .unwrap();
        assert_eq!(second["code"], "rollout_disabled");
    }

    /// Phase 1: repair_if_safe returns rollout_disabled before dedup/fingerprint checks
    /// when the class is enabled. Fingerprint-conflict behavior (idempotency_conflict) is Phase 2+ only.
    #[tokio::test]
    async fn p080_dedup_repair_phase1_returns_rollout_disabled() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "acp_startup_stale").await;
        enable_rollout_class(&pool, "detection_only").await;
        insert_stale_readback(&pool, "run-fp-01", "stage-fp-01", "wi-fp-01").await;
        insert_stale_readback(&pool, "run-fp-02", "stage-fp-02", "wi-fp-02").await;

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let dedup_key = "dedup-fp-conflict-01";

        // Phase 1: every repair_if_safe returns action_disabled_in_phase.
        let first = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-fp-01",
                    "stage_id": "stage-fp-01",
                    "work_item_id": "wi-fp-01",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": dedup_key
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(first["code"], "rollout_disabled");

        // Same dedup_key, different run_id — still returns rollout_disabled
        // because the Phase 1 gate fires before any dedup logic.
        let second = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-fp-02",
                    "stage_id": "stage-fp-02",
                    "work_item_id": "wi-fp-02",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": dedup_key
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(second["schema_version"], "p080_error_response_v1");
        // Phase 1 gate fires before any dedup or fingerprint check.
        assert_eq!(second["code"], "rollout_disabled");
    }

    /// Phase 1: repair_if_safe returns rollout_disabled before any dedup or fingerprint
    /// checks execute.  The idempotency_conflict response for fingerprint mismatches
    /// is Phase 2+ behavior and is NOT tested here.
    #[tokio::test]
    async fn p080_repair_if_safe_phase1_rollout_disabled_before_dedup_replay() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "acp_startup_stale").await;
        enable_rollout_class(&pool, "detection_only").await;
        insert_stale_readback(&pool, "run-fp-01", "stage-fp-01", "wi-fp-01").await;
        insert_stale_readback(&pool, "run-fp-02", "stage-fp-02", "wi-fp-02").await;

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let dedup_key = "dedup-fp-conflict-01";

        // Phase 1: every repair_if_safe returns rollout_disabled.
        let first = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-fp-01",
                    "stage_id": "stage-fp-01",
                    "work_item_id": "wi-fp-01",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": dedup_key
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(first["code"], "rollout_disabled");

        // Same dedup_key, different run_id — still returns rollout_disabled
        // because the Phase 1 gate fires before any dedup or fingerprint check.
        let second = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-fp-02",
                    "stage_id": "stage-fp-02",
                    "work_item_id": "wi-fp-02",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": dedup_key
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(second["schema_version"], "p080_error_response_v1");
        // Phase 1 gate fires before any dedup or fingerprint check.
        assert_eq!(second["code"], "rollout_disabled");
    }

    /// Phase 1: live_disable generation changes do not affect repair_if_safe outcome
    /// because the rollout gate fires before any fence checks execute.
    /// fence_check/generation-mismatch conflict behavior is Phase 2+ only.
    #[tokio::test]
    async fn p080_repair_if_safe_phase1_rollout_disabled_before_fence_check() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        enable_rollout_class(&pool, "acp_startup_stale").await;
        enable_rollout_class(&pool, "detection_only").await;
        sqlx::query(
            "INSERT OR REPLACE INTO p080_rollout_control_v1 \
             (class, enabled, phase, generation, updated_at, updated_by_principal_id, reason) \
             VALUES ('live_disable', 0, 'phase_0', 1, '2026-01-01T00:00:00Z', 'system', 'startup_seed')",
        )
        .execute(&pool)
        .await
        .unwrap();
        insert_stale_readback(&pool, "run-ld-01", "stage-ld-01", "wi-ld-01").await;

        let principal = auth::Principal::new("test-operator", auth::PrincipalClass::Operator);
        let dedup_key = "dedup-ld-fence-01";

        let first = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-ld-01",
                    "stage_id": "stage-ld-01",
                    "work_item_id": "wi-ld-01",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": dedup_key
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        // acp_startup_stale enabled, repair phase not reached → rollout_disabled.
        assert_eq!(first["code"], "rollout_disabled");

        sqlx::query("UPDATE p080_rollout_control_v1 SET generation=2 WHERE class='live_disable'")
            .execute(&pool)
            .await
            .unwrap();

        let second = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "target": {
                    "run_id": "run-ld-01",
                    "stage_id": "stage-ld-01",
                    "work_item_id": "wi-ld-01",
                    "stale_class": "acp_startup_stale"
                },
                "requested_action": "repair_if_safe",
                "operator_request_dedup_key": dedup_key
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        // Rollout gate fires before any fence check — live_disable fence is Phase 2+ only.
        assert_eq!(second["code"], "rollout_disabled");
    }

    // ── SEC-P080-HIGH-002 regression: ReadOnlyOperator tool isolation ────────
    // ReadOnlyOperator has no P080DiagnosticsGet or P080ReconcileRequest capability
    // (denied at auth layer).  The execute() function is called directly in these
    // tests to verify handler behavior independently; the auth gate sits above it.

    /// ReadOnlyOperator with an explicit run_scope containing the queried run_id must
    /// proceed past the scope check and return an empty page.
    #[tokio::test]
    async fn sec_p080_med_001_read_only_operator_scoped_diagnostics_get_proceeds() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "detection_only").await;
        // Scoped principal: run_scope includes the target run_id.
        let mut principal = auth::Principal::new("ro-test", auth::PrincipalClass::ReadOnlyOperator);
        principal.run_scope = Some(vec!["run-scope-01".to_string()]);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "filter": { "run_id": "run-scope-01" }
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(result["schema_version"], "p080_diagnostics_get_response_v1");
        assert_eq!(result["items"].as_array().unwrap().len(), 0);
    }

    /// ReadOnlyOperator WITHOUT run_scope configured must be rejected regardless of
    /// whether a run_id filter is supplied — SEC-P080-001 fail-closed behavior.
    #[tokio::test]
    async fn sec_p080_high_001_read_only_operator_unscoped_diagnostics_get_rejected() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "detection_only").await;
        let principal = auth::Principal::new("ro-test", auth::PrincipalClass::ReadOnlyOperator);
        // With run_id: still rejected — no run_scope configured.
        let with_run_id = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "filter": { "run_id": "run-scope-01" }
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(with_run_id["code"], "unauthorized_missing_capability");
        // Without run_id: also rejected.
        let without_run_id = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({ "schema_version": "p080_diagnostics_get_request_v1" }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(without_run_id["code"], "unauthorized_missing_capability");
    }

    // ── SEC-P080-MED-001: closed-schema enforcement tests ────────────────────

    #[tokio::test]
    async fn p080_diagnostics_get_rejects_unknown_top_level_field() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "detection_only").await;
        let principal = auth::Principal::new("op", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "unknown_key": "value"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(
            result["code"], "unknown_field",
            "diagnostics.get must reject unknown top-level fields"
        );
        assert_eq!(result["detail"]["field_path"], "unknown_key");
    }

    #[tokio::test]
    async fn p080_reconcile_request_rejects_unknown_top_level_field() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("op", auth::PrincipalClass::Operator);
        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "requested_action": "diagnose_only",
                "target": {
                    "run_id": "r1",
                    "stage_id": "s1",
                    "work_item_id": "w1",
                    "stale_class": "acp_startup_stale"
                },
                "injected_extra": "bad"
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();
        assert_eq!(
            result["code"], "unknown_field",
            "reconcile.request must reject unknown top-level fields"
        );
        assert_eq!(result["detail"]["field_path"], "injected_extra");
    }

    #[tokio::test]
    async fn p080_diagnostics_get_rejects_unknown_nested_filter_field() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "detection_only").await;
        let principal = auth::Principal::new("op", auth::PrincipalClass::Operator);

        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "filter": {
                    "run_id": "run-1",
                    "unexpected_nested": "bad"
                }
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(result["code"], "unknown_field");
        assert_eq!(result["detail"]["field_path"], "filter.unexpected_nested");
    }

    #[tokio::test]
    async fn p080_reconcile_request_rejects_unknown_nested_target_field() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        let principal = auth::Principal::new("op", auth::PrincipalClass::Operator);

        let result = execute(
            "p080.reconcile.request.v1",
            serde_json::json!({
                "schema_version": "p080_reconcile_request_v1",
                "requested_action": "diagnose_only",
                "target": {
                    "run_id": "r1",
                    "stage_id": "s1",
                    "work_item_id": "w1",
                    "stale_class": "acp_startup_stale",
                    "unexpected_nested": "bad"
                }
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(result["code"], "unknown_field");
        assert_eq!(result["detail"]["field_path"], "target.unexpected_nested");
    }

    #[tokio::test]
    async fn p080_diagnostics_get_rejects_invalid_filter_instead_of_broadening() {
        let pool = db::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        db::repos::p080::seed_rollout_control_if_absent(&pool)
            .await
            .unwrap();
        enable_rollout_class(&pool, "detection_only").await;
        let principal = auth::Principal::new("op", auth::PrincipalClass::Operator);

        let result = execute(
            "p080.diagnostics.get.v1",
            serde_json::json!({
                "schema_version": "p080_diagnostics_get_request_v1",
                "filter": {
                    "run_id": ""
                }
            }),
            &pool,
            &principal,
        )
        .await
        .unwrap();

        assert_eq!(result["code"], "invalid_field");
        assert_eq!(result["detail"]["field_path"], "filter.run_id");
    }

    /// Resource limit vocabulary: check_p080_resource_limits must return the proposal-specified
    /// error codes (json_depth_exceeded, array_length_exceeded, string_too_large) not the old
    /// generic resource_limit_exceeded code.
    #[test]
    fn p080_resource_limits_use_proposal_vocabulary() {
        // Depth exceeds 32 levels → json_depth_exceeded
        let deep = build_nested_json(33);
        let result = check_p080_resource_limits(&deep, 0);
        assert!(result.is_some(), "depth 33 must exceed limit");
        assert_eq!(
            result.unwrap()["code"],
            "json_depth_exceeded",
            "depth error must use proposal vocabulary code json_depth_exceeded"
        );

        // Array with 501 elements → array_length_exceeded
        let arr = serde_json::Value::Array(vec![serde_json::Value::Null; 501]);
        let result = check_p080_resource_limits(&arr, 0);
        assert!(result.is_some(), "array len 501 must exceed limit");
        assert_eq!(
            result.unwrap()["code"],
            "array_length_exceeded",
            "array error must use proposal vocabulary code array_length_exceeded"
        );

        // String exceeding 16 KiB → string_too_large
        let long_str = serde_json::Value::String("x".repeat(16385));
        let result = check_p080_resource_limits(&long_str, 0);
        assert!(result.is_some(), "string >16KiB must exceed limit");
        assert_eq!(
            result.unwrap()["code"],
            "string_too_large",
            "string error must use proposal vocabulary code string_too_large"
        );
    }

    fn build_nested_json(depth: usize) -> serde_json::Value {
        let mut v = serde_json::Value::Null;
        for _ in 0..depth {
            v = serde_json::json!({ "x": v });
        }
        v
    }
}
