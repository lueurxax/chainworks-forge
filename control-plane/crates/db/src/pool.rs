use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};

pub const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);
const SQLITE_BUSY_RETRY_ATTEMPTS: usize = 4;
const SQLITE_BUSY_RETRY_BASE_DELAY: Duration = Duration::from_millis(25);

/// Open a writable SQLite pool after running the P042 preflight
/// (`db::migrate::run_preflight`). Existing callers that expect "migrations
/// just work" continue to work in development and test because clean
/// installs go down the `MissingOrZeroByte` branch, and up-to-date DBs go
/// down the `TrackedEqual` branch — both succeed silently.
///
/// The packaged daemon's `main.rs` calls `db::migrate::run_preflight`
/// directly so it can intercept typed errors (§8.4) and drop into
/// failed-serve mode (§8.7) instead of panicking. Use that entry point
/// when you need typed migration outcomes.
pub async fn create_pool(database_url: &str) -> Result<SqlitePool> {
    // Run preflight so the DB is at the binary's schema version before we
    // hand out a pool. Preflight errors surface here as anyhow errors; the
    // daemon's `main.rs` handles the typed variants separately via
    // `run_preflight` directly for failed-serve behavior.
    crate::migrate::run_preflight(database_url, None)
        .await
        .map_err(|e| anyhow::anyhow!("migration preflight: {e}"))?;

    let opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        // WAL mode enables concurrent readers + one writer across processes.
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        // Busy timeout: wait long enough for short serialized writes.
        .busy_timeout(SQLITE_BUSY_TIMEOUT);

    let max_connections = if database_url.contains(":memory:") {
        1
    } else {
        5
    };

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(opts)
        .await?;

    // In-memory SQLite DBs are per-connection: `run_preflight` applied
    // migrations to a throwaway connection pool that's now closed. Re-run
    // the migrator on the returned pool so tests and ephemeral workloads
    // that use `sqlite::memory:` get a schema-populated DB. Idempotent for
    // file-backed DBs (MIGRATOR records `_sqlx_migrations` and skips applied
    // versions), so the file path is unchanged.
    if database_url.contains(":memory:") {
        crate::migrate::MIGRATOR
            .run(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("apply migrations to in-memory pool: {e}"))?;
    }

    Ok(pool)
}

pub async fn begin_immediate_with_retry<'a>(
    pool: &'a SqlitePool,
    operation: &str,
) -> Result<Transaction<'a, Sqlite>> {
    let wait_started = Instant::now();
    let mut retries = 0usize;

    loop {
        match pool.begin_with("BEGIN IMMEDIATE").await {
            Ok(tx) => {
                let wait_ms = wait_started.elapsed().as_millis() as u64;
                if retries > 0 || wait_ms >= 10 {
                    info!(
                        db_operation = operation,
                        write_lock_wait_ms = wait_ms,
                        busy_retries = retries,
                        "sqlite write lock acquired"
                    );
                } else {
                    debug!(
                        db_operation = operation,
                        write_lock_wait_ms = wait_ms,
                        busy_retries = retries,
                        "sqlite write lock acquired"
                    );
                }
                return Ok(tx);
            }
            Err(error) if is_sqlite_busy_error(&error) && retries < SQLITE_BUSY_RETRY_ATTEMPTS => {
                let delay = SQLITE_BUSY_RETRY_BASE_DELAY * (1u32 << retries);
                retries += 1;
                warn!(
                    db_operation = operation,
                    busy_retries = retries,
                    backoff_ms = delay.as_millis() as u64,
                    error = %error,
                    "sqlite busy while acquiring write lock; retrying"
                );
                sleep(delay).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

pub fn log_write_transaction(operation: &str, transaction_started: Instant) {
    let elapsed_ms = transaction_started.elapsed().as_millis() as u64;
    if operation.ends_with(".empty") && elapsed_ms < 10 {
        debug!(
            db_operation = operation,
            write_transaction_ms = elapsed_ms,
            "sqlite write transaction committed"
        );
    } else {
        info!(
            db_operation = operation,
            write_transaction_ms = elapsed_ms,
            "sqlite write transaction committed"
        );
    }
}

fn is_sqlite_busy_error(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(database_error) = error else {
        return false;
    };
    let code = database_error.code().map(|code| code.to_string());
    let message = database_error.message().to_ascii_lowercase();
    matches!(code.as_deref(), Some("5") | Some("6"))
        || message.contains("database is locked")
        || message.contains("database is busy")
}
