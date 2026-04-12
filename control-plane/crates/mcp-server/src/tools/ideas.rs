use anyhow::Result;
use sqlx::SqlitePool;

use db::repos::ideas;
use domain::idea::{Idea, IdeaStatus};
use domain::ids::IdeaId;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "ideas.list".to_string(),
            description: "List all ideas, optionally including archived ones".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "include_archived": {
                        "type": "boolean",
                        "description": "Whether to include archived ideas (default: false)"
                    }
                }
            }),
        },
        McpTool {
            name: "ideas.create".to_string(),
            description: "Create a new idea".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["title", "body"],
                "properties": {
                    "title": { "type": "string", "description": "Idea title" },
                    "body": { "type": "string", "description": "Idea body / description" },
                    "workspace_root_path": { "type": "string", "description": "Optional workspace root path" }
                }
            }),
        },
    ]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    _cmd_handler: &CommandHandler,
) -> Result<serde_json::Value> {
    match tool_name {
        "ideas.list" => {
            let include_archived = params["include_archived"].as_bool().unwrap_or(false);
            let items = ideas::list(pool, include_archived).await?;
            Ok(serde_json::to_value(items)?)
        }
        "ideas.create" => {
            let title = params["title"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'title'"))?
                .to_string();
            let body = params["body"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'body'"))?
                .to_string();
            let workspace_root_path = params["workspace_root_path"]
                .as_str()
                .map(|s| s.to_string());

            let idea = Idea {
                id: IdeaId::new(),
                title,
                body,
                workspace_root_path,
                status: IdeaStatus::Draft,
                created_at: chrono::Utc::now(),
                archived_at: None,
            };
            ideas::insert(pool, &idea).await?;
            Ok(serde_json::to_value(&idea)?)
        }
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}
