//! MCP Streamable HTTP transport (MCP spec 2025-03-26).
//!
//! Single `/mcp` endpoint:
//! - POST: client sends JSON-RPC request, server responds with JSON or 202.
//! - Session tracked via `Mcp-Session-Id` header.
//!
//! This runs inside the daemon's axum router — same process, same SQLite pool.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use tracing::info;

use crate::protocol::JsonRpcRequest;
use crate::request_context;
use crate::server::McpServer;

pub const MCP_HTTP_BODY_LIMIT_BYTES: usize = 1024 * 1024;

/// Build the axum router for MCP HTTP transport.
///
/// Mount this on the daemon's main router:
/// ```ignore
/// let app = Router::new()
///     .merge(mcp_server::http::routes(mcp))
///     .route("/graphql", ...);
/// ```
pub fn routes(mcp: Arc<McpServer>) -> Router {
    Router::new()
        .route("/mcp", post(handle_mcp_post))
        .layer(DefaultBodyLimit::max(MCP_HTTP_BODY_LIMIT_BYTES))
        .with_state(mcp)
}

async fn handle_mcp_post(
    State(mcp): State<Arc<McpServer>>,
    headers: HeaderMap,
    // P042 §9.3: the axum request-id middleware attaches the id (either
    // the client-supplied safe value or a freshly minted UUID) to the
    // request Extensions. Reading it via `Option<Extension<RequestId>>`
    // picks up BOTH the client-supplied and the middleware-generated
    // IDs; reading the raw `x-request-id` inbound header would miss the
    // generated case because the middleware puts the ID on the
    // response, not the inbound request. MCP stdio never enters this
    // handler, so the extension is absent and the task-local resolves
    // to `None`.
    request_id: Option<Extension<graphql_server::request_id::RequestId>>,
    body: String,
) -> Response {
    let inbound_request_id = request_id.map(|Extension(r)| r.0);

    let trimmed = body.trim();
    if trimmed.is_empty() {
        return (StatusCode::BAD_REQUEST, "Empty body").into_response();
    }

    // ── Resolve principal from Authorization header ──────────────────────
    // SEC-P081-M002: derive token_id here before losing the raw token; only the
    // derived id (sha256 hex) is propagated downstream — the raw token never leaves.
    let (principal, resolved_token_id) = {
        let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
        match auth_header {
            Some(header_value) => match auth::extract_bearer_token(header_value) {
                Ok(token) => match mcp.principal_table.resolve_bearer(token) {
                    Ok(p) => {
                        let tid = auth::derive_token_id(token, &p.id);
                        (p, Some(tid))
                    }
                    Err(_) => {
                        let resp = crate::protocol::JsonRpcResponse::error(
                            None,
                            -32000,
                            "unauthorized".to_string(),
                        )
                        .with_error_request_id(inbound_request_id.as_deref());
                        return json_response(StatusCode::OK, &resp, None);
                    }
                },
                Err(_) => {
                    let resp = crate::protocol::JsonRpcResponse::error(
                        None,
                        -32000,
                        "unauthorized".to_string(),
                    )
                    .with_error_request_id(inbound_request_id.as_deref());
                    return json_response(StatusCode::OK, &resp, None);
                }
            },
            None => {
                let resp = crate::protocol::JsonRpcResponse::error(
                    None,
                    -32000,
                    "unauthorized".to_string(),
                )
                .with_error_request_id(inbound_request_id.as_deref());
                return json_response(StatusCode::OK, &resp, None);
            }
        }
    };

    // Parse JSON-RPC request
    let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
        Ok(r) => r,
        Err(e) => {
            let resp =
                crate::protocol::JsonRpcResponse::error(None, -32700, format!("Parse error: {e}"))
                    .with_error_request_id(inbound_request_id.as_deref());
            return json_response(StatusCode::OK, &resp, None);
        }
    };

    let is_initialize = request.method == "initialize";
    let is_notification =
        request.id.is_none() || matches!(&request.id, Some(serde_json::Value::Null));

    // For notifications (no id), return 202 Accepted. The request-id and
    // token-id scopes are set so the downstream command journal INSERT can
    // carry the correlation ids.
    if is_notification {
        let _ = request_context::scope_request_id(
            inbound_request_id.clone(),
            request_context::scope_token_id(resolved_token_id.clone(), async {
                mcp.handle_request(request, &principal).await
            }),
        )
        .await;
        return StatusCode::ACCEPTED.into_response();
    }

    // Process the request inside the request-id and token-id scopes so tool
    // handlers and audit writers pick both ids up via task-locals.
    //
    // `rid_for_errors` keeps a copy of the id around so we can stamp
    // the final JSON-RPC response with it AFTER the scope finishes
    // (scope_request_id consumes the Option by move).
    let rid_for_errors = inbound_request_id.clone();
    let response = request_context::scope_request_id(
        inbound_request_id,
        request_context::scope_token_id(resolved_token_id, async {
            mcp.handle_request(request, &principal).await
        }),
    )
    .await
    .with_error_request_id(rid_for_errors.as_deref());

    // On initialize, generate a session ID
    let session_id = if is_initialize {
        Some(uuid::Uuid::new_v4().to_string())
    } else {
        headers
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
    };

    if is_initialize {
        info!("MCP HTTP: new session initialized");
    }

    json_response(StatusCode::OK, &response, session_id.as_deref())
}

