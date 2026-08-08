use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use domain::ids::{RunId, StageExecutionId};
use domain::retry_authority::RetryPayloadRecoveryEvent;
use serde_json::Value;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

pub async fn upsert(pool: &SqlitePool, event: &RetryPayloadRecoveryEvent) -> Result<()> {
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "retry_payload_recovery_events.upsert")
            .await?;
    upsert_tx(&mut tx, event).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn upsert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    event: &RetryPayloadRecoveryEvent,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO retry_payload_recovery_events
           (idempotency_key, run_id, invoke_work_item_id, retry_authority_id,
            target_stage_execution_id, completed_agent_execution_id, reason_code, mode,
            repaired, current_json, provenance_json, repaired_fields_json, diagnostic_json,
            created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
           ON CONFLICT(idempotency_key) DO UPDATE SET
             retry_authority_id = excluded.retry_authority_id,
             target_stage_execution_id = excluded.target_stage_execution_id,
             completed_agent_execution_id = excluded.completed_agent_execution_id,
             reason_code = excluded.reason_code,
             mode = excluded.mode,
             repaired = CASE
                 WHEN retry_payload_recovery_events.repaired = 1 THEN 1
                 ELSE excluded.repaired
             END,
             current_json = excluded.current_json,
             provenance_json = excluded.provenance_json,
             repaired_fields_json = excluded.repaired_fields_json,
             diagnostic_json = excluded.diagnostic_json,
             updated_at = excluded.updated_at"#,
    )
    .bind(&event.idempotency_key)
    .bind(event.run_id.to_string())
    .bind(&event.invoke_work_item_id)
    .bind(&event.retry_authority_id)
    .bind(event.target_stage_execution_id.map(|id| id.to_string()))
    .bind(&event.completed_agent_execution_id)
    .bind(&event.reason_code)
    .bind(&event.mode)
    .bind(event.repaired)
    .bind(event.current_json.to_string())
    .bind(event.provenance_json.as_ref().map(Value::to_string))
    .bind(event.repaired_fields_json.as_ref().map(Value::to_string))
    .bind(event.diagnostic_json.as_ref().map(Value::to_string))
    .bind(event.created_at.to_rfc3339())
    .bind(event.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .context("upsert retry payload recovery event")?;
    Ok(())
}

pub async fn list_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<RetryPayloadRecoveryEvent>> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(select_sql(
        "WHERE run_id = ?1 ORDER BY updated_at ASC",
    )))
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await
    .context("list retry payload recovery events by run")?;
    rows.iter().map(parse_row).collect()
}

pub async fn latest_by_authority_for_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<std::collections::HashMap<String, RetryPayloadRecoveryEvent>> {
    let events = list_by_run(pool, run_id).await?;
    let mut by_authority = std::collections::HashMap::new();
    for event in events {
        if let Some(authority_id) = event.retry_authority_id.clone() {
            by_authority.insert(authority_id, event);
        }
    }
    Ok(by_authority)
}

pub async fn latest_by_invoke_for_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<std::collections::HashMap<String, RetryPayloadRecoveryEvent>> {
    let events = list_by_run(pool, run_id).await?;
    let mut by_invoke = std::collections::HashMap::new();
    for event in events {
        by_invoke.insert(event.invoke_work_item_id.clone(), event);
    }
    Ok(by_invoke)
}

fn select_sql(where_clause: &str) -> String {
    format!(
        r#"SELECT idempotency_key, run_id, invoke_work_item_id, retry_authority_id,
                  target_stage_execution_id, completed_agent_execution_id, reason_code, mode,
                  repaired, current_json, provenance_json, repaired_fields_json, diagnostic_json,
                  created_at, updated_at
           FROM retry_payload_recovery_events
           {where_clause}"#
    )
}

fn parse_row(row: &sqlx::sqlite::SqliteRow) -> Result<RetryPayloadRecoveryEvent> {
    let run_id_raw: String = row.get("run_id");
    let target_raw: Option<String> = row.get("target_stage_execution_id");
    let created_raw: String = row.get("created_at");
    let updated_raw: String = row.get("updated_at");
    Ok(RetryPayloadRecoveryEvent {
        idempotency_key: row.get("idempotency_key"),
        run_id: run_id_raw.parse::<RunId>().context("parse P092 run_id")?,
        invoke_work_item_id: row.get("invoke_work_item_id"),
        retry_authority_id: row.get("retry_authority_id"),
        target_stage_execution_id: target_raw
            .map(|raw| raw.parse::<StageExecutionId>())
            .transpose()
            .context("parse P092 target_stage_execution_id")?,
        completed_agent_execution_id: row.get("completed_agent_execution_id"),
        reason_code: row.get("reason_code"),
        mode: row.get("mode"),
        repaired: row.get::<i64, _>("repaired") != 0,
        current_json: parse_json(row.get::<String, _>("current_json")).context("current_json")?,
        provenance_json: parse_optional_json(row.get("provenance_json"))
            .context("provenance_json")?,
        repaired_fields_json: parse_optional_json(row.get("repaired_fields_json"))
            .context("repaired_fields_json")?,
        diagnostic_json: parse_optional_json(row.get("diagnostic_json"))
            .context("diagnostic_json")?,
        created_at: DateTime::parse_from_rfc3339(&created_raw)
            .context("parse P092 created_at")?
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_raw)
            .context("parse P092 updated_at")?
            .with_timezone(&Utc),
    })
}

fn parse_json(raw: String) -> Result<Value> {
    serde_json::from_str(&raw).context("parse json")
}

fn parse_optional_json(raw: Option<String>) -> Result<Option<Value>> {
    raw.map(parse_json).transpose()
}
