//! P065: Repo helpers for `retry_operator_instruction_bindings` and
//! `retry_operator_instruction_deliveries`.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::ids::{AgentExecutionId, RunId, StageExecutionId};
use domain::retry_instruction::{
    RetryInstructionBindingInput, RetryInstructionDeliveryStatus, RetryOperatorInstructionBinding,
    RetryOperatorInstructionDelivery,
};

// ── Parent binding ────────────────────────────────────────────────────

/// Create a parent binding row atomically within the caller's transaction.
pub async fn create_for_retry_attempt_tx(
    tx: &mut Transaction<'_, Sqlite>,
    input: &RetryInstructionBindingInput,
) -> Result<RetryOperatorInstructionBinding> {
    let now = Utc::now();
    let binding_id = uuid::Uuid::new_v4().to_string();
    let instruction_sha256 = domain::retry_instruction::instruction_sha256(&input.instruction_text);

    sqlx::query(
        r#"INSERT INTO retry_operator_instruction_bindings
           (binding_id, journal_id, run_id, stage_id,
            source_stage_execution_id, retry_stage_execution_id,
            retry_attempt_number, target_agent_execution_id,
            scope_kind, instruction_text, instruction_sha256,
            created_by_principal_id, created_by_principal_class, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
    )
    .bind(&binding_id)
    .bind(&input.journal_id)
    .bind(input.run_id.to_string())
    .bind(&input.stage_id)
    .bind(input.source_stage_execution_id.to_string())
    .bind(input.retry_stage_execution_id.to_string())
    .bind(input.retry_attempt_number)
    .bind(input.target_agent_execution_id.map(|id| id.to_string()))
    .bind(input.scope_kind.to_string())
    .bind(&input.instruction_text)
    .bind(&instruction_sha256)
    .bind(&input.created_by_principal_id)
    .bind(&input.created_by_principal_class)
    .bind(now.to_rfc3339())
    .execute(&mut **tx)
    .await?;

    Ok(RetryOperatorInstructionBinding {
        binding_id,
        journal_id: input.journal_id.clone(),
        run_id: input.run_id,
        stage_id: input.stage_id.clone(),
        source_stage_execution_id: input.source_stage_execution_id,
        retry_stage_execution_id: input.retry_stage_execution_id,
        retry_attempt_number: input.retry_attempt_number,
        target_agent_execution_id: input.target_agent_execution_id,
        scope_kind: input.scope_kind.clone(),
        instruction_text: input.instruction_text.clone(),
        instruction_sha256,
        created_by_principal_id: input.created_by_principal_id.clone(),
        created_by_principal_class: input.created_by_principal_class.clone(),
        created_at: now,
    })
}

/// List active bindings for a given retry stage execution (used by orchestrator fanout).
pub async fn list_by_retry_stage_execution_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    retry_stage_execution_id: StageExecutionId,
) -> Result<Vec<RetryOperatorInstructionBinding>> {
    let rows = sqlx::query(
        r#"SELECT binding_id, journal_id, run_id, stage_id,
                  source_stage_execution_id, retry_stage_execution_id,
                  retry_attempt_number, target_agent_execution_id,
                  scope_kind, instruction_text, instruction_sha256,
                  created_by_principal_id, created_by_principal_class, created_at
           FROM retry_operator_instruction_bindings
           WHERE retry_stage_execution_id = ?1
           ORDER BY created_at ASC"#,
    )
    .bind(retry_stage_execution_id.to_string())
    .fetch_all(&mut **tx)
    .await?;
    rows.iter().map(parse_binding_row).collect()
}

// ── Child delivery ────────────────────────────────────────────────────

/// Create a child delivery row for a specific work item (used by command handler
/// for targeted retry, or orchestrator for full-stage fanout).
pub async fn create_for_work_item_tx(
    tx: &mut Transaction<'_, Sqlite>,
    binding_id: &str,
    work_item_id: Option<&str>,
    agent_execution_id: Option<AgentExecutionId>,
) -> Result<RetryOperatorInstructionDelivery> {
    let now = Utc::now();
    let delivery_id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        r#"INSERT INTO retry_operator_instruction_deliveries
           (delivery_id, binding_id, work_item_id, agent_execution_id,
            status, failure_reason, created_at, updated_at, delivered_at)
           VALUES (?1, ?2, ?3, ?4, 'pending', NULL, ?5, ?5, NULL)"#,
    )
    .bind(&delivery_id)
    .bind(binding_id)
    .bind(work_item_id)
    .bind(agent_execution_id.map(|id| id.to_string()))
    .bind(now.to_rfc3339())
    .execute(&mut **tx)
    .await?;

    Ok(RetryOperatorInstructionDelivery {
        delivery_id,
        binding_id: binding_id.to_string(),
        work_item_id: work_item_id.map(String::from),
        agent_execution_id,
        status: RetryInstructionDeliveryStatus::Pending,
        failure_reason: None,
        created_at: now,
        updated_at: now,
        delivered_at: None,
    })
}

