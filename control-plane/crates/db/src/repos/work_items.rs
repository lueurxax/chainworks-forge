use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use std::time::Instant;

use domain::agent::{AgentStatus, ArtifactSourceClaimState};
use domain::artifact_contracts::ArtifactSourceGenerationClaimKey;
use domain::ids::{AgentExecutionId, RunId, StageExecutionId};
use domain::provider::InvokeAgentCapacityConfig;
use domain::retry_authority::AdvanceRunPayloadV1;

use crate::pool::log_write_transaction;
use crate::work_item::{WorkItem, WorkItemKind, WorkItemStatus};
use crate::writer::begin_registered_immediate_transaction;

use super::scheduler;

pub async fn enqueue(pool: &SqlitePool, item: &WorkItem) -> Result<()> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.enqueue",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.enqueue",
        ),
        "work_items.enqueue",
    )
    .await?;
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
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.claim_next",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.claim_next",
        ),
        "work_items.claim_next",
    )
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
    let row = loop {
        let row = sqlx::query(&query)
            .bind(&pending_status)
            .bind(&now)
            .fetch_optional(&mut **tx)
            .await
            .context("select next work item")?;

        let Some(row) = row else {
            tx.commit().await.context("commit empty claim_next")?;
            log_write_transaction("work_items.claim_next.empty", tx_started);
            return Ok(None);
        };

        let kind: String = row.get("kind");
        let payload_json: String = row.get("payload_json");
        if kind == WorkItemKind::AdvanceRun.to_string() {
            match classify_advance_payload_scope(&payload_json) {
                AdvancePayloadScope::Malformed(code) => {
                    let item_id: String = row.get("id");
                    quarantine_advance_work_item_tx(&mut tx, &item_id, code).await?;
                    continue;
                }
                AdvancePayloadScope::LegacyRunScoped | AdvancePayloadScope::Targeted => {}
            }
        }
        break row;
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
    .execute(&mut **tx)
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
    let stage_execution_id = stage_execution_id.to_string();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_preclaimed_invoke",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_preclaimed_invoke",
        ),
        "work_items.requeue_preclaimed_invoke",
    )
    .await?;
    let requeued = requeue_running_preclaimed_invoke_for_stage_tx(
        &mut tx,
        run_id,
        &stage_execution_id,
        stage_id,
    )
    .await?;
    tx.commit()
        .await
        .context("commit preclaimed InvokeAgent requeue")?;
    Ok(requeued)
}

pub async fn requeue_running_preclaimed_invoke_for_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_execution_id: &str,
    stage_id: &str,
) -> Result<usize> {
    let rows = sqlx::query(
        r#"SELECT id, payload_json
           FROM work_items
           WHERE run_id = ?1 AND stage_id = ?2 AND kind = ?3 AND status = ?4
           ORDER BY scheduled_at ASC, rowid ASC"#,
    )
    .bind(run_id.to_string())
    .bind(stage_id)
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("load running preclaimed InvokeAgent work items")?;

    let mut requeued = 0usize;
    for row in rows {
        let payload_json: String = row.get("payload_json");
        let mut payload = match serde_json::from_str::<serde_json::Value>(&payload_json) {
            Ok(payload) => payload,
            Err(_) => continue,
        };
        let has_preclaimed = payload
            .pointer("/p058_claimed/agent_execution_id")
            .is_some();
        let payload_stage_execution_id = payload
            .get("stage_execution_id")
            .and_then(|value| value.as_str());
        if !has_preclaimed || payload_stage_execution_id != Some(stage_execution_id) {
            continue;
        }
        supersede_abandoned_preclaim_for_retry_tx(
            tx,
            &row.get::<String, _>("id"),
            &payload,
            Utc::now(),
        )
        .await?;
        if let Some(object) = payload.as_object_mut() {
            object.remove("p058_claimed");
            object.insert(
                "p061_startup_recovery".to_string(),
                serde_json::json!({
                    "requeued_at": Utc::now().to_rfc3339(),
                    "reason": "startup_repair_preclaimed_invoke_agent",
                }),
            );
        }
        let payload_json = serde_json::to_string(&payload)
            .context("serialize startup recovery requeued InvokeAgent payload")?;

        sqlx::query(
            r#"UPDATE work_items
               SET status = ?1, payload_json = ?2, started_at = NULL, failed_at = NULL, last_error = NULL
               WHERE id = ?3 AND status = ?4"#,
        )
        .bind(WorkItemStatus::Pending.to_string())
        .bind(payload_json)
        .bind(row.get::<String, _>("id"))
        .bind(WorkItemStatus::Running.to_string())
        .execute(&mut **tx)
        .await
        .context("requeue running preclaimed InvokeAgent work item")?;
        requeued += 1;
    }
    Ok(requeued)
}

pub async fn has_pending_or_running_by_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM work_items
           WHERE run_id = ?1 AND status IN (?2, ?3)"#,
    )
    .bind(run_id.to_string())
    .bind(WorkItemStatus::Pending.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_one(&mut **tx)
    .await
    .context("count pending/running work items by run")?;
    Ok(count > 0)
}

pub async fn settle_terminal_preclaimed_invoke_agent_executions(
    pool: &SqlitePool,
    fallback_completed_at: DateTime<Utc>,
) -> Result<u64> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.settle_terminal_preclaimed_invoke_agent_executions",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.settle_terminal_preclaimed_invoke_agent_executions",
        ),
        "work_items.settle_terminal_preclaimed_invoke_agent_executions",
    )
    .await?;
    let settled =
        settle_terminal_preclaimed_invoke_agent_executions_tx(&mut tx, fallback_completed_at)
            .await?;
    tx.commit()
        .await
        .context("commit terminal preclaimed InvokeAgent execution settlement")?;
    log_write_transaction(
        "work_items.settle_terminal_preclaimed_invoke_agent_executions",
        tx_started,
    );
    Ok(settled)
}

pub async fn settle_terminal_preclaimed_invoke_agent_executions_tx(
    tx: &mut Transaction<'_, Sqlite>,
    fallback_completed_at: DateTime<Utc>,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT id, payload_json, status, completed_at, failed_at
           FROM work_items
           WHERE kind = ?1 AND status IN (?2, ?3, ?4)
           ORDER BY created_at ASC, rowid ASC"#,
    )
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Completed.to_string())
    .bind(WorkItemStatus::Failed.to_string())
    .bind(WorkItemStatus::Cancelled.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("load terminal preclaimed InvokeAgent work items")?;

    let mut settled = 0_u64;
    for row in rows {
        let payload_json: String = row.get("payload_json");
        let Ok(payload) = serde_json::from_str::<serde_json::Value>(&payload_json) else {
            continue;
        };
        let Some(agent_execution_id) = payload
            .pointer("/p058_claimed/agent_execution_id")
            .and_then(|value| value.as_str())
        else {
            continue;
        };

        let work_item_status: String = row.get("status");
        let agent_status = match work_item_status.as_str() {
            "completed" => AgentStatus::Completed,
            "failed" => AgentStatus::Failed,
            "cancelled" => AgentStatus::Cancelled,
            _ => continue,
        };
        let completed_at = row
            .get::<Option<String>, _>("completed_at")
            .or_else(|| row.get::<Option<String>, _>("failed_at"))
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc))
            .unwrap_or(fallback_completed_at);

        let result = sqlx::query(
            r#"UPDATE agent_executions
               SET status = ?1, completed_at = ?2
               WHERE id = ?3 AND status = ?4"#,
        )
        .bind(agent_status.to_string())
        .bind(completed_at.to_rfc3339())
        .bind(agent_execution_id)
        .bind(AgentStatus::Running.to_string())
        .execute(&mut **tx)
        .await
        .with_context(|| {
            format!(
                "settle terminal preclaimed InvokeAgent execution for work item {}",
                row.get::<String, _>("id")
            )
        })?;
        settled += result.rows_affected();
        sqlx::query(
            r#"UPDATE artifact_source_generation_claims
               SET claim_state = ?1,
                   closed_at = COALESCE(closed_at, ?2),
                   updated_at = ?2
               WHERE source_work_item_id = ?3
                 AND agent_execution_id = ?4
                 AND claim_state = ?5"#,
        )
        .bind("closed")
        .bind(completed_at.to_rfc3339())
        .bind(row.get::<String, _>("id"))
        .bind(agent_execution_id)
        .bind("active")
        .execute(&mut **tx)
        .await
        .with_context(|| {
            format!(
                "close terminal preclaimed InvokeAgent source-generation claim for work item {}",
                row.get::<String, _>("id")
            )
        })?;
    }

    Ok(settled)
}

async fn supersede_abandoned_preclaim_for_retry_tx(
    tx: &mut Transaction<'_, Sqlite>,
    work_item_id: &str,
    payload: &serde_json::Value,
    settled_at: DateTime<Utc>,
) -> Result<()> {
    let Some(claimed) = payload.get("p058_claimed") else {
        return Ok(());
    };
    let Some(agent_execution_id) = claimed
        .get("agent_execution_id")
        .and_then(|value| value.as_str())
    else {
        return Ok(());
    };
    let settled_at = settled_at.to_rfc3339();
    sqlx::query(
        r#"UPDATE agent_executions
           SET status = ?1, completed_at = COALESCE(completed_at, ?2)
           WHERE id = ?3 AND status = ?4"#,
    )
    .bind(AgentStatus::Cancelled.to_string())
    .bind(&settled_at)
    .bind(agent_execution_id)
    .bind(AgentStatus::Running.to_string())
    .execute(&mut **tx)
    .await
    .context("settle abandoned preclaimed InvokeAgent execution before retry")?;

    let Some(claim_key) = claimed
        .get("artifact_claim_key")
        .cloned()
        .and_then(|value| serde_json::from_value::<ArtifactSourceGenerationClaimKey>(value).ok())
    else {
        return Ok(());
    };
    sqlx::query(
        r#"UPDATE artifact_source_generation_claims
           SET claim_state = ?1,
               superseding_work_item_id = ?2,
               superseded_at = ?3,
               updated_at = ?3
           WHERE run_id = ?4 AND owner_kind = ?5 AND owner_id = ?6 AND agent_execution_id = ?7
             AND source_work_item_id = ?8 AND claim_state = ?9"#,
    )
    .bind(ArtifactSourceClaimState::SupersededPendingRetry.to_string())
    .bind(work_item_id)
    .bind(&settled_at)
    .bind(claim_key.run_id.to_string())
    .bind(claim_key.owner_kind.to_string())
    .bind(&claim_key.owner_id)
    .bind(agent_execution_id)
    .bind(&claim_key.source_work_item_id)
    .bind(ArtifactSourceClaimState::Active.to_string())
    .execute(&mut **tx)
    .await
    .context("supersede abandoned preclaimed source-generation claim before retry")?;

    Ok(())
}

pub async fn requeue_running_advance_by_run(
    pool: &SqlitePool,
    run_id: RunId,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_running_advance_by_run",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_running_advance_by_run",
        ),
        "work_items.requeue_running_advance_by_run",
    )
    .await?;
    let requeued = requeue_running_advance_by_run_tx(&mut tx, run_id, scheduled_at, reason).await?;
    tx.commit()
        .await
        .context("commit requeue running AdvanceRun work items by run")?;
    log_write_transaction("work_items.requeue_running_advance_by_run", tx_started);
    Ok(requeued)
}

pub async fn requeue_running_advance_by_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT id, payload_json
           FROM work_items
           WHERE run_id = ?1
             AND kind = ?2
             AND status = ?3"#,
    )
    .bind(run_id.to_string())
    .bind(WorkItemKind::AdvanceRun.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("select running AdvanceRun work items by run")?;
    let mut requeued = 0;
    for row in rows {
        let id: String = row.get("id");
        let payload: String = row.get("payload_json");
        match classify_advance_payload_scope(&payload) {
            AdvancePayloadScope::LegacyRunScoped => {
                let result = sqlx::query(
                    r#"UPDATE work_items
                       SET status = ?1,
                           scheduled_at = ?2,
                           attempt_count = attempt_count + 1,
                           last_error = ?3
                       WHERE id = ?4
                         AND status = ?5"#,
                )
                .bind(WorkItemStatus::Pending.to_string())
                .bind(scheduled_at.to_rfc3339())
                .bind(reason)
                .bind(id)
                .bind(WorkItemStatus::Running.to_string())
                .execute(&mut **tx)
                .await
                .context("requeue run-scoped AdvanceRun work item")?;
                requeued += result.rows_affected();
            }
            AdvancePayloadScope::Targeted => {}
            AdvancePayloadScope::Malformed(code) => {
                quarantine_advance_work_item_tx(tx, &id, code).await?;
            }
        }
    }
    Ok(requeued)
}

pub async fn requeue_running_advance_by_retry_authority_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    retry_authority_id: &str,
    target_stage_execution_id: StageExecutionId,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT id, payload_json
           FROM work_items
           WHERE run_id = ?1
             AND kind = ?2
             AND status = ?3"#,
    )
    .bind(run_id.to_string())
    .bind(WorkItemKind::AdvanceRun.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("select running targeted AdvanceRun work items")?;

    let mut requeued = 0;
    for row in rows {
        let id: String = row.get("id");
        let payload: String = row.get("payload_json");
        match advance_payload_matches_authority(
            &payload,
            retry_authority_id,
            target_stage_execution_id,
        ) {
            AdvancePayloadAuthorityMatch::Match => {}
            AdvancePayloadAuthorityMatch::NoMatch => continue,
            AdvancePayloadAuthorityMatch::Malformed(code) => {
                quarantine_advance_work_item_tx(tx, &id, code).await?;
                continue;
            }
        }
        let result = sqlx::query(
            r#"UPDATE work_items
               SET status = ?1,
                   scheduled_at = ?2,
                   attempt_count = attempt_count + 1,
                   last_error = ?3
               WHERE id = ?4
                 AND status = ?5"#,
        )
        .bind(WorkItemStatus::Pending.to_string())
        .bind(scheduled_at.to_rfc3339())
        .bind(reason)
        .bind(id)
        .bind(WorkItemStatus::Running.to_string())
        .execute(&mut **tx)
        .await
        .context("requeue targeted AdvanceRun work item")?;
        requeued += result.rows_affected();
    }
    Ok(requeued)
}

