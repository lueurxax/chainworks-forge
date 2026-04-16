use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use tracing::info;
use domain::ids::RunId;

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
}

/// List active runs via the projection layer.
///
/// Primary table is `runs`; `run_summaries` is LEFT-JOINed so that runs whose
/// projection hasn't been rebuilt yet are still returned (with zero counts).
/// Status is sourced from `run_summaries` when available, falling back to `runs`.
pub async fn list_active_projection(pool: &SqlitePool) -> Result<Vec<RunProjectionRow>> {
    let rows = sqlx::query(
        r#"SELECT r.id, r.idea_id, r.workflow_id, r.workflow_title, r.workspace_root,
                  r.artifact_root, r.started_at, r.completed_at,
                  r.cancellation_requested_at, r.cancellation_settled_at,
                  rs.cancellation_settlement_summary,
                  COALESCE(rs.status, r.status) AS status,
                  COALESCE(rs.total_stages, 0) AS total_stages,
                  COALESCE(rs.completed_stages, 0) AS completed_stages,
                  COALESCE(rs.failed_stages, 0) AS failed_stages,
                  COALESCE(rs.pending_approvals, 0) AS pending_approvals
           FROM runs r
           LEFT JOIN run_summaries rs ON rs.run_id = r.id
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
                total_stages: r.get("total_stages"),
                completed_stages: r.get("completed_stages"),
                failed_stages: r.get("failed_stages"),
                pending_approvals: r.get("pending_approvals"),
            })
        })
        .collect()
}

/// List runs for a specific idea via the projection layer.
pub async fn list_by_idea_projection(pool: &SqlitePool, idea_id: &str) -> Result<Vec<RunProjectionRow>> {
    let rows = sqlx::query(
        r#"SELECT r.id, r.idea_id, r.workflow_id, r.workflow_title, r.workspace_root,
                  r.artifact_root, r.started_at, r.completed_at,
                  r.cancellation_requested_at, r.cancellation_settled_at,
                  rs.cancellation_settlement_summary,
                  COALESCE(rs.status, r.status) AS status,
                  COALESCE(rs.total_stages, 0) AS total_stages,
                  COALESCE(rs.completed_stages, 0) AS completed_stages,
                  COALESCE(rs.failed_stages, 0) AS failed_stages,
                  COALESCE(rs.pending_approvals, 0) AS pending_approvals
           FROM runs r
           LEFT JOIN run_summaries rs ON rs.run_id = r.id
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
                total_stages: r.get("total_stages"),
                completed_stages: r.get("completed_stages"),
                failed_stages: r.get("failed_stages"),
                pending_approvals: r.get("pending_approvals"),
            })
        })
        .collect()
}

/// List stages for a run via the projection layer.
///
/// Primary table is `stage_executions`; `stage_summaries` is LEFT-JOINed so
/// that stages whose projection hasn't been rebuilt yet are still returned.
pub async fn list_stages_projection(pool: &SqlitePool, run_id: &str) -> Result<Vec<StageSummaryRow>> {
    let rows = sqlx::query(
        r#"SELECT se.id, se.run_id, se.stage_id, se.label, se.iteration, se.settlement_kind,
                  se.started_at, se.completed_at,
                  COALESCE(ss.status, se.status) AS status,
                  COALESCE(ss.attempt_number, se.attempt_number) AS attempt_number,
                  COALESCE(ss.has_artifacts, 0) AS has_artifacts,
                  COALESCE(ss.has_pending_approval, 0) AS has_pending_approval,
                  COALESCE(ss.has_validation_failure, 0) AS has_validation_failure
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
            })
        })
        .collect()
}

/// Find a single run via the projection layer.
///
/// LEFT-JOINs `run_summaries` so that a run whose projection hasn't been
/// rebuilt yet is still returned (with zero counts). Returns `None` only if
/// the run does not exist in the canonical `runs` table.
pub async fn find_run_projection(pool: &SqlitePool, run_id: &str) -> Result<Option<RunProjectionRow>> {
    let row = sqlx::query(
        r#"SELECT r.id, r.idea_id, r.workflow_id, r.workflow_title, r.workspace_root,
                  r.artifact_root, r.started_at, r.completed_at,
                  r.cancellation_requested_at, r.cancellation_settled_at,
                  rs.cancellation_settlement_summary,
                  COALESCE(rs.status, r.status) AS status,
                  COALESCE(rs.total_stages, 0) AS total_stages,
                  COALESCE(rs.completed_stages, 0) AS completed_stages,
                  COALESCE(rs.failed_stages, 0) AS failed_stages,
                  COALESCE(rs.pending_approvals, 0) AS pending_approvals
           FROM runs r
           LEFT JOIN run_summaries rs ON rs.run_id = r.id
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
        })
    })
    .transpose()
}

