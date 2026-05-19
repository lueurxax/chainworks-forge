use std::future::Future;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use domain::ids::RunId;
use serde::Serialize;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};
use tracing::info;

use super::artifact_contracts;
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
    let start = std::time::Instant::now();
    let res = tokio::task::spawn_blocking(move || {
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
    .with_context(|| format!("join projection rebuild task for {name}"))?;

    crate::metrics::record_projection_rebuild(name, start.elapsed());
    res
}

/// P087: Trigger projection invalidation for a terminal run state change.
/// Records the invalidation in the log and freezes the cursor watermark.
///
/// A full projection backlog (ProjectionInvalidationThrottle) is absorbed here — the
/// canonical terminal write must commit regardless of backlog state. The projection
/// will remain stale (cursor poisoned, freshness = stale) until the backlog drains.
pub async fn invalidate_projections_terminal(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let run_id_str = run_id.to_string();

    // 1. Record invalidation for all projections watching 'runs'.
    // P087: absorb ProjectionInvalidationThrottle — a full backlog must not abort
    // the canonical terminal status update. The projection degrades gracefully.
    let invalidation_result = crate::repos::projection_invalidation::record_invalidation_internal(
        &mut **tx,
        "run_summaries",
        "runs",
        &run_id_str,
        "upsert",
        None,
    )
    .await;

    if let Err(ref e) = invalidation_result {
        if e.downcast_ref::<crate::repos::projection_invalidation::ProjectionInvalidationThrottle>()
            .is_some()
        {
            crate::metrics::increment_counter(
                "projection_invalidation_backlog_exceeded_terminal_total",
            );
            tracing::warn!(
                event = "projection_invalidation_backlog_exceeded_terminal",
                run_id = %run_id,
                "Projection backlog full at terminal transition; canonical write proceeds, projection stays stale"
            );
        } else {
            invalidation_result?;
        }
    }

    // 2. Cursor watermark: record position so background rebuilder can resume correctly.
    // Set first_healthy_at_ms on first healthy advance (COALESCE keeps the earliest timestamp).
    // A cursor that becomes poisoned will have first_healthy_at_ms reset to NULL there.
    sqlx::query(
        "INSERT INTO projection_cursors (projection_name, source_name, watermark_ms, updated_at_ms, first_healthy_at_ms)
         VALUES ('run_summaries', 'runs', ?, ?, ?)
         ON CONFLICT(projection_name, source_name) DO UPDATE SET
            watermark_ms = excluded.watermark_ms,
            updated_at_ms = excluded.updated_at_ms,
            first_healthy_at_ms = CASE WHEN is_poisoned = 0 THEN COALESCE(projection_cursors.first_healthy_at_ms, excluded.first_healthy_at_ms) ELSE NULL END",
    )
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    tracing::debug!(
        event = "projection_invalidation_terminal",
        run_id = %run_id,
        "Registered terminal projection invalidation"
    );

    Ok(())
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

fn parse_projection_json(raw: Option<String>) -> Result<Option<serde_json::Value>> {
    raw.map(|raw| serde_json::from_str(&raw).context("parse run summary projection JSON"))
        .transpose()
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
    pub projection_present: bool,
    pub projection_updated_at: Option<String>,
    pub projection_lag: bool,
    /// P087/P088: compact projection-backed list readback, not per-row detail loading.
    #[serde(rename = "implementationCompletion")]
    pub implementation_completion: Option<serde_json::Value>,
    /// P087/P077: compact projection-backed list readback, not per-row detail loading.
    pub closeout_readiness_summary: Option<serde_json::Value>,
    /// P077 documented alias for closeout_readiness_summary.
    pub implementation_closeout_readiness_summary: Option<serde_json::Value>,
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

/// List all runs (active and terminal) via the projection layer.
///
/// Primary table is `runs`; `run_summaries` is LEFT-JOINed so that runs whose
/// projection hasn't been rebuilt yet are still returned (with zero counts).
/// Status is sourced from canonical `runs`; summary lag is exposed separately.
/// Used by `runs.list` MCP tool — returns all runs so operators have full history.
pub async fn list_all_projection(pool: &SqlitePool) -> Result<Vec<RunProjectionRow>> {
    let rows = sqlx::query(
        r#"WITH live_approvals AS (
             SELECT run_id, COUNT(*) AS count
             FROM approvals
             WHERE decision IN ('pending', 'requested')
             GROUP BY run_id
           )
           SELECT r.id, r.idea_id,
                  COALESCE(rs.workflow_id, r.workflow_id) AS workflow_id,
                  COALESCE(rs.workflow_title, r.workflow_title) AS workflow_title,
                  COALESCE(rs.workspace_root, r.workspace_root) AS workspace_root,
                  COALESCE(rs.artifact_root, r.artifact_root) AS artifact_root,
                  r.started_at,
                  COALESCE(rs.completed_at, r.completed_at) AS completed_at,
                  COALESCE(rs.cancellation_requested_at, r.cancellation_requested_at) AS cancellation_requested_at,
                  COALESCE(rs.cancellation_settled_at, r.cancellation_settled_at) AS cancellation_settled_at,
                  rs.cancellation_settlement_summary,
                  rs.implementation_completion_json,
                  rs.closeout_readiness_summary_json,
                  r.status,
                  COALESCE(rs.total_stages, 0) AS total_stages,
                  COALESCE(rs.completed_stages, 0) AS completed_stages,
                  COALESCE(rs.failed_stages, 0) AS failed_stages,
                  COALESCE(la.count, 0) AS pending_approvals,
                  COALESCE(rs.chainworks_meta_root, r.chainworks_meta_root) AS chainworks_meta_root,
                  CASE WHEN rs.run_id IS NULL THEN 0 ELSE 1 END AS projection_present,
                  rs.updated_at AS projection_updated_at,
                  CASE WHEN rs.run_id IS NULL
                            OR rs.status != r.status
                            OR COALESCE(rs.pending_approvals, 0) != COALESCE(la.count, 0)
                       THEN 1 ELSE 0 END AS projection_lag
           FROM runs r
           LEFT JOIN run_summaries rs ON rs.run_id = r.id
           LEFT JOIN live_approvals la ON la.run_id = r.id
           ORDER BY r.started_at DESC"#,
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(RunProjectionRow {
                id: r.get("id"),
                idea_id: r.get("idea_id"),
                workflow_id: r
                    .get::<Option<String>, _>("workflow_id")
                    .unwrap_or_default(),
                workflow_title: r.get("workflow_title"),
                status: r.get("status"),
                workspace_root: r
                    .get::<Option<String>, _>("workspace_root")
                    .unwrap_or_default(),
                artifact_root: r
                    .get::<Option<String>, _>("artifact_root")
                    .unwrap_or_default(),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                cancellation_requested_at: r.get("cancellation_requested_at"),
                cancellation_settled_at: r.get("cancellation_settled_at"),
                cancellation_settlement_summary: r.get("cancellation_settlement_summary"),
                total_stages: r.get("total_stages"),
                completed_stages: r.get("completed_stages"),
                failed_stages: r.get("failed_stages"),
                pending_approvals: r.get("pending_approvals"),
                chainworks_meta_root: r.get("chainworks_meta_root"),
                projection_present: r.get::<i64, _>("projection_present") != 0,
                projection_updated_at: r.get("projection_updated_at"),
                projection_lag: r.get::<i64, _>("projection_lag") != 0,
                implementation_completion: parse_projection_json(
                    r.get("implementation_completion_json"),
                )?,
                closeout_readiness_summary: parse_projection_json(
                    r.get("closeout_readiness_summary_json"),
                )?,
                implementation_closeout_readiness_summary: parse_projection_json(
                    r.get("closeout_readiness_summary_json"),
                )?,
            })
        })
        .collect()
}

/// List active (non-terminal) runs via the projection layer.
///
/// Primary table is `runs`; `run_summaries` is LEFT-JOINed so that runs whose
/// projection hasn't been rebuilt yet are still returned (with zero counts).
/// Status is sourced from canonical `runs`; summary lag is exposed separately.
pub async fn list_active_projection(pool: &SqlitePool) -> Result<Vec<RunProjectionRow>> {
    let rows = sqlx::query(
        r#"WITH live_approvals AS (
             SELECT run_id, COUNT(*) AS count
             FROM approvals
             WHERE decision IN ('pending', 'requested')
             GROUP BY run_id
           )
           SELECT r.id, r.idea_id,
                  COALESCE(rs.workflow_id, r.workflow_id) AS workflow_id,
                  COALESCE(rs.workflow_title, r.workflow_title) AS workflow_title,
                  COALESCE(rs.workspace_root, r.workspace_root) AS workspace_root,
                  COALESCE(rs.artifact_root, r.artifact_root) AS artifact_root,
                  r.started_at,
                  COALESCE(rs.completed_at, r.completed_at) AS completed_at,
                  COALESCE(rs.cancellation_requested_at, r.cancellation_requested_at) AS cancellation_requested_at,
                  COALESCE(rs.cancellation_settled_at, r.cancellation_settled_at) AS cancellation_settled_at,
                  rs.cancellation_settlement_summary,
                  rs.implementation_completion_json,
                  rs.closeout_readiness_summary_json,
                  r.status,
                  COALESCE(rs.total_stages, 0) AS total_stages,
                  COALESCE(rs.completed_stages, 0) AS completed_stages,
                  COALESCE(rs.failed_stages, 0) AS failed_stages,
                  COALESCE(la.count, 0) AS pending_approvals,
                  COALESCE(rs.chainworks_meta_root, r.chainworks_meta_root) AS chainworks_meta_root,
                  CASE WHEN rs.run_id IS NULL THEN 0 ELSE 1 END AS projection_present,
                  rs.updated_at AS projection_updated_at,
                  CASE WHEN rs.run_id IS NULL
                            OR rs.status != r.status
                            OR COALESCE(rs.pending_approvals, 0) != COALESCE(la.count, 0)
                       THEN 1 ELSE 0 END AS projection_lag
           FROM runs r
           LEFT JOIN run_summaries rs ON rs.run_id = r.id
           LEFT JOIN live_approvals la ON la.run_id = r.id
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
                workflow_id: r
                    .get::<Option<String>, _>("workflow_id")
                    .unwrap_or_default(),
                workflow_title: r.get("workflow_title"),
                status: r.get("status"),
                workspace_root: r
                    .get::<Option<String>, _>("workspace_root")
                    .unwrap_or_default(),
                artifact_root: r
                    .get::<Option<String>, _>("artifact_root")
                    .unwrap_or_default(),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                cancellation_requested_at: r.get("cancellation_requested_at"),
                cancellation_settled_at: r.get("cancellation_settled_at"),
                cancellation_settlement_summary: r.get("cancellation_settlement_summary"),
                total_stages: r.get("total_stages"),
                completed_stages: r.get("completed_stages"),
                failed_stages: r.get("failed_stages"),
                pending_approvals: r.get("pending_approvals"),
                chainworks_meta_root: r.get("chainworks_meta_root"),
                projection_present: r.get::<i64, _>("projection_present") != 0,
                projection_updated_at: r.get("projection_updated_at"),
                projection_lag: r.get::<i64, _>("projection_lag") != 0,
                implementation_completion: parse_projection_json(
                    r.get("implementation_completion_json"),
                )?,
                closeout_readiness_summary: parse_projection_json(
                    r.get("closeout_readiness_summary_json"),
                )?,
                implementation_closeout_readiness_summary: parse_projection_json(
                    r.get("closeout_readiness_summary_json"),
                )?,
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
        r#"WITH live_approvals AS (
             SELECT run_id, COUNT(*) AS count
             FROM approvals
             WHERE decision IN ('pending', 'requested')
             GROUP BY run_id
           )
           SELECT r.id, r.idea_id,
                  COALESCE(rs.workflow_id, r.workflow_id) AS workflow_id,
                  COALESCE(rs.workflow_title, r.workflow_title) AS workflow_title,
                  COALESCE(rs.workspace_root, r.workspace_root) AS workspace_root,
                  COALESCE(rs.artifact_root, r.artifact_root) AS artifact_root,
                  r.started_at,
                  COALESCE(rs.completed_at, r.completed_at) AS completed_at,
                  COALESCE(rs.cancellation_requested_at, r.cancellation_requested_at) AS cancellation_requested_at,
                  COALESCE(rs.cancellation_settled_at, r.cancellation_settled_at) AS cancellation_settled_at,
                  rs.cancellation_settlement_summary,
                  rs.implementation_completion_json,
                  rs.closeout_readiness_summary_json,
                  r.status,
                  COALESCE(rs.total_stages, 0) AS total_stages,
                  COALESCE(rs.completed_stages, 0) AS completed_stages,
                  COALESCE(rs.failed_stages, 0) AS failed_stages,
                  COALESCE(la.count, 0) AS pending_approvals,
                  COALESCE(rs.chainworks_meta_root, r.chainworks_meta_root) AS chainworks_meta_root,
                  CASE WHEN rs.run_id IS NULL THEN 0 ELSE 1 END AS projection_present,
                  rs.updated_at AS projection_updated_at,
                  CASE WHEN rs.run_id IS NULL
                            OR rs.status != r.status
                            OR COALESCE(rs.pending_approvals, 0) != COALESCE(la.count, 0)
                       THEN 1 ELSE 0 END AS projection_lag
           FROM runs r
           LEFT JOIN run_summaries rs ON rs.run_id = r.id
           LEFT JOIN live_approvals la ON la.run_id = r.id
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
                workflow_id: r
                    .get::<Option<String>, _>("workflow_id")
                    .unwrap_or_default(),
                workflow_title: r.get("workflow_title"),
                status: r.get("status"),
                workspace_root: r
                    .get::<Option<String>, _>("workspace_root")
                    .unwrap_or_default(),
                artifact_root: r
                    .get::<Option<String>, _>("artifact_root")
                    .unwrap_or_default(),
                started_at: r.get("started_at"),
                completed_at: r.get("completed_at"),
                cancellation_requested_at: r.get("cancellation_requested_at"),
                cancellation_settled_at: r.get("cancellation_settled_at"),
                cancellation_settlement_summary: r.get("cancellation_settlement_summary"),
                total_stages: r.get("total_stages"),
                completed_stages: r.get("completed_stages"),
                failed_stages: r.get("failed_stages"),
                pending_approvals: r.get("pending_approvals"),
                chainworks_meta_root: r.get("chainworks_meta_root"),
                projection_present: r.get::<i64, _>("projection_present") != 0,
                projection_updated_at: r.get("projection_updated_at"),
                projection_lag: r.get::<i64, _>("projection_lag") != 0,
                implementation_completion: parse_projection_json(
                    r.get("implementation_completion_json"),
                )?,
                closeout_readiness_summary: parse_projection_json(
                    r.get("closeout_readiness_summary_json"),
                )?,
                implementation_closeout_readiness_summary: parse_projection_json(
                    r.get("closeout_readiness_summary_json"),
                )?,
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
            total_stages: r.get("total_stages"),
            completed_stages: r.get("completed_stages"),
            failed_stages: r.get("failed_stages"),
            pending_approvals: r.get("pending_approvals"),
            chainworks_meta_root: r.get("chainworks_meta_root"),
            projection_present: r.get::<i64, _>("projection_present") != 0,
            projection_updated_at: r.get("projection_updated_at"),
            projection_lag: r.get::<i64, _>("projection_lag") != 0,
            implementation_completion: parse_projection_json(
                r.get("implementation_completion_json"),
            )?,
            closeout_readiness_summary: parse_projection_json(
                r.get("closeout_readiness_summary_json"),
            )?,
            implementation_closeout_readiness_summary: parse_projection_json(
                r.get("closeout_readiness_summary_json"),
            )?,
        })
    })
    .transpose()
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
    let now_dt = Utc::now();
    let now = now_dt.to_rfc3339();
    let now_ms = now_dt.timestamp_millis();
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
                       (run_id, idea_id, workflow_id, workflow_title, status,
                        workspace_root, artifact_root, started_at, completed_at,
                        cancellation_requested_at, cancellation_settled_at,
                        chainworks_meta_root, total_stages, completed_stages,
                        failed_stages, pending_approvals, updated_at)
                       SELECT
                         r.id,
                         r.idea_id,
                         r.workflow_id,
                         r.workflow_title,
                         r.status,
                         r.workspace_root,
                         r.artifact_root,
                         r.started_at,
                         r.completed_at,
                         r.cancellation_requested_at,
                         r.cancellation_settled_at,
                         r.chainworks_meta_root,
                         COUNT(DISTINCT se.id),
                         COUNT(DISTINCT CASE WHEN se.status = 'completed' THEN se.id END),
                         COUNT(DISTINCT CASE WHEN se.status = 'failed' THEN se.id END),
                         COUNT(DISTINCT CASE WHEN a.decision IN ('pending','requested') THEN a.id END),
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

    refresh_run_list_readbacks(pool, run_id).await?;

    // P087: Mark invalidation log rows for this run as consumed
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "projection.invalidation.mark_consumed")
            .await?;
    crate::repos::projection_invalidation::mark_consumed_entity_tx(
        &mut tx,
        "run_summaries",
        "runs",
        &run_id_string,
        now_ms,
    )
    .await?;
    tx.commit().await?;

    info!(run_id = %run_id, "Rebuilt run_summary projection");
    Ok(())
}

