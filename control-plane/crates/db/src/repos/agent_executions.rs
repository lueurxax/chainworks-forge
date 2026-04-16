use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use domain::agent::{AgentExecution, AgentStatus};
use domain::ids::{AgentExecutionId, RunId, StageExecutionId};

const SELECT_COLS: &str = r#"id, stage_execution_id, agent_id, provider, model, status, started_at, completed_at,
                owner_execution_lineage_id, session_lineage_id, session_generation_id, rehydrated_from_checkpoint_artifact_id,
                invocation_owner_key, session_reuse_scope, session_family_id,
                session_reuse_disposition, session_reset_reason,
                backend_profile_id, requested_mcp_extensions_json, predicted_mcp_extensions_json,
                predicted_mcp_runtime_ids_json, actual_mcp_extensions_json, actual_mcp_runtime_ids_json,
                denied_mcp_extensions_json, mcp_blocking_issues_json, actual_mcp_observation_json,
                mcp_session_startup_latency_ms"#;

pub async fn insert(pool: &SqlitePool, exec: &AgentExecution) -> Result<()> {
    sqlx::query(
        "INSERT INTO agent_executions
         (id, stage_execution_id, agent_id, provider, model, status, started_at, completed_at,
          owner_execution_lineage_id, session_lineage_id, session_generation_id, rehydrated_from_checkpoint_artifact_id,
          invocation_owner_key, session_reuse_scope, session_family_id,
          session_reuse_disposition, session_reset_reason,
          backend_profile_id, requested_mcp_extensions_json, predicted_mcp_extensions_json,
          predicted_mcp_runtime_ids_json, actual_mcp_extensions_json, actual_mcp_runtime_ids_json,
          denied_mcp_extensions_json, mcp_blocking_issues_json, actual_mcp_observation_json,
          mcp_session_startup_latency_ms)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(exec.id.to_string())
    .bind(exec.stage_execution_id.to_string())
    .bind(&exec.agent_id)
    .bind(&exec.provider)
    .bind(&exec.model)
    .bind(exec.status.to_string())
    .bind(exec.started_at.to_rfc3339())
    .bind(exec.completed_at.map(|d| d.to_rfc3339()))
    .bind(&exec.owner_execution_lineage_id)
    .bind(&exec.session_lineage_id)
    .bind(&exec.session_generation_id)
    .bind(&exec.rehydrated_from_checkpoint_artifact_id)
    .bind(&exec.invocation_owner_key)
    .bind(&exec.session_reuse_scope)
    .bind(&exec.session_family_id)
    .bind(&exec.session_reuse_disposition)
    .bind(&exec.session_reset_reason)
    .bind(&exec.backend_profile_id)
    .bind(&exec.requested_mcp_extensions_json)
    .bind(&exec.predicted_mcp_extensions_json)
    .bind(&exec.predicted_mcp_runtime_ids_json)
    .bind(&exec.actual_mcp_extensions_json)
    .bind(&exec.actual_mcp_runtime_ids_json)
    .bind(&exec.denied_mcp_extensions_json)
    .bind(&exec.mcp_blocking_issues_json)
    .bind(&exec.actual_mcp_observation_json)
    .bind(exec.mcp_session_startup_latency_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: AgentExecutionId) -> Result<Option<AgentExecution>> {
    let query = format!("SELECT {SELECT_COLS} FROM agent_executions WHERE id = ?");
    let row = sqlx::query(&query)
        .bind(id.to_string())
        .fetch_optional(pool)
        .await?;

    row.map(|row| parse_agent_execution_row(&row)).transpose()
}

pub async fn update_completed(
    pool: &SqlitePool,
    id: AgentExecutionId,
    status: AgentStatus,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query("UPDATE agent_executions SET status = ?, completed_at = ? WHERE id = ?")
        .bind(status.to_string())
        .bind(completed_at.to_rfc3339())
        .bind(id.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_mcp_provenance(
    pool: &SqlitePool,
    id: AgentExecutionId,
    backend_profile_id: Option<&str>,
    requested_mcp_extensions_json: Option<&str>,
    predicted_mcp_extensions_json: Option<&str>,
    predicted_mcp_runtime_ids_json: Option<&str>,
    denied_mcp_extensions_json: Option<&str>,
    mcp_blocking_issues_json: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE agent_executions
         SET backend_profile_id = ?,
             requested_mcp_extensions_json = ?,
             predicted_mcp_extensions_json = ?,
             predicted_mcp_runtime_ids_json = ?,
             denied_mcp_extensions_json = ?,
             mcp_blocking_issues_json = ?
         WHERE id = ?",
    )
    .bind(backend_profile_id)
    .bind(requested_mcp_extensions_json)
    .bind(predicted_mcp_extensions_json)
    .bind(predicted_mcp_runtime_ids_json)
    .bind(denied_mcp_extensions_json)
    .bind(mcp_blocking_issues_json)
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_mcp_actual(
    pool: &SqlitePool,
    id: AgentExecutionId,
    actual_mcp_extensions_json: Option<&str>,
    actual_mcp_runtime_ids_json: Option<&str>,
    actual_mcp_observation_json: Option<&str>,
    mcp_session_startup_latency_ms: Option<i64>,
) -> Result<()> {
    sqlx::query(
        "UPDATE agent_executions
         SET actual_mcp_extensions_json = ?,
             actual_mcp_runtime_ids_json = ?,
             actual_mcp_observation_json = ?,
             mcp_session_startup_latency_ms = ?
         WHERE id = ?",
    )
    .bind(actual_mcp_extensions_json)
    .bind(actual_mcp_runtime_ids_json)
    .bind(actual_mcp_observation_json)
    .bind(mcp_session_startup_latency_ms)
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_stage(
    pool: &SqlitePool,
    stage_execution_id: StageExecutionId,
) -> Result<Vec<AgentExecution>> {
    let query = format!(
        "SELECT {SELECT_COLS} FROM agent_executions WHERE stage_execution_id = ? ORDER BY started_at ASC"
    );
    let rows = sqlx::query(&query)
        .bind(stage_execution_id.to_string())
        .fetch_all(pool)
        .await?;

    rows.iter().map(parse_agent_execution_row).collect()
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<AgentExecution>> {
    let prefixed_select_cols = SELECT_COLS
        .split(',')
        .map(|col| format!("ae.{}", col.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT {prefixed_select_cols}
         FROM agent_executions ae
         INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
         WHERE se.run_id = ?
         ORDER BY ae.started_at ASC"
    );
    let rows = sqlx::query(&query)
        .bind(run_id.to_string())
        .fetch_all(pool)
        .await?;

    rows.iter().map(parse_agent_execution_row).collect()
}

pub async fn cancel_running_by_run(
    pool: &SqlitePool,
    run_id: RunId,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE agent_executions
         SET status = ?, completed_at = ?
         WHERE status = ? AND stage_execution_id IN (
             SELECT id FROM stage_executions WHERE run_id = ?
         )",
    )
    .bind(AgentStatus::Cancelled.to_string())
    .bind(completed_at.to_rfc3339())
    .bind(AgentStatus::Running.to_string())
    .bind(run_id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

fn parse_agent_execution_row(row: &sqlx::sqlite::SqliteRow) -> Result<AgentExecution> {
    let id: String = row.get("id");
    let seid: String = row.get("stage_execution_id");
    let status_str: String = row.get("status");
    let started_str: String = row.get("started_at");
    let completed_str: Option<String> = row.get("completed_at");

    Ok(AgentExecution {
        id: id.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
        stage_execution_id: seid.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
        agent_id: row.get("agent_id"),
        provider: row.get("provider"),
        model: row.get("model"),
        status: status_str.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
        started_at: started_str.parse::<DateTime<Utc>>()?,
        completed_at: completed_str
            .map(|s| s.parse::<DateTime<Utc>>())
            .transpose()?,
        owner_execution_lineage_id: row.get("owner_execution_lineage_id"),
        session_lineage_id: row.get("session_lineage_id"),
        session_generation_id: row.get("session_generation_id"),
        rehydrated_from_checkpoint_artifact_id: row.get("rehydrated_from_checkpoint_artifact_id"),
        invocation_owner_key: row.get("invocation_owner_key"),
        session_reuse_scope: row.get("session_reuse_scope"),
        session_family_id: row.get("session_family_id"),
        session_reuse_disposition: row.get("session_reuse_disposition"),
        session_reset_reason: row.get("session_reset_reason"),
        backend_profile_id: row.get("backend_profile_id"),
        requested_mcp_extensions_json: row.get("requested_mcp_extensions_json"),
        predicted_mcp_extensions_json: row.get("predicted_mcp_extensions_json"),
        predicted_mcp_runtime_ids_json: row.get("predicted_mcp_runtime_ids_json"),
        actual_mcp_extensions_json: row.get("actual_mcp_extensions_json"),
        actual_mcp_runtime_ids_json: row.get("actual_mcp_runtime_ids_json"),
        denied_mcp_extensions_json: row.get("denied_mcp_extensions_json"),
        mcp_blocking_issues_json: row.get("mcp_blocking_issues_json"),
        actual_mcp_observation_json: row.get("actual_mcp_observation_json"),
        mcp_session_startup_latency_ms: row.get("mcp_session_startup_latency_ms"),
    })
}
