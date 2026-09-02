use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use tokio::io::{AsyncWriteExt, BufReader};
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
use domain::CapabilityToolId;
use domain::ResourceTemplateId;

pub struct McpServer {
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    acp_runtime: Option<Arc<acp::AcpRuntimeManager>>,
    pub principal_table: auth::PrincipalTable,
    live_principal_source: auth::LivePrincipalSource,
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

fn provider_quota_retry_after_response(
    id: Option<serde_json::Value>,
    tool_name: &str,
    request_id: Option<&str>,
    error: &anyhow::Error,
) -> Option<JsonRpcResponse> {
    let quota_wait =
        error.downcast_ref::<engine::command_handler::ProviderQuotaRetryAfterError>()?;
    Some(JsonRpcResponse::error_with_data(
        id,
        -32077,
        "provider quota retry window has not elapsed",
        serde_json::json!({
            "code": "PROVIDER_QUOTA_RETRY_AFTER",
            "tool_name": tool_name,
            "retry_after": quota_wait.retry_after.to_rfc3339(),
            "request_id": request_id,
        }),
    ))
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
            acp_runtime: None,
            live_principal_source: auth::LivePrincipalSource::new(principal_table.clone()),
            principal_table,
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
        Self {
            pool,
            cmd_handler,
            acp_runtime: None,
            live_principal_source: auth::LivePrincipalSource::new(principal_table.clone()),
            principal_table,
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
        Self {
            pool,
            cmd_handler,
            acp_runtime: None,
            live_principal_source: auth::LivePrincipalSource::new(principal_table.clone()),
            principal_table,
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
        Self {
            pool,
            cmd_handler,
            acp_runtime: None,
            live_principal_source: auth::LivePrincipalSource::new(principal_table.clone()),
            principal_table,
            events: Some(events),
            storage_writer_heartbeat: None,
            boundary_policy: Some(embedded_shadow_boundary_policy()),
        }
    }

    /// P081 Phase 3: attach the shared immutable boundary policy service.
    /// Call this after construction; the daemon chains it on the existing constructor.
    pub fn with_boundary_policy(mut self, policy: Arc<auth::boundary::BoundaryPolicy>) -> Self {
        self.boundary_policy = Some(policy);
        self
    }

    /// P086: Provider-session resurrection admission needs the actual selected
    /// ACP adapter capability, not provider-family string matching.
    pub fn with_acp_runtime(mut self, acp_runtime: Arc<acp::AcpRuntimeManager>) -> Self {
        self.acp_runtime = Some(acp_runtime);
        self
    }

    pub fn resolve_current_bearer(&self, token: &str) -> Result<auth::Principal, auth::AuthError> {
        self.live_principal_source.resolve_bearer(token)
    }

    pub fn live_principal_source(&self) -> auth::LivePrincipalSource {
        self.live_principal_source.clone()
    }

    pub async fn run_stdio(&self) -> Result<()> {
        info!("McpServer: starting stdio JSON-RPC loop");

        let stdin = tokio::io::stdin();
        let stdout = Arc::new(Mutex::new(tokio::io::stdout()));
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();
        let mut session_principal: Option<auth::Principal> = None;
        let mut session_token_fingerprint: Option<String> = None;
        let mut notification_task: Option<tokio::task::JoinHandle<()>> = None;

        loop {
            // SEC-MED-001: use size-capped read to prevent unbounded memory allocation
            // from oversized lines before auth, duplicate-key scan, or JSON parsing.
            line.clear();
            match stdio_read_line_limited(&mut reader, &mut line, MCP_STDIO_LINE_LIMIT_BYTES).await
            {
                Ok(0) => break, // EOF
                Ok(_) => {}

                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    // Line exceeded size limit; oversized line already drained.
                    db::metrics::increment_counter_with_label(
                        "p080_mcp_parser_rejected_total",
                        "stdio_line_too_long",
                    );
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: Some(serde_json::json!({
                            "schema_version": "p080_error_response_v1",
                            "code": "request_too_large",
                            "message": "stdio line exceeds 256 KiB limit; request rejected",
                            "retry_after": null,
                            "readback": null,
                        })),
                        error: None,
                    };
                    write_json_line(&stdout, &resp).await;
                    continue;
                }
                Err(e) => return Err(e.into()),
            };

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // SEC-P080-HIGH-001: duplicate-key rejection at the raw-parse boundary for ALL
            // JSON-RPC requests (mirrors HTTP transport). Runs before typed extraction so
            // unicode-escaped method/name values cannot bypass last-value-wins rejection.
            if let Some(dup_key) = crate::http::find_duplicate_json_object_key(trimmed) {
                if dup_key == "__budget_exceeded__" {
                    db::metrics::increment_counter_with_label(
                        "p080_mcp_canonicalization_budget_exceeded_total",
                        "scan_key_budget_stdio",
                    );
                    // SEC-P080-HIGH-001: budget exceeded means we cannot guarantee no duplicate
                    // keys exist. Reject so oversized payloads cannot bypass the duplicate-key
                    // gate (proposal lines 170-186).
                    db::metrics::increment_counter_with_label(
                        "p080_mcp_parser_rejected_total",
                        "canonicalization_budget_exceeded",
                    );
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: Some(serde_json::json!({
                            "schema_version": "p080_error_response_v1",
                            "code": "canonicalization_budget_exceeded",
                            "message": "JSON request scan budget exceeded; request rejected to prevent duplicate-key bypass",
                            "retry_after": null,
                            "readback": null,
                            "detail": { "observed": "budget_exceeded" }
                        })),
                        error: None,
                    };
                    write_json_line(&stdout, &resp).await;
                    continue;
                } else {
                    db::metrics::increment_counter_with_label(
                        "p080_mcp_parser_rejected_total",
                        "duplicate_key",
                    );
                    let resp = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: Some(serde_json::json!({
                            "schema_version": "p080_error_response_v1",
                            "code": "duplicate_key",
                            "message": "JSON request contains duplicate object key; request rejected",
                            "retry_after": null,
                            "readback": null,
                            "detail": { "duplicate_key": dup_key }
                        })),
                        error: None,
                    };
                    write_json_line(&stdout, &resp).await;
                    continue;
                }
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
                    }
                    Some(t) => match self.resolve_current_bearer(t) {
                        Ok(p) => {
                            let is_operator = matches!(p.class, auth::PrincipalClass::Operator);
                            session_token_fingerprint = Some(auth::token_fingerprint(t));
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
            let principal = match (
                session_principal.as_ref(),
                session_token_fingerprint.as_ref(),
            ) {
                (Some(p), Some(fingerprint)) => match self
                    .live_principal_source
                    .resolve_principal_by_id_and_token_fingerprint(&p.id, fingerprint)
                {
                    Ok(current) => current,
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
                _ => {
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
                // P081 Phase 4: evaluate BoundaryPolicy for mcp_initialize so the same
                // daemon-injected policy instance governs this transport.
                // Denied callers receive an auth failure without capability inventory.
                // Shadow mode logs the matrix decision but proceeds with the response.
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(&principal);
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
                            // P081 AC25: durable deny audit before returning denial.
                            // Fail closed if the audit write cannot commit.
                            // matrix_row: p081.agent_operator.mcp_initialize.capability
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

                // P081 Phase 4: include boundary_policy capability metadata when the
                // shared policy service is available so clients can detect -32004 semantics.
                let mut capabilities = serde_json::json!({ "tools": {} });
                if let Some(policy) = &self.boundary_policy {
                    capabilities["boundary_policy"] = serde_json::json!({
                        "matrix_id": "p081-boundary-matrix-v1",
                        "schema_version": 1,
                        "capability_schema_version": 1,
                        "mode": policy.mode().as_str(),
                        "denied_known_tool_code": -32004,
                        "field_casing": "snake_case"
                    });
                }
                JsonRpcResponse::success(
                    id,
                    serde_json::json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": capabilities,
                        "serverInfo": {
                            "name": "chainworks-control-plane",
                            "version": "0.1.0"
                        }
                    }),
                )
            }

            "tools/list" => {
                // P081 Phase 3: evaluate BoundaryPolicy for mcp_tools_list.
                // Denied callers see an empty tools list; no command_journal write.
                // Shadow mode logs the matrix decision without filtering.
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(&principal);
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
                            // P081 AC25: durable deny audit before returning the filtered response.
                            // Fail closed if the audit write cannot commit.
                            // matrix_row: p081.agent_operator.mcp_tools_list.discovery
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
                                    "BoundaryPolicy shadow: matrix would filter mcp_tools_list"
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

                // Audit defect 2 fix: gate P080 tools on rollout-control readability.
                // Proposal §5.6 L601 requires refusing to register P080 tools when the
                // rollout_control table is unreadable (absent row or DB error).
                let p080_rollout_readable = db::repos::p080::is_rollout_readable(&self.pool).await;

                let tools_json: Vec<serde_json::Value> = self
                    .visible_tool_specs(principal)
                    .into_iter()
                    .filter(|t| {
                        if tools::canonical_tool_name(&t.name).starts_with("p080.") {
                            p080_rollout_readable
                        } else {
                            true
                        }
                    })
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
                    // Unknown tool — not in the registry at all → -32601 per spec.
                    return JsonRpcResponse::error(
                        id,
                        -32601,
                        format!("Method not found: {tool_name}"),
                    );
                };
                let canonical_tool_name = tools::canonical_tool_name(&tool_name);

                if !tools::p064_operator_tool_enabled(canonical_tool_name) {
                    // Tool exists but is disabled by P064 feature gate → -32601 (same as unknown).
                    return JsonRpcResponse::error(
                        id,
                        -32601,
                        format!("Method not found: {tool_name}"),
                    );
                }

                // P081 Phase 3: evaluate BoundaryPolicy for mcp_tools_call when injected.
                // Known-but-denied tools return -32004, not -32601, per the MCP contract.
                // Shadow mode: log disagreement but fall through. LegacyPassthrough: no check.
                // Capture the Allow row_id so it can be threaded into CallerContext via task-local.
                let mut policy_allowed_row_id: Option<String> = None;
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(&principal);
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
                            // P081 AC25: durable deny audit before returning denial.
                            // Fail closed if the audit write cannot commit.
                            // matrix_row: p081.agent_operator.mcp_tools_call.command
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                "mcp_tools_call",
                                &tool_name,
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
                                );
                            }
                            return JsonRpcResponse::policy_denial(
                                id,
                                &reason_code,
                                caller_class.as_str(),
                                row_id.as_deref(),
                                "p081-boundary-matrix-v1",
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
                    // H-003: write_mcp_deny_audit must be called for ALL capability denials,
                    // including storage tools; the audit write happens before any response.
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
                    // Storage tools use a typed response protocol so clients can distinguish
                    // storage-specific error codes. Return typed-success after audit commit.
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
                    // Return -32004 to distinguish known-but-denied from unknown tools (-32601).
                    return JsonRpcResponse::policy_denial(
                        id,
                        "CAPABILITY_OUT_OF_SCOPE",
                        caller_class.as_str(),
                        None,
                        "p081-boundary-matrix-v1",
                    );
                }

