use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::writer::{
    CRITICAL_WAL_SIZE_BYTES, TELEMETRY_FLUSH_CADENCE_MS, TELEMETRY_MAX_SAMPLES,
    TELEMETRY_MEMORY_CAP_BYTES, TELEMETRY_SNAPSHOT_TTL_HOURS, WARN_WAL_SIZE_BYTES,
};

const PRESSURE_STALE_AFTER_MS: i64 = 5 * 60 * 1000;
const STORAGE_HEALTH_STALE_AFTER_MS: i64 = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageWritePressureSnapshot {
    pub id: String,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub payload_json: Value,
    pub created_at: DateTime<Utc>,
}

impl StorageWritePressureSnapshot {
    pub fn new(
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
        payload_json: Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            window_start,
            window_end,
            payload_json,
            created_at: Utc::now(),
        }
    }
}

pub async fn insert_write_pressure_snapshot(
    pool: &SqlitePool,
    snapshot: &StorageWritePressureSnapshot,
) -> Result<()> {
    let payload = serde_json::to_string(&snapshot.payload_json)
        .context("serialize storage write pressure payload")?;
    if payload.len() > 65_536 {
        anyhow::bail!("storage write pressure payload exceeds 65536 bytes");
    }
    sqlx::query(
        r#"INSERT INTO storage_write_pressure_snapshots
           (id, window_start, window_end, payload_json, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5)"#,
    )
    .bind(&snapshot.id)
    .bind(snapshot.window_start.to_rfc3339())
    .bind(snapshot.window_end.to_rfc3339())
    .bind(payload)
    .bind(snapshot.created_at.to_rfc3339())
    .execute(pool)
    .await
    .context("insert storage write pressure snapshot")?;
    Ok(())
}

