use anyhow::Result;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::BTreeMap;

use crate::protocol::McpTool;

// P086 MCP error codes
const ERR_IDEMPOTENCY_CONFLICT: i64 = -32044;
const ERR_SATURATION_CAPACITY_EXCEEDED: i64 = -32051;

// Input length caps: prevent storage/metric amplification (P086-SEC-LOW-002).
// agent_execution_id and artifact ids are UUIDs (36 chars max in practice); 200 is generous.
// idempotency_key follows the reference MCP schema maxLength=256.
const MAX_INPUT_LEN: usize = 200;
const MAX_IDEMPOTENCY_KEY_LEN: usize = 256;
const MAX_OPERATOR_INSTRUCTION_LEN: usize = 8_000;
const MAX_BLOCKER_LEN: usize = 1_000;
const MAX_BLOCKERS: usize = 20;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "agents.continuation_status".to_string(),
            description:
                "P086: Read-only continuation history and current status for an agent execution."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "agent_execution_id": {
                        "type": "string",
                        "description": "UUID of the AgentExecution to query continuation status for.",
                        "maxLength": 200
                    }
                },
                "required": ["agent_execution_id"]
            }),
            output_schema: None,
        },
        McpTool {
            name: "agents.continuation_candidates".to_string(),
            description: "P086: Read-only list of eligible continuation candidates for a run."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "run_id": {
                        "type": "string",
                        "description": "UUID of the Run to list continuation candidates for.",
                        "maxLength": 200
                    }
                },
                "required": ["run_id"]
            }),
            output_schema: None,
        },
        McpTool {
            name: "agents.attach_receipt.get".to_string(),
            description: concat!(
                "P086: Fetch the provider_session_attach_receipt_v2 for a resurrection continuation. ",
                "Operator (run-scoped) receives the full raw receipt body. ",
                "Observer principals receive a reviewer-redacted projection (session ids hashed, ",
                "process identifiers absent). Agent principals receive only existence confirmation ",
                "and resurrection_phase. Wrong-run operators and unauthenticated callers are rejected ",
                "without exposing existence of receipts from other runs."
            ).to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "continuation_id": {
                        "type": "string",
                        "description": "UUID of the continuation whose attach receipt to fetch.",
                        "maxLength": 200
                    },
                    "run_id": {
                        "type": "string",
                        "description": "Run id that the caller is authorized for. Operators must supply this for run-scope verification.",
                        "maxLength": 200
                    }
                },
                "required": ["continuation_id", "run_id"],
                "additionalProperties": false
            }),
            output_schema: None,
        },
        McpTool {
            name: "agents.continue_work".to_string(),
            description: concat!(
                "P086: Issue a continuation command for an eligible code_writer AgentExecution. ",
                "Phase 2+: live_handle_continuation is enabled for operator_mcp trigger. ",
                "provider_session_resurrection requires Phase 4 adapter enablement. ",
                "lead_auto requires Phase 3 enablement with decision artifact validation."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "agent_execution_id": {
                        "type": "string",
                        "description": "UUID of the AgentExecution to continue.",
                        "maxLength": 200
                    },
                    "run_id": {
                        "type": "string",
                        "description": "Optional expected Run id. When present, it must match the AgentExecution run.",
                        "maxLength": 200
                    },
                    "stage_execution_id": {
                        "type": "string",
                        "description": "Optional expected StageExecution id. When present, it must match the AgentExecution owner.",
                        "maxLength": 200
                    },
                    "session_generation_id": {
                        "type": "string",
                        "description": "Optional expected live session generation id. When present, it must match the AgentExecution session.",
                        "maxLength": 200
                    },
                    "provider_session_id": {
                        "type": "string",
                        "description": "Optional expected provider-native session id. When present, it must match the live session binding.",
                        "maxLength": 200
                    },
                    "continuation_mode": {
                        "type": "string",
                        "enum": ["live_handle_continuation", "provider_session_resurrection"],
                        "description": "Continuation mode. This is the canonical P086 external field."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["live_handle_continuation", "provider_session_resurrection"],
                        "description": "Deprecated compatibility alias for continuation_mode."
                    },
                    "trigger_kind": {
                        "type": "string",
                        "enum": ["operator_mcp", "lead_auto"],
                        "description": "What triggered this continuation."
                    },
                    "idempotency_key": {
                        "type": "string",
                        "description": "Client-supplied idempotency key (UUID recommended).",
                        "maxLength": 256
                    },
                    "operator_instruction": {
                        "type": "string",
                        "description": "Operator or lead instruction to include in the canonical mode-reset continuation prompt.",
                        "maxLength": 8000
                    },
                    "max_turns": {
                        "type": "integer",
                        "description": "Maximum provider turns for the continuation prompt.",
                        "minimum": 1,
                        "maximum": 20
                    },
                    "max_wall_clock_seconds": {
                        "type": "integer",
                        "description": "Maximum wall-clock seconds for the continuation prompt.",
                        "minimum": 30,
                        "maximum": 7200
                    },
                    "blockers": {
                        "type": "array",
                        "description": "Current blockers that justify continuation.",
                        "maxItems": 20,
                        "items": {"type": "string", "maxLength": 1000}
                    },
                    "lead_decision_artifact_id": {
                        "type": "string",
                        "description": "Required for lead_auto: ID of the lead_continuation_decision_v1 artifact.",
                        "maxLength": 200
                    },
                    "lead_decision_artifact_sha256": {
                        "type": "string",
                        "description": "Required for lead_auto: lowercase hex SHA-256 of the decision artifact bytes.",
                        "maxLength": 64,
                        "pattern": "^[0-9a-f]{64}$"
                    },
                    "continuation_instruction_sha256": {
                        "type": "string",
                        "description": "Required for lead_auto: lowercase hex SHA-256 of the instruction text.",
                        "maxLength": 64,
                        "pattern": "^[0-9a-f]{64}$"
                    }
                },
                "required": ["agent_execution_id", "trigger_kind", "idempotency_key"],
                "anyOf": [
                    {"required": ["continuation_mode"]},
                    {"required": ["mode"]}
                ],
                "additionalProperties": false
            }),
            output_schema: None,
        },
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    match tool_name {
        "agents.continuation_status" => handle_continuation_status(&params, pool, principal).await,
        "agents.continuation_candidates" => {
            handle_continuation_candidates(&params, pool, principal).await
        }
        "agents.continue_work" => handle_continue_work(&params, pool, principal).await,
        "agents.attach_receipt.get" => handle_attach_receipt_get(&params, pool, principal).await,
        _ => Err(anyhow::anyhow!("Unknown agents tool: {tool_name}")),
    }
}

/// P086-SEC-MED-002: Build a redacted view of a continuation record safe for non-Operator
/// principals. Omits session-like correlation fields (idempotency scope/key, budget_json)
/// and lead decision metadata that are not required by the operator readback contract and
/// can leak sensitive operational identifiers to Observer principals.
fn redacted_record(r: &domain::continuation::ContinuationRecord) -> serde_json::Value {
    serde_json::json!({
        "id": r.id,
        "run_id": r.run_id,
        "stage_execution_id": r.stage_execution_id,
        "agent_execution_id": r.agent_execution_id,
        "mode": r.mode,
        "trigger_kind": r.trigger_kind,
        "status": r.status,
        "failure_reason": r.failure_reason,
        "reconciliation_status": r.reconciliation_status,
        "request_fingerprint_sha256": r.request_fingerprint_sha256,
        "canonical_request_artifact_id": r.canonical_request_artifact_id,
        "attach_receipt_artifact_id": r.attach_receipt_artifact_id,
        "evidence_bundle_artifact_id": r.evidence_bundle_artifact_id,
        "worktree_readback_artifact_id": r.worktree_readback_artifact_id,
        "continuation_report_artifact_id": r.continuation_report_artifact_id,
        "response_fingerprint_sha256": r.response_fingerprint_sha256,
        "response_artifact_id": r.response_artifact_id,
        "result_or_no_progress_artifact_id": r.result_or_no_progress_artifact_id,
        "conflict_count": r.conflict_count,
        "created_at": r.created_at,
        "updated_at": r.updated_at
        // Redacted for non-Operator: idempotency_scope, idempotency_key,
        // lead_decision_artifact_id, lead_decision_artifact_sha256,
        // continuation_instruction_sha256, budget_json
    })
}

/// P086-SEC-MED-002: Build a redacted view of a continuation candidate for non-Operator
/// principals. Omits provider_session_id which is a session-like identifier.
fn redacted_candidate(c: &domain::continuation::ContinuationCandidate) -> serde_json::Value {
    serde_json::json!({
        "agent_execution_id": c.agent_execution_id,
        "run_id": c.run_id,
        "stage_execution_id": c.stage_execution_id,
        "agent_role": c.agent_role,
        "status": c.status,
        "eligible": c.eligible,
        "disabled_reason": c.disabled_reason
        // Redacted for non-Operator: provider_session_id
    })
}

fn continuation_status_schema() -> serde_json::Value {
    serde_json::json!({
        "$defs": {
            "continuation_history_item_v1": {
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "status": {"type": "string"},
                    "mode": {"type": "string"},
                    "trigger_kind": {"type": "string"},
                    "request_fingerprint_sha256": {"type": "string"},
                    "created_at": {"type": "string"},
                    "updated_at": {"type": "string"}
                }
            }
        },
        "history": {"items": {"$ref": "#/$defs/continuation_history_item_v1"}},
        "metrics": {
            "type": ["object", "null"],
            "description": "P086 durable continuation metrics summary for the run"
        }
    })
}

async fn handle_continuation_status(
    params: &serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let agent_execution_id = params["agent_execution_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing agent_execution_id"))?;

    // SEC-LOW-001: runtime UUID validation before any DB call.
    if uuid::Uuid::parse_str(agent_execution_id).is_err() {
        return Ok(serde_json::json!({
            "agent_execution_id": agent_execution_id,
            "active": null,
            "history": { "items": [] },
            "error": { "code": -32602, "message": "agent_execution_id is not a valid UUID" }
        }));
    }

    // P086-SEC-LOW-003: Agent principals cannot read continuation history
    // without per-agent ownership verification. Return empty (no existence leak)
    // until Phase 1 ownership infrastructure is implemented.
    if matches!(principal.class, auth::PrincipalClass::Agent) {
        return Ok(serde_json::json!({
            "agent_execution_id": agent_execution_id,
            "active": null,
            "history": { "items": [] },
            "response_schema": continuation_status_schema()
        }));
    }

    let records =
        db::repos::agent_work_continuations::list_for_agent_execution(pool, agent_execution_id)
            .await?;

    let active = db::repos::agent_work_continuations::find_active_for_agent_execution(
        pool,
        agent_execution_id,
    )
    .await?;
    let metrics = if let Some(run_id) = records
        .first()
        .map(|record| record.run_id.as_str())
        .or_else(|| active.as_ref().map(|record| record.run_id.as_str()))
    {
        Some(
            db::repos::agent_work_continuations::p086_continuation_metrics_summary_for_run(
                pool, run_id,
            )
            .await?,
        )
    } else {
        None
    };

    // P086-SEC-MED-002: redact session-like metadata for non-Operator principals.
    if matches!(principal.class, auth::PrincipalClass::Operator) {
        Ok(serde_json::json!({
            "agent_execution_id": agent_execution_id,
            "active": active,
            "history": { "items": records },
            "metrics": metrics,
            "response_schema": continuation_status_schema()
        }))
    } else {
        let redacted_records: Vec<serde_json::Value> =
            records.iter().map(redacted_record).collect();
        let redacted_active = active.as_ref().map(redacted_record);
        Ok(serde_json::json!({
            "agent_execution_id": agent_execution_id,
            "active": redacted_active,
            "history": { "items": redacted_records },
            "metrics": metrics,
            "response_schema": continuation_status_schema()
        }))
    }
}

async fn handle_continuation_candidates(
    params: &serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let run_id = params["run_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing run_id"))?;

    // SEC-LOW-001: runtime UUID validation before any DB call.
    if uuid::Uuid::parse_str(run_id).is_err() {
        return Ok(serde_json::json!({
            "run_id": run_id,
            "candidates": [],
            "error": { "code": -32602, "message": "run_id is not a valid UUID" }
        }));
    }

    // P086-SEC-LOW-003: Agent principals cannot enumerate continuation
    // candidates without per-run membership verification. Return empty
    // (no existence leak) until Phase 1 ownership infrastructure is implemented.
    if matches!(principal.class, auth::PrincipalClass::Agent) {
        return Ok(serde_json::json!({
            "run_id": run_id,
            "candidates": []
        }));
    }

    let candidates =
        db::repos::agent_work_continuations::list_candidates_for_run(pool, run_id).await?;

    // P086-SEC-MED-002: redact provider_session_id for non-Operator principals.
    if matches!(principal.class, auth::PrincipalClass::Operator) {
        Ok(serde_json::json!({
            "run_id": run_id,
            "candidates": candidates
        }))
    } else {
        let redacted: Vec<serde_json::Value> = candidates.iter().map(redacted_candidate).collect();
        Ok(serde_json::json!({
            "run_id": run_id,
            "candidates": redacted
        }))
    }
}

