use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tracing::{error, info};

use db::repos::{agent_executions, projections, rollout_contract_checks, runs, stages};
use db::writer::DbWriterHeartbeat;
use engine::command_handler::CommandHandler;
use engine::event_bus::EventSender;

use crate::hot_read_guard;
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
    principal_source: auth::LivePrincipalSource,
    events: Option<EventSender>,
    storage_writer_heartbeat: Option<Arc<DbWriterHeartbeat>>,
    // P081 Phase 3: shared immutable boundary policy service injected at daemon startup.
    boundary_policy: Option<Arc<auth::boundary::BoundaryPolicy>>,
}

fn embedded_shadow_boundary_policy() -> Arc<auth::boundary::BoundaryPolicy> {
    Arc::new(
        auth::boundary::BoundaryPolicy::from_embedded_with_mode(auth::boundary::PolicyMode::Shadow)
            .expect("embedded P081 boundary fixture must be valid"),
    )
}

async fn write_json_line<T: serde::Serialize>(stdout: &Arc<Mutex<tokio::io::Stdout>>, value: &T) {
    if let Ok(json) = serde_json::to_string(value) {
        let mut stdout = stdout.lock().await;
        let _ = stdout.write_all(format!("{json}\n").as_bytes()).await;
        let _ = stdout.flush().await;
    }
}

fn public_json_rpc_request_id(id: &Option<serde_json::Value>) -> String {
    let Some(value) = id.as_ref() else {
        return uuid::Uuid::new_v4().to_string();
    };

    match value {
        serde_json::Value::String(raw) => public_request_id_reference(raw),
        serde_json::Value::Number(number) => public_request_id_reference(&number.to_string()),
        _ => uuid::Uuid::new_v4().to_string(),
    }
}

