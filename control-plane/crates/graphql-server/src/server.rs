use anyhow::Result;
use async_graphql::http::ALL_WEBSOCKET_PROTOCOLS;
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::{
    extract::ws::{CloseFrame, Message},
    extract::{Extension, WebSocketUpgrade},
    http::StatusCode,
    middleware,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use domain::lifecycle::DaemonLifecycleState;
use engine::lifecycle_reporter::LifecycleReporter;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tracing::info;

use crate::schema::{attach_p081_collected_redactions, AppSchema, P081GraphqlRedactionCollector};
use crate::types::session::P046LiveCredential;

async fn graphql_playground() -> impl IntoResponse {
    Html(async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/graphql")
            .subscription_endpoint("/graphql/ws"),
    ))
}

/// Liveness probe (Proposal 042 §5.2). Returns 200 iff the process is
/// live-and-serving — `Ready` or `Degraded`. `Degraded` explicitly stays
/// 200 so a supervisor's liveness-keyed restart does not loop-restart a
/// recoverable daemon.
///
/// Response body shape:
/// - `state=ready`: `{state, schema_version, pid}`
/// - `state=degraded`: adds `degraded: [{kind, since, ...}]`
/// - `state=failed`: adds `failure: {kind, detail, since, backup_path?}`
pub(crate) async fn health_handler(
    Extension(reporter): Extension<LifecycleReporter>,
) -> impl IntoResponse {
    let status = reporter.snapshot();
    let code = match status.state {
        DaemonLifecycleState::Ready | DaemonLifecycleState::Degraded => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    (code, Json(status))
}

/// Readiness probe (Proposal 042 §5.2). Returns 200 only when the daemon
/// is `Ready`; `Degraded` returns 503 so client bootstrap surfaces the
/// condition to the user.
pub(crate) async fn ready_handler(
    Extension(reporter): Extension<LifecycleReporter>,
) -> impl IntoResponse {
    let status = reporter.snapshot();
    let code = if status.state == DaemonLifecycleState::Ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (code, Json(status))
}

pub(crate) const GRAPHQL_WS_UNAUTHORIZED_CLOSE_CODE: u16 = 4401;
pub(crate) const GRAPHQL_WS_FORBIDDEN_CLOSE_CODE: u16 = 4403;
pub(crate) const GRAPHQL_WS_INIT_TIMEOUT_CLOSE_CODE: u16 = 4408;
#[allow(dead_code)]
pub(crate) const GRAPHQL_WS_POLICY_RELOAD_CLOSE_CODE: u16 = 4408;
#[allow(dead_code)]
pub(crate) const GRAPHQL_WS_POLICY_RELOAD_CLOSE_REASON: &str = "POLICY_RELOAD";
const GRAPHQL_WS_INIT_TIMEOUT: Duration = Duration::from_secs(5);

async fn graphql_http_handler(
    Extension(schema): Extension<AppSchema>,
    Extension(principal): Extension<auth::Principal>,
    // P042 §9.3: the request-id middleware attaches this to every
    // inbound HTTP request; we inject it into the async-graphql request
    // data so mutation resolvers can stamp `CallerContext.request_id`
    // and the command journal picks it up in the same transaction.
    request_id: Option<Extension<crate::request_id::RequestId>>,
    // SEC-P081-M002: derived token_id for audit correlation, inserted by auth middleware.
    token_id: Option<Extension<crate::auth_layer::GraphqlTokenId>>,
    request: GraphQLRequest,
) -> GraphQLResponse {
    let mut request = request.into_inner();
    request = request.data(principal);
    let p081_redactions = P081GraphqlRedactionCollector::default();
    request = request.data(p081_redactions.clone());
    if let Some(Extension(tid)) = token_id {
        request = request.data(tid);
    }
    // Keep a copy of the id around so we can stamp every outbound
    // error with it even after we move the `RequestId` into request
    // data (R12 API-001 / AC-15: "GraphQL errors must include the
    // request id").
    let rid_for_errors: Option<String> = request_id.as_ref().map(|Extension(rid)| rid.0.clone());
    if let Some(Extension(rid)) = request_id {
        request = request.data(rid);
    }
    let mut response = schema.execute(request).await;
    if let Some(rid) = rid_for_errors {
        for err in response.errors.iter_mut() {
            // `ErrorExtensionValues::set` owns the value; clone for
            // each error so all entries in a multi-error response
            // carry the id independently.
            err.extensions
                .get_or_insert_with(async_graphql::ErrorExtensionValues::default)
                .set("requestId", rid.clone());
        }
    }
    add_p081_response_redactions(&mut response);
    attach_p081_collected_redactions(&mut response, &p081_redactions);
    response.into()
}

fn add_p081_response_redactions(response: &mut async_graphql::Response) {
    let redactions: Vec<_> = response
        .errors
        .iter()
        .filter_map(p081_redaction_from_error)
        .collect();
    if redactions.is_empty() {
        return;
    }
    response
        .extensions
        .insert("redactions".into(), async_graphql::Value::List(redactions));
}

fn p081_redaction_from_error(err: &async_graphql::ServerError) -> Option<async_graphql::Value> {
    let extensions = err.extensions.as_ref()?;
    let reason_code = graphql_value_string(extensions.get("reasonCode"))?;
    let row_id = graphql_value_string(extensions.get("rowId"));
    let caller_class = graphql_value_string(extensions.get("callerClass"));
    let mut object = async_graphql::indexmap::IndexMap::new();
    object.insert(
        async_graphql::Name::new("path"),
        async_graphql::Value::List(
            err.path
                .iter()
                .map(|segment| async_graphql::Value::String(path_segment_string(segment)))
                .collect(),
        ),
    );
    object.insert(
        async_graphql::Name::new("reasonCode"),
        async_graphql::Value::String(reason_code.clone()),
    );
    if let Some(row_id) = row_id.clone() {
        object.insert(
            async_graphql::Name::new("rowId"),
            async_graphql::Value::String(row_id),
        );
    }
    object.insert(
        async_graphql::Name::new("redactionMode"),
        async_graphql::Value::String("drop_resource".into()),
    );
    if let Some(caller_class) = caller_class.clone() {
        object.insert(
            async_graphql::Name::new("callerClass"),
            async_graphql::Value::String(caller_class),
        );
    }
    object.insert(
        async_graphql::Name::new("redactionId"),
        async_graphql::Value::String(p081_redaction_id(
            err.path
                .iter()
                .map(path_segment_string)
                .collect::<Vec<_>>()
                .join(".")
                .as_str(),
            &reason_code,
            row_id.as_deref(),
        )),
    );
    Some(async_graphql::Value::Object(object))
}

fn path_segment_string(segment: &async_graphql::PathSegment) -> String {
    match segment {
        async_graphql::PathSegment::Field(field) => field.clone(),
        async_graphql::PathSegment::Index(index) => index.to_string(),
    }
}

fn graphql_value_string(value: Option<&async_graphql::Value>) -> Option<String> {
    match value {
        Some(async_graphql::Value::String(s)) => Some(s.clone()),
        Some(async_graphql::Value::Enum(name)) => Some(name.to_string()),
        _ => None,
    }
}

fn p081_redaction_id(path: &str, reason_code: &str, row_id: Option<&str>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hasher.update(b"\0");
    hasher.update(reason_code.as_bytes());
    hasher.update(b"\0");
    hasher.update(row_id.unwrap_or("").as_bytes());
    format!("p081_redaction_{:x}", hasher.finalize())[..29].to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct P081WsClose {
    code: u16,
    reason: &'static str,
}

impl P081WsClose {
    const UNAUTHORIZED: Self = Self {
        code: GRAPHQL_WS_UNAUTHORIZED_CLOSE_CODE,
        reason: "UNAUTHORIZED",
    };
    const FORBIDDEN: Self = Self {
        code: GRAPHQL_WS_FORBIDDEN_CLOSE_CODE,
        reason: "FORBIDDEN",
    };
    const INIT_TIMEOUT: Self = Self {
        code: GRAPHQL_WS_INIT_TIMEOUT_CLOSE_CODE,
        reason: "CONNECTION_INIT_TIMEOUT",
    };
}

async fn p081_connection_init_data(
    value: serde_json::Value,
    table: auth::PrincipalTable,
) -> std::result::Result<async_graphql::Data, P081WsClose> {
    // Strict bearer grammar via auth::extract_bearer_token (exactly "Bearer <token>",
    // one SP, no surrounding whitespace, visible ASCII, non-empty token).
    let header = value
        .get("Authorization")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match auth::extract_bearer_token(header) {
        Ok(token) => match auth::resolve_bearer(token, &table) {
            Ok(principal) => {
                if auth::derive_caller_class(&principal) != auth::CallerClass::UiOperator {
                    return Err(P081WsClose::FORBIDDEN);
                }
                // SEC-P081-M002: derive token_id for WS subscription audit correlation.
                let token_id = auth::derive_token_id(token, &principal.id);
                let mut data = async_graphql::Data::default();
                data.insert(crate::auth_layer::GraphqlTokenId(token_id));
                data.insert(P046LiveCredential {
                    principal_id: principal.id.clone(),
                    token_fingerprint: auth::token_fingerprint(token),
                });
                data.insert(principal);
                Ok(data)
            }
            Err(_) => Err(P081WsClose::UNAUTHORIZED),
        },
        Err(_) => Err(P081WsClose::UNAUTHORIZED),
    }
}

pub async fn connection_init_data(
    value: serde_json::Value,
    table: auth::PrincipalTable,
) -> std::result::Result<async_graphql::Data, async_graphql::Error> {
    p081_connection_init_data(value, table)
        .await
        .map_err(|close| async_graphql::Error::new(close.reason))
}

fn connection_init_payload_from_message(
    message: &Message,
) -> std::result::Result<serde_json::Value, P081WsClose> {
    let bytes: Vec<u8> = match message {
        Message::Text(text) => text.as_str().as_bytes().to_vec(),
        Message::Binary(bytes) => bytes.to_vec(),
        _ => return Err(P081WsClose::INIT_TIMEOUT),
    };
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| P081WsClose::INIT_TIMEOUT)?;
    if value.get("type").and_then(|v| v.as_str()) != Some("connection_init") {
        return Err(P081WsClose::INIT_TIMEOUT);
    }
    Ok(value
        .get("payload")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({})))
}

