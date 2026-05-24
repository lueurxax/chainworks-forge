use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::session::{
    SessionEvent, SessionEventType, SessionGeneration, SessionGenerationStatus, SessionLineage,
};

pub async fn insert_lineage(pool: &SqlitePool, lineage: &SessionLineage) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "sessions.insert_lineage").await?;
    insert_lineage_tx(&mut tx, lineage).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_lineage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage: &SessionLineage,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO session_lineages
           (id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id, active_generation_id, created_at, closed_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"#,
    )
    .bind(&lineage.id)
    .bind(&lineage.run_id)
    .bind(&lineage.agent_id)
    .bind(&lineage.lineage_id)
    .bind(&lineage.session_reuse_scope)
    .bind(&lineage.session_family_id)
    .bind(&lineage.active_generation_id)
    .bind(lineage.created_at.to_rfc3339())
    .bind(lineage.closed_at.map(|v| v.to_rfc3339()))
    .execute(&mut **tx)
    .await
    .context("insert session_lineage")?;
    Ok(())
}

pub async fn insert_generation(pool: &SqlitePool, generation: &SessionGeneration) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "sessions.insert_generation").await?;
    insert_generation_tx(&mut tx, generation).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_generation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    generation: &SessionGeneration,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO session_generations
           (id, lineage_id, generation, invocation_owner_key, provider_session_id, binding_fingerprint,
            rehydrated_from_checkpoint_artifact_id, working_directory, workspace_mode, runtime_provider,
            runtime_model, status, turn_count, estimated_input_tokens, latest_cached_input_tokens,
            latest_output_tokens, latest_model_context_window, cumulative_prompt_tokens,
            cumulative_cost_cents, created_at, last_activity_at, ended_at, end_reason)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)"#,
    )
    .bind(&generation.id)
    .bind(&generation.lineage_id)
    .bind(generation.generation)
    .bind(&generation.invocation_owner_key)
    .bind(&generation.provider_session_id)
    .bind(&generation.binding_fingerprint)
    .bind(&generation.rehydrated_from_checkpoint_artifact_id)
    .bind(&generation.working_directory)
    .bind(&generation.workspace_mode)
    .bind(&generation.runtime_provider)
    .bind(&generation.runtime_model)
    .bind(session_generation_status_to_str(&generation.status))
    .bind(generation.turn_count)
    .bind(generation.estimated_input_tokens)
    .bind(generation.latest_cached_input_tokens)
    .bind(generation.latest_output_tokens)
    .bind(generation.latest_model_context_window)
    .bind(generation.cumulative_prompt_tokens)
    .bind(generation.cumulative_cost_cents)
    .bind(generation.created_at.to_rfc3339())
    .bind(generation.last_activity_at.map(|v| v.to_rfc3339()))
    .bind(generation.ended_at.map(|v| v.to_rfc3339()))
    .bind(&generation.end_reason)
    .execute(&mut **tx)
    .await
    .context("insert session_generation")?;
    Ok(())
}

pub async fn insert_event(pool: &SqlitePool, event: &SessionEvent) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(pool, "sessions.insert_event").await?;
    insert_event_tx(&mut tx, event).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_event_tx(tx: &mut Transaction<'_, Sqlite>, event: &SessionEvent) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO session_events
           (id, lineage_id, generation_id, event_type, recorded_at, details_json)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)"#,
    )
    .bind(&event.id)
    .bind(&event.lineage_id)
    .bind(&event.generation_id)
    .bind(session_event_type_to_str(&event.event_type))
    .bind(event.recorded_at.to_rfc3339())
    .bind(&event.details_json)
    .execute(&mut **tx)
    .await
    .context("insert session_event")?;
    Ok(())
}

pub async fn list_generations_for_lineage(
    pool: &SqlitePool,
    lineage_id: &str,
) -> Result<Vec<SessionGeneration>> {
    let rows = sqlx::query(
        r#"SELECT id, lineage_id, generation, invocation_owner_key, provider_session_id,
                  binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
                  workspace_mode, runtime_provider, runtime_model, status, turn_count,
                  estimated_input_tokens, latest_cached_input_tokens, latest_output_tokens,
                  latest_model_context_window, cumulative_prompt_tokens, cumulative_cost_cents,
                  created_at, last_activity_at, ended_at, end_reason
           FROM session_generations
           WHERE lineage_id = ?1
           ORDER BY generation ASC"#,
    )
    .bind(lineage_id)
    .fetch_all(pool)
    .await
    .context("list session_generations for lineage")?;

    rows.into_iter().map(parse_generation_row).collect()
}

pub async fn list_generations_for_lineage_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage_id: &str,
) -> Result<Vec<SessionGeneration>> {
    let rows = sqlx::query(
        r#"SELECT id, lineage_id, generation, invocation_owner_key, provider_session_id,
                  binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
                  workspace_mode, runtime_provider, runtime_model, status, turn_count,
                  estimated_input_tokens, latest_cached_input_tokens, latest_output_tokens,
                  latest_model_context_window, cumulative_prompt_tokens, cumulative_cost_cents,
                  created_at, last_activity_at, ended_at, end_reason
           FROM session_generations
           WHERE lineage_id = ?1
           ORDER BY generation ASC"#,
    )
    .bind(lineage_id)
    .fetch_all(&mut **tx)
    .await
    .context("list session_generations for lineage")?;

    rows.into_iter().map(parse_generation_row).collect()
}

pub async fn find_lineage_by_run_and_key(
    pool: &SqlitePool,
    run_id: &str,
    lineage_key: &str,
) -> Result<Option<SessionLineage>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
                  active_generation_id, created_at, closed_at
           FROM session_lineages
           WHERE run_id = ?1 AND lineage_id = ?2"#,
    )
    .bind(run_id)
    .bind(lineage_key)
    .fetch_optional(pool)
    .await
    .context("find session_lineage by run and key")?;

    row.map(parse_lineage_row).transpose()
}

pub async fn find_lineage_by_id(
    pool: &SqlitePool,
    lineage_row_id: &str,
) -> Result<Option<SessionLineage>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
                  active_generation_id, created_at, closed_at
           FROM session_lineages
           WHERE id = ?1"#,
    )
    .bind(lineage_row_id)
    .fetch_optional(pool)
    .await
    .context("find session_lineage by id")?;

    row.map(parse_lineage_row).transpose()
}

pub async fn find_lineage_by_run_and_key_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run_id: &str,
    lineage_key: &str,
) -> Result<Option<SessionLineage>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
                  active_generation_id, created_at, closed_at
           FROM session_lineages
           WHERE run_id = ?1 AND lineage_id = ?2"#,
    )
    .bind(run_id)
    .bind(lineage_key)
    .fetch_optional(&mut **tx)
    .await
    .context("find session_lineage by run and key")?;

    row.map(parse_lineage_row).transpose()
}

