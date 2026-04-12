use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

/// Record a startup repair action for audit.
pub async fn record(
    pool: &SqlitePool,
    id: &str,
    run_id: &str,
    repair_kind: &str,
    repaired_at: DateTime<Utc>,
    notes: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO startup_repairs (id, run_id, repair_kind, repaired_at, notes)
           VALUES (?1, ?2, ?3, ?4, ?5)"#,
    )
    .bind(id)
    .bind(run_id)
    .bind(repair_kind)
    .bind(repaired_at.to_rfc3339())
    .bind(notes)
    .execute(pool)
    .await
    .context("record startup repair")?;
    Ok(())
}

/// Record a recovery recommendation for a run.
pub async fn recommend(
    pool: &SqlitePool,
    id: &str,
    run_id: &str,
    stage_id: Option<&str>,
    recommendation_kind: &str,
    reason: &str,
    created_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO recovery_recommendations (id, run_id, stage_id, recommendation_kind, reason, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
    )
    .bind(id)
    .bind(run_id)
    .bind(stage_id)
    .bind(recommendation_kind)
    .bind(reason)
    .bind(created_at.to_rfc3339())
    .execute(pool)
    .await
    .context("record recovery recommendation")?;
    Ok(())
}
