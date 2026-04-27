use anyhow::Result;
use sqlx::SqlitePool;

use db::repos::{artifact_contracts, legacy_discovery_overrides, projections, runs};
use domain::commands::{CancelRunCmd, Command, StartRunCmd};
use domain::ids::{IdeaId, RunId};
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "runs.start".to_string(),
            description: "Start a new run for an idea".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["idea_id", "workflow_id", "workflow_title", "workspace_root", "artifact_root", "workflow_yaml_path", "agent_catalog_yaml_path"],
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
    principal: &auth::Principal,
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
                .ok_or_else(|| anyhow::anyhow!("Missing 'workflow_yaml_path'"))?
                .to_string();
            let agent_catalog_yaml_path = params["agent_catalog_yaml_path"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'agent_catalog_yaml_path'"))?
                .to_string();
            let delivery_configuration_json = params["delivery_configuration_json"]
                .as_str()
                .map(String::from);

            let caller = mcp_caller(&principal.id, &principal.class, "runs.start");
            let cmd = Command::StartRun(StartRunCmd {
                idea_id,
                workflow_id,
                workflow_title,
                workspace_root,
                artifact_root,
                delivery_configuration_json,
                workflow_yaml_path,
                agent_catalog_yaml_path,
                review_routing_json: None,
            });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            let run_id = match &commanded.result {
                engine::command_handler::CommandResult::RunStarted { run_id } => *run_id,
                engine::command_handler::CommandResult::StartRunBlockedByDeliveryPreflight(
                    blocked,
                ) => {
                    return Ok(serde_json::json!({
                        "blocked": true,
                        "reason": "delivery_preflight_failed",
                        "delivery_preflight": blocked.delivery_preflight,
                        "journal_id": commanded.journal_id,
                    }));
                }
                _ => return Err(anyhow::anyhow!("Unexpected result")),
            };
            let run = runs::find_by_id(pool, run_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Run not found"))?;
            let mut value = serde_json::to_value(&run)?;
            if let Some(obj) = value.as_object_mut() {
                obj.insert(
                    "journal_id".to_string(),
                    serde_json::Value::String(commanded.journal_id),
                );
            }
            attach_implementation_self_assessment_summary(pool, value).await
        }

        "runs.get" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let run = runs::find_by_id(pool, run_id).await?;
            match run {
                Some(run) => {
                    let mut value = serde_json::to_value(&run)?;
                    if let Some(obj) = value.as_object_mut() {
                        if let Some(projection) =
                            db::repos::artifact_contracts::find_run_state_projection(pool, run_id)
                                .await?
                        {
                            obj.insert(
                                "active_artifact_index".into(),
                                projection.active_index_json,
                            );
                            obj.insert("run_state_projection".into(), projection.run_state_json);
                            obj.insert(
                                "operator_overrides".into(),
                                serde_json::to_value(
                                    db::repos::artifact_contracts::list_overrides(pool, run_id)
                                        .await?,
                                )?,
                            );
                        }
                        obj.insert(
                            "legacy_discovery_overrides".into(),
                            serde_json::to_value(
                                legacy_discovery_overrides::list_by_run(pool, run_id).await?,
                            )?,
                        );
                    }
                    attach_implementation_self_assessment_summary(pool, value).await
                }
                None => Ok(serde_json::Value::Null),
            }
        }

        "runs.list" => {
            let items = projections::list_active_projection(pool).await?;
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(
                    attach_implementation_self_assessment_summary(
                        pool,
                        serde_json::to_value(&item)?,
                    )
                    .await?,
                );
            }
            Ok(serde_json::Value::Array(values))
        }

        "runs.cancel" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;
            let caller = mcp_caller(&principal.id, &principal.class, "runs.cancel");
            let cmd = Command::CancelRun(CancelRunCmd { run_id });
            let commanded = cmd_handler.handle(cmd, caller).await?;
            Ok(serde_json::json!({
                "cancelled": true,
                "journal_id": commanded.journal_id,
            }))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