/// P087: Rebuild artifact_noise_summary for a single run.
pub async fn rebuild_artifact_noise_summary(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let run_id_string = run_id.to_string();
    let now = Utc::now().timestamp_millis();

    execute_projection_write(
        pool,
        "projections.rebuild_artifact_noise_summary",
        format!("run:{run_id_string}:projection:artifact_noise"),
        Box::new(move |tx| {
            Box::pin(async move {
                let rows = sqlx::query(
                    r#"INSERT OR REPLACE INTO artifact_noise_summary
                       (run_id, artifact_count, superseded_count, duplicate_candidate_count, archive_eligible_count, updated_at_ms)
                       WITH duplicate_names AS (
                         SELECT run_id, name, COUNT(*) AS duplicate_count
                         FROM artifacts
                         WHERE run_id = ?1
                         GROUP BY run_id, name
                         HAVING COUNT(*) > 1
                       ),
                       duplicate_rollup AS (
                         SELECT run_id,
                                COUNT(*) AS duplicate_candidate_count,
                                SUM(duplicate_count - 1) AS superseded_count
                         FROM duplicate_names
                         GROUP BY run_id
                       )
                       SELECT
                         a.run_id,
                         COUNT(*) AS artifact_count,
                         COALESCE(dr.superseded_count, 0) AS superseded_count,
                         COALESCE(dr.duplicate_candidate_count, 0) AS duplicate_candidate_count,
                         SUM(CASE WHEN a.is_pinned = 0 THEN 1 ELSE 0 END) AS archive_eligible_count,
                         ?2
                       FROM artifacts a
                       LEFT JOIN duplicate_rollup dr ON dr.run_id = a.run_id
                       WHERE a.run_id = ?1
                       GROUP BY a.run_id"#
                )
                .bind(&run_id_string)
                .bind(now)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;

                // P087: Mark invalidation log rows for this run as consumed
                crate::repos::projection_invalidation::mark_consumed_entity_tx(
                    tx,
                    "artifact_noise_summary",
                    "artifacts",
                    &run_id_string,
                    now,
                )
                .await?;

                Ok(((), rows))
            })
        })
    ).await
}

