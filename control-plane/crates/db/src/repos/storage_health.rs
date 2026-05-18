use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::writer::{
    execute_repository_transaction_operation, repository_transaction_operation,
    DbWriterHealthSnapshot, DbWriterHeartbeat, CRITICAL_WAL_SIZE_BYTES, TELEMETRY_FLUSH_CADENCE_MS,
    TELEMETRY_MAX_SAMPLES, TELEMETRY_MEMORY_CAP_BYTES, TELEMETRY_SNAPSHOT_RETAIN_LATEST,
    TELEMETRY_SNAPSHOT_TTL_HOURS, WARN_WAL_SIZE_BYTES,
};

const PRESSURE_STALE_AFTER_MS: i64 = 5 * 60 * 1000;
const STORAGE_HEALTH_STALE_AFTER_MS: i64 = 5_000;
const DIAGNOSTIC_ROW_LIMIT: i64 = 100;

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
    let payload_value = snapshot.payload_json.clone();
    let payload = serde_json::to_string(&payload_value)
        .context("serialize storage write pressure payload")?;
    if payload.len() > 65_536 {
        anyhow::bail!("storage write pressure payload exceeds 65536 bytes");
    }
    let mut op = repository_transaction_operation("storage_health.insert_write_pressure_snapshot");
    op.idempotency_key = format!(
        "storage_health:write_pressure:{}:{}",
        snapshot.window_start.timestamp_millis(),
        snapshot.window_end.timestamp_millis()
    );
    let id = snapshot.id.clone();
    let window_start = snapshot.window_start.to_rfc3339();
    let window_end = snapshot.window_end.to_rfc3339();
    let created_at = snapshot.created_at.to_rfc3339();
    let retention_cutoff = (snapshot.created_at
        - chrono::Duration::hours(TELEMETRY_SNAPSHOT_TTL_HOURS as i64))
    .to_rfc3339();
    execute_repository_transaction_operation(
        pool,
        op,
        "storage_health.insert_write_pressure_snapshot",
        Box::new(move |tx| {
            Box::pin(async move {
                let existing = sqlx::query(
                    r#"SELECT id, payload_json
                       FROM storage_write_pressure_snapshots
                       WHERE window_start = ?1 AND window_end = ?2
                       ORDER BY created_at ASC, id ASC
                       LIMIT 1"#,
                )
                .bind(&window_start)
                .bind(&window_end)
                .fetch_optional(&mut **tx)
                .await?;
                let rows = if let Some(existing) = existing {
                    let existing_id: String = existing.get("id");
                    let existing_payload: String = existing.get("payload_json");
                    let existing_payload: Value = serde_json::from_str(&existing_payload)
                        .context("parse existing storage pressure payload for telemetry merge")?;
                    let merged_payload =
                        merge_write_pressure_payload(existing_payload, payload_value.clone());
                    let merged_payload = serde_json::to_string(&merged_payload)
                        .context("serialize merged storage pressure payload")?;
                    if merged_payload.len() > 65_536 {
                        anyhow::bail!("merged storage write pressure payload exceeds 65536 bytes");
                    }
                    sqlx::query(
                        r#"UPDATE storage_write_pressure_snapshots
                           SET payload_json = ?1, created_at = ?2
                           WHERE id = ?3"#,
                    )
                    .bind(merged_payload)
                    .bind(&created_at)
                    .bind(existing_id)
                    .execute(&mut **tx)
                    .await?
                    .rows_affected() as u32
                } else {
                    sqlx::query(
                        r#"INSERT INTO storage_write_pressure_snapshots
                           (id, window_start, window_end, payload_json, created_at)
                           VALUES (?1, ?2, ?3, ?4, ?5)"#,
                    )
                    .bind(id)
                    .bind(&window_start)
                    .bind(&window_end)
                    .bind(payload)
                    .bind(&created_at)
                    .execute(&mut **tx)
                    .await?
                    .rows_affected() as u32
                };
                let ttl_deleted = sqlx::query(
                    r#"DELETE FROM storage_write_pressure_snapshots
                       WHERE created_at < ?1"#,
                )
                .bind(retention_cutoff)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;
                let cap_deleted = sqlx::query(
                    r#"DELETE FROM storage_write_pressure_snapshots
                       WHERE id NOT IN (
                         SELECT id
                         FROM storage_write_pressure_snapshots
                         ORDER BY created_at DESC
                         LIMIT ?1
                       )"#,
                )
                .bind(TELEMETRY_SNAPSHOT_RETAIN_LATEST)
                .execute(&mut **tx)
                .await?
                .rows_affected() as u32;
                Ok(((), rows + ttl_deleted + cap_deleted))
            })
        }),
    )
    .await
    .context("insert storage write pressure snapshot")?;
    Ok(())
}

pub async fn record_live_write_pressure_rollup(
    pool: &SqlitePool,
    writer_heartbeat: &DbWriterHeartbeat,
) -> Result<StorageWritePressureSnapshot> {
    let now = Utc::now();
    let writer = writer_heartbeat.snapshot();
    let payload = live_rollup_payload(&writer)?;
    let snapshot =
        StorageWritePressureSnapshot::new(now - chrono::Duration::milliseconds(1), now, payload);
    insert_write_pressure_snapshot(pool, &snapshot).await?;
    Ok(snapshot)
}

fn merge_write_pressure_payload(existing: Value, incoming: Value) -> Value {
    let (mut existing, incoming) = match (existing, incoming) {
        (Value::Object(existing), Value::Object(incoming)) => (existing, incoming),
        (_, incoming) => return incoming,
    };

    for (key, incoming_value) in incoming {
        if key == "rollup" {
            let existing_rollup = existing.remove("rollup").unwrap_or(Value::Null);
            existing.insert(key, merge_rollup_payload(existing_rollup, incoming_value));
        } else {
            existing.insert(key, incoming_value);
        }
    }

    Value::Object(existing)
}

fn merge_rollup_payload(existing: Value, incoming: Value) -> Value {
    let (mut existing, incoming) = match (existing, incoming) {
        (Value::Object(existing), Value::Object(incoming)) => (existing, incoming),
        (_, incoming) => return incoming,
    };

    for (key, incoming_value) in incoming {
        if key == "lanes" {
            let existing_lanes = existing.remove("lanes").unwrap_or(Value::Null);
            existing.insert(key, merge_lane_payloads(existing_lanes, incoming_value));
        } else if is_additive_counter(&key) {
            existing.insert(
                key.clone(),
                json!(number_as_u64(existing.get(&key)) + number_as_u64(Some(&incoming_value))),
            );
        } else if is_max_gauge(&key) {
            existing.insert(
                key.clone(),
                json!(number_as_u64(existing.get(&key)).max(number_as_u64(Some(&incoming_value)))),
            );
        } else {
            existing.insert(key, incoming_value);
        }
    }

    Value::Object(existing)
}

fn merge_lane_payloads(existing: Value, incoming: Value) -> Value {
    let (existing, incoming) = match (existing, incoming) {
        (Value::Array(existing), Value::Array(incoming)) => (existing, incoming),
        (_, incoming) => return incoming,
    };
    let mut by_lane = serde_json::Map::new();
    for lane in existing {
        let Some(lane_name) = lane.get("lane").and_then(Value::as_str).map(str::to_string) else {
            continue;
        };
        by_lane.insert(lane_name, lane);
    }
    for lane in incoming {
        let Some(lane_name) = lane.get("lane").and_then(Value::as_str).map(str::to_string) else {
            continue;
        };
        let merged = if let Some(existing_lane) = by_lane.remove(&lane_name) {
            merge_lane_payload(existing_lane, lane)
        } else {
            lane
        };
        by_lane.insert(lane_name, merged);
    }
    Value::Array(by_lane.into_values().collect())
}

fn merge_lane_payload(existing: Value, incoming: Value) -> Value {
    let (mut existing, incoming) = match (existing, incoming) {
        (Value::Object(existing), Value::Object(incoming)) => (existing, incoming),
        (_, incoming) => return incoming,
    };
    for (key, incoming_value) in incoming {
        if is_additive_counter(&key) {
            existing.insert(
                key.clone(),
                json!(number_as_u64(existing.get(&key)) + number_as_u64(Some(&incoming_value))),
            );
        } else if is_max_gauge(&key) {
            existing.insert(
                key.clone(),
                json!(number_as_u64(existing.get(&key)).max(number_as_u64(Some(&incoming_value)))),
            );
        } else {
            existing.insert(key, incoming_value);
        }
    }
    Value::Object(existing)
}

fn is_additive_counter(key: &str) -> bool {
    key.ends_with("Total")
}

