use anyhow::Result;
use async_graphql::http::ALL_WEBSOCKET_PROTOCOLS;
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::{
    extract::{Extension, WebSocketUpgrade},
    http::StatusCode,
    middleware,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use domain::lifecycle::DaemonLifecycleState;
use engine::lifecycle_reporter::LifecycleReporter;
use tracing::info;

use crate::schema::AppSchema;

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

#[cfg(test)]
const GRAPHQL_WS_UNAUTHORIZED_CLOSE_CODE: u16 = 1002;

async fn graphql_http_handler(
    Extension(schema): Extension<AppSchema>,
    Extension(principal): Extension<auth::Principal>,
    // P042 §9.3: the request-id middleware attaches this to every
    // inbound HTTP request; we inject it into the async-graphql request
    // data so mutation resolvers can stamp `CallerContext.request_id`
    // and the command journal picks it up in the same transaction.
    request_id: Option<Extension<crate::request_id::RequestId>>,
    request: GraphQLRequest,
) -> GraphQLResponse {
    let mut request = request.into_inner();
    request = request.data(principal);
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
                .set("request_id", rid.clone());
        }
    }
    response.into()
}

async fn connection_init_data(
    value: serde_json::Value,
    table: auth::PrincipalTable,
) -> std::result::Result<async_graphql::Data, async_graphql::Error> {
    let token = value
        .get("Authorization")
        .and_then(|v| v.as_str())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim());

    match token {
        Some(t) => match auth::resolve_bearer(t, &table) {
            Ok(principal) => {
                let mut data = async_graphql::Data::default();
                data.insert(principal);
                Ok(data)
            }
            Err(_) => Err(async_graphql::Error::new("unauthorized")),
        },
        None => Err(async_graphql::Error::new("unauthorized")),
    }
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
        .on_upgrade(move |stream| {
            let table = principal_table;
            GraphQLWebSocket::new(stream, schema, protocol)
                .on_connection_init(move |value: serde_json::Value| {
                    let table = table;
                    async move { connection_init_data(value, table).await }
                })
                .serve()
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
    use std::sync::Arc;
    use tower::ServiceExt;

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
                    .header("authorization", "Bearer test-token")
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
        let principal_path = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            principal_path.path(),
            r#"{"principals":[{"token":"observer-token","id":"observer","class":"observer"}]}"#,
        )
        .unwrap();
        let principal_table =
            auth::PrincipalTable::load_or_bootstrap(principal_path.path()).unwrap();
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
                    .header("authorization", "Bearer observer-token")
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
        let err = connection_init_data(serde_json::json!({}), auth::PrincipalTable::test_fixture())
            .await
            .expect_err("missing WS token must fail");

        assert_eq!(err.message, "unauthorized");
        assert_eq!(GRAPHQL_WS_UNAUTHORIZED_CLOSE_CODE, 1002);
    }

    #[tokio::test]
    async fn test_graphql_ws_rejects_unknown_connection_init_token() {
        let err = connection_init_data(
            serde_json::json!({"Authorization":"Bearer bad-token"}),
            auth::PrincipalTable::test_fixture(),
        )
        .await
        .expect_err("unknown WS token must fail");

        assert_eq!(err.message, "unauthorized");
        assert_eq!(GRAPHQL_WS_UNAUTHORIZED_CLOSE_CODE, 1002);
    }

    #[tokio::test]
    async fn test_graphql_ws_accepts_valid_connection_init_token() {
        let data = connection_init_data(
            serde_json::json!({"Authorization":"Bearer test-token"}),
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

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
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
        format!(
            r#"
            mutation ApproveApproval {{
              approveApproval(approvalId: "{approval_id}") {{
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