/// P087: Rebuild the global runtime_health_summary projection.
pub async fn rebuild_runtime_health_summary(pool: &SqlitePool) -> Result<()> {
    let now = Utc::now().timestamp_millis();

    // 1. Compute families JSON
    let active_sessions_by_family = sqlx::query(
        r#"SELECT COALESCE(provider_family, provider, 'unknown') AS family, COUNT(*) AS count
           FROM agent_executions
           WHERE status = 'running'
           GROUP BY COALESCE(provider_family, provider, 'unknown')
           ORDER BY family"#,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| {
        serde_json::json!({
            "runtimeFamily": row.get::<String, _>("family"),
            "activeSessions": row.get::<i64, _>("count"),
        })
    })
    .collect::<Vec<_>>();

    let active_sessions: i64 = active_sessions_by_family
        .iter()
        .filter_map(|s| s["activeSessions"].as_i64())
        .sum();

    let runtime_families_json = serde_json::to_string(&active_sessions_by_family)?;

    // 2. Compute other counts
    let open_hot_read_circuits = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM hot_read_circuit_states WHERE circuit_status = 'open'",
    )
    .fetch_one(pool)
    .await?;

    let side_effect_unresolved_count = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*)
           FROM side_effects
           WHERE status IN (
             'prepared','executing','externally_observed',
             'needs_reconciliation','conflict','unrecoverable'
           )"#,
    )
    .fetch_one(pool)
    .await?;

    let continuation_active_count = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*)
           FROM work_items
           WHERE status IN ('pending','running')
             AND kind IN ('advance_run','invoke_agent')"#,
    )
    .fetch_one(pool)
    .await?;

    // 3. Update the single row
    execute_projection_write(
        pool,
        "projections.rebuild_runtime_health_summary",
        "global:projection:runtime_health",
        Box::new(move |tx| {
            Box::pin(async move {
                let rows = sqlx::query(
                    r#"UPDATE runtime_health_summary
                       SET active_sessions = ?1,
                           open_hot_read_circuits = ?2,
                           side_effect_unresolved_count = ?3,
                           continuation_active_count = ?4,
                           runtime_families_json = ?5,
                           updated_at_ms = ?6
                       WHERE id = 1"#,
                )
                .bind(active_sessions)
                .bind(open_hot_read_circuits)
                .bind(side_effect_unresolved_count)
                .bind(continuation_active_count)
                .bind(runtime_families_json)
                .bind(now)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;

                // P087: Mark invalidation log rows for this global projection as consumed
                crate::repos::projection_invalidation::mark_consumed_tx(
                    tx,
                    "runtime_health_summary",
                    "runtime",
                    now,
                )
                .await?;

                Ok(((), rows))
            })
        }),
    )
    .await
}

