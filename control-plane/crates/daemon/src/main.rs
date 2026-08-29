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
//! 3. `supervisor::acquire_database_lock` — authoritative singleton per
//!    SQLite DB file. This catches mixed launch paths, for example a
//!    debug `target/debug/control-plane` pointed at the packaged DB and
//!    the bundled helper trying to start on a fallback port.
//! 4. `supervisor::acquire_pid_lock` — advisory singleton per app-support
//!    root (§6.1): `DuplicateHealthy` → exit 0; `AnomalousHolder` →
//!    exit 75 (`EX_TEMPFAIL`); `Acquired` → continue.
//! 5. `supervisor::read_crash_budget` — ≥5 crashes within 60 s puts the
//!    daemon into §8.7 failed-serve mode with
//!    `FailureKind::CrashLoopBudgetExhausted` instead of a normal startup.
//! 6. `db::migrate::run_preflight` — typed three-branch migration classifier.
//!    Any error maps (§8.4) to a `FailureKind` and the daemon drops into
//!    failed-serve mode rather than panicking, so `/health`, `/ready`,
//!    and `daemonStatus` still report the terminal condition.
//! 7. `packaging::bind_with_fallback` — try preferred port, fall back to
//!    ephemeral loopback in packaged modes; write the chosen port to
//!    `daemon.port` so the Swift client can locate us (§7.3).
//! 8. `packaging::write_build_sha` — record the embedded build SHA for
//!    the Swift diagnostics bundle (§9.4).
//!
//! If any §8.4 terminal condition is hit, the daemon transitions the
//! lifecycle reporter to `Failed { failure: Some(..) }` and calls
//! `failed_serve::serve_failed_state`; the ACP runtime, background
//! executor, steward runtime, and GraphQL/MCP mutations are never
//! constructed in that path.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{Context, Result};
use tracing::{error, info, warn};

use acp::{
    AcpRuntimeManager, DefaultXcodeShimProcessInspector, XcodeHostExecutorProcessConfig,
    XcodeMcpBridgePool, XcodeMcpBridgePoolConfig, XcodeRuntimeObservationSink,
};
use daemon::failed_serve;
use daemon::packaging::{self, DaemonMode, ModePaths};
use daemon::supervisor::{self, CrashBudgetDecision, PidLockOutcome};
#[cfg(unix)]
use daemon::xcode_shim_socket::{
    bind_xcode_shim_listener, cleanup_xcode_shim_socket, ensure_xcode_shim_dir,
    spawn_xcode_shim_socket_service, XcodeShimGrantRegistry,
};
use db::{
    migrate::{self, MigrationError, MigrationOutcome},
    repos::{scheduler, work_items},
};
use domain::lifecycle::{
    DaemonLifecycleState, DegradedKind, FailureKind, XcodeBrokerHealthSnapshot,
    XcodeBrokerHealthState,
};
use domain::provider::InvokeAgentCapacityConfig;
use engine::command_handler::CommandHandler;
use engine::event_bus::new_bus;
use engine::executor::BackgroundExecutor;
use engine::host_interruption::{spawn_runtime_heartbeat_monitor, HostInterruptionService};
use engine::lifecycle_reporter::LifecycleReporter;
use engine::orchestrator::Orchestrator;
use engine::recovery::RecoveryService;
use engine::work_queue::WorkQueue;
use sqlx::SqlitePool;

/// Exit code returned when the PID-lock path reports the anomalous
/// "flock held by someone, but the recorded PID is dead" case (§6.1).
/// Matches `sysexits.h::EX_TEMPFAIL` so supervisors can distinguish a
/// transient launch conflict from a normal daemon crash.
const EX_TEMPFAIL: i32 = 75;
const DAEMON_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

struct StartupProbeServer {
    listener: tokio::net::TcpListener,
    port: u16,
    shutdown: tokio::sync::oneshot::Sender<()>,
    handle: tokio::task::JoinHandle<()>,
}

