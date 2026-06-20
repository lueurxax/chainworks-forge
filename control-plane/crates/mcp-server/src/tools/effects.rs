use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::SqlitePool;
use std::path::Path;

use db::repos::runs;
use db::repos::side_effects::{self, apply_operator_disposition_tx, DispositionOutcome};
use domain::side_effect::{
    EffectKind, SideEffect, SideEffectId, SideEffectStatus, MCP_ERROR_DISPOSITION_PAYLOAD_MISMATCH,
    MCP_ERROR_EFFECT_NOT_FOUND, MCP_ERROR_INVALID_STATUS_TRANSITION,
};

use crate::protocol::McpTool;

const DECISION_JSON_MAX_BYTES: usize = 64 * 1024;
const LAST_ERROR_MAX_BYTES: usize = 512;
const RECONCILIATION_REPORT_SCHEMA_VERSION: &str = "p078_reconciliation_report_v1";

/// Redact a stored last_error before MCP readback. Strips credential-like tokens
/// then truncates so raw git stderr / xcodebuild output cannot leak secrets through MCP.
fn redact_last_error(raw: Option<&str>) -> Option<String> {
    let s = raw?;
    Some(domain::error_sanitizer::sanitize_error_for_storage(
        s,
        LAST_ERROR_MAX_BYTES,
    ))
}

struct ReconciliationReadback {
    readback_source: String,
    report_details: serde_json::Value,
}

struct ReadbackProbeSpec {
    matched_evidence_kind: &'static str,
    relative_paths: &'static [&'static str],
}

fn parse_json_or_string(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_string()))
}

fn readback_probe_spec(effect_kind: &EffectKind) -> ReadbackProbeSpec {
    match effect_kind {
        EffectKind::GitCommit => ReadbackProbeSpec {
            matched_evidence_kind: "git_commit_receipt",
            relative_paths: &["git-commit.json", "release/git-commit.json", "commit.json"],
        },
        EffectKind::GitPush => ReadbackProbeSpec {
            matched_evidence_kind: "git_push_receipt",
            relative_paths: &["git-push.json", "release/git-push.json"],
        },
        EffectKind::BuildArchive => ReadbackProbeSpec {
            matched_evidence_kind: "release_manifest",
            relative_paths: &["manifest.json", "release/manifest.json"],
        },
        EffectKind::ConnectUpload => ReadbackProbeSpec {
            matched_evidence_kind: "connect_upload_receipt",
            relative_paths: &["connect-upload.json", "release/connect-upload.json"],
        },
        EffectKind::TagCreate => ReadbackProbeSpec {
            matched_evidence_kind: "tag_create_receipt",
            relative_paths: &["tag-create.json", "release/tag-create.json"],
        },
        EffectKind::ArtifactPublish => ReadbackProbeSpec {
            matched_evidence_kind: "artifact_publish_receipt",
            relative_paths: &["artifact-publish.json", "release/artifact-publish.json"],
        },
    }
}

fn candidate_probe_paths(effect: &SideEffect, artifact_root: &str) -> Vec<String> {
    let spec = readback_probe_spec(&effect.effect_kind);
    let mut candidates = Vec::new();

    if let Some(root) = effect.evidence_root.as_deref() {
        if !root.trim().is_empty() {
            for relative in spec.relative_paths {
                let path = Path::new(root).join(relative);
                let value = path.to_string_lossy().into_owned();
                if !candidates.contains(&value) {
                    candidates.push(value);
                }
            }
        }
    }

    for relative in spec.relative_paths {
        let path = Path::new(artifact_root).join(relative);
        let value = path.to_string_lossy().into_owned();
        if !candidates.contains(&value) {
            candidates.push(value);
        }
    }

    candidates
}

