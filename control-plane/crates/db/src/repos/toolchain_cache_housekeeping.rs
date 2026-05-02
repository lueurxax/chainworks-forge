// P066 T18: toolchain_cache_housekeeping_readbacks repo.
//
// Low-churn projection: one row per housekeeping sweep.
// Used as promotion gate evidence for Phase 3 catalog backfill.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

#[derive(Clone, Debug, PartialEq)]
pub struct ToolchainCacheHousekeepingReadback {
    pub id: String,
    pub last_sweep_started_at: DateTime<Utc>,
    pub run_scoped_roots_pruned: i64,
    pub run_scoped_prune_failures: i64,
    pub oldest_eligible_root_age_days: Option<f64>,
    pub disk_pressure_blocks: i64,
    pub quarantined_roots_created: i64,
    pub created_at: DateTime<Utc>,
}

pub async fn insert(
    pool: &SqlitePool,
    readback: &ToolchainCacheHousekeepingReadback,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO toolchain_cache_housekeeping_readbacks
           (id, last_sweep_started_at, run_scoped_roots_pruned, run_scoped_prune_failures,
            oldest_eligible_root_age_days, disk_pressure_blocks, quarantined_roots_created,
            created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
    )
    .bind(&readback.id)
    .bind(readback.last_sweep_started_at.to_rfc3339())
    .bind(readback.run_scoped_roots_pruned)
    .bind(readback.run_scoped_prune_failures)
    .bind(readback.oldest_eligible_root_age_days)
    .bind(readback.disk_pressure_blocks)
    .bind(readback.quarantined_roots_created)
    .bind(readback.created_at.to_rfc3339())
    .execute(pool)
    .await
    .context("insert toolchain_cache_housekeeping_readback")?;
    Ok(())
}

pub async fn latest(pool: &SqlitePool) -> Result<Option<ToolchainCacheHousekeepingReadback>> {
    let row = sqlx::query(
        r#"SELECT id, last_sweep_started_at, run_scoped_roots_pruned,
                  run_scoped_prune_failures, oldest_eligible_root_age_days,
                  disk_pressure_blocks, quarantined_roots_created, created_at
           FROM toolchain_cache_housekeeping_readbacks
           ORDER BY last_sweep_started_at DESC, id ASC
           LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .context("load latest toolchain_cache_housekeeping_readback")?;

    row.map(parse_row).transpose()
}

fn parse_row(row: sqlx::sqlite::SqliteRow) -> Result<ToolchainCacheHousekeepingReadback> {
    Ok(ToolchainCacheHousekeepingReadback {
        id: row.get("id"),
        last_sweep_started_at: parse_datetime(row.get("last_sweep_started_at"))?,
        run_scoped_roots_pruned: row.get("run_scoped_roots_pruned"),
        run_scoped_prune_failures: row.get("run_scoped_prune_failures"),
        oldest_eligible_root_age_days: row.get("oldest_eligible_root_age_days"),
        disk_pressure_blocks: row.get("disk_pressure_blocks"),
        quarantined_roots_created: row.get("quarantined_roots_created"),
        created_at: parse_datetime(row.get("created_at"))?,
    })
}

fn parse_datetime(raw: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(raw)
        .with_context(|| format!("parse datetime: {raw}"))?
        .with_timezone(&Utc))
}
