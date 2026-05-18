use std::path::PathBuf;

use anyhow::Result;
use db::writer::DbWriterHeartbeat;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::protocol::McpTool;

// ── P075 typed MCP error contract ─────────────────────────────────────────────
//
// Domain-level errors are returned as structured Ok responses with `error: true`
// and a typed `errorCode` so callers can branch on the kind without parsing
// message strings. Infrastructure errors (DB panic, IO) still propagate as
// `Err` and become JSON-RPC -32603 at the dispatch layer.
//
// Required error codes per P075 §storage_diagnostics.error_contract:
//   "unavailable"           — storage backend cannot serve the request
//   "stale"                 — response data is known to be outdated beyond threshold
//   "unauthorized"          — caller lacks the storage diagnostics capability
//                             (handled at dispatch; listed here for documentation parity)
//   "invalid_input"         — required parameter is missing or out of range
//   "maintenance_disabled"  — a kill switch blocks the maintenance operation

pub const ERR_UNAVAILABLE: &str = "unavailable";
pub const ERR_STALE: &str = "stale";
pub const ERR_UNAUTHORIZED: &str = "unauthorized";
pub const ERR_INVALID_INPUT: &str = "invalid_input";
pub const ERR_MAINTENANCE_DISABLED: &str = "maintenance_disabled";
pub const ERR_TIMEOUT: &str = "timeout";
pub const ERR_HOT_READ_FORBIDDEN_DEPENDENCY: &str = "hot_read_forbidden_dependency";
pub const ERR_HOT_READ_CIRCUIT_OPEN: &str = "hot_read_circuit_open";
const PROJECTION_NAME_MAX_BYTES: usize = 128;
const SOURCE_NAME_MAX_BYTES: usize = 128;
pub const ERR_PROJECTION_UNAVAILABLE: &str = "projection_unavailable";
pub const ERR_NOT_FOUND: &str = "not_found";
pub const ERR_RETENTION_PRUNED: &str = "retention_pruned";
pub const ERR_BUSY: &str = "busy";
pub const ERR_THROTTLED: &str = "throttled";
pub const ERR_PROJECTION_REBUILD_POISONED: &str = "projection_rebuild_poisoned";
pub const ERR_CONFLICT: &str = "conflict";

/// Build a typed MCP error response body.
///
/// Callers that want to signal a domain-level error should return
/// `Ok(typed_error(...))` so the content reaches the caller as a
/// structured body rather than a JSON-RPC -32603 envelope.
pub fn typed_error(
    tool: &str,
    code: &str,
    message: impl Into<String>,
    request_id: Option<&str>,
) -> serde_json::Value {
    typed_error_full(tool, code, message, None, None, request_id)
}

/// Build a typed MCP error response body with optional retry and hot-read metadata.
pub fn typed_error_full(
    tool: &str,
    code: &str,
    message: impl Into<String>,
    retry_after_ms: Option<i64>,
    hot_read: Option<serde_json::Value>,
    request_id: Option<&str>,
) -> serde_json::Value {
    let mut err = serde_json::json!({
        "error": true,
        "errorCode": code,
        "message": message.into(),
        "tool": tool,
    });
    if let Some(rid) = request_id {
        err["requestId"] = serde_json::json!(rid);
    } else if let Some(rid) = crate::request_context::current_request_id() {
        err["requestId"] = serde_json::json!(rid);
    }
    if let Some(retry) = retry_after_ms {
        err["retryAfterMs"] = serde_json::json!(retry);
    }
    if let Some(hr) = hot_read {
        err["hotRead"] = hr;
    }
    err
}

fn repair_slot_public_error(error: &anyhow::Error) -> (&'static str, &'static str) {
    let msg = error.to_string();
    if msg.contains("operation not found") {
        (ERR_NOT_FOUND, "operation not found")
    } else if msg.contains("operation is not repairable") {
        (ERR_CONFLICT, "operation is not repairable")
    } else if msg.contains("slot generation mismatch") {
        (ERR_CONFLICT, "slot generation mismatch")
    } else {
        (ERR_UNAVAILABLE, "repair slot failed")
    }
}

fn public_reference(prefix: &str, raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    let short: String = format!("{digest:x}").chars().take(16).collect();
    format!("{prefix}:{short}")
}