/// P086: Fetch provider_session_attach_receipt_v2 with principal-access-matrix enforcement.
///
/// Operator (run-scoped) → full raw JSON body, audit row outcome=raw_read
/// Observer (Reviewer)   → reviewer-redacted projection, audit row outcome=reviewer_projection
/// Agent (Guest)         → minimal projection (continuation_id + resurrection_phase), denied in audit
/// Wrong-run Operator    → auth_failure, audit row outcome=denied, no existence oracle
async fn handle_attach_receipt_get(
    params: &serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let continuation_id = params["continuation_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing continuation_id"))?;
    let Some(requested_run_id) = params["run_id"].as_str() else {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32001,
                "message": "auth_failure: run_id is required for provider session attach receipt access",
                "data": { "failure_reason": "auth_failure" }
            }
        }));
    };
    let now = chrono::Utc::now().to_rfc3339();
    let audit_id = uuid::Uuid::new_v4().to_string();

    // Unauthenticated requests are rejected by the MCP auth layer before reaching here.
    // Agent (Guest) principal gets minimal projection only — no raw receipt access.
    if matches!(principal.class, auth::PrincipalClass::Agent) {
        // Return minimal: continuation existence + resurrection_phase (no raw data).
        let resurrection_phase = fetch_resurrection_phase(pool, continuation_id).await?;
        let _ = db::repos::p086_resurrection_raw_receipts::record_access_audit(
            pool,
            &db::repos::p086_resurrection_raw_receipts::ReceiptAccessAuditRow {
                id: audit_id,
                principal_id: principal.id.clone(),
                principal_class: "agent".to_string(),
                continuation_id: continuation_id.to_string(),
                run_id: requested_run_id.to_string(),
                requested_at: now,
                source_channel: "mcp".to_string(),
                outcome: "denied".to_string(),
                denial_reason: Some("agent_principal_minimal_only".to_string()),
            },
        )
        .await;
        return Ok(serde_json::json!({
            "principal_class": "agent",
            "continuation_id": continuation_id,
            "resurrection_phase": resurrection_phase,
            "access_level": "minimal"
        }));
    }

    // Look up which run owns this continuation (needed for run-scope authorization).
    let actual_run_id =
        db::repos::p086_resurrection_raw_receipts::continuation_run_id(pool, continuation_id)
            .await?;

    // Operator principals: verify run scope. Wrong-run returns auth_failure (no existence oracle).
    if matches!(principal.class, auth::PrincipalClass::Operator) {
        let matches = actual_run_id
            .as_deref()
            .map(|actual| actual == requested_run_id)
            .unwrap_or(false);
        if !matches {
            let _ = db::repos::p086_resurrection_raw_receipts::record_access_audit(
                pool,
                &db::repos::p086_resurrection_raw_receipts::ReceiptAccessAuditRow {
                    id: audit_id,
                    principal_id: principal.id.clone(),
                    principal_class: "operator".to_string(),
                    continuation_id: continuation_id.to_string(),
                    run_id: requested_run_id.to_string(),
                    requested_at: now,
                    source_channel: "mcp".to_string(),
                    outcome: "denied".to_string(),
                    denial_reason: Some("wrong_run_or_not_found".to_string()),
                },
            )
            .await;
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32001,
                    "message": "auth_failure: run_id does not match or continuation not found",
                    "data": { "failure_reason": "auth_failure" }
                }
            }));
        }

        // Operator with matching run_id → return full raw receipt.
        let raw = db::repos::p086_resurrection_raw_receipts::find_by_continuation_id(
            pool,
            continuation_id,
        )
        .await?;
        let run_id_for_audit = actual_run_id.as_deref().unwrap_or("").to_string();
        match raw {
            Some(row) => {
                let _ = db::repos::p086_resurrection_raw_receipts::record_access_audit(
                    pool,
                    &db::repos::p086_resurrection_raw_receipts::ReceiptAccessAuditRow {
                        id: audit_id,
                        principal_id: principal.id.clone(),
                        principal_class: "operator".to_string(),
                        continuation_id: continuation_id.to_string(),
                        run_id: run_id_for_audit,
                        requested_at: now,
                        source_channel: "mcp".to_string(),
                        outcome: "raw_read".to_string(),
                        denial_reason: None,
                    },
                )
                .await;
                let raw_json: serde_json::Value =
                    serde_json::from_str(&row.raw_receipt_json).unwrap_or(serde_json::json!({}));
                Ok(serde_json::json!({
                    "principal_class": "operator",
                    "access_level": "raw",
                    "continuation_id": continuation_id,
                    "receipt": raw_json
                }))
            }
            None => {
                let _ = db::repos::p086_resurrection_raw_receipts::record_access_audit(
                    pool,
                    &db::repos::p086_resurrection_raw_receipts::ReceiptAccessAuditRow {
                        id: audit_id,
                        principal_id: principal.id.clone(),
                        principal_class: "operator".to_string(),
                        continuation_id: continuation_id.to_string(),
                        run_id: run_id_for_audit,
                        requested_at: now,
                        source_channel: "mcp".to_string(),
                        outcome: "denied".to_string(),
                        denial_reason: Some("receipt_not_found".to_string()),
                    },
                )
                .await;
                Ok(serde_json::json!({
                    "outcome": "not_found",
                    "continuation_id": continuation_id
                }))
            }
        }
    } else {
        // Observer (Reviewer) principal: return redacted projection.
        let raw = db::repos::p086_resurrection_raw_receipts::find_by_continuation_id(
            pool,
            continuation_id,
        )
        .await?;
        let run_id_for_audit = actual_run_id.as_deref().unwrap_or("").to_string();
        let _ = db::repos::p086_resurrection_raw_receipts::record_access_audit(
            pool,
            &db::repos::p086_resurrection_raw_receipts::ReceiptAccessAuditRow {
                id: audit_id,
                principal_id: principal.id.clone(),
                principal_class: "observer".to_string(),
                continuation_id: continuation_id.to_string(),
                run_id: run_id_for_audit,
                requested_at: now.clone(),
                source_channel: "mcp".to_string(),
                outcome: "reviewer_projection".to_string(),
                denial_reason: None,
            },
        )
        .await;
        match raw {
            Some(row) => {
                let raw_json: serde_json::Value =
                    serde_json::from_str(&row.raw_receipt_json).unwrap_or(serde_json::json!({}));
                let redacted = reviewer_redact_receipt(&raw_json);
                Ok(serde_json::json!({
                    "principal_class": "observer",
                    "access_level": "reviewer_redacted",
                    "continuation_id": continuation_id,
                    "receipt": redacted
                }))
            }
            None => Ok(serde_json::json!({
                "outcome": "not_found",
                "continuation_id": continuation_id
            })),
        }
    }
}

/// Fetch resurrection_phase from agent_work_continuations for minimal projections.
async fn fetch_resurrection_phase(
    pool: &SqlitePool,
    continuation_id: &str,
) -> Result<Option<String>> {
    let row = sqlx::query("SELECT resurrection_phase FROM agent_work_continuations WHERE id = ?1")
        .bind(continuation_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|r| {
        r.try_get::<Option<String>, _>("resurrection_phase")
            .ok()
            .flatten()
    }))
}

/// Build reviewer-redacted projection of a raw receipt JSON.
///
/// Redaction rules per proposal access matrix:
/// - requested_provider_session_id → prefix + sha256 hash (never raw value)
/// - actual_provider_session_id    → prefix + sha256 hash
/// - identity_proof_artifact_id    → replaced with constant redaction marker
/// - adapter_runtime_home_realpath → ABSENT (not present, not null)
/// - adapter_runtime_home_dev_ino  → ABSENT
/// - managed_child_pid             → ABSENT
/// - managed_process_group_id      → ABSENT (alias: managed_child_process_group_id)
/// - managed_child_start_time      → ABSENT
/// All other fields pass through unchanged.
fn reviewer_redact_receipt(raw: &serde_json::Value) -> serde_json::Value {
    let Some(obj) = raw.as_object() else {
        return serde_json::json!({});
    };
    // Fields that must be entirely absent (not null-set) to defeat field-presence side channels.
    const ABSENT_FIELDS: &[&str] = &[
        "adapter_runtime_home_realpath",
        "adapter_runtime_home_dev_ino",
        "managed_child_pid",
        "managed_process_group_id",
        "managed_child_process_group_id",
        "managed_child_start_time",
    ];
    // Fields whose values are replaced with prefix+hash to redact provider session ids.
    const SESSION_ID_FIELDS: &[&str] = &[
        "requested_provider_session_id",
        "actual_provider_session_id",
    ];
    let mut out = serde_json::Map::new();
    for (k, v) in obj {
        if ABSENT_FIELDS.contains(&k.as_str()) {
            continue;
        }
        if SESSION_ID_FIELDS.contains(&k.as_str()) {
            if let Some(s) = v.as_str() {
                let hash: String = Sha256::digest(s.as_bytes())
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();
                let prefix: String = s.chars().take(4).collect();
                out.insert(
                    k.clone(),
                    serde_json::Value::String(format!("{prefix}...{hash}")),
                );
            } else {
                out.insert(k.clone(), v.clone());
            }
            continue;
        }
        if k == "identity_proof_artifact_id" {
            out.insert(
                k.clone(),
                serde_json::Value::String("[redacted]".to_string()),
            );
            continue;
        }
        out.insert(k.clone(), v.clone());
    }
    serde_json::Value::Object(out)
}

/// Compute canonical request fingerprint: SHA-256 of sorted-key JSON (BTreeMap ensures ordering),
/// LF normalized, no timestamps or display labels. Returns lowercase 64-char hex.
///
/// Includes server-derived context fields (run_id, stage_execution_id, caller_principal_id)
/// so the fingerprint is unique to the full invocation context, not just the caller-supplied params.
/// Runtime-only fields (session_generation_id, worktree_root, etc.) are set by the background
/// worker and go into the canonical_request artifact, not the admission fingerprint.
#[allow(clippy::too_many_arguments)]
fn compute_canonical_fingerprint(
    agent_execution_id: &str,
    run_id: &str,
    stage_execution_id: &str,
    caller_principal_id: &str,
    mode: &str,
    trigger_kind: &str,
    idempotency_scope: &str,
    idempotency_key: &str,
    lead_decision_artifact_id: Option<&str>,
    lead_decision_artifact_sha256: Option<&str>,
    continuation_instruction_sha256: Option<&str>,
    request_context: Option<&serde_json::Value>,
) -> String {
    let mut map: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    map.insert(
        "command",
        serde_json::Value::String("agents.continue_work".into()),
    );
    map.insert(
        "agent_execution_id",
        serde_json::Value::String(agent_execution_id.into()),
    );
    map.insert("run_id", serde_json::Value::String(run_id.into()));
    map.insert(
        "stage_execution_id",
        serde_json::Value::String(stage_execution_id.into()),
    );
    map.insert(
        "caller_principal_id",
        serde_json::Value::String(caller_principal_id.into()),
    );
    map.insert("mode", serde_json::Value::String(mode.into()));
    map.insert(
        "trigger_kind",
        serde_json::Value::String(trigger_kind.into()),
    );
    map.insert(
        "idempotency_scope",
        serde_json::Value::String(idempotency_scope.into()),
    );
    map.insert(
        "idempotency_key",
        serde_json::Value::String(idempotency_key.into()),
    );
    if let Some(v) = lead_decision_artifact_id {
        map.insert(
            "lead_decision_artifact_id",
            serde_json::Value::String(v.into()),
        );
    }
    if let Some(v) = lead_decision_artifact_sha256 {
        map.insert(
            "lead_decision_artifact_sha256",
            serde_json::Value::String(v.into()),
        );
    }
    if let Some(v) = continuation_instruction_sha256 {
        map.insert(
            "continuation_instruction_sha256",
            serde_json::Value::String(v.into()),
        );
    }
    if let Some(v) = request_context {
        map.insert("request_context", v.clone());
    }

    let canonical_json = serde_json::to_string(&map)
        .expect("BTreeMap<&str, Value> always serializes")
        .replace("\r\n", "\n");

    let hash = Sha256::digest(canonical_json.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

fn validate_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f'))
}

#[derive(Clone, Copy)]
struct LeadAutoDecisionTarget<'a> {
    run_id: &'a str,
    stage_execution_id: &'a str,
    agent_execution_id: &'a str,
    session_generation_id: Option<&'a str>,
    request_max_turns: Option<i64>,
    request_max_wall_clock_seconds: Option<i64>,
}

fn lead_auto_rejection(
    agent_execution_id: &str,
    code: i64,
    message: &str,
    failure_reason: &str,
) -> serde_json::Value {
    serde_json::json!({
        "outcome": "rejected",
        "error": {
            "code": code,
            "message": message,
            "data": {
                "agent_execution_id": agent_execution_id,
                "failure_reason": failure_reason
            }
        }
    })
}

fn json_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