pub async fn requeue_stale_running_advance_items(
    pool: &SqlitePool,
    stale_before: DateTime<Utc>,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_stale_running_advance_items",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_stale_running_advance_items",
        ),
        "work_items.requeue_stale_running_advance_items",
    )
    .await?;
    let requeued =
        requeue_stale_running_advance_items_tx(&mut tx, stale_before, scheduled_at, reason).await?;
    tx.commit()
        .await
        .context("commit stale running AdvanceRun watchdog requeue")?;
    log_write_transaction("work_items.requeue_stale_running_advance_items", tx_started);
    Ok(requeued)
}

pub async fn requeue_stale_running_advance_items_tx(
    tx: &mut Transaction<'_, Sqlite>,
    stale_before: DateTime<Utc>,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, payload_json
           FROM work_items
           WHERE kind = ?1
             AND status = ?2
             AND COALESCE(started_at, scheduled_at) <= ?3"#,
    )
    .bind(WorkItemKind::AdvanceRun.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .bind(stale_before.to_rfc3339())
    .fetch_all(&mut **tx)
    .await
    .context("select stale running AdvanceRun work items")?;

    let mut requeued = 0;
    for row in rows {
        let id: String = row.get("id");
        let run_id: Option<String> = row.get("run_id");
        let payload: String = row.get("payload_json");
        match classify_advance_payload_scope(&payload) {
            AdvancePayloadScope::LegacyRunScoped => {
                requeued +=
                    requeue_running_advance_work_item_by_id_tx(tx, &id, scheduled_at, reason)
                        .await?;
            }
            AdvancePayloadScope::Targeted => {
                let payload = AdvanceRunPayloadV1::parse_json(&payload)
                    .context("parse targeted stale AdvanceRun payload")?;
                let Some(authority_id) = payload.retry_authority_id.as_deref() else {
                    quarantine_advance_work_item_tx(
                        tx,
                        &id,
                        "advance_run_payload_missing_retry_authority",
                    )
                    .await?;
                    continue;
                };
                let Some(target_stage_execution_id) = payload.target_stage_execution_id else {
                    quarantine_advance_work_item_tx(
                        tx,
                        &id,
                        "advance_run_payload_missing_target_for_authority",
                    )
                    .await?;
                    continue;
                };
                let payload_run_id = payload.run_id.to_string();
                let active_authority =
                    find_active_retry_authority_by_id_tx(tx, &payload_run_id, authority_id).await?;
                let Some(active_authority) = active_authority else {
                    continue;
                };
                let authority_target: String = active_authority.get("target_stage_execution_id");
                if authority_target != target_stage_execution_id.to_string() {
                    quarantine_advance_work_item_tx(
                        tx,
                        &id,
                        "advance_run_payload_target_authority_mismatch",
                    )
                    .await?;
                    continue;
                }
                if run_id.as_deref() != Some(payload.run_id.to_string().as_str()) {
                    quarantine_advance_work_item_tx(tx, &id, "advance_run_payload_run_id_mismatch")
                        .await?;
                    continue;
                }
                requeued +=
                    requeue_running_advance_work_item_by_id_tx(tx, &id, scheduled_at, reason)
                        .await?;
            }
            AdvancePayloadScope::Malformed(code) => {
                quarantine_advance_work_item_tx(tx, &id, code).await?;
            }
        }
    }
    Ok(requeued)
}

async fn requeue_running_advance_work_item_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let result = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1,
               scheduled_at = ?2,
               started_at = NULL,
               failed_at = NULL,
               attempt_count = attempt_count + 1,
               last_error = ?3
           WHERE id = ?4
             AND kind = ?5
             AND status = ?6"#,
    )
    .bind(WorkItemStatus::Pending.to_string())
    .bind(scheduled_at.to_rfc3339())
    .bind(reason)
    .bind(id)
    .bind(WorkItemKind::AdvanceRun.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .execute(&mut **tx)
    .await
    .context("requeue stale running AdvanceRun work item")?;
    Ok(result.rows_affected())
}

pub async fn requeue_running_steward_analysis_on_startup(
    pool: &SqlitePool,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_running_steward_analysis_on_startup",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_running_steward_analysis_on_startup",
        ),
        "work_items.requeue_running_steward_analysis_on_startup",
    )
    .await?;
    let requeued =
        requeue_running_steward_analysis_on_startup_tx(&mut tx, scheduled_at, reason).await?;
    tx.commit()
        .await
        .context("commit requeue running StewardAnalysis work items")?;
    log_write_transaction(
        "work_items.requeue_running_steward_analysis_on_startup",
        tx_started,
    );
    Ok(requeued)
}

pub async fn requeue_running_steward_analysis_on_startup_tx(
    tx: &mut Transaction<'_, Sqlite>,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let pending = WorkItemStatus::Pending.to_string();
    let running = WorkItemStatus::Running.to_string();
    let result = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1,
               scheduled_at = ?2,
               started_at = NULL,
               completed_at = NULL,
               failed_at = NULL,
               attempt_count = attempt_count + 1,
               last_error = ?3
           WHERE kind = ?4
             AND status = ?5"#,
    )
    .bind(pending)
    .bind(scheduled_at.to_rfc3339())
    .bind(reason)
    .bind(WorkItemKind::StewardAnalysis.to_string())
    .bind(running)
    .execute(&mut **tx)
    .await
    .context("requeue running StewardAnalysis work items on startup")?;
    Ok(result.rows_affected())
}

pub async fn requeue_running_invoke_agent_on_startup(
    pool: &SqlitePool,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_running_invoke_on_startup",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_running_invoke_on_startup",
        ),
        "work_items.requeue_running_invoke_on_startup",
    )
    .await?;
    let requeued =
        requeue_running_invoke_agent_on_startup_tx(&mut tx, scheduled_at, reason).await?;
    tx.commit()
        .await
        .context("commit requeue running InvokeAgent work items")?;
    log_write_transaction("work_items.requeue_running_invoke_on_startup", tx_started);
    Ok(requeued)
}

fn p082_source_command_journal_id_from_payload(payload: &serde_json::Value) -> Option<String> {
    [
        "/targeted_retry/journal_id",
        "/operator_retry_instruction/journal_id",
        "/source_command_journal_id",
        "/journal_id",
    ]
    .iter()
    .find_map(|pointer| {
        payload
            .pointer(pointer)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

async fn p082_latest_command_journal_id_for_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
) -> Result<Option<String>> {
    if run_id.trim().is_empty() {
        return Ok(None);
    }
    sqlx::query_scalar::<_, String>(
        r#"SELECT id
           FROM command_journal
           WHERE run_id = ?1
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await
    .context("load latest command journal id for P082 startup repair")
}

pub async fn requeue_running_invoke_agent_on_startup_tx(
    tx: &mut Transaction<'_, Sqlite>,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT id, payload_json
           FROM work_items
           WHERE kind = ?1 AND status = ?2
           ORDER BY scheduled_at ASC, rowid ASC"#,
    )
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("load running InvokeAgent work items on startup")?;

    let pending = WorkItemStatus::Pending.to_string();
    let running = WorkItemStatus::Running.to_string();
    let scheduled_at_rfc3339 = scheduled_at.to_rfc3339();
    let mut requeued = 0_u64;
    for row in rows {
        let item_id: String = row.get("id");
        let payload_json: String = row.get("payload_json");
        let mut payload = serde_json::from_str::<serde_json::Value>(&payload_json)
            .unwrap_or_else(|_| serde_json::json!({}));
        supersede_abandoned_preclaim_for_retry_tx(tx, &item_id, &payload, scheduled_at).await?;
        let run_id_for_repair = payload
            .get("run_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let source_command_journal_id = match p082_source_command_journal_id_from_payload(&payload)
        {
            Some(journal_id) => journal_id,
            None => p082_latest_command_journal_id_for_run_tx(tx, &run_id_for_repair)
                .await?
                .unwrap_or_else(|| "unavailable".to_string()),
        };
        // P082-R01: build the startup repair idempotency key and summary.
        // Key format: p082-requeue:{command_journal.id}:{source_work_item_id}:1.
        let repair_id = format!("p082-requeue:{source_command_journal_id}:{item_id}:1");
        let p082_summary = domain::recovery_matrix::build_startup_repair_summary(
            &repair_id,
            &item_id,
            &source_command_journal_id,
            1,
            1,
            false,
            60_000,
            &scheduled_at_rfc3339,
            false,
            None,
            "global",
        );
        let p082_readback = domain::recovery_matrix::build_readback_v1(
            "P082-R01",
            "repaired",
            "retry",
            domain::recovery_matrix::REASON_STARTUP_REQUEUE_ONCE,
            "Startup recovery requeued abandoned InvokeAgent work item.",
            "startup_repairs, work_items, command_journal",
            "startup_repairs, work_items",
            &repair_id,
            Some("startup_repairs.notes.p082_recovery_matrix_readback"),
            "valid",
            &scheduled_at_rfc3339,
        );
        let p082_readback_with_summary =
            domain::recovery_matrix::set_readback_startup_repair(p082_readback, p082_summary, None);
        let notes_json = serde_json::json!({
            "p082_recovery_matrix_readback": p082_readback_with_summary,
        });
        let record_result = super::startup_repairs::record_tx(
            &mut **tx,
            &repair_id,
            &run_id_for_repair,
            "p082_requeue_once",
            scheduled_at,
            Some(&notes_json.to_string()),
        )
        .await;
        if record_result.is_err() {
            // P082-R16: idempotency key already exists — generation 1 was already consumed.
            // Do NOT write a second startup_repairs row (one-idempotency-row invariant).
            // UPDATE startup_repairs.notes with the R16 readback so Source 1 of the
            // readbacks_for_run accessor surfaces it from the approved storage owner
            // (startup_repairs.notes.p082_recovery_matrix_readback).
            let held_summary = domain::recovery_matrix::build_startup_repair_summary(
                &repair_id,
                &item_id,
                &source_command_journal_id,
                1,
                1,
                true,
                60_000,
                &scheduled_at_rfc3339,
                false,
                None,
                "global",
            );
            let held_readback = domain::recovery_matrix::set_readback_startup_repair(
                domain::recovery_matrix::build_readback_v1(
                    "P082-R16",
                    "held",
                    "wait",
                    domain::recovery_matrix::REASON_STARTUP_REQUEUE_EXHAUSTED,
                    "Startup requeue generation 1 was already consumed; no duplicate work was enqueued.",
                    "startup_repairs, work_items",
                    "startup_repairs, work_items",
                    &repair_id,
                    Some("startup_repairs.notes.p082_recovery_matrix_readback"),
                    "valid",
                    &scheduled_at_rfc3339,
                ),
                held_summary,
                Some("Startup requeue exhausted: generation 1 was already consumed. Use existing recovery inspection or cancellation paths to clear the hold."),
            );
            let r16_notes = serde_json::json!({
                "p082_recovery_matrix_readback": held_readback,
            });
            sqlx::query(
                "UPDATE startup_repairs SET notes = ?1 WHERE id = ?2",
            )
            .bind(r16_notes.to_string())
            .bind(&repair_id)
            .execute(&mut **tx)
            .await
            .context("update startup_repairs notes for P082-R16 held state")?;
            crate::metrics::increment_counter_with_label(
                "p082_recovery_idempotency_replay_total",
                "P082-R16:startup_requeue_exhausted",
            );
            sqlx::query(
                r#"UPDATE work_items
                   SET status = ?1, failed_at = ?2, last_error = ?3
                   WHERE id = ?4 AND status = ?5"#,
            )
            .bind(WorkItemStatus::Failed.to_string())
            .bind(scheduled_at.to_rfc3339())
            .bind("startup_requeue_exhausted: generation 1 already consumed")
            .bind(&item_id)
            .bind(WorkItemStatus::Running.to_string())
            .execute(&mut **tx)
            .await
            .context("fail running InvokeAgent work item on startup requeue exhaustion")?;
            continue;
        }
        if let Some(object) = payload.as_object_mut() {
            object.remove("p058_claimed");
            object.insert(
                "p061_startup_recovery".to_string(),
                serde_json::json!({
                    "requeued_at": scheduled_at_rfc3339.clone(),
                    "reason": reason,
                    "startup_repair_id": repair_id,
                    "source_command_journal_id": source_command_journal_id,
                    "source_work_item_id": item_id,
                    "requeue_generation": 1,
                    "max_requeue_generation": 1,
                }),
            );
        }
        let updated = sqlx::query(
            r#"UPDATE work_items
               SET status = ?1,
                   payload_json = ?2,
                   started_at = NULL,
                   completed_at = NULL,
                   failed_at = NULL,
                   last_error = ?3
               WHERE id = ?4 AND status = ?5"#,
        )
        .bind(&pending)
        .bind(serde_json::to_string(&payload)?)
        .bind(reason)
        .bind(&item_id)
        .bind(&running)
        .execute(&mut **tx)
        .await
        .context("requeue running InvokeAgent work item on startup")?
        .rows_affected();
        if updated > 0 {
            crate::metrics::increment_counter_with_label(
                "p082_recovery_idempotency_replay_total",
                "P082-R01:startup_requeue_once",
            );
        }
        requeued += updated;
    }
    Ok(requeued)
}

pub async fn complete_running_invoke_agents_with_terminal_valid_outputs_on_startup(
    pool: &SqlitePool,
    _reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"
        SELECT wi.id
        FROM work_items wi
        JOIN agent_executions ae
          ON ae.id = json_extract(wi.payload_json, '$.p058_claimed.agent_execution_id')
         AND ae.status = ?1
        JOIN agent_execution_runtime_facts facts
          ON facts.agent_execution_id = ae.id
         AND facts.output_settlement IN (?2, ?3)
         AND COALESCE(facts.valid_required_outputs, 0) > 0
        WHERE wi.kind = ?4
          AND wi.status = ?5
          AND wi.id NOT LIKE 'auto-contract-output-retry:%'
          AND json_type(wi.payload_json, '$.retry_authority_id') IS NULL
          AND json_type(wi.payload_json, '$.targeted_retry') IS NULL
          AND json_extract(wi.payload_json, '$.p058_claimed.agent_execution_id') IS NOT NULL
        ORDER BY wi.scheduled_at ASC, wi.rowid ASC
        "#,
    )
    .bind(AgentStatus::Completed.to_string())
    .bind("valid_outputs_from_completed_execution")
    .bind("valid_outputs_from_failed_execution")
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_all(pool)
    .await
    .context("select running InvokeAgent work items with terminal valid agent outputs")?;

    let mut completed = 0_u64;
    for row in rows {
        let item_id: String = row.get("id");
        complete(pool, &item_id)
            .await
            .with_context(|| format!("complete recovered InvokeAgent work item {item_id}"))?;
        completed += 1;
    }

    Ok(completed)
}

pub async fn requeue_stale_starting_invoke_agent_sessions(
    pool: &SqlitePool,
    scheduled_at: DateTime<Utc>,
    standard_stale_cutoff: DateTime<Utc>,
    xcode_stale_cutoff: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_stale_starting_invoke_agent_sessions",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_stale_starting_invoke_agent_sessions",
        ),
        "work_items.requeue_stale_starting_invoke_agent_sessions",
    )
    .await?;
    let requeued = requeue_stale_starting_invoke_agent_sessions_tx(
        &mut tx,
        scheduled_at,
        standard_stale_cutoff,
        xcode_stale_cutoff,
        reason,
    )
    .await?;
    tx.commit()
        .await
        .context("commit stale ACP startup InvokeAgent requeue")?;
    log_write_transaction(
        "work_items.requeue_stale_starting_invoke_agent_sessions",
        tx_started,
    );
    Ok(requeued)
}