pub async fn refresh_run_list_readbacks(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let run_id_string = run_id.to_string();
    let implementation_completion_json = {
        let readbacks =
            crate::repos::code_writer_completion_receipts::list_canonical_by_run(pool, run_id)
                .await?;
        let summary = domain::code_writer_completion::project_implementation_completion(&readbacks);
        serde_json::to_string(&summary).context("serialize implementation completion summary")?
    };
    let closeout_summary_json = if let Some(summary) =
        crate::repos::closeout::load_closeout_readiness_summary(pool, &run_id_string).await?
    {
        Some(serde_json::to_string(&summary).context("serialize closeout readiness summary")?)
    } else {
        None
    };

    execute_projection_write(
        pool,
        "projections.refresh_run_list_readbacks",
        format!("run:{run_id_string}:projection:run_list_readbacks"),
        Box::new(move |tx| {
            Box::pin(async move {
                let rows = sqlx::query(
                    r#"UPDATE run_summaries
                       SET implementation_completion_json = ?1,
                           closeout_readiness_summary_json = ?2
                       WHERE run_id = ?3"#,
                )
                .bind(implementation_completion_json)
                .bind(closeout_summary_json)
                .bind(run_id_string)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;
                Ok(((), rows))
            })
        }),
    )
    .await
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
    let now_dt = Utc::now();
    let now = now_dt.to_rfc3339();
    let now_ms = now_dt.timestamp_millis();
    let run_id_string = run_id.to_string();

    execute_projection_write(
        pool,
        "projections.rebuild_stage_summaries",
        format!("run:{run_id_string}:projection:stage_summaries"),
        {
            let now = now.clone();
            let run_id_string = run_id_string.clone();
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
                .bind(&run_id_string)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;

                // P087: Mark invalidation log rows for this run as consumed
                crate::repos::projection_invalidation::mark_consumed_entity_tx(
                    tx,
                    "stage_summaries",
                    "stages",
                    &run_id_string,
                    now_ms,
                )
                .await?;

                Ok(((), rows))
                })
            })
        },
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
    let now_dt = Utc::now();
    let now = now_dt.to_rfc3339();
    let now_ms = now_dt.timestamp_millis();
    let run_id_string = run_id.to_string();

    execute_projection_write(
        pool,
        "projections.rebuild_approval_inbox",
        format!("run:{run_id_string}:projection:approval_inbox"),
        {
            let now = now.clone();
            let run_id_string = run_id_string.clone();
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
                .bind(&run_id_string)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;

                // P087: Mark invalidation log rows for this run as consumed
                crate::repos::projection_invalidation::mark_consumed_entity_tx(
                    tx,
                    "approval_inbox",
                    "approvals",
                    &run_id_string,
                    now_ms,
                )
                .await?;

                Ok(((), rows))
                })
            })
        },
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
    let now_ms = Utc::now().timestamp_millis();
    let run_id_string = run_id.to_string();
    execute_projection_write(
        pool,
        "projections.upsert_artifact_index_entry",
        format!("run:{run_id_string}:projection:artifact_index"),
        {
            let run_id_string = run_id_string.clone();
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
                .bind(&run_id_string)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;

                // P087: Mark invalidation log rows for this run as consumed
                crate::repos::projection_invalidation::mark_consumed_entity_tx(
                    tx,
                    "artifact_index",
                    "artifacts",
                    &run_id_string,
                    now_ms,
                )
                .await?;

                Ok(((), rows))
                })
            })
        },
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
    let pool_for_freeze = pool.clone();
    let pool = pool.clone();
    let result = run_projection_rebuild_on_dedicated_stack("all-for-run", move || async move {
        rebuild_all_for_run_on_current_thread(&pool, run_id).await
    })
    .await;

    // P087: On rebuild failure, freeze the run_summaries/runs cursor so operators must
    // explicitly clear the poison before the backlog can resume draining.
    if let Err(ref e) = result {
        tracing::warn!(
            event = "projection_rebuild_failed_freeze_cursor",
            run_id = %run_id,
            error = %e,
            "Projection rebuild failed; freezing run_summaries/runs cursor"
        );
        if let Err(freeze_err) =
            crate::repos::projection_invalidation::freeze_cursor_after_retry_exhaustion(
                &pool_for_freeze,
                "run_summaries",
                "runs",
                1,
            )
            .await
        {
            tracing::warn!(
                event = "projection_cursor_freeze_failed",
                run_id = %run_id,
                error = %freeze_err,
                "Failed to freeze cursor after rebuild failure"
            );
        }
    }

    result
}

