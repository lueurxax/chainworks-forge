use anyhow::Result;
use async_graphql_axum::{GraphQL, GraphQLSubscription};
use axum::{
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

pub async fn start(schema: AppSchema, addr: &str) -> Result<()> {
    start_with_extra_routes(schema, addr, Router::new()).await
}

/// Start the GraphQL server with additional axum routes merged in.
/// Used by the daemon to mount MCP HTTP transport on the same port.
pub async fn start_with_extra_routes(
    schema: AppSchema,
    addr: &str,
    extra: Router,
) -> Result<()> {
    let graphql_service = GraphQL::new(schema.clone());
    let subscription_service = GraphQLSubscription::new(schema);

    let app = Router::new()
        .route("/graphql", get(graphql_playground).post_service(graphql_service))
        .route_service("/graphql/ws", subscription_service)
        .merge(extra);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(addr = %addr, "Server listening (GraphQL + MCP)");
    axum::serve(listener, app).await?;
    Ok(())
}
