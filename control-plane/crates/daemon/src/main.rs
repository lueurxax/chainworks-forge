//! control-plane daemon entry point (Proposal 042 §7, §8).
//!
//! Composition root for the control-plane daemon. Wires together the P042
//! startup contract before any user-visible service is bound:
//!
//! 1. `DaemonMode::from_env_var` + `packaging::resolve_paths` — mode-aware
//!    path resolution for DB, principals, logs, PID lock, and the bind addr.
//! 2. `packaging::enforce_loopback` — packaged modes rewrite `0.0.0.0`
//!    overrides to `127.0.0.1` so a stolen bearer token is not usable
//!    from the LAN (§7.4).
//! 3. `supervisor::acquire_pid_lock` — advisory singleton per app-support
//!    root (§6.1): `DuplicateHealthy` → exit 0; `AnomalousHolder` →
//!    exit 75 (`EX_TEMPFAIL`); `Acquired` → continue.
//! 4. `supervisor::read_crash_budget` — ≥5 crashes within 60 s puts the
//!    daemon into §8.7 failed-serve mode with
//!    `FailureKind::CrashLoopBudgetExhausted` instead of a normal startup.
//! 5. `db::migrate::run_preflight` — typed three-branch migration classifier.
//!    Any error maps (§8.4) to a `FailureKind` and the daemon drops into
//!    failed-serve mode rather than panicking, so `/health`, `/ready`,
//!    and `daemonStatus` still report the terminal condition.
//! 6. `packaging::bind_with_fallback` — try preferred port, fall back to
//!    ephemeral loopback in packaged modes; write the chosen port to
//!    `daemon.port` so the Swift client can locate us (§7.3).
//! 7. `packaging::write_build_sha` — record the embedded build SHA for
//!    the Swift diagnostics bundle (§9.4).
//!
//! If any §8.4 terminal condition is hit, the daemon transitions the
//! lifecycle reporter to `Failed { failure: Some(..) }` and calls
//! `failed_serve::serve_failed_state`; the ACP runtime, background
//! executor, steward runtime, and GraphQL/MCP mutations are never
//! constructed in that path.

use std::sync::Arc;

use anyhow::Result;
use tracing::{error, info, warn};

use acp::{
    AcpRuntimeManager, DefaultXcodeShimProcessInspector, XcodeHostExecutorProcessConfig,
    XcodeMcpBridgePool, XcodeMcpBridgePoolConfig,
};
use daemon::failed_serve;
use daemon::packaging::{self, DaemonMode, ModePaths};
use daemon::supervisor::{self, CrashBudgetDecision, PidLockOutcome};
#[cfg(unix)]
use daemon::xcode_shim_socket::{
    bind_xcode_shim_listener, cleanup_xcode_shim_socket, ensure_xcode_shim_dir,
    spawn_xcode_shim_socket_service, XcodeShimGrantRegistry,
};
use db::migrate::{self, MigrationError, MigrationOutcome};
use domain::lifecycle::{
    DaemonLifecycleState, FailureKind, XcodeBrokerHealthSnapshot, XcodeBrokerHealthState,
};
use engine::command_handler::CommandHandler;
use engine::event_bus::new_bus;
use engine::executor::BackgroundExecutor;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::orchestrator::Orchestrator;
use engine::recovery::RecoveryService;
use engine::work_queue::WorkQueue;

/// Exit code returned when the PID-lock path reports the anomalous
/// "flock held by someone, but the recorded PID is dead" case (§6.1).
/// Matches `sysexits.h::EX_TEMPFAIL` so supervisors can distinguish a
/// transient launch conflict from a normal daemon crash.
const EX_TEMPFAIL: i32 = 75;