pub async fn requeue_stale_pre_session_invoke_agents(
    pool: &SqlitePool,
    scheduled_at: DateTime<Utc>,
    standard_stale_cutoff: DateTime<Utc>,
    xcode_stale_cutoff: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_stale_pre_session_invoke_agents",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_stale_pre_session_invoke_agents",
        ),
        "work_items.requeue_stale_pre_session_invoke_agents",
    )
    .await?;
    let requeued = requeue_stale_pre_session_invoke_agents_tx(
        &mut tx,
        scheduled_at,
        standard_stale_cutoff,
        xcode_stale_cutoff,
        reason,
    )
    .await?;
    tx.commit()
        .await
        .context("commit stale pre-session InvokeAgent requeue")?;
    log_write_transaction(
        "work_items.requeue_stale_pre_session_invoke_agents",
        tx_started,
    );
    Ok(requeued)
}

pub async fn requeue_stale_pre_session_invoke_agents_tx(
    tx: &mut Transaction<'_, Sqlite>,
    scheduled_at: DateTime<Utc>,
    standard_stale_cutoff: DateTime<Utc>,
    xcode_stale_cutoff: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT wi.id,
                  wi.payload_json,
                  COALESCE(wi.started_at, wi.scheduled_at) AS work_started_at
           FROM work_items wi
           INNER JOIN agent_executions ae
             ON ae.id = json_extract(wi.payload_json, '$.p058_claimed.agent_execution_id')
           WHERE wi.kind = ?1
             AND wi.status = ?2
             AND ae.status = ?3
             AND ae.session_generation_id IS NULL
           ORDER BY wi.scheduled_at ASC, wi.rowid ASC"#,
    )
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .bind(AgentStatus::Running.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("load stale pre-session InvokeAgent work")?;

    let pending = WorkItemStatus::Pending.to_string();
    let running = WorkItemStatus::Running.to_string();
    let scheduled_at_rfc3339 = scheduled_at.to_rfc3339();
    let standard_stale_cutoff_rfc3339 = standard_stale_cutoff.to_rfc3339();
    let xcode_stale_cutoff_rfc3339 = xcode_stale_cutoff.to_rfc3339();
    let mut requeued = 0_u64;

    for row in rows {
        let item_id: String = row.get("id");
        let payload_json: String = row.get("payload_json");
        let mut payload = serde_json::from_str::<serde_json::Value>(&payload_json)
            .unwrap_or_else(|_| serde_json::json!({}));
        let xcode_required = payload
            .get("xcode_broker_required")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
            || payload
                .get("requested_mcp_server_ids")
                .and_then(|value| value.as_array())
                .map(|servers| {
                    servers
                        .iter()
                        .any(|server| server.as_str() == Some("xcode"))
                })
                .unwrap_or(false);
        let stale_cutoff = if xcode_required {
            xcode_stale_cutoff
        } else {
            standard_stale_cutoff
        };
        let work_started_at = row
            .get::<Option<String>, _>("work_started_at")
            .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
            .map(|value| value.with_timezone(&Utc));
        if work_started_at.is_none_or(|started| started > stale_cutoff) {
            continue;
        }

        supersede_abandoned_preclaim_for_retry_tx(tx, &item_id, &payload, scheduled_at).await?;
        if let Some(object) = payload.as_object_mut() {
            object.remove("p058_claimed");
            object.insert(
                "p061_startup_recovery".to_string(),
                serde_json::json!({
                    "requeued_at": scheduled_at_rfc3339.clone(),
                    "reason": reason,
                    "stale_cutoff": stale_cutoff.to_rfc3339(),
                    "xcode_grace_applied": xcode_required,
                    "standard_stale_cutoff": standard_stale_cutoff_rfc3339,
                    "xcode_stale_cutoff": xcode_stale_cutoff_rfc3339,
                }),
            );
        }
        let updated = sqlx::query(
            r#"UPDATE work_items
               SET status = ?1,
                   payload_json = ?2,
                   started_at = NULL,
                   completed_at = NULL,
                   failed_at = NULL,
                   last_error = ?3
               WHERE id = ?4 AND status = ?5"#,
        )
        .bind(&pending)
        .bind(serde_json::to_string(&payload)?)
        .bind(reason)
        .bind(&item_id)
        .bind(&running)
        .execute(&mut **tx)
        .await
        .context("requeue stale pre-session InvokeAgent work item")?
        .rows_affected();
        requeued += updated;
    }
    Ok(requeued)
}

pub async fn requeue_stale_starting_invoke_agent_sessions_tx(
    tx: &mut Transaction<'_, Sqlite>,
    scheduled_at: DateTime<Utc>,
    standard_stale_cutoff: DateTime<Utc>,
    xcode_stale_cutoff: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT wi.id,
                  wi.payload_json,
                  COALESCE(wi.started_at, wi.scheduled_at) AS work_started_at,
                  ae.session_generation_id AS session_generation_id,
                  sg.lineage_id AS session_lineage_id,
                  sg.created_at AS generation_created_at
           FROM work_items wi
           INNER JOIN agent_executions ae
             ON ae.id = json_extract(wi.payload_json, '$.p058_claimed.agent_execution_id')
           INNER JOIN session_generations sg
             ON sg.id = ae.session_generation_id
           WHERE wi.kind = ?1
             AND wi.status = ?2
             AND ae.status = ?3
             AND sg.status = 'active'
             AND sg.provider_session_id IS NULL
             AND sg.last_activity_at IS NULL
           ORDER BY wi.scheduled_at ASC, wi.rowid ASC"#,
    )
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .bind(AgentStatus::Running.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("load stale starting InvokeAgent sessions")?;

    let pending = WorkItemStatus::Pending.to_string();
    let running = WorkItemStatus::Running.to_string();
    let scheduled_at_rfc3339 = scheduled_at.to_rfc3339();
    let mut requeued = 0_u64;

    for row in rows {
        let item_id: String = row.get("id");
        let payload_json: String = row.get("payload_json");
        let mut payload = serde_json::from_str::<serde_json::Value>(&payload_json)
            .unwrap_or_else(|_| serde_json::json!({}));
        let xcode_required = payload
            .get("xcode_broker_required")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
            || payload
                .get("requested_mcp_server_ids")
                .and_then(|value| value.as_array())
                .map(|servers| {
                    servers
                        .iter()
                        .any(|server| server.as_str() == Some("xcode"))
                })
                .unwrap_or(false);
        let stale_cutoff = if xcode_required {
            xcode_stale_cutoff
        } else {
            standard_stale_cutoff
        };
        let stale_cutoff_rfc3339 = stale_cutoff.to_rfc3339();
        let work_started_at = row
            .get::<Option<String>, _>("work_started_at")
            .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
            .map(|value| value.with_timezone(&Utc));
        let generation_created_at = row
            .get::<Option<String>, _>("generation_created_at")
            .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
            .map(|value| value.with_timezone(&Utc));
        if work_started_at.is_none_or(|started| started > stale_cutoff)
            || generation_created_at.is_none_or(|created| created > stale_cutoff)
        {
            continue;
        }

        supersede_abandoned_preclaim_for_retry_tx(tx, &item_id, &payload, scheduled_at).await?;

        let session_generation_id: String = row.get("session_generation_id");
        let session_lineage_id: String = row.get("session_lineage_id");
        sqlx::query(
            r#"UPDATE session_generations
               SET status = 'invalidated',
                   ended_at = ?1,
                   end_reason = ?2
               WHERE id = ?3
                 AND status = 'active'
                 AND provider_session_id IS NULL"#,
        )
        .bind(&scheduled_at_rfc3339)
        .bind("stale_acp_startup_without_provider_session")
        .bind(&session_generation_id)
        .execute(&mut **tx)
        .await
        .context("invalidate stale ACP startup session generation")?;
        sqlx::query(
            r#"INSERT INTO session_events
               (id, lineage_id, generation_id, event_type, recorded_at, details_json)
               VALUES (?1, ?2, ?3, 'invalidated', ?4, ?5)"#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_lineage_id)
        .bind(&session_generation_id)
        .bind(&scheduled_at_rfc3339)
        .bind(
            serde_json::json!({
                "reason": "stale_acp_startup_without_provider_session",
                "stale_cutoff": stale_cutoff_rfc3339,
                "source_work_item_id": item_id,
            })
            .to_string(),
        )
        .execute(&mut **tx)
        .await
        .context("insert stale ACP startup session invalidation event")?;

        if let Some(object) = payload.as_object_mut() {
            object.remove("p058_claimed");
            object.insert(
                "p061_startup_recovery".to_string(),
                serde_json::json!({
                    "requeued_at": scheduled_at_rfc3339.clone(),
                    "reason": reason,
                    "stale_cutoff": stale_cutoff_rfc3339,
                }),
            );
        }
        let updated = sqlx::query(
            r#"UPDATE work_items
               SET status = ?1,
                   payload_json = ?2,
                   started_at = NULL,
                   completed_at = NULL,
                   failed_at = NULL,
                   last_error = ?3
               WHERE id = ?4 AND status = ?5"#,
        )
        .bind(&pending)
        .bind(serde_json::to_string(&payload)?)
        .bind(reason)
        .bind(&item_id)
        .bind(&running)
        .execute(&mut **tx)
        .await
        .context("requeue stale starting InvokeAgent work item")?
        .rows_affected();
        requeued += updated;
    }
    Ok(requeued)
}

pub async fn cancel_pending_or_running_advance_by_run(
    pool: &SqlitePool,
    run_id: RunId,
    completed_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.cancel_advance_for_parked_cursor",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.cancel_advance_for_parked_cursor",
        ),
        "work_items.cancel_advance_for_parked_cursor",
    )
    .await?;
    let cancelled =
        cancel_pending_or_running_advance_by_run_tx(&mut tx, run_id, completed_at, reason).await?;
    tx.commit()
        .await
        .context("commit cancel pending/running AdvanceRun work items by run")?;
    log_write_transaction("work_items.cancel_advance_for_parked_cursor", tx_started);
    Ok(cancelled)
}

pub async fn cancel_pending_or_running_advance_by_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    completed_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT id, payload_json
           FROM work_items
           WHERE run_id = ?1
             AND kind = ?2
             AND status IN (?3, ?4)"#,
    )
    .bind(run_id.to_string())
    .bind(WorkItemKind::AdvanceRun.to_string())
    .bind(WorkItemStatus::Pending.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("select pending/running AdvanceRun work items by run")?;
    let mut cancelled = 0;
    for row in rows {
        let id: String = row.get("id");
        let payload: String = row.get("payload_json");
        match classify_advance_payload_scope(&payload) {
            AdvancePayloadScope::LegacyRunScoped => {
                let result = sqlx::query(
                    r#"UPDATE work_items
                       SET status = ?1, completed_at = ?2, last_error = ?3
                       WHERE id = ?4
                         AND status IN (?5, ?6)"#,
                )
                .bind(WorkItemStatus::Cancelled.to_string())
                .bind(completed_at.to_rfc3339())
                .bind(reason)
                .bind(id)
                .bind(WorkItemStatus::Pending.to_string())
                .bind(WorkItemStatus::Running.to_string())
                .execute(&mut **tx)
                .await
                .context("cancel run-scoped AdvanceRun work item")?;
                cancelled += result.rows_affected();
            }
            AdvancePayloadScope::Targeted => {}
            AdvancePayloadScope::Malformed(code) => {
                quarantine_advance_work_item_tx(tx, &id, code).await?;
            }
        }
    }
    Ok(cancelled)
}

