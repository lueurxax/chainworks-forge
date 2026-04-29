//! SQLite migration preflight, backup, and apply orchestration (Proposal 042 §8).
//!
//! # Contract summary (§8.1)
//!
//! Before any `sqlx::migrate!()` invocation, [`classify_db_state`] inspects
//! the target file and puts it into one of three buckets:
//!
//! - [`DbState::MissingOrZeroByte`] — clean-install path; create + apply all.
//! - [`DbState::ExistingWithoutTracker`] — populated DB without an
//!   `_sqlx_migrations` table. Fail closed; operator must move the file
//!   aside or point at the intended DB.
//! - [`DbState::Tracked { applied }`] — normal path; compare `applied` to
//!   the compile-time `binary_schema_version` and dispatch.
//!
//! Every non-Ready branch hits [`run_preflight`] error-return, which the
//! daemon's `main.rs` turns into **failed-serve mode** (§8.7) rather than
//! exiting. Variant-by-variant mapping lives in `FailureKind` + §8.4 of
//! the proposal.
//!
//! # Backup contract (§8.2)
//!
//! Backup runs only on the `Tracked{applied ⊂ binary}` branch. File naming
//! is `<db>.backup-<unix_ts>-v<old_max>-to-v<new_max>.sqlite` next to the
//! original so the operator can locate it trivially. Retention is 30 days,
//! always keeping the newest successful backup regardless of age.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

/// Embedded migrator pointing at `crates/db/migrations/`. Same target as the
/// legacy `pool.rs` path, just exposed so the preflight can list expected
/// versions.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const P058_MIGRATION_VERSION: i64 = 16;
const P058_LEGACY_CHECKSUM: &[u8] = &[
    0xbb, 0x62, 0x1a, 0x52, 0x2e, 0x2c, 0x14, 0xe0, 0xfa, 0x0a, 0xcc, 0xf7, 0x83, 0xf2, 0x90, 0x2c,
    0x7d, 0x69, 0x03, 0x8b, 0x16, 0xa3, 0xfa, 0xac, 0xf5, 0x88, 0xce, 0x71, 0x49, 0xae, 0xd2, 0x53,
    0xd0, 0x9e, 0xba, 0x38, 0x7b, 0x24, 0x7b, 0x5c, 0x54, 0x17, 0x87, 0x4a, 0xa2, 0xe7, 0x80, 0xe4,
];
const P017_MIGRATION_029_VERSION: i64 = 29;
const P017_MIGRATION_029_LEGACY_CHECKSUM: &[u8] = &[
    0x6f, 0xe7, 0x12, 0xd7, 0xdb, 0x30, 0x35, 0x01, 0x86, 0x3e, 0xf4, 0x72, 0x60, 0xf5, 0xed, 0xeb,
    0x35, 0x12, 0xd8, 0xa3, 0x30, 0x9d, 0xb4, 0x2d, 0xad, 0xd2, 0xa8, 0x96, 0x87, 0xdb, 0x99, 0x72,
    0x0c, 0xcc, 0x3b, 0x51, 0x79, 0x0a, 0x40, 0xf7, 0xe3, 0xa5, 0xd0, 0xc5, 0x98, 0x30, 0x2a, 0x4b,
];

/// Classification of a SQLite target at startup time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbState {
    /// File does not exist, or exists with zero bytes. Clean-install path.
    MissingOrZeroByte,
    /// File exists with non-zero bytes but has no `_sqlx_migrations` table
    /// (or the table is unparseable). Legacy or foreign DB — fail closed.
    ExistingWithoutTracker,
    /// File exists with a readable `_sqlx_migrations` table.
    Tracked { applied: BTreeSet<i64> },
}

/// Outcome of a successful preflight + apply cycle.
#[derive(Debug, Clone)]
pub struct MigrationOutcome {
    /// Schema version after preflight completed (max of applied after apply).
    pub schema_version: u32,
    /// Absolute path of the backup that was written, if any.
    pub backup_path: Option<PathBuf>,
    /// The `DbState` classification that drove the branch.
    pub classified_as: DbStateKind,
    /// True iff a migration was actually applied on this startup.
    pub applied_migrations: bool,
}

