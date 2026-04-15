use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

use db::repos::{projections, runs};
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

            // JSON-RPC 2.0: notifications (id absent or null) must not
            // receive a response. Only handle + reply for requests.
            let is_notification = request.id.is_none()
                || matches!(&request.id, Some(serde_json::Value::Null));

            if is_notification {
                // Fire-and-forget: process but don't reply.
                let _ = self.handle_request(request).await;
            } else {
                let response = self.handle_request(request).await;
                if let Ok(json) = serde_json::to_string(&response) {
                    let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
                }
            }
        }

        Ok(())
    }

    pub async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
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
                            "description": "Full canonical state for a single workflow run",
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
                            "description": "Execution report for a run: completed stages, artifacts, and decoded validation-failure payloads",
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
            let mut artifact_payloads = Vec::with_capacity(run_artifacts.len());
            for artifact in &run_artifacts {
                artifact_payloads.push(
                    tools::reports::artifact_report_json(&self.pool, artifact).await?,
                );
            }

            return Ok(serde_json::json!({
                "run_id": run_id,
                "status": run_proj.status,
                "total_stages": run_proj.total_stages,
                "completed_stages": run_proj.completed_stages,
                "failed_stages": run_proj.failed_stages,
                "has_artifacts": !artifact_rows.is_empty(),
                "stages": stage_rows,
                "artifact_index": artifact_rows,
                "artifacts": artifact_payloads,
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
                obj.insert("completed_stages".into(), serde_json::json!(row.completed_stages));
                obj.insert("failed_stages".into(), serde_json::json!(row.failed_stages));
                obj.insert("pending_approvals".into(), serde_json::json!(row.pending_approvals));
            }
        }
        Ok(value)
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

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{artifacts, ideas, projections, runs, validation};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::run::{Run, RunStatus};
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
        }
    }

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:").await.expect("in-memory pool failed")
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
        runs::insert(&pool, &make_run(run_id, idea_id)).await.unwrap();

        let server = McpServer::new(pool.clone(), make_command_handler(pool.clone()));
        let value = server.read_resource(&format!("run://{}", run_id)).await.unwrap();
        let run = value.as_object().expect("run object");

        assert_eq!(
            run.get("delivery_configuration_json"),
            Some(&serde_json::json!(
                "{\"repo_identifier\":\"repo-3\",\"repo_root\":\"/repo-3\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
            ))
        );
    }

    #[tokio::test]
    async fn report_resource_decodes_validation_failure_payload() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id)).await.unwrap();

        let payload_path =
            std::env::temp_dir().join(format!("validation-failure-report-{}.json", run_id));
        std::fs::write(&payload_path, serde_json::to_vec(&validation_failure_payload(run_id)).unwrap())
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
            &validation_failure_record(
                artifact.id,
                run_id,
                stage_execution_id,
                agent_execution_id,
            ),
        )
            .await
            .unwrap();

        projections::rebuild_all_for_run(&pool, run_id).await.unwrap();

        let server = McpServer::new(pool.clone(), make_command_handler(pool.clone()));
        let value = server.read_resource(&format!("report://{}", run_id)).await.unwrap();
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
    }
}
