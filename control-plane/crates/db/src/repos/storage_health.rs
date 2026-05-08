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

    Ok(json!({
        "schemaVersion": 1,
        "state": "ready",
        "generatedAt": Utc::now().to_rfc3339(),
        "writer": {
            "alive": null,
            "heartbeatFresh": null,
            "heartbeatAgeMs": null,
            "heartbeatFreshness": "not_mounted_in_db_readback",
            "units": {
                "heartbeatAgeMs": "milliseconds",
                "laneDepth": "count",
                "queueWaitMs": "milliseconds",
                "txDurationMs": "milliseconds"
            }
        },
        "wal": {
            "sizeBytes": null,
            "warnThresholdBytes": WARN_WAL_SIZE_BYTES,
            "criticalThresholdBytes": CRITICAL_WAL_SIZE_BYTES,
            "checkpointPolicy": {
                "passiveAboveBytes": WARN_WAL_SIZE_BYTES,
                "truncateOnlyOnShutdownOrExplicitMaintenance": true
            },
            "units": {
                "sizeBytes": "bytes",
                "warnThresholdBytes": "bytes",
                "criticalThresholdBytes": "bytes"
            }
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
            "dbWriterBypassFailClosed": true,
            "evidenceSpoolWritesEnabled": true,
            "telemetryRollupEnabled": true
        }
    }))
}

pub async fn evidence_spool_summary(pool: &SqlitePool) -> Result<Value> {
    let rows = sqlx::query(
        r#"SELECT status, COUNT(*) AS count, COALESCE(SUM(size_bytes), 0) AS bytes
           FROM evidence_spool_refs
           GROUP BY status"#,
    )
    .fetch_all(pool)
    .await
    .context("evidence spool summary")?;
    let mut total_count = 0i64;
    let mut total_bytes = 0i64;
    let mut by_status = serde_json::Map::new();
    for row in rows {
        let status: String = row.get("status");
        let count: i64 = row.get("count");
        let bytes: i64 = row.get("bytes");
        total_count += count;
        total_bytes += bytes;
        by_status.insert(
            status,
            json!({
                "count": count,
                "sizeBytes": bytes
            }),
        );
    }
    Ok(json!({
        "totalRefs": total_count,
        "totalSizeBytes": total_bytes,
        "byStatus": by_status,
        "units": {
            "totalRefs": "count",
            "totalSizeBytes": "bytes",
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