/// Mark a pending delivery as delivered.
pub async fn mark_delivered_tx(tx: &mut Transaction<'_, Sqlite>, delivery_id: &str) -> Result<()> {
    let now = Utc::now();
    let rows = sqlx::query(
        r#"UPDATE retry_operator_instruction_deliveries
           SET status = 'delivered', updated_at = ?1, delivered_at = ?1
           WHERE delivery_id = ?2 AND status = 'pending'"#,
    )
    .bind(now.to_rfc3339())
    .bind(delivery_id)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if rows != 1 {
        anyhow::bail!(
            "retry instruction delivery {delivery_id} not updated (expected pending, got 0 rows)"
        );
    }
    Ok(())
}

/// Mark a pending delivery as failed.
pub async fn mark_failed_tx(
    tx: &mut Transaction<'_, Sqlite>,
    delivery_id: &str,
    reason: &str,
) -> Result<()> {
    let now = Utc::now();
    let rows = sqlx::query(
        r#"UPDATE retry_operator_instruction_deliveries
           SET status = 'failed', failure_reason = ?1, updated_at = ?2
           WHERE delivery_id = ?3 AND status = 'pending'"#,
    )
    .bind(reason)
    .bind(now.to_rfc3339())
    .bind(delivery_id)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if rows != 1 {
        anyhow::bail!(
            "retry instruction delivery {delivery_id} not marked failed (expected pending, got 0 rows)"
        );
    }
    Ok(())
}

/// List all bindings for a run (MCP readback for runs.get and reports.get).
pub async fn list_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<RetryOperatorInstructionBinding>> {
    let rows = sqlx::query(
        r#"SELECT binding_id, journal_id, run_id, stage_id,
                  source_stage_execution_id, retry_stage_execution_id,
                  retry_attempt_number, target_agent_execution_id,
                  scope_kind, instruction_text, instruction_sha256,
                  created_by_principal_id, created_by_principal_class, created_at
           FROM retry_operator_instruction_bindings
           WHERE run_id = ?1
           ORDER BY created_at ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_binding_row).collect()
}

/// List all deliveries for a binding.
pub async fn list_deliveries_by_binding(
    pool: &SqlitePool,
    binding_id: &str,
) -> Result<Vec<RetryOperatorInstructionDelivery>> {
    let rows = sqlx::query(
        r#"SELECT delivery_id, binding_id, work_item_id, agent_execution_id,
                  status, failure_reason, created_at, updated_at, delivered_at
           FROM retry_operator_instruction_deliveries
           WHERE binding_id = ?1
           ORDER BY created_at ASC"#,
    )
    .bind(binding_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_delivery_row).collect()
}

