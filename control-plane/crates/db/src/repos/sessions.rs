use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::session::{
    SessionEvent, SessionEventType, SessionGeneration, SessionGenerationStatus, SessionLineage,
};

pub async fn insert_lineage(pool: &SqlitePool, lineage: &SessionLineage) -> Result<()> {
    let mut tx = pool.begin().await?;
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
    let mut tx = pool.begin().await?;
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
    let mut tx = pool.begin().await?;
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
    let mut tx = pool.begin().await?;
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
    let mut tx = pool.begin().await?;
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
    .execute(pool)
    .await
    .context("update session generation usage")?;
    Ok(())
}

pub async fn touch_generation_activity(
    pool: &SqlitePool,
    generation_id: &str,
    last_activity_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE session_generations
           SET last_activity_at = ?1
           WHERE id = ?2
             AND status = 'active'"#,
    )
    .bind(last_activity_at.to_rfc3339())
    .bind(generation_id)
    .execute(pool)
    .await
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
    sqlx::query(
        r#"UPDATE session_generations
           SET provider_session_id = ?1, turn_count = ?2
           WHERE id = ?3"#,
    )
    .bind(provider_session_id)
    .bind(turn_count)
    .bind(generation_id)
    .execute(pool)
    .await
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
    }
}
