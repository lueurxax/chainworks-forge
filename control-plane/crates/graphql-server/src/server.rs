use anyhow::Result;
use async_graphql_axum::{GraphQL, GraphQLSubscription};
use axum::{
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
    let graphql_service = GraphQL::new(schema.clone());
    let subscription_service = GraphQLSubscription::new(schema);

    let pt = principal_table.clone();
    let app = Router::new()
        .route(
            "/graphql",
            get(graphql_playground).post_service(graphql_service),
        )
        .layer(middleware::from_fn(move |req, next| {
            let table = pt.clone();
            async move { crate::auth_layer::require_auth(req, next, table).await }
        }))
        .route_service("/graphql/ws", subscription_service)
        .merge(extra);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(addr = %addr, "Server listening (GraphQL + MCP)");
    axum::serve(listener, app).await?;
    Ok(())
}