/// List all deliveries for a binding within a transaction.
pub async fn list_deliveries_by_binding_tx(
    tx: &mut Transaction<'_, Sqlite>,
    binding_id: &str,
) -> Result<Vec<RetryOperatorInstructionDelivery>> {
    let rows = sqlx::query(
        r#"SELECT delivery_id, binding_id, work_item_id, agent_execution_id,
                  status, failure_reason, created_at, updated_at, delivered_at
           FROM retry_operator_instruction_deliveries
           WHERE binding_id = ?1
           ORDER BY created_at ASC"#,
    )
    .bind(binding_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.iter().map(parse_delivery_row).collect()
}

/// Find the pending delivery for a given work item (used by executor to locate
/// the delivery row that needs status transition).
pub async fn find_pending_delivery_for_work_item_tx(
    tx: &mut Transaction<'_, Sqlite>,
    work_item_id: &str,
) -> Result<Option<RetryOperatorInstructionDelivery>> {
    let row = sqlx::query(
        r#"SELECT d.delivery_id, d.binding_id, d.work_item_id, d.agent_execution_id,
                  d.status, d.failure_reason, d.created_at, d.updated_at, d.delivered_at
           FROM retry_operator_instruction_deliveries d
           WHERE d.work_item_id = ?1 AND d.status = 'pending'
           LIMIT 1"#,
    )
    .bind(work_item_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.as_ref().map(parse_delivery_row).transpose()
}

/// Find the binding for a delivery (used by executor to get instruction text).
pub async fn find_binding_by_id_tx(
    tx: &mut Transaction<'_, Sqlite>,
    binding_id: &str,
) -> Result<Option<RetryOperatorInstructionBinding>> {
    let row = sqlx::query(
        r#"SELECT binding_id, journal_id, run_id, stage_id,
                  source_stage_execution_id, retry_stage_execution_id,
                  retry_attempt_number, target_agent_execution_id,
                  scope_kind, instruction_text, instruction_sha256,
                  created_by_principal_id, created_by_principal_class, created_at
           FROM retry_operator_instruction_bindings
           WHERE binding_id = ?1"#,
    )
    .bind(binding_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.as_ref().map(parse_binding_row).transpose()
}

/// Startup repair: fail all stale pending deliveries whose retry stage execution
/// is in a terminal state (failed/completed/skipped).
pub async fn fail_stale_pending_deliveries_tx(tx: &mut Transaction<'_, Sqlite>) -> Result<u64> {
    let now = Utc::now();
    let result = sqlx::query(
        r#"UPDATE retry_operator_instruction_deliveries
           SET status = 'failed',
               failure_reason = 'startup_repair_stale_pending',
               updated_at = ?1
           WHERE status = 'pending'
             AND binding_id IN (
                 SELECT b.binding_id
                 FROM retry_operator_instruction_bindings b
                 JOIN stage_executions se ON se.id = b.retry_stage_execution_id
                 WHERE se.status IN ('failed', 'completed', 'skipped')
             )"#,
    )
    .bind(now.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

// ── Row parsers ───────────────────────────────────────────────────────

fn parse_binding_row(row: &sqlx::sqlite::SqliteRow) -> Result<RetryOperatorInstructionBinding> {
    let run_id: String = row.get("run_id");
    let source_se: String = row.get("source_stage_execution_id");
    let retry_se: String = row.get("retry_stage_execution_id");
    let target_ae: Option<String> = row.get("target_agent_execution_id");
    let scope_kind: String = row.get("scope_kind");
    let created_at: String = row.get("created_at");

    Ok(RetryOperatorInstructionBinding {
        binding_id: row.get("binding_id"),
        journal_id: row.get("journal_id"),
        run_id: run_id.parse().map_err(|e| anyhow!("{}", e))?,
        stage_id: row.get("stage_id"),
        source_stage_execution_id: source_se.parse().map_err(|e| anyhow!("{}", e))?,
        retry_stage_execution_id: retry_se.parse().map_err(|e| anyhow!("{}", e))?,
        retry_attempt_number: row.get("retry_attempt_number"),
        target_agent_execution_id: target_ae
            .map(|v| v.parse().map_err(|e| anyhow!("{}", e)))
            .transpose()?,
        scope_kind: scope_kind.parse().map_err(|e| anyhow!("{}", e))?,
        instruction_text: row.get("instruction_text"),
        instruction_sha256: row.get("instruction_sha256"),
        created_by_principal_id: row.get("created_by_principal_id"),
        created_by_principal_class: row.get("created_by_principal_class"),
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .context("parse retry instruction binding created_at")?
            .with_timezone(&Utc),
    })
}

fn parse_delivery_row(row: &sqlx::sqlite::SqliteRow) -> Result<RetryOperatorInstructionDelivery> {
    let binding_id: String = row.get("binding_id");
    let work_item_id: Option<String> = row.get("work_item_id");
    let agent_execution_id: Option<String> = row.get("agent_execution_id");
    let status: String = row.get("status");
    let created_at: String = row.get("created_at");
    let updated_at: String = row.get("updated_at");
    let delivered_at: Option<String> = row.get("delivered_at");

    Ok(RetryOperatorInstructionDelivery {
        delivery_id: row.get("delivery_id"),
        binding_id,
        work_item_id,
        agent_execution_id: agent_execution_id
            .map(|v| v.parse().map_err(|e| anyhow!("{}", e)))
            .transpose()?,
        status: status.parse().map_err(|e| anyhow!("{}", e))?,
        failure_reason: row.get("failure_reason"),
        created_at: DateTime::parse_from_rfc3339(&created_at)
            .context("parse retry instruction delivery created_at")?
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at)
            .context("parse retry instruction delivery updated_at")?
            .with_timezone(&Utc),
        delivered_at: delivered_at
            .map(|v| DateTime::parse_from_rfc3339(&v).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .context("parse retry instruction delivery delivered_at")?,
    })
}
