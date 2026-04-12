use std::sync::Arc;

use anyhow::Result;
use tracing::info;

use acp::AcpRuntimeManager;
use db::pool::create_pool;
use engine::command_handler::CommandHandler;
use engine::event_bus::new_bus;
use engine::executor::BackgroundExecutor;
use engine::orchestrator::Orchestrator;
use engine::recovery::RecoveryService;
use engine::work_queue::WorkQueue;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // 2. Read config from env
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://./chainworks-control-plane.db".to_string());
    let graphql_addr = std::env::var("GRAPHQL_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:4000".to_string());
    let mode = std::env::var("MODE").unwrap_or_else(|_| "daemon".to_string());
    info!(
        database_url = %database_url,
        mode = %mode,
        "Starting control-plane daemon"
    );

    // 3. Create SQLite pool (runs migrations)
    let pool = create_pool(&database_url).await?;
    info!("Database pool created and migrations applied");

    // 4. Create EventBus
    let events = new_bus(1024);

    // 5. Create WorkQueue
    let work_queue = WorkQueue::new(pool.clone());

    // 6. Create CommandHandler
    let cmd_handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));

    // 7. Create AcpRuntimeManager
    let acp = Arc::new(AcpRuntimeManager::new());

    // 8. Create Orchestrator
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));

    // 9. Create BackgroundExecutor and start it
    let executor = Arc::new(BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator.clone(),
        acp.clone(),
        events.clone(),
    ));
    let _executor_handle = executor.start();
    info!("BackgroundExecutor started");

    // 10. Run startup recovery
    let recovery = RecoveryService::new(pool.clone(), work_queue.clone(), events.clone());
    let summary = recovery.run_startup_repair().await?;
    info!(
        runs_inspected = summary.runs_inspected,
        runs_repaired = summary.runs_repaired,
        work_items_requeued = summary.work_items_requeued,
        "Startup recovery complete"
    );

    // 11. Mode dispatch
    match mode.as_str() {
        "mcp" => {
            // Run McpServer::run_stdio() and exit
            let mcp = mcp_server::server::McpServer::new(pool.clone(), cmd_handler.clone());
            mcp.run_stdio().await?;
        }
        _ => {
            // Daemon mode: single process with GraphQL + MCP HTTP on the same port.
            let mcp = std::sync::Arc::new(
                mcp_server::server::McpServer::new(pool.clone(), cmd_handler.clone()),
            );
            let mcp_routes = mcp_server::http::routes(mcp);
            info!("MCP HTTP transport mounted at /mcp");

            let schema = graphql_server::schema::build_schema(
                pool.clone(),
                cmd_handler.clone(),
                events.clone(),
            );
            graphql_server::server::start_with_extra_routes(
                schema,
                &graphql_addr,
                mcp_routes,
            )
            .await?;
        }
    }

    Ok(())
}