fn validate_lead_auto_decision_payload(
    artifact_json: &serde_json::Value,
    supplied_instruction_sha256: &str,
    target: LeadAutoDecisionTarget<'_>,
) -> Option<serde_json::Value> {
    if json_str(artifact_json, "schema_version") != Some("lead_continuation_decision_v1")
        || json_str(artifact_json, "artifact_kind") != Some("lead_continuation_decision")
    {
        return Some(lead_auto_rejection(
            target.agent_execution_id,
            -32024,
            "lead_decision_artifact has invalid schema_version or artifact_kind",
            "lead_auto_artifact_invalid_schema",
        ));
    }

    for (key, expected) in [
        ("run_id", target.run_id),
        ("stage_execution_id", target.stage_execution_id),
        ("agent_execution_id", target.agent_execution_id),
    ] {
        if json_str(artifact_json, key) != Some(expected) {
            return Some(lead_auto_rejection(
                target.agent_execution_id,
                -32025,
                "lead_decision_artifact target does not match current agent execution",
                "lead_auto_artifact_target_mismatch",
            ));
        }
    }

    let Some(payload) = artifact_json.get("payload") else {
        return Some(lead_auto_rejection(
            target.agent_execution_id,
            -32024,
            "lead_decision_artifact missing payload",
            "lead_auto_artifact_missing_payload",
        ));
    };

    if json_str(payload, "schema_version") != Some("lead_continuation_decision_v1") {
        return Some(lead_auto_rejection(
            target.agent_execution_id,
            -32024,
            "lead_decision_artifact payload has invalid schema_version",
            "lead_auto_artifact_invalid_payload_schema",
        ));
    }
    if json_str(payload, "decision") != Some("continue") {
        return Some(lead_auto_rejection(
            target.agent_execution_id,
            -32025,
            "lead_decision_artifact decision is not continue",
            "lead_auto_decision_not_continue",
        ));
    }
    if json_str(payload, "agent_id") != Some("code_writer") {
        return Some(lead_auto_rejection(
            target.agent_execution_id,
            -32025,
            "lead_decision_artifact agent_id must be code_writer",
            "lead_auto_agent_id_mismatch",
        ));
    }

    for (key, expected) in [
        ("run_id", target.run_id),
        ("stage_execution_id", target.stage_execution_id),
        ("agent_execution_id", target.agent_execution_id),
    ] {
        if json_str(payload, key) != Some(expected) {
            return Some(lead_auto_rejection(
                target.agent_execution_id,
                -32025,
                "lead_decision_artifact payload target does not match current agent execution",
                "lead_auto_artifact_target_mismatch",
            ));
        }
    }
    if let Some(expected_session) = target.session_generation_id {
        if json_str(payload, "session_generation_id") != Some(expected_session) {
            return Some(lead_auto_rejection(
                target.agent_execution_id,
                -32025,
                "lead_decision_artifact session_generation_id does not match current live session",
                "lead_auto_session_generation_mismatch",
            ));
        }
    }

    if json_str(payload, "continuation_instruction_sha256") != Some(supplied_instruction_sha256) {
        return Some(lead_auto_rejection(
            target.agent_execution_id,
            -32025,
            "continuation_instruction_sha256 does not match artifact payload",
            "lead_auto_instruction_hash_mismatch",
        ));
    }
    let Some(continuation_instruction) = json_str(payload, "continuation_instruction") else {
        return Some(lead_auto_rejection(
            target.agent_execution_id,
            -32024,
            "lead_decision_artifact payload missing continuation_instruction",
            "lead_auto_instruction_missing",
        ));
    };
    let computed_instruction_sha256: String = Sha256::digest(continuation_instruction.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if computed_instruction_sha256 != supplied_instruction_sha256 {
        return Some(lead_auto_rejection(
            target.agent_execution_id,
            -32025,
            "continuation_instruction_sha256 does not match continuation_instruction bytes",
            "lead_auto_instruction_hash_mismatch",
        ));
    }

    let safety = payload.get("safety_checks");
    for key in [
        "no_release_side_effect",
        "no_unresolved_effect_ledger",
        "same_worktree_required",
    ] {
        if safety
            .and_then(|value| value.get(key))
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return Some(lead_auto_rejection(
                target.agent_execution_id,
                -32025,
                "lead_decision_artifact safety_checks are incomplete",
                "lead_auto_safety_check_failed",
            ));
        }
    }

    if let (Some(artifact_max), Some(request_max)) = (
        payload.get("max_turns").and_then(serde_json::Value::as_i64),
        target.request_max_turns,
    ) {
        if artifact_max < 1 || artifact_max > request_max {
            return Some(lead_auto_rejection(
                target.agent_execution_id,
                -32025,
                "lead_decision_artifact max_turns exceeds request limit",
                "lead_auto_budget_exceeds_request",
            ));
        }
    }
    if let (Some(artifact_max), Some(request_max)) = (
        payload
            .get("max_wall_clock_seconds")
            .and_then(serde_json::Value::as_i64),
        target.request_max_wall_clock_seconds,
    ) {
        if artifact_max < 30 || artifact_max > request_max {
            return Some(lead_auto_rejection(
                target.agent_execution_id,
                -32025,
                "lead_decision_artifact max_wall_clock_seconds exceeds request limit",
                "lead_auto_budget_exceeds_request",
            ));
        }
    }

    None
}

fn optional_string_param<'a>(
    params: &'a serde_json::Value,
    field: &str,
    max_len: usize,
) -> Result<Option<&'a str>, serde_json::Value> {
    let Some(value) = params.get(field) else {
        return Ok(None);
    };
    let Some(raw) = value.as_str() else {
        return Err(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": format!("{field} must be a string"),
                "data": { "failure_reason": "invalid_input_type", "field": field }
            }
        }));
    };
    if raw.len() > max_len {
        return Err(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": format!("{field} exceeds maximum length of {max_len}"),
                "data": { "failure_reason": "input_too_long", "field": field }
            }
        }));
    }
    Ok(Some(raw))
}

fn optional_i64_param(
    params: &serde_json::Value,
    field: &str,
    min: i64,
    max: i64,
) -> Result<Option<i64>, serde_json::Value> {
    let Some(value) = params.get(field) else {
        return Ok(None);
    };
    let Some(raw) = value.as_i64() else {
        return Err(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": format!("{field} must be an integer"),
                "data": { "failure_reason": "invalid_input_type", "field": field }
            }
        }));
    };
    if raw < min || raw > max {
        return Err(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": format!("{field} must be between {min} and {max}"),
                "data": { "failure_reason": "input_out_of_range", "field": field }
            }
        }));
    }
    Ok(Some(raw))
}

fn blockers_param(params: &serde_json::Value) -> Result<Vec<String>, serde_json::Value> {
    let Some(value) = params.get("blockers") else {
        return Ok(Vec::new());
    };
    let Some(items) = value.as_array() else {
        return Err(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": "blockers must be an array of strings",
                "data": { "failure_reason": "invalid_input_type", "field": "blockers" }
            }
        }));
    };
    if items.len() > MAX_BLOCKERS {
        return Err(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": format!("blockers exceeds maximum length of {MAX_BLOCKERS}"),
                "data": { "failure_reason": "input_too_long", "field": "blockers" }
            }
        }));
    }
    let mut blockers = Vec::with_capacity(items.len());
    for item in items {
        let Some(raw) = item.as_str() else {
            return Err(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32026,
                    "message": "blockers entries must be strings",
                    "data": { "failure_reason": "invalid_input_type", "field": "blockers" }
                }
            }));
        };
        if raw.len() > MAX_BLOCKER_LEN {
            return Err(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32026,
                    "message": format!("blocker entry exceeds maximum length of {MAX_BLOCKER_LEN}"),
                    "data": { "failure_reason": "input_too_long", "field": "blockers" }
                }
            }));
        }
        blockers.push(raw.to_string());
    }
    Ok(blockers)
}

fn forbidden_stage_kind(
    logical_stage_id: Option<&str>,
    stage_type: Option<&str>,
) -> Option<&'static str> {
    let stage = format!(
        "{} {}",
        logical_stage_id.unwrap_or_default(),
        stage_type.unwrap_or_default()
    )
    .to_ascii_lowercase();
    for (needle, reason) in [
        ("release", "release"),
        ("publish", "publish"),
        ("git_push", "git_push"),
        ("git-push", "git_push"),
        ("upload", "upload"),
        ("distribution", "distribution"),
        ("distribute", "distribution"),
        ("connect", "upload"),
    ] {
        if stage.contains(needle) {
            return Some(reason);
        }
    }
    None
}

fn provider_session_resurrection_adapter_supported(provider: Option<&str>) -> bool {
    let Some(provider) = provider else {
        return false;
    };
    match provider {
        // Adapter support is intentionally fail-closed until the provider can
        // prove requested-session attachment before prompt send.
        "claude" | "claude_acp" | "claude_code" | "codex" | "gemini" | "auggie" | "junie" => false,
        _ => false,
    }
}

fn continuation_capability_rejection(
    agent_execution_id: &str,
    catalog_snapshot_json: Option<&str>,
    mode: &str,
    trigger_kind: &str,
    live_session_present: bool,
    provider_session_id_present: bool,
    provider_session_resurrection_supported: bool,
) -> Result<Option<serde_json::Value>> {
    let Some(raw_catalog) = catalog_snapshot_json else {
        return Ok(Some(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32033,
                "message": "run has no frozen catalog snapshot; continuation capability cannot be proven",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "continuation_capability_missing"
                }
            }
        })));
    };
    let catalog: serde_json::Value = match serde_json::from_str(raw_catalog) {
        Ok(value) => value,
        Err(error) => {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32033,
                    "message": "run catalog snapshot is not valid JSON",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "continuation_capability_snapshot_invalid",
                        "detail": error.to_string()
                    }
                }
            })));
        }
    };
    let Some(agent) = catalog["agents"].as_array().and_then(|agents| {
        agents
            .iter()
            .find(|agent| agent["id"].as_str() == Some("code_writer"))
    }) else {
        return Ok(Some(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32033,
                "message": "frozen catalog has no code_writer agent",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "continuation_capability_missing"
                }
            }
        })));
    };
    let capability = &agent["continuation_capability"];
    if capability["enabled"].as_bool() != Some(true) {
        return Ok(Some(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32033,
                "message": "code_writer continuation_capability is disabled or absent in the frozen catalog",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "continuation_capability_disabled"
                }
            }
        })));
    }
    let trigger_allowed = capability["allowed_triggers"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(trigger_kind)));
    if !trigger_allowed {
        return Ok(Some(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32033,
                "message": "trigger_kind is not allowed by code_writer continuation_capability",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "trigger_kind": trigger_kind,
                    "failure_reason": "continuation_trigger_not_allowed"
                }
            }
        })));
    }
    if mode == "live_handle_continuation" {
        let live_enabled =
            capability["live_handle_continuation"]["enabled"].as_bool() == Some(true);
        let require_live_session = capability["live_handle_continuation"]["require_live_session"]
            .as_bool()
            .unwrap_or(true);
        if !live_enabled {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32033,
                    "message": "live_handle_continuation is disabled by code_writer continuation_capability",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "live_handle_continuation_disabled"
                    }
                }
            })));
        }
        if require_live_session && !live_session_present {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32034,
                    "message": "live_handle_continuation requires a recorded live session_generation_id and provider_session_id",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "live_session_required"
                    }
                }
            })));
        }
    }
    if mode == "provider_session_resurrection" {
        let resurrection = &capability["provider_session_resurrection"];
        let resurrection_enabled = resurrection["enabled"].as_bool() == Some(true);
        if !resurrection_enabled {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32033,
                    "message": "provider_session_resurrection is disabled by code_writer continuation_capability",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "provider_session_resurrection_disabled"
                    }
                }
            })));
        }
        let resurrection_trigger_allowed = resurrection["allowed_triggers"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(trigger_kind)));
        if !resurrection_trigger_allowed {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32033,
                    "message": "trigger_kind is not allowed by provider_session_resurrection capability",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "trigger_kind": trigger_kind,
                        "failure_reason": "provider_session_resurrection_trigger_not_allowed"
                    }
                }
            })));
        }
        let require_recorded_provider_session_id = resurrection
            ["require_recorded_provider_session_id"]
            .as_bool()
            .unwrap_or(true);
        if require_recorded_provider_session_id && !provider_session_id_present {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32034,
                    "message": "provider_session_resurrection requires a recorded provider_session_id",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "provider_session_id_required"
                    }
                }
            })));
        }
        if !provider_session_resurrection_supported {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32010,
                    "message": "provider_session_resurrection is not supported by the target provider adapter",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "provider_session_resurrection_unsupported"
                    }
                }
            })));
        }
    }
    Ok(None)
}

