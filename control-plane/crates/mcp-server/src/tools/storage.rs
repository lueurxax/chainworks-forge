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
                    "runid": {
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
            description: "Run the P075 evidence orphan sweep for an artifact root and backfill recovered_orphan metadata".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "artifact_root": {
                        "type": "string",
                        "description": "Artifact root containing evidence/runs"
                    },
                    "runid": {
                        "type": "string"
                    },
                    "dryrun": {
                        "type": "boolean",
                        "default": true
                    },
                    "maxfiles": {
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
        "storage.health" => db::repos::storage_health::storage_health(pool).await,
        "storage.write_pressure" => {
            let health = db::repos::storage_health::storage_health(pool).await?;
            Ok(health["writePressure"].clone())
        }
        "storage.evidence_spool_summary" => {
            db::repos::storage_health::evidence_spool_summary(pool).await
        }
        "storage.reconcile_evidence_orphans" => {
            let dry_run = params
                .get("dryrun")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let max_files = params
                .get("maxfiles")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(1000)
                .min(1000);
            let Some(artifact_root) = params["artifact_root"].as_str() else {
                return Ok(serde_json::json!({
                    "dryrun": dry_run,
                    "maxfiles": max_files,
                    "scanned": 0,
                    "recovered": 0,
                    "checksum_mismatch": 0,
                    "scheduled_delete": 0,
                    "skipped": 0,
                    "errors": 0,
                    "unavailableReason": "artifact_root_not_mounted"
                }));
            };
            if dry_run {
                return Ok(serde_json::json!({
                    "dryrun": true,
                    "maxfiles": max_files,
                    "scanned": 0,
                    "recovered": 0,
                    "checksum_mismatch": 0,
                    "scheduled_delete": 0,
                    "skipped": 0,
                    "errors": 0,
                    "unavailableReason": "dry_run_inventory_not_mounted"
                }));
            }
            let report = db::evidence_spool::sweep_evidence_orphans(
                pool,
                std::path::Path::new(artifact_root),
            )
            .await?;
            Ok(serde_json::json!({
                "dryrun": false,
                "maxfiles": max_files,
                "scanned": report.scanned_files,
                "recovered": report.recovered_orphans,
                "checksum_mismatch": 0,
                "scheduled_delete": 0,
                "skipped": report.skipped_files,
                "errors": 0,
                "already_indexed": report.already_indexed
            }))
        }
        _ => Err(anyhow::anyhow!("Unknown storage tool: {tool_name}")),
    }
}
