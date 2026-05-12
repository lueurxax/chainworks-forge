use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tracing::{error, info};

use db::repos::{agent_executions, projections, rollout_contract_checks, runs, stages};
use db::writer::DbWriterHeartbeat;
use engine::command_handler::CommandHandler;
use engine::event_bus::EventSender;

use crate::protocol::JsonRpcRequest;
use crate::protocol::JsonRpcResponse;
use crate::protocol::McpTool;
use crate::tools;
use domain::events::DomainEvent;
use domain::ResourceTemplateId;

pub struct McpServer {
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    pub principal_table: auth::PrincipalTable,
    events: Option<EventSender>,
    storage_writer_heartbeat: Option<Arc<DbWriterHeartbeat>>,
}

async fn write_json_line<T: serde::Serialize>(stdout: &Arc<Mutex<tokio::io::Stdout>>, value: &T) {
    if let Ok(json) = serde_json::to_string(value) {
        let mut stdout = stdout.lock().await;
        let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
    }
}

fn spawn_scheduler_notification_pump(
    events: EventSender,
    stdout: Arc<Mutex<tokio::io::Stdout>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = events.subscribe();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Some(notification) = scheduler_backpressure_mcp_notification(event) {
                        write_json_line(&stdout, &notification).await;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

fn scheduler_backpressure_mcp_notification(event: DomainEvent) -> Option<serde_json::Value> {
    let DomainEvent::SchedulerBackpressureChanged {
        run_id,
        stage_execution_id,
        provider_family,
        top_reason,
        queued_count,
        oldest_queued_age_ms,
        global_queue_depth,
        state,
        updated_at,
        stale_after_ms,
    } = event
    else {
        return None;
    };

    Some(serde_json::json!({
        "jsonrpc": "2.0",
        "method": "scheduler.backpressure.changed",
        "params": {
            "run_id": run_id,
            "stage_execution_id": stage_execution_id,
            "provider_family": provider_family,
            "top_reason": top_reason,
            "queued_count": queued_count,
            "oldest_queued_age_ms": oldest_queued_age_ms,
            "global_queue_depth": global_queue_depth,
            "state": state,
            "updated_at": updated_at.to_rfc3339(),
            "stale_after_ms": stale_after_ms
        }
    }))
}

async fn rollout_contract_readback_json(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> anyhow::Result<serde_json::Value> {
    Ok(
        rollout_contract_checks::find_terminal_rollout_contract_check_for_run(pool, run_id.inner())
            .await?
            .map(|check| check.operator_readback_json_for_lane("mcp"))
            .unwrap_or(serde_json::Value::Null),
    )
}

async fn rollout_contract_readback_lanes_json(
    pool: &SqlitePool,
    run_id: domain::ids::RunId,
) -> anyhow::Result<(serde_json::Value, serde_json::Value)> {
    Ok(
        match rollout_contract_checks::find_terminal_rollout_contract_check_for_run(
            pool,
            run_id.inner(),
        )
        .await?
        {
            Some(check) => (
                check.operator_readback_json_for_lane("mcp"),
                check.operator_readback_json_for_lane("run_report"),
            ),
            None => (serde_json::Value::Null, serde_json::Value::Null),
        },
    )
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
            events: None,
            storage_writer_heartbeat: None,
        }
    }

    pub fn new_with_storage_writer(
        pool: SqlitePool,
        cmd_handler: Arc<CommandHandler>,
        principal_table: auth::PrincipalTable,
        storage_writer_heartbeat: Arc<DbWriterHeartbeat>,
    ) -> Self {
        Self {
            pool,
            cmd_handler,
            principal_table,
            events: None,
            storage_writer_heartbeat: Some(storage_writer_heartbeat),
        }
    }

    pub fn new_with_events(
        pool: SqlitePool,
        cmd_handler: Arc<CommandHandler>,
        principal_table: auth::PrincipalTable,
        events: EventSender,
    ) -> Self {
        Self {
            pool,
            cmd_handler,
            principal_table,
            events: Some(events),
            storage_writer_heartbeat: None,
        }
    }

    pub async fn run_stdio(&self) -> Result<()> {
        info!("McpServer: starting stdio JSON-RPC loop");

        let stdin = tokio::io::stdin();
        let stdout = Arc::new(Mutex::new(tokio::io::stdout()));
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        let mut session_principal: Option<auth::Principal> = None;
        let mut notification_task: Option<tokio::task::JoinHandle<()>> = None;

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
                    write_json_line(&stdout, &resp).await;
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
                    write_json_line(&stdout, &resp).await;
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
                            let is_operator = matches!(p.class, auth::PrincipalClass::Operator);
                            session_principal = Some(p);
                            // Return normal initialize response
                            let resp = self
                                .handle_request(request, session_principal.as_ref().unwrap())
                                .await;
                            if notification_task.is_none() && is_operator {
                                if let Some(events) = &self.events {
                                    notification_task = Some(spawn_scheduler_notification_pump(
                                        events.clone(),
                                        Arc::clone(&stdout),
                                    ));
                                }
                            }
                            if !is_notification {
                                write_json_line(&stdout, &resp).await;
                            }
                        }
                        Err(_) => {
                            let resp = JsonRpcResponse::error(
                                request.id.clone(),
                                -32000,
                                "unauthorized: unknown token".to_string(),
                            );
                            write_json_line(&stdout, &resp).await;
                            break;
                        }
                    },
                    None => {
                        let resp = JsonRpcResponse::error(
                            request.id.clone(),
                            -32000,
                            "unauthorized: principal_token required on initialize".to_string(),
                        );
                        write_json_line(&stdout, &resp).await;
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
                    write_json_line(&stdout, &resp).await;
                    break;
                }
            };

            if is_notification {
                // Fire-and-forget: process but don't reply.
                let _ = self.handle_request(request, principal).await;
            } else {
                let response = self.handle_request(request, principal).await;
                write_json_line(&stdout, &response).await;
            }
        }

        if let Some(task) = notification_task {
            task.abort();
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
                let canonical_tool_name = tools::canonical_tool_name(&tool_name);

                if !tools::p064_operator_tool_enabled(canonical_tool_name) {
                    return JsonRpcResponse::error(
                        id,
                        -32601,
                        format!("Method not found: {tool_name}"),
                    );
                }

                if !principal.tool_capabilities.contains(&tool_id) {
                    if canonical_tool_name.starts_with("storage.") {
                        let result = tools::storage::typed_error(
                            canonical_tool_name,
                            tools::storage::ERR_UNAUTHORIZED,
                            "caller lacks storage diagnostics capability",
                        );
                        return JsonRpcResponse::success(
                            id,
                            serde_json::json!({
                                "content": [{
                                    "type": "text",
                                    "text": serde_json::to_string(&result).unwrap_or_default()
                                }]
                            }),
                        );
                    }
                    return JsonRpcResponse::error(
                        id,
                        -32601,
                        format!("Method not found: {tool_name}"),
                    );
                }

                let tool_params = params["arguments"].clone();

                match self
                    .dispatch_tool(canonical_tool_name, tool_params, principal)
                    .await
                {
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
                        );
                    }
                };
                if auth::match_resource_uri(principal, &uri, resource_template_id_for_uri).is_none()
                {
                    return JsonRpcResponse::error(id, -32002, "Resource not found".to_string());
                }
                self.handle_resource_read(id, &uri, principal).await
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
            .filter(|id| tools::p064_operator_tool_enabled(&tools::mcp_tool_for(*id).name))
            .map(tools::mcp_tool_for)
            .map(tools::codex_compatible_tool)
            .collect()
    }

    async fn handle_resource_read(
        &self,
        id: Option<serde_json::Value>,
        uri: &str,
        principal: &auth::Principal,
    ) -> JsonRpcResponse {
        let result: anyhow::Result<serde_json::Value> =
            self.read_resource_for_principal(uri, principal).await;
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

    #[allow(dead_code)]
    pub(crate) async fn read_resource(&self, uri: &str) -> anyhow::Result<serde_json::Value> {
        self.read_resource_for_principal(
            uri,
            &auth::Principal::new("operator", auth::PrincipalClass::Operator),
        )
        .await
    }

    async fn read_resource_for_principal(
        &self,
        uri: &str,
        principal: &auth::Principal,
    ) -> anyhow::Result<serde_json::Value> {
        if let Some(run_id) = uri.strip_prefix("run://") {
            return self.read_canonical_run_resource(run_id, principal).await;
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
            let (mcp_rollout_readback, run_report_rollout_readback) =
                rollout_contract_readback_lanes_json(&self.pool, run_id_parsed).await?;
            let mut artifact_payloads = Vec::with_capacity(run_artifacts.len());
            for artifact in &run_artifacts {
                artifact_payloads.push(
                    tools::reports::artifact_report_json(
                        &self.pool,
                        artifact,
                        Some(&run_report_rollout_readback),
                    )
                    .await?,
                );
            }
            let closeout_readiness_summary =
                tools::reports::closeout_readiness_summary_json(&self.pool, run_id_parsed).await?;
            let code_writer_completion_receipts =
                tools::reports::code_writer_completion_receipts_json(&self.pool, run_id_parsed)
                    .await?;
            let implementation_completion =
                tools::reports::implementation_completion_json(&self.pool, run_id_parsed).await?;

            return Ok(serde_json::json!({
                "run_id": run_id,
                "status": run_proj.status,
                "total_stages": run_proj.total_stages,
                "completed_stages": run_proj.completed_stages,
                "failed_stages": run_proj.failed_stages,
                "has_artifacts": !artifact_rows.is_empty(),
                "stages": stage_rows,
                "agent_executions": tools::reports::execution_mcp_truth_json(
                    &self.pool,
                    run_id_parsed,
                    principal.class == auth::PrincipalClass::Operator,
                )
                .await?,
                "code_writer_completion_receipts": code_writer_completion_receipts,
                "implementationCompletion": implementation_completion,
                "workflow_conflict": tools::reports::workflow_conflict_json(&self.pool, &self.cmd_handler, run_id_parsed).await?,
                "implementation_self_assessment_summary": tools::reports::implementation_self_assessment_summary_json(&self.pool, run_id_parsed).await?,
                "rollout_contract_readback": mcp_rollout_readback,
                "implementation_closeout_readiness_summary": closeout_readiness_summary.clone(),
                "closeout_readiness_summary": closeout_readiness_summary,
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
                return self.read_canonical_run_resource(run_id, principal).await;
            }
        }

        anyhow::bail!("Unknown resource URI: {}", uri)
    }

    async fn read_canonical_run_resource(
        &self,
        run_id: &str,
        _principal: &auth::Principal,
    ) -> anyhow::Result<serde_json::Value> {
        let run_id_parsed: domain::ids::RunId = run_id
            .parse::<uuid::Uuid>()
            .map_err(|_| anyhow::anyhow!("Invalid run id: {run_id}"))?
            .into();
        let run = runs::find_by_id(&self.pool, run_id_parsed)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {run_id}"))?;
        let mut value = serde_json::to_value(run)?;
        if let Some(obj) = value.as_object_mut() {
            if let Some(projection) =
                db::repos::artifact_contracts::find_run_state_projection(&self.pool, run_id_parsed)
                    .await?
            {
                obj.insert("active_artifact_index".into(), projection.active_index_json);
                obj.insert("run_state_projection".into(), projection.run_state_json);
                obj.insert(
                    "operator_overrides".into(),
                    serde_json::to_value(
                        db::repos::artifact_contracts::list_overrides(&self.pool, run_id_parsed)
                            .await?,
                    )?,
                );
            }
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
            obj.insert(
                "implementation_self_assessment_summary".into(),
                tools::reports::implementation_self_assessment_summary_json(
                    &self.pool,
                    run_id_parsed,
                )
                .await?,
            );
            obj.insert(
                "rollout_contract_readback".into(),
                rollout_contract_readback_json(&self.pool, run_id_parsed).await?,
            );
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
        } else if tool_name.starts_with("stages.")
            || tool_name == "legacy_discovery_override_create"
            || tool_name == "workflow_conflicts.resolve"
        {
            tools::stages::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("reports.") {
            tools::reports::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("artifacts.") {
            tools::artifacts::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("steward.") {
            tools::steward::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("effects.") {
            tools::effects::execute(tool_name, params, pool, principal).await
        } else if tool_name.starts_with("storage.") {
            tools::storage::execute_with_writer(
                tool_name,
                params,
                pool,
                self.storage_writer_heartbeat
                    .as_ref()
                    .map(|heartbeat| heartbeat.as_ref()),
            )
            .await
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
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn scheduler_backpressure_domain_event_maps_to_mcp_notification() {
        let updated_at = chrono::Utc::now();
        let notification =
            scheduler_backpressure_mcp_notification(DomainEvent::SchedulerBackpressureChanged {
                run_id: Some("run-1".into()),
                stage_execution_id: Some("stage-1".into()),
                provider_family: Some("codex".into()),
                top_reason: "provider_capacity".into(),
                queued_count: 2,
                oldest_queued_age_ms: 300_000,
                global_queue_depth: 5,
                state: "active".into(),
                updated_at,
                stale_after_ms: 60_000,
            })
            .expect("scheduler event should map to MCP notification");

        assert_eq!(notification["jsonrpc"], serde_json::json!("2.0"));
        assert_eq!(
            notification["method"],
            serde_json::json!("scheduler.backpressure.changed")
        );
        assert_eq!(notification["params"]["run_id"], serde_json::json!("run-1"));
        assert_eq!(
            notification["params"]["provider_family"],
            serde_json::json!("codex")
        );
        assert_eq!(notification["params"]["state"], serde_json::json!("active"));
        assert_eq!(
            notification["params"]["updated_at"],
            serde_json::json!(updated_at.to_rfc3339())
        );
    }

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

// ── Proposal 029 §9.1 capability-policy tests ────────────────────────────
//
// These tests exercise the same `auth::filter_tools` / `auth::filter_resources`
// / `auth::match_resource_uri` composition that the live MCP handler uses at
// `tools/list`, `tools/call`, `resources/list`, and `resources/read`. They
// stay at the unit level (no axum, no subprocess) so the focused gate can
// run them quickly via `cargo test -p mcp-server <name>`.
#[cfg(test)]
mod p029_capability_tests {
    use super::*;
    use auth::{Principal, PrincipalClass};
    use std::collections::BTreeSet;

    /// Mirror of `McpServer::visible_tool_specs` without needing a server
    /// instance: filter → map to `McpTool` names.
    fn tools_list_names_for(principal: &Principal) -> BTreeSet<String> {
        let ids = tools::all_capability_tool_ids();
        auth::filter_tools(principal, &ids)
            .into_iter()
            .filter(|id| tools::p064_operator_tool_enabled(&tools::mcp_tool_for(*id).name))
            .map(|id| tools::mcp_tool_for(id).name)
            .map(|name| name.replace('.', "_"))
            .collect()
    }

    fn resource_list_uris_for(principal: &Principal) -> BTreeSet<String> {
        auth::filter_resources(principal, &auth::all_resource_templates())
            .into_iter()
            .map(|id| resource_template_uri(id).to_string())
            .collect()
    }

    /// Mirror of the `tools/call` capability check in `server.rs`:
    /// returns `true` iff the call would be allowed, `false` iff it would
    /// return `-32601 Method not found`.
    fn tools_call_allowed(principal: &Principal, tool_name: &str) -> bool {
        if !tools::p064_operator_tool_enabled(tool_name) {
            return false;
        }
        let Some(id) = tools::capability_id_for(tool_name) else {
            return false;
        };
        principal.tool_capabilities.contains(&id)
    }

    fn resources_read_allowed(principal: &Principal, uri: &str) -> bool {
        auth::match_resource_uri(principal, uri, resource_template_id_for_uri).is_some()
    }

    // ── tools/list filtering ─────────────────────────────────────────

    #[test]
    fn test_mcp_tools_list_filtered_for_operator() {
        let op = Principal::new("op", PrincipalClass::Operator);
        let names = tools_list_names_for(&op);
        // Operators see every registered tool (command + read + steward trio).
        for expected in [
            "ideas.create",
            "ideas.list",
            "runs.start",
            "runs.list",
            "runs.get",
            "runs.cancel",
            "approvals.list",
            "approvals.resolve",
            "stages.retry",
            "workflow_conflicts.resolve",
            "legacy_discovery_override_create",
            "reports.get",
            "steward.run_analysis",
            "steward.list_analyses",
            "steward.get_analysis",
        ] {
            let expected = expected.replace('.', "_");
            assert!(
                names.contains(&expected),
                "operator tools/list must expose {expected}, got {names:?}"
            );
        }
    }

    #[test]
    fn test_mcp_tools_list_filtered_for_agent() {
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let names = tools_list_names_for(&ag);
        // Agents can create ideas and start runs but cannot approve, cancel,
        // retry, or enter the steward surface.
        for expected in [
            "ideas.create",
            "ideas.list",
            "runs.start",
            "runs.list",
            "runs.get",
            "reports.get",
        ] {
            let expected = expected.replace('.', "_");
            assert!(names.contains(&expected), "agent missing {expected}");
        }
        for forbidden in [
            "runs.main_sync.request",
            "runs.main_sync.retry",
            "runs.main_sync.set_override",
            "runs.main_sync.repair_state",
            "runs.main_sync.record_recovery_decision",
            "runs.knowledge_capsule.ignore",
            "approvals.list",
            "approvals.resolve",
            "stages.retry",
            "legacy_discovery_override_create",
            "runs.cancel",
            "steward.run_analysis",
            "steward.list_analyses",
            "steward.get_analysis",
        ] {
            let forbidden = forbidden.replace('.', "_");
            assert!(
                !names.contains(&forbidden),
                "agent must not see {forbidden}"
            );
        }
    }

    #[test]
    fn test_mcp_tools_list_filtered_for_observer() {
        let ob = Principal::new("ob", PrincipalClass::Observer);
        let names = tools_list_names_for(&ob);
        // Observer is read-only: sees list/get surfaces + approvals.list +
        // steward readers. Must not see any command tool.
        for expected in [
            "ideas.list",
            "runs.list",
            "runs.get",
            "approvals.list",
            "reports.get",
            "steward.list_analyses",
            "steward.get_analysis",
        ] {
            let expected = expected.replace('.', "_");
            assert!(names.contains(&expected), "observer missing {expected}");
        }
        for forbidden in [
            "ideas.create",
            "runs.start",
            "runs.cancel",
            "approvals.resolve",
            "stages.retry",
            "legacy_discovery_override_create",
            "steward.run_analysis",
        ] {
            let forbidden = forbidden.replace('.', "_");
            assert!(
                !names.contains(&forbidden),
                "observer must not see {forbidden}"
            );
        }
    }

    // ── tools/call denial ────────────────────────────────────────────

    #[test]
    fn test_mcp_tools_call_denied_returns_method_not_found() {
        // An observer invoking a command tool must fail the capability check
        // that `server.rs` turns into -32601 Method not found.
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(!tools_call_allowed(&ob, "runs.start"));
        assert!(!tools_call_allowed(&ob, "approvals.resolve"));
        assert!(!tools_call_allowed(&ob, "runs.cancel"));
        assert!(!tools_call_allowed(&ob, "legacy_discovery_override_create"));

        // Unknown tool name also denied (capability_id_for returns None).
        let op = Principal::new("op", PrincipalClass::Operator);
        assert!(!tools_call_allowed(&op, "runs.main_sync.request"));
        assert!(!tools_call_allowed(&op, "runs.knowledge_capsule.ignore"));
        assert!(!tools_call_allowed(&op, "does.not.exist"));
    }

    // ── resources/list filtering ─────────────────────────────────────

    #[test]
    fn test_mcp_resources_list_is_capability_filtered() {
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let uris = resource_list_uris_for(&ag);
        // Agent must NOT see steward-analysis template or approvals inbox /
        // chainworks run stages / chainworks run artifacts (operator+observer-only).
        for forbidden in [
            "steward-analysis://{analysis_id}",
            "chainworks://approvals/inbox",
            "chainworks://runs/{run_id}/stages",
            "chainworks://runs/{run_id}/artifacts",
        ] {
            assert!(!uris.contains(forbidden), "agent must not see {forbidden}");
        }

        // Observer sees the approvals inbox and the steward template.
        let ob = Principal::new("ob", PrincipalClass::Observer);
        let ob_uris = resource_list_uris_for(&ob);
        assert!(ob_uris.contains("steward-analysis://{analysis_id}"));
        assert!(ob_uris.contains("chainworks://approvals/inbox"));
    }

    // ── resources/read denial ────────────────────────────────────────

    #[test]
    fn test_mcp_resources_read_denied_returns_not_found() {
        // Agent reading a steward-analysis URI: denial path produces
        // -32002 Resource not found in server.rs.
        let ag = Principal::new("ag", PrincipalClass::Agent);
        assert!(!resources_read_allowed(&ag, "steward-analysis://abc-123"));

        // Same agent CAN read run://, idea://, artifact://, report:// (allowed
        // for all classes).
        assert!(resources_read_allowed(&ag, "run://r-1"));
        assert!(resources_read_allowed(&ag, "idea://i-1"));

        // Unknown URI scheme also denied.
        assert!(!resources_read_allowed(&ag, "bogus://1"));

        // Operator can read steward-analysis.
        let op = Principal::new("op", PrincipalClass::Operator);
        assert!(resources_read_allowed(&op, "steward-analysis://abc-123"));
    }

    // ── Steward-specific capability tests ────────────────────────────

    #[test]
    fn test_mcp_tools_list_includes_steward_trio_for_operator() {
        let op = Principal::new("op", PrincipalClass::Operator);
        let names = tools_list_names_for(&op);
        assert!(names.contains("steward_run_analysis"));
        assert!(names.contains("steward_list_analyses"));
        assert!(names.contains("steward_get_analysis"));
    }

    #[test]
    fn test_mcp_tools_list_includes_steward_readers_for_observer() {
        let ob = Principal::new("ob", PrincipalClass::Observer);
        let names = tools_list_names_for(&ob);
        assert!(
            !names.contains("steward_run_analysis"),
            "observer must NOT see steward.run_analysis"
        );
        assert!(names.contains("steward_list_analyses"));
        assert!(names.contains("steward_get_analysis"));
    }

    #[test]
    fn test_mcp_tools_list_excludes_steward_entirely_for_agent() {
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let names = tools_list_names_for(&ag);
        assert!(!names.contains("steward_run_analysis"));
        assert!(!names.contains("steward_list_analyses"));
        assert!(!names.contains("steward_get_analysis"));
    }

    #[test]
    fn test_mcp_tools_call_steward_run_analysis_denied_for_observer_returns_method_not_found() {
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(!tools_call_allowed(&ob, "steward.run_analysis"));
        // But the read-only steward tools ARE allowed.
        assert!(tools_call_allowed(&ob, "steward.list_analyses"));
        assert!(tools_call_allowed(&ob, "steward.get_analysis"));
    }

    #[test]
    fn test_mcp_tools_call_steward_run_analysis_denied_for_agent_returns_method_not_found() {
        let ag = Principal::new("ag", PrincipalClass::Agent);
        assert!(!tools_call_allowed(&ag, "steward.run_analysis"));
        assert!(!tools_call_allowed(&ag, "steward.list_analyses"));
        assert!(!tools_call_allowed(&ag, "steward.get_analysis"));
    }

    #[test]
    fn test_mcp_resources_list_includes_steward_analysis_template_for_operator_and_observer() {
        let op = Principal::new("op", PrincipalClass::Operator);
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(resource_list_uris_for(&op).contains("steward-analysis://{analysis_id}"));
        assert!(resource_list_uris_for(&ob).contains("steward-analysis://{analysis_id}"));
    }

    #[test]
    fn test_mcp_resources_list_excludes_steward_analysis_template_for_agent() {
        let ag = Principal::new("ag", PrincipalClass::Agent);
        assert!(!resource_list_uris_for(&ag).contains("steward-analysis://{analysis_id}"));
    }

    #[test]
    fn test_mcp_resources_read_steward_analysis_denied_for_agent_returns_not_found() {
        let ag = Principal::new("ag", PrincipalClass::Agent);
        // Agent can never open a steward-analysis:// URI — matches the -32002
        // path in server.rs.
        assert!(!resources_read_allowed(&ag, "steward-analysis://xyz-789"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{
        artifact_contracts, artifacts, ideas, projections, rollout_contract_checks, runs, steward,
        validation,
    };
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
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
    use std::fs;
    use std::path::{Path, PathBuf};

    const P041_FIXTURES: &[&str] = &[
        "proposal-loop-basic",
        "implementation-refine-review",
        "approval-pause-resume",
        "retry-recovery-flow",
        "cancelled-or-blocked-run",
        "terminal-report-evidence",
        "projection-readback-surface",
    ];

    fn p041_selected_fixtures() -> Vec<&'static str> {
        match std::env::var("P041_ONLY_FIXTURE") {
            Ok(raw) if !raw.trim().is_empty() => {
                let requested = raw.trim().to_string();
                let fixture = P041_FIXTURES
                    .iter()
                    .copied()
                    .find(|candidate| *candidate == requested.as_str())
                    .unwrap_or_else(|| {
                        panic!("P041_ONLY_FIXTURE {requested:?} is not in P041_FIXTURES")
                    });
                vec![fixture]
            }
            _ => P041_FIXTURES.to_vec(),
        }
    }

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
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: None,
        }
    }

    async fn persist_blocked_implementation_summary(
        pool: &sqlx::SqlitePool,
        run_id: domain::ids::RunId,
    ) {
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
            agent_execution_id: None,
        };
        artifacts::insert(pool, &artifact).await.unwrap();
        let raw = serde_json::json!({
            "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
            "implementation_complete": true,
            "verification_green": false,
            "remaining_code_tasks": [],
            "handoff_tasks": [],
            "known_risks": ["verification blocked"],
            "tests_run": ["proposal-054: blocked"],
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

    async fn persist_rollout_contract_readback(
        pool: &sqlx::SqlitePool,
        run_id: domain::ids::RunId,
    ) {
        use rollout_contract_checks::{
            ProjectionIntegrity, RolloutContractDecision, RolloutContractEnforcementMode,
            RolloutContractLifecycleState, RolloutContractStatus, UpsertRolloutContractCheck,
        };

        let now = Utc::now();
        rollout_contract_checks::upsert_rollout_contract_check(
            pool,
            &UpsertRolloutContractCheck {
                id: uuid::Uuid::new_v4(),
                run_id: run_id.inner(),
                proposal_id: "proposal-084".into(),
                proposal_revision_id: "p084-r5".into(),
                proposal_content_hash: "sha256:proposal".into(),
                contract_object_hash: "sha256:contract".into(),
                content_snapshot_id: "snapshot-1".into(),
                checker_version: "p084-lint-1".into(),
                status: RolloutContractStatus::Fail,
                decision: RolloutContractDecision::Hold,
                lifecycle_state: RolloutContractLifecycleState::Terminal,
                enforcement_mode: RolloutContractEnforcementMode::Enforce,
                failure_reasons: vec!["missing_metrics".into()],
                diagnostics: vec!["bounded diagnostic".into()],
                waiver: None,
                rollback_disposition: serde_json::json!({
                    "mode": "feature_flag_disable_or_enforcement_mode_permissive",
                    "data_loss_risk": "none",
                    "steps": ["Move enforcement mode through an audited mutation."]
                }),
                projection_integrity: ProjectionIntegrity::Valid,
                cutover_policy_revision: Some("p084-cutover-v1".into()),
                redaction_state: "partial".into(),
                retry_count: 0,
                preflight_timeout_seconds: 45,
            },
            now,
        )
        .await
        .unwrap();
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
                stage_execution_id: Some(stage_execution_id),
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
                owner_kind: None,
                owner_id: None,
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
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
        persist_blocked_implementation_summary(&pool, run_id).await;
        persist_rollout_contract_readback(&pool, run_id).await;

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
        assert_eq!(
            run["implementation_self_assessment_summary"]["status"],
            serde_json::json!("blocked"),
            "run:// resource must expose implementation self-assessment summary"
        );
        assert_eq!(
            run["rollout_contract_readback"]["schema_version"],
            serde_json::json!("operator_readback_v1"),
            "run:// resource must expose rollout contract operator readback"
        );
        assert_eq!(
            run["rollout_contract_readback"]["backend_decision"],
            serde_json::json!("hold")
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
            agent_execution_id: None,
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
                agent_execution_id: None,
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
    async fn proposal_041_report_resource_readback_parity_surface() {
        for fixture_id in p041_selected_fixtures() {
            // Same cross-binary ordering dependency as the graphql-server
            // P041 readback test: under `cargo test --workspace` the
            // engine integration binary and this mcp-server lib binary
            // run in parallel. If the engine test hasn't produced the
            // report/DB yet (or the operator cleaned `target/parity/`),
            // skip this fixture. The dedicated
            // `./scripts/test-gate.sh proposal-041` lane orders both
            // correctly and is the authoritative readiness signal.
            let report_path = p041_report_path(fixture_id);
            let replay_path = p041_replay_path(fixture_id);
            if !report_path.is_file() || !replay_path.is_file() {
                eprintln!(
                    "P041 MCP readback: skipping fixture '{fixture_id}' — engine-side \
                     replay has not produced {} yet.",
                    report_path.display()
                );
                return;
            }
            let mut report = p041_report(fixture_id);
            let replay = p041_replay(fixture_id);
            let run_id = replay["run_id"].as_str().expect("run_id");
            let db_path =
                workspace_root().join(report["database_ref"].as_str().expect("database_ref"));
            if !db_path.is_file() {
                eprintln!(
                    "P041 MCP readback: skipping fixture '{fixture_id}' — engine-side \
                     replay DB {} is missing (likely cleaned between runs).",
                    db_path.display()
                );
                return;
            }
            let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
                .await
                .expect("open P041 fixture DB");
            let handler = make_command_handler(pool.clone());
            let tool_value = crate::tools::reports::execute(
                "reports.get",
                serde_json::json!({ "run_id": run_id }),
                &pool,
                &handler,
                &auth::Principal::new("operator", auth::PrincipalClass::Operator),
            )
            .await
            .unwrap();
            let server =
                McpServer::new(pool.clone(), handler, auth::PrincipalTable::test_fixture());
            let resource_value = server
                .read_resource(&format!("report://{}", run_id))
                .await
                .unwrap();
            let actual = normalize_p041_mcp_actual(tool_value, resource_value);
            update_p041_surface(
                &mut report,
                "mcp_report_readback",
                actual,
                "mcp-server::tools::reports::execute + mcp-server::server::McpServer::read_resource",
            );
            write_p041_report(fixture_id, &report);
        }
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("mcp crate should be under control-plane/crates")
            .to_path_buf()
    }

    fn control_plane_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("mcp crate should be under control-plane/crates")
            .to_path_buf()
    }

    fn p041_report_path(fixture_id: &str) -> PathBuf {
        control_plane_root()
            .join("target/parity/reports")
            .join(p041_generation_id())
            .join(fixture_id)
            .join("behavioral-diff-report.json")
    }

    fn p041_replay_path(fixture_id: &str) -> PathBuf {
        control_plane_root()
            .join("target/parity/work")
            .join(p041_generation_id())
            .join(fixture_id)
            .join("server-replay.json")
    }

    fn p041_generation_id() -> String {
        let generation_id = std::env::var("P041_PUBLICATION_GENERATION_ID")
            .unwrap_or_else(|_| "unscoped-fixture-replay".to_string());
        assert_safe_p041_generation_id(&generation_id);
        generation_id
    }

    fn assert_safe_p041_generation_id(raw: &str) {
        if raw == "unscoped-fixture-replay" {
            return;
        }
        let valid_prefix = raw.starts_with("p041-");
        let valid_chars = raw
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | ':' | 'T' | 'Z'));
        assert!(
            valid_prefix
                && valid_chars
                && !raw.contains("..")
                && !raw.contains('/')
                && !raw.contains('\\'),
            "P041_PUBLICATION_GENERATION_ID must be a safe path segment"
        );
    }

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(path).expect("read JSON")).expect("parse JSON")
    }

    fn p041_report(fixture_id: &str) -> serde_json::Value {
        read_json(&p041_report_path(fixture_id))
    }

    fn p041_replay(fixture_id: &str) -> serde_json::Value {
        read_json(&p041_replay_path(fixture_id))
    }

    fn write_p041_report(fixture_id: &str, report: &serde_json::Value) {
        fs::write(
            p041_report_path(fixture_id),
            serde_json::to_string_pretty(report).expect("serialize P041 report"),
        )
        .expect("write P041 report");
    }

    fn normalize_p041_mcp_actual(
        tool_value: serde_json::Value,
        resource_value: serde_json::Value,
    ) -> serde_json::Value {
        // P041 §6.5: exclude mcp_execution_truth (runtime-only) and
        // canonical_artifact_contracts (P057 system projection, not in golden fixtures).
        let tool_reports = tool_value
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|report| report["report_kind"] != serde_json::json!("mcp_execution_truth"))
            .filter(|report| {
                report["report_kind"] != serde_json::json!("canonical_artifact_contracts")
            })
            .map(|report| {
                serde_json::json!({
                    "kind": report["report_kind"],
                    "version": report["report_version"].as_i64().unwrap_or(1),
                    "fixture_id": resource_value["run"]["workflow_id"],
                })
            })
            .collect::<Vec<_>>();
        let tool_report_artifacts = report_artifact_names_from_reports(&tool_value);
        let resource_report_artifacts =
            report_artifact_names_from_reports(&resource_value["artifacts"]);
        serde_json::json!({
            "collector_owner": "mcp-server::tools::reports::execute + mcp-server::server::McpServer::read_resource",
            "tool": {
                "name": "reports.get",
                "reports": tool_reports,
                "report_artifacts": tool_report_artifacts,
            },
            "resource": {
                "uri": "report://$run_id",
                "reports": resource_value["artifacts"].as_array().cloned().unwrap_or_default().into_iter()
                    .filter(|artifact| !artifact["report_kind"].is_null())
                    .map(|artifact| serde_json::json!({
                        "kind": artifact["report_kind"],
                        "version": artifact["report_version"].as_i64().unwrap_or(1),
                        "fixture_id": resource_value["run"]["workflow_id"],
                    }))
                    .collect::<Vec<_>>(),
                "report_artifacts": resource_report_artifacts,
            },
        })
    }

    fn report_artifact_names_from_reports(value: &serde_json::Value) -> Vec<String> {
        let mut names = value
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|artifact| {
                !artifact["report_kind"].is_null()
                    && artifact["report_kind"] != serde_json::json!("mcp_execution_truth")
                    && artifact["report_kind"] != serde_json::json!("canonical_artifact_contracts")
            })
            .filter_map(|artifact| artifact["name"].as_str().map(str::to_string))
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    fn update_p041_surface(
        report: &mut serde_json::Value,
        surface: &str,
        actual: serde_json::Value,
        collector_owner: &str,
    ) {
        let comparisons = report["surface_comparisons"]
            .as_array_mut()
            .expect("surface_comparisons");
        let comparison = comparisons
            .iter_mut()
            .find(|item| item["surface"] == serde_json::json!(surface))
            .expect("surface comparison");
        let expected = comparison["expected"].clone();
        let matched = expected == actual;
        comparison["actual"] = actual.clone();
        comparison["collector_owner"] = serde_json::json!(collector_owner);
        comparison["status"] = serde_json::json!(if matched { "matched" } else { "diverged" });

        let divergences = report["divergences"].as_array_mut().expect("divergences");
        divergences.retain(|item| item["owner_surface"] != serde_json::json!(surface));
        if !matched {
            divergences.push(serde_json::json!({
                "path": format!("$.{surface}"),
                "expected": expected,
                "actual": actual,
                "severity": "blocking",
                "owner_surface": surface,
                "investigation_hint": "P041 fixture-bound MCP readback diverged from expected client truth."
            }));
        }
        let blocking_count = divergences
            .iter()
            .filter(|item| item["severity"] == "blocking")
            .count();
        report["summary"]["blocking_count"] = serde_json::json!(blocking_count);
        report["verdict"] = serde_json::json!(if blocking_count == 0 { "ready" } else { "red" });
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

    // ───────────────────────────────────────────────────────────────────
    // Proposal 029 §4.4 / §9.1 — `journal_id` surfacing contract
    // ───────────────────────────────────────────────────────────────────
    //
    // MCP command tools (runs.start, runs.cancel, approvals.resolve,
    // stages.retry, steward.run_analysis) include `journal_id` inside
    // `content[0].text`'s stringified JSON. Direct (non-CommandHandler)
    // tools (runs.list, runs.get, ideas.list, reports.get,
    // steward.list_analyses, steward.get_analysis) MUST NOT include
    // `journal_id` — they never produce a journal row.
    //
    // The MCP wire format at `tools/call` is:
    //   response.result = { "content": [{ "type":"text", "text": <stringified JSON> }] }
    // These tests parse `text`, then assert the inner payload has/omits
    // `journal_id`.

    use crate::protocol::JsonRpcRequest;

    /// Drive a `tools/call` request through `handle_request` and return the
    /// parsed inner payload from `result.content[0].text`.
    async fn call_tool_and_parse(
        server: &McpServer,
        principal: &auth::Principal,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> serde_json::Value {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments,
            })),
        };
        let resp = server.handle_request(req, principal).await;
        let result = resp
            .result
            .expect("tools/call response must have result when the call succeeded");
        let text = result["content"][0]["text"]
            .as_str()
            .expect("result.content[0].text is a string")
            .to_string();
        serde_json::from_str(&text).expect("content[0].text parses as JSON")
    }

    fn operator_principal() -> auth::Principal {
        auth::Principal::new("test-operator", auth::PrincipalClass::Operator)
    }

    #[tokio::test]
    async fn proposal_075_storage_tool_dispatch_returns_typed_unauthorized() {
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let observer = auth::Principal::new("storage-observer", auth::PrincipalClass::Observer);

        let payload =
            call_tool_and_parse(&server, &observer, "storage.health", serde_json::json!({})).await;

        assert_eq!(payload["error"], true);
        assert_eq!(payload["errorCode"], tools::storage::ERR_UNAUTHORIZED);
        assert_eq!(payload["tool"], "storage.health");
    }

    #[tokio::test]
    async fn proposal_075_storage_tool_dispatch_returns_typed_maintenance_disabled() {
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );

        std::env::set_var("CHAINWORKS_STORAGE_MAINTENANCE_DISABLED", "1");
        let payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "storage.reconcile_evidence_orphans",
            serde_json::json!({"dryRun": true}),
        )
        .await;
        std::env::remove_var("CHAINWORKS_STORAGE_MAINTENANCE_DISABLED");

        assert_eq!(payload["error"], true);
        assert_eq!(
            payload["errorCode"],
            tools::storage::ERR_MAINTENANCE_DISABLED
        );
        assert_eq!(payload["tool"], "storage.reconcile_evidence_orphans");
    }

    #[tokio::test]
    async fn test_mcp_tools_call_response_includes_journal_id_in_content_text() {
        // runs.cancel is a command tool — its payload must carry journal_id
        // inside the MCP content[0].text wire format.
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "runs.cancel",
            serde_json::json!({ "run_id": run_id.to_string() }),
        )
        .await;

        let journal_id = payload["journal_id"]
            .as_str()
            .expect("runs.cancel response must contain journal_id as a string");
        assert!(
            !journal_id.is_empty(),
            "journal_id must be non-empty (uuid from CommandHandler::handle)"
        );
    }

    #[tokio::test]
    async fn test_mcp_read_only_tool_response_omits_journal_id() {
        // runs.list never invokes CommandHandler — no journal row is written,
        // so no journal_id should appear in the response payload.
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "runs.list",
            serde_json::json!({}),
        )
        .await;

        let top_level_has_journal_id = payload
            .as_object()
            .map(|m| m.contains_key("journal_id"))
            .unwrap_or(false);
        assert!(
            !top_level_has_journal_id,
            "read-only tool runs.list must not emit journal_id at top level, got {payload}"
        );
    }

    #[tokio::test]
    async fn test_mcp_steward_list_analyses_response_omits_journal_id() {
        // steward.list_analyses is a direct reader, not a command tool.
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "steward.list_analyses",
            serde_json::json!({}),
        )
        .await;
        let has_journal_id = payload
            .as_object()
            .map(|m| m.contains_key("journal_id"))
            .unwrap_or_else(|| {
                // `steward.list_analyses` returns an array — whose members
                // should not contain journal_id either.
                if let Some(arr) = payload.as_array() {
                    arr.iter().any(|v| {
                        v.as_object()
                            .map(|m| m.contains_key("journal_id"))
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            });
        assert!(
            !has_journal_id,
            "steward.list_analyses (read-only) must not surface journal_id"
        );
    }

    #[tokio::test]
    async fn test_mcp_steward_get_analysis_response_omits_journal_id() {
        // Seed one analysis so steward.get_analysis can return it.
        let pool = test_pool().await;
        let now = Utc::now();
        steward::insert_analysis(
            &pool,
            &StewardAnalysis {
                id: "analysis-omit".into(),
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
                degradation_count: 0,
                improvement_count: 0,
                workflow_snapshot_artifact_hash: "w".into(),
                agent_catalog_snapshot_hash: "c".into(),
                steward_config_snapshot_hash: "s".into(),
                metrics_snapshot_artifact_id: None,
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

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "steward.get_analysis",
            serde_json::json!({ "analysis_id": "analysis-omit" }),
        )
        .await;

        let has_journal_id = payload
            .as_object()
            .map(|m| m.contains_key("journal_id"))
            .unwrap_or(false);
        assert!(
            !has_journal_id,
            "steward.get_analysis (read-only) must not surface journal_id"
        );
    }

    #[tokio::test]
    async fn test_mcp_steward_run_analysis_response_includes_journal_id() {
        // steward.run_analysis is a command tool — every successful call
        // must include journal_id in the response payload. The command
        // will likely succeed with zero runs / no artifacts in this
        // minimal fixture, but even on failure the MCP layer inserts
        // journal_id alongside the error surface per tool implementation.
        //
        // To keep the test hermetic and fast we only assert: if the tool
        // call succeeds (returns Some(result)), the parsed payload contains
        // a journal_id key. Failure handling is tested elsewhere.
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );

        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "steward.run_analysis",
                "arguments": { "reason": "manual" },
            })),
        };
        let resp = server.handle_request(req, &operator_principal()).await;
        if let Some(result) = resp.result {
            let text = result["content"][0]["text"]
                .as_str()
                .expect("content[0].text");
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(
                parsed.get("journal_id").is_some(),
                "steward.run_analysis success payload must include journal_id, got {parsed}"
            );
        } else {
            // Command failed (likely missing steward runtime setup). In that
            // case the journal row WAS still written — verify via DB lookup.
            let row: (String,) = sqlx::query_as(
                "SELECT id FROM command_journal WHERE command_type = 'RunStewardAnalysis' ORDER BY created_at DESC LIMIT 1"
            )
            .fetch_one(&pool)
            .await
            .expect(
                "steward.run_analysis must insert a command_journal row even when execution fails",
            );
            assert!(
                !row.0.is_empty(),
                "journal_id must be set on the audit row for the failed run"
            );
        }
    }
}
