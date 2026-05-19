use anyhow::{anyhow, Result};
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

const REPAIR_SLOT_OPERATION_NOT_FOUND: &str = "operation not found";
const REPAIR_SLOT_OPERATION_NOT_REPAIRABLE: &str = "operation is not repairable";
const REPAIR_SLOT_SLOT_GENERATION_MISMATCH: &str = "slot generation mismatch";
const REPAIR_SLOT_TRANSIENT_FAILURE: &str = "repair slot failed";

#[cfg(test)]
static FORCE_CAS_MISSES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MaintenanceOperation {
    pub id: String,
    pub operation_kind: String,
    pub status: String,
    pub idempotency_key: String,
    pub slot_generation: i64,
    pub metadata_json: Option<Value>,
    pub started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
    pub error: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

pub async fn repair_slot(
    pool: &SqlitePool,
    idempotency_key: &str,
    operation_id: &str,
    slot_generation: i64,
    caller_principal_id: &str,
    caller_principal_class: &str,
    request_id: Option<&str>,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<MaintenanceOperation> {
    if idempotency_key.is_empty() {
        return Err(anyhow!("idempotency_key cannot be empty"));
    }
    if idempotency_key.len() > 256 {
        return Err(anyhow!("idempotency_key too long (max 256 bytes)"));
    }
    if operation_id.len() > 256 {
        return Err(anyhow!("operation_id too long (max 256 bytes)"));
    }

    let delays = [50, 100, 250];
    for attempt in 0..=3 {
        match repair_slot_once(
            pool,
            idempotency_key,
            operation_id,
            slot_generation,
            caller_principal_id,
            caller_principal_class,
            request_id,
            cancel.clone(),
        )
        .await
        {
            Ok(op) => return Ok(op),
            Err(e) if attempt < 3 && e.to_string().contains("(TOCTOU)") => {
                let base_delay = delays[attempt];
                // P087: 20 percent jitter using rand
                let sleep_ms = {
                    let mut rng = rand::thread_rng();
                    let jitter_range = (base_delay as f64 * 0.4) as i64; // total 40% range (-20% to +20%)
                    let jitter: i64 = rng.gen_range(0..jitter_range.max(1)) - (jitter_range / 2);
                    (base_delay as i64 + jitter).max(1) as u64
                };

                tracing::warn!(
                    event = "maintenance_slot_release_cas_retry",
                    attempt = attempt + 1,
                    operation_id = %operation_id,
                    "CAS repair_slot attempt {} failed for {}, retrying in {}ms: {}",
                    attempt + 1,
                    operation_id,
                    sleep_ms,
                    e
                );
                tokio::select! {
                    _ = cancel.cancelled() => return Err(anyhow!("cancelled")),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)) => {}
                }
            }
            Err(e) => {
                if e.to_string().contains("(TOCTOU)") {
                    crate::metrics::increment_counter("maintenance_slot_release_cas_failed_total");
                    if let Err(diag_err) =
                        record_poisoned_slot_diagnostic(pool, operation_id, slot_generation, &e)
                            .await
                    {
                        tracing::warn!(
                            event = "maintenance_slot_release_poisoned_diagnostic_failed",
                            operation_id = %operation_id,
                            error = %diag_err,
                            "failed to persist poisoned repair_slot diagnostic"
                        );
                    }
                    tracing::error!(
                        event = "maintenance_slot_release_cas_failed",
                        operation_id = %operation_id,
                        slot_generation = slot_generation,
                        error = %e,
                        "CAS slot repair failed after all retries (poisoned slot)"
                    );
                }
                return Err(public_repair_slot_error(&e));
            }
        }
    }
    crate::metrics::increment_counter("maintenance_slot_release_cas_failed_total");
    tracing::error!(
        event = "maintenance_slot_release_cas_failed",
        operation_id = %operation_id,
        slot_generation = slot_generation,
        "CAS slot repair failed after 3 retries (exhausted)"
    );
    Err(anyhow!(REPAIR_SLOT_SLOT_GENERATION_MISMATCH))
}

