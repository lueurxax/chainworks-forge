use std::path::PathBuf;

use anyhow::Result;
use sqlx::SqlitePool;

use crate::protocol::McpTool;

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
    match tool_name {
        "storage.health" => {
            let include_thresholds = params
                .get("includeThresholds")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let mut health = db::repos::storage_health::storage_health(pool).await?;
            if !include_thresholds {
                if let Some(obj) = health.as_object_mut() {
                    obj.remove("thresholds");
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
            let health = db::repos::storage_health::storage_health(pool).await?;
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
                return Err(anyhow::anyhow!(
                    "runId is required for non-dry-run orphan reconciliation (SEC-001)"
                ));
            }

            // Resolve artifact root server-side — never from client params (SEC-001).
            let artifact_root = resolve_artifact_root()?;
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