pub async fn latest_write_pressure_snapshot(
    pool: &SqlitePool,
) -> Result<Option<StorageWritePressureSnapshot>> {
    let row = sqlx::query(
        r#"SELECT id, window_start, window_end, payload_json, created_at
           FROM storage_write_pressure_snapshots
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await
    .context("latest storage write pressure snapshot")?;
    row.map(parse_snapshot_row).transpose()
}

pub async fn storage_health(pool: &SqlitePool) -> Result<Value> {
    let evidence = evidence_spool_summary(pool).await?;
    let latest_pressure = latest_write_pressure_snapshot(pool).await?;
    let updated_at = Utc::now();
    let is_pressure_stale = latest_pressure
        .as_ref()
        .map(|snapshot| {
            (updated_at - snapshot.created_at).num_milliseconds() > PRESSURE_STALE_AFTER_MS
        })
        .unwrap_or(false);
    let pressure_json = latest_pressure
        .as_ref()
        .map(write_pressure_snapshot_json)
        .unwrap_or_else(|| {
            json!({
                "state": "missing",
                "latestSnapshot": null,
                "freshness": {
                    "state": "missing",
                    "observedAt": null,
                    "ageMs": null,
                    "staleAfterMs": PRESSURE_STALE_AFTER_MS
                },
                "units": {
                    "queueDepth": "count",
                    "oldestQueuedMs": "milliseconds",
                    "dbWriterWaitP95Ms": "milliseconds",
                    "walSizeBytes": "bytes"
                }
            })
        });

    // Fail-closed: no live DbWriter heartbeat is available from this static call.
    // Report alive=false and degrade dbState so operators are not misled into
    // believing storage is healthy when writer telemetry is placeholder (SEC-003).
    let writer_alive = false;
    let is_stale = !writer_alive || is_pressure_stale;
    let db_state = if evidence["metadataRowsTotal"].as_i64().unwrap_or(0) == 0 {
        "MIGRATION_EMPTY"
    } else if !writer_alive {
        // Cannot certify health without a real writer heartbeat.
        "DEGRADED"
    } else if is_pressure_stale {
        "STALE"
    } else {
        "HEALTHY"
    };

    Ok(json!({
        "schemaVersion": "storage_health.v1",
        "updatedAt": updated_at.to_rfc3339(),
        "staleAfterMs": STORAGE_HEALTH_STALE_AFTER_MS,
        "isStale": is_stale,
        "dbState": db_state,
        "writer": {
            "alive": writer_alive,
            "lastHeartbeatAt": null,
            "lastDrainAt": null,
            "totalQueued": 0,
            "lanes": lane_health(),
            "writeLockWaitP50Ms": null,
            "writeLockWaitP95Ms": null,
            "transactionDurationP95Ms": null,
            "busyRetryRatePerMinute": 0,
            "busyRetryExhaustedTotal": 0,
            "rejectedTotal": 0,
            "droppedTelemetryTotal": 0
        },
        "wal": {
            "available": false,
            "unavailableReason": "wal_size_probe_not_mounted",
            "sizeBytes": null,
            "warnSizeBytes": WARN_WAL_SIZE_BYTES,
            "criticalSizeBytes": CRITICAL_WAL_SIZE_BYTES,
            "lastCheckpointAt": null,
            "checkpointDurationP95Ms": null
        },
        "projections": {
            "pendingInvalidations": 0,
            "projectionLagMs": null,
            "coalescedKeysPending": 0,
            "coalescedMergedTotal": 0,
            "coalescedFlushAgeP95Ms": null
        },
        "evidenceSpool": evidence,
        "writePressure": pressure_json,
        "telemetryRollup": {
            "memoryCapBytes": TELEMETRY_MEMORY_CAP_BYTES,
            "maxSamples": TELEMETRY_MAX_SAMPLES,
            "flushCadenceMs": TELEMETRY_FLUSH_CADENCE_MS,
            "snapshotTtlHours": TELEMETRY_SNAPSHOT_TTL_HOURS,
            "units": {
                "memoryCapBytes": "bytes",
                "flushCadenceMs": "milliseconds",
                "snapshotTtlHours": "hours"
            }
        },
        "killSwitches": {
            "dbWriterBypassClasses": [],
            "coalescingDisabledKeys": [],
            "evidenceSpoolDisabledKinds": []
        },
        "thresholds": storage_thresholds()
    }))
}

pub async fn evidence_spool_summary(pool: &SqlitePool) -> Result<Value> {
    evidence_spool_summary_for_run(pool, None).await
}

pub async fn evidence_spool_summary_for_run(
    pool: &SqlitePool,
    run_id: Option<&str>,
) -> Result<Value> {
    let mut query = r#"SELECT status, COUNT(*) AS count, COALESCE(SUM(size_bytes), 0) AS bytes
           FROM evidence_spool_refs"#
        .to_string();
    if run_id.is_some() {
        query.push_str(" WHERE run_id = ?1");
    }
    query.push_str(" GROUP BY status");
    let mut q = sqlx::query(&query);
    if let Some(run_id) = run_id {
        q = q.bind(run_id);
    }
    let rows = q.fetch_all(pool).await.context("evidence spool summary")?;
    let mut total_count = 0i64;
    let mut total_bytes = 0i64;
    let mut orphan_files = 0i64;
    let mut orphan_bytes = 0i64;
    let mut recovered_files = 0i64;
    let mut checksum_mismatch_files = 0i64;
    let mut pending_delete_files = 0i64;
    let mut by_status = serde_json::Map::new();
    for row in rows {
        let status: String = row.get("status");
        let count: i64 = row.get("count");
        let bytes: i64 = row.get("bytes");
        total_count += count;
        total_bytes += bytes;
        match status.as_str() {
            "recovered_orphan" => {
                orphan_files += count;
                orphan_bytes += bytes;
                recovered_files += count;
            }
            "checksum_mismatch" => checksum_mismatch_files += count,
            "pending_delete" => pending_delete_files += count,
            _ => {}
        }
        by_status.insert(
            status,
            json!({
                "count": count,
                "sizeBytes": bytes
            }),
        );
    }
    Ok(json!({
        "enabled": true,
        "filesWrittenTotal": total_count,
        "bytesWrittenTotal": total_bytes,
        "metadataRowsTotal": total_count,
        "orphanFiles": orphan_files,
        "orphanBytes": orphan_bytes,
        "recoveredFiles": recovered_files,
        "checksumMismatchFiles": checksum_mismatch_files,
        "pendingDeleteFiles": pending_delete_files,
        "byStatus": by_status,
        "units": {
            "filesWrittenTotal": "count",
            "bytesWrittenTotal": "bytes",
            "metadataRowsTotal": "count",
            "orphanFiles": "count",
            "orphanBytes": "bytes",
            "recoveredFiles": "count",
            "checksumMismatchFiles": "count",
            "pendingDeleteFiles": "count",
            "sizeBytes": "bytes"
        }
    }))
}

fn write_pressure_snapshot_json(snapshot: &StorageWritePressureSnapshot) -> Value {
    let age_ms = (Utc::now() - snapshot.created_at).num_milliseconds().max(0);
    let freshness = if age_ms <= PRESSURE_STALE_AFTER_MS {
        "fresh"
    } else {
        "stale"
    };
    json!({
        "state": "available",
        "latestSnapshot": {
            "id": snapshot.id,
            "windowStart": snapshot.window_start.to_rfc3339(),
            "windowEnd": snapshot.window_end.to_rfc3339(),
            "createdAt": snapshot.created_at.to_rfc3339(),
            "payload": snapshot.payload_json
        },
        "freshness": {
            "state": freshness,
            "observedAt": snapshot.created_at.to_rfc3339(),
            "ageMs": age_ms,
            "staleAfterMs": PRESSURE_STALE_AFTER_MS
        },
        "units": {
            "queueDepth": "count",
            "oldestQueuedMs": "milliseconds",
            "dbWriterWaitP95Ms": "milliseconds",
            "walSizeBytes": "bytes"
        }
    })
}

fn lane_health() -> Value {
    json!([
        lane_health_entry("critical_barrier", 1024),
        lane_health_entry("operator_command", 512),
        lane_health_entry("projection_invalidation", 2048),
        lane_health_entry("coalesced_projection", 4096),
        lane_health_entry("evidence_metadata", 2048),
        lane_health_entry("telemetry_rollup", 1024)
    ])
}

fn lane_health_entry(lane: &str, capacity: i64) -> Value {
    json!({
        "lane": lane,
        "capacity": capacity,
        "queuedDepth": 0,
        "queuedDepthRatio": 0.0,
        "oldestQueuedAgeMs": null,
        "rejectedTotal": 0,
        "droppedTotal": 0
    })
}

fn storage_thresholds() -> Value {
    json!([
        {"metric": "queued_depth_ratio_by_lane", "warn": 0.5, "critical": 0.8, "unit": "ratio", "action": "inspect_producer_rate"},
        {"metric": "oldest_queued_age_ms_class_a", "warn": 500.0, "critical": 1500.0, "unit": "milliseconds", "action": "inspect_lock_holder"},
        {"metric": "write_lock_wait_p95_ms", "warn": 100.0, "critical": 500.0, "unit": "milliseconds", "action": "inspect_contention"},
        {"metric": "class_a_transaction_duration_p95_ms", "warn": 50.0, "critical": 200.0, "unit": "milliseconds", "action": "audit_transaction_body"},
        {"metric": "busy_retry_rate_per_minute", "warn": 5.0, "critical": 30.0, "unit": "count_per_minute", "action": "check_write_contention"},
        {"metric": "wal_size_bytes", "warn": WARN_WAL_SIZE_BYTES as f64, "critical": CRITICAL_WAL_SIZE_BYTES as f64, "unit": "bytes", "action": "checkpoint_or_schedule_maintenance"},
        {"metric": "evidence_orphan_bytes", "warn": 10_485_760.0, "critical": 104_857_600.0, "unit": "bytes", "action": "run_storage_reconcile_evidence_orphans"}
    ])
}

fn parse_snapshot_row(row: sqlx::sqlite::SqliteRow) -> Result<StorageWritePressureSnapshot> {
    let payload: String = row.get("payload_json");
    Ok(StorageWritePressureSnapshot {
        id: row.get("id"),
        window_start: parse_time(row.get("window_start"))?,
        window_end: parse_time(row.get("window_end"))?,
        payload_json: serde_json::from_str(&payload).context("parse storage pressure payload")?,
        created_at: parse_time(row.get("created_at"))?,
    })
}

fn parse_time(value: String) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(&value)
        .context("parse storage write pressure timestamp")?
        .with_timezone(&Utc))
}