#[tokio::main]
async fn main() -> Result<()> {
    // ── §7.1 mode + paths ──────────────────────────────────────────────
    // Resolve mode/paths first so the log-sink decision below can route
    // packaged modes to the app-support log file per §9.1.
    let mode_raw = std::env::var("MODE").unwrap_or_else(|_| "dev".to_string());
    let mode = DaemonMode::from_env_var(&mode_raw);
    let mut paths = packaging::resolve_paths(mode)?;
    packaging::enforce_loopback(&mut paths);

    // ── R13 OPS-002 / §7 packaged cwd contract ────────────────────────
    // Packaged modes must not inherit the dev-shell or source-checkout
    // cwd. Launchd usually starts the agent with cwd=`/` (sometimes
    // the operator's home), but the Debug `Process` fallback spawn
    // from `Chainworks_ForgeApp.swift` inherits the app's cwd. To
    // guarantee "zero source-checkout reads post-startup" we chdir to
    // `$HOME` before any runtime machinery spins up so relative
    // defaults (catalog YAMLs, workflow YAMLs, test fixtures) resolve
    // under a stable, operator-owned root rather than whatever the
    // launcher's cwd happened to be. Dev/test/MCP modes keep their
    // original cwd for `cargo run`/`cargo test` ergonomics — the
    // contract is explicitly packaged-only.
    if let Some(target) = packaging::packaged_cwd_target(mode, dirs::home_dir()) {
        if let Err(err) = std::env::set_current_dir(&target) {
            warn!(
                err = %err,
                target = %target.display(),
                "packaged mode: could not chdir to $HOME; continuing from launcher cwd"
            );
        }
    } else if mode.is_packaged() {
        warn!("packaged mode: could not resolve $HOME; skipping chdir");
    }

    // ── §9.1 packaged log writer with daily rotation ──────────────────
    // Dev/test/MCP stdio modes keep tracing on stderr so MCP stdio's
    // JSON-RPC protocol is not polluted. Packaged modes route structured
    // JSON-lines logs to `$HOME/Library/Logs/Chainworks Forge/daemon.log`
    // with daily rotation. `_log_guard` MUST stay alive for the process
    // lifetime — dropping it flushes buffered logs.
    let _log_guard: Option<tracing_appender::non_blocking::WorkerGuard> =
        init_tracing_for_mode(mode, paths.log_path.as_deref());
    // `build-sha.txt` is written after the daemon reaches `Ready` per P042
    // §9.4 so the Swift diagnostics bundle never reads a sha for a
    // process that never actually came up.
    info!(
        mode = %mode,
        database_url = %paths.database_url,
        bind_addr = %paths.bind_addr,
        app_support = %paths.app_support_dir.display(),
        "P042 paths resolved"
    );

    // ── Lifecycle reporter owns the authoritative DaemonStatus ─────────
    let events = new_bus(1024);
    let binary_schema_version = migrate::binary_schema_version();
    let build_sha = option_env!("GIT_SHA").unwrap_or("dev");
    let reporter = LifecycleReporter::new(binary_schema_version, build_sha, events.clone());
    reporter.set_state(DaemonLifecycleState::Starting);

    // ── §6.1 PID lock (packaged modes only) ────────────────────────────
    let _pid_lock = if mode.is_packaged() {
        match supervisor::acquire_pid_lock(&paths.pid_path()) {
            Ok(PidLockOutcome::Acquired(lock)) => {
                info!(path = %lock.path().display(), "PID lock acquired");
                Some(lock)
            }
            Ok(PidLockOutcome::DuplicateHealthy { peer_pid }) => {
                info!(
                    peer_pid,
                    "another packaged daemon is live — singleton policy, exit 0"
                );
                return Ok(());
            }
            Ok(PidLockOutcome::AnomalousHolder { recorded_pid }) => {
                error!(
                    recorded_pid,
                    path = %paths.pid_path().display(),
                    "PID lock anomalous: flock held but recorded PID is dead; exit {EX_TEMPFAIL}"
                );
                std::process::exit(EX_TEMPFAIL);
            }
            Err(e) => {
                error!(err = %e, "PID lock acquisition errored");
                return Err(anyhow::anyhow!("pid lock: {e}"));
            }
        }
    } else {
        None
    };

    // R13 API-001: failed-serve `/graphql` now requires a bearer
    // principal, so the principal table must be loaded BEFORE any
    // branch that short-circuits into `serve_failed`. Earlier the
    // table was loaded far below, after migration + runtime wiring,
    // which meant the failed-serve router for the crash-budget /
    // PID-lock / migration-failure paths never had a principal table
    // to consult and would have panicked on `Extension<PrincipalTable>`
    // extraction.
    let principal_table = auth::PrincipalTable::load_or_bootstrap(&paths.principals_path)?;
    info!(
        path = %paths.principals_path.display(),
        "Principal table loaded"
    );

    // ── §6.2 crash-loop budget: exhausted → failed-serve ───────────────
    if mode.is_packaged() {
        if let CrashBudgetDecision::Exhausted {
            count,
            first_crash_at,
        } = supervisor::read_crash_budget(&paths.crash_budget_path())
        {
            warn!(
                count,
                first_crash_at, "crash-loop budget exhausted — entering failed-serve mode per §6.2"
            );
            reporter.set_failed(
                FailureKind::CrashLoopBudgetExhausted,
                format!("{count} crashes; first at unix={first_crash_at}"),
                None,
            );
            return serve_failed(&reporter, &principal_table, &paths).await;
        }
    }

    // ── §8 typed migration preflight ───────────────────────────────────
    let preflight_outcome: MigrationOutcome =
        match migrate::run_preflight(&paths.database_url, None).await {
            Ok(outcome) => {
                info!(
                    classified_as = ?outcome.classified_as,
                    schema_version = outcome.schema_version,
                    applied_migrations = outcome.applied_migrations,
                    backup_path = ?outcome.backup_path,
                    "migration preflight succeeded"
                );
                reporter.set_schema_version(outcome.schema_version);
                outcome
            }
            Err(err) => {
                let (kind, detail, backup_path) = map_migration_error(&err);
                error!(
                    err = %err,
                    ?kind,
                    "migration preflight failed — entering §8.7 failed-serve mode"
                );
                reporter.set_failed(kind, detail, backup_path);
                return serve_failed(&reporter, &principal_table, &paths).await;
            }
        };

    // Open the pool now that preflight has validated the DB.
    let pool = db::pool::create_pool(&paths.database_url).await?;
    info!(
        schema_version = preflight_outcome.schema_version,
        "SQLite pool opened"
    );

    // ── Steward runtime + control-plane wiring ─────────────────────────
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

    let work_queue = WorkQueue::new(pool.clone());
    let acp = Arc::new(AcpRuntimeManager::new());
    let cmd_handler = Arc::new(CommandHandler::new_with_acp(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
        acp.clone(),
    ));
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = Arc::new(BackgroundExecutor::new_with_steward_runtime_inputs(
        pool.clone(),
        work_queue.clone(),
        orchestrator.clone(),
        acp.clone(),
        events.clone(),
        steward_runtime_inputs.clone(),
    ));
    // Startup recovery: repair any run left mid-flight by a previous crash.
    let recovery = RecoveryService::new(pool.clone(), work_queue.clone(), events.clone());
    let summary = recovery.run_startup_repair().await?;
    info!(
        runs_inspected = summary.runs_inspected,
        runs_repaired = summary.runs_repaired,
        work_items_requeued = summary.work_items_requeued,
        "startup recovery complete"
    );

    // (Principal table is loaded up-front so failed-serve branches have
    // it available; see the R13 API-001 comment at the top of `main`.)

    // ── Mode dispatch ──────────────────────────────────────────────────
    // Per P042 §5.1 `Ready` is emitted only AFTER HTTP bind. MCP stdio
    // mode has no HTTP bind, so `Ready` fires before `run_stdio` there.
    match mode {
        DaemonMode::Mcp => {
            let _executor_handle = executor.start();
            info!("BackgroundExecutor started");
            reporter.set_state(DaemonLifecycleState::Ready);
            if let Err(e) = packaging::write_build_sha(&paths) {
                warn!(err = %e, "failed to write build-sha.txt (non-fatal)");
            }
            let mcp = mcp_server::server::McpServer::new(
                pool.clone(),
                cmd_handler.clone(),
                principal_table,
            );
            mcp.run_stdio().await?;
        }
        _ => {
            // Daemon mode: GraphQL + MCP HTTP on the same port.
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
                reporter.clone(),
            );

            // §7.3 port fallback: bind the listener here so the
            // 4000→ephemeral fallback + `daemon.port` write runs before the
            // GraphQL server takes the socket.
            let (listener, port) = packaging::bind_with_fallback(&paths).await?;
            info!(port, "bound HTTP listener; handing off to graphql-server");

            let xcode_broker_pool = Arc::new(XcodeMcpBridgePool::new_with_sink(
                XcodeMcpBridgePoolConfig {
                    base_url: format!("http://127.0.0.1:{port}/xcode-mcp"),
                    broker_disabled: std::env::var("CHAINWORKS_XCODE_BROKER_DISABLED")
                        .map(|value| value == "1")
                        .unwrap_or(false),
                    use_local_host_probe: true,
                    ..Default::default()
                },
                acp.xcode_runtime_observation_sink(),
            ));
            acp.set_xcode_broker_lease_attacher(xcode_broker_pool.clone());
            reporter.set_xcode_broker_health(xcode_broker_health_for_lifecycle(
                xcode_broker_pool.health_snapshot().await,
            ));
            spawn_xcode_broker_health_publisher(reporter.clone(), xcode_broker_pool.clone());

            #[cfg(unix)]
            let xcode_shim_socket_service = {
                let shim_registry = Arc::new(XcodeShimGrantRegistry::default());
                let shim_socket_path = XcodeShimGrantRegistry::socket_path(&paths.app_support_dir);
                let shim_dir = ensure_xcode_shim_dir(&paths.app_support_dir)?;
                let listener = bind_xcode_shim_listener(&shim_socket_path)?;
                acp.set_xcode_shim_runtime(
                    shim_registry.clone(),
                    shim_socket_path.to_string_lossy().into_owned(),
                    shim_dir.to_string_lossy().into_owned(),
                );
                info!(
                    socket_path = %shim_socket_path.display(),
                    shim_dir = %shim_dir.display(),
                    "Xcode shim dispatch socket bound"
                );
                (
                    shim_socket_path,
                    spawn_xcode_shim_socket_service(
                        listener,
                        shim_registry,
                        Arc::new(XcodeHostExecutorProcessConfig::default()),
                        acp.xcode_runtime_observation_sink(),
                        Arc::new(DefaultXcodeShimProcessInspector),
                    ),
                )
            };

            let _executor_handle = executor.start();
            info!("BackgroundExecutor started");

            // §5.1: Ready only AFTER HTTP bind so a client receiving
            // `state=ready` is guaranteed it can connect.
            reporter.set_state(DaemonLifecycleState::Ready);

            // §9.4: write build-sha.txt after successful Ready so the
            // Swift diagnostics bundle only ever reports a sha for a
            // process that actually came up.
            if let Err(e) = packaging::write_build_sha(&paths) {
                warn!(err = %e, "failed to write build-sha.txt (non-fatal)");
            }

            // §6.2 crash-budget reset: after the daemon has been
            // continuously in `Ready` for `CRASH_BUDGET_RESET_AFTER_READY_SECS`,
            // clear `crash-budget.json`. Only meaningful in packaged
            // modes — dev/test modes neither read nor write the file.
            if mode.is_packaged() {
                let budget_path = paths.crash_budget_path();
                let reset_reporter = reporter.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(
                        supervisor::CRASH_BUDGET_RESET_AFTER_READY_SECS,
                    ))
                    .await;
                    // Only reset if we are STILL Ready — a transient
                    // Degraded that returned to Ready resets the window
                    // in the lifecycle reporter but shouldn't count as
                    // "stable Ready uptime" for the crash-budget file.
                    if reset_reporter.snapshot().state == DaemonLifecycleState::Ready {
                        match supervisor::reset_crash_budget(&budget_path) {
                            Ok(()) => info!(
                                path = %budget_path.display(),
                                "crash-budget reset after stable Ready window"
                            ),
                            Err(e) => warn!(
                                path = %budget_path.display(),
                                err = %e,
                                "crash-budget reset failed (non-fatal)"
                            ),
                        }
                    }
                });
            }

            // §6.3 graceful shutdown: serve until SIGTERM / SIGINT, then
            // transition to `Shutdown` and give axum's graceful_shutdown
            // at most 5 seconds to finish in-flight requests. Clean
            // drain → exit 0; deadline hit → exit 75 (`EX_TEMPFAIL`) so
            // the supervisor can distinguish a stuck daemon from a
            // voluntary shutdown.
            let shutdown_reporter = reporter.clone();
            let shutdown_acp = Arc::clone(&acp);
            let (drain_tx, drain_rx) = tokio::sync::oneshot::channel::<()>();
            let shutdown_signal = async move {
                wait_for_shutdown_signal().await;
                info!("shutdown signal received; transitioning to Shutdown");
                shutdown_reporter.set_state(DaemonLifecycleState::Shutdown);
                match tokio::time::timeout(
                    SHUTDOWN_DRAIN_DEADLINE,
                    shutdown_acp.close_all_sessions(),
                )
                .await
                {
                    Ok(closed_sessions) => info!(
                        closed_sessions,
                        "closed live ACP sessions during daemon shutdown"
                    ),
                    Err(_) => warn!(
                        deadline_secs = SHUTDOWN_DRAIN_DEADLINE.as_secs(),
                        "timed out closing live ACP sessions during daemon shutdown"
                    ),
                }
                // Notify the main task that the drain window has started.
                // Ignore the send error — it only fails if the receiver
                // was dropped, which means serve is already finishing.
                let _ = drain_tx.send(());
            };

            let serve_fut = async {
                let extra_routes =
                    mcp_routes.merge(daemon::xcode_broker_http::routes(xcode_broker_pool.clone()));
                graphql_server::server::serve_with_listener_until(
                    schema,
                    listener,
                    extra_routes,
                    principal_table,
                    reporter.clone(),
                    shutdown_signal,
                )
                .await
                .map_err(anyhow::Error::from)
            };

            let drain_outcome =
                run_drain_protocol(serve_fut, drain_rx, SHUTDOWN_DRAIN_DEADLINE).await?;
            #[cfg(unix)]
            shutdown_xcode_shim_socket_service(xcode_shim_socket_service).await;

            match drain_outcome {
                DrainOutcome::ServeReturnedFirst => {
                    info!("HTTP serve returned before shutdown signal; exit 0");
                }
                DrainOutcome::DrainedWithinDeadline => info!(
                    deadline_secs = SHUTDOWN_DRAIN_DEADLINE.as_secs(),
                    "HTTP serve drained cleanly within deadline; exit 0"
                ),
                DrainOutcome::DrainDeadlineExceeded => {
                    error!(
                        deadline_secs = SHUTDOWN_DRAIN_DEADLINE.as_secs(),
                        "graceful shutdown did not drain within deadline; exit {EX_TEMPFAIL}"
                    );
                    std::process::exit(EX_TEMPFAIL);
                }
            }
        }
    }

    Ok(())
}

