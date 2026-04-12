use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use domain::ids::RunId;

use crate::work_item::{WorkItem, WorkItemKind, WorkItemStatus};

pub async fn enqueue(pool: &SqlitePool, item: &WorkItem) -> Result<()> {
    let kind = item.kind.to_string();
    let status = item.status.to_string();
    let run_id = item.run_id.map(|r| r.to_string());
    let created_at = item.created_at.to_rfc3339();
    let scheduled_at = item.scheduled_at.to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO work_items (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        "#,
    )
    .bind(&item.id)
    .bind(kind)
    .bind(&item.payload_json)
    .bind(status)
    .bind(run_id)
    .bind(&item.stage_id)
    .bind(created_at)
    .bind(scheduled_at)
    .bind(item.attempt_count)
    .bind(&item.last_error)
    .execute(pool)
    .await
    .context("enqueue work item")?;
    Ok(())
}

pub async fn claim_next(pool: &SqlitePool) -> Result<Option<WorkItem>> {
    // Use a transaction to atomically select and update the next pending item.
    let mut tx = pool.begin().await.context("begin claim_next transaction")?;

    let now = Utc::now().to_rfc3339();
    let pending_status = WorkItemStatus::Pending.to_string();

    let row = sqlx::query(
        r#"SELECT id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items
           WHERE status = ?1 AND scheduled_at <= ?2
           ORDER BY scheduled_at ASC
           LIMIT 1"#,
    )
    .bind(&pending_status)
    .bind(&now)
    .fetch_optional(&mut *tx)
    .await
    .context("select next work item")?;

    let Some(row) = row else {
        tx.commit().await.context("commit empty claim_next")?;
        return Ok(None);
    };

    let item_id: String = row.get("id");
    let running_status = WorkItemStatus::Running.to_string();

    sqlx::query(
        r#"UPDATE work_items
           SET status = ?1, started_at = ?2, attempt_count = attempt_count + 1
           WHERE id = ?3"#,
    )
    .bind(&running_status)
    .bind(&now)
    .bind(&item_id)
    .execute(&mut *tx)
    .await
    .context("mark work item running")?;

    tx.commit().await.context("commit claim_next")?;

    let item = parse_work_item_row(
        row.get("id"),
        row.get("kind"),
        row.get("payload_json"),
        running_status,
        row.get("run_id"),
        row.get("stage_id"),
        row.get("created_at"),
        row.get("scheduled_at"),
        row.get::<i64, _>("attempt_count") + 1,
        row.get("last_error"),
    )?;
    Ok(Some(item))
}

pub async fn complete(pool: &SqlitePool, id: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let status = WorkItemStatus::Completed.to_string();
    sqlx::query(r#"UPDATE work_items SET status = ?1, completed_at = ?2 WHERE id = ?3"#)
        .bind(status)
        .bind(now)
        .bind(id)
        .execute(pool)
        .await
        .context("complete work item")?;
    Ok(())
}

pub async fn fail(pool: &SqlitePool, id: &str, error: &str) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let status = WorkItemStatus::Failed.to_string();
    sqlx::query(
        r#"UPDATE work_items SET status = ?1, failed_at = ?2, last_error = ?3 WHERE id = ?4"#,
    )
    .bind(status)
    .bind(now)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await
    .context("fail work item")?;
    Ok(())
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<WorkItem>> {
    let run_id_str = run_id.to_string();
    let rows = sqlx::query(
        r#"SELECT id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items WHERE run_id = ?1 ORDER BY created_at ASC"#,
    )
    .bind(run_id_str)
    .fetch_all(pool)
    .await
    .context("list work items by run")?;

    rows.into_iter()
        .map(|r| {
            parse_work_item_row(
                r.get("id"),
                r.get("kind"),
                r.get("payload_json"),
                r.get("status"),
                r.get("run_id"),
                r.get("stage_id"),
                r.get("created_at"),
                r.get("scheduled_at"),
                r.get("attempt_count"),
                r.get("last_error"),
            )
        })
        .collect()
}

fn parse_work_item_row(
    id: String,
    kind: String,
    payload_json: String,
    status: String,
    run_id: Option<String>,
    stage_id: Option<String>,
    created_at: String,
    scheduled_at: String,
    attempt_count: i64,
    last_error: Option<String>,
) -> Result<WorkItem> {
    let kind_val: WorkItemKind = kind.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let status_val: WorkItemStatus = status.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let run_id_val: Option<RunId> = run_id
        .map(|s| {
            s.parse::<uuid::Uuid>()
                .context("parse work item run_id")
                .map(|u| u.into())
        })
        .transpose()?;
    let created_at_dt: DateTime<Utc> = DateTime::parse_from_rfc3339(&created_at)
        .context("parse work item created_at")?
        .with_timezone(&Utc);
    let scheduled_at_dt: DateTime<Utc> = DateTime::parse_from_rfc3339(&scheduled_at)
        .context("parse work item scheduled_at")?
        .with_timezone(&Utc);

    Ok(WorkItem {
        id,
        kind: kind_val,
        payload_json,
        status: status_val,
        run_id: run_id_val,
        stage_id,
        created_at: created_at_dt,
        scheduled_at: scheduled_at_dt,
        attempt_count,
        last_error,
    })
}