async fn send_p081_ws_close<S>(sink: &mut S, close: P081WsClose)
where
    S: futures_util::Sink<Message> + Unpin,
{
    let _ = sink
        .send(Message::Close(Some(CloseFrame {
            code: close.code,
            reason: close.reason.into(),
        })))
        .await;
}

/// WebSocket subscription handler with `connection_init` auth.
///
/// `GraphQLSubscription` (the tower Service from `async_graphql_axum`) does NOT
/// expose an `on_connection_init` hook — it creates a bare `GraphQLWebSocket`
/// internally.  We therefore use a manual axum handler that accepts the
/// `WebSocketUpgrade`, extracts the `GraphQLProtocol`, and wires up the
/// `on_connection_init` callback ourselves.
///
/// Per P029 §4.1.c the `/graphql/ws` route is mounted OUTSIDE the HTTP auth
/// middleware; authentication happens inside `connection_init` after the
/// WebSocket handshake completes.
async fn graphql_ws_handler(
    ws: WebSocketUpgrade,
    protocol: GraphQLProtocol,
    Extension(schema): Extension<AppSchema>,
    Extension(principal_table): Extension<auth::PrincipalTable>,
) -> impl IntoResponse {
    ws.protocols(ALL_WEBSOCKET_PROTOCOLS)
        .on_upgrade(move |stream| async move {
            let table = principal_table;
            let (mut sink, mut stream) = stream.split();
            let first = match tokio::time::timeout(GRAPHQL_WS_INIT_TIMEOUT, stream.next()).await {
                Ok(Some(Ok(message))) => message,
                Ok(Some(Err(_))) | Ok(None) | Err(_) => {
                    send_p081_ws_close(&mut sink, P081WsClose::INIT_TIMEOUT).await;
                    return;
                }
            };
            let payload = match connection_init_payload_from_message(&first) {
                Ok(payload) => payload,
                Err(close) => {
                    send_p081_ws_close(&mut sink, close).await;
                    return;
                }
            };
            if let Err(close) = p081_connection_init_data(payload, table.clone()).await {
                send_p081_ws_close(&mut sink, close).await;
                return;
            }
            let replay = futures_util::stream::once(async move { Ok(first) }).chain(stream);
            GraphQLWebSocket::new_with_pair(sink, replay, schema, protocol)
                .on_connection_init(move |value: serde_json::Value| {
                    let table = table;
                    async move { connection_init_data(value, table).await }
                })
                .serve()
                .await
        })
}