pub async fn cancel_pending_or_running_advance_by_retry_authority_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    retry_authority_id: &str,
    target_stage_execution_id: StageExecutionId,
    completed_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT id, payload_json
           FROM work_items
           WHERE run_id = ?1
             AND kind = ?2
             AND status IN (?3, ?4)"#,
    )
    .bind(run_id.to_string())
    .bind(WorkItemKind::AdvanceRun.to_string())
    .bind(WorkItemStatus::Pending.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("select pending/running targeted AdvanceRun work items")?;

    let mut cancelled = 0;
    for row in rows {
        let id: String = row.get("id");
        let payload: String = row.get("payload_json");
        match advance_payload_matches_authority(
            &payload,
            retry_authority_id,
            target_stage_execution_id,
        ) {
            AdvancePayloadAuthorityMatch::Match => {}
            AdvancePayloadAuthorityMatch::NoMatch => continue,
            AdvancePayloadAuthorityMatch::Malformed(code) => {
                quarantine_advance_work_item_tx(tx, &id, code).await?;
                continue;
            }
        }
        let result = sqlx::query(
            r#"UPDATE work_items
               SET status = ?1, completed_at = ?2, last_error = ?3
               WHERE id = ?4
                 AND status IN (?5, ?6)"#,
        )
        .bind(WorkItemStatus::Cancelled.to_string())
        .bind(completed_at.to_rfc3339())
        .bind(reason)
        .bind(id)
        .bind(WorkItemStatus::Pending.to_string())
        .bind(WorkItemStatus::Running.to_string())
        .execute(&mut **tx)
        .await
        .context("cancel targeted AdvanceRun work item")?;
        cancelled += result.rows_affected();
    }
    Ok(cancelled)
}

pub async fn requeue_running_invoke_agent_by_stage_for_host_interruption(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    stage_execution_id: StageExecutionId,
    scheduled_at: DateTime<Utc>,
) -> Result<Vec<String>> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_host_interruption_invoke",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_host_interruption_invoke",
        ),
        "work_items.requeue_host_interruption_invoke",
    )
    .await?;
    let requeued = requeue_running_invoke_agent_by_stage_for_host_interruption_tx(
        &mut tx,
        run_id,
        stage_id,
        stage_execution_id,
        scheduled_at,
    )
    .await?;
    tx.commit()
        .await
        .context("commit host interruption InvokeAgent requeue")?;
    log_write_transaction("work_items.requeue_host_interruption_invoke", tx_started);
    Ok(requeued)
}

pub async fn requeue_running_invoke_agent_by_stage_for_host_interruption_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_id: &str,
    stage_execution_id: StageExecutionId,
    scheduled_at: DateTime<Utc>,
) -> Result<Vec<String>> {
    let run_id = run_id.to_string();
    let stage_execution_id = stage_execution_id.to_string();
    let running = WorkItemStatus::Running.to_string();
    let invoke_agent = WorkItemKind::InvokeAgent.to_string();
    let rows = sqlx::query(
        r#"SELECT id, payload_json
           FROM work_items
           WHERE run_id = ?1 AND stage_id = ?2 AND kind = ?3 AND status = ?4
           ORDER BY scheduled_at ASC, rowid ASC"#,
    )
    .bind(&run_id)
    .bind(stage_id)
    .bind(&invoke_agent)
    .bind(&running)
    .fetch_all(&mut **tx)
    .await
    .context("load running InvokeAgent work items for host interruption requeue")?;

    let pending = WorkItemStatus::Pending.to_string();
    let scheduled_at_rfc3339 = scheduled_at.to_rfc3339();
    let mut requeued = Vec::new();

    for row in rows {
        let item_id: String = row.get("id");
        let payload_json: String = row.get("payload_json");
        let mut payload = match serde_json::from_str::<serde_json::Value>(&payload_json) {
            Ok(payload) => payload,
            Err(_) => continue,
        };
        if payload
            .get("stage_execution_id")
            .and_then(|value| value.as_str())
            != Some(stage_execution_id.as_str())
        {
            continue;
        }
        supersede_abandoned_preclaim_for_retry_tx(tx, &item_id, &payload, scheduled_at).await?;
        if let Some(object) = payload.as_object_mut() {
            object.remove("p058_claimed");
            object.insert(
                "host_interruption_retry".to_string(),
                serde_json::json!({
                    "stage_execution_id": stage_execution_id.clone(),
                    "scheduled_at": scheduled_at_rfc3339.clone(),
                }),
            );
        }
        let updated = sqlx::query(
            r#"UPDATE work_items
               SET status = ?1,
                   payload_json = ?2,
                   scheduled_at = ?3,
                   started_at = NULL,
                   completed_at = NULL,
                   failed_at = NULL,
                   last_error = NULL
               WHERE id = ?4 AND status = ?5"#,
        )
        .bind(&pending)
        .bind(serde_json::to_string(&payload)?)
        .bind(&scheduled_at_rfc3339)
        .bind(&item_id)
        .bind(&running)
        .execute(&mut **tx)
        .await
        .context("requeue running InvokeAgent work item for host interruption")?
        .rows_affected();
        if updated == 1 {
            requeued.push(item_id);
        }
    }

    Ok(requeued)
}

pub async fn requeue_running_invoke_agent_after_active_prompt_close(
    pool: &SqlitePool,
    work_item_id: &str,
    claim_key: &ArtifactSourceGenerationClaimKey,
    failed_session_generation_id: Option<&str>,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<bool> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_active_prompt_closed_invoke",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_active_prompt_closed_invoke",
        ),
        "work_items.requeue_active_prompt_closed_invoke",
    )
    .await?;
    let requeued = requeue_running_invoke_agent_after_active_prompt_close_tx(
        &mut tx,
        work_item_id,
        claim_key,
        failed_session_generation_id,
        scheduled_at,
        reason,
    )
    .await?;
    tx.commit()
        .await
        .context("commit active prompt close InvokeAgent requeue")?;
    log_write_transaction("work_items.requeue_active_prompt_closed_invoke", tx_started);
    Ok(requeued)
}

pub async fn requeue_running_invoke_agent_after_active_prompt_close_tx(
    tx: &mut Transaction<'_, Sqlite>,
    work_item_id: &str,
    claim_key: &ArtifactSourceGenerationClaimKey,
    failed_session_generation_id: Option<&str>,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<bool> {
    let row = sqlx::query(
        r#"SELECT payload_json
           FROM work_items
           WHERE id = ?1 AND kind = ?2 AND status = ?3"#,
    )
    .bind(work_item_id)
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_optional(&mut **tx)
    .await
    .context("load running InvokeAgent work item for active prompt close requeue")?;

    let Some(row) = row else {
        return Ok(false);
    };
    let payload_json: String = row.get("payload_json");
    let mut payload: serde_json::Value =
        serde_json::from_str(&payload_json).context("parse InvokeAgent payload for requeue")?;
    if let Some(object) = payload.as_object_mut() {
        object.remove("p058_claimed");
        object.insert(
            "acp_active_prompt_recovery".to_string(),
            serde_json::json!({
                "reason": reason,
                "failed_agent_execution_id": claim_key.agent_execution_id.to_string(),
                "failed_session_generation_id": failed_session_generation_id,
                "requeued_at": scheduled_at.to_rfc3339(),
            }),
        );
    }

    let scheduled_at_rfc3339 = scheduled_at.to_rfc3339();
    let updated = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1,
               payload_json = ?2,
               scheduled_at = ?3,
               started_at = NULL,
               completed_at = NULL,
               failed_at = NULL,
               last_error = NULL
           WHERE id = ?4 AND status = ?5"#,
    )
    .bind(WorkItemStatus::Pending.to_string())
    .bind(serde_json::to_string(&payload)?)
    .bind(&scheduled_at_rfc3339)
    .bind(work_item_id)
    .bind(WorkItemStatus::Running.to_string())
    .execute(&mut **tx)
    .await
    .context("requeue running InvokeAgent work item after active prompt close")?
    .rows_affected();

    if updated != 1 {
        return Ok(false);
    }

    let now = scheduled_at_rfc3339;
    sqlx::query(
        r#"UPDATE artifact_source_generation_claims
           SET claim_state = ?1,
               superseding_work_item_id = ?2,
               superseded_at = ?3,
               updated_at = ?3
           WHERE run_id = ?4 AND owner_kind = ?5 AND owner_id = ?6 AND agent_execution_id = ?7
             AND source_work_item_id = ?8 AND claim_state = ?9"#,
    )
    .bind(ArtifactSourceClaimState::SupersededPendingRetry.to_string())
    .bind(work_item_id)
    .bind(&now)
    .bind(claim_key.run_id.to_string())
    .bind(claim_key.owner_kind.to_string())
    .bind(&claim_key.owner_id)
    .bind(claim_key.agent_execution_id.to_string())
    .bind(&claim_key.source_work_item_id)
    .bind(ArtifactSourceClaimState::Active.to_string())
    .execute(&mut **tx)
    .await
    .context("mark active source-generation claim superseded for active prompt close retry")?;

    Ok(true)
}

pub async fn requeue_running_invoke_agent_after_provider_capacity_wait(
    pool: &SqlitePool,
    work_item_id: &str,
    claim_key: &ArtifactSourceGenerationClaimKey,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<bool> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_provider_capacity_invoke",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_provider_capacity_invoke",
        ),
        "work_items.requeue_provider_capacity_invoke",
    )
    .await?;
    let requeued = requeue_running_invoke_agent_after_provider_capacity_wait_tx(
        &mut tx,
        work_item_id,
        claim_key,
        scheduled_at,
        reason,
    )
    .await?;
    tx.commit()
        .await
        .context("commit provider capacity InvokeAgent requeue")?;
    log_write_transaction("work_items.requeue_provider_capacity_invoke", tx_started);
    Ok(requeued)
}

pub async fn requeue_running_invoke_agent_after_provider_capacity_wait_tx(
    tx: &mut Transaction<'_, Sqlite>,
    work_item_id: &str,
    claim_key: &ArtifactSourceGenerationClaimKey,
    scheduled_at: DateTime<Utc>,
    reason: &str,
) -> Result<bool> {
    let row = sqlx::query(
        r#"SELECT payload_json
           FROM work_items
           WHERE id = ?1 AND kind = ?2 AND status = ?3"#,
    )
    .bind(work_item_id)
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_optional(&mut **tx)
    .await
    .context("load running InvokeAgent work item for provider capacity requeue")?;

    let Some(row) = row else {
        return Ok(false);
    };
    let payload_json: String = row.get("payload_json");
    let mut payload: serde_json::Value = serde_json::from_str(&payload_json)
        .context("parse InvokeAgent payload for provider capacity requeue")?;
    if let Some(object) = payload.as_object_mut() {
        object.remove("p058_claimed");
        object.insert(
            "acp_provider_capacity_recovery".to_string(),
            serde_json::json!({
                "reason": reason,
                "failed_agent_execution_id": claim_key.agent_execution_id.to_string(),
                "requeued_at": scheduled_at.to_rfc3339(),
            }),
        );
    }

    let scheduled_at_rfc3339 = scheduled_at.to_rfc3339();
    let updated = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1,
               payload_json = ?2,
               scheduled_at = ?3,
               started_at = NULL,
               completed_at = NULL,
               failed_at = NULL,
               last_error = ?4
           WHERE id = ?5 AND status = ?6"#,
    )
    .bind(WorkItemStatus::Pending.to_string())
    .bind(serde_json::to_string(&payload)?)
    .bind(&scheduled_at_rfc3339)
    .bind(reason)
    .bind(work_item_id)
    .bind(WorkItemStatus::Running.to_string())
    .execute(&mut **tx)
    .await
    .context("requeue running InvokeAgent work item after provider capacity wait")?
    .rows_affected();

    if updated != 1 {
        return Ok(false);
    }

    let now = scheduled_at_rfc3339;
    sqlx::query(
        r#"UPDATE artifact_source_generation_claims
           SET claim_state = ?1,
               superseding_work_item_id = ?2,
               superseded_at = ?3,
               updated_at = ?3
           WHERE run_id = ?4 AND owner_kind = ?5 AND owner_id = ?6 AND agent_execution_id = ?7
             AND source_work_item_id = ?8 AND claim_state = ?9"#,
    )
    .bind(ArtifactSourceClaimState::SupersededPendingRetry.to_string())
    .bind(work_item_id)
    .bind(&now)
    .bind(claim_key.run_id.to_string())
    .bind(claim_key.owner_kind.to_string())
    .bind(&claim_key.owner_id)
    .bind(claim_key.agent_execution_id.to_string())
    .bind(&claim_key.source_work_item_id)
    .bind(ArtifactSourceClaimState::Active.to_string())
    .execute(&mut **tx)
    .await
    .context("mark active source-generation claim superseded for provider capacity retry")?;

    Ok(true)
}

pub async fn complete(
    pool: &SqlitePool,
    id: &str,
) -> Result<scheduler::RefreshQueueSummariesResult> {
    let capacity = InvokeAgentCapacityConfig::default();
    complete_with_capacity(pool, id, &capacity).await
}