/// Tag version of [`DbState`] without the `applied` payload — useful for
/// lifecycle reporter telemetry where we only need the branch label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbStateKind {
    MissingOrZeroByte,
    ExistingWithoutTracker,
    TrackedEqual,
    TrackedSubset,
    TrackedSuperset,
    TrackedInterleaved,
}

/// Typed migration-failure surface. Each variant maps to a
/// `domain::lifecycle::FailureKind` per §8.4.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("existing database without migration tracker: {0}")]
    ExistingWithoutTracker(String),

    #[error("schema newer than binary: applied_max={applied_max}, binary_max={binary_max}")]
    NewerThanBinary { applied_max: i64, binary_max: i64 },

    #[error("interleaved migration divergence: extras in applied set = {extras:?}")]
    InterleavedDivergence { extras: Vec<i64> },

    #[error("backup failed: {0}")]
    BackupFailed(String),

    #[error("lock acquisition failed: {0}")]
    LockFailed(String),

    #[error("migration apply failed: {0}")]
    ApplyFailed(String),

    #[error("classification i/o error: {0}")]
    IoError(String),
}

/// Compile-time maximum migration version known to this binary. Derived
/// from the `migrations/` directory by the `MIGRATOR` static.
pub fn binary_schema_version() -> u32 {
    MIGRATOR.iter().map(|m| m.version as u32).max().unwrap_or(0)
}

/// The set of migration versions the current binary would apply, as
/// declared in the `migrations/` directory at build time.
fn binary_versions() -> BTreeSet<i64> {
    MIGRATOR.iter().map(|m| m.version).collect()
}

/// Extract the file path from a `sqlite://…` URL. Returns `None` for
/// `sqlite::memory:` and unparseable URLs.
fn parse_sqlite_file_path(database_url: &str) -> Option<PathBuf> {
    if database_url.contains(":memory:") {
        return None;
    }
    // sqlx accepts `sqlite://path` and `sqlite:path` and `sqlite:///abs`.
    let trimmed = database_url
        .strip_prefix("sqlite://")
        .or_else(|| database_url.strip_prefix("sqlite:"))
        .unwrap_or(database_url);
    // Strip any `?mode=rwc` style query params.
    let path_only = trimmed.split('?').next().unwrap_or(trimmed);
    if path_only.is_empty() {
        return None;
    }
    Some(PathBuf::from(path_only))
}

/// Classify the DB at `database_url` without mutating it. Opens the file
/// read-only if it exists, probes for `_sqlx_migrations`, returns a typed
/// state.
pub async fn classify_db_state(database_url: &str) -> Result<DbState, MigrationError> {
    // Memory DBs are always MissingOrZeroByte — sqlx will create the schema
    // fresh on every pool open.
    let Some(path) = parse_sqlite_file_path(database_url) else {
        return Ok(DbState::MissingOrZeroByte);
    };

    // Check the file stat.
    match std::fs::metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DbState::MissingOrZeroByte);
        }
        Err(e) => {
            return Err(MigrationError::IoError(format!(
                "stat {}: {e}",
                path.display()
            )))
        }
        Ok(m) if m.len() == 0 => {
            return Ok(DbState::MissingOrZeroByte);
        }
        Ok(_) => {}
    }

    // File is non-empty. Probe read-only for `_sqlx_migrations`.
    let opts = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| MigrationError::IoError(format!("parse {database_url}: {e}")))?
        .read_only(true)
        .create_if_missing(false);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| MigrationError::IoError(format!("read-only open {database_url}: {e}")))?;

    // Does `_sqlx_migrations` exist?
    let has_tracker: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| MigrationError::IoError(format!("probe _sqlx_migrations: {e}")))?;

    if has_tracker.is_none() {
        pool.close().await;
        return Ok(DbState::ExistingWithoutTracker);
    }

    // Read applied versions.
    let rows = sqlx::query("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(&pool)
        .await
        .map_err(|e| MigrationError::IoError(format!("read _sqlx_migrations: {e}")))?;

    let mut applied = BTreeSet::new();
    for row in rows {
        let v: i64 = row
            .try_get::<i64, _>("version")
            .map_err(|e| MigrationError::IoError(format!("parse version row: {e}")))?;
        applied.insert(v);
    }

    pool.close().await;
    Ok(DbState::Tracked { applied })
}