fn spawn_xcode_broker_health_publisher(reporter: LifecycleReporter, pool: Arc<XcodeMcpBridgePool>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            reporter.set_xcode_broker_health(xcode_broker_health_for_lifecycle(
                pool.health_snapshot().await,
            ));
        }
    });
}

fn xcode_broker_health_for_lifecycle(
    health: acp::XcodeBrokerHealthSnapshot,
) -> XcodeBrokerHealthSnapshot {
    XcodeBrokerHealthSnapshot {
        state: match health.state {
            acp::XcodeBrokerHealthState::Disabled => XcodeBrokerHealthState::Disabled,
            acp::XcodeBrokerHealthState::Healthy => XcodeBrokerHealthState::Healthy,
            acp::XcodeBrokerHealthState::Degraded => XcodeBrokerHealthState::Degraded,
            acp::XcodeBrokerHealthState::Failed => XcodeBrokerHealthState::Failed,
        },
        pool_id: health.pool_id,
        active_leases: health.active_leases,
        queued_leases: health.queued_leases,
        max_active_leases: health.max_active_leases,
        max_queued_leases: health.max_queued_leases,
        broker_disabled: health.broker_disabled,
    }
}

#[cfg(unix)]
async fn shutdown_xcode_shim_socket_service(
    (socket_path, handle): (std::path::PathBuf, tokio::task::JoinHandle<()>),
) {
    handle.abort();
    let _ = handle.await;
    match cleanup_xcode_shim_socket(&socket_path) {
        Ok(()) => info!(
            socket_path = %socket_path.display(),
            "Xcode shim dispatch socket cleaned up"
        ),
        Err(error) => warn!(
            error = %error,
            socket_path = %socket_path.display(),
            "Failed to clean up Xcode shim dispatch socket"
        ),
    }
}

