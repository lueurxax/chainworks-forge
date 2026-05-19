use async_graphql::*;

/// P075 storage database state classification.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "StorageDbState")]
pub enum GqlStorageDbState {
    #[graphql(name = "HEALTHY")]
    Healthy,
    #[graphql(name = "DEGRADED")]
    Degraded,
    #[graphql(name = "STALE")]
    Stale,
    #[graphql(name = "MIGRATION_EMPTY")]
    MigrationEmpty,
    #[graphql(name = "LEGACY_ABSENT")]
    LegacyAbsent,
}

/// P075 DbWriter write class (priority lane classification).
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "WriteClass")]
pub enum GqlWriteClass {
    #[graphql(name = "A")]
    A,
    #[graphql(name = "B")]
    B,
    #[graphql(name = "C")]
    C,
    #[graphql(name = "D")]
    D,
}

/// P075 evidence spool file kind.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "EvidenceKind")]
pub enum GqlEvidenceKind {
    #[graphql(name = "TRANSCRIPT")]
    Transcript,
    #[graphql(name = "TOOL_TRACE")]
    ToolTrace,
    #[graphql(name = "STDOUT")]
    Stdout,
    #[graphql(name = "STDERR")]
    Stderr,
    #[graphql(name = "RECEIPT")]
    Receipt,
    #[graphql(name = "RUNTIME_EVENT")]
    RuntimeEvent,
    #[graphql(name = "MODEL_DELTA")]
    ModelDelta,
    #[graphql(name = "DELIVERY_READBACK")]
    DeliveryReadback,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "StorageHealthThreshold", rename_fields = "camelCase")]
pub struct GqlStorageHealthThreshold {
    pub metric: String,
    pub warn: f64,
    pub critical: f64,
    pub unit: String,
    pub action: String,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "DbWriterLaneHealth", rename_fields = "camelCase")]
pub struct GqlDbWriterLaneHealth {
    pub lane: String,
    pub capacity: i64,
    pub queued_depth: i64,
    pub queued_depth_ratio: f64,
    pub oldest_queued_age_ms: Option<i64>,
    pub rejected_total: i64,
    pub dropped_total: i64,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "DbWriterHealth", rename_fields = "camelCase")]
pub struct GqlDbWriterHealth {
    pub alive: bool,
    pub last_heartbeat_at: Option<String>,
    pub last_drain_at: Option<String>,
    pub total_queued: i64,
    pub lanes: Vec<GqlDbWriterLaneHealth>,
    pub write_lock_wait_p50_ms: Option<f64>,
    pub write_lock_wait_p95_ms: Option<f64>,
    pub transaction_duration_p50_ms: Option<f64>,
    pub transaction_duration_p95_ms: Option<f64>,
    pub busy_retry_rate_per_minute: f64,
    pub busy_retry_exhausted_total: i64,
    pub rejected_total: i64,
    pub dropped_telemetry_total: i64,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "WalHealth", rename_fields = "camelCase")]
pub struct GqlWalHealth {
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub size_bytes: Option<i64>,
    pub warn_size_bytes: i64,
    pub critical_size_bytes: i64,
    pub last_checkpoint_at: Option<String>,
    pub checkpoint_duration_p95_ms: Option<f64>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ProjectionStorageHealth", rename_fields = "camelCase")]
pub struct GqlProjectionStorageHealth {
    pub pending_invalidations: i64,
    pub projection_lag_ms: Option<i64>,
    pub latency_ms: Option<i64>,
    pub rebuild_duration_p95_ms: Option<f64>,
    pub coalesced_keys_pending: i64,
    pub coalesced_merged_total: i64,
    pub coalesced_flush_age_p95_ms: Option<f64>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ProjectionFreshnessV1", rename_fields = "camelCase")]
pub struct GqlProjectionFreshnessV1 {
    pub projection_name: String,
    pub source_name: Option<String>,
    pub watermark_ms: i64,
    pub is_poisoned: bool,
    pub last_error: Option<String>,
    pub updated_at_ms: i64,
    pub throttled_until_ms: Option<i64>,
    pub backlog_rows: i64,
    pub backlog_bytes: i64,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "CircuitStatusV1")]
pub enum GqlCircuitStatusV1 {
    #[graphql(name = "CLOSED")]
    Closed,
    #[graphql(name = "OPEN")]
    Open,
    #[graphql(name = "HALF_OPEN")]
    HalfOpen,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "HotReadCircuitStateV1", rename_fields = "camelCase")]
