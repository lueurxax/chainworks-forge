use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::agent::AgentExecutionRuntimeReceiptRecord;
use domain::ids::{AgentExecutionId, RunId};

pub async fn upsert(pool: &SqlitePool, receipt: &AgentExecutionRuntimeReceiptRecord) -> Result<()> {
    let mut tx = crate::writer::begin_repository_transaction(
        pool,
        "agent_execution_runtime_receipts.upsert",
    )
    .await?;
    upsert_tx(&mut tx, receipt).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn upsert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    receipt: &AgentExecutionRuntimeReceiptRecord,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO agent_execution_runtime_receipts
           (agent_execution_id, provider, transport_family, status, failure_phase,
            event_count, last_event_kind, last_event_at_ms, receipt_json, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
           ON CONFLICT(agent_execution_id) DO UPDATE SET
             provider = excluded.provider,
             transport_family = excluded.transport_family,
             status = excluded.status,
             failure_phase = excluded.failure_phase,
             event_count = excluded.event_count,
             last_event_kind = excluded.last_event_kind,
             last_event_at_ms = excluded.last_event_at_ms,
             receipt_json = excluded.receipt_json,
             updated_at = excluded.updated_at"#,
    )
    .bind(receipt.agent_execution_id.to_string())
    .bind(&receipt.provider)
    .bind(&receipt.transport_family)
    .bind(&receipt.status)
    .bind(&receipt.failure_phase)
    .bind(receipt.event_count)
    .bind(&receipt.last_event_kind)
    .bind(receipt.last_event_at_ms)
    .bind(&receipt.receipt_json)
    .bind(receipt.created_at.to_rfc3339())
    .bind(receipt.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn find_by_execution_id(
    pool: &SqlitePool,
    agent_execution_id: AgentExecutionId,
) -> Result<Option<AgentExecutionRuntimeReceiptRecord>> {
    let row = sqlx::query(
        r#"SELECT agent_execution_id, provider, transport_family, status, failure_phase,
                  event_count, last_event_kind, last_event_at_ms, receipt_json,
                  created_at, updated_at
           FROM agent_execution_runtime_receipts
           WHERE agent_execution_id = ?1"#,
    )
    .bind(agent_execution_id.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(|row| parse_row(&row)).transpose()
}

pub async fn list_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<AgentExecutionRuntimeReceiptRecord>> {
    let rows = sqlx::query(
        r#"SELECT ar.agent_execution_id, ar.provider, ar.transport_family, ar.status,
                  ar.failure_phase, ar.event_count, ar.last_event_kind,
                  ar.last_event_at_ms, ar.receipt_json, ar.created_at, ar.updated_at
           FROM agent_execution_runtime_receipts ar
           INNER JOIN agent_executions ae ON ae.id = ar.agent_execution_id
           LEFT JOIN stage_executions se ON se.id = ae.stage_execution_id
           LEFT JOIN lead_conflict_mediations lcm
             ON ae.owner_kind = 'lead_conflict_mediation'
             AND ae.lead_mediation_record_id = lcm.id
           WHERE se.run_id = ?1 OR lcm.run_id = ?1
           ORDER BY ae.started_at ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_row).collect()
}

fn parse_row(row: &sqlx::sqlite::SqliteRow) -> Result<AgentExecutionRuntimeReceiptRecord> {
    let created_at_raw: String = row.get("created_at");
    let updated_at_raw: String = row.get("updated_at");
    let agent_execution_id: String = row.get("agent_execution_id");
    Ok(AgentExecutionRuntimeReceiptRecord {
        agent_execution_id: agent_execution_id
            .parse()
            .map_err(|e| anyhow::anyhow!("{e}"))?,
        provider: row.get("provider"),
        transport_family: row.get("transport_family"),
        status: row.get("status"),
        failure_phase: row.get("failure_phase"),
        event_count: row.get("event_count"),
        last_event_kind: row.get("last_event_kind"),
        last_event_at_ms: row.get("last_event_at_ms"),
        receipt_json: row.get("receipt_json"),
        created_at: DateTime::parse_from_rfc3339(&created_at_raw)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at_raw)?.with_timezone(&Utc),
    })
}
