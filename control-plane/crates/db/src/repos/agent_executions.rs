use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use domain::agent::{AgentExecution, AgentStatus};
use domain::ids::{AgentExecutionId, StageExecutionId};

pub async fn insert(pool: &SqlitePool, exec: &AgentExecution) -> Result<()> {
    sqlx::query(
        "INSERT INTO agent_executions (id, stage_execution_id, agent_id, provider, model, status, started_at, completed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(exec.id.to_string())
    .bind(exec.stage_execution_id.to_string())
    .bind(&exec.agent_id)
    .bind(&exec.provider)
    .bind(&exec.model)
    .bind(exec.status.to_string())
    .bind(exec.started_at.to_rfc3339())
    .bind(exec.completed_at.map(|d| d.to_rfc3339()))
    .execute(pool)
    .await?;
    Ok(())
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
        "SELECT id, stage_execution_id, agent_id, provider, model, status, started_at, completed_at
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
        });
    }
    Ok(result)
}
