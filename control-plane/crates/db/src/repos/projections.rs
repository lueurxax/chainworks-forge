use std::future::Future;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use domain::ids::RunId;
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use tracing::info;

use super::{artifact_contracts, closeout, code_writer_completion_receipts};
use crate::writer::{
    execute_repository_transaction_operation, repository_transaction_operation, TransactionWork,
};

async fn execute_projection_write(
    pool: &SqlitePool,
    operation_name: &'static str,
    idempotency_key: impl Into<String>,
    work: TransactionWork<()>,
) -> Result<()> {
    let mut op = repository_transaction_operation(operation_name);
    op.idempotency_key = idempotency_key.into();
    op.observed_at = Some(Utc::now().timestamp_millis().max(0) as u64);
    execute_repository_transaction_operation(pool, op, operation_name, work).await
}

const PROJECTION_REBUILD_STACK_BYTES: usize = 16 * 1024 * 1024;

async fn run_projection_rebuild_on_dedicated_stack<F, Fut, T>(
    name: &'static str,
    future: F,
) -> Result<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        let worker = std::thread::Builder::new()
            .name(format!("projection-rebuild-{name}"))
            .stack_size(PROJECTION_REBUILD_STACK_BYTES)
            .spawn(move || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    runtime.block_on(future())
                }))
            })
            .with_context(|| format!("spawn projection rebuild worker for {name}"))?;

        match worker.join() {
            Ok(Ok(result)) => result,
            Ok(Err(payload)) => Err(anyhow!(
                "projection rebuild {name} panicked: {}",
                panic_payload_to_string(payload)
            )),
            Err(payload) => Err(anyhow!(
                "projection rebuild {name} worker panicked: {}",
                panic_payload_to_string(payload)
            )),
        }
    })
    .await
    .with_context(|| format!("join projection rebuild task for {name}"))?
}

