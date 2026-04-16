use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use domain::agent::{AgentExecution, AgentStatus};
use domain::ids::{AgentExecutionId, RunId, StageExecutionId};

pub async fn insert(pool: &SqlitePool, exec: &AgentExecution) -> Result<()> {
    sqlx::query(
        "INSERT INTO agent_executions
         (id, stage_execution_id, agent_id, provider, model, status, started_at, completed_at,
          owner_execution_lineage_id, session_lineage_id, session_generation_id, rehydrated_from_checkpoint_artifact_id,
          invocation_owner_key, session_reuse_scope, session_family_id,
          session_reuse_disposition, session_reset_reason)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_id(pool: &SqlitePool, id: AgentExecutionId) -> Result<Option<AgentExecution>> {
    let row = sqlx::query(
        "SELECT id, stage_execution_id, agent_id, provider, model, status, started_at, completed_at,
                owner_execution_lineage_id, session_lineage_id, session_generation_id, rehydrated_from_checkpoint_artifact_id,
                invocation_owner_key, session_reuse_scope, session_family_id,
                session_reuse_disposition, session_reset_reason
         FROM agent_executions WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        use sqlx::Row;
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
            completed_at: completed_str.map(|s| s.parse::<DateTime<Utc>>()).transpose()?,
            owner_execution_lineage_id: row.get("owner_execution_lineage_id"),
            session_lineage_id: row.get("session_lineage_id"),
            session_generation_id: row.get("session_generation_id"),
            rehydrated_from_checkpoint_artifact_id: row.get("rehydrated_from_checkpoint_artifact_id"),
            invocation_owner_key: row.get("invocation_owner_key"),
            session_reuse_scope: row.get("session_reuse_scope"),
            session_family_id: row.get("session_family_id"),
            session_reuse_disposition: row.get("session_reuse_disposition"),
            session_reset_reason: row.get("session_reset_reason"),
        })
    })
    .transpose()
}

pub async fn update_completed(
    pool: &SqlitePool,
    id: AgentExecutionId,
    status: AgentStatus,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE agent_executions SET status = ?, completed_at = ? WHERE id = ?",
    )
    .bind(status.to_string())
    .bind(completed_at.to_rfc3339())
    .bind(id.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_by_stage(pool: &SqlitePool, stage_execution_id: StageExecutionId) -> Result<Vec<AgentExecution>> {
    let rows = sqlx::query(
        "SELECT id, stage_execution_id, agent_id, provider, model, status, started_at, completed_at,
                owner_execution_lineage_id, session_lineage_id, session_generation_id, rehydrated_from_checkpoint_artifact_id,
                invocation_owner_key, session_reuse_scope, session_family_id,
                session_reuse_disposition, session_reset_reason
         FROM agent_executions WHERE stage_execution_id = ? ORDER BY started_at ASC",
    )
    .bind(stage_execution_id.to_string())
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        use sqlx::Row;
        let id: String = row.get("id");
        let seid: String = row.get("stage_execution_id");
        let status_str: String = row.get("status");
        let started_str: String = row.get("started_at");
        let completed_str: Option<String> = row.get("completed_at");

        result.push(AgentExecution {
            id: id.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
            stage_execution_id: seid.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
            agent_id: row.get("agent_id"),
            provider: row.get("provider"),
            model: row.get("model"),
            status: status_str.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
            started_at: started_str.parse::<DateTime<Utc>>()?,
            completed_at: completed_str.map(|s| s.parse::<DateTime<Utc>>()).transpose()?,
            owner_execution_lineage_id: row.get("owner_execution_lineage_id"),
            session_lineage_id: row.get("session_lineage_id"),
            session_generation_id: row.get("session_generation_id"),
            rehydrated_from_checkpoint_artifact_id: row.get("rehydrated_from_checkpoint_artifact_id"),
            invocation_owner_key: row.get("invocation_owner_key"),
            session_reuse_scope: row.get("session_reuse_scope"),
            session_family_id: row.get("session_family_id"),
            session_reuse_disposition: row.get("session_reuse_disposition"),
            session_reset_reason: row.get("session_reset_reason"),
        });
    }
    Ok(result)
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<AgentExecution>> {
    let rows = sqlx::query(
        "SELECT ae.id, ae.stage_execution_id, ae.agent_id, ae.provider, ae.model, ae.status, ae.started_at, ae.completed_at,
                ae.owner_execution_lineage_id, ae.session_lineage_id, ae.session_generation_id, ae.rehydrated_from_checkpoint_artifact_id,
                ae.invocation_owner_key, ae.session_reuse_scope, ae.session_family_id,
                ae.session_reuse_disposition, ae.session_reset_reason
         FROM agent_executions ae
         INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
         WHERE se.run_id = ?
         ORDER BY ae.started_at ASC",
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;

    let mut result = Vec::new();
    for row in rows {
        use sqlx::Row;
        let id: String = row.get("id");
        let seid: String = row.get("stage_execution_id");
        let status_str: String = row.get("status");
        let started_str: String = row.get("started_at");
        let completed_str: Option<String> = row.get("completed_at");

        result.push(AgentExecution {
            id: id.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
            stage_execution_id: seid.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
            agent_id: row.get("agent_id"),
            provider: row.get("provider"),
            model: row.get("model"),
            status: status_str.parse().map_err(|e| anyhow::anyhow!("{}", e))?,
            started_at: started_str.parse::<DateTime<Utc>>()?,
            completed_at: completed_str.map(|s| s.parse::<DateTime<Utc>>()).transpose()?,
            owner_execution_lineage_id: row.get("owner_execution_lineage_id"),
            session_lineage_id: row.get("session_lineage_id"),
            session_generation_id: row.get("session_generation_id"),
            rehydrated_from_checkpoint_artifact_id: row.get("rehydrated_from_checkpoint_artifact_id"),
            invocation_owner_key: row.get("invocation_owner_key"),
            session_reuse_scope: row.get("session_reuse_scope"),
            session_family_id: row.get("session_family_id"),
            session_reuse_disposition: row.get("session_reuse_disposition"),
            session_reset_reason: row.get("session_reset_reason"),
        });
    }
    Ok(result)
}

pub async fn cancel_running_by_run(pool: &SqlitePool, run_id: RunId, completed_at: DateTime<Utc>) -> Result<()> {
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
