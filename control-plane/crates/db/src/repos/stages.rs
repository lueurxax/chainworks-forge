use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::ids::{RunId, StageExecutionId};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};

pub async fn insert(pool: &SqlitePool, stage: &StageExecution) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(pool, "stages.insert").await?;
    insert_tx(&mut tx, stage).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_tx(tx: &mut Transaction<'_, Sqlite>, stage: &StageExecution) -> Result<()> {
    let id = stage.id.to_string();
    let run_id = stage.run_id.to_string();
    let status = stage.status.to_string();
    let settlement_kind = stage.settlement_kind.as_ref().map(|k| k.to_string());
    let started_at = stage.started_at.to_rfc3339();
    let completed_at = stage.completed_at.map(|t| t.to_rfc3339());

    sqlx::query(
        r#"
        INSERT INTO stage_executions
            (id, run_id, stage_id, label, status, iteration, attempt_number,
             settlement_kind, started_at, completed_at,
             owner_agent, provider, model, stage_type,
             validation_failure_json, evidence_packet_json, recovery_snapshot_json, retry_reason)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
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
    .bind(&stage.owner_agent)
    .bind(&stage.provider)
    .bind(&stage.model)
    .bind(&stage.stage_type)
    .bind(&stage.validation_failure_json)
    .bind(&stage.evidence_packet_json)
    .bind(&stage.recovery_snapshot_json)
    .bind(&stage.retry_reason)
    .execute(&mut **tx)
    .await
    .context("insert stage execution")?;
    Ok(())
}

const SELECT_COLS: &str = r#"id, run_id, stage_id, label, status, iteration, attempt_number,
             settlement_kind, started_at, completed_at,
             owner_agent, provider, model, stage_type,
             validation_failure_json, evidence_packet_json, recovery_snapshot_json, retry_reason"#;

pub async fn find_by_id(pool: &SqlitePool, id: StageExecutionId) -> Result<Option<StageExecution>> {
    let id_str = id.to_string();
    let query = format!("SELECT {SELECT_COLS} FROM stage_executions WHERE id = ?1");
    let row = sqlx::query(&query)
        .bind(id_str)
        .fetch_optional(pool)
        .await
        .context("find stage execution by id")?;

    row.map(|r| parse_stage_row(&r)).transpose()
}

pub async fn find_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: StageExecutionId,
) -> Result<Option<StageExecution>> {
    let id_str = id.to_string();
    let query = format!("SELECT {SELECT_COLS} FROM stage_executions WHERE id = ?1");
    let row = sqlx::query(&query)
        .bind(id_str)
        .fetch_optional(&mut **tx)
        .await
        .context("find stage execution by id")?;

    row.map(|r| parse_stage_row(&r)).transpose()
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<StageExecution>> {
    let run_id_str = run_id.to_string();
    let query = format!(
        "SELECT {SELECT_COLS} FROM stage_executions WHERE run_id = ?1 ORDER BY started_at ASC"
    );
    let rows = sqlx::query(&query)
        .bind(run_id_str)
        .fetch_all(pool)
        .await
        .context("list stage executions by run")?;

    rows.iter().map(|r| parse_stage_row(r)).collect()
}

pub async fn list_by_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
) -> Result<Vec<StageExecution>> {
    let run_id_str = run_id.to_string();
    let query = format!(
        "SELECT {SELECT_COLS} FROM stage_executions WHERE run_id = ?1 ORDER BY started_at ASC"
    );
    let rows = sqlx::query(&query)
        .bind(run_id_str)
        .fetch_all(&mut **tx)
        .await
        .context("list stage executions by run")?;

    rows.iter().map(|r| parse_stage_row(r)).collect()
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

pub async fn update_status_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: StageExecutionId,
    status: StageStatus,
) -> Result<()> {
    let id_str = id.to_string();
    let status_str = status.to_string();
    sqlx::query(r#"UPDATE stage_executions SET status = ?1 WHERE id = ?2"#)
        .bind(status_str)
        .bind(id_str)
        .execute(&mut **tx)
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
    let mut tx = crate::writer::begin_repository_transaction(pool, "stages.settle").await?;
    settle_tx(&mut tx, id, kind, at).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn settle_tx(
    tx: &mut Transaction<'_, Sqlite>,
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
    .execute(&mut **tx)
    .await
    .context("settle stage execution")?;
    Ok(())
}

pub async fn update_validation_failure_json(
    pool: &SqlitePool,
    id: StageExecutionId,
    validation_failure_json: &str,
) -> Result<()> {
    sqlx::query(r#"UPDATE stage_executions SET validation_failure_json = ?1 WHERE id = ?2"#)
        .bind(validation_failure_json)
        .bind(id.to_string())
        .execute(pool)
        .await
        .context("update stage validation failure json")?;
    Ok(())
}

pub async fn update_recovery_snapshot_json(
    pool: &SqlitePool,
    id: StageExecutionId,
    recovery_snapshot_json: &str,
) -> Result<()> {
    sqlx::query(r#"UPDATE stage_executions SET recovery_snapshot_json = ?1 WHERE id = ?2"#)
        .bind(recovery_snapshot_json)
        .bind(id.to_string())
        .execute(pool)
        .await
        .context("update stage recovery snapshot json")?;
    Ok(())
}

pub async fn update_evidence_packet_json(
    pool: &SqlitePool,
    id: StageExecutionId,
    evidence_packet_json: &str,
) -> Result<()> {
    sqlx::query(r#"UPDATE stage_executions SET evidence_packet_json = ?1 WHERE id = ?2"#)
        .bind(evidence_packet_json)
        .bind(id.to_string())
        .execute(pool)
        .await
        .context("update stage evidence packet json")?;
    Ok(())
}

fn parse_stage_row(r: &sqlx::sqlite::SqliteRow) -> Result<StageExecution> {
    let id: String = r.get("id");
    let run_id: String = r.get("run_id");
    let status: String = r.get("status");
    let settlement_kind: Option<String> = r.get("settlement_kind");
    let started_at: String = r.get("started_at");
    let completed_at: Option<String> = r.get("completed_at");

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
        stage_id: r.get("stage_id"),
        label: r.get("label"),
        status: stage_status,
        iteration: r.get("iteration"),
        attempt_number: r.get("attempt_number"),
        settlement_kind: settlement,
        started_at: started_at_dt,
        completed_at: completed_at_dt,
        owner_agent: r.get("owner_agent"),
        provider: r.get("provider"),
        model: r.get("model"),
        stage_type: r.get("stage_type"),
        validation_failure_json: r.get("validation_failure_json"),
        evidence_packet_json: r.get("evidence_packet_json"),
        recovery_snapshot_json: r.get("recovery_snapshot_json"),
        retry_reason: r.get("retry_reason"),
    })
}
