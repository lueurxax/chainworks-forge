use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

use db::repos::{agent_executions, projections, runs, stages};
use engine::command_handler::CommandHandler;

use crate::protocol::JsonRpcRequest;
use crate::protocol::JsonRpcResponse;
use crate::protocol::McpTool;
use crate::tools;
use domain::ResourceTemplateId;

pub struct McpServer {
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    pub principal_table: auth::PrincipalTable,
}

impl McpServer {
    pub fn new(
        pool: SqlitePool,
        cmd_handler: Arc<CommandHandler>,
        principal_table: auth::PrincipalTable,
    ) -> Self {
        Self {
            pool,
            cmd_handler,
            principal_table,
        }
    }

    pub async fn run_stdio(&self) -> Result<()> {
        info!("McpServer: starting stdio JSON-RPC loop");

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        let mut session_principal: Option<auth::Principal> = None;

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                // EOF
                break;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(e) => {
                    let resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {e}"));
                    let json = serde_json::to_string(&resp).unwrap_or_default();
                    let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
                    continue;
                }
            };

            // JSON-RPC 2.0: notifications (id absent or null) must not
            // receive a response. Only handle + reply for requests.
            let is_notification =
                request.id.is_none() || matches!(&request.id, Some(serde_json::Value::Null));

            // Handle initialize: bind session principal from clientInfo.principal_token
            if request.method == "initialize" {
                if session_principal.is_some() {
                    // Re-initialize rejected
                    let resp = JsonRpcResponse::error(
                        request.id.clone(),
                        -32600,
                        "Session already initialized".to_string(),
                    );
                    if let Ok(json) = serde_json::to_string(&resp) {
                        let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
                    }
                    continue;
                }

                let params = request
                    .params
                    .as_ref()
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                let token = params["clientInfo"]["principal_token"].as_str();
                match token {
                    Some(t) => match auth::resolve_bearer(t, &self.principal_table) {
                        Ok(p) => {
                            session_principal = Some(p);
                            // Return normal initialize response
                            let resp = self
                                .handle_request(request, session_principal.as_ref().unwrap())
                                .await;
                            if !is_notification {
                                if let Ok(json) = serde_json::to_string(&resp) {
                                    let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
                                }
                            }
                        }
                        Err(_) => {
                            let resp = JsonRpcResponse::error(
                                request.id.clone(),
                                -32000,
                                "unauthorized: unknown token".to_string(),
                            );
                            if let Ok(json) = serde_json::to_string(&resp) {
                                let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
                            }
                            break;
                        }
                    },
                    None => {
                        let resp = JsonRpcResponse::error(
                            request.id.clone(),
                            -32000,
                            "unauthorized: principal_token required on initialize".to_string(),
                        );
                        if let Ok(json) = serde_json::to_string(&resp) {
                            let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
                        }
                        break;
                    }
                }
                continue;
            }

            // For all other methods, require session_principal
            let principal = match session_principal.as_ref() {
                Some(p) => p,
                None => {
                    let resp = JsonRpcResponse::error(
                        request.id.clone(),
                        -32002,
                        "server not initialized".to_string(),
                    );
                    if let Ok(json) = serde_json::to_string(&resp) {
                        let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
                    }
                    break;
                }
            };

