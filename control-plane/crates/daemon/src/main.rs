use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;
use tracing::info;

use acp::AcpRuntimeManager;
use db::pool::create_pool;
use db::repos::agent_executions;
use engine::command_handler::CommandHandler;
use engine::event_bus::new_bus;
use engine::executor::BackgroundExecutor;
use engine::orchestrator::Orchestrator;
use engine::recovery::RecoveryService;
use engine::work_queue::WorkQueue;

mod xcode_broker_http;

struct DbXcodeRuntimeObservationSink {
    pool: SqlitePool,
}

#[async_trait::async_trait]
impl acp::XcodeRuntimeObservationSink for DbXcodeRuntimeObservationSink {
    async fn append_xcode_runtime_observation(
        &self,
        agent_execution_id: domain::ids::AgentExecutionId,
        update: domain::xcode_runtime::XcodeRuntimeObservationUpdate,
    ) -> Result<()> {
        agent_executions::append_xcode_runtime_observation(&self.pool, agent_execution_id, update)
            .await
    }
}

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
    let xcode_broker_config = xcode_broker_config_from_env(&graphql_addr);
    let xcode_broker_pool = Arc::new(acp::XcodeMcpBridgePool::new_with_sink_and_backend(
        xcode_broker_config.clone(),
        Arc::new(DbXcodeRuntimeObservationSink { pool: pool.clone() }),
        Arc::new(acp::XcodeMcpProcessBackend::new(
            xcode_process_backend_config_from_env(),
        )),
    ));
    acp.set_xcode_broker_lease_attacher(xcode_broker_pool.clone());
    info!(
        base_url = %xcode_broker_config.base_url,
        disabled = xcode_broker_config.broker_disabled,
        "Xcode MCP broker configured"
    );

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
    let principals_path = principals_path_from_env()?;
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
            let xcode_broker_routes = xcode_broker_http::routes(xcode_broker_pool);
            info!("MCP HTTP transport mounted at /mcp");
            info!("Xcode MCP broker mounted at /xcode-mcp/{{lease_id}}");

            let schema = graphql_server::schema::build_schema(
                pool.clone(),
                cmd_handler.clone(),
                events.clone(),
                principal_table.clone(),
            );
            graphql_server::server::start_with_extra_routes(
                schema,
                &graphql_addr,
                mcp_routes.merge(xcode_broker_routes),
                principal_table,
            )
            .await?;
        }
    }

    Ok(())
}

fn principals_path_from_env() -> Result<std::path::PathBuf> {
    match std::env::var("CHAINWORKS_AUTH_PRINCIPALS_PATH") {
        Ok(value) if value.trim().is_empty() => {
            anyhow::bail!("CHAINWORKS_AUTH_PRINCIPALS_PATH must not be empty")
        }
        Ok(value) => Ok(std::path::PathBuf::from(value)),
        Err(_) => Ok(dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".chainworks")
            .join("auth")
            .join("principals.json")),
    }
}

fn xcode_broker_config_from_env(graphql_addr: &str) -> acp::XcodeMcpBridgePoolConfig {
    let mut config = acp::XcodeMcpBridgePoolConfig::default();
    config.base_url = std::env::var("CHAINWORKS_XCODE_BROKER_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| xcode_broker_base_url_from_graphql_addr(graphql_addr));
    config.broker_disabled = env_flag_enabled("CHAINWORKS_XCODE_BROKER_DISABLED");
    config.use_local_host_probe = true;
    config
}

fn xcode_process_backend_config_from_env() -> acp::XcodeMcpProcessBackendConfig {
    let mut config = acp::XcodeMcpProcessBackendConfig::default();
    if let Ok(command) = std::env::var("CHAINWORKS_XCODE_MCPBRIDGE_COMMAND") {
        if !command.trim().is_empty() {
            config.command = command;
        }
    }
    if let Ok(args) = std::env::var("CHAINWORKS_XCODE_MCPBRIDGE_ARGS") {
        let parsed = args
            .split_whitespace()
            .filter(|arg| !arg.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if !parsed.is_empty() {
            config.args = parsed;
        }
    }
    config
}

fn xcode_broker_base_url_from_graphql_addr(graphql_addr: &str) -> String {
    let trimmed = graphql_addr.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return format!("{}/xcode-mcp", trimmed.trim_end_matches('/'));
    }

    let (host, port) = split_host_port(trimmed).unwrap_or(("127.0.0.1", "4000"));
    let connect_host = match host {
        "" | "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        value => value.trim_matches(&['[', ']'][..]),
    };
    let host_for_url = if connect_host.contains(':') {
        format!("[{connect_host}]")
    } else {
        connect_host.to_string()
    };
    format!("http://{host_for_url}:{port}/xcode-mcp")
}

fn split_host_port(value: &str) -> Option<(&str, &str)> {
    if let Some(stripped) = value.strip_prefix('[') {
        if let Some((host, port)) = stripped.split_once("]:") {
            return Some((host, port));
        }
    }
    value.rsplit_once(':')
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_principals_path_rejects_empty_env() {
        std::env::set_var("CHAINWORKS_AUTH_PRINCIPALS_PATH", "");
        let result = super::principals_path_from_env();
        std::env::remove_var("CHAINWORKS_AUTH_PRINCIPALS_PATH");

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must not be empty"));
    }

    #[test]
    fn xcode_broker_base_url_rewrites_wildcard_listener_to_loopback() {
        assert_eq!(
            super::xcode_broker_base_url_from_graphql_addr("0.0.0.0:4000"),
            "http://127.0.0.1:4000/xcode-mcp"
        );
        assert_eq!(
            super::xcode_broker_base_url_from_graphql_addr("[::]:5000"),
            "http://127.0.0.1:5000/xcode-mcp"
        );
        assert_eq!(
            super::xcode_broker_base_url_from_graphql_addr("127.0.0.1:4100"),
            "http://127.0.0.1:4100/xcode-mcp"
        );
    }
}
