//! Failed-serve mode (Proposal 042 §8.7).
//!
//! When migration preflight, PID lock (recoverable path), or crash-budget
//! exhaustion hits, the daemon MUST NOT exit before HTTP bind — AC-12
//! requires `/health`, `/ready`, and `daemonStatus` to report the
//! terminal failure. This module provides a minimal axum router that
//! binds the three read-only surfaces and refuses every other request
//! with a typed error envelope.
//!
//! The daemon's `main.rs` invokes [`serve_failed_state`] in place of the
//! normal startup path when a terminal `FailureKind` has been reported to
//! the lifecycle reporter; the function blocks until SIGTERM.

use std::net::SocketAddr;
use std::str::FromStr;

use anyhow::Result;
use axum::{
    extract::Extension,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{any, get, post},
    Router,
};
use engine::lifecycle_reporter::LifecycleReporter;
use tracing::{info, warn};

/// Minimal router for failed-serve mode (P042 §8.7). Serves `/health`,
/// `/ready`, a `daemonStatus`-only GraphQL passthrough on `/graphql`,
/// and a JSON-RPC–shaped `/mcp` refusal so ACP/MCP clients see the
/// protocol framing they expect. Every other route returns a typed
/// HTTP 503 error envelope.
pub fn build_failed_serve_router(
    reporter: LifecycleReporter,
    principal_table: auth::PrincipalTable,
) -> Router {
    Router::new()
        .route("/health", get(health_fallback))
        .route("/ready", get(ready_fallback))
        // §8.7 allows daemonStatus to resolve; other queries + all
        // mutations return the same refusal envelope as the catch-all.
        .route("/graphql", post(graphql_daemon_status_only))
        // R13 API-002 / §8.7: `/mcp` in failed-serve MUST return a
        // JSON-RPC–shaped `-32000` error, not a bare HTTP 503. ACP/MCP
        // clients expect `{jsonrpc, id, error: {code, message, data}}`
        // framing; dropping to the axum fallback below would give them
        // a plain JSON blob they can't parse as JSON-RPC.
        .route("/mcp", post(mcp_refuse_handler))
        // Catch-all for every non-status request. The exact path matching
        // axum uses means a more-specific route wins; this is the
        // last-resort refusal.
        .fallback(any(refuse_handler))
        .layer(Extension(reporter))
        // R13 API-001: GraphQL in failed-serve MUST still require a
        // bearer principal. `/health` and `/ready` stay unauthenticated
        // (P042 §5.2 loopback probes). The principal table is threaded
        // through so `graphql_daemon_status_only` can inspect the
        // `Authorization` header before honouring a `daemonStatus` read.
        .layer(Extension(principal_table))
}

async fn health_fallback(Extension(reporter): Extension<LifecycleReporter>) -> impl IntoResponse {
    let status = reporter.snapshot();
    (StatusCode::SERVICE_UNAVAILABLE, Json(status))
}

async fn ready_fallback(Extension(reporter): Extension<LifecycleReporter>) -> impl IntoResponse {
    let status = reporter.snapshot();
    (StatusCode::SERVICE_UNAVAILABLE, Json(status))
}