fn panic_payload_to_string(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

// ---------------------------------------------------------------------------
// Projection read types (ARCH-002 fix)
// ---------------------------------------------------------------------------

/// A run row produced by joining `runs` with `run_summaries`.
/// Used by the GraphQL list resolvers so reads go through the projection layer.
#[derive(Clone, Debug, Serialize)]
pub struct RunProjectionRow {
    pub id: String,
    pub idea_id: String,
    pub workflow_id: String,
    pub workflow_title: String,
    pub status: String,
    pub workspace_root: String,
    pub artifact_root: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub cancellation_requested_at: Option<String>,
    pub cancellation_settled_at: Option<String>,
    pub cancellation_settlement_summary: Option<String>,
    /// Total stage count from run_summaries (0 if projection not yet built).
    pub total_stages: i64,
    pub completed_stages: i64,
    pub failed_stages: i64,
    pub pending_approvals: i64,
    /// P050: Per-run meta root (read-only, nullable for legacy runs).
    pub chainworks_meta_root: Option<String>,
    pub implementation_self_assessment_summary: Option<serde_json::Value>,
    #[serde(rename = "implementationCompletion")]
    pub implementation_completion: serde_json::Value,
    pub closeout_readiness_summary: Option<serde_json::Value>,
    pub implementation_closeout_readiness_summary: Option<serde_json::Value>,
    pub projection_present: bool,
    pub projection_updated_at: Option<String>,
    pub projection_lag: bool,
}

/// A stage row produced by joining `stage_executions` with `stage_summaries`.
#[derive(Clone, Debug, Serialize)]
pub struct StageSummaryRow {
    pub id: String,
    pub run_id: String,
    pub stage_id: String,
    pub label: String,
    pub status: String,
    pub iteration: i64,
    pub attempt_number: i64,
    pub settlement_kind: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    /// Populated from stage_summaries; false if projection not yet built.
    pub has_artifacts: bool,
    pub has_pending_approval: bool,
    pub has_validation_failure: bool,
    pub terminal_reason: Option<String>,
    pub retry_authority_id: Option<String>,
    pub is_retry_authoritative: bool,
    pub retry_authority_state: Option<String>,
    pub projection_present: bool,
    pub projection_updated_at: Option<String>,
    pub projection_lag: bool,
}

/// List active runs via the projection layer.
///
/// Primary table is `runs`; `run_summaries` is LEFT-JOINed so that runs whose
/// projection hasn't been rebuilt yet are still returned (with zero counts).
/// Status is sourced from canonical `runs`; summary lag is exposed separately.
pub async fn list_active_projection(pool: &SqlitePool) -> Result<Vec<RunProjectionRow>> {
    let rows = sqlx::query(
        r#"WITH canonical_pending AS (
             SELECT run_id, COUNT(*) AS pending_approvals
             FROM approvals
             WHERE decision IN ('pending','requested')
             GROUP BY run_id
           )
           SELECT r.id, r.idea_id, r.workflow_id, r.workflow_title, r.workspace_root,
                  r.artifact_root, r.started_at, r.completed_at,
                  r.cancellation_requested_at, r.cancellation_settled_at,
                  rs.cancellation_settlement_summary,
                  rs.implementation_self_assessment_summary_json,
                  rs.implementation_completion_json,
                  rs.closeout_readiness_summary_json,
                  r.status AS status,
                  COALESCE(rs.total_stages, 0) AS total_stages,
                  COALESCE(rs.completed_stages, 0) AS completed_stages,
                  COALESCE(rs.failed_stages, 0) AS failed_stages,
                  COALESCE(cp.pending_approvals, 0) AS pending_approvals,
                  r.chainworks_meta_root,
                  CASE WHEN rs.run_id IS NULL THEN 0 ELSE 1 END AS projection_present,
                  rs.updated_at AS projection_updated_at,
                  CASE
                    WHEN rs.run_id IS NULL
                      OR rs.status != r.status
                      OR COALESCE(rs.pending_approvals, 0) != COALESCE(cp.pending_approvals, 0)
                    THEN 1 ELSE 0
                  END AS projection_lag
           FROM runs r
           LEFT JOIN run_summaries rs ON rs.run_id = r.id
           LEFT JOIN canonical_pending cp ON cp.run_id = r.id
           WHERE r.status NOT IN ('completed', 'failed', 'cancelled')
           ORDER BY r.started_at DESC"#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(RunProjectionRow {
                id: r.get("id"),
                idea_id: r.get("idea_id"),
                workflow_id: r.get("workflow_id"),
                workflow_title: r.get("workflow_title"),
                status: r.get("status"),
                workspace_root: r.get("workspace_root"),
                artifact_root: r.get("artifact_root"),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                cancellation_requested_at: r.get("cancellation_requested_at"),
                cancellation_settled_at: r.get("cancellation_settled_at"),
                cancellation_settlement_summary: r.get("cancellation_settlement_summary"),
                implementation_self_assessment_summary: parse_optional_json_column(
                    &r,
                    "implementation_self_assessment_summary_json",
                )?,
                implementation_completion: parse_optional_json_column(
                    &r,
                    "implementation_completion_json",
                )?
                .unwrap_or_else(not_attempted_implementation_completion_json),
                closeout_readiness_summary: parse_optional_json_column(
                    &r,
                    "closeout_readiness_summary_json",
                )?,
                implementation_closeout_readiness_summary: parse_optional_json_column(
                    &r,
                    "closeout_readiness_summary_json",
                )?,
                total_stages: r.get("total_stages"),
                completed_stages: r.get("completed_stages"),
                failed_stages: r.get("failed_stages"),
                pending_approvals: r.get("pending_approvals"),
                chainworks_meta_root: r.get("chainworks_meta_root"),
                projection_present: r.get::<i64, _>("projection_present") != 0,
                projection_updated_at: r.get("projection_updated_at"),
                projection_lag: r.get::<i64, _>("projection_lag") != 0,
            })
        })
        .collect()
}

