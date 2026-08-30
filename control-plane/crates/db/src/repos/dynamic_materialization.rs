use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::ids::{DynamicMaterializationId, RunId};
use domain::routing::DynamicMaterializationRecord;

/// Insert a materialization record. Uses the unique idempotency index to
/// detect duplicates — returns Ok(false) if a record already exists for
/// the same (run_id, stage_id, attempt_id, phase_id, plan_hash, binding_id).
/// Engine callers store a stage-execution epoch in `attempt_id`, not just the
/// workflow attempt number, so loop re-entry can rematerialize the same plan.
pub async fn insert_idempotent(
    pool: &SqlitePool,
    record: &DynamicMaterializationRecord,
) -> Result<bool> {
    let result = crate::execute_repository_write!(
        pool,
        "dynamic_materialization.insert_idempotent",
        sqlx::query(
            r#"
        INSERT OR IGNORE INTO dynamic_materialization_records
            (id, run_id, stage_id, attempt_id, phase_id, plan_hash,
             binding_id, agent_execution_id, idempotency_key, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
        )
        .bind(record.id.to_string())
        .bind(record.run_id.to_string())
        .bind(&record.stage_id)
        .bind(record.attempt_id)
        .bind(&record.phase_id)
        .bind(&record.plan_hash)
        .bind(&record.binding_id)
        .bind(&record.agent_execution_id)
        .bind(&record.idempotency_key)
        .bind(record.created_at.to_rfc3339())
    )
    .context("insert dynamic_materialization_record")?;

    Ok(result.rows_affected() > 0)
}

pub async fn insert_idempotent_tx(
    tx: &mut Transaction<'_, Sqlite>,
    record: &DynamicMaterializationRecord,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO dynamic_materialization_records
            (id, run_id, stage_id, attempt_id, phase_id, plan_hash,
             binding_id, agent_execution_id, idempotency_key, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(record.id.to_string())
    .bind(record.run_id.to_string())
    .bind(&record.stage_id)
    .bind(record.attempt_id)
    .bind(&record.phase_id)
    .bind(&record.plan_hash)
    .bind(&record.binding_id)
    .bind(&record.agent_execution_id)
    .bind(&record.idempotency_key)
    .bind(record.created_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .context("insert dynamic_materialization_record (tx)")?;

    Ok(result.rows_affected() > 0)
}

/// List all materialization records for a stage attempt.
pub async fn list_by_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    attempt_id: i64,
) -> Result<Vec<DynamicMaterializationRecord>> {
    let rows = sqlx::query(
        "SELECT id, run_id, stage_id, attempt_id, phase_id, plan_hash, \
         binding_id, agent_execution_id, idempotency_key, created_at \
         FROM dynamic_materialization_records \
         WHERE run_id = ?1 AND stage_id = ?2 AND attempt_id = ?3 \
         ORDER BY created_at",
    )
    .bind(run_id.to_string())
    .bind(stage_id)
    .bind(attempt_id)
    .fetch_all(pool)
    .await
    .context("list dynamic_materialization_records by stage")?;

    rows.iter().map(parse_row).collect()
}

/// Check if a binding has already been materialized for a given plan hash.
pub async fn is_materialized(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    attempt_id: i64,
    plan_hash: &str,
    binding_id: &str,
) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM dynamic_materialization_records \
         WHERE run_id = ?1 AND stage_id = ?2 AND attempt_id = ?3 \
         AND plan_hash = ?4 AND binding_id = ?5",
    )
    .bind(run_id.to_string())
    .bind(stage_id)
    .bind(attempt_id)
    .bind(plan_hash)
    .bind(binding_id)
    .fetch_one(pool)
    .await
    .context("check dynamic_materialization_record exists")?;

    Ok(count > 0)
}

fn parse_row(r: &sqlx::sqlite::SqliteRow) -> Result<DynamicMaterializationRecord> {
    let id_str: String = r.get("id");
    let id: DynamicMaterializationId = id_str
        .parse()
        .map_err(|e| anyhow::anyhow!("parse dynamic_materialization id: {e}"))?;
    let run_id_str: String = r.get("run_id");
    let run_id: RunId = run_id_str
        .parse()
        .map_err(|e| anyhow::anyhow!("parse dynamic_materialization run_id: {e}"))?;
    let created_at_str: String = r.get("created_at");
    let created_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| anyhow::anyhow!("parse dynamic_materialization created_at: {e}"))?
        .with_timezone(&Utc);

    Ok(DynamicMaterializationRecord {
        id,
        run_id,
        stage_id: r.get("stage_id"),
        attempt_id: r.get("attempt_id"),
        phase_id: r.get("phase_id"),
        plan_hash: r.get("plan_hash"),
        binding_id: r.get("binding_id"),
        agent_execution_id: r.get("agent_execution_id"),
        idempotency_key: r.get("idempotency_key"),
        created_at,
    })
}
