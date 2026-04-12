use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use domain::ids::{RunId, StageExecutionId};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};

pub async fn insert(pool: &SqlitePool, stage: &StageExecution) -> Result<()> {
    let id = stage.id.to_string();
    let run_id = stage.run_id.to_string();
    let status = stage.status.to_string();
    let settlement_kind = stage.settlement_kind.as_ref().map(|k| k.to_string());
    let started_at = stage.started_at.to_rfc3339();
    let completed_at = stage.completed_at.map(|t| t.to_rfc3339());

    sqlx::query(
        r#"
        INSERT INTO stage_executions (id, run_id, stage_id, label, status, iteration, attempt_number, settlement_kind, started_at, completed_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(id)
    .bind(run_id)
    .bind(&stage.stage_id)
    .bind(&stage.label)
    .bind(status)
    .bind(stage.iteration)
    .bind(stage.attempt_number)
    .bind(settlement_kind)
    .bind(started_at)
    .bind(completed_at)
    .execute(pool)
    .await
    .context("insert stage execution")?;
    Ok(())
}

pub async fn find_by_id(
    pool: &SqlitePool,
    id: StageExecutionId,
) -> Result<Option<StageExecution>> {
    let id_str = id.to_string();
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_id, label, status, iteration, attempt_number, settlement_kind, started_at, completed_at
           FROM stage_executions WHERE id = ?1"#,
    )
    .bind(id_str)
    .fetch_optional(pool)
    .await
    .context("find stage execution by id")?;

    row.map(|r| {
        parse_stage_row(
            r.get("id"),
            r.get("run_id"),
            r.get("stage_id"),
            r.get("label"),
            r.get("status"),
            r.get("iteration"),
            r.get("attempt_number"),
            r.get("settlement_kind"),
            r.get("started_at"),
            r.get("completed_at"),
        )
    })
    .transpose()
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<StageExecution>> {
    let run_id_str = run_id.to_string();
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_id, label, status, iteration, attempt_number, settlement_kind, started_at, completed_at
           FROM stage_executions WHERE run_id = ?1 ORDER BY started_at ASC"#,
    )
    .bind(run_id_str)
    .fetch_all(pool)
    .await
    .context("list stage executions by run")?;

    rows.into_iter()
        .map(|r| {
            parse_stage_row(
                r.get("id"),
                r.get("run_id"),
                r.get("stage_id"),
                r.get("label"),
                r.get("status"),
                r.get("iteration"),
                r.get("attempt_number"),
                r.get("settlement_kind"),
                r.get("started_at"),
                r.get("completed_at"),
            )
        })
        .collect()
}

pub async fn update_status(
    pool: &SqlitePool,
    id: StageExecutionId,
    status: StageStatus,
) -> Result<()> {
    let id_str = id.to_string();
    let status_str = status.to_string();
    sqlx::query(r#"UPDATE stage_executions SET status = ?1 WHERE id = ?2"#)
        .bind(status_str)
        .bind(id_str)
        .execute(pool)
        .await
        .context("update stage execution status")?;
    Ok(())
}

pub async fn settle(
    pool: &SqlitePool,
    id: StageExecutionId,
    kind: StageSettlementKind,
    at: DateTime<Utc>,
) -> Result<()> {
    let id_str = id.to_string();
    let kind_str = kind.to_string();
    let at_str = at.to_rfc3339();
    let status = match kind {
        StageSettlementKind::Completed => StageStatus::Completed.to_string(),
        StageSettlementKind::Skipped => StageStatus::Skipped.to_string(),
        StageSettlementKind::Failed => StageStatus::Failed.to_string(),
    };

    sqlx::query(
        r#"UPDATE stage_executions SET status = ?1, settlement_kind = ?2, completed_at = ?3 WHERE id = ?4"#,
    )
    .bind(status)
    .bind(kind_str)
    .bind(at_str)
    .bind(id_str)
    .execute(pool)
    .await
    .context("settle stage execution")?;
    Ok(())
}

fn parse_stage_row(
    id: String,
    run_id: String,
    stage_id: String,
    label: String,
    status: String,
    iteration: i64,
    attempt_number: i64,
    settlement_kind: Option<String>,
    started_at: String,
    completed_at: Option<String>,
) -> Result<StageExecution> {
    let stage_exec_id: StageExecutionId = id
        .parse::<uuid::Uuid>()
        .context("parse stage execution id")?
        .into();
    let run_id_val: RunId = run_id
        .parse::<uuid::Uuid>()
        .context("parse stage run_id")?
        .into();
    let stage_status: StageStatus = status.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let settlement: Option<StageSettlementKind> = settlement_kind
        .map(|s| s.parse().map_err(|e: String| anyhow::anyhow!(e)))
        .transpose()?;
    let started_at_dt: DateTime<Utc> = DateTime::parse_from_rfc3339(&started_at)
        .context("parse stage started_at")?
        .with_timezone(&Utc);
    let completed_at_dt = completed_at
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .context("parse stage completed_at")
                .map(|dt| dt.with_timezone(&Utc))
        })
        .transpose()?;

    Ok(StageExecution {
        id: stage_exec_id,
        run_id: run_id_val,
        stage_id,
        label,
        status: stage_status,
        iteration,
        attempt_number,
        settlement_kind: settlement,
        started_at: started_at_dt,
        completed_at: completed_at_dt,
    })
}
