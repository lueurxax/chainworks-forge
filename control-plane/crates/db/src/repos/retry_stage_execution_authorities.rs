use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use domain::ids::{RunId, StageExecutionId};
use domain::retry_authority::{
    RetryAuthorityEntryKind, RetryAuthorityState, RetryStageExecutionAuthority,
};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

pub async fn create_active(
    pool: &SqlitePool,
    authority: &RetryStageExecutionAuthority,
) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "retry_stage_execution_authorities.create_active",
    )
    .await?;
    create_tx(&mut tx, authority).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn create_tx(
    tx: &mut Transaction<'_, Sqlite>,
    authority: &RetryStageExecutionAuthority,
) -> Result<()> {
    insert_tx(tx, authority).await
}

pub async fn create_recovered_orphan_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: impl Into<String>,
    run_id: RunId,
    stage_id: impl Into<String>,
    target_stage_execution_id: StageExecutionId,
    terminal_reason: impl Into<String>,
    now: DateTime<Utc>,
) -> Result<RetryStageExecutionAuthority> {
    let authority = RetryStageExecutionAuthority {
        id: id.into(),
        run_id,
        stage_id: stage_id.into(),
        target_stage_execution_id,
        entry_kind: RetryAuthorityEntryKind::HistoricalOrphanRecovery,
        source_command_journal_id: None,
        source_retry_work_item_id: None,
        source_invoke_work_item_id: None,
        source_agent_execution_id: None,
        authority_state: RetryAuthorityState::RecoveredOrphan,
        created_at: now,
        updated_at: now,
        terminal_reason: Some(terminal_reason.into()),
    };
    insert_tx(tx, &authority).await?;
    Ok(authority)
}

pub async fn create_active_targeted_agent_retry_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_id: &str,
    target_stage_execution_id: StageExecutionId,
    source_command_journal_id: Option<String>,
    source_retry_work_item_id: Option<String>,
    source_invoke_work_item_id: String,
    source_agent_execution_id: Option<String>,
    now: DateTime<Utc>,
) -> Result<RetryStageExecutionAuthority> {
    supersede_active_for_stage_tx(
        tx,
        run_id,
        stage_id,
        now,
        "superseded_by_new_targeted_retry",
    )
    .await?;
    let authority = RetryStageExecutionAuthority {
        id: format!("p091-retry-authority:{target_stage_execution_id}"),
        run_id,
        stage_id: stage_id.to_string(),
        target_stage_execution_id,
        entry_kind: RetryAuthorityEntryKind::TargetedAgentRetry,
        source_command_journal_id,
        source_retry_work_item_id,
        source_invoke_work_item_id: Some(source_invoke_work_item_id),
        source_agent_execution_id,
        authority_state: RetryAuthorityState::Active,
        created_at: now,
        updated_at: now,
        terminal_reason: None,
    };
    create_tx(tx, &authority).await?;
    Ok(authority)
}

pub async fn supersede_active_for_stage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: RunId,
    stage_id: &str,
    now: DateTime<Utc>,
    terminal_reason: &str,
) -> Result<u64> {
    let result = sqlx::query(
        r#"UPDATE retry_stage_execution_authorities
           SET authority_state = ?1,
               terminal_reason = ?2,
               updated_at = ?3
           WHERE run_id = ?4
             AND stage_id = ?5
             AND authority_state = ?6"#,
    )
    .bind(RetryAuthorityState::Superseded.to_string())
    .bind(terminal_reason)
    .bind(now.to_rfc3339())
    .bind(run_id.to_string())
    .bind(stage_id)
    .bind(RetryAuthorityState::Active.to_string())
    .execute(&mut **tx)
    .await
    .context("supersede active retry authority")?;
    Ok(result.rows_affected())
}

pub async fn mark_terminalized_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    now: DateTime<Utc>,
    terminal_reason: &str,
) -> Result<u64> {
    let result = sqlx::query(
        r#"UPDATE retry_stage_execution_authorities
           SET authority_state = ?1,
               terminal_reason = ?2,
               updated_at = ?3
           WHERE id = ?4
             AND authority_state = ?5"#,
    )
    .bind(RetryAuthorityState::Terminalized.to_string())
    .bind(terminal_reason)
    .bind(now.to_rfc3339())
    .bind(id)
    .bind(RetryAuthorityState::Active.to_string())
    .execute(&mut **tx)
    .await
    .context("terminalize retry authority")?;
    Ok(result.rows_affected())
}