/// P042 §6.3 drain deadline. Supervisor sends SIGTERM → daemon flips to
/// `Shutdown` and has this long to finish in-flight requests. Exceeding
/// it maps to `EX_TEMPFAIL` so the supervisor knows to force-kill.
const SHUTDOWN_DRAIN_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// Outcome of the two-phase shutdown protocol. Extracted from `main`
/// so the drain-deadline + exit-75 contract can be unit-tested without
/// binding a real HTTP listener.
#[derive(Debug, PartialEq, Eq)]
enum DrainOutcome {
    /// Serve returned before the shutdown signal fired (listener error,
    /// panic in handler). Exit 0.
    ServeReturnedFirst,
    /// Shutdown signal fired and the serve future finished draining
    /// within `SHUTDOWN_DRAIN_DEADLINE`. Exit 0.
    DrainedWithinDeadline,
    /// Shutdown signal fired and the serve future was still running
    /// when the deadline elapsed. Caller should exit 75.
    DrainDeadlineExceeded,
}

/// Two-phase shutdown protocol (P042 §6.3). Phase 1 waits for either the
/// serve future to complete on its own or the drain signal to fire;
/// Phase 2 imposes `SHUTDOWN_DRAIN_DEADLINE` on the serve future once
/// the drain starts. `serve_fut` is expected to finish after the
/// `shutdown_signal` that powers `drain_rx` completes (axum's
/// `with_graceful_shutdown` semantics).
async fn run_drain_protocol<F>(
    serve_fut: F,
    drain_rx: tokio::sync::oneshot::Receiver<()>,
    deadline: std::time::Duration,
) -> anyhow::Result<DrainOutcome>
where
    F: std::future::Future<Output = anyhow::Result<()>>,
{
    tokio::pin!(serve_fut);
    tokio::pin!(drain_rx);

    let drain_started = tokio::select! {
        result = &mut serve_fut => {
            result?;
            return Ok(DrainOutcome::ServeReturnedFirst);
        }
        _ = &mut drain_rx => true,
    };
    debug_assert!(drain_started);

    match tokio::time::timeout(deadline, &mut serve_fut).await {
        Ok(result) => {
            result?;
            Ok(DrainOutcome::DrainedWithinDeadline)
        }
        Err(_elapsed) => Ok(DrainOutcome::DrainDeadlineExceeded),
    }
}

