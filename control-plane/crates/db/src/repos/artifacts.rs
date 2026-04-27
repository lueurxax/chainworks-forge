use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::artifact::{Artifact, ArtifactFormat};
use domain::ids::{ArtifactId, RunId};

pub async fn insert(pool: &SqlitePool, artifact: &Artifact) -> Result<()> {
    let mut tx = pool.begin().await.context("begin insert artifact")?;
    insert_tx(&mut tx, artifact).await?;
    tx.commit().await.context("commit insert artifact")?;
    Ok(())
}

pub async fn insert_tx(tx: &mut Transaction<'_, Sqlite>, artifact: &Artifact) -> Result<()> {
    let id = artifact.id.to_string();
    let run_id = artifact.run_id.to_string();
    let format = artifact.format.to_string();
    let created_at = artifact.created_at.to_rfc3339();
    let is_pinned: i64 = if artifact.is_pinned { 1 } else { 0 };

    sqlx::query(
        r#"
        INSERT INTO artifacts (id, run_id, stage_id, agent_id, name, contract_id, format, file_path,
                               checksum_sha256, size_bytes, provider, model, created_at, is_pinned,
                               report_kind, report_version, agent_execution_id)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
        "#,
    )
    .bind(id)
    .bind(run_id)
    .bind(&artifact.stage_id)
    .bind(&artifact.agent_id)
    .bind(&artifact.name)
    .bind(&artifact.contract_id)
    .bind(format)
    .bind(&artifact.file_path)
    .bind(&artifact.checksum_sha256)
    .bind(artifact.size_bytes)
    .bind(&artifact.provider)
    .bind(&artifact.model)
    .bind(created_at)
    .bind(is_pinned)
    .bind(&artifact.report_kind)
    .bind(artifact.report_version)
    .bind(&artifact.agent_execution_id)
    .execute(&mut **tx)
    .await
    .context("insert artifact")?;
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: ArtifactId) -> Result<Option<Artifact>> {
    let id_str = id.to_string();
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_id, agent_id, name, contract_id, format, file_path,
                  checksum_sha256, size_bytes, provider, model, created_at, is_pinned,
                  report_kind, report_version, agent_execution_id
           FROM artifacts WHERE id = ?1"#,
    )
    .bind(id_str)
    .fetch_optional(pool)
    .await
    .context("find artifact by id")?;

    row.map(|r| parse_artifact_row(&r)).transpose()
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<Artifact>> {
    let run_id_str = run_id.to_string();
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_id, agent_id, name, contract_id, format, file_path,
                  checksum_sha256, size_bytes, provider, model, created_at, is_pinned,
                  report_kind, report_version, agent_execution_id
           FROM artifacts WHERE run_id = ?1 ORDER BY created_at ASC"#,
    )
    .bind(run_id_str)
    .fetch_all(pool)
    .await
    .context("list artifacts by run")?;

    rows.iter().map(parse_artifact_row).collect()
}

pub async fn list_by_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
) -> Result<Vec<Artifact>> {
    let run_id_str = run_id.to_string();
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_id, agent_id, name, contract_id, format, file_path,
                  checksum_sha256, size_bytes, provider, model, created_at, is_pinned,
                  report_kind, report_version, agent_execution_id
           FROM artifacts WHERE run_id = ?1 AND stage_id = ?2 ORDER BY created_at ASC"#,
    )
    .bind(run_id_str)
    .bind(stage_id)
    .fetch_all(pool)
    .await
    .context("list artifacts by stage")?;

    rows.iter().map(parse_artifact_row).collect()
}

fn parse_artifact_row(r: &sqlx::sqlite::SqliteRow) -> Result<Artifact> {
    let id: String = r.get("id");
    let run_id: String = r.get("run_id");
    let format_str: String = r.get("format");
    let created_at_str: String = r.get("created_at");
    let is_pinned_val: i64 = r.get("is_pinned");

    let artifact_id: ArtifactId = id
        .parse::<uuid::Uuid>()
        .context("parse artifact id")?
        .into();
    let run_id_val: RunId = run_id
        .parse::<uuid::Uuid>()
        .context("parse artifact run_id")?
        .into();
    let format_val: ArtifactFormat = format_str.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let created_at_dt: DateTime<Utc> = DateTime::parse_from_rfc3339(&created_at_str)
        .context("parse artifact created_at")?
        .with_timezone(&Utc);

    Ok(Artifact {
        id: artifact_id,
        run_id: run_id_val,
        stage_id: r.get("stage_id"),
        agent_id: r.get("agent_id"),
        name: r.get("name"),
        contract_id: r.get("contract_id"),
        format: format_val,
        file_path: r.get("file_path"),
        checksum_sha256: r.get("checksum_sha256"),
        size_bytes: r.get("size_bytes"),
        provider: r.get("provider"),
        model: r.get("model"),
        created_at: created_at_dt,
        is_pinned: is_pinned_val != 0,
        report_kind: r.get("report_kind"),
        report_version: r.get("report_version"),
        // P017 R5 / API-003: direct execution-attempt linkage.
        agent_execution_id: r.get("agent_execution_id"),
    })
}

/// P017 R5 / API-003: list artifacts produced by a specific
/// `AgentExecution` attempt via the direct
/// `artifacts.agent_execution_id` FK. Used by MCP/GraphQL
/// `execution_attempts.artifacts` so retries by the same lead agent
/// don't cross-attach artifacts.
pub async fn list_by_agent_execution(
    pool: &SqlitePool,
    agent_execution_id: &str,
) -> Result<Vec<Artifact>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_id, agent_id, name, contract_id, format, file_path,
                  checksum_sha256, size_bytes, provider, model, created_at, is_pinned,
                  report_kind, report_version, agent_execution_id
           FROM artifacts WHERE agent_execution_id = ?1 ORDER BY created_at ASC"#,
    )
    .bind(agent_execution_id)
    .fetch_all(pool)
    .await
    .context("list artifacts by agent_execution_id")?;
    rows.iter().map(parse_artifact_row).collect()
}