/// Inspect the target DB, decide the appropriate action, back it up if
/// needed, and apply migrations under exclusive lock. Returns a
/// [`MigrationOutcome`] on success or a typed [`MigrationError`] that the
/// daemon turns into failed-serve mode.
///
/// `backup_dir` controls where backup files are written. `None` means
/// backups go next to the DB file (recommended default).
pub async fn run_preflight(
    database_url: &str,
    backup_dir: Option<&Path>,
) -> Result<MigrationOutcome, MigrationError> {
    let state = classify_db_state(database_url).await?;
    let binary = binary_versions();
    let binary_max = binary.iter().copied().max().unwrap_or(0);

    match state {
        DbState::MissingOrZeroByte => {
            let pool = open_pool(database_url).await?;
            apply_under_exclusive_lock(&pool).await?;
            pool.close().await;
            Ok(MigrationOutcome {
                schema_version: binary_max as u32,
                backup_path: None,
                classified_as: DbStateKind::MissingOrZeroByte,
                applied_migrations: true,
            })
        }
        DbState::ExistingWithoutTracker => Err(MigrationError::ExistingWithoutTracker(format!(
            "{}: no _sqlx_migrations table — move the file aside or point DATABASE_URL at the intended DB",
            database_url
        ))),
        DbState::Tracked { applied } => {
            let applied_max = applied.iter().copied().max().unwrap_or(0);
            if applied == binary {
                // No-op path.
                return Ok(MigrationOutcome {
                    schema_version: applied_max as u32,
                    backup_path: None,
                    classified_as: DbStateKind::TrackedEqual,
                    applied_migrations: false,
                });
            }
            if applied.is_subset(&binary) {
                let subset_max = applied.iter().copied().max().unwrap_or(0);
                let backup_path = write_backup(database_url, subset_max, binary_max, backup_dir)
                    .await?;
                let pool = open_pool(database_url).await?;
                reconcile_known_applied_migration_checksums(&pool).await?;
                let apply_result = apply_under_exclusive_lock(&pool).await;
                pool.close().await;
                match apply_result {
                    Ok(()) => {
                        // Clean up old backups (retain newest always).
                        let _ = prune_old_backups(&backup_path).await;
                        Ok(MigrationOutcome {
                            schema_version: binary_max as u32,
                            backup_path: Some(backup_path),
                            classified_as: DbStateKind::TrackedSubset,
                            applied_migrations: true,
                        })
                    }
                    Err(MigrationError::LockFailed(msg)) => {
                        // Backup was written before lock failed — preserve the
                        // path in the error so the daemon can surface it.
                        Err(MigrationError::LockFailed(format!(
                            "{msg} (backup at {})",
                            backup_path.display()
                        )))
                    }
                    Err(MigrationError::ApplyFailed(msg)) => {
                        // R13 OPS-001: the tracked-subset branch writes a
                        // backup before applying. Historically only
                        // `LockFailed` propagated the backup path in its
                        // detail, which meant that an `ApplyFailed` (the
                        // most common real-world failure — a migration
                        // raised a runtime error mid-transaction) hid the
                        // very artifact operators need for recovery. Fold
                        // the backup path into the detail so
                        // `map_migration_error` in the daemon can surface
                        // it on `DaemonStatus.failure.backup_path`.
                        Err(MigrationError::ApplyFailed(format!(
                            "{msg} (backup at {})",
                            backup_path.display()
                        )))
                    }
                    Err(e) => Err(e),
                }
            } else if binary.is_subset(&applied) {
                Err(MigrationError::NewerThanBinary {
                    applied_max,
                    binary_max,
                })
            } else {
                let extras: Vec<i64> = applied.difference(&binary).copied().collect();
                Err(MigrationError::InterleavedDivergence { extras })
            }
        }
    }
}

/// Open a writable pool against `database_url` with WAL mode + busy timeout.
/// Factored out so preflight branches share one definition.
async fn open_pool(database_url: &str) -> Result<SqlitePool, MigrationError> {
    let opts = SqliteConnectOptions::from_str(database_url)
        .map_err(|e| MigrationError::IoError(format!("parse {database_url}: {e}")))?
        .create_if_missing(true)
        // Migrations must be able to rebuild legacy tables even when
        // historical data already contains dangling foreign keys. The
        // runtime pool re-enables enforcement after preflight succeeds.
        .foreign_keys(false)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(crate::pool::SQLITE_BUSY_TIMEOUT);
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .map_err(|e| MigrationError::IoError(format!("open_pool: {e}")))
}

