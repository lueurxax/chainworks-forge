use anyhow::Result;
use sqlx::SqlitePool;

use db::repos::steward as steward_repo;
use domain::commands::{Command, RunStewardAnalysisCmd};
use engine::command_handler::{CommandHandler, CommandResult};

use crate::protocol::McpTool;
use crate::request_context::mcp_caller;

pub fn tool_specs() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "steward.run_analysis".to_string(),
            description: "Queue a Steward analysis work item".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "reason": { "type": "string", "description": "manual, post_run_hook, or config_change" },
                    "artifact_base": { "type": "string", "description": "Optional base directory for steward artifacts" }
                }
            }),
        },
        McpTool {
            name: "steward.list_analyses".to_string(),
            description: "List persisted Steward analyses".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "description": "Maximum analyses to return" },
                    "status": { "type": "string", "description": "Optional status filter" }
                }
            }),
        },
        McpTool {
            name: "steward.get_analysis".to_string(),
            description: "Get a persisted Steward analysis with links and recommendations"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "required": ["analysis_id"],
                "properties": {
                    "analysis_id": { "type": "string", "description": "Steward analysis ID" }
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
        "steward.run_analysis" => {
            let reason = params["reason"].as_str().unwrap_or("manual").to_string();
            let caller = mcp_caller(&principal.id, &principal.class, tool_name);
            let commanded = cmd_handler
                .handle(
                    Command::RunStewardAnalysis(RunStewardAnalysisCmd {
                        reason,
                        artifact_base: params["artifact_base"].as_str().map(String::from),
                    }),
                    caller,
                )
                .await?;
            match commanded.result {
                CommandResult::StewardAnalysisQueued => Ok(serde_json::json!({
                    "queued": true,
                    "kind": "steward_analysis",
                    "journal_id": commanded.journal_id,
                })),
                _ => Err(anyhow::anyhow!("Unexpected command result")),
            }
        }
        "steward.list_analyses" => {
            let limit = params["limit"].as_i64().unwrap_or(50);
            let status = params["status"]
                .as_str()
                .map(str::parse)
                .transpose()
                .map_err(|e: String| anyhow::anyhow!(e))?;
            let analyses = steward_repo::list_analyses(pool, limit, status).await?;
            let mut payloads = Vec::with_capacity(analyses.len());
            for analysis in analyses {
                payloads.push(analysis_payload(pool, analysis).await?);
            }
            Ok(serde_json::json!({ "analyses": payloads }))
        }
        "steward.get_analysis" => {
            let analysis_id = params["analysis_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'analysis_id'"))?;
            let analysis = steward_repo::find_analysis(pool, analysis_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Steward analysis not found: {analysis_id}"))?;
            analysis_payload(pool, analysis).await
        }
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

async fn analysis_payload(
    pool: &SqlitePool,
    analysis: domain::steward::StewardAnalysis,
) -> Result<serde_json::Value> {
    let analysis_id = analysis.id.clone();
    let links = steward_repo::list_run_links(pool, &analysis_id).await?;
    let recommendations = steward_repo::list_recommendations(pool, &analysis_id).await?;
    Ok(serde_json::json!({
        "analysis": analysis,
        "run_links": links,
        "recommendations": recommendations
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{ideas, runs, steward, work_items};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{IdeaId, RunId};
    use domain::run::{Run, RunStatus};
    use domain::steward::{
        CohortQuality, StewardAnalysis, StewardAnalysisRunLink, StewardAnalysisStatus,
        StewardRecommendation,
    };
    use engine::event_bus;
    use engine::work_queue::WorkQueue;

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    fn make_command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        CommandHandler::new(pool, events, work_queue)
    }

    fn test_principal() -> auth::Principal {
        auth::Principal::new("test-operator", auth::PrincipalClass::Operator)
    }

    #[tokio::test]
    async fn steward_mcp_tools_tests_manual_trigger_enqueues_work_item() {
        let pool = test_pool().await;
        let handler = make_command_handler(pool.clone());

        let result = execute(
            "steward.run_analysis",
            serde_json::json!({
                "reason": "manual",
                "artifact_base": "/tmp/steward"
            }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();

        assert_eq!(result["queued"], true);
        let item = work_items::claim_next(&pool)
            .await
            .unwrap()
            .expect("steward work item should be queued");
        assert_eq!(item.kind, db::work_item::WorkItemKind::StewardAnalysis);
    }

    #[tokio::test]
    async fn steward_mcp_tools_tests_get_analysis_returns_p049_readback() {
        let pool = test_pool().await;
        let handler = make_command_handler(pool.clone());
        let now = Utc::now();
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(
            &pool,
            &Idea {
                id: idea_id,
                title: "idea".into(),
                body: "body".into(),
                workspace_root_path: None,
                project_key: None,
                status: IdeaStatus::Active,
                created_at: now,
                archived_at: None,
            },
        )
        .await
        .unwrap();
        runs::insert(
            &pool,
            &Run {
                id: run_id,
                idea_id,
                status: RunStatus::Completed,
                workflow_id: "wf".into(),
                workflow_title: "Workflow".into(),
                workspace_root: "/tmp/ws".into(),
                artifact_root: "/tmp/art".into(),
                started_at: now,
                completed_at: Some(now),
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
                delivery_configuration_json: None,
                delivery_preflight_json: None,
                workflow_family: Some("mvp".into()),
                project_key: Some("project".into()),
                risk_class: Some("standard".into()),
                stack: Some("rust".into()),
                workflow_snapshot_hash: Some("a".repeat(64)),
                catalog_snapshot_hash: Some("b".repeat(64)),
                workflow_snapshot_json: Some("{}".into()),
                catalog_snapshot_json: Some("{}".into()),
                drift_detected_at: None,
                drift_details_json: None,
                chainworks_meta_root: None,
            },
        )
        .await
        .unwrap();
        steward::insert_analysis(
            &pool,
            &StewardAnalysis {
                id: "analysis-mcp".into(),
                created_at: now,
                window_start: now,
                window_end: now,
                run_count: 1,
                cohort_keys_json: serde_json::json!({
                    "workflow_family": "mvp",
                    "risk_class": "standard"
                })
                .to_string(),
                cohort_quality: CohortQuality::Acceptable,
                status: StewardAnalysisStatus::Completed,
                degradation_count: 1,
                improvement_count: 0,
                workflow_snapshot_artifact_hash: "workflow".into(),
                agent_catalog_snapshot_hash: "catalog".into(),
                steward_config_snapshot_hash: "config".into(),
                metrics_snapshot_artifact_id: Some("steward/metrics-window.json".into()),
                baseline_snapshot_artifact_id: None,
                agent_catalog_snapshot_artifact_id: None,
                workflow_snapshot_artifact_id: None,
                config_change_log_artifact_id: None,
                health_report_artifact_id: None,
                degradation_alert_artifact_id: None,
                agent_tuning_artifact_id: None,
                workflow_tuning_artifact_id: None,
                experiment_plan_artifact_id: None,
                audit_report_artifact_id: None,
                trigger_reason: "manual".into(),
                error_summary: None,
            },
        )
        .await
        .unwrap();
        steward::insert_run_link(
            &pool,
            &StewardAnalysisRunLink {
                id: "link-mcp".into(),
                analysis_id: "analysis-mcp".into(),
                run_id: run_id.to_string(),
                role: "implicated".into(),
            },
        )
        .await
        .unwrap();
        steward::insert_recommendation(
            &pool,
            &StewardRecommendation {
                id: "rec-mcp".into(),
                analysis_id: "analysis-mcp".into(),
                created_at: now,
                category: "degradation".into(),
                summary: "Regression".into(),
                target_metric: "lead_time_median_seconds".into(),
                confidence_level: "high".into(),
                status: "proposed".into(),
                source_artifact_name: Some("deterministic_signal".into()),
                decision_comment: None,
                decided_at: None,
            },
        )
        .await
        .unwrap();

        let payload = execute(
            "steward.get_analysis",
            serde_json::json!({ "analysis_id": "analysis-mcp" }),
            &pool,
            &handler,
            &test_principal(),
        )
        .await
        .unwrap();
        assert_eq!(payload["analysis"]["run_count"], 1);
        assert_eq!(payload["run_links"][0]["role"], "implicated");
        assert_eq!(
            payload["recommendations"][0]["target_metric"],
            "lead_time_median_seconds"
        );
    }
}