/// Rebuild run_summary for a single run from canonical tables.
pub async fn rebuild_run_summary(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    // Upsert run_summaries by computing counts from canonical tables
    sqlx::query(
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
    .bind(&now)
    .bind(run_id.to_string())
    .execute(pool)
    .await?;

    let settlement_summary = sqlx::query_scalar::<_, Option<String>>(
        "SELECT cancellation_settlement_log FROM runs WHERE id = ?",
    )
    .bind(run_id.to_string())
    .fetch_one(pool)
    .await?
    .and_then(|log| build_cancellation_settlement_summary(&log));

    sqlx::query(
        "UPDATE run_summaries SET cancellation_settlement_summary = ?1 WHERE run_id = ?2",
    )
    .bind(settlement_summary)
    .bind(run_id.to_string())
    .execute(pool)
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
        .filter(|entry| entry.get("session_close_succeeded").and_then(|v| v.as_bool()) == Some(true))
        .count();

    Some(format!(
        "{settled_count}/{total_count} agents settled, {close_ok} sessions closed"
    ))
}

/// Rebuild stage_summaries for all stages in a run.
pub async fn rebuild_stage_summaries(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"INSERT OR REPLACE INTO stage_summaries
           (stage_execution_id, run_id, stage_id, label, status, attempt_number, has_artifacts, has_pending_approval, has_validation_failure, updated_at)
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
             ?
           FROM stage_executions se
           WHERE se.run_id = ?"#,
    )
    .bind(&now)
    .bind(run_id.to_string())
    .execute(pool)
    .await?;

    Ok(())
}

/// Rebuild approval_inbox for a run.
pub async fn rebuild_approval_inbox(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    // Remove stale entries for this run
    sqlx::query("DELETE FROM approval_inbox WHERE run_id = ?")
        .bind(run_id.to_string())
        .execute(pool)
        .await?;

    // Insert pending/requested approvals
    sqlx::query(
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
    .bind(&now)
    .bind(run_id.to_string())
    .execute(pool)
    .await?;

    Ok(())
}

/// Rebuild artifact_index entries for new artifacts in a run.
pub async fn upsert_artifact_index_entry(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"INSERT OR IGNORE INTO artifact_index
           (artifact_id, run_id, stage_id, name, format, file_path, is_pinned, report_kind, created_at)
           SELECT id, run_id, stage_id, name, format, file_path, is_pinned, report_kind, created_at
           FROM artifacts WHERE run_id = ?"#,
    )
    .bind(run_id.to_string())
    .execute(pool)
    .await?;

    // Update is_pinned from source
    sqlx::query(
        "UPDATE artifact_index SET is_pinned = (SELECT is_pinned FROM artifacts WHERE artifacts.id = artifact_index.artifact_id) WHERE run_id = ?"
    )
    .bind(run_id.to_string())
    .execute(pool)
    .await?;

    let _ = now; // suppress unused warning
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
pub async fn list_artifacts_projection(pool: &SqlitePool, run_id: &str) -> Result<Vec<ArtifactIndexRow>> {
    let rows = sqlx::query(
        r#"SELECT ai.artifact_id AS id, ai.run_id, ai.stage_id, a.agent_id, ai.name,
                  a.contract_id, ai.format, ai.file_path, a.checksum_sha256, a.size_bytes,
                  a.provider, a.model, ai.created_at, ai.is_pinned, ai.report_kind,
                  a.report_version
           FROM artifact_index ai
           JOIN artifacts a ON a.id = ai.artifact_id
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
}

/// Full projection rebuild for a run (all tables).
pub async fn rebuild_all_for_run(pool: &SqlitePool, run_id: RunId) -> Result<()> {
    rebuild_run_summary(pool, run_id).await?;
    rebuild_stage_summaries(pool, run_id).await?;
    rebuild_approval_inbox(pool, run_id).await?;
    upsert_artifact_index_entry(pool, run_id).await?;
    info!(run_id = %run_id, "Full projection rebuild complete");
    Ok(())
}