async fn record_poisoned_slot_diagnostic(
    pool: &SqlitePool,
    operation_id: &str,
    slot_generation: i64,
    error: &anyhow::Error,
) -> Result<()> {
    let now_ms = Utc::now().timestamp_millis();
    let diagnostic_id = Uuid::new_v4().to_string();
    let metadata = serde_json::json!({
        "target_operation_id": operation_id,
        "target_slot_generation": slot_generation,
        "failure_kind": "cas_retry_exhausted",
    });
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "maintenance.repair_slot_poisoned")
            .await?;
    sqlx::query(
        "INSERT INTO maintenance_operations \
         (id, operation_kind, status, idempotency_key, slot_generation, metadata_json, error, created_at_ms, updated_at_ms) \
         VALUES (?, 'repair_slot_poisoned', 'failed', ?, ?, ?, ?, ?, ?)",
    )
    .bind(&diagnostic_id)
    .bind(format!(
        "repair-slot-poisoned:{operation_id}:{slot_generation}:{now_ms}"
    ))
    .bind(slot_generation)
    .bind(metadata.to_string())
    .bind(public_repair_slot_error(error).to_string())
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

fn public_repair_slot_error(error: &anyhow::Error) -> anyhow::Error {
    let msg = error.to_string();
    if msg.contains(REPAIR_SLOT_OPERATION_NOT_FOUND) {
        anyhow!(REPAIR_SLOT_OPERATION_NOT_FOUND)
    } else if msg.contains(REPAIR_SLOT_OPERATION_NOT_REPAIRABLE) {
        anyhow!(REPAIR_SLOT_OPERATION_NOT_REPAIRABLE)
    } else if msg.contains(REPAIR_SLOT_SLOT_GENERATION_MISMATCH) || msg.contains("(TOCTOU)") {
        anyhow!(REPAIR_SLOT_SLOT_GENERATION_MISMATCH)
    } else {
        anyhow!("{}: {}", REPAIR_SLOT_TRANSIENT_FAILURE, msg)
    }
}