pub async fn complete_with_capacity(
    pool: &SqlitePool,
    id: &str,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<scheduler::RefreshQueueSummariesResult> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.complete",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.complete",
        ),
        "work_items.complete",
    )
    .await?;
    let now = Utc::now().to_rfc3339();
    let status = WorkItemStatus::Completed.to_string();
    let mut refresh_scheduler = false;
    let existing =
        sqlx::query(r#"SELECT kind, run_id, status, payload_json FROM work_items WHERE id = ?1"#)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await
            .context("select work item before complete")?;
    if let Some(row) = existing {
        let kind: String = row.get("kind");
        let run_id: Option<String> = row.get("run_id");
        let previous_status: String = row.get("status");
        if !matches!(previous_status.as_str(), "pending" | "running") {
            tx.commit()
                .await
                .context("commit terminal complete no-op")?;
            log_write_transaction("work_items.complete.terminal_noop", tx_started);
            return Ok(scheduler::RefreshQueueSummariesResult::default());
        }
        sqlx::query(r#"UPDATE work_items SET status = ?1, completed_at = ?2 WHERE id = ?3 AND status IN (?4, ?5)"#)
            .bind(&status)
            .bind(&now)
            .bind(id)
            .bind(WorkItemStatus::Pending.to_string())
            .bind(WorkItemStatus::Running.to_string())
            .execute(&mut **tx)
            .await
            .context("complete work item")?;
        if kind == WorkItemKind::InvokeAgent.to_string()
            && previous_status == WorkItemStatus::Running.to_string()
        {
            refresh_scheduler = true;
            let payload_json: String = row.get("payload_json");
            if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&payload_json) {
                if let Some(agent_execution_id) = payload
                    .pointer("/p058_claimed/agent_execution_id")
                    .and_then(|value| value.as_str())
                {
                    sqlx::query(
                        r#"UPDATE agent_executions
                           SET status = ?1, completed_at = COALESCE(completed_at, ?2)
                           WHERE id = ?3 AND status = ?4"#,
                    )
                    .bind(AgentStatus::Completed.to_string())
                    .bind(&now)
                    .bind(agent_execution_id)
                    .bind(AgentStatus::Running.to_string())
                    .execute(&mut **tx)
                    .await
                    .context("settle preclaimed InvokeAgent execution on work item complete")?;
                    sqlx::query(
                        r#"UPDATE artifact_source_generation_claims
                           SET claim_state = ?1,
                               closed_at = COALESCE(closed_at, ?2),
                               updated_at = ?2
                           WHERE source_work_item_id = ?3
                             AND agent_execution_id = ?4
                             AND claim_state = ?5"#,
                    )
                    .bind("closed")
                    .bind(&now)
                    .bind(id)
                    .bind(agent_execution_id)
                    .bind("active")
                    .execute(&mut **tx)
                    .await
                    .context("close active source-generation claim on work item complete")?;
                }
            }
            if let Some(run_id) = run_id {
                let advance_kind = WorkItemKind::AdvanceRun.to_string();
                let pending_status = WorkItemStatus::Pending.to_string();
                let advance_id = format!("advance-after-invoke:{id}");
                let (payload_json, target_stage_id) = build_post_invoke_advance_payload_tx(
                    &mut tx,
                    &run_id,
                    "invoke_agent_completed",
                    "completed_invoke_work_item_id",
                    id,
                    &payload_json,
                )
                .await?;
                sqlx::query(
                    r#"
                    INSERT OR IGNORE INTO work_items
                      (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0, NULL)
                    "#,
                )
                .bind(advance_id)
                .bind(advance_kind)
                .bind(payload_json)
                .bind(pending_status)
                .bind(run_id)
                .bind(target_stage_id)
                .bind(&now)
                .execute(&mut **tx)
                .await
                .context("enqueue post-completion AdvanceRun for InvokeAgent")?;
            }
        } else if kind == WorkItemKind::InvokeAgent.to_string()
            && previous_status == WorkItemStatus::Pending.to_string()
        {
            refresh_scheduler = true;
        }
    }
    let refresh = if refresh_scheduler {
        scheduler::refresh_queue_summaries_for_notification_tx(
            &mut tx,
            capacity,
            Utc::now(),
            "work_items.complete",
            0,
        )
        .await?
    } else {
        scheduler::RefreshQueueSummariesResult::default()
    };
    tx.commit().await.context("commit complete work item")?;
    log_write_transaction("work_items.complete", tx_started);
    Ok(refresh)
}

pub async fn fail(
    pool: &SqlitePool,
    id: &str,
    error: &str,
) -> Result<scheduler::RefreshQueueSummariesResult> {
    let capacity = InvokeAgentCapacityConfig::default();
    fail_with_capacity(pool, id, error, &capacity).await
}

pub async fn fail_with_capacity(
    pool: &SqlitePool,
    id: &str,
    error: &str,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<scheduler::RefreshQueueSummariesResult> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.fail",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.fail",
        ),
        "work_items.fail",
    )
    .await?;
    let now = Utc::now().to_rfc3339();
    let status = WorkItemStatus::Failed.to_string();
    let existing =
        sqlx::query(r#"SELECT kind, run_id, status, payload_json FROM work_items WHERE id = ?1"#)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await
            .context("select work item before fail")?;
    let refresh = if let Some(row) = existing {
        let kind: String = row.get("kind");
        let run_id: Option<String> = row.get("run_id");
        let previous_status: String = row.get("status");
        if !matches!(previous_status.as_str(), "pending" | "running") {
            tx.commit().await.context("commit terminal fail no-op")?;
            log_write_transaction("work_items.fail.terminal_noop", tx_started);
            return Ok(scheduler::RefreshQueueSummariesResult::default());
        }
        sqlx::query(
            r#"UPDATE work_items
               SET status = ?1, failed_at = ?2, last_error = ?3
               WHERE id = ?4 AND status IN (?5, ?6)"#,
        )
        .bind(&status)
        .bind(&now)
        .bind(error)
        .bind(id)
        .bind(WorkItemStatus::Pending.to_string())
        .bind(WorkItemStatus::Running.to_string())
        .execute(&mut **tx)
        .await
        .context("fail work item")?;
        if kind == WorkItemKind::InvokeAgent.to_string()
            && matches!(previous_status.as_str(), "pending" | "running")
        {
            if let Some(run_id) = run_id {
                let advance_kind = WorkItemKind::AdvanceRun.to_string();
                let pending_status = WorkItemStatus::Pending.to_string();
                let advance_id = format!("advance-after-invoke:{id}");
                let source_payload_json: String = row.get("payload_json");
                let (payload_json, target_stage_id) = build_post_invoke_advance_payload_tx(
                    &mut tx,
                    &run_id,
                    "invoke_agent_failed",
                    "failed_invoke_work_item_id",
                    id,
                    &source_payload_json,
                )
                .await?;
                sqlx::query(
                    r#"
                    INSERT OR IGNORE INTO work_items
                      (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0, NULL)
                    "#,
                )
                .bind(advance_id)
                .bind(advance_kind)
                .bind(payload_json)
                .bind(pending_status)
                .bind(run_id)
                .bind(target_stage_id)
                .bind(&now)
                .execute(&mut **tx)
                .await
                .context("enqueue post-failure AdvanceRun for InvokeAgent")?;
            }
            scheduler::refresh_queue_summaries_for_notification_tx(
                &mut tx,
                capacity,
                Utc::now(),
                "work_items.fail",
                0,
            )
            .await?
        } else {
            scheduler::RefreshQueueSummariesResult::default()
        }
    } else {
        scheduler::RefreshQueueSummariesResult::default()
    };
    tx.commit().await.context("commit fail work item")?;
    log_write_transaction("work_items.fail", tx_started);
    Ok(refresh)
}

async fn build_post_invoke_advance_payload_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    reason: &str,
    terminal_work_item_field: &str,
    invoke_work_item_id: &str,
    source_payload_json: &str,
) -> Result<(String, Option<String>)> {
    let source_payload = serde_json::from_str::<serde_json::Value>(source_payload_json)
        .unwrap_or_else(|_| serde_json::json!({}));
    let enqueue_reason = match reason {
        "invoke_agent_completed" => "post_invoke_completion",
        "invoke_agent_failed" => "post_invoke_failure",
        other => other,
    };
    let mut payload = serde_json::json!({
        "schema_version": "advance_run_payload.v1",
        "run_id": run_id,
        "reason": reason,
        "enqueue_reason": enqueue_reason,
        terminal_work_item_field: invoke_work_item_id,
    });

    let explicit_target_stage_execution_id = source_payload
        .get("target_stage_execution_id")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let source_agent_execution_id = source_payload
        .get("source_agent_execution_id")
        .and_then(|value| value.as_str())
        .or_else(|| {
            source_payload
                .pointer("/targeted_retry/source_agent_execution_id")
                .and_then(|value| value.as_str())
        })
        .or_else(|| {
            source_payload
                .pointer("/p058_claimed/agent_execution_id")
                .and_then(|value| value.as_str())
        })
        .map(str::to_owned);
    let agent_execution_stage_id =
        if let Some(agent_execution_id) = source_agent_execution_id.as_deref() {
            find_agent_execution_stage_id_tx(tx, agent_execution_id).await?
        } else {
            None
        };
    let source_stage_execution_id = explicit_target_stage_execution_id
        .clone()
        .or_else(|| {
            source_payload
                .get("stage_execution_id")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        })
        .or_else(|| agent_execution_stage_id.map(|id| id.to_string()));
    let source_retry_authority_id = source_payload
        .get("retry_authority_id")
        .and_then(|value| value.as_str())
        .or_else(|| {
            source_payload
                .pointer("/targeted_retry/retry_authority_id")
                .and_then(|value| value.as_str())
        })
        .map(str::to_owned);
    let has_targeted_hint = explicit_target_stage_execution_id.is_some()
        || source_retry_authority_id.is_some()
        || source_payload.get("targeted_retry").is_some();
    if has_targeted_hint && source_stage_execution_id.is_none() {
        anyhow::bail!(
            "advance_run_source_target_mismatch: targeted InvokeAgent {invoke_work_item_id} has no durable source stage"
        );
    }

    let active_by_target = if let Some(target_id) = source_stage_execution_id.as_deref() {
        find_active_retry_authority_by_target_tx(tx, run_id, target_id).await?
    } else {
        None
    };
    let active_by_id = if let Some(authority_id) = source_retry_authority_id.as_deref() {
        find_active_retry_authority_by_id_tx(tx, run_id, authority_id).await?
    } else {
        None
    };
    let mut active_authority = match (active_by_target, active_by_id) {
        (Some(target_row), Some(id_row)) => {
            let target_authority_id: String = target_row.get("id");
            let source_authority_id: String = id_row.get("id");
            if target_authority_id != source_authority_id {
                anyhow::bail!(
                    "advance_run_source_authority_mismatch: source invoke {invoke_work_item_id} target resolves {target_authority_id}, source resolves {source_authority_id}"
                );
            }
            Some(target_row)
        }
        (Some(row), None) | (None, Some(row)) => Some(row),
        (None, None) => None,
    };
    if has_targeted_hint && active_authority.is_none() {
        anyhow::bail!(
            "advance_run_source_authority_mismatch: targeted InvokeAgent {invoke_work_item_id} has no active retry authority"
        );
    }
    if let Some(active) = active_authority.as_ref() {
        let source_invoke_work_item_id: Option<String> = active.get("source_invoke_work_item_id");
        if let Some(source_invoke_work_item_id) = source_invoke_work_item_id {
            if source_invoke_work_item_id != invoke_work_item_id {
                if has_targeted_hint {
                    anyhow::bail!(
                        "advance_run_source_authority_mismatch: source invoke {invoke_work_item_id} does not match authority source invoke {source_invoke_work_item_id}"
                    );
                }
                let retry_authority_id: String = active.get("id");
                crate::repos::retry_stage_execution_authorities::mark_terminalized_tx(
                    tx,
                    &retry_authority_id,
                    Utc::now(),
                    "stale_targeted_authority_superseded_by_normal_invoke",
                )
                .await?;
                active_authority = None;
            }
        }
    }
    if let Some(active) = active_authority.as_ref() {
        if let Some(source_stage_execution_id) = source_stage_execution_id.as_deref() {
            let authority_target: String = active.get("target_stage_execution_id");
            if authority_target != source_stage_execution_id {
                anyhow::bail!(
                    "advance_run_source_target_mismatch: source invoke {invoke_work_item_id} stage {source_stage_execution_id} does not match authority target {authority_target}"
                );
            }
        }
    }

    let stage_id = active_authority
        .as_ref()
        .map(|row| row.get::<String, _>("stage_id"))
        .or_else(|| {
            source_payload
                .get("stage_id")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
        });
    let retry_authority_id = active_authority
        .as_ref()
        .map(|row| row.get::<String, _>("id"));
    let target_stage_execution_id = active_authority
        .as_ref()
        .map(|row| row.get::<String, _>("target_stage_execution_id"))
        .or(source_stage_execution_id);

    if let (Some(stage_id), Some(target_id), Some(authority_id)) = (
        stage_id.as_deref(),
        target_stage_execution_id.as_deref(),
        retry_authority_id.as_deref(),
    ) {
        payload["stage_id"] = serde_json::json!(stage_id);
        payload["target_stage_execution_id"] = serde_json::json!(target_id);
        payload["retry_authority_id"] = serde_json::json!(authority_id);
        payload["source_work_item_id"] = serde_json::json!(invoke_work_item_id);
        payload["source_invoke_work_item_id"] = serde_json::json!(invoke_work_item_id);
        payload["source_stage_execution_id"] = serde_json::json!(target_id);
        if let Some(source_agent_execution_id) = source_agent_execution_id.as_deref() {
            payload["source_agent_execution_id"] = serde_json::json!(source_agent_execution_id);
        }
        return Ok((payload.to_string(), Some(stage_id.to_string())));
    }

    Ok((payload.to_string(), None))
}

async fn find_agent_execution_stage_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    agent_execution_id: &str,
) -> Result<Option<StageExecutionId>> {
    if agent_execution_id.parse::<AgentExecutionId>().is_err() {
        return Ok(None);
    }
    let row = sqlx::query(
        r#"SELECT stage_execution_id
           FROM agent_executions
           WHERE id = ?1"#,
    )
    .bind(agent_execution_id)
    .fetch_optional(&mut **tx)
    .await
    .context("find agent execution stage for post-invoke AdvanceRun")?;
    row.and_then(|row| row.get::<Option<String>, _>("stage_execution_id"))
        .map(|raw| {
            raw.parse::<StageExecutionId>()
                .context("parse agent execution stage id")
        })
        .transpose()
}

