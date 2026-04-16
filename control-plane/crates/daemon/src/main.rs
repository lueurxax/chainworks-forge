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
    // 1. Init tracing — always write to stderr so MCP stdio mode
    //    keeps stdout clean for JSON-RPC protocol messages.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // 2. Read config from env
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://./chainworks-control-plane.db".to_string());
    let graphql_addr = std::env::var("GRAPHQL_ADDR").unwrap_or_else(|_| "0.0.0.0:4000".to_string());
    let mode = std::env::var("MODE").unwrap_or_else(|_| "daemon".to_string());
    info!(
        database_url = %database_url,
        mode = %mode,
        "Starting control-plane daemon"
    );

    // 3. Create SQLite pool (runs migrations)
    let pool = create_pool(&database_url).await?;
    info!("Database pool created and migrations applied");

    let steward_config_path = std::env::var("STEWARD_CONFIG_PATH")
        .ok()
        .map(std::path::PathBuf::from);
    let steward_catalog_path = std::env::var("AGENT_CATALOG_PATH")
        .ok()
        .map(std::path::PathBuf::from);
    let steward_runtime_inputs = Arc::new(
        daemon::steward_runtime::bootstrap_steward_runtime(
            &pool,
            steward_config_path.as_deref(),
            steward_catalog_path.as_deref(),
        )
        .await?,
    );
    info!(
        steward_config_hash = %steward_runtime_inputs.steward_config_hash,
        agent_catalog_hash = %steward_runtime_inputs.agent_catalog_hash,
        "Steward runtime config loaded"
    );

    // 4. Create EventBus
    let events = new_bus(1024);

    // 5. Create WorkQueue
    let work_queue = WorkQueue::new(pool.clone());

    // 6. Create AcpRuntimeManager
    let acp = Arc::new(AcpRuntimeManager::new());

    // 7. Create CommandHandler
    let cmd_handler = Arc::new(CommandHandler::new_with_acp(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
        acp.clone(),
    ));

    // 8. Create Orchestrator
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));

    // 9. Create BackgroundExecutor and start it
    let executor = Arc::new(BackgroundExecutor::new_with_steward_runtime_inputs(
        pool.clone(),
        work_queue.clone(),
        orchestrator.clone(),
        acp.clone(),
        events.clone(),
        steward_runtime_inputs.clone(),
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

    // 11. Load principal table (auto-bootstraps if missing)
    let principals_path = std::env::var("CHAINWORKS_AUTH_PRINCIPALS_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".chainworks")
                .join("auth")
                .join("principals.json")
        });
    let principal_table = auth::PrincipalTable::load_or_bootstrap(&principals_path)?;
    info!("Principal table loaded from {}", principals_path.display());

    // 12. Mode dispatch
    match mode.as_str() {
        "mcp" => {
            // Run McpServer::run_stdio() and exit
            let mcp = mcp_server::server::McpServer::new(
                pool.clone(),
                cmd_handler.clone(),
                principal_table,
            );
            mcp.run_stdio().await?;
        }
        _ => {
            // Daemon mode: single process with GraphQL + MCP HTTP on the same port.
            let mcp = std::sync::Arc::new(mcp_server::server::McpServer::new(
                pool.clone(),
                cmd_handler.clone(),
                principal_table.clone(),
            ));
            let mcp_routes = mcp_server::http::routes(mcp);
            info!("MCP HTTP transport mounted at /mcp");

            let schema = graphql_server::schema::build_schema(
                pool.clone(),
                cmd_handler.clone(),
                events.clone(),
                principal_table.clone(),
            );
            graphql_server::server::start_with_extra_routes(
                schema,
                &graphql_addr,
                mcp_routes,
                principal_table,
            )
                .await?;
        }
    }

    Ok(())
}