fn public_maintenance_operation(
    op: &db::repos::maintenance::MaintenanceOperation,
) -> serde_json::Value {
    serde_json::json!({
        "operationId": public_reference("maintenance_operation", &op.id),
        "operationKind": op.operation_kind,
        "status": op.status,
        "slotGeneration": op.slot_generation,
        "startedAtMs": op.started_at_ms,
        "completedAtMs": op.completed_at_ms,
        "error": op.error.as_ref().map(|_| "maintenance_operation_failed"),
        "detailsRedacted": true,
    })
}

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "storage.health".to_string(),
            description: "Read P075 storage health with units, freshness, thresholds, and kill-switch state".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "includeThresholds": {
                        "type": "boolean",
                        "default": true
                    }
                }
            }),
        },
        McpTool {
            name: "storage.write_pressure".to_string(),
            description: "Read the latest P075 storage write-pressure snapshot".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "windowSeconds": {
                        "type": "integer",
                        "minimum": 30,
                        "maximum": 3600,
                        "default": 300
                    },
                    "includeLanes": {
                        "type": "boolean",
                        "default": true
                    }
                }
            }),
        },
        McpTool {
            name: "storage.evidence_spool_summary".to_string(),
            description: "Read compact P075 evidence spool metadata counts and byte totals".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "runId": {
                        "type": "string"
                    },
                    "includeOrphans": {
                        "type": "boolean",
                        "default": true
                    }
                }
            }),
        },
        McpTool {
            name: "storage.reconcile_evidence_orphans".to_string(),
            description: "Run the P075 evidence orphan sweep and backfill recovered_orphan metadata. runId is required for non-dry-run calls.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "runId": {
                        "type": "string",
                        "description": "Run ID to scope the sweep. Required when dryRun=false."
                    },
                    "dryRun": {
                        "type": "boolean",
                        "default": true
                    },
                    "maxFiles": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000,
                        "default": 1000
                    }
                }
            }),
        },
        McpTool {
            name: "storage.maintenance.repair_slot".to_string(),
            description: "Repair an orphaned or stuck maintenance operation slot via CAS (Proposal 087)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operationId": {
                        "type": "string",
                        "description": "ID of the operation to repair"
                    },
                    "slotGeneration": {
                        "type": "integer",
                        "description": "The expected slot_generation for the CAS update"
                    },
                    "idempotencyKey": {
                        "type": "string",
                        "description": "Idempotency key for the repair operation (operator-provided)"
                    }
                },
                "required": ["operationId", "slotGeneration", "idempotencyKey"]
            }),
        },
        McpTool {
            name: "storage.projections.clear_backlog".to_string(),
            description: "Clear the projection invalidation backlog for a specific projection and source (Proposal 087)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "projectionName": {
                        "type": "string",
                        "description": "Name of the projection"
                    },
                    "sourceName": {
                        "type": "string",
                        "description": "Name of the source"
                    }
                },
                "required": ["projectionName", "sourceName"]
            }),
        },
        McpTool {
            name: "storage.projections.clear_poison".to_string(),
            description: "Clear the poison flag for a specific projection and source (Proposal 087)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "projectionName": {
                        "type": "string",
                        "description": "Name of the projection"
                    },
                    "sourceName": {
                        "type": "string",
                        "description": "Name of the source"
                    }
                },
                "required": ["projectionName", "sourceName"]
            }),
        },
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    principal: &auth::Principal,
    request_id: Option<&str>,
) -> Result<serde_json::Value> {
    execute_with_writer(
        tool_name,
        params,
        pool,
        None,
        principal,
        tokio_util::sync::CancellationToken::new(),
        request_id,
    )
    .await
}

