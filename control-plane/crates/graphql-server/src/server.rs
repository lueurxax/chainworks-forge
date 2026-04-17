use anyhow::Result;
use async_graphql::http::ALL_WEBSOCKET_PROTOCOLS;
use async_graphql_axum::{GraphQLProtocol, GraphQLRequest, GraphQLResponse, GraphQLWebSocket};
use axum::{
    extract::{Extension, WebSocketUpgrade},
    middleware,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use tracing::info;

use crate::schema::AppSchema;

async fn graphql_playground() -> impl IntoResponse {
    Html(async_graphql::http::playground_source(
        async_graphql::http::GraphQLPlaygroundConfig::new("/graphql")
            .subscription_endpoint("/graphql/ws"),
    ))
}

#[cfg(test)]
const GRAPHQL_WS_UNAUTHORIZED_CLOSE_CODE: u16 = 1002;

async fn graphql_http_handler(
    Extension(schema): Extension<AppSchema>,
    Extension(principal): Extension<auth::Principal>,
    request: GraphQLRequest,
) -> GraphQLResponse {
    let mut request = request.into_inner();
    request = request.data(principal);
    schema.execute(request).await.into()
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
) -> Result<()> {
    start_with_extra_routes(schema, addr, Router::new(), principal_table).await
}

/// Start the GraphQL server with additional axum routes merged in.
/// Used by the daemon to mount MCP HTTP transport on the same port.
///
/// Auth middleware is mounted on the `/graphql` route only.
/// The subscription route (`/graphql/ws`) is outside the auth layer
/// because WS auth happens in `connection_init`, not at upgrade (P029 §4.1.c).
pub async fn start_with_extra_routes(
    schema: AppSchema,
    addr: &str,
    extra: Router,
    principal_table: auth::PrincipalTable,
) -> Result<()> {
    let app = build_router(schema, extra, principal_table);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(addr = %addr, "Server listening (GraphQL + MCP)");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(schema: AppSchema, extra: Router, principal_table: auth::PrincipalTable) -> Router {
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
        .layer(Extension(schema))
        .layer(Extension(principal_table))
        .merge(extra)
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use db::pool::create_pool;
    use db::repos::ideas;
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::IdeaId;
    use engine::command_handler::CommandHandler;
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    use std::sync::Arc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_graphql_mutation_reads_principal_from_context() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let schema = crate::schema::build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
        );
        let app = build_router(schema, Router::new(), auth::PrincipalTable::test_fixture());
        let body = serde_json::json!({
            "query": start_run_mutation(&idea_id),
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
            json["data"]["startRun"]["journalId"].as_str().is_some(),
            "mutation response must expose journalId: {json}"
        );
    }

    #[tokio::test]
    async fn test_graphql_observer_class_cannot_invoke_start_run() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
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
        );
        let app = build_router(schema, Router::new(), principal_table);
        let body = serde_json::json!({
            "query": start_run_mutation(&idea_id),
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
        );
        let app = build_router(schema, Router::new(), auth::PrincipalTable::test_fixture());

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
        );
        let app = build_router(schema, Router::new(), auth::PrincipalTable::test_fixture());

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

    fn start_run_mutation(idea_id: &IdeaId) -> String {
        format!(
            r#"
            mutation StartRun {{
              startRun(
                ideaId: "{idea_id}",
                workflowId: "wf-start",
                workflowTitle: "Start Run",
                workspaceRoot: "/tmp/ws",
                artifactRoot: "/tmp/art",
                workflowYamlPath: "{workflow_yaml_path}",
                agentCatalogYamlPath: "{agent_catalog_yaml_path}"
              ) {{
                ... on StartRunStartedPayload {{ run {{ id }} journalId }}
                ... on StartRunBlockedPayload {{ deliveryPreflight {{ passed }} journalId }}
              }}
            }}
            "#,
            workflow_yaml_path = test_workflow_yaml_path(),
            agent_catalog_yaml_path = test_agent_catalog_yaml_path(),
        )
    }

    fn test_workflow_yaml_path() -> String {
        format!(
            "{}/../../../examples/workflows/workflow.yaml",
            env!("CARGO_MANIFEST_DIR")
        )
    }

    fn test_agent_catalog_yaml_path() -> String {
        format!(
            "{}/../../../examples/agents/agents.yaml",
            env!("CARGO_MANIFEST_DIR")
        )
    }
}