/// Initialize tracing for the daemon. Dev/test/MCP stdio modes emit
/// human-readable logs to stderr; packaged modes emit JSON-lines logs to
/// a daily-rotated file under `$HOME/Library/Logs/Chainworks Forge/`.
///
/// Returns an optional `WorkerGuard` that must stay alive for the
/// process lifetime — dropping it flushes the non-blocking appender's
/// buffered lines.
fn init_tracing_for_mode(
    mode: DaemonMode,
    log_path: Option<&std::path::Path>,
) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use daemon::log_redaction::RedactingMakeWriter;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let home = dirs::home_dir()
        .and_then(|p| p.to_str().map(str::to_string))
        .unwrap_or_default();

    match (mode.is_packaged(), log_path) {
        (true, Some(log_path)) => {
            // Ensure the log directory exists; if create_dir_all fails
            // fall through to stderr so the daemon still starts.
            let parent = log_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."));
            if std::fs::create_dir_all(parent).is_err() {
                eprintln!(
                    "daemon: could not create log dir {}; falling back to stderr",
                    parent.display()
                );
                // §9.2: every byte passes through the redactor before
                // reaching the sink, even on the fallback path.
                let writer = RedactingMakeWriter::new(std::io::stderr, home.clone());
                tracing_subscriber::fmt()
                    .with_writer(writer)
                    .with_env_filter(filter)
                    .init();
                return None;
            }
            let file_name = log_path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("daemon.log");
            // §9.1: sweep the rotated-log archive before the new
            // writer starts. The rolling appender itself never
            // deletes old files, so without this sweep a daily-run
            // daemon accumulates unbounded disk usage. The sweep is
            // best-effort: a read_dir/remove_file failure never
            // blocks startup.
            let retention_report = daemon::log_retention::enforce_retention(parent, file_name);
            let appender = tracing_appender::rolling::daily(parent, file_name);
            let (non_blocking, guard) = tracing_appender::non_blocking(appender);
            // §9.2: wrap the rolling appender in the redactor so the
            // JSON writer's output is scrubbed in-place.
            let writer = RedactingMakeWriter::new(non_blocking, home);
            tracing_subscriber::fmt()
                .json()
                .with_writer(writer)
                .with_env_filter(filter)
                .init();
            // Log the sweep outcome under the live subscriber so
            // operators can see retention behavior in the packaged
            // log itself. Emitting after `init()` means the event
            // participates in redaction + rolling just like any
            // other daemon event.
            if retention_report.rotated_found > 0 || retention_report.bytes_freed > 0 {
                tracing::info!(
                    rotated_found = retention_report.rotated_found,
                    deleted_by_age = retention_report.deleted_by_age,
                    deleted_by_count = retention_report.deleted_by_count,
                    deleted_by_size = retention_report.deleted_by_size,
                    bytes_freed = retention_report.bytes_freed,
                    "packaged log retention sweep completed"
                );
            }
            Some(guard)
        }
        _ => {
            let writer = RedactingMakeWriter::new(std::io::stderr, home);
            tracing_subscriber::fmt()
                .with_writer(writer)
                .with_env_filter(filter)
                .init();
            None
        }
    }
}