fn is_max_gauge(key: &str) -> bool {
    matches!(
        key,
        "totalQueued"
            | "queuedDepth"
            | "capacity"
            | "oldestQueuedAgeMs"
            | "transactionDurationP50Ms"
            | "transactionDurationP95Ms"
            | "transactionDurationSampleCount"
            | "estimatedMemoryBytes"
    )
}

fn number_as_u64(value: Option<&Value>) -> u64 {
    value.and_then(Value::as_u64).unwrap_or(0)
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
    storage_health_with_writer(pool, None).await
}

pub fn reset_read_path_metrics_for_tests() {
    crate::metrics::reset_read_path_metrics_for_tests();
}

pub async fn storage_health_with_writer(
    pool: &SqlitePool,
    writer_heartbeat: Option<&DbWriterHeartbeat>,
) -> Result<Value> {
    let evidence = evidence_spool_summary(pool).await?;
    let wal_json = wal_health(pool).await;
    let latest_pressure = latest_write_pressure_snapshot(pool).await?;
    let updated_at = Utc::now();
    let writer_snapshot = writer_heartbeat.map(DbWriterHeartbeat::snapshot);
    let lock_metrics = crate::pool::write_lock_metrics_snapshot();
    let (pending_invalidations, projection_lag_ms) = projection_invalidation_stats(pool)
        .await
        .unwrap_or((0, None));
    let projection_freshness = projection_freshness_summary(pool).await.unwrap_or_default();
    let hot_read_guards = hot_read_circuit_summary(pool).await.unwrap_or_default();
    let maintenance_operations = maintenance_operations_summary(pool)
        .await
        .unwrap_or_default();
    let artifact_noise_projection = artifact_noise_projection(pool).await.unwrap_or_default();
    let rollout = rollout_readback(pool).await.unwrap_or(json!({}));

    let is_pressure_stale = latest_pressure
        .as_ref()
        .map(|snapshot| {
            (updated_at - snapshot.created_at).num_milliseconds() > PRESSURE_STALE_AFTER_MS
        })
        .unwrap_or(false);
    let pressure_json = latest_pressure
        .as_ref()
        .map(write_pressure_snapshot_json)
        .or_else(|| {
            writer_snapshot
                .as_ref()
                .map(|snapshot| live_write_pressure_json(updated_at, snapshot))
        })
        .unwrap_or_else(missing_write_pressure_json);

    // Fail-closed when no live DbWriter heartbeat is supplied. Daemon-owned
    // GraphQL/MCP paths inject the shared writer heartbeat; tests and static
    // helpers that omit it must not claim storage is healthy from placeholders.
    let writer_alive = writer_snapshot
        .as_ref()
        .map(|snapshot| snapshot.alive)
        .unwrap_or(false);
    let is_stale = !writer_alive || is_pressure_stale;
    let (circuit_status, _, _, _, _, _) =
        crate::repos::hot_read_circuit::get_circuit_state(pool, "storage.health")
            .await
            .unwrap_or((
                crate::repos::hot_read_circuit::CircuitStatus::Closed,
                0,
                0,
                None,
                None,
                false,
            ));
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

    let degraded = if !writer_alive {
        Some(json!({
            "severity": "critical",
            "reason": "writer_offline",
            "message": "DbWriter heartbeat is missing or stale"
        }))
    } else if is_pressure_stale {
        Some(json!({
            "severity": "warn",
            "reason": "telemetry_stale",
            "message": "Storage write pressure telemetry is stale"
        }))
    } else {
        None
    };

    let runs_list_p95 = crate::metrics::get_runs_list_read_latency_p95().unwrap_or(0);
    let mcp_liveness_p95 = crate::metrics::get_mcp_liveness_gate_duration_p95().unwrap_or(0);

    let reaper_status = {
        let last_reaper_ms = sqlx::query("SELECT MAX(updated_at_ms) as last FROM maintenance_operations WHERE operation_kind = 'restart_reaper'")
            .fetch_one(pool)
            .await?
            .get::<Option<i64>, _>("last")
            .unwrap_or(0);
        let age_ms = Utc::now().timestamp_millis() - last_reaper_ms;
        if age_ms > 60 * 60 * 1000 {
            "breach"
        } else if age_ms > 15 * 60 * 1000 {
            "warn"
        } else {
            "healthy"
        }
    };

    Ok(json!({
        "schemaVersion": "storage_health.v1",
        "updatedAt": updated_at.to_rfc3339(),
        "staleAfterMs": STORAGE_HEALTH_STALE_AFTER_MS,
        "isStale": is_stale,
        "dbState": db_state,
        "writer": {
            "alive": writer_alive,
            "lastHeartbeatAt": writer_snapshot.as_ref().and_then(|snapshot| snapshot.last_heartbeat_at.clone()),
            "lastDrainAt": writer_snapshot.as_ref().and_then(|snapshot| snapshot.last_drain_at.clone()),
            "totalQueued": writer_snapshot.as_ref().map(|snapshot| snapshot.total_queued).unwrap_or(0),
            "lanes": lane_health(writer_snapshot.as_ref()),
            "writeLockWaitP50Ms": lock_metrics.wait_p50_ms,
            "writeLockWaitP95Ms": lock_metrics.wait_p95_ms,
            "transactionDurationP50Ms": writer_snapshot.as_ref().and_then(|snapshot| snapshot.transaction_duration_p50_ms),
            "transactionDurationP95Ms": writer_snapshot.as_ref().and_then(|snapshot| snapshot.transaction_duration_p95_ms),
            "busyRetryRatePerMinute": lock_metrics.busy_retry_rate_per_minute,
            "busyRetryExhaustedTotal": lock_metrics.busy_retry_exhausted_total,
            "rejectedTotal": writer_snapshot.as_ref().map(|snapshot| snapshot.coalesced_rejected_total).unwrap_or(0),
            "droppedTelemetryTotal": writer_snapshot.as_ref().map(|snapshot| snapshot.telemetry_dropped_total).unwrap_or(0)
        },
        "wal": wal_json,
        "projections": {
            "pendingInvalidations": pending_invalidations,
            "projectionLagMs": projection_lag_ms,
            "latencyMs": projection_lag_ms,
            "rebuildDurationP95Ms": crate::metrics::get_projection_rebuild_p95("run-summary"),
            "coalescedKeysPending": 0,
            "coalescedMergedTotal": writer_snapshot.as_ref().map(|snapshot| snapshot.coalesced_merged_total).unwrap_or(0),
            "coalescedFlushAgeP95Ms": null
        },
        "projectionFreshness": projection_freshness,
        "hotReadGuards": hot_read_guards,
        "readPathMetrics": {
            "runsListReadLatencyP95Ms": crate::metrics::get_runs_list_read_latency_p95(),
            "mcpLivenessGateDurationP95Ms": crate::metrics::get_mcp_liveness_gate_duration_p95(),
            "mcpLivenessGateDurationSource": "runtime.health",
            "mcpLivenessGateLastRecordedAtMs": crate::metrics::get_mcp_liveness_gate_last_recorded_at_ms(),
        },
        "readPath": {
            "runsList": {
                "status": if runs_list_p95 > 500 { "breach" } else if runs_list_p95 >= 350 { "warn" } else { "healthy" },
                "sampleCount": crate::metrics::get_hot_read_sample_count("runs.list"),
                "runs_list_read_latency_ms": crate::metrics::get_hot_read_latest("runs_list_read_latency_ms"),
                "p95Ms": runs_list_p95,
            },
            "mcpLivenessGate": {
                "status": if mcp_liveness_p95 > 500 { "breach" } else if mcp_liveness_p95 >= 350 { "warn" } else { "healthy" },
                "sampleCount": crate::metrics::get_hot_read_sample_count("mcp_liveness_gate_duration_ms"),
                "mcp_liveness_gate_duration_ms": crate::metrics::get_hot_read_latest("mcp_liveness_gate_duration_ms"),
                "p95Ms": mcp_liveness_p95,
                "lastRecordedAtMs": crate::metrics::get_mcp_liveness_gate_last_recorded_at_ms(),
            }
        },
        "artifactNoiseProjection": artifact_noise_projection,
        "maintenanceOperations": maintenance_operations,
        "maintenanceReaper": {
            "status": reaper_status,
        },
        "degraded": degraded,
        "hotRead": json!({
            "status": if is_stale { "unavailable" } else { "healthy" },
            "circuitState": circuit_status.as_str(),
            "reason": if is_stale { Some("storage_stale") } else { None }
        }),
        "rollout": rollout,
        "evidenceSpool": evidence,
        "writePressure": pressure_json,
        "telemetryRollup": {
            "memoryCapBytes": TELEMETRY_MEMORY_CAP_BYTES,
            "maxSamples": TELEMETRY_MAX_SAMPLES,
            "flushCadenceMs": TELEMETRY_FLUSH_CADENCE_MS,
            "snapshotTtlHours": TELEMETRY_SNAPSHOT_TTL_HOURS,
            "latestWindowLimit": TELEMETRY_SNAPSHOT_RETAIN_LATEST,
            "units": {
                "memoryCapBytes": "bytes",
                "flushCadenceMs": "milliseconds",
                "snapshotTtlHours": "hours",
                "latestWindowLimit": "count"
            }
        },
        "killSwitches": {
            "dbWriterBypassClasses": [],
            "coalescingDisabledKeys": [],
            "evidenceSpoolDisabledKinds": []
        },
        "thresholds": storage_thresholds(),
        "p087EvaluatedMetrics": evaluate_p087_metrics(pool, writer_snapshot.as_ref()).await
    }))
}