async fn repair_slot_once(
    pool: &SqlitePool,
    idempotency_key: &str,
    operation_id: &str,
    slot_generation: i64,
    caller_principal_id: &str,
    caller_principal_class: &str,
    request_id: Option<&str>,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<MaintenanceOperation> {
    // P087-SEC-L-002: validate lengths before consuming a writer slot.
    if idempotency_key.len() > 256 {
        return Err(anyhow!("idempotency_key too long (max 256 bytes)"));
    }
    if operation_id.len() > 256 {
        return Err(anyhow!("operation_id too long (max 256 bytes)"));
    }
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let mut tx = crate::writer::begin_repository_transaction_cancellable(
        pool,
        "maintenance.repair_slot",
        cancel,
    )
    .await?;

    // 0. Audit Log: record the start of the repair operation
    let journal_id = Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "operation_id": operation_id,
        "slot_generation": slot_generation,
        "idempotency_key": idempotency_key,
    });
    crate::repos::command_journal::record_tx(
        &mut tx,
        &journal_id,
        "StorageMaintenanceRepairSlot",
        &payload.to_string(),
        None,
        now,
        Some("mcp"),
        Some(caller_principal_id),
        Some(caller_principal_class),
        Some("storage.maintenance.repair_slot"),
        request_id,
    )
    .await?;

    // 1. Check existing by idempotency_key
    let existing = sqlx::query(
        "SELECT id, operation_kind, status, idempotency_key, slot_generation, metadata_json, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms 
         FROM maintenance_operations WHERE idempotency_key = ?"
    )
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(row) = existing {
        let op = row_to_maintenance_op(row)?;
        if op.operation_kind == "repair_slot" {
            // Idempotency: return the original target recorded when this idempotency_key
            // was first used — not the current request's operation_id parameter.
            let original_target_id = op
                .metadata_json
                .as_ref()
                .and_then(|m| m.get("target_operation_id"))
                .and_then(|v| v.as_str())
                .unwrap_or(operation_id)
                .to_string();
            let target_row = sqlx::query(
                "SELECT id, operation_kind, status, idempotency_key, slot_generation, metadata_json, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms
                 FROM maintenance_operations WHERE id = ?",
            )
            .bind(&original_target_id)
            .fetch_one(&mut **tx)
            .await?;

            crate::repos::command_journal::complete_entry_tx(&mut tx, &journal_id, Utc::now())
                .await?;
            tx.commit().await?;
            return row_to_maintenance_op(target_row);
        }

        crate::repos::command_journal::complete_entry_tx(&mut tx, &journal_id, Utc::now()).await?;
        tx.commit().await?;
        return Ok(op);
    }

    // P087: Check if already repaired by anyone (different idempotency key)
    let already_repaired = sqlx::query(
        "SELECT id FROM maintenance_operations 
         WHERE operation_kind = 'repair_slot' 
         AND json_extract(metadata_json, '$.target_operation_id') = ?",
    )
    .bind(operation_id)
    .fetch_optional(&mut **tx)
    .await?;

    if already_repaired.is_some() {
        // Already repaired. Return the target operation.
        let target_row = sqlx::query(
            "SELECT id, operation_kind, status, idempotency_key, slot_generation, metadata_json, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms 
             FROM maintenance_operations WHERE id = ?",
        )
        .bind(operation_id)
        .fetch_one(&mut **tx)
        .await?;

        crate::repos::command_journal::complete_entry_tx(&mut tx, &journal_id, Utc::now()).await?;
        tx.commit().await?;
        return row_to_maintenance_op(target_row);
    }

    // 2. Read target operation
    let target = sqlx::query(
        "SELECT id, operation_kind, status, idempotency_key, slot_generation, metadata_json, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms 
         FROM maintenance_operations WHERE id = ?",
    )
    .bind(operation_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(target_row) = target else {
        crate::repos::command_journal::fail_entry_tx(
            &mut tx,
            &journal_id,
            Utc::now(),
            REPAIR_SLOT_OPERATION_NOT_FOUND,
        )
        .await?;
        tx.commit().await?;
        return Err(anyhow!(REPAIR_SLOT_OPERATION_NOT_FOUND));
    };
    let found_gen: i64 = target_row.get("slot_generation");
    let found_status: String = target_row.get("status");

    // P087: Verify terminal or orphaned state before release (CAS increment).
    // If the slot has already been repaired (found_gen > requested slot_generation),
    // return the current terminal state without a second release, even for different idempotency keys.
    if found_gen > slot_generation {
        crate::repos::command_journal::complete_entry_tx(&mut tx, &journal_id, Utc::now()).await?;
        tx.commit().await?;
        return row_to_maintenance_op(target_row);
    }

    // Operations in 'running' or 'pending' state must be reaped by the
    // background restart_reaper or naturally complete before they are repairable.
    if found_gen < slot_generation {
        crate::repos::command_journal::fail_entry_tx(
            &mut tx,
            &journal_id,
            Utc::now(),
            REPAIR_SLOT_SLOT_GENERATION_MISMATCH,
        )
        .await?;
        tx.commit().await?;
        return Err(anyhow!(REPAIR_SLOT_SLOT_GENERATION_MISMATCH));
    }

    if !matches!(found_status.as_str(), "completed" | "failed" | "orphaned") {
        crate::repos::command_journal::fail_entry_tx(
            &mut tx,
            &journal_id,
            Utc::now(),
            REPAIR_SLOT_OPERATION_NOT_REPAIRABLE,
        )
        .await?;
        tx.commit().await?;
        return Err(anyhow!(REPAIR_SLOT_OPERATION_NOT_REPAIRABLE));
    }

    // 3. CAS Update: increment slot_generation to ensure any zombie writes are blocked
    let rows_affected =
        release_slot_with_cas(&mut tx, operation_id, slot_generation, now_ms).await?;

    if rows_affected == 0 {
        crate::repos::command_journal::fail_entry_tx(
            &mut tx,
            &journal_id,
            Utc::now(),
            REPAIR_SLOT_SLOT_GENERATION_MISMATCH,
        )
        .await?;
        tx.commit().await?;
        return Err(anyhow!("{REPAIR_SLOT_SLOT_GENERATION_MISMATCH} (TOCTOU)"));
    }

    // 4. Record repair
    let repair_id = Uuid::new_v4().to_string();
    let metadata = serde_json::json!({
        "target_operation_id": operation_id,
        "target_slot_generation": slot_generation,
    });
    sqlx::query(
        "INSERT INTO maintenance_operations 
         (id, operation_kind, status, idempotency_key, slot_generation, metadata_json, created_at_ms, updated_at_ms) 
         VALUES (?, 'repair_slot', 'completed', ?, 1, ?, ?, ?)"
    )
    .bind(&repair_id)
    .bind(idempotency_key)
    .bind(serde_json::to_string(&metadata)?)
    .bind(now_ms)
    .bind(now_ms)
    .execute(&mut **tx)
    .await?;

    // Return the repaired operation (re-read to get new generation)
    let final_row = sqlx::query(
        "SELECT id, operation_kind, status, idempotency_key, slot_generation, metadata_json, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms 
         FROM maintenance_operations WHERE id = ?"
    )
    .bind(operation_id)
    .fetch_one(&mut **tx)
    .await?;

    let final_op = row_to_maintenance_op(final_row)?;
    crate::repos::command_journal::complete_entry_tx(&mut tx, &journal_id, Utc::now()).await?;
    tx.commit().await?;
    Ok(final_op)
}

