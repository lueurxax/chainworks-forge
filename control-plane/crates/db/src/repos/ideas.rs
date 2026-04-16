use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use domain::idea::{Idea, IdeaStatus};
use domain::ids::IdeaId;

pub async fn insert(pool: &SqlitePool, idea: &Idea) -> Result<()> {
    let id = idea.id.to_string();
    let status = idea.status.to_string();
    let created_at = idea.created_at.to_rfc3339();
    let archived_at = idea.archived_at.map(|t| t.to_rfc3339());

    sqlx::query(
        r#"
        INSERT INTO ideas (id, title, body, workspace_root_path, project_key, status, created_at, archived_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(id)
    .bind(&idea.title)
    .bind(&idea.body)
    .bind(&idea.workspace_root_path)
    .bind(&idea.project_key)
    .bind(status)
    .bind(created_at)
    .bind(archived_at)
    .execute(pool)
    .await
    .context("insert idea")?;
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: IdeaId) -> Result<Option<Idea>> {
    let id_str = id.to_string();
    let row = sqlx::query(
        r#"SELECT id, title, body, workspace_root_path, project_key, status, created_at, archived_at
           FROM ideas WHERE id = ?1"#,
    )
    .bind(id_str)
    .fetch_optional(pool)
    .await
    .context("find idea by id")?;

    row.map(|r| {
        parse_idea_row(
            r.get("id"),
            r.get("title"),
            r.get("body"),
            r.get("workspace_root_path"),
            r.get("project_key"),
            r.get("status"),
            r.get("created_at"),
            r.get("archived_at"),
        )
    })
    .transpose()
}

pub async fn list(pool: &SqlitePool, include_archived: bool) -> Result<Vec<Idea>> {
    let rows = if include_archived {
        sqlx::query(
            r#"SELECT id, title, body, workspace_root_path, project_key, status, created_at, archived_at
               FROM ideas ORDER BY created_at DESC"#,
        )
        .fetch_all(pool)
        .await
        .context("list ideas")?
    } else {
        sqlx::query(
            r#"SELECT id, title, body, workspace_root_path, project_key, status, created_at, archived_at
               FROM ideas WHERE status != 'archived' ORDER BY created_at DESC"#,
        )
        .fetch_all(pool)
        .await
        .context("list ideas (non-archived)")?
    };

    rows.into_iter()
        .map(|r| {
            parse_idea_row(
                r.get("id"),
                r.get("title"),
                r.get("body"),
                r.get("workspace_root_path"),
                r.get("project_key"),
                r.get("status"),
                r.get("created_at"),
                r.get("archived_at"),
            )
        })
        .collect()
}

pub async fn update_status(pool: &SqlitePool, id: IdeaId, status: IdeaStatus) -> Result<()> {
    let id_str = id.to_string();
    let status_str = status.to_string();
    let archived_at: Option<String> = if matches!(status, IdeaStatus::Archived) {
        Some(Utc::now().to_rfc3339())
    } else {
        None
    };

    sqlx::query(
        r#"UPDATE ideas SET status = ?1, archived_at = COALESCE(?2, archived_at) WHERE id = ?3"#,
    )
    .bind(status_str)
    .bind(archived_at)
    .bind(id_str)
    .execute(pool)
    .await
    .context("update idea status")?;
    Ok(())
}

fn parse_idea_row(
    id: String,
    title: String,
    body: String,
    workspace_root_path: Option<String>,
    project_key: Option<String>,
    status: String,
    created_at: String,
    archived_at: Option<String>,
) -> Result<Idea> {
    let idea_id: IdeaId = id.parse::<uuid::Uuid>().context("parse idea id")?.into();
    let idea_status: IdeaStatus = status.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let created_at_dt: DateTime<Utc> = DateTime::parse_from_rfc3339(&created_at)
        .context("parse idea created_at")?
        .with_timezone(&Utc);
    let archived_at_dt: Option<DateTime<Utc>> = archived_at
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .context("parse idea archived_at")
                .map(|dt| dt.with_timezone(&Utc))
        })
        .transpose()?;

    Ok(Idea {
        id: idea_id,
        title,
        body,
        workspace_root_path,
        project_key,
        status: idea_status,
        created_at: created_at_dt,
        archived_at: archived_at_dt,
    })
}