async fn evaluate_p087_metrics(
    _pool: &SqlitePool,
    writer: Option<&DbWriterHealthSnapshot>,
) -> Value {
    let thresholds = storage_thresholds();
    let mut evaluated = Vec::new();

    if let Some(arr) = thresholds.as_array() {
        for t in arr {
            let metric = t["metric"].as_str().unwrap_or_default();
            let warn = t["warn"].as_f64().unwrap_or(0.0);
            let critical = t["critical"].as_f64().unwrap_or(0.0);

            let value = match metric {
                "mcp_hot_read_violation_total" => crate::metrics::get_counter(metric) as f64,
                "storage_maintenance_reaper_sla_breach_total" => {
                    crate::metrics::get_counter(metric) as f64
                }
                "projection_invalidation_coalesce_near_capacity_total" => {
                    crate::metrics::get_counter(metric) as f64
                }
                "write_lock_wait_p95_ms" => crate::pool::write_lock_metrics_snapshot()
                    .wait_p95_ms
                    .unwrap_or(0) as f64,
                "class_a_transaction_duration_p95_ms" => writer
                    .and_then(|w| w.transaction_duration_p95_ms)
                    .unwrap_or(0) as f64,
                "busy_retry_rate_per_minute" => {
                    crate::pool::write_lock_metrics_snapshot().busy_retry_rate_per_minute
                }
                "runs_list_read_latency_ms" => {
                    crate::metrics::get_runs_list_read_latency_p95().unwrap_or(0) as f64
                }
                "projection_lag_ms" => {
                    crate::metrics::get_projection_lag_p95("run-summary").unwrap_or(0) as f64
                }
                "hot_read_circuit_open_total" => crate::metrics::get_counter(metric) as f64,
                "projection_invalidation_backlog_exceeded_total" => {
                    crate::metrics::get_counter(metric) as f64
                }
                _ => 0.0,
            };

            let band = if value >= critical {
                "breach"
            } else if value >= warn {
                "warn"
            } else {
                "healthy"
            };

            evaluated.push(json!({
                "metric": metric,
                "value": value,
                "band": band,
                "thresholds": t
            }));
        }
    }

    json!(evaluated)
}

async fn projection_freshness_summary(pool: &SqlitePool) -> Result<Vec<Value>> {
    let cursors = sqlx::query(
        "SELECT projection_name, source_name, watermark_ms, is_poisoned, last_error, updated_at_ms, throttled_until_ms
         FROM projection_cursors
         ORDER BY updated_at_ms DESC, projection_name ASC, source_name ASC
         LIMIT ?"
    )
    .bind(DIAGNOSTIC_ROW_LIMIT)
    .fetch_all(pool)
    .await?;

    let backlog_stats = sqlx::query(
        "SELECT projection_name, source_name, COUNT(*) as count, SUM(size_bytes) as total_bytes
         FROM projection_invalidation_log
         WHERE is_consumed = 0
         GROUP BY projection_name, source_name",
    )
    .fetch_all(pool)
    .await?;

    let mut stats_map = std::collections::HashMap::new();
    for row in backlog_stats {
        let p_name: String = row.get("projection_name");
        let s_name: String = row.get("source_name");
        let count: i64 = row.get("count");
        let total_bytes: i64 = row.get::<Option<i64>, _>("total_bytes").unwrap_or(0);
        stats_map.insert((p_name, s_name), (count, total_bytes));
    }

    let mut all_identities = std::collections::BTreeSet::new();
    for row in &cursors {
        let p_name: String = row.get("projection_name");
        let s_name: String = row.get("source_name");
        all_identities.insert((p_name, s_name));
    }
    for (p_name, s_name) in stats_map.keys() {
        all_identities.insert((p_name.clone(), s_name.clone()));
    }

    let mut list = Vec::new();
    for (p_name, s_name) in all_identities {
        // Find cursor if it exists
        let cursor_row = cursors.iter().find(|r| {
            let rp: String = r.get("projection_name");
            let rs: String = r.get("source_name");
            rp == p_name && rs == s_name
        });

        let (backlog_rows, backlog_bytes) = stats_map
            .get(&(p_name.clone(), s_name.clone()))
            .cloned()
            .unwrap_or((0, 0));

        if let Some(row) = cursor_row {
            list.push(json!({
                "projectionName": p_name,
                "sourceName": s_name,
                "watermarkMs": row.get::<i64, _>("watermark_ms"),
                "isPoisoned": row.get::<i32, _>("is_poisoned") != 0,
                "lastError": public_error_code(
                    row.get::<Option<String>, _>("last_error").as_deref(),
                    "projection_error"
                ),
                "updatedAtMs": row.get::<i64, _>("updated_at_ms"),
                "throttledUntilMs": row.get::<Option<i64>, _>("throttled_until_ms"),
                "backlogRows": backlog_rows,
                "backlogBytes": backlog_bytes,
            }));
        } else {
            // Backlog exists but no cursor (yet)
            list.push(json!({
                "projectionName": p_name,
                "sourceName": s_name,
                "watermarkMs": 0,
                "isPoisoned": false,
                "lastError": null,
                "updatedAtMs": 0,
                "throttledUntilMs": null,
                "backlogRows": backlog_rows,
                "backlogBytes": backlog_bytes,
            }));
        }
    }
    Ok(list)
}

async fn hot_read_circuit_summary(pool: &SqlitePool) -> Result<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT governed_surface, circuit_status, consecutive_successes, consecutive_failures, last_violation_kind, last_opened_at_ms, retry_after_ms, would_open, updated_at_ms
         FROM hot_read_circuit_states
         ORDER BY updated_at_ms DESC, governed_surface ASC
         LIMIT ?"
    )
    .bind(DIAGNOSTIC_ROW_LIMIT)
    .fetch_all(pool)
    .await?;

    let mut list = Vec::new();
    for row in rows {
        let surface = row.get::<String, _>("governed_surface");
        list.push(json!({
            "governedSurface": surface,
            "circuitStatus": row.get::<String, _>("circuit_status"),
            "consecutiveSuccesses": row.get::<i32, _>("consecutive_successes"),
            "consecutiveFailures": row.get::<i32, _>("consecutive_failures"),
            "lastViolationKind": row.get::<Option<String>, _>("last_violation_kind"),
            "wouldOpen": row.get::<i32, _>("would_open") != 0,
            "lastOpenedAtMs": row.get::<Option<i64>, _>("last_opened_at_ms"),
            "retryAfterMs": row.get::<Option<i64>, _>("retry_after_ms"),
            "updatedAtMs": row.get::<i64, _>("updated_at_ms"),
            "latencyMs": crate::metrics::get_hot_read_p95(&surface),
        }));
    }
    Ok(list)
}

async fn maintenance_operations_summary(pool: &SqlitePool) -> Result<Vec<Value>> {
    let rows = sqlx::query(
        "SELECT id, operation_kind, status, idempotency_key, slot_generation, started_at_ms, completed_at_ms, error, created_at_ms, updated_at_ms
         FROM maintenance_operations
         WHERE operation_kind != 'restart_reaper'
         ORDER BY created_at_ms DESC
         LIMIT 10"
    )
    .fetch_all(pool)
    .await?;

    let mut list = Vec::new();
    for row in rows {
        let raw_id = row.get::<String, _>("id");
        let raw_key = row.get::<String, _>("idempotency_key");
        let raw_error = row.get::<Option<String>, _>("error");
        list.push(json!({
            "id": public_reference("maintenance_operation", &raw_id),
            "operationKind": row.get::<String, _>("operation_kind"),
            "status": row.get::<String, _>("status"),
            "idempotencyKey": public_hash(&raw_key),
            "slotGeneration": row.get::<i64, _>("slot_generation"),
            "startedAtMs": row.get::<Option<i64>, _>("started_at_ms"),
            "completedAtMs": row.get::<Option<i64>, _>("completed_at_ms"),
            "error": public_error_code(raw_error.as_deref(), "maintenance_operation_failed"),
            "detailsRedacted": !raw_key.is_empty() || raw_error.is_some(),
            "createdAtMs": row.get::<i64, _>("created_at_ms"),
            "updatedAtMs": row.get::<i64, _>("updated_at_ms"),
        }));
    }
    Ok(list)
}