                let tool_params = params["arguments"].clone();
                if canonical_tool_name == "automation.auto_retry.latest" {
                    let requested_versions = tool_params
                        .get("client_supported_versions")
                        .and_then(|value| value.as_array())
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(|value| value.as_str().map(str::to_owned))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_else(|| vec!["auto_retry_readback.v1".to_string()]);
                    if !requested_versions
                        .iter()
                        .any(|version| version == "auto_retry_readback.v1")
                    {
                        let request_id = crate::request_context::current_request_id();
                        return JsonRpcResponse::error_with_data(
                            id,
                            -32076,
                            "unsupported_version".to_string(),
                            serde_json::json!({
                                "code": "unsupported_version",
                                "supported_versions": ["auto_retry_readback.v1"],
                                "unsupported_versions": requested_versions,
                                "requested_versions": requested_versions
                            }),
                        )
                        .with_error_request_id(request_id.as_deref());
                    }
                }

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
                // SEC-P083-LOW-002: P083 command tools bypass this precheck because they use
                // request_id (CallerRequestId UUIDv4) via command_idempotency_contract_v1, not
                // idempotency_key (P081 UUIDv7). They are still subject to audit-budget checks.
                let (idempotency_claimed_key, idempotency_claimed_hash) =
                    if is_state_changing_call(canonical_tool_name, &tool_params)
                        && !is_p083_command_idempotency_tool(canonical_tool_name)
                    {
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
                            self.dispatch_tool(canonical_tool_name, tool_params, principal),
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
                        if let Some(response) = provider_quota_retry_after_response(
                            id.clone(),
                            canonical_tool_name,
                            rid.as_deref(),
                            &e,
                        ) {
                            return response;
                        }
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
                // P081: evaluate BoundaryPolicy before exposing resource listing.
                // Observer compact reads are modeled as mcp_tools_call in the matrix;
                // operator/automation discovery remains mcp_tools_list.
                // matrix_row: p081.agent_operator.mcp_tools_list.discovery
                // matrix_row: p081.observer.mcp_tools_call.compact_read
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(&principal);
                    let policy_transport = resources_list_policy_transport(caller_class.as_str());
                    match policy.evaluate(
                        caller_class.as_str(),
                        policy_transport,
                        Some("resources.list"),
                    ) {
                        auth::boundary::PolicyDecision::Deny {
                            reason_code,
                            row_id,
                            ..
                        } => {
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                policy_transport,
                                "resources/list",
                                &reason_code,
                                row_id.as_deref(),
                            )
                            .await
                            {
                                tracing::error!(
                                    error = %e,
                                    "boundary deny audit write failed; failing closed (resources/list)"
                                );
                                return JsonRpcResponse::error(
                                    id,
                                    -32000,
                                    "unauthorized".to_string(),
                                );
                            }
                            return JsonRpcResponse::policy_denial(
                                id,
                                &reason_code,
                                caller_class.as_str(),
                                row_id.as_deref(),
                                "p081-boundary-matrix-v1",
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
                // P081: evaluate BoundaryPolicy for resource template discovery.
                // Use concrete action so absent matrix rows fail closed (H-002 fix).
                // matrix_row: p081.agent_operator.mcp_tools_list.discovery
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(&principal);
                    match policy.evaluate(
                        caller_class.as_str(),
                        "mcp_tools_list",
                        Some("resources.templates.list"),
                    ) {
                        auth::boundary::PolicyDecision::Deny {
                            reason_code,
                            row_id,
                            ..
                        } => {
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                "mcp_tools_list",
                                "resources/templates/list",
                                &reason_code,
                                row_id.as_deref(),
                            )
                            .await
                            {
                                tracing::error!(
                                    error = %e,
                                    "boundary deny audit write failed; failing closed (resources/templates/list)"
                                );
                                return JsonRpcResponse::error(
                                    id,
                                    -32000,
                                    "unauthorized".to_string(),
                                );
                            }
                            return JsonRpcResponse::policy_denial(
                                id,
                                &reason_code,
                                caller_class.as_str(),
                                row_id.as_deref(),
                                "p081-boundary-matrix-v1",
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
                                    "BoundaryPolicy shadow: matrix would deny resources/templates/list"
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
                        .filter(|id| resource_template_uri(*id).contains('{'))
                        .map(resource_template_definition_value)
                        .collect();

                JsonRpcResponse::success(id, serde_json::json!({ "resourceTemplates": filtered }))
            }

            "resources/read" => {
                // P081: evaluate BoundaryPolicy before exposing resource data.
                // Use concrete action "resources.read" so absent matrix rows fail closed (H-002 fix).
                // matrix_row: p081.agent_operator.mcp_tools_call.command
                if let Some(policy) = &self.boundary_policy {
                    let caller_class = auth::derive_caller_class_for_mcp(&principal);
                    match policy.evaluate(
                        caller_class.as_str(),
                        "mcp_tools_call",
                        Some("resources.read"),
                    ) {
                        auth::boundary::PolicyDecision::Deny {
                            reason_code,
                            row_id,
                            ..
                        } => {
                            if let Err(e) = write_mcp_deny_audit(
                                &self.pool,
                                self.boundary_policy.as_deref(),
                                principal,
                                "mcp_tools_call",
                                "resources/read",
                                &reason_code,
                                row_id.as_deref(),
                            )
                            .await
                            {
                                tracing::error!(
                                    error = %e,
                                    "boundary deny audit write failed; failing closed (resources/read)"
                                );
                                return JsonRpcResponse::error(
                                    id,
                                    -32000,
                                    "unauthorized".to_string(),
                                );
                            }
                            return JsonRpcResponse::policy_denial(
                                id,
                                &reason_code,
                                caller_class.as_str(),
                                row_id.as_deref(),
                                "p081-boundary-matrix-v1",
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
                // SEC-P080-001 (belt-and-suspenders): Agent, Observer, and ReadOnlyOperator
                // must never read artifact:// or report:// resources regardless of how the
                // resource capability matrix evolves. resource_allowed_for_class already
                // denies ReadOnlyOperator all resources, but this guard makes the contract
                // explicit and prevents future regressions if the matrix widens.
                if matches!(
                    principal.class,
                    auth::PrincipalClass::Agent
                        | auth::PrincipalClass::Observer
                        | auth::PrincipalClass::ReadOnlyOperator
                ) && (uri.starts_with("artifact://") || uri.starts_with("report://"))
                {
                    return JsonRpcResponse::error(id, -32002, "Resource not found".to_string());
                }
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
            .chain(
                principal
                    .tool_capabilities
                    .contains(&CapabilityToolId::RuntimeHealth)
                    .then(|| {
                        tools::codex_compatible_tool(tools::runtime::boundary_runtime_tool_spec())
                    }),
            )
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
            Err(e) => {
                // SEC-P081-003: Log full error chain server-side; return bounded envelope
                // with request_id correlation only — no raw error text to clients.
                let rid = crate::request_context::current_request_id();
                tracing::error!(
                    error = %e,
                    request_id = ?rid,
                    uri = %uri,
                    "mcp resources/read: internal error"
                );
                let data = serde_json::json!({
                    "code": "INTERNAL",
                    "request_id": rid,
                });
                JsonRpcResponse::error_with_data(id, -32603, "INTERNAL", data)
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
            let artifact_rows_raw =
                projections::list_artifacts_projection(&self.pool, run_id).await?;
            let artifact_rows: Vec<serde_json::Value> = artifact_rows_raw
                .iter()
                .map(|row| {
                    let mut v = serde_json::to_value(row).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut m) = v {
                        m.remove("file_path");
                    }
                    v
                })
                .collect();
            let run_artifacts =
                db::repos::artifacts::list_by_run(&self.pool, run_id_parsed).await?;
            let (mcp_rollout_readback, run_report_rollout_readback) =
                rollout_contract_readback_lanes_json(&self.pool, run_id_parsed).await?;
            // Scan once per resource read and reuse across every artifact projected
            // below (SR-HIGH-002: avoids one full filesystem scan per report artifact).
            let temp_artifact_inventory_dto =
                tools::reports::p089_temp_artifact_inventory_run_report_section(
                    run_id_parsed,
                    &principal.class,
                )
                .await;
            let mut artifact_payloads = Vec::with_capacity(run_artifacts.len());
            for artifact in &run_artifacts {
                artifact_payloads.push(
                    tools::reports::artifact_report_json_with_temp_artifact_inventory(
                        &self.pool,
                        artifact,
                        Some(&run_report_rollout_readback),
                        &principal.class,
                        Some(&temp_artifact_inventory_dto),
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
            let retry_authority_history =
                tools::reports::retry_authority_history_json(&self.pool, run_id_parsed).await?;
            let retry_authority =
                tools::reports::retry_authority_current_json(&self.pool, run_id_parsed).await?;
            let p091_orphan_repair_readback =
                tools::reports::p091_orphan_repair_readback_json(&self.pool, run_id_parsed).await?;
            let p082_recovery_matrix_readbacks =
                tools::reports::p082_recovery_matrix_readbacks_json(
                    &self.pool,
                    run_id_parsed,
                    &principal.class,
                    "report_resource",
                )
                .await?;

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
                "retryAuthority": retry_authority,
                "retryAuthorityHistory": retry_authority_history,
                "p091OrphanRepairReadback": p091_orphan_repair_readback,
                "p082_recovery_matrix_readbacks": p082_recovery_matrix_readbacks,
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
            let is_operator = principal.class == auth::PrincipalClass::Operator;
            let rows = projections::list_active_projection(&self.pool).await?;
            // SEC-003: Redact local filesystem path fields for non-Operator principals.
            let values = rows
                .into_iter()
                .map(|row| {
                    let mut v = serde_json::to_value(row)?;
                    tools::runs::redact_run_projection_paths(&mut v, is_operator);
                    Ok(v)
                })
                .collect::<std::result::Result<Vec<serde_json::Value>, serde_json::Error>>()?;
            return Ok(serde_json::Value::Array(values));
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
                let redacted: Vec<serde_json::Value> = rows
                    .iter()
                    .map(|row| {
                        let mut v = serde_json::to_value(row).unwrap_or_default();
                        if let serde_json::Value::Object(ref mut m) = v {
                            m.remove("file_path");
                        }
                        v
                    })
                    .collect();
                return Ok(serde_json::to_value(redacted)?);
            } else if let Some(rid) = run_id.strip_suffix("/temp-artifact-inventory") {
                return tools::temp_artifacts::inventory_preview_for_run_resource(rid, principal)
                    .await;
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
        let is_operator = matches!(principal.class, auth::PrincipalClass::Operator);
        let mut value = serde_json::to_value(run)?;
        // SEC HIGH-001: strip operator-only snapshot fields before any caller-visible return.
        tools::runs::redact_run_snapshot_fields(&mut value, is_operator);
        if let Some(obj) = value.as_object_mut() {
            // HIGH-002: projection, overrides, and rollout readback are Operator-only.
            if is_operator {
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
                obj.insert(
                    "rollout_contract_readback".into(),
                    rollout_contract_readback_json(&self.pool, run_id_parsed).await?,
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
            // SEC-001: escalation attribution columns are Operator-only; strip them from
            // agent_executions rows served to Agent and Observer principals.
            let is_operator = matches!(principal.class, auth::PrincipalClass::Operator);
            const ESCALATION_EXECUTION_FIELDS: &[&str] = &[
                "escalation_policy_id",
                "escalation_policy_hash",
                "escalation_tier_id",
                "escalation_tier_kind_raw",
                "escalation_trigger_raw",
                "escalation_digest_version",
                "escalation_ledger_id",
            ];
            let stage_rows = stages::list_by_run(&self.pool, run_id_parsed).await?;
            let mut stage_values = Vec::new();
            for stage in stage_rows {
                let executions = agent_executions::find_by_stage(&self.pool, stage.id).await?;
                let mut stage_value = serde_json::to_value(&stage)?;
                if let Some(stage_obj) = stage_value.as_object_mut() {
                    let exec_values: Vec<serde_json::Value> = executions
                        .iter()
                        .map(|exec| {
                            let mut v = serde_json::to_value(exec)
                                .map_err(|e| anyhow::anyhow!("exec serialize: {e}"))?;
                            if !is_operator {
                                if let Some(o) = v.as_object_mut() {
                                    for field in ESCALATION_EXECUTION_FIELDS {
                                        o.remove(*field);
                                    }
                                }
                            }
                            Ok(v)
                        })
                        .collect::<anyhow::Result<_>>()?;
                    stage_obj.insert(
                        "agent_executions".into(),
                        serde_json::Value::Array(exec_values),
                    );
                }
                stage_values.push(stage_value);
            }
            obj.insert("stages".into(), serde_json::Value::Array(stage_values));
            // P058 Phase 1: attach escalation_readback parity on run:// resource.
            // Full chain detail only for Operator; summary-only for Agent/Observer.
            let escalation_readback = if is_operator {
                tools::runs::build_escalation_readback_json(&self.pool, run_id_parsed).await?
            } else {
                tools::runs::build_escalation_readback_summary_json(&self.pool, run_id_parsed)
                    .await?
            };
            obj.insert("escalation_readback".into(), escalation_readback);
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
        } else if tool_name.starts_with("effects.") {
            tools::effects::execute(tool_name, params, pool, principal).await
        } else if tool_name == "runtime.health" {
            tools::runtime::execute(params, pool, self.boundary_policy.as_deref()).await
        } else if tool_name.starts_with("runtime.") {
            tools::runtime::execute_with_name(
                tool_name,
                params,
                pool,
                self.boundary_policy.as_deref(),
            )
            .await
        } else if tool_name == "boundary.runtime.get" {
            tools::runtime::execute_with_name(
                tool_name,
                params,
                pool,
                self.boundary_policy.as_deref(),
            )
            .await
        } else if tool_name == "operator.alerts.list" {
            tools::runtime::execute_with_name(
                tool_name,
                params,
                pool,
                self.boundary_policy.as_deref(),
            )
            .await
        } else if tool_name.starts_with("storage.") {
            let cancel = tokio_util::sync::CancellationToken::new();
            let request_id = crate::request_context::current_request_id();
            tools::storage::execute_with_writer(
                tool_name,
                params,
                pool,
                self.storage_writer_heartbeat
                    .as_ref()
                    .map(|heartbeat| heartbeat.as_ref()),
                principal,
                cancel,
                request_id.as_deref(),
            )
            .await
        } else if tool_name.starts_with("agents.") {
            // Box::pin isolates the agents future from dispatch_tool_internal's state machine,
            // preventing the large agents executor from inflating the state machine of all
            // non-agents paths (storage, runs, runtime, etc.) in debug builds.
            Box::pin(tools::agents::execute(
                tool_name,
                params,
                pool,
                principal,
                self.acp_runtime.as_deref(),
            ))
            .await
        } else if tool_name.starts_with("automation.") {
            tools::automation::execute(tool_name, params, principal).await
        } else if tool_name.starts_with("p080.") {
            tools::p080::execute(tool_name, params, pool, principal).await
        } else if tool_name.starts_with("temp_artifacts.") {
            tools::temp_artifacts::execute(tool_name, params, principal).await
        } else {
            Err(anyhow::anyhow!("Unknown tool namespace: {tool_name}"))
        }
    }
}

// ── SEC-MED-001: stdio line size cap ─────────────────────────────────────────

/// Hard per-line byte cap for MCP stdio transport. Matches HTTP body limit so
/// both transports enforce the same maximum request size. (SEC-MED-001)
pub(crate) const MCP_STDIO_LINE_LIMIT_BYTES: usize = 256 * 1024;

/// Read one newline-terminated line from `reader` into `buf`, capped at `limit` bytes.
///
/// - `Ok(0)`:  EOF before any bytes were read.
/// - `Ok(n)`:  Successful read of `n` bytes (including the trailing `\n` if present).
/// - `Err(e)` with `e.kind() == InvalidData`: line exceeded `limit`; the oversized line
///   has been drained from the reader (up to the next `\n` or EOF) so the caller can
///   continue reading subsequent lines. `buf` is cleared before returning.
/// - `Err(e)` (any other kind): underlying I/O error; caller should propagate or close.
///
/// Unlike `AsyncBufReadExt::read_line`, this function never allocates more than
/// `limit + chunk` bytes, preventing memory exhaustion from unterminated oversized lines.
async fn stdio_read_line_limited<R>(
    reader: &mut R,
    buf: &mut String,
    limit: usize,
) -> std::io::Result<usize>
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    use tokio::io::AsyncBufReadExt as _;
    buf.clear();
    let mut total: usize = 0;

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(total); // EOF
        }

        let newline_pos = available.iter().position(|&b| b == b'\n');
        let chunk_end = newline_pos.map_or(available.len(), |p| p + 1);

        if total + chunk_end > limit {
            // Over limit: drain to the end of this line without appending to buf.
            let drain = chunk_end;
            reader.consume(drain);
            if newline_pos.is_none() {
                // Newline not in this chunk; keep draining until we find one.
                loop {
                    let a = reader.fill_buf().await?;
                    if a.is_empty() {
                        break;
                    }
                    let nl = a.iter().position(|&b| b == b'\n');
                    let d = nl.map_or(a.len(), |p| p + 1);
                    reader.consume(d);
                    if nl.is_some() {
                        break;
                    }
                }
            }
            buf.clear();
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("stdio line exceeds {limit} byte limit"),
            ));
        }

        // Safe to append: lossy UTF-8 so malformed bytes don't abort valid requests.
        match std::str::from_utf8(&available[..chunk_end]) {
            Ok(s) => buf.push_str(s),
            Err(_) => buf.push_str(&String::from_utf8_lossy(&available[..chunk_end])),
        }
        total += chunk_end;
        reader.consume(chunk_end);

        if newline_pos.is_some() {
            return Ok(total); // Complete line
        }
    }
}

// ── P081 AC-13: MCP command idempotency helpers ──────────────────────────────

/// Returns true if this tool is state-changing and requires an idempotency key.
/// P081 AC-13: all tools that perform durable DB or filesystem writes must be
/// listed here. storage.reconcile_evidence_orphans has a dry-run mode — see
/// `is_state_changing_call` which does param-aware classification for it.
///
/// NOTE: `agents.continue_work` is intentionally excluded. It uses the P086
/// atomic admission fence (`admit_continuation_atomic`) for its own idempotency
/// and does NOT return a `journal_id` in its accepted response. Routing it
/// through the generic MCP idempotency precheck would cause
/// `IDEMPOTENCY_JOURNAL_LINK_MISSING` errors after a successfully committed
/// continuation row. The P086 replay fence is the authoritative guard for that
/// tool. (SEC-HIGH-001)
fn is_state_changing_tool(tool_name: &str) -> bool {
    matches!(
        tools::canonical_tool_name(tool_name),
        "runs.start"
            | "runs.main_sync.request"
            | "runs.main_sync.retry"
            | "runs.main_sync.set_override"
            | "runs.main_sync.repair_state"
            | "runs.main_sync.record_recovery_decision"
            | "runs.knowledge_capsule.ignore"
            | "runs.resume_escalation_deadline"
            | "runs.resume_escalation_chain"
            | "runs.settle_proposal_gate"
            | "ideas.create"
            | "stages.consume_provider_quota_hold"
            | "legacy_discovery_override_create"
            | "workflow_conflicts.resolve"
            | "workflow_loop_budget.extend"
            | "artifacts.override_contract"
            | "steward.run_analysis"
            | "effects.mark_conflict"
            | "effects.mark_unrecoverable"
            | "effects.clear_after_manual_verification"
            | "storage.maintenance.repair_slot"
            | "storage.projections.clear_backlog"
            | "storage.projections.clear_poison" // runs.cancel, stages.retry, approvals.resolve use is_p083_command_idempotency_tool.
    )
}

/// Returns true if this tool uses the P083 command_idempotency_contract_v1 (request_id /
/// CallerRequestId UUIDv4) instead of the P081 idempotency_key (UUIDv7). These tools are
/// state-changing and subject to audit-budget checks, but they must bypass the P081
/// mcp_idempotency_precheck because that precheck requires idempotency_key, which is not
/// present in the P083 tool schemas (additionalProperties=false). Each P083 tool handler
/// validates request_id and acquires its own command_idempotency lease.
/// SEC-P083-LOW-002: separating classification prevents P083 MCP callers from being
/// denied with IDEMPOTENCY_KEY_REQUIRED when following the published request_id schema.
fn is_p083_command_idempotency_tool(tool_name: &str) -> bool {
    matches!(
        tools::canonical_tool_name(tool_name),
        "provider_session.shutdown"
            | "provider_session.mark_process_absent"
            | "p083.rollback_execution"
            | "p083.set_enforcement_mode"
            | "runs.retry"
            | "runs.cancel"
            | "stages.retry"
            | "approvals.resolve"
            | "side_effects.force_reconcile"
    )
}

/// Returns true if this call is state-changing given the supplied parameters.
/// Wraps `is_state_changing_tool` and adds param-aware classification for tools
/// that have both a read-only (dry-run) and a mutating (live) execution mode.
fn is_state_changing_call(tool_name: &str, params: &serde_json::Value) -> bool {
    if is_state_changing_tool(tool_name) {
        return true;
    }
    // P083 tools are state-changing for audit-budget purposes even though they bypass P081
    // idempotency precheck (they use command_idempotency_contract_v1 with request_id).
    if is_p083_command_idempotency_tool(tool_name) {
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
    // SEC-P080-MED-002 fix: p080.reconcile.request.v1 (repair_if_safe / hold) is NOT
    // classified as generic state-changing.  P080 uses operator_request_dedup_key
    // (not idempotency_key/idempotencyKey), so the generic UUID idempotency precheck
    // would always deny it with IDEMPOTENCY_KEY_REQUIRED before the P080 handler can
    // run its own auth-before-dedup contract.  The P080 handler enforces replay-fence
    // semantics via its own operator_request_dedup_key path; falling through to the
    // else branch here lets the handler do that without the generic precheck blocking it.
    // diagnose_only is handled by is_read_only_call below.
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
            // P080: diagnostics.get is unconditionally read-only.
            | "p080.diagnostics.get.v1"
            // P086: continuation readback tools are unconditionally read-only. (SEC-LOW-001)
            | "agents.continuation_status"
            | "agents.continuation_candidates"
            // P089: inventory preview is read-only advisory (no scanning or mutation).
            | "temp_artifacts.inventory.preview"
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
    // P080: reconcile.request with diagnose_only produces no durable writes.
    if tools::canonical_tool_name(tool_name) == "p080.reconcile.request.v1" {
        let action = params
            .get("requested_action")
            .and_then(|v| v.as_str())
            .unwrap_or("diagnose_only");
        return action == "diagnose_only";
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

fn resources_list_policy_transport(caller_class: &str) -> &'static str {
    if caller_class == "observer" {
        "mcp_tools_call"
    } else {
        "mcp_tools_list"
    }
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
        ResourceTemplateId::ChainworksRunTempArtifactInventory => {
            "chainworks://runs/{run_id}/temp-artifact-inventory"
        }
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
        ResourceTemplateId::ChainworksRunTempArtifactInventory => serde_json::json!({
            "uri": "chainworks://runs/{run_id}/temp-artifact-inventory",
            "name": "Temporary Artifact Inventory",
            "description": "Read-only advisory managed temporary artifact inventory for a run",
            "mimeType": "application/json"
        }),
    }
}

fn resource_template_definition_value(id: ResourceTemplateId) -> serde_json::Value {
    let mut value = resource_template_value(id);
    if let Some(obj) = value.as_object_mut() {
        if let Some(uri) = obj.remove("uri") {
            obj.insert("uriTemplate".to_string(), uri);
        }
    }
    value
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
        assert_eq!(
            resource_template_id_for_uri("chainworks://runs/run-1/temp-artifact-inventory"),
            Some(ResourceTemplateId::ChainworksRunTempArtifactInventory)
        );
        assert_eq!(resource_template_id_for_uri("workflow://wf-1"), None);
    }

    #[test]
    fn p089_temp_artifact_inventory_resource_template_uses_mcp_uri_template_key() {
        let value = resource_template_definition_value(
            ResourceTemplateId::ChainworksRunTempArtifactInventory,
        );
        assert_eq!(
            value["uriTemplate"],
            serde_json::json!("chainworks://runs/{run_id}/temp-artifact-inventory")
        );
        assert!(
            value.get("uri").is_none(),
            "resources/templates/list entries must use uriTemplate"
        );
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
            "runs.resume_escalation_deadline",
            "runs.resume_escalation_chain",
            "approvals.list",
            "approvals.resolve",
            "stages.retry",
            "stages.consume_provider_quota_hold",
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
    fn p080_codex_alias_names_are_identified_for_rollout_fail_closed_filtering() {
        for name in [
            "p080.diagnostics.get.v1",
            "p080_diagnostics_get_v1",
            "p080.reconcile.request.v1",
            "p080_reconcile_request_v1",
            "p080.clear_permanent_hold.v1",
            "p080_clear_permanent_hold_v1",
        ] {
            assert!(
                tools::canonical_tool_name(name).starts_with("p080."),
                "P080 tool alias must remain visible to rollout fail-closed filtering: {name}"
            );
        }
    }

    #[test]
    fn test_mcp_tools_list_filtered_for_agent() {
        let ag = Principal::new("ag", PrincipalClass::Agent);
        let names = tools_list_names_for(&ag);
        // SEC-001: Agents can create ideas, list and get runs, but cannot start runs
        // (runs.start supplies filesystem paths to the daemon — Operator-only).
        // Agents also cannot approve, cancel, retry, or enter the steward surface.
        for expected in ["ideas.create", "ideas.list", "runs.list", "runs.get"] {
            let expected = expected.replace('.', "_");
            assert!(names.contains(&expected), "agent missing {expected}");
        }
        for forbidden in [
            "runs.start", // SEC-001: Operator-only (supplies daemon-side filesystem paths)
            "runs.main_sync.request",
            "runs.main_sync.retry",
            "runs.main_sync.set_override",
            "runs.main_sync.repair_state",
            "runs.main_sync.record_recovery_decision",
            "runs.knowledge_capsule.ignore",
            "approvals.list",
            "approvals.resolve",
            "stages.retry",
            "stages.consume_provider_quota_hold",
            "legacy_discovery_override_create",
            "runs.cancel",
            "runs.resume_escalation_deadline",
            "runs.resume_escalation_chain",
            "steward.run_analysis",
            "steward.list_analyses",
            "steward.get_analysis",
            "reports.get", // SEC-HIGH-001: exposes file_path, evidence, rollout readback — Operator-only
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
            "runs.resume_escalation_deadline",
            "runs.resume_escalation_chain",
            "approvals.resolve",
            "stages.retry",
            "stages.consume_provider_quota_hold",
            "legacy_discovery_override_create",
            "steward.run_analysis",
            "reports.get", // SEC-HIGH-001: exposes file_path, evidence, rollout readback — Operator-only
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
        assert!(!tools_call_allowed(&ob, "runs.resume_escalation_deadline"));
        assert!(!tools_call_allowed(&ob, "runs.resume_escalation_chain"));
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
        // Agent must NOT see steward-analysis template, approvals inbox,
        // chainworks run stages/artifacts, or artifact:// / report:// (HIGH-001).
        for forbidden in [
            "steward-analysis://{analysis_id}",
            "chainworks://approvals/inbox",
            "chainworks://runs/{run_id}/stages",
            "chainworks://runs/{run_id}/artifacts",
            "chainworks://runs/{run_id}/temp-artifact-inventory",
            "artifact://{artifact_id}",
            "report://{run_id}",
        ] {
            assert!(!uris.contains(forbidden), "agent must not see {forbidden}");
        }

        // Observer sees the approvals inbox and the steward template but NOT
        // artifact:// or report:// (HIGH-001).
        let ob = Principal::new("ob", PrincipalClass::Observer);
        let ob_uris = resource_list_uris_for(&ob);
        assert!(ob_uris.contains("steward-analysis://{analysis_id}"));
        assert!(ob_uris.contains("chainworks://approvals/inbox"));
        assert!(
            !ob_uris.contains("artifact://{artifact_id}"),
            "observer must not see artifact://"
        );
        assert!(
            !ob_uris.contains("report://{run_id}"),
            "observer must not see report://"
        );
        assert!(
            !ob_uris.contains("chainworks://runs/{run_id}/temp-artifact-inventory"),
            "observer must not see temp artifact inventory resource"
        );
    }

    // ── resources/read denial ────────────────────────────────────────

    #[test]
    fn test_mcp_resources_read_denied_returns_not_found() {
        // Agent reading a steward-analysis URI: denial path produces
        // -32002 Resource not found in server.rs.
        let ag = Principal::new("ag", PrincipalClass::Agent);
        assert!(!resources_read_allowed(&ag, "steward-analysis://abc-123"));

        // Agent can read run:// and idea://.
        assert!(resources_read_allowed(&ag, "run://r-1"));
        assert!(resources_read_allowed(&ag, "idea://i-1"));

        // HIGH-001: artifact:// and report:// are Operator-only; Agent must be denied.
        assert!(!resources_read_allowed(&ag, "artifact://a-1"));
        assert!(!resources_read_allowed(&ag, "report://r-1"));

        // Observer also denied artifact:// and report://.
        let ob = Principal::new("ob", PrincipalClass::Observer);
        assert!(!resources_read_allowed(&ob, "artifact://a-1"));
        assert!(!resources_read_allowed(&ob, "report://r-1"));

        // Unknown URI scheme also denied.
        assert!(!resources_read_allowed(&ag, "bogus://1"));

        // Operator can read steward-analysis, artifact://, and report://.
        let op = Principal::new("op", PrincipalClass::Operator);
        assert!(resources_read_allowed(&op, "steward-analysis://abc-123"));
        assert!(resources_read_allowed(&op, "artifact://a-1"));
        assert!(resources_read_allowed(&op, "report://r-1"));
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
        artifact_contracts, artifacts, audit_log, command_journal, ideas, projections,
        rollout_contract_checks, runs, startup_repairs, steward, validation,
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

    #[test]
    fn p080_clear_permanent_hold_uses_p080_specific_dedup_not_generic_precheck() {
        assert!(
            !is_state_changing_tool("p080.clear_permanent_hold.v1"),
            "P080 clear_permanent_hold must reach the P080 handler before generic MCP idempotency"
        );
    }

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

    async fn persist_p082_startup_repair_readback(
        pool: &sqlx::SqlitePool,
        run_id: domain::ids::RunId,
    ) -> serde_json::Value {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let startup_repair_id = format!("p082-startup-repair-{run_id}");
        let summary = domain::recovery_matrix::build_startup_repair_summary(
            &startup_repair_id,
            "work-item-p082",
            "command-journal-p082",
            1,
            1,
            false,
            60_000,
            &now_str,
            false,
            None,
            "run",
        );
        let readback = domain::recovery_matrix::set_readback_startup_repair(
            domain::recovery_matrix::build_readback_v1(
                "P082-R01",
                "repaired",
                "retry",
                domain::recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
                "Continue from the startup repair requeue.",
                "startup_repairs",
                "startup_repairs",
                &startup_repair_id,
                Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                "valid",
                &now_str,
            ),
            summary,
            Some("Startup repair requeued work once under P082 policy."),
        );
        assert!(
            domain::recovery_matrix::validate_readback_v1_shape(&readback),
            "test fixture must persist a valid P082 readback"
        );
        let notes = serde_json::json!({
            "p082_recovery_matrix_readback": readback.clone(),
        })
        .to_string();
        startup_repairs::record(
            pool,
            &startup_repair_id,
            &run_id.to_string(),
            "startup_requeue_once",
            now,
            Some(&notes),
        )
        .await
        .unwrap();
        readback
    }

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed");
        let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer)
            .await
            .expect("register shared DbWriter for test pool");
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
            diagnostic_artifact_paths: vec![],
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
    async fn p082_report_resource_includes_plural_readbacks_not_singular() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let expected = persist_p082_startup_repair_readback(&pool, run_id).await;
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

        assert!(
            report.contains_key("p082_recovery_matrix_readbacks"),
            "report:// must expose the plural P082 readback lane"
        );
        assert!(
            !report.contains_key("p082_recovery_matrix_readback"),
            "report:// must not expose the legacy singular P082 field"
        );
        assert_eq!(
            report["p082_recovery_matrix_readbacks"][0]["source_identifier"],
            expected["source_identifier"]
        );
    }

    #[tokio::test]
    async fn p082_report_resource_non_empty_readbacks_when_startup_repair_exists() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_p082_startup_repair_readback(&pool, run_id).await;
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
        let readbacks = value["p082_recovery_matrix_readbacks"]
            .as_array()
            .expect("P082 readbacks array");

        assert!(
            readbacks
                .iter()
                .any(|readback| readback["scenario_id"] == serde_json::json!("P082-R01")),
            "report:// must surface startup-repair P082 readbacks for operators"
        );
    }

    #[tokio::test]
    async fn p082_report_resource_run_report_artifact_empty_for_non_operator() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_p082_startup_repair_readback(&pool, run_id).await;
        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_9_implementation_reviewed".into(),
            agent_id: "implementation_auditor".into(),
            name: "run_report".into(),
            contract_id: "run_report".into(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/p082-run-report.json".into(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("run_report".into()),
            report_version: Some(1),
            agent_execution_id: None,
        };

        let value = tools::reports::artifact_report_json(
            &pool,
            &artifact,
            Some(&serde_json::json!({"schema_version": "operator_readback_v1"})),
            &auth::PrincipalClass::Agent,
        )
        .await
        .unwrap();

        assert_eq!(
            value["p082_recovery_matrix_readbacks"],
            serde_json::json!([]),
            "non-Operator run_report artifact lane must not expose P082 operator readbacks"
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

        // SEC-P081: evidence file must reside inside the run's artifact_root.
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
            let tool_value = match crate::tools::reports::execute(
                "reports.get",
                serde_json::json!({ "run_id": run_id }),
                &pool,
                &handler,
                &auth::Principal::new("operator", auth::PrincipalClass::Operator),
            )
            .await
            {
                Ok(v) => v,
                Err(e) if e.to_string().contains("Run not found") => {
                    eprintln!(
                        "P041 MCP readback: skipping fixture '{fixture_id}' — \
                         run {run_id} not found in fixture DB (stale parity DB, \
                         re-run engine integration tests): {e}"
                    );
                    continue;
                }
                Err(e) => panic!("P041 reports.get failed for fixture {fixture_id}: {e}"),
            };
            let server =
                McpServer::new(pool.clone(), handler, auth::PrincipalTable::test_fixture());
            let resource_value = match server.read_resource(&format!("report://{}", run_id)).await {
                Ok(v) => v,
                Err(e)
                    if e.to_string().contains("Run not found")
                        || e.to_string().contains("not found") =>
                {
                    eprintln!(
                        "P041 MCP readback: skipping fixture '{fixture_id}' resource read — \
                         run {run_id} not found in fixture DB (stale parity DB): {e}"
                    );
                    continue;
                }
                Err(e) => panic!("P041 read_resource failed for fixture {fixture_id}: {e}"),
            };
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

    #[test]
    fn provider_quota_retry_after_is_a_typed_mcp_error() {
        let retry_after = chrono::DateTime::parse_from_rfc3339("2026-09-01T02:40:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let error = anyhow::Error::new(engine::command_handler::ProviderQuotaRetryAfterError {
            stage_execution_id: domain::ids::StageExecutionId::new(),
            retry_after,
        });

        let response = provider_quota_retry_after_response(
            Some(serde_json::json!(1)),
            "stages.retry",
            Some("request-123"),
            &error,
        )
        .expect("provider quota retry wait must have a typed MCP response");

        let error = response.error.expect("typed response must be an MCP error");
        assert_eq!(error.code, -32077);
        assert_eq!(error.message, "provider quota retry window has not elapsed");
        let data = error.data.expect("typed response must include error data");
        assert_eq!(data["code"], "PROVIDER_QUOTA_RETRY_AFTER");
        assert_eq!(data["tool_name"], "stages.retry");
        assert_eq!(data["retry_after"], "2026-09-01T02:40:00+00:00");
        assert_eq!(data["request_id"], "request-123");
        assert!(
            data.get("stage_execution_id").is_none(),
            "MCP response must not expose internal stage identifiers"
        );
    }

    fn observer_principal() -> auth::Principal {
        auth::Principal::new("test-observer", auth::PrincipalClass::Observer)
    }

    async fn force_p081_audit_budget_safe_mode(pool: &sqlx::SqlitePool) {
        let now_ms = Utc::now().timestamp_millis();
        let payload = "x".repeat(16_100);
        let entry = audit_log::AuditEntry {
            id: "p081-mcp-budget-safe-mode",
            request_id: "p081-mcp-budget-safe-mode",
            timestamp_ms: now_ms,
            event_type: "policy_denied",
            principal_id: Some("test-operator"),
            principal_class: Some("operator"),
            caller_class: Some("ui_operator"),
            token_id: None,
            transport: "mcp_tools_call",
            action_attempted: "ideas.create",
            decision: "deny",
            denial_reason_code: None,
            row_id: Some("p081.audit_budget.safe_mode"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "enforce",
            fixture_version: "p081-boundary-matrix-v1",
            payload: &payload,
            original_payload_bytes: None,
            diagnostic_truncated: false,
            checkpoint_id: None,
            created_at_ms: now_ms,
        };
        audit_log::append(pool, &entry).await.unwrap();
        let health = audit_log::health_snapshot(pool).await.unwrap();
        assert_eq!(health.payload_budget_state, "read_only_safe_mode");
    }

    #[tokio::test]
    async fn proposal_081_observer_resources_list_matches_compact_read_matrix_row() {
        let pool = test_pool().await;
        let policy = Arc::new(
            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                auth::boundary::PolicyMode::Enforce,
            )
            .unwrap(),
        );
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        )
        .with_boundary_policy(policy);

        let response = server
            .handle_request(
                JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(serde_json::json!(1)),
                    method: "resources/list".to_string(),
                    params: Some(serde_json::json!({})),
                },
                &observer_principal(),
            )
            .await;

        assert!(
            response.error.is_none(),
            "observer resources/list must be governed by p081.observer.mcp_tools_call.compact_read: {:?}",
            response.error
        );
        let resources = response.result.expect("resources/list result")["resources"]
            .as_array()
            .cloned()
            .expect("resources/list returns resource array");
        assert!(
            resources
                .iter()
                .any(|resource| resource["uri"] == "chainworks://approvals/inbox"),
            "observer compact resources/list must still expose observer-readable resources"
        );
    }

    #[tokio::test]
    async fn proposal_081_audit_budget_safe_mode_denies_state_changing_mcp_call() {
        let pool = test_pool().await;
        force_p081_audit_budget_safe_mode(&pool).await;
        let policy = Arc::new(
            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                auth::boundary::PolicyMode::Enforce,
            )
            .unwrap(),
        );
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        )
        .with_boundary_policy(policy);

        let response = server
            .handle_request(
                JsonRpcRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(serde_json::json!(81)),
                    method: "tools/call".to_string(),
                    params: Some(serde_json::json!({
                        "name": "ideas.create",
                        "arguments": {
                            "title": "blocked by audit budget",
                            "body": "must not commit",
                            "idempotency_key": "01900000-0000-7000-8081-000000000001"
                        }
                    })),
                },
                &operator_principal(),
            )
            .await;

        let error = response
            .error
            .expect("audit budget safe mode must deny state-changing call");
        assert_eq!(error.code, -32004);
        let data = error.data.expect("policy denial data");
        assert_eq!(data["reason_code"], "AUDIT_BUDGET_EXHAUSTED");
        assert_eq!(data["row_id"], "p081.audit_budget.safe_mode");
        let ideas: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ideas")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(ideas, 0, "denied MCP call must not mutate domain state");
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
            serde_json::json!({
                "run_id": run_id.to_string(),
                "caller_request_id": uuid::Uuid::new_v4().to_string(),
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
                .is_some_and(|n| n >= 1),
            "expected at least 1 liveness gate sample after tools/list call"
        );
        assert!(
            health["readPath"]["mcpLivenessGate"]["mcp_liveness_gate_duration_ms"]
                .as_u64()
                .is_some()
        );
    }

    #[tokio::test]
    async fn p080_tools_list_omits_codex_aliases_when_rollout_control_unreadable() {
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
        let tools = response.result.expect("tools/list result")["tools"]
            .as_array()
            .expect("tools/list returns array")
            .clone();
        let names: Vec<String> = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str().map(str::to_string))
            .collect();

        assert!(
            names
                .iter()
                .all(|name| !tools::canonical_tool_name(name).starts_with("p080.")),
            "P080 tools must fail closed when rollout-control is unreadable: {names:?}"
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
    async fn proposal_096_runtime_health_includes_tool_output_guard() {
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
        let guard = &payload["toolOutputGuard"];

        assert_eq!(guard["status"], "available");
        assert_eq!(guard["policyReadback"]["status"], "available");
        assert_eq!(guard["enforcement"]["status"], "configured");
        assert_eq!(
            guard["enforcement"]["pathStrategy"],
            "runtime_home_bin_prepend"
        );
        assert_eq!(
            guard["enforcement"]["activeProbeStatus"],
            "not_run_by_runtime_health"
        );
        assert_eq!(
            guard["policyVersion"],
            domain::tool_policy::TOOL_POLICY_VERSION
        );
        assert_eq!(
            guard["guardVersion"],
            domain::tool_policy::TOOL_GUARD_VERSION
        );
        assert_eq!(
            guard["maxOutputBytes"].as_u64(),
            Some(domain::tool_policy::DEFAULT_TOOL_OUTPUT_MAX_BYTES)
        );
        assert_eq!(
            guard["maxOutputLines"].as_u64(),
            Some(domain::tool_policy::DEFAULT_TOOL_OUTPUT_MAX_LINES)
        );
        assert_eq!(
            guard["maxCumulativeOutputBytes"].as_u64(),
            Some(domain::tool_policy::DEFAULT_CUMULATIVE_TOOL_OUTPUT_MAX_BYTES)
        );

        let denylist = guard["generatedRootDenylist"]
            .as_array()
            .expect("toolOutputGuard.generatedRootDenylist must be an array");
        for required in domain::tool_policy::GENERATED_ROOT_DENYLIST {
            assert!(
                denylist
                    .iter()
                    .any(|value| value.as_str() == Some(required)),
                "toolOutputGuard.generatedRootDenylist missing {required}"
            );
        }
    }

    #[tokio::test]
    async fn proposal_081_runtime_health_includes_boundary_runtime_readback() {
        let pool = test_pool().await;
        let policy = Arc::new(
            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                auth::boundary::PolicyMode::Enforce,
            )
            .unwrap(),
        );
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        )
        .with_boundary_policy(policy);

        let payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "runtime.health",
            serde_json::json!({}),
        )
        .await;
        let boundary = &payload["boundaryRuntime"];

        assert_eq!(boundary["schemaVersion"], "boundary_runtime.v1");
        assert_eq!(boundary["matrixId"], "p081-boundary-matrix-v1");
        assert_eq!(boundary["policyInjected"], true);
        assert_eq!(boundary["policyMode"], "enforce");
        assert_eq!(boundary["safeModeActive"], false);
        assert_eq!(
            boundary["auditLogHealth"]["schemaVersion"],
            "audit_log_health.v1"
        );
        assert!(boundary["auditLogHealth"]["rowCount"].as_i64().is_some());
        assert_eq!(boundary["auditLogHealth"]["writable"], true);
        assert_eq!(boundary["auditLogHealth"]["retentionMinDays"], 90);
        assert!(boundary["auditLogHealth"]["cleanupState"]
            .as_str()
            .is_some());
        assert!(boundary["auditLogHealth"]["cleanupEligibleRowCount"]
            .as_i64()
            .is_some());
        assert!(boundary["auditLogHealth"]["cleanupProtectedRowCount"]
            .as_i64()
            .is_some());
        assert!(boundary["auditLogHealth"]["payloadBudgetBytes"]
            .as_i64()
            .is_some());
        assert!(boundary["auditLogHealth"]["payloadUsedBytes"]
            .as_i64()
            .is_some());
        assert_eq!(
            boundary["auditLogHealth"]["shadowCoverageReportRef"],
            "docs/evidence/boundary-policy-shadow-coverage/report.json"
        );
        assert!(boundary["auditLogHealth"]["integrityState"]
            .as_str()
            .is_some());
        assert!(
            boundary.get("rows").is_none(),
            "runtime.health must expose bounded audit health, not raw audit rows"
        );

        let snake_payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "boundary.runtime.get",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(snake_payload["schema_version"], "boundary_runtime.v1");
        assert_eq!(snake_payload["matrix_id"], "p081-boundary-matrix-v1");
        assert_eq!(snake_payload["policy_injected"], true);
        assert_eq!(
            snake_payload["audit_log_health"]["schema_version"],
            "audit_log_health.v1"
        );
        assert!(snake_payload["auditLogHealth"].is_null());
        assert_eq!(
            snake_payload["subscription_replay"]["sequence_cursor"],
            "seq-0"
        );
    }

    #[tokio::test]
    async fn proposal_081_operator_alerts_list_exposes_safe_mode_alert() {
        let pool = test_pool().await;
        let policy = Arc::new(
            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                auth::boundary::PolicyMode::ReadOnlySafeMode,
            )
            .unwrap(),
        );
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        )
        .with_boundary_policy(policy);

        let payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "operator.alerts.list",
            serde_json::json!({}),
        )
        .await;