pub async fn find_lineage_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage_row_id: &str,
) -> Result<Option<SessionLineage>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
                  active_generation_id, created_at, closed_at
           FROM session_lineages
           WHERE id = ?1"#,
    )
    .bind(lineage_row_id)
    .fetch_optional(&mut **tx)
    .await
    .context("find session_lineage by id")?;

    row.map(parse_lineage_row).transpose()
}

pub async fn find_active_generation(
    pool: &SqlitePool,
    lineage_id: &str,
) -> Result<Option<SessionGeneration>> {
    let row = sqlx::query(
        r#"SELECT g.id, g.lineage_id, g.generation, g.invocation_owner_key, g.provider_session_id,
                  g.binding_fingerprint, g.rehydrated_from_checkpoint_artifact_id, g.working_directory,
                  g.workspace_mode, g.runtime_provider, g.runtime_model, g.status, g.turn_count,
                  g.estimated_input_tokens, g.latest_cached_input_tokens, g.latest_output_tokens,
                  g.latest_model_context_window, g.cumulative_prompt_tokens, g.cumulative_cost_cents,
                  g.created_at, g.last_activity_at, g.ended_at, g.end_reason
           FROM session_generations g
           INNER JOIN session_lineages l ON l.active_generation_id = g.id
           WHERE l.id = ?1"#,
    )
    .bind(lineage_id)
    .fetch_optional(pool)
    .await
    .context("find active session_generation")?;

    row.map(parse_generation_row).transpose()
}

pub async fn find_generation_by_id(
    pool: &SqlitePool,
    generation_id: &str,
) -> Result<Option<SessionGeneration>> {
    let row = sqlx::query(
        r#"SELECT id, lineage_id, generation, invocation_owner_key, provider_session_id,
                  binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
                  workspace_mode, runtime_provider, runtime_model, status, turn_count,
                  estimated_input_tokens, latest_cached_input_tokens, latest_output_tokens,
                  latest_model_context_window, cumulative_prompt_tokens, cumulative_cost_cents,
                  created_at, last_activity_at, ended_at, end_reason
           FROM session_generations
           WHERE id = ?1"#,
    )
    .bind(generation_id)
    .fetch_optional(pool)
    .await
    .context("find session_generation by id")?;

    row.map(parse_generation_row).transpose()
}

pub async fn find_active_generation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage_id: &str,
) -> Result<Option<SessionGeneration>> {
    let row = sqlx::query(
        r#"SELECT g.id, g.lineage_id, g.generation, g.invocation_owner_key, g.provider_session_id,
                  g.binding_fingerprint, g.rehydrated_from_checkpoint_artifact_id, g.working_directory,
                  g.workspace_mode, g.runtime_provider, g.runtime_model, g.status, g.turn_count,
                  g.estimated_input_tokens, g.latest_cached_input_tokens, g.latest_output_tokens,
                  g.latest_model_context_window, g.cumulative_prompt_tokens, g.cumulative_cost_cents,
                  g.created_at, g.last_activity_at, g.ended_at, g.end_reason
           FROM session_generations g
           INNER JOIN session_lineages l ON l.active_generation_id = g.id
           WHERE l.id = ?1"#,
    )
    .bind(lineage_id)
    .fetch_optional(&mut **tx)
    .await
    .context("find active session_generation")?;

    row.map(parse_generation_row).transpose()
}

pub async fn find_generation_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    generation_id: &str,
) -> Result<Option<SessionGeneration>> {
    let row = sqlx::query(
        r#"SELECT id, lineage_id, generation, invocation_owner_key, provider_session_id,
                  binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
                  workspace_mode, runtime_provider, runtime_model, status, turn_count,
                  estimated_input_tokens, latest_cached_input_tokens, latest_output_tokens,
                  latest_model_context_window, cumulative_prompt_tokens, cumulative_cost_cents,
                  created_at, last_activity_at, ended_at, end_reason
           FROM session_generations
           WHERE id = ?1"#,
    )
    .bind(generation_id)
    .fetch_optional(&mut **tx)
    .await
    .context("find session_generation by id")?;

    row.map(parse_generation_row).transpose()
}

pub async fn next_generation_number(pool: &SqlitePool, lineage_id: &str) -> Result<i64> {
    let row = sqlx::query(
        r#"SELECT COALESCE(MAX(generation), 0) + 1 AS next_generation
           FROM session_generations
           WHERE lineage_id = ?1"#,
    )
    .bind(lineage_id)
    .fetch_one(pool)
    .await
    .context("compute next session generation number")?;
    Ok(row.get("next_generation"))
}

pub async fn next_generation_number_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage_id: &str,
) -> Result<i64> {
    let row = sqlx::query(
        r#"SELECT COALESCE(MAX(generation), 0) + 1 AS next_generation
           FROM session_generations
           WHERE lineage_id = ?1"#,
    )
    .bind(lineage_id)
    .fetch_one(&mut **tx)
    .await
    .context("compute next session generation number")?;
    Ok(row.get("next_generation"))
}

pub async fn set_active_generation(
    pool: &SqlitePool,
    lineage_id: &str,
    generation_id: Option<&str>,
) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "sessions.set_active_generation").await?;
    set_active_generation_tx(&mut tx, lineage_id, generation_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn set_active_generation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    lineage_id: &str,
    generation_id: Option<&str>,
) -> Result<()> {
    sqlx::query(r#"UPDATE session_lineages SET active_generation_id = ?1 WHERE id = ?2"#)
        .bind(generation_id)
        .bind(lineage_id)
        .execute(&mut **tx)
        .await
        .context("set active generation on lineage")?;
    Ok(())
}

pub async fn end_generation(
    pool: &SqlitePool,
    generation_id: &str,
    status: SessionGenerationStatus,
    end_reason: &str,
    ended_at: DateTime<Utc>,
) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "sessions.end_generation").await?;
    end_generation_tx(&mut tx, generation_id, status, end_reason, ended_at).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn end_generation_tx(
    tx: &mut Transaction<'_, Sqlite>,
    generation_id: &str,
    status: SessionGenerationStatus,
    end_reason: &str,
    ended_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE session_generations
           SET status = ?1, end_reason = ?2, ended_at = ?3
           WHERE id = ?4"#,
    )
    .bind(session_generation_status_to_str(&status))
    .bind(end_reason)
    .bind(ended_at.to_rfc3339())
    .bind(generation_id)
    .execute(&mut **tx)
    .await
    .context("end session generation")?;
    Ok(())
}

pub async fn update_generation_usage(
    pool: &SqlitePool,
    generation_id: &str,
    provider_session_id: &str,
    turn_count: i64,
    prompt_tokens_increment: i64,
    cost_cents_increment: i64,
    estimated_input_tokens: i64,
    latest_cached_input_tokens: Option<i64>,
    latest_output_tokens: Option<i64>,
    latest_model_context_window: Option<i64>,
    last_activity_at: DateTime<Utc>,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "sessions.update_generation_usage",
        sqlx::query(
            r#"UPDATE session_generations
           SET provider_session_id = ?1,
               turn_count = ?2,
               estimated_input_tokens = ?3,
               latest_cached_input_tokens = ?4,
               latest_output_tokens = ?5,
               latest_model_context_window = ?6,
               cumulative_prompt_tokens = cumulative_prompt_tokens + ?7,
               cumulative_cost_cents = cumulative_cost_cents + ?8,
               last_activity_at = ?9
           WHERE id = ?10"#,
        )
        .bind(provider_session_id)
        .bind(turn_count)
        .bind(estimated_input_tokens)
        .bind(latest_cached_input_tokens)
        .bind(latest_output_tokens)
        .bind(latest_model_context_window)
        .bind(prompt_tokens_increment)
        .bind(cost_cents_increment)
        .bind(last_activity_at.to_rfc3339())
        .bind(generation_id)
    )
    .context("update session generation usage")?;
    Ok(())
}