/// List runs for a specific idea via the projection layer.
pub async fn list_by_idea_projection(
    pool: &SqlitePool,
    idea_id: &str,
) -> Result<Vec<RunProjectionRow>> {
    let rows = sqlx::query(
        r#"WITH canonical_pending AS (
             SELECT run_id, COUNT(*) AS pending_approvals
             FROM approvals
             WHERE decision IN ('pending','requested')
             GROUP BY run_id
           )
           SELECT r.id, r.idea_id, r.workflow_id, r.workflow_title, r.workspace_root,
                  r.artifact_root, r.started_at, r.completed_at,
                  r.cancellation_requested_at, r.cancellation_settled_at,
                  rs.cancellation_settlement_summary,
                  rs.implementation_self_assessment_summary_json,
                  rs.implementation_completion_json,
                  rs.closeout_readiness_summary_json,
                  r.status AS status,
                  COALESCE(rs.total_stages, 0) AS total_stages,
                  COALESCE(rs.completed_stages, 0) AS completed_stages,
                  COALESCE(rs.failed_stages, 0) AS failed_stages,
                  COALESCE(cp.pending_approvals, 0) AS pending_approvals,
                  r.chainworks_meta_root,
                  CASE WHEN rs.run_id IS NULL THEN 0 ELSE 1 END AS projection_present,
                  rs.updated_at AS projection_updated_at,
                  CASE
                    WHEN rs.run_id IS NULL
                      OR rs.status != r.status
                      OR COALESCE(rs.pending_approvals, 0) != COALESCE(cp.pending_approvals, 0)
                    THEN 1 ELSE 0
                  END AS projection_lag
           FROM runs r
           LEFT JOIN run_summaries rs ON rs.run_id = r.id
           LEFT JOIN canonical_pending cp ON cp.run_id = r.id
           WHERE r.idea_id = ?
           ORDER BY r.started_at DESC"#,
    )
    .bind(idea_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(RunProjectionRow {
                id: r.get("id"),
                idea_id: r.get("idea_id"),
                workflow_id: r.get("workflow_id"),
                workflow_title: r.get("workflow_title"),
                status: r.get("status"),
                workspace_root: r.get("workspace_root"),
                artifact_root: r.get("artifact_root"),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                cancellation_requested_at: r.get("cancellation_requested_at"),
                cancellation_settled_at: r.get("cancellation_settled_at"),
                cancellation_settlement_summary: r.get("cancellation_settlement_summary"),
                implementation_self_assessment_summary: parse_optional_json_column(
                    &r,
                    "implementation_self_assessment_summary_json",
                )?,
                implementation_completion: parse_optional_json_column(
                    &r,
                    "implementation_completion_json",
                )?
                .unwrap_or_else(not_attempted_implementation_completion_json),
                closeout_readiness_summary: parse_optional_json_column(
                    &r,
                    "closeout_readiness_summary_json",
                )?,
                implementation_closeout_readiness_summary: parse_optional_json_column(
                    &r,
                    "closeout_readiness_summary_json",
                )?,
                total_stages: r.get("total_stages"),
                completed_stages: r.get("completed_stages"),
                failed_stages: r.get("failed_stages"),
                pending_approvals: r.get("pending_approvals"),
                chainworks_meta_root: r.get("chainworks_meta_root"),
                projection_present: r.get::<i64, _>("projection_present") != 0,
                projection_updated_at: r.get("projection_updated_at"),
                projection_lag: r.get::<i64, _>("projection_lag") != 0,
            })
        })
        .collect()
}

/// List stages for a run via the projection layer.
///
/// Primary table is `stage_executions`; `stage_summaries` is LEFT-JOINed so
/// that stages whose projection hasn't been rebuilt yet are still returned.
pub async fn list_stages_projection(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<StageSummaryRow>> {
    let rows = sqlx::query(
        r#"SELECT se.id, se.run_id, se.stage_id, se.label, se.iteration, se.settlement_kind,
                  se.started_at, se.completed_at,
                  COALESCE(ss.status, se.status) AS status,
                  COALESCE(ss.attempt_number, se.attempt_number) AS attempt_number,
                  COALESCE(ss.has_artifacts, 0) AS has_artifacts,
                  COALESCE(ss.has_pending_approval, 0) AS has_pending_approval,
                  COALESCE(ss.has_validation_failure, 0) AS has_validation_failure,
                  COALESCE(ss.terminal_reason, se.terminal_reason) AS terminal_reason,
                  ss.retry_authority_id,
                  COALESCE(ss.is_retry_authoritative, 0) AS is_retry_authoritative,
                  ss.retry_authority_state,
                  CASE WHEN ss.stage_execution_id IS NULL THEN 0 ELSE 1 END AS projection_present,
                  ss.updated_at AS projection_updated_at,
                  CASE WHEN ss.stage_execution_id IS NULL OR ss.status != se.status OR ss.attempt_number != se.attempt_number THEN 1 ELSE 0 END AS projection_lag
           FROM stage_executions se
           LEFT JOIN stage_summaries ss ON ss.stage_execution_id = se.id
           WHERE se.run_id = ?
           ORDER BY se.started_at ASC"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(StageSummaryRow {
                id: r.get("id"),
                run_id: r.get("run_id"),
                stage_id: r.get("stage_id"),
                label: r.get("label"),
                status: r.get("status"),
                iteration: r.get("iteration"),
                attempt_number: r.get("attempt_number"),
                settlement_kind: r.get("settlement_kind"),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                has_artifacts: r.get::<i64, _>("has_artifacts") != 0,
                has_pending_approval: r.get::<i64, _>("has_pending_approval") != 0,
                has_validation_failure: r.get::<i64, _>("has_validation_failure") != 0,
                terminal_reason: r.get("terminal_reason"),
                retry_authority_id: r.get("retry_authority_id"),
                is_retry_authoritative: r.get::<i64, _>("is_retry_authoritative") != 0,
                retry_authority_state: r.get("retry_authority_state"),
                projection_present: r.get::<i64, _>("projection_present") != 0,
                projection_updated_at: r.get("projection_updated_at"),
                projection_lag: r.get::<i64, _>("projection_lag") != 0,
            })
        })
        .collect()
}