async fn release_slot_with_cas(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    operation_id: &str,
    slot_generation: i64,
    now_ms: i64,
) -> Result<u64> {
    #[cfg(test)]
    {
        if FORCE_CAS_MISSES.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            FORCE_CAS_MISSES.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            return Ok(0);
        }
    }

    let result = sqlx::query(
        "UPDATE maintenance_operations 
         SET slot_generation = slot_generation + 1, updated_at_ms = ? 
         WHERE id = ? AND slot_generation = ?",
    )
    .bind(now_ms)
    .bind(operation_id)
    .bind(slot_generation)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected())
}

fn row_to_maintenance_op(row: sqlx::sqlite::SqliteRow) -> Result<MaintenanceOperation> {
    let metadata_json: Option<String> = row.get("metadata_json");

    // SEC-LOW-008: Cap metadata_json deserialization at 64 KiB
    if let Some(ref s) = metadata_json {
        if s.len() > 64 * 1024 {
            return Err(anyhow!("metadata_json too large (max 64 KiB)"));
        }
    }

    let metadata_json = metadata_json
        .map(|s| serde_json::from_str(&s))
        .transpose()?;

    Ok(MaintenanceOperation {
        id: row.get("id"),
        operation_kind: row.get("operation_kind"),
        status: row.get("status"),
        idempotency_key: row.get("idempotency_key"),
        slot_generation: row.get("slot_generation"),
        metadata_json,
        started_at_ms: row.get("started_at_ms"),
        completed_at_ms: row.get("completed_at_ms"),
        error: row.get("error"),
        created_at_ms: row.get("created_at_ms"),
        updated_at_ms: row.get("updated_at_ms"),
    })
}