/// Drain one pending projection invalidation group using the oldest source
/// watermark first. This is the production consumer paired with
/// `projection_invalidation_log`; successful rebuilds mark rows consumed inside
/// each projection writer, while failed rebuilds freeze the cursor so operators
/// see a poisoned/stale projection instead of an unbounded silent backlog.
pub async fn drain_oldest_pending_invalidation(
    pool: &SqlitePool,
) -> Result<Option<(String, String)>> {
    let queue = crate::repos::projection_invalidation::get_drain_priority_queue(pool).await?;
    for (projection_name, source_name) in queue {
        let result = drain_projection_source(pool, &projection_name, &source_name).await;
        match result {
            Ok(true) => return Ok(Some((projection_name, source_name))),
            Ok(false) => {
                crate::repos::projection_invalidation::freeze_cursor_after_retry_exhaustion(
                    pool,
                    &projection_name,
                    &source_name,
                    1,
                )
                .await?;
                tracing::warn!(
                    event = "projection_invalidation_unknown_source_frozen",
                    projection_name = %projection_name,
                    source_name = %source_name,
                    "Unknown projection invalidation source frozen for operator clear"
                );
                return Ok(Some((projection_name, source_name)));
            }
            Err(error) => {
                let _ =
                    crate::repos::projection_invalidation::freeze_cursor_after_retry_exhaustion(
                        pool,
                        &projection_name,
                        &source_name,
                        1,
                    )
                    .await;
                return Err(error).with_context(|| {
                    format!("drain projection invalidation {projection_name}/{source_name}")
                });
            }
        }
    }
    Ok(None)
}

