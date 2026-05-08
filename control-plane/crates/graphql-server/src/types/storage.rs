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
    pub coalesced_keys_pending: i64,
    pub coalesced_merged_total: i64,
    pub coalesced_flush_age_p95_ms: Option<f64>,
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
#[graphql(name = "StorageHealth", rename_fields = "camelCase")]
pub struct GqlStorageHealth {
    pub updated_at: String,
    pub stale_after_ms: i64,
    pub is_stale: bool,
    pub db_state: GqlStorageDbState,
    pub writer: GqlDbWriterHealth,
    pub wal: GqlWalHealth,
    pub projections: GqlProjectionStorageHealth,
    pub evidence_spool: GqlEvidenceSpoolSummary,
    pub kill_switches: GqlStorageKillSwitchState,
    pub thresholds: Vec<GqlStorageHealthThreshold>,
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
            coalesced_keys_pending: proj["coalescedKeysPending"].as_i64().unwrap_or(0),
            coalesced_merged_total: proj["coalescedMergedTotal"].as_i64().unwrap_or(0),
            coalesced_flush_age_p95_ms: proj["coalescedFlushAgeP95Ms"].as_f64(),
        };

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
            is_stale: json["isStale"].as_bool().unwrap_or(false),
            db_state,
            writer,
            wal,
            projections,
            evidence_spool,
            kill_switches,
            thresholds,
        })
    }
}
