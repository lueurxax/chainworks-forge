use anyhow::Result;
use sqlx::SqlitePool;

use db::repos::approvals;
use domain::commands::{ApproveStageCmd, Command, RejectStageCmd};
use domain::ids::RunId;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "approvals.list".to_string(),
            description: "List pending approvals (the approval inbox)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpTool {
            name: "approvals.resolve".to_string(),
            description: "Approve or reject a pending stage approval".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id", "stage_id", "decision"],
                "properties": {
                    "run_id": { "type": "string" },
                    "stage_id": { "type": "string" },
                    "decision": {
                        "type": "string",
                        "enum": ["granted", "rejected"],
                        "description": "Approval decision"
                    },
                    "comment": { "type": "string" }
                }
            }),
        },
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    cmd_handler: &CommandHandler,
    principal: &auth::Principal,
) -> Result<serde_json::Value> {
    match tool_name {
        "approvals.list" => {
            let items = approvals::list_pending(pool).await?;
            Ok(serde_json::to_value(&items)?)
        }

        "approvals.resolve" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let stage_id = params["stage_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'stage_id'"))?
                .to_string();
            let decision = params["decision"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'decision'"))?;
            let comment = params["comment"].as_str().map(|s| s.to_string());

            let caller = mcp_caller(&principal.id, &principal.class, "approvals.resolve");
            let cmd = match decision {
                "granted" => Command::ApproveStage(ApproveStageCmd {
                    run_id,
                    stage_id,
                    comment,
                }),
                "rejected" => Command::RejectStage(RejectStageCmd {
                    run_id,
                    stage_id,
                    comment,
                }),
                other => return Err(anyhow::anyhow!("Unknown decision: {other}")),
            };

            let commanded = cmd_handler.handle(cmd, caller).await?;
            Ok(serde_json::json!({
                "resolved": true,
                "journal_id": commanded.journal_id,
            }))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}