async fn build_reconciliation_readback(
    pool: &SqlitePool,
    effect: &SideEffect,
) -> Result<ReconciliationReadback> {
    let run = runs::find_by_id(pool, effect.run_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("effect {} references missing run", effect.id))?;
    let probe_spec = readback_probe_spec(&effect.effect_kind);
    let candidate_paths = candidate_probe_paths(effect, &run.artifact_root);
    let matched_path = candidate_paths
        .iter()
        .find(|path| Path::new(path).is_file())
        .cloned();
    // SEC-P083-MED-002: canonicalize path and enforce containment under artifact_root
    // before reading. evidence_root comes from DB metadata and could be corrupted or
    // crafted to redirect reads outside the artifact directory. Apply the same 1 MiB
    // cap used by reports.get and reject symlink escapes via canonicalization.
    const MAX_EVIDENCE_BYTES: u64 = 1_048_576; // 1 MiB
    let matched_payload = matched_path
        .as_ref()
        .map(|path| -> Result<serde_json::Value> {
            let canonical_root = std::fs::canonicalize(&run.artifact_root).with_context(|| {
                format!("cannot canonicalize artifact_root: {}", run.artifact_root)
            })?;
            let canonical_path = std::fs::canonicalize(path)
                .with_context(|| format!("cannot canonicalize evidence path: {path}"))?;
            if !canonical_path.starts_with(&canonical_root) {
                anyhow::bail!("evidence path escapes artifact_root containment");
            }
            let meta = std::fs::metadata(&canonical_path)
                .with_context(|| format!("stat evidence path: {path}"))?;
            if meta.len() > MAX_EVIDENCE_BYTES {
                anyhow::bail!(
                    "evidence file exceeds {} byte limit ({} bytes)",
                    MAX_EVIDENCE_BYTES,
                    meta.len()
                );
            }
            let raw = std::fs::read_to_string(&canonical_path)
                .with_context(|| format!("read reconciliation evidence {}", path))?;
            Ok(parse_json_or_string(&raw))
        })
        .transpose()?;

    let report_scope = match effect.evidence_root.as_deref() {
        Some(root) if !root.trim().is_empty() => "evidence_root",
        _ => "effect_scoped_fallback",
    };
    let readback_source = if matched_path.is_some() {
        "filesystem_receipt_probe"
    } else if report_scope == "evidence_root" {
        "evidence_root_scan"
    } else {
        "effect_scoped_reconciliation_report"
    };

    let report_details = serde_json::json!({
        "effect_kind": effect.effect_kind.to_string(),
        "matched_evidence_kind": matched_path.as_ref().map(|_| probe_spec.matched_evidence_kind),
        "matched_path": matched_path,
        "matched_payload": matched_payload,
        "candidate_paths": candidate_paths,
        "report_scope": report_scope,
        "evidence_root": effect.evidence_root,
        "artifact_root": run.artifact_root,
        "observed_evidence_summary": effect
            .observed_evidence_summary_json
            .as_deref()
            .map(parse_json_or_string),
        "last_error_kind": effect.last_error_kind,
        "schema_version": RECONCILIATION_REPORT_SCHEMA_VERSION,
        "generated_at": Utc::now().to_rfc3339(),
    });

    Ok(ReconciliationReadback {
        readback_source: readback_source.to_string(),
        report_details,
    })
}

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "effects.list".to_string(),
            description: "List unresolved side-effect records. Read-only projection; no mutations."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "run_id": {
                        "type": "string",
                        "description": "Filter by run ID (optional)"
                    },
                    "first": {
                        "type": "integer",
                        "description": "Maximum number of results (1-100, default 50)",
                        "minimum": 1,
                        "maximum": 100
                    }
                }
            }),
            output_schema: None,
        },
        McpTool {
            name: "effects.inspect".to_string(),
            description: "Inspect a specific side-effect record by ID. Read-only.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["effect_id"],
                "properties": {
                    "effect_id": {
                        "type": "string",
                        "description": "The side-effect ID to inspect"
                    }
                }
            }),
            output_schema: None,
        },
        McpTool {
            name: "effects.reconcile".to_string(),
            description: concat!(
                "Perform read-only reconciliation readback for an unresolved side effect. ",
                "Does NOT push, upload, publish, delete, retry release execution, ",
                "or mutate external systems. Returns observed evidence and recommended disposition."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["effect_id"],
                "properties": {
                    "effect_id": {
                        "type": "string",
                        "description": "The side-effect ID to reconcile"
                    }
                }
            }),
            output_schema: None,
        },
        McpTool {
            name: "effects.mark_conflict".to_string(),
            description: concat!(
                "Mark a side-effect as conflict after operator confirmation. ",
                "Requires disposition_id for domain-level idempotency and ",
                "idempotency_key for MCP transport-level idempotency."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["effect_id", "disposition_id", "decision_json", "idempotency_key"],
                "properties": {
                    "effect_id": {
                        "type": "string",
                        "description": "The side-effect ID"
                    },
                    "disposition_id": {
                        "type": "string",
                        "description": "Unique disposition ID for domain-level audit idempotency"
                    },
                    "decision_json": {
                        "type": "string",
                        "description": "JSON payload describing the conflict determination"
                    },
                    "idempotency_key": {
                        "type": "string",
                        "description": "UUIDv7 idempotency key for MCP transport-level retry safety"
                    }
                }
            }),
            output_schema: None,
        },
        McpTool {
            name: "effects.mark_unrecoverable".to_string(),
            description: concat!(
                "Mark a side-effect as unrecoverable after operator confirmation. ",
                "Requires disposition_id for domain-level idempotency and ",
                "idempotency_key for MCP transport-level idempotency."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["effect_id", "disposition_id", "decision_json", "idempotency_key"],
                "properties": {
                    "effect_id": {
                        "type": "string",
                        "description": "The side-effect ID"
                    },
                    "disposition_id": {
                        "type": "string",
                        "description": "Unique disposition ID for domain-level audit idempotency"
                    },
                    "decision_json": {
                        "type": "string",
                        "description": "JSON payload describing the unrecoverable determination"
                    },
                    "idempotency_key": {
                        "type": "string",
                        "description": "UUIDv7 idempotency key for MCP transport-level retry safety"
                    }
                }
            }),
            output_schema: None,
        },
        McpTool {
            name: "effects.clear_after_manual_verification".to_string(),
            description: concat!(
                "Clear a side-effect after operator manual verification. ",
                "Requires disposition_id and evidence of manual verification. ",
                "idempotency_key for MCP transport-level idempotency."
            )
            .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["effect_id", "disposition_id", "decision_json", "idempotency_key"],
                "properties": {
                    "effect_id": {
                        "type": "string",
                        "description": "The side-effect ID"
                    },
                    "disposition_id": {
                        "type": "string",
                        "description": "Unique disposition ID for domain-level audit idempotency"
                    },
                    "decision_json": {
                        "type": "string",
                        "description": "JSON payload describing the manual verification result"
                    },
                    "idempotency_key": {
                        "type": "string",
                        "description": "UUIDv7 idempotency key for MCP transport-level retry safety"
                    }
                }
            }),
            output_schema: None,
        },
    ]
}