pub async fn touch_generation_activity(
    pool: &SqlitePool,
    generation_id: &str,
    last_activity_at: DateTime<Utc>,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "sessions.touch_generation_activity",
        sqlx::query(
            r#"UPDATE session_generations
           SET last_activity_at = ?1
           WHERE id = ?2
             AND status = 'active'"#,
        )
        .bind(last_activity_at.to_rfc3339())
        .bind(generation_id)
    )
    .context("touch session generation activity")?;
    Ok(())
}

pub async fn count_generation_events(
    pool: &SqlitePool,
    generation_id: &str,
    event_type: SessionEventType,
) -> Result<i64> {
    let row = sqlx::query(
        r#"SELECT COUNT(*) AS event_count
           FROM session_events
           WHERE generation_id = ?1
             AND event_type = ?2"#,
    )
    .bind(generation_id)
    .bind(session_event_type_to_str(&event_type))
    .fetch_one(pool)
    .await
    .context("count session generation events")?;
    Ok(row.get("event_count"))
}

pub async fn count_generation_events_for_agent_execution(
    pool: &SqlitePool,
    generation_id: &str,
    event_type: SessionEventType,
    agent_execution_id: &str,
) -> Result<i64> {
    let row = sqlx::query(
        r#"SELECT COUNT(*) AS event_count
           FROM session_events
           WHERE generation_id = ?1
             AND event_type = ?2
             AND json_extract(details_json, '$.agent_execution_id') = ?3"#,
    )
    .bind(generation_id)
    .bind(session_event_type_to_str(&event_type))
    .bind(agent_execution_id)
    .fetch_one(pool)
    .await
    .context("count session generation events for agent execution")?;
    Ok(row.get("event_count"))
}

pub async fn count_generation_events_tx(
    tx: &mut Transaction<'_, Sqlite>,
    generation_id: &str,
    event_type: SessionEventType,
) -> Result<i64> {
    let row = sqlx::query(
        r#"SELECT COUNT(*) AS event_count
           FROM session_events
           WHERE generation_id = ?1
             AND event_type = ?2"#,
    )
    .bind(generation_id)
    .bind(session_event_type_to_str(&event_type))
    .fetch_one(&mut **tx)
    .await
    .context("count session generation events")?;
    Ok(row.get("event_count"))
}

pub async fn update_generation_runtime_session(
    pool: &SqlitePool,
    generation_id: &str,
    provider_session_id: &str,
    turn_count: i64,
) -> Result<()> {
    crate::execute_repository_write!(
        pool,
        "sessions.update_generation_runtime_session",
        sqlx::query(
            r#"UPDATE session_generations
           SET provider_session_id = ?1, turn_count = ?2
           WHERE id = ?3"#,
        )
        .bind(provider_session_id)
        .bind(turn_count)
        .bind(generation_id)
    )
    .context("update session generation runtime session")?;
    Ok(())
}

/// P066 T14: Return all session generation IDs that are currently live
/// (active_generation_id IS NOT NULL AND lineage not closed).
/// Used by startup recovery to identify orphan toolchain session-scoped roots.
pub async fn list_live_session_generation_ids(
    pool: &SqlitePool,
) -> Result<std::collections::HashSet<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"SELECT active_generation_id
           FROM session_lineages
           WHERE active_generation_id IS NOT NULL
             AND closed_at IS NULL"#,
    )
    .fetch_all(pool)
    .await
    .context("list live session generation ids")?;

    Ok(rows.into_iter().map(|(id,)| id).collect())
}

fn parse_generation_row(row: sqlx::sqlite::SqliteRow) -> Result<SessionGeneration> {
    let status: String = row.get("status");
    let created_at: String = row.get("created_at");
    let last_activity_at: Option<String> = row.get("last_activity_at");
    let ended_at: Option<String> = row.get("ended_at");
    Ok(SessionGeneration {
        id: row.get("id"),
        lineage_id: row.get("lineage_id"),
        generation: row.get("generation"),
        invocation_owner_key: row.get("invocation_owner_key"),
        provider_session_id: row.get("provider_session_id"),
        binding_fingerprint: row.get("binding_fingerprint"),
        rehydrated_from_checkpoint_artifact_id: row.get("rehydrated_from_checkpoint_artifact_id"),
        working_directory: row.get("working_directory"),
        workspace_mode: row.get("workspace_mode"),
        runtime_provider: row.get("runtime_provider"),
        runtime_model: row.get("runtime_model"),
        status: session_generation_status_from_str(&status)?,
        turn_count: row.get("turn_count"),
        estimated_input_tokens: row.get("estimated_input_tokens"),
        latest_cached_input_tokens: row.get("latest_cached_input_tokens"),
        latest_output_tokens: row.get("latest_output_tokens"),
        latest_model_context_window: row.get("latest_model_context_window"),
        cumulative_prompt_tokens: row.get("cumulative_prompt_tokens"),
        cumulative_cost_cents: row.get("cumulative_cost_cents"),
        created_at: parse_dt(&created_at)?,
        last_activity_at: last_activity_at.map(|v| parse_dt(&v)).transpose()?,
        ended_at: ended_at.map(|v| parse_dt(&v)).transpose()?,
        end_reason: row.get("end_reason"),
    })
}

fn parse_lineage_row(row: sqlx::sqlite::SqliteRow) -> Result<SessionLineage> {
    let created_at: String = row.get("created_at");
    let closed_at: Option<String> = row.get("closed_at");
    Ok(SessionLineage {
        id: row.get("id"),
        run_id: row.get("run_id"),
        agent_id: row.get("agent_id"),
        lineage_id: row.get("lineage_id"),
        session_reuse_scope: row.get("session_reuse_scope"),
        session_family_id: row.get("session_family_id"),
        active_generation_id: row.get("active_generation_id"),
        created_at: parse_dt(&created_at)?,
        closed_at: closed_at.map(|v| parse_dt(&v)).transpose()?,
    })
}

fn parse_dt(raw: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(raw)
        .context("parse session timestamp")?
        .with_timezone(&Utc))
}

pub fn session_generation_status_to_str(status: &SessionGenerationStatus) -> &'static str {
    match status {
        SessionGenerationStatus::Active => "active",
        SessionGenerationStatus::Invalidated => "invalidated",
        SessionGenerationStatus::Closed => "closed",
        SessionGenerationStatus::Reset => "reset",
    }
}