async fn artifact_noise_projection(pool: &SqlitePool) -> Result<Value> {
    let totals = sqlx::query(
        r#"SELECT COUNT(*) as total_run_count,
                  COALESCE(SUM(artifact_count), 0) as artifact_count,
                  COALESCE(SUM(superseded_count), 0) as superseded_count,
                  COALESCE(SUM(duplicate_candidate_count), 0) as duplicate_candidate_count,
                  COALESCE(SUM(archive_eligible_count), 0) as archive_eligible_count
           FROM artifact_noise_summary"#,
    )
    .fetch_one(pool)
    .await?;

    let rows = sqlx::query(
        r#"SELECT run_id, artifact_count, superseded_count, duplicate_candidate_count, archive_eligible_count
           FROM artifact_noise_summary
           ORDER BY run_id
           LIMIT ?"#,
    )
    .bind(DIAGNOSTIC_ROW_LIMIT)
    .fetch_all(pool)
    .await?;

    let total_run_count = totals.get::<i64, _>("total_run_count");
    let total_artifact_count = totals.get::<i64, _>("artifact_count");
    let total_superseded_count = totals.get::<i64, _>("superseded_count");
    let total_duplicate_candidate_count = totals.get::<i64, _>("duplicate_candidate_count");
    let total_archive_eligible_count = totals.get::<i64, _>("archive_eligible_count");
    let mut runs = Vec::new();
    for row in rows {
        let artifact_count = row.get::<i64, _>("artifact_count");
        let superseded_count = row.get::<i64, _>("superseded_count");
        let duplicate_candidate_count = row.get::<i64, _>("duplicate_candidate_count");
        let archive_eligible_count = row.get::<i64, _>("archive_eligible_count");
        runs.push(json!({
            "runId": row.get::<String, _>("run_id"),
            "artifactCount": artifact_count,
            "supersededCount": superseded_count,
            "duplicateCandidateCount": duplicate_candidate_count,
            "archiveEligibleCount": archive_eligible_count,
            "compactionRecommended": superseded_count > 0 || duplicate_candidate_count > 0 || artifact_count > 500,
        }));
    }

    Ok(json!({
        "schemaVersion": "artifact_noise_projection.v1",
        "artifactCount": total_artifact_count,
        "supersededCount": total_superseded_count,
        "duplicateCandidateCount": total_duplicate_candidate_count,
        "archiveEligibleCount": total_archive_eligible_count,
        "compactionRecommended": total_superseded_count > 0
            || total_duplicate_candidate_count > 0
            || total_artifact_count > 500,
        "totalRunCount": total_run_count,
        "returnedRunCount": runs.len(),
        "truncated": total_run_count > runs.len() as i64,
        "runs": runs,
    }))
}

fn public_reference(prefix: &str, raw: &str) -> String {
    format!("{prefix}:{}", short_sha256(raw))
}

fn public_hash(raw: &str) -> String {
    format!("sha256:{}", short_sha256(raw))
}

fn short_sha256(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    format!("{digest:x}").chars().take(16).collect()
}

fn public_error_code(raw: Option<&str>, fallback: &str) -> Option<String> {
    raw.map(|value| {
        let value = value.trim();
        if is_public_error_code(value) {
            value.to_string()
        } else {
            fallback.to_string()
        }
    })
}

fn is_public_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '-' | ':' | '.')
        })
}

pub async fn runtime_health_projection(pool: &SqlitePool) -> Result<Value> {
    let row = sqlx::query(
        r#"SELECT active_sessions, open_hot_read_circuits, side_effect_unresolved_count,
                  continuation_active_count, runtime_families_json
           FROM runtime_health_summary
           WHERE id = 1"#,
    )
    .fetch_one(pool)
    .await?;

    let runtime_families_json: String = row.get("runtime_families_json");
    let runtime_families: Value = serde_json::from_str(&runtime_families_json)?;

    Ok(json!({
        "schemaVersion": "runtime_health_projection.v1",
        "runtimeFamilies": runtime_families,
        "activeSessions": row.get::<i64, _>("active_sessions"),
        "degradedFlags": {
            "hotReadCircuitOpen": row.get::<i64, _>("open_hot_read_circuits") > 0
        },
        "writePressureFlags": {
            "writerHeartbeatRequiredForStorageHealth": true
        },
        "sideEffectUnresolvedCount": row.get::<i64, _>("side_effect_unresolved_count"),
        "continuationActiveCount": row.get::<i64, _>("continuation_active_count")
    }))
}

async fn rollout_readback(pool: &SqlitePool) -> Result<Value> {
    let mode = crate::hot_read_guard::LivenessMode::current();
    let mode_raw = std::env::var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE").ok();
    let invalid_mode = matches!(
        mode_raw.as_deref(),
        Some(value) if !matches!(value, "observe" | "enforce" | "disabled")
    );
    let mode_str = match mode {
        crate::hot_read_guard::LivenessMode::Enforce => "enforce",
        crate::hot_read_guard::LivenessMode::Disabled => "disabled",
        _ => "observe",
    };

    let (m_count, m_age) = maintenance_active_stats(pool).await.unwrap_or((0, None));
    let last_reaper = crate::repos::maintenance::last_reaper_run(pool)
        .await
        .unwrap_or(None);
    let promotion_budget = compute_promotion_budget(pool).await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "p087 compute_promotion_budget db error; falling back to pending");
        PromotionBudget::default()
    });

    let now_ms = Utc::now().timestamp_millis();
    let reaper_sla_breach = if mode == crate::hot_read_guard::LivenessMode::Enforce {
        let breach = last_reaper.map_or(true, |ts| now_ms - ts > 60_000);
        if breach {
            crate::metrics::increment_counter("storage_maintenance_reaper_sla_breach_total");
        }
        breach
    } else {
        false
    };

    let poisoned_hold = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM maintenance_operations
         WHERE operation_kind = 'repair_slot_poisoned' AND status = 'failed' AND created_at_ms < ?",
    )
    .bind(now_ms - 5 * 60 * 1000)
    .fetch_one(pool)
    .await
    .unwrap_or(0)
        > 0;

    let status = if reaper_sla_breach || poisoned_hold {
        "degraded"
    } else {
        "active"
    };

    // Check for poisoned projection cursors: projection freshness is not healthy.
    let projection_freshness_degraded = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM projection_cursors WHERE is_poisoned = 1",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0)
        > 0;

    // Check for sustained projection invalidation backlog exceeded events.
    let backlog_exceeded =
        crate::metrics::get_counter("projection_invalidation_backlog_exceeded_total") > 0;

    // Collect all hold conditions; every criterion must pass before the contract is ready.
    let mut hold_conditions: Vec<&str> = Vec::new();
    let mut failure_reasons: Vec<&str> = Vec::new();

    if status != "active" {
        hold_conditions.push(status);
        failure_reasons.push(status);
    }
    if invalid_mode {
        hold_conditions.push("invalid_liveness_mode_config");
        failure_reasons.push("invalid_liveness_mode_config");
    }
    if !promotion_budget.promotion_budget_met {
        hold_conditions.push("p087_hot_read_promotion_budget_pending");
        failure_reasons.push("p087_hot_read_promotion_budget_pending");
    }
    if projection_freshness_degraded {
        hold_conditions.push("p087_projection_freshness_degraded");
        failure_reasons.push("p087_projection_freshness_degraded");
    }
    if backlog_exceeded {
        hold_conditions.push("p087_projection_invalidation_backlog_exceeded");
        failure_reasons.push("p087_projection_invalidation_backlog_exceeded");
    }

    let is_ready = hold_conditions.is_empty();

    Ok(json!({
        "p087_storage_tiering_status": status,
        "p087_mcp_liveness_status": status,
        "p087_runs_list_projection_only_status": status,
        "p087_projection_rebuild_status": status,
        "p087_hot_read_enforcement_status": mode_str,
        "p087_storage_exit_threshold_status": status,
        "p087_mcp_wire_compatibility_status": "active",
        "p087_graphql_storage_health_compatibility_status": "active",
        "p087_per_tool_circuit_state": "active",
        "p087_per_projection_freshness": "active",
        "p087_maintenance_active_count": m_count,
        "p087_maintenance_status_age_ms": m_age,
        "p087_restart_reaper_last_run": last_reaper,
        "p087_projection_invalidation_backlog_status": if backlog_exceeded { "exceeded" } else { status },
        "p087_liveness_mode_config_status": if invalid_mode { "invalid_observe_fallback" } else { "active" },
        "p087_liveness_mode_configured_value": mode_raw,
        "p087_would_open_rate": promotion_budget.worst_would_open_rate,
        "p087_total_requests_min": promotion_budget.min_total_requests,
        "p087_flap_free_hours_min": promotion_budget.min_flap_free_hours,
        "p087_promotion_budget_met": promotion_budget.promotion_budget_met,
        "p087_per_surface_promotion_budget": promotion_budget.per_surface,
        "rollout_contract_status": if is_ready { "pass" } else { "hold" },
        "rollout_contract_decision": if is_ready { "ready" } else { "hold" },
        "rollout_contract_failure_reasons": json!(failure_reasons),
        "rollout_contract_waiver_state": "none",
        "rollout_contract_waiver_expires_at": Value::Null,
        "rollout_contract_enforcement_mode": mode_str,
        "rollout_contract_enforcement_mode_reason": if invalid_mode { "invalid_liveness_mode_observe_fallback" } else if promotion_budget.promotion_budget_met { "p087_hot_read_promotion_budget_met" } else { "p087_hot_read_promotion_budget_pending" },
        "rollout_contract_hold_conditions": json!(hold_conditions),
        "rollout_contract_rollback_disposition": {
            "status": "not_required",
            "data_loss_risk": "none"
        },
        "rollout_contract_source_lane": "storage_health",
        "rollout_contract_enabled_state": if mode_str == "disabled" { "disabled" } else { "enabled" },
        "rollout_contract_disabled_reason_code": if mode_str == "disabled" { json!("operator_disabled") } else { Value::Null },
        "rollout_contract_action_id": "p087-enforce-cutover",
        "rollout_contract_operator_message": if is_ready {
            "P087 storage tiering read path is ready for enforce mode."
        } else {
            "P087 storage tiering read path has rollout holds."
        },
        "rollout_contract_projection_integrity": if is_ready { "pass" } else { "hold" },
        "rollout_contract_cutover_policy_revision": "p087-r6-2026-05-10-graphql-projection-compatibility",
        "rollout_contract_diagnostic_redaction": "none",
        "rollout_contract_next_steps": if is_ready { json!([]) } else { json!(["inspect_p087_rollout_holds"]) }
    }))
}

