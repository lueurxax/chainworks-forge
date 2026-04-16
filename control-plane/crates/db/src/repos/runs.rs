use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use domain::ids::{IdeaId, RunId};
use domain::run::{Run, RunStatus};

const SELECT_COLS: &str = r#"id, idea_id, status, workflow_id, workflow_title, workspace_root,
             artifact_root, started_at, completed_at, cancellation_requested_at,
             cancellation_settled_at, cancellation_settlement_log, current_state, workflow_yaml_path,
             agent_catalog_yaml_path, worktree_root, base_branch, base_revision,
             target_branch, delivery_configuration_json"#;

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
                          started_at, completed_at, cancellation_requested_at, cancellation_settled_at,
                          cancellation_settlement_log, current_state, workflow_yaml_path, agent_catalog_yaml_path,
                          worktree_root, base_branch, base_revision, target_branch,
                          delivery_configuration_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
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
    .bind(&run.cancellation_settlement_log)
    .bind(&run.current_state)
    .bind(&run.workflow_yaml_path)
    .bind(&run.agent_catalog_yaml_path)
    .bind(&run.worktree_root)
    .bind(&run.base_branch)
    .bind(&run.base_revision)
    .bind(&run.target_branch)
    .bind(&run.delivery_configuration_json)
    .execute(pool)
    .await
    .context("insert run")?;
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: RunId) -> Result<Option<Run>> {
    let id_str = id.to_string();
    let query = format!("SELECT {SELECT_COLS} FROM runs WHERE id = ?1");
    let row = sqlx::query(&query)
        .bind(id_str)
        .fetch_optional(pool)
        .await
        .context("find run by id")?;

    row.map(|r| parse_run_row(&r)).transpose()
}

pub async fn list_by_idea(pool: &SqlitePool, idea_id: IdeaId) -> Result<Vec<Run>> {
    let idea_id_str = idea_id.to_string();
    let query = format!(
        "SELECT {SELECT_COLS} FROM runs WHERE idea_id = ?1 ORDER BY started_at DESC"
    );
    let rows = sqlx::query(&query)
        .bind(idea_id_str)
        .fetch_all(pool)
        .await
        .context("list runs by idea")?;

    rows.iter().map(|r| parse_run_row(r)).collect()
}

pub async fn list_active(pool: &SqlitePool) -> Result<Vec<Run>> {
    let query = format!(
        "SELECT {SELECT_COLS} FROM runs WHERE status NOT IN ('completed', 'failed', 'cancelled') ORDER BY started_at DESC"
    );
    let rows = sqlx::query(&query)
        .fetch_all(pool)
        .await
        .context("list active runs")?;

    rows.iter().map(|r| parse_run_row(r)).collect()
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

/// Update the current_state for a workflow-driven run.
pub async fn update_current_state(pool: &SqlitePool, id: RunId, state: &str) -> Result<()> {
    let id_str = id.to_string();
    sqlx::query(r#"UPDATE runs SET current_state = ?1 WHERE id = ?2"#)
        .bind(state)
        .bind(id_str)
        .execute(pool)
        .await
        .context("update run current_state")?;
    Ok(())
}

/// Transition a run into the Cancelling state with a cancellation timestamp.
pub async fn mark_cancelling(
    pool: &SqlitePool,
    id: RunId,
    requested_at: DateTime<Utc>,
) -> Result<()> {
    let id_str = id.to_string();
    let status = RunStatus::Cancelling.to_string();
    sqlx::query(
        r#"UPDATE runs SET status = ?1, cancellation_requested_at = ?2 WHERE id = ?3"#,
    )
    .bind(status)
    .bind(requested_at.to_rfc3339())
    .bind(id_str)
    .execute(pool)
    .await
    .context("mark run cancelling")?;
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

pub async fn update_cancellation_settlement_log(
    pool: &SqlitePool,
    id: RunId,
    settlement_log: &str,
) -> Result<()> {
    sqlx::query(r#"UPDATE runs SET cancellation_settlement_log = ?1 WHERE id = ?2"#)
        .bind(settlement_log)
        .bind(id.to_string())
        .execute(pool)
        .await
        .context("update cancellation settlement log")?;
    Ok(())
}

pub async fn finalize_cancellation(
    pool: &SqlitePool,
    id: RunId,
    settled_at: DateTime<Utc>,
    settlement_log: &str,
) -> Result<()> {
    let status = RunStatus::Cancelled.to_string();
    sqlx::query(
        r#"UPDATE runs
           SET status = ?1, cancellation_settled_at = ?2, cancellation_settlement_log = ?3
           WHERE id = ?4"#,
    )
    .bind(status)
    .bind(settled_at.to_rfc3339())
    .bind(settlement_log)
    .bind(id.to_string())
    .execute(pool)
    .await
    .context("finalize run cancellation")?;
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

/// Update worktree fields after provisioning (Proposal 007).
pub async fn update_worktree_fields(
    pool: &SqlitePool,
    id: RunId,
    worktree_root: &str,
    base_branch: &str,
    base_revision: &str,
    target_branch: &str,
) -> Result<()> {
    let id_str = id.to_string();
    sqlx::query(
        r#"UPDATE runs SET worktree_root = ?1, base_branch = ?2, base_revision = ?3, target_branch = ?4 WHERE id = ?5"#,
    )
    .bind(worktree_root)
    .bind(base_branch)
    .bind(base_revision)
    .bind(target_branch)
    .bind(id_str)
    .execute(pool)
    .await
    .context("update run worktree fields")?;
    Ok(())
}

fn parse_run_row(r: &sqlx::sqlite::SqliteRow) -> Result<Run> {
    let id: String = r.get("id");
    let idea_id: String = r.get("idea_id");
    let status: String = r.get("status");
    let started_at: String = r.get("started_at");
    let completed_at: Option<String> = r.get("completed_at");
    let cancellation_requested_at: Option<String> = r.get("cancellation_requested_at");
    let cancellation_settled_at: Option<String> = r.get("cancellation_settled_at");
    let cancellation_settlement_log: Option<String> = r.get("cancellation_settlement_log");

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
        workflow_id: r.get("workflow_id"),
        workflow_title: r.get("workflow_title"),
        workspace_root: r.get("workspace_root"),
        artifact_root: r.get("artifact_root"),
        started_at: started_at_dt,
        completed_at: completed_at_dt,
        cancellation_requested_at: cancellation_requested_at_dt,
        cancellation_settled_at: cancellation_settled_at_dt,
        cancellation_settlement_log,
        current_state: r.get("current_state"),
        workflow_yaml_path: r.get("workflow_yaml_path"),
        agent_catalog_yaml_path: r.get("agent_catalog_yaml_path"),
        worktree_root: r.get("worktree_root"),
        base_branch: r.get("base_branch"),
        base_revision: r.get("base_revision"),
        target_branch: r.get("target_branch"),
        delivery_configuration_json: r.get("delivery_configuration_json"),
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
