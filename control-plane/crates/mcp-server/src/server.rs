use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

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
                // Expose the primary domain resource URI templates so MCP clients
                // can discover what shadow-truth surfaces the daemon owns.
                JsonRpcResponse::success(id, serde_json::json!({
                    "resources": [
                        {
                            "uri": "chainworks://runs",
                            "name": "Active Runs",
                            "description": "Workflow runs currently tracked by the control-plane daemon",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "chainworks://runs/{run_id}",
                            "name": "Run Detail",
                            "description": "Full state for a single run, sourced from the projection layer",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "chainworks://ideas",
                            "name": "Ideas",
                            "description": "Idea backlog items that back-fill run origin metadata",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "chainworks://approvals/inbox",
                            "name": "Approval Inbox",
                            "description": "Pending stage approvals surfaced from the approval_inbox projection",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "chainworks://runs/{run_id}/stages",
                            "name": "Stage Executions",
                            "description": "Stage execution list for a run, sourced from the stage_summaries projection",
                            "mimeType": "application/json"
                        },
                        {
                            "uri": "chainworks://runs/{run_id}/artifacts",
                            "name": "Artifacts",
                            "description": "Artifact list for a run, sourced from the artifact_index projection",
                            "mimeType": "application/json"
                        }
                    ]
                }))
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