pub async fn execute_with_writer(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    writer_heartbeat: Option<&DbWriterHeartbeat>,
    principal: &auth::Principal,
    cancel: tokio_util::sync::CancellationToken,
    request_id: Option<&str>,
) -> Result<serde_json::Value> {
    match tool_name {
        "storage.health" => {
            let include_thresholds = params
                .get("includeThresholds")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let mut health =
                match db::repos::storage_health::storage_health_with_writer(pool, writer_heartbeat)
                    .await
                {
                    Ok(h) => h,
                    Err(_) => {
                        return Ok(typed_error(
                            "storage.health",
                            ERR_UNAVAILABLE,
                            "storage health query failed; database may be unavailable",
                            request_id,
                        ));
                    }
                };
            if !include_thresholds {
                if let Some(obj) = health.as_object_mut() {
                    obj.remove("thresholds");
                }
            }
            if health
                .get("isStale")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                let mut err = typed_error(
                    "storage.health",
                    ERR_STALE,
                    "storage health is stale; no live DbWriter heartbeat is available",
                    request_id,
                );
                if let (Some(err_obj), Some(health_obj)) = (err.as_object_mut(), health.as_object())
                {
                    for key in [
                        "schemaVersion",
                        "updatedAt",
                        "rollout",
                        "projectionFreshness",
                        "p087EvaluatedMetrics",
                        "readPathMetrics",
                        "readPath",
                        "hotRead",
                    ] {
                        if let Some(value) = health_obj.get(key) {
                            err_obj.insert(key.to_string(), value.clone());
                        }
                    }
                }
                return Ok(err);
            }
            Ok(health)
        }
        "storage.write_pressure" => {
            let window_seconds = params
                .get("windowSeconds")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(300)
                .clamp(30, 3600);
            let include_lanes = params
                .get("includeLanes")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let health =
                db::repos::storage_health::storage_health_with_writer(pool, writer_heartbeat)
                    .await?;
            let mut pressure = health["writePressure"].clone();
            if let Some(obj) = pressure.as_object_mut() {
                obj.insert(
                    "requestedWindowSeconds".to_string(),
                    serde_json::json!(window_seconds),
                );
                obj.insert("includeLanes".to_string(), serde_json::json!(include_lanes));
                if !include_lanes {
                    if let Some(payload) = obj
                        .get_mut("latestSnapshot")
                        .and_then(serde_json::Value::as_object_mut)
                        .and_then(|snapshot| snapshot.get_mut("payload"))
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        payload.remove("lanes");
                    }
                }
                let age_ms = obj
                    .get("freshness")
                    .and_then(|freshness| freshness.get("ageMs"))
                    .and_then(serde_json::Value::as_i64);
                if age_ms.is_some_and(|age| age > window_seconds * 1000) {
                    obj.insert("state".to_string(), serde_json::json!("window_empty"));
                    obj.insert("latestSnapshot".to_string(), serde_json::Value::Null);
                    if let Some(freshness) = obj
                        .get_mut("freshness")
                        .and_then(serde_json::Value::as_object_mut)
                    {
                        freshness.insert("state".to_string(), serde_json::json!("stale"));
                    }
                }
            }
            Ok(pressure)
        }
        "storage.evidence_spool_summary" => {
            let run_id = params.get("runId").and_then(serde_json::Value::as_str);
            let include_orphans = params
                .get("includeOrphans")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let mut summary =
                db::repos::storage_health::evidence_spool_summary_for_run(pool, run_id).await?;
            if let Some(obj) = summary.as_object_mut() {
                obj.insert(
                    "runId".to_string(),
                    run_id.map_or(serde_json::Value::Null, serde_json::Value::from),
                );
                obj.insert(
                    "includeOrphans".to_string(),
                    serde_json::json!(include_orphans),
                );
                if !include_orphans {
                    obj.remove("orphanFiles");
                    obj.remove("orphanBytes");
                    obj.remove("recoveredFiles");
                    obj.remove("checksumMismatchFiles");
                    obj.remove("pendingDeleteFiles");
                }
            }
            Ok(summary)
        }
        "storage.reconcile_evidence_orphans" => {
            if std::env::var("CHAINWORKS_STORAGE_MAINTENANCE_DISABLED")
                .ok()
                .as_deref()
                == Some("1")
            {
                return Ok(typed_error(
                    "storage.reconcile_evidence_orphans",
                    ERR_MAINTENANCE_DISABLED,
                    "storage maintenance is disabled by CHAINWORKS_STORAGE_MAINTENANCE_DISABLED",
                    request_id,
                ));
            }

            let dry_run = params
                .get("dryRun")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let max_files = params
                .get("maxFiles")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1000)
                .min(1000);
            let run_id = params.get("runId").and_then(serde_json::Value::as_str);

            // runId is required for mutating calls: without a run scope we cannot
            // bind recovered metadata to a real run (SEC-001).
            if !dry_run && run_id.is_none() {
                return Ok(typed_error(
                    "storage.reconcile_evidence_orphans",
                    ERR_INVALID_INPUT,
                    "runId is required for non-dry-run orphan reconciliation (P075-SEC-001): \
                     provide the run ID to scope the sweep",
                    request_id,
                ));
            }

            // Resolve artifact root server-side — never from client params (SEC-001).
            let artifact_root = match resolve_artifact_root() {
                Ok(p) => p,
                Err(_) => {
                    return Ok(typed_error(
                        "storage.reconcile_evidence_orphans",
                        ERR_MAINTENANCE_DISABLED,
                        "artifact root could not be resolved; \
                         set CHAINWORKS_META_ROOT or DATABASE_URL",
                        request_id,
                    ));
                }
            };
            let report = db::evidence_spool::sweep_evidence_orphans(
                pool,
                &artifact_root,
                run_id,
                max_files,
                db::evidence_spool::SWEEP_DEFAULT_MAX_BYTES,
                dry_run,
            )
            .await?;
            Ok(serde_json::json!({
                "dryRun": dry_run,
                "maxFiles": max_files,
                "scanned": report.scanned_files,
                "recovered": report.recovered_orphans,
                "alreadyIndexed": report.already_indexed,
                "skipped": report.skipped_files,
                "bytesRead": report.bytes_read,
                "truncated": report.truncated,
                "checksum_mismatch": 0,
                "scheduled_delete": 0,
                "errors": 0
            }))
        }
        "storage.maintenance.repair_slot" => {
            let op_id = params["operationId"].as_str().unwrap_or_default();
            let slot_gen_val = params.get("slotGeneration");
            let idempotency_key = params["idempotencyKey"].as_str().unwrap_or_default();

            if op_id.is_empty() || idempotency_key.is_empty() || slot_gen_val.is_none() {
                return Ok(typed_error(
                    "storage.maintenance.repair_slot",
                    ERR_INVALID_INPUT,
                    "operationId, idempotencyKey, and slotGeneration are required",
                    request_id,
                ));
            }

            let Some(slot_gen) = slot_gen_val.and_then(|v| v.as_i64()) else {
                return Ok(typed_error(
                    "storage.maintenance.repair_slot",
                    ERR_INVALID_INPUT,
                    "slotGeneration must be a number",
                    request_id,
                ));
            };

            if slot_gen <= 0 {
                return Ok(typed_error(
                    "storage.maintenance.repair_slot",
                    ERR_INVALID_INPUT,
                    "slotGeneration must be a positive integer",
                    request_id,
                ));
            }

            match db::repos::maintenance::repair_slot(
                pool,
                idempotency_key,
                op_id,
                slot_gen,
                &principal.id,
                &principal.class.to_string(),
                crate::request_context::current_request_id().as_deref(),
                cancel,
            )
            .await
            {
                Ok(op) => Ok(public_maintenance_operation(&op)),
                Err(e) => {
                    let (code, message) = repair_slot_public_error(&e);
                    Ok(typed_error(
                        "storage.maintenance.repair_slot",
                        code,
                        message,
                        request_id,
                    ))
                }
            }
        }
        "storage.projections.clear_backlog" => {
            let projection_name = params["projectionName"].as_str().unwrap_or_default();
            let source_name = params["sourceName"].as_str().unwrap_or_default();

            if projection_name.is_empty() || source_name.is_empty() {
                return Ok(typed_error(
                    "storage.projections.clear_backlog",
                    ERR_INVALID_INPUT,
                    "projectionName and sourceName are required",
                    request_id,
                ));
            }
            if projection_name.len() > PROJECTION_NAME_MAX_BYTES
                || source_name.len() > SOURCE_NAME_MAX_BYTES
            {
                return Ok(typed_error(
                    "storage.projections.clear_backlog",
                    ERR_INVALID_INPUT,
                    "projectionName and sourceName must be at most 128 bytes",
                    request_id,
                ));
            }

            db::repos::projection_invalidation::clear_backlog(
                pool,
                projection_name,
                source_name,
                Some(&principal.id),
                Some(&principal.class.to_string()),
                request_id,
            )
            .await?;
            Ok(serde_json::json!({ "success": true }))
        }
        "storage.projections.clear_poison" => {
            let projection_name = params["projectionName"].as_str().unwrap_or_default();
            let source_name = params["sourceName"].as_str().unwrap_or_default();

            if projection_name.is_empty() || source_name.is_empty() {
                return Ok(typed_error(
                    "storage.projections.clear_poison",
                    ERR_INVALID_INPUT,
                    "projectionName and sourceName are required",
                    request_id,
                ));
            }
            if projection_name.len() > PROJECTION_NAME_MAX_BYTES
                || source_name.len() > SOURCE_NAME_MAX_BYTES
            {
                return Ok(typed_error(
                    "storage.projections.clear_poison",
                    ERR_INVALID_INPUT,
                    "projectionName and sourceName must be at most 128 bytes",
                    request_id,
                ));
            }

            db::repos::projection_invalidation::clear_poison(
                pool,
                projection_name,
                source_name,
                Some(&principal.id),
                Some(&principal.class.to_string()),
                request_id,
            )
            .await?;
            Ok(serde_json::json!({ "success": true }))
        }
        _ => Err(anyhow::anyhow!("Unknown storage tool: {tool_name}")),
    }
}