// ── P081: Effect-disposition write unit ───────────────────────────────────────

/// Apply an operator disposition inside a single BEGIN IMMEDIATE write unit that
/// also records a command_journal row. Returns the journal_id for MCP idempotency
/// linkage and the disposition outcome for the MCP response payload.
///
/// Atomicity: command_journal row, disposition update, and settlement INSERT are
/// all in the same SQLite transaction. If the transaction rolls back, no business
/// state changes persist.
async fn apply_effect_disposition_write_unit(
    pool: &SqlitePool,
    command_type: &str,
    tool_name: &str,
    effect_id: &SideEffectId,
    new_status: SideEffectStatus,
    disposition_id: &str,
    decision_json: &str,
    decision_json_hash: &str,
    applied_by: &str,
    now: chrono::DateTime<Utc>,
    caller_principal_id: &str,
    caller_principal_class: &str,
    idempotency_key: Option<&str>,
    idempotency_request_hash: Option<&str>,
    boundary_row_id: Option<&str>,
    request_id: Option<&str>,
) -> Result<(String, DispositionOutcome)> {
    let mut tx = db::pool::begin_immediate_with_retry(pool, tool_name)
        .await
        .context("effect disposition: begin write unit")?;

    let journal_id = uuid::Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "effect_id": effect_id.as_ref(),
        "new_status": new_status.to_string(),
        "disposition_id": disposition_id,
    });

    db::repos::mcp_command_idempotency::claim_pending_for_command_tx(
        &mut tx,
        idempotency_key,
        tool_name,
        caller_principal_id,
        idempotency_request_hash,
        boundary_row_id,
    )
    .await
    .context("effect disposition: claim mcp idempotency")?;

    db::repos::command_journal::record_tx(
        &mut tx,
        &journal_id,
        command_type,
        &payload.to_string(),
        None,
        now,
        Some("mcp"),
        Some(caller_principal_id),
        Some(caller_principal_class),
        Some(tool_name),
        request_id,
        None,
        idempotency_key,
        boundary_row_id,
    )
    .await
    .context("effect disposition: record command journal")?;

    let outcome = apply_operator_disposition_tx(
        &mut tx,
        effect_id,
        new_status,
        "mcp_operator",
        disposition_id,
        decision_json,
        decision_json_hash,
        applied_by,
        now,
    )
    .await
    .context("effect disposition: apply disposition")?;

    db::repos::command_journal::complete_entry_tx(&mut tx, &journal_id, Utc::now())
        .await
        .context("effect disposition: complete journal")?;

    tx.commit().await.context("effect disposition: commit")?;

    Ok((journal_id, outcome))
}