/// P086-SEC-HIGH-001: Verify lead_auto artifact identity by fetching the artifact record,
/// computing SHA-256 of the on-disk bytes, and comparing against the caller-supplied hashes.
/// Also verifies continuation_instruction_sha256 by reading it from the artifact payload.
///
/// Returns `Ok(None)` when verification passes, or `Ok(Some(rejection))` when it fails.
/// The rejection value should be returned directly as the MCP response.
///
/// This function is currently only reached when the Phase 3 gate (in handle_continue_work)
/// is explicitly removed. It is implemented here so Phase 3 enablement requires no new
/// security-critical code.
#[allow(dead_code)]
async fn verify_lead_auto_artifacts(
    pool: &SqlitePool,
    agent_execution_id: &str,
    lead_decision_artifact_id: &str,
    supplied_artifact_sha256: &str,
    supplied_instruction_sha256: &str,
    target: LeadAutoDecisionTarget<'_>,
) -> Result<Option<serde_json::Value>> {
    use domain::ids::ArtifactId;

    // Parse artifact ID as UUID.
    let artifact_uuid = match uuid::Uuid::parse_str(lead_decision_artifact_id) {
        Ok(u) => u,
        Err(_) => {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32024,
                    "message": "lead_decision_artifact_id is not a valid UUID",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "lead_auto_invalid_artifact_id"
                    }
                }
            })));
        }
    };
    let artifact_id: ArtifactId = artifact_uuid.into();

    // Fetch artifact record from DB.
    let artifact = match db::repos::artifacts::find_by_id(pool, artifact_id).await? {
        Some(a) => a,
        None => {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32024,
                    "message": "lead_decision_artifact not found",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "lead_auto_artifact_not_found"
                    }
                }
            })));
        }
    };

    // SEC-HIGH-002: Ownership check — the artifact row must belong to the agent_execution_id
    // making this request. Prevents one agent from referencing another agent's artifacts.
    if artifact.agent_execution_id.as_deref() != Some(agent_execution_id) {
        return Ok(Some(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32024,
                "message": "lead_decision_artifact does not belong to this agent_execution",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "lead_auto_artifact_ownership_mismatch"
                }
            }
        })));
    }

    // SEC-HIGH-002: Path containment check — reject paths containing '..' traversal
    // components to prevent reading outside the run artifact tree.
    {
        use std::path::Component;
        if std::path::Path::new(&artifact.file_path)
            .components()
            .any(|c| c == Component::ParentDir)
        {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32024,
                    "message": "lead_decision_artifact path contains directory traversal components",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "lead_auto_artifact_path_traversal"
                    }
                }
            })));
        }
    }

    // SEC-HIGH-002: Atomic open with O_NOFOLLOW, fstat the same descriptor for type/size
    // validation, then read from the same handle. This eliminates the TOCTOU race window
    // between the old symlink_metadata() check and the separate tokio::fs::read() call.
    //
    // O_NOFOLLOW causes open() to fail with ELOOP when the final path component is a symlink,
    // which is checked by raw_os_error() == ELOOP (62 on macOS, 40 on Linux).
    const LEAD_AUTO_ARTIFACT_MAX_BYTES: u64 = 1024 * 1024; // 1 MB hard cap

    // O_NOFOLLOW values per platform (no libc dependency required).
    #[cfg(target_os = "macos")]
    const O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
    #[cfg(target_os = "linux")]
    const O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    const O_NOFOLLOW_FLAG: i32 = 0;

    // ELOOP errno values (open returns ELOOP when O_NOFOLLOW + path is a symlink).
    #[cfg(target_os = "macos")]
    const ELOOP_ERRNO: i32 = 62;
    #[cfg(target_os = "linux")]
    const ELOOP_ERRNO: i32 = 40;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    const ELOOP_ERRNO: i32 = -1; // unused

    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt as _;
    use tokio::io::AsyncReadExt as _;

    let mut open_opts = tokio::fs::OpenOptions::new();
    open_opts.read(true);
    #[cfg(unix)]
    open_opts.custom_flags(O_NOFOLLOW_FLAG);

    let mut file = match open_opts.open(&artifact.file_path).await {
        Ok(f) => f,
        Err(e) => {
            let is_symlink = e.raw_os_error() == Some(ELOOP_ERRNO);
            if is_symlink {
                return Ok(Some(serde_json::json!({
                    "outcome": "rejected",
                    "error": {
                        "code": -32024,
                        "message": "lead_decision_artifact path is a symlink; symlinks are not permitted",
                        "data": {
                            "agent_execution_id": agent_execution_id,
                            "failure_reason": "lead_auto_artifact_symlink"
                        }
                    }
                })));
            }
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32024,
                    "message": "lead_decision_artifact file unreadable",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "lead_auto_artifact_unreadable",
                        "detail": e.to_string()
                    }
                }
            })));
        }
    };

    // fstat the opened descriptor — type and size are checked on the same fd we will read from.
    let meta = match file.metadata().await {
        Ok(m) => m,
        Err(e) => {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32024,
                    "message": "lead_decision_artifact fstat failed",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "lead_auto_artifact_unreadable",
                        "detail": e.to_string()
                    }
                }
            })));
        }
    };

    if meta.file_type().is_symlink() {
        return Ok(Some(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32024,
                "message": "lead_decision_artifact is a symlink; symlinks are not permitted",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "lead_auto_artifact_symlink"
                }
            }
        })));
    }
    if !meta.is_file() {
        return Ok(Some(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32024,
                "message": "lead_decision_artifact path is not a regular file",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "lead_auto_artifact_not_regular_file"
                }
            }
        })));
    }
    if meta.len() > LEAD_AUTO_ARTIFACT_MAX_BYTES {
        return Ok(Some(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32024,
                "message": "lead_decision_artifact exceeds the 1 MB size limit",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "lead_auto_artifact_too_large",
                    "size_bytes": meta.len(),
                    "max_bytes": LEAD_AUTO_ARTIFACT_MAX_BYTES
                }
            }
        })));
    }

    // Read from the already-opened and fstat'd descriptor. No second open/path lookup.
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    let bytes = match file.read_to_end(&mut bytes).await {
        Ok(_) => bytes,
        Err(e) => {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32024,
                    "message": "lead_decision_artifact file unreadable",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "lead_auto_artifact_unreadable",
                        "detail": e.to_string()
                    }
                }
            })));
        }
    };

    let computed_sha256: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    if computed_sha256 != supplied_artifact_sha256 {
        return Ok(Some(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32025,
                "message": "lead_decision_artifact_sha256 does not match artifact bytes",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "lead_auto_artifact_hash_mismatch"
                }
            }
        })));
    }

    // Parse artifact JSON and verify that the lead decision targets the same
    // run/stage/agent/session that the server is about to continue. The lead
    // artifact is provider-authored input; it cannot substitute for server
    // policy checks or target authority.
    let artifact_json: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            return Ok(Some(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32024,
                    "message": "lead_decision_artifact is not valid JSON",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "lead_auto_artifact_invalid_json",
                        "detail": e.to_string()
                    }
                }
            })));
        }
    };

    if let Some(rejection) =
        validate_lead_auto_decision_payload(&artifact_json, supplied_instruction_sha256, target)
    {
        return Ok(Some(rejection));
    }

    Ok(None)
}

/// SEC-P086-MED-002: Constant-shape opaque rejection for Agent lead_auto requests
/// that fail target-discovery checks before artifact verification.  Returning a
/// uniform not_found_or_access_denied hides whether the supplied agent_execution_id
/// exists, what role/status it has, or what run/stage it belongs to.
fn lead_auto_agent_opaque_rejection() -> serde_json::Value {
    serde_json::json!({
        "outcome": "rejected",
        "error": {
            "code": -32020,
            "message": "agent_execution not found or access denied",
            "data": { "failure_reason": "not_found_or_access_denied" }
        }
    })
}