/// Wait for SIGTERM (packaged supervisor) or SIGINT (developer Ctrl-C).
/// Resolves as soon as either signal is observed.
#[cfg(unix)]
async fn wait_for_shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    tokio::select! {
        _ = sigterm.recv() => info!("SIGTERM received"),
        _ = sigint.recv() => info!("SIGINT received"),
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    info!("Ctrl-C received");
}

/// Map a typed [`MigrationError`] to the `(FailureKind, detail, backup_path)`
/// triple per P042 §8.4. Keeps the mapping in one place so the test matrix
/// can assert on it directly.
fn map_migration_error(err: &MigrationError) -> (FailureKind, String, Option<String>) {
    match err {
        MigrationError::ExistingWithoutTracker(msg) => {
            (FailureKind::MigrationFailed, msg.clone(), None)
        }
        MigrationError::NewerThanBinary {
            applied_max,
            binary_max,
        } => (
            FailureKind::SchemaNewerThanBinary,
            format!("applied_max={applied_max}, binary_max={binary_max}"),
            None,
        ),
        MigrationError::InterleavedDivergence { extras } => (
            FailureKind::MigrationFailed,
            format!("interleaved divergence; extras={extras:?}"),
            None,
        ),
        MigrationError::BackupFailed(msg) => (FailureKind::BackupFailed, msg.clone(), None),
        MigrationError::LockFailed(msg) => {
            // P042 §8.4: LockFailed maps to MigrationFailed with the
            // "lock acquisition failed: …" detail prefix. The backup_path
            // is embedded in the msg when we failed on the subset branch.
            let backup_path = extract_backup_path_hint(msg);
            (
                FailureKind::MigrationFailed,
                format!("lock acquisition failed: {msg}"),
                backup_path,
            )
        }
        MigrationError::ApplyFailed(msg) => {
            // R13 OPS-001 / §8.4: a tracked-subset branch `ApplyFailed`
            // runs after `write_backup` has already produced the backup
            // file. `run_preflight` now folds the backup path into the
            // ApplyFailed detail with the same "(backup at <path>)"
            // suffix used for LockFailed — reuse the shared extractor
            // so `DaemonStatus.failure.backup_path` stays populated for
            // the operator recovery UI.
            let backup_path = extract_backup_path_hint(msg);
            (FailureKind::MigrationFailed, msg.clone(), backup_path)
        }
        MigrationError::IoError(msg) => (FailureKind::MigrationFailed, msg.clone(), None),
    }
}