async fn attach_implementation_self_assessment_summary(
    pool: &SqlitePool,
    mut value: serde_json::Value,
) -> Result<serde_json::Value> {
    let summary = value
        .get("id")
        .and_then(|id| id.as_str())
        .and_then(|id| id.parse::<RunId>().ok());
    let summary = match summary {
        Some(run_id) => {
            artifact_contracts::find_active_implementation_self_assessment_summary(pool, run_id)
                .await?
                .map(|stored| {
                    let mut summary = stored.summary;
                    summary.artifact_path =
                        super::reports::public_artifact_path(&summary.artifact_path);
                    serde_json::to_value(summary)
                })
                .transpose()?
        }
        None => None,
    };

    if let Some(object) = value.as_object_mut() {
        object.insert(
            "implementation_self_assessment_summary".to_string(),
            summary.unwrap_or(serde_json::Value::Null),
        );
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{artifact_contracts, artifacts, ideas, runs};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::run::{Run, RunStatus};
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    fn make_idea(id: IdeaId) -> Idea {
        Idea {
            id,
            title: "Test idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        }
    }

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    fn make_run(id: RunId, idea_id: IdeaId) -> Run {
        Run {
            id,
            idea_id,
            status: RunStatus::Ready,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: None,
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: None,
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: Some(
                "{\"repo_identifier\":\"repo-3\",\"repo_root\":\"/repo-3\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                    .into(),
            ),
            delivery_preflight_json: Some(r#"{"passed":true,"checks":[{"id":"repo_root_exists","passed":true}]}"#.into()),
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: None,
            catalog_snapshot_hash: None,
            workflow_snapshot_json: None,
            catalog_snapshot_json: None,
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: None,
            review_routing_json: None,
        }
    }

    fn test_workflow_yaml_path() -> String {
        format!(
            "{}/../../../examples/workflows/workflow.yaml",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn test_agent_catalog_yaml_path() -> String {
        format!(
            "{}/../../../examples/agents/agents.yaml",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn make_command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        CommandHandler::new(pool, events, work_queue)
    }

    fn test_principal() -> auth::Principal {
        auth::Principal::new("test-operator", auth::PrincipalClass::Operator)
    }

    async fn persist_blocked_implementation_summary(pool: &SqlitePool, run_id: RunId) {
        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_8_implementation_continued".into(),
            agent_id: "code_writer".into(),
            name: "implementation_self_assessment".into(),
            contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/implementation/self-assessment.json".into(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "test".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
        };
        artifacts::insert(pool, &artifact).await.unwrap();
        let raw = serde_json::json!({
            "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
            "implementation_complete": true,
            "verification_green": false,
            "remaining_code_tasks": [],
            "handoff_tasks": [],
            "known_risks": ["verification blocked by environment"],
            "tests_run": ["cargo test: blocked"],
            "docs_impacted": []
        });
        let summary = parse_implementation_self_assessment_v2(
            &raw,
            ContractParseContext {
                run_id: run_id.to_string(),
                run_age: None,
                declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
                canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
                raw_artifact_path: Some(artifact.file_path.clone()),
                source_generation_id: None,
                artifact_created_at: Some(artifact.created_at),
                v2_generation_seen_for_run: true,
                legacy_v1_generation_available: false,
            },
        );
        artifact_contracts::persist_implementation_self_assessment_summary(
            pool,
            run_id,
            artifact.id,
            &artifact.contract_id,
            &summary,
            artifact.created_at,
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn runs_start_persists_delivery_configuration_json() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let handler = make_command_handler(pool.clone());
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(repo.path())
            .output()
            .expect("git init should run");
        let worktrees = tempfile::tempdir().unwrap();
        let delivery_json = format!(
            r#"{{"repo_identifier":"repo-1","repo_root":"{}","base_branch":"main","worktree_base_path":"{}","target_branch":"cw/release","release_target_id":"app-store"}}"#,
            repo.path().display(),
            worktrees.path().display()
        );
        let params = serde_json::json!({
            "idea_id": idea_id.to_string(),
            "workflow_id": "wf-start",
            "workflow_title": "Start Run",
            "workspace_root": "/tmp/ws",
            "artifact_root": "/tmp/art",
            "workflow_yaml_path": test_workflow_yaml_path(),
            "agent_catalog_yaml_path": test_agent_catalog_yaml_path(),
            "delivery_configuration_json": delivery_json
        });

        let result = execute("runs.start", params, &pool, &handler, &test_principal())
            .await
            .unwrap();
        let run_id = result["id"].as_str().expect("run id");
        let run = runs::find_by_id(&pool, run_id.parse().unwrap())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(run.delivery_configuration_json, Some(delivery_json));
    }

    #[tokio::test]
    async fn runs_get_returns_cancellation_settlement_log() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let run = domain::run::Run {
            cancellation_settlement_log: Some(
                serde_json::json!([
                    {
                        "agent_execution_id": "ae-1",
                        "agent_id": "writer",
                        "prior_status": "running",
                        "terminal_status": "cancelled",
                        "session_close_attempted": true,
                        "session_close_succeeded": true,
                        "settled_at": "2026-04-15T10:00:00Z"
                    }
                ])
                .to_string(),
            ),
            ..make_run(RunId::new(), idea_id)
        };
        runs::insert(&pool, &run).await.unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(result["cancellation_settlement_log"].as_str().unwrap()).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!([
                {
                    "agent_execution_id": "ae-1",
                    "agent_id": "writer",
                    "prior_status": "running",
                    "terminal_status": "cancelled",
                    "session_close_attempted": true,
                    "session_close_succeeded": true,
                    "settled_at": "2026-04-15T10:00:00Z"
                }
            ])
        );
    }

    #[tokio::test]
    async fn runs_get_returns_implementation_self_assessment_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();
        persist_blocked_implementation_summary(&pool, run.id).await;

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let summary = &result["implementation_self_assessment_summary"];
        assert_eq!(summary["status"], serde_json::json!("blocked"));
        assert_eq!(summary["implementation_complete"], serde_json::json!(true));
        assert_eq!(summary["verification_green"], serde_json::json!(false));
    }

    #[tokio::test]
    async fn delivery_preflight_mcp_readback_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.get",
            serde_json::json!({ "run_id": run.id.to_string() }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        assert!(result["delivery_preflight_json"]
            .as_str()
            .unwrap()
            .contains("repo_root_exists"));
    }

    #[tokio::test]
    async fn runs_list_returns_projection_summary_not_full_log() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let run = domain::run::Run {
            status: domain::run::RunStatus::Cancelling,
            cancellation_settlement_log: Some(
                serde_json::json!([
                    {
                        "agent_execution_id": "ae-1",
                        "agent_id": "writer",
                        "prior_status": "running",
                        "terminal_status": "cancelled",
                        "session_close_attempted": true,
                        "session_close_succeeded": true,
                        "settled_at": "2026-04-15T10:00:00Z"
                    },
                    {
                        "agent_execution_id": "ae-2",
                        "agent_id": "reviewer",
                        "prior_status": "running",
                        "terminal_status": "cancelled",
                        "session_close_attempted": true,
                        "session_close_succeeded": false,
                        "settled_at": "2026-04-15T10:00:02Z"
                    }
                ])
                .to_string(),
            ),
            ..make_run(RunId::new(), idea_id)
        };
        runs::insert(&pool, &run).await.unwrap();
        db::repos::projections::rebuild_all_for_run(&pool, run.id)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.list",
            serde_json::json!({}),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let item = result.as_array().unwrap().first().unwrap();
        assert_eq!(
            item["cancellation_settlement_summary"],
            serde_json::json!("2/2 agents settled, 1 sessions closed")
        );
        assert!(item.get("cancellation_settlement_log").is_none());
    }

    #[tokio::test]
    async fn runs_list_includes_implementation_self_assessment_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run = make_run(RunId::new(), idea_id);
        runs::insert(&pool, &run).await.unwrap();
        persist_blocked_implementation_summary(&pool, run.id).await;
        db::repos::projections::rebuild_all_for_run(&pool, run.id)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "runs.list",
            serde_json::json!({}),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        let item = result.as_array().unwrap().first().unwrap();
        assert_eq!(
            item["implementation_self_assessment_summary"]["status"],
            serde_json::json!("blocked")
        );
    }
}
