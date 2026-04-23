use anyhow::Result;
use sqlx::SqlitePool;

use domain::commands::{Command, RetryStageCmd};
use domain::ids::RunId;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "stages.retry".to_string(),
        description: "Retry a failed or blocked stage".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["run_id", "stage_id"],
            "properties": {
                "run_id": { "type": "string" },
                "stage_id": { "type": "string" },
                "agent_execution_id": {
                    "type": "string",
                    "description": "Optional. Retry only this InvokeAgent execution instead of the full stage fanout."
                },
                "consume_quota_budget_now": {
                    "type": "boolean",
                    "description": "Allow an early retry before a persisted quota retry_after has elapsed."
                }
            }
        }),
    }]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    _pool: &SqlitePool,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
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
            let consume_quota_budget_now = params["consume_quota_budget_now"]
                .as_bool()
                .unwrap_or(false);
            let agent_execution_id = params["agent_execution_id"]
                .as_str()
                .map(|value| value.parse())
                .transpose()?;

            let caller = mcp_caller(&principal.id, &principal.class, "stages.retry");
            let cmd = Command::RetryStage(RetryStageCmd {
                run_id,
                stage_id,
                consume_quota_budget_now,
                agent_execution_id,
            });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            Ok(serde_json::json!({
                "scheduled": true,
                "journal_id": commanded.journal_id,
            }))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}