async fn start_startup_probe_server(
    paths: &ModePaths,
    reporter: LifecycleReporter,
    build_sha: &str,
) -> Result<StartupProbeServer> {
    let (listener, port) = packaging::bind_with_fallback(paths).await?;
    packaging::write_daemon_endpoint_snapshot(paths, std::process::id(), port, build_sha)?;

    let std_listener = listener
        .into_std()
        .context("convert startup probe listener to std listener")?;
    let probe_std_listener = std_listener
        .try_clone()
        .context("clone startup probe listener")?;
    let listener = tokio::net::TcpListener::from_std(std_listener)
        .context("convert full server listener back to tokio")?;
    let probe_listener = tokio::net::TcpListener::from_std(probe_std_listener)
        .context("convert startup probe listener clone to tokio")?;

    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let probe_reporter = reporter.clone();
    let handle = tokio::spawn(async move {
        let app = graphql_server::server::build_probe_router(probe_reporter);
        let addr = probe_listener.local_addr().ok();
        info!(
            addr = ?addr,
            "startup probe server listening on /health and /ready"
        );
        let result = axum::serve(probe_listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
        if let Err(err) = result {
            warn!(err = %err, "startup probe server stopped with error");
        }
    });

    Ok(StartupProbeServer {
        listener,
        port,
        shutdown,
        handle,
    })
}

async fn stop_startup_probe_server(server: StartupProbeServer) -> tokio::net::TcpListener {
    let StartupProbeServer {
        listener,
        shutdown,
        handle,
        ..
    } = server;
    let _ = shutdown.send(());
    match tokio::time::timeout(std::time::Duration::from_secs(2), handle).await {
        Ok(Ok(())) => info!("startup probe server stopped before full HTTP handoff"),
        Ok(Err(err)) => warn!(err = %err, "startup probe server task join failed"),
        Err(_) => warn!("startup probe server did not stop before full HTTP handoff deadline"),
    }
    listener
}

fn main() -> Result<()> {
    // P080: --p080-rollout-control-seed is a one-shot admin subcommand.
    // It seeds the rollout-control matrix (rejected if already seeded) and exits.
    // This implements the Phase0/1 explicit operator seed path from proposal §Feature Flags.
    if std::env::args().any(|a| a == "--p080-rollout-control-seed") {
        return tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_stack_size(DAEMON_WORKER_STACK_BYTES)
            .build()
            .context("build tokio runtime for seed command")?
            .block_on(run_p080_rollout_control_seed());
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(DAEMON_WORKER_STACK_BYTES)
        .build()
        .context("build daemon tokio runtime")?
        .block_on(run_daemon())
}

/// One-shot P080 rollout-control seeding subcommand.
///
/// Runs migration preflight, then calls `seed_rollout_control_if_absent` (no-op if
/// already seeded) and validates completeness. Exits 0 on success, non-zero on failure.
/// Rejected with an error if the rollout-control table already has rows.
async fn run_p080_rollout_control_seed() -> Result<()> {
    let mode_raw = std::env::var("MODE").unwrap_or_else(|_| "dev".to_string());
    let mode = packaging::DaemonMode::from_env_var(&mode_raw);
    let paths = packaging::resolve_paths(mode)?;

    // Run migration preflight before opening the runtime pool.
    migrate::run_preflight(&paths.database_url, None)
        .await
        .map_err(|e| anyhow::anyhow!("Migration preflight failed: {e:?}"))?;

    let pool = db::pool::create_pool(&paths.database_url).await?;

    let n = db::repos::p080::seed_rollout_control_if_absent(&pool).await?;
    if n == 0 {
        anyhow::bail!(
            "P080 rollout-control table already seeded; \
             --p080-rollout-control-seed is rejected when rows are present"
        );
    }
    db::repos::p080::validate_rollout_control_completeness(&pool).await?;
    println!("P080 rollout-control matrix seeded ({n} rows). Validation passed.");
    Ok(())
}

/// P089 §6.1: eagerly materializes the process-scoped temp-artifact-inventory
/// redaction key at daemon startup, ahead of any inventory scan or lazy
/// first-use `compute_path_hash` call. Extracted as its own function so the
/// startup contract is directly unit-testable (see `tests` below).
fn init_p089_redaction_key_at_startup() {
    domain::temp_artifact_inventory::init_process_path_hash_key();
}

async fn run_daemon() -> Result<()> {
    // P089 §6.1: materialize the process-scoped HMAC redaction key eagerly, before
    // any inventory scan could reach `compute_path_hash` and trigger lazy first-use
    // init. Must run before the backend below (or any other P089 lane) is reachable.
    init_p089_redaction_key_at_startup();

    // P089: install the MCP-backed temp artifact inventory backend so the GraphQL
    // resolver reuses the exact mode-check, validation, redaction, permit-guard, and
    // scanner path as the MCP tool and resource lane (readback parity requirement).
    graphql_server::types::temp_artifact_inventory::install_backend(std::sync::Arc::new(
        mcp_server::tools::temp_artifacts::McpTempArtifactInventoryBackend,
    ));

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
    let build_sha = packaging::resolved_build_sha();
    let reporter = LifecycleReporter::new(binary_schema_version, build_sha, events.clone());
    reporter.set_state(DaemonLifecycleState::Starting);

    // ── DB singleton lock (all file-backed modes) ──────────────────────
    let _database_lock = match supervisor::acquire_database_lock(&paths.database_url) {
        Ok(Some(PidLockOutcome::Acquired(lock))) => {
            info!(path = %lock.path().display(), "database singleton lock acquired");
            Some(lock)
        }
        Ok(Some(PidLockOutcome::DuplicateHealthy { peer_pid })) => {
            info!(
                peer_pid,
                "another daemon already owns this database — singleton policy, exit 0"
            );
            return Ok(());
        }
        Ok(Some(PidLockOutcome::AnomalousHolder { recorded_pid })) => {
            error!(
                recorded_pid,
                database_url = %paths.database_url,
                "database singleton lock anomalous: flock held but recorded PID is dead; exit {EX_TEMPFAIL}"
            );
            std::process::exit(EX_TEMPFAIL);
        }
        Ok(None) => None,
        Err(e) => {
            error!(err = %e, "database singleton lock acquisition errored");
            return Err(anyhow::anyhow!("database singleton lock: {e}"));
        }
    };

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

    // P081 Phase 3: Construct the shared immutable BoundaryPolicy service at startup.
    // One instance is built here and shared (via Arc) across GraphQL, MCP, and approval paths.
    // Request paths never reload or re-parse the fixture; only daemon restart rebuilds the service.
    //
    // CHAINWORKS_BOUNDARY_POLICY: startup-only mode override.
    //   shadow           — (default) evaluate matrix but do not enforce; violations logged only
    //   enforce          — matrix decisions are authoritative (requires Phase 4 exit evidence)
    //   read_only_safe_mode — deny mutations; permit reads with embedded fixture
    //   legacy/legacy_compat — disable P081 matrix; legacy guards remain authoritative
    //
    // CHAINWORKS_BOUNDARY_POLICY_PATH: path to a deployed fixture JSON file.
    //   If set, daemon attempts to load and validate that fixture first.
    //   If invalid, daemon falls back to embedded fixture + read_only_safe_mode.
    let boundary_policy = std::sync::Arc::new({
        let mode_override = match std::env::var("CHAINWORKS_BOUNDARY_POLICY") {
            Err(_) => None,
            Ok(v) => {
                let parsed = auth::boundary::PolicyMode::from_env_value(&v);
                if parsed.is_none() {
                    warn!(
                        value = %v,
                        "CHAINWORKS_BOUNDARY_POLICY has unrecognized value; \
                         entering read_only_safe_mode. Valid: enforce, shadow, \
                         read_only_safe_mode, legacy_compat"
                    );
                }
                // A set-but-unrecognized value is treated as read_only_safe_mode
                // so a typo cannot silently fall back to shadow mode.
                Some(parsed.unwrap_or(auth::boundary::PolicyMode::ReadOnlySafeMode))
            }
        };

        // SEC-M001: validate CHAINWORKS_BOUNDARY_POLICY_PATH before reading.
        // Reject relative paths, non-regular files, symlinks, and paths outside
        // the trusted boundary fixture root to prevent config injection at startup.
        // P081 malformed-fixture contract: if the env var is SET but the path is
        // invalid, enter read_only_safe_mode rather than silently ignoring the var.
        //
        // Trusted root: CHAINWORKS_META_ROOT (or ~/.chainworks) plus /boundary/.
        // Any absolute path that does not have the trusted root as a prefix is
        // rejected to prevent an env/config injection from pointing the daemon at an
        // arbitrary valid-but-untrusted fixture file.
        let raw_fixture_path = std::env::var("CHAINWORKS_BOUNDARY_POLICY_PATH").ok();
        let trusted_boundary_root: std::path::PathBuf = {
            let meta_root = std::env::var("CHAINWORKS_META_ROOT")
                .ok()
                .map(std::path::PathBuf::from)
                .or_else(|| dirs::home_dir().map(|h| h.join(".chainworks")))
                .unwrap_or_else(|| std::path::PathBuf::from(".chainworks"));
            // Canonicalize best-effort; fall back to the raw joined path if it doesn't exist yet.
            meta_root
                .canonicalize()
                .unwrap_or(meta_root)
                .join("boundary")
        };
        let fixture_path: Option<String> = raw_fixture_path.as_deref().and_then(|p| {
            let path = std::path::Path::new(p);
            if !path.is_absolute() {
                warn!(
                    path = %p,
                    "CHAINWORKS_BOUNDARY_POLICY_PATH is not absolute; will enter read_only_safe_mode"
                );
                return None;
            }
            // Reject if the path is a symlink (check before canonicalize to avoid following it).
            #[cfg(unix)]
            {
                if let Ok(meta) = std::fs::symlink_metadata(path) {
                    if meta.file_type().is_symlink() {
                        warn!(
                            path = %p,
                            "CHAINWORKS_BOUNDARY_POLICY_PATH is a symlink; will enter read_only_safe_mode"
                        );
                        return None;
                    }
                }
            }
            // SEC-M001: enforce containment within the trusted boundary fixture root.
            // Canonicalize the supplied path (resolves .. etc.) then check prefix.
            let canonical = match std::fs::canonicalize(path) {
                Ok(c) => c,
                Err(e) => {
                    warn!(
                        path = %p,
                        err = %e,
                        "CHAINWORKS_BOUNDARY_POLICY_PATH cannot be canonicalized; \
                         will enter read_only_safe_mode"
                    );
                    return None;
                }
            };
            if !canonical.starts_with(&trusted_boundary_root) {
                warn!(
                    path = %p,
                    canonical = %canonical.display(),
                    trusted_root = %trusted_boundary_root.display(),
                    "CHAINWORKS_BOUNDARY_POLICY_PATH is outside trusted boundary fixture root; \
                     will enter read_only_safe_mode"
                );
                return None;
            }
            // SEC-M001: use the validated canonical path for the actual read to close
            // the TOCTOU window between validation and open.
            Some(canonical.to_string_lossy().into_owned())
        });
        // True when the env var was set but the path failed validation above.
        let fixture_path_set_but_rejected = raw_fixture_path.is_some() && fixture_path.is_none();

        // SEC-M-002: Maximum boundary fixture size (1 MiB). Larger files trigger safe-mode
        // fallback to prevent startup DoS from an oversized or special file.
        const MAX_FIXTURE_FILE_BYTES: u64 = 1_048_576;

        // P081 malformed-fixture contract: invalid path → safe_mode, before size check.
        let policy = if fixture_path_set_but_rejected {
            warn!(
                "CHAINWORKS_BOUNDARY_POLICY_PATH was set but path validation failed; \
                 entering read_only_safe_mode per P081 malformed-fixture contract"
            );
            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                auth::boundary::PolicyMode::ReadOnlySafeMode,
            )
            .expect("embedded fixture must always be valid")
        } else {
            match fixture_path {
                Some(path) => {
                    // SEC-M-002: Check file size and regular-file type before reading.
                    let size_ok = std::fs::metadata(&path)
                        .map(|m| m.is_file() && m.len() <= MAX_FIXTURE_FILE_BYTES)
                        .unwrap_or(false);
                    let read_result = if size_ok {
                        std::fs::read_to_string(&path).map_err(|e| e.to_string())
                    } else {
                        Err(format!(
                            "fixture file exceeds {} bytes or is not a regular file; entering safe-mode",
                            MAX_FIXTURE_FILE_BYTES
                        ))
                    };
                    match read_result {
                        Ok(json) => {
                            let policy = match mode_override.clone() {
                                Some(mode) => auth::boundary::BoundaryPolicy::from_deployed_or_safe_mode_with_override(&json, mode),
                                None => auth::boundary::BoundaryPolicy::from_deployed_or_safe_mode(&json),
                            };
                            info!(
                                path = %path,
                                mode = %policy.mode(),
                                fixture_digest = %policy.fixture_digest(),
                                "BoundaryPolicy service constructed from deployed fixture"
                            );
                            policy
                        }
                        Err(e) => {
                            warn!(
                                path = %path,
                                err = %e,
                                "BoundaryPolicy: could not read deployed fixture; falling back to embedded + read_only_safe_mode"
                            );
                            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                                auth::boundary::PolicyMode::ReadOnlySafeMode,
                            )
                            .expect("embedded fixture must always be valid")
                        }
                    }
                }
                None => {
                    // No deployed fixture path; use embedded fixture with optional mode override.
                    // Default is shadow (matrix is advisory, legacy guards remain authoritative)
                    // per docs/reference/boundary-first-api-auth-contract.md:133, until Phase 4
                    // shadow-coverage gates are met. Enforce requires Phase 4 exit evidence and
                    // CHAINWORKS_BOUNDARY_POLICY=enforce. ReadOnlySafeMode requires explicit opt-in
                    // via CHAINWORKS_BOUNDARY_POLICY=read_only_safe_mode.
                    let policy_mode = mode_override.unwrap_or(auth::boundary::PolicyMode::Shadow);
                    match auth::boundary::BoundaryPolicy::from_embedded_with_mode(policy_mode) {
                        Ok(policy) => {
                            info!(
                                mode = %policy.mode(),
                                fixture_digest = %policy.fixture_digest(),
                                "BoundaryPolicy service constructed from embedded fixture"
                            );
                            if matches!(policy.mode(), auth::boundary::PolicyMode::ReadOnlySafeMode)
                            {
                                warn!(
                                    "BoundaryPolicy is in READ_ONLY_SAFE_MODE (explicitly configured). \
                                     Mutations are denied; reads are served from the embedded fixture. \
                                     Set CHAINWORKS_BOUNDARY_POLICY=shadow for advisory shadow mode \
                                     or CHAINWORKS_BOUNDARY_POLICY=enforce after Phase 4 exit evidence."
                                );
                            }
                            policy
                        }
                        Err(e) => {
                            error!(err = %e, "BoundaryPolicy: embedded fixture error; entering read_only_safe_mode");
                            auth::boundary::BoundaryPolicy::from_deployed_or_safe_mode(
                                auth::boundary::EMBEDDED_FIXTURE_JSON,
                            )
                        }
                    }
                }
            }
        };
        policy
    });
    // boundary_policy is shared (via Arc) into both McpServer and GraphQL schema below.

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

    reporter.set_startup_phase("http_probe_bind");
    let mut startup_probe_server = if mode == DaemonMode::Mcp {
        None
    } else {
        Some(start_startup_probe_server(&paths, reporter.clone(), build_sha).await?)
    };

    let db_writer = Arc::new(db::writer::DbWriter::new(pool.clone()));
    db::writer::register_shared_writer(&pool, db_writer.clone()).await?;

    // P080: seed rollout control matrix (first-run only, all-or-nothing).
    // Fail-closed: if seeding fails (including partial-row-set detection) the
    // daemon refuses to start so tool registration and the reconciliation loop
    // cannot proceed with an unknown rollout state (HIGH-001 fix).
    match db::repos::p080::seed_rollout_control_if_absent(&pool).await {
        Ok(n) if n > 0 => info!(seeded = n, "P080 rollout control matrix seeded (first run)"),
        Ok(_) => {}
        Err(e) => {
            error!(
                err = %e,
                "P080 rollout control seeding failed; refusing to start (fail-closed)"
            );
            return Err(e);
        }
    }
    // HIGH-001: validate that the complete set of required rollout-control rows
    // is present after seeding.  Refuses daemon startup if any class is missing
    // (e.g. a deleted or corrupted live_disable row must not be silently skipped).
    if let Err(e) = db::repos::p080::validate_rollout_control_completeness(&pool).await {
        error!(
            err = %e,
            "P080 rollout control validation failed; refusing to start (fail-closed)"
        );
        return Err(e);
    }

    // SEC-M-004: Verify the latest audit checkpoint window before constructing
    // request-serving components. If tamper_suspected, replace boundary_policy
    // with a read_only_safe_mode instance so state-changing calls are denied
    // until an operator confirms or repairs the audit trail.
    //
    // This runs after pool open but before cmd_handler / GraphQL / MCP wiring,
    // so the replacement Arc is the one injected into all consumers below.
    let boundary_policy = {
        use db::repos::audit_log::{verify_latest_checkpoint, AuditIntegrityState};
        let integrity = verify_latest_checkpoint(&pool).await;
        match &integrity {
            AuditIntegrityState::TamperSuspected => {
                error!(
                    integrity_state = integrity.as_str(),
                    "Audit checkpoint verification failed: tamper_suspected. \
                     Entering read_only_safe_mode; state-changing calls denied until \
                     operator repair or explicit emergency override."
                );
                std::sync::Arc::new(
                    auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                        auth::boundary::PolicyMode::ReadOnlySafeMode,
                    )
                    .expect("embedded fixture must be valid"),
                )
            }
            AuditIntegrityState::Degraded => {
                warn!(
                    integrity_state = integrity.as_str(),
                    "Audit checkpoint verification degraded (covered rows unreadable). \
                     Continuing with existing policy mode; operator inspection recommended."
                );
                boundary_policy
            }
            AuditIntegrityState::Verified => {
                info!(
                    integrity_state = integrity.as_str(),
                    "Audit checkpoint verified"
                );
                boundary_policy
            }
            AuditIntegrityState::NoCheckpoints => {
                info!(
                    integrity_state = integrity.as_str(),
                    "No audit checkpoints yet (normal for fresh install)"
                );
                boundary_policy
            }
        }
    };

    // ── Steward runtime + control-plane wiring ─────────────────────────
    let steward_config_path = std::env::var("STEWARD_CONFIG_PATH")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            mode.is_packaged()
                .then(|| packaging::bundled_resource_path("steward_config.yaml"))
                .flatten()
        });
    let steward_catalog_path = std::env::var("AGENT_CATALOG_PATH")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            mode.is_packaged()
                .then(|| packaging::bundled_resource_path("agents.yaml"))
                .flatten()
        });
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
    let _storage_rollup_handle =
        spawn_storage_write_pressure_rollup(pool.clone(), db_writer.heartbeat.clone());
    let _audit_checkpoint_handle = spawn_audit_checkpoint_task(pool.clone());
    let cmd_handler = Arc::new(
        CommandHandler::new_with_acp_capacity_and_db_writer(
            pool.clone(),
            events.clone(),
            work_queue.clone(),
            acp.clone(),
            domain::provider::InvokeAgentCapacityConfig::default(),
            Some(db_writer.clone()),
        )
        // P081 Phase 3: inject the shared BoundaryPolicy so command transactions
        // can record accurate policy_mode and fixture_version in audit_log entries.
        .with_boundary_policy(Arc::clone(&boundary_policy)),
    );
    let orchestrator = Arc::new(Orchestrator::new_with_db_writer(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
        db_writer.clone(),
    ));
    let executor = Arc::new(
        BackgroundExecutor::new_with_steward_runtime_inputs_and_db_writer(
            pool.clone(),
            work_queue.clone(),
            orchestrator.clone(),
            acp.clone(),
            events.clone(),
            steward_runtime_inputs.clone(),
            Some(db_writer.clone()),
        ),
    );
    // Startup recovery: repair any run left mid-flight by a previous crash.
    let recovery = RecoveryService::new_with_db_writer(
        pool.clone(),
        work_queue.clone(),
        events.clone(),
        db_writer.clone(),
    );
    reporter.set_startup_phase("startup_recovery");
    let summary = recovery.run_startup_repair().await?;
    info!(
        runs_inspected = summary.runs_inspected,
        runs_repaired = summary.runs_repaired,
        work_items_requeued = summary.work_items_requeued,
        "startup recovery complete"
    );
    // P083: scan unresolved shutdown signals and identity-ambiguous provider sessions.
    reporter.set_startup_phase("shutdown_recovery_scan");
    match engine::shutdown_service::recover_shutdown_state(&pool).await {
        Ok(report) => {
            info!(
                sessions_scanned = report.sessions_scanned,
                retry_signal = report.retry_signal_count,
                observation_required = report.observation_required_count,
                identity_ambiguous = report.identity_ambiguous_count,
                "P083 shutdown recovery scan complete"
            );
        }
        Err(err) => {
            warn!(err = %err, "P083 shutdown recovery scan failed (non-fatal)");
        }
    }
    // SEC-P083-HIGH-001: dispatch any planned shutdown signals that were never issued.
    // Per shutdown_contract_v1.recovery_rules: planned rows recorded before a crash
    // must be dispatched through identity-checked OS signal issuance on restart.
    reporter.set_startup_phase("shutdown_signal_dispatch");
    match engine::shutdown_service::dispatch_planned_shutdown_signals(&pool).await {
        Ok(dispatched) => {
            if dispatched > 0 {
                info!(
                    dispatched,
                    "P083 shutdown signal dispatch on startup complete"
                );
            }
        }
        Err(err) => {
            warn!(err = %err, "P083 shutdown signal dispatch failed (non-fatal)");
        }
    }
    // P083 MISSING-007: insert a durable_monotonic_clock_samples baseline sample on startup.
    // Per durable_monotonic_clock_contract: records wall_clock_ms and boot_id at startup so
    // recovery code can detect post-restart monotonic epoch drift.
    // Per durable_monotonic_clock_contract: baseline insert failure is FATAL.
    // Recovery relies on at least one baseline row existing after daemon start.
    reporter.set_startup_phase("durable_clock_baseline");
    insert_durable_monotonic_clock_baseline(&pool)
        .await
        .context("P083 fatal: durable_monotonic_clock_samples baseline insert failed")?;
    reporter.set_startup_phase("maintenance_reaper");
    match db::repos::maintenance::run_reaper(&pool).await {
        Ok(()) => {
            info!("P087 startup maintenance reaper complete");
        }
        Err(err) => {
            warn!(
                err = %err,
                "P087 startup maintenance reaper failed; continuing startup with degraded storage health readback"
            );
        }
    }
    let _maintenance_reaper = db::repos::maintenance::spawn_maintenance_reaper(pool.clone());
    reporter.set_startup_phase("host_interruption_monitors");
    let host_interruption_service =
        HostInterruptionService::with_capacity_config_runtime_cleanup_and_db_writer(
            pool.clone(),
            work_queue.clone(),
            InvokeAgentCapacityConfig::default(),
            acp.clone(),
            db_writer.clone(),
        );
    let _runtime_heartbeat_monitor =
        spawn_runtime_heartbeat_monitor(host_interruption_service.clone());
    let _native_host_interruption_monitor =
        daemon::host_interruption_sources::spawn_native_host_interruption_monitor(
            host_interruption_service,
        );
    info!("Host interruption monitors started");

    // (Principal table is loaded up-front so failed-serve branches have
    // it available; see the R13 API-001 comment at the top of `main`.)

    // ── Mode dispatch ──────────────────────────────────────────────────
    // Per P042 §5.1 `Ready` is emitted only AFTER HTTP bind. MCP stdio
    // mode has no HTTP bind, so `Ready` fires before `run_stdio` there.
    match mode {
        DaemonMode::Mcp => {
            let executor_handle = executor.start();
            let _executor_watchdog_handle = spawn_background_executor_watchdog(
                pool.clone(),
                work_queue.clone(),
                events.clone(),
                db_writer.clone(),
                reporter.clone(),
                executor_handle,
            );
            info!("BackgroundExecutor started");
            reporter.set_state(DaemonLifecycleState::Ready);
            let _evidence_orphan_sweep =
                daemon::storage_startup::spawn_startup_evidence_orphan_sweep(pool.clone());
            if let Err(e) = packaging::write_build_sha(&paths) {
                warn!(err = %e, "failed to write build-sha.txt (non-fatal)");
            }
            let mcp = mcp_server::server::McpServer::new_with_storage_writer_and_boundary_policy(
                pool.clone(),
                cmd_handler.clone(),
                principal_table,
                db_writer.heartbeat.clone(),
                Arc::clone(&boundary_policy),
            )
            .with_acp_runtime(Arc::clone(&acp));
            let mcp_live_source = mcp.live_principal_source();
            let principals_reload_secs: u64 = std::env::var("CHAINWORKS_PRINCIPALS_RELOAD_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2);
            let principals_path_for_reload = paths.principals_path.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(principals_reload_secs))
                        .await;
                    match auth::PrincipalTable::load_or_bootstrap(&principals_path_for_reload) {
                        Ok(new_table) => {
                            mcp_live_source.update(new_table);
                            tracing::debug!(
                                "MCP stdio principal table reloaded for live revocation"
                            );
                        }
                        Err(e) => {
                            mcp_live_source.mark_unavailable();
                            tracing::warn!(
                                "MCP stdio principal reload failed; auth source marked unavailable \
                                 (requests will fail-closed): {e:#}"
                            );
                        }
                    }
                }
            });
            mcp.run_stdio().await?;
        }
        _ => {
            // Daemon mode: GraphQL + MCP HTTP on the same port.
            // P081 Phase 3: both servers receive the same shared immutable BoundaryPolicy instance.
            let mcp = std::sync::Arc::new(
                mcp_server::server::McpServer::new_with_storage_writer_and_boundary_policy(
                    pool.clone(),
                    cmd_handler.clone(),
                    principal_table.clone(),
                    db_writer.heartbeat.clone(),
                    Arc::clone(&boundary_policy),
                )
                .with_acp_runtime(Arc::clone(&acp)),
            );
            let mcp_live_source = mcp.live_principal_source();
            let mcp_routes = mcp_server::http::routes(mcp);
            info!("MCP HTTP transport mounted at /mcp");

            let (schema, p046_live_handle) =
                graphql_server::schema::build_schema_with_storage_writer_boundary_policy_and_handle(
                    pool.clone(),
                    cmd_handler.clone(),
                    events.clone(),
                    principal_table.clone(),
                    reporter.clone(),
                    db_writer.heartbeat.clone(),
                    Arc::clone(&boundary_policy),
                );
            // P046: periodically reload principals.json so subscription auth rechecks
            // observe revocation promptly, without requiring a daemon restart.
            // Default interval is 2 seconds; override with CHAINWORKS_PRINCIPALS_RELOAD_SECS.
            let principals_reload_secs: u64 = std::env::var("CHAINWORKS_PRINCIPALS_RELOAD_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2);
            let principals_path_for_reload = paths.principals_path.clone();
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(principals_reload_secs))
                        .await;
                    match auth::PrincipalTable::load_or_bootstrap(&principals_path_for_reload) {
                        Ok(new_table) => {
                            mcp_live_source.update(new_table.clone());
                            p046_live_handle.update(new_table).await;
                            tracing::debug!("principal table reloaded for live revocation");
                        }
                        Err(e) => {
                            // Mark the auth source unavailable so running subscriptions
                            // terminate fail-closed (authorization_recheck_failed) rather
                            // than continuing under stale grants after a revocation write
                            // that we could not observe.
                            mcp_live_source.mark_unavailable();
                            p046_live_handle.mark_unavailable().await;
                            tracing::warn!(
                                "principal reload failed; auth source marked unavailable \
                                 (MCP requests and subscriptions will fail-closed): {e:#}"
                            );
                        }
                    }
                }
            });

            // §7.3 port fallback: bind during startup and serve probe-only
            // `/health` + `/ready` while recovery runs. Now stop the probe
            // clone and hand the same endpoint to the full GraphQL/MCP server.
            reporter.set_startup_phase("http_full_handoff");
            let (listener, port) = if let Some(probe) = startup_probe_server.take() {
                let port = probe.port;
                let listener = stop_startup_probe_server(probe).await;
                (listener, port)
            } else {
                let (listener, port) = packaging::bind_with_fallback(&paths).await?;
                packaging::write_daemon_endpoint_snapshot(
                    &paths,
                    std::process::id(),
                    port,
                    build_sha,
                )?;
                (listener, port)
            };
            info!(port, "bound HTTP listener; handing off to graphql-server");

            let xcode_broker_pool =
                new_daemon_xcode_broker_pool(port, acp.xcode_runtime_observation_sink());
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

            let executor_handle = executor.start();
            let _executor_watchdog_handle = spawn_background_executor_watchdog(
                pool.clone(),
                work_queue.clone(),
                events.clone(),
                db_writer.clone(),
                reporter.clone(),
                executor_handle,
            );
            info!("BackgroundExecutor started");

            // §5.1: Ready only AFTER HTTP bind so a client receiving
            // `state=ready` is guaranteed it can connect.
            reporter.set_state(DaemonLifecycleState::Ready);
            let _evidence_orphan_sweep =
                daemon::storage_startup::spawn_startup_evidence_orphan_sweep(pool.clone());

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
                // Signal any in-progress temp-artifact inventory scans to stop cooperatively.
                mcp_server::tools::scanner::request_global_shutdown();
                match tokio::time::timeout(
                    SHUTDOWN_DRAIN_DEADLINE,
                    async {
                        tokio::join!(
                            shutdown_acp.close_all_sessions(),
                            mcp_server::tools::scanner::drain_global_scan_permits()
                        )
                    },
                )
                .await
                {
                    Ok((closed_sessions, inventory_drain)) => {
                        info!(
                            closed_sessions,
                            "closed live ACP sessions during daemon shutdown"
                        );
                        match inventory_drain {
                            Ok(()) => info!(
                                "released all temporary-artifact scan permits during daemon shutdown"
                            ),
                            Err(error) => warn!(
                                error = %error,
                                "temporary-artifact scan permit drain failed"
                            ),
                        }
                    }
                    Err(_) => warn!(
                        deadline_secs = SHUTDOWN_DRAIN_DEADLINE.as_secs(),
                        "timed out draining ACP sessions and temporary-artifact scans during daemon shutdown"
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

            // P081 clean-shutdown audit checkpoint: flush the open window (any unchecked
            // rows, even fewer than 1000) so the checkpoint chain is complete at exit.
            match db::repos::audit_log::write_checkpoint_if_non_empty(&pool).await {
                Ok(true) => info!("P081 clean-shutdown audit checkpoint written"),
                Ok(false) => {}
                Err(err) => warn!(err = %err, "P081 clean-shutdown audit checkpoint failed"),
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
            let _ = pool.cleanup_first_connect_timeouts().await;
            let _ = pool.cleanup_pid_drift().await;
            let _ = pool.cleanup_idle_active_leases().await;
            reporter.set_xcode_broker_health(xcode_broker_health_for_lifecycle(
                pool.health_snapshot().await,
            ));
        }
    });
}