fn json_response(
    status: StatusCode,
    body: &crate::protocol::JsonRpcResponse,
    session_id: Option<&str>,
) -> Response {
    let json = serde_json::to_string(body).unwrap_or_default();

    let mut builder = Response::builder()
        .status(status)
        .header("content-type", "application/json");

    if let Some(sid) = session_id {
        builder = builder.header("mcp-session-id", sid);
    }

    builder
        .body(Body::from(json))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    use db::pool::create_pool;
    use engine::command_handler::CommandHandler;
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_mcp_http_rejects_missing_authorization_header() {
        let mcp = test_server().await;
        let response = handle_mcp_post(
            State(mcp),
            HeaderMap::new(),
            None,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(json["error"]["code"], -32000);
        assert_eq!(json["error"]["message"], "unauthorized");
    }

    #[tokio::test]
    async fn test_mcp_http_rejects_unknown_bearer_token() {
        let mcp = test_server().await;
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            "Bearer bad-token-xxxxxxxxxxxxxxxxxxxxxx".parse().unwrap(),
        );
        let response = handle_mcp_post(
            State(mcp),
            headers,
            None,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(json["error"]["code"], -32000);
        assert_eq!(json["error"]["message"], "unauthorized");
    }

    #[tokio::test]
    async fn p082_mcp_http_uses_live_principal_table_after_reload() {
        let mcp = test_server().await;

        for replacement in [
            auth::PrincipalTable::test_fixture_with_class(
                "observer-token-xxxxxxxxxxxxxxxxxx",
                "observer-after-reload",
                auth::PrincipalClass::Observer,
            ),
            auth::PrincipalTable::test_fixture_disabled_token(
                "test-token-xxxxxxxxxxxxxxxxxxxxx",
                "test-operator",
            ),
        ] {
            mcp.principal_table_handle().update(replacement);
            let response = handle_mcp_post(
                State(mcp.clone()),
                operator_auth_header(),
                None,
                r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
            )
            .await;
            let json = response_json(response).await;
            assert_eq!(
                json["error"]["message"], "unauthorized",
                "revoked/disabled startup snapshot token must be rejected after live reload"
            );
        }

        mcp.principal_table_handle()
            .update(auth::PrincipalTable::test_fixture_with_class(
                "test-token-xxxxxxxxxxxxxxxxxxxxx",
                "test-operator",
                auth::PrincipalClass::Observer,
            ));
        let response = handle_mcp_post(
            State(mcp.clone()),
            operator_auth_header(),
            None,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#.to_string(),
        )
        .await;
        let json = response_json(response).await;
        assert!(
            json["error"].is_null(),
            "observer-scoped token remains authenticated: {json}"
        );
        let tools = json["result"]["tools"].as_array().expect("tools array");
        assert!(
            !tools.iter().any(|tool| tool["name"] == "reports_get"),
            "re-scoped bearer must use current Observer capabilities, not startup Operator grants"
        );
    }

    /// R12 API-001 / AC-15: every error response MUST include the
    /// ambient request id in `error.data.request_id` so an operator
    /// pasting a failed MCP response can grep logs and
    /// `command_journal` for the same id.
    #[tokio::test]
    async fn test_mcp_http_error_includes_request_id_in_error_data() {
        let mcp = test_server().await;
        let response = handle_mcp_post(
            State(mcp),
            HeaderMap::new(),
            Some(Extension(graphql_server::request_id::RequestId(
                "rid-mcp-xyz".to_string(),
            ))),
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(json["error"]["code"], -32000, "unauthorized envelope");
        assert_eq!(
            json["error"]["data"]["request_id"], "rid-mcp-xyz",
            "error.data.request_id must carry the inbound correlation id"
        );
    }

    #[tokio::test]
    async fn test_mcp_http_parse_error_includes_request_id() {
        let mcp = test_server().await;
        let mut headers = HeaderMap::new();
        // `PrincipalTable::test_fixture()` uses a stable 32-char token
        // so integration callers don't have to scrape the bootstrapped uuid.
        // Re-using the literal here keeps this test readable and avoids
        // adding a new introspection API just for error-envelope verification.
        headers.insert(
            "authorization",
            "Bearer test-token-xxxxxxxxxxxxxxxxxxxxx".parse().unwrap(),
        );
        let response = handle_mcp_post(
            State(mcp),
            headers,
            Some(Extension(graphql_server::request_id::RequestId(
                "rid-parse-1".to_string(),
            ))),
            "not-json".to_string(),
        )
        .await;
        let json = response_json(response).await;
        assert_eq!(json["error"]["code"], -32700);
        assert_eq!(json["error"]["data"]["request_id"], "rid-parse-1");
    }

    #[tokio::test]
    async fn proposal_087_mcp_http_rejects_oversized_unauthenticated_body_before_auth() {
        let app = routes(test_server().await);
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/mcp")
            .body(Body::from("x".repeat(MCP_HTTP_BODY_LIMIT_BYTES + 1)))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn proposal_087_mcp_http_rejects_oversized_authenticated_body_before_parse() {
        let app = routes(test_server().await);
        let request = axum::http::Request::builder()
            .method("POST")
            .uri("/mcp")
            .header("authorization", "Bearer test-token")
            .body(Body::from("x".repeat(MCP_HTTP_BODY_LIMIT_BYTES + 1)))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    async fn test_server() -> Arc<McpServer> {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer)
            .await
            .unwrap();
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        let command_handler = Arc::new(CommandHandler::new(pool.clone(), events, work_queue));
        Arc::new(McpServer::new(
            pool,
            command_handler,
            auth::PrincipalTable::test_fixture(),
        ))
    }

    async fn test_server_with_boundary_policy() -> Arc<McpServer> {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let writer = std::sync::Arc::new(db::writer::DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer)
            .await
            .unwrap();
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        let command_handler = Arc::new(CommandHandler::new(pool.clone(), events, work_queue));
        // Enforce mode: Operator principals on MCP derive as agent_operator, which
        // the embedded fixture allows for all MCP transports (initialize/list/call).
        let policy = Arc::new(
            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                auth::boundary::PolicyMode::Enforce,
            )
            .expect("embedded fixture must be valid"),
        );
        Arc::new(
            McpServer::new(pool, command_handler, auth::PrincipalTable::test_fixture())
                .with_boundary_policy(policy),
        )
    }

    fn operator_auth_header() -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            "authorization",
            "Bearer test-token-xxxxxxxxxxxxxxxxxxxxx".parse().unwrap(),
        );
        h
    }

    async fn response_json(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    // ── P081 Phase 4: boundary_policy capability in initialize ─────────────

    /// matrix_row: p081.agent_operator.mcp_initialize.capability
    /// required_test: mcp_initialize_boundary_policy_capability
    #[tokio::test]
    async fn p081_mcp_initialize_exposes_boundary_policy_capability() {
        let mcp = test_server_with_boundary_policy().await;
        let response = handle_mcp_post(
            State(mcp),
            operator_auth_header(),
            None,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert!(json["error"].is_null(), "initialize must succeed: {json}");
        let bp = &json["result"]["capabilities"]["boundary_policy"];
        assert!(
            !bp.is_null(),
            "boundary_policy capability must be present in initialize result"
        );
        assert_eq!(bp["denied_known_tool_code"], -32004);
        assert_eq!(bp["field_casing"], "snake_case");
        assert_eq!(bp["capability_schema_version"], 1);
    }

    /// Legacy test constructors install the embedded shadow policy by default,
    /// so new call sites do not silently bypass the boundary service.
    #[tokio::test]
    async fn p081_mcp_initialize_default_constructor_exposes_shadow_boundary_capability() {
        let mcp = test_server().await;
        let response = handle_mcp_post(
            State(mcp),
            operator_auth_header(),
            None,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
        )
        .await;

        let json = response_json(response).await;
        assert!(json["error"].is_null(), "initialize must succeed: {json}");
        let bp = &json["result"]["capabilities"]["boundary_policy"];
        assert_eq!(bp["mode"], "shadow");
        assert_eq!(bp["denied_known_tool_code"], -32004);
    }

    // ── P081: known-but-denied tools must return -32004 ───────────────────

    /// matrix_row: p081.agent_operator.mcp_tools_call.command
    /// required_test: denied_out_of_scope_tool
    ///
    /// An operator principal with no tool capabilities but a valid token
    /// gets a known (registered) tool name → must receive -32004, not -32601.
    #[tokio::test]
    async fn p081_known_tool_denied_by_capability_returns_32004() {
        // Drive handle_request directly with a custom server+principal; we cannot
        // use test_server_with_boundary_policy because the HTTP handler resolves the
        // principal from the token and the test fixture operator is fully-capable.
        let policy = Arc::new(
            auth::boundary::BoundaryPolicy::from_embedded()
                .expect("embedded fixture must be valid"),
        );
        let server = Arc::new(
            McpServer::new(
                {
                    let pool = create_pool("sqlite::memory:").await.unwrap();
                    let writer = Arc::new(db::writer::DbWriter::new(pool.clone()));
                    db::writer::register_shared_writer(&pool, writer)
                        .await
                        .unwrap();
                    pool
                },
                Arc::new(CommandHandler::new(
                    create_pool("sqlite::memory:").await.unwrap(),
                    event_bus::new_bus(64),
                    WorkQueue::new(create_pool("sqlite::memory:").await.unwrap()),
                )),
                auth::PrincipalTable::test_fixture(),
            )
            .with_boundary_policy(policy),
        );

        // "runs.list" is a registered canonical tool name.
        // An agent_operator (Agent principal) is allowed by the matrix, but
        // this principal has tool capabilities explicitly cleared → capability check fails → -32004.
        let mut agent = auth::Principal::new("agent-no-caps", auth::PrincipalClass::Agent);
        agent.tool_capabilities.clear();
        let req = crate::protocol::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(42)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({ "name": "runs.list", "arguments": {} })),
        };
        let resp = server.handle_request(req, &agent).await;
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(
            json["error"]["code"], -32004,
            "known tool denied by capability must return -32004, got: {json}"
        );
    }

    /// matrix_row: none (unknown tool)
    /// Unknown tools that are not registered must still return -32601.
    #[tokio::test]
    async fn p081_unknown_tool_returns_32601_not_32004() {
        let mcp = test_server_with_boundary_policy().await;
        let operator = auth::Principal::new("op", auth::PrincipalClass::Operator);
        let req = crate::protocol::JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(99)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({ "name": "nonexistent.tool", "arguments": {} })),
        };

        // Borrow inner McpServer to call handle_request directly.
        let resp = mcp.handle_request(req, &operator).await;
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(
            json["error"]["code"], -32601,
            "unknown tool must still return -32601, got: {json}"
        );
    }
}