/// P087: Periodic reaper for stuck maintenance operations.
/// Marks 'running' operations older than 10 minutes as 'orphaned'.
pub async fn run_reaper(pool: &SqlitePool) -> Result<()> {
    let now = Utc::now().timestamp_millis();
    let mut tx = crate::writer::begin_repository_transaction(pool, "maintenance.reaper").await?;

    // 1. Reap stuck 'running' operations (older than 10 minutes)
    let cutoff = now - 10 * 60 * 1000;
    sqlx::query(
        "UPDATE maintenance_operations 
         SET status = 'orphaned', slot_generation = slot_generation + 1, updated_at_ms = ? 
         WHERE status = 'running' AND updated_at_ms < ?",
    )
    .bind(now)
    .bind(cutoff)
    .execute(&mut **tx)
    .await?;

    // 2. Delete terminal or orphaned operations that are older than 24 hours (86,400,000 ms).
    // P087: Reaping an orphaned or terminal slot holds the row for 24 hours before deletion
    // to allow idempotency readback and post-mortem diagnostics.
    let hold_cutoff = now - 24 * 60 * 60 * 1000;
    sqlx::query(
        "DELETE FROM maintenance_operations 
         WHERE status IN ('completed', 'orphaned') 
         AND operation_kind != 'restart_reaper'
         AND updated_at_ms < ?",
    )
    .bind(hold_cutoff)
    .execute(&mut **tx)
    .await?;

    // P087: Reap consumed projection invalidation log rows older than 24 hours.
    crate::repos::projection_invalidation::reap_consumed_log_tx(&mut tx).await?;

    tx.commit().await?;

    // P087: Production consumer for projection invalidation backlog.
    // Drain the oldest source watermark first and freeze its cursor on retry
    // exhaustion/failure so stale projections are visible instead of silently
    // accumulating.
    match crate::repos::projections::drain_oldest_pending_invalidation(pool).await {
        Ok(Some((projection_name, source_name))) => {
            tracing::info!(
                event = "projection_invalidation_drain_priority",
                projection_name = %projection_name,
                source_name = %source_name,
                "Drained oldest projection invalidation source"
            );
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(event = "projection_invalidation_drain_failed", error = %e);
        }
    }

    // 3. Record reaper run
    let mut tx =
        crate::writer::begin_repository_transaction(pool, "maintenance.reaper.record").await?;
    let reaper_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO maintenance_operations 
         (id, operation_kind, status, idempotency_key, slot_generation, created_at_ms, updated_at_ms) 
         VALUES (?, 'restart_reaper', 'completed', ?, 1, ?, ?)",
    )
    .bind(&reaper_id)
    .bind(format!("reaper-run-{}", now))
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn acquire_slot(
    pool: &SqlitePool,
    idempotency_key: &str,
    operation_id: &str,
    operation_kind: &str,
    _caller_principal_id: &str,
    _caller_principal_class: &str,
    _request_id: Option<&str>,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<MaintenanceOperation> {
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let mut tx = crate::writer::begin_repository_transaction_cancellable(
        pool,
        "maintenance.acquire_slot",
        cancel,
    )
    .await?;

    // 1. Check idempotency
    let existing = sqlx::query(
        "SELECT id, operation_kind, status, idempotency_key, slot_generation, metadata_json, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms 
         FROM maintenance_operations WHERE idempotency_key = ?"
    )
    .bind(idempotency_key)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(row) = existing {
        let op = row_to_maintenance_op(row)?;
        tx.commit().await?;
        return Ok(op);
    }

    // 2. Check if operation_id exists and is acquireable
    let target =
        sqlx::query("SELECT id, status, slot_generation FROM maintenance_operations WHERE id = ?")
            .bind(operation_id)
            .fetch_optional(&mut **tx)
            .await?;

    if let Some(row) = target {
        let status: String = row.get("status");
        let gen: i64 = row.get("slot_generation");

        if status == "running" || status == "pending" {
            // Already active, but different idempotency key
            tx.commit().await?;
            return Err(anyhow!("operation already active (busy)"));
        }

        // Move to running, same generation? No, usually acquire bumps it?
        // Actually, if we acquired it after it was completed/failed/orphaned, we don't necessarily need to bump generation?
        // But to be safe against zombies, we should probably bump it.
        sqlx::query(
            "UPDATE maintenance_operations 
             SET status = 'running', idempotency_key = ?, operation_kind = ?, updated_at_ms = ?, started_at_ms = ?
             WHERE id = ? AND slot_generation = ?"
        )
        .bind(idempotency_key)
        .bind(operation_kind)
        .bind(now_ms)
        .bind(now_ms)
        .bind(operation_id)
        .bind(gen)
        .execute(&mut **tx)
        .await?;
    } else {
        // Create new
        sqlx::query(
            "INSERT INTO maintenance_operations 
             (id, operation_kind, status, idempotency_key, slot_generation, created_at_ms, updated_at_ms, started_at_ms) 
             VALUES (?, ?, 'running', ?, 1, ?, ?, ?)"
        )
        .bind(operation_id)
        .bind(operation_kind)
        .bind(idempotency_key)
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms)
        .execute(&mut **tx)
        .await?;
    }

    let final_row = sqlx::query(
        "SELECT id, operation_kind, status, idempotency_key, slot_generation, metadata_json, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms 
         FROM maintenance_operations WHERE idempotency_key = ?"
    )
    .bind(idempotency_key)
    .fetch_one(&mut **tx)
    .await?;

    let final_op = row_to_maintenance_op(final_row)?;
    tx.commit().await?;
    Ok(final_op)
}