/// P081 Phase 1: Background audit checkpoint writer.
///
/// Polls every 30 seconds and writes a checkpoint whenever 1000 or more
/// audit_log rows have accumulated since the last checkpoint window.
/// Also stamps `checkpoint_id` on covered rows so retention cleanup can run.
///
/// Per the P081 contract: checkpoints are written every 1000 rows and at
/// clean shutdown when the open window is non-empty.
fn spawn_audit_checkpoint_task(pool: SqlitePool) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            match db::repos::audit_log::write_checkpoint_if_needed(&pool).await {
                Ok(true) => {
                    info!("P081 audit checkpoint written");
                }
                Ok(false) => {}
                Err(err) => {
                    warn!(err = %err, "P081 audit checkpoint write failed");
                }
            }
        }
    })
}

fn spawn_storage_write_pressure_rollup(
    pool: SqlitePool,
    heartbeat: Arc<db::writer::DbWriterHeartbeat>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(
            db::writer::TELEMETRY_FLUSH_CADENCE_MS,
        ));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(err) =
                db::repos::storage_health::record_live_write_pressure_rollup(&pool, &heartbeat)
                    .await
            {
                warn!(err = %err, "P075 storage write-pressure rollup failed");
            }
        }
    })
}

fn new_daemon_xcode_broker_pool(
    port: u16,
    observation_sink: Arc<dyn XcodeRuntimeObservationSink>,
) -> Arc<XcodeMcpBridgePool> {
    Arc::new(XcodeMcpBridgePool::new_with_sink_and_process_backend(
        XcodeMcpBridgePoolConfig {
            base_url: format!("http://127.0.0.1:{port}/xcode-mcp"),
            broker_disabled: std::env::var("CHAINWORKS_XCODE_BROKER_DISABLED")
                .map(|value| value == "1")
                .unwrap_or(false),
            tool_allowlists_by_hash: engine::mcp::load_xcode_broker_tool_allowlists()
                .unwrap_or_else(|err| {
                    warn!(error = %err, "Failed to load Xcode broker tool allowlists from MCP registry");
                    Default::default()
                }),
            use_local_host_probe: true,
            ..Default::default()
        },
        observation_sink,
    ))
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
        reason_code: health.reason_code,
        can_acquire_new_xcode_leases: health.can_acquire_new_xcode_leases,
        active_lease_count: health.active_lease_count,
        initialize_queue_depth: health.initialize_queue_depth,
        last_transition_at: health.last_transition_at,
        operator_message: health.operator_message,
        pool_id: health.pool_id,
        active_leases: health.active_leases,
        queued_leases: health.queued_leases,
        max_active_leases: health.max_active_leases,
        max_queued_leases: health.max_queued_leases,
        broker_disabled: health.broker_disabled,
        backend_available: health.backend_available,
        observation_persistence_failures: health.observation_persistence_failures,
        stale_lease_count: health.stale_lease_count,
        backend_session_count: health.backend_session_count,
        helper_cleanup_reaped_leases_total: health.helper_cleanup_reaped_leases_total,
    }
}