fn session_generation_status_from_str(status: &str) -> Result<SessionGenerationStatus> {
    match status {
        "active" => Ok(SessionGenerationStatus::Active),
        "invalidated" => Ok(SessionGenerationStatus::Invalidated),
        "closed" => Ok(SessionGenerationStatus::Closed),
        "reset" => Ok(SessionGenerationStatus::Reset),
        other => Err(anyhow::anyhow!("Unknown SessionGenerationStatus: {other}")),
    }
}

fn session_event_type_to_str(event_type: &SessionEventType) -> &'static str {
    match event_type {
        SessionEventType::Created => "created",
        SessionEventType::Reused => "reused",
        SessionEventType::Invalidated => "invalidated",
        SessionEventType::Closed => "closed",
        SessionEventType::OperatorReset => "operator_reset",
        SessionEventType::BudgetExceeded => "budget_exceeded",
        SessionEventType::Compacted => "compacted",
        SessionEventType::OutputContractRepairStarted => "output_contract_repair_started",
        SessionEventType::OutputContractRepairSucceeded => "output_contract_repair_succeeded",
        SessionEventType::OutputContractRepairFailed => "output_contract_repair_failed",
        SessionEventType::OutputContractRepairSkipped => "output_contract_repair_skipped",
        SessionEventType::CodeWriterCompletionStarted => "code_writer_completion_started",
        SessionEventType::CodeWriterCompletionSucceeded => "code_writer_completion_succeeded",
        SessionEventType::CodeWriterCompletionFailed => "code_writer_completion_failed",
    }
}

// ── P046: paginated read helpers ─────────────────────────────────────────────