/// Find a single run via the projection layer.
///
/// LEFT-JOINs `run_summaries` so that a run whose projection hasn't been
/// rebuilt yet is still returned (with zero counts). Returns `None` only if
/// the run does not exist in the canonical `runs` table.
pub async fn find_run_projection(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<RunProjectionRow>> {
    let row = sqlx::query(
        r#"WITH canonical_pending AS (
             SELECT run_id, COUNT(*) AS pending_approvals
             FROM approvals
             WHERE decision IN ('pending','requested')
             GROUP BY run_id
           )
           SELECT r.id, r.idea_id, r.workflow_id, r.workflow_title, r.workspace_root,
                  r.artifact_root, r.started_at, r.completed_at,
                  r.cancellation_requested_at, r.cancellation_settled_at,
                  rs.cancellation_settlement_summary,
                  rs.implementation_self_assessment_summary_json,
                  rs.implementation_completion_json,
                  rs.closeout_readiness_summary_json,
                  r.status AS status,
                  COALESCE(rs.total_stages, 0) AS total_stages,
                  COALESCE(rs.completed_stages, 0) AS completed_stages,
                  COALESCE(rs.failed_stages, 0) AS failed_stages,
                  COALESCE(cp.pending_approvals, 0) AS pending_approvals,
                  r.chainworks_meta_root,
                  CASE WHEN rs.run_id IS NULL THEN 0 ELSE 1 END AS projection_present,
                  rs.updated_at AS projection_updated_at,
                  CASE
                    WHEN rs.run_id IS NULL
                      OR rs.status != r.status
                      OR COALESCE(rs.pending_approvals, 0) != COALESCE(cp.pending_approvals, 0)
                    THEN 1 ELSE 0
                  END AS projection_lag
           FROM runs r
           LEFT JOIN run_summaries rs ON rs.run_id = r.id
           LEFT JOIN canonical_pending cp ON cp.run_id = r.id
           WHERE r.id = ?"#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await?;

    row.map(|r| {
        Ok(RunProjectionRow {
            id: r.get("id"),
            idea_id: r.get("idea_id"),
            workflow_id: r.get("workflow_id"),
            workflow_title: r.get("workflow_title"),
            status: r.get("status"),
            workspace_root: r.get("workspace_root"),
            artifact_root: r.get("artifact_root"),
            started_at: r.get("started_at"),
            completed_at: r.get("completed_at"),
            cancellation_requested_at: r.get("cancellation_requested_at"),
            cancellation_settled_at: r.get("cancellation_settled_at"),
            cancellation_settlement_summary: r.get("cancellation_settlement_summary"),
            implementation_self_assessment_summary: parse_optional_json_column(
                &r,
                "implementation_self_assessment_summary_json",
            )?,
            implementation_completion: parse_optional_json_column(
                &r,
                "implementation_completion_json",
            )?
            .unwrap_or_else(not_attempted_implementation_completion_json),
            closeout_readiness_summary: parse_optional_json_column(
                &r,
                "closeout_readiness_summary_json",
            )?,
            implementation_closeout_readiness_summary: parse_optional_json_column(
                &r,
                "closeout_readiness_summary_json",
            )?,
            total_stages: r.get("total_stages"),
            completed_stages: r.get("completed_stages"),
            failed_stages: r.get("failed_stages"),
            pending_approvals: r.get("pending_approvals"),
            chainworks_meta_root: r.get("chainworks_meta_root"),
            projection_present: r.get::<i64, _>("projection_present") != 0,
            projection_updated_at: r.get("projection_updated_at"),
            projection_lag: r.get::<i64, _>("projection_lag") != 0,
        })
    })
    .transpose()
}

fn parse_optional_json_column(
    row: &sqlx::sqlite::SqliteRow,
    column: &str,
) -> Result<Option<serde_json::Value>> {
    let raw: Option<String> = row
        .try_get(column)
        .with_context(|| format!("read run projection JSON column {column}"))?;
    raw.map(|value| {
        serde_json::from_str(&value)
            .with_context(|| format!("parse run projection JSON column {column}"))
    })
    .transpose()
}

fn not_attempted_implementation_completion_json() -> serde_json::Value {
    serde_json::to_value(domain::code_writer_completion::project_implementation_completion(&[]))
        .unwrap_or(serde_json::Value::Null)
}

/// Rebuild run_summary for a single run from canonical tables.
pub async fn rebuild_run_summary(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let pool = pool.clone();
    run_projection_rebuild_on_dedicated_stack("run-summary", move || async move {
        rebuild_run_summary_on_current_thread(&pool, run_id).await
    })
    .await
}

async fn rebuild_run_summary_on_current_thread(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let run_id_string = run_id.to_string();

    // Upsert run_summaries by computing counts from canonical tables
    execute_projection_write(
        pool,
        "projections.rebuild_run_summary",
        format!("run:{run_id_string}:projection:run_summary:upsert"),
        {
            let now = now.clone();
            let run_id_string = run_id_string.clone();
            Box::new(move |tx| {
                Box::pin(async move {
                let rows = sqlx::query(
                    r#"INSERT OR REPLACE INTO run_summaries
                       (run_id, idea_id, workflow_title, status, total_stages, completed_stages, failed_stages, pending_approvals, started_at, updated_at)
                       SELECT
                         r.id,
                         r.idea_id,
                         r.workflow_title,
                         r.status,
                         COUNT(DISTINCT se.id),
                         COUNT(DISTINCT CASE WHEN se.status = 'completed' THEN se.id END),
                         COUNT(DISTINCT CASE WHEN se.status = 'failed' THEN se.id END),
                         COUNT(DISTINCT CASE WHEN a.decision IN ('pending','requested') THEN a.id END),
                         r.started_at,
                         ?
                       FROM runs r
                       LEFT JOIN stage_executions se ON se.run_id = r.id
                       LEFT JOIN approvals a ON a.run_id = r.id
                       WHERE r.id = ?
                       GROUP BY r.id"#,
                )
                .bind(now)
                .bind(run_id_string)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;
                Ok(((), rows))
                })
            })
        },
    )
    .await?;

    let settlement_summary = sqlx::query_scalar::<_, Option<String>>(
        "SELECT cancellation_settlement_log FROM runs WHERE id = ?",
    )
    .bind(&run_id_string)
    .fetch_one(pool)
    .await?
    .and_then(|log| build_cancellation_settlement_summary(&log));

    execute_projection_write(
        pool,
        "projections.rebuild_run_summary",
        format!("run:{run_id_string}:projection:run_summary:cancellation"),
        {
            let run_id_string = run_id_string.clone();
            Box::new(move |tx| {
                Box::pin(async move {
                let rows = sqlx::query(
                    "UPDATE run_summaries SET cancellation_settlement_summary = ?1 WHERE run_id = ?2",
                )
                .bind(settlement_summary)
                .bind(run_id_string)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;
                Ok(((), rows))
                })
            })
        },
    )
    .await?;

    let implementation_self_assessment_summary_json =
        artifact_contracts::find_active_implementation_self_assessment_summary(pool, run_id)
            .await?
            .map(|stored| serde_json::to_string(&stored.summary))
            .transpose()?;
    let canonical_receipts =
        code_writer_completion_receipts::list_canonical_by_run(pool, run_id).await?;
    let implementation_completion_json = serde_json::to_string(
        &domain::code_writer_completion::project_implementation_completion(&canonical_receipts),
    )?;
    let closeout_readiness_summary_json =
        closeout::load_closeout_readiness_summary(pool, &run_id_string)
            .await?
            .map(|summary| serde_json::to_string(&summary))
            .transpose()?;

    execute_projection_write(
        pool,
        "projections.rebuild_run_summary",
        format!("run:{run_id_string}:projection:run_summary:hot_read_payloads"),
        {
            let run_id_string = run_id_string.clone();
            Box::new(move |tx| {
                Box::pin(async move {
                    let rows = sqlx::query(
                        r#"UPDATE run_summaries
                           SET implementation_self_assessment_summary_json = ?1,
                               implementation_completion_json = ?2,
                               closeout_readiness_summary_json = ?3
                           WHERE run_id = ?4"#,
                    )
                    .bind(implementation_self_assessment_summary_json)
                    .bind(implementation_completion_json)
                    .bind(closeout_readiness_summary_json)
                    .bind(run_id_string)
                    .execute(&mut **tx)
                    .await?
                    .rows_affected() as u32;
                    Ok(((), rows))
                })
            })
        },
    )
    .await?;

    info!(run_id = %run_id, "Rebuilt run_summary projection");
    Ok(())
}

fn build_cancellation_settlement_summary(raw: &str) -> Option<String> {
    let entries = serde_json::from_str::<Vec<serde_json::Value>>(raw).ok()?;
    if entries.is_empty() {
        return None;
    }

    let settled_count = entries
        .iter()
        .filter(|entry| entry.get("terminal_status").and_then(|v| v.as_str()) == Some("cancelled"))
        .count();
    let total_count = entries.len();
    let close_ok = entries
        .iter()
        .filter(|entry| {
            entry
                .get("session_close_succeeded")
                .and_then(|v| v.as_bool())
                == Some(true)
        })
        .count();

    Some(format!(
        "{settled_count}/{total_count} agents settled, {close_ok} sessions closed"
    ))
}

/// Rebuild stage_summaries for all stages in a run.
pub async fn rebuild_stage_summaries(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let pool = pool.clone();
    run_projection_rebuild_on_dedicated_stack("stage-summaries", move || async move {
        rebuild_stage_summaries_on_current_thread(&pool, run_id).await
    })
    .await
}

async fn rebuild_stage_summaries_on_current_thread(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let run_id_string = run_id.to_string();

    execute_projection_write(
        pool,
        "projections.rebuild_stage_summaries",
        format!("run:{run_id_string}:projection:stage_summaries"),
        Box::new(move |tx| {
            Box::pin(async move {
            let rows = sqlx::query(
                r#"INSERT OR REPLACE INTO stage_summaries
                   (stage_execution_id, run_id, stage_id, label, status, attempt_number,
                    has_artifacts, has_pending_approval, has_validation_failure,
                    terminal_reason, retry_authority_id, is_retry_authoritative,
                    retry_authority_state, updated_at)
                   SELECT
                     se.id,
                     se.run_id,
                     se.stage_id,
                     se.label,
                     se.status,
                     se.attempt_number,
                     EXISTS(SELECT 1 FROM artifacts art WHERE art.run_id = se.run_id AND art.stage_id = se.stage_id),
                     EXISTS(SELECT 1 FROM approvals ap WHERE ap.run_id = se.run_id AND ap.stage_id = se.stage_id AND ap.decision IN ('pending','requested')),
                     EXISTS(SELECT 1 FROM validation_failure_records vfr WHERE vfr.run_id = se.run_id AND vfr.stage_execution_id = se.id),
                     se.terminal_reason,
                     rsa.id,
                     CASE WHEN rsa.id IS NULL THEN 0 ELSE 1 END,
                     rsa.authority_state,
                     ?
                   FROM stage_executions se
                   LEFT JOIN retry_stage_execution_authorities rsa
                     ON rsa.target_stage_execution_id = se.id
                    AND rsa.authority_state = 'active'
                   WHERE se.run_id = ?"#,
            )
            .bind(now)
            .bind(run_id_string)
            .execute(&mut **tx)
            .await?
            .rows_affected() as u32;
            Ok(((), rows))
            })
        }),
    )
    .await?;

    Ok(())
}

/// Rebuild approval_inbox for a run.
pub async fn rebuild_approval_inbox(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let pool = pool.clone();
    run_projection_rebuild_on_dedicated_stack("approval-inbox", move || async move {
        rebuild_approval_inbox_on_current_thread(&pool, run_id).await
    })
    .await
}

async fn rebuild_approval_inbox_on_current_thread(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let run_id_string = run_id.to_string();

    execute_projection_write(
        pool,
        "projections.rebuild_approval_inbox",
        format!("run:{run_id_string}:projection:approval_inbox"),
        Box::new(move |tx| {
            Box::pin(async move {
            let mut rows = sqlx::query("DELETE FROM approval_inbox WHERE run_id = ?")
                .bind(&run_id_string)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;
            rows += sqlx::query(
                r#"INSERT OR IGNORE INTO approval_inbox
                   (approval_id, run_id, stage_id, workflow_title, requested_at, expires_at, updated_at)
                   SELECT
                     a.id,
                     a.run_id,
                     a.stage_id,
                     r.workflow_title,
                     a.requested_at,
                     a.expires_at,
                     ?
                   FROM approvals a
                   JOIN runs r ON r.id = a.run_id
                   WHERE a.run_id = ? AND a.decision IN ('pending','requested')"#,
            )
            .bind(now)
            .bind(run_id_string)
            .execute(&mut **tx)
            .await?
            .rows_affected() as u32;
            Ok(((), rows))
            })
        }),
    )
    .await?;

    Ok(())
}

/// Rebuild artifact_index entries for new artifacts in a run.
pub async fn upsert_artifact_index_entry(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let pool = pool.clone();
    run_projection_rebuild_on_dedicated_stack("artifact-index", move || async move {
        upsert_artifact_index_entry_on_current_thread(&pool, run_id).await
    })
    .await
}

async fn upsert_artifact_index_entry_on_current_thread(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<()> {
    let run_id_string = run_id.to_string();
    execute_projection_write(
        pool,
        "projections.upsert_artifact_index_entry",
        format!("run:{run_id_string}:projection:artifact_index"),
        Box::new(move |tx| {
            Box::pin(async move {
            let mut rows = sqlx::query(
                r#"INSERT OR IGNORE INTO artifact_index
                   (artifact_id, run_id, stage_id, name, format, file_path, is_pinned, report_kind, created_at)
                   SELECT id, run_id, stage_id, name, format, file_path, is_pinned, report_kind, created_at
                   FROM artifacts WHERE run_id = ?"#,
            )
            .bind(&run_id_string)
            .execute(&mut **tx)
            .await?
            .rows_affected() as u32;
            rows += sqlx::query(
                "UPDATE artifact_index SET is_pinned = (SELECT is_pinned FROM artifacts WHERE artifacts.id = artifact_index.artifact_id) WHERE run_id = ?",
            )
            .bind(run_id_string)
            .execute(&mut **tx)
            .await?
            .rows_affected() as u32;
            Ok(((), rows))
            })
        }),
    )
    .await?;
    Ok(())
}

/// Query the approval inbox via the projection layer.
///
/// Reads from `approval_inbox` (projection) and JOINs with `approvals` for
/// full field set. Only pending/requested approvals appear in the inbox.
pub async fn list_pending_inbox_projection(pool: &SqlitePool) -> Result<Vec<ApprovalInboxRow>> {
    let rows = sqlx::query(
        r#"SELECT a.id, a.run_id, a.stage_id, a.decision, a.requested_at, a.decided_at,
                  a.comment, a.expires_at
           FROM approval_inbox ai
           JOIN approvals a ON a.id = ai.approval_id
           ORDER BY ai.requested_at ASC"#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(ApprovalInboxRow {
                id: r.get("id"),
                run_id: r.get("run_id"),
                stage_id: r.get("stage_id"),
                decision: r.get("decision"),
                requested_at: r.get("requested_at"),
                decided_at: r.get("decided_at"),
                comment: r.get("comment"),
                expires_at: r.get("expires_at"),
            })
        })
        .collect()
}