// ── Handlers ──────────────────────────────────────────────────────────────────

pub async fn handle_effects_list(
    pool: &SqlitePool,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let first: u32 = params
        .get("first")
        .and_then(|v| v.as_u64())
        .map(|v| v.min(100) as u32)
        .unwrap_or(50);

    let run_id_filter = params.get("run_id").and_then(|v| v.as_str());

    let effects = if let Some(run_id) = run_id_filter {
        let all = side_effects::list_unresolved_for_run(pool, run_id).await?;
        all.into_iter().take(first as usize).collect::<Vec<_>>()
    } else {
        side_effects::list_unresolved(pool, first).await?
    };

    let items: Vec<serde_json::Value> = effects
        .iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id.to_string(),
                "run_id": e.run_id.to_string(),
                "stage_execution_id": e.stage_execution_id.to_string(),
                "effect_kind": e.effect_kind.to_string(),
                "effect_kind_raw": e.effect_kind.to_string(),
                "status": e.status.to_string(),
                "status_raw": e.status.to_string(),
                "target_key": e.target_key,
                "idempotency_key": e.idempotency_key,
                "external_write_attempted": e.external_write_attempted,
                "attempt_budget_remaining": e.attempt_budget_remaining,
                "last_error_kind": e.last_error_kind,
                "recommended_mcp_tool": "effects.inspect or effects.reconcile",
                "created_at": e.created_at.to_rfc3339(),
                "updated_at": e.updated_at.to_rfc3339()
            })
        })
        .collect();

    Ok(serde_json::json!({
        "unresolved_count": items.len(),
        "effects": items
    }))
}

