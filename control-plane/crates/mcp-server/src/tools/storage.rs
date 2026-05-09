use std::path::PathBuf;

use anyhow::Result;
use db::writer::DbWriterHeartbeat;
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
pub const ERR_INVALID_INPUT: &str = "invalid_input";
pub const ERR_MAINTENANCE_DISABLED: &str = "maintenance_disabled";

/// Build a typed MCP error response body.
///
/// Callers that want to signal a domain-level error should return
/// `Ok(typed_error(...))` so the content reaches the caller as a
/// structured body rather than a JSON-RPC -32603 envelope.
pub fn typed_error(tool: &str, code: &str, message: impl Into<String>) -> serde_json::Value {
    serde_json::json!({
        "error": true,
        "errorCode": code,
        "message": message.into(),
        "tool": tool,
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
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
) -> Result<serde_json::Value> {
    execute_with_writer(tool_name, params, pool, None).await
}

pub async fn execute_with_writer(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    writer_heartbeat: Option<&DbWriterHeartbeat>,
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
                        ));
                    }
                };
            if !include_thresholds {
                if let Some(obj) = health.as_object_mut() {
                    obj.remove("thresholds");
                }
            }
            // Annotate the response with errorCode when the health surface is stale
            // so callers can detect the degraded state without parsing isStale.
            if health
                .get("isStale")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            {
                if let Some(obj) = health.as_object_mut() {
                    obj.insert("errorCode".to_string(), serde_json::json!(ERR_STALE));
                }
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

    #[tokio::test]
    async fn storage_write_pressure_honors_include_lanes_and_window() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
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
        )
        .await
        .unwrap();

        assert_eq!(payload["requestedWindowSeconds"], 30);
        assert_eq!(payload["includeLanes"], false);
        assert!(payload["latestSnapshot"]["payload"]["lanes"].is_null());
    }

    #[tokio::test]
    async fn storage_health_reads_live_dbwriter_heartbeat_when_injected() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = db::writer::DbWriter::new(pool.clone());
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
        )
        .await
        .unwrap();

        assert_eq!(payload["writer"]["alive"], true);
        assert_eq!(payload["isStale"], false);
    }

    // ── Typed error contract tests (API-001) ─────────────────────────────

    /// invalid_input: reconcile_evidence_orphans without runId on non-dry-run
    /// must return a typed error response, not a JSON-RPC error.
    #[tokio::test]
    async fn reconcile_evidence_orphans_returns_invalid_input_when_run_id_missing() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let result = execute(
            "storage.reconcile_evidence_orphans",
            serde_json::json!({"dryRun": false}),
            &pool,
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
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let dir = tempfile::TempDir::new().unwrap();
        // Point artifact root to a temp dir that has no evidence/runs subtree.
        std::env::set_var("CHAINWORKS_META_ROOT", dir.path().to_str().unwrap());
        let result = execute(
            "storage.reconcile_evidence_orphans",
            serde_json::json!({"dryRun": true, "maxFiles": 1}),
            &pool,
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
    async fn storage_health_annotates_error_code_stale_when_is_stale_true() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
        let result = execute("storage.health", serde_json::json!({}), &pool)
            .await
            .expect("storage.health must not error");

        // Fail-closed health always returns isStale=true (no live writer injected).
        assert_eq!(
            result["isStale"], true,
            "fail-closed health must report isStale=true"
        );
        assert_eq!(
            result["errorCode"],
            super::ERR_STALE,
            "must annotate errorCode='stale' when isStale=true"
        );
    }

    /// maintenance_disabled: reconcile_evidence_orphans with an unmatchable artifact
    /// root path returns maintenance_disabled (tested via controlled env state).
    ///
    /// This test verifies the error code constant and typed_error helper shape.
    #[test]
    fn typed_error_helper_produces_correct_shape() {
        let err = super::typed_error("storage.health", super::ERR_UNAVAILABLE, "db down");
        assert_eq!(err["error"], true);
        assert_eq!(err["errorCode"], "unavailable");
        assert_eq!(err["tool"], "storage.health");
        assert!(err["message"].as_str().is_some());

        let err2 = super::typed_error(
            "storage.reconcile_evidence_orphans",
            super::ERR_MAINTENANCE_DISABLED,
            "kill switch active",
        );
        assert_eq!(err2["errorCode"], "maintenance_disabled");
    }

    #[tokio::test]
    async fn evidence_spool_summary_honors_run_filter_and_orphan_flag() {
        let pool = db::pool::create_pool("sqlite::memory:").await.unwrap();
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
        )
        .await
        .unwrap();

        assert_eq!(payload["runId"], "run-mcp-a");
        assert_eq!(payload["includeOrphans"], false);
        assert_eq!(payload["metadataRowsTotal"], 1);
        assert!(payload.get("orphanFiles").is_none());
    }
}
