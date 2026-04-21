use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::time::Instant;

use domain::ids::RunId;

use crate::pool::{begin_immediate_with_retry, log_write_transaction};
use crate::work_item::{WorkItem, WorkItemKind, WorkItemStatus};

pub async fn enqueue(pool: &SqlitePool, item: &WorkItem) -> Result<()> {
    let tx_started = Instant::now();
    let mut tx = begin_immediate_with_retry(pool, "work_items.enqueue").await?;
    enqueue_tx(&mut tx, item).await?;
    tx.commit().await?;
    log_write_transaction("work_items.enqueue", tx_started);
    Ok(())
}

pub async fn enqueue_tx(tx: &mut Transaction<'_, Sqlite>, item: &WorkItem) -> Result<()> {
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
    .execute(&mut **tx)
    .await
    .context("enqueue work item")?;
    Ok(())
}

pub async fn claim_next(pool: &SqlitePool) -> Result<Option<WorkItem>> {
    claim_next_where(pool, "1 = 1").await
}

pub async fn claim_next_non_invoke(pool: &SqlitePool) -> Result<Option<WorkItem>> {
    claim_next_where(pool, "kind != 'invoke_agent'").await
}

async fn claim_next_where(pool: &SqlitePool, kind_predicate: &str) -> Result<Option<WorkItem>> {
    // Use a transaction to atomically select and update the next pending item.
    let tx_started = Instant::now();
    let mut tx = begin_immediate_with_retry(pool, "work_items.claim_next")
        .await
        .context("begin claim_next transaction")?;

    let now = Utc::now().to_rfc3339();
    let pending_status = WorkItemStatus::Pending.to_string();

    // FIFO ordering with a deterministic tiebreaker. Without `rowid ASC`, two
    // work items enqueued within the same RFC3339 millisecond can be returned
    // in undefined order — a nondeterminism source that flakes tests which
    // depend on enqueue order (e.g. release tests that expect commit before
    // publish). `rowid` is SQLite's monotonic insert sequence, guaranteeing
    // true FIFO semantics in the tiebreaker case.
    let query = format!(
        r#"SELECT id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items
           WHERE status = ?1 AND (scheduled_at <= ?2 OR datetime(scheduled_at) <= datetime(?2)) AND ({kind_predicate})
           ORDER BY scheduled_at ASC, rowid ASC
           LIMIT 1"#
    );
    let row = sqlx::query(&query)
        .bind(&pending_status)
        .bind(&now)
        .fetch_optional(&mut *tx)
        .await
        .context("select next work item")?;

    let Some(row) = row else {
        tx.commit().await.context("commit empty claim_next")?;
        log_write_transaction("work_items.claim_next.empty", tx_started);
        return Ok(None);
    };

    let item_id: String = row.get("id");
    let running_status = WorkItemStatus::Running.to_string();

    let updated = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1, started_at = ?2, attempt_count = attempt_count + 1
           WHERE id = ?3 AND status = ?4"#,
    )
    .bind(&running_status)
    .bind(&now)
    .bind(&item_id)
    .bind(&pending_status)
    .execute(&mut *tx)
    .await
    .context("mark work item running")?
    .rows_affected();
    if updated != 1 {
        anyhow::bail!("claim_next CAS failed for work item {item_id}");
    }

    tx.commit().await.context("commit claim_next")?;
    log_write_transaction("work_items.claim_next", tx_started);

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

pub async fn select_next_pending_invoke_agent_for_start_tx(
    tx: &mut Transaction<'_, Sqlite>,
    now: DateTime<Utc>,
) -> Result<Option<WorkItem>> {
    Ok(select_pending_invoke_agents_for_start_tx(tx, now, 1)
        .await?
        .into_iter()
        .next())
}