async fn drain_projection_source(
    pool: &SqlitePool,
    projection_name: &str,
    source_name: &str,
) -> Result<bool> {
    if projection_name == "runtime_health_summary" && source_name == "runtime" {
        rebuild_runtime_health_summary(pool).await?;
        return Ok(true);
    }

    // Return false (unknown) before fetching keys so unknown projections are
    // immediately frozen rather than producing a misleading RunId parse error.
    let is_run_scoped = matches!(
        (projection_name, source_name),
        ("run_summaries", "runs")
            | ("stage_summaries", "stages")
            | ("approval_inbox", "approvals")
            | ("artifact_index", "artifacts")
            | ("artifact_noise_summary", "artifacts")
    );
    if !is_run_scoped {
        return Ok(false);
    }

    let keys = pending_invalidation_keys(pool, projection_name, source_name, 128).await?;
    if keys.is_empty() {
        return Ok(true);
    }

    for key in keys {
        let run_id: RunId = key
            .parse()
            .with_context(|| format!("invalid run id in projection invalidation key: {key}"))?;
        match (projection_name, source_name) {
            ("run_summaries", "runs") => rebuild_run_summary(pool, run_id).await?,
            ("stage_summaries", "stages") => rebuild_stage_summaries(pool, run_id).await?,
            ("approval_inbox", "approvals") => rebuild_approval_inbox(pool, run_id).await?,
            ("artifact_index", "artifacts") => upsert_artifact_index_entry(pool, run_id).await?,
            ("artifact_noise_summary", "artifacts") => {
                rebuild_artifact_noise_summary(pool, run_id).await?
            }
            _ => unreachable!("already guarded by is_run_scoped"),
        }
    }
    Ok(true)
}