pub async fn handle_effects_inspect(
    pool: &SqlitePool,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let effect_id_str = params
        .get("effect_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;

    let effect_id = SideEffectId::from_str(effect_id_str);
    let effect = side_effects::find_by_id(pool, &effect_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;

    let attempts = side_effects::list_attempts(pool, &effect_id).await?;

    Ok(serde_json::json!({
        "id": effect.id.to_string(),
        "run_id": effect.run_id.to_string(),
        "stage_execution_id": effect.stage_execution_id.to_string(),
        "agent_execution_id": effect.agent_execution_id.as_ref().map(|a| a.to_string()),
        "effect_kind": effect.effect_kind.to_string(),
        "effect_kind_raw": effect.effect_kind.to_string(),
        "status": effect.status.to_string(),
        "status_raw": effect.status.to_string(),
        "target_key": effect.target_key,
        "idempotency_key": effect.idempotency_key,
        "external_write_attempted": effect.external_write_attempted,
        "attempt_budget_remaining": effect.attempt_budget_remaining,
        "last_error_kind": effect.last_error_kind,
        "last_error": redact_last_error(effect.last_error.as_deref()),
        "evidence_root": effect.evidence_root,
        "observed_evidence_summary": effect.observed_evidence_summary_json,
        "deadline_at": effect.deadline_at.map(|t| t.to_rfc3339()),
        "lease_expires_at": effect.lease_expires_at.map(|t| t.to_rfc3339()),
        "recommended_mcp_tool": "effects.reconcile",
        "retry_forbidden": true,
        "created_at": effect.created_at.to_rfc3339(),
        "updated_at": effect.updated_at.to_rfc3339(),
        "attempts": attempts.iter().map(|a| serde_json::json!({
            "id": a.id.to_string(),
            "attempt_number": a.attempt_number,
            "attempt_kind": a.attempt_kind,
            "started_at": a.started_at.to_rfc3339(),
            "completed_at": a.completed_at.map(|t| t.to_rfc3339()),
            "exit_status": a.exit_status,
            "error_kind": a.error_kind,
        })).collect::<Vec<_>>()
    }))
}

/// Read-only reconciliation readback. Does NOT mutate external systems.
pub async fn handle_effects_reconcile(
    pool: &SqlitePool,
    params: &serde_json::Value,
) -> Result<serde_json::Value> {
    let effect_id_str = params
        .get("effect_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;

    let effect_id = SideEffectId::from_str(effect_id_str);
    let effect = side_effects::find_by_id(pool, &effect_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;
    let readback = build_reconciliation_readback(pool, &effect).await?;

    // Read-only: just surface current state and recommended next action
    let recommended_action = match effect.status {
        SideEffectStatus::NeedsReconciliation | SideEffectStatus::Conflict => {
            "Use effects.mark_unrecoverable or effects.clear_after_manual_verification"
        }
        SideEffectStatus::Unrecoverable => {
            "Use effects.clear_after_manual_verification after operator review"
        }
        SideEffectStatus::ExternallyObserved => {
            "Evidence observed externally; use effects.clear_after_manual_verification"
        }
        _ => "Use effects.inspect for current state",
    };

    Ok(serde_json::json!({
        "effect_id": effect.id.to_string(),
        "status": effect.status.to_string(),
        "status_raw": effect.status.to_string(),
        "effect_kind": effect.effect_kind.to_string(),
        "target_key": effect.target_key,
        "external_write_attempted": effect.external_write_attempted,
        "last_error_kind": effect.last_error_kind,
        "last_error": redact_last_error(effect.last_error.as_deref()),
        "observed_evidence_summary": effect.observed_evidence_summary_json,
        "evidence_root": effect.evidence_root,
        "readback_source": readback.readback_source,
        "readback_unavailable": false,
        "report_details": readback.report_details,
        "recommended_action": recommended_action,
        "retry_forbidden": true,
        "updated_at": effect.updated_at.to_rfc3339()
    }))
}

pub async fn handle_effects_mark_conflict(
    pool: &SqlitePool,
    params: &serde_json::Value,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let effect_id_str = params
        .get("effect_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;
    let disposition_id = params
        .get("disposition_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("disposition_id is required"))?;
    let decision_json = params
        .get("decision_json")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("decision_json is required"))?;

    validate_uuid_format(effect_id_str, "effect_id")?;
    validate_uuid_format(disposition_id, "disposition_id")?;

    if decision_json.len() > DECISION_JSON_MAX_BYTES {
        return Err(anyhow::anyhow!(
            "decision_json_too_large: decision_json exceeds {} byte limit",
            DECISION_JSON_MAX_BYTES
        ));
    }

    let decision_value: serde_json::Value = serde_json::from_str(decision_json)
        .map_err(|e| anyhow::anyhow!("decision_json must be valid JSON: {e}"))?;

    validate_decision_v1_schema(&decision_value)?;

    let decision_hash = compute_hash(decision_json);
    let effect_id = SideEffectId::from_str(effect_id_str);

    let effect = side_effects::find_by_id(pool, &effect_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;

    // Only allow transitioning to conflict from needs_reconciliation.
    // Executor handles executing -> conflict via executor_settle_cas.
    if effect.status != SideEffectStatus::NeedsReconciliation {
        return Err(anyhow::anyhow!(
            "{}: cannot mark_conflict from status {} — only needs_reconciliation is allowed",
            MCP_ERROR_INVALID_STATUS_TRANSITION,
            effect.status
        ));
    }

    let idempotency_key = crate::request_context::current_idempotency_key();
    let idempotency_request_hash = crate::request_context::current_idempotency_request_hash();
    let boundary_row_id = crate::request_context::current_boundary_row_id();
    let request_id = crate::request_context::current_request_id();

    let (journal_id, outcome) = apply_effect_disposition_write_unit(
        pool,
        "EffectsMarkConflict",
        "effects.mark_conflict",
        &effect_id,
        SideEffectStatus::Conflict,
        disposition_id,
        decision_json,
        &decision_hash,
        &principal.id,
        Utc::now(),
        &principal.id,
        &principal.class.to_string(),
        idempotency_key.as_deref(),
        idempotency_request_hash.as_deref(),
        boundary_row_id.as_deref(),
        request_id.as_deref(),
    )
    .await?;

    match outcome {
        DispositionOutcome::Applied => Ok(serde_json::json!({
            "status": "applied",
            "effect_id": effect_id_str,
            "new_status": "conflict",
            "disposition_id": disposition_id,
            "journal_id": journal_id
        })),
        DispositionOutcome::AlreadyApplied => Ok(serde_json::json!({
            "status": "already_applied",
            "effect_id": effect_id_str,
            "disposition_id": disposition_id,
            "journal_id": journal_id
        })),
        DispositionOutcome::PayloadMismatch => Err(anyhow::anyhow!(
            "{}: disposition_id {} already used with different payload",
            MCP_ERROR_DISPOSITION_PAYLOAD_MISMATCH,
            disposition_id
        )),
        DispositionOutcome::NotApplicable => Err(anyhow::anyhow!(
            "{}: effect {} is not in a state that allows mark_conflict",
            MCP_ERROR_INVALID_STATUS_TRANSITION,
            effect_id_str
        )),
    }
}

pub async fn handle_effects_mark_unrecoverable(
    pool: &SqlitePool,
    params: &serde_json::Value,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let effect_id_str = params
        .get("effect_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;
    let disposition_id = params
        .get("disposition_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("disposition_id is required"))?;
    let decision_json = params
        .get("decision_json")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("decision_json is required"))?;

    validate_uuid_format(effect_id_str, "effect_id")?;
    validate_uuid_format(disposition_id, "disposition_id")?;

    if decision_json.len() > DECISION_JSON_MAX_BYTES {
        return Err(anyhow::anyhow!(
            "decision_json_too_large: decision_json exceeds {} byte limit",
            DECISION_JSON_MAX_BYTES
        ));
    }

    let decision_value: serde_json::Value = serde_json::from_str(decision_json)
        .map_err(|e| anyhow::anyhow!("decision_json must be valid JSON: {e}"))?;

    // Validate side_effect_decision_v1 schema (same as clear_after_manual_verification).
    validate_decision_v1_schema(&decision_value)?;

    let decision_hash = compute_hash(decision_json);
    let effect_id = SideEffectId::from_str(effect_id_str);

    // Verify effect exists and is in a valid source status
    let effect = side_effects::find_by_id(pool, &effect_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;

    if !matches!(
        effect.status,
        SideEffectStatus::NeedsReconciliation
            | SideEffectStatus::Conflict
            | SideEffectStatus::Unrecoverable
    ) {
        return Err(anyhow::anyhow!(
            "{}: cannot mark_unrecoverable from status {}",
            MCP_ERROR_INVALID_STATUS_TRANSITION,
            effect.status
        ));
    }

    let idempotency_key = crate::request_context::current_idempotency_key();
    let idempotency_request_hash = crate::request_context::current_idempotency_request_hash();
    let boundary_row_id = crate::request_context::current_boundary_row_id();
    let request_id = crate::request_context::current_request_id();

    let (journal_id, outcome) = apply_effect_disposition_write_unit(
        pool,
        "EffectsMarkUnrecoverable",
        "effects.mark_unrecoverable",
        &effect_id,
        SideEffectStatus::Unrecoverable,
        disposition_id,
        decision_json,
        &decision_hash,
        &principal.id,
        Utc::now(),
        &principal.id,
        &principal.class.to_string(),
        idempotency_key.as_deref(),
        idempotency_request_hash.as_deref(),
        boundary_row_id.as_deref(),
        request_id.as_deref(),
    )
    .await?;

    match outcome {
        DispositionOutcome::Applied => Ok(serde_json::json!({
            "status": "applied",
            "effect_id": effect_id_str,
            "new_status": "unrecoverable",
            "disposition_id": disposition_id,
            "journal_id": journal_id
        })),
        DispositionOutcome::AlreadyApplied => Ok(serde_json::json!({
            "status": "already_applied",
            "effect_id": effect_id_str,
            "disposition_id": disposition_id,
            "journal_id": journal_id
        })),
        DispositionOutcome::PayloadMismatch => Err(anyhow::anyhow!(
            "{}: disposition_id {} already used with different payload",
            MCP_ERROR_DISPOSITION_PAYLOAD_MISMATCH,
            disposition_id
        )),
        DispositionOutcome::NotApplicable => Err(anyhow::anyhow!(
            "{}: effect {} is not in a reconcilable state",
            MCP_ERROR_INVALID_STATUS_TRANSITION,
            effect_id_str
        )),
    }
}

pub async fn handle_effects_clear_after_manual_verification(
    pool: &SqlitePool,
    params: &serde_json::Value,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    let effect_id_str = params
        .get("effect_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;
    let disposition_id = params
        .get("disposition_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("disposition_id is required"))?;
    let decision_json = params
        .get("decision_json")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("decision_json is required"))?;

    validate_uuid_format(effect_id_str, "effect_id")?;
    validate_uuid_format(disposition_id, "disposition_id")?;

    if decision_json.len() > DECISION_JSON_MAX_BYTES {
        return Err(anyhow::anyhow!(
            "decision_json_too_large: decision_json exceeds {} byte limit",
            DECISION_JSON_MAX_BYTES
        ));
    }

    let decision_value: serde_json::Value = serde_json::from_str(decision_json)
        .map_err(|e| anyhow::anyhow!("decision_json must be valid JSON: {e}"))?;

    // Validate side_effect_decision_v1 schema: schema_version + at least one required field.
    validate_decision_v1_schema(&decision_value)?;

    let decision_hash = compute_hash(decision_json);
    let effect_id = SideEffectId::from_str(effect_id_str);

    let effect = side_effects::find_by_id(pool, &effect_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!(MCP_ERROR_EFFECT_NOT_FOUND))?;

    if !effect.status.is_unresolved() {
        return Err(anyhow::anyhow!(
            "{}: effect {} is already in terminal status {}",
            MCP_ERROR_INVALID_STATUS_TRANSITION,
            effect_id_str,
            effect.status
        ));
    }

    let idempotency_key = crate::request_context::current_idempotency_key();
    let idempotency_request_hash = crate::request_context::current_idempotency_request_hash();
    let boundary_row_id = crate::request_context::current_boundary_row_id();
    let request_id = crate::request_context::current_request_id();

    let (journal_id, outcome) = apply_effect_disposition_write_unit(
        pool,
        "EffectsClearAfterManualVerification",
        "effects.clear_after_manual_verification",
        &effect_id,
        SideEffectStatus::Reconciled,
        disposition_id,
        decision_json,
        &decision_hash,
        &principal.id,
        Utc::now(),
        &principal.id,
        &principal.class.to_string(),
        idempotency_key.as_deref(),
        idempotency_request_hash.as_deref(),
        boundary_row_id.as_deref(),
        request_id.as_deref(),
    )
    .await?;

    match outcome {
        DispositionOutcome::Applied => Ok(serde_json::json!({
            "status": "applied",
            "effect_id": effect_id_str,
            "new_status": "reconciled",
            "disposition_id": disposition_id,
            "journal_id": journal_id
        })),
        DispositionOutcome::AlreadyApplied => Ok(serde_json::json!({
            "status": "already_applied",
            "effect_id": effect_id_str,
            "disposition_id": disposition_id,
            "journal_id": journal_id
        })),
        DispositionOutcome::PayloadMismatch => Err(anyhow::anyhow!(
            "{}: disposition_id {} already used with different payload",
            MCP_ERROR_DISPOSITION_PAYLOAD_MISMATCH,
            disposition_id
        )),
        DispositionOutcome::NotApplicable => Err(anyhow::anyhow!(
            "{}: effect {} is not in a reconcilable state",
            MCP_ERROR_INVALID_STATUS_TRANSITION,
            effect_id_str
        )),
    }
}

const ALLOWED_DECISION_VALUES: &[&str] = &[
    "reconciled",
    "unrecoverable",
    "conflict",
    "cleared",
    "manual_verified",
];

/// Validate that a disposition payload conforms to the side_effect_decision_v1 schema.
/// Per P078-SEC-MED-001: validates typed fields, allowed decision enum, non-empty operator_notes.
fn validate_decision_v1_schema(value: &serde_json::Value) -> Result<()> {
    let obj = value.as_object().ok_or_else(|| {
        anyhow::anyhow!("insufficient_evidence: decision_json must be a JSON object")
    })?;

    let schema_version = obj
        .get("schema_version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "insufficient_evidence: decision_json missing required field 'schema_version'"
            )
        })?;

    if schema_version != "side_effect_decision_v1" {
        return Err(anyhow::anyhow!(
            "insufficient_evidence: decision_json schema_version must be \
             'side_effect_decision_v1', got '{schema_version}'"
        ));
    }

    // 'decision' must be a string with an allowed enum value.
    let decision = obj
        .get("decision")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "insufficient_evidence: decision_json 'decision' must be a non-null string"
            )
        })?;

    if !ALLOWED_DECISION_VALUES.contains(&decision) {
        return Err(anyhow::anyhow!(
            "insufficient_evidence: decision_json 'decision' must be one of {:?}, got '{decision}'",
            ALLOWED_DECISION_VALUES
        ));
    }

    // 'operator_notes' must be a non-empty string.
    let notes = obj
        .get("operator_notes")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "insufficient_evidence: decision_json 'operator_notes' must be a non-null string"
            )
        })?;

    if notes.trim().is_empty() {
        return Err(anyhow::anyhow!(
            "insufficient_evidence: decision_json 'operator_notes' must not be empty"
        ));
    }

    Ok(())
}

/// Validate that an effect_id string is a valid UUID format.
fn validate_uuid_format(id_str: &str, field: &str) -> Result<()> {
    if id_str.len() != 36 || id_str.chars().filter(|&c| c == '-').count() != 4 {
        return Err(anyhow::anyhow!(
            "invalid_disposition: {field} must be a valid UUID (got {id_str:?})"
        ));
    }
    // Validate UUID hyphen positions: 8-4-4-4-12
    let parts: Vec<&str> = id_str.split('-').collect();
    if parts.len() != 5
        || parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
        || !parts
            .iter()
            .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(anyhow::anyhow!(
            "invalid_disposition: {field} must be a valid UUID (got {id_str:?})"
        ));
    }
    Ok(())
}