            if is_notification {
                // Fire-and-forget: process but don't reply.
                let _ = self.handle_request(request, principal).await;
            } else {
                let response = self.handle_request(request, principal).await;
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
                }
            }
        }

        Ok(())
    }

    pub async fn handle_request(
        &self,
        req: JsonRpcRequest,
        principal: &auth::Principal,
    ) -> JsonRpcResponse {
        let id = req.id.clone();

        match req.method.as_str() {
            "initialize" => JsonRpcResponse::success(
                id,
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "chainworks-control-plane",
                        "version": "0.1.0"
                    }
                }),
            ),

            "tools/list" => {
                let tools_json: Vec<serde_json::Value> = self
                    .visible_tool_specs(principal)
                    .into_iter()
                    .map(|t| {
                        serde_json::json!({
                            "name": t.name,
                            "description": t.description,
                            "inputSchema": t.input_schema
                        })
                    })
                    .collect();

                JsonRpcResponse::success(id, serde_json::json!({ "tools": tools_json }))
            }

            "tools/call" => {
                let params = req.params.unwrap_or(serde_json::Value::Null);
                let tool_name = match params["name"].as_str() {
                    Some(n) => n.to_string(),
                    None => {
                        return JsonRpcResponse::error(id, -32602, "Missing tool name".to_string());
                    }
                };

                let Some(tool_id) = tools::capability_id_for(&tool_name) else {
                    return JsonRpcResponse::error(
                        id,
                        -32601,
                        format!("Method not found: {tool_name}"),
                    );
                };

                if !principal.tool_capabilities.contains(&tool_id) {
                    return JsonRpcResponse::error(
                        id,
                        -32601,
                        format!("Method not found: {tool_name}"),
                    );
                }

                let tool_params = params["arguments"].clone();

                match self.dispatch_tool(&tool_name, tool_params, principal).await {
                    Ok(result) => JsonRpcResponse::success(
                        id,
                        serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string(&result).unwrap_or_default()
                            }]
                        }),
                    ),
                    Err(e) => JsonRpcResponse::error(id, -32603, e.to_string()),
                }
            }

            "resources/list" => {
                // Expose domain resource URI templates.
                // Primary scheme matches the proposal contract:
                //   run://{id}  idea://{id}  artifact://{id}  report://{run_id}
                // The chainworks:// family is also kept for backward compatibility.
                let filtered: Vec<_> =
                    auth::filter_resources(principal, &auth::all_resource_templates())
                        .into_iter()
                        .map(resource_template_value)
                        .collect();

                JsonRpcResponse::success(id, serde_json::json!({ "resources": filtered }))
            }

            "resources/templates/list" => {
                // No parameterized resource templates yet.
                JsonRpcResponse::success(id, serde_json::json!({ "resourceTemplates": [] }))
            }

            "resources/read" => {
                let params = req.params.unwrap_or(serde_json::Value::Null);
                let uri = match params["uri"].as_str() {
                    Some(u) => u.to_string(),
                    None => {
                        return JsonRpcResponse::error(
                            id,
                            -32602,
                            "resources/read requires a 'uri' parameter".to_string(),
                        )
                    }
                };
                if auth::match_resource_uri(principal, &uri, resource_template_id_for_uri).is_none()
                {
                    return JsonRpcResponse::error(id, -32002, "Resource not found".to_string());
                }
                self.handle_resource_read(id, &uri).await
            }

            "notifications/initialized" => {
                info!("MCP client initialized notification received");
                // JSON-RPC spec: the response for a notification is suppressed
                // by the stdio loop, but we return a no-op here for completeness.
                JsonRpcResponse::success(id, serde_json::json!(null))
            }

            method => {
                error!(method = %method, "Unknown MCP method");
                JsonRpcResponse::error(id, -32601, format!("Method not found: {method}"))
            }
        }
    }

    fn visible_tool_specs(&self, principal: &auth::Principal) -> Vec<McpTool> {
        let ids = tools::all_capability_tool_ids();
        auth::filter_tools(principal, &ids)
            .into_iter()
            .map(tools::mcp_tool_for)
            .collect()
    }

    async fn handle_resource_read(
        &self,
        id: Option<serde_json::Value>,
        uri: &str,
    ) -> JsonRpcResponse {
        let result: anyhow::Result<serde_json::Value> = self.read_resource(uri).await;
        match result {
            Ok(data) => JsonRpcResponse::success(
                id,
                serde_json::json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": data.to_string()
                    }]
                }),
            ),
            Err(e) => JsonRpcResponse::error(id, -32603, e.to_string()),
        }
    }

    async fn read_resource(&self, uri: &str) -> anyhow::Result<serde_json::Value> {
        if let Some(run_id) = uri.strip_prefix("run://") {
            return self.read_canonical_run_resource(run_id).await;
        }

        if let Some(idea_id_str) = uri.strip_prefix("idea://") {
            let idea_id: domain::ids::IdeaId = idea_id_str
                .parse::<uuid::Uuid>()
                .map_err(|_| anyhow::anyhow!("Invalid idea id: {idea_id_str}"))?
                .into();
            return match db::repos::ideas::find_by_id(&self.pool, idea_id).await? {
                Some(idea) => Ok(serde_json::json!({
                    "id": idea.id.to_string(),
                    "title": idea.title,
                    "body": idea.body,
                    "status": idea.status.to_string(),
                    "created_at": idea.created_at.to_rfc3339(),
                })),
                None => anyhow::bail!("Idea not found: {idea_id_str}"),
            };
        }

        if let Some(artifact_id_str) = uri.strip_prefix("artifact://") {
            let artifact_id: domain::ids::ArtifactId = artifact_id_str
                .parse::<uuid::Uuid>()
                .map_err(|_| anyhow::anyhow!("Invalid artifact id: {artifact_id_str}"))?
                .into();
            return match db::repos::artifacts::find_by_id(&self.pool, artifact_id).await? {
                Some(art) => Ok(serde_json::json!({
                    "id": art.id.to_string(),
                    "run_id": art.run_id.to_string(),
                    "stage_id": art.stage_id,
                    "name": art.name,
                    "contract_id": art.contract_id,
                    "format": art.format.to_string(),
                    "file_path": art.file_path,
                    "provider": art.provider,
                    "report_kind": art.report_kind,
                    "created_at": art.created_at.to_rfc3339(),
                })),
                None => anyhow::bail!("Artifact not found: {artifact_id_str}"),
            };
        }

        if let Some(run_id) = uri.strip_prefix("report://") {
            let run_proj = projections::find_run_projection(&self.pool, run_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Run not found: {run_id}"))?;
            let run_id_parsed: domain::ids::RunId = run_id
                .parse::<uuid::Uuid>()
                .map_err(|_| anyhow::anyhow!("Invalid run id: {run_id}"))?
                .into();
            let stage_rows = projections::list_stages_projection(&self.pool, run_id).await?;
            let artifact_rows = projections::list_artifacts_projection(&self.pool, run_id).await?;
            let run_artifacts =
                db::repos::artifacts::list_by_run(&self.pool, run_id_parsed).await?;
            let mut artifact_payloads = Vec::with_capacity(run_artifacts.len());
            for artifact in &run_artifacts {
                artifact_payloads
                    .push(tools::reports::artifact_report_json(&self.pool, artifact).await?);
            }

            return Ok(serde_json::json!({
                "run_id": run_id,
                "status": run_proj.status,
                "total_stages": run_proj.total_stages,
                "completed_stages": run_proj.completed_stages,
                "failed_stages": run_proj.failed_stages,
                "has_artifacts": !artifact_rows.is_empty(),
                "stages": stage_rows,
                "agent_executions": tools::reports::execution_mcp_truth_json(&self.pool, run_id_parsed).await?,
                "artifact_index": artifact_rows,
                "artifacts": artifact_payloads,
            }));
        }

        if let Some(analysis_id) = uri.strip_prefix("steward-analysis://") {
            let analysis = db::repos::steward::find_analysis(&self.pool, analysis_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Steward analysis not found: {analysis_id}"))?;
            let links = db::repos::steward::list_run_links(&self.pool, analysis_id).await?;
            let recommendations =
                db::repos::steward::list_recommendations(&self.pool, analysis_id).await?;
            return Ok(serde_json::json!({
                "analysis": analysis,
                "run_links": links,
                "recommendations": recommendations
            }));
        }

        if uri == "chainworks://runs" {
            let rows = projections::list_active_projection(&self.pool).await?;
            return Ok(serde_json::to_value(rows)?);
        }

        if uri == "chainworks://ideas" {
            let items = db::repos::ideas::list(&self.pool, false).await?;
            return Ok(serde_json::to_value(
                items
                    .iter()
                    .map(|i| {
                        serde_json::json!({
                            "id": i.id.to_string(),
                            "title": i.title,
                            "status": i.status.to_string(),
                            "created_at": i.created_at.to_rfc3339(),
                        })
                    })
                    .collect::<Vec<_>>(),
            )?);
        }

        if uri == "chainworks://approvals/inbox" {
            let rows = projections::list_pending_inbox_projection(&self.pool).await?;
            return Ok(serde_json::to_value(rows)?);
        }

        if let Some(run_id) = uri.strip_prefix("chainworks://runs/") {
            if let Some(rid) = run_id.strip_suffix("/stages") {
                let rows = projections::list_stages_projection(&self.pool, rid).await?;
                return Ok(serde_json::to_value(rows)?);
            } else if let Some(rid) = run_id.strip_suffix("/artifacts") {
                let rows = projections::list_artifacts_projection(&self.pool, rid).await?;
                return Ok(serde_json::to_value(rows)?);
            } else {
                return self.read_canonical_run_resource(run_id).await;
            }
        }

        anyhow::bail!("Unknown resource URI: {}", uri)
    }

    async fn read_canonical_run_resource(&self, run_id: &str) -> anyhow::Result<serde_json::Value> {
        let run_id_parsed: domain::ids::RunId = run_id
            .parse::<uuid::Uuid>()
            .map_err(|_| anyhow::anyhow!("Invalid run id: {run_id}"))?
            .into();
        let run = runs::find_by_id(&self.pool, run_id_parsed)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {run_id}"))?;
        let mut value = serde_json::to_value(run)?;
        if let Some(obj) = value.as_object_mut() {
            if let Some(row) = projections::find_run_projection(&self.pool, run_id).await? {
                obj.insert("total_stages".into(), serde_json::json!(row.total_stages));
                obj.insert(
                    "completed_stages".into(),
                    serde_json::json!(row.completed_stages),
                );
                obj.insert("failed_stages".into(), serde_json::json!(row.failed_stages));
                obj.insert(
                    "pending_approvals".into(),
                    serde_json::json!(row.pending_approvals),
                );
            }
            let stage_rows = stages::list_by_run(&self.pool, run_id_parsed).await?;
            let mut stage_values = Vec::new();
            for stage in stage_rows {
                let executions = agent_executions::find_by_stage(&self.pool, stage.id).await?;
                let mut stage_value = serde_json::to_value(&stage)?;
                if let Some(stage_obj) = stage_value.as_object_mut() {
                    stage_obj.insert("agent_executions".into(), serde_json::to_value(executions)?);
                }
                stage_values.push(stage_value);
            }
            obj.insert("stages".into(), serde_json::Value::Array(stage_values));
        }
        Ok(value)
    }

    async fn dispatch_tool(
        &self,
        tool_name: &str,
        params: serde_json::Value,
        principal: &auth::Principal,
    ) -> Result<serde_json::Value> {
        let pool = &self.pool;
        let cmd = self.cmd_handler.as_ref();

        if tool_name.starts_with("ideas.") {
            tools::ideas::execute(tool_name, params, pool, cmd).await
        } else if tool_name.starts_with("runs.") {
            tools::runs::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("approvals.") {
            tools::approvals::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("stages.") {
            tools::stages::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("reports.") {
            tools::reports::execute(tool_name, params, pool, cmd).await
        } else if tool_name.starts_with("steward.") {
            tools::steward::execute(tool_name, params, pool, cmd, principal).await
        } else {
            Err(anyhow::anyhow!("Unknown tool namespace: {tool_name}"))
        }
    }
}

fn resource_template_id_for_uri(uri: &str) -> Option<ResourceTemplateId> {
    auth::all_resource_templates()
        .into_iter()
        .find(|id| uri_matches_template(uri, resource_template_uri(*id)))
}

fn resource_template_uri(id: ResourceTemplateId) -> &'static str {
    match id {
        ResourceTemplateId::RunEntity => "run://{run_id}",
        ResourceTemplateId::IdeaEntity => "idea://{idea_id}",
        ResourceTemplateId::ArtifactEntity => "artifact://{artifact_id}",
        ResourceTemplateId::ReportEntity => "report://{run_id}",
        ResourceTemplateId::StewardAnalysisEntity => "steward-analysis://{analysis_id}",
        ResourceTemplateId::ChainworksRuns => "chainworks://runs",
        ResourceTemplateId::ChainworksIdeas => "chainworks://ideas",
        ResourceTemplateId::ChainworksApprovalsInbox => "chainworks://approvals/inbox",
        ResourceTemplateId::ChainworksRunStages => "chainworks://runs/{run_id}/stages",
        ResourceTemplateId::ChainworksRunArtifacts => "chainworks://runs/{run_id}/artifacts",
        _ => "unsupported://resource-template",
    }
}

fn uri_matches_template(uri: &str, template: &str) -> bool {
    if let Some(prefix) = template.strip_suffix("{run_id}") {
        return uri.starts_with(prefix) && uri.len() > prefix.len();
    }
    if let Some(prefix) = template.strip_suffix("{idea_id}") {
        return uri.starts_with(prefix) && uri.len() > prefix.len();
    }
    if let Some(prefix) = template.strip_suffix("{artifact_id}") {
        return uri.starts_with(prefix) && uri.len() > prefix.len();
    }
    if let Some(prefix) = template.strip_suffix("{analysis_id}") {
        return uri.starts_with(prefix) && uri.len() > prefix.len();
    }
    if template.ends_with("://") {
        return uri.starts_with(template);
    }
    if uri == template {
        return true;
    }
    let t_parts: Vec<&str> = template.split('/').collect();
    let u_parts: Vec<&str> = uri.split('/').collect();
    if t_parts.len() != u_parts.len() {
        return false;
    }
    t_parts
        .iter()
        .zip(u_parts.iter())
        .all(|(t, u)| t.starts_with('{') && t.ends_with('}') || t == u)
}

fn resource_template_value(id: ResourceTemplateId) -> serde_json::Value {
    match id {
        ResourceTemplateId::RunEntity => serde_json::json!({
            "uri": "run://{run_id}",
            "name": "Run",
            "description": "Full canonical state for a single workflow run",
            "mimeType": "application/json"
        }),
        ResourceTemplateId::IdeaEntity => serde_json::json!({
            "uri": "idea://{idea_id}",
            "name": "Idea",
            "description": "A single idea and its metadata",
            "mimeType": "application/json"
        }),
        ResourceTemplateId::ArtifactEntity => serde_json::json!({
            "uri": "artifact://{artifact_id}",
            "name": "Artifact",
            "description": "A single artifact produced by an agent stage",
            "mimeType": "application/json"
        }),
        ResourceTemplateId::ReportEntity => serde_json::json!({
            "uri": "report://{run_id}",
            "name": "Run Report",
            "description": "Execution report for a run: completed stages, artifacts, and decoded validation-failure payloads",
            "mimeType": "application/json"
        }),
        ResourceTemplateId::StewardAnalysisEntity => serde_json::json!({
            "uri": "steward-analysis://{analysis_id}",
            "name": "Steward Analysis",
            "description": "Persisted Steward analysis, run links, and recommendations",
            "mimeType": "application/json"
        }),
        ResourceTemplateId::ChainworksRuns => serde_json::json!({
            "uri": "chainworks://runs",
            "name": "Active Runs",
            "description": "All workflow runs tracked by the daemon (projection-backed)",
            "mimeType": "application/json"
        }),
        ResourceTemplateId::ChainworksIdeas => serde_json::json!({
            "uri": "chainworks://ideas",
            "name": "Ideas",
            "description": "Idea backlog items",
            "mimeType": "application/json"
        }),
        ResourceTemplateId::ChainworksApprovalsInbox => serde_json::json!({
            "uri": "chainworks://approvals/inbox",
            "name": "Approval Inbox",
            "description": "Pending stage approvals from the approval_inbox projection",
            "mimeType": "application/json"
        }),
        ResourceTemplateId::ChainworksRunStages => serde_json::json!({
            "uri": "chainworks://runs/{run_id}/stages",
            "name": "Stage Executions",
            "description": "Stage list for a run (stage_summaries projection)",
            "mimeType": "application/json"
        }),
        ResourceTemplateId::ChainworksRunArtifacts => serde_json::json!({
            "uri": "chainworks://runs/{run_id}/artifacts",
            "name": "Artifacts",
            "description": "Artifact list for a run (artifact_index projection)",
            "mimeType": "application/json"
        }),
        _ => serde_json::json!({
            "uri": resource_template_uri(id),
            "name": "Unsupported resource template",
            "description": "Unsupported resource template",
            "mimeType": "application/json"
        }),
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn test_mcp_resource_uri_parser_maps_templates_at_server_boundary() {
        assert_eq!(
            resource_template_id_for_uri("run://run-1"),
            Some(ResourceTemplateId::RunEntity)
        );
        assert_eq!(
            resource_template_id_for_uri("chainworks://runs/run-1/artifacts"),
            Some(ResourceTemplateId::ChainworksRunArtifacts)
        );
        assert_eq!(resource_template_id_for_uri("workflow://wf-1"), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{artifacts, ideas, projections, runs, steward, validation};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::run::{Run, RunStatus};
    use domain::steward::{
        CohortQuality, StewardAnalysis, StewardAnalysisRunLink, StewardAnalysisStatus,
        StewardRecommendation,
    };
    use domain::validation::{
        ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
        ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
    };
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

    fn make_run(id: domain::ids::RunId, idea_id: IdeaId) -> Run {
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
        }
    }

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    async fn seed_validation_attempt(
        pool: &sqlx::SqlitePool,
        run_id: RunId,
    ) -> (domain::ids::StageExecutionId, domain::ids::AgentExecutionId) {
        let stage_execution_id = domain::ids::StageExecutionId::new();
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        db::repos::stages::insert(
            pool,
            &domain::stage::StageExecution {
                id: stage_execution_id,
                run_id,
                stage_id: "stage_1".to_string(),
                label: "Stage 1".to_string(),
                status: domain::stage::StageStatus::Failed,
                iteration: 1,
                attempt_number: 1,
                settlement_kind: Some(domain::stage::StageSettlementKind::Failed),
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                owner_agent: Some("validation_agent".to_string()),
                provider: Some("system".to_string()),
                model: None,
                stage_type: None,
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: None,
            },
        )
        .await
        .unwrap();
        db::repos::agent_executions::insert(
            pool,
            &domain::agent::AgentExecution {
                id: agent_execution_id,
                stage_execution_id,
                agent_id: "validation_agent".to_string(),
                provider: "system".to_string(),
                model: None,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                status: domain::agent::AgentStatus::Failed,
                owner_execution_lineage_id: None,
                session_lineage_id: None,
                session_generation_id: None,
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: None,
                session_reuse_scope: None,
                session_family_id: None,
                session_reuse_disposition: Some("reused".into()),
                session_reset_reason: Some("operator_reset".into()),
                backend_profile_id: Some("codex_with_mcp".into()),
                requested_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                actual_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                actual_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                denied_mcp_extensions_json: Some("[]".into()),
                mcp_blocking_issues_json: Some("[]".into()),
                actual_mcp_observation_json: Some(
                    r#"{"source":"provider_session_new_response"}"#.into(),
                ),
                actual_xcode_runtime_observation_json: None,
                mcp_session_startup_latency_ms: Some(17),
            },
        )
        .await
        .unwrap();
        (stage_execution_id, agent_execution_id)
    }

    fn make_command_handler(pool: sqlx::SqlitePool) -> Arc<CommandHandler> {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        Arc::new(CommandHandler::new(pool, events, work_queue))
    }

    fn validation_failure_payload(run_id: RunId) -> serde_json::Value {
        serde_json::json!({
            "id": "44444444-4444-4444-4444-444444444444",
            "timestamp": "2026-04-15T09:30:00Z",
            "agentID": "validation_agent",
            "stageID": "stage_1",
            "runID": run_id.to_string(),
            "outputResults": [{
                "outputName": "report",
                "contractID": "report_v1",
                "status": "failed",
                "missingFields": ["summary"],
                "validationError": "Missing required fields: summary",
                "rawPayloadSize": 17
            }],
            "failureSummary": "report: Missing required fields: summary",
            "failureClass": "output_contract_mismatch",
            "contractMetadata": [{
                "outputName": "report",
                "contractID": "report_v1",
                "machineFormat": "json",
                "validationMode": "strict_structured",
                "requiredFieldCount": 1,
                "rawArtifactName": "report_raw",
                "normalizedArtifactName": "report"
            }],
            "rawOutputExists": true,
            "receiptExists": false,
            "transcriptExists": true,
            "recoveryRecommendation": {
                "action": "retry_failed_agent",
                "explanation": "Retry the agent with the same inputs.",
                "source": "runtime_policy"
            }
        })
    }

    fn validation_failure_record(
        artifact_id: ArtifactId,
        run_id: RunId,
        stage_execution_id: domain::ids::StageExecutionId,
        agent_execution_id: domain::ids::AgentExecutionId,
    ) -> ValidationFailureRecord {
        ValidationFailureRecord {
            id: "44444444-4444-4444-4444-444444444444".to_string(),
            artifact_id,
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-15T09:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
            agent_id: "validation_agent".to_string(),
            stage_id: "stage_1".to_string(),
            stage_execution_id,
            agent_execution_id,
            run_id,
            output_results: vec![OutputValidationResult {
                output_name: "report".to_string(),
                contract_id: Some("report_v1".to_string()),
                status: ValidationStatus::Failed,
                missing_fields: vec!["summary".to_string()],
                validation_error: Some("Missing required fields: summary".to_string()),
                raw_payload_size: 17,
            }],
            failure_summary: "report: Missing required fields: summary".to_string(),
            failure_class: ValidationFailureClass::OutputContractMismatch,
            contract_metadata: vec![ContractValidationMetadata {
                output_name: "report".to_string(),
                contract_id: "report_v1".to_string(),
                machine_format: "json".to_string(),
                validation_mode: "strict_structured".to_string(),
                required_field_count: 1,
                raw_artifact_name: Some("report_raw".to_string()),
                normalized_artifact_name: Some("report".to_string()),
            }],
            raw_output_exists: true,
            receipt_exists: false,
            transcript_exists: true,
            recovery_recommendation: RecoveryRecommendation {
                action: "retry_failed_agent".to_string(),
                explanation: "Retry the agent with the same inputs.".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn run_resource_exposes_delivery_configuration_json() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = domain::ids::RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let value = server
            .read_resource(&format!("run://{}", run_id))
            .await
            .unwrap();
        let run = value.as_object().expect("run object");

        assert_eq!(
            run.get("delivery_configuration_json"),
            Some(&serde_json::json!(
                "{\"repo_identifier\":\"repo-3\",\"repo_root\":\"/repo-3\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
            ))
        );
        assert!(
            run.get("delivery_preflight_json")
                .and_then(serde_json::Value::as_str)
                .unwrap()
                .contains("repo_root_exists"),
            "run:// resource must expose persisted delivery preflight truth"
        );
    }

    #[tokio::test]
    async fn report_resource_decodes_validation_failure_payload() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let payload_path =
            std::env::temp_dir().join(format!("validation-failure-report-{}.json", run_id));
        std::fs::write(
            &payload_path,
            serde_json::to_vec(&validation_failure_payload(run_id)).unwrap(),
        )
        .unwrap();
        let (stage_execution_id, agent_execution_id) = seed_validation_attempt(&pool, run_id).await;

        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "stage_1".into(),
            agent_id: "validation_agent".into(),
            name: "validation_failure_validation_agent".into(),
            contract_id: "validation_failure_record".into(),
            format: ArtifactFormat::Json,
            file_path: payload_path.to_string_lossy().to_string(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("validation_failure".into()),
            report_version: None,
        };
        artifacts::insert(&pool, &artifact).await.unwrap();
        validation::insert(
            &pool,
            &validation_failure_record(artifact.id, run_id, stage_execution_id, agent_execution_id),
        )
        .await
        .unwrap();

        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let value = server
            .read_resource(&format!("report://{}", run_id))
            .await
            .unwrap();
        let report = value.as_object().expect("report object");
        let artifacts = report["artifacts"].as_array().expect("artifacts array");
        let validation_failure = artifacts
            .iter()
            .find(|artifact| artifact["report_kind"] == serde_json::json!("validation_failure"))
            .expect("validation failure artifact");

        assert_eq!(
            validation_failure["validation_failure_record"]["failureSummary"],
            serde_json::json!("report: Missing required fields: summary")
        );
        assert_eq!(
            validation_failure["validation_failure_record"]["contractMetadata"][0]["contractID"],
            serde_json::json!("report_v1")
        );
        assert_eq!(
            validation_failure["validation_failure_record"]["sessionReuseDisposition"],
            serde_json::json!("reused")
        );
        assert_eq!(
            validation_failure["validation_failure_record"]["sessionResetReason"],
            serde_json::json!("operator_reset")
        );
    }

    #[tokio::test]
    async fn report_resource_exposes_mcp_execution_truth() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        seed_validation_attempt(&pool, run_id).await;
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let value = server
            .read_resource(&format!("report://{}", run_id))
            .await
            .unwrap();
        let execution = &value["agent_executions"][0];

        assert_eq!(
            execution["backend_profile_id"],
            serde_json::json!("codex_with_mcp")
        );
        assert_eq!(
            execution["predicted_mcp_runtime_ids_json"],
            serde_json::json!(r#"["fs-runtime"]"#)
        );
        assert_eq!(
            execution["mcp_blocking_issues_json"],
            serde_json::json!("[]")
        );
    }

    #[tokio::test]
    async fn report_resource_decodes_failed_stage_evidence_payload() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let payload_path =
            std::env::temp_dir().join(format!("failed-stage-evidence-report-{run_id}.json"));
        std::fs::write(
            &payload_path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "report_kind": "failed_stage_evidence",
                "run_id": run_id.to_string(),
                "stage_id": "stage_1",
                "failure_summary": "failed",
                "recovery_snapshot": { "status": "available" }
            }))
            .unwrap(),
        )
        .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: ArtifactId::new(),
                run_id,
                stage_id: "stage_1".into(),
                agent_id: "agent_1".into(),
                name: "failed_stage_evidence_stage_1".into(),
                contract_id: "failed_stage_evidence".into(),
                format: ArtifactFormat::Json,
                file_path: payload_path.to_string_lossy().to_string(),
                checksum_sha256: None,
                size_bytes: None,
                provider: "system".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("failed_stage_evidence".into()),
                report_version: Some(1),
            },
        )
        .await
        .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let value = server
            .read_resource(&format!("report://{}", run_id))
            .await
            .unwrap();
        let artifacts = value["artifacts"].as_array().expect("artifacts array");
        let evidence = artifacts
            .iter()
            .find(|artifact| artifact["report_kind"] == serde_json::json!("failed_stage_evidence"))
            .expect("failed-stage evidence artifact");

        assert_eq!(
            evidence["failed_stage_evidence"]["recovery_snapshot"]["status"],
            serde_json::json!("available")
        );
    }

    #[tokio::test]
    async fn steward_mcp_resource_returns_same_persisted_truth_as_tool_readback() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let now = Utc::now();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        steward::insert_analysis(
            &pool,
            &StewardAnalysis {
                id: "analysis-resource".into(),
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
                id: "link-resource".into(),
                analysis_id: "analysis-resource".into(),
                run_id: run_id.to_string(),
                role: "implicated".into(),
            },
        )
        .await
        .unwrap();
        steward::insert_recommendation(
            &pool,
            &StewardRecommendation {
                id: "rec-resource".into(),
                analysis_id: "analysis-resource".into(),
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

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let value = server
            .read_resource("steward-analysis://analysis-resource")
            .await
            .unwrap();
        assert_eq!(value["analysis"]["run_count"], 1);
        assert_eq!(value["run_links"][0]["role"], "implicated");
        assert_eq!(
            value["recommendations"][0]["target_metric"],
            "lead_time_median_seconds"
        );
    }
}
