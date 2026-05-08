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
                "properties": {}
            }),
        },
        McpTool {
            name: "storage.write_pressure".to_string(),
            description: "Read the latest P075 storage write-pressure snapshot".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpTool {
            name: "storage.evidence_spool_summary".to_string(),
            description: "Read compact P075 evidence spool metadata counts and byte totals".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpTool {
            name: "storage.reconcile_evidence_orphans".to_string(),
            description: "Run the P075 evidence orphan sweep for an artifact root and backfill recovered_orphan metadata".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["artifact_root"],
                "properties": {
                    "artifact_root": {
                        "type": "string",
                        "description": "Artifact root containing evidence/runs"
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
            let latest = db::repos::storage_health::latest_write_pressure_snapshot(pool).await?;
            Ok(serde_json::json!({ "latestSnapshot": latest }))
        }
        "storage.evidence_spool_summary" => {
            db::repos::storage_health::evidence_spool_summary(pool).await
        }
        "storage.reconcile_evidence_orphans" => {
            let artifact_root = params["artifact_root"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'artifact_root'"))?;
            let report = db::evidence_spool::sweep_evidence_orphans(
                pool,
                std::path::Path::new(artifact_root),
            )
            .await?;
            Ok(serde_json::to_value(report)?)
        }
        _ => Err(anyhow::anyhow!("Unknown storage tool: {tool_name}")),
    }
}