/// Extract a `(backup at <path>)` hint from a LockFailed detail string per
/// `run_preflight`'s subset-branch error wrapping. Returns `None` if the
/// hint is absent; the failed-serve UI then shows `backup_path: null`.
fn extract_backup_path_hint(msg: &str) -> Option<String> {
    let start = msg.find("(backup at ")?;
    let after = &msg[start + "(backup at ".len()..];
    let end = after.find(')')?;
    Some(after[..end].to_string())
}

/// Transition handler for any §8.4 terminal condition: run the §8.7
/// failed-serve loop. Uses `packaging::bind_with_fallback` so the
/// preferred port → ephemeral-loopback fallback + `daemon.port` write
/// happens for the Failed path too — the Swift client's port-discovery
/// is therefore uniform for Ready and Failed servers.
async fn serve_failed(
    reporter: &LifecycleReporter,
    principal_table: &auth::PrincipalTable,
    paths: &ModePaths,
) -> Result<()> {
    let (listener, port) = packaging::bind_with_fallback(paths).await?;
    info!(
        port,
        bind_addr = %paths.bind_addr,
        "failed-serve: bound listener; daemon.port written so clients can discover the status surface"
    );
    failed_serve::serve_failed_state_with_listener(
        reporter.clone(),
        principal_table.clone(),
        listener,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_migration_error_covers_every_variant() {
        let (k1, _, b1) = map_migration_error(&MigrationError::ExistingWithoutTracker("x".into()));
        assert_eq!(k1, FailureKind::MigrationFailed);
        assert!(b1.is_none());

        let (k2, _, b2) = map_migration_error(&MigrationError::NewerThanBinary {
            applied_max: 42,
            binary_max: 14,
        });
        assert_eq!(k2, FailureKind::SchemaNewerThanBinary);
        assert!(b2.is_none());

        let (k3, _, _) =
            map_migration_error(&MigrationError::InterleavedDivergence { extras: vec![99] });
        assert_eq!(k3, FailureKind::MigrationFailed);

        let (k4, _, _) = map_migration_error(&MigrationError::BackupFailed("disk full".into()));
        assert_eq!(k4, FailureKind::BackupFailed);

        let (k5, _, _) = map_migration_error(&MigrationError::ApplyFailed("syntax".into()));
        assert_eq!(k5, FailureKind::MigrationFailed);

        let (k6, _, _) = map_migration_error(&MigrationError::IoError("stat".into()));
        assert_eq!(k6, FailureKind::MigrationFailed);
    }

    #[test]
    fn map_migration_error_lock_failed_extracts_backup_path_when_present() {
        let err = MigrationError::LockFailed("db is locked (backup at /tmp/bk.sqlite)".into());
        let (kind, detail, backup) = map_migration_error(&err);
        assert_eq!(kind, FailureKind::MigrationFailed);
        assert!(detail.starts_with("lock acquisition failed:"));
        assert_eq!(backup, Some("/tmp/bk.sqlite".into()));
    }

    #[test]
    fn map_migration_error_lock_failed_without_backup_hint_has_none() {
        let err = MigrationError::LockFailed("db is locked".into());
        let (_, _, backup) = map_migration_error(&err);
        assert!(backup.is_none());
    }

    /// R13 OPS-001: the most common tracked-subset branch failure is
    /// `ApplyFailed` (a migration raised a runtime error). The
    /// preflight now folds the backup path into the error detail with
    /// the same "(backup at <path>)" suffix used for LockFailed; this
    /// test pins the extractor + mapping behaviour for that path.
    #[test]
    fn map_migration_error_apply_failed_extracts_backup_path_when_present() {
        let err = MigrationError::ApplyFailed(
            "migration 004_worktree_support.sql: syntax error at ALTER TABLE (backup at /tmp/p042/bk.sqlite)"
                .into(),
        );
        let (kind, detail, backup) = map_migration_error(&err);
        assert_eq!(kind, FailureKind::MigrationFailed);
        assert!(
            detail.contains("syntax error"),
            "ApplyFailed detail must still carry the underlying message: {detail}"
        );
        assert_eq!(
            backup,
            Some("/tmp/p042/bk.sqlite".into()),
            "DaemonStatus.failure.backup_path must expose the recovered backup file"
        );
    }

    #[test]
    fn map_migration_error_apply_failed_without_backup_hint_stays_none() {
        let err = MigrationError::ApplyFailed("generic apply failure".into());
        let (_, _, backup) = map_migration_error(&err);
        assert!(backup.is_none());
    }

    // ── P042 §6.3 shutdown protocol ───────────────────────────────────

    #[tokio::test]
    async fn shutdown_drain_completes_within_deadline_exits_zero() {
        // Serve finishes ~100ms after the drain signal. With a 1s deadline
        // this must return DrainedWithinDeadline.
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let serve = async move {
            // Wait for the "signal has fired" flag, then pretend to drain
            // for a short time. This mimics axum::serve returning after
            // graceful_shutdown completes.
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            Ok::<(), anyhow::Error>(())
        };
        // Fire the drain signal immediately so serve's sleep races against it.
        let _ = tx.send(());
        let outcome = run_drain_protocol(serve, rx, std::time::Duration::from_secs(1))
            .await
            .unwrap();
        assert_eq!(outcome, DrainOutcome::DrainedWithinDeadline);
    }

    #[tokio::test]
    async fn shutdown_drain_exceeds_deadline_reports_timeout() {
        // Serve never returns; deadline is very short. This must
        // return DrainDeadlineExceeded so `main.rs` can exit 75.
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let serve = async move {
            // Hang forever — simulate a stuck in-flight request that
            // refuses to finish within the drain window.
            std::future::pending::<()>().await;
            Ok::<(), anyhow::Error>(())
        };
        let _ = tx.send(());
        let outcome = run_drain_protocol(serve, rx, std::time::Duration::from_millis(100))
            .await
            .unwrap();
        assert_eq!(outcome, DrainOutcome::DrainDeadlineExceeded);
    }

    #[tokio::test]
    async fn shutdown_serve_returning_before_signal_reports_serve_first() {
        // Serve finishes on its own (e.g. listener error) before the
        // drain signal is sent. The drain-deadline must not fire.
        let (_tx, rx) = tokio::sync::oneshot::channel::<()>();
        let serve = async {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            Ok::<(), anyhow::Error>(())
        };
        let outcome = run_drain_protocol(serve, rx, std::time::Duration::from_secs(10))
            .await
            .unwrap();
        assert_eq!(outcome, DrainOutcome::ServeReturnedFirst);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_xcode_shim_socket_service_removes_socket_path() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let socket_path = tempdir.path().join("xcode-shim.sock");
        std::fs::write(&socket_path, "").expect("write socket placeholder");
        let handle = tokio::spawn(async {
            std::future::pending::<()>().await;
        });

        shutdown_xcode_shim_socket_service((socket_path.clone(), handle)).await;

        assert!(!socket_path.exists());
    }

    // ── P042 §9.1 packaged log routing (routing-only; global subscriber
    // init can only happen once per process, so we verify the decision
    // branch returns the expected sink shape without actually installing
    // a subscriber in tests). ─────────────────────────────────────────

    #[test]
    fn packaged_mode_with_log_path_would_install_rolling_appender() {
        // Smoke test for the packaging path helper: packaged modes must
        // resolve a non-None `log_path`, and that path must live under
        // `$HOME/Library/Logs/Chainworks Forge/`. The subscriber install
        // itself is covered by runtime in the daemon entrypoint.
        let paths = daemon::packaging::resolve_paths(daemon::packaging::DaemonMode::PackagedApp)
            .expect("resolve_paths for packaged-app");
        let log = paths.log_path.expect("packaged modes must have a log path");
        let as_str = log.to_string_lossy();
        assert!(
            as_str.contains("Library/Logs/Chainworks Forge"),
            "packaged log path must live under app-support Logs: {as_str}"
        );
        assert!(
            as_str.ends_with("daemon.log"),
            "packaged log file name must be daemon.log: {as_str}"
        );
    }

    #[test]
    fn dev_mode_has_no_log_path_so_stderr_is_used() {
        let paths = daemon::packaging::resolve_paths(daemon::packaging::DaemonMode::Dev)
            .expect("resolve_paths for dev");
        assert!(
            paths.log_path.is_none(),
            "dev mode must not allocate a file log sink (stderr only)"
        );
    }
}
