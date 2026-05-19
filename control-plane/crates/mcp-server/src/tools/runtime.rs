use anyhow::{anyhow, Result};
use chrono::Utc;
use serde_json::json;
use sqlx::SqlitePool;

use crate::protocol::McpTool;

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "runtime.health".to_string(),
        description: "Read lightweight MCP/runtime liveness status".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }]
}

pub async fn execute(_params: serde_json::Value, pool: &SqlitePool) -> Result<serde_json::Value> {
    let hot_read_circuit_rows =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM hot_read_circuit_states")
            .fetch_one(pool)
            .await
            .unwrap_or(0);

    let runtime_health_projection =
        db::repos::storage_health::runtime_health_projection(pool).await?;

    Ok(json!({
        "schemaVersion": "runtime_health.v1",
        "status": "available",
        "updatedAt": Utc::now().to_rfc3339(),
        "runtimeHealthProjection": runtime_health_projection,
        "mcpRequestLoop": {
            "status": "available",
            "singleRequestSerialized": false
        },
        "hotReadGuard": {
            "status": "available",
            "trackedSurfaces": hot_read_circuit_rows
        }
    }))
}

pub async fn execute_with_name(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
) -> Result<serde_json::Value> {
    match tool_name {
        "runtime.health" => execute(params, pool).await,
        _ => Err(anyhow!("Unknown runtime tool: {tool_name}")),
    }
}