/// SHA-256 hex digest for audit-critical payload comparison (disposition_json_hash).
/// Using a cryptographic hash prevents birthday-bound collisions that could
/// allow a second distinct payload to be treated as AlreadyApplied.
fn compute_hash(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash_bytes = Sha256::digest(s.as_bytes());
    hash_bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
) -> anyhow::Result<serde_json::Value> {
    match tool_name {
        "effects.list" => handle_effects_list(pool, &params).await,
        "effects.inspect" => handle_effects_inspect(pool, &params).await,
        "effects.reconcile" => handle_effects_reconcile(pool, &params).await,
        "effects.mark_conflict" => handle_effects_mark_conflict(pool, &params, principal).await,
        "effects.mark_unrecoverable" => {
            handle_effects_mark_unrecoverable(pool, &params, principal).await
        }
        "effects.clear_after_manual_verification" => {
            handle_effects_clear_after_manual_verification(pool, &params, principal).await
        }
        _ => Err(anyhow::anyhow!("Unknown effects tool: {tool_name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_078_redact_last_error_none_returns_none() {
        assert_eq!(redact_last_error(None), None);
    }

    #[test]
    fn proposal_078_redact_last_error_clean_short_string_passthrough() {
        let result = redact_last_error(Some("git push failed: network timeout"));
        assert_eq!(result, Some("git push failed: network timeout".to_string()));
    }

    #[test]
    fn proposal_078_redact_last_error_strips_bearer_token() {
        let raw = "Authorization: Bearer ghp_supersecrettoken123 failed";
        let result = redact_last_error(Some(raw)).unwrap();
        assert!(
            !result.contains("ghp_supersecrettoken123"),
            "bearer token must be redacted"
        );
        assert!(result.contains("[redacted]"));
    }

    #[test]
    fn proposal_078_redact_last_error_strips_api_key_assignment() {
        let raw = "error: api_key=sk-abc1234567890 is invalid";
        let result = redact_last_error(Some(raw)).unwrap();
        assert!(
            !result.contains("sk-abc1234567890"),
            "api key value must be redacted"
        );
    }

    #[test]
    fn proposal_078_redact_last_error_strips_ssh_path() {
        let raw = "error reading /home/user/.ssh/id_rsa permission denied";
        let result = redact_last_error(Some(raw)).unwrap();
        assert!(
            !result.contains("/home/user/.ssh/id_rsa"),
            "ssh path must be redacted"
        );
        assert!(result.contains("[redacted]"));
    }

    #[test]
    fn proposal_078_redact_last_error_multibyte_no_panic() {
        let prefix = "e".repeat(511);
        let multibyte = "Ä"; // 2 bytes
        let s = format!("{prefix}{multibyte}trailing");
        let result = redact_last_error(Some(&s)).unwrap();
        assert!(
            result.ends_with("...[truncated]"),
            "must be truncated: {result:?}"
        );
        assert!(
            std::str::from_utf8(result.as_bytes()).is_ok(),
            "result must be valid UTF-8"
        );
    }

    #[test]
    fn proposal_078_redact_last_error_long_string_truncated() {
        let long = "x".repeat(600);
        let result = redact_last_error(Some(&long)).unwrap();
        assert!(result.ends_with("...[truncated]"));
    }
}