pub struct GqlHotReadCircuitStateV1 {
    pub governed_surface: String,
    pub circuit_status: GqlCircuitStatusV1,
    pub consecutive_successes: i32,
    pub consecutive_failures: i32,
    pub last_violation_kind: Option<String>,
    pub would_open: bool,
    pub last_opened_at_ms: Option<i64>,
    pub retry_after_ms: Option<i64>,
    pub updated_at_ms: i64,
    pub latency_ms: Option<i64>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "MaintenanceOperationStatusV1", rename_fields = "camelCase")]
pub struct GqlMaintenanceOperationStatusV1 {
    pub id: String,
    pub operation_kind: String,
    pub status: String,
    pub idempotency_key: String,
    pub slot_generation: i64,
    pub started_at_ms: Option<i64>,
    pub completed_at_ms: Option<i64>,
    pub error: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "DegradedStateV1", rename_fields = "camelCase")]
pub struct GqlDegradedStateV1 {
    pub severity: String,
    pub reason: String,
    pub message: String,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "EvidenceSpoolSummary", rename_fields = "camelCase")]
pub struct GqlEvidenceSpoolSummary {
    pub enabled: bool,
    pub files_written_total: i64,
    pub bytes_written_total: i64,
    pub metadata_rows_total: i64,
    pub orphan_files: i64,
    pub orphan_bytes: i64,
    pub recovered_files: i64,
    pub checksum_mismatch_files: i64,
    pub pending_delete_files: i64,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "StorageKillSwitchState", rename_fields = "camelCase")]
pub struct GqlStorageKillSwitchState {
    pub db_writer_bypass_classes: Vec<GqlWriteClass>,
    pub coalescing_disabled_keys: Vec<String>,
    pub evidence_spool_disabled_kinds: Vec<GqlEvidenceKind>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "StorageHealth", rename_fields = "camelCase", complex)]
pub struct GqlStorageHealth {
    pub updated_at: String,
    pub stale_after_ms: i64,
    pub is_stale: bool,
    pub db_state: GqlStorageDbState,
    pub writer: GqlDbWriterHealth,
    pub wal: GqlWalHealth,
    #[graphql(skip)]
    pub projections: GqlProjectionStorageHealth,
    pub evidence_spool: GqlEvidenceSpoolSummary,
    pub kill_switches: GqlStorageKillSwitchState,
    pub thresholds: Vec<GqlStorageHealthThreshold>,
    pub projection_freshness: Vec<GqlProjectionFreshnessV1>,
    pub hot_read_guards: Vec<GqlHotReadCircuitStateV1>,
    pub maintenance_operations: Vec<GqlMaintenanceOperationStatusV1>,
    pub degraded: Option<GqlDegradedStateV1>,
    pub rollout: serde_json::Value,
}

#[ComplexObject]
impl GqlStorageHealth {
    async fn projections(&self) -> GqlProjectionStorageHealth {
        db::metrics::increment_counter("storage_health_legacy_projection_field_compat_total");
        self.projections.clone()
    }

    async fn projection_freshness_by_source(
        &self,
        #[graphql(default)] projection_name: Option<String>,
        #[graphql(default)] source_name: Option<String>,
    ) -> Vec<GqlProjectionFreshnessV1> {
        self.projection_freshness
            .iter()
            .filter(|f| {
                let name_match = projection_name
                    .as_ref()
                    .map_or(true, |p| &f.projection_name == p);
                let source_match = source_name
                    .as_ref()
                    .map_or(true, |s| f.source_name.as_ref() == Some(s));
                name_match && source_match
            })
            .cloned()
            .collect()
    }
}