fn public_request_id_reference(raw: &str) -> String {
    if raw.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        let mut hasher = Sha256::new();
        hasher.update(raw.as_bytes());
        let digest = hasher.finalize();
        let short: String = format!("{digest:x}").chars().take(16).collect();
        format!("request_id:sha256:{short}")
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
        let principal_source = auth::LivePrincipalSource::new(principal_table.clone());
        Self {
            pool,
            cmd_handler,
            principal_table,
            principal_source,
            events: None,
            storage_writer_heartbeat: None,
            boundary_policy: Some(embedded_shadow_boundary_policy()),
        }
    }

    pub fn new_with_storage_writer(
        pool: SqlitePool,
        cmd_handler: Arc<CommandHandler>,
        principal_table: auth::PrincipalTable,
        storage_writer_heartbeat: Arc<DbWriterHeartbeat>,
    ) -> Self {
        let principal_source = auth::LivePrincipalSource::new(principal_table.clone());
        Self {
            pool,
            cmd_handler,
            principal_table,
            principal_source,
            events: None,
            storage_writer_heartbeat: Some(storage_writer_heartbeat),
            boundary_policy: Some(embedded_shadow_boundary_policy()),
        }
    }

    pub fn new_with_storage_writer_and_boundary_policy(
        pool: SqlitePool,
        cmd_handler: Arc<CommandHandler>,
        principal_table: auth::PrincipalTable,
        storage_writer_heartbeat: Arc<DbWriterHeartbeat>,
        boundary_policy: Arc<auth::boundary::BoundaryPolicy>,
    ) -> Self {
        let principal_source = auth::LivePrincipalSource::new(principal_table.clone());
        Self {
            pool,
            cmd_handler,
            principal_table,
            principal_source,
            events: None,
            storage_writer_heartbeat: Some(storage_writer_heartbeat),
            boundary_policy: Some(boundary_policy),
        }
    }

    pub fn new_with_events(
        pool: SqlitePool,
        cmd_handler: Arc<CommandHandler>,
        principal_table: auth::PrincipalTable,
        events: EventSender,
    ) -> Self {
        let principal_source = auth::LivePrincipalSource::new(principal_table.clone());
        Self {
            pool,
            cmd_handler,
            principal_table,
            principal_source,
            events: Some(events),
            storage_writer_heartbeat: None,
            boundary_policy: Some(embedded_shadow_boundary_policy()),
        }
    }

    /// P081 Phase 3: attach the shared immutable boundary policy service.
    pub fn with_boundary_policy(mut self, policy: Arc<auth::boundary::BoundaryPolicy>) -> Self {
        self.boundary_policy = Some(policy);
        self
    }

    pub fn with_live_principal_source(mut self, source: auth::LivePrincipalSource) -> Self {
        self.principal_source = source;
        self
    }

    pub fn live_principal_source(&self) -> auth::LivePrincipalSource {
        self.principal_source.clone()
    }

    pub fn principal_table_handle(&self) -> auth::LivePrincipalSource {
        self.live_principal_source()
    }

    pub fn resolve_current_bearer(&self, token: &str) -> anyhow::Result<auth::Principal> {
        self.principal_source
            .resolve_bearer(token)
            .map_err(|_| anyhow::anyhow!("unauthorized"))
    }

    pub fn resolve_current_credential(
        &self,
        credential: &auth::LivePrincipalCredential,
    ) -> anyhow::Result<auth::Principal> {
        self.principal_source
            .resolve_credential(credential)
            .map_err(|_| anyhow::anyhow!("unauthorized"))
    }

    pub async fn run_stdio(&self) -> Result<()> {
        info!("McpServer: starting stdio JSON-RPC loop");

        let stdin = tokio::io::stdin();
        let stdout = Arc::new(Mutex::new(tokio::io::stdout()));
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        let mut session_credential: Option<auth::LivePrincipalCredential> = None;
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
                if session_credential.is_some() {
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
                    // SEC-P081: apply the same length and character-set validation to the
                    // stdio principal_token as extract_bearer_token applies to HTTP headers.
                    // A raw token that fails validation is rejected as unauthorized so that
                    // malformed or oversized values cannot reach resolve_bearer.
                    Some(t) if !auth::validate_raw_token(t) => {
                        tracing::warn!(
                            "MCP stdio: principal_token failed format validation; rejecting"
                        );
                        write_json_line(
                            &stdout,
                            &JsonRpcResponse::error(
                                request.id.clone(),
                                -32000,
                                "unauthorized".to_string(),
                            ),
                        )
                        .await;
                        break;
                    }
                    Some(t) => match self.resolve_current_bearer(t) {
                        Ok(p) => {
                            let is_operator = matches!(p.class, auth::PrincipalClass::Operator);
                            session_credential = Some(auth::LivePrincipalCredential {
                                principal_id: p.id.clone(),
                                token_fingerprint: auth::token_fingerprint(t),
                            });
                            // Return normal initialize response
                            let resp = self.handle_request(request, &p).await;
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
                            // SEC-REQ-1: Collapse all unauthorized cases to a single
                            // opaque string; do not distinguish missing vs unknown token.
                            let resp = JsonRpcResponse::error(
                                request.id.clone(),
                                -32000,
                                "unauthorized".to_string(),
                            );
                            write_json_line(&stdout, &resp).await;
                            break;
                        }
                    },
                    None => {
                        // SEC-REQ-1: Opaque unauthorized response; do not reveal
                        // that the token field was absent vs present-but-invalid.
                        let resp = JsonRpcResponse::error(
                            request.id.clone(),
                            -32000,
                            "unauthorized".to_string(),
                        );
                        write_json_line(&stdout, &resp).await;
                        break;
                    }
                }
                continue;
            }

            // For all other methods, require session_principal
            let principal = match session_credential.as_ref() {
                Some(credential) => match self.resolve_current_credential(credential) {
                    Ok(principal) => principal,
                    Err(_) => {
                        let resp = JsonRpcResponse::error(
                            request.id.clone(),
                            -32000,
                            "unauthorized".to_string(),
                        );
                        write_json_line(&stdout, &resp).await;
                        break;
                    }
                },
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
                let _ = self.handle_request(request, &principal).await;
            } else {
                let response = self.handle_request(request, &principal).await;
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
        let started = std::time::Instant::now();
        // Box::pin keeps the large handle_request_inner future off the caller's state machine.
        // This prevents the complex MCP dispatch tree from inflating state machines of
        // test functions that call handle_request multiple times sequentially.
        let response = Box::pin(self.handle_request_inner(req, principal)).await;
        db::metrics::record_mcp_liveness_gate_duration(started.elapsed());
        response
    }

    async fn handle_request_inner(
        &self,
        req: JsonRpcRequest,
        principal: &auth::Principal,
    ) -> JsonRpcResponse {
        let id = req.id.clone();

        match req.method.as_str() {
            "initialize" => {
                // P081 Phase 4: evaluate BoundaryPolicy for mcp_initialize.
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(principal);
                    match policy.evaluate(
                        caller_class.as_str(),
                        "mcp_initialize",
                        Some("initialize"),
                    ) {
                        auth::boundary::PolicyDecision::Deny {
                            reason_code,
                            row_id,
                            ..
                        } => {
                            tracing::debug!(
                                caller_class = caller_class.as_str(),
                                reason_code = %reason_code,
                                row_id = ?row_id,
                                "BoundaryPolicy: mcp_initialize denied; returning auth failure"
                            );
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                "mcp_initialize",
                                "initialize",
                                &reason_code,
                                row_id.as_deref(),
                            )
                            .await
                            {
                                tracing::error!(
                                    error = %e,
                                    "boundary deny audit write failed; failing closed (mcp_initialize)"
                                );
                                return JsonRpcResponse::error(
                                    id,
                                    -32000,
                                    "unauthorized".to_string(),
                                );
                            }
                            return JsonRpcResponse::error(
                                id,
                                -32004,
                                format!("auth_failure: {reason_code}"),
                            );
                        }
                        auth::boundary::PolicyDecision::Shadow { matched_decision } => {
                            if let auth::boundary::PolicyDecision::Deny {
                                reason_code,
                                row_id,
                                ..
                            } = *matched_decision
                            {
                                tracing::debug!(
                                    caller_class = caller_class.as_str(),
                                    reason_code = %reason_code,
                                    row_id = ?row_id,
                                    "BoundaryPolicy shadow: matrix would deny mcp_initialize"
                                );
                            }
                        }
                        auth::boundary::PolicyDecision::Allow { .. }
                        | auth::boundary::PolicyDecision::LegacyPassthrough => {}
                    }
                }

                let boundary_policy_cap = self.boundary_policy.as_ref().map(|policy| {
                    serde_json::json!({
                        "matrix_id": "p081-boundary-matrix-v1",
                        "schema_version": 1,
                        "capability_schema_version": 1,
                        "mode": policy.mode().as_str(),
                        "denied_known_tool_code": -32004,
                        "field_casing": "snake_case"
                    })
                });

                let request_id = public_json_rpc_request_id(&id);
                self.handle_hot_read_json_rpc(
                    id,
                    "initialize",
                    async move {
                        let mut capabilities = serde_json::json!({ "tools": {} });
                        if let Some(cap) = boundary_policy_cap {
                            capabilities["boundary_policy"] = cap;
                        }
                        Ok(serde_json::json!({
                            "protocolVersion": "2024-11-05",
                            "capabilities": capabilities,
                            "serverInfo": {
                                "name": "chainworks-control-plane",
                                "version": "0.1.0"
                            }
                        }))
                    },
                    &request_id,
                )
                .await
            }

            "tools/list" => {
                // P081 Phase 3: evaluate BoundaryPolicy for mcp_tools_list.
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(principal);
                    let started = std::time::Instant::now();
                    let decision = policy.evaluate(
                        caller_class.as_str(),
                        "mcp_tools_list",
                        Some("tools/list"),
                    );
                    let elapsed = started.elapsed();
                    db::metrics::record_p081_boundary_decision_latency(
                        "mcp_tools_list",
                        caller_class.as_str(),
                        policy.mode().as_str(),
                        elapsed,
                    );
                    match decision {
                        auth::boundary::PolicyDecision::Deny {
                            reason_code,
                            row_id,
                            ..
                        } => {
                            tracing::debug!(
                                caller_class = caller_class.as_str(),
                                reason_code = %reason_code,
                                row_id = ?row_id,
                                "BoundaryPolicy: mcp_tools_list denied; returning empty list"
                            );
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                "mcp_tools_list",
                                "tools/list",
                                &reason_code,
                                row_id.as_deref(),
                            )
                            .await
                            {
                                tracing::error!(
                                    error = %e,
                                    "boundary deny audit write failed; failing closed (mcp_tools_list)"
                                );
                                return JsonRpcResponse::error(
                                    id,
                                    -32000,
                                    "unauthorized".to_string(),
                                );
                            }
                            return JsonRpcResponse::success(
                                id,
                                serde_json::json!({ "tools": [] }),
                            );
                        }
                        auth::boundary::PolicyDecision::Shadow { matched_decision } => {
                            if let auth::boundary::PolicyDecision::Deny {
                                reason_code,
                                row_id,
                                ..
                            } = *matched_decision
                            {
                                tracing::debug!(
                                    caller_class = caller_class.as_str(),
                                    reason_code = %reason_code,
                                    row_id = ?row_id,
                                    "BoundaryPolicy shadow: matrix would deny mcp_tools_list"
                                );
                                if principal.class == auth::PrincipalClass::Operator {
                                    db::metrics::record_p081_boundary_policy_enforcement_parity(
                                        "allow", "deny",
                                    );
                                    db::metrics::record_p081_boundary_shadow_disagreement(
                                        "mcp_tools_list",
                                        row_id.as_deref(),
                                        caller_class.as_str(),
                                        "tools/list",
                                        "allow",
                                        "deny",
                                        Some(reason_code.as_str()),
                                    );
                                }
                            }
                        }
                        auth::boundary::PolicyDecision::Allow { .. }
                        | auth::boundary::PolicyDecision::LegacyPassthrough => {}
                    }
                }

                let request_id = public_json_rpc_request_id(&id);
                self.handle_hot_read_json_rpc(
                    id,
                    "tools.list",
                    async {
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

                        Ok(serde_json::json!({ "tools": tools_json }))
                    },
                    &request_id,
                )
                .await
            }

            "tools/call" => {
                let params = req.params.unwrap_or(serde_json::Value::Null);
                let tool_name = match params["name"].as_str() {
                    Some(n) => n.to_string(),
                    None => {
                        return JsonRpcResponse::error(id, -32602, "Missing tool name".to_string());
                    }
                };
                let request_id = public_json_rpc_request_id(&id);

                let Some(tool_id) = tools::capability_id_for(&tool_name) else {
                    return JsonRpcResponse::error(
                        id,
                        -32601,
                        format!("Method not found: {tool_name}"),
                    )
                    .with_error_request_id(Some(&request_id));
                };
                let canonical_tool_name = tools::canonical_tool_name(&tool_name);

                if !tools::p064_operator_tool_enabled(canonical_tool_name) {
                    return JsonRpcResponse::error(
                        id,
                        -32601,
                        format!("Method not found: {tool_name}"),
                    )
                    .with_error_request_id(Some(&request_id));
                }

                // P081 Phase 3: evaluate BoundaryPolicy for mcp_tools_call.
                let mut policy_allowed_row_id: Option<String> = None;
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(principal);
                    let started = std::time::Instant::now();
                    let decision = policy.evaluate(
                        caller_class.as_str(),
                        "mcp_tools_call",
                        Some(canonical_tool_name),
                    );
                    let elapsed = started.elapsed();
                    db::metrics::record_p081_boundary_decision_latency(
                        "mcp_tools_call",
                        caller_class.as_str(),
                        policy.mode().as_str(),
                        elapsed,
                    );
                    match decision {
                        auth::boundary::PolicyDecision::Deny {
                            reason_code,
                            row_id,
                            ..
                        } => {
                            tracing::debug!(
                                caller_class = caller_class.as_str(),
                                transport = "mcp_tools_call",
                                tool = %tool_name,
                                reason_code = %reason_code,
                                row_id = ?row_id,
                                "BoundaryPolicy: mcp_tools_call denied; returning auth failure"
                            );
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                "mcp_tools_call",
                                canonical_tool_name,
                                &reason_code,
                                row_id.as_deref(),
                            )
                            .await
                            {
                                tracing::error!(
                                    error = %e,
                                    "boundary deny audit write failed; failing closed (mcp_tools_call)"
                                );
                                return JsonRpcResponse::error(
                                    id,
                                    -32000,
                                    "unauthorized".to_string(),
                                )
                                .with_error_request_id(Some(&request_id));
                            }
                            return JsonRpcResponse::error(
                                id,
                                -32004,
                                format!("auth_failure: {reason_code}"),
                            )
                            .with_error_request_id(Some(&request_id));
                        }
                        auth::boundary::PolicyDecision::Shadow { matched_decision } => {
                            if let auth::boundary::PolicyDecision::Deny {
                                reason_code,
                                row_id,
                                ..
                            } = *matched_decision
                            {
                                tracing::debug!(
                                    caller_class = caller_class.as_str(),
                                    transport = "mcp_tools_call",
                                    tool = %tool_name,
                                    reason_code = %reason_code,
                                    row_id = ?row_id,
                                    "BoundaryPolicy shadow: matrix would deny this mcp_tools_call"
                                );
                                if principal.class == auth::PrincipalClass::Operator {
                                    db::metrics::record_p081_boundary_policy_enforcement_parity(
                                        "allow", "deny",
                                    );
                                    db::metrics::record_p081_boundary_shadow_disagreement(
                                        "mcp_tools_call",
                                        row_id.as_deref(),
                                        caller_class.as_str(),
                                        canonical_tool_name,
                                        "allow",
                                        "deny",
                                        Some(reason_code.as_str()),
                                    );
                                }
                            }
                        }
                        auth::boundary::PolicyDecision::Allow { row_id } => {
                            policy_allowed_row_id = row_id;
                        }
                        auth::boundary::PolicyDecision::LegacyPassthrough => {}
                    }
                }
                if !principal.tool_capabilities.contains(&tool_id) {
                    // Known tool, allowed by boundary policy, but caller lacks token capability.
                    // P081 AC25: durable deny audit required; fail closed if write fails.
                    let caller_class = auth::derive_caller_class_for_mcp(&principal);
                    if let Err(e) = write_mcp_deny_audit(
                        &self.pool,
                        self.boundary_policy.as_deref(),
                        principal,
                        "mcp_tools_call",
                        canonical_tool_name,
                        "CAPABILITY_OUT_OF_SCOPE",
                        None,
                    )
                    .await
                    {
                        tracing::warn!(error = %e, tool = %canonical_tool_name, "capability-denial audit write failed; returning fail-closed denial");
                        return JsonRpcResponse::policy_denial(
                            id,
                            "E_AUDIT_UNAVAILABLE",
                            caller_class.as_str(),
                            None,
                            "p081-boundary-matrix-v1",
                        );
                    }
                    if canonical_tool_name.starts_with("storage.") {
                        let result = tools::storage::typed_error(
                            canonical_tool_name,
                            tools::storage::ERR_UNAUTHORIZED,
                            "caller lacks storage diagnostics capability",
                            None,
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
                    return JsonRpcResponse::policy_denial(
                        id,
                        "CAPABILITY_OUT_OF_SCOPE",
                        caller_class.as_str(),
                        None,
                        "p081-boundary-matrix-v1",
                    );
                }

                let tool_params = params["arguments"].clone();

                if is_state_changing_call(canonical_tool_name, &tool_params) {
                    match db::repos::audit_log::audit_budget_requires_safe_mode(&self.pool).await {
                        Ok(true) => {
                            let caller_class = auth::derive_caller_class_for_mcp(&principal);
                            db::metrics::record_p081_audit_log_rate_limited(
                                "mcp_tools_call",
                                "AUDIT_BUDGET_EXHAUSTED",
                            );
                            return JsonRpcResponse::policy_denial(
                                id,
                                "AUDIT_BUDGET_EXHAUSTED",
                                caller_class.as_str(),
                                Some("p081.audit_budget.safe_mode"),
                                "p081-boundary-matrix-v1",
                            );
                        }
                        Err(e) => {
                            db::metrics::record_p081_boundary_policy_evaluation_error(
                                "mcp_tools_call",
                                "audit_budget_health_unavailable",
                            );
                            tracing::error!(
                                error = %e,
                                "P081 audit-budget health unavailable; denying state-changing MCP call"
                            );
                            return JsonRpcResponse::error(
                                id,
                                -32000,
                                "audit budget health unavailable".to_string(),
                            );
                        }
                        Ok(false) => {}
                    }
                }

                // P081 AC-13: MCP command idempotency enforcement.
                // The transport validates and looks up the key before dispatch; the pending
                // sentinel is inserted inside the command transaction so the idempotency claim,
                // command_journal row, and durable domain writes share one write unit.
                // Logic extracted to module-level helpers to keep this async fn's stack frame small.
                let (idempotency_claimed_key, idempotency_claimed_hash) =
                    if is_state_changing_call(canonical_tool_name, &tool_params) {
                        match mcp_idempotency_precheck(
                            &self.pool,
                            id.clone(),
                            canonical_tool_name,
                            &tool_params,
                            &principal,
                            policy_allowed_row_id.as_deref(),
                        )
                        .await
                        {
                            IdempotencyOutcome::Proceed { key, hash } => (Some(key), Some(hash)),
                            IdempotencyOutcome::Cached(resp) => return resp,
                            IdempotencyOutcome::Denied(resp) => return resp,
                        }
                    } else if is_read_only_call(canonical_tool_name, &tool_params) {
                        // Reject any idempotency key (snake_case or camelCase) on read-only tools.
                        if extract_idempotency_key(&tool_params).is_some() {
                            return JsonRpcResponse::error_with_data(
                                id,
                                -32602,
                                "idempotency_key not accepted for read-only tools",
                                serde_json::json!({
                                    "code": "IDEMPOTENCY_KEY_NOT_ACCEPTED",
                                    "tool_name": canonical_tool_name,
                                }),
                            );
                        }
                        (None, None)
                    } else {
                        (None, None)
                    };

                // P081 Phase 3: scope idempotency key and boundary row_id as task-locals so
                // mcp_caller() inside dispatch_tool stamps them into CallerContext, which
                // CommandJournalEntry::new() then persists in command_journal.
                let dispatch_result = crate::request_context::scope_idempotency_key(
                    idempotency_claimed_key.clone(),
                    crate::request_context::scope_idempotency_request_hash(
                        idempotency_claimed_hash.clone(),
                        crate::request_context::scope_boundary_row_id(
                            policy_allowed_row_id.clone(),
                            self.dispatch_tool(
                                canonical_tool_name,
                                tool_params,
                                principal,
                                &request_id,
                            ),
                        ),
                    ),
                )
                .await;
                match dispatch_result {
                    Ok(result) => {
                        // Update the pending idempotency claim with the committed result.
                        // Extract journal_id from the result JSON to link the idempotency record
                        // back to the command_journal row per P081 mcp_idempotency_contract.
                        if let (Some(ref key), Some(_)) =
                            (&idempotency_claimed_key, &idempotency_claimed_hash)
                        {
                            let result_json = serde_json::to_string(&result).unwrap_or_default();
                            let journal_id = result
                                .get("journal_id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            if let Some(err_resp) = mcp_idempotency_commit(
                                &self.pool,
                                id.clone(),
                                canonical_tool_name,
                                key,
                                &result_json,
                                journal_id.as_deref(),
                            )
                            .await
                            {
                                return err_resp;
                            }
                        }
                        JsonRpcResponse::success(
                            id,
                            serde_json::json!({
                                "content": [{
                                    "type": "text",
                                    "text": serde_json::to_string(&result).unwrap_or_default()
                                }]
                            }),
                        )
                    }
                    Err(e) => {
                        // SEC-P081-002: Log full error chain server-side; expose only INTERNAL
                        // plus the ambient request_id for correlation.
                        let rid = crate::request_context::current_request_id();
                        let error_text = e.to_string();
                        if error_text.starts_with("IDEMPOTENCY_IN_FLIGHT") {
                            return JsonRpcResponse::error_with_data(
                                id,
                                -32603,
                                "idempotency key has an in-flight request; retry after completion",
                                serde_json::json!({
                                    "code": "IDEMPOTENCY_IN_FLIGHT",
                                    "tool_name": canonical_tool_name,
                                    "request_id": rid,
                                }),
                            );
                        }
                        tracing::error!(
                            error = %e,
                            request_id = ?rid,
                            tool = %canonical_tool_name,
                            "mcp_tools_call: internal dispatch error"
                        );
                        // The pending idempotency claim is owned by the command transaction.
                        // On rollback it disappears with the command writes; on a committed
                        // failure it remains as committed-unack evidence and must not be
                        // deleted by a racing transport retry.
                        let data = serde_json::json!({
                            "code": "INTERNAL",
                            "request_id": rid,
                        });
                        JsonRpcResponse::error_with_data(id, -32603, "INTERNAL", data)
                    }
                }
            }

            "resources/list" => {
                // Expose domain resource URI templates.
                // Primary scheme matches the proposal contract:
                //   run://{id}  idea://{id}  artifact://{id}  report://{run_id}
                // The chainworks:// family is also kept for backward compatibility.
                // SEC-HIGH-001: evaluate BoundaryPolicy for resources/list as a first-class action.
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(principal);
                    let decision = policy.evaluate(
                        caller_class.as_str(),
                        "mcp_tools_list",
                        Some("resources.list"),
                    );
                    match decision {
                        auth::boundary::PolicyDecision::Deny {
                            reason_code,
                            row_id,
                            ..
                        } => {
                            tracing::debug!(
                                caller_class = caller_class.as_str(),
                                reason_code = %reason_code,
                                row_id = ?row_id,
                                "BoundaryPolicy: resources/list denied"
                            );
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                "mcp_tools_list",
                                "resources.list",
                                &reason_code,
                                row_id.as_deref(),
                            )
                            .await
                            {
                                tracing::error!(error = %e, "resources/list denial audit write failed; failing closed");
                                return JsonRpcResponse::error(
                                    id,
                                    -32603,
                                    "audit_store_failure".to_string(),
                                );
                            }
                            return JsonRpcResponse::success(
                                id,
                                serde_json::json!({ "resources": [] }),
                            );
                        }
                        auth::boundary::PolicyDecision::Shadow { matched_decision } => {
                            if let auth::boundary::PolicyDecision::Deny {
                                reason_code,
                                row_id,
                                ..
                            } = *matched_decision
                            {
                                tracing::debug!(
                                    caller_class = caller_class.as_str(),
                                    reason_code = %reason_code,
                                    row_id = ?row_id,
                                    "BoundaryPolicy shadow: matrix would deny resources/list"
                                );
                            }
                        }
                        auth::boundary::PolicyDecision::Allow { .. }
                        | auth::boundary::PolicyDecision::LegacyPassthrough => {}
                    }
                }
                let filtered: Vec<_> =
                    auth::filter_resources(principal, &auth::all_resource_templates())
                        .into_iter()
                        .map(resource_template_value)
                        .collect();

                JsonRpcResponse::success(id, serde_json::json!({ "resources": filtered }))
            }

            "resources/templates/list" => {
                // No parameterized resource templates yet.
                // SEC-HIGH-001: evaluate BoundaryPolicy for resources/templates/list.
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(principal);
                    let decision = policy.evaluate(
                        caller_class.as_str(),
                        "mcp_tools_list",
                        Some("resources.templates.list"),
                    );
                    match decision {
                        auth::boundary::PolicyDecision::Deny {
                            reason_code,
                            row_id,
                            ..
                        } => {
                            tracing::debug!(
                                caller_class = caller_class.as_str(),
                                reason_code = %reason_code,
                                row_id = ?row_id,
                                "BoundaryPolicy: resources/templates/list denied"
                            );
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                "mcp_tools_list",
                                "resources.templates.list",
                                &reason_code,
                                row_id.as_deref(),
                            )
                            .await
                            {
                                tracing::error!(error = %e, "resources/templates/list denial audit write failed; failing closed");
                                return JsonRpcResponse::error(
                                    id,
                                    -32603,
                                    "audit_store_failure".to_string(),
                                );
                            }
                            return JsonRpcResponse::success(
                                id,
                                serde_json::json!({ "resourceTemplates": [] }),
                            );
                        }
                        auth::boundary::PolicyDecision::Shadow { .. } => {}
                        auth::boundary::PolicyDecision::Allow { .. }
                        | auth::boundary::PolicyDecision::LegacyPassthrough => {}
                    }
                }
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
                // SEC-HIGH-001: evaluate BoundaryPolicy for resources/read as a first-class action.
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(principal);
                    let decision = policy.evaluate(
                        caller_class.as_str(),
                        "mcp_tools_call",
                        Some("resources.read"),
                    );
                    match decision {
                        auth::boundary::PolicyDecision::Deny {
                            reason_code,
                            row_id,
                            ..
                        } => {
                            tracing::debug!(
                                caller_class = caller_class.as_str(),
                                reason_code = %reason_code,
                                row_id = ?row_id,
                                "BoundaryPolicy: resources/read denied"
                            );
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                "mcp_tools_call",
                                "resources.read",
                                &reason_code,
                                row_id.as_deref(),
                            )
                            .await
                            {
                                tracing::error!(error = %e, "resources/read denial audit write failed; failing closed");
                                return JsonRpcResponse::error(
                                    id,
                                    -32603,
                                    "audit_store_failure".to_string(),
                                );
                            }
                            return JsonRpcResponse::error(
                                id,
                                -32004,
                                format!("auth_failure: {reason_code}"),
                            );
                        }
                        auth::boundary::PolicyDecision::Shadow { matched_decision } => {
                            if let auth::boundary::PolicyDecision::Deny {
                                reason_code,
                                row_id,
                                ..
                            } = *matched_decision
                            {
                                tracing::debug!(
                                    caller_class = caller_class.as_str(),
                                    reason_code = %reason_code,
                                    row_id = ?row_id,
                                    "BoundaryPolicy shadow: matrix would deny resources/read"
                                );
                            }
                        }
                        auth::boundary::PolicyDecision::Allow { .. }
                        | auth::boundary::PolicyDecision::LegacyPassthrough => {}
                    }
                }
                let request_id = public_json_rpc_request_id(&id);
                if matches!(
                    principal.class,
                    auth::PrincipalClass::Agent | auth::PrincipalClass::Observer
                ) && (uri.starts_with("artifact://") || uri.starts_with("report://"))
                {
                    return JsonRpcResponse::error(id, -32002, "Resource not found".to_string());
                }
                if auth::match_resource_uri(principal, &uri, resource_template_id_for_uri).is_none()
                {
                    return JsonRpcResponse::error(id, -32002, "Resource not found".to_string());
                }
                self.handle_hot_read_json_rpc(
                    id,
                    "artifacts.metadata.get",
                    async {
                        let data = self.read_resource_for_principal(&uri, principal).await?;
                        Ok(serde_json::json!({
                            "contents": [{
                                "uri": uri,
                                "mimeType": "application/json",
                                "text": data.to_string()
                            }]
                        }))
                    },
                    &request_id,
                )
                .await
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

    async fn handle_hot_read_json_rpc<F>(
        &self,
        id: Option<serde_json::Value>,
        surface: &str,
        read: F,
        request_id: &str,
    ) -> JsonRpcResponse
    where
        F: Future<Output = anyhow::Result<serde_json::Value>>,
    {
        let guard = hot_read_guard::HotReadGuard::new(self.pool.clone(), surface);
        let check = match guard.check(Some(request_id)).await {
            Ok(check) => check,
            Err(error) => {
                return JsonRpcResponse::error(id, -32603, error.to_string())
                    .with_error_request_id(Some(request_id))
            }
        };
        let (is_probe, _probe_guard) = match check {
            hot_read_guard::CheckResult::Allowed {
                is_probe,
                probe_guard,
            } => (is_probe, probe_guard),
            hot_read_guard::CheckResult::Denied(err) => return JsonRpcResponse::success(id, err),
        };

        let timeout_ms = if is_probe { 500 } else { 10_000 };
        let start = std::time::Instant::now();

        // P087: Create a cancellation token so timeout-dropped futures release SQLite/metadata/lane resources.
        let cancel = tokio_util::sync::CancellationToken::new();
        let _cancel_guard = cancel.clone().drop_guard();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            db::writer::CANCELLATION_TOKEN.scope(cancel, read),
        )
        .await;
        let duration = start.elapsed();
        db::metrics::record_hot_read_latency(surface, duration);
        // initialize/tools.list are MCP handshake probes; runtime.health is the explicit liveness surface.
        if surface == "runtime.health" || surface == "tools.list" || surface == "initialize" {
            db::metrics::record_mcp_liveness_gate_duration(duration);
        }

        match result {
            Ok(Ok(value)) => {
                let _ = guard.record_success().await;
                JsonRpcResponse::success(id, value)
            }
            Ok(Err(error)) => JsonRpcResponse::error(id, -32603, error.to_string())
                .with_error_request_id(Some(request_id)),
            Err(_) => {
                let _ = guard.record_violation("timeout").await;
                JsonRpcResponse::success(
                    id,
                    tools::storage::typed_error(
                        surface,
                        tools::storage::ERR_TIMEOUT,
                        format!("hot read timeout ({timeout_ms}ms)"),
                        Some(request_id),
                    ),
                )
            }
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
        if matches!(
            principal.class,
            auth::PrincipalClass::Agent | auth::PrincipalClass::Observer
        ) && (uri.starts_with("artifact://") || uri.starts_with("report://"))
        {
            // matrix_row: p081.agent_operator.resources_read.report
            anyhow::bail!("Resource not found");
        }

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
                    "artifact_metadata_pointer": {
                        "schemaVersion": "artifact_metadata_pointer.v1",
                        "artifactId": art.id.to_string(),
                        "checksumSha256": art.checksum_sha256,
                        "sizeBytes": art.size_bytes,
                        "authorizedPayloadRoute": format!("/artifacts/{}/payload", art.id),
                        "payloadPathRedacted": true,
                        "forbiddenFields": ["absolutePath", "filesystemPath", "rawPayload"]
                    },
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
            let public_artifact_index: Vec<_> = artifact_rows
                .iter()
                .map(tools::reports::public_artifact_index_row)
                .collect();
            let run_artifacts =
                db::repos::artifacts::list_by_run(&self.pool, run_id_parsed).await?;
            let is_operator = principal.class == auth::PrincipalClass::Operator;
            let (mcp_rollout_readback, run_report_rollout_readback) = if is_operator {
                rollout_contract_readback_lanes_json(&self.pool, run_id_parsed).await?
            } else {
                (serde_json::Value::Null, serde_json::Value::Null)
            };
            // Fetch p082 readbacks once and emit "report_resource" lane metrics.
            // Pass the pre-fetched value into artifact_report_json so per-artifact
            // lanes are not re-emitted for each artifact in the loop.
            let p082_recovery_matrix_readbacks =
                tools::reports::p082_recovery_matrix_readbacks_json(
                    &self.pool,
                    run_id_parsed,
                    &principal.class,
                    "report_resource",
                )
                .await?;
            let mut artifact_payloads = Vec::with_capacity(run_artifacts.len());
            for artifact in &run_artifacts {
                artifact_payloads.push(
                    tools::reports::artifact_report_json(
                        &self.pool,
                        artifact,
                        Some(&run_report_rollout_readback),
                        &principal.class,
                        &run_proj.artifact_root,
                        Some(&p082_recovery_matrix_readbacks),
                    )
                    .await?,
                );
            }
            let closeout_readiness_summary =
                tools::reports::closeout_readiness_summary_json(&self.pool, run_id_parsed).await?;
            let mut response = serde_json::Map::new();
            response.insert("run_id".into(), serde_json::json!(run_id));
            response.insert("status".into(), serde_json::json!(run_proj.status));
            response.insert(
                "total_stages".into(),
                serde_json::json!(run_proj.total_stages),
            );
            response.insert(
                "completed_stages".into(),
                serde_json::json!(run_proj.completed_stages),
            );
            response.insert(
                "failed_stages".into(),
                serde_json::json!(run_proj.failed_stages),
            );
            response.insert(
                "has_artifacts".into(),
                serde_json::json!(!artifact_rows.is_empty()),
            );
            response.insert("stages".into(), serde_json::to_value(stage_rows)?);
            response.insert(
                "agent_executions".into(),
                tools::reports::execution_mcp_truth_json(&self.pool, run_id_parsed, is_operator)
                    .await?,
            );
            response.insert(
                "p082_recovery_matrix_readbacks".into(),
                p082_recovery_matrix_readbacks,
            );
            response.insert(
                "implementation_self_assessment_summary".into(),
                tools::reports::implementation_self_assessment_summary_json(
                    &self.pool,
                    run_id_parsed,
                )
                .await?,
            );
            response.insert(
                "implementation_closeout_readiness_summary".into(),
                closeout_readiness_summary.clone(),
            );
            response.insert(
                "closeout_readiness_summary".into(),
                closeout_readiness_summary,
            );
            response.insert(
                "artifact_index".into(),
                serde_json::to_value(public_artifact_index)?,
            );
            response.insert(
                "artifacts".into(),
                serde_json::Value::Array(artifact_payloads),
            );
            if is_operator {
                response.insert(
                    "code_writer_completion_receipts".into(),
                    tools::reports::code_writer_completion_receipts_json(&self.pool, run_id_parsed)
                        .await?,
                );
                response.insert(
                    "implementationCompletion".into(),
                    tools::reports::implementation_completion_json(&self.pool, run_id_parsed)
                        .await?,
                );
                response.insert(
                    "workflow_conflict".into(),
                    tools::reports::workflow_conflict_json(
                        &self.pool,
                        &self.cmd_handler,
                        run_id_parsed,
                    )
                    .await?,
                );
                response.insert(
                    "retryAuthority".into(),
                    tools::reports::retry_authority_current_json(&self.pool, run_id_parsed).await?,
                );
                response.insert(
                    "retryAuthorityHistory".into(),
                    tools::reports::retry_authority_history_json(&self.pool, run_id_parsed).await?,
                );
                response.insert(
                    "p091OrphanRepairReadback".into(),
                    tools::reports::p091_orphan_repair_readback_json(&self.pool, run_id_parsed)
                        .await?,
                );
                response.insert(
                    "p094_boundary_readback".into(),
                    db::repos::artifact_contracts::p094_readback_json(&self.pool, run_id_parsed)
                        .await?,
                );
                response.insert(
                    "p094_rollout_decision".into(),
                    tools::reports::p094_rollout_decision_json(&self.pool, run_id_parsed).await?,
                );
                response.insert("rollout_contract_readback".into(), mcp_rollout_readback);
            }

            return Ok(serde_json::Value::Object(response));
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
            if principal.class != auth::PrincipalClass::Operator {
                let sanitized: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|row| {
                        let mut v = serde_json::to_value(row).unwrap_or(serde_json::Value::Null);
                        tools::runs::redact_non_operator_run_projection(&mut v);
                        v
                    })
                    .collect();
                return Ok(serde_json::to_value(sanitized)?);
            }
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
                if principal.class != auth::PrincipalClass::Operator {
                    let sanitized: Vec<serde_json::Value> = rows
                        .iter()
                        .map(|row| {
                            let mut v =
                                serde_json::to_value(row).unwrap_or(serde_json::Value::Null);
                            if let Some(obj) = v.as_object_mut() {
                                obj.remove("file_path");
                                obj.remove("source_agent_execution_id");
                                obj.remove("source_stage_execution_id");
                                obj.remove("source_session_generation_id");
                                obj.remove("source_work_item_id");
                            }
                            v
                        })
                        .collect();
                    return Ok(serde_json::to_value(sanitized)?);
                }
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
        principal: &auth::Principal,
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
            // SEC-HIGH-002: strip sensitive fields for non-Operator principals.
            if principal.class != auth::PrincipalClass::Operator {
                tools::runs::redact_run_for_non_operator(obj);
            }
            // active_artifact_index and run_state_projection include operator-only
            // recovery diagnostics, local paths, and source IDs — Operator only.
            if principal.class == auth::PrincipalClass::Operator {
                if let Some(projection) = db::repos::artifact_contracts::find_run_state_projection(
                    &self.pool,
                    run_id_parsed,
                )
                .await?
                {
                    obj.insert("active_artifact_index".into(), projection.active_index_json);
                    obj.insert("run_state_projection".into(), projection.run_state_json);
                    obj.insert(
                        "operator_overrides".into(),
                        serde_json::to_value(
                            db::repos::artifact_contracts::list_overrides(
                                &self.pool,
                                run_id_parsed,
                            )
                            .await?,
                        )?,
                    );
                }
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
            // Detail run resources are readable by Agent/Observer, but operator
            // diagnostics and execution internals are not. Keep the same base
            // run/projection redaction above and attach rich recovery/readback
            // lanes only for Operators.
            if principal.class == auth::PrincipalClass::Operator {
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
                        stage_obj
                            .insert("agent_executions".into(), serde_json::to_value(executions)?);
                    }
                    stage_values.push(stage_value);
                }
                obj.insert("stages".into(), serde_json::Value::Array(stage_values));
            }
        }
        Ok(value)
    }

    async fn dispatch_tool(
        &self,
        tool_name: &str,
        params: serde_json::Value,
        principal: &auth::Principal,
        request_id: &str,
    ) -> Result<serde_json::Value> {
        let span = tracing::info_span!("dispatch_tool", tool_name, request_id);
        self.dispatch_tool_with_span(tool_name, params, principal, span, request_id)
            .await
    }

    async fn dispatch_tool_with_span(
        &self,
        tool_name: &str,
        params: serde_json::Value,
        principal: &auth::Principal,
        span: tracing::Span,
        request_id: &str,
    ) -> Result<serde_json::Value> {
        use tracing::Instrument;
        self.dispatch_tool_instrumented(tool_name, params, principal, request_id)
            .instrument(span)
            .await
    }

    async fn dispatch_tool_instrumented(
        &self,
        tool_name: &str,
        params: serde_json::Value,
        principal: &auth::Principal,
        request_id: &str,
    ) -> Result<serde_json::Value> {
        let pool = &self.pool;

        if hot_read_guard::is_hot_read_tool(tool_name) {
            let guard = hot_read_guard::HotReadGuard::new(pool.clone(), tool_name);
            let check = guard.check(Some(request_id)).await?;
            let (is_probe, _probe_guard) = match check {
                hot_read_guard::CheckResult::Allowed {
                    is_probe,
                    probe_guard,
                } => (is_probe, probe_guard),
                hot_read_guard::CheckResult::Denied(err) => return Ok(err),
            };

            // P087: Enforce probe budget capped at 500 ms, 10s cancellation for normal reads.
            let timeout_ms = if is_probe { 500 } else { 10000 };
            let start = std::time::Instant::now();

            // P087: Create a cancellation token and link it to a drop guard.
            // When tokio::time::timeout expires, the future is dropped, triggerring cancellation.
            let cancel = tokio_util::sync::CancellationToken::new();
            let _cancel_guard = cancel.clone().drop_guard();

            let result = tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                db::writer::CANCELLATION_TOKEN.scope(
                    cancel.clone(),
                    self.dispatch_tool_internal(tool_name, params, principal, cancel, request_id),
                ),
            )
            .await;
            let duration = start.elapsed();
            db::metrics::record_hot_read_latency(tool_name, duration);
            if tool_name == "runtime.health" {
                db::metrics::record_mcp_liveness_gate_duration(duration);
            }

            let mut result = match result {
                Ok(res) => res,
                Err(_) => {
                    let _ = guard.record_violation("timeout").await;
                    return Ok(tools::storage::typed_error(
                        tool_name,
                        tools::storage::ERR_TIMEOUT,
                        format!("hot read timeout ({}ms)", timeout_ms),
                        Some(request_id),
                    ));
                }
            };

            match &result {
                Ok(val) if val["error"].as_bool() == Some(true) => {
                    // Domain error, check if it counts as violation
                    let code = val["errorCode"].as_str().unwrap_or("");
                    db::metrics::increment_counter_with_label(
                        "mcp_hot_read_error_total_by_code",
                        code,
                    );
                    if matches!(
                        code,
                        tools::storage::ERR_TIMEOUT
                            | tools::storage::ERR_BUSY
                            | tools::storage::ERR_UNAVAILABLE
                    ) {
                        let _ = guard.record_violation(code).await;
                    }
                }
                Ok(val) => {
                    let _ = guard.record_success().await;
                    // P087: Inject hotRead success metadata if missing
                    if val.is_object() && val["hotRead"].is_null() {
                        if let Ok(obj) = result.as_mut() {
                            if let Some(map) = obj.as_object_mut() {
                                let (status, _, _, _, _, _) =
                                    db::repos::hot_read_circuit::get_circuit_state(pool, tool_name)
                                        .await
                                        .unwrap_or((
                                            db::repos::hot_read_circuit::CircuitStatus::Closed,
                                            0,
                                            0,
                                            None,
                                            None,
                                            false,
                                        ));
                                map.insert(
                                    "hotRead".to_string(),
                                    serde_json::json!({
                                        "status": "healthy",
                                        "circuitState": status.as_str()
                                    }),
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("timeout")
                        || msg.contains("busy")
                        || msg.contains("unavailable")
                    {
                        let _ = guard.record_violation(&msg).await;
                    }
                }
            }
            result
        } else {
            let cancel = tokio_util::sync::CancellationToken::new();
            db::writer::CANCELLATION_TOKEN
                .scope(
                    cancel.clone(),
                    self.dispatch_tool_internal(tool_name, params, principal, cancel, request_id),
                )
                .await
        }
    }

    async fn dispatch_tool_internal(
        &self,
        tool_name: &str,
        params: serde_json::Value,
        principal: &auth::Principal,
        cancel: tokio_util::sync::CancellationToken,
        request_id: &str,
    ) -> Result<serde_json::Value> {
        let pool = &self.pool;
        let cmd = self.cmd_handler.as_ref();

        if tool_name.starts_with("ideas.") {
            tools::ideas::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("runs.") {
            tools::runs::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("approvals.") {
            tools::approvals::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("stages.")
            || tool_name == "legacy_discovery_override_create"
            || tool_name == "workflow_conflicts.resolve"
            || tool_name == "workflow_loop_budget.extend"
        {
            tools::stages::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("reports.") {
            tools::reports::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("artifacts.") {
            tools::artifacts::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name.starts_with("steward.") {
            tools::steward::execute(tool_name, params, pool, cmd, principal).await
        } else if tool_name == "runtime.health" {
            tools::runtime::execute(params, pool, self.boundary_policy.as_deref()).await
        } else if tool_name.starts_with("effects.") {
            tools::effects::execute(tool_name, params, pool, principal).await
        } else if tool_name.starts_with("runtime.") {
            tools::runtime::execute_with_name(
                tool_name,
                params,
                pool,
                self.boundary_policy.as_deref(),
            )
            .await
        } else if tool_name.starts_with("storage.") {
            tools::storage::execute_with_writer(
                tool_name,
                params,
                pool,
                self.storage_writer_heartbeat
                    .as_ref()
                    .map(|heartbeat| heartbeat.as_ref()),
                principal,
                cancel,
                Some(request_id),
            )
            .await
        } else if tool_name.starts_with("agents.") {
            tools::agents::execute(tool_name, params, pool, principal).await
        } else if tool_name.starts_with("automation.") {
            tools::automation::execute(tool_name, params).await
        } else if tool_name.starts_with("operator.") {
            tools::runtime::execute_with_name(
                tool_name,
                params,
                pool,
                self.boundary_policy.as_deref(),
            )
            .await
        } else {
            Err(anyhow::anyhow!("Unknown tool namespace: {tool_name}"))
        }
    }
}

// ── P081 AC-13: MCP command idempotency helpers ──────────────────────────────

/// Returns true if this tool is state-changing and requires an idempotency key.
/// P081 AC-13: all tools that perform durable DB or filesystem writes must be
/// listed here. storage.reconcile_evidence_orphans has a dry-run mode — see
/// `is_state_changing_call` which does param-aware classification for it.
fn is_state_changing_tool(tool_name: &str) -> bool {
    matches!(
        tools::canonical_tool_name(tool_name),
        "runs.start"
            | "runs.cancel"
            | "runs.main_sync.request"
            | "runs.main_sync.retry"
            | "runs.main_sync.set_override"
            | "runs.main_sync.repair_state"
            | "runs.main_sync.record_recovery_decision"
            | "runs.knowledge_capsule.ignore"
            | "runs.retrofit_catalog_snapshot"
            | "runs.settle_proposal_gate"
            | "ideas.create"
            | "stages.retry"
            | "stages.consume_provider_quota_hold"
            | "legacy_discovery_override_create"
            | "workflow_conflicts.resolve"
            | "workflow_loop_budget.extend"
            | "artifacts.override_contract"
            | "steward.run_analysis"
            | "approvals.resolve"
            | "effects.mark_conflict"
            | "effects.mark_unrecoverable"
            | "effects.clear_after_manual_verification"
            | "storage.maintenance.repair_slot"
            | "storage.projections.clear_backlog"
            | "storage.projections.clear_poison"
    )
}

/// Returns true if this call is state-changing given the supplied parameters.
/// Wraps `is_state_changing_tool` and adds param-aware classification for tools
/// that have both a read-only (dry-run) and a mutating (live) execution mode.
fn is_state_changing_call(tool_name: &str, params: &serde_json::Value) -> bool {
    if is_state_changing_tool(tool_name) {
        return true;
    }
    // storage.reconcile_evidence_orphans: dryRun=false is state-changing.
    // dryRun defaults to true when absent per the tool's own contract.
    if tools::canonical_tool_name(tool_name) == "storage.reconcile_evidence_orphans" {
        let dry_run = params
            .get("dryRun")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        return !dry_run;
    }
    false
}

/// Returns true if this tool is unconditionally read-only and must reject any idempotency key.
fn is_read_only_tool(tool_name: &str) -> bool {
    matches!(
        tools::canonical_tool_name(tool_name),
        "runs.list"
            | "runs.get"
            | "ideas.list"
            | "approvals.list"
            | "reports.get"
            | "steward.list_analyses"
            | "steward.get_analysis"
            | "runtime.health"
            | "boundary.runtime.get"
            | "storage.health"
            | "storage.write_pressure"
            | "storage.evidence_spool_summary"
            | "effects.list"
            | "effects.inspect"
            | "effects.reconcile"
    )
}

/// Returns true if this call is read-only given the supplied parameters.
/// Extends `is_read_only_tool` with param-aware classification for tools that
/// operate in both read-only (dry-run) and mutating (live) modes.
fn is_read_only_call(tool_name: &str, params: &serde_json::Value) -> bool {
    if is_read_only_tool(tool_name) {
        return true;
    }
    // storage.reconcile_evidence_orphans: dryRun=true (the default) is read-only.
    if tools::canonical_tool_name(tool_name) == "storage.reconcile_evidence_orphans" {
        let dry_run = params
            .get("dryRun")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        return dry_run;
    }
    false
}

/// Extract idempotency key from tool arguments, accepting both snake_case
/// (`idempotency_key`) and camelCase (`idempotencyKey`) field names.
fn extract_idempotency_key(tool_params: &serde_json::Value) -> Option<String> {
    tool_params["idempotency_key"]
        .as_str()
        .or_else(|| tool_params["idempotencyKey"].as_str())
        .map(|s| s.to_string())
}

/// Returns true if the string is a valid UUID (any version).
/// P081 SEC: idempotency keys must be UUIDs to prevent unbounded strings from
/// Validate that the idempotency key is a valid UUIDv7 per P081 mcp_idempotency_contract.
/// UUIDv7 is required (not just any UUID) for replay handle format consistency.
fn is_valid_uuid_key(s: &str) -> bool {
    if s.len() > 36 {
        return false;
    }
    match uuid::Uuid::parse_str(s) {
        Ok(u) => u.get_version() == Some(uuid::Version::SortRand),
        Err(_) => false,
    }
}

/// Derives a non-sensitive token_id for canonical request hashing.
/// Uses the ambient task-local token_id if present; otherwise returns empty
/// string for backward compatibility.
fn derive_token_id_for_idempotency(principal: &auth::Principal) -> String {
    // token_id in the canonical hash is diagnostic-only per P081 §security_hardening_contract.
    // Use the task-local derived token_id to avoid including the raw bearer token.
    let _ = principal; // principal_id already included separately in the hash
    crate::request_context::current_token_id().unwrap_or_default()
}

fn canonicalize_json_for_hash(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut canonical = serde_json::Map::new();
            for key in keys {
                if let Some(child) = map.get(key) {
                    canonical.insert(key.clone(), canonicalize_json_for_hash(child));
                }
            }
            serde_json::Value::Object(canonical)
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(canonicalize_json_for_hash)
                .collect::<Vec<_>>(),
        ),
        _ => value.clone(),
    }
}

/// Compute canonical_request_hash for idempotency deduplication.
/// The idempotency_key itself is excluded from the hash (it is retry metadata).
/// Also strips camelCase idempotencyKey if present.
/// Includes row_id per P081 mcp_idempotency_contract canonical_request_hash fields.
fn compute_canonical_request_hash(
    tool_name: &str,
    arguments: &serde_json::Value,
    caller_class: &str,
    principal_id: &str,
    token_id: &str,
    row_id: Option<&str>,
) -> String {
    use sha2::{Digest, Sha256};
    let mut args_sorted = arguments.clone();
    if let serde_json::Value::Object(ref mut map) = args_sorted {
        map.remove("idempotency_key");
        map.remove("idempotencyKey");
    }
    let args_canonical = canonicalize_json_for_hash(&args_sorted);
    let canonical = serde_json::json!({
        "tool_name": tool_name,
        "arguments": args_canonical,
        "caller_class": caller_class,
        "principal_id": principal_id,
        "token_id": token_id,
        "row_id": row_id,
    });
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    let digest = Sha256::digest(&bytes);
    format!("{digest:x}")
}

/// Outcome of the pre-dispatch idempotency precheck for state-changing MCP calls.
enum IdempotencyOutcome {
    /// Claim won; proceed to dispatch. Contains the key and hash for post-dispatch update.
    Proceed { key: String, hash: String },
    /// A committed duplicate was found; the cached response should be returned directly.
    Cached(JsonRpcResponse),
    /// The call must be denied (missing key, invalid key, conflict, storage error, etc.).
    Denied(JsonRpcResponse),
}

/// Committed-unack recovery: when a pending sentinel record has aged past the in-flight
/// timeout, query the command_journal to check whether the command actually committed.
/// If found, synthesize a recovery response and update the idempotency record so future
/// retries see a cached result. Extracted to keep async stack frames small.
async fn mcp_idempotency_committed_unack_recovery(
    pool: &sqlx::SqlitePool,
    id: Option<serde_json::Value>,
    canonical_tool_name: &str,
    idempotency_key: &str,
    rid: Option<String>,
) -> IdempotencyOutcome {
    match db::repos::command_journal::find_committed_by_idempotency_key(pool, idempotency_key).await
    {
        Ok(Some(journal_id)) => {
            let recovery_json = serde_json::json!({
                "_idempotency": "committed_unack_recovery",
                "journal_id": journal_id,
                "note": "command committed; original result unavailable; retry for fresh state",
            })
            .to_string();
            let _ = db::repos::mcp_command_idempotency::update_result(
                pool,
                idempotency_key,
                &recovery_json,
                None,
                Some(&journal_id),
            )
            .await;
            IdempotencyOutcome::Cached(JsonRpcResponse::success(
                id,
                serde_json::json!({
                    "content": [{ "type": "text", "text": recovery_json }],
                    "_idempotency": "committed_unack_recovery",
                }),
            ))
        }
        Ok(None) => IdempotencyOutcome::Denied(JsonRpcResponse::error_with_data(
            id,
            -32603,
            "prior request committed but result is unavailable; check system state",
            serde_json::json!({
                "code": "IDEMPOTENCY_COMMITTED_UNACK",
                "tool_name": canonical_tool_name,
                "request_id": rid,
            }),
        )),
        Err(e) => {
            tracing::warn!(
                error = %e,
                tool = %canonical_tool_name,
                "committed-unack journal lookup failed; returning COMMITTED_UNACK"
            );
            IdempotencyOutcome::Denied(JsonRpcResponse::error_with_data(
                id,
                -32603,
                "prior request committed but result is unavailable; check system state",
                serde_json::json!({
                    "code": "IDEMPOTENCY_COMMITTED_UNACK",
                    "tool_name": canonical_tool_name,
                    "request_id": rid,
                }),
            ))
        }
    }
}

/// Execute the P081 idempotency lookup precheck for state-changing MCP calls.
///
/// `boundary_row_id` is threaded from the BoundaryPolicy Allow decision so it is
/// included in the canonical_request_hash per P081 mcp_idempotency_contract. The
/// first-attempt pending sentinel is claimed later inside the command transaction.
///
/// Extracted into a module-level async fn to keep the async state machine of the
/// outer request handler small enough to avoid stack overflows.
async fn mcp_idempotency_precheck(
    pool: &sqlx::SqlitePool,
    id: Option<serde_json::Value>,
    canonical_tool_name: &str,
    tool_params: &serde_json::Value,
    principal: &auth::Principal,
    boundary_row_id: Option<&str>,
) -> IdempotencyOutcome {
    let key_opt = extract_idempotency_key(tool_params);
    let key_str = match key_opt {
        None => {
            return IdempotencyOutcome::Denied(JsonRpcResponse::error_with_data(
                id,
                -32602,
                "idempotency_key required for state-changing tools",
                serde_json::json!({
                    "code": "IDEMPOTENCY_KEY_REQUIRED",
                    "tool_name": canonical_tool_name,
                }),
            ));
        }
        Some(k) => k,
    };

    if !is_valid_uuid_key(&key_str) {
        return IdempotencyOutcome::Denied(JsonRpcResponse::error_with_data(
            id,
            -32602,
            "idempotency_key must be a valid UUID",
            serde_json::json!({
                "code": "IDEMPOTENCY_KEY_INVALID",
                "tool_name": canonical_tool_name,
            }),
        ));
    }

    let caller_class = auth::derive_caller_class_for_mcp(principal);
    let token_id = derive_token_id_for_idempotency(principal);
    let canonical_hash = compute_canonical_request_hash(
        canonical_tool_name,
        tool_params,
        caller_class.as_str(),
        &principal.id,
        &token_id,
        boundary_row_id,
    );

    match db::repos::mcp_command_idempotency::find_by_key(pool, &key_str).await {
        Ok(None) => IdempotencyOutcome::Proceed {
            key: key_str,
            hash: canonical_hash,
        },
        Ok(Some(record)) => {
            if record.result_json == db::repos::mcp_command_idempotency::PENDING_SENTINEL {
                let age_ms = chrono::Utc::now().timestamp_millis() - record.committed_at_ms;
                let rid = crate::request_context::current_request_id();
                if age_ms < 30_000 {
                    IdempotencyOutcome::Denied(JsonRpcResponse::error_with_data(
                        id,
                        -32603,
                        "idempotency key has an in-flight request; retry after completion",
                        serde_json::json!({
                            "code": "IDEMPOTENCY_IN_FLIGHT",
                            "tool_name": canonical_tool_name,
                            "request_id": rid,
                        }),
                    ))
                } else {
                    mcp_idempotency_committed_unack_recovery(
                        pool,
                        id,
                        canonical_tool_name,
                        &key_str,
                        rid,
                    )
                    .await
                }
            } else if record.canonical_request_hash == canonical_hash {
                db::metrics::increment_counter("mcp_command_idempotency_replay_total");
                let result: serde_json::Value =
                    serde_json::from_str(&record.result_json).unwrap_or(serde_json::Value::Null);
                IdempotencyOutcome::Cached(JsonRpcResponse::success(
                    id,
                    serde_json::json!({
                        "content": [{ "type": "text", "text": serde_json::to_string(&result).unwrap_or_default() }],
                        "_idempotency": "duplicate_ok"
                    }),
                ))
            } else {
                db::metrics::increment_counter("mcp_command_idempotency_conflict_total");
                IdempotencyOutcome::Denied(JsonRpcResponse::error_with_data(
                    id,
                    -32603,
                    "idempotency conflict",
                    serde_json::json!({
                        "code": "IDEMPOTENCY_CONFLICT",
                        "tool_name": canonical_tool_name,
                    }),
                ))
            }
        }
        Err(e) => {
            let rid = crate::request_context::current_request_id();
            tracing::error!(
                error = %e,
                request_id = ?rid,
                tool = %canonical_tool_name,
                "idempotency lookup failed; failing closed before command dispatch"
            );
            IdempotencyOutcome::Denied(JsonRpcResponse::error_with_data(
                id,
                -32603,
                "idempotency storage unavailable",
                serde_json::json!({
                    "code": "SQLITE_CONTENTION_RETRY_EXHAUSTED",
                    "request_id": rid,
                }),
            ))
        }
    }
}

/// Update a pending idempotency claim with the committed result after successful dispatch.
/// Returns an error response if the update fails (committed-unack), or None if the caller
/// should proceed to return the success response.
/// `journal_id` is extracted from the result JSON (the `journal_id` field that state-changing
/// tools return) and stored in `mcp_command_idempotency.command_journal_id` for audit linkage.
async fn mcp_idempotency_commit(
    pool: &sqlx::SqlitePool,
    id: Option<serde_json::Value>,
    canonical_tool_name: &str,
    key: &str,
    result_json: &str,
    journal_id: Option<&str>,
) -> Option<JsonRpcResponse> {
    let Some(journal_id) = journal_id else {
        let rid = crate::request_context::current_request_id();
        tracing::error!(
            request_id = ?rid,
            tool = %canonical_tool_name,
            "state-changing MCP command returned success without journal_id; refusing to mark idempotency committed"
        );
        return Some(JsonRpcResponse::error_with_data(
            id,
            -32603,
            "state-changing MCP command result did not include journal linkage",
            serde_json::json!({
                "code": "IDEMPOTENCY_JOURNAL_LINK_MISSING",
                "request_id": rid,
            }),
        ));
    };
    match db::repos::mcp_command_idempotency::update_result(
        pool,
        key,
        result_json,
        None,
        Some(journal_id),
    )
    .await
    {
        Ok(true) => None,
        Ok(false) => {
            // P081 security review H-003 fix: Ok(false) means no pending record was
            // found after a successful command dispatch. This is an invariant violation
            // (the pending record must exist because mcp_idempotency_precheck inserted
            // it). Treating it as success would leave no replay record, breaking the
            // committed-unack and retry contract. Fail closed.
            let rid = crate::request_context::current_request_id();
            tracing::error!(
                request_id = ?rid,
                tool = %canonical_tool_name,
                "idempotency update_result returned Ok(false): pending record absent after commit; \
                 failing closed to protect committed-unack contract"
            );
            Some(JsonRpcResponse::error_with_data(
                id,
                -32603,
                "idempotency replay record missing after commit; check system state before retry",
                serde_json::json!({
                    "code": "IDEMPOTENCY_COMMITTED_UNACK",
                    "request_id": rid,
                }),
            ))
        }
        Err(e) => {
            let rid = crate::request_context::current_request_id();
            tracing::error!(
                error = %e,
                request_id = ?rid,
                tool = %canonical_tool_name,
                "idempotency update_result failed after successful dispatch (committed-unack state)"
            );
            Some(JsonRpcResponse::error_with_data(
                id,
                -32603,
                "idempotency result could not be stored; command committed — check system state before retry",
                serde_json::json!({
                    "code": "IDEMPOTENCY_COMMITTED_UNACK",
                    "request_id": rid,
                }),
            ))
        }
    }
}

/// P081 AC25: Write exactly one durable audit_log row for a boundary denial at an
/// MCP transport seam. Uses the standalone bounded `append` path (opens its own
/// BEGIN IMMEDIATE transaction) so the denial is durably recorded before the
/// error response is sent. Callers must fail closed when this returns Err.
///
/// Bearer tokens are never written; the principal_id and class come from the
/// already-resolved Principal, not from any caller-supplied header.
// matrix_row: p081.agent_operator.mcp_tools_call.command
async fn write_mcp_deny_audit(
    pool: &SqlitePool,
    policy: Option<&auth::boundary::BoundaryPolicy>,
    principal: &auth::Principal,
    transport: &str,
    action_attempted: &str,
    reason_code: &str,
    row_id: Option<&str>,
) -> anyhow::Result<()> {
    let id = uuid::Uuid::now_v7().to_string();
    let now = chrono::Utc::now();
    let timestamp_ms = now.timestamp_millis();
    // Use the ambient request id when inside an HTTP scope; fall back to a
    // synthetic UUID for stdio paths where no HTTP request id is available.
    let request_id = crate::request_context::current_request_id()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    // SEC-P081-M002: read derived token_id from task-local; never the raw token.
    let token_id = crate::request_context::current_token_id();
    let mode = policy.map(|p| p.mode().as_str()).unwrap_or("legacy_compat");
    let principal_class_str = principal.class.to_string();
    let caller_class_str = auth::derive_caller_class_for_mcp(&principal);

    let payload_raw = serde_json::json!({
        "event": "boundary_decision",
        "decision": "deny",
        "transport": transport,
        "action_attempted": action_attempted,
        "reason_code": reason_code,
    })
    .to_string();
    let (payload, _original_sha256, truncated) = db::repos::audit_log::build_envelope(&payload_raw);

    let entry = db::repos::audit_log::AuditEntry {
        id: &id,
        request_id: &request_id,
        timestamp_ms,
        event_type: "boundary_decision",
        principal_id: Some(principal.id.as_str()),
        principal_class: Some(principal_class_str.as_str()),
        caller_class: Some(caller_class_str.as_str()),
        token_id: token_id.as_deref(),
        transport,
        action_attempted,
        decision: "deny",
        denial_reason_code: Some(reason_code),
        row_id,
        env_gate_state: None,
        source_ip_hash_or_local_process_id: None,
        boundary_policy_mode: mode,
        fixture_version: "p081-boundary-matrix-v1",
        payload: &payload,
        // SEC-P081-M003: pass the raw payload so the repo computes sha256 independently.
        original_payload_bytes: if truncated { Some(&payload_raw) } else { None },
        diagnostic_truncated: truncated,
        checkpoint_id: None,
        created_at_ms: timestamp_ms,
    };

    db::repos::audit_log::append(pool, &entry)
        .await
        .map_err(|e| {
            db::metrics::record_p081_audit_log_append_failure("boundary_decision", transport, mode);
            e
        })
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
            "workflow_loop_budget.extend",
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
        // Agents can create ideas, start runs, and read reports but cannot approve,
        // cancel, retry, or enter the steward surface.
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
        // Observer is read-only: sees list/get/report surfaces + approvals.list +
        // steward readers. Must not see any command tool.
        for expected in [
            "ideas.list",
            "runs.list",
            "runs.get",
            "reports.get",
            "approvals.list",
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

        // Agent CAN read run://, idea://.
        assert!(resources_read_allowed(&ag, "run://r-1"));
        assert!(resources_read_allowed(&ag, "idea://i-1"));
        assert!(!resources_read_allowed(&ag, "report://some-run-id"));

        // Unknown URI scheme also denied.
        assert!(!resources_read_allowed(&ag, "bogus://1"));

        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(!resources_read_allowed(&ob, "report://some-run-id"));

        // Operator can read steward-analysis and report://.
        let op = Principal::new("op", PrincipalClass::Operator);
        assert!(resources_read_allowed(&op, "steward-analysis://abc-123"));
        assert!(resources_read_allowed(&op, "report://some-run-id"));
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
        artifact_contracts, artifacts, ideas, projections, rollout_contract_checks, runs,
        startup_repairs, steward, validation,
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
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed");
        db::writer::register_shared_writer(
            &pool,
            std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
        )
        .await
        .expect("register shared writer");
        pool
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
                escalation_policy_id: None,
                escalation_policy_hash: None,
                escalation_tier_id: None,
                escalation_tier_kind_raw: None,
                escalation_trigger_raw: None,
                escalation_digest_version: None,
                escalation_ledger_id: None,
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
        let rendered = value.to_string();
        assert!(
            !rendered.contains("file_path"),
            "report:// public payload must not expose raw file_path fields"
        );
        assert!(
            !rendered.contains(payload_path.to_string_lossy().as_ref()),
            "report:// public payload must not expose filesystem paths"
        );
        assert_eq!(
            validation_failure["artifact_metadata_pointer"]["payloadPathRedacted"],
            serde_json::json!(true)
        );

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
        // Use a real temp dir as artifact_root so read_failed_stage_evidence_safe
        // passes the containment check (canonical_path must start_with artifact_root).
        let artifact_dir = tempfile::TempDir::new().unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_dir.path().to_string_lossy().to_string();
        runs::insert(&pool, &run).await.unwrap();
        let payload_path = artifact_dir
            .path()
            .join(format!("failed-stage-evidence-report-{run_id}.json"));
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
            db::writer::register_shared_writer(
                &pool,
                std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
            )
            .await
            .expect("register P041 fixture shared writer");
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
        let tool_reports = tool_value["reports"]
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
        let tool_report_artifacts = report_artifact_names_from_reports(&tool_value["reports"]);
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
    async fn p082_mcp_stdio_session_recheck_uses_live_principal_table_after_reload() {
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let source = server.live_principal_source();
        let token = "test-token-xxxxxxxxxxxxxxxxxxxxx";
        let credential = auth::LivePrincipalCredential {
            principal_id: "test-operator".into(),
            token_fingerprint: auth::token_fingerprint(token),
        };

        assert!(
            source.resolve_credential(&credential).is_ok(),
            "stdio initialized session precondition"
        );

        source.update(auth::PrincipalTable::test_fixture_disabled_token(
            token,
            "test-operator",
        ));
        assert!(
            source.resolve_credential(&credential).is_err(),
            "disabled bearer must be rejected by stdio live-session recheck"
        );

        source.update(auth::PrincipalTable::test_fixture_with_class(
            "observer-token-xxxxxxxxxxxxxxxxxx",
            "test-operator",
            auth::PrincipalClass::Observer,
        ));
        assert!(
            source.resolve_credential(&credential).is_err(),
            "revoked bearer fingerprint must be rejected by stdio live-session recheck"
        );

        source.update(auth::PrincipalTable::test_fixture_with_class(
            token,
            "test-operator",
            auth::PrincipalClass::Observer,
        ));
        let current = source
            .resolve_credential(&credential)
            .expect("re-scoped bearer remains resolvable");
        assert_eq!(current.class, auth::PrincipalClass::Observer);
        assert!(
            !current
                .tool_capabilities
                .contains(&domain::CapabilityToolId::ReportsGet),
            "stdio must observe re-scoped capabilities after reload"
        );
    }

    #[tokio::test]
    async fn p082_reports_get_tools_call_denies_non_operator_principals() {
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );

        for principal in [
            auth::Principal::new("agent-report-reader", auth::PrincipalClass::Agent),
            auth::Principal::new("observer-report-reader", auth::PrincipalClass::Observer),
        ] {
            let response = server
                .handle_request(
                    JsonRpcRequest {
                        jsonrpc: "2.0".to_string(),
                        id: Some(serde_json::json!(82)),
                        method: "tools/call".to_string(),
                        params: Some(serde_json::json!({
                            "name": "reports.get",
                            "arguments": {
                                "run_id": domain::ids::RunId::new().to_string()
                            }
                        })),
                    },
                    &principal,
                )
                .await;

            let error = response
                .error
                .expect("non-Operator reports.get call must be denied");
            assert_eq!(error.code, -32004);
            assert!(
                response.result.is_none(),
                "non-Operator reports.get must not return report lanes"
            );
        }
    }

    #[tokio::test]
    async fn p082_report_resource_read_denies_non_operator_principals() {
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );

        for principal in [
            auth::Principal::new("agent-report-resource", auth::PrincipalClass::Agent),
            auth::Principal::new("observer-report-resource", auth::PrincipalClass::Observer),
        ] {
            let response = server
                .handle_request(
                    JsonRpcRequest {
                        jsonrpc: "2.0".to_string(),
                        id: Some(serde_json::json!(83)),
                        method: "resources/read".to_string(),
                        params: Some(serde_json::json!({
                            "uri": format!("report://{}", domain::ids::RunId::new())
                        })),
                    },
                    &principal,
                )
                .await;

            let error = response
                .error
                .expect("non-Operator report:// read must be denied");
            assert_eq!(error.code, -32002);
            assert!(
                response.result.is_none(),
                "non-Operator report:// must not return report contents"
            );
        }
    }

    async fn read_runs_resource_json(
        server: &McpServer,
        principal: &auth::Principal,
    ) -> serde_json::Value {
        let response = server
            .handle_request(
                JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(serde_json::json!(1)),
                    method: "resources/read".to_string(),
                    params: Some(serde_json::json!({"uri": "chainworks://runs"})),
                },
                principal,
            )
            .await;
        assert!(
            response.error.is_none(),
            "resources/read chainworks://runs returned error: {:?}",
            response.error
        );
        let text = response.result.expect("resources/read result")["contents"][0]["text"]
            .as_str()
            .expect("resources/read text")
            .to_string();
        serde_json::from_str(&text).expect("chainworks://runs resource is JSON")
    }

    async fn read_resource_json(
        server: &McpServer,
        principal: &auth::Principal,
        uri: &str,
    ) -> serde_json::Value {
        let response = server
            .handle_request(
                JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(serde_json::json!(1)),
                    method: "resources/read".to_string(),
                    params: Some(serde_json::json!({ "uri": uri })),
                },
                principal,
            )
            .await;
        assert!(
            response.error.is_none(),
            "resources/read {uri} returned error: {:?}",
            response.error
        );
        let text = response.result.expect("resources/read result")["contents"][0]["text"]
            .as_str()
            .expect("resources/read text")
            .to_string();
        serde_json::from_str(&text).expect("resource payload is JSON")
    }

    #[tokio::test]
    async fn p082_chainworks_runs_resource_redacts_operator_only_projection_fields_for_non_operators(
    ) {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_blocked_implementation_summary(&pool, run_id).await;
        projections::rebuild_run_summary(&pool, run_id)
            .await
            .unwrap();

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let operator_runs = read_runs_resource_json(&server, &operator_principal()).await;
        let operator_row = operator_runs
            .as_array()
            .and_then(|rows| rows.first())
            .expect("operator chainworks://runs row");
        assert!(
            operator_row.get("workspace_root").is_some(),
            "operator resource read should include workspace_root before non-operator redaction"
        );
        assert!(
            operator_row.get("implementationCompletion").is_some(),
            "operator resource read should include projected implementationCompletion before non-operator redaction"
        );

        for principal_class in [auth::PrincipalClass::Observer, auth::PrincipalClass::Agent] {
            let principal = auth::Principal::new("non-operator", principal_class);
            let runs_json = read_runs_resource_json(&server, &principal).await;
            let row = runs_json
                .as_array()
                .and_then(|rows| rows.first())
                .expect("non-operator chainworks://runs row");
            for field in [
                "workspace_root",
                "artifact_root",
                "chainworks_meta_root",
                "implementationCompletion",
                "closeout_readiness_summary",
                "implementation_closeout_readiness_summary",
            ] {
                assert!(
                    row.get(field).is_none(),
                    "non-operator chainworks://runs resource must redact {field}: {row}"
                );
            }
        }
    }

    #[tokio::test]
    async fn p082_run_detail_resources_redact_operator_only_diagnostics_for_non_operators() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_blocked_implementation_summary(&pool, run_id).await;
        persist_rollout_contract_readback(&pool, run_id).await;
        seed_validation_attempt(&pool, run_id).await;
        projections::rebuild_run_summary(&pool, run_id)
            .await
            .unwrap();

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );

        let operator = operator_principal();
        let operator_run = read_resource_json(&server, &operator, &format!("run://{run_id}")).await;
        assert!(
            operator_run
                .get("implementation_self_assessment_summary")
                .is_some(),
            "operator run:// must retain implementation summary"
        );
        assert!(
            operator_run.get("rollout_contract_readback").is_some(),
            "operator run:// must retain rollout readback"
        );
        assert!(
            operator_run["stages"][0].get("agent_executions").is_some(),
            "operator run:// must retain execution details"
        );

        for principal_class in [auth::PrincipalClass::Observer, auth::PrincipalClass::Agent] {
            let principal = auth::Principal::new("non-operator-run-detail", principal_class);
            let public_payload =
                read_resource_json(&server, &principal, &format!("run://{run_id}")).await;
            let direct_chainworks_payload = server
                .read_resource_for_principal(&format!("chainworks://runs/{run_id}"), &principal)
                .await
                .expect("direct chainworks run detail resource should resolve");
            for (uri, payload) in [
                (format!("run://{run_id}"), public_payload),
                (
                    format!("chainworks://runs/{run_id}"),
                    direct_chainworks_payload,
                ),
            ] {
                for field in [
                    "implementation_self_assessment_summary",
                    "rollout_contract_readback",
                    "active_artifact_index",
                    "run_state_projection",
                    "operator_overrides",
                ] {
                    assert!(
                        payload.get(field).is_none(),
                        "non-Operator {uri} must redact {field}: {payload}"
                    );
                }
                assert!(
                    payload.get("stages").is_none(),
                    "non-Operator {uri} must not receive raw stage/agent execution internals: {payload}"
                );
            }
        }
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

    async fn open_hot_read_circuit(pool: &sqlx::SqlitePool, surface: &str) {
        for _ in 0..3 {
            db::repos::hot_read_circuit::record_violation(pool, surface, "timeout")
                .await
                .unwrap();
        }
    }

    async fn json_rpc_result(
        server: &McpServer,
        principal: &auth::Principal,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(format!("p087-{method}"))),
            method: method.to_string(),
            params,
        };
        let resp = server.handle_request(req, principal).await;
        assert!(
            resp.error.is_none(),
            "P087 liveness request {method} returned JSON-RPC error: {:?}",
            resp.error
        );
        resp.result
            .unwrap_or_else(|| panic!("P087 liveness request {method} must return result"))
    }

    #[tokio::test]
    async fn proposal_087_mcp_liveness_sequence_survives_running_maintenance_and_records_metrics() {
        let _guard = crate::hot_read_guard::P087_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "enforce",
        );
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();
        let now_ms = Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO maintenance_operations \
             (id, operation_kind, status, idempotency_key, slot_generation, created_at_ms, updated_at_ms) \
             VALUES ('p087-running-maintenance', 'repair_slot', 'running', 'p087-running-maintenance', 1, ?, ?)",
        )
        .bind(now_ms)
        .bind(now_ms)
        .execute(&pool)
        .await
        .unwrap();

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let principal = operator_principal();
        let started = std::time::Instant::now();

        json_rpc_result(
            &server,
            &principal,
            "initialize",
            Some(serde_json::json!({})),
        )
        .await;
        let tools_result = json_rpc_result(&server, &principal, "tools/list", None).await;
        assert!(
            tools_result["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| {
                    tool["name"] == serde_json::json!("runtime.health")
                        || tool["name"] == serde_json::json!("runtime_health")
                }),
            "P087 liveness inventory must include runtime.health"
        );
        let runs_list =
            call_tool_and_parse(&server, &principal, "runs.list", serde_json::json!({})).await;
        assert!(
            runs_list.as_array().is_some(),
            "runs.list must remain a direct bounded read payload"
        );
        let runtime_health =
            call_tool_and_parse(&server, &principal, "runtime.health", serde_json::json!({})).await;
        assert_eq!(runtime_health["schemaVersion"], "runtime_health.v1");
        assert_eq!(
            runtime_health["runtimeHealthProjection"]["schemaVersion"],
            "runtime_health_projection.v1"
        );
        assert!(runtime_health["runtimeHealthProjection"]["activeSessions"].is_number());
        assert!(
            runtime_health["runtimeHealthProjection"]["degradedFlags"]["hotReadCircuitOpen"]
                .is_boolean()
        );
        assert!(
            runtime_health["runtimeHealthProjection"]["writePressureFlags"]
                ["writerHeartbeatRequiredForStorageHealth"]
                .is_boolean()
        );
        assert!(runtime_health["runtimeHealthProjection"]["sideEffectUnresolvedCount"].is_number());
        assert!(runtime_health["runtimeHealthProjection"]["continuationActiveCount"].is_number());
        let storage_health =
            call_tool_and_parse(&server, &principal, "storage.health", serde_json::json!({})).await;
        assert_eq!(storage_health["tool"], "storage.health");
        assert_eq!(
            storage_health["errorCode"],
            tools::storage::ERR_STALE,
            "storage.health should fail fast with typed degraded status without a live writer"
        );
        json_rpc_result(
            &server,
            &principal,
            "resources/read",
            Some(serde_json::json!({"uri": format!("run://{run_id}")})),
        )
        .await;
        let _elapsed = started.elapsed();
        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");

        assert!(db::metrics::get_hot_read_p95("tools.list").is_some());
        assert!(db::metrics::get_hot_read_p95("runs.list").is_some());
        assert!(db::metrics::get_runs_list_read_latency_p95().is_some());
        assert!(db::metrics::get_hot_read_p95("runtime.health").is_some());
        assert!(db::metrics::get_hot_read_p95("storage.health").is_some());
        assert!(db::metrics::get_hot_read_p95("artifacts.metadata.get").is_some());
        assert!(db::metrics::get_mcp_liveness_gate_duration_p95().is_some());

        let readback = db::repos::storage_health::storage_health(&pool)
            .await
            .unwrap();
        assert_eq!(
            readback["artifactNoiseProjection"]["schemaVersion"],
            "artifact_noise_projection.v1"
        );
        assert!(readback["readPathMetrics"]["runsListReadLatencyP95Ms"].is_number());
        assert!(readback["readPathMetrics"]["mcpLivenessGateDurationP95Ms"].is_number());
        assert_eq!(
            readback["readPathMetrics"]["mcpLivenessGateDurationSource"],
            "runtime.health"
        );
        assert!(readback["readPathMetrics"]["mcpLivenessGateLastRecordedAtMs"].is_number());
    }

    #[tokio::test]
    async fn proposal_087_runs_list_seeded_load_stays_under_500ms_and_records_p95() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        for _ in 0..10 {
            let run_id = RunId::new();
            runs::insert(&pool, &make_run(run_id, idea_id))
                .await
                .unwrap();
            db::repos::projections::rebuild_run_summary(&pool, run_id)
                .await
                .unwrap();
        }
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let principal = operator_principal();

        for _ in 0..7 {
            let started = std::time::Instant::now();
            let payload =
                call_tool_and_parse(&server, &principal, "runs.list", serde_json::json!({})).await;
            let elapsed_ms = started.elapsed().as_millis();
            assert!(
                elapsed_ms < 500,
                "P087 runs.list seeded hot-read budget exceeded: {elapsed_ms}ms"
            );
            assert!(
                payload.as_array().is_some_and(|items| items.len() >= 10),
                "runs.list must return the seeded active run projection set"
            );
        }

        let p95 = db::metrics::get_runs_list_read_latency_p95()
            .expect("runs.list p95 must be recorded by production hot-read wrapper");
        assert!(p95 < 500, "P087 runs.list p95 budget exceeded: {p95}ms");
    }

    #[tokio::test]
    async fn proposal_087_tools_list_json_rpc_is_hot_read_guarded() {
        let _guard = crate::hot_read_guard::P087_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "enforce",
        );
        let pool = test_pool().await;
        open_hot_read_circuit(&pool, "tools.list").await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("p087-tools-list")),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = server.handle_request(req, &operator_principal()).await;
        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");

        let result = resp.result.expect("hot-read denial is a typed result");
        assert_eq!(result["error"], true);
        assert_eq!(
            result["errorCode"],
            tools::storage::ERR_HOT_READ_CIRCUIT_OPEN
        );
        assert_eq!(result["tool"], "tools.list");
    }

    #[tokio::test]
    async fn proposal_087_initialize_json_rpc_is_hot_read_guarded() {
        let _guard = crate::hot_read_guard::P087_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "enforce",
        );
        let pool = test_pool().await;
        open_hot_read_circuit(&pool, "initialize").await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("p087-initialize")),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({})),
        };
        let resp = server.handle_request(req, &operator_principal()).await;
        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");

        let result = resp.result.expect("hot-read denial is a typed result");
        assert_eq!(result["error"], true);
        assert_eq!(
            result["errorCode"],
            tools::storage::ERR_HOT_READ_CIRCUIT_OPEN
        );
        assert_eq!(result["tool"], "initialize");
    }

    #[tokio::test]
    async fn proposal_087_tools_list_typed_error_sanitizes_request_id() {
        let _guard = crate::hot_read_guard::P087_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "enforce",
        );
        let pool = test_pool().await;
        open_hot_read_circuit(&pool, "tools.list").await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let raw_id = format!("p087-tools-list\n{}", "x".repeat(2_048));
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(raw_id.clone())),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = server.handle_request(req, &operator_principal()).await;
        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");

        let result = resp.result.expect("hot-read denial is a typed result");
        let request_id = result["requestId"].as_str().unwrap();
        assert_ne!(request_id, raw_id);
        assert!(request_id.len() <= 128);
        assert!(!request_id.contains('\n'));
        assert!(request_id.starts_with("request_id:sha256:"));
        assert_eq!(
            result["errorCode"],
            tools::storage::ERR_HOT_READ_CIRCUIT_OPEN
        );
    }

    #[tokio::test]
    async fn proposal_087_tools_list_typed_error_rejects_structured_request_id() {
        let _guard = crate::hot_read_guard::P087_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "enforce",
        );
        let pool = test_pool().await;
        open_hot_read_circuit(&pool, "tools.list").await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!({"raw": "do-not-publish"})),
            method: "tools/list".to_string(),
            params: None,
        };
        let resp = server.handle_request(req, &operator_principal()).await;
        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");

        let result = resp.result.expect("hot-read denial is a typed result");
        let request_id = result["requestId"].as_str().unwrap();
        assert!(!request_id.contains("do-not-publish"));
        assert!(uuid::Uuid::parse_str(request_id).is_ok());
    }

    #[tokio::test]
    async fn proposal_087_storage_health_tool_call_open_circuit_is_typed_result() {
        let _guard = crate::hot_read_guard::P087_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "enforce",
        );
        let pool = test_pool().await;
        open_hot_read_circuit(&pool, "storage.health").await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("p087-storage-health")),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "storage.health",
                "arguments": {}
            })),
        };
        let resp = server.handle_request(req, &operator_principal()).await;
        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");

        assert!(
            resp.error.is_none(),
            "storage.health hot-read denial must not become JSON-RPC -32603"
        );
        let result = resp.result.expect("typed tool-result body is returned");
        let text = result["content"][0]["text"]
            .as_str()
            .expect("tools/call result text must be JSON");
        let payload: serde_json::Value = serde_json::from_str(text).expect("typed error JSON");
        assert_eq!(payload["error"], true);
        assert_eq!(
            payload["errorCode"],
            tools::storage::ERR_HOT_READ_CIRCUIT_OPEN
        );
        assert_eq!(payload["tool"], "storage.health");
        assert!(payload["retryAfterMs"].as_i64().is_some());
        assert_eq!(payload["hotRead"]["status"], "open");
    }

    #[tokio::test]
    async fn proposal_087_resources_read_json_rpc_is_hot_read_guarded() {
        let _guard = crate::hot_read_guard::P087_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "enforce",
        );
        let pool = test_pool().await;
        open_hot_read_circuit(&pool, "artifacts.metadata.get").await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!("p087-resource-read")),
            method: "resources/read".to_string(),
            params: Some(serde_json::json!({"uri": "run://00000000-0000-0000-0000-000000000001"})),
        };
        let resp = server.handle_request(req, &operator_principal()).await;
        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");

        let result = resp.result.expect("hot-read denial is a typed result");
        assert_eq!(result["error"], true);
        assert_eq!(
            result["errorCode"],
            tools::storage::ERR_HOT_READ_CIRCUIT_OPEN
        );
        assert_eq!(result["tool"], "artifacts.metadata.get");
    }

    #[test]
    fn test_mcp_tools_call_response_includes_journal_id_in_content_text() {
        // runs.cancel exercises a deep async call chain (handle_request →
        // dispatch_tool_instrumented → dispatch_tool_internal → CommandHandler).
        // In debug builds the combined future state machines can overflow the
        // default tokio worker-thread stack (2 MiB), so spawn on a dedicated
        // thread with a larger stack.
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
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
                            serde_json::json!({
                                "run_id": run_id.to_string(),
                                "idempotency_key": uuid::Uuid::now_v7().to_string(),
                            }),
                        )
                        .await;

                        let journal_id = payload["journal_id"]
                            .as_str()
                            .expect("runs.cancel response must contain journal_id as a string");
                        assert!(
                            !journal_id.is_empty(),
                            "journal_id must be non-empty (uuid from CommandHandler::handle)"
                        );
                    })
            })
            .unwrap()
            .join()
            .unwrap();
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
    async fn tools_list_records_mcp_liveness_duration_metric() {
        db::repos::storage_health::reset_read_path_metrics_for_tests();
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: "tools/list".to_string(),
            params: Some(serde_json::json!({})),
        };
        let response = server.handle_request(req, &operator_principal()).await;
        assert!(response.result.is_some());

        let health = db::repos::storage_health::storage_health(&pool)
            .await
            .unwrap();
        assert!(
            health["readPath"]["mcpLivenessGate"]["sampleCount"]
                .as_u64()
                .is_some_and(|c| c >= 1),
            "tools/list must record at least one mcp_liveness_gate_duration sample"
        );
        assert!(
            health["readPath"]["mcpLivenessGate"]["mcp_liveness_gate_duration_ms"]
                .as_u64()
                .is_some()
        );
    }

    #[tokio::test]
    async fn proposal_087_runtime_health_returns_projection_backed_summary() {
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );

        let payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "runtime.health",
            serde_json::json!({}),
        )
        .await;

        assert_eq!(
            payload["schemaVersion"], "runtime_health.v1",
            "runtime.health must expose the compact projection-backed runtime summary"
        );
        assert_eq!(
            payload["runtimeHealthProjection"]["schemaVersion"],
            "runtime_health_projection.v1"
        );
        assert!(payload["runtimeHealthProjection"]["activeSessions"]
            .as_i64()
            .is_some());
        assert!(
            payload["runtimeHealthProjection"]["degradedFlags"]["hotReadCircuitOpen"]
                .as_bool()
                .is_some()
        );
    }

    #[tokio::test]
    async fn proposal_087_mcp_liveness_gate_covers_required_read_sequence() {
        db::repos::storage_health::reset_read_path_metrics_for_tests();
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let principal = operator_principal();

        let initialize = server
            .handle_request(
                JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(serde_json::json!(1)),
                    method: "initialize".to_string(),
                    params: Some(serde_json::json!({
                        "clientInfo": {"principal_token": "test-token"}
                    })),
                },
                &principal,
            )
            .await;
        assert!(initialize.result.is_some());

        let tools_list = server
            .handle_request(
                JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(serde_json::json!(2)),
                    method: "tools/list".to_string(),
                    params: Some(serde_json::json!({})),
                },
                &principal,
            )
            .await;
        let tools = tools_list.result.expect("tools/list result")["tools"]
            .as_array()
            .cloned()
            .expect("tools/list returns tools array");
        assert!(
            tools.iter().any(|tool| tool["name"] == "runtime_health"),
            "operator tools/list must expose runtime.health as Codex-compatible runtime_health"
        );

        let runs_resource = server
            .handle_request(
                JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(serde_json::json!(3)),
                    method: "resources/read".to_string(),
                    params: Some(serde_json::json!({"uri": "chainworks://runs"})),
                },
                &principal,
            )
            .await;
        let runs_text = runs_resource.result.expect("resources/read result")["contents"][0]["text"]
            .as_str()
            .expect("resources/read text")
            .to_string();
        let runs_json: serde_json::Value =
            serde_json::from_str(&runs_text).expect("chainworks://runs resource is JSON");
        assert!(runs_json.is_array());

        let runs_list =
            call_tool_and_parse(&server, &principal, "runs.list", serde_json::json!({})).await;
        assert!(runs_list.is_array());

        let runtime_health =
            call_tool_and_parse(&server, &principal, "runtime.health", serde_json::json!({})).await;
        assert_eq!(runtime_health["schemaVersion"], "runtime_health.v1");
        assert_eq!(
            runtime_health["runtimeHealthProjection"]["schemaVersion"],
            "runtime_health_projection.v1"
        );

        let storage_health =
            call_tool_and_parse(&server, &principal, "storage.health", serde_json::json!({})).await;
        assert_eq!(storage_health["tool"], "storage.health");
        assert_eq!(storage_health["error"], true);
        assert_eq!(storage_health["errorCode"], tools::storage::ERR_STALE);

        let final_health = db::repos::storage_health::storage_health(&pool)
            .await
            .unwrap();
        assert!(
            final_health["readPath"]["mcpLivenessGate"]["sampleCount"]
                .as_u64()
                .unwrap_or(0)
                >= 6,
            "required MCP liveness sequence must be measured end to end: {final_health}"
        );
        assert_eq!(
            final_health["readPath"]["runsList"]["sampleCount"], 1,
            "runs.list latency must be recorded by the production tool path"
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
                "arguments": {
                    "reason": "manual",
                    "idempotency_key": uuid::Uuid::now_v7().to_string(),
                },
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

    // ── P082: report:// resource parity tests ──────────────────────────────

    #[tokio::test]
    async fn p082_report_resource_includes_plural_readbacks_not_singular() {
        // P082 lane contract: report://{run_id} exposes p082_recovery_matrix_readbacks
        // (plural array) only. The singular p082_recovery_matrix_readback must NOT be
        // present in the report resource payload.
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
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
            .read_resource(&format!("report://{run_id}"))
            .await
            .unwrap();

        // Plural must be present and must be an array (empty when no recovery rows).
        assert!(
            value.get("p082_recovery_matrix_readbacks").is_some(),
            "P082: report:// resource must include p082_recovery_matrix_readbacks (plural)"
        );
        assert!(
            value["p082_recovery_matrix_readbacks"].is_array(),
            "P082: report:// p082_recovery_matrix_readbacks must be an array"
        );

        // Singular must NOT be present per the lane contract.
        assert!(
            value.get("p082_recovery_matrix_readback").is_none(),
            "P082: report:// resource must NOT include singular p082_recovery_matrix_readback (lane contract)"
        );
    }

    #[tokio::test]
    async fn p082_report_resource_non_empty_readbacks_when_startup_repair_exists() {
        // When a startup_repair row carries a valid P082 readback in its notes,
        // report://{run_id} must return a non-empty p082_recovery_matrix_readbacks array
        // with content byte-equivalent to reports.get mcp_execution_truth lane.
        use db::repos::startup_repairs;
        use domain::recovery_matrix;

        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        // Seed a startup_repair with a valid P082 readback in notes.
        let repair_id = format!("p082-requeue:cj-server-test:{run_id}:1");
        let readback = recovery_matrix::set_readback_startup_repair(
            recovery_matrix::build_readback_v1(
                "P082-R01",
                "repaired",
                "retry",
                recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
                "Startup requeue scheduled; requeue_generation=1.",
                "startup_repairs, work_items, command_journal",
                "startup_repairs, work_items",
                &repair_id,
                Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                "valid",
                "2026-05-22T00:00:00Z",
            ),
            recovery_matrix::build_startup_repair_summary(
                &repair_id,
                "wi-server-test",
                "cj-server-test",
                1,
                1,
                false,
                60_000,
                "2026-05-22T00:00:00Z",
                false,
                None,
                "global",
            ),
            None,
        );
        let notes = serde_json::json!({
            "requeue_generation": 1,
            "max_requeue_generation": 1,
            "p082_recovery_matrix_readback": readback,
        })
        .to_string();

        startup_repairs::record(
            &pool,
            &format!("p082-requeue:cj-server-test:{run_id}:1"),
            &run_id.to_string(),
            "requeue_once",
            Utc::now(),
            Some(&notes),
        )
        .await
        .unwrap();

        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let value = server
            .read_resource(&format!("report://{run_id}"))
            .await
            .unwrap();

        let readbacks = value["p082_recovery_matrix_readbacks"]
            .as_array()
            .expect("P082: p082_recovery_matrix_readbacks must be an array");

        assert_eq!(
            readbacks.len(),
            1,
            "P082: report:// must return exactly one readback row when one startup_repair row exists"
        );
        assert_eq!(
            readbacks[0]["scenario_id"].as_str(),
            Some("P082-R01"),
            "P082: report:// readback must have scenario_id=P082-R01"
        );
        assert_eq!(
            readbacks[0]["scenario_status"].as_str(),
            Some("repaired"),
            "P082: report:// readback must use approved vocabulary (repaired)"
        );
        assert_eq!(
            readbacks[0]["recovery_reason_code"].as_str(),
            Some("startup_requeue_once"),
            "P082: report:// readback must have correct reason_code"
        );
        // Singular must not be present in report:// resource.
        assert!(
            value.get("p082_recovery_matrix_readback").is_none(),
            "P082: report:// resource must NOT expose singular p082_recovery_matrix_readback"
        );
    }

    #[tokio::test]
    async fn p082_report_resource_run_report_artifact_empty_for_non_operator() {
        use domain::recovery_matrix;

        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let now = Utc::now();
        let repair_id = format!("p082-requeue:cj-server-authz:{run_id}:1");
        let readback = recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup requeue scheduled; requeue_generation=1.",
            "startup_repairs",
            "startup_repairs, work_items",
            &repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &now.to_rfc3339(),
        );
        let notes = serde_json::json!({ "p082_recovery_matrix_readback": readback }).to_string();
        startup_repairs::record(
            &pool,
            &repair_id,
            &run_id.to_string(),
            "requeue_once",
            now,
            Some(&notes),
        )
        .await
        .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: ArtifactId::new(),
                run_id,
                stage_id: "state_12_workflow_complete".into(),
                agent_id: "lead_orchestrator".into(),
                name: "run_report".into(),
                contract_id: "run_report_v1".into(),
                format: ArtifactFormat::Json,
                file_path: "/tmp/p082-report-resource-run-report.json".into(),
                checksum_sha256: None,
                size_bytes: Some(2),
                provider: "system".into(),
                model: None,
                created_at: now,
                is_pinned: false,
                report_kind: Some("run_report".into()),
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

        for principal_class in [auth::PrincipalClass::Agent, auth::PrincipalClass::Observer] {
            let principal = auth::Principal::new("non-operator", principal_class);
            let err = server
                .read_resource_for_principal(&format!("report://{run_id}"), &principal)
                .await
                .expect_err("non-operator report:// helper must deny before materialization");
            assert!(
                err.to_string().contains("Resource not found"),
                "P082 SEC-HIGH-1: report:// must deny non-Operators before payload materialization"
            );
        }
    }
}
