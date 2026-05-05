use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::ids::{RoutingReceiptId, RunId, SystemExecutionId};
use domain::routing::{RoutingReceipt, RoutingReceiptStatus};

pub async fn insert_tx(tx: &mut Transaction<'_, Sqlite>, receipt: &RoutingReceipt) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO routing_receipts (receipt_id, run_id, stage_id, attempt_id, system_execution_id,
                                      status, failure_kind, plan_hash, input_snapshot_hashes_json,
                                      operator_actions_json, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(receipt.receipt_id.to_string())
    .bind(receipt.run_id.to_string())
    .bind(&receipt.stage_id)
    .bind(receipt.attempt_id)
    .bind(receipt.system_execution_id.to_string())
    .bind(receipt.status.to_string())
    .bind(&receipt.failure_kind)
    .bind(&receipt.plan_hash)
    .bind(&receipt.input_snapshot_hashes_json)
    .bind(&receipt.operator_actions_json)
    .bind(receipt.created_at.to_rfc3339())
    .execute(&mut **tx)
    .await
    .context("insert routing_receipt")?;
    Ok(())
}

pub async fn insert(pool: &SqlitePool, receipt: &RoutingReceipt) -> Result<()> {
    let mut tx = crate::pool::begin_immediate_with_retry(pool, "routing_receipts.insert").await?;
    insert_tx(&mut tx, receipt).await?;
    tx.commit().await.context("commit insert routing_receipt")?;
    Ok(())
}

pub async fn find_by_id(
    pool: &SqlitePool,
    receipt_id: RoutingReceiptId,
) -> Result<Option<RoutingReceipt>> {
    let row = sqlx::query(
        "SELECT receipt_id, run_id, stage_id, attempt_id, system_execution_id, status, \
         failure_kind, plan_hash, input_snapshot_hashes_json, operator_actions_json, created_at \
         FROM routing_receipts WHERE receipt_id = ?1",
    )
    .bind(receipt_id.to_string())
    .fetch_optional(pool)
    .await
    .context("find routing_receipt by id")?;

    row.map(|r| parse_row(&r)).transpose()
}

pub async fn list_by_run(pool: &SqlitePool, run_id: RunId) -> Result<Vec<RoutingReceipt>> {
    let rows = sqlx::query(
        "SELECT receipt_id, run_id, stage_id, attempt_id, system_execution_id, status, \
         failure_kind, plan_hash, input_snapshot_hashes_json, operator_actions_json, created_at \
         FROM routing_receipts WHERE run_id = ?1 ORDER BY created_at",
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await
    .context("list routing_receipts by run")?;

    rows.iter().map(parse_row).collect()
}

fn parse_row(r: &sqlx::sqlite::SqliteRow) -> Result<RoutingReceipt> {
    let receipt_id_str: String = r.get("receipt_id");
    let receipt_id: RoutingReceiptId = receipt_id_str
        .parse()
        .map_err(|e| anyhow::anyhow!("parse routing_receipt receipt_id: {e}"))?;
    let run_id_str: String = r.get("run_id");
    let run_id: RunId = run_id_str
        .parse()
        .map_err(|e| anyhow::anyhow!("parse routing_receipt run_id: {e}"))?;
    let sys_exec_id_str: String = r.get("system_execution_id");
    let system_execution_id: SystemExecutionId = sys_exec_id_str
        .parse()
        .map_err(|e| anyhow::anyhow!("parse routing_receipt system_execution_id: {e}"))?;
    let status_str: String = r.get("status");
    let status: RoutingReceiptStatus = status_str
        .parse()
        .map_err(|e| anyhow::anyhow!("parse routing_receipt status: {e}"))?;
    let created_at_str: String = r.get("created_at");
    let created_at: DateTime<Utc> = DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| anyhow::anyhow!("parse routing_receipt created_at: {e}"))?
        .with_timezone(&Utc);

    Ok(RoutingReceipt {
        receipt_id,
        run_id,
        stage_id: r.get("stage_id"),
        attempt_id: r.get("attempt_id"),
        system_execution_id,
        status,
        failure_kind: r.get("failure_kind"),
        plan_hash: r.get("plan_hash"),
        input_snapshot_hashes_json: r.get("input_snapshot_hashes_json"),
        operator_actions_json: r.get("operator_actions_json"),
        created_at,
    })
}