/// P042 §8.7 daemonStatus-only GraphQL passthrough. Inspects the request
/// body for a `daemonStatus` selection — if present, returns a GraphQL
/// `{"data":{"daemonStatus":{...}}}` envelope built from the in-process
/// snapshot. Any other operation returns the refusal envelope. This
/// avoids constructing a full async-graphql schema under a poisoned DB.
async fn graphql_daemon_status_only(
    Extension(reporter): Extension<LifecycleReporter>,
    Extension(principal_table): Extension<auth::PrincipalTable>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // R13 API-001: enforce the same bearer-principal contract as the
    // normal GraphQL route. An unauthenticated `/graphql` call in
    // failed-serve must not leak the failure detail / backup path.
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if auth::extract_bearer_token(auth_header)
        .ok()
        .and_then(|token| auth::resolve_bearer(token, &principal_table).ok())
        .is_none()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "unauthorized",
                "detail": "failed-serve /graphql requires a valid bearer principal",
            })),
        )
            .into_response();
    }

    let text = std::str::from_utf8(&body).unwrap_or_default();
    // Try parse as JSON `{"query": "..."}` (standard GraphQL HTTP shape).
    let query = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("query").and_then(|q| q.as_str()).map(str::to_string))
        .unwrap_or_else(|| text.to_string());

    // Refuse anything that could mutate, subscribe, or read non-status.
    let lower = query.to_lowercase();
    let has_daemon_status = query.contains("daemonStatus");
    let has_mutation = lower.contains("mutation");
    let has_subscription = lower.contains("subscription");
    let is_daemon_status_only = has_daemon_status && !has_mutation && !has_subscription;

    if !is_daemon_status_only {
        return refusal_response(reporter.snapshot());
    }

    // Build the daemonStatus envelope — mirrors graphql-server::GqlDaemonStatus
    // field names so a Swift client parses the same shape on the failed
    // and healthy paths.
    let snapshot = reporter.snapshot();
    let json_str = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_string());
    let body = serde_json::json!({
        "data": {
            "daemonStatus": {
                "state": snapshot.state.to_string(),
                "schemaVersion": snapshot.schema_version,
                "binarySchemaVersion": snapshot.binary_schema_version,
                "buildSha": snapshot.build_sha,
                "startedAt": snapshot.started_at.map(|t| t.to_rfc3339()),
                "lastStateChangeAt": snapshot.last_state_change_at.to_rfc3339(),
                "restartCountSinceBoot": snapshot.restart_count_since_boot,
                "pid": snapshot.pid,
                "json": json_str,
            }
        }
    });
    (StatusCode::OK, Json(body)).into_response()
}

async fn refuse_handler(Extension(reporter): Extension<LifecycleReporter>) -> impl IntoResponse {
    refusal_response(reporter.snapshot())
}

/// R13 API-002 / §8.7: JSON-RPC–shaped refusal for `/mcp`. Parses the
/// inbound body just far enough to echo the request id, then returns
/// the canonical `-32000` error with the failure detail in
/// `error.data`. Returns HTTP 200 because MCP HTTP clients treat the
/// protocol-level status via the JSON-RPC envelope rather than the
/// transport status code. Notifications (missing or null `id`) still
/// get 202 Accepted per JSON-RPC 2.0.
async fn mcp_refuse_handler(
    Extension(reporter): Extension<LifecycleReporter>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let parsed: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
    let id = parsed
        .as_ref()
        .and_then(|v| v.get("id"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let is_notification = id.is_null();
    if is_notification {
        return (StatusCode::ACCEPTED, ()).into_response();
    }

    let snapshot = reporter.snapshot();
    let failure = snapshot.failure.as_ref().map(|f| {
        serde_json::json!({
            "kind": f.kind.to_string(),
            "detail": f.detail,
            "backup_path": f.backup_path,
        })
    });
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32000,
            "message": "daemon_in_failed_state",
            "data": {
                "state": snapshot.state.to_string(),
                "failure": failure,
            }
        }
    });
    (StatusCode::OK, Json(body)).into_response()
}

fn refusal_response(status: domain::lifecycle::DaemonStatus) -> axum::response::Response {
    let failure = status
        .failure
        .as_ref()
        .map(|f| {
            serde_json::json!({
                "kind": f.kind.to_string(),
                "detail": f.detail,
                "backup_path": f.backup_path,
            })
        })
        .unwrap_or(serde_json::Value::Null);
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({
            "error": "daemon_in_failed_state",
            "failure": failure,
        })),
    )
        .into_response()
}

/// Bind the failed-serve router on `addr` and block until the listener
/// errors or SIGTERM is received. The caller should have already
/// transitioned the lifecycle to `Failed` via the reporter.
///
/// Prefer [`serve_failed_state_with_listener`] in the daemon startup
/// path — it lets the caller hand in a `TcpListener` produced by
/// `packaging::bind_with_fallback`, so the ephemeral-port `daemon.port`
/// write runs for the failed path too (P042 §7.3 / §8.7).
pub async fn serve_failed_state(
    reporter: LifecycleReporter,
    principal_table: auth::PrincipalTable,
    addr: &str,
) -> Result<()> {
    let sock_addr =
        SocketAddr::from_str(addr).unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 4000)));
    info!(
        addr = %sock_addr,
        state = %reporter.snapshot().state,
        "failed-serve mode: binding minimal status-only router"
    );
    let listener = tokio::net::TcpListener::bind(sock_addr).await?;
    serve_failed_state_with_listener(reporter, principal_table, listener).await
}

