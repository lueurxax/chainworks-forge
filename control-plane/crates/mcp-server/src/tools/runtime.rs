use anyhow::Result;
use sqlx::SqlitePool;

use crate::protocol::McpTool;

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "runtime.health".to_string(),
        description: "Read compact ACP/runtime health projection".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {}
        }),
    }]
}

pub async fn execute(
    tool_name: &str,
    _params: serde_json::Value,
    pool: &SqlitePool,
) -> Result<serde_json::Value> {
    match tool_name {
        "runtime.health" => {
            let health = db::repos::storage_health::storage_health(pool).await?;
            Ok(health["runtimeHealthProjection"].clone())
        }
        _ => Err(anyhow::anyhow!("Unknown runtime tool: {tool_name}")),
    }
}