/// Summarized per-surface promotion budget for observe-to-enforce promotion.
#[derive(Default)]
struct PromotionBudget {
    worst_would_open_rate: f64,
    min_total_requests: i64,
    min_flap_free_hours: f64,
    promotion_budget_met: bool,
    per_surface: serde_json::Value,
}

/// The canonical set of hot-read governed surfaces that must all meet the
/// observe-to-enforce promotion criteria. Adding or removing surfaces here
/// changes the promotion requirements and must be coordinated with a proposal.
const CANONICAL_HOT_READ_SURFACES: &[&str] = &[
    "initialize",
    "runs.list",
    "tools.list",
    "runtime.health",
    "storage.health",
    "artifacts.metadata.get",
];

/// Query all governed surfaces and evaluate the observe-to-enforce promotion criteria:
/// - wouldOpen rate below 0.1% per surface
/// - at least 100 requests per surface
/// - 48-hour flap-free window per surface
///
/// All CANONICAL_HOT_READ_SURFACES are always evaluated. Surfaces with no DB row
/// default to total_requests=0, which means they can never satisfy the 100-request
/// floor, so promotion_budget_met remains false until every surface has sufficient traffic.
async fn compute_promotion_budget(pool: &SqlitePool) -> Result<PromotionBudget> {
    let rows = sqlx::query(
        "SELECT governed_surface, total_requests, total_would_open, last_state_change_at_ms, first_observed_at_ms
         FROM hot_read_circuit_states",
    )
    .fetch_all(pool)
    .await?;

    // Build a lookup from DB rows; surfaces absent from the DB are treated as
    // (0 requests, 0 would_open, no state change, never observed) — i.e. not yet ready.
    let mut row_map: std::collections::HashMap<String, (i64, i64, Option<i64>, Option<i64>)> =
        std::collections::HashMap::new();
    for row in &rows {
        let surface: String = row.get("governed_surface");
        let total_requests: i64 = row.get("total_requests");
        let total_would_open: i64 = row.get("total_would_open");
        let last_state_change: Option<i64> = row.get("last_state_change_at_ms");
        let first_observed: Option<i64> = row.get("first_observed_at_ms");
        row_map.insert(
            surface,
            (
                total_requests,
                total_would_open,
                last_state_change,
                first_observed,
            ),
        );
    }

    let now_ms = Utc::now().timestamp_millis();
    let required_ms: i64 = 48 * 60 * 60 * 1000;
    let mut per_surface = Vec::new();
    let mut worst_rate: f64 = 0.0;
    let mut min_requests: i64 = i64::MAX;
    let mut min_flap_free_ms: i64 = 0;

    for &surface in CANONICAL_HOT_READ_SURFACES {
        let (total_requests, total_would_open, last_state_change, first_observed) =
            row_map.get(surface).copied().unwrap_or((0, 0, None, None));

        let rate = if total_requests > 0 {
            (total_would_open as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        // The observation window starts at first_observed_at_ms (when this surface was first
        // ever seen), unless the circuit later changed state (open/close recovery), in which
        // case stability is only guaranteed from the last state change.
        // If the surface has never been observed, it cannot satisfy the 48-hour requirement.
        let effective_window_start_ms: Option<i64> = match (first_observed, last_state_change) {
            (None, _) => None,                  // never observed — not ready
            (Some(first), None) => Some(first), // observed but never had a state change
            (Some(first), Some(last_change)) => Some(last_change.max(first)),
        };

        let flap_free_ms = match effective_window_start_ms {
            Some(t) => (now_ms - t).max(0),
            None => 0, // never observed — treat as 0 ms flap-free
        };
        let flap_free_hours = flap_free_ms as f64 / 3_600_000.0;

        let surface_met = rate < 0.1
            && total_requests >= 100
            && effective_window_start_ms.is_some()
            && flap_free_ms >= required_ms;

        per_surface.push(serde_json::json!({
            "governed_surface": surface,
            "total_requests": total_requests,
            "total_would_open": total_would_open,
            "would_open_rate_pct": (rate * 1000.0).round() / 1000.0,
            "flap_free_hours": (flap_free_hours * 10.0).round() / 10.0,
            "promotion_ready": surface_met,
        }));

        if rate > worst_rate {
            worst_rate = rate;
        }
        if total_requests < min_requests {
            min_requests = total_requests;
        }
        if flap_free_ms < min_flap_free_ms || per_surface.len() == 1 {
            min_flap_free_ms = flap_free_ms;
        }
    }

    let all_met = per_surface
        .iter()
        .all(|s| s["promotion_ready"].as_bool().unwrap_or(false));
    let final_flap_free = (min_flap_free_ms as f64 / 3_600_000.0 * 10.0).round() / 10.0;

    Ok(PromotionBudget {
        worst_would_open_rate: (worst_rate * 1000.0).round() / 1000.0,
        min_total_requests: if min_requests == i64::MAX {
            0
        } else {
            min_requests
        },
        min_flap_free_hours: final_flap_free,
        promotion_budget_met: all_met,
        per_surface: serde_json::json!(per_surface),
    })
}

/// Returns the live P087 rollout readback fields for merging into production
/// readback lanes (run_report, release_receipt).
pub async fn p087_rollout_readback_fields(pool: &SqlitePool) -> Value {
    rollout_readback(pool).await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "p087 rollout_readback db error; returning empty readback");
        json!({})
    })
}

async fn maintenance_active_stats(pool: &SqlitePool) -> Result<(i64, Option<i64>)> {
    let row = sqlx::query(
        "SELECT COUNT(*) as count, MIN(updated_at_ms) as oldest
         FROM maintenance_operations
         WHERE status = 'running'",
    )
    .fetch_one(pool)
    .await?;

    let count: i64 = row.get("count");
    let oldest: Option<i64> = row.get("oldest");
    let now = Utc::now().timestamp_millis();
    let age = oldest.map(|o| (now - o).max(0));
    Ok((count, age))
}