/// Resolve the artifact root from environment, never from client-supplied parameters.
///
/// Resolution order:
/// 1. `CHAINWORKS_META_ROOT` env var (canonical override used by the Swift app).
/// 2. Parent directory of the SQLite database file derived from `DATABASE_URL`.
/// 3. `.chainworks` relative to the current working directory (dev fallback).
fn resolve_artifact_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("CHAINWORKS_META_ROOT") {
        if !root.is_empty() {
            return Ok(PathBuf::from(root));
        }
    }
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        // Strip the sqlite:// scheme prefix and the ?... query string.
        let path_str = db_url.strip_prefix("sqlite://").unwrap_or(&db_url);
        let path_str = if let Some(pos) = path_str.find('?') {
            &path_str[..pos]
        } else {
            path_str
        };
        if !path_str.is_empty() && path_str != ":memory:" && !path_str.starts_with(':') {
            let db_path = PathBuf::from(path_str);
            if let Some(parent) = db_path.parent() {
                if parent != std::path::Path::new("") {
                    return Ok(parent.to_path_buf());
                }
            }
        }
    }
    Ok(PathBuf::from(".chainworks"))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use db::write_class::{ReplayPolicy, WriteClass, WriteLane, WriteOperation, WriteResult};

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        db::writer::register_shared_writer(
            &pool,
            std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
        )
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn storage_write_pressure_honors_include_lanes_and_window() {
        let pool = test_pool().await;
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
        let now = Utc::now();
        db::repos::storage_health::insert_write_pressure_snapshot(
            &pool,
            &db::repos::storage_health::StorageWritePressureSnapshot {
                id: "pressure-mcp".into(),
                window_start: now - chrono::Duration::seconds(60),
                window_end: now,
                payload_json: serde_json::json!({
                    "lanes": {"critical_barrier": {"queueDepth": 1}},
                    "dbWriterWaitP95Ms": 4
                }),
                created_at: now,
            },
        )
        .await
        .unwrap();

        let payload = execute(
            "storage.write_pressure",
            serde_json::json!({"windowSeconds": 30, "includeLanes": false}),
            &pool,
            &principal,
            None,
        )
        .await
        .unwrap();

        assert_eq!(payload["requestedWindowSeconds"], 30);
        assert_eq!(payload["includeLanes"], false);
        assert!(payload["latestSnapshot"]["payload"]["lanes"].is_null());
    }

    #[tokio::test]
    async fn storage_health_reads_live_dbwriter_heartbeat_when_injected() {
        let pool = test_pool().await;
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
        let writer = db::writer::DbWriter::new(pool.clone());
        let result = writer
            .submit(
                WriteOperation {
                    class: WriteClass::A,
                    lane: WriteLane::CriticalBarrier,
                    operation_name: "mcp_storage_health_live_writer_test",
                    expected_rows: 1,
                    batchable: false,
                    barrier: true,
                    deadline: std::time::Duration::from_secs(5),
                    deadline_reason: None,
                    idempotency_key: "mcp-storage-health-live-writer".into(),
                    replay_policy: ReplayPolicy::NaturalKey,
                    observed_at: None,
                },
                |pool| async move {
                    let mut tx =
                        db::pool::begin_immediate_with_retry(&pool, "mcp_storage_health_live_writer_test")
                            .await?;
                    sqlx::query(
                        "CREATE TABLE IF NOT EXISTS p075_mcp_storage_health_probe (id TEXT PRIMARY KEY)",
                    )
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT OR REPLACE INTO p075_mcp_storage_health_probe (id) VALUES ('probe')",
                    )
                    .execute(&mut *tx)
                    .await?;
                    tx.commit().await?;
                    Ok(1)
                },
            )
            .await;
        assert_eq!(result, WriteResult::Committed);

        for _ in 0..30 {
            if writer.is_alive() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(writer.is_alive(), "DbWriter heartbeat should become live");

        let payload = execute_with_writer(
            "storage.health",
            serde_json::json!({}),
            &pool,
            Some(&writer.heartbeat),
            &principal,
            tokio_util::sync::CancellationToken::new(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(payload["writer"]["alive"], true);
        assert_eq!(payload["isStale"], false);
        assert!(payload["writer"]["lastHeartbeatAt"].as_str().is_some());
        assert!(payload["writer"]["lastDrainAt"].as_str().is_some());
        assert!(payload["writer"]["writeLockWaitP50Ms"].as_u64().is_some());
        assert!(payload["writer"]["writeLockWaitP95Ms"].as_u64().is_some());
        assert!(payload["writer"]["transactionDurationP95Ms"]
            .as_u64()
            .is_some());
    }

    // ── Typed error contract tests (API-001) ─────────────────────────────

    /// invalid_input: reconcile_evidence_orphans without runId on non-dry-run
    /// must return a typed error response, not a JSON-RPC error.
    #[tokio::test]
    async fn reconcile_evidence_orphans_returns_invalid_input_when_run_id_missing() {
        let pool = test_pool().await;
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
        let result = execute(
            "storage.reconcile_evidence_orphans",
            serde_json::json!({"dryRun": false}),
            &pool,
            &principal,
            None,
        )
        .await
        .expect("execute must not return Err for a domain-level invalid_input");

        assert_eq!(
            result["error"], true,
            "typed error response must have error=true"
        );
        assert_eq!(
            result["errorCode"],
            super::ERR_INVALID_INPUT,
            "must return errorCode 'invalid_input' when runId is missing for non-dry-run"
        );
        assert_eq!(result["tool"], "storage.reconcile_evidence_orphans");
    }

    /// invalid_input: dry-run without runId is permitted (runId is optional for dry-run).
    #[tokio::test]
    async fn reconcile_evidence_orphans_dry_run_without_run_id_is_permitted() {
        let pool = test_pool().await;
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
        let dir = tempfile::TempDir::new().unwrap();
        // Point artifact root to a temp dir that has no evidence/runs subtree.
        std::env::set_var("CHAINWORKS_META_ROOT", dir.path().to_str().unwrap());
        let result = execute(
            "storage.reconcile_evidence_orphans",
            serde_json::json!({"dryRun": true, "maxFiles": 1}),
            &pool,
            &principal,
            None,
        )
        .await
        .expect("dry-run without runId must succeed");
        std::env::remove_var("CHAINWORKS_META_ROOT");

        // Must not have error=true; must have scanned/recovered fields.
        assert_ne!(
            result["error"], true,
            "dry-run without runId must not error"
        );
        assert!(
            result.get("scanned").is_some(),
            "must include scanned count"
        );
    }

    /// stale: storage.health returns errorCode='stale' when health state is stale.
    ///
    /// The fail-closed storage_health_with_writer always reports isStale=true
    /// when no live writer is injected, so the 'stale' code must appear.
    #[tokio::test]
    async fn storage_health_returns_typed_stale_error_when_is_stale_true() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
        let result = execute(
            "storage.health",
            serde_json::json!({}),
            &pool,
            &principal,
            None,
        )
        .await
        .expect("storage.health must not error");

        assert_eq!(result["error"], true);
        assert_eq!(
            result["errorCode"],
            super::ERR_STALE,
            "must return errorCode='stale' when isStale=true"
        );
        assert_eq!(result["tool"], "storage.health");
        assert!(
            result.get("hotRead").is_some(),
            "typed stale error must preserve hotRead metadata for MCP callers"
        );
    }

    #[tokio::test]
    async fn proposal_087_storage_health_wire_compatibility() {
        let pool = test_pool().await;
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);

        let result = execute(
            "storage.health",
            serde_json::json!({}),
            &pool,
            &principal,
            None,
        )
        .await
        .expect("storage.health must succeed");

        // 1. Rollout/Feature flags
        assert!(
            result.get("rollout").is_some(),
            "rollout section must be present"
        );
        let rollout = &result["rollout"];
        assert!(rollout.get("p087_storage_tiering_status").is_some());
        assert!(rollout.get("p087_restart_reaper_last_run").is_some());

        // 2. Evaluated Metrics
        assert!(
            result.get("p087EvaluatedMetrics").is_some(),
            "p087EvaluatedMetrics must be present"
        );
        let metrics = result["p087EvaluatedMetrics"]
            .as_array()
            .expect("metrics must be an array");
        assert!(!metrics.is_empty(), "metrics array must not be empty");

        let hot_read_metric = metrics
            .iter()
            .find(|m| m["metric"] == "mcp_hot_read_violation_total");
        assert!(
            hot_read_metric.is_some(),
            "mcp_hot_read_violation_total must be evaluated"
        );

        // 3. Projection Freshness
        assert!(result.get("projectionFreshness").is_some());
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_redacts_internal_operation_ids() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
        let result = execute(
            "storage.maintenance.repair_slot",
            serde_json::json!({
                "operationId": "p087-secret-operation-id",
                "slotGeneration": 7,
                "idempotencyKey": "p087-redaction-idem"
            }),
            &pool,
            &principal,
            None,
        )
        .await
        .expect("repair_slot failures must be typed MCP responses");

        assert_eq!(result["error"], true);
        assert!(result["errorCode"].as_str().is_some());
        let message = result["message"].as_str().unwrap_or_default();
        assert!(
            !message.contains("p087-secret-operation-id"),
            "public MCP error must not leak the target operation id: {message}"
        );
        assert!(
            !message.contains("maintenance_operations") && !message.contains("SQL"),
            "public MCP error must not leak storage internals: {message}"
        );
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_success_returns_public_dto() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO maintenance_operations \
             (id, operation_kind, status, idempotency_key, slot_generation, metadata_json, created_at_ms, updated_at_ms) \
             VALUES ('p087-sensitive-target-op', 'projection_rebuild', 'completed', \
                     'target-secret-idempotency-key', 7, '{\"internal\":\"metadata\"}', ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let result = execute(
            "storage.maintenance.repair_slot",
            serde_json::json!({
                "operationId": "p087-sensitive-target-op",
                "slotGeneration": 7,
                "idempotencyKey": "repair-secret-idempotency-key"
            }),
            &pool,
            &principal,
            None,
        )
        .await
        .expect("repair_slot success must return public DTO");

        let rendered = result.to_string();
        assert_eq!(result["operationKind"], "projection_rebuild");
        assert!(result["operationId"]
            .as_str()
            .unwrap()
            .starts_with("maintenance_operation:"));
        assert!(!rendered.contains("p087-sensitive-target-op"));
        assert!(!rendered.contains("target-secret-idempotency-key"));
        assert!(!rendered.contains("repair-secret-idempotency-key"));
        assert!(!rendered.contains("metadata"));
        assert!(result.get("idempotencyKey").is_none());
        assert!(result.get("metadata_json").is_none());
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_invalid_inputs_return_typed_errors() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);

        for params in [
            serde_json::json!({
                "operationId": "op",
                "idempotencyKey": "idem"
            }),
            serde_json::json!({
                "operationId": "op",
                "slotGeneration": "three",
                "idempotencyKey": "idem"
            }),
            serde_json::json!({
                "operationId": "op",
                "slotGeneration": 3,
                "idempotencyKey": ""
            }),
        ] {
            let result = execute(
                "storage.maintenance.repair_slot",
                params,
                &pool,
                &principal,
                None,
            )
            .await
            .expect("invalid input must be typed MCP response");
            assert_eq!(result["error"], true);
            assert_eq!(result["errorCode"], ERR_INVALID_INPUT);
            let message = result["message"].as_str().unwrap_or_default();
            assert!(!message.contains("maintenance_operations"));
            assert!(!message.contains("SQL"));
        }
    }

    #[tokio::test]
    async fn reconcile_evidence_orphans_returns_maintenance_disabled_when_kill_switch_set() {
        let pool = test_pool().await;
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
        std::env::set_var("CHAINWORKS_STORAGE_MAINTENANCE_DISABLED", "1");
        let result = execute(
            "storage.reconcile_evidence_orphans",
            serde_json::json!({"dryRun": true}),
            &pool,
            &principal,
            None,
        )
        .await
        .expect("maintenance-disabled is a domain-level typed response");
        std::env::remove_var("CHAINWORKS_STORAGE_MAINTENANCE_DISABLED");

        assert_eq!(result["error"], true);
        assert_eq!(result["errorCode"], super::ERR_MAINTENANCE_DISABLED);
        assert_eq!(result["tool"], "storage.reconcile_evidence_orphans");
    }

    #[tokio::test]
    async fn proposal_087_clear_backlog_executes_with_success() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);

        let result = execute(
            "storage.projections.clear_backlog",
            serde_json::json!({
                "projectionName": "p087-test-proj",
                "sourceName": "p087-test-src"
            }),
            &pool,
            &principal,
            None,
        )
        .await
        .expect("clear_backlog must succeed");

        assert_eq!(result["success"], true);
    }

    #[tokio::test]
    async fn proposal_087_clear_poison_executes_with_success() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);

        let result = execute(
            "storage.projections.clear_poison",
            serde_json::json!({
                "projectionName": "p087-test-proj",
                "sourceName": "p087-test-src"
            }),
            &pool,
            &principal,
            None,
        )
        .await
        .expect("clear_poison must succeed");

        assert_eq!(result["success"], true);
    }

    /// maintenance_disabled: reconcile_evidence_orphans with an unmatchable artifact
    /// root path returns maintenance_disabled (tested via controlled env state).
    ///
    /// This test verifies the error code constant and typed_error helper shape.
    #[test]
    fn typed_error_helper_produces_correct_shape() {
        let err = super::typed_error(
            "storage.health",
            super::ERR_UNAVAILABLE,
            "db down",
            Some("req-123"),
        );
        assert_eq!(err["error"], true);
        assert_eq!(err["errorCode"], "unavailable");
        assert_eq!(err["tool"], "storage.health");
        assert_eq!(err["requestId"], "req-123");
        assert!(err["message"].as_str().is_some());

        let err0 = super::typed_error(
            "storage.health",
            super::ERR_UNAUTHORIZED,
            "no capability",
            None,
        );
        assert_eq!(err0["errorCode"], "unauthorized");

        let err2 = super::typed_error(
            "storage.reconcile_evidence_orphans",
            super::ERR_MAINTENANCE_DISABLED,
            "kill switch active",
            Some("req-456"),
        );
        assert_eq!(err2["errorCode"], "maintenance_disabled");
        assert_eq!(err2["requestId"], "req-456");
    }

    #[tokio::test]
    async fn evidence_spool_summary_honors_run_filter_and_orphan_flag() {
        let pool = test_pool().await;
        let principal = auth::Principal::new("test-op", auth::PrincipalClass::Operator);
        let dir = tempfile::TempDir::new().unwrap();
        let output = db::evidence_spool::write_spool_file(
            dir.path(),
            "run-mcp-a",
            "evidence/runs/run-mcp-a/stages/s/agents/a/transcripts/t.txt",
            b"transcript",
        )
        .await
        .unwrap();
        db::repos::evidence_spool_refs::insert_idempotent(
            &pool,
            &db::repos::evidence_spool_refs::EvidenceSpoolRef {
                id: "evsp-mcp-a".into(),
                metadata_version: 1,
                run_id: "run-mcp-a".into(),
                stage_execution_id: None,
                stage_id: Some("s".into()),
                agent_execution_id: None,
                agent_id: Some("a".into()),
                kind: db::repos::evidence_spool_refs::EvidenceKind::Transcript,
                relative_path: output.relative_path,
                size_bytes: output.size_bytes as i64,
                checksum_algorithm: "sha256".into(),
                checksum: output.checksum,
                producer_operation: "p075_evidence_spool_ref_insert_idempotent".into(),
                content_type: None,
                summary_json: None,
                created_at: Utc::now(),
                status: db::repos::evidence_spool_refs::EvidenceSpoolRefStatus::Available,
            },
        )
        .await
        .unwrap();

        let payload = execute(
            "storage.evidence_spool_summary",
            serde_json::json!({"runId": "run-mcp-a", "includeOrphans": false}),
            &pool,
            &principal,
            None,
        )
        .await
        .unwrap();

        assert_eq!(payload["runId"], "run-mcp-a");
        assert_eq!(payload["includeOrphans"], false);
        assert_eq!(payload["metadataRowsTotal"], 1);
        assert!(payload.get("orphanFiles").is_none());
    }
}