/// Query artifacts for a run via the projection layer.
///
/// Reads from `artifact_index` (projection) and JOINs with `artifacts` for
/// full field set. The projection's `is_pinned` is authoritative.
pub async fn list_artifacts_projection(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Vec<ArtifactIndexRow>> {
    let rows = sqlx::query(
        r#"SELECT ai.artifact_id AS id, ai.run_id, ai.stage_id, a.agent_id, ai.name,
                  a.contract_id, ai.format, ai.file_path, a.checksum_sha256, a.size_bytes,
                  a.provider, a.model, ai.created_at, ai.is_pinned, ai.report_kind,
                  a.report_version, g.generation_id AS artifact_generation_id,
                  g.source_agent_execution_id, g.source_stage_execution_id,
                  g.source_session_generation_id, g.source_work_item_id,
                  g.supersedes_generation_id AS supersedes_artifact_generation_id,
                  g.output_settlement, g.source_generation_verified
           FROM artifact_index ai
           JOIN artifacts a ON a.id = ai.artifact_id
           LEFT JOIN artifact_contract_generations g ON g.artifact_id = ai.artifact_id
           WHERE ai.run_id = ?
           ORDER BY ai.created_at ASC"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(ArtifactIndexRow {
                id: r.get("id"),
                run_id: r.get("run_id"),
                stage_id: r.get("stage_id"),
                agent_id: r.get("agent_id"),
                name: r.get("name"),
                contract_id: r.get("contract_id"),
                format: r.get("format"),
                file_path: r.get("file_path"),
                checksum_sha256: r.get("checksum_sha256"),
                size_bytes: r.get("size_bytes"),
                provider: r.get("provider"),
                model: r.get("model"),
                created_at: r.get("created_at"),
                is_pinned: r.get::<i64, _>("is_pinned") != 0,
                report_kind: r.get("report_kind"),
                report_version: r.get("report_version"),
                artifact_generation_id: r.get("artifact_generation_id"),
                source_agent_execution_id: r.get("source_agent_execution_id"),
                source_stage_execution_id: r.get("source_stage_execution_id"),
                source_session_generation_id: r.get("source_session_generation_id"),
                source_work_item_id: r.get("source_work_item_id"),
                supersedes_artifact_generation_id: r.get("supersedes_artifact_generation_id"),
                output_settlement: r.get("output_settlement"),
                source_generation_verified: r
                    .get::<Option<i64>, _>("source_generation_verified")
                    .map(|value| value != 0),
            })
        })
        .collect()
}