pub struct SessionLineagePage {
    pub items: Vec<SessionLineage>,
    pub has_next_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

pub struct SessionGenerationPage {
    pub items: Vec<SessionGeneration>,
    pub has_next_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
}

pub struct SessionEventPage {
    pub items: Vec<SessionEvent>,
    pub has_next_page: bool,
    pub start_cursor: Option<String>,
    pub end_cursor: Option<String>,
    /// The generationId filter active when this page was produced; empty string means no filter.
    /// Cursors are bound to this value so reuse under a different filter is rejected.
    pub gen_id_filter: String,
}

pub struct SessionKpiSummary {
    pub lineage_count: i64,
    pub generation_count: i64,
    pub active_generation_count: i64,
    pub closed_generation_count: i64,
    pub reset_generation_count: i64,
    pub invalidated_generation_count: i64,
    pub reuse_event_count: i64,
    pub operator_reset_event_count: i64,
    pub total_turn_count: i64,
    pub total_prompt_tokens: i64,
    pub total_cost_cents: i64,
    pub latest_activity_at: Option<DateTime<Utc>>,
    pub stale_active_generation_count: i64,
}

pub struct SessionHealthData {
    pub lineages: Vec<SessionLineage>,
    pub active_generations: Vec<SessionGeneration>,
    pub recent_operator_reset_events: Vec<SessionEvent>,
    pub recent_repair_failed_events: Vec<SessionEvent>,
    /// Operator reset events within the last 24 hours (for the ≥3 threshold check).
    pub operator_reset_events_24h: Vec<SessionEvent>,
    /// Whether the owning run is in a terminal state (completed/failed/cancelled).
    /// Stale-active-generation warnings are suppressed for terminal runs.
    pub run_is_terminal: bool,
    /// Number of generation rows for this run whose lineage_id does not exist in session_lineages.
    /// Non-zero indicates a data integrity problem (generation_without_lineage health condition).
    pub orphan_generation_count: i64,
}

fn encode_cursor_parts(parts: &[String]) -> String {
    use base64::Engine as _;
    let raw = parts.join("\x00");
    base64::engine::general_purpose::STANDARD.encode(raw.as_bytes())
}

fn decode_cursor_parts(cursor: &str, expected_len: usize) -> Option<Vec<String>> {
    use base64::Engine as _;
    if cursor.is_empty() {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cursor)
        .ok()?;
    let raw = String::from_utf8(bytes).ok()?;
    let parts: Vec<String> = raw.split('\x00').map(str::to_string).collect();
    (parts.len() == expected_len).then_some(parts)
}

/// Encode an event cursor bound to its lineage and generation-filter dimensions.
/// Format: (lineage_id, gen_id_filter_or_empty, recorded_at, id) — 4 parts.
/// `gen_id_filter_or_empty` is the active generationId filter, or "" for an unfiltered query.
pub fn encode_session_cursor(
    lineage_id: &str,
    gen_id_filter_or_empty: &str,
    created_at: &str,
    id: &str,
) -> String {
    encode_cursor_parts(&[
        lineage_id.to_string(),
        gen_id_filter_or_empty.to_string(),
        created_at.to_string(),
        id.to_string(),
    ])
}

/// Decode a cursor produced by `encode_session_cursor`. Returns None for invalid cursors.
/// Returns (lineage_id, gen_id_filter_or_empty, recorded_at, id).
pub fn decode_session_cursor(cursor: &str) -> Option<(String, String, String, String)> {
    let parts = decode_cursor_parts(cursor, 4)?;
    Some((
        parts[0].clone(),
        parts[1].clone(),
        parts[2].clone(),
        parts[3].clone(),
    ))
}

/// Cursor format: (run_id, agent_id, lineage_id, created_at, id) — 5 parts.
/// The run_id prefix binds the cursor to the owning run so cross-run reuse is rejected.
pub fn encode_session_lineage_cursor(lineage: &SessionLineage) -> String {
    encode_cursor_parts(&[
        lineage.run_id.clone(),
        lineage.agent_id.clone(),
        lineage.lineage_id.clone(),
        lineage.created_at.to_rfc3339(),
        lineage.id.clone(),
    ])
}

/// Returns (run_id, agent_id, lineage_id, created_at, id) or None for invalid cursors.
pub fn decode_session_lineage_cursor(
    cursor: &str,
) -> Option<(String, String, String, String, String)> {
    let parts = decode_cursor_parts(cursor, 5)?;
    Some((
        parts[0].clone(),
        parts[1].clone(),
        parts[2].clone(),
        parts[3].clone(),
        parts[4].clone(),
    ))
}

/// Encodes a generation cursor bound to its lineage_id filter.
/// Format: (lineage_id, generation, created_at, id) — 4 parts.
pub fn encode_session_generation_cursor(generation: &SessionGeneration) -> String {
    encode_cursor_parts(&[
        generation.lineage_id.clone(),
        generation.generation.to_string(),
        generation.created_at.to_rfc3339(),
        generation.id.clone(),
    ])
}

/// Returns (lineage_id, generation, created_at, id) or None for invalid cursors.
pub fn decode_session_generation_cursor(cursor: &str) -> Option<(String, i64, String, String)> {
    let parts = decode_cursor_parts(cursor, 4)?;
    Some((
        parts[0].clone(),
        parts[1].parse().ok()?,
        parts[2].clone(),
        parts[3].clone(),
    ))
}

pub async fn list_lineages_for_run_paginated(
    pool: &SqlitePool,
    run_id: &str,
    first: i64,
    after_cursor: Option<&str>,
) -> Result<SessionLineagePage> {
    if let Some(c) = after_cursor {
        match decode_session_lineage_cursor(c) {
            Some((cursor_run_id, _, _, _, _)) if cursor_run_id == run_id => {}
            Some(_) => anyhow::bail!("invalid cursor"), // wrong run
            None => anyhow::bail!("invalid cursor"),
        }
    }
    let limit = first + 1;
    let rows = if let Some(cursor) = after_cursor {
        let (_, agent_id, lineage_key, ts, cid) =
            decode_session_lineage_cursor(cursor).expect("cursor validated above");
        sqlx::query(
            r#"SELECT id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
                      active_generation_id, created_at, closed_at
               FROM session_lineages
               WHERE run_id = ?1
                 AND (
                      agent_id > ?2
                   OR (agent_id = ?2 AND lineage_id > ?3)
                   OR (agent_id = ?2 AND lineage_id = ?3 AND created_at > ?4)
                   OR (agent_id = ?2 AND lineage_id = ?3 AND created_at = ?4 AND id > ?5)
                 )
               ORDER BY agent_id ASC, lineage_id ASC, created_at ASC, id ASC
               LIMIT ?6"#,
        )
        .bind(run_id)
        .bind(&agent_id)
        .bind(&lineage_key)
        .bind(&ts)
        .bind(&cid)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list session lineages paginated")?
    } else {
        sqlx::query(
            r#"SELECT id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
                      active_generation_id, created_at, closed_at
               FROM session_lineages
               WHERE run_id = ?1
               ORDER BY agent_id ASC, lineage_id ASC, created_at ASC, id ASC
               LIMIT ?2"#,
        )
        .bind(run_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list session lineages paginated")?
    };

    let has_next_page = rows.len() as i64 > first;
    let all_items: Vec<SessionLineage> = rows
        .into_iter()
        .take(first as usize)
        .map(parse_lineage_row)
        .collect::<Result<Vec<_>>>()?;

    let start_cursor = all_items.first().map(encode_session_lineage_cursor);
    let end_cursor = all_items.last().map(encode_session_lineage_cursor);

    Ok(SessionLineagePage {
        items: all_items,
        has_next_page,
        start_cursor,
        end_cursor,
    })
}

/// Returns the run_id that owns a given lineage row id, or None if not found.
pub async fn find_lineage_owner_run(
    pool: &SqlitePool,
    lineage_row_id: &str,
) -> Result<Option<String>> {
    let row = sqlx::query("SELECT run_id FROM session_lineages WHERE id = ?1")
        .bind(lineage_row_id)
        .fetch_optional(pool)
        .await
        .context("find lineage owner run")?;
    Ok(row.map(|r| r.get::<String, _>("run_id")))
}

pub async fn list_generations_for_lineage_paginated(
    pool: &SqlitePool,
    lineage_id: &str,
    first: i64,
    after_cursor: Option<&str>,
) -> Result<SessionGenerationPage> {
    if let Some(c) = after_cursor {
        match decode_session_generation_cursor(c) {
            Some((cursor_lineage_id, _, _, _)) if cursor_lineage_id == lineage_id => {}
            Some(_) => anyhow::bail!("invalid cursor"), // mismatched filter
            None => anyhow::bail!("invalid cursor"),
        }
    }
    let limit = first + 1;
    let rows = if let Some(cursor) = after_cursor {
        let (_, generation, ts, cid) =
            decode_session_generation_cursor(cursor).expect("cursor validated above");
        sqlx::query(
            r#"SELECT id, lineage_id, generation, invocation_owner_key, provider_session_id,
                      binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
                      workspace_mode, runtime_provider, runtime_model, status, turn_count,
                      estimated_input_tokens, latest_cached_input_tokens, latest_output_tokens,
                      latest_model_context_window, cumulative_prompt_tokens, cumulative_cost_cents,
                      created_at, last_activity_at, ended_at, end_reason
               FROM session_generations
               WHERE lineage_id = ?1
                 AND (
                      generation > ?2
                   OR (generation = ?2 AND created_at > ?3)
                   OR (generation = ?2 AND created_at = ?3 AND id > ?4)
                 )
               ORDER BY generation ASC, created_at ASC, id ASC
               LIMIT ?5"#,
        )
        .bind(lineage_id)
        .bind(generation)
        .bind(&ts)
        .bind(&cid)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list generations paginated")?
    } else {
        sqlx::query(
            r#"SELECT id, lineage_id, generation, invocation_owner_key, provider_session_id,
                      binding_fingerprint, rehydrated_from_checkpoint_artifact_id, working_directory,
                      workspace_mode, runtime_provider, runtime_model, status, turn_count,
                      estimated_input_tokens, latest_cached_input_tokens, latest_output_tokens,
                      latest_model_context_window, cumulative_prompt_tokens, cumulative_cost_cents,
                      created_at, last_activity_at, ended_at, end_reason
               FROM session_generations
               WHERE lineage_id = ?1
               ORDER BY generation ASC, created_at ASC, id ASC
               LIMIT ?2"#,
        )
        .bind(lineage_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list generations paginated")?
    };

    let has_next_page = rows.len() as i64 > first;
    let all_items: Vec<SessionGeneration> = rows
        .into_iter()
        .take(first as usize)
        .map(parse_generation_row)
        .collect::<Result<Vec<_>>>()?;

    let start_cursor = all_items.first().map(encode_session_generation_cursor);
    let end_cursor = all_items.last().map(encode_session_generation_cursor);

    Ok(SessionGenerationPage {
        items: all_items,
        has_next_page,
        start_cursor,
        end_cursor,
    })
}

/// Returns (generation, owning_run_id) or None if not found.
pub async fn find_generation_with_lineage_owner(
    pool: &SqlitePool,
    generation_id: &str,
) -> Result<Option<(SessionGeneration, String)>> {
    let row = sqlx::query(
        r#"SELECT g.id, g.lineage_id, g.generation, g.invocation_owner_key, g.provider_session_id,
                  g.binding_fingerprint, g.rehydrated_from_checkpoint_artifact_id, g.working_directory,
                  g.workspace_mode, g.runtime_provider, g.runtime_model, g.status, g.turn_count,
                  g.estimated_input_tokens, g.latest_cached_input_tokens, g.latest_output_tokens,
                  g.latest_model_context_window, g.cumulative_prompt_tokens, g.cumulative_cost_cents,
                  g.created_at, g.last_activity_at, g.ended_at, g.end_reason,
                  l.run_id AS owning_run_id
           FROM session_generations g
           JOIN session_lineages l ON g.lineage_id = l.id
           WHERE g.id = ?1"#,
    )
    .bind(generation_id)
    .fetch_optional(pool)
    .await
    .context("find generation with lineage owner")?;

    match row {
        None => Ok(None),
        Some(r) => {
            let run_id: String = r.get("owning_run_id");
            let gen = parse_generation_row(r)?;
            Ok(Some((gen, run_id)))
        }
    }
}

pub async fn list_events_paginated(
    pool: &SqlitePool,
    lineage_id: &str,
    generation_id_filter: Option<&str>,
    first: i64,
    after_cursor: Option<&str>,
) -> Result<SessionEventPage> {
    let gen_id_filter_key = generation_id_filter.unwrap_or("");
    if let Some(c) = after_cursor {
        match decode_session_cursor(c) {
            Some((cursor_lid, cursor_gen, _, _))
                if cursor_lid == lineage_id && cursor_gen == gen_id_filter_key => {}
            Some(_) => anyhow::bail!("invalid cursor"), // mismatched filter or lineage
            None => anyhow::bail!("invalid cursor"),
        }
    }
    let limit = first + 1;
    // Strip lineage_id + gen filter from decoded cursor; use only ts+id for WHERE clause.
    let cursor_parts = after_cursor
        .and_then(decode_session_cursor)
        .map(|(_, _, ts, id)| (ts, id));

    let rows = match (cursor_parts, generation_id_filter) {
        (Some((ts, cid)), Some(gen_id)) => sqlx::query(
            r#"SELECT id, lineage_id, generation_id, event_type, recorded_at, details_json
               FROM session_events
               WHERE lineage_id = ?1 AND generation_id = ?2
                 AND (recorded_at > ?3 OR (recorded_at = ?3 AND id > ?4))
               ORDER BY recorded_at ASC, id ASC LIMIT ?5"#,
        )
        .bind(lineage_id)
        .bind(gen_id)
        .bind(&ts)
        .bind(&cid)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list events paginated")?,

        (Some((ts, cid)), None) => sqlx::query(
            r#"SELECT id, lineage_id, generation_id, event_type, recorded_at, details_json
               FROM session_events
               WHERE lineage_id = ?1
                 AND (recorded_at > ?2 OR (recorded_at = ?2 AND id > ?3))
               ORDER BY recorded_at ASC, id ASC LIMIT ?4"#,
        )
        .bind(lineage_id)
        .bind(&ts)
        .bind(&cid)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list events paginated")?,

        (None, Some(gen_id)) => sqlx::query(
            r#"SELECT id, lineage_id, generation_id, event_type, recorded_at, details_json
               FROM session_events
               WHERE lineage_id = ?1 AND generation_id = ?2
               ORDER BY recorded_at ASC, id ASC LIMIT ?3"#,
        )
        .bind(lineage_id)
        .bind(gen_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list events paginated")?,

        (None, None) => sqlx::query(
            r#"SELECT id, lineage_id, generation_id, event_type, recorded_at, details_json
               FROM session_events
               WHERE lineage_id = ?1
               ORDER BY recorded_at ASC, id ASC LIMIT ?2"#,
        )
        .bind(lineage_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .context("list events paginated")?,
    };

    let has_next_page = rows.len() as i64 > first;
    let all_items: Vec<SessionEvent> = rows
        .into_iter()
        .take(first as usize)
        .map(parse_event_row)
        .collect::<Result<Vec<_>>>()?;

    let start_cursor = all_items.first().map(|e| {
        encode_session_cursor(
            &e.lineage_id,
            gen_id_filter_key,
            &e.recorded_at.to_rfc3339(),
            &e.id,
        )
    });
    let end_cursor = all_items.last().map(|e| {
        encode_session_cursor(
            &e.lineage_id,
            gen_id_filter_key,
            &e.recorded_at.to_rfc3339(),
            &e.id,
        )
    });

    Ok(SessionEventPage {
        items: all_items,
        has_next_page,
        start_cursor,
        end_cursor,
        gen_id_filter: gen_id_filter_key.to_string(),
    })
}

pub async fn aggregate_kpis_for_run(pool: &SqlitePool, run_id: &str) -> Result<SessionKpiSummary> {
    let stale_threshold = (chrono::Utc::now() - chrono::Duration::minutes(15)).to_rfc3339();
    let row = sqlx::query(
        r#"SELECT
             COUNT(DISTINCT l.id) AS lineage_count,
             COUNT(g.id) AS generation_count,
             SUM(CASE WHEN g.status = 'active' THEN 1 ELSE 0 END) AS active_generation_count,
             SUM(CASE WHEN g.status = 'closed' THEN 1 ELSE 0 END) AS closed_generation_count,
             SUM(CASE WHEN g.status = 'reset' THEN 1 ELSE 0 END) AS reset_generation_count,
             SUM(CASE WHEN g.status = 'invalidated' THEN 1 ELSE 0 END) AS invalidated_generation_count,
             COALESCE(SUM(g.turn_count), 0) AS total_turn_count,
             COALESCE(SUM(g.cumulative_prompt_tokens), 0) AS total_prompt_tokens,
             COALESCE(SUM(g.cumulative_cost_cents), 0) AS total_cost_cents,
             MAX(g.last_activity_at) AS latest_activity_at,
             SUM(CASE WHEN g.status = 'active' AND g.last_activity_at IS NOT NULL AND g.last_activity_at < ?2 THEN 1 ELSE 0 END) AS stale_active_generation_count
           FROM session_lineages l
           LEFT JOIN session_generations g ON g.lineage_id = l.id
           WHERE l.run_id = ?1"#,
    )
    .bind(run_id)
    .bind(&stale_threshold)
    .fetch_one(pool)
    .await
    .context("aggregate session kpis")?;

    let latest_activity_str: Option<String> = row.try_get("latest_activity_at").ok().flatten();
    let latest_activity_at = latest_activity_str.as_deref().map(parse_dt).transpose()?;

    // Count reuse and operator reset events as separate lightweight queries.
    let reuse_row = sqlx::query(
        r#"SELECT COUNT(*) AS cnt FROM session_events e
           JOIN session_lineages l ON e.lineage_id = l.id
           WHERE l.run_id = ?1 AND e.event_type = 'reused'"#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .context("count reuse events for kpi")?;
    let reuse_event_count: i64 = reuse_row.get("cnt");

    let reset_ev_row = sqlx::query(
        r#"SELECT COUNT(*) AS cnt FROM session_events e
           JOIN session_lineages l ON e.lineage_id = l.id
           WHERE l.run_id = ?1 AND e.event_type = 'operator_reset'"#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .context("count operator reset events for kpi")?;
    let operator_reset_event_count: i64 = reset_ev_row.get("cnt");

    Ok(SessionKpiSummary {
        lineage_count: row.get("lineage_count"),
        generation_count: row.get("generation_count"),
        active_generation_count: row
            .get::<Option<i64>, _>("active_generation_count")
            .unwrap_or(0),
        closed_generation_count: row
            .get::<Option<i64>, _>("closed_generation_count")
            .unwrap_or(0),
        reset_generation_count: row
            .get::<Option<i64>, _>("reset_generation_count")
            .unwrap_or(0),
        invalidated_generation_count: row
            .get::<Option<i64>, _>("invalidated_generation_count")
            .unwrap_or(0),
        reuse_event_count,
        operator_reset_event_count,
        total_turn_count: row.get("total_turn_count"),
        total_prompt_tokens: row.get("total_prompt_tokens"),
        total_cost_cents: row.get("total_cost_cents"),
        latest_activity_at,
        stale_active_generation_count: row
            .get::<Option<i64>, _>("stale_active_generation_count")
            .unwrap_or(0),
    })
}

pub async fn load_health_data_for_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<SessionHealthData> {
    let lineages = sqlx::query(
        r#"SELECT id, run_id, agent_id, lineage_id, session_reuse_scope, session_family_id,
                  active_generation_id, created_at, closed_at
           FROM session_lineages WHERE run_id = ?1 ORDER BY created_at, id"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("load lineages for health")?
    .into_iter()
    .map(parse_lineage_row)
    .collect::<Result<Vec<_>>>()?;

    // Load all generations referenced by active_generation_id regardless of their own status,
    // so health can detect invalidated_active_generation (active_generation_id points to
    // a generation whose status is INVALIDATED or RESET).
    let active_generations = sqlx::query(
        r#"SELECT g.id, g.lineage_id, g.generation, g.invocation_owner_key, g.provider_session_id,
                  g.binding_fingerprint, g.rehydrated_from_checkpoint_artifact_id, g.working_directory,
                  g.workspace_mode, g.runtime_provider, g.runtime_model, g.status, g.turn_count,
                  g.estimated_input_tokens, g.latest_cached_input_tokens, g.latest_output_tokens,
                  g.latest_model_context_window, g.cumulative_prompt_tokens, g.cumulative_cost_cents,
                  g.created_at, g.last_activity_at, g.ended_at, g.end_reason
           FROM session_generations g
           JOIN session_lineages l ON g.lineage_id = l.id
           WHERE l.run_id = ?1 AND l.active_generation_id = g.id"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("load active generations for health")?
    .into_iter()
    .map(parse_generation_row)
    .collect::<Result<Vec<_>>>()?;

    let threshold_30m_ts = (chrono::Utc::now() - chrono::Duration::minutes(30)).to_rfc3339();
    let threshold_24h_ts = (chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339();

    let recent_operator_reset_events = sqlx::query(
        r#"SELECT e.id, e.lineage_id, e.generation_id, e.event_type, e.recorded_at, e.details_json
           FROM session_events e
           JOIN session_lineages l ON e.lineage_id = l.id
           WHERE l.run_id = ?1 AND e.event_type = 'operator_reset' AND e.recorded_at >= ?2
           ORDER BY e.recorded_at, e.id"#,
    )
    .bind(run_id)
    .bind(&threshold_30m_ts)
    .fetch_all(pool)
    .await
    .context("load operator reset events for health")?
    .into_iter()
    .map(parse_event_row)
    .collect::<Result<Vec<_>>>()?;

    let recent_repair_failed_events = sqlx::query(
        r#"SELECT e.id, e.lineage_id, e.generation_id, e.event_type, e.recorded_at, e.details_json
           FROM session_events e
           JOIN session_lineages l ON e.lineage_id = l.id
           WHERE l.run_id = ?1 AND e.event_type = 'output_contract_repair_failed' AND e.recorded_at >= ?2
           ORDER BY e.recorded_at, e.id"#,
    )
    .bind(run_id)
    .bind(&threshold_30m_ts)
    .fetch_all(pool)
    .await
    .context("load repair failed events for health")?
    .into_iter()
    .map(parse_event_row)
    .collect::<Result<Vec<_>>>()?;

    let operator_reset_events_24h = sqlx::query(
        r#"SELECT e.id, e.lineage_id, e.generation_id, e.event_type, e.recorded_at, e.details_json
           FROM session_events e
           JOIN session_lineages l ON e.lineage_id = l.id
           WHERE l.run_id = ?1 AND e.event_type = 'operator_reset' AND e.recorded_at >= ?2
           ORDER BY e.recorded_at, e.id"#,
    )
    .bind(run_id)
    .bind(&threshold_24h_ts)
    .fetch_all(pool)
    .await
    .context("load 24h operator reset events for health")?
    .into_iter()
    .map(parse_event_row)
    .collect::<Result<Vec<_>>>()?;

    // Determine if the run is terminal (completed/failed/cancelled/cancelling).
    let run_is_terminal = sqlx::query("SELECT status FROM runs WHERE id = ?1")
        .bind(run_id)
        .fetch_optional(pool)
        .await
        .context("load run status for health")?
        .map(|r| {
            let status: String = r.get("status");
            matches!(
                status.as_str(),
                "completed" | "failed" | "cancelled" | "cancelling"
            )
        })
        .unwrap_or(false);

    // Run-scoped orphan probe: count generation rows that were created for this run
    // (via agent_executions → stage_executions.run_id) but whose lineage row no longer
    // exists in session_lineages. Uses agent_executions for run-scoping rather than
    // session_lineages, which avoids the self-contradictory pattern of querying a table
    // for IDs and then checking they don't exist in that same table.
    let orphan_generation_count: i64 = sqlx::query(
        r#"SELECT COUNT(*) as cnt FROM session_generations g
           WHERE g.lineage_id IN (
               SELECT DISTINCT ae.session_lineage_id
               FROM agent_executions ae
               JOIN stage_executions se ON ae.stage_execution_id = se.id
               WHERE se.run_id = ?1
                 AND ae.session_lineage_id IS NOT NULL
           )
           AND NOT EXISTS (
               SELECT 1 FROM session_lineages sl WHERE sl.id = g.lineage_id
           )"#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .map(|r| r.get::<i64, _>("cnt"))
    .unwrap_or(0);

    Ok(SessionHealthData {
        lineages,
        active_generations,
        recent_operator_reset_events,
        recent_repair_failed_events,
        operator_reset_events_24h,
        run_is_terminal,
        orphan_generation_count,
    })
}

/// Returns the most recent session event for any lineage belonging to the run, or None.
pub async fn latest_session_event_for_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<Option<SessionEvent>> {
    let row = sqlx::query(
        r#"SELECT e.id, e.lineage_id, e.generation_id, e.event_type, e.recorded_at, e.details_json
           FROM session_events e
           JOIN session_lineages l ON e.lineage_id = l.id
           WHERE l.run_id = ?1
           ORDER BY e.recorded_at DESC, e.id DESC
           LIMIT 1"#,
    )
    .bind(run_id)
    .fetch_optional(pool)
    .await
    .context("latest session event for run")?;
    row.map(parse_event_row).transpose()
}

/// Per-lineage aggregated stats used to populate GqlSessionLineage computed fields.
pub struct LineageStats {
    pub generation_count: i64,
    pub latest_event_at: Option<DateTime<Utc>>,
    /// Status of the active generation (l.active_generation_id → g.status), if present.
    /// None when the lineage has no active_generation_id or the generation row was not found.
    pub active_generation_status: Option<SessionGenerationStatus>,
}

/// Load generation_count, latest_event_at, and active generation status for all lineages
/// belonging to a run in one query.
pub async fn aggregate_lineage_stats_for_run(
    pool: &SqlitePool,
    run_id: &str,
) -> Result<std::collections::HashMap<String, LineageStats>> {
    let rows = sqlx::query(
        r#"SELECT
             l.id AS lineage_id,
             COUNT(DISTINCT g.id) AS generation_count,
             MAX(e.recorded_at) AS latest_event_at,
             ag.status AS active_gen_status
           FROM session_lineages l
           LEFT JOIN session_generations g ON g.lineage_id = l.id
           LEFT JOIN session_events e ON e.lineage_id = l.id
           LEFT JOIN session_generations ag ON ag.id = l.active_generation_id
           WHERE l.run_id = ?1
           GROUP BY l.id, ag.status"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .context("aggregate lineage stats")?;

    Ok(parse_lineage_stats_rows(rows))
}

/// Load generation_count, latest_event_at, and active generation status for a specific set of
/// lineage row IDs (the current page). This bounds the DB work to the returned page rather
/// than scanning all lineages in the run.
pub async fn aggregate_lineage_stats_for_page(
    pool: &SqlitePool,
    lineage_row_ids: &[String],
) -> Result<std::collections::HashMap<String, LineageStats>> {
    if lineage_row_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let mut qb = sqlx::QueryBuilder::<Sqlite>::new(
        r#"SELECT
             l.id AS lineage_id,
             COUNT(DISTINCT g.id) AS generation_count,
             MAX(e.recorded_at) AS latest_event_at,
             ag.status AS active_gen_status
           FROM session_lineages l
           LEFT JOIN session_generations g ON g.lineage_id = l.id
           LEFT JOIN session_events e ON e.lineage_id = l.id
           LEFT JOIN session_generations ag ON ag.id = l.active_generation_id
           WHERE l.id IN ("#,
    );
    let mut sep = qb.separated(", ");
    for id in lineage_row_ids {
        sep.push_bind(id.as_str());
    }
    qb.push(") GROUP BY l.id, ag.status");
    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .context("aggregate lineage stats for page")?;
    Ok(parse_lineage_stats_rows(rows))
}

fn parse_lineage_stats_rows(
    rows: Vec<sqlx::sqlite::SqliteRow>,
) -> std::collections::HashMap<String, LineageStats> {
    let mut map = std::collections::HashMap::new();
    for row in rows {
        let lid: String = row.get("lineage_id");
        let generation_count: i64 = row
            .try_get::<Option<i64>, _>("generation_count")
            .ok()
            .flatten()
            .unwrap_or(0);
        let latest_event_at_str: Option<String> = row.try_get("latest_event_at").ok().flatten();
        let latest_event_at = latest_event_at_str
            .as_deref()
            .map(parse_dt)
            .transpose()
            .unwrap_or(None);
        let active_gen_status_str: Option<String> = row.try_get("active_gen_status").ok().flatten();
        let active_generation_status = active_gen_status_str
            .as_deref()
            .and_then(|s| serde_json::from_value(serde_json::Value::String(s.to_string())).ok());
        map.insert(
            lid,
            LineageStats {
                generation_count,
                latest_event_at,
                active_generation_status,
            },
        );
    }
    map
}

/// Load generation_count, latest_event_at, and active generation status for a single lineage row.
pub async fn aggregate_lineage_stats_for_lineage(
    pool: &SqlitePool,
    lineage_row_id: &str,
) -> Result<Option<LineageStats>> {
    let row = sqlx::query(
        r#"SELECT
             COUNT(DISTINCT g.id) AS generation_count,
             MAX(e.recorded_at) AS latest_event_at,
             ag.status AS active_gen_status
           FROM session_lineages l
           LEFT JOIN session_generations g ON g.lineage_id = l.id
           LEFT JOIN session_events e ON e.lineage_id = l.id
           LEFT JOIN session_generations ag ON ag.id = l.active_generation_id
           WHERE l.id = ?1
           GROUP BY l.id, ag.status"#,
    )
    .bind(lineage_row_id)
    .fetch_optional(pool)
    .await
    .context("aggregate lineage stats for single lineage")?;

    let Some(row) = row else { return Ok(None) };
    let generation_count: i64 = row
        .try_get::<Option<i64>, _>("generation_count")
        .ok()
        .flatten()
        .unwrap_or(0);
    let latest_event_at_str: Option<String> = row.try_get("latest_event_at").ok().flatten();
    let latest_event_at = latest_event_at_str
        .as_deref()
        .map(parse_dt)
        .transpose()
        .unwrap_or(None);
    let active_gen_status_str: Option<String> = row.try_get("active_gen_status").ok().flatten();
    let active_generation_status = active_gen_status_str
        .as_deref()
        .and_then(|s| serde_json::from_value(serde_json::Value::String(s.to_string())).ok());
    Ok(Some(LineageStats {
        generation_count,
        latest_event_at,
        active_generation_status,
    }))
}

fn parse_event_row(row: sqlx::sqlite::SqliteRow) -> Result<SessionEvent> {
    let event_type_str: String = row.get("event_type");
    let recorded_at_str: String = row.get("recorded_at");
    Ok(SessionEvent {
        id: row.get("id"),
        lineage_id: row.get("lineage_id"),
        generation_id: row.get("generation_id"),
        event_type: session_event_type_from_str(&event_type_str)?,
        recorded_at: parse_dt(&recorded_at_str)?,
        details_json: row.get("details_json"),
    })
}

fn session_event_type_from_str(s: &str) -> Result<SessionEventType> {
    match s {
        "created" => Ok(SessionEventType::Created),
        "reused" => Ok(SessionEventType::Reused),
        "invalidated" => Ok(SessionEventType::Invalidated),
        "closed" => Ok(SessionEventType::Closed),
        "operator_reset" => Ok(SessionEventType::OperatorReset),
        "budget_exceeded" => Ok(SessionEventType::BudgetExceeded),
        "compacted" => Ok(SessionEventType::Compacted),
        "output_contract_repair_started" => Ok(SessionEventType::OutputContractRepairStarted),
        "output_contract_repair_succeeded" => Ok(SessionEventType::OutputContractRepairSucceeded),
        "output_contract_repair_failed" => Ok(SessionEventType::OutputContractRepairFailed),
        "output_contract_repair_skipped" => Ok(SessionEventType::OutputContractRepairSkipped),
        "code_writer_completion_started" => Ok(SessionEventType::CodeWriterCompletionStarted),
        "code_writer_completion_succeeded" => Ok(SessionEventType::CodeWriterCompletionSucceeded),
        "code_writer_completion_failed" => Ok(SessionEventType::CodeWriterCompletionFailed),
        // Unknown event types map to Compacted which renders as UNKNOWN_EVENT_SHAPE in GraphQL.
        // Details are withheld by the GraphQL redaction layer.
        other => {
            tracing::warn!(
                "p046 unknown session event type in DB: {other:?}; mapping to UNKNOWN_EVENT_SHAPE"
            );
            Ok(SessionEventType::Compacted)
        }
    }
}