pub async fn start(
    schema: AppSchema,
    addr: &str,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
) -> Result<()> {
    start_with_extra_routes(schema, addr, Router::new(), principal_table, reporter).await
}

/// Start the GraphQL server with additional axum routes merged in.
/// Used by the daemon to mount MCP HTTP transport on the same port.
///
/// Auth middleware is mounted on the `/graphql` route only.
/// The subscription route (`/graphql/ws`) is outside the auth layer
/// because WS auth happens in `connection_init`, not at upgrade (P029 §4.1.c).
/// `/health` and `/ready` are also outside auth per P042 §5.2 — they are
/// loopback-only probes used by supervisors and client bootstrap before
/// a bearer token is in scope.
pub async fn start_with_extra_routes(
    schema: AppSchema,
    addr: &str,
    extra: Router,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
) -> Result<()> {
    let app = build_router(schema, extra, principal_table, reporter);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(addr = %addr, "Server listening (GraphQL + MCP)");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Serve on an already-bound listener (P042 §7.3). The daemon binds the
/// preferred port (with ephemeral fallback + `daemon.port` write) before
/// calling into the GraphQL server so the port-allocation decision stays
/// in the `daemon::packaging` layer and the GraphQL server only has to
/// wire routes + run `axum::serve`.
pub async fn serve_with_listener(
    schema: AppSchema,
    listener: tokio::net::TcpListener,
    extra: Router,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
) -> Result<()> {
    let app = build_router(schema, extra, principal_table, reporter);
    let local_addr = listener.local_addr().ok();
    info!(addr = ?local_addr, "Server listening (GraphQL + MCP)");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Serve until the supplied shutdown future resolves (P042 §6.3 graceful
/// shutdown). Axum finishes in-flight requests before the serve future
/// returns so clients see a clean close rather than a reset connection.
pub async fn serve_with_listener_until<F>(
    schema: AppSchema,
    listener: tokio::net::TcpListener,
    extra: Router,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    shutdown: F,
) -> Result<()>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let app = build_router(schema, extra, principal_table, reporter);
    let local_addr = listener.local_addr().ok();
    info!(addr = ?local_addr, "Server listening (GraphQL + MCP) — graceful shutdown armed");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

pub(crate) fn build_router(
    schema: AppSchema,
    extra: Router,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
) -> Router {
    // SEC-003: Emit a one-time startup warning when the playground auth bypass
    // is active so it is visible in operator logs and never silently set in prod.
    if std::env::var("CHAINWORKS_PLAYGROUND_AUTH")
        .ok()
        .map(|v| v == "skip")
        .unwrap_or(false)
    {
        tracing::warn!(
            "CHAINWORKS_PLAYGROUND_AUTH=skip is set: GET /graphql playground HTML \
             shell bypasses bearer authentication. Do not enable in production."
        );
    }
    let pt = principal_table.clone();
    Router::new()
        .route(
            "/graphql",
            get(graphql_playground).post(graphql_http_handler),
        )
        .layer(middleware::from_fn(move |req, next| {
            let table = pt.clone();
            async move { crate::auth_layer::require_auth(req, next, table).await }
        }))
        .route("/graphql/ws", get(graphql_ws_handler))
        // P042 §5.2: liveness/readiness are unauthenticated loopback probes.
        // Mounted after the auth layer so the `require_auth` middleware
        // does not apply to them.
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler))
        .layer(Extension(schema))
        .layer(Extension(principal_table))
        .layer(Extension(reporter))
        // P042 §9.3 / R13 API-003: merge `extra` (MCP HTTP routes,
        // diagnostic endpoints…) BEFORE the request-id layer so the
        // layer wraps BOTH the GraphQL routes and the merged routes.
        // `Router::layer` only wraps routes that exist at the moment
        // it is called; anything `.merge`d after the layer is
        // silently un-wrapped. Prior to this fix, MCP HTTP never
        // received an `X-Request-ID` extension from the middleware
        // and the `mcp_caller` helper always observed `None`, which
        // dropped the correlation id from every `command_journal`
        // row landed via the MCP HTTP transport.
        .merge(extra)
        .layer(middleware::from_fn(crate::request_id::layer))
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use db::pool::create_pool;
    use db::repos::{approvals, ideas, runs, stages};
    use domain::approval::{Approval, ApprovalDecision};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ApprovalId, IdeaId, RunId, StageExecutionId};
    use domain::lifecycle::{DaemonLifecycleState, DegradedKind};
    use domain::run::{Run, RunStatus};
    use domain::stage::{StageExecution, StageStatus};
    use engine::command_handler::CommandHandler;
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn secure_principal_table(contents: &str) -> auth::PrincipalTable {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("principals.json");
        std::fs::write(&path, contents).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        auth::PrincipalTable::load_or_bootstrap(&path).unwrap()
    }

    fn test_reporter() -> LifecycleReporter {
        LifecycleReporter::new(0, "test-sha", event_bus::new_bus(16))
    }

    fn test_reporter_in_state(state: DaemonLifecycleState) -> LifecycleReporter {
        let reporter = test_reporter();
        reporter.set_state(state);
        reporter
    }

    #[tokio::test]
    async fn test_graphql_mutation_reads_principal_from_context() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        stages::insert(&pool, &make_manual_gate_stage(run_id, "state_6"))
            .await
            .unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();
        let schema = crate::schema::build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let app = build_router(
            schema,
            Router::new(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let body = serde_json::json!({
            "query": approve_approval_mutation(approval.id),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/graphql")
                    .header("authorization", "Bearer test-token-xxxxxxxxxxxxxxxxxxxxx")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(
            json.get("errors").is_none(),
            "authorized GraphQL HTTP mutation should succeed: {json}"
        );
        assert!(
            json["data"]["approveApproval"]["journalId"]
                .as_str()
                .is_some(),
            "mutation response must expose journalId: {json}"
        );
    }

    #[tokio::test]
    async fn test_graphql_observer_class_cannot_invoke_approval_mutation() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        stages::insert(&pool, &make_manual_gate_stage(run_id, "state_6"))
            .await
            .unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();
        let principal_table = secure_principal_table(
            r#"{"principals":[{"token":"observer-token-xxxxxxxxxxxxxxxxx","id":"observer","class":"observer"}]}"#,
        );
        let schema = crate::schema::build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table.clone(),
            test_reporter(),
        );
        let app = build_router(schema, Router::new(), principal_table, test_reporter());
        let body = serde_json::json!({
            "query": approve_approval_mutation(approval.id),
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/graphql")
                    .header("authorization", "Bearer observer-token-xxxxxxxxxxxxxxxxx")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["errors"][0]["message"], "forbidden");
        assert_eq!(
            json["extensions"]["redactions"][0]["redactionMode"], "drop_resource",
            "P081 GraphQL denial responses must expose top-level extensions.redactions"
        );
        let journal_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(journal_count.0, 0);
    }

    #[tokio::test]
    async fn test_graphql_rejects_missing_authorization_header() {
        let pool = test_pool().await;
        let schema = crate::schema::build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let app = build_router(
            schema,
            Router::new(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/graphql")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"query":"{ __typename }"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["errors"][0]["message"], "unauthorized");
    }

    #[tokio::test]
    async fn test_graphql_rejects_unknown_bearer_token() {
        let pool = test_pool().await;
        let schema = crate::schema::build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let app = build_router(
            schema,
            Router::new(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/graphql")
                    .header("authorization", "Bearer bad-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"query":"{ __typename }"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["errors"][0]["message"], "unauthorized");
    }

    #[tokio::test]
    async fn test_graphql_ws_rejects_missing_connection_init_auth() {
        let err =
            p081_connection_init_data(serde_json::json!({}), auth::PrincipalTable::test_fixture())
                .await
                .expect_err("missing WS token must fail");

        assert_eq!(err, P081WsClose::UNAUTHORIZED);
        assert_eq!(GRAPHQL_WS_UNAUTHORIZED_CLOSE_CODE, 4401);
    }

    #[tokio::test]
    async fn test_graphql_ws_rejects_unknown_connection_init_token() {
        let err = p081_connection_init_data(
            serde_json::json!({"Authorization":"Bearer bad-token"}),
            auth::PrincipalTable::test_fixture(),
        )
        .await
        .expect_err("unknown WS token must fail");

        assert_eq!(err, P081WsClose::UNAUTHORIZED);
        assert_eq!(GRAPHQL_WS_UNAUTHORIZED_CLOSE_CODE, 4401);
    }

    #[tokio::test]
    async fn test_graphql_ws_rejects_non_ui_caller_with_forbidden_close() {
        let principal_table = secure_principal_table(
            r#"{"principals":[{"token":"observer-token-xxxxxxxxxxxxxxxxx","id":"observer","class":"observer"}]}"#,
        );
        let err = p081_connection_init_data(
            serde_json::json!({"Authorization":"Bearer observer-token-xxxxxxxxxxxxxxxxx"}),
            principal_table,
        )
        .await
        .expect_err("observer GraphQL WS subscription must fail before subscribe");

        assert_eq!(err, P081WsClose::FORBIDDEN);
        assert_eq!(GRAPHQL_WS_FORBIDDEN_CLOSE_CODE, 4403);
    }

    #[test]
    fn test_graphql_ws_connection_init_parser_requires_init_message() {
        let err = connection_init_payload_from_message(&Message::Text(
            r#"{"type":"subscribe","id":"1"}"#.into(),
        ))
        .expect_err("first WS frame must be connection_init");
        assert_eq!(err, P081WsClose::INIT_TIMEOUT);
        assert_eq!(GRAPHQL_WS_INIT_TIMEOUT_CLOSE_CODE, 4408);
    }

    #[tokio::test]
    async fn test_graphql_ws_accepts_valid_connection_init_token() {
        let data = connection_init_data(
            serde_json::json!({"Authorization":"Bearer test-token-xxxxxxxxxxxxxxxxxxxxx"}),
            auth::PrincipalTable::test_fixture(),
        )
        .await
        .expect("valid WS token must produce connection data");
        let principal = data
            .get(&std::any::TypeId::of::<auth::Principal>())
            .and_then(|boxed| boxed.downcast_ref::<auth::Principal>())
            .unwrap();

        assert_eq!(principal.id, "test-operator");
    }

    #[test]
    fn proposal_081_websocket_policy_reload_close_contract_is_explicit() {
        assert_eq!(GRAPHQL_WS_POLICY_RELOAD_CLOSE_CODE, 4408);
        assert_eq!(GRAPHQL_WS_POLICY_RELOAD_CLOSE_REASON, "POLICY_RELOAD");
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

    fn make_command_handler(pool: sqlx::SqlitePool) -> Arc<CommandHandler> {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        Arc::new(CommandHandler::new(pool, events, work_queue))
    }

    fn make_idea(id: IdeaId) -> Idea {
        Idea {
            id,
            title: "GraphQL route idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: chrono::Utc::now(),
            archived_at: None,
        }
    }

    fn make_run(id: RunId, idea_id: IdeaId) -> Run {
        Run {
            id,
            idea_id,
            status: RunStatus::Ready,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: chrono::Utc::now(),
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
            delivery_configuration_json: None,
            delivery_preflight_json: None,
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

    fn make_manual_gate_stage(run_id: RunId, stage_id: &str) -> StageExecution {
        StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: stage_id.to_string(),
            label: stage_id.to_string(),
            status: StageStatus::WaitingApproval,
            iteration: 0,
            attempt_number: 1,
            settlement_kind: None,
            started_at: chrono::Utc::now(),
            completed_at: None,
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: Some("manual_gate".into()),
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        }
    }

    fn make_approval(run_id: RunId, stage_id: &str) -> Approval {
        Approval {
            id: ApprovalId::new(),
            run_id,
            stage_id: stage_id.to_string(),
            decision: ApprovalDecision::Pending,
            requested_at: chrono::Utc::now(),
            decided_at: None,
            comment: None,
            expires_at: None,
        }
    }

    fn approve_approval_mutation(approval_id: ApprovalId) -> String {
        let key = uuid::Uuid::now_v7();
        format!(
            r#"
            mutation ApproveApproval {{
              approveApproval(approvalId: "{approval_id}", idempotencyKey: "{key}") {{
                approval {{ id }}
                journalId
              }}
            }}
            "#
        )
    }

    // ── Proposal 042 §5.2 health/ready probe tests ────────────────────

    async fn probe(router: Router, path: &str) -> (StatusCode, serde_json::Value) {
        let response = router
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let code = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        (code, json)
    }

    fn build_probe_router(reporter: LifecycleReporter) -> Router {
        // We only need the two probe routes + the reporter extension.
        Router::new()
            .route("/health", get(health_handler))
            .route("/ready", get(ready_handler))
            .layer(Extension(reporter))
    }

    #[tokio::test]
    async fn test_health_endpoint_returns_200_when_ready() {
        let reporter = test_reporter_in_state(DaemonLifecycleState::Ready);
        let (code, body) = probe(build_probe_router(reporter), "/health").await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body["state"], "ready");
    }

    #[tokio::test]
    async fn test_health_endpoint_returns_200_in_degraded() {
        let reporter = test_reporter_in_state(DaemonLifecycleState::Ready);
        reporter.raise_degraded(DegradedKind::StaleProjection, "test");
        let (code, body) = probe(build_probe_router(reporter), "/health").await;
        // Critical invariant: liveness stays 200 in Degraded (P042 §5.2).
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body["state"], "degraded");
    }

    #[tokio::test]
    async fn test_health_endpoint_returns_503_only_when_starting_failed_or_shutdown() {
        for (state, expected_str) in [
            (DaemonLifecycleState::Starting, "starting"),
            (DaemonLifecycleState::Failed, "failed"),
            (DaemonLifecycleState::Shutdown, "shutdown"),
            (DaemonLifecycleState::Restarting, "restarting"),
        ] {
            let reporter = test_reporter_in_state(state);
            let (code, body) = probe(build_probe_router(reporter), "/health").await;
            assert_eq!(
                code,
                StatusCode::SERVICE_UNAVAILABLE,
                "state {expected_str} should return 503"
            );
            assert_eq!(body["state"], expected_str);
        }
    }

    #[tokio::test]
    async fn test_ready_endpoint_returns_200_only_when_ready() {
        let reporter = test_reporter_in_state(DaemonLifecycleState::Ready);
        let (code, _) = probe(build_probe_router(reporter), "/ready").await;
        assert_eq!(code, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_ready_endpoint_returns_503_in_degraded() {
        let reporter = test_reporter_in_state(DaemonLifecycleState::Ready);
        reporter.raise_degraded(DegradedKind::StaleProjection, "test");
        let (code, body) = probe(build_probe_router(reporter), "/ready").await;
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["state"], "degraded");
    }

    #[tokio::test]
    async fn test_daemon_status_failure_field_populated_only_when_failed() {
        use domain::lifecycle::FailureKind;
        // Ready → no failure field in JSON.
        let reporter = test_reporter_in_state(DaemonLifecycleState::Ready);
        let (_, body) = probe(build_probe_router(reporter), "/health").await;
        assert!(body.get("failure").is_none(), "{body}");
        // Failed → failure populated.
        let reporter = test_reporter();
        reporter.set_failed(FailureKind::MigrationFailed, "test", Some("/tmp/bk".into()));
        let (_, body) = probe(build_probe_router(reporter), "/health").await;
        assert_eq!(body["failure"]["kind"], "migration_failed");
        assert_eq!(body["failure"]["backup_path"], "/tmp/bk");
    }
}
