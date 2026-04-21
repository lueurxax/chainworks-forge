use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use domain::ids::RunId;
use domain::ids::StageExecutionId;
use domain::provider::{InvokeAgentCapacityConfig, ProviderFamily};

use crate::work_item::{WorkItem, WorkItemKind, WorkItemStatus};

const INVOKE_AGENT_CANDIDATE_WINDOW: i64 = 50;

#[derive(Debug)]
pub struct CapacityAwareClaimResult {
    pub item: Option<WorkItem>,
    pub all_invoke_agent_candidates_blocked: bool,
}

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

    // FIFO ordering with a deterministic tiebreaker. Without `rowid ASC`, two
    // work items enqueued within the same RFC3339 millisecond can be returned
    // in undefined order — a nondeterminism source that flakes tests which
    // depend on enqueue order (e.g. release tests that expect commit before
    // publish). `rowid` is SQLite's monotonic insert sequence, guaranteeing
    // true FIFO semantics in the tiebreaker case.
    let row = sqlx::query(
        r#"SELECT id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items
           WHERE status = ?1 AND scheduled_at <= ?2
           ORDER BY scheduled_at ASC, rowid ASC
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

pub async fn claim_next_with_invoke_agent_capacity(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<Option<WorkItem>> {
    Ok(claim_next_with_invoke_agent_capacity_result(pool, capacity)
        .await?
        .item)
}

pub async fn claim_next_with_invoke_agent_capacity_result(
    pool: &SqlitePool,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<CapacityAwareClaimResult> {
    let preflight_now = Utc::now().to_rfc3339();
    let preflight_rows = select_capacity_candidate_rows(pool, &preflight_now).await?;
    if preflight_rows.is_empty() {
        return Ok(CapacityAwareClaimResult {
            item: None,
            all_invoke_agent_candidates_blocked: false,
        });
    }

    let preflight_active = ActiveInvokeAgentCounts::load_from_pool(pool).await?;
    let preflight_run_service_recency = load_run_service_recency_from_pool(pool).await?;
    let preflight_selection = select_capacity_aware_candidate(
        preflight_rows,
        &preflight_active,
        capacity,
        &preflight_run_service_recency,
    )?;
    if preflight_selection.all_invoke_agent_candidates_blocked() {
        return Ok(CapacityAwareClaimResult {
            item: None,
            all_invoke_agent_candidates_blocked: true,
        });
    }

    let mut tx = pool
        .begin()
        .await
        .context("begin capacity-aware claim_next transaction")?;

    let now = Utc::now().to_rfc3339();

    let rows = select_capacity_candidate_rows_tx(&mut tx, &now).await?;
    let active = ActiveInvokeAgentCounts::load(&mut tx).await?;
    let run_service_recency = load_run_service_recency(&mut tx).await?;
    let selection = select_capacity_aware_candidate(rows, &active, capacity, &run_service_recency)?;

    let Some(row) = selection.selected else {
        tx.commit()
            .await
            .context("commit empty capacity-aware claim_next")?;
        return Ok(CapacityAwareClaimResult {
            item: None,
            all_invoke_agent_candidates_blocked: selection.all_invoke_agent_candidates_blocked(),
        });
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
    .context("mark capacity-aware work item running")?;

    upsert_scheduler_claim_state(&mut tx, "global", "", &item_id, &now).await?;
    if let Some(run_id) = row.get::<Option<String>, _>("run_id") {
        upsert_scheduler_claim_state(&mut tx, "run", &run_id, &item_id, &now).await?;
    }

    tx.commit()
        .await
        .context("commit capacity-aware claim_next")?;

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
    Ok(CapacityAwareClaimResult {
        item: Some(item),
        all_invoke_agent_candidates_blocked: false,
    })
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

pub async fn cancel_running_by_run(pool: &SqlitePool, run_id: RunId) -> Result<()> {
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
    .execute(pool)
    .await
    .context("cancel running work items by run")?;
    Ok(())
}

pub async fn cancel_unfinished_by_run(pool: &SqlitePool, run_id: RunId) -> Result<u64> {
    let now = Utc::now().to_rfc3339();
    let cancelled = WorkItemStatus::Cancelled.to_string();
    let pending = WorkItemStatus::Pending.to_string();
    let running = WorkItemStatus::Running.to_string();
    let result = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1,
               completed_at = ?2,
               last_error = COALESCE(last_error, ?3)
           WHERE run_id = ?4 AND status IN (?5, ?6)"#,
    )
    .bind(cancelled)
    .bind(now)
    .bind("cancelled by run cancellation")
    .bind(run_id.to_string())
    .bind(pending)
    .bind(running)
    .execute(pool)
    .await
    .context("cancel unfinished work items by run")?;
    Ok(result.rows_affected())
}

pub async fn cancel_running_invoke_agent_by_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    stage_execution_id: StageExecutionId,
) -> Result<u64> {
    update_running_invoke_agent_by_stage(
        pool,
        run_id,
        stage_id,
        stage_execution_id,
        WorkItemStatus::Cancelled,
        Some(Utc::now()),
        None,
        Some("superseded by retry stage"),
    )
    .await
}

pub async fn requeue_running_invoke_agent_by_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    stage_execution_id: StageExecutionId,
    scheduled_at: DateTime<Utc>,
) -> Result<u64> {
    update_running_invoke_agent_by_stage(
        pool,
        run_id,
        stage_id,
        stage_execution_id,
        WorkItemStatus::Pending,
        None,
        Some(scheduled_at),
        Some("requeued by startup repair"),
    )
    .await
}

pub async fn requeue_running_invoke_agent_by_stage_for_host_interruption(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    stage_execution_id: StageExecutionId,
    scheduled_at: DateTime<Utc>,
) -> Result<u64> {
    update_running_invoke_agent_by_stage(
        pool,
        run_id,
        stage_id,
        stage_execution_id,
        WorkItemStatus::Pending,
        None,
        Some(scheduled_at),
        Some("requeued after host interruption"),
    )
    .await
}

async fn update_running_invoke_agent_by_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    stage_execution_id: StageExecutionId,
    next_status: WorkItemStatus,
    completed_at: Option<DateTime<Utc>>,
    scheduled_at: Option<DateTime<Utc>>,
    last_error: Option<&str>,
) -> Result<u64> {
    let stage_execution_pattern = format!("%{}%", stage_execution_id);
    let result = sqlx::query(
        r#"UPDATE work_items
           SET status = ?1,
               completed_at = ?2,
               scheduled_at = COALESCE(?3, scheduled_at),
               started_at = NULL,
               last_error = ?4
           WHERE run_id = ?5
             AND kind = ?6
             AND status = ?7
             AND (stage_id = ?8 OR payload_json LIKE ?9)"#,
    )
    .bind(next_status.to_string())
    .bind(completed_at.map(|value| value.to_rfc3339()))
    .bind(scheduled_at.map(|value| value.to_rfc3339()))
    .bind(last_error)
    .bind(run_id.to_string())
    .bind(WorkItemKind::InvokeAgent.to_string())
    .bind(WorkItemStatus::Running.to_string())
    .bind(stage_id)
    .bind(stage_execution_pattern)
    .execute(pool)
    .await
    .context("update running InvokeAgent work items by stage")?;
    Ok(result.rows_affected())
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

struct CapacityCandidateSelection {
    selected: Option<sqlx::sqlite::SqliteRow>,
    saw_invoke_agent_candidate: bool,
}

impl CapacityCandidateSelection {
    fn all_invoke_agent_candidates_blocked(&self) -> bool {
        self.saw_invoke_agent_candidate && self.selected.is_none()
    }
}

fn select_capacity_aware_candidate(
    rows: Vec<sqlx::sqlite::SqliteRow>,
    active: &ActiveInvokeAgentCounts,
    capacity: &InvokeAgentCapacityConfig,
    run_service_recency: &std::collections::BTreeMap<String, Option<DateTime<Utc>>>,
) -> Result<CapacityCandidateSelection> {
    let mut selected_non_invoke = None;
    let mut selected_invoke = None;
    let mut saw_invoke_agent_candidate = false;

    for row in rows {
        let kind: String = row.get("kind");
        if kind != WorkItemKind::InvokeAgent.to_string() {
            if selected_invoke.is_none() {
                selected_non_invoke = Some(row);
            }
            break;
        }

        saw_invoke_agent_candidate = true;
        if invoke_agent_candidate_is_eligible(&row, active, capacity)? {
            let candidate = InvokeAgentCandidate::from_row(row, run_service_recency)?;
            let should_select = match selected_invoke.as_ref() {
                Some(selected) => candidate.sorts_before(selected),
                None => true,
            };
            if should_select {
                selected_invoke = Some(candidate);
            }
        }
    }

    Ok(CapacityCandidateSelection {
        selected: selected_invoke
            .map(|candidate: InvokeAgentCandidate| candidate.row)
            .or(selected_non_invoke),
        saw_invoke_agent_candidate,
    })
}

async fn select_capacity_candidate_rows(
    pool: &SqlitePool,
    now: &str,
) -> Result<Vec<sqlx::sqlite::SqliteRow>> {
    let pending_status = WorkItemStatus::Pending.to_string();
    sqlx::query(
        r#"SELECT rowid AS work_rowid, id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items
           WHERE status = ?1 AND scheduled_at <= ?2
           ORDER BY scheduled_at ASC, rowid ASC
           LIMIT ?3"#,
    )
    .bind(&pending_status)
    .bind(now)
    .bind(INVOKE_AGENT_CANDIDATE_WINDOW)
    .fetch_all(pool)
    .await
    .context("select capacity-aware work item candidates")
}

async fn select_capacity_candidate_rows_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    now: &str,
) -> Result<Vec<sqlx::sqlite::SqliteRow>> {
    let pending_status = WorkItemStatus::Pending.to_string();
    sqlx::query(
        r#"SELECT rowid AS work_rowid, id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at, attempt_count, last_error
           FROM work_items
           WHERE status = ?1 AND scheduled_at <= ?2
           ORDER BY scheduled_at ASC, rowid ASC
           LIMIT ?3"#,
    )
    .bind(&pending_status)
    .bind(now)
    .bind(INVOKE_AGENT_CANDIDATE_WINDOW)
    .fetch_all(&mut **tx)
    .await
    .context("select capacity-aware work item candidates")
}

#[derive(Clone, Debug, Default)]
struct ActiveInvokeAgentCounts {
    global: i64,
    by_run: std::collections::BTreeMap<String, i64>,
    by_provider: std::collections::BTreeMap<String, i64>,
}

struct InvokeAgentCandidate {
    row: sqlx::sqlite::SqliteRow,
    last_served_at: Option<DateTime<Utc>>,
    scheduled_at: DateTime<Utc>,
    rowid: i64,
}

impl InvokeAgentCandidate {
    fn from_row(
        row: sqlx::sqlite::SqliteRow,
        run_service_recency: &std::collections::BTreeMap<String, Option<DateTime<Utc>>>,
    ) -> Result<Self> {
        let run_id: Option<String> = row.get("run_id");
        let last_served_at = run_id
            .as_ref()
            .and_then(|run_id| run_service_recency.get(run_id).cloned().flatten());
        let scheduled_at: String = row.get("scheduled_at");
        let scheduled_at = DateTime::parse_from_rfc3339(&scheduled_at)
            .context("parse InvokeAgent candidate scheduled_at")?
            .with_timezone(&Utc);
        let rowid = row.get("work_rowid");

        Ok(Self {
            row,
            last_served_at,
            scheduled_at,
            rowid,
        })
    }

    fn sorts_before(&self, other: &Self) -> bool {
        (self.last_served_at.as_ref(), &self.scheduled_at, self.rowid)
            < (
                other.last_served_at.as_ref(),
                &other.scheduled_at,
                other.rowid,
            )
    }
}

async fn load_run_service_recency(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<std::collections::BTreeMap<String, Option<DateTime<Utc>>>> {
    let rows = sqlx::query(
        r#"SELECT scope_id, last_served_at
           FROM scheduler_service_state
           WHERE scope = 'run'"#,
    )
    .fetch_all(&mut **tx)
    .await
    .context("load scheduler run service recency")?;

    let mut recency = std::collections::BTreeMap::new();
    for row in rows {
        let last_served_at = row
            .get::<Option<String>, _>("last_served_at")
            .map(|raw| {
                DateTime::parse_from_rfc3339(&raw)
                    .context("parse scheduler run last_served_at")
                    .map(|value| value.with_timezone(&Utc))
            })
            .transpose()?;
        recency.insert(row.get("scope_id"), last_served_at);
    }
    Ok(recency)
}

async fn load_run_service_recency_from_pool(
    pool: &SqlitePool,
) -> Result<std::collections::BTreeMap<String, Option<DateTime<Utc>>>> {
    let rows = sqlx::query(
        r#"SELECT scope_id, last_served_at
           FROM scheduler_service_state
           WHERE scope = 'run'"#,
    )
    .fetch_all(pool)
    .await
    .context("load scheduler run service recency")?;

    let mut recency = std::collections::BTreeMap::new();
    for row in rows {
        let last_served_at = row
            .get::<Option<String>, _>("last_served_at")
            .map(|raw| {
                DateTime::parse_from_rfc3339(&raw)
                    .context("parse scheduler run last_served_at")
                    .map(|value| value.with_timezone(&Utc))
            })
            .transpose()?;
        recency.insert(row.get("scope_id"), last_served_at);
    }
    Ok(recency)
}

impl ActiveInvokeAgentCounts {
    async fn load_from_pool(pool: &SqlitePool) -> Result<Self> {
        let mut counts = Self::default();

        let global_agent_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_executions WHERE status = 'running'")
                .fetch_one(pool)
                .await
                .context("count running agent executions")?;
        counts.global = global_agent_count;

        let provider_rows = sqlx::query(
            r#"SELECT COALESCE(provider_family, '') AS provider_family, COUNT(*) AS count
               FROM agent_executions
               WHERE status = 'running'
               GROUP BY COALESCE(provider_family, '')"#,
        )
        .fetch_all(pool)
        .await
        .context("count running agent executions by provider")?;
        for row in provider_rows {
            let provider_family: String = row.get("provider_family");
            if !provider_family.is_empty() {
                counts
                    .by_provider
                    .insert(provider_family, row.get::<i64, _>("count"));
            }
        }

        let run_rows = sqlx::query(
            r#"SELECT se.run_id AS run_id, COUNT(*) AS count
               FROM agent_executions ae
               JOIN stage_executions se ON se.id = ae.stage_execution_id
               WHERE ae.status = 'running'
               GROUP BY se.run_id"#,
        )
        .fetch_all(pool)
        .await
        .context("count running agent executions by run")?;
        for row in run_rows {
            counts
                .by_run
                .insert(row.get("run_id"), row.get::<i64, _>("count"));
        }

        let running_rows = sqlx::query(
            r#"SELECT payload_json, run_id
               FROM work_items
               WHERE kind = 'invoke_agent' AND status = 'running'"#,
        )
        .fetch_all(pool)
        .await
        .context("count running InvokeAgent work items")?;

        let mut running_work_global = 0_i64;
        let mut running_work_by_run = std::collections::BTreeMap::new();
        let mut running_work_by_provider = std::collections::BTreeMap::new();
        for row in running_rows {
            running_work_global += 1;
            if let Some(run_id) = row.get::<Option<String>, _>("run_id") {
                *running_work_by_run.entry(run_id).or_insert(0) += 1;
            }
            let payload_json: String = row.get("payload_json");
            if let Some(provider_family) = provider_family_from_payload(&payload_json)? {
                *running_work_by_provider.entry(provider_family).or_insert(0) += 1;
            }
        }

        counts.global = counts.global.max(running_work_global);
        for (run_id, count) in running_work_by_run {
            let entry = counts.by_run.entry(run_id).or_insert(0);
            *entry = (*entry).max(count);
        }
        for (provider_family, count) in running_work_by_provider {
            let entry = counts.by_provider.entry(provider_family).or_insert(0);
            *entry = (*entry).max(count);
        }

        Ok(counts)
    }

    async fn load(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> Result<Self> {
        let mut counts = Self::default();

        let global_agent_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_executions WHERE status = 'running'")
                .fetch_one(&mut **tx)
                .await
                .context("count running agent executions")?;
        counts.global = global_agent_count;

        let provider_rows = sqlx::query(
            r#"SELECT COALESCE(provider_family, '') AS provider_family, COUNT(*) AS count
               FROM agent_executions
               WHERE status = 'running'
               GROUP BY COALESCE(provider_family, '')"#,
        )
        .fetch_all(&mut **tx)
        .await
        .context("count running agent executions by provider")?;
        for row in provider_rows {
            let provider_family: String = row.get("provider_family");
            if !provider_family.is_empty() {
                counts
                    .by_provider
                    .insert(provider_family, row.get::<i64, _>("count"));
            }
        }

        let run_rows = sqlx::query(
            r#"SELECT se.run_id AS run_id, COUNT(*) AS count
               FROM agent_executions ae
               JOIN stage_executions se ON se.id = ae.stage_execution_id
               WHERE ae.status = 'running'
               GROUP BY se.run_id"#,
        )
        .fetch_all(&mut **tx)
        .await
        .context("count running agent executions by run")?;
        for row in run_rows {
            counts
                .by_run
                .insert(row.get("run_id"), row.get::<i64, _>("count"));
        }

        let running_rows = sqlx::query(
            r#"SELECT payload_json, run_id
               FROM work_items
               WHERE kind = 'invoke_agent' AND status = 'running'"#,
        )
        .fetch_all(&mut **tx)
        .await
        .context("count running InvokeAgent work items")?;

        let mut running_work_global = 0_i64;
        let mut running_work_by_run = std::collections::BTreeMap::new();
        let mut running_work_by_provider = std::collections::BTreeMap::new();
        for row in running_rows {
            running_work_global += 1;
            if let Some(run_id) = row.get::<Option<String>, _>("run_id") {
                *running_work_by_run.entry(run_id).or_insert(0) += 1;
            }
            let payload_json: String = row.get("payload_json");
            if let Some(provider_family) = provider_family_from_payload(&payload_json)? {
                *running_work_by_provider.entry(provider_family).or_insert(0) += 1;
            }
        }

        counts.global = counts.global.max(running_work_global);
        for (run_id, count) in running_work_by_run {
            let entry = counts.by_run.entry(run_id).or_insert(0);
            *entry = (*entry).max(count);
        }
        for (provider_family, count) in running_work_by_provider {
            let entry = counts.by_provider.entry(provider_family).or_insert(0);
            *entry = (*entry).max(count);
        }

        Ok(counts)
    }
}

fn invoke_agent_candidate_is_eligible(
    row: &sqlx::sqlite::SqliteRow,
    active: &ActiveInvokeAgentCounts,
    capacity: &InvokeAgentCapacityConfig,
) -> Result<bool> {
    if active.global >= capacity.global_active_agent_executions as i64 {
        return Ok(false);
    }

    if let Some(run_id) = row.get::<Option<String>, _>("run_id") {
        if active.by_run.get(&run_id).copied().unwrap_or(0)
            >= capacity.per_run_active_agent_executions as i64
        {
            return Ok(false);
        }
    }

    let payload_json: String = row.get("payload_json");
    let Some(provider_family) = provider_family_from_payload(&payload_json)? else {
        return Ok(true);
    };
    let family: ProviderFamily = provider_family.parse()?;
    Ok(active
        .by_provider
        .get(&provider_family)
        .copied()
        .unwrap_or(0)
        < capacity.provider_cap(family) as i64)
}

fn provider_family_from_payload(payload_json: &str) -> Result<Option<String>> {
    let payload: serde_json::Value =
        serde_json::from_str(payload_json).context("parse InvokeAgent work payload")?;
    payload["provider"]
        .as_str()
        .map(ProviderFamily::canonicalize_alias)
        .transpose()
        .map_err(Into::into)
}

async fn upsert_scheduler_claim_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    scope: &str,
    scope_id: &str,
    item_id: &str,
    now: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO scheduler_service_state
           (scope, scope_id, last_served_at, last_claimed_work_item_id, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5)
           ON CONFLICT(scope, scope_id) DO UPDATE SET
             last_served_at = excluded.last_served_at,
             last_claimed_work_item_id = excluded.last_claimed_work_item_id,
             updated_at = excluded.updated_at"#,
    )
    .bind(scope)
    .bind(scope_id)
    .bind(now)
    .bind(item_id)
    .bind(now)
    .execute(&mut **tx)
    .await
    .context("upsert scheduler claim state")?;
    Ok(())
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