/// A row from the approval_inbox projection (pending approvals only).
#[derive(Clone, Debug, Serialize)]
pub struct ApprovalInboxRow {
    pub id: String,
    pub run_id: String,
    pub stage_id: String,
    pub decision: String,
    pub requested_at: String,
    pub decided_at: Option<String>,
    pub comment: Option<String>,
    pub expires_at: Option<String>,
}

/// A row from the artifact_index projection.
#[derive(Clone, Debug, Serialize)]
pub struct ArtifactIndexRow {
    pub id: String,
    pub run_id: String,
    pub stage_id: String,
    pub agent_id: String,
    pub name: String,
    pub contract_id: String,
    pub format: String,
    pub file_path: String,
    pub checksum_sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub provider: String,
    pub model: Option<String>,
    pub created_at: String,
    pub is_pinned: bool,
    pub report_kind: Option<String>,
    pub report_version: Option<i64>,
    pub artifact_generation_id: Option<String>,
    pub source_agent_execution_id: Option<String>,
    pub source_stage_execution_id: Option<String>,
    pub source_session_generation_id: Option<String>,
    pub source_work_item_id: Option<String>,
    pub supersedes_artifact_generation_id: Option<String>,
    pub output_settlement: Option<String>,
    pub source_generation_verified: Option<bool>,
}

/// Full projection rebuild for a run (all tables).
pub async fn rebuild_all_for_run(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let pool = pool.clone();
    run_projection_rebuild_on_dedicated_stack("all-for-run", move || async move {
        rebuild_all_for_run_on_current_thread(&pool, run_id).await
    })
    .await
}

async fn rebuild_all_for_run_on_current_thread(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    rebuild_run_summary_on_current_thread(pool, run_id).await?;
    rebuild_stage_summaries_on_current_thread(pool, run_id).await?;
    rebuild_approval_inbox_on_current_thread(pool, run_id).await?;
    upsert_artifact_index_entry_on_current_thread(pool, run_id).await?;
    artifact_contracts::rebuild_projection_and_exports(pool, run_id).await?;
    info!(run_id = %run_id, "Full projection rebuild complete");
    Ok(())
}