pub async fn find_by_id(
    pool: &SqlitePool,
    id: &str,
) -> Result<Option<RetryStageExecutionAuthority>> {
    let row = sqlx::query(select_sql("WHERE id = ?1").as_str())
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("find retry authority by id")?;
    row.map(|row| parse_row(&row)).transpose()
}

pub async fn find_active_by_run_stage(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
) -> Result<Option<RetryStageExecutionAuthority>> {
    let row = sqlx::query(
        select_sql("WHERE run_id = ?1 AND stage_id = ?2 AND authority_state = 'active'").as_str(),
    )
    .bind(run_id.to_string())
    .bind(stage_id)
    .fetch_optional(pool)
    .await
    .context("find active retry authority by run/stage")?;
    row.map(|row| parse_row(&row)).transpose()
}

pub async fn find_active_by_target(
    pool: &SqlitePool,
    target_stage_execution_id: StageExecutionId,
) -> Result<Option<RetryStageExecutionAuthority>> {
    let row = sqlx::query(
        select_sql("WHERE target_stage_execution_id = ?1 AND authority_state = 'active'").as_str(),
    )
    .bind(target_stage_execution_id.to_string())
    .fetch_optional(pool)
    .await
    .context("find active retry authority by target")?;
    row.map(|row| parse_row(&row)).transpose()
}

pub async fn list_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<RetryStageExecutionAuthority>> {
    let rows = sqlx::query(select_sql("WHERE run_id = ?1 ORDER BY created_at ASC").as_str())
        .bind(run_id.to_string())
        .fetch_all(pool)
        .await
        .context("list retry authorities by run")?;
    rows.iter().map(parse_row).collect()
}

async fn insert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    authority: &RetryStageExecutionAuthority,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO retry_stage_execution_authorities
           (id, run_id, stage_id, target_stage_execution_id, entry_kind,
            source_command_journal_id, source_retry_work_item_id,
            source_invoke_work_item_id, source_agent_execution_id,
            authority_state, created_at, updated_at, terminal_reason)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
    )
    .bind(&authority.id)
    .bind(authority.run_id.to_string())
    .bind(&authority.stage_id)
    .bind(authority.target_stage_execution_id.to_string())
    .bind(authority.entry_kind.to_string())
    .bind(&authority.source_command_journal_id)
    .bind(&authority.source_retry_work_item_id)
    .bind(&authority.source_invoke_work_item_id)
    .bind(&authority.source_agent_execution_id)
    .bind(authority.authority_state.to_string())
    .bind(authority.created_at.to_rfc3339())
    .bind(authority.updated_at.to_rfc3339())
    .bind(&authority.terminal_reason)
    .execute(&mut **tx)
    .await
    .context("insert retry stage execution authority")?;
    Ok(())
}

fn select_sql(where_clause: &str) -> String {
    format!(
        r#"SELECT id, run_id, stage_id, target_stage_execution_id, entry_kind,
                  source_command_journal_id, source_retry_work_item_id,
                  source_invoke_work_item_id, source_agent_execution_id,
                  authority_state, created_at, updated_at, terminal_reason
           FROM retry_stage_execution_authorities {where_clause}"#
    )
}

fn parse_row(row: &sqlx::sqlite::SqliteRow) -> Result<RetryStageExecutionAuthority> {
    let run_id: String = row.get("run_id");
    let target: String = row.get("target_stage_execution_id");
    let entry_kind: String = row.get("entry_kind");
    let authority_state: String = row.get("authority_state");
    let created_at: String = row.get("created_at");
    let updated_at: String = row.get("updated_at");

    Ok(RetryStageExecutionAuthority {
        id: row.get("id"),
        run_id: run_id.parse().context("parse retry authority run_id")?,
        stage_id: row.get("stage_id"),
        target_stage_execution_id: target
            .parse()
            .context("parse retry authority target stage execution id")?,
        entry_kind: entry_kind
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))
            .context("parse retry authority entry kind")?,
        source_command_journal_id: row.get("source_command_journal_id"),
        source_retry_work_item_id: row.get("source_retry_work_item_id"),
        source_invoke_work_item_id: row.get("source_invoke_work_item_id"),
        source_agent_execution_id: row.get("source_agent_execution_id"),
        authority_state: authority_state
            .parse()
            .map_err(|e: String| anyhow::anyhow!(e))
            .context("parse retry authority state")?,
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .context("parse retry authority created_at")?
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)
            .context("parse retry authority updated_at")?
            .with_timezone(&Utc),
        terminal_reason: row.get("terminal_reason"),
    })
}