fn spawn_background_executor_watchdog(
    pool: SqlitePool,
    work_queue: WorkQueue,
    events: engine::event_bus::EventSender,
    db_writer: Arc<db::writer::DbWriter>,
    reporter: LifecycleReporter,
    executor_handle: tokio::task::JoinHandle<()>,
) -> tokio::task::JoinHandle<()> {
    let executor_alive = Arc::new(AtomicBool::new(true));
    let join_alive = executor_alive.clone();
    let join_reporter = reporter.clone();
    tokio::spawn(async move {
        match executor_handle.await {
            Ok(()) => warn!("BackgroundExecutor work loop returned unexpectedly"),
            Err(error) => error!(error = %error, "BackgroundExecutor work loop task failed"),
        }
        join_alive.store(false, Ordering::SeqCst);
        join_reporter.raise_degraded(
            DegradedKind::BackgroundExecutorStalled,
            "background executor work loop terminated; work queue is not being serviced",
        );
    });

    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(
            std::env::var("CHAINWORKS_EXECUTOR_WATCHDOG_INTERVAL_SECS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(30),
        );
        let stale_advance_after = chrono::Duration::seconds(
            std::env::var("CHAINWORKS_ADVANCE_RUN_STALE_SECS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(300),
        );
        let scheduler_health_stale_after = chrono::Duration::seconds(
            std::env::var("CHAINWORKS_SCHEDULER_HEALTH_STALE_SECS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(180),
        );

        loop {
            tokio::time::sleep(interval).await;
            let now = chrono::Utc::now();
            let stale_before = now - stale_advance_after;
            match work_items::requeue_stale_running_advance_items(
                &pool,
                stale_before,
                now,
                "watchdog_stale_advance_run",
            )
            .await
            {
                Ok(0) => {}
                Ok(requeued) => {
                    warn!(
                        requeued,
                        "BackgroundExecutor watchdog requeued stale running AdvanceRun work items"
                    );
                    if let Err(error) = work_queue.refresh_scheduler_projection().await {
                        warn!(
                            error = %error,
                            "BackgroundExecutor watchdog failed to refresh scheduler projection after stale AdvanceRun requeue"
                        );
                    }
                }
                Err(error) => warn!(
                    error = %error,
                    "BackgroundExecutor watchdog failed to inspect stale running AdvanceRun work items"
                ),
            }

            let p092_recovery = RecoveryService::new_with_db_writer(
                pool.clone(),
                work_queue.clone(),
                events.clone(),
                db_writer.clone(),
            );
            match p092_recovery
                .run_p092_retry_payload_recovery_for_active_runs()
                .await
            {
                Ok(0) => {}
                Ok(repaired) => {
                    warn!(
                        repaired,
                        "BackgroundExecutor watchdog repaired P092 retry payload recovery candidates"
                    );
                }
                Err(error) => warn!(
                    error = %error,
                    "BackgroundExecutor watchdog failed to inspect P092 retry payload recovery candidates"
                ),
            }

            match work_items::complete_running_invoke_agents_with_terminal_valid_outputs_on_startup(
                &pool,
                "watchdog_repair_completed_agent_valid_outputs",
            )
            .await
            {
                Ok(0) => {}
                Ok(completed) => {
                    warn!(
                        completed,
                        "BackgroundExecutor watchdog completed InvokeAgent work items whose agent executions already had valid outputs"
                    );
                    if let Err(error) = work_queue.refresh_scheduler_projection().await {
                        warn!(
                            error = %error,
                            "BackgroundExecutor watchdog failed to refresh scheduler projection after terminal InvokeAgent recovery"
                        );
                    }
                }
                Err(error) => warn!(
                    error = %error,
                    "BackgroundExecutor watchdog failed to inspect terminal InvokeAgent work items"
                ),
            }

            let live_work_count = match pending_or_running_work_item_count(&pool).await {
                Ok(count) => count,
                Err(error) => {
                    warn!(
                        error = %error,
                        "BackgroundExecutor watchdog could not count live work items"
                    );
                    continue;
                }
            };
            let active_agents = match active_agent_execution_count(&pool).await {
                Ok(count) => count,
                Err(error) => {
                    warn!(
                        error = %error,
                        "BackgroundExecutor watchdog could not count active agent executions"
                    );
                    continue;
                }
            };
            let scheduler_stale = match scheduler::latest_health_snapshot(&pool).await {
                Ok(Some(snapshot)) => {
                    let explicit_stale_at = snapshot.updated_at
                        + chrono::Duration::milliseconds(snapshot.stale_after_ms);
                    let watchdog_stale_at = snapshot.updated_at + scheduler_health_stale_after;
                    explicit_stale_at <= now || watchdog_stale_at <= now
                }
                Ok(None) => live_work_count > 0,
                Err(error) => {
                    warn!(
                        error = %error,
                        "BackgroundExecutor watchdog could not read scheduler health snapshot"
                    );
                    continue;
                }
            };

            if !executor_alive.load(Ordering::SeqCst) {
                continue;
            }
            if scheduler_stale && live_work_count > 0 && active_agents == 0 {
                reporter.raise_degraded(
                    DegradedKind::BackgroundExecutorStalled,
                    format!(
                        "scheduler heartbeat stale while {live_work_count} work item(s) are live and no agent executions are active"
                    ),
                );
            } else {
                reporter.clear_degraded(DegradedKind::BackgroundExecutorStalled);
            }
        }
    })
}

async fn pending_or_running_work_item_count(pool: &SqlitePool) -> Result<i64> {
    sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM work_items
           WHERE status IN ('pending', 'running')"#,
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn active_agent_execution_count(pool: &SqlitePool) -> Result<i64> {
    sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM agent_executions
           WHERE status = 'running'"#,
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
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
    packaging::write_daemon_endpoint_snapshot(
        paths,
        std::process::id(),
        port,
        packaging::resolved_build_sha(),
    )?;
    info!(
        port,
        bind_addr = %paths.bind_addr,
        "failed-serve: bound listener; daemon.port written so clients can discover the status surface"
    );
    failed_serve::serve_failed_state_with_listener(
        reporter.clone(),
        auth::LivePrincipalTable::new(principal_table.clone()),
        listener,
    )
    .await
}

/// Determine a stable boot-session discriminator for durable_monotonic_clock_samples.
///
/// On macOS, uses sysctl kern.boottime (seconds since Unix epoch at last boot) so the
/// value survives daemon restarts and changes only after a host reboot.
/// On Linux, reads /proc/sys/kernel/random/boot_id which is assigned at boot.
/// Falls back to daemon PID as a weak session discriminator when OS APIs are unavailable.
fn read_platform_boot_id() -> String {
    #[cfg(target_os = "macos")]
    {
        use libc::{c_void, size_t, timeval, CTL_KERN, KERN_BOOTTIME};
        let mut boot_time = timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        let mut size: size_t = std::mem::size_of::<timeval>();
        let mut mib: [libc::c_int; 2] = [CTL_KERN, KERN_BOOTTIME];
        let ret = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                2,
                &mut boot_time as *mut timeval as *mut c_void,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if ret == 0 && boot_time.tv_sec > 0 {
            return format!("macos_boottime_s{}", boot_time.tv_sec);
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/sys/kernel/random/boot_id") {
            let trimmed = contents.trim().to_string();
            if !trimmed.is_empty() {
                return format!("linux_boot_{}", trimmed);
            }
        }
    }
    // Fallback: daemon PID is a weak session discriminator; it changes across restarts
    // but cannot distinguish a host reboot from a daemon restart.
    format!("daemon_pid_{}", std::process::id())
}

/// Read the monotonic clock value in milliseconds using CLOCK_MONOTONIC (system uptime).
/// This value is zero at boot and increases continuously, surviving daemon restarts.
/// Recovery can compare baseline values across restarts to detect clock rollbacks.
#[cfg(unix)]
fn monotonic_clock_ms() -> i64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime with a valid timespec pointer and a valid clock ID is always safe.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    (ts.tv_sec as i64) * 1000 + (ts.tv_nsec as i64) / 1_000_000
}

#[cfg(not(unix))]
fn monotonic_clock_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Insert a durable_monotonic_clock_samples baseline sample on daemon startup.
/// Per durable_monotonic_clock_contract: records monotonic_ms (from CLOCK_MONOTONIC)
/// and wall clock at startup so recovery can detect post-restart monotonic epoch drift.
/// Fatal: baseline insert failure aborts startup per the approved contract.
async fn insert_durable_monotonic_clock_baseline(pool: &SqlitePool) -> anyhow::Result<()> {
    let monotonic_ms = monotonic_clock_ms();
    let observed_at_wall_clock = chrono::Utc::now().to_rfc3339();

    let boot_id = read_platform_boot_id();
    let sample_id = uuid::Uuid::new_v4().to_string();
    let baseline_generation: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(baseline_generation), 0) + 1 \
         FROM durable_monotonic_clock_samples WHERE boot_id = ?1",
    )
    .bind(&boot_id)
    .fetch_one(pool)
    .await
    .context("P083: allocate durable_monotonic_clock_samples baseline_generation")?;

    sqlx::query(
        r#"INSERT INTO durable_monotonic_clock_samples
               (sample_id, boot_id, baseline_generation, sample_state, monotonic_ms,
                observed_at_wall_clock, wall_clock_iso8601, clock_skew_ms, created_at)
               VALUES (?1, ?2, ?3, 'baseline', ?4, ?5, ?5, 0, ?5)"#,
    )
    .bind(&sample_id)
    .bind(&boot_id)
    .bind(baseline_generation)
    .bind(monotonic_ms)
    .bind(&observed_at_wall_clock)
    .execute(pool)
    .await
    .context("P083: insert durable_monotonic_clock_samples baseline")?;

    info!(
        boot_id,
        baseline_generation,
        sample_id,
        monotonic_ms,
        "P083 durable monotonic clock baseline recorded"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P089 §6.1: the daemon startup path must materialize the redaction key
    /// before any inventory lane can reach it lazily. This exercises the exact
    /// helper `run_daemon` calls first.
    #[test]
    fn init_p089_redaction_key_at_startup_materializes_key() {
        init_p089_redaction_key_at_startup();
        assert!(
            domain::temp_artifact_inventory::process_path_hash_key_initialized(),
            "startup helper must materialize the process-scoped redaction key"
        );
    }

    #[test]
    fn daemon_xcode_broker_pool_has_process_backend() {
        let pool =
            new_daemon_xcode_broker_pool(41234, Arc::new(acp::NoopXcodeRuntimeObservationSink));

        assert!(pool.has_backend());
    }

    #[test]
    fn daemon_xcode_broker_pool_installs_registry_tool_allowlists() {
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = ENV_LOCK.lock().expect("env lock poisoned");
        let tmp = tempfile::tempdir().unwrap();
        let registry_path = tmp.path().join("mcp.yaml");
        std::fs::write(
            &registry_path,
            r#"
mcp:
  xcode:
    type: xcode_broker
    runtime_id: xcode
    tool_allowlist:
      - xcode.build
      - xcode.test
"#,
        )
        .unwrap();

        let previous = std::env::var("CHAINWORKS_CODEX_CONFIG_PATH").ok();
        std::env::set_var("CHAINWORKS_CODEX_CONFIG_PATH", &registry_path);

        let allowlists = engine::mcp::load_xcode_broker_tool_allowlists().unwrap();
        let hash = allowlists
            .keys()
            .next()
            .expect("fixture registry should produce a tool allowlist")
            .clone();
        let pool =
            new_daemon_xcode_broker_pool(41234, Arc::new(acp::NoopXcodeRuntimeObservationSink));

        assert!(pool.has_tool_allowlist_hash(&hash));

        match previous {
            Some(value) => std::env::set_var("CHAINWORKS_CODEX_CONFIG_PATH", value),
            None => std::env::remove_var("CHAINWORKS_CODEX_CONFIG_PATH"),
        }
    }

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

    #[test]
    fn p083_monotonic_clock_baseline_insert_function_exists() {
        // Compile-time proof that insert_durable_monotonic_clock_baseline exists and
        // accepts a SqlitePool reference. The function is called on startup after
        // P083 shutdown recovery scan.
        let _ = insert_durable_monotonic_clock_baseline;
    }

    #[test]
    fn daemon_runtime_uses_explicit_large_worker_stack_for_mcp_commands() {
        let source = include_str!("main.rs");
        assert!(
            !source.contains(concat!("#[", "tokio::main]")),
            "the daemon must not rely on tokio's default worker stack; MCP command paths such as runs.cancel are too deep in debug builds"
        );
        assert!(
            source.contains("thread_stack_size(DAEMON_WORKER_STACK_BYTES)"),
            "the daemon runtime must set an explicit worker stack size for deep MCP command paths"
        );
    }
}
