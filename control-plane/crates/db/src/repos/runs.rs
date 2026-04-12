use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use domain::ids::{IdeaId, RunId};
use domain::run::{Run, RunStatus};

pub async fn insert(pool: &SqlitePool, run: &Run) -> Result<()> {
    let id = run.id.to_string();
    let idea_id = run.idea_id.to_string();
    let status = run.status.to_string();
    let started_at = run.started_at.to_rfc3339();
    let completed_at = run.completed_at.map(|t| t.to_rfc3339());
    let cancellation_requested_at = run.cancellation_requested_at.map(|t| t.to_rfc3339());
    let cancellation_settled_at = run.cancellation_settled_at.map(|t| t.to_rfc3339());

    sqlx::query(
        r#"
        INSERT INTO runs (id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root,
                          started_at, completed_at, cancellation_requested_at, cancellation_settled_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(id)
    .bind(idea_id)
    .bind(status)
    .bind(&run.workflow_id)
    .bind(&run.workflow_title)
    .bind(&run.workspace_root)
    .bind(&run.artifact_root)
    .bind(started_at)
    .bind(completed_at)
    .bind(cancellation_requested_at)
    .bind(cancellation_settled_at)
    .execute(pool)
    .await
    .context("insert run")?;
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: RunId) -> Result<Option<Run>> {
    let id_str = id.to_string();
    let row = sqlx::query(
        r#"SELECT id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root,
                  started_at, completed_at, cancellation_requested_at, cancellation_settled_at
           FROM runs WHERE id = ?1"#,
    )
    .bind(id_str)
    .fetch_optional(pool)
    .await
    .context("find run by id")?;

    row.map(|r| {
        parse_run_row(
            r.get("id"),
            r.get("idea_id"),
            r.get("status"),
            r.get("workflow_id"),
            r.get("workflow_title"),
            r.get("workspace_root"),
            r.get("artifact_root"),
            r.get("started_at"),
            r.get("completed_at"),
            r.get("cancellation_requested_at"),
            r.get("cancellation_settled_at"),
        )
    })
    .transpose()
}

pub async fn list_by_idea(pool: &SqlitePool, idea_id: IdeaId) -> Result<Vec<Run>> {
    let idea_id_str = idea_id.to_string();
    let rows = sqlx::query(
        r#"SELECT id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root,
                  started_at, completed_at, cancellation_requested_at, cancellation_settled_at
           FROM runs WHERE idea_id = ?1 ORDER BY started_at DESC"#,
    )
    .bind(idea_id_str)
    .fetch_all(pool)
    .await
    .context("list runs by idea")?;

    rows.into_iter()
        .map(|r| {
            parse_run_row(
                r.get("id"),
                r.get("idea_id"),
                r.get("status"),
                r.get("workflow_id"),
                r.get("workflow_title"),
                r.get("workspace_root"),
                r.get("artifact_root"),
                r.get("started_at"),
                r.get("completed_at"),
                r.get("cancellation_requested_at"),
                r.get("cancellation_settled_at"),
            )
        })
        .collect()
}

pub async fn list_active(pool: &SqlitePool) -> Result<Vec<Run>> {
    let rows = sqlx::query(
        r#"SELECT id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root,
                  started_at, completed_at, cancellation_requested_at, cancellation_settled_at
           FROM runs
           WHERE status NOT IN ('completed', 'failed', 'cancelled')
           ORDER BY started_at DESC"#,
    )
    .fetch_all(pool)
    .await
    .context("list active runs")?;

    rows.into_iter()
        .map(|r| {
            parse_run_row(
                r.get("id"),
                r.get("idea_id"),
                r.get("status"),
                r.get("workflow_id"),
                r.get("workflow_title"),
                r.get("workspace_root"),
                r.get("artifact_root"),
                r.get("started_at"),
                r.get("completed_at"),
                r.get("cancellation_requested_at"),
                r.get("cancellation_settled_at"),
            )
        })
        .collect()
}

pub async fn update_status(pool: &SqlitePool, id: RunId, status: RunStatus) -> Result<()> {
    let id_str = id.to_string();
    let status_str = status.to_string();
    sqlx::query(r#"UPDATE runs SET status = ?1 WHERE id = ?2"#)
        .bind(status_str)
        .bind(id_str)
        .execute(pool)
        .await
        .context("update run status")?;
    Ok(())
}

pub async fn mark_cancelled(pool: &SqlitePool, id: RunId, settled_at: DateTime<Utc>) -> Result<()> {
    let id_str = id.to_string();
    let settled_at_str = settled_at.to_rfc3339();
    let status = RunStatus::Cancelled.to_string();
    sqlx::query(
        r#"UPDATE runs SET status = ?1, cancellation_settled_at = ?2 WHERE id = ?3"#,
    )
    .bind(status)
    .bind(settled_at_str)
    .bind(id_str)
    .execute(pool)
    .await
    .context("mark run cancelled")?;
    Ok(())
}

pub async fn mark_completed(
    pool: &SqlitePool,
    id: RunId,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    let id_str = id.to_string();
    let completed_at_str = completed_at.to_rfc3339();
    let status = RunStatus::Completed.to_string();
    sqlx::query(r#"UPDATE runs SET status = ?1, completed_at = ?2 WHERE id = ?3"#)
        .bind(status)
        .bind(completed_at_str)
        .bind(id_str)
        .execute(pool)
        .await
        .context("mark run completed")?;
    Ok(())
}

fn parse_run_row(
    id: String,
    idea_id: String,
    status: String,
    workflow_id: String,
    workflow_title: String,
    workspace_root: String,
    artifact_root: String,
    started_at: String,
    completed_at: Option<String>,
    cancellation_requested_at: Option<String>,
    cancellation_settled_at: Option<String>,
) -> Result<Run> {
    let run_id: RunId = id.parse::<uuid::Uuid>().context("parse run id")?.into();
    let idea_id_val: IdeaId = idea_id
        .parse::<uuid::Uuid>()
        .context("parse run idea_id")?
        .into();
    let run_status: RunStatus = status.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let started_at_dt: DateTime<Utc> = DateTime::parse_from_rfc3339(&started_at)
        .context("parse run started_at")?
        .with_timezone(&Utc);
    let completed_at_dt = parse_optional_dt(completed_at, "run completed_at")?;
    let cancellation_requested_at_dt =
        parse_optional_dt(cancellation_requested_at, "run cancellation_requested_at")?;
    let cancellation_settled_at_dt =
        parse_optional_dt(cancellation_settled_at, "run cancellation_settled_at")?;

    Ok(Run {
        id: run_id,
        idea_id: idea_id_val,
        status: run_status,
        workflow_id,
        workflow_title,
        workspace_root,
        artifact_root,
        started_at: started_at_dt,
        completed_at: completed_at_dt,
        cancellation_requested_at: cancellation_requested_at_dt,
        cancellation_settled_at: cancellation_settled_at_dt,
    })
}

fn parse_optional_dt(s: Option<String>, field: &str) -> Result<Option<DateTime<Utc>>> {
    s.map(|v| {
        DateTime::parse_from_rfc3339(&v)
            .with_context(|| format!("parse {field}"))
            .map(|dt| dt.with_timezone(&Utc))
    })
    .transpose()
}
