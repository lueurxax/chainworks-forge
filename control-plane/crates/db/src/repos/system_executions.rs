use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::ids::{RoutingReceiptId, RunId, SystemExecutionId};
use domain::routing::{SystemExecution, SystemExecutionStatus};

pub async fn insert_tx(tx: &mut Transaction<'_, Sqlite>, exec: &SystemExecution) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO system_executions (id, run_id, stage_id, attempt_id, task_id, task_type,
                                       status, started_at, completed_at, receipt_id, plan_hash, failure_kind)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(exec.id.to_string())
    .bind(exec.run_id.to_string())
    .bind(&exec.stage_id)
    .bind(exec.attempt_id)
    .bind(&exec.task_id)
    .bind(&exec.task_type)
    .bind(exec.status.to_string())
    .bind(exec.started_at.to_rfc3339())
    .bind(exec.completed_at.map(|t| t.to_rfc3339()))
    .bind(exec.receipt_id.map(|r| r.to_string()))
    .bind(&exec.plan_hash)
    .bind(&exec.failure_kind)
    .execute(&mut **tx)
    .await
    .context("insert system_execution")?;
    Ok(())
}

pub async fn insert(pool: &SqlitePool, exec: &SystemExecution) -> Result<()> {
    let mut tx = crate::writer::begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "system_executions.insert",
            crate::write_class::WriteLane::CriticalBarrier,
            "system_executions.insert",
        ),
        "system_executions.insert",
    )
    .await?;
    insert_tx(&mut tx, exec).await?;
    tx.commit()
        .await
        .context("commit insert system_execution")?;
    Ok(())
}

pub async fn find_by_id(
    pool: &SqlitePool,
    id: SystemExecutionId,
) -> Result<Option<SystemExecution>> {
    let row = sqlx::query(
        "SELECT id, run_id, stage_id, attempt_id, task_id, task_type, status, started_at, \
         completed_at, receipt_id, plan_hash, failure_kind FROM system_executions WHERE id = ?1",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await
    .context("find system_execution by id")?;

    row.map(|r| parse_row(&r)).transpose()
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<SystemExecution>> {
    let rows = sqlx::query(
        "SELECT id, run_id, stage_id, attempt_id, task_id, task_type, status, started_at, \
         completed_at, receipt_id, plan_hash, failure_kind FROM system_executions \
         WHERE run_id = ?1 ORDER BY started_at",
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await
    .context("list system_executions by run")?;

    rows.iter().map(parse_row).collect()
}

pub async fn list_by_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    attempt_id: i64,
) -> Result<Vec<SystemExecution>> {
    let rows = sqlx::query(
        "SELECT id, run_id, stage_id, attempt_id, task_id, task_type, status, started_at, \
         completed_at, receipt_id, plan_hash, failure_kind FROM system_executions \
         WHERE run_id = ?1 AND stage_id = ?2 AND attempt_id = ?3 ORDER BY started_at",
    )
    .bind(run_id.to_string())
    .bind(stage_id)
    .bind(attempt_id)
    .fetch_all(pool)
    .await
    .context("list system_executions by stage")?;

    rows.iter().map(parse_row).collect()
}

fn parse_row(r: &sqlx::sqlite::SqliteRow) -> Result<SystemExecution> {
    let id_str: String = r.get("id");
    let id: SystemExecutionId = id_str
        .parse()
        .map_err(|e| anyhow::anyhow!("parse system_execution id: {e}"))?;
    let run_id_str: String = r.get("run_id");
    let run_id: RunId = run_id_str
        .parse()
        .map_err(|e| anyhow::anyhow!("parse system_execution run_id: {e}"))?;
    let status_str: String = r.get("status");
    let status: SystemExecutionStatus = status_str
        .parse()
        .map_err(|e| anyhow::anyhow!("parse system_execution status: {e}"))?;
    let started_at_str: String = r.get("started_at");
    let started_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&started_at_str)
        .map_err(|e| anyhow::anyhow!("parse system_execution started_at: {e}"))?
        .with_timezone(&Utc);
    let completed_at: Option<DateTime<Utc>> = r
        .get::<Option<String>, _>("completed_at")
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| anyhow::anyhow!("parse system_execution completed_at: {e}"))
        })
        .transpose()?;
    let receipt_id: Option<RoutingReceiptId> = r
        .get::<Option<String>, _>("receipt_id")
        .map(|s| {
            s.parse()
                .map_err(|e| anyhow::anyhow!("parse system_execution receipt_id: {e}"))
        })
        .transpose()?;

    Ok(SystemExecution {
        id,
        run_id,
        stage_id: r.get("stage_id"),
        attempt_id: r.get("attempt_id"),
        task_id: r.get("task_id"),
        task_type: r.get("task_type"),
        status,
        started_at,
        completed_at,
        receipt_id,
        plan_hash: r.get("plan_hash"),
        failure_kind: r.get("failure_kind"),
    })
}
