//! MCP Streamable HTTP transport (MCP spec 2025-03-26).
//!
//! Single `/mcp` endpoint:
//! - POST: client sends JSON-RPC request, server responds with JSON or 202.
//! - Session tracked via `Mcp-Session-Id` header.
//!
//! This runs inside the daemon's axum router — same process, same SQLite pool.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Extension, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use tracing::info;

use crate::protocol::JsonRpcRequest;
use crate::request_context;
use crate::server::McpServer;

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
    let principal = {
        let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
        match auth_header {
            Some(header_value) => match auth::extract_bearer_token(header_value) {
                Ok(token) => match auth::resolve_bearer(token, &mcp.principal_table) {
                    Ok(p) => p,
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

    // For notifications (no id), return 202 Accepted. The request-id
    // scope is still set so the downstream command journal INSERT can
    // carry the correlation id.
    if is_notification {
        let _ = request_context::scope_request_id(inbound_request_id.clone(), async {
            mcp.handle_request(request, &principal).await
        })
        .await;
        return StatusCode::ACCEPTED.into_response();
    }

    // Process the request inside the request-id scope so tool handlers
    // pick the id up via `request_context::mcp_caller`.
    //
    // `rid_for_errors` keeps a copy of the id around so we can stamp
    // the final JSON-RPC response with it AFTER the scope finishes
    // (scope_request_id consumes the Option by move).
    let rid_for_errors = inbound_request_id.clone();
    let response = request_context::scope_request_id(inbound_request_id, async {
        mcp.handle_request(request, &principal).await
    })
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
        headers.insert("authorization", "Bearer bad-token".parse().unwrap());
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
        // `PrincipalTable::test_fixture()` uses a stable `"test-token"`
        // string so integration callers don't have to scrape the
        // bootstrapped uuid. Re-using the literal here keeps this test
        // readable and avoids adding a new introspection API just for
        // error-envelope verification.
        headers.insert("authorization", "Bearer test-token".parse().unwrap());
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

    async fn test_server() -> Arc<McpServer> {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        db::writer::register_shared_writer(
            &pool,
            Arc::new(db::writer::DbWriter::new(pool.clone())),
        )
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

    async fn response_json(response: Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}
