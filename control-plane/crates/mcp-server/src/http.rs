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

pub const MCP_HTTP_BODY_LIMIT_BYTES: usize = 256 * 1024;

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

    // SEC-P080-HIGH-001: duplicate-key rejection at the raw-parse boundary for ALL
    // JSON-RPC requests. Runs before auth and before serde_json typed extraction so
    // unicode-escaped method/name values cannot bypass last-value-wins rejection.
    // The body-size cap (MCP_HTTP_BODY_LIMIT_BYTES) bounds the scan work.
    if let Some(dup_key) = find_duplicate_json_object_key(trimmed) {
        if dup_key == "__budget_exceeded__" {
            db::metrics::increment_counter_with_label(
                "p080_mcp_canonicalization_budget_exceeded_total",
                "scan_key_budget",
            );
            // SEC-P080-HIGH-001: budget exceeded means we cannot guarantee no duplicate keys
            // exist in the remaining payload. Reject so oversized payloads cannot bypass the
            // duplicate-key gate by exceeding SCAN_KEY_BUDGET (proposal lines 170-186).
            db::metrics::increment_counter_with_label(
                "p080_mcp_parser_rejected_total",
                "canonicalization_budget_exceeded",
            );
            let resp = crate::protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: Some(serde_json::json!({
                    "schema_version": "p080_error_response_v1",
                    "code": "canonicalization_budget_exceeded",
                    "message": "JSON request scan budget exceeded; request rejected to prevent duplicate-key bypass",
                    "retry_after": null,
                    "readback": null,
                    "detail": { "limit": SCAN_KEY_BUDGET, "observed": "budget_exceeded" }
                })),
                error: None,
            };
            return json_response(StatusCode::OK, &resp, None);
        } else {
            db::metrics::increment_counter_with_label(
                "p080_mcp_parser_rejected_total",
                "duplicate_key",
            );
            let resp = crate::protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: Some(serde_json::json!({
                    "schema_version": "p080_error_response_v1",
                    "code": "duplicate_key",
                    "message": "JSON request contains duplicate object key; request rejected",
                    "retry_after": null,
                    "readback": null,
                    "detail": { "limit": 1, "observed": 2, "duplicate_key": dup_key }
                })),
                error: None,
            };
            return json_response(StatusCode::OK, &resp, None);
        }
    }

    // ── Resolve principal from Authorization header ──────────────────────
    // SEC-P081-M002: derive token_id here before losing the raw token; only the
    // derived id (sha256 hex) is propagated downstream — the raw token never leaves.
    let (principal, resolved_token_id) = {
        let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
        match auth_header {
            Some(header_value) => match auth::extract_bearer_token(header_value) {
                Ok(token) => match mcp.resolve_current_bearer(token) {
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

/// SEC-P080-HIGH-001: retained for unit tests; the gate was removed (all requests
/// now go through find_duplicate_json_object_key regardless of tool name).
///
/// Returns true when the raw JSON looks like a P080 tools/call by substring match.
/// This function is NOT the authoritative security gate — see find_duplicate_json_object_key.
#[allow(dead_code)]
pub(crate) fn raw_looks_like_p080_tools_call(s: &str) -> bool {
    let trimmed = s.trim_start();
    if !trimmed.starts_with('{') {
        return false;
    }
    let mentions_tools_call = trimmed.contains(r#""method":"tools/call""#)
        || trimmed.contains(r#""method": "tools/call""#)
        || trimmed.contains(r#""method":"tools\/call""#)
        || trimmed.contains(r#""method": "tools\/call""#);
    // Match both "p080." (canonical) and "p080_" (Codex alias prefix) name forms.
    let mentions_p080_tool = trimmed.contains(r#""name":"p080."#)
        || trimmed.contains(r#""name": "p080."#)
        || trimmed.contains(r#""name":"p080_"#)
        || trimmed.contains(r#""name": "p080_"#);
    mentions_tools_call && mentions_p080_tool
}

/// Maximum total keys scanned across all nested objects before stopping early.
/// Prevents DoS from crafting JSON with unbounded unique keys. Body size limit
/// (MCP_HTTP_BODY_LIMIT_BYTES) is the primary defense; this is a secondary bound.
const SCAN_KEY_BUDGET: usize = 2048;

/// SEC-P080-MED-001: Scan raw JSON bytes for duplicate object keys at any nesting depth.
///
/// Returns `Some(key)` with the first duplicate key found, `None` if all object keys
/// are unique, or `Some("__budget_exceeded__")` when the key budget is exhausted.
/// Returns `None` on any parse ambiguity (conservative: prefers false negatives
/// over false positives so valid non-P080 requests are never incorrectly rejected).
///
/// This function is called at the raw-byte boundary BEFORE serde_json typed extraction,
/// so duplicate keys that would be silently collapsed by last-value-wins semantics are caught.
pub(crate) fn find_duplicate_json_object_key(s: &str) -> Option<String> {
    struct Scanner<'a> {
        b: &'a [u8],
        p: usize,
        keys_scanned: usize,
    }
    impl<'a> Scanner<'a> {
        fn peek(&self) -> Option<u8> {
            self.b.get(self.p).copied()
        }
        fn advance(&mut self) {
            if self.p < self.b.len() {
                self.p += 1;
            }
        }
        fn skip_ws(&mut self) {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.advance();
            }
        }
        /// SEC-P080-HIGH-002: read a JSON string with canonical Unicode decoding.
        /// `\uXXXX` sequences are decoded to the actual Unicode character (not
        /// replaced with U+FFFD), so `requested_action` and `requested_action`
        /// produce the same key string as serde_json does, closing the escape bypass.
        fn read_string(&mut self) -> Option<String> {
            if self.peek() != Some(b'"') {
                return None;
            }
            self.advance();
            let mut out = String::new();
            loop {
                match self.peek()? {
                    b'"' => {
                        self.advance();
                        return Some(out);
                    }
                    b'\\' => {
                        self.advance();
                        match self.peek()? {
                            b'"' | b'\\' | b'/' => {
                                out.push(self.b[self.p] as char);
                                self.advance();
                            }
                            b'n' => {
                                out.push('\n');
                                self.advance();
                            }
                            b'r' => {
                                out.push('\r');
                                self.advance();
                            }
                            b't' => {
                                out.push('\t');
                                self.advance();
                            }
                            b'b' => {
                                out.push('\x08');
                                self.advance();
                            }
                            b'f' => {
                                out.push('\x0C');
                                self.advance();
                            }
                            b'u' => {
                                self.advance();
                                match self.read_hex4() {
                                    None => out.push('\u{FFFD}'),
                                    Some(cp) if (0xD800..=0xDBFF).contains(&(cp as u32)) => {
                                        // High surrogate — try to consume the paired \uXXXX.
                                        let saved = self.p;
                                        let combined = if self.peek() == Some(b'\\') {
                                            self.advance();
                                            if self.peek() == Some(b'u') {
                                                self.advance();
                                                match self.read_hex4() {
                                                    Some(low)
                                                        if (0xDC00..=0xDFFF)
                                                            .contains(&(low as u32)) =>
                                                    {
                                                        let full = 0x10000u32
                                                            + ((cp as u32 - 0xD800) << 10)
                                                            + (low as u32 - 0xDC00);
                                                        char::from_u32(full)
                                                    }
                                                    _ => {
                                                        self.p = saved;
                                                        None
                                                    }
                                                }
                                            } else {
                                                self.p = saved;
                                                None
                                            }
                                        } else {
                                            None
                                        };
                                        out.push(combined.unwrap_or('\u{FFFD}'));
                                    }
                                    Some(cp) if (0xDC00..=0xDFFF).contains(&(cp as u32)) => {
                                        // Lone low surrogate — not valid standalone.
                                        out.push('\u{FFFD}');
                                    }
                                    Some(cp) => {
                                        out.push(char::from_u32(cp as u32).unwrap_or('\u{FFFD}'));
                                    }
                                }
                            }
                            _ => {
                                self.advance();
                                out.push('\u{FFFD}');
                            }
                        }
                    }
                    b => {
                        out.push(b as char);
                        self.advance();
                    }
                }
            }
        }

        /// Consume exactly 4 hex digits and return the decoded u16 value,
        /// or None if any digit is missing or not hexadecimal.
        fn read_hex4(&mut self) -> Option<u16> {
            let mut val: u16 = 0;
            for _ in 0..4 {
                let d: u16 = match self.peek()? {
                    c @ b'0'..=b'9' => (c - b'0') as u16,
                    c @ b'a'..=b'f' => (c - b'a') as u16 + 10,
                    c @ b'A'..=b'F' => (c - b'A') as u16 + 10,
                    _ => return None,
                };
                val = (val << 4) | d;
                self.advance();
            }
            Some(val)
        }
        fn skip_value(&mut self) {
            self.skip_ws();
            match self.peek() {
                Some(b'"') => {
                    let _ = self.read_string();
                }
                Some(b'{') => {
                    self.advance();
                    self.skip_object_body();
                }
                Some(b'[') => {
                    self.advance();
                    self.skip_array_body();
                }
                Some(b't' | b'f' | b'n') => {
                    while matches!(self.peek(), Some(b'a'..=b'z')) {
                        self.advance();
                    }
                }
                _ => {
                    while !matches!(
                        self.peek(),
                        None | Some(b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
                    ) {
                        self.advance();
                    }
                }
            }
        }
        fn skip_object_body(&mut self) {
            self.skip_ws();
            if self.peek() == Some(b'}') {
                self.advance();
                return;
            }
            loop {
                self.skip_ws();
                let _ = self.read_string();
                self.skip_ws();
                if self.peek() == Some(b':') {
                    self.advance();
                }
                self.skip_value();
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.advance();
                    }
                    Some(b'}') => {
                        self.advance();
                        return;
                    }
                    _ => return,
                }
            }
        }
        fn skip_array_body(&mut self) {
            self.skip_ws();
            if self.peek() == Some(b']') {
                self.advance();
                return;
            }
            loop {
                self.skip_value();
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.advance();
                    }
                    Some(b']') => {
                        self.advance();
                        return;
                    }
                    _ => return,
                }
            }
        }
        fn scan_value(&mut self) -> Option<String> {
            self.skip_ws();
            match self.peek()? {
                b'{' => {
                    self.advance();
                    self.scan_object_body()
                }
                b'[' => {
                    self.advance();
                    self.scan_array_body()
                }
                b'"' => {
                    let _ = self.read_string();
                    None
                }
                _ => {
                    self.skip_value();
                    None
                }
            }
        }
        fn scan_object_body(&mut self) -> Option<String> {
            self.skip_ws();
            if self.peek() == Some(b'}') {
                self.advance();
                return None;
            }
            let mut seen = std::collections::HashSet::new();
            loop {
                self.skip_ws();
                let key = self.read_string()?;
                self.keys_scanned += 1;
                if self.keys_scanned > SCAN_KEY_BUDGET {
                    // Budget exhausted: stop scanning, return sentinel.
                    return Some("__budget_exceeded__".to_string());
                }
                if !seen.insert(key.clone()) {
                    return Some(key);
                }
                self.skip_ws();
                if self.peek() == Some(b':') {
                    self.advance();
                }
                if let Some(dup) = self.scan_value() {
                    return Some(dup);
                }
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.advance();
                    }
                    Some(b'}') => {
                        self.advance();
                        return None;
                    }
                    _ => return None,
                }
            }
        }
        fn scan_array_body(&mut self) -> Option<String> {
            self.skip_ws();
            if self.peek() == Some(b']') {
                self.advance();
                return None;
            }
            loop {
                if let Some(dup) = self.scan_value() {
                    return Some(dup);
                }
                self.skip_ws();
                match self.peek() {
                    Some(b',') => {
                        self.advance();
                    }
                    Some(b']') => {
                        self.advance();
                        return None;
                    }
                    _ => return None,
                }
            }
        }
    }
    let mut scanner = Scanner {
        b: s.as_bytes(),
        p: 0,
        keys_scanned: 0,
    };
    scanner.scan_value()
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
    async fn sec_high_001_mcp_http_observes_live_principal_revocation() {
        let mcp = test_server().await;
        let live_source = mcp.live_principal_source();
        live_source.update(auth::PrincipalTable::test_fixture());

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            "Bearer test-token-xxxxxxxxxxxxxxxxxxxxx".parse().unwrap(),
        );
        let authorized = handle_mcp_post(
            State(mcp.clone()),
            headers.clone(),
            None,
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#.to_string(),
        )
        .await;
        let authorized_json = response_json(authorized).await;
        assert!(
            authorized_json.get("error").is_none(),
            "known bearer should authorize before revocation: {authorized_json}"
        );

        live_source.update(auth::PrincipalTable::test_fixture_with_class(
            "observer-token-xxxxxxxxxxxxxxxxxx",
            "test-operator",
            auth::PrincipalClass::Observer,
        ));
        let revoked = handle_mcp_post(
            State(mcp.clone()),
            headers.clone(),
            None,
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{}}"#.to_string(),
        )
        .await;
        let revoked_json = response_json(revoked).await;
        assert_eq!(revoked_json["error"]["code"], -32000);
        assert_eq!(revoked_json["error"]["message"], "unauthorized");

        live_source.update(auth::PrincipalTable::test_fixture_disabled_token(
            "test-token-xxxxxxxxxxxxxxxxxxxxx",
            "test-operator",
        ));
        let disabled = handle_mcp_post(
            State(mcp.clone()),
            headers.clone(),
            None,
            r#"{"jsonrpc":"2.0","id":3,"method":"initialize","params":{}}"#.to_string(),
        )
        .await;
        let disabled_json = response_json(disabled).await;
        assert_eq!(disabled_json["error"]["code"], -32000);
        assert_eq!(disabled_json["error"]["message"], "unauthorized");

        live_source.update(auth::PrincipalTable::test_fixture_with_class(
            "test-token-xxxxxxxxxxxxxxxxxxxxx",
            "test-operator",
            auth::PrincipalClass::Observer,
        ));
        let rescoped = handle_mcp_post(
            State(mcp),
            headers,
            None,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/list","params":{}}"#.to_string(),
        )
        .await;
        let rescoped_json = response_json(rescoped).await;
        assert!(
            rescoped_json.get("error").is_none(),
            "re-scoped bearer should remain valid with new class: {rescoped_json}"
        );
        let tools = rescoped_json["result"]["tools"]
            .as_array()
            .expect("tools/list result");
        assert!(
            !tools.iter().any(|tool| tool["name"] == "reports_get"),
            "re-scoped Observer bearer must not retain reports.get capability"
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

    #[tokio::test]
    async fn p080_http_rejects_duplicate_keys_before_jsonrpc_last_wins_parse() {
        let response = handle_mcp_post(
            State(test_server().await),
            operator_auth_header(),
            None,
            r#"{
                "jsonrpc":"2.0",
                "id":80,
                "method":"tools/call",
                "method":"initialize",
                "params":{
                    "name":"p080.diagnostics.get.v1",
                    "arguments":{
                        "schema_version":"p080_diagnostics_get_request_v1",
                        "schema_version":"p080_diagnostics_get_request_v1"
                    }
                }
            }"#
            .to_string(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(json["result"]["schema_version"], "p080_error_response_v1");
        assert_eq!(json["result"]["code"], "duplicate_key");
        assert_eq!(json["result"]["detail"]["duplicate_key"], "method");
    }

    #[tokio::test]
    async fn p080_http_rejects_duplicate_keys_before_auth_and_with_escaped_method() {
        let response = handle_mcp_post(
            State(test_server().await),
            HeaderMap::new(),
            None,
            r#"{
                "jsonrpc":"2.0",
                "id":80,
                "method":"tools\/call",
                "params":{
                    "name":"p080.diagnostics.get.v1",
                    "arguments":{
                        "schema_version":"p080_diagnostics_get_request_v1",
                        "schema_version":"p080_diagnostics_get_request_v1"
                    }
                }
            }"#
            .to_string(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(json["result"]["schema_version"], "p080_error_response_v1");
        assert_eq!(json["result"]["code"], "duplicate_key");
        assert_eq!(json["result"]["detail"]["duplicate_key"], "schema_version");
    }

    #[tokio::test]
    async fn duplicate_key_preflight_rejects_all_tool_calls_not_just_p080() {
        // SEC-P080-001 fix: duplicate-key rejection now applies to ALL JSON-RPC requests,
        // not only P080 tool calls. Non-P080 tools with duplicate keys must also be rejected.
        let response = handle_mcp_post(
            State(test_server().await),
            operator_auth_header(),
            None,
            r#"{
                "jsonrpc":"2.0",
                "id":80,
                "method":"tools/call",
                "params":{
                    "name":"runtime.health",
                    "arguments":{
                        "schema_version":"runtime_health_request_v1",
                        "schema_version":"runtime_health_request_v1"
                    }
                }
            }"#
            .to_string(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let json = response_json(response).await;
        assert_eq!(
            json["result"]["code"], "duplicate_key",
            "all tool calls with duplicate keys must now be rejected"
        );
    }

    #[test]
    fn p080_gate_matches_codex_underscore_alias_names() {
        // Codex alias p080_diagnostics_get_v1 → p080.diagnostics.get.v1 must trigger the scan.
        assert!(raw_looks_like_p080_tools_call(
            r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"p080_diagnostics_get_v1","arguments":{}}}"#
        ));
        assert!(raw_looks_like_p080_tools_call(
            r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"p080_reconcile_request_v1","arguments":{}}}"#
        ));
        assert!(raw_looks_like_p080_tools_call(
            r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"p080_clear_permanent_hold_v1","arguments":{}}}"#
        ));
        // Non-P080 tools must not trigger the scan.
        assert!(!raw_looks_like_p080_tools_call(
            r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"runtime.health","arguments":{}}}"#
        ));
    }

    #[test]
    fn p080_duplicate_key_scanner_canonicalizes_unicode_escaped_keys() {
        let dup = find_duplicate_json_object_key(
            r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"p080.diagnostics.get.v1","arguments":{"requested_action":"diagnose_only","requested\u005faction":"repair_if_safe"}}}"#,
        );

        assert_eq!(dup.as_deref(), Some("requested_action"));
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