pub async fn release_slot(
    pool: &SqlitePool,
    idempotency_key: &str,
    operation_id: &str,
    status: &str,
    error: Option<&str>,
    _caller_principal_id: &str,
    _caller_principal_class: &str,
    _request_id: Option<&str>,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<MaintenanceOperation> {
    let now = Utc::now();
    let now_ms = now.timestamp_millis();
    let mut tx = crate::writer::begin_repository_transaction_cancellable(
        pool,
        "maintenance.release_slot",
        cancel,
    )
    .await?;

    // 1. Check if matches idempotency_key
    let existing = sqlx::query(
        "SELECT id, operation_kind, status, idempotency_key, slot_generation, metadata_json, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms 
         FROM maintenance_operations WHERE id = ?"
    )
    .bind(operation_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(row) = existing else {
        tx.commit().await?;
        return Err(anyhow!("operation not found"));
    };
    let op = row_to_maintenance_op(row)?;
    if op.idempotency_key != idempotency_key {
        tx.commit().await?;
        return Ok(op); // Idempotent: if already released by someone else or matched by original key, return current state.
    }

    if op.status == "completed" || op.status == "failed" {
        tx.commit().await?;
        return Ok(op);
    }

    // 2. CAS Update: increment slot_generation and set status
    let rows_affected = sqlx::query(
        "UPDATE maintenance_operations 
         SET status = ?, slot_generation = slot_generation + 1, completed_at_ms = ?, error = ?, updated_at_ms = ? 
         WHERE id = ? AND idempotency_key = ?"
    )
    .bind(status)
    .bind(now_ms)
    .bind(error)
    .bind(now_ms)
    .bind(operation_id)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        tx.commit().await?;
        return Err(anyhow!(
            "failed to release slot (CAS mismatch or wrong key)"
        ));
    }

    let final_row = sqlx::query(
        "SELECT id, operation_kind, status, idempotency_key, slot_generation, metadata_json, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms 
         FROM maintenance_operations WHERE id = ?"
    )
    .bind(operation_id)
    .fetch_one(&mut **tx)
    .await?;

    let final_op = row_to_maintenance_op(final_row)?;
    tx.commit().await?;
    Ok(final_op)
}

pub fn spawn_maintenance_reaper(pool: SqlitePool) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = run_reaper(&pool).await {
                tracing::error!(err = %e, "P087 maintenance reaper failed");
            }
        }
    })
}