async fn pending_invalidation_keys(
    pool: &SqlitePool,
    projection_name: &str,
    source_name: &str,
    limit: i64,
) -> Result<Vec<String>> {
    let rows = sqlx::query(
        "SELECT DISTINCT primary_key
         FROM projection_invalidation_log
         WHERE projection_name = ? AND source_name = ? AND is_consumed = 0
         ORDER BY created_at_ms ASC
         LIMIT ?",
    )
    .bind(projection_name)
    .bind(source_name)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("primary_key"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::idea::{Idea, IdeaStatus};
    use domain::run::{Run, RunStatus};

    fn make_idea(idea_id: domain::ids::IdeaId) -> Idea {
        Idea {
            id: idea_id,
            title: "P087 projection drain".to_string(),
            body: "exercise projection invalidation consumer".to_string(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        }
    }

    fn make_run(run_id: RunId, idea_id: domain::ids::IdeaId) -> Run {
        Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-p087".to_string(),
            workflow_title: "P087".to_string(),
            workspace_root: "/tmp/p087".to_string(),
            artifact_root: "/tmp/p087/artifacts".to_string(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: Some("state_implementation".to_string()),
            workflow_yaml_path: Some("examples/workflows/full-mvp-live.yaml".to_string()),
            agent_catalog_yaml_path: Some("examples/agents/agents.yaml".to_string()),
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
        }
    }

    #[tokio::test]
    async fn proposal_087_drain_oldest_pending_invalidation_rebuilds_and_marks_consumed() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let idea_id = domain::ids::IdeaId::new();
        let run_id = RunId::new();
        crate::repos::ideas::insert(&pool, &make_idea(idea_id))
            .await
            .unwrap();
        crate::repos::runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        crate::repos::projection_invalidation::record_invalidation(
            &pool,
            "run_summaries",
            "runs",
            &run_id.to_string(),
            "upsert",
            None,
        )
        .await
        .unwrap();

        let drained = drain_oldest_pending_invalidation(&pool).await.unwrap();
        assert_eq!(
            drained,
            Some(("run_summaries".to_string(), "runs".to_string()))
        );

        let summary_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM run_summaries WHERE run_id = ?")
                .bind(run_id.to_string())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(summary_count, 1);

        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM projection_invalidation_log WHERE is_consumed = 0",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(pending, 0);
    }

    #[tokio::test]
    async fn proposal_087_drain_unknown_invalidation_freezes_cursor() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        crate::repos::projection_invalidation::record_invalidation(
            &pool,
            "unknown_projection",
            "unknown_source",
            "key",
            "upsert",
            None,
        )
        .await
        .unwrap();

        let drained = drain_oldest_pending_invalidation(&pool).await.unwrap();
        assert_eq!(
            drained,
            Some((
                "unknown_projection".to_string(),
                "unknown_source".to_string()
            ))
        );

        let poisoned: i64 = sqlx::query_scalar(
            "SELECT is_poisoned FROM projection_cursors WHERE projection_name = 'unknown_projection' AND source_name = 'unknown_source'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(poisoned, 1);
    }
}

async fn rebuild_all_for_run_on_current_thread(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    rebuild_run_summary_on_current_thread(pool, run_id).await?;
    rebuild_stage_summaries_on_current_thread(pool, run_id).await?;
    rebuild_approval_inbox_on_current_thread(pool, run_id).await?;
    upsert_artifact_index_entry_on_current_thread(pool, run_id).await?;
    artifact_contracts::rebuild_projection_and_exports(pool, run_id).await?;

    // P087: Rebuild new health/noise projections
    rebuild_artifact_noise_summary(pool, run_id).await?;
    rebuild_runtime_health_summary(pool).await?;

    info!(run_id = %run_id, "Full projection rebuild complete");
    Ok(())
}