pub async fn select_pending_invoke_agents_for_start(
    pool: &SqlitePool,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<WorkItem>> {
    let pending_status = WorkItemStatus::Pending.to_string();
    let invoke_kind = WorkItemKind::InvokeAgent.to_string();
    let now = now.to_rfc3339();
    let rows = sqlx::query(
        r#"SELECT id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items
           WHERE status = ?1 AND (scheduled_at <= ?2 OR datetime(scheduled_at) <= datetime(?2)) AND kind = ?3
           ORDER BY scheduled_at ASC, rowid ASC
           LIMIT ?4"#,
    )
    .bind(&pending_status)
    .bind(&now)
    .bind(&invoke_kind)
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("select pending InvokeAgent work items")?;

    rows.into_iter()
        .map(|row| {
            parse_work_item_row(
                row.get("id"),
                row.get("kind"),
                row.get("payload_json"),
                row.get("status"),
                row.get("run_id"),
                row.get("stage_id"),
                row.get("created_at"),
                row.get("scheduled_at"),
                row.get("attempt_count"),
                row.get("last_error"),
            )
        })
        .collect()
}

pub async fn select_pending_invoke_agents_for_start_tx(
    tx: &mut Transaction<'_, Sqlite>,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<WorkItem>> {
    let pending_status = WorkItemStatus::Pending.to_string();
    let invoke_kind = WorkItemKind::InvokeAgent.to_string();
    let now = now.to_rfc3339();
    let rows = sqlx::query(
        r#"SELECT id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items
           WHERE status = ?1 AND (scheduled_at <= ?2 OR datetime(scheduled_at) <= datetime(?2)) AND kind = ?3
           ORDER BY scheduled_at ASC, rowid ASC
           LIMIT ?4"#,
    )
    .bind(&pending_status)
    .bind(&now)
    .bind(&invoke_kind)
    .bind(limit)
    .fetch_all(&mut **tx)
    .await
    .context("select pending InvokeAgent work items")?;

    rows.into_iter()
        .map(|row| {
            parse_work_item_row(
                row.get("id"),
                row.get("kind"),
                row.get("payload_json"),
                row.get("status"),
                row.get("run_id"),
                row.get("stage_id"),
                row.get("created_at"),
                row.get("scheduled_at"),
                row.get("attempt_count"),
                row.get("last_error"),
            )
        })
        .collect()
}

pub async fn mark_claimed_running_tx(
    tx: &mut Transaction<'_, Sqlite>,
    work_item_id: &str,
    now: DateTime<Utc>,
) -> Result<WorkItem> {
    let running_status = WorkItemStatus::Running.to_string();
    let pending_status = WorkItemStatus::Pending.to_string();
    let updated = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1, started_at = ?2, attempt_count = attempt_count + 1
           WHERE id = ?3 AND status = ?4"#,
    )
    .bind(&running_status)
    .bind(now.to_rfc3339())
    .bind(work_item_id)
    .bind(&pending_status)
    .execute(&mut **tx)
    .await
    .context("mark InvokeAgent work item running")?
    .rows_affected();
    if updated != 1 {
        anyhow::bail!("claim/start CAS failed for InvokeAgent work item {work_item_id}");
    }

    let row = sqlx::query(
        r#"SELECT id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items
           WHERE id = ?1"#,
    )
    .bind(work_item_id)
    .fetch_one(&mut **tx)
    .await
    .context("load claimed work item")?;

    parse_work_item_row(
        row.get("id"),
        row.get("kind"),
        row.get("payload_json"),
        row.get("status"),
        row.get("run_id"),
        row.get("stage_id"),
        row.get("created_at"),
        row.get("scheduled_at"),
        row.get("attempt_count"),
        row.get("last_error"),
    )
}

pub async fn update_payload_json_tx(
    tx: &mut Transaction<'_, Sqlite>,
    work_item_id: &str,
    payload_json: &str,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE work_items
           SET payload_json = ?1
           WHERE id = ?2"#,
    )
    .bind(payload_json)
    .bind(work_item_id)
    .execute(&mut **tx)
    .await
    .context("update work item payload_json")?;
    Ok(())
}

pub async fn requeue_running_preclaimed_invoke_for_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_execution_id: domain::ids::StageExecutionId,
    stage_id: &str,
) -> Result<usize> {
    let items = list_by_run(pool, run_id).await?;
    let stage_execution_id = stage_execution_id.to_string();
    let mut requeued = 0usize;
    for item in items {
        if item.kind != WorkItemKind::InvokeAgent
            || item.status != WorkItemStatus::Running
            || item.stage_id.as_deref() != Some(stage_id)
        {
            continue;
        }
        let payload = match serde_json::from_str::<serde_json::Value>(&item.payload_json) {
            Ok(payload) => payload,
            Err(_) => continue,
        };
        let has_preclaimed = payload
            .pointer("/p058_claimed/agent_execution_id")
            .is_some();
        let payload_stage_execution_id = payload
            .get("stage_execution_id")
            .and_then(|value| value.as_str());
        if !has_preclaimed || payload_stage_execution_id != Some(stage_execution_id.as_str()) {
            continue;
        }
        sqlx::query(
            r#"UPDATE work_items
               SET status = ?1, started_at = NULL, failed_at = NULL, last_error = NULL
               WHERE id = ?2 AND status = ?3"#,
        )
        .bind(WorkItemStatus::Pending.to_string())
        .bind(&item.id)
        .bind(WorkItemStatus::Running.to_string())
        .execute(pool)
        .await
        .context("requeue running preclaimed InvokeAgent work item")?;
        requeued += 1;
    }
    Ok(requeued)
}