/// Same as [`serve_failed_state`] but accepts a pre-bound listener so
/// the daemon startup path can route the failed branch through the same
/// `packaging::bind_with_fallback` that writes `daemon.port`. This keeps
/// the Swift client's port discovery uniform across Ready and Failed
/// surfaces (P042 §7.3 / §8.7).
pub async fn serve_failed_state_with_listener(
    reporter: LifecycleReporter,
    principal_table: auth::PrincipalTable,
    listener: tokio::net::TcpListener,
) -> Result<()> {
    let addr = listener.local_addr().ok();
    let router = build_failed_serve_router(reporter.clone(), principal_table);
    info!(
        addr = ?addr,
        state = %reporter.snapshot().state,
        "failed-serve mode: serving minimal status-only router on pre-bound listener"
    );
    if let Err(e) = axum::serve(listener, router).await {
        warn!("failed-serve axum::serve exited: {e}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use domain::lifecycle::FailureKind;
    use engine::event_bus;
    use tower::util::ServiceExt;

    fn test_reporter_failed() -> LifecycleReporter {
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "test", bus);
        reporter.set_failed(
            FailureKind::MigrationFailed,
            "synthetic",
            Some("/tmp/db.backup".into()),
        );
        reporter
    }

    fn test_principal_table() -> auth::PrincipalTable {
        auth::PrincipalTable::test_fixture()
    }

    fn test_failed_router() -> Router {
        build_failed_serve_router(test_reporter_failed(), test_principal_table())
    }

    async fn call(router: Router, method: &str, path: &str) -> (StatusCode, serde_json::Value) {
        call_with_body(router, method, path, Body::empty()).await
    }

    async fn call_with_body(
        router: Router,
        method: &str,
        path: &str,
        body: Body,
    ) -> (StatusCode, serde_json::Value) {
        call_with_body_and_headers(router, method, path, body, &[]).await
    }

    async fn call_with_body_and_headers(
        router: Router,
        method: &str,
        path: &str,
        body: Body,
        extra_headers: &[(&str, &str)],
    ) -> (StatusCode, serde_json::Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json");
        for (name, value) in extra_headers {
            req = req.header(*name, *value);
        }
        let response = router.oneshot(req.body(body).unwrap()).await.unwrap();
        let code = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        (code, json)
    }

    #[tokio::test]
    async fn test_failed_serve_health_returns_503_with_failure() {
        let (code, body) = call(test_failed_router(), "GET", "/health").await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["state"], "failed");
        assert_eq!(body["failure"]["kind"], "migration_failed");
        assert_eq!(body["failure"]["backup_path"], "/tmp/db.backup");
    }

    #[tokio::test]
    async fn test_failed_serve_ready_returns_503_with_failure() {
        let (code, body) = call(test_failed_router(), "GET", "/ready").await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["state"], "failed");
    }

    #[tokio::test]
    async fn test_failed_serve_mutation_refused_with_typed_envelope() {
        // A POST /graphql with a mutation must be refused with the typed envelope.
        let mutation =
            serde_json::json!({"query": "mutation { startRun(ideaId: \"x\") { journalId } }"});
        let (code, body) = call_with_body_and_headers(
            test_failed_router(),
            "POST",
            "/graphql",
            Body::from(mutation.to_string()),
            &[("authorization", "Bearer test-token")],
        )
        .await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "daemon_in_failed_state");
        assert_eq!(body["failure"]["kind"], "migration_failed");
    }

    /// R13 API-001: unauthenticated GraphQL against failed-serve must
    /// return 401, NEVER the daemon-status envelope. Previously the
    /// handler answered `{data: {daemonStatus: …}}` to any caller,
    /// leaking failure detail + backup path.
    #[tokio::test]
    async fn test_failed_serve_graphql_rejects_missing_authorization() {
        let query = serde_json::json!({"query": "{ daemonStatus { state } }"});
        let (code, body) = call_with_body(
            test_failed_router(),
            "POST",
            "/graphql",
            Body::from(query.to_string()),
        )
        .await;
        assert_eq!(code, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "unauthorized");
        // Failure detail must NOT be disclosed on the unauthorized branch.
        assert!(body.get("failure").is_none());
    }

    #[tokio::test]
    async fn test_failed_serve_graphql_rejects_unknown_bearer_token() {
        let query = serde_json::json!({"query": "{ daemonStatus { state } }"});
        let (code, _body) = call_with_body_and_headers(
            test_failed_router(),
            "POST",
            "/graphql",
            Body::from(query.to_string()),
            &[("authorization", "Bearer definitely-not-a-real-token")],
        )
        .await;
        assert_eq!(code, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_failed_serve_daemon_status_query_resolves_with_snapshot() {
        // P042 §8.7: authenticated POST /graphql with the `daemonStatus`
        // query must return the GraphQL-shaped response derived from the
        // in-process snapshot, even though the DB pool is closed / the
        // full schema is not constructed.
        let query =
            serde_json::json!({"query": "{ daemonStatus { state binarySchemaVersion json } }"});
        let (code, body) = call_with_body_and_headers(
            test_failed_router(),
            "POST",
            "/graphql",
            Body::from(query.to_string()),
            &[("authorization", "Bearer test-token")],
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body["data"]["daemonStatus"]["state"], "failed");
        assert_eq!(body["data"]["daemonStatus"]["binarySchemaVersion"], 14);
        let inner = body["data"]["daemonStatus"]["json"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(inner).unwrap();
        assert_eq!(parsed["failure"]["kind"], "migration_failed");
        assert_eq!(parsed["failure"]["backup_path"], "/tmp/db.backup");
    }

    #[tokio::test]
    async fn test_failed_serve_non_status_query_refused_with_typed_envelope() {
        // Any GraphQL query that isn't daemonStatus — e.g. listing runs —
        // must be refused; only daemonStatus resolves in failed-serve.
        let query = serde_json::json!({"query": "{ runs { id } }"});
        let (code, body) = call_with_body_and_headers(
            test_failed_router(),
            "POST",
            "/graphql",
            Body::from(query.to_string()),
            &[("authorization", "Bearer test-token")],
        )
        .await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "daemon_in_failed_state");
    }

    /// R13 API-002: `/mcp` in failed-serve must return a JSON-RPC
    /// -32000 envelope rather than a bare HTTP 503 blob. Preserves
    /// the inbound `id` so the client can match the response.
    #[tokio::test]
    async fn test_failed_serve_mcp_returns_jsonrpc_error_with_request_id() {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/list",
            "params": {}
        });
        let (code, body) = call_with_body(
            test_failed_router(),
            "POST",
            "/mcp",
            Body::from(req.to_string()),
        )
        .await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["id"], 7);
        assert_eq!(body["error"]["code"], -32000);
        assert_eq!(body["error"]["message"], "daemon_in_failed_state");
        assert_eq!(body["error"]["data"]["state"], "failed");
        assert_eq!(body["error"]["data"]["failure"]["kind"], "migration_failed");
    }

    #[tokio::test]
    async fn test_failed_serve_mcp_notification_returns_202() {
        // JSON-RPC 2.0 notifications have no `id` — the spec forbids a
        // response body. Failed-serve must honour that and return HTTP
        // 202 without a JSON-RPC envelope.
        let req = serde_json::json!({"jsonrpc": "2.0", "method": "tools/list", "params": {}});
        let response = test_failed_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mcp")
                    .header("content-type", "application/json")
                    .body(Body::from(req.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn test_failed_serve_listener_variant_binds_and_serves_status() {
        // P042 §7.3 / §8.7: the daemon startup path hands a pre-bound
        // listener in (produced by `packaging::bind_with_fallback` so
        // `daemon.port` is already written), and the failed-serve loop
        // must accept it and serve the status-only router from there.
        // We prove the wiring works by issuing a raw HTTP/1.1 GET over a
        // TCP socket rather than pulling in a full HTTP client crate.
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let reporter = test_reporter_failed();
        let principals = test_principal_table();
        let server = tokio::spawn(async move {
            let serve = serve_failed_state_with_listener(reporter, principals, listener);
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), serve).await;
        });

        // Give axum a moment to start accepting.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
        let request = format!(
            "GET /health HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            addr
        );
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = Vec::new();
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            stream.read_to_end(&mut response),
        )
        .await
        .unwrap();
        let text = String::from_utf8_lossy(&response).to_string();

        assert!(
            text.starts_with("HTTP/1.1 503"),
            "expected 503, got:\n{text}"
        );
        assert!(
            text.contains("\"state\":\"failed\""),
            "expected failed state in body:\n{text}"
        );
        assert!(
            text.contains("\"kind\":\"migration_failed\""),
            "expected migration_failed kind in body:\n{text}"
        );
        server.abort();
    }
}
