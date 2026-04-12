use anyhow::Result;
use sqlx::SqlitePool;

use domain::commands::{Command, RetryStageCmd};
use domain::ids::RunId;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "stages.retry".to_string(),
        description: "Retry a failed or blocked stage".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["run_id", "stage_id"],
            "properties": {
                "run_id": { "type": "string" },
                "stage_id": { "type": "string" }
            }
        }),
    }]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    _pool: &SqlitePool,
    cmd_handler: &CommandHandler,
) -> Result<serde_json::Value> {
    match tool_name {
        "stages.retry" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let stage_id = params["stage_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'stage_id'"))?
                .to_string();

            let cmd = Command::RetryStage(RetryStageCmd { run_id, stage_id });
            cmd_handler.handle(cmd).await?;
            Ok(serde_json::json!({ "scheduled": true }))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}