async fn find_active_retry_authority_by_target_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    target_id: &str,
) -> Result<Option<sqlx::sqlite::SqliteRow>> {
    let rows = sqlx::query(
        r#"SELECT id, stage_id, target_stage_execution_id, source_invoke_work_item_id
           FROM retry_stage_execution_authorities
           WHERE target_stage_execution_id = ?1
             AND run_id = ?2
             AND authority_state = 'active'
           ORDER BY created_at DESC"#,
    )
    .bind(target_id)
    .bind(run_id)
    .fetch_all(&mut **tx)
    .await
    .context("find active retry authority by target for post-invoke AdvanceRun")?;
    if rows.len() > 1 {
        anyhow::bail!(
            "advance_run_authority_conflict: target stage {target_id} has duplicate active retry authorities"
        );
    }
    Ok(rows.into_iter().next())
}

async fn find_active_retry_authority_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    authority_id: &str,
) -> Result<Option<sqlx::sqlite::SqliteRow>> {
    sqlx::query(
        r#"SELECT id, stage_id, target_stage_execution_id, source_invoke_work_item_id
           FROM retry_stage_execution_authorities
           WHERE id = ?1
             AND run_id = ?2
             AND authority_state = 'active'
           LIMIT 1"#,
    )
    .bind(authority_id)
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await
    .context("find active retry authority by id for post-invoke AdvanceRun")
}

enum AdvancePayloadScope {
    LegacyRunScoped,
    Targeted,
    Malformed(&'static str),
}

enum AdvancePayloadAuthorityMatch {
    Match,
    NoMatch,
    Malformed(&'static str),
}

fn classify_advance_payload_scope(payload_json: &str) -> AdvancePayloadScope {
    match AdvanceRunPayloadV1::parse_json(payload_json) {
        Ok(payload) if payload.target_stage_execution_id.is_some() => AdvancePayloadScope::Targeted,
        Ok(_) => AdvancePayloadScope::LegacyRunScoped,
        Err(error) if payload_looks_targeted_or_typed(payload_json) => {
            AdvancePayloadScope::Malformed(error.code())
        }
        Err(_) => AdvancePayloadScope::LegacyRunScoped,
    }
}

fn advance_payload_matches_authority(
    payload_json: &str,
    retry_authority_id: &str,
    target_stage_execution_id: StageExecutionId,
) -> AdvancePayloadAuthorityMatch {
    if let Err(error) = AdvanceRunPayloadV1::parse_json(payload_json) {
        return if payload_looks_targeted_or_typed(payload_json) {
            AdvancePayloadAuthorityMatch::Malformed(error.code())
        } else {
            AdvancePayloadAuthorityMatch::NoMatch
        };
    }
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload_json) else {
        return AdvancePayloadAuthorityMatch::Malformed("advance_run_payload_malformed");
    };
    let target = target_stage_execution_id.to_string();
    if payload
        .get("retry_authority_id")
        .and_then(|value| value.as_str())
        == Some(retry_authority_id)
        && payload
            .get("target_stage_execution_id")
            .and_then(|value| value.as_str())
            == Some(target.as_str())
    {
        AdvancePayloadAuthorityMatch::Match
    } else {
        AdvancePayloadAuthorityMatch::NoMatch
    }
}

fn payload_looks_targeted_or_typed(payload_json: &str) -> bool {
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload_json) else {
        return true;
    };
    payload.get("schema_version").is_some()
        || payload.get("target_stage_execution_id").is_some()
        || payload.get("retry_authority_id").is_some()
        || payload.get("source_stage_execution_id").is_some()
        || payload.get("source_work_item_id").is_some()
        || payload.get("source_invoke_work_item_id").is_some()
}

async fn quarantine_advance_work_item_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    error_code: &str,
) -> Result<u64> {
    let result = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1,
               failed_at = ?2,
               last_error = ?3
           WHERE id = ?4
             AND kind = ?5
             AND status IN (?6, ?7)"#,
    )
    .bind(WorkItemStatus::Failed.to_string())
    .bind(Utc::now().to_rfc3339())
    .bind(error_code)
    .bind(id)
    .bind(WorkItemKind::AdvanceRun.to_string())
    .bind(WorkItemStatus::Pending.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .execute(&mut **tx)
    .await
    .context("quarantine malformed targeted AdvanceRun payload")?;
    Ok(result.rows_affected())
}

pub async fn fail_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    error: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    let status = WorkItemStatus::Failed.to_string();
    sqlx::query(
        r#"UPDATE work_items
           SET status = ?1, failed_at = ?2, last_error = ?3
           WHERE id = ?4 AND status IN (?5, ?6)"#,
    )
    .bind(status)
    .bind(now.to_rfc3339())
    .bind(error)
    .bind(id)
    .bind(WorkItemStatus::Pending.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .execute(&mut **tx)
    .await
    .context("fail work item")?;
    Ok(())
}

pub async fn requeue_running_after_transient_persistence_contention(
    pool: &SqlitePool,
    id: &str,
    now: DateTime<Utc>,
    error: &str,
) -> Result<bool> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.requeue_transient_persistence_contention",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.requeue_transient_persistence_contention",
        ),
        "work_items.requeue_transient_persistence_contention",
    )
    .await?;
    let row = sqlx::query(r#"SELECT attempt_count FROM work_items WHERE id = ?1 AND status = ?2"#)
        .bind(id)
        .bind(WorkItemStatus::Running.to_string())
        .fetch_optional(&mut **tx)
        .await
        .context("load running work item for transient persistence requeue")?;

    let Some(row) = row else {
        tx.commit()
            .await
            .context("commit transient persistence requeue no-op")?;
        log_write_transaction(
            "work_items.requeue_transient_persistence_contention.noop",
            tx_started,
        );
        return Ok(false);
    };

    let attempt_count: i64 = row.get("attempt_count");
    let backoff_seconds = (1_i64 << attempt_count.clamp(0, 6)).min(60);
    let scheduled_at = now + chrono::Duration::seconds(backoff_seconds);
    let last_error = format!("transient_persistence_contention: {error}");
    let updated = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1,
               scheduled_at = ?2,
               started_at = NULL,
               failed_at = NULL,
               last_error = ?3
           WHERE id = ?4 AND status = ?5"#,
    )
    .bind(WorkItemStatus::Pending.to_string())
    .bind(scheduled_at.to_rfc3339())
    .bind(last_error)
    .bind(id)
    .bind(WorkItemStatus::Running.to_string())
    .execute(&mut **tx)
    .await
    .context("requeue work item after transient persistence contention")?
    .rows_affected();

    tx.commit()
        .await
        .context("commit transient persistence requeue")?;
    log_write_transaction(
        "work_items.requeue_transient_persistence_contention",
        tx_started,
    );
    Ok(updated == 1)
}

pub async fn cancel_running_by_run(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let tx_started = Instant::now();
    let mut tx = begin_registered_immediate_transaction(
        pool,
        crate::writer::class_a_operation(
            "work_items.cancel_running_by_run",
            crate::write_class::WriteLane::CriticalBarrier,
            "work_items.cancel_running_by_run",
        ),
        "work_items.cancel_running_by_run",
    )
    .await?;
    cancel_running_by_run_tx(&mut tx, run_id, Utc::now()).await?;
    tx.commit()
        .await
        .context("commit cancel running work items by run")?;
    log_write_transaction("work_items.cancel_running_by_run", tx_started);
    Ok(())
}

pub async fn cancel_running_by_run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    completed_at: DateTime<Utc>,
) -> Result<u64> {
    let cancelled = WorkItemStatus::Cancelled.to_string();
    let running = WorkItemStatus::Running.to_string();
    let result = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1, completed_at = ?2
           WHERE run_id = ?3 AND status = ?4"#,
    )
    .bind(cancelled)
    .bind(completed_at.to_rfc3339())
    .bind(run_id.to_string())
    .bind(running)
    .execute(&mut **tx)
    .await
    .context("cancel running work items by run")?;
    Ok(result.rows_affected())
}

pub async fn cancel_pending_or_running_by_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_id: &str,
    completed_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let cancelled = WorkItemStatus::Cancelled.to_string();
    let pending = WorkItemStatus::Pending.to_string();
    let running = WorkItemStatus::Running.to_string();
    let result = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1, completed_at = ?2, last_error = ?3
           WHERE run_id = ?4
             AND stage_id = ?5
             AND status IN (?6, ?7)"#,
    )
    .bind(cancelled)
    .bind(completed_at.to_rfc3339())
    .bind(reason)
    .bind(run_id.to_string())
    .bind(stage_id)
    .bind(pending)
    .bind(running)
    .execute(&mut **tx)
    .await
    .context("cancel pending/running work items by stage")?;
    Ok(result.rows_affected())
}

