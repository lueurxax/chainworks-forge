use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{debug, info, warn};

pub const SQLITE_BUSY_TIMEOUT: Duration = Duration::from_secs(30);
const SQLITE_BUSY_RETRY_ATTEMPTS: usize = 4;
const SQLITE_BUSY_RETRY_BASE_DELAY: Duration = Duration::from_millis(25);
const WRITE_LOCK_METRIC_LIMIT: usize = 1024;
const BUSY_RETRY_WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq)]
pub struct WriteLockMetricsSnapshot {
    pub wait_p50_ms: Option<u64>,
    pub wait_p95_ms: Option<u64>,
    pub busy_retry_rate_per_minute: f64,
    pub busy_retry_exhausted_total: u64,
}

#[derive(Debug)]
struct WriteLockMetricSample {
    wait_ms: u64,
    busy_retries: usize,
    observed_at: Instant,
}

#[derive(Debug, Default)]
struct WriteLockMetrics {
    samples: VecDeque<WriteLockMetricSample>,
    busy_retry_exhausted_total: u64,
}

static WRITE_LOCK_METRICS: OnceLock<Mutex<WriteLockMetrics>> = OnceLock::new();

fn write_lock_metrics() -> &'static Mutex<WriteLockMetrics> {
    WRITE_LOCK_METRICS.get_or_init(|| Mutex::new(WriteLockMetrics::default()))
}

fn record_write_lock_metrics(wait_ms: u64, busy_retries: usize, exhausted: bool) {
    let mut metrics = write_lock_metrics().lock().unwrap();
    if metrics.samples.len() >= WRITE_LOCK_METRIC_LIMIT {
        metrics.samples.pop_front();
    }
    metrics.samples.push_back(WriteLockMetricSample {
        wait_ms,
        busy_retries,
        observed_at: Instant::now(),
    });
    if exhausted {
        metrics.busy_retry_exhausted_total += 1;
    }
}

pub fn write_lock_metrics_snapshot() -> WriteLockMetricsSnapshot {
    let metrics = write_lock_metrics().lock().unwrap();
    let waits = metrics
        .samples
        .iter()
        .map(|sample| sample.wait_ms)
        .collect::<Vec<_>>();
    let now = Instant::now();
    let retries_in_window = metrics
        .samples
        .iter()
        .filter(|sample| now.duration_since(sample.observed_at) <= BUSY_RETRY_WINDOW)
        .map(|sample| sample.busy_retries as u64)
        .sum::<u64>();
    WriteLockMetricsSnapshot {
        wait_p50_ms: percentile(waits.clone(), 50),
        wait_p95_ms: percentile(waits, 95),
        busy_retry_rate_per_minute: retries_in_window as f64,
        busy_retry_exhausted_total: metrics.busy_retry_exhausted_total,
    }
}

fn percentile(mut values: Vec<u64>, percentile: usize) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let idx = ((values.len() - 1) * percentile).div_ceil(100);
    values.get(idx).copied()
}

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
) -> Result<Transaction<'static, Sqlite>> {
    let wait_started = Instant::now();
    let mut retries = 0usize;

    loop {
        match pool.begin_with("BEGIN IMMEDIATE").await {
            Ok(tx) => {
                let wait_ms = wait_started.elapsed().as_millis() as u64;
                record_write_lock_metrics(wait_ms, retries, false);
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
            Err(error) => {
                if is_sqlite_busy_error(&error) {
                    let wait_ms = wait_started.elapsed().as_millis() as u64;
                    record_write_lock_metrics(wait_ms, retries, true);
                }
                return Err(error.into());
            }
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