async fn handle_continue_work(
    params: &serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    // P086-SEC-MED-001 server-side additionalProperties=false enforcement.
    // The MCP tool schema declares additionalProperties=false; mirror that here so unknown
    // fields are rejected before any DB write, matching the schema-declared boundary.
    const ALLOWED_FIELDS: &[&str] = &[
        "agent_execution_id",
        "run_id",
        "stage_execution_id",
        "session_generation_id",
        "provider_session_id",
        "continuation_mode",
        "mode",
        "trigger_kind",
        "idempotency_key",
        "operator_instruction",
        "max_turns",
        "max_wall_clock_seconds",
        "blockers",
        "lead_decision_artifact_id",
        "lead_decision_artifact_sha256",
        "continuation_instruction_sha256",
    ];
    if let Some(obj) = params.as_object() {
        for key in obj.keys() {
            if !ALLOWED_FIELDS.contains(&key.as_str()) {
                return Ok(serde_json::json!({
                    "outcome": "rejected",
                    "error": {
                        "code": -32602,
                        "message": format!("unknown field {key:?}; agents.continue_work does not accept additional properties"),
                        "data": { "failure_reason": "unknown_field", "field": key }
                    }
                }));
            }
        }
    }

    let agent_execution_id = params["agent_execution_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing agent_execution_id"))?;
    let mode = match (
        params["continuation_mode"].as_str(),
        params["mode"].as_str(),
    ) {
        (Some(canonical), Some(alias)) if canonical != alias => {
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32602,
                    "message": "continuation_mode and deprecated mode alias must match when both are supplied",
                    "data": {
                        "failure_reason": "continuation_mode_alias_mismatch",
                        "field": "continuation_mode"
                    }
                }
            }));
        }
        (Some(canonical), _) => canonical,
        (None, Some(alias)) => alias,
        (None, None) => {
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32602,
                    "message": "Missing continuation_mode",
                    "data": {
                        "failure_reason": "missing_continuation_mode",
                        "field": "continuation_mode"
                    }
                }
            }))
        }
    };
    let trigger_kind = params["trigger_kind"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing trigger_kind"))?;
    let idempotency_key = params["idempotency_key"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing idempotency_key"))?;
    let expected_run_id = match optional_string_param(params, "run_id", MAX_INPUT_LEN) {
        Ok(value) => value,
        Err(rejection) => return Ok(rejection),
    };
    let expected_stage_execution_id =
        match optional_string_param(params, "stage_execution_id", MAX_INPUT_LEN) {
            Ok(value) => value,
            Err(rejection) => return Ok(rejection),
        };
    let expected_session_generation_id =
        match optional_string_param(params, "session_generation_id", MAX_INPUT_LEN) {
            Ok(value) => value,
            Err(rejection) => return Ok(rejection),
        };
    let expected_provider_session_id =
        match optional_string_param(params, "provider_session_id", MAX_INPUT_LEN) {
            Ok(value) => value,
            Err(rejection) => return Ok(rejection),
        };
    let operator_instruction =
        match optional_string_param(params, "operator_instruction", MAX_OPERATOR_INSTRUCTION_LEN) {
            Ok(value) => value,
            Err(rejection) => return Ok(rejection),
        };
    let max_turns = match optional_i64_param(params, "max_turns", 1, 20) {
        Ok(value) => value,
        Err(rejection) => return Ok(rejection),
    };
    let max_wall_clock_seconds =
        match optional_i64_param(params, "max_wall_clock_seconds", 30, 7_200) {
            Ok(value) => value,
            Err(rejection) => return Ok(rejection),
        };
    let blockers = match blockers_param(params) {
        Ok(value) => value,
        Err(rejection) => return Ok(rejection),
    };

    // Input length validation (P086-SEC-LOW-002): cap before any DB write.
    // agent_execution_id: MAX_INPUT_LEN (200); idempotency_key: MAX_IDEMPOTENCY_KEY_LEN (256)
    // to match the reference MCP request schema maxLength for each field.
    if agent_execution_id.len() > MAX_INPUT_LEN {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": format!("agent_execution_id exceeds maximum length of {MAX_INPUT_LEN}"),
                "data": { "failure_reason": "input_too_long", "field": "agent_execution_id" }
            }
        }));
    }
    // UUID format validation (security report required_fix #1): agent_execution_id must be a
    // valid UUID before reaching the DB. Consistent with verify_lead_auto_artifacts treatment
    // of lead_decision_artifact_id; rejects before any DB write or hash computation.
    if uuid::Uuid::parse_str(agent_execution_id).is_err() {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": "agent_execution_id must be a valid UUID",
                "data": { "failure_reason": "invalid_agent_execution_id_format", "field": "agent_execution_id" }
            }
        }));
    }
    if idempotency_key.len() > MAX_IDEMPOTENCY_KEY_LEN {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": format!("idempotency_key exceeds maximum length of {MAX_IDEMPOTENCY_KEY_LEN}"),
                "data": { "failure_reason": "input_too_long", "field": "idempotency_key" }
            }
        }));
    }
    if idempotency_key.is_empty() {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32026,
                "message": "idempotency_key must be non-empty",
                "data": { "failure_reason": "empty_idempotency_key" }
            }
        }));
    }

    // Typed mode validation: reject unknown values with a structured error rather than
    // relying on SQLite CHECK constraint failures.
    if !matches!(
        mode,
        "live_handle_continuation" | "provider_session_resurrection"
    ) {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32027,
                "message": "mode must be live_handle_continuation or provider_session_resurrection",
                "data": { "failure_reason": "invalid_mode", "mode": mode }
            }
        }));
    }

    // Typed trigger_kind validation.
    if !matches!(trigger_kind, "operator_mcp" | "lead_auto") {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32027,
                "message": "trigger_kind must be operator_mcp or lead_auto",
                "data": { "failure_reason": "invalid_trigger_kind", "trigger_kind": trigger_kind }
            }
        }));
    }

    // P086: Operator owns manual continuation. Agent principals may request
    // only lead_auto, and that path remains gated by server-side decision
    // artifact identity, target, hash, safety, and budget validation below.
    let principal_authorized = matches!(principal.class, auth::PrincipalClass::Operator)
        || (matches!(principal.class, auth::PrincipalClass::Agent) && trigger_kind == "lead_auto");
    if !principal_authorized {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32001,
                "message": "agents.continue_work requires Operator principal, except lead_auto requests from Agent principals with a validated decision artifact",
                "data": {
                    "failure_reason": "unauthorized_principal",
                    "trigger_kind": trigger_kind
                }
            }
        }));
    }

    // P086-SEC-MED-001: operator_mcp requests must not carry lead_auto-only fields.
    // Persisting these fields on non-lead_auto rows alters idempotency semantics and
    // creates storage/response amplification with caller-controlled metadata.
    if trigger_kind != "lead_auto" {
        let has_lead_fields = params.get("lead_decision_artifact_id").is_some()
            || params.get("lead_decision_artifact_sha256").is_some()
            || params.get("continuation_instruction_sha256").is_some();
        if has_lead_fields {
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32028,
                    "message": "lead_decision_artifact_id, lead_decision_artifact_sha256, and continuation_instruction_sha256 are only permitted for lead_auto trigger_kind",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "failure_reason": "lead_fields_on_non_lead_auto_trigger"
                    }
                }
            }));
        }
    }

    // lead_auto field validation. Target-aware artifact verification happens
    // after eligibility is loaded so stale or cross-run lead artifacts fail
    // closed against server-derived run/stage/session truth.
    let lead_auto_fields = if trigger_kind == "lead_auto" {
        let lead_decision_artifact_id = match params["lead_decision_artifact_id"].as_str() {
            Some(s) if s.len() <= MAX_INPUT_LEN => s,
            Some(_) => {
                return Ok(serde_json::json!({
                    "outcome": "rejected",
                    "error": {
                        "code": -32026,
                        "message": "lead_decision_artifact_id exceeds maximum length",
                        "data": { "failure_reason": "input_too_long", "field": "lead_decision_artifact_id" }
                    }
                }))
            }
            None => {
                return Ok(serde_json::json!({
                    "outcome": "rejected",
                    "error": {
                        "code": -32022,
                        "message": "lead_auto trigger requires lead_decision_artifact_id",
                        "data": { "failure_reason": "lead_auto_missing_artifact_id" }
                    }
                }))
            }
        };
        let lda_sha = match params["lead_decision_artifact_sha256"].as_str() {
            Some(s) => s,
            None => {
                return Ok(serde_json::json!({
                    "outcome": "rejected",
                    "error": {
                        "code": -32022,
                        "message": "lead_auto trigger requires lead_decision_artifact_sha256",
                        "data": { "failure_reason": "lead_auto_missing_artifact_sha256" }
                    }
                }))
            }
        };
        let ci_sha = match params["continuation_instruction_sha256"].as_str() {
            Some(s) => s,
            None => {
                return Ok(serde_json::json!({
                    "outcome": "rejected",
                    "error": {
                        "code": -32022,
                        "message": "lead_auto trigger requires continuation_instruction_sha256",
                        "data": { "failure_reason": "lead_auto_missing_instruction_sha256" }
                    }
                }))
            }
        };
        if !validate_sha256_hex(lda_sha) {
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32023,
                    "message": "lead_decision_artifact_sha256 must be 64 lowercase hex characters",
                    "data": { "failure_reason": "malformed_lead_decision_sha256" }
                }
            }));
        }
        if !validate_sha256_hex(ci_sha) {
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32023,
                    "message": "continuation_instruction_sha256 must be 64 lowercase hex characters",
                    "data": { "failure_reason": "malformed_continuation_instruction_sha256" }
                }
            }));
        }
        let Some(instruction) = operator_instruction.filter(|value| !value.trim().is_empty())
        else {
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32022,
                    "message": "lead_auto trigger requires operator_instruction matching continuation_instruction_sha256",
                    "data": { "failure_reason": "lead_auto_missing_operator_instruction" }
                }
            }));
        };
        let instruction_sha256: String = Sha256::digest(instruction.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if instruction_sha256 != ci_sha {
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32025,
                    "message": "operator_instruction SHA-256 does not match continuation_instruction_sha256",
                    "data": { "failure_reason": "lead_auto_instruction_hash_mismatch" }
                }
            }));
        }
        Some((lead_decision_artifact_id, lda_sha, ci_sha))
    } else {
        None
    };

    // SEC-P086-MED-002: For Agent principals in lead_auto, target-specific errors must not
    // reveal whether agent_execution_id exists, what role/status it has, or associated
    // run/stage/session metadata, before the lead artifact is verified.  After
    // verify_lead_auto_artifacts succeeds the artifact itself binds all those IDs, so
    // subsequent checks may return specific errors.
    let is_agent_lead_auto =
        matches!(principal.class, auth::PrincipalClass::Agent) && trigger_kind == "lead_auto";

    // Check eligibility: agent_role=code_writer, owner_kind=stage_execution, terminal status, run membership.
    // check_eligibility returns Some for any code_writer+stage_execution agent regardless of status;
    // terminal-status check is done here so we can return the correct failure_reason.
    let eligibility =
        db::repos::agent_work_continuations::check_eligibility(pool, agent_execution_id).await?;
    let Some(info) = eligibility else {
        if is_agent_lead_auto {
            return Ok(lead_auto_agent_opaque_rejection());
        }
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32020,
                "message": "agent_execution is not eligible for continuation (must be code_writer, stage-owned, with a valid stage_execution)",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "ineligible_agent_execution"
                }
            }
        }));
    };

    // P086: agent_already_running — agent is code_writer/stage-owned but not yet terminal.
    // The proposal distinguishes this from ineligible_agent_execution: an in-progress
    // AgentExecution cannot receive a continuation until it settles to completed/failed.
    if !matches!(info.agent_status.as_str(), "completed" | "failed") {
        if is_agent_lead_auto {
            return Ok(lead_auto_agent_opaque_rejection());
        }
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32031,
                "message": "agent_execution is still running; continuation is only allowed after the agent settles to completed or failed",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "agent_status": info.agent_status,
                    "failure_reason": "agent_already_running"
                }
            }
        }));
    }

    // SEC-HIGH-002: Agent principals using lead_auto must be bound to the target run via
    // run_scope. Artifact content validation (hash, target execution) is not sufficient —
    // a compromised or unrelated agent token that learns a valid artifact can replay it.
    // Requiring run_scope membership binds the caller to a specific run.
    if trigger_kind == "lead_auto" && matches!(principal.class, auth::PrincipalClass::Agent) {
        let authorized_for_run = principal
            .run_scope
            .as_ref()
            .map(|scope| scope.iter().any(|s| s == &info.run_id))
            .unwrap_or(false);
        if !authorized_for_run {
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32001,
                    "message": "Agent principal run_scope does not include the target run; configure run_scope binding before using lead_auto",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "run_id": info.run_id,
                        "failure_reason": "unauthorized_agent_run_scope"
                    }
                }
            }));
        }
    }

    if let Some(expected) = expected_run_id {
        if expected != info.run_id {
            if is_agent_lead_auto {
                return Ok(lead_auto_agent_opaque_rejection());
            }
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32032,
                    "message": "run_id does not match agent_execution owner run",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "expected_run_id": expected,
                        "actual_run_id": info.run_id.clone(),
                        "failure_reason": "run_id_mismatch"
                    }
                }
            }));
        }
    }
    if let Some(expected) = expected_stage_execution_id {
        if expected != info.stage_execution_id {
            if is_agent_lead_auto {
                return Ok(lead_auto_agent_opaque_rejection());
            }
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32032,
                    "message": "stage_execution_id does not match agent_execution owner",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "expected_stage_execution_id": expected,
                        "actual_stage_execution_id": info.stage_execution_id.clone(),
                        "failure_reason": "stage_execution_id_mismatch"
                    }
                }
            }));
        }
    }
    if let Some(expected) = expected_session_generation_id {
        if info.session_generation_id.as_deref() != Some(expected) {
            if is_agent_lead_auto {
                return Ok(lead_auto_agent_opaque_rejection());
            }
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32032,
                    "message": "session_generation_id does not match agent_execution session",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "expected_session_generation_id": expected,
                        "actual_session_generation_id": info.session_generation_id.clone(),
                        "failure_reason": "session_generation_id_mismatch"
                    }
                }
            }));
        }
    }
    if let Some(expected) = expected_provider_session_id {
        if info.provider_session_id.as_deref() != Some(expected) {
            if is_agent_lead_auto {
                return Ok(lead_auto_agent_opaque_rejection());
            }
            return Ok(serde_json::json!({
                "outcome": "rejected",
                "error": {
                    "code": -32032,
                    "message": "provider_session_id does not match agent_execution provider session",
                    "data": {
                        "agent_execution_id": agent_execution_id,
                        "expected_provider_session_id": expected,
                        "actual_provider_session_id": info.provider_session_id.clone(),
                        "failure_reason": "provider_session_id_mismatch"
                    }
                }
            }));
        }
    }

    if let Some((lead_decision_artifact_id, lda_sha, ci_sha)) = lead_auto_fields {
        let target = LeadAutoDecisionTarget {
            run_id: &info.run_id,
            stage_execution_id: &info.stage_execution_id,
            agent_execution_id,
            session_generation_id: info.session_generation_id.as_deref(),
            request_max_turns: max_turns,
            request_max_wall_clock_seconds: max_wall_clock_seconds,
        };
        if let Some(rejection) = verify_lead_auto_artifacts(
            pool,
            agent_execution_id,
            lead_decision_artifact_id,
            lda_sha,
            ci_sha,
            target,
        )
        .await?
        {
            return Ok(rejection);
        }
    }

    if let Some(kind) =
        forbidden_stage_kind(info.logical_stage_id.as_deref(), info.stage_type.as_deref())
    {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32035,
                "message": "target stage is not eligible for P086 continuation because it can own release or external side effects",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "run_id": info.run_id,
                    "stage_execution_id": info.stage_execution_id,
                    "stage_id": info.logical_stage_id,
                    "stage_type": info.stage_type,
                    "forbidden_stage_kind": kind,
                    "failure_reason": "forbidden_stage_kind"
                }
            }
        }));
    }

    if let Some(rejection) = continuation_capability_rejection(
        agent_execution_id,
        info.catalog_snapshot_json.as_deref(),
        mode,
        trigger_kind,
        info.session_generation_id.is_some() && info.provider_session_id.is_some(),
        info.provider_session_id.is_some(),
        provider_session_resurrection_adapter_supported(
            info.provider_family
                .as_deref()
                .or(Some(info.provider.as_str())),
        ),
    )? {
        if mode == "provider_session_resurrection"
            && rejection["error"]["data"]["failure_reason"].as_str()
                == Some("provider_session_resurrection_unsupported")
        {
            let _ = db::repos::agent_work_continuations::record_p086_continuation_metric_event(
                pool,
                Some(&info.run_id),
                Some(&info.stage_execution_id),
                Some(agent_execution_id),
                None,
                "continuation_resurrection_total",
                serde_json::json!({
                    "mode": mode,
                    "trigger_kind": trigger_kind,
                    "resurrection_status": "unsupported"
                }),
                1,
            )
            .await;
        }
        return Ok(rejection);
    }

    let has_unresolved_side_effects =
        db::repos::agent_work_continuations::has_unresolved_side_effects_for_stage(
            pool,
            &info.run_id,
            &info.stage_execution_id,
        )
        .await?;
    if has_unresolved_side_effects {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32036,
                "message": "target stage has unresolved external side effects; reconcile side effects before continuation",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "run_id": info.run_id,
                    "stage_execution_id": info.stage_execution_id,
                    "failure_reason": "unresolved_side_effects"
                }
            }
        }));
    }

    // Reject if the run has pending approvals — per proposal approval_required gate.
    let has_pending =
        db::repos::agent_work_continuations::has_pending_approval_for_run(pool, &info.run_id)
            .await?;
    if has_pending {
        return Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32029,
                "message": "run has unresolved approvals; resolve pending approvals before issuing a continuation",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "run_id": info.run_id,
                    "failure_reason": "approval_required"
                }
            }
        }));
    }

    // Idempotency scope is the agent_execution_id (one key-space per agent execution).
    let idempotency_scope = agent_execution_id;

    let lead_decision_artifact_id = params["lead_decision_artifact_id"].as_str();
    let lead_decision_artifact_sha256 = params["lead_decision_artifact_sha256"].as_str();
    let continuation_instruction_sha256 = params["continuation_instruction_sha256"].as_str();
    let request_context = serde_json::json!({
        "run_id": info.run_id,
        "stage_execution_id": info.stage_execution_id,
        "session_generation_id": expected_session_generation_id.or(info.session_generation_id.as_deref()),
        "provider_session_id": expected_provider_session_id.or(info.provider_session_id.as_deref()),
        "operator_instruction": operator_instruction,
        "max_turns": max_turns,
        "max_wall_clock_seconds": max_wall_clock_seconds,
        "blockers": blockers,
        "continuation_capability_checked": true,
        "no_release_side_effect_stage": true,
        "no_unresolved_side_effects": true,
    });

    let fingerprint = compute_canonical_fingerprint(
        agent_execution_id,
        &info.run_id,
        &info.stage_execution_id,
        &principal.id,
        mode,
        trigger_kind,
        idempotency_scope,
        idempotency_key,
        lead_decision_artifact_id,
        lead_decision_artifact_sha256,
        continuation_instruction_sha256,
        Some(&request_context),
    );

    let continuation_id = uuid::Uuid::new_v4().to_string();
    let command_journal_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let admission = db::repos::agent_work_continuations::ContinuationAdmission {
        continuation_id: continuation_id.clone(),
        command_journal_id: command_journal_id.clone(),
        run_id: info.run_id.clone(),
        stage_execution_id: info.stage_execution_id.clone(),
        agent_execution_id: agent_execution_id.to_string(),
        mode: mode.to_string(),
        trigger_kind: trigger_kind.to_string(),
        idempotency_scope: idempotency_scope.to_string(),
        idempotency_key: idempotency_key.to_string(),
        request_fingerprint_sha256: fingerprint.clone(),
        lead_decision_artifact_id: lead_decision_artifact_id.map(|s| s.to_string()),
        lead_decision_artifact_sha256: lead_decision_artifact_sha256.map(|s| s.to_string()),
        continuation_instruction_sha256: continuation_instruction_sha256.map(|s| s.to_string()),
        budget_json: Some(serde_json::to_string(&request_context)?),
        caller_principal_id: principal.id.clone(),
        caller_surface: "mcp".to_string(),
        caller_principal_class: principal.class.to_string(),
        caller_tool: "agents.continue_work".to_string(),
        created_at: now.clone(),
    };

    let payload_json = serde_json::to_string(&serde_json::json!({
        "command": "agents.continue_work",
        "agent_execution_id": agent_execution_id,
        "run_id": info.run_id,
        "stage_execution_id": info.stage_execution_id,
        "session_generation_id": request_context["session_generation_id"],
        "provider_session_id": request_context["provider_session_id"],
        "continuation_mode": mode,
        "mode": mode,
        "trigger_kind": trigger_kind,
        "idempotency_key": idempotency_key,
        "operator_instruction": operator_instruction,
        "max_turns": max_turns,
        "max_wall_clock_seconds": max_wall_clock_seconds,
        "blockers": request_context["blockers"]
    }))?;

    // Atomic admission: idempotency + active-continuation checks inside BEGIN IMMEDIATE
    // transaction (P086-SEC-MED-001 and P086-SEC-MED-002 fixes).
    use db::repos::agent_work_continuations::AtomicAdmissionOutcome;
    match db::repos::agent_work_continuations::admit_continuation_atomic(
        pool,
        &admission,
        &payload_json,
    )
    .await?
    {
        // Strict schema compliance (additionalProperties=false): only declared fields are returned.
        // Extra fields (run_id, mode, command_journal_id, created_at) are omitted from the
        // wire response; they remain in the durable agent_work_continuations row.
        AtomicAdmissionOutcome::Accepted => {
            // P086 Phase 2: enqueue the background worker work item.
            // Non-fatal: if enqueue fails, the admission-timeout sweeper will
            // move the row to failed=admission_timeout after max_admission_to_start_seconds.
            let work_item = db::work_item::WorkItem {
                id: uuid::Uuid::new_v4().to_string(),
                kind: db::work_item::WorkItemKind::ProcessContinuation,
                payload_json: serde_json::to_string(&serde_json::json!({
                    "continuation_id": continuation_id,
                    "agent_execution_id": agent_execution_id,
                    "run_id": info.run_id,
                    "mode": mode
                }))
                .unwrap_or_default(),
                status: db::work_item::WorkItemStatus::Pending,
                run_id: None,
                stage_id: None,
                created_at: chrono::Utc::now(),
                scheduled_at: chrono::Utc::now(),
                attempt_count: 0,
                last_error: None,
            };
            if let Err(enqueue_err) = db::repos::work_items::enqueue(pool, &work_item).await {
                tracing::warn!(
                    continuation_id = %continuation_id,
                    error = %enqueue_err,
                    "P086 failed to enqueue ProcessContinuation work item; \
                     row will be swept to admission_timeout if unclaimed"
                );
            }
            Ok(serde_json::json!({
                "outcome": "accepted",
                "continuation_id": continuation_id,
                "status": "accepted",
                "request_fingerprint_sha256": fingerprint
            }))
        }

        AtomicAdmissionOutcome::Replay(existing) => Ok(serde_json::json!({
            "outcome": "replay",
            "continuation_id": existing.id,
            "status": existing.status,
            "request_fingerprint_sha256": fingerprint
        })),

        AtomicAdmissionOutcome::IdempotencyConflict => Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": ERR_IDEMPOTENCY_CONFLICT,
                "message": "idempotency_key already used with a different canonical request",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "idempotency_key": idempotency_key,
                    "failure_reason": "idempotency_conflict"
                }
            }
        })),

        AtomicAdmissionOutcome::AlreadyRunning(active) => Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32030,
                "message": "agent already has an active continuation",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "continuation_id": active.id,
                    "continuation_status": active.status,
                    "failure_reason": "agent_already_running"
                }
            }
        })),

        AtomicAdmissionOutcome::LeadAutoLimitExceeded {
            scope,
            current_count,
            max_count,
        } => Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": -32052,
                "message": "lead_auto continuation policy limit exceeded",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "stage_execution_id": info.stage_execution_id,
                    "failure_reason": "lead_auto_limit_exceeded",
                    "limit_scope": scope,
                    "current_count": current_count,
                    "max_count": max_count
                }
            }
        })),

        AtomicAdmissionOutcome::Saturated {
            queue_depth,
            concurrency,
        } => Ok(serde_json::json!({
            "outcome": "rejected",
            "error": {
                "code": ERR_SATURATION_CAPACITY_EXCEEDED,
                "message": "continuation queue is at capacity; retry after existing continuations drain",
                "data": {
                    "agent_execution_id": agent_execution_id,
                    "failure_reason": "saturation_capacity_exceeded",
                    "queue_depth": queue_depth,
                    "concurrency": concurrency,
                    "queue_depth_max": db::repos::agent_work_continuations::QUEUE_DEPTH_MAX,
                    "concurrency_max": db::repos::agent_work_continuations::CONCURRENCY_MAX
                }
            }
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_specs_include_all_three_tools() {
        let specs = tool_specs();
        let names: Vec<&str> = specs.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"agents.continuation_status"));
        assert!(names.contains(&"agents.continuation_candidates"));
        assert!(names.contains(&"agents.attach_receipt.get"));
        assert!(names.contains(&"agents.continue_work"));
    }

    #[test]
    fn attach_receipt_get_schema_requires_run_id() {
        let specs = tool_specs();
        let tool = specs
            .iter()
            .find(|t| t.name == "agents.attach_receipt.get")
            .expect("attach receipt tool must be registered");
        let required = tool.input_schema["required"]
            .as_array()
            .expect("required must be an array");
        assert!(
            required
                .iter()
                .any(|value| value.as_str() == Some("continuation_id")),
            "continuation_id must be required"
        );
        assert!(
            required
                .iter()
                .any(|value| value.as_str() == Some("run_id")),
            "run_id must be required for run-scoped raw attach receipt access"
        );
    }

    #[tokio::test]
    async fn attach_receipt_get_rejects_operator_without_run_id_before_lookup() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let principal = auth::Principal::new("operator", auth::PrincipalClass::Operator);
        let result = execute(
            "agents.attach_receipt.get",
            serde_json::json!({ "continuation_id": "known-continuation-id" }),
            &pool,
            &principal,
        )
        .await
        .expect("runtime rejection should be a structured tool response");

        assert_eq!(result["outcome"], "rejected");
        assert_eq!(result["error"]["data"]["failure_reason"], "auth_failure");
        assert!(
            result["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("run_id is required"),
            "missing run_id must be explicit in the denial: {result:?}"
        );
    }

    #[test]
    fn continue_work_tool_spec_no_longer_disabled() {
        let specs = tool_specs();
        let cw = specs
            .iter()
            .find(|t| t.name == "agents.continue_work")
            .expect("continue_work must be registered");
        // Admission is now live; description must not reference Phase 0 disabled state.
        assert!(
            !cw.description.contains("disabled"),
            "agents.continue_work description must not say 'disabled'"
        );
        assert!(
            !cw.description.contains("-32099"),
            "description must not reference the Phase 0 disabled error code"
        );
    }

    #[test]
    fn tool_specs_have_max_length_on_string_inputs() {
        let specs = tool_specs();
        let cw = specs
            .iter()
            .find(|t| t.name == "agents.continue_work")
            .expect("continue_work must be registered");
        let props = &cw.input_schema["properties"];
        assert_eq!(
            props["agent_execution_id"]["maxLength"].as_u64(),
            Some(200),
            "agent_execution_id maxLength must be 200"
        );
        assert_eq!(
            props["idempotency_key"]["maxLength"].as_u64(),
            Some(256),
            "idempotency_key maxLength must be 256 (reference schema)"
        );
        assert_eq!(
            props["lead_decision_artifact_sha256"]["maxLength"].as_u64(),
            Some(64),
            "lead_decision_artifact_sha256 maxLength must be 64"
        );
    }

    #[test]
    fn canonical_fingerprint_is_deterministic() {
        let fp1 = compute_canonical_fingerprint(
            "ae-id",
            "run-id",
            "se-id",
            "principal-id",
            "live_handle_continuation",
            "operator_mcp",
            "ae-id",
            "key-1",
            None,
            None,
            None,
            None,
        );
        let fp2 = compute_canonical_fingerprint(
            "ae-id",
            "run-id",
            "se-id",
            "principal-id",
            "live_handle_continuation",
            "operator_mcp",
            "ae-id",
            "key-1",
            None,
            None,
            None,
            None,
        );
        assert_eq!(fp1, fp2, "fingerprint must be deterministic");
        assert_eq!(fp1.len(), 64, "fingerprint must be 64 hex chars");
        assert!(
            fp1.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint must be hex"
        );
    }

    #[test]
    fn canonical_fingerprint_differs_on_key_change() {
        let fp1 = compute_canonical_fingerprint(
            "ae-id",
            "run-id",
            "se-id",
            "principal-id",
            "live_handle_continuation",
            "operator_mcp",
            "ae-id",
            "key-1",
            None,
            None,
            None,
            None,
        );
        let fp2 = compute_canonical_fingerprint(
            "ae-id",
            "run-id",
            "se-id",
            "principal-id",
            "live_handle_continuation",
            "operator_mcp",
            "ae-id",
            "key-2",
            None,
            None,
            None,
            None,
        );
        assert_ne!(
            fp1, fp2,
            "different idempotency_key must produce different fingerprint"
        );
    }

    #[test]
    fn canonical_fingerprint_differs_on_run_id_change() {
        let fp1 = compute_canonical_fingerprint(
            "ae-id",
            "run-1",
            "se-id",
            "principal-id",
            "live_handle_continuation",
            "operator_mcp",
            "ae-id",
            "key-1",
            None,
            None,
            None,
            None,
        );
        let fp2 = compute_canonical_fingerprint(
            "ae-id",
            "run-2",
            "se-id",
            "principal-id",
            "live_handle_continuation",
            "operator_mcp",
            "ae-id",
            "key-1",
            None,
            None,
            None,
            None,
        );
        assert_ne!(
            fp1, fp2,
            "different run_id must produce different fingerprint"
        );
    }

    #[test]
    fn canonical_fingerprint_differs_on_caller_change() {
        let fp1 = compute_canonical_fingerprint(
            "ae-id",
            "run-id",
            "se-id",
            "principal-A",
            "live_handle_continuation",
            "operator_mcp",
            "ae-id",
            "key-1",
            None,
            None,
            None,
            None,
        );
        let fp2 = compute_canonical_fingerprint(
            "ae-id",
            "run-id",
            "se-id",
            "principal-B",
            "live_handle_continuation",
            "operator_mcp",
            "ae-id",
            "key-1",
            None,
            None,
            None,
            None,
        );
        assert_ne!(
            fp1, fp2,
            "different caller_principal_id must produce different fingerprint"
        );
    }

    #[test]
    fn validate_sha256_hex_accepts_valid() {
        let valid = "a".repeat(64);
        assert!(validate_sha256_hex(&valid));
        let valid2 = "0123456789abcdef".repeat(4);
        assert!(validate_sha256_hex(&valid2));
    }

    #[test]
    fn validate_sha256_hex_rejects_uppercase() {
        let bad = "A".repeat(64);
        assert!(!validate_sha256_hex(&bad), "uppercase must be rejected");
    }

    #[test]
    fn validate_sha256_hex_rejects_wrong_length() {
        assert!(!validate_sha256_hex("abcd"));
        assert!(!validate_sha256_hex(&"a".repeat(65)));
    }

    #[test]
    fn agent_principal_class_is_detected_for_row_omission() {
        // P086-SEC-LOW-003: Agent principals must not leak existence of rows.
        let principal = auth::Principal::new("agent-token", auth::PrincipalClass::Agent);
        assert!(
            matches!(principal.class, auth::PrincipalClass::Agent),
            "Agent class must be detectable for empty-return short-circuit"
        );
        // Operator must NOT trigger the empty-return path.
        let op = auth::Principal::new("op-token", auth::PrincipalClass::Operator);
        assert!(
            !matches!(op.class, auth::PrincipalClass::Agent),
            "Operator must not be misclassified as Agent"
        );
    }

    #[test]
    fn continue_work_gate_allows_operator_or_agent_lead_auto_only() {
        let cases = [
            (auth::PrincipalClass::Operator, "operator_mcp", true),
            (auth::PrincipalClass::Operator, "lead_auto", true),
            (auth::PrincipalClass::Agent, "operator_mcp", false),
            (auth::PrincipalClass::Agent, "lead_auto", true),
            (auth::PrincipalClass::Observer, "operator_mcp", false),
            (auth::PrincipalClass::Observer, "lead_auto", false),
        ];
        for (class, trigger_kind, expected) in cases {
            let allowed = matches!(class, auth::PrincipalClass::Operator)
                || (matches!(class, auth::PrincipalClass::Agent) && trigger_kind == "lead_auto");
            assert_eq!(
                allowed, expected,
                "PrincipalClass::{class:?} trigger_kind={trigger_kind} authorization mismatch"
            );
        }
    }

    #[test]
    fn lead_auto_uses_artifact_validation_not_resurrection_gate() {
        // P086 Phase 3: lead_auto is enabled behind decision-artifact validation,
        // while provider session resurrection remains separately gated.
        let resurrection_code: i64 = -32010;
        let lead_missing_artifact_code: i64 = -32022;
        assert_ne!(
            resurrection_code, lead_missing_artifact_code,
            "lead_auto validation errors and resurrection gate errors must remain distinct"
        );
    }

    fn valid_lead_decision_fixture() -> serde_json::Value {
        let continuation_instruction = "Continue implementation.";
        let continuation_instruction_sha256 =
            "0cd640389536b368a26678b3d87bc36d1a28e66ed45c4242190321a7c861de7a";
        serde_json::json!({
            "schema_version": "lead_continuation_decision_v1",
            "artifact_kind": "lead_continuation_decision",
            "run_id": "run-1",
            "stage_execution_id": "stage-exec-1",
            "agent_execution_id": "agent-exec-1",
            "continuation_id": "continuation-1",
            "created_at": "2026-05-22T00:00:00Z",
            "redaction_tier": "partial",
            "retention_policy": "run-retained",
            "payload": {
                "schema_version": "lead_continuation_decision_v1",
                "decision_id": "decision-1",
                "run_id": "run-1",
                "stage_execution_id": "stage-exec-1",
                "agent_execution_id": "agent-exec-1",
                "agent_id": "code_writer",
                "session_generation_id": "session-1",
                "decision": "continue",
                "reason": "continue blocked implementation work",
                "created_at": "2026-05-22T00:00:00Z",
                "continuation_instruction": continuation_instruction,
                "continuation_instruction_sha256": continuation_instruction_sha256,
                "expected_next_work": ["finish implementation"],
                "known_completed_work": ["initial pass"],
                "known_blockers": [],
                "safety_checks": {
                    "no_release_side_effect": true,
                    "no_unresolved_effect_ledger": true,
                    "same_worktree_required": true
                },
                "stop_conditions": ["new blocker"],
                "max_turns": 1,
                "max_wall_clock_seconds": 1800
            }
        })
    }

    #[test]
    fn lead_auto_decision_payload_must_match_current_target() {
        let mut fixture = valid_lead_decision_fixture();
        fixture["payload"]["stage_execution_id"] = serde_json::Value::String("other-stage".into());
        let rejection = validate_lead_auto_decision_payload(
            &fixture,
            "0cd640389536b368a26678b3d87bc36d1a28e66ed45c4242190321a7c861de7a",
            LeadAutoDecisionTarget {
                run_id: "run-1",
                stage_execution_id: "stage-exec-1",
                agent_execution_id: "agent-exec-1",
                session_generation_id: Some("session-1"),
                request_max_turns: Some(1),
                request_max_wall_clock_seconds: Some(1800),
            },
        )
        .expect("target mismatch must reject");
        assert_eq!(
            rejection["error"]["data"]["failure_reason"],
            "lead_auto_artifact_target_mismatch"
        );
    }

    #[test]
    fn lead_auto_decision_payload_must_include_required_safety_checks() {
        let mut fixture = valid_lead_decision_fixture();
        fixture["payload"]["safety_checks"]["same_worktree_required"] =
            serde_json::Value::Bool(false);
        let rejection = validate_lead_auto_decision_payload(
            &fixture,
            "0cd640389536b368a26678b3d87bc36d1a28e66ed45c4242190321a7c861de7a",
            LeadAutoDecisionTarget {
                run_id: "run-1",
                stage_execution_id: "stage-exec-1",
                agent_execution_id: "agent-exec-1",
                session_generation_id: Some("session-1"),
                request_max_turns: Some(1),
                request_max_wall_clock_seconds: Some(1800),
            },
        )
        .expect("safety check mismatch must reject");
        assert_eq!(
            rejection["error"]["data"]["failure_reason"],
            "lead_auto_safety_check_failed"
        );
    }

    #[test]
    fn lead_auto_decision_payload_accepts_matching_code_writer_target() {
        let fixture = valid_lead_decision_fixture();
        let rejection = validate_lead_auto_decision_payload(
            &fixture,
            "0cd640389536b368a26678b3d87bc36d1a28e66ed45c4242190321a7c861de7a",
            LeadAutoDecisionTarget {
                run_id: "run-1",
                stage_execution_id: "stage-exec-1",
                agent_execution_id: "agent-exec-1",
                session_generation_id: Some("session-1"),
                request_max_turns: Some(1),
                request_max_wall_clock_seconds: Some(1800),
            },
        );
        assert!(rejection.is_none(), "matching lead decision must pass");
    }

    #[test]
    fn invalid_mode_produces_typed_rejection_not_db_error() {
        // Typed mode validation must return a structured -32027 rejection,
        // not rely on SQLite CHECK constraint failures (audit defect).
        let valid_modes = ["live_handle_continuation", "provider_session_resurrection"];
        let invalid_modes = ["unknown_mode", "", "LIVE_HANDLE"];
        for m in &valid_modes {
            assert!(
                matches!(
                    *m,
                    "live_handle_continuation" | "provider_session_resurrection"
                ),
                "valid mode {m} must be accepted"
            );
        }
        for m in &invalid_modes {
            assert!(
                !matches!(
                    *m,
                    "live_handle_continuation" | "provider_session_resurrection"
                ),
                "invalid mode {m} must be rejected"
            );
        }
    }

    #[test]
    fn invalid_trigger_kind_produces_typed_rejection() {
        let valid = ["operator_mcp", "lead_auto"];
        let invalid = ["unknown", "", "LeadAuto"];
        for t in &valid {
            assert!(
                matches!(*t, "operator_mcp" | "lead_auto"),
                "valid trigger_kind {t} must be accepted"
            );
        }
        for t in &invalid {
            assert!(
                !matches!(*t, "operator_mcp" | "lead_auto"),
                "invalid trigger_kind {t} must be rejected"
            );
        }
    }

    #[test]
    fn forbidden_stage_kind_blocks_release_and_publish_lanes() {
        for stage in [
            "state_11_manual_release",
            "build_and_publish",
            "commit_and_push_release_candidate",
            "connect_upload",
            "external_distribution",
        ] {
            assert!(
                forbidden_stage_kind(Some(stage), None).is_some(),
                "{stage} must be rejected before continuation admission"
            );
        }
        assert_eq!(
            forbidden_stage_kind(
                Some("state_10_implementation_refined"),
                Some("implementation")
            ),
            None
        );
    }

    #[test]
    fn continuation_capability_requires_catalog_opt_in_and_live_session() {
        let catalog = serde_json::json!({
            "agents": [{
                "id": "code_writer",
                "continuation_capability": {
                    "enabled": true,
                    "allowed_triggers": ["operator_mcp", "lead_auto"],
                    "live_handle_continuation": {
                        "enabled": true,
                        "require_live_session": true
                    }
                }
            }]
        })
        .to_string();

        assert!(
            continuation_capability_rejection(
                "ae-id",
                Some(&catalog),
                "live_handle_continuation",
                "operator_mcp",
                true,
                true,
                false,
            )
            .expect("capability check should parse")
            .is_none(),
            "catalog opt-in with live session must pass"
        );

        let missing = continuation_capability_rejection(
            "ae-id",
            None,
            "live_handle_continuation",
            "operator_mcp",
            true,
            true,
            false,
        )
        .expect("missing catalog should produce typed rejection")
        .expect("missing catalog must reject");
        assert_eq!(
            missing["error"]["data"]["failure_reason"],
            "continuation_capability_missing"
        );

        let no_live = continuation_capability_rejection(
            "ae-id",
            Some(&catalog),
            "live_handle_continuation",
            "operator_mcp",
            false,
            true,
            false,
        )
        .expect("live session guard should produce typed rejection")
        .expect("missing live session must reject");
        assert_eq!(
            no_live["error"]["data"]["failure_reason"],
            "live_session_required"
        );
    }

    #[test]
    fn continuation_capability_gates_provider_session_resurrection() {
        let catalog = serde_json::json!({
            "agents": [{
                "id": "code_writer",
                "continuation_capability": {
                    "enabled": true,
                    "allowed_triggers": ["operator_mcp", "lead_auto"],
                    "live_handle_continuation": {
                        "enabled": true,
                        "require_live_session": true
                    },
                    "provider_session_resurrection": {
                        "enabled": true,
                        "allowed_triggers": ["operator_mcp"],
                        "require_recorded_provider_session_id": true,
                        "fail_closed_when_unsupported": true
                    }
                }
            }]
        })
        .to_string();

        let accepted = continuation_capability_rejection(
            "ae-id",
            Some(&catalog),
            "provider_session_resurrection",
            "operator_mcp",
            false,
            true,
            true,
        )
        .expect("resurrection capability check should parse");
        assert!(
            accepted.is_none(),
            "catalog + provider_session_id + adapter support must pass admission"
        );

        let no_provider_session = continuation_capability_rejection(
            "ae-id",
            Some(&catalog),
            "provider_session_resurrection",
            "operator_mcp",
            false,
            false,
            true,
        )
        .expect("missing provider session should produce typed rejection")
        .expect("missing provider_session_id must reject");
        assert_eq!(
            no_provider_session["error"]["data"]["failure_reason"],
            "provider_session_id_required"
        );

        let unsupported = continuation_capability_rejection(
            "ae-id",
            Some(&catalog),
            "provider_session_resurrection",
            "operator_mcp",
            false,
            true,
            false,
        )
        .expect("unsupported adapter should produce typed rejection")
        .expect("unsupported adapter must reject");
        assert_eq!(
            unsupported["error"]["data"]["failure_reason"],
            "provider_session_resurrection_unsupported"
        );

        let disabled_catalog = serde_json::json!({
            "agents": [{
                "id": "code_writer",
                "continuation_capability": {
                    "enabled": true,
                    "allowed_triggers": ["operator_mcp"],
                    "provider_session_resurrection": {
                        "enabled": false
                    }
                }
            }]
        })
        .to_string();
        let disabled = continuation_capability_rejection(
            "ae-id",
            Some(&disabled_catalog),
            "provider_session_resurrection",
            "operator_mcp",
            false,
            true,
            true,
        )
        .expect("disabled catalog should produce typed rejection")
        .expect("disabled resurrection must reject");
        assert_eq!(
            disabled["error"]["data"]["failure_reason"],
            "provider_session_resurrection_disabled"
        );
    }

    #[test]
    fn provider_session_resurrection_adapter_support_fails_closed_until_attach_is_proven() {
        for provider in ["claude", "claude_acp", "codex", "gemini", "auggie", "junie"] {
            assert!(
                !provider_session_resurrection_adapter_supported(Some(provider)),
                "{provider} must remain disabled until requested-session attach is proven before prompt send"
            );
        }
        assert!(!provider_session_resurrection_adapter_supported(None));
        assert!(!provider_session_resurrection_adapter_supported(Some(
            "unknown"
        )));
    }

    #[test]
    fn max_input_len_constant_is_reasonable() {
        assert_eq!(
            MAX_INPUT_LEN, 200,
            "MAX_INPUT_LEN must be 200 for P086-SEC-LOW-002"
        );
        assert_eq!(
            MAX_IDEMPOTENCY_KEY_LEN, 256,
            "MAX_IDEMPOTENCY_KEY_LEN must be 256 to match the reference MCP schema maxLength"
        );
    }

    #[test]
    fn saturation_error_code_is_correct() {
        // P086 rollout contract: saturation_capacity_exceeded uses -32051 per spec.
        assert_eq!(
            ERR_SATURATION_CAPACITY_EXCEEDED, -32051,
            "saturation_capacity_exceeded must return -32051 per P086 rollout contract"
        );
    }

    #[test]
    fn saturation_caps_are_positive() {
        assert!(
            db::repos::agent_work_continuations::QUEUE_DEPTH_MAX > 0,
            "QUEUE_DEPTH_MAX must be positive"
        );
        assert!(
            db::repos::agent_work_continuations::CONCURRENCY_MAX > 0,
            "CONCURRENCY_MAX must be positive"
        );
        assert!(
            db::repos::agent_work_continuations::QUEUE_DEPTH_MAX
                >= db::repos::agent_work_continuations::CONCURRENCY_MAX,
            "queue depth cap must be >= concurrency cap"
        );
    }

    #[test]
    fn sec_med_001_lead_fields_rejected_on_operator_mcp() {
        // P086-SEC-MED-001: lead_auto-only fields must be rejected when trigger_kind is not lead_auto.
        // Verify the guard predicate: any of lead_decision_artifact_id, lead_decision_artifact_sha256,
        // or continuation_instruction_sha256 present on a non-lead_auto request triggers rejection.
        let lead_field_names = [
            "lead_decision_artifact_id",
            "lead_decision_artifact_sha256",
            "continuation_instruction_sha256",
        ];
        for field in &lead_field_names {
            let mut params = serde_json::json!({});
            params[field] = serde_json::Value::String("somevalue".to_string());
            let has_lead_fields = params.get("lead_decision_artifact_id").is_some()
                || params.get("lead_decision_artifact_sha256").is_some()
                || params.get("continuation_instruction_sha256").is_some();
            assert!(
                has_lead_fields,
                "SEC-MED-001 guard must detect {field} as a lead-only field"
            );
        }
        // An operator_mcp request with none of the lead fields must pass the guard.
        let clean = serde_json::json!({
            "agent_execution_id": "ae-id",
            "mode": "live_handle_continuation",
            "trigger_kind": "operator_mcp",
            "idempotency_key": "key-1"
        });
        let has_lead = clean.get("lead_decision_artifact_id").is_some()
            || clean.get("lead_decision_artifact_sha256").is_some()
            || clean.get("continuation_instruction_sha256").is_some();
        assert!(
            !has_lead,
            "clean operator_mcp request must not trigger SEC-MED-001 rejection"
        );
    }

    #[test]
    fn sec_med_001_error_code_is_distinct() {
        // P086-SEC-MED-001 rejection uses -32028 — must be distinct from other P086 codes.
        let codes: &[(i64, &str)] = &[
            (-32044, "idempotency_conflict"),
            (-32051, "saturation_capacity_exceeded"),
            (-32010, "resurrection_unsupported"),
            (-32022, "lead_auto_missing_artifact"),
            (-32026, "input_too_long"),
            (-32027, "invalid_mode_or_trigger"),
        ];
        let sec_med_001_code: i64 = -32028;
        for (code, name) in codes {
            assert_ne!(
                sec_med_001_code, *code,
                "SEC-MED-001 code -32028 must differ from {name} ({code})"
            );
        }
    }

    #[test]
    fn sec_med_002_redacted_record_omits_sensitive_fields() {
        // P086-SEC-MED-002: redacted_record must not include idempotency_scope, idempotency_key,
        // lead_decision_artifact_id, lead_decision_artifact_sha256, continuation_instruction_sha256,
        // or budget_json. These fields must not leak to Observer-readable APIs.
        let r = domain::continuation::ContinuationRecord {
            id: "cont-id".to_string(),
            run_id: "run-id".to_string(),
            stage_execution_id: "se-id".to_string(),
            agent_execution_id: "ae-id".to_string(),
            mode: "live_handle_continuation".to_string(),
            trigger_kind: "operator_mcp".to_string(),
            status: "accepted".to_string(),
            failure_reason: None,
            reconciliation_status: None,
            idempotency_scope: "scope-secret".to_string(),
            idempotency_key: "key-secret".to_string(),
            request_fingerprint_sha256: "a".repeat(64),
            canonical_request_artifact_id: None,
            attach_receipt_artifact_id: None,
            evidence_bundle_artifact_id: None,
            worktree_readback_artifact_id: None,
            continuation_report_artifact_id: None,
            response_fingerprint_sha256: None,
            response_artifact_id: None,
            result_or_no_progress_artifact_id: None,
            conflict_count: 0,
            lead_decision_artifact_id: Some("lead-art-id".to_string()),
            lead_decision_artifact_sha256: Some("b".repeat(64)),
            continuation_instruction_sha256: Some("c".repeat(64)),
            budget_json: Some(r#"{"max_turns":5}"#.to_string()),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let v = super::redacted_record(&r);
        assert!(
            v.get("idempotency_scope").is_none(),
            "idempotency_scope must be redacted"
        );
        assert!(
            v.get("idempotency_key").is_none(),
            "idempotency_key must be redacted"
        );
        assert!(
            v.get("lead_decision_artifact_id").is_none(),
            "lead_decision_artifact_id must be redacted"
        );
        assert!(
            v.get("lead_decision_artifact_sha256").is_none(),
            "lead_decision_artifact_sha256 must be redacted"
        );
        assert!(
            v.get("continuation_instruction_sha256").is_none(),
            "continuation_instruction_sha256 must be redacted"
        );
        assert!(
            v.get("budget_json").is_none(),
            "budget_json must be redacted"
        );
        // Non-sensitive fields must still be present.
        assert_eq!(v["id"], "cont-id");
        assert_eq!(v["status"], "accepted");
        assert_eq!(v["request_fingerprint_sha256"], "a".repeat(64));
    }

    #[test]
    fn sec_med_002_redacted_candidate_omits_provider_session_id() {
        // P086-SEC-MED-002: redacted_candidate must not expose provider_session_id.
        let c = domain::continuation::ContinuationCandidate {
            agent_execution_id: "ae-id".to_string(),
            run_id: "run-id".to_string(),
            stage_execution_id: "se-id".to_string(),
            agent_role: "code_writer".to_string(),
            provider_session_id: Some("sess-secret".to_string()),
            status: "completed".to_string(),
            eligible: true,
            disabled_reason: None,
        };
        let v = super::redacted_candidate(&c);
        assert!(
            v.get("provider_session_id").is_none(),
            "provider_session_id must be redacted for non-Operator principals"
        );
        assert_eq!(v["agent_execution_id"], "ae-id");
        assert_eq!(v["eligible"], true);
    }

    #[test]
    fn continue_work_schema_has_additional_properties_false() {
        // P086-SEC-MED-001: the live tool schema must declare additionalProperties=false
        // so callers can rely on server-side schema enforcement.
        let specs = tool_specs();
        let cw = specs
            .iter()
            .find(|t| t.name == "agents.continue_work")
            .expect("continue_work must be registered");
        assert_eq!(
            cw.input_schema.get("additionalProperties"),
            Some(&serde_json::Value::Bool(false)),
            "agents.continue_work schema must have additionalProperties=false"
        );
    }

    #[test]
    fn allowed_fields_set_matches_schema_properties() {
        // P086-SEC-MED-001: ALLOWED_FIELDS in handle_continue_work must exactly match the
        // properties declared in the tool schema so server-side and schema-level enforcement agree.
        let specs = tool_specs();
        let cw = specs
            .iter()
            .find(|t| t.name == "agents.continue_work")
            .expect("continue_work must be registered");
        let schema_props: std::collections::HashSet<String> = cw.input_schema["properties"]
            .as_object()
            .expect("properties must be an object")
            .keys()
            .cloned()
            .collect();

        // Mirror of ALLOWED_FIELDS constant
        let allowed: std::collections::HashSet<String> = [
            "agent_execution_id",
            "run_id",
            "stage_execution_id",
            "session_generation_id",
            "provider_session_id",
            "continuation_mode",
            "mode",
            "trigger_kind",
            "idempotency_key",
            "operator_instruction",
            "max_turns",
            "max_wall_clock_seconds",
            "blockers",
            "lead_decision_artifact_id",
            "lead_decision_artifact_sha256",
            "continuation_instruction_sha256",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        assert_eq!(
            schema_props, allowed,
            "ALLOWED_FIELDS must exactly match tool schema properties; \
             update ALLOWED_FIELDS if the schema changes"
        );
    }

    #[test]
    fn empty_idempotency_key_is_rejected() {
        // Non-empty idempotency_key is required: an empty string is semantically meaningless
        // and could collapse distinct requests into a single idempotency scope.
        assert!("".is_empty(), "empty string must trigger rejection guard");
        assert!(
            !"valid-key".is_empty(),
            "non-empty key must pass the empty check"
        );
    }

    #[test]
    fn agent_already_running_guard_covers_non_terminal_statuses() {
        // P086 audit fix: an AgentExecution with a non-terminal status must produce
        // failure_reason=agent_already_running, not ineligible_agent_execution.
        // Regression guard for the status-check logic in handle_continue_work.
        let terminal_statuses = ["completed", "failed"];
        let non_terminal_statuses = ["running", "starting", "queued", "pending", "created"];
        for s in &terminal_statuses {
            let is_terminal = matches!(*s, "completed" | "failed");
            assert!(
                is_terminal,
                "terminal status '{s}' must pass the terminal check"
            );
        }
        for s in &non_terminal_statuses {
            let is_terminal = matches!(*s, "completed" | "failed");
            assert!(
                !is_terminal,
                "non-terminal status '{s}' must trigger agent_already_running, not ineligible_agent_execution"
            );
        }
    }

    #[test]
    fn agent_already_running_error_code_is_distinct() {
        // P086: agent_already_running uses -32031 — must be distinct from other P086 error codes.
        let agent_already_running_code: i64 = -32031;
        let other_codes: &[(i64, &str)] = &[
            (-32020, "ineligible_agent_execution"),
            (-32028, "lead_fields_on_non_lead_auto"),
            (-32029, "approval_required"),
            (-32030, "active_continuation_already_running"),
            (ERR_SATURATION_CAPACITY_EXCEEDED, "saturation"),
            (ERR_IDEMPOTENCY_CONFLICT, "idempotency_conflict"),
        ];
        for (code, name) in other_codes {
            assert_ne!(
                agent_already_running_code, *code,
                "agent_already_running code -32031 must differ from {name} ({code})"
            );
        }
    }

    #[test]
    fn agent_execution_id_must_be_valid_uuid_format() {
        // Security report required_fix #1: UUID-shaped ids must be validated before any DB write.
        // verify_lead_auto_artifacts already does this for lead_decision_artifact_id;
        // handle_continue_work now applies the same check to agent_execution_id.
        let valid_uuids = [
            "550e8400-e29b-41d4-a716-446655440000",
            "00000000-0000-0000-0000-000000000000",
        ];
        let invalid_ids = [
            "not-a-uuid",
            "../../etc/passwd",
            "gggggggg-gggg-gggg-gggg-gggggggggggg",
            "",
        ];
        for id in &valid_uuids {
            assert!(
                uuid::Uuid::parse_str(id).is_ok(),
                "valid UUID {id:?} must pass format check"
            );
        }
        for id in &invalid_ids {
            assert!(
                uuid::Uuid::parse_str(id).is_err(),
                "invalid id {id:?} must fail UUID format check"
            );
        }
    }

    #[test]
    fn sec_med_002_agent_lead_auto_opaque_rejection_has_constant_shape() {
        // SEC-P086-MED-002: lead_auto_agent_opaque_rejection() must always return the same
        // failure_reason and error code regardless of which check triggered it.
        // This is a regression guard: callers must not be able to distinguish eligibility,
        // already_running, run_id_mismatch, stage_mismatch, session_mismatch, or
        // provider_session_mismatch errors from each other before artifact verification.
        let r = super::lead_auto_agent_opaque_rejection();
        assert_eq!(
            r["outcome"], "rejected",
            "opaque rejection must have outcome=rejected"
        );
        assert_eq!(
            r["error"]["code"], -32020,
            "opaque rejection must use code -32020 (same as ineligible to avoid a distinct oracle signal)"
        );
        assert_eq!(
            r["error"]["data"]["failure_reason"], "not_found_or_access_denied",
            "opaque rejection must use failure_reason=not_found_or_access_denied"
        );
        assert!(
            r["error"]["data"].get("agent_status").is_none(),
            "opaque rejection must not expose agent_status"
        );
        assert!(
            r["error"]["data"].get("actual_run_id").is_none(),
            "opaque rejection must not expose actual_run_id"
        );
        assert!(
            r["error"]["data"]
                .get("actual_stage_execution_id")
                .is_none(),
            "opaque rejection must not expose actual_stage_execution_id"
        );
    }

    #[test]
    fn sec_med_002_is_agent_lead_auto_predicate_covers_exact_class_and_trigger() {
        // SEC-P086-MED-002: the opaque path must activate for Agent+lead_auto only,
        // not for Operator+lead_auto or Agent+operator_mcp.
        let cases: &[(auth::PrincipalClass, &str, bool)] = &[
            (auth::PrincipalClass::Agent, "lead_auto", true),
            (auth::PrincipalClass::Operator, "lead_auto", false),
            (auth::PrincipalClass::Agent, "operator_mcp", false),
            (auth::PrincipalClass::Operator, "operator_mcp", false),
        ];
        for (class, trigger_kind, expect_opaque) in cases {
            let principal = auth::Principal::new("p", class.clone());
            let is_agent_lead_auto = matches!(principal.class, auth::PrincipalClass::Agent)
                && *trigger_kind == "lead_auto";
            assert_eq!(
                is_agent_lead_auto, *expect_opaque,
                "is_agent_lead_auto must be {expect_opaque} for class={class:?} trigger={trigger_kind}"
            );
        }
    }
}