/// Apply all pending migrations. Serialization across processes comes
/// from two layers: (1) the daemon's PID lock (§6.1) enforces singleton
/// per DB/auth root, and (2) SQLite's WAL mode + busy_timeout handles the
/// rare case of a non-daemon writer racing with migration application.
/// Per-migration atomicity is owned by sqlx's `Migrator`, which wraps
/// each file in its own transaction — an `ApplyFailed` on one migration
/// leaves the prior applied set consistent.
///
/// `LockFailed` is reserved for the SQLite-level lock acquisition error
/// during `BEGIN EXCLUSIVE` *inside* a migration file (i.e. the migration
/// itself tries to take an exclusive txn and the DB is held by another
/// connection). It comes back from sqlx as a specific error code we
/// detect here.
async fn apply_under_exclusive_lock(pool: &SqlitePool) -> Result<(), MigrationError> {
    match MIGRATOR.run(pool).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            // SQLite error code 5 (SQLITE_BUSY) or 6 (SQLITE_LOCKED) indicate
            // lock contention. Every other migration error is an apply
            // failure (SQL syntax, constraint violation, etc.).
            if msg.contains("(code: 5)")
                || msg.contains("(code: 6)")
                || msg.contains("database is locked")
            {
                Err(MigrationError::LockFailed(msg))
            } else {
                Err(MigrationError::ApplyFailed(msg))
            }
        }
    }
}

async fn reconcile_known_applied_migration_checksums(
    pool: &SqlitePool,
) -> Result<(), MigrationError> {
    reconcile_p058_checksum(pool).await?;
    reconcile_p017_migration_029_checksum(pool).await
}

async fn reconcile_p058_checksum(pool: &SqlitePool) -> Result<(), MigrationError> {
    let Some(current_checksum) = MIGRATOR
        .iter()
        .find(|migration| migration.version == P058_MIGRATION_VERSION)
        .map(|migration| migration.checksum.as_ref().to_vec())
    else {
        return Ok(());
    };

    let row =
        sqlx::query("SELECT checksum FROM _sqlx_migrations WHERE version = ?1 AND success = 1")
            .bind(P058_MIGRATION_VERSION)
            .fetch_optional(pool)
            .await
            .map_err(|e| MigrationError::IoError(format!("read migration 16 checksum: {e}")))?;
    let Some(row) = row else {
        return Ok(());
    };
    let checksum = row
        .try_get::<Vec<u8>, _>("checksum")
        .map_err(|e| MigrationError::IoError(format!("parse migration 16 checksum: {e}")))?;
    if checksum == current_checksum {
        return Ok(());
    }
    if checksum.as_slice() != P058_LEGACY_CHECKSUM {
        return Ok(());
    }
    if !p058_schema_shape_matches(pool).await? {
        return Ok(());
    }

    sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2 AND success = 1")
        .bind(current_checksum)
        .bind(P058_MIGRATION_VERSION)
        .execute(pool)
        .await
        .map_err(|e| MigrationError::IoError(format!("repair migration 16 checksum: {e}")))?;
    Ok(())
}

async fn reconcile_p017_migration_029_checksum(pool: &SqlitePool) -> Result<(), MigrationError> {
    let Some(current_checksum) = MIGRATOR
        .iter()
        .find(|migration| migration.version == P017_MIGRATION_029_VERSION)
        .map(|migration| migration.checksum.as_ref().to_vec())
    else {
        return Ok(());
    };

    let row =
        sqlx::query("SELECT checksum FROM _sqlx_migrations WHERE version = ?1 AND success = 1")
            .bind(P017_MIGRATION_029_VERSION)
            .fetch_optional(pool)
            .await
            .map_err(|e| MigrationError::IoError(format!("read migration 29 checksum: {e}")))?;
    let Some(row) = row else {
        return Ok(());
    };
    let checksum = row
        .try_get::<Vec<u8>, _>("checksum")
        .map_err(|e| MigrationError::IoError(format!("parse migration 29 checksum: {e}")))?;
    if checksum == current_checksum {
        return Ok(());
    }
    if checksum.as_slice() != P017_MIGRATION_029_LEGACY_CHECKSUM {
        return Ok(());
    }
    if !p017_migration_029_schema_shape_matches(pool).await? {
        return Ok(());
    }

    sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2 AND success = 1")
        .bind(current_checksum)
        .bind(P017_MIGRATION_029_VERSION)
        .execute(pool)
        .await
        .map_err(|e| MigrationError::IoError(format!("repair migration 29 checksum: {e}")))?;
    Ok(())
}

