//! MCP Streamable HTTP transport (MCP spec 2025-03-26).
//!
//! Single `/mcp` endpoint:
//! - POST: client sends JSON-RPC request, server responds with JSON or 202.
//! - Session tracked via `Mcp-Session-Id` header.
//!
//! This runs inside the daemon's axum router — same process, same SQLite pool.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use tracing::info;

use crate::protocol::JsonRpcRequest;
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
    body: String,
) -> Response {
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
                        );
                        return json_response(StatusCode::OK, &resp, None);
                    }
                },
                Err(_) => {
                    let resp = crate::protocol::JsonRpcResponse::error(
                        None,
                        -32000,
                        "unauthorized".to_string(),
                    );
                    return json_response(StatusCode::OK, &resp, None);
                }
            },
            None => {
                let resp = crate::protocol::JsonRpcResponse::error(
                    None,
                    -32000,
                    "unauthorized".to_string(),
                );
                return json_response(StatusCode::OK, &resp, None);
            }
        }
    };

    // Parse JSON-RPC request
    let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
        Ok(r) => r,
        Err(e) => {
            let resp =
                crate::protocol::JsonRpcResponse::error(None, -32700, format!("Parse error: {e}"));
            return json_response(StatusCode::OK, &resp, None);
        }
    };

    let is_initialize = request.method == "initialize";
    let is_notification =
        request.id.is_none() || matches!(&request.id, Some(serde_json::Value::Null));

    // For notifications (no id), return 202 Accepted
    if is_notification {
        let _ = mcp.handle_request(request, &principal).await;
        return StatusCode::ACCEPTED.into_response();
    }

    // Process the request
    let response = mcp.handle_request(request, &principal).await;

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