        assert_eq!(payload["schemaVersion"], "operator_alerts_readback_v1");
        let alerts = payload["alerts"].as_array().expect("alerts array");
        let safe_mode = alerts
            .iter()
            .find(|alert| alert["dedupeKey"] == "p081.boundary.safe_mode_active")
            .expect("safe-mode alert must be present");
        assert_eq!(safe_mode["schemaVersion"], "operator_alert_v1");
        assert_eq!(safe_mode["severity"], "critical");
        assert_eq!(safe_mode["active"], true);
        assert_eq!(safe_mode["silenceable"], false);
        assert_eq!(safe_mode["lifecycle"]["state"], "active_unacknowledged");
        assert_eq!(
            safe_mode["nativeDelivery"]["dedupePolicy"],
            "dedupe_key_until_clear"
        );
        assert_eq!(
            safe_mode["boundaryRuntime"]["safeModeActive"],
            serde_json::Value::Bool(true)
        );
        assert!(
            !payload.to_string().contains("\"rows\""),
            "operator.alerts.list must not expose raw audit rows"
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
        assert!(
            tools.iter().any(|tool| tool["name"] == "boundary_runtime_get"),
            "operator tools/list must expose P081 boundary.runtime.get as Codex-compatible boundary_runtime_get"
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

    #[tokio::test]
    async fn p081_ideas_create_records_command_journal_and_idempotency_linkage() {
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let key = uuid::Uuid::now_v7().to_string();

        let payload = call_tool_and_parse(
            &server,
            &operator_principal(),
            "ideas.create",
            serde_json::json!({
                "title": "P081 journaled idea",
                "body": "state-changing MCP calls must be journaled",
                "idempotency_key": key,
            }),
        )
        .await;

        let journal_id = payload["journal_id"]
            .as_str()
            .expect("ideas.create must return journal_id after command settlement");
        assert!(
            payload["idea"]["id"].as_str().is_some(),
            "ideas.create must still return the created idea payload"
        );

        let row: (String, String, String, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT command_type, result_status, mcp_idempotency_key, boundary_row_id, caller_class \
             FROM command_journal WHERE id = ?1",
        )
        .bind(journal_id)
        .fetch_one(&pool)
        .await
        .expect("journal row must exist");
        assert_eq!(row.0, "CreateIdea");
        assert_eq!(row.1, "completed");
        assert_eq!(row.2, key);
        assert_eq!(row.4.as_deref(), Some("agent_operator"));

        let idempotency: (String, Option<String>) = sqlx::query_as(
            "SELECT result_json, command_journal_id FROM mcp_command_idempotency \
             WHERE idempotency_key = ?1",
        )
        .bind(&key)
        .fetch_one(&pool)
        .await
        .expect("idempotency row must exist");
        assert_eq!(
            idempotency.1.as_deref(),
            Some(journal_id),
            "idempotency record must link back to the command_journal row"
        );
        assert_ne!(
            idempotency.0,
            db::repos::mcp_command_idempotency::PENDING_SENTINEL,
            "idempotency record must be committed, not left pending"
        );
    }

    #[tokio::test]
    async fn p081_ideas_create_idempotency_replay_does_not_duplicate_command_commit() {
        let pool = test_pool().await;
        let server = McpServer::new(
            pool.clone(),
            make_command_handler(pool.clone()),
            auth::PrincipalTable::test_fixture(),
        );
        let key = uuid::Uuid::now_v7().to_string();
        let args = serde_json::json!({
            "title": "P081 replayed idea",
            "body": "same request must not duplicate durable writes",
            "idempotency_key": key,
        });

        let first =
            call_tool_and_parse(&server, &operator_principal(), "ideas.create", args.clone()).await;
        let second =
            call_tool_and_parse(&server, &operator_principal(), "ideas.create", args).await;

        assert_eq!(
            first["idea"]["id"], second["idea"]["id"],
            "same idempotency key + same request must replay the original result"
        );
        let journal_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM command_journal WHERE mcp_idempotency_key = ?1")
                .bind(&key)
                .fetch_one(&pool)
                .await
                .expect("journal count");
        assert_eq!(
            journal_count.0, 1,
            "idempotency replay must not append a second command_journal row"
        );
        let idea_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM ideas WHERE title = 'P081 replayed idea'")
                .fetch_one(&pool)
                .await
                .expect("idea count");
        assert_eq!(
            idea_count.0, 1,
            "idempotency replay must not duplicate domain writes"
        );
    }

    #[test]
    fn p081_canonical_request_hash_sorts_nested_argument_objects() {
        let left = serde_json::json!({
            "idempotency_key": uuid::Uuid::now_v7().to_string(),
            "projectionName": "runs_home",
            "nested": {
                "b": 2,
                "a": 1,
                "array": [
                    { "z": true, "a": false }
                ]
            }
        });
        let right = serde_json::json!({
            "projectionName": "runs_home",
            "nested": {
                "array": [
                    { "a": false, "z": true }
                ],
                "a": 1,
                "b": 2
            },
            "idempotencyKey": uuid::Uuid::now_v7().to_string()
        });

        assert_eq!(
            compute_canonical_request_hash(
                "storage.projections.clear_backlog",
                &left,
                "agent_operator",
                "test-operator",
                "token:test",
                Some("p081.agent_operator.mcp_tools_call.command"),
            ),
            compute_canonical_request_hash(
                "storage.projections.clear_backlog",
                &right,
                "agent_operator",
                "test-operator",
                "token:test",
                Some("p081.agent_operator.mcp_tools_call.command"),
            ),
            "nested JSON key order and idempotency-key casing must not change the retry hash"
        );
    }

    #[tokio::test]
    async fn p081_storage_clear_backlog_claims_idempotency_in_write_unit() {
        let pool = test_pool().await;
        let principal = operator_principal();
        let key = uuid::Uuid::now_v7().to_string();
        let request_hash = "sha256:p081-storage-request";
        let row_id = "p081.agent_operator.mcp_tools_call.command";
        let args = serde_json::json!({
            "projectionName": "p081-test-projection",
            "sourceName": "p081-test-source",
        });

        let first = crate::request_context::scope_idempotency_key(
            Some(key.clone()),
            crate::request_context::scope_idempotency_request_hash(
                Some(request_hash.to_string()),
                crate::request_context::scope_boundary_row_id(
                    Some(row_id.to_string()),
                    tools::storage::execute_with_writer(
                        "storage.projections.clear_backlog",
                        args,
                        &pool,
                        None,
                        &principal,
                        tokio_util::sync::CancellationToken::new(),
                        Some("request-p081-storage"),
                    ),
                ),
            ),
        )
        .await
        .expect("storage projection clear must succeed");

        let journal_id = first["journal_id"]
            .as_str()
            .expect("storage projection clear must return journal_id");

        let journal_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM command_journal WHERE mcp_idempotency_key = ?1")
                .bind(&key)
                .fetch_one(&pool)
                .await
                .expect("journal count");
        assert_eq!(
            journal_count.0, 1,
            "storage write unit must append exactly one command_journal row"
        );

        let idempotency: (String, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT result_json, command_journal_id, row_id FROM mcp_command_idempotency \
             WHERE idempotency_key = ?1",
        )
        .bind(&key)
        .fetch_one(&pool)
        .await
        .expect("idempotency row must exist");
        assert_eq!(
            idempotency.0,
            db::repos::mcp_command_idempotency::PENDING_SENTINEL,
            "direct handler leaves final result update to the MCP server"
        );
        assert!(idempotency.1.is_none());
        assert_eq!(idempotency.2.as_deref(), Some(row_id));

        let result_json = serde_json::to_string(&first).unwrap();
        let committed = db::repos::mcp_command_idempotency::update_result(
            &pool,
            &key,
            &result_json,
            None,
            Some(journal_id),
        )
        .await
        .expect("server-side idempotency finalization must update pending row");
        assert!(committed);
    }

    #[tokio::test]
    async fn p081_idempotency_pending_sentinel_recovers_committed_unack_without_reexecution() {
        let pool = test_pool().await;
        let key = uuid::Uuid::now_v7().to_string();
        let journal_id = uuid::Uuid::new_v4().to_string();
        let row_id = "p081.agent_operator.mcp_tools_call.command";

        db::repos::mcp_command_idempotency::insert_pending(
            &pool,
            &key,
            "ideas.create",
            "test-operator",
            "hash-before-dispatch",
            Some(row_id),
        )
        .await
        .expect("pending idempotency preclaim");
        sqlx::query(
            "UPDATE mcp_command_idempotency SET committed_at_ms = ?1 WHERE idempotency_key = ?2",
        )
        .bind(Utc::now().timestamp_millis() - 60_000)
        .bind(&key)
        .execute(&pool)
        .await
        .expect("age pending sentinel past in-flight timeout");

        command_journal::record(
            &pool,
            &journal_id,
            "CreateIdea",
            r#"{"title":"committed before response"}"#,
            None,
            Utc::now(),
            Some("mcp"),
            Some("test-operator"),
            Some("operator"),
            Some("ideas.create"),
            Some("request-1"),
            Some("agent_operator"),
            Some(&key),
            Some(row_id),
        )
        .await
        .expect("journal record");
        command_journal::complete_entry(&pool, &journal_id, Utc::now())
            .await
            .expect("journal complete");

        let args = serde_json::json!({
            "title": "committed before response",
            "body": "retry must recover from journal instead of re-running",
            "idempotency_key": key,
        });
        let outcome = mcp_idempotency_precheck(
            &pool,
            Some(serde_json::json!(1)),
            "ideas.create",
            &args,
            &operator_principal(),
            Some(row_id),
        )
        .await;

        let IdempotencyOutcome::Cached(response) = outcome else {
            panic!("expected committed-unack cached recovery response");
        };
        assert!(response.error.is_none());
        let result = response.result.expect("success result");
        assert_eq!(result["_idempotency"], "committed_unack_recovery");
        let text = result["content"][0]["text"].as_str().expect("text result");
        let recovery: serde_json::Value = serde_json::from_str(text).expect("recovery json");
        assert_eq!(recovery["journal_id"], journal_id);

        let rows: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM command_journal WHERE mcp_idempotency_key = ?1")
                .bind(&key)
                .fetch_one(&pool)
                .await
                .expect("journal count");
        assert_eq!(rows.0, 1, "recovery must not create a second command row");
        let record = db::repos::mcp_command_idempotency::find_by_key(&pool, &key)
            .await
            .expect("idempotency lookup")
            .expect("idempotency record");
        assert_ne!(
            record.result_json,
            db::repos::mcp_command_idempotency::PENDING_SENTINEL,
            "recovery must replace the pending sentinel with a durable recovery result"
        );
        assert_eq!(
            record.command_journal_id.as_deref(),
            Some(journal_id.as_str())
        );
    }

    #[tokio::test]
    async fn p081_idempotency_storage_unavailable_fails_closed_with_sqlite_contention_code() {
        let pool = test_pool().await;
        pool.close().await;

        let outcome = mcp_idempotency_precheck(
            &pool,
            Some(serde_json::json!(1)),
            "ideas.create",
            &serde_json::json!({
                "title": "contention",
                "body": "closed pool simulates storage outage before dispatch",
                "idempotency_key": uuid::Uuid::now_v7().to_string(),
            }),
            &operator_principal(),
            Some("p081.agent_operator.mcp_tools_call.command"),
        )
        .await;

        let IdempotencyOutcome::Denied(response) = outcome else {
            panic!("storage outage must fail closed before dispatch");
        };
        let error = response.error.expect("error response");
        assert_eq!(error.code, -32603);
        let data = error.data.expect("structured error data");
        assert_eq!(data["code"], "SQLITE_CONTENTION_RETRY_EXHAUSTED");
    }
}