pub async fn cancel_pending_or_running_invoke_by_stage_execution_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_execution_id: &str,
    completed_at: DateTime<Utc>,
    reason: &str,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"SELECT id, payload_json
           FROM work_items
           WHERE run_id = ?1
             AND kind = ?2
             AND status IN (?3, ?4)
           ORDER BY scheduled_at ASC, rowid ASC"#,
    )
    .bind(run_id.to_string())
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Pending.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .fetch_all(&mut **tx)
    .await
    .context("load pending/running InvokeAgent work items for stage execution")?;

    let mut cancelled = 0_u64;
    for row in rows {
        let payload_json: String = row.get("payload_json");
        let Ok(payload) = serde_json::from_str::<serde_json::Value>(&payload_json) else {
            continue;
        };
        if payload
            .get("stage_execution_id")
            .and_then(|value| value.as_str())
            != Some(stage_execution_id)
        {
            continue;
        }
        let result = sqlx::query(
            r#"UPDATE work_items
               SET status = ?1, completed_at = ?2, last_error = ?3
               WHERE id = ?4 AND status IN (?5, ?6)"#,
        )
        .bind(WorkItemStatus::Cancelled.to_string())
        .bind(completed_at.to_rfc3339())
        .bind(reason)
        .bind(row.get::<String, _>("id"))
        .bind(WorkItemStatus::Pending.to_string())
        .bind(WorkItemStatus::Running.to_string())
        .execute(&mut **tx)
        .await
        .context("cancel pending/running InvokeAgent work item by stage execution")?;
        cancelled += result.rows_affected();
    }

    Ok(cancelled)
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> Result<Option<WorkItem>> {
    let row = sqlx::query(
        r#"SELECT id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items WHERE id = ?1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .context("find work item by id")?;

    row.map(|r| {
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
    .transpose()
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    async fn test_pool() -> SqlitePool {
        let pool = crate::pool::create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool");
        crate::writer::register_shared_writer(
            &pool,
            std::sync::Arc::new(crate::writer::DbWriter::new(pool.clone())),
        )
        .await
        .expect("register shared writer");
        pool
    }

    #[tokio::test]
    async fn invoke_agent_failure_enqueues_advance_run_for_fan_in() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let now = Utc::now();
        enqueue(
            &pool,
            &WorkItem {
                id: "invoke-startup-failed".to_string(),
                kind: WorkItemKind::InvokeAgent,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "stage_id": "review",
                })
                .to_string(),
                status: WorkItemStatus::Running,
                run_id: Some(run_id),
                stage_id: Some("review".to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 1,
                last_error: None,
            },
        )
        .await
        .expect("insert running invoke work item");

        fail(&pool, "invoke-startup-failed", "xcode_target_not_found")
            .await
            .expect("fail invoke work item");

        let items = list_by_run(&pool, run_id).await.expect("list by run");
        let failed = items
            .iter()
            .find(|item| item.id == "invoke-startup-failed")
            .expect("failed invoke item");
        assert_eq!(failed.status, WorkItemStatus::Failed);
        assert_eq!(failed.last_error.as_deref(), Some("xcode_target_not_found"));

        let advance = items
            .iter()
            .find(|item| item.id == "advance-after-invoke:invoke-startup-failed")
            .expect("post-failure advance");
        assert_eq!(advance.kind, WorkItemKind::AdvanceRun);
        assert_eq!(advance.status, WorkItemStatus::Pending);
        assert_eq!(advance.stage_id, None);

        let payload: serde_json::Value =
            serde_json::from_str(&advance.payload_json).expect("advance payload");
        assert_eq!(payload["run_id"], run_id.to_string());
        assert_eq!(payload["reason"], "invoke_agent_failed");
        assert_eq!(
            payload["failed_invoke_work_item_id"],
            "invoke-startup-failed"
        );
    }

    #[tokio::test]
    async fn p091_authority_scoped_advance_cancel_does_not_touch_siblings() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let target = StageExecutionId::new();
        let sibling_target = StageExecutionId::new();
        let now = Utc::now();
        for (id, authority, stage_execution_id) in [
            ("advance-target", "auth-target", target),
            ("advance-sibling", "auth-sibling", sibling_target),
        ] {
            enqueue(
                &pool,
                &WorkItem {
                    id: id.to_string(),
                    kind: WorkItemKind::AdvanceRun,
                    payload_json: serde_json::json!({
                        "schema_version": "advance_run_payload.v1",
                        "run_id": run_id.to_string(),
                        "stage_id": "implement",
                        "target_stage_execution_id": stage_execution_id.to_string(),
                        "retry_authority_id": authority,
                        "enqueue_reason": "retry_stage",
                        "reason": "operator_full_stage_retry",
                    })
                    .to_string(),
                    status: WorkItemStatus::Running,
                    run_id: Some(run_id),
                    stage_id: Some("implement".to_string()),
                    created_at: now,
                    scheduled_at: now,
                    attempt_count: 0,
                    last_error: None,
                },
            )
            .await
            .expect("insert advance");
        }
        enqueue(
            &pool,
            &WorkItem {
                id: "advance-malformed-targeted".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "schema_version": "advance_run_payload.v1",
                    "run_id": run_id.to_string(),
                    "stage_id": "implement",
                    "retry_authority_id": "auth-target",
                    "enqueue_reason": "retry_stage",
                    "reason": "operator_full_stage_retry",
                })
                .to_string(),
                status: WorkItemStatus::Pending,
                run_id: Some(run_id),
                stage_id: Some("implement".to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
        )
        .await
        .expect("insert malformed targeted advance");

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "p091-test-cancel")
            .await
            .unwrap();
        let cancelled = cancel_pending_or_running_advance_by_retry_authority_tx(
            &mut tx,
            run_id,
            "auth-target",
            target,
            now,
            "targeted_cancel",
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(cancelled, 1);
        assert_eq!(
            find_by_id(&pool, "advance-target")
                .await
                .unwrap()
                .unwrap()
                .status,
            WorkItemStatus::Cancelled
        );
        assert_eq!(
            find_by_id(&pool, "advance-sibling")
                .await
                .unwrap()
                .unwrap()
                .status,
            WorkItemStatus::Running
        );
        let malformed = find_by_id(&pool, "advance-malformed-targeted")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(malformed.status, WorkItemStatus::Failed);
        assert_eq!(
            malformed.last_error.as_deref(),
            Some("advance_run_payload_missing_target_for_authority")
        );
    }

    #[tokio::test]
    async fn p091_claim_next_quarantines_malformed_typed_advance_and_claims_next() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let now = Utc::now();
        enqueue(
            &pool,
            &WorkItem {
                id: "advance-malformed-first".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "schema_version": "advance_run_payload.v1",
                    "run_id": run_id.to_string(),
                    "stage_id": "implement",
                    "retry_authority_id": "auth-target",
                    "enqueue_reason": "retry_stage",
                })
                .to_string(),
                status: WorkItemStatus::Pending,
                run_id: Some(run_id),
                stage_id: Some("implement".to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
        )
        .await
        .unwrap();
        enqueue(
            &pool,
            &WorkItem {
                id: "advance-valid-next".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "reason": "startup_catchup",
                })
                .to_string(),
                status: WorkItemStatus::Pending,
                run_id: Some(run_id),
                stage_id: None,
                created_at: now + Duration::milliseconds(1),
                scheduled_at: now + Duration::milliseconds(1),
                attempt_count: 0,
                last_error: None,
            },
        )
        .await
        .unwrap();

        let claimed = claim_next(&pool).await.unwrap().unwrap();
        assert_eq!(claimed.id, "advance-valid-next");
        assert_eq!(claimed.status, WorkItemStatus::Running);
        let malformed = find_by_id(&pool, "advance-malformed-first")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(malformed.status, WorkItemStatus::Failed);
        assert_eq!(
            malformed.last_error.as_deref(),
            Some("advance_run_payload_missing_target_for_authority")
        );
    }

    #[tokio::test]
    async fn p091_claim_next_quarantines_source_work_item_only_retry_advance() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let now = Utc::now();
        enqueue(
            &pool,
            &WorkItem {
                id: "advance-source-only-first".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "schema_version": "advance_run_payload.v1",
                    "run_id": run_id.to_string(),
                    "stage_id": "implement",
                    "source_work_item_id": "advance-source-only-first",
                    "enqueue_reason": "retry_stage",
                })
                .to_string(),
                status: WorkItemStatus::Pending,
                run_id: Some(run_id),
                stage_id: Some("implement".to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
        )
        .await
        .unwrap();
        enqueue(
            &pool,
            &WorkItem {
                id: "advance-legacy-after-source-only".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "reason": "startup_catchup",
                })
                .to_string(),
                status: WorkItemStatus::Pending,
                run_id: Some(run_id),
                stage_id: None,
                created_at: now + Duration::milliseconds(1),
                scheduled_at: now + Duration::milliseconds(1),
                attempt_count: 0,
                last_error: None,
            },
        )
        .await
        .unwrap();

        let claimed = claim_next(&pool).await.unwrap().unwrap();
        assert_eq!(claimed.id, "advance-legacy-after-source-only");
        let malformed = find_by_id(&pool, "advance-source-only-first")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(malformed.status, WorkItemStatus::Failed);
        assert_eq!(
            malformed.last_error.as_deref(),
            Some("advance_run_payload_target_required")
        );
    }

    #[tokio::test]
    async fn p091_run_scoped_requeue_skips_targeted_and_quarantines_malformed() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let target = StageExecutionId::new();
        let now = Utc::now();
        for item in [
            WorkItem {
                id: "advance-legacy".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "reason": "startup_catchup",
                })
                .to_string(),
                status: WorkItemStatus::Running,
                run_id: Some(run_id),
                stage_id: None,
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
            WorkItem {
                id: "advance-targeted".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "schema_version": "advance_run_payload.v1",
                    "run_id": run_id.to_string(),
                    "stage_id": "implement",
                    "target_stage_execution_id": target.to_string(),
                    "retry_authority_id": "auth-target",
                    "enqueue_reason": "retry_stage",
                })
                .to_string(),
                status: WorkItemStatus::Running,
                run_id: Some(run_id),
                stage_id: Some("implement".to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
            WorkItem {
                id: "advance-malformed".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "schema_version": "advance_run_payload.v1",
                    "run_id": run_id.to_string(),
                    "stage_id": "implement",
                    "retry_authority_id": "auth-target",
                    "enqueue_reason": "retry_stage",
                })
                .to_string(),
                status: WorkItemStatus::Running,
                run_id: Some(run_id),
                stage_id: Some("implement".to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 0,
                last_error: None,
            },
        ] {
            enqueue(&pool, &item).await.unwrap();
        }

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "p091-test-requeue")
            .await
            .unwrap();
        let requeued = requeue_running_advance_by_run_tx(
            &mut tx,
            run_id,
            now + Duration::seconds(1),
            "startup_repair_abandoned_advance_run",
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(requeued, 1);
        assert_eq!(
            find_by_id(&pool, "advance-legacy")
                .await
                .unwrap()
                .unwrap()
                .status,
            WorkItemStatus::Pending
        );
        assert_eq!(
            find_by_id(&pool, "advance-targeted")
                .await
                .unwrap()
                .unwrap()
                .status,
            WorkItemStatus::Running
        );
        let malformed = find_by_id(&pool, "advance-malformed")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(malformed.status, WorkItemStatus::Failed);
        assert_eq!(
            malformed.last_error.as_deref(),
            Some("advance_run_payload_missing_target_for_authority")
        );
    }

    #[tokio::test]
    async fn watchdog_requeues_stale_legacy_and_targeted_advance_items() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let target = StageExecutionId::new();
        let stale = Utc::now() - Duration::minutes(10);
        let fresh = Utc::now();
        sqlx::query(
            r#"INSERT INTO ideas (id, title, body, status, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
        )
        .bind("idea-watchdog-advance")
        .bind("Watchdog advance")
        .bind("Watchdog")
        .bind("active")
        .bind(stale.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO runs
               (id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )
        .bind(run_id.to_string())
        .bind("idea-watchdog-advance")
        .bind("running")
        .bind("wf-watchdog")
        .bind("Watchdog")
        .bind("/tmp/ws")
        .bind("/tmp/artifacts")
        .bind(stale.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO stage_executions
               (id, run_id, stage_id, label, status, iteration, attempt_number, started_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )
        .bind(target.to_string())
        .bind(run_id.to_string())
        .bind("implement")
        .bind("Implement")
        .bind("running")
        .bind(0_i64)
        .bind(1_i64)
        .bind(stale.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO retry_stage_execution_authorities
               (id, run_id, stage_id, target_stage_execution_id, entry_kind,
                source_invoke_work_item_id, authority_state, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
        )
        .bind("auth-watchdog-targeted")
        .bind(run_id.to_string())
        .bind("implement")
        .bind(target.to_string())
        .bind("targeted_agent_retry")
        .bind("invoke-watchdog-targeted")
        .bind("active")
        .bind(stale.to_rfc3339())
        .bind(stale.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

        for item in [
            WorkItem {
                id: "advance-watchdog-legacy".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "reason": "startup_catchup",
                })
                .to_string(),
                status: WorkItemStatus::Running,
                run_id: Some(run_id),
                stage_id: None,
                created_at: stale,
                scheduled_at: stale,
                attempt_count: 0,
                last_error: None,
            },
            WorkItem {
                id: "advance-watchdog-targeted".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "schema_version": "advance_run_payload.v1",
                    "run_id": run_id.to_string(),
                    "stage_id": "implement",
                    "target_stage_execution_id": target.to_string(),
                    "retry_authority_id": "auth-watchdog-targeted",
                    "enqueue_reason": "retry_stage",
                })
                .to_string(),
                status: WorkItemStatus::Running,
                run_id: Some(run_id),
                stage_id: Some("implement".to_string()),
                created_at: stale,
                scheduled_at: stale,
                attempt_count: 2,
                last_error: None,
            },
            WorkItem {
                id: "advance-watchdog-fresh".to_string(),
                kind: WorkItemKind::AdvanceRun,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "reason": "fresh",
                })
                .to_string(),
                status: WorkItemStatus::Running,
                run_id: Some(run_id),
                stage_id: None,
                created_at: fresh,
                scheduled_at: fresh,
                attempt_count: 0,
                last_error: None,
            },
        ] {
            enqueue(&pool, &item).await.unwrap();
            sqlx::query("UPDATE work_items SET started_at = ?1 WHERE id = ?2")
                .bind(item.scheduled_at.to_rfc3339())
                .bind(&item.id)
                .execute(&pool)
                .await
                .unwrap();
        }

        let requeued = requeue_stale_running_advance_items(
            &pool,
            Utc::now() - Duration::minutes(5),
            Utc::now(),
            "watchdog_stale_advance_run",
        )
        .await
        .unwrap();

        assert_eq!(requeued, 2);
        let legacy = find_by_id(&pool, "advance-watchdog-legacy")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(legacy.status, WorkItemStatus::Pending);
        assert_eq!(legacy.attempt_count, 1);
        assert_eq!(
            legacy.last_error.as_deref(),
            Some("watchdog_stale_advance_run")
        );
        let targeted = find_by_id(&pool, "advance-watchdog-targeted")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(targeted.status, WorkItemStatus::Pending);
        assert_eq!(targeted.attempt_count, 3);
        assert_eq!(
            targeted.last_error.as_deref(),
            Some("watchdog_stale_advance_run")
        );
        assert_eq!(
            find_by_id(&pool, "advance-watchdog-fresh")
                .await
                .unwrap()
                .unwrap()
                .status,
            WorkItemStatus::Running
        );
    }

    async fn seed_p091_post_invoke_rows(
        pool: &SqlitePool,
        run_id: RunId,
        target: StageExecutionId,
        agent_execution_id: AgentExecutionId,
        source_invoke_work_item_id: &str,
    ) {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"INSERT INTO ideas (id, title, body, status, created_at)
               VALUES (?1, ?2, ?3, ?4, ?5)"#,
        )
        .bind("idea-p091-post-invoke")
        .bind("P091 post invoke")
        .bind("P091")
        .bind("active")
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO runs
               (id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )
        .bind(run_id.to_string())
        .bind("idea-p091-post-invoke")
        .bind("running")
        .bind("wf-p091")
        .bind("P091")
        .bind("/tmp/ws")
        .bind("/tmp/artifacts")
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO stage_executions
               (id, run_id, stage_id, label, status, iteration, attempt_number, started_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        )
        .bind(target.to_string())
        .bind(run_id.to_string())
        .bind("implement")
        .bind("Implement")
        .bind("running")
        .bind(0_i64)
        .bind(2_i64)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO retry_stage_execution_authorities
               (id, run_id, stage_id, target_stage_execution_id, entry_kind,
                source_invoke_work_item_id, authority_state, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
        )
        .bind("auth-post-invoke")
        .bind(run_id.to_string())
        .bind("implement")
        .bind(target.to_string())
        .bind("targeted_agent_retry")
        .bind(source_invoke_work_item_id)
        .bind("active")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO agent_executions
               (id, stage_execution_id, agent_id, provider, status, started_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
        )
        .bind(agent_execution_id.to_string())
        .bind(target.to_string())
        .bind("code_writer")
        .bind("junie")
        .bind("completed")
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn p091_post_invoke_advance_preserves_target_from_agent_execution_stage() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let target = StageExecutionId::new();
        let agent_execution_id = AgentExecutionId::new();
        seed_p091_post_invoke_rows(
            &pool,
            run_id,
            target,
            agent_execution_id,
            "invoke-post-invoke",
        )
        .await;

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "p091-post-invoke")
            .await
            .unwrap();
        let (payload_json, stage_id) = build_post_invoke_advance_payload_tx(
            &mut tx,
            &run_id.to_string(),
            "invoke_agent_completed",
            "completed_invoke_work_item_id",
            "invoke-post-invoke",
            &serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "implement",
                "targeted_retry": {
                    "retry_authority_id": "auth-post-invoke",
                    "source_agent_execution_id": agent_execution_id.to_string()
                }
            })
            .to_string(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let payload = AdvanceRunPayloadV1::parse_json(&payload_json).unwrap();
        assert_eq!(stage_id.as_deref(), Some("implement"));
        assert_eq!(
            payload.target_stage_execution_id.map(|id| id.to_string()),
            Some(target.to_string())
        );
        assert_eq!(
            payload.retry_authority_id.as_deref(),
            Some("auth-post-invoke")
        );
        assert_eq!(
            payload.source_work_item_id.as_deref(),
            Some("invoke-post-invoke")
        );
        assert_eq!(
            payload.source_stage_execution_id.map(|id| id.to_string()),
            Some(target.to_string())
        );
    }

    #[tokio::test]
    async fn p091_normal_post_invoke_terminalizes_stale_targeted_authority_from_prior_retry() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let target = StageExecutionId::new();
        let agent_execution_id = AgentExecutionId::new();
        seed_p091_post_invoke_rows(
            &pool,
            run_id,
            target,
            agent_execution_id,
            "auto-contract-output-retry:old-source",
        )
        .await;

        let mut tx = crate::pool::begin_immediate_with_retry(
            &pool,
            "p091-post-invoke-stale-targeted-authority",
        )
        .await
        .unwrap();
        let (payload_json, stage_id) = build_post_invoke_advance_payload_tx(
            &mut tx,
            &run_id.to_string(),
            "invoke_agent_completed",
            "completed_invoke_work_item_id",
            "p058-invoke:normal-fanout",
            &serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "implement",
                "stage_execution_id": target.to_string(),
                "p058_claimed": {
                    "agent_execution_id": agent_execution_id.to_string()
                }
            })
            .to_string(),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        assert_eq!(stage_id, None);
        let payload: serde_json::Value = serde_json::from_str(&payload_json).unwrap();
        assert_eq!(
            payload["completed_invoke_work_item_id"],
            serde_json::json!("p058-invoke:normal-fanout")
        );
        assert!(payload.get("retry_authority_id").is_none());
        assert!(payload.get("target_stage_execution_id").is_none());

        let authority =
            crate::repos::retry_stage_execution_authorities::find_by_id(&pool, "auth-post-invoke")
                .await
                .unwrap()
                .unwrap();
        assert_eq!(
            authority.authority_state,
            domain::retry_authority::RetryAuthorityState::Terminalized
        );
        assert_eq!(
            authority.terminal_reason.as_deref(),
            Some("stale_targeted_authority_superseded_by_normal_invoke")
        );
    }

    #[tokio::test]
    async fn p091_post_invoke_authority_hint_without_source_stage_fails_closed() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let target = StageExecutionId::new();
        seed_p091_post_invoke_rows(
            &pool,
            run_id,
            target,
            AgentExecutionId::new(),
            "invoke-post-invoke",
        )
        .await;

        let mut tx =
            crate::pool::begin_immediate_with_retry(&pool, "p091-post-invoke-missing-stage")
                .await
                .unwrap();
        let error = build_post_invoke_advance_payload_tx(
            &mut tx,
            &run_id.to_string(),
            "invoke_agent_completed",
            "completed_invoke_work_item_id",
            "invoke-post-invoke",
            &serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "implement",
                "retry_authority_id": "auth-post-invoke"
            })
            .to_string(),
        )
        .await
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("advance_run_source_target_mismatch"));
    }

    #[tokio::test]
    async fn p091_post_invoke_authority_target_mismatch_fails_closed() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let target = StageExecutionId::new();
        let wrong_target = StageExecutionId::new();
        seed_p091_post_invoke_rows(
            &pool,
            run_id,
            target,
            AgentExecutionId::new(),
            "invoke-post-invoke",
        )
        .await;

        let mut tx = crate::pool::begin_immediate_with_retry(&pool, "p091-post-invoke-mismatch")
            .await
            .unwrap();
        let error = build_post_invoke_advance_payload_tx(
            &mut tx,
            &run_id.to_string(),
            "invoke_agent_failed",
            "failed_invoke_work_item_id",
            "invoke-post-invoke",
            &serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "implement",
                "target_stage_execution_id": wrong_target.to_string(),
                "retry_authority_id": "auth-post-invoke"
            })
            .to_string(),
        )
        .await
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("advance_run_source_target_mismatch"));
    }

    #[tokio::test]
    async fn proposal_061_host_interruption_requeue_strips_preclaim_and_reschedules() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let stage_execution_id = StageExecutionId::new();
        let now = Utc::now();
        let retry_at = now + Duration::seconds(17);
        let item = WorkItem {
            id: "invoke-host-retry".to_string(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "code",
                "stage_execution_id": stage_execution_id.to_string(),
                "p058_claimed": {
                    "agent_execution_id": domain::ids::AgentExecutionId::new().to_string()
                }
            })
            .to_string(),
            status: WorkItemStatus::Running,
            run_id: Some(run_id),
            stage_id: Some("code".to_string()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 1,
            last_error: Some("stale transport".to_string()),
        };
        enqueue(&pool, &item)
            .await
            .expect("insert running work item");

        let requeued = requeue_running_invoke_agent_by_stage_for_host_interruption(
            &pool,
            run_id,
            "code",
            stage_execution_id,
            retry_at,
        )
        .await
        .expect("requeue host interruption work");

        assert_eq!(requeued, vec!["invoke-host-retry".to_string()]);
        let items = list_by_run(&pool, run_id).await.expect("list by run");
        let requeued_item = items
            .iter()
            .find(|item| item.id == "invoke-host-retry")
            .expect("requeued item");
        assert_eq!(requeued_item.status, WorkItemStatus::Pending);
        assert_eq!(requeued_item.scheduled_at, retry_at);
        assert_eq!(requeued_item.last_error, None);
        let payload: serde_json::Value =
            serde_json::from_str(&requeued_item.payload_json).expect("payload json");
        assert!(payload.get("p058_claimed").is_none());
        assert_eq!(
            payload
                .pointer("/host_interruption_retry/stage_execution_id")
                .and_then(|value| value.as_str())
                .map(str::to_string),
            Some(stage_execution_id.to_string())
        );
    }

    #[tokio::test]
    async fn startup_requeue_strips_preclaim_so_retry_gets_new_execution() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let stage_execution_id = StageExecutionId::new();
        let now = Utc::now();
        let item = WorkItem {
            id: "invoke-startup-retry".to_string(),
            kind: WorkItemKind::InvokeAgent,
            payload_json: serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "review",
                "stage_execution_id": stage_execution_id.to_string(),
                "p058_claimed": {
                    "agent_execution_id": domain::ids::AgentExecutionId::new().to_string(),
                    "session_generation_id": uuid::Uuid::new_v4().to_string()
                }
            })
            .to_string(),
            status: WorkItemStatus::Running,
            run_id: Some(run_id),
            stage_id: Some("review".to_string()),
            created_at: now,
            scheduled_at: now,
            attempt_count: 1,
            last_error: Some("old runtime vanished".to_string()),
        };
        enqueue(&pool, &item)
            .await
            .expect("insert running work item");

        let requeued = {
            let mut tx = crate::pool::begin_immediate_with_retry(
                &pool,
                "test.startup_requeue_strips_preclaim",
            )
            .await
            .unwrap();
            let count = requeue_running_invoke_agent_on_startup_tx(
                &mut tx,
                now + Duration::seconds(5),
                "startup_repair_abandoned_invoke_agent",
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
            count
        };

        assert_eq!(requeued, 1);
        let items = list_by_run(&pool, run_id).await.expect("list by run");
        let requeued_item = items
            .iter()
            .find(|item| item.id == "invoke-startup-retry")
            .expect("requeued item");
        assert_eq!(requeued_item.status, WorkItemStatus::Pending);
        assert_eq!(
            requeued_item.last_error.as_deref(),
            Some("startup_repair_abandoned_invoke_agent")
        );
        let payload: serde_json::Value =
            serde_json::from_str(&requeued_item.payload_json).expect("payload json");
        assert!(payload.get("p058_claimed").is_none());
        assert_eq!(
            payload
                .pointer("/p061_startup_recovery/reason")
                .and_then(|value| value.as_str()),
            Some("startup_repair_abandoned_invoke_agent")
        );
    }

    #[tokio::test]
    async fn startup_recovery_completes_running_invoke_when_agent_already_has_valid_outputs() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let stage_execution_id = StageExecutionId::new();
        let agent_execution_id = AgentExecutionId::new();
        let now = Utc::now();

        let idea_id = domain::ids::IdeaId::new();
        crate::repos::ideas::insert(
            &pool,
            &domain::idea::Idea {
                id: idea_id,
                title: "stale close recovery".to_string(),
                body: "agent output already settled".to_string(),
                workspace_root_path: Some("/tmp/chainworks-test".to_string()),
                project_key: None,
                status: domain::idea::IdeaStatus::Active,
                created_at: now,
                archived_at: None,
            },
        )
        .await
        .expect("insert idea");
        crate::repos::runs::insert(
            &pool,
            &domain::run::Run {
                id: run_id,
                idea_id,
                status: domain::run::RunStatus::Running,
                workflow_id: "test-workflow".to_string(),
                workflow_title: "Test Workflow".to_string(),
                workspace_root: "/tmp/chainworks-test".to_string(),
                artifact_root: "/tmp/chainworks-test/.chainworks".to_string(),
                started_at: now,
                completed_at: None,
                cancellation_requested_at: None,
                cancellation_settled_at: None,
                cancellation_settlement_log: None,
                current_state: Some("implementation_review".to_string()),
                workflow_yaml_path: None,
                agent_catalog_yaml_path: None,
                worktree_root: None,
                base_branch: None,
                base_revision: None,
                target_branch: None,
                delivery_configuration_json: None,
                delivery_preflight_json: None,
                workflow_family: None,
                project_key: None,
                risk_class: None,
                stack: None,
                workflow_snapshot_hash: None,
                catalog_snapshot_hash: None,
                workflow_snapshot_json: None,
                catalog_snapshot_json: None,
                drift_detected_at: None,
                drift_details_json: None,
                chainworks_meta_root: None,
                review_routing_json: None,
                closeout_readiness_mode: None,
            },
        )
        .await
        .expect("insert run");
        crate::repos::stages::insert(
            &pool,
            &domain::stage::StageExecution {
                id: stage_execution_id,
                run_id,
                stage_id: "implementation_review".to_string(),
                label: "Implementation Review".to_string(),
                status: domain::stage::StageStatus::Running,
                iteration: 1,
                attempt_number: 1,
                settlement_kind: None,
                started_at: now,
                completed_at: None,
                owner_agent: Some("proposal_implementation_auditor".to_string()),
                provider: Some("codex".to_string()),
                model: Some("gpt-5.5".to_string()),
                stage_type: None,
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: None,
            },
        )
        .await
        .expect("insert stage execution");
        crate::repos::agent_executions::insert(
            &pool,
            &domain::agent::AgentExecution {
                id: agent_execution_id,
                stage_execution_id: Some(stage_execution_id),
                agent_id: "proposal_implementation_auditor".to_string(),
                provider: "codex".to_string(),
                model: Some("gpt-5.5".to_string()),
                started_at: now - Duration::minutes(30),
                completed_at: Some(now - Duration::minutes(1)),
                status: AgentStatus::Completed,
                owner_execution_lineage_id: None,
                session_lineage_id: None,
                session_generation_id: None,
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: None,
                session_reuse_scope: Some("none".to_string()),
                session_family_id: None,
                session_reuse_disposition: None,
                session_reset_reason: None,
                backend_profile_id: None,
                requested_mcp_extensions_json: None,
                predicted_mcp_extensions_json: None,
                predicted_mcp_runtime_ids_json: None,
                actual_mcp_extensions_json: None,
                actual_mcp_runtime_ids_json: None,
                denied_mcp_extensions_json: None,
                mcp_blocking_issues_json: None,
                actual_mcp_observation_json: None,
                actual_xcode_runtime_observation_json: None,
                mcp_session_startup_latency_ms: None,
                owner_kind: None,
                owner_id: None,
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
            },
        )
        .await
        .expect("insert completed agent execution");
        let mut facts =
            domain::agent::AgentExecutionRuntimeFacts::defaults_for(agent_execution_id, now);
        facts.output_settlement =
            domain::agent::AgentOutputSettlement::ValidOutputsFromCompletedExecution;
        facts.valid_required_outputs = true;
        crate::repos::agent_execution_runtime_facts::upsert(&pool, &facts)
            .await
            .expect("insert runtime facts");
        enqueue(
            &pool,
            &WorkItem {
                id: "invoke-close-hung-after-output".to_string(),
                kind: WorkItemKind::InvokeAgent,
                payload_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "stage_id": "implementation_review",
                    "stage_execution_id": stage_execution_id.to_string(),
                    "p058_claimed": {
                        "agent_execution_id": agent_execution_id.to_string(),
                        "session_generation_id": uuid::Uuid::new_v4().to_string()
                    }
                })
                .to_string(),
                status: WorkItemStatus::Running,
                run_id: Some(run_id),
                stage_id: Some("implementation_review".to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 1,
                last_error: None,
            },
        )
        .await
        .expect("insert running work item");

        let completed = complete_running_invoke_agents_with_terminal_valid_outputs_on_startup(
            &pool,
            "startup_repair_completed_agent_valid_outputs",
        )
        .await
        .expect("complete stale running invoke");

        assert_eq!(completed, 1);
        let item = find_by_id(&pool, "invoke-close-hung-after-output")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(item.status, WorkItemStatus::Completed);
        let advance = find_by_id(&pool, "advance-after-invoke:invoke-close-hung-after-output")
            .await
            .unwrap()
            .expect("post-invoke advance");
        assert_eq!(advance.status, WorkItemStatus::Pending);
    }

    #[tokio::test]
    async fn proposal_061_host_interruption_requeue_ignores_other_stage_attempts() {
        let pool = test_pool().await;
        let run_id = RunId::new();
        let stage_execution_id = StageExecutionId::new();
        let other_stage_execution_id = StageExecutionId::new();
        let now = Utc::now();
        enqueue(
            &pool,
            &WorkItem {
                id: "invoke-other-attempt".to_string(),
                kind: WorkItemKind::InvokeAgent,
                payload_json: serde_json::json!({
                    "stage_id": "code",
                    "stage_execution_id": other_stage_execution_id.to_string(),
                    "p058_claimed": {
                        "agent_execution_id": domain::ids::AgentExecutionId::new().to_string()
                    }
                })
                .to_string(),
                status: WorkItemStatus::Running,
                run_id: Some(run_id),
                stage_id: Some("code".to_string()),
                created_at: now,
                scheduled_at: now,
                attempt_count: 1,
                last_error: None,
            },
        )
        .await
        .expect("insert other attempt");

        let requeued = requeue_running_invoke_agent_by_stage_for_host_interruption(
            &pool,
            run_id,
            "code",
            stage_execution_id,
            now + Duration::seconds(5),
        )
        .await
        .expect("requeue host interruption work");

        assert!(requeued.is_empty());
        let items = list_by_run(&pool, run_id).await.expect("list by run");
        assert_eq!(items[0].status, WorkItemStatus::Running);
    }
}