impl GqlStorageHealth {
    /// Build from the JSON produced by `db::repos::storage_health::storage_health()`.
    pub fn from_storage_health_json(json: serde_json::Value) -> anyhow::Result<Self> {
        // Fail-closed: absent or unrecognised dbState is DEGRADED, not HEALTHY (SEC-005).
        let db_state = match json["dbState"].as_str().unwrap_or("DEGRADED") {
            "HEALTHY" => GqlStorageDbState::Healthy,
            "STALE" => GqlStorageDbState::Stale,
            "MIGRATION_EMPTY" => GqlStorageDbState::MigrationEmpty,
            "LEGACY_ABSENT" => GqlStorageDbState::LegacyAbsent,
            _ => GqlStorageDbState::Degraded,
        };

        let w = &json["writer"];
        let lanes: Vec<GqlDbWriterLaneHealth> = w["lanes"]
            .as_array()
            .map(|lanes| {
                lanes
                    .iter()
                    .map(|l| GqlDbWriterLaneHealth {
                        lane: l["lane"].as_str().unwrap_or("").to_string(),
                        capacity: l["capacity"].as_i64().unwrap_or(0),
                        queued_depth: l["queuedDepth"].as_i64().unwrap_or(0),
                        queued_depth_ratio: l["queuedDepthRatio"].as_f64().unwrap_or(0.0),
                        oldest_queued_age_ms: l["oldestQueuedAgeMs"].as_i64(),
                        rejected_total: l["rejectedTotal"].as_i64().unwrap_or(0),
                        dropped_total: l["droppedTotal"].as_i64().unwrap_or(0),
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Fail-closed: absent writer.alive is false, not true (SEC-005).
        let writer = GqlDbWriterHealth {
            alive: w["alive"].as_bool().unwrap_or(false),
            last_heartbeat_at: w["lastHeartbeatAt"].as_str().map(String::from),
            last_drain_at: w["lastDrainAt"].as_str().map(String::from),
            total_queued: w["totalQueued"].as_i64().unwrap_or(0),
            lanes,
            write_lock_wait_p50_ms: w["writeLockWaitP50Ms"].as_f64(),
            write_lock_wait_p95_ms: w["writeLockWaitP95Ms"].as_f64(),
            transaction_duration_p50_ms: w["transactionDurationP50Ms"].as_f64(),
            transaction_duration_p95_ms: w["transactionDurationP95Ms"].as_f64(),
            busy_retry_rate_per_minute: w["busyRetryRatePerMinute"].as_f64().unwrap_or(0.0),
            busy_retry_exhausted_total: w["busyRetryExhaustedTotal"].as_i64().unwrap_or(0),
            rejected_total: w["rejectedTotal"].as_i64().unwrap_or(0),
            dropped_telemetry_total: w["droppedTelemetryTotal"].as_i64().unwrap_or(0),
        };

        let wal_json = &json["wal"];
        let wal = GqlWalHealth {
            available: wal_json["available"].as_bool().unwrap_or(false),
            unavailable_reason: wal_json["unavailableReason"].as_str().map(String::from),
            size_bytes: wal_json["sizeBytes"].as_i64(),
            warn_size_bytes: wal_json["warnSizeBytes"].as_i64().unwrap_or(134_217_728),
            critical_size_bytes: wal_json["criticalSizeBytes"]
                .as_i64()
                .unwrap_or(536_870_912),
            last_checkpoint_at: wal_json["lastCheckpointAt"].as_str().map(String::from),
            checkpoint_duration_p95_ms: wal_json["checkpointDurationP95Ms"].as_f64(),
        };

        let proj = &json["projections"];
        let projections = GqlProjectionStorageHealth {
            pending_invalidations: proj["pendingInvalidations"].as_i64().unwrap_or(0),
            projection_lag_ms: proj["projectionLagMs"].as_i64(),
            latency_ms: proj["latencyMs"].as_i64(),
            rebuild_duration_p95_ms: proj["rebuildDurationP95Ms"].as_f64(),
            coalesced_keys_pending: proj["coalescedKeysPending"].as_i64().unwrap_or(0),
            coalesced_merged_total: proj["coalescedMergedTotal"].as_i64().unwrap_or(0),
            coalesced_flush_age_p95_ms: proj["coalescedFlushAgeP95Ms"].as_f64(),
        };

        let projection_freshness = json["projectionFreshness"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|f| GqlProjectionFreshnessV1 {
                        projection_name: f["projectionName"].as_str().unwrap_or("").to_string(),
                        source_name: f["sourceName"].as_str().map(String::from),
                        watermark_ms: f["watermarkMs"].as_i64().unwrap_or(0),
                        is_poisoned: f["isPoisoned"].as_bool().unwrap_or(false),
                        last_error: f["lastError"].as_str().map(String::from),
                        updated_at_ms: f["updatedAtMs"].as_i64().unwrap_or(0),
                        throttled_until_ms: f["throttledUntilMs"].as_i64(),
                        backlog_rows: f["backlogRows"].as_i64().unwrap_or(0),
                        backlog_bytes: f["backlogBytes"].as_i64().unwrap_or(0),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let hot_read_guards = json["hotReadGuards"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|g| GqlHotReadCircuitStateV1 {
                        governed_surface: g["governedSurface"].as_str().unwrap_or("").to_string(),
                        circuit_status: match g["circuitStatus"].as_str().unwrap_or("CLOSED") {
                            "OPEN" | "open" => GqlCircuitStatusV1::Open,
                            "HALF_OPEN" | "half_open" => GqlCircuitStatusV1::HalfOpen,
                            _ => GqlCircuitStatusV1::Closed,
                        },
                        consecutive_successes: g["consecutiveSuccesses"].as_i64().unwrap_or(0)
                            as i32,
                        consecutive_failures: g["consecutiveFailures"].as_i64().unwrap_or(0) as i32,
                        last_violation_kind: g["lastViolationKind"].as_str().map(String::from),
                        would_open: g["wouldOpen"].as_bool().unwrap_or(false),
                        last_opened_at_ms: g["lastOpenedAtMs"].as_i64(),
                        retry_after_ms: g["retryAfterMs"].as_i64(),
                        updated_at_ms: g["updatedAtMs"].as_i64().unwrap_or(0),
                        latency_ms: g["latencyMs"].as_i64(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let maintenance_operations = json["maintenanceOperations"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|o| GqlMaintenanceOperationStatusV1 {
                        id: o["id"].as_str().unwrap_or("").to_string(),
                        operation_kind: o["operationKind"].as_str().unwrap_or("").to_string(),
                        status: o["status"].as_str().unwrap_or("").to_string(),
                        idempotency_key: o["idempotencyKey"].as_str().unwrap_or("").to_string(),
                        slot_generation: o["slotGeneration"].as_i64().unwrap_or(1),
                        started_at_ms: o["startedAtMs"].as_i64(),
                        completed_at_ms: o["completedAtMs"].as_i64(),
                        error: o["error"].as_str().map(String::from),
                        created_at_ms: o["createdAtMs"].as_i64().unwrap_or(0),
                        updated_at_ms: o["updatedAtMs"].as_i64().unwrap_or(0),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let degraded = json["degraded"].as_object().map(|d| GqlDegradedStateV1 {
            severity: d["severity"].as_str().unwrap_or("info").to_string(),
            reason: d["reason"].as_str().unwrap_or("").to_string(),
            message: d["message"].as_str().unwrap_or("").to_string(),
        });

        let rollout = json["rollout"].clone();

        let ev = &json["evidenceSpool"];
        // Fail-closed: absent evidenceSpool.enabled is false, not true (SEC-005).
        let evidence_spool = GqlEvidenceSpoolSummary {
            enabled: ev["enabled"].as_bool().unwrap_or(false),
            files_written_total: ev["filesWrittenTotal"].as_i64().unwrap_or(0),
            bytes_written_total: ev["bytesWrittenTotal"].as_i64().unwrap_or(0),
            metadata_rows_total: ev["metadataRowsTotal"].as_i64().unwrap_or(0),
            orphan_files: ev["orphanFiles"].as_i64().unwrap_or(0),
            orphan_bytes: ev["orphanBytes"].as_i64().unwrap_or(0),
            recovered_files: ev["recoveredFiles"].as_i64().unwrap_or(0),
            checksum_mismatch_files: ev["checksumMismatchFiles"].as_i64().unwrap_or(0),
            pending_delete_files: ev["pendingDeleteFiles"].as_i64().unwrap_or(0),
        };

        let ks = &json["killSwitches"];
        let kill_switches = GqlStorageKillSwitchState {
            db_writer_bypass_classes: ks["dbWriterBypassClasses"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| match v.as_str()? {
                            "A" => Some(GqlWriteClass::A),
                            "B" => Some(GqlWriteClass::B),
                            "C" => Some(GqlWriteClass::C),
                            "D" => Some(GqlWriteClass::D),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            coalescing_disabled_keys: ks["coalescingDisabledKeys"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            evidence_spool_disabled_kinds: ks["evidenceSpoolDisabledKinds"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| match v.as_str()? {
                            "TRANSCRIPT" => Some(GqlEvidenceKind::Transcript),
                            "TOOL_TRACE" => Some(GqlEvidenceKind::ToolTrace),
                            "STDOUT" => Some(GqlEvidenceKind::Stdout),
                            "STDERR" => Some(GqlEvidenceKind::Stderr),
                            "RECEIPT" => Some(GqlEvidenceKind::Receipt),
                            "RUNTIME_EVENT" => Some(GqlEvidenceKind::RuntimeEvent),
                            "MODEL_DELTA" => Some(GqlEvidenceKind::ModelDelta),
                            "DELIVERY_READBACK" => Some(GqlEvidenceKind::DeliveryReadback),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default(),
        };

        let thresholds: Vec<GqlStorageHealthThreshold> = json["thresholds"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| {
                        Some(GqlStorageHealthThreshold {
                            metric: t["metric"].as_str()?.to_string(),
                            warn: t["warn"].as_f64()?,
                            critical: t["critical"].as_f64()?,
                            unit: t["unit"].as_str()?.to_string(),
                            action: t["action"].as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(GqlStorageHealth {
            updated_at: json["updatedAt"].as_str().unwrap_or("").to_string(),
            stale_after_ms: json["staleAfterMs"].as_i64().unwrap_or(5000),
            is_stale: json["isStale"].as_bool().unwrap_or(true),
            db_state,
            writer,
            wal,
            projections,
            evidence_spool,
            kill_switches,
            thresholds,
            projection_freshness,
            hot_read_guards,
            maintenance_operations,
            degraded,
            rollout,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_health_json(is_stale: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": "storage_health.v1",
            "updatedAt": "2026-01-01T00:00:00Z",
            "staleAfterMs": 5000,
            "isStale": is_stale,
            "dbState": "DEGRADED",
            "writer": { "alive": false, "totalQueued": 0, "lanes": [],
                        "busyRetryRatePerMinute": 0.0, "busyRetryExhaustedTotal": 0,
                        "rejectedTotal": 0, "droppedTelemetryTotal": 0 },
            "wal": { "available": false, "warnSizeBytes": 134217728,
                     "criticalSizeBytes": 536870912 },
            "projections": { "pendingInvalidations": 0, "coalescedKeysPending": 0,
                             "coalescedMergedTotal": 0 },
            "evidenceSpool": { "enabled": false, "filesWrittenTotal": 0, "bytesWrittenTotal": 0,
                               "metadataRowsTotal": 0, "orphanFiles": 0, "orphanBytes": 0,
                               "recoveredFiles": 0, "checksumMismatchFiles": 0,
                               "pendingDeleteFiles": 0 },
            "killSwitches": { "dbWriterBypassClasses": [], "coalescingDisabledKeys": [],
                              "evidenceSpoolDisabledKinds": [] },
            "thresholds": [],
            "rollout": {}
        })
    }

    /// SEC-003: absent isStale must default to true (fail-closed).
    #[test]
    fn sec003_absent_is_stale_defaults_to_true() {
        let mut json = minimal_health_json(serde_json::Value::Null);
        // Remove isStale entirely.
        json.as_object_mut().unwrap().remove("isStale");
        let gql = GqlStorageHealth::from_storage_health_json(json)
            .expect("from_storage_health_json must succeed");
        assert!(
            gql.is_stale,
            "absent isStale must default to true (fail-closed, SEC-003)"
        );
    }

    /// SEC-003: malformed isStale (wrong type) must default to true (fail-closed).
    #[test]
    fn sec003_malformed_is_stale_defaults_to_true() {
        let json = minimal_health_json(serde_json::json!("not-a-bool"));
        let gql = GqlStorageHealth::from_storage_health_json(json)
            .expect("from_storage_health_json must succeed for malformed isStale");
        assert!(
            gql.is_stale,
            "malformed isStale must default to true (fail-closed, SEC-003)"
        );
    }

    /// SEC-003: explicit false isStale is preserved (not overridden by the default).
    #[test]
    fn sec003_explicit_false_is_stale_is_preserved() {
        let json = minimal_health_json(serde_json::json!(false));
        let gql = GqlStorageHealth::from_storage_health_json(json)
            .expect("from_storage_health_json must succeed");
        assert!(!gql.is_stale, "explicit isStale=false must be preserved");
    }

    #[test]
    fn proposal_087_storage_health_v1_preservation() {
        let health_json = serde_json::json!({
            "updatedAt": "2024-01-01T00:00:00Z",
            "staleAfterMs": 1000,
            "isStale": false,
            "dbState": "HEALTHY",
            "writer": {
                "alive": true,
                "lanes": [],
                "writeLockWaitP50Ms": 1,
                "writeLockWaitP95Ms": 2,
                "busyRetryRatePerMinute": 0.0,
                "busyRetryExhaustedTotal": 0,
                "rejectedTotal": 0,
                "droppedTelemetryTotal": 0
            },
            "wal": {
                "available": true,
                "sizeBytes": 0,
                "warnSizeBytes": 100,
                "criticalSizeBytes": 200,
                "checkpointDurationP95Ms": 1
            },
            "projections": {
                "pendingInvalidations": 5,
                "projectionLagMs": 100,
                "latencyMs": 100,
                "rebuildDurationP95Ms": 10,
                "coalescedKeysPending": 0,
                "coalescedMergedTotal": 0,
                "coalescedFlushAgeP95Ms": null
            },
            "evidenceSpool": {
                "enabled": true,
                "filesWrittenTotal": 0,
                "bytesWrittenTotal": 0,
                "metadataRowsTotal": 0,
                "orphanFiles": 0,
                "orphanBytes": 0,
                "recoveredFiles": 0,
                "checksumMismatchFiles": 0,
                "pendingDeleteFiles": 0
            },
            "killSwitches": {
                "dbWriterBypassClasses": [],
                "coalescingDisabledKeys": [],
                "evidenceSpoolDisabledKinds": []
            },
            "thresholds": [],
            "projectionFreshness": [],
            "hotReadGuards": [],
            "maintenanceOperations": [],
            "rollout": {
                "p087_storage_tiering_status": "active"
            }
        });

        let health = GqlStorageHealth::from_storage_health_json(health_json).unwrap();
        assert_eq!(health.projections.pending_invalidations, 5);
        assert_eq!(health.projections.projection_lag_ms, Some(100));
    }

    #[test]
    fn proposal_087_hot_read_guard_status_accepts_db_lowercase() {
        let mut health_json = minimal_health_json(serde_json::json!(true));
        health_json["hotReadGuards"] = serde_json::json!([
            {
                "governedSurface": "storage.health",
                "circuitStatus": "open",
                "consecutiveSuccesses": 0,
                "consecutiveFailures": 3,
                "lastViolationKind": "timeout",
                "wouldOpen": false,
                "lastOpenedAtMs": 1715712000000i64,
                "retryAfterMs": 1715712030000i64,
                "updatedAtMs": 1715712000000i64,
                "latencyMs": 500
            },
            {
                "governedSurface": "runs.list",
                "circuitStatus": "half_open",
                "consecutiveSuccesses": 1,
                "consecutiveFailures": 0,
                "wouldOpen": false,
                "updatedAtMs": 1715712000000i64
            }
        ]);
        let health = GqlStorageHealth::from_storage_health_json(health_json).unwrap();
        assert_eq!(
            health.hot_read_guards[0].circuit_status,
            GqlCircuitStatusV1::Open
        );
        assert_eq!(
            health.hot_read_guards[1].circuit_status,
            GqlCircuitStatusV1::HalfOpen
        );
    }
}
