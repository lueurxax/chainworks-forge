use anyhow::Result;
use sqlx::SqlitePool;

use db::repos::artifacts;
use domain::ids::RunId;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "reports.get".to_string(),
        description: "Get report-kind artifact metadata for a run".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["run_id"],
            "properties": {
                "run_id": { "type": "string", "description": "The run ID to retrieve report artifacts for" }
            }
        }),
    }]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    _cmd_handler: &CommandHandler,
) -> Result<serde_json::Value> {
    match tool_name {
        "reports.get" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;

            let all_artifacts = artifacts::list_by_run(pool, run_id).await?;
            // Filter to report-kind artifacts only
            let reports: Vec<_> = all_artifacts
                .into_iter()
                .filter(|a| a.report_kind.is_some())
                .collect();

            Ok(serde_json::to_value(&reports)?)
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}
