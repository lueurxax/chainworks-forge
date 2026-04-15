use anyhow::Result;
use sqlx::SqlitePool;

use db::repos::runs;
use domain::commands::{CancelRunCmd, Command, StartRunCmd};
use domain::ids::{IdeaId, RunId};
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "runs.start".to_string(),
            description: "Start a new run for an idea".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["idea_id", "workflow_id", "workflow_title", "workspace_root", "artifact_root"],
                "properties": {
                    "idea_id": { "type": "string", "description": "ID of the idea" },
                    "workflow_id": { "type": "string" },
                    "workflow_title": { "type": "string" },
                    "workspace_root": { "type": "string" },
                    "artifact_root": { "type": "string" },
                    "workflow_yaml_path": { "type": "string", "description": "Path to workflow YAML file (enables state-machine execution)" },
                    "agent_catalog_yaml_path": { "type": "string", "description": "Path to agent catalog YAML file" },
                    "delivery_configuration_json": { "type": "string", "description": "Frozen delivery configuration JSON for repo-backed runs" }
                }
            }),
        },
        McpTool {
            name: "runs.get".to_string(),
            description: "Get a run by ID".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id"],
                "properties": {
                    "run_id": { "type": "string" }
                }
            }),
        },
        McpTool {
            name: "runs.list".to_string(),
            description: "List active runs".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpTool {
            name: "runs.cancel".to_string(),
            description: "Cancel a run".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["run_id"],
                "properties": {
                    "run_id": { "type": "string" }
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
) -> Result<serde_json::Value> {
    match tool_name {
        "runs.start" => {
            let idea_id: IdeaId = params["idea_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'idea_id'"))?
                .parse()?;
            let workflow_id = params["workflow_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'workflow_id'"))?
                .to_string();
            let workflow_title = params["workflow_title"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'workflow_title'"))?
                .to_string();
            let workspace_root = params["workspace_root"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'workspace_root'"))?
                .to_string();
            let artifact_root = params["artifact_root"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'artifact_root'"))?
                .to_string();

            let workflow_yaml_path = params["workflow_yaml_path"]
                .as_str()
                .map(String::from);
            let agent_catalog_yaml_path = params["agent_catalog_yaml_path"]
                .as_str()
                .map(String::from);
            let delivery_configuration_json = params["delivery_configuration_json"]
                .as_str()
                .map(String::from);

            let cmd = Command::StartRun(StartRunCmd {
                idea_id,
                workflow_id,
                workflow_title,
                workspace_root,
                artifact_root,
                delivery_configuration_json,
                workflow_yaml_path,
                agent_catalog_yaml_path,
            });
            let result = cmd_handler.handle(cmd).await?;
            let run_id = match result {
                engine::command_handler::CommandResult::RunStarted { run_id } => run_id,
                _ => return Err(anyhow::anyhow!("Unexpected result")),
            };
            let run = runs::find_by_id(pool, run_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Run not found"))?;
            Ok(serde_json::to_value(&run)?)
        }

        "runs.get" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let run = runs::find_by_id(pool, run_id).await?;
            Ok(serde_json::to_value(&run)?)
        }

        "runs.list" => {
            let items = runs::list_active(pool).await?;
            Ok(serde_json::to_value(&items)?)
        }

        "runs.cancel" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let cmd = Command::CancelRun(CancelRunCmd { run_id });
            cmd_handler.handle(cmd).await?;
            Ok(serde_json::json!({ "cancelled": true }))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{ideas, runs};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::IdeaId;
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    fn make_idea(id: IdeaId) -> Idea {
        Idea {
            id,
            title: "Test idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        }
    }

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:").await.expect("in-memory pool failed")
    }

    fn make_command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        CommandHandler::new(pool, events, work_queue)
    }

    #[tokio::test]
    async fn runs_start_persists_delivery_configuration_json() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let handler = make_command_handler(pool.clone());
        let params = serde_json::json!({
            "idea_id": idea_id.to_string(),
            "workflow_id": "wf-start",
            "workflow_title": "Start Run",
            "workspace_root": "/tmp/ws",
            "artifact_root": "/tmp/art",
            "workflow_yaml_path": null,
            "agent_catalog_yaml_path": null,
            "delivery_configuration_json": "{\"repo_identifier\":\"repo-1\",\"repo_root\":\"/repo\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
        });

        let result = execute("runs.start", params, &pool, &handler).await.unwrap();
        let run_id = result["id"].as_str().expect("run id");
        let run = runs::find_by_id(&pool, run_id.parse().unwrap())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            run.delivery_configuration_json,
            Some(
                "{\"repo_identifier\":\"repo-1\",\"repo_root\":\"/repo\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                    .to_string()
            )
        );
    }
}