pub async fn complete(pool: &SqlitePool, id: &str) -> Result<()> {
    let tx_started = Instant::now();
    let mut tx = begin_immediate_with_retry(pool, "work_items.complete").await?;
    let now = Utc::now().to_rfc3339();
    let status = WorkItemStatus::Completed.to_string();
    let existing = sqlx::query(r#"SELECT kind, run_id, status FROM work_items WHERE id = ?1"#)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .context("select work item before complete")?;
    sqlx::query(r#"UPDATE work_items SET status = ?1, completed_at = ?2 WHERE id = ?3"#)
        .bind(status)
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await
        .context("complete work item")?;
    if let Some(row) = existing {
        let kind: String = row.get("kind");
        let run_id: Option<String> = row.get("run_id");
        let previous_status: String = row.get("status");
        if kind == WorkItemKind::InvokeAgent.to_string()
            && previous_status == WorkItemStatus::Running.to_string()
        {
            if let Some(run_id) = run_id {
                let advance_kind = WorkItemKind::AdvanceRun.to_string();
                let pending_status = WorkItemStatus::Pending.to_string();
                let advance_id = format!("advance-after-invoke:{id}");
                let payload_json = serde_json::json!({
                    "run_id": run_id,
                    "reason": "invoke_agent_completed",
                    "completed_invoke_work_item_id": id,
                })
                .to_string();
                sqlx::query(
                    r#"
                    INSERT OR IGNORE INTO work_items
                      (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error)
                    VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?6, 0, NULL)
                    "#,
                )
                .bind(advance_id)
                .bind(advance_kind)
                .bind(payload_json)
                .bind(pending_status)
                .bind(run_id)
                .bind(&now)
                .execute(&mut *tx)
                .await
                .context("enqueue post-completion AdvanceRun for InvokeAgent")?;
            }
        }
    }
    tx.commit().await.context("commit complete work item")?;
    log_write_transaction("work_items.complete", tx_started);
    Ok(())
}

pub async fn fail(pool: &SqlitePool, id: &str, error: &str) -> Result<()> {
    let tx_started = Instant::now();
    let mut tx = begin_immediate_with_retry(pool, "work_items.fail").await?;
    let now = Utc::now().to_rfc3339();
    let status = WorkItemStatus::Failed.to_string();
    sqlx::query(
        r#"UPDATE work_items SET status = ?1, failed_at = ?2, last_error = ?3 WHERE id = ?4"#,
    )
    .bind(status)
    .bind(now)
    .bind(error)
    .bind(id)
    .execute(&mut *tx)
    .await
    .context("fail work item")?;
    tx.commit().await.context("commit fail work item")?;
    log_write_transaction("work_items.fail", tx_started);
    Ok(())
}

pub async fn fail_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    error: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let status = WorkItemStatus::Failed.to_string();
    sqlx::query(
        r#"UPDATE work_items SET status = ?1, failed_at = ?2, last_error = ?3 WHERE id = ?4"#,
    )
    .bind(status)
    .bind(now.to_rfc3339())
    .bind(error)
    .bind(id)
    .execute(&mut **tx)
    .await
    .context("fail work item")?;
    Ok(())
}

pub async fn cancel_running_by_run(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let tx_started = Instant::now();
    let mut tx = begin_immediate_with_retry(pool, "work_items.cancel_running_by_run").await?;
    let now = Utc::now().to_rfc3339();
    let cancelled = WorkItemStatus::Cancelled.to_string();
    let running = WorkItemStatus::Running.to_string();
    sqlx::query(
        r#"UPDATE work_items
           SET status = ?1, completed_at = ?2
           WHERE run_id = ?3 AND status = ?4"#,
    )
    .bind(cancelled)
    .bind(now)
    .bind(run_id.to_string())
    .bind(running)
    .execute(&mut *tx)
    .await
    .context("cancel running work items by run")?;
    tx.commit()
        .await
        .context("commit cancel running work items by run")?;
    log_write_transaction("work_items.cancel_running_by_run", tx_started);
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

pub async fn list_by_status(pool: &SqlitePool, status: WorkItemStatus) -> Result<Vec<WorkItem>> {
    let rows = sqlx::query(
        r#"SELECT id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items WHERE status = ?1 ORDER BY created_at ASC"#,
    )
    .bind(status.to_string())
    .fetch_all(pool)
    .await
    .context("list work items by status")?;

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