async fn projection_invalidation_stats(pool: &SqlitePool) -> Result<(i64, Option<i64>)> {
    let row = sqlx::query(
        "SELECT COUNT(*) as count, MIN(created_at_ms) as oldest
         FROM projection_invalidation_log
         WHERE is_consumed = 0",
    )
    .fetch_one(pool)
    .await?;

    let count: i64 = row.get("count");
    let oldest: Option<i64> = row.get("oldest");
    let lag = oldest.map(|o| (Utc::now().timestamp_millis() - o).max(0));
    if let Some(l) = lag {
        crate::metrics::record_projection_lag("global", std::time::Duration::from_millis(l as u64));
    }
    Ok((count, lag))
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

async fn wal_health(pool: &SqlitePool) -> Value {
    let unavailable = |reason: &str| {
        json!({
            "available": false,
            "unavailableReason": reason,
            "sizeBytes": null,
            "warnSizeBytes": WARN_WAL_SIZE_BYTES,
            "criticalSizeBytes": CRITICAL_WAL_SIZE_BYTES,
            "lastCheckpointAt": null,
            "checkpointDurationP95Ms": null
        })
    };

    let rows = match sqlx::query("PRAGMA database_list").fetch_all(pool).await {
        Ok(rows) => rows,
        Err(_) => return unavailable("database_list_probe_failed"),
    };
    let Some(main_path) = rows.into_iter().find_map(|row| {
        let name: String = row.try_get("name").ok()?;
        if name != "main" {
            return None;
        }
        let file: String = row.try_get("file").ok()?;
        (!file.is_empty()).then_some(file)
    }) else {
        return unavailable("memory_or_ephemeral_database");
    };

    let wal_path = format!("{main_path}-wal");
    let size_bytes = match tokio::fs::metadata(&wal_path).await {
        Ok(metadata) => metadata.len() as i64,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(_) => return unavailable("wal_size_probe_failed"),
    };

    json!({
        "available": true,
        "unavailableReason": null,
        "sizeBytes": size_bytes,
        "warnSizeBytes": WARN_WAL_SIZE_BYTES,
        "criticalSizeBytes": CRITICAL_WAL_SIZE_BYTES,
        "lastCheckpointAt": null,
        "checkpointDurationP95Ms": null
    })
}

fn live_write_pressure_json(updated_at: DateTime<Utc>, snapshot: &DbWriterHealthSnapshot) -> Value {
    let lanes = snapshot
        .lanes
        .iter()
        .map(|lane| {
            (
                lane.lane.as_str().to_string(),
                json!({
                    "queueDepth": lane.queued_depth,
                    "capacity": lane.capacity,
                    "queuedDepthRatio": if lane.capacity == 0 {
                        0.0
                    } else {
                        lane.queued_depth as f64 / lane.capacity as f64
                    },
                    "oldestQueuedMs": lane.oldest_queued_age_ms,
                }),
            )
        })
        .collect::<serde_json::Map<String, Value>>();
    json!({
        "state": "available",
        "latestSnapshot": {
            "id": "live-dbwriter-heartbeat",
            "windowStart": updated_at.to_rfc3339(),
            "windowEnd": updated_at.to_rfc3339(),
            "createdAt": updated_at.to_rfc3339(),
            "payload": {
                "source": "dbwriter_heartbeat",
                "writerAlive": snapshot.alive,
                "totalQueued": snapshot.total_queued,
                "lastHeartbeatAgeMs": snapshot.last_heartbeat_age_ms,
                "lastDrainAgeMs": snapshot.last_drain_age_ms,
                "coalescedRejectedTotal": snapshot.coalesced_rejected_total,
                "coalescedMergedTotal": snapshot.coalesced_merged_total,
                "telemetryDroppedTotal": snapshot.telemetry_dropped_total,
                "starvationTotal": snapshot.starvation_total,
                "lanes": lanes
            }
        },
        "freshness": {
            "state": if snapshot.alive { "fresh" } else { "stale" },
            "observedAt": updated_at.to_rfc3339(),
            "ageMs": 0,
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

fn live_rollup_payload(snapshot: &DbWriterHealthSnapshot) -> Result<Value> {
    let lanes = snapshot
        .lanes
        .iter()
        .map(|lane| {
            json!({
                "lane": lane.lane.as_str(),
                "queuedDepth": lane.queued_depth,
                "capacity": lane.capacity,
                "oldestQueuedAgeMs": lane.oldest_queued_age_ms,
            })
        })
        .collect::<Vec<_>>();
    let sample_count =
        (lanes.len() + snapshot.transaction_duration_sample_count).min(TELEMETRY_MAX_SAMPLES);
    let mut payload = json!({
        "source": "dbwriter_telemetry_rollup",
        "writerAlive": snapshot.alive,
        "limits": {
            "memoryCapBytes": TELEMETRY_MEMORY_CAP_BYTES,
            "maxSamples": TELEMETRY_MAX_SAMPLES,
            "payloadCapBytes": 65_536,
        },
        "retention": {
            "ttlHours": TELEMETRY_SNAPSHOT_TTL_HOURS,
            "latestWindowLimit": TELEMETRY_SNAPSHOT_RETAIN_LATEST,
        },
        "rollup": {
            "sampleCount": sample_count,
            "sampleCapEnforced": sample_count <= TELEMETRY_MAX_SAMPLES,
            "totalQueued": snapshot.total_queued,
            "coalescedRejectedTotal": snapshot.coalesced_rejected_total,
            "coalescedMergedTotal": snapshot.coalesced_merged_total,
            "telemetryDroppedTotal": snapshot.telemetry_dropped_total,
            "starvationTotal": snapshot.starvation_total,
            "transactionDurationP50Ms": snapshot.transaction_duration_p50_ms,
            "transactionDurationP95Ms": snapshot.transaction_duration_p95_ms,
            "transactionDurationSampleCount": snapshot.transaction_duration_sample_count,
            "lanes": lanes,
        }
    });
    let bytes = serde_json::to_vec(&payload)?.len();
    if bytes > TELEMETRY_MEMORY_CAP_BYTES || bytes > 65_536 {
        payload["rollup"] = json!({
            "sampleCount": 0,
            "sampleCapEnforced": true,
            "payloadCompacted": true,
            "totalQueued": snapshot.total_queued,
            "telemetryDroppedTotal": snapshot.telemetry_dropped_total,
        });
    } else {
        payload["rollup"]["estimatedMemoryBytes"] = json!(bytes);
    }
    Ok(payload)
}

fn missing_write_pressure_json() -> Value {
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
}

fn lane_health(writer_snapshot: Option<&DbWriterHealthSnapshot>) -> Value {
    if let Some(snapshot) = writer_snapshot {
        return Value::Array(
            snapshot
                .lanes
                .iter()
                .map(|lane| {
                    let ratio = if lane.capacity == 0 {
                        0.0
                    } else {
                        lane.queued_depth as f64 / lane.capacity as f64
                    };
                    json!({
                        "lane": lane.lane.as_str(),
                        "capacity": lane.capacity as i64,
                        "queuedDepth": lane.queued_depth,
                        "queuedDepthRatio": ratio,
                        "oldestQueuedAgeMs": lane.oldest_queued_age_ms,
                        "rejectedTotal": if lane.lane.as_str() == "coalesced_projection" {
                            snapshot.coalesced_rejected_total
                        } else {
                            0
                        },
                        "droppedTotal": if lane.lane.as_str() == "telemetry_rollup" {
                            snapshot.telemetry_dropped_total
                        } else {
                            0
                        }
                    })
                })
                .collect(),
        );
    }
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
        {"metric": "evidence_orphan_bytes", "warn": 10_485_760.0, "critical": 104_857_600.0, "unit": "bytes", "action": "run_storage_reconcile_evidence_orphans"},
        {"metric": "mcp_hot_read_violation_total", "warn": 1.0, "critical": 10.0, "unit": "count", "action": "inspect_hot_read_latencies"},
        {"metric": "storage_maintenance_reaper_sla_breach_total", "warn": 1.0, "critical": 5.0, "unit": "count", "action": "check_reaper_loop"},
        {"metric": "projection_invalidation_coalesce_near_capacity_total", "warn": 1.0, "critical": 100.0, "unit": "count", "action": "increase_projection_concurrency"},
        {"metric": "runs_list_read_latency_ms", "warn": 350.0, "critical": 500.0, "unit": "milliseconds", "action": "check_sqlite_contention"},
        {"metric": "projection_lag_ms", "warn": 15000.0, "critical": 30000.0, "unit": "milliseconds", "action": "check_projection_writer"},
        {"metric": "hot_read_circuit_open_total", "warn": 1.0, "critical": 1.0, "unit": "count", "action": "check_storage_stability"},
        {"metric": "projection_invalidation_backlog_exceeded_total", "warn": 1.0, "critical": 1.0, "unit": "count", "action": "check_projection_backlog"}
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

#[cfg(test)]
pub(crate) static P087_STORAGE_HEALTH_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn storage_health_uses_live_dbwriter_heartbeat_when_supplied() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let writer = crate::writer::DbWriter::new(pool.clone());
        let result = writer
            .submit(
                crate::write_class::WriteOperation {
                    class: crate::write_class::WriteClass::A,
                    lane: crate::write_class::WriteLane::CriticalBarrier,
                    operation_name: "storage_health_live_writer_test",
                    expected_rows: 1,
                    batchable: false,
                    barrier: true,
                    deadline: std::time::Duration::from_secs(2),
                    deadline_reason: None,
                    idempotency_key: "storage-health-live-writer-test".to_string(),
                    replay_policy: crate::write_class::ReplayPolicy::NaturalKey,
                    observed_at: None,
                },
                |pool| async move {
                    let mut tx = crate::pool::begin_immediate_with_retry(
                        &pool,
                        "storage_health_live_writer_test",
                    )
                    .await?;
                    sqlx::query("CREATE TABLE IF NOT EXISTS p075_storage_health_probe (id TEXT)")
                        .execute(&mut *tx)
                        .await?;
                    sqlx::query("INSERT INTO p075_storage_health_probe (id) VALUES ('probe')")
                        .execute(&mut *tx)
                        .await?;
                    tx.commit().await?;
                    Ok(1)
                },
            )
            .await;
        assert_eq!(result, crate::write_class::WriteResult::Committed);

        for _ in 0..30 {
            if writer.is_alive() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(writer.is_alive(), "DbWriter heartbeat should become live");

        let health = storage_health_with_writer(&pool, Some(&writer.heartbeat))
            .await
            .unwrap();
        assert_eq!(health["writer"]["alive"], true);
        assert!(health["writer"]["lastHeartbeatAt"].as_str().is_some());
        assert!(health["writer"]["lastDrainAt"].as_str().is_some());
        assert_eq!(health["writer"]["totalQueued"], 0);
        assert!(health["writer"]["writeLockWaitP50Ms"].as_u64().is_some());
        assert!(health["writer"]["writeLockWaitP95Ms"].as_u64().is_some());
        assert!(health["writer"]["transactionDurationP50Ms"]
            .as_u64()
            .is_some());
        assert!(health["writer"]["transactionDurationP95Ms"]
            .as_u64()
            .is_some());
        assert!(health["writer"]["lanes"]
            .as_array()
            .is_some_and(|lanes| lanes.len() == 6));
        assert_eq!(health["isStale"], false);
        assert_eq!(health["writePressure"]["state"], "available");
        assert_eq!(
            health["writePressure"]["latestSnapshot"]["payload"]["source"],
            "dbwriter_heartbeat"
        );
        assert_eq!(
            health["writePressure"]["latestSnapshot"]["payload"]["writerAlive"],
            true
        );
    }

    #[tokio::test]
    async fn storage_health_file_backed_canary_reports_lock_wal_and_writer_metrics() {
        let dir = tempfile::TempDir::new().unwrap();
        let db_path = dir.path().join("p075-storage-health-canary.sqlite");
        let pool = crate::pool::create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
            .await
            .unwrap();
        let writer = crate::writer::DbWriter::new(pool.clone());

        for idx in 0..8 {
            let result = writer
                .submit(
                    crate::write_class::WriteOperation {
                        class: crate::write_class::WriteClass::A,
                        lane: crate::write_class::WriteLane::CriticalBarrier,
                        operation_name: "storage_health_file_backed_canary",
                        expected_rows: 1,
                        batchable: false,
                        barrier: true,
                        deadline: std::time::Duration::from_secs(2),
                        deadline_reason: None,
                        idempotency_key: format!("storage-health-file-backed-canary-{idx}"),
                        replay_policy: crate::write_class::ReplayPolicy::NaturalKey,
                        observed_at: None,
                    },
                    move |pool| async move {
                        let mut tx = crate::pool::begin_immediate_with_retry(
                            &pool,
                            "storage_health_file_backed_canary",
                        )
                        .await?;
                        sqlx::query(
                            "CREATE TABLE IF NOT EXISTS p075_storage_health_canary (id INTEGER PRIMARY KEY, value TEXT)",
                        )
                        .execute(&mut *tx)
                        .await?;
                        sqlx::query(
                            "INSERT OR REPLACE INTO p075_storage_health_canary (id, value) VALUES (?1, ?2)",
                        )
                        .bind(idx as i64)
                        .bind(format!("value-{idx}"))
                        .execute(&mut *tx)
                        .await?;
                        tx.commit().await?;
                        Ok(1)
                    },
                )
                .await;
            assert_eq!(result, crate::write_class::WriteResult::Committed);
        }

        let health = storage_health_with_writer(&pool, Some(&writer.heartbeat))
            .await
            .unwrap();

        assert_eq!(health["writer"]["alive"], true);
        assert!(health["writer"]["lastHeartbeatAt"].as_str().is_some());
        assert!(health["writer"]["lastDrainAt"].as_str().is_some());
        assert!(health["writer"]["writeLockWaitP50Ms"].as_u64().is_some());
        assert!(health["writer"]["writeLockWaitP95Ms"].as_u64().is_some());
        assert!(health["writer"]["transactionDurationP50Ms"]
            .as_u64()
            .is_some());
        assert!(health["writer"]["transactionDurationP95Ms"]
            .as_u64()
            .is_some());
        assert_eq!(health["wal"]["available"], true);
        assert!(health["wal"]["sizeBytes"].as_u64().is_some());
        assert_eq!(health["writePressure"]["state"], "available");
    }

    #[tokio::test]
    async fn storage_health_fails_closed_without_live_dbwriter_heartbeat() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let health = storage_health(&pool).await.unwrap();

        assert_eq!(health["writer"]["alive"], false);
        assert_eq!(health["isStale"], true);
    }

    #[tokio::test]
    async fn proposal_087_storage_health_reports_poisoned_projection_throttle() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let now = Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO projection_cursors \
             (projection_name, source_name, watermark_ms, is_poisoned, last_error, updated_at_ms, throttled_until_ms) \
             VALUES ('runs_home', 'runs', ?, 1, 'projection_invalidation_backlog_exceeded', ?, ?)",
        )
        .bind(now - 10_000)
        .bind(now)
        .bind(now + 60_000)
        .execute(&pool)
        .await
        .unwrap();

        let health = storage_health(&pool).await.unwrap();
        let freshness = health["projectionFreshness"].as_array().unwrap();
        let row = freshness
            .iter()
            .find(|row| row["projectionName"] == "runs_home" && row["sourceName"] == "runs")
            .expect("projection freshness row must be present");
        assert_eq!(row["isPoisoned"], true);
        assert_eq!(row["lastError"], "projection_invalidation_backlog_exceeded");
        assert!(row["throttledUntilMs"].as_i64().is_some());
    }

    #[tokio::test]
    async fn proposal_087_storage_health_v1_legacy_field_names_correct() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let health = storage_health(&pool).await.unwrap();

        let writer = &health["writer"];
        assert!(
            writer.get("busyRetryExhaustedTotal").is_some(),
            "busyRetryExhaustedTotal must be present"
        );
        assert!(
            writer.get("busyRetryEhaustedTotal").is_none(),
            "typo busyRetryEhaustedTotal must NOT be present"
        );
    }

    #[tokio::test]
    async fn proposal_087_storage_health_redacts_maintenance_operation_details() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let now = Utc::now().timestamp_millis();

        sqlx::query(
            "INSERT INTO maintenance_operations \
             (id, operation_kind, status, idempotency_key, error, created_at_ms, updated_at_ms) \
             VALUES ('raw-maintenance-operation-id', 'repair_slot', 'failed', \
                     'operator-provided-idempotency-key', '/Users/user/private/db.sqlite: boom', ?, ?)",
        )
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let health = storage_health(&pool).await.unwrap();
        let operations = health["maintenanceOperations"].as_array().unwrap();
        let operation = operations
            .iter()
            .find(|row| row["operationKind"] == "repair_slot")
            .expect("repair operation should be returned");

        let public_id = operation["id"].as_str().unwrap();
        assert_ne!(public_id, "raw-maintenance-operation-id");
        assert!(
            public_id.starts_with("maintenance_operation:"),
            "operation id must be an opaque public reference"
        );

        let public_key = operation["idempotencyKey"].as_str().unwrap();
        assert_ne!(public_key, "operator-provided-idempotency-key");
        assert!(
            public_key.starts_with("sha256:"),
            "idempotency key must be hash-redacted"
        );
        assert_eq!(operation["error"], "maintenance_operation_failed");
        assert_eq!(operation["detailsRedacted"], true);
    }

    #[tokio::test]
    async fn proposal_087_storage_health_redacts_projection_cursor_errors() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let now = Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO projection_cursors \
             (projection_name, source_name, watermark_ms, is_poisoned, last_error, updated_at_ms, throttled_until_ms) \
             VALUES ('runs_home', 'runs', ?, 1, '/Users/user/private/projection.log: stack', ?, NULL)",
        )
        .bind(now - 10_000)
        .bind(now)
        .execute(&pool)
        .await
        .unwrap();

        let health = storage_health(&pool).await.unwrap();
        let freshness = health["projectionFreshness"].as_array().unwrap();
        let row = freshness
            .iter()
            .find(|row| row["projectionName"] == "runs_home" && row["sourceName"] == "runs")
            .expect("projection freshness row must be present");
        assert_eq!(row["lastError"], "projection_error");
    }

    #[tokio::test]
    async fn proposal_087_artifact_noise_projection_is_bounded() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let now = Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO ideas (id, title, body, status, created_at) \
             VALUES ('idea-artifact-noise', 'Artifact noise', 'body', 'draft', '2026-05-18T00:00:00Z')",
        )
        .execute(&pool)
        .await
        .unwrap();
        for idx in 0..125 {
            let run_id = format!("run-{idx:03}");
            sqlx::query(
                "INSERT INTO runs \
                 (id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at) \
                 VALUES (?, 'idea-artifact-noise', 'running', 'wf', 'Workflow', '/tmp', '/tmp/artifacts', '2026-05-18T00:00:00Z')",
            )
            .bind(&run_id)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO artifact_noise_summary \
                 (run_id, artifact_count, superseded_count, duplicate_candidate_count, archive_eligible_count, updated_at_ms) \
                 VALUES (?, 1, 0, 0, 0, ?)",
            )
            .bind(run_id)
            .bind(now)
            .execute(&pool)
            .await
            .unwrap();
        }

        let health = storage_health(&pool).await.unwrap();
        let projection = &health["artifactNoiseProjection"];
        assert_eq!(projection["totalRunCount"], 125);
        assert_eq!(projection["returnedRunCount"], 100);
        assert_eq!(projection["truncated"], true);
        assert_eq!(projection["runs"].as_array().unwrap().len(), 100);
    }

    #[tokio::test]
    async fn proposal_087_storage_health_enforces_reaper_sla_in_enforce_mode() {
        let _guard = P087_STORAGE_HEALTH_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let now = Utc::now().timestamp_millis();

        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "enforce",
        );

        // 1. No reaper run yet -> degraded
        let health = storage_health(&pool).await.unwrap();
        let rollout = &health["rollout"];
        assert_eq!(rollout["p087_storage_tiering_status"], "degraded");

        // 2. Old reaper run (70s ago) -> degraded
        sqlx::query("INSERT INTO maintenance_operations (id, operation_kind, status, idempotency_key, created_at_ms, updated_at_ms) VALUES ('r1', 'restart_reaper', 'completed', 'i1', ?, ?)")
            .bind(now - 70_000)
            .bind(now - 70_000)
            .execute(&pool).await.unwrap();

        let health = storage_health(&pool).await.unwrap();
        let rollout = &health["rollout"];
        assert_eq!(rollout["p087_storage_tiering_status"], "degraded");

        // 3. Fresh reaper run (10s ago) -> active
        sqlx::query("INSERT INTO maintenance_operations (id, operation_kind, status, idempotency_key, created_at_ms, updated_at_ms) VALUES ('r2', 'restart_reaper', 'completed', 'i2', ?, ?)")
            .bind(now - 10_000)
            .bind(now - 10_000)
            .execute(&pool).await.unwrap();

        let health = storage_health(&pool).await.unwrap();
        let rollout = &health["rollout"];
        assert_eq!(rollout["p087_storage_tiering_status"], "active");

        // 4. Fresh reaper BUT old poisoned slot -> degraded
        sqlx::query("INSERT INTO maintenance_operations (id, operation_kind, status, idempotency_key, created_at_ms, updated_at_ms) VALUES ('p1', 'repair_slot_poisoned', 'failed', 'i3', ?, ?)")
            .bind(now - 6 * 60 * 1000)
            .bind(now - 6 * 60 * 1000)
            .execute(&pool).await.unwrap();

        let health = storage_health(&pool).await.unwrap();
        let rollout = &health["rollout"];
        assert_eq!(rollout["p087_storage_tiering_status"], "degraded");

        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");
    }

    #[tokio::test]
    async fn proposal_087_promotion_budget_false_positive_subset_surfaces() {
        // Regression: promotion_budget_met must stay false when only a subset of
        // canonical governed surfaces has passing counters. Before this fix,
        // absent surfaces were ignored, making all_met=true for a partial set.
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let old_enough = now_ms - (49 * 60 * 60 * 1000_i64); // > 48 hours ago

        // Insert passing rows for only 2 of 6 canonical surfaces
        for surface in ["runs.list", "tools.list"] {
            sqlx::query(
                "INSERT OR REPLACE INTO hot_read_circuit_states
                 (governed_surface, circuit_status, consecutive_successes, consecutive_failures,
                  last_opened_at_ms, retry_after_ms, would_open, updated_at_ms,
                  total_requests, total_would_open, last_state_change_at_ms, first_observed_at_ms)
                 VALUES (?, 'closed', 0, 0, NULL, NULL, 0, ?, 500, 0, NULL, ?)",
            )
            .bind(surface)
            .bind(now_ms)
            .bind(old_enough)
            .execute(&pool)
            .await
            .unwrap();
        }

        let budget = compute_promotion_budget(&pool).await.unwrap();

        assert!(
            !budget.promotion_budget_met,
            "promotion_budget_met must be false when 4 canonical surfaces have no traffic"
        );
        assert_eq!(
            budget.per_surface.as_array().unwrap().len(),
            CANONICAL_HOT_READ_SURFACES.len(),
            "per_surface must enumerate all {} canonical surfaces",
            CANONICAL_HOT_READ_SURFACES.len()
        );

        let not_ready: Vec<_> = budget
            .per_surface
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| !s["promotion_ready"].as_bool().unwrap_or(true))
            .collect();
        assert!(
            not_ready.len() >= 4,
            "at least 4 surfaces (missing from DB) must not be promotion_ready"
        );
    }

    #[tokio::test]
    async fn proposal_087_p087_rollout_readback_fields_returns_required_p087_keys() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();
        let fields = p087_rollout_readback_fields(&pool).await;

        for key in [
            "p087_storage_tiering_status",
            "p087_mcp_liveness_status",
            "p087_hot_read_enforcement_status",
            "p087_promotion_budget_met",
            "p087_per_surface_promotion_budget",
            "p087_would_open_rate",
            "p087_total_requests_min",
            "p087_flap_free_hours_min",
            "rollout_contract_status",
            "rollout_contract_decision",
        ] {
            assert!(
                fields.get(key).is_some(),
                "p087_rollout_readback_fields missing key: {key}"
            );
        }
    }

    /// Regression: 100 immediate successes must NOT satisfy the 48-hour promotion window.
    /// first_observed_at_ms is set to "now" by the record_success inserts, so flap_free_ms < 48h.
    #[tokio::test]
    async fn proposal_087_100_immediate_successes_do_not_bypass_48h_window() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();

        // Record 100+ successes for every canonical surface right now
        for &surface in CANONICAL_HOT_READ_SURFACES {
            for _ in 0..110 {
                crate::repos::hot_read_circuit::record_success(&pool, surface)
                    .await
                    .unwrap();
            }
        }

        let budget = compute_promotion_budget(&pool).await.unwrap();

        assert!(
            !budget.promotion_budget_met,
            "promotion_budget_met must be false when first_observed_at_ms is too recent \
             (< 48 hours ago), even with 110 successful requests per surface"
        );
        for surface_entry in budget.per_surface.as_array().unwrap() {
            assert!(
                !surface_entry["promotion_ready"].as_bool().unwrap_or(true),
                "surface '{}' must not be promotion_ready when observation window < 48h",
                surface_entry["governed_surface"]
            );
            let hours = surface_entry["flap_free_hours"].as_f64().unwrap_or(999.0);
            assert!(
                hours < 1.0,
                "flap_free_hours for '{}' should be near 0 for immediate traffic, got {hours}",
                surface_entry["governed_surface"]
            );
        }
    }

    /// Regression: rollout_contract_status must be "hold" when promotion budget is pending.
    /// Before the fix, it returned "pass"/"ready" based on status=="active" alone.
    #[tokio::test]
    async fn proposal_087_rollout_contract_holds_when_promotion_budget_pending() {
        let pool = crate::pool::create_pool("sqlite::memory:").await.unwrap();

        // Fresh DB: no traffic, promotion budget not met
        let fields = p087_rollout_readback_fields(&pool).await;
        assert_eq!(
            fields["p087_promotion_budget_met"], false,
            "promotion_budget_met should be false with no traffic"
        );
        assert_eq!(
            fields["rollout_contract_status"], "hold",
            "rollout_contract_status must be 'hold' when promotion_budget_met is false"
        );
        assert_eq!(
            fields["rollout_contract_decision"], "hold",
            "rollout_contract_decision must be 'hold' when promotion_budget_met is false"
        );
        let hold_conditions = fields["rollout_contract_hold_conditions"]
            .as_array()
            .expect("rollout_contract_hold_conditions must be an array");
        assert!(
            hold_conditions
                .iter()
                .any(|c| c.as_str() == Some("p087_hot_read_promotion_budget_pending")),
            "hold_conditions must include p087_hot_read_promotion_budget_pending, got: {hold_conditions:?}"
        );
    }
}