async fn p017_migration_029_schema_shape_matches(
    pool: &SqlitePool,
) -> Result<bool, MigrationError> {
    if !table_contains_columns(
        pool,
        "agent_executions",
        &[
            "stage_execution_id",
            "owner_kind",
            "owner_id",
            "lead_mediation_record_id",
            "origin_stage_execution_id",
        ],
    )
    .await?
    {
        return Ok(false);
    }
    if !table_column_is_nullable(pool, "agent_executions", "stage_execution_id").await? {
        return Ok(false);
    }
    for table in [
        "agent_retry_budget_ledger",
        "artifact_source_generation_claims",
    ] {
        if !table_contains_columns(
            pool,
            table,
            &["owner_kind", "owner_id", "stage_execution_id"],
        )
        .await?
        {
            return Ok(false);
        }
        if !table_column_is_nullable(pool, table, "stage_execution_id").await? {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn p058_schema_shape_matches(pool: &SqlitePool) -> Result<bool, MigrationError> {
    for (table, columns) in [
        (
            "agent_execution_runtime_facts",
            &[
                "agent_execution_id",
                "failure_kind",
                "failure_kind_raw_debug",
                "failure_kind_version",
                "failure_message_redacted",
                "failure_message_redaction_version",
                "retry_after",
                "operator_action_hint",
                "provider_exit_status",
                "transport_error_code",
                "supervision_classification",
                "output_settlement",
                "valid_required_outputs",
                "late_output_count",
                "ignored_late_output_count",
                "session_reuse_reason",
                "quota_ledger_id",
                "created_at",
                "updated_at",
            ][..],
        ),
        (
            "agent_retry_budget_ledger",
            &[
                "id",
                "run_id",
                "owner_kind",
                "owner_id",
                "stage_execution_id",
                "agent_execution_id",
                "failure_kind",
                "retry_after",
                "normal_budget_consumed",
                "early_retry_journal_id",
                "idempotency_key",
                "state",
                "created_at",
                "updated_at",
            ][..],
        ),
        (
            "artifact_source_generation_claims",
            &[
                "run_id",
                "owner_kind",
                "owner_id",
                "stage_execution_id",
                "agent_execution_id",
                "source_work_item_id",
                "current_session_generation_id",
                "claim_state",
                "superseding_work_item_id",
                "superseded_by_agent_execution_id",
                "supersession_journal_id",
                "superseded_at",
                "closed_at",
                "created_at",
                "updated_at",
            ][..],
        ),
        (
            "artifact_contract_generations",
            &[
                "source_stage_execution_id",
                "source_session_generation_id",
                "source_work_item_id",
                "supersedes_generation_id",
                "output_settlement",
                "source_generation_verified",
            ][..],
        ),
    ] {
        if !table_contains_columns(pool, table, columns).await? {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn table_column_is_nullable(
    pool: &SqlitePool,
    table: &str,
    column: &str,
) -> Result<bool, MigrationError> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .map_err(|e| MigrationError::IoError(format!("inspect {table} column nullability: {e}")))?;
    let Some(row) = rows.into_iter().find(|row| {
        row.try_get::<String, _>("name")
            .map(|name| name == column)
            .unwrap_or(false)
    }) else {
        return Ok(false);
    };
    let notnull = row
        .try_get::<i64, _>("notnull")
        .map_err(|e| MigrationError::IoError(format!("parse {table}.{column} nullability: {e}")))?;
    Ok(notnull == 0)
}

async fn table_contains_columns(
    pool: &SqlitePool,
    table: &str,
    expected_columns: &[&str],
) -> Result<bool, MigrationError> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .map_err(|e| MigrationError::IoError(format!("inspect {table} columns: {e}")))?;
    let columns: BTreeSet<String> = rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .collect();
    Ok(expected_columns
        .iter()
        .all(|column| columns.contains(*column)))
}

/// Copy the DB file to a timestamped backup and verify the copy is
/// complete. Returns the absolute path of the written backup.
async fn write_backup(
    database_url: &str,
    old_max: i64,
    new_max: i64,
    backup_dir: Option<&Path>,
) -> Result<PathBuf, MigrationError> {
    let src = parse_sqlite_file_path(database_url).ok_or_else(|| {
        MigrationError::BackupFailed("cannot derive file path from DATABASE_URL".into())
    })?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let filename = format!(
        "{}.backup-{}-v{}-to-v{}.sqlite",
        src.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("control-plane.db"),
        ts,
        old_max,
        new_max
    );

    let parent = backup_dir.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        src.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    });

    // Ensure the backup directory exists.
    if let Err(e) = std::fs::create_dir_all(&parent) {
        return Err(MigrationError::BackupFailed(format!(
            "create_dir_all {}: {e}",
            parent.display()
        )));
    }

    let dst = parent.join(filename);

    let src_bytes = std::fs::metadata(&src)
        .map_err(|e| MigrationError::BackupFailed(format!("stat src {}: {e}", src.display())))?
        .len();

    std::fs::copy(&src, &dst).map_err(|e| MigrationError::BackupFailed(format!("copy: {e}")))?;

    let dst_bytes = std::fs::metadata(&dst)
        .map_err(|e| MigrationError::BackupFailed(format!("stat dst: {e}")))?
        .len();

    if src_bytes != dst_bytes {
        let _ = std::fs::remove_file(&dst);
        return Err(MigrationError::BackupFailed(format!(
            "size mismatch: src={src_bytes} dst={dst_bytes}"
        )));
    }

    Ok(dst.canonicalize().unwrap_or(dst))
}

/// Retention: remove backups older than 30 days in the same directory as
/// `latest`, but never remove `latest` itself regardless of age.
async fn prune_old_backups(latest: &Path) -> Result<(), MigrationError> {
    let Some(dir) = latest.parent() else {
        return Ok(());
    };
    let Some(latest_name) = latest.file_name() else {
        return Ok(());
    };

    let cutoff = SystemTime::now() - std::time::Duration::from_secs(30 * 24 * 60 * 60);

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        if name == latest_name {
            continue;
        }
        let name_str = name.to_string_lossy();
        // Only delete `*.backup-*-v*-to-v*.sqlite` files — conservative.
        if !(name_str.contains(".backup-") && name_str.ends_with(".sqlite")) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::now());
        if modified < cutoff {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

impl MigrationOutcome {
    /// Convenience: construct a thin wrapper the daemon can reuse as the
    /// initial `DaemonStatus.schema_version`.
    pub fn schema_version_u32(&self) -> u32 {
        self.schema_version
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn new_tmp_db() -> (TempDir, PathBuf, String) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.db");
        let url = format!("sqlite://{}?mode=rwc", path.display());
        (dir, path, url)
    }

    #[tokio::test]
    async fn classify_missing_file_is_clean_install() {
        let (_dir, _path, url) = new_tmp_db().await;
        let state = classify_db_state(&url).await.unwrap();
        assert!(matches!(state, DbState::MissingOrZeroByte));
    }

    #[tokio::test]
    async fn classify_zero_byte_file_is_clean_install() {
        let (_dir, path, url) = new_tmp_db().await;
        std::fs::write(&path, b"").unwrap();
        let state = classify_db_state(&url).await.unwrap();
        assert!(matches!(state, DbState::MissingOrZeroByte));
    }

    #[tokio::test]
    async fn classify_populated_no_tracker_is_existing_without_tracker() {
        let (_dir, path, url) = new_tmp_db().await;
        // Manually create a SQLite DB with a random table — no `_sqlx_migrations`.
        let pool = open_pool(&url).await.unwrap();
        sqlx::query("CREATE TABLE foo (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        assert!(path.exists());
        let state = classify_db_state(&url).await.unwrap();
        assert!(
            matches!(state, DbState::ExistingWithoutTracker),
            "got {state:?}"
        );
    }

    #[tokio::test]
    async fn classify_fully_migrated_is_tracked_equal() {
        let (_dir, _path, url) = new_tmp_db().await;
        // Let sqlx apply everything.
        let outcome = run_preflight(&url, None).await.unwrap();
        assert!(outcome.applied_migrations);
        let state = classify_db_state(&url).await.unwrap();
        let binary = binary_versions();
        match state {
            DbState::Tracked { applied } => assert_eq!(applied, binary),
            _ => panic!("expected Tracked"),
        }
    }

    #[tokio::test]
    async fn run_preflight_missing_db_clean_installs_and_applies_all() {
        let (_dir, _path, url) = new_tmp_db().await;
        let outcome = run_preflight(&url, None).await.unwrap();
        assert_eq!(outcome.classified_as, DbStateKind::MissingOrZeroByte);
        assert!(outcome.applied_migrations);
        assert!(outcome.backup_path.is_none());
        assert_eq!(outcome.schema_version, binary_schema_version());
    }

    #[tokio::test]
    async fn run_preflight_zero_byte_db_clean_installs_and_applies_all() {
        let (_dir, path, url) = new_tmp_db().await;
        std::fs::write(&path, b"").unwrap();
        let outcome = run_preflight(&url, None).await.unwrap();
        assert_eq!(outcome.classified_as, DbStateKind::MissingOrZeroByte);
        assert!(outcome.applied_migrations);
        assert!(outcome.backup_path.is_none());
    }

    #[tokio::test]
    async fn run_preflight_existing_db_without_tracker_fails_closed() {
        let (_dir, path, url) = new_tmp_db().await;
        let pool = open_pool(&url).await.unwrap();
        sqlx::query("CREATE TABLE foo (id INTEGER PRIMARY KEY)")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
        let size_before = std::fs::metadata(&path).unwrap().len();
        assert!(size_before > 0);

        let err = run_preflight(&url, None).await.unwrap_err();
        match err {
            MigrationError::ExistingWithoutTracker(msg) => {
                assert!(msg.contains("no _sqlx_migrations table"), "msg={msg}");
            }
            other => panic!("expected ExistingWithoutTracker, got {other:?}"),
        }
        // DB is unmodified.
        let size_after = std::fs::metadata(&path).unwrap().len();
        assert_eq!(size_before, size_after);
    }

    #[tokio::test]
    async fn run_preflight_applied_equals_binary_is_noop() {
        let (_dir, _path, url) = new_tmp_db().await;
        // First run: clean install.
        run_preflight(&url, None).await.unwrap();
        // Second run: applied == binary.
        let outcome = run_preflight(&url, None).await.unwrap();
        assert_eq!(outcome.classified_as, DbStateKind::TrackedEqual);
        assert!(!outcome.applied_migrations);
        assert!(outcome.backup_path.is_none());
    }

    #[tokio::test]
    async fn reconciles_legacy_p058_checksum_when_schema_shape_matches() {
        let (_dir, _path, url) = new_tmp_db().await;
        run_preflight(&url, None).await.unwrap();

        let pool = open_pool(&url).await.unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2")
            .bind(P058_LEGACY_CHECKSUM)
            .bind(P058_MIGRATION_VERSION)
            .execute(&pool)
            .await
            .unwrap();

        reconcile_known_applied_migration_checksums(&pool)
            .await
            .unwrap();

        let repaired: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?1")
                .bind(P058_MIGRATION_VERSION)
                .fetch_one(&pool)
                .await
                .unwrap();
        let expected = MIGRATOR
            .iter()
            .find(|migration| migration.version == P058_MIGRATION_VERSION)
            .unwrap()
            .checksum
            .as_ref()
            .to_vec();
        assert_eq!(repaired, expected);
        pool.close().await;
    }

    #[tokio::test]
    async fn reconciles_legacy_p017_migration_029_checksum_when_schema_shape_matches() {
        let (_dir, _path, url) = new_tmp_db().await;
        run_preflight(&url, None).await.unwrap();

        let pool = open_pool(&url).await.unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2")
            .bind(P017_MIGRATION_029_LEGACY_CHECKSUM)
            .bind(P017_MIGRATION_029_VERSION)
            .execute(&pool)
            .await
            .unwrap();

        reconcile_known_applied_migration_checksums(&pool)
            .await
            .unwrap();

        let repaired: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?1")
                .bind(P017_MIGRATION_029_VERSION)
                .fetch_one(&pool)
                .await
                .unwrap();
        let expected = MIGRATOR
            .iter()
            .find(|migration| migration.version == P017_MIGRATION_029_VERSION)
            .unwrap()
            .checksum
            .as_ref()
            .to_vec();
        assert_eq!(repaired, expected);
        pool.close().await;
    }

    #[tokio::test]
    async fn run_preflight_subset_classifies_correctly_and_writes_backup() {
        // Classification-only test for the subset branch. We deliberately
        // do NOT test full re-apply because undoing a real migration's
        // schema effects in SQLite to re-run it is a fixture-fidelity
        // problem, not a correctness guarantee of `run_preflight` —
        // production subset scenarios have a real schema gap, which test
        // data cannot faithfully reproduce with the live `MIGRATOR`.
        //
        // What this test DOES prove:
        // 1. `classify_db_state` correctly returns `Tracked { applied ⊂ binary }`.
        // 2. `write_backup` is called on that branch.
        // 3. The backup file lands with the expected filename shape.
        //
        // What it does NOT prove (covered by integration/gate tests on
        // a real pre-existing DB file fixture):
        // 4. `sqlx::migrate!` re-applies the missing versions cleanly.
        let (dir, _path, url) = new_tmp_db().await;
        run_preflight(&url, None).await.unwrap();
        let pool = open_pool(&url).await.unwrap();
        let max_v: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
            .bind(max_v)
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        // Classification check.
        let state = classify_db_state(&url).await.unwrap();
        match state {
            DbState::Tracked { ref applied } => {
                assert!(
                    applied.len() < binary_versions().len(),
                    "expected strict subset, got applied={:?} binary={:?}",
                    applied,
                    binary_versions()
                );
                assert!(
                    applied.is_subset(&binary_versions()),
                    "applied should be subset"
                );
            }
            _ => panic!("expected Tracked, got {state:?}"),
        }

        // Backup write check — call directly rather than through the full
        // run_preflight pipeline which would try to re-apply migration 13.
        let backup_path = write_backup(&url, max_v - 1, max_v, Some(dir.path()))
            .await
            .unwrap();
        assert!(backup_path.exists());
        assert!(
            backup_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains(".backup-"),
            "backup filename has expected shape: {backup_path:?}"
        );
    }

    #[tokio::test]
    async fn run_preflight_newer_than_binary_fails_closed() {
        let (_dir, path, url) = new_tmp_db().await;
        run_preflight(&url, None).await.unwrap();
        // Insert a fake "future" version into _sqlx_migrations.
        let pool = open_pool(&url).await.unwrap();
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) \
             VALUES (?, ?, CURRENT_TIMESTAMP, 1, randomblob(20), 0)",
        )
        .bind(99_999_i64)
        .bind("future_migration")
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
        let size_before = std::fs::metadata(&path).unwrap().len();

        let err = run_preflight(&url, None).await.unwrap_err();
        match err {
            MigrationError::NewerThanBinary {
                applied_max,
                binary_max,
            } => {
                assert_eq!(applied_max, 99_999);
                assert_eq!(binary_max as u32, binary_schema_version());
            }
            other => panic!("expected NewerThanBinary, got {other:?}"),
        }
        let size_after = std::fs::metadata(&path).unwrap().len();
        assert_eq!(size_before, size_after);
    }

    #[tokio::test]
    async fn run_preflight_interleaved_divergence_fails_closed() {
        let (_dir, _path, url) = new_tmp_db().await;
        run_preflight(&url, None).await.unwrap();
        // Simulate interleaved divergence: delete a mid migration AND add a
        // fake future one, producing applied that is neither subset nor
        // superset of binary.
        let pool = open_pool(&url).await.unwrap();
        let min_v: i64 = sqlx::query_scalar("SELECT MIN(version) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
            .bind(min_v)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) \
             VALUES (88_888, 'future_interleaved', CURRENT_TIMESTAMP, 1, randomblob(20), 0)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;

        let err = run_preflight(&url, None).await.unwrap_err();
        match err {
            MigrationError::InterleavedDivergence { extras } => {
                assert!(extras.contains(&88_888), "extras={extras:?}");
            }
            other => panic!("expected InterleavedDivergence, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn binary_schema_version_matches_migrator_max() {
        let v = binary_schema_version();
        let max_from_migrator = MIGRATOR.iter().map(|m| m.version).max().unwrap() as u32;
        assert_eq!(v, max_from_migrator);
    }

    #[tokio::test]
    async fn classify_memory_db_is_missing_or_zero_byte() {
        let state = classify_db_state("sqlite::memory:").await.unwrap();
        assert!(matches!(state, DbState::MissingOrZeroByte));
    }
}