pub async fn last_reaper_run(pool: &SqlitePool) -> Result<Option<i64>> {
    let row = sqlx::query(
        "SELECT updated_at_ms FROM maintenance_operations 
         WHERE operation_kind = 'restart_reaper' 
         ORDER BY updated_at_ms DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.get("updated_at_ms")))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    async fn proposal_087_pool() -> SqlitePool {
        crate::pool::create_pool("sqlite::memory:").await.unwrap()
    }

    async fn insert_maintenance_operation(pool: &SqlitePool, id: &str, status: &str) {
        let now_ms = Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO maintenance_operations \
             (id, operation_kind, status, idempotency_key, slot_generation, created_at_ms, updated_at_ms) \
             VALUES (?, 'projection_rebuild', ?, ?, 3, ?, ?)",
        )
        .bind(id)
        .bind(status)
        .bind(format!("{id}-idem"))
        .bind(now_ms)
        .bind(now_ms)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn latest_repair_journal(
        pool: &SqlitePool,
    ) -> (String, String, Option<String>, String, String, String) {
        sqlx::query_as(
            "SELECT result_status, payload_json, error, caller_principal_id, caller_principal_class, request_id \
             FROM command_journal \
             WHERE command_type = 'StorageMaintenanceRepairSlot' \
             ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_missing_target_fails_closed_with_audit() {
        let _lock = test_lock();
        let pool = proposal_087_pool().await;
        let err = repair_slot(
            &pool,
            "p087-missing-idem",
            "p087-missing-operation",
            3,
            "operator-p087",
            "Operator",
            Some("request-p087"),
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect_err("missing target must fail");

        let message = err.to_string();
        assert!(
            !message.contains("p087-missing-operation"),
            "public repair_slot error must not leak the target id: {message}"
        );

        let (status, payload, error, caller, class, request_id) =
            latest_repair_journal(&pool).await;
        assert_eq!(status, "failed");
        assert!(payload.contains("p087-missing-operation"));
        assert_eq!(error.as_deref(), Some("operation not found"));
        assert_eq!(caller, "operator-p087");
        assert_eq!(class, "Operator");
        assert_eq!(request_id, "request-p087");
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_reject_active_target_with_audit() {
        let _lock = test_lock();
        let pool = proposal_087_pool().await;
        insert_maintenance_operation(&pool, "p087-pending-operation", "pending").await;
        insert_maintenance_operation(&pool, "p087-running-operation", "running").await;

        for op_id in ["p087-pending-operation", "p087-running-operation"] {
            let err = repair_slot(
                &pool,
                &format!("{op_id}-repair-idem"),
                op_id,
                3,
                "operator-p087",
                "Operator",
                Some("request-p087"),
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .expect_err(
                "active targets must not be repairable by repair_slot (must wait for reaper)",
            );

            assert_eq!(err.to_string(), "operation is not repairable");
        }
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_rejects_empty_idempotency_key() {
        let _lock = test_lock();
        let pool = proposal_087_pool().await;
        let err = repair_slot(
            &pool,
            "",
            "op-1",
            3,
            "op-user",
            "Operator",
            None,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect_err("empty idempotency key must fail");
        assert_eq!(err.to_string(), "idempotency_key cannot be empty");
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_checks_generation_mismatch() {
        let _lock = test_lock();
        let pool = proposal_087_pool().await;
        insert_maintenance_operation(&pool, "op-1", "running").await;

        let err = repair_slot(
            &pool,
            "idem-1",
            "op-1",
            4, // Wrong generation (inserted as 3)
            "op-user",
            "Operator",
            None,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect_err("generation mismatch must fail");
        assert_eq!(err.to_string(), "slot generation mismatch");
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_success_on_terminal_or_orphaned_state() {
        let _lock = test_lock();
        let pool = proposal_087_pool().await;
        insert_maintenance_operation(&pool, "op-completed", "completed").await;
        insert_maintenance_operation(&pool, "op-failed", "failed").await;
        insert_maintenance_operation(&pool, "op-orphaned", "orphaned").await;

        for (op_id, expected_status) in [
            ("op-completed", "completed"),
            ("op-failed", "failed"),
            ("op-orphaned", "orphaned"),
        ] {
            let op = repair_slot(
                &pool,
                &format!("{op_id}-repair-idem"),
                op_id,
                3,
                "op-user",
                "Operator",
                None,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
            .expect("terminal/orphaned state must be repairable (slot release)");
            assert_eq!(op.status, expected_status);

            // Verify generation was incremented
            let new_gen: i64 = sqlx::query_scalar(
                "SELECT slot_generation FROM maintenance_operations WHERE id = ?",
            )
            .bind(op_id)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(new_gen, 4);
        }

        // Verify repair_slot records were created (one per successful repair)
        let repair_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM maintenance_operations WHERE operation_kind = 'repair_slot'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(repair_rows, 3);
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_cas_exhaustion_persists_poisoned_diagnostic() {
        let _lock = test_lock();
        let pool = proposal_087_pool().await;
        insert_maintenance_operation(&pool, "op-cas", "orphaned").await;
        FORCE_CAS_MISSES.store(4, std::sync::atomic::Ordering::SeqCst);

        let err = repair_slot(
            &pool,
            "op-cas-repair-idem",
            "op-cas",
            3,
            "op-user",
            "Operator",
            None,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect_err("forced CAS misses must fail after bounded retries");
        FORCE_CAS_MISSES.store(0, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(err.to_string(), "slot generation mismatch");

        let target: (String, i64) = sqlx::query_as(
            "SELECT status, slot_generation FROM maintenance_operations WHERE id = 'op-cas'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(target, ("orphaned".to_string(), 3));

        let diagnostic: (String, String, Option<String>) = sqlx::query_as(
            "SELECT operation_kind, status, error FROM maintenance_operations \
             WHERE operation_kind = 'repair_slot_poisoned' ORDER BY created_at_ms DESC LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(diagnostic.0, "repair_slot_poisoned");
        assert_eq!(diagnostic.1, "failed");
        assert_eq!(diagnostic.2.as_deref(), Some("slot generation mismatch"));
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_idempotency_different_keys_after_success() {
        let _lock = test_lock();
        let pool = proposal_087_pool().await;
        insert_maintenance_operation(&pool, "op-repaired", "orphaned").await;

        // 1. First repair with key-1
        let op1 = repair_slot(
            &pool,
            "key-1",
            "op-repaired",
            3,
            "op-user",
            "Operator",
            None,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("first repair must succeed");
        assert_eq!(op1.slot_generation, 4);

        // 2. Second repair with SAME key-1 (standard idempotency)
        let op2 = repair_slot(
            &pool,
            "key-1",
            "op-repaired",
            3,
            "op-user",
            "Operator",
            None,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("same-key retry must succeed");
        assert_eq!(op2.slot_generation, 4);

        // 3. Third repair with DIFFERENT key-2 after it was already repaired (slot_generation is now 4)
        // Requested generation is still 3 (the one we want to repair).
        let op3 = repair_slot(
            &pool, "key-2", "op-repaired", 3,
            "op-user", "Operator", None, tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("different-key retry after repair must return current terminal state without second release");
        assert_eq!(op3.slot_generation, 4);

        // Verify only ONE repair_slot record exists (from the first successful one)
        let repair_keys: Vec<String> = sqlx::query_scalar(
            "SELECT idempotency_key FROM maintenance_operations WHERE operation_kind = 'repair_slot' ORDER BY created_at_ms",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(repair_keys, vec!["key-1".to_string()]);
    }

    #[tokio::test]
    async fn proposal_087_repair_slot_idempotency_different_keys_after_success_with_current_gen() {
        let _lock = test_lock();
        let pool = proposal_087_pool().await;
        insert_maintenance_operation(&pool, "op-repaired", "orphaned").await;

        // 1. First repair with key-1 (stale gen 3) -> bumps to 4
        let op1 = repair_slot(
            &pool,
            "key-1",
            "op-repaired",
            3,
            "op-user",
            "Operator",
            None,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("first repair must succeed");
        assert_eq!(op1.slot_generation, 4);

        // 2. Second repair with DIFFERENT key-2 using the NEW gen 4
        let op2 = repair_slot(
            &pool,
            "key-2",
            "op-repaired",
            4,
            "op-user",
            "Operator",
            None,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("different-key with current gen");

        // If it currently increments again, this will be 5.
        // The audit report implies it should stay 4.
        assert_eq!(
            op2.slot_generation, 4,
            "Should NOT have incremented again if already repaired"
        );
    }

    #[tokio::test]
    async fn proposal_087_restart_reaper_orphans_only_stale_running_slots() {
        let _lock = test_lock();
        let pool = proposal_087_pool().await;
        let now_ms = Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO maintenance_operations \
             (id, operation_kind, status, idempotency_key, slot_generation, created_at_ms, updated_at_ms) \
             VALUES \
             ('old-running', 'projection_rebuild', 'running', 'old-running-idem', 2, ?, ?), \
             ('fresh-running', 'projection_rebuild', 'running', 'fresh-running-idem', 2, ?, ?), \
             ('pending-slot', 'projection_rebuild', 'pending', 'pending-slot-idem', 2, ?, ?)",
        )
        .bind(now_ms - 20 * 60 * 1000)
        .bind(now_ms - 20 * 60 * 1000)
        .bind(now_ms)
        .bind(now_ms)
        .bind(now_ms - 20 * 60 * 1000)
        .bind(now_ms - 20 * 60 * 1000)
        .execute(&pool)
        .await
        .unwrap();

        run_reaper(&pool).await.unwrap();

        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT id, status, slot_generation FROM maintenance_operations \
             WHERE id IN ('old-running', 'fresh-running', 'pending-slot') \
             ORDER BY id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![
                ("fresh-running".to_string(), "running".to_string(), 2),
                ("old-running".to_string(), "orphaned".to_string(), 3),
                ("pending-slot".to_string(), "pending".to_string(), 2),
            ]
        );
        assert!(
            last_reaper_run(&pool).await.unwrap().is_some(),
            "storage health rollout readback must have a restart reaper timestamp"
        );
    }
}
