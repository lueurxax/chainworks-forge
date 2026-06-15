use std::path::Path;

use anyhow::{Context, Result};
use sqlx::SqlitePool;

use db::repos::ideas;
use domain::commands::{Command, CreateIdeaCmd};
use engine::command_handler::{CommandHandler, CommandResult};

use crate::protocol::McpTool;
use crate::request_context;

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
                "required": ["title", "body", "idempotency_key"],
                "properties": {
                    "title": { "type": "string", "description": "Idea title" },
                    "body": { "type": "string", "description": "Idea body / description" },
                    "workspace_root_path": { "type": "string", "description": "Optional workspace root path" },
                    "project_key": { "type": "string", "description": "Optional stable project cohort key" },
                    "idempotency_key": { "type": "string", "description": "Required UUIDv7 per attempt for safe retry." }
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
            if let Some(path) = workspace_root_path.as_deref() {
                validate_workspace_root_path(path)?;
            }
            let project_key = params["project_key"].as_str().map(|s| s.to_string());

            let commanded = cmd_handler
                .handle(
                    Command::CreateIdea(CreateIdeaCmd {
                        title,
                        body,
                        workspace_root_path,
                        project_key,
                    }),
                    request_context::mcp_caller(principal, "ideas.create"),
                )
                .await?;
            match commanded.result {
                CommandResult::IdeaCreated { idea } => Ok(serde_json::json!({
                    "idea": idea,
                    "journal_id": commanded.journal_id,
                })),
                _ => Err(anyhow::anyhow!(
                    "ideas.create returned unexpected command result"
                )),
            }
        }
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

fn validate_workspace_root_path(path: &str) -> Result<()> {
    if path.contains('\0') {
        anyhow::bail!("ideas.create: workspace_root_path contains a null byte");
    }
    if path.contains('\\') {
        anyhow::bail!("ideas.create: workspace_root_path contains a backslash separator");
    }
    for component in path.split('/') {
        if component == ".." {
            anyhow::bail!(
                "ideas.create: workspace_root_path contains a path traversal component '..'"
            );
        }
    }
    if path.contains("://") {
        anyhow::bail!("ideas.create: workspace_root_path contains a URI scheme separator");
    }
    let raw = Path::new(path);
    if !raw.exists() {
        anyhow::bail!(
            "ideas.create: workspace_root_path '{}' does not exist; create the directory before using it",
            path
        );
    }
    let canonical = std::fs::canonicalize(raw)
        .with_context(|| format!("ideas.create: canonicalize workspace_root_path '{path}'"))?;
    super::runs::reject_broad_workspace_root(&canonical).map_err(|_| {
        anyhow::anyhow!(
            "ideas.create: workspace_root_path '{}' is too broad; choose a project directory",
            canonical.display()
        )
    })
}
