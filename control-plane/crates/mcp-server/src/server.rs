use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

use db::repos::projections;
use engine::command_handler::CommandHandler;

use crate::protocol::{JsonRpcRequest, JsonRpcResponse, McpTool};
use crate::tools;

pub struct McpServer {
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    tool_specs: Vec<McpTool>,
}

impl McpServer {
    pub fn new(pool: SqlitePool, cmd_handler: Arc<CommandHandler>) -> Self {
        let mut specs = Vec::new();
        specs.extend(tools::ideas::tool_specs());
        specs.extend(tools::runs::tool_specs());
        specs.extend(tools::approvals::tool_specs());
        specs.extend(tools::stages::tool_specs());
        specs.extend(tools::reports::tool_specs());

        Self {
            pool,
            cmd_handler,
            tool_specs: specs,
        }
    }

    pub async fn run_stdio(&self) -> Result<()> {
        info!("McpServer: starting stdio JSON-RPC loop");

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

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

            let response = self.handle_request(request).await;
            if let Ok(json) = serde_json::to_string(&response) {
                // MCP rule: responses go to stdout, logs to stderr
                let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
            }
        }

        Ok(())
    }

    async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
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
                    .tool_specs
                    .iter()
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
                let tool_params = params["arguments"].clone();

                match self.dispatch_tool(&tool_name, tool_params).await {
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
                JsonRpcResponse::success(id, serde_json::json!({
                    "resources": [
                        // ── Proposal-spec URI family (P027 §8.1) ──────────────
                        {
                            "uri": "run://{run_id}",
                            "name": "Run",
                            "description": "Full state for a single workflow run (projection-backed shadow truth)",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "idea://{idea_id}",
                            "name": "Idea",
                            "description": "A single idea and its metadata",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "artifact://{artifact_id}",
                            "name": "Artifact",
                            "description": "A single artifact produced by an agent stage",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "report://{run_id}",
                            "name": "Run Report",
                            "description": "Execution report for a run: completed stages and their artifacts",
                            "mimeType": "application/json"
                        },
                        // ── chainworks:// family (collection surfaces) ─────────
                        {
                            "uri": "chainworks://runs",
                            "name": "Active Runs",
                            "description": "All workflow runs tracked by the daemon (projection-backed)",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "chainworks://ideas",
                            "name": "Ideas",
                            "description": "Idea backlog items",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "chainworks://approvals/inbox",
                            "name": "Approval Inbox",
                            "description": "Pending stage approvals from the approval_inbox projection",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "chainworks://runs/{run_id}/stages",
                            "name": "Stage Executions",
                            "description": "Stage list for a run (stage_summaries projection)",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "chainworks://runs/{run_id}/artifacts",
                            "name": "Artifacts",
                            "description": "Artifact list for a run (artifact_index projection)",
                            "mimeType": "application/json"
                        }
                    ]
                }))
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
                self.handle_resource_read(id, &uri).await
            }

            "notifications/initialized" => {
                // Notification — no response needed, but return an empty result
                JsonRpcResponse::success(id, serde_json::json!(null))
            }

            method => {
                error!(method = %method, "Unknown MCP method");
                JsonRpcResponse::error(id, -32601, format!("Method not found: {method}"))
            }
        }
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
        // ── Proposal-spec URI scheme (P027 §8.1) ─────────────────────────────
        if let Some(run_id) = uri.strip_prefix("run://") {
            return match projections::find_run_projection(&self.pool, run_id).await? {
                Some(row) => Ok(serde_json::to_value(row)?),
                None => anyhow::bail!("Run not found: {run_id}"),
            };
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
            // Run report: projection summary + completed stages + their artifacts.
            let run_proj = projections::find_run_projection(&self.pool, run_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Run not found: {run_id}"))?;
            let run_id_parsed: domain::ids::RunId = run_id
                .parse::<uuid::Uuid>()
                .map_err(|_| anyhow::anyhow!("Invalid run id: {run_id}"))?
                .into();
            let stage_rows = projections::list_stages_projection(&self.pool, run_id).await?;
            let artifact_rows =
                projections::list_artifacts_projection(&self.pool, run_id).await?;
            let run_artifacts = db::repos::artifacts::list_by_run(&self.pool, run_id_parsed)
                .await?;

            return Ok(serde_json::json!({
                "run_id": run_id,
                "status": run_proj.status,
                "total_stages": run_proj.total_stages,
                "completed_stages": run_proj.completed_stages,
                "failed_stages": run_proj.failed_stages,
                "has_artifacts": !artifact_rows.is_empty(),
                "stages": stage_rows,
                "artifact_index": artifact_rows,
                "artifacts": run_artifacts.iter().map(|a| serde_json::json!({
                    "id": a.id.to_string(),
                    "name": a.name,
                    "stage_id": a.stage_id,
                    "contract_id": a.contract_id,
                    "format": a.format.to_string(),
                    "file_path": a.file_path,
                    "provider": a.provider,
                    "report_kind": a.report_kind,
                })).collect::<Vec<_>>(),
            }));
        }

        // ── chainworks:// collection surfaces ────────────────────────────────
        if uri == "chainworks://runs" {
            let rows = projections::list_active_projection(&self.pool).await?;
            return Ok(serde_json::to_value(rows)?);
        }

        if uri == "chainworks://ideas" {
            let items = db::repos::ideas::list(&self.pool, false).await?;
            return Ok(serde_json::to_value(
                items
                    .iter()
                    .map(|i| serde_json::json!({
                        "id": i.id.to_string(),
                        "title": i.title,
                        "status": i.status.to_string(),
                        "created_at": i.created_at.to_rfc3339(),
                    }))
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
                return match projections::find_run_projection(&self.pool, run_id).await? {
                    Some(row) => Ok(serde_json::to_value(row)?),
                    None => anyhow::bail!("Run not found: {}", run_id),
                };
            }
        }

        anyhow::bail!("Unknown resource URI: {}", uri)
    }

    async fn dispatch_tool(
        &self,
        tool_name: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let pool = &self.pool;
        let cmd = self.cmd_handler.as_ref();

        if tool_name.starts_with("ideas.") {
            tools::ideas::execute(tool_name, params, pool, cmd).await
        } else if tool_name.starts_with("runs.") {
            tools::runs::execute(tool_name, params, pool, cmd).await
        } else if tool_name.starts_with("approvals.") {
            tools::approvals::execute(tool_name, params, pool, cmd).await
        } else if tool_name.starts_with("stages.") {
            tools::stages::execute(tool_name, params, pool, cmd).await
        } else if tool_name.starts_with("reports.") {
            tools::reports::execute(tool_name, params, pool, cmd).await
        } else {
            Err(anyhow::anyhow!("Unknown tool namespace: {tool_name}"))
        }
    }
}
