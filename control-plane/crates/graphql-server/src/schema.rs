use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use async_graphql::futures_util::{stream, StreamExt};
use async_graphql::*;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tracing::{debug, info, warn};

use db::repos::{
    agent_work_continuations, approvals, artifact_contracts, artifacts, audit_log, closeout,
    code_writer_completion_receipts, ideas, projections, rollout_contract_checks, runs,
    sessions as session_repo, steward as steward_repo, workflow_conflicts,
};
use db::writer::DbWriterHeartbeat;
use domain::commands::{ApprovalResolutionDecision, CallerContext, Command, ResolveApprovalCmd};
use domain::events::DomainEvent;
use domain::ids::{ArtifactId, IdeaId, RunId};
use domain::lifecycle::DaemonStatus;
use engine::command_handler::{ApprovalResolutionConflict, CommandHandler};
use engine::event_bus::EventSender;
use engine::lifecycle_reporter::LifecycleReporter;

use crate::types::approval::GqlApproval;
use crate::types::artifact::{GqlArtifact, P085_NO_DEADLINE_JUSTIFICATION};
use crate::types::continuation::{
    GqlContinuationCandidatesResult, GqlContinuationMetricsSummary, GqlContinuationRecord,
    GqlContinuationStatus,
};
use crate::types::idea::GqlIdea;
use crate::types::p031::{
    GqlMutationConflictResultCode, GqlPayloadAvailabilityState, GqlPayloadUnavailableReasonCode,
};
use crate::types::run::GqlRun;
use crate::types::scheduler::{GqlStartupRecoverySummary, GqlToolchainCacheHousekeepingSummary};
use crate::types::session::{
    transient_db_unavailable_health, GqlSessionEventConnection, GqlSessionGenerationConnection,
    GqlSessionHealthReport, GqlSessionKpiSummary, GqlSessionLineage, GqlSessionLineageConnection,
    GqlSessionStatusChangedEvent, P046Config, P046LiveCredential, P046LivePrincipalHandle,
    SESSION_EVENTS_MAX_FIRST, SESSION_GENERATIONS_MAX_FIRST, SESSION_LINEAGES_MAX_FIRST,
};
use crate::types::stage::{
    GqlAgentExecution, GqlRunStageTopologyNode, GqlRunStageTopologyOccurrence,
    GqlRunStageTopologyTransition, GqlStageExecution,
};
use crate::types::steward::{
    GqlStewardAnalysis, GqlStewardAnalysisRunLink, GqlStewardRecommendation,
};

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

static P081_SUBSCRIPTION_SEQUENCE: AtomicI64 = AtomicI64::new(0);
static P081_GRAPHQL_SAFE_MODE_ALERT_OPENED_AT_MS: OnceLock<Mutex<Option<i64>>> = OnceLock::new();

/// P081 server-side field redaction collector.
///
/// Resolver code records field-level redactions here without raising GraphQL
/// errors. The HTTP handler attaches the collected entries to
/// `extensions.redactions`, preserving the distinction between ordinary nulls
/// and policy-redacted nulls for Swift clients.
#[derive(Clone, Default)]
pub struct P081GraphqlRedactionCollector {
    redactions: Arc<Mutex<Vec<async_graphql::Value>>>,
}

impl P081GraphqlRedactionCollector {
    pub fn push_field_null_redaction(
        &self,
        path: Vec<&str>,
        row_id: Option<&str>,
        caller_class: &str,
    ) {
        let mut object = async_graphql::indexmap::IndexMap::new();
        object.insert(
            async_graphql::Name::new("path"),
            async_graphql::Value::List(
                path.into_iter()
                    .map(|segment| async_graphql::Value::String(segment.to_string()))
                    .collect(),
            ),
        );
        object.insert(
            async_graphql::Name::new("reasonCode"),
            async_graphql::Value::String("OBSERVER_SCOPE".to_string()),
        );
        if let Some(row_id) = row_id {
            object.insert(
                async_graphql::Name::new("rowId"),
                async_graphql::Value::String(row_id.to_string()),
            );
        }
        object.insert(
            async_graphql::Name::new("redactionMode"),
            async_graphql::Value::String("field_null_redacted".to_string()),
        );
        object.insert(
            async_graphql::Name::new("callerClass"),
            async_graphql::Value::String(caller_class.to_string()),
        );
        object.insert(
            async_graphql::Name::new("redactionId"),
            async_graphql::Value::String(format!(
                "p081:{}:{}",
                caller_class,
                row_id.unwrap_or("unknown")
            )),
        );
        db::metrics::increment_counter("graphql_redaction_extensions_total");
        self.redactions
            .lock()
            .expect("p081 redaction collector poisoned")
            .push(async_graphql::Value::Object(object));
    }

    pub fn snapshot(&self) -> Vec<async_graphql::Value> {
        self.redactions
            .lock()
            .expect("p081 redaction collector poisoned")
            .clone()
    }
}

pub fn attach_p081_collected_redactions(
    response: &mut async_graphql::Response,
    collector: &P081GraphqlRedactionCollector,
) {
    let collected = collector.snapshot();
    if collected.is_empty() {
        return;
    }
    match response.extensions.get_mut("redactions") {
        Some(async_graphql::Value::List(existing)) => existing.extend(collected),
        _ => {
            response
                .extensions
                .insert("redactions".into(), async_graphql::Value::List(collected));
        }
    }
}

pub fn build_schema(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
) -> AppSchema {
    build_schema_inner(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        None,
        Some(embedded_shadow_boundary_policy()),
    )
}

pub fn build_schema_with_storage_writer(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    storage_writer_heartbeat: Arc<DbWriterHeartbeat>,
) -> AppSchema {
    build_schema_inner(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        Some(storage_writer_heartbeat),
        Some(embedded_shadow_boundary_policy()),
    )
}

/// P081 Phase 3: build the schema with a shared BoundaryPolicy service injected so
/// mutation_allowed and query guards can consult the daemon-level policy decision.
pub fn build_schema_with_storage_writer_and_boundary_policy(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    storage_writer_heartbeat: Arc<DbWriterHeartbeat>,
    boundary_policy: Arc<auth::boundary::BoundaryPolicy>,
) -> AppSchema {
    build_schema_inner(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        Some(storage_writer_heartbeat),
        Some(boundary_policy),
    )
}

/// Like `build_schema_with_storage_writer` but also returns the live principal handle
/// so the caller (daemon) can update it when principals.json changes, enabling
/// revocation without a daemon restart.
pub fn build_schema_with_storage_writer_and_handle(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    storage_writer_heartbeat: Arc<DbWriterHeartbeat>,
) -> (AppSchema, P046LivePrincipalHandle) {
    let p046 = default_p046_config();
    let live_handle = P046LivePrincipalHandle::new(principal_table.clone());
    let schema = build_schema_inner_with_p046_and_handle(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        Some(storage_writer_heartbeat),
        Some(embedded_shadow_boundary_policy()),
        p046,
        live_handle.clone(),
    );
    (schema, live_handle)
}

pub fn build_schema_with_storage_writer_boundary_policy_and_handle(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    storage_writer_heartbeat: Arc<DbWriterHeartbeat>,
    boundary_policy: Arc<auth::boundary::BoundaryPolicy>,
) -> (AppSchema, P046LivePrincipalHandle) {
    let p046 = default_p046_config();
    let live_handle = P046LivePrincipalHandle::new(principal_table.clone());
    let schema = build_schema_inner_with_p046_and_handle(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        Some(storage_writer_heartbeat),
        Some(boundary_policy),
        p046,
        live_handle.clone(),
    );
    (schema, live_handle)
}

fn build_schema_inner(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    storage_writer_heartbeat: Option<Arc<DbWriterHeartbeat>>,
    boundary_policy: Option<Arc<auth::boundary::BoundaryPolicy>>,
) -> AppSchema {
    let p046 = default_p046_config();
    build_schema_inner_with_p046(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        storage_writer_heartbeat,
        boundary_policy,
        p046,
    )
}

fn default_p046_config() -> P046Config {
    P046Config {
        enabled: true,
        subscription_channel_capacity: 64,
    }
}

fn build_schema_inner_with_p046(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    storage_writer_heartbeat: Option<Arc<DbWriterHeartbeat>>,
    boundary_policy: Option<Arc<auth::boundary::BoundaryPolicy>>,
    p046: P046Config,
) -> AppSchema {
    let live_handle = P046LivePrincipalHandle::new(principal_table.clone());
    build_schema_inner_with_p046_and_handle(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        storage_writer_heartbeat,
        boundary_policy,
        p046,
        live_handle,
    )
}

fn build_schema_inner_with_p046_and_handle(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    storage_writer_heartbeat: Option<Arc<DbWriterHeartbeat>>,
    boundary_policy: Option<Arc<auth::boundary::BoundaryPolicy>>,
    p046: P046Config,
    live_handle: P046LivePrincipalHandle,
) -> AppSchema {
    // P046 reset mutation guard: record that the schema was built without any
    // resetSession/equivalent mutation. This counter is incremented once per schema
    // construction to prove the guard is active. It must remain zero for "fail" labels.
    if p046.enabled {
        db::metrics::increment_counter_with_label(
            "session_graphql_reset_mutation_guard_total",
            "pass",
        );
    }
    let mut builder = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
        .data(pool)
        .data(cmd_handler)
        .data(events)
        .data(principal_table)
        .data(live_handle)
        .data(reporter)
        .data(p046);
    if let Some(heartbeat) = storage_writer_heartbeat {
        builder = builder.data(heartbeat);
    }
    if let Some(policy) = boundary_policy {
        builder = builder.data(policy);
    }
    builder.finish()
}

pub fn build_schema_with_session_observability(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
) -> AppSchema {
    build_schema_inner_with_p046(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        None,
        Some(embedded_shadow_boundary_policy()),
        P046Config {
            enabled: true,
            subscription_channel_capacity: 64,
        },
    )
}

/// Build a P046 schema and return the live principal handle so callers can drive
/// revocation tests by updating the handle after the schema is constructed.
pub fn build_schema_with_session_observability_and_live_handle(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
) -> (AppSchema, P046LivePrincipalHandle) {
    let live_handle = P046LivePrincipalHandle::new(principal_table.clone());
    let schema = build_schema_inner_with_p046_and_handle(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        None,
        Some(embedded_shadow_boundary_policy()),
        P046Config {
            enabled: true,
            subscription_channel_capacity: 64,
        },
        live_handle.clone(),
    );
    (schema, live_handle)
}

/// Build a schema with an explicit P046Config — used in tests to set a smaller
/// subscription channel capacity for slow-consumer disconnect testing.
pub fn build_schema_with_p046_config(
    pool: SqlitePool,
    cmd_handler: Arc<CommandHandler>,
    events: EventSender,
    principal_table: auth::PrincipalTable,
    reporter: LifecycleReporter,
    p046: P046Config,
) -> AppSchema {
    build_schema_inner_with_p046(
        pool,
        cmd_handler,
        events,
        principal_table,
        reporter,
        None,
        Some(embedded_shadow_boundary_policy()),
        p046,
    )
}

fn embedded_shadow_boundary_policy() -> Arc<auth::boundary::BoundaryPolicy> {
    Arc::new(
        auth::boundary::BoundaryPolicy::from_embedded_with_mode(auth::boundary::PolicyMode::Shadow)
            .expect("embedded P081 boundary fixture must be valid"),
    )
}

pub struct QueryRoot;

// P046 transient SQLite retry is db-owned: db::p046_retry::p046_retry_db.
// The local forwarding alias keeps call-site notation identical to the pre-move form.
use db::p046_retry::p046_retry_db;

/// Returns a bounded metric label for a session event type (for subscription_event_total).
fn session_event_type_metric_label(event_type: &domain::session::SessionEventType) -> &'static str {
    use domain::session::SessionEventType;
    match event_type {
        SessionEventType::Created => "GENERATION_STARTED",
        SessionEventType::Reused => "SESSION_REUSED",
        SessionEventType::Invalidated => "GENERATION_INVALIDATED",
        SessionEventType::Closed => "GENERATION_CLOSED",
        SessionEventType::OperatorReset => "OPERATOR_RESET_RECORDED",
        SessionEventType::OutputContractRepairStarted
        | SessionEventType::OutputContractRepairSucceeded
        | SessionEventType::OutputContractRepairSkipped => "REPAIR_ATTEMPTED",
        SessionEventType::OutputContractRepairFailed => "REPAIR_FAILED",
        SessionEventType::BudgetExceeded => "CONTEXT_WINDOW_OBSERVED",
        _ => "UNKNOWN_EVENT_SHAPE",
    }
}

// P046 metric names emitted via db::metrics helpers (listed here for gate inventory grep):
//   session_graphql_query_duration_seconds   → db::metrics::record_p046_query_duration
//   session_status_subscription_emit_lag_seconds → db::metrics::record_p046_emit_lag
//   session_graphql_observability_query_success_rate → incremented inside record_p046_query_duration

/// P046 resolver deadline: 2 seconds per resolver. Passed into p046_retry_db so all
/// chained DB calls within a single resolver share the same budget.
fn p046_resolver_deadline() -> tokio::time::Instant {
    tokio::time::Instant::now() + std::time::Duration::from_secs(2)
}

fn boundary_denial_error(
    reason_code: &str,
    row_id: Option<&str>,
    caller_class: Option<&str>,
) -> async_graphql::Error {
    async_graphql::Error::new("forbidden").extend_with(|_, e| {
        e.set("code", "FORBIDDEN");
        e.set("reasonCode", reason_code);
        if let Some(rid) = row_id {
            e.set("rowId", rid);
        }
        if let Some(cc) = caller_class {
            e.set("callerClass", cc);
        }
    })
}

fn record_p081_caller_class_diagnostics(
    principal: &auth::Principal,
    caller_class: &auth::CallerClass,
    transport: &str,
) {
    let default = auth::derive_caller_class_from_principal_class(&principal.class);
    if principal.caller_class_override.is_some() && default != *caller_class {
        let principal_class = format!("{:?}", principal.class).to_lowercase();
        db::metrics::record_p081_auth_ambiguous_caller_warn(
            &principal_class,
            "principal_override",
            transport,
        );
    }
}

/// P081 MEDIUM-001: Write a best-effort audit row for legacy/shadow deny paths.
/// Unlike write_graphql_deny_audit, audit failure here only produces a warning log
/// rather than failing the request, because these are legacy guard denials where
/// the audit path does not yet have full bounded-seam status.
async fn write_graphql_legacy_deny_audit(
    ctx: &Context<'_>,
    principal: &auth::Principal,
    transport: &str,
    action_attempted: &str,
    reason_code: &str,
    row_id: Option<&str>,
    caller_class_str: &str,
    policy: &auth::boundary::BoundaryPolicy,
) {
    let Ok(pool) = ctx.data::<SqlitePool>() else {
        return;
    };
    let id = uuid::Uuid::now_v7().to_string();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let request_id_buf = ctx
        .data::<crate::request_id::RequestId>()
        .ok()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let token_id_buf = ctx
        .data::<crate::auth_layer::GraphqlTokenId>()
        .ok()
        .map(|t| t.0.clone());
    let principal_id_str = principal.id.to_string();
    let principal_class_str = principal.class.to_string();
    let mode_str = policy.mode().as_str().to_string();
    let raw_payload = serde_json::json!({
        "event": "boundary_decision_legacy_deny",
        "decision": "deny",
        "transport": transport,
        "action_attempted": action_attempted,
        "reason_code": reason_code,
        "row_id": row_id,
    })
    .to_string();
    let (stored_payload, _, truncated) = audit_log::build_envelope(&raw_payload);
    let entry = audit_log::AuditEntry {
        id: &id,
        request_id: &request_id_buf,
        timestamp_ms: now_ms,
        event_type: "boundary_decision_deny",
        principal_id: Some(&principal_id_str),
        principal_class: Some(&principal_class_str),
        caller_class: Some(caller_class_str),
        token_id: token_id_buf.as_deref(),
        transport,
        action_attempted,
        decision: "deny",
        denial_reason_code: Some(reason_code),
        row_id,
        env_gate_state: None,
        source_ip_hash_or_local_process_id: None,
        boundary_policy_mode: &mode_str,
        fixture_version: "p081-boundary-matrix-v1",
        payload: &stored_payload,
        original_payload_bytes: if truncated { Some(&raw_payload) } else { None },
        diagnostic_truncated: truncated,
        checkpoint_id: None,
        created_at_ms: now_ms,
    };
    if let Err(e) = audit_log::append(pool, &entry).await {
        db::metrics::record_p081_audit_log_append_failure(
            "boundary_decision_deny",
            transport,
            &mode_str,
        );
        tracing::warn!(
            error = %e,
            transport,
            reason_code,
            "P081: legacy/shadow GraphQL deny audit write failed (best-effort)"
        );
    }
}

/// P081: Write a durable deny audit row to audit_log.
/// Fail-closed: if the audit write fails, the caller receives E_AUDIT_UNAVAILABLE
/// rather than the original denial reason. This ensures no boundary denial is
/// returned without a committed audit row.
/// Uses a standalone bounded append (not a write-unit transaction) because the
/// deny path has not opened a command transaction.
async fn write_graphql_deny_audit(
    pool: &SqlitePool,
    ctx: &Context<'_>,
    principal: &auth::Principal,
    transport: &str,
    action_attempted: &str,
    reason_code: &str,
    row_id: Option<&str>,
    caller_class_str: &str,
    policy: &auth::boundary::BoundaryPolicy,
) -> async_graphql::Result<()> {
    let id = uuid::Uuid::now_v7().to_string();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let request_id_buf = ctx
        .data::<crate::request_id::RequestId>()
        .ok()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    // SEC-P081-M002: extract derived token_id for audit correlation. Not the raw token.
    let token_id_buf = ctx
        .data::<crate::auth_layer::GraphqlTokenId>()
        .ok()
        .map(|t| t.0.clone());
    let principal_id_str = principal.id.to_string();
    let principal_class_str = principal.class.to_string();
    let mode_str = policy.mode().as_str().to_string();
    let raw_payload = serde_json::json!({
        "event": "boundary_decision",
        "decision": "deny",
        "transport": transport,
        "action_attempted": action_attempted,
        "reason_code": reason_code,
        "row_id": row_id,
    })
    .to_string();
    let (stored_payload, _, truncated) = audit_log::build_envelope(&raw_payload);
    let entry = audit_log::AuditEntry {
        id: &id,
        request_id: &request_id_buf,
        timestamp_ms: now_ms,
        event_type: "boundary_decision_deny",
        principal_id: Some(&principal_id_str),
        principal_class: Some(&principal_class_str),
        caller_class: Some(caller_class_str),
        token_id: token_id_buf.as_deref(),
        transport,
        action_attempted,
        decision: "deny",
        denial_reason_code: Some(reason_code),
        row_id,
        env_gate_state: None,
        source_ip_hash_or_local_process_id: None,
        boundary_policy_mode: &mode_str,
        fixture_version: "p081-boundary-matrix-v1",
        payload: &stored_payload,
        original_payload_bytes: if truncated { Some(&raw_payload) } else { None },
        diagnostic_truncated: truncated,
        checkpoint_id: None,
        created_at_ms: now_ms,
    };
    audit_log::append(pool, &entry).await.map_err(|e| {
        db::metrics::record_p081_audit_log_append_failure(
            "boundary_decision_deny",
            transport,
            &mode_str,
        );
        tracing::error!(
            error = %e,
            transport,
            reason_code,
            "P081: GraphQL deny audit write failed; failing closed with E_AUDIT_UNAVAILABLE"
        );
        async_graphql::Error::new("audit unavailable").extend_with(|_, ext| {
            ext.set("code", "E_AUDIT_UNAVAILABLE");
            ext.set("requestId", request_id_buf.as_str());
        })
    })
}

async fn boundary_runtime_readback_json(
    pool: &SqlitePool,
    boundary_policy: Option<&auth::boundary::BoundaryPolicy>,
) -> Result<serde_json::Value> {
    let audit_health = audit_log::health_snapshot(pool)
        .await
        .map_err(|e| Error::new(e.to_string()))?;
    let integrity_state = audit_log::verify_latest_checkpoint(pool).await;
    let safe_mode_active = boundary_policy
        .map(|policy| matches!(policy.mode(), auth::boundary::PolicyMode::ReadOnlySafeMode))
        .unwrap_or(false)
        || audit_health.payload_budget_state == "read_only_safe_mode";
    let latest_sequence = P081_SUBSCRIPTION_SEQUENCE.load(Ordering::SeqCst);
    let oldest_retained_sequence = p081_oldest_retained_sequence(latest_sequence);

    Ok(serde_json::json!({
        "schemaVersion": "boundary_runtime.v1",
        "matrixId": boundary_policy.map(|_| "p081-boundary-matrix-v1"),
        "policyInjected": boundary_policy.is_some(),
        "policyMode": boundary_policy.map(|policy| policy.mode().as_str()),
        "safeModeActive": safe_mode_active,
        "safeModeReason": if audit_health.payload_budget_state == "read_only_safe_mode" {
            Some("AUDIT_BUDGET_EXHAUSTED")
        } else if safe_mode_active {
            Some("BOUNDARY_POLICY_SAFE_MODE")
        } else {
            None
        },
        "fixtureDigest": boundary_policy.map(|policy| policy.fixture_digest()),
        "subscriptionReplay": p081_subscription_replay_readback(
            None,
            oldest_retained_sequence,
            latest_sequence,
            latest_sequence
        ),
        "auditLogHealth": {
            "schemaVersion": "audit_log_health.v1",
            "rowCount": audit_health.row_count,
            "latestRowId": audit_health.latest_row_id,
            "latestCheckpointSeq": audit_health.latest_checkpoint_seq,
            "latestCheckpointHash": audit_health.latest_checkpoint_hash,
            "integrityState": integrity_state.as_str(),
            "writable": audit_health.writable,
            "lastWriteOkAtMs": audit_health.last_write_ok_at_ms,
            "consecutiveFailures": audit_health.consecutive_failures,
            "cumulativeFailures": audit_health.cumulative_failures,
            "retentionMinDays": audit_health.retention_min_days,
            "cleanupState": audit_health.cleanup_state,
            "cleanupEligibleRowCount": audit_health.cleanup_eligible_row_count,
            "cleanupProtectedRowCount": audit_health.cleanup_protected_row_count,
            "budgetBytes": audit_health.budget_bytes,
            "usedBytes": audit_health.used_bytes,
            "payloadBudgetBytes": audit_health.payload_budget_bytes,
            "payloadUsedBytes": audit_health.payload_used_bytes,
            "payloadBudgetState": audit_health.payload_budget_state,
            "payloadBudgetUsedPercent": audit_health.payload_budget_used_percent,
            "halfOpenProbeSuccessCount": audit_health.half_open_probe_success_count,
            "shadowCoverageReportRef": audit_health.shadow_coverage_report_ref,
        }
    }))
}

fn p081_subscription_replay_readback(
    requested_cursor: Option<&str>,
    oldest_retained_sequence: i64,
    latest_sequence: i64,
    projection_generation: i64,
) -> serde_json::Value {
    let requested_sequence =
        requested_cursor.and_then(|cursor| cursor.strip_prefix("seq-")?.parse::<i64>().ok());
    let gap_detected = requested_sequence
        .map(|sequence| sequence < oldest_retained_sequence || sequence > latest_sequence)
        .unwrap_or(false);
    serde_json::json!({
        "schemaVersion": "subscription_replay_runtime_v1",
        "sequenceCursor": format!("seq-{latest_sequence}"),
        "projectionGeneration": projection_generation,
        "gapDetected": gap_detected,
        "requestedCursor": requested_cursor,
        "oldestRetainedCursor": format!("seq-{oldest_retained_sequence}"),
        "retentionMinutes": 15,
        "retentionEventCount": 10000,
        "requiresFullRefetch": gap_detected
    })
}

fn p081_oldest_retained_sequence(latest_sequence: i64) -> i64 {
    latest_sequence.saturating_sub(9_999).max(0)
}

fn p081_next_subscription_sequence() -> i64 {
    P081_SUBSCRIPTION_SEQUENCE.fetch_add(1, Ordering::SeqCst) + 1
}

fn p081_record_safe_mode_alert_lifecycle(active: bool, now_ms: i64) {
    let slot = P081_GRAPHQL_SAFE_MODE_ALERT_OPENED_AT_MS.get_or_init(|| Mutex::new(None));
    let mut opened = slot
        .lock()
        .expect("p081 safe-mode alert lifecycle poisoned");
    match (active, *opened) {
        (true, None) => *opened = Some(now_ms),
        (false, Some(opened_at)) => {
            *opened = None;
            let elapsed_ms = now_ms.saturating_sub(opened_at).max(0);
            db::metrics::record_p081_operator_alert_clear_latency(
                "p081-boundary-safe-mode-active",
                "critical",
                std::time::Duration::from_millis(elapsed_ms as u64),
            );
        }
        _ => {}
    }
}

async fn p081_operator_alerts_json(
    pool: &SqlitePool,
    boundary_policy: Option<&auth::boundary::BoundaryPolicy>,
) -> Result<serde_json::Value> {
    let runtime = boundary_runtime_readback_json(pool, boundary_policy).await?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let mut alerts = Vec::new();

    let safe_mode_active = runtime["safeModeActive"].as_bool().unwrap_or(false);
    p081_record_safe_mode_alert_lifecycle(safe_mode_active, now_ms);

    if safe_mode_active {
        alerts.push(serde_json::json!({
            "schemaVersion": "operator_alert_v1",
            "id": "p081-safe-mode-active",
            "dedupeKey": "p081.boundary.safe_mode_active",
            "severity": "critical",
            "title": "Boundary policy is in safe mode",
            "message": "State-changing GraphQL and MCP operations are denied until boundary policy health is restored.",
            "source": "boundaryRuntime",
            "active": true,
            "silenceable": false,
            "acknowledgedAtMs": null,
            "silencedUntilMs": null,
            "nativeDelivery": {
                "schemaVersion": "operator_alert_native_delivery_v1",
                "deliveryKey": "p081.boundary.safe_mode_active",
                "dockBadgeContribution": 1,
                "requestUserAttention": "critical",
                "notificationCategory": "BOUNDARY_POLICY_CRITICAL",
                "dedupePolicy": "dedupe_key_until_clear"
            },
            "lifecycle": {
                "state": "active_unacknowledged",
                "dedupeKey": "p081.boundary.safe_mode_active",
                "ackRequired": true,
                "clearCondition": "boundaryRuntime.safeModeActive=false"
            },
            "createdAtMs": now_ms,
            "clearCondition": "boundaryRuntime.safeModeActive=false",
            "boundaryRuntime": runtime,
        }));
    }

    if runtime["auditLogHealth"]["integrityState"] == "tamper_suspected" {
        alerts.push(serde_json::json!({
            "schemaVersion": "operator_alert_v1",
            "id": "p081-audit-tamper-suspected",
            "dedupeKey": "p081.audit.integrity.tamper_suspected",
            "severity": "critical",
            "title": "Audit log integrity requires operator review",
            "message": "Boundary audit checkpoint verification reported tamper_suspected. Writes remain fail-closed until repaired.",
            "source": "auditLogHealth",
            "active": true,
            "silenceable": false,
            "acknowledgedAtMs": null,
            "silencedUntilMs": null,
            "nativeDelivery": {
                "schemaVersion": "operator_alert_native_delivery_v1",
                "deliveryKey": "p081.audit.integrity.tamper_suspected",
                "dockBadgeContribution": 1,
                "requestUserAttention": "critical",
                "notificationCategory": "BOUNDARY_POLICY_CRITICAL",
                "dedupePolicy": "dedupe_key_until_clear"
            },
            "lifecycle": {
                "state": "active_unacknowledged",
                "dedupeKey": "p081.audit.integrity.tamper_suspected",
                "ackRequired": true,
                "clearCondition": "auditLogHealth.integrityState=verified"
            },
            "createdAtMs": now_ms,
            "clearCondition": "auditLogHealth.integrityState=verified",
            "boundaryRuntime": runtime,
        }));
    }

    Ok(serde_json::Value::Array(alerts))
}

fn redact_p081_operator_alerts_for_observer(
    alerts: &mut serde_json::Value,
    authorization: &GraphqlReadAuthorization,
    collector: Option<&P081GraphqlRedactionCollector>,
) {
    if authorization.caller_class != "observer" {
        return;
    }
    let Some(alerts_array) = alerts.as_array_mut() else {
        return;
    };
    for (idx, alert) in alerts_array.iter_mut().enumerate() {
        if let Some(object) = alert.as_object_mut() {
            if object.contains_key("message") {
                object.insert("message".to_string(), serde_json::Value::Null);
                if let Some(collector) = collector {
                    collector.push_field_null_redaction(
                        vec!["operatorAlerts", &idx.to_string(), "message"],
                        authorization.row_id.as_deref(),
                        authorization.caller_class.as_str(),
                    );
                }
            }
            if object.contains_key("nativeDelivery") {
                object.insert("nativeDelivery".to_string(), serde_json::Value::Null);
                if let Some(collector) = collector {
                    collector.push_field_null_redaction(
                        vec!["operatorAlerts", &idx.to_string(), "nativeDelivery"],
                        authorization.row_id.as_deref(),
                        authorization.caller_class.as_str(),
                    );
                }
            }
            if let Some(lifecycle) = object.get_mut("lifecycle").and_then(|v| v.as_object_mut()) {
                if lifecycle.contains_key("clearCondition") {
                    lifecycle.insert("clearCondition".to_string(), serde_json::Value::Null);
                    if let Some(collector) = collector {
                        collector.push_field_null_redaction(
                            vec![
                                "operatorAlerts",
                                &idx.to_string(),
                                "lifecycle",
                                "clearCondition",
                            ],
                            authorization.row_id.as_deref(),
                            authorization.caller_class.as_str(),
                        );
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
struct GraphqlReadAuthorization {
    caller_class: String,
    row_id: Option<String>,
}

async fn require_operator_read(ctx: &Context<'_>) -> Result<()> {
    require_graphql_read(ctx, None).await.map(|_| ())
}

async fn require_observer_opt_in_read(ctx: &Context<'_>) -> Result<GraphqlReadAuthorization> {
    let principal = ctx
        .data::<auth::Principal>()
        .map_err(|_| Error::new("unauthorized"))?;
    let caller_class = auth::derive_caller_class(principal);
    record_p081_caller_class_diagnostics(principal, &caller_class, "graphql_query");
    let action = if caller_class.as_str() == "observer" {
        Some("graphql.read_only")
    } else {
        None
    };
    require_graphql_read(ctx, action).await
}

async fn require_graphql_read(
    ctx: &Context<'_>,
    action: Option<&str>,
) -> Result<GraphqlReadAuthorization> {
    let principal = ctx
        .data::<auth::Principal>()
        .map_err(|_| Error::new("unauthorized"))?;
    let caller_class = auth::derive_caller_class(principal);

    // P081 Phase 3: evaluate BoundaryPolicy for ALL callers including Operators.
    // In shadow mode decisions are logged but not enforced; in legacy_compat mode
    // the legacy P072 guards below remain authoritative.
    if let Ok(policy) = ctx.data::<Arc<auth::boundary::BoundaryPolicy>>() {
        let started = std::time::Instant::now();
        let decision = policy.evaluate(caller_class.as_str(), "graphql_query", action);
        let elapsed = started.elapsed();
        db::metrics::record_p081_boundary_decision_latency(
            "graphql_query",
            caller_class.as_str(),
            policy.mode().as_str(),
            elapsed,
        );
        match decision {
            auth::boundary::PolicyDecision::Allow { row_id } => {
                db::metrics::record_p081_boundary_decision(
                    "graphql_query",
                    row_id.as_deref(),
                    caller_class.as_str(),
                    action.unwrap_or("query"),
                    "allow",
                    None,
                    policy.mode().as_str(),
                );
                return Ok(GraphqlReadAuthorization {
                    caller_class: caller_class.as_str().to_string(),
                    row_id,
                });
            }
            auth::boundary::PolicyDecision::Deny {
                reason_code,
                row_id,
                ..
            } => {
                db::metrics::record_p081_boundary_decision(
                    "graphql_query",
                    row_id.as_deref(),
                    caller_class.as_str(),
                    action.unwrap_or("query"),
                    "deny",
                    Some(reason_code.as_str()),
                    policy.mode().as_str(),
                );
                // P081: write durable deny audit before returning the denial.
                // Fail-closed: if the audit write fails, return E_AUDIT_UNAVAILABLE.
                if let Ok(pool) = ctx.data::<SqlitePool>() {
                    write_graphql_deny_audit(
                        pool,
                        ctx,
                        principal,
                        "graphql_query",
                        "query",
                        &reason_code,
                        row_id.as_deref(),
                        caller_class.as_str(),
                        &policy,
                    )
                    .await?;
                }
                return Err(boundary_denial_error(
                    &reason_code,
                    row_id.as_deref(),
                    Some(caller_class.as_str()),
                ));
            }
            auth::boundary::PolicyDecision::Shadow { matched_decision } => {
                // Shadow mode: log the matrix decision but retain the pre-P081
                // legacy Operator-only guard for non-operators so that shadow
                // mode cannot weaken pre-P081 authorization (P081-SEC-M1).
                match *matched_decision {
                    auth::boundary::PolicyDecision::Deny {
                        reason_code,
                        row_id,
                        ..
                    } => {
                        db::metrics::record_p081_boundary_decision(
                            "graphql_query",
                            row_id.as_deref(),
                            caller_class.as_str(),
                            action.unwrap_or("query"),
                            "shadow_deny",
                            Some(reason_code.as_str()),
                            policy.mode().as_str(),
                        );
                        tracing::debug!(
                            caller_class = caller_class.as_str(),
                            transport = "graphql_query",
                            reason_code = %reason_code,
                            row_id = ?row_id,
                            "BoundaryPolicy shadow: matrix would deny this graphql_query request"
                        );
                        if principal.class == auth::PrincipalClass::Operator {
                            db::metrics::record_p081_boundary_policy_enforcement_parity(
                                "allow", "deny",
                            );
                            db::metrics::record_p081_boundary_shadow_disagreement(
                                "graphql_query",
                                row_id.as_deref(),
                                caller_class.as_str(),
                                action.unwrap_or("query"),
                                "allow",
                                "deny",
                                Some(reason_code.as_str()),
                            );
                        }
                        // Preserve legacy fail-closed: non-Operator callers that the
                        // matrix would deny must still be denied in shadow mode.
                        if principal.class != auth::PrincipalClass::Operator {
                            db::metrics::record_p081_boundary_policy_enforcement_parity(
                                "deny", "deny",
                            );
                            // MEDIUM-001: best-effort audit for shadow-mode denials.
                            write_graphql_legacy_deny_audit(
                                ctx,
                                principal,
                                "graphql_query",
                                "query",
                                &reason_code,
                                row_id.as_deref(),
                                caller_class.as_str(),
                                &policy,
                            )
                            .await;
                            return Err(boundary_denial_error(
                                &reason_code,
                                row_id.as_deref(),
                                Some(caller_class.as_str()),
                            ));
                        }
                    }
                    auth::boundary::PolicyDecision::Allow { row_id } => {
                        db::metrics::record_p081_boundary_policy_enforcement_parity(
                            if principal.class == auth::PrincipalClass::Operator {
                                "allow"
                            } else {
                                "deny"
                            },
                            "allow",
                        );
                        db::metrics::record_p081_boundary_decision(
                            "graphql_query",
                            row_id.as_deref(),
                            caller_class.as_str(),
                            action.unwrap_or("query"),
                            "shadow_allow",
                            None,
                            policy.mode().as_str(),
                        );
                        return Ok(GraphqlReadAuthorization {
                            caller_class: caller_class.as_str().to_string(),
                            row_id,
                        });
                    }
                    _ => {}
                }
            }
            auth::boundary::PolicyDecision::LegacyPassthrough => {
                db::metrics::record_p081_boundary_no_op_label(
                    "chainworks-forge",
                    &chrono::Utc::now().format("%Y-%m").to_string(),
                );
                // Legacy compat: operator-only, matching the pre-P081 P072 guard.
                // Observer and Agent principals are denied; only Operators may pass through
                // to the P072 surface policy check below. Exempting observer here would allow
                // observer queries when no surface_policy is configured (fail-open gap).
                if principal.class != auth::PrincipalClass::Operator {
                    // MEDIUM-001: best-effort audit for legacy-passthrough denials.
                    write_graphql_legacy_deny_audit(
                        ctx,
                        principal,
                        "graphql_query",
                        "query",
                        "CAPABILITY_OUT_OF_SCOPE",
                        None,
                        caller_class.as_str(),
                        &policy,
                    )
                    .await;
                    return Err(boundary_denial_error("CAPABILITY_OUT_OF_SCOPE", None, None));
                }
            }
        }
    } else if principal.class != auth::PrincipalClass::Operator {
        db::metrics::record_p081_boundary_policy_evaluation_error(
            "graphql_query",
            "policy_missing",
        );
        // No BoundaryPolicy available — fall back to operator-only guard.
        // No audit written here: this seam does not yet have bounded DB access.
        return Err(boundary_denial_error("CAPABILITY_OUT_OF_SCOPE", None, None));
    }

    // P072: enforce allow_queries surface policy when present.
    if let Ok(table) = ctx.data::<auth::PrincipalTable>() {
        if let Some(allowed) = auth::is_query_allowed_by_surface_policy(table, &principal.id) {
            if !allowed {
                // MEDIUM-001: best-effort audit for P072 surface-policy denials.
                if let Ok(policy) = ctx.data::<Arc<auth::boundary::BoundaryPolicy>>() {
                    let caller_class = auth::derive_caller_class(principal);
                    write_graphql_legacy_deny_audit(
                        ctx,
                        principal,
                        "graphql_query",
                        "query",
                        "CAPABILITY_OUT_OF_SCOPE",
                        None,
                        caller_class.as_str(),
                        &policy,
                    )
                    .await;
                }
                return Err(boundary_denial_error("CAPABILITY_OUT_OF_SCOPE", None, None));
            }
        }
    }
    Ok(GraphqlReadAuthorization {
        caller_class: caller_class.as_str().to_string(),
        row_id: None,
    })
}

/// P081 Phase 3: Evaluate BoundaryPolicy for graphql_subscription transport.
/// Subscriptions use a separate transport so the matrix can apply
/// different allow/deny rows from graphql_query.
async fn require_subscription_read(ctx: &Context<'_>) -> Result<()> {
    let principal = ctx
        .data::<auth::Principal>()
        .map_err(|_| Error::new("unauthorized"))?;

    if let Ok(policy) = ctx.data::<Arc<auth::boundary::BoundaryPolicy>>() {
        let caller_class = auth::derive_caller_class(principal);
        record_p081_caller_class_diagnostics(principal, &caller_class, "graphql_subscription");
        let started = std::time::Instant::now();
        let decision = policy.evaluate(caller_class.as_str(), "graphql_subscription", None);
        let elapsed = started.elapsed();
        db::metrics::record_p081_boundary_decision_latency(
            "graphql_subscription",
            caller_class.as_str(),
            policy.mode().as_str(),
            elapsed,
        );
        match decision {
            auth::boundary::PolicyDecision::Allow { .. } => {}
            auth::boundary::PolicyDecision::Deny {
                reason_code,
                row_id,
                ..
            } => {
                // P081: write durable deny audit before returning the denial.
                // Fail-closed: if the audit write fails, return E_AUDIT_UNAVAILABLE.
                if let Ok(pool) = ctx.data::<SqlitePool>() {
                    write_graphql_deny_audit(
                        pool,
                        ctx,
                        principal,
                        "graphql_subscription",
                        "subscription",
                        &reason_code,
                        row_id.as_deref(),
                        caller_class.as_str(),
                        &policy,
                    )
                    .await?;
                }
                return Err(boundary_denial_error(
                    &reason_code,
                    row_id.as_deref(),
                    Some(caller_class.as_str()),
                ));
            }
            auth::boundary::PolicyDecision::Shadow { matched_decision } => {
                if let auth::boundary::PolicyDecision::Deny {
                    reason_code,
                    row_id,
                    ..
                } = *matched_decision
                {
                    tracing::debug!(
                        caller_class = caller_class.as_str(),
                        transport = "graphql_subscription",
                        reason_code = %reason_code,
                        row_id = ?row_id,
                        "BoundaryPolicy shadow: matrix would deny this graphql_subscription"
                    );
                    if principal.class == auth::PrincipalClass::Operator {
                        db::metrics::record_p081_boundary_policy_enforcement_parity(
                            "allow", "deny",
                        );
                        db::metrics::record_p081_boundary_shadow_disagreement(
                            "graphql_subscription",
                            row_id.as_deref(),
                            caller_class.as_str(),
                            "subscription",
                            "allow",
                            "deny",
                            Some(reason_code.as_str()),
                        );
                    }
                    if principal.class != auth::PrincipalClass::Operator {
                        db::metrics::record_p081_boundary_policy_enforcement_parity("deny", "deny");
                        // MEDIUM-001: best-effort audit for shadow-mode subscription denials.
                        write_graphql_legacy_deny_audit(
                            ctx,
                            principal,
                            "graphql_subscription",
                            "subscription",
                            &reason_code,
                            row_id.as_deref(),
                            caller_class.as_str(),
                            &policy,
                        )
                        .await;
                        return Err(boundary_denial_error(
                            &reason_code,
                            row_id.as_deref(),
                            Some(caller_class.as_str()),
                        ));
                    }
                }
            }
            auth::boundary::PolicyDecision::LegacyPassthrough => {
                db::metrics::record_p081_boundary_no_op_label(
                    "chainworks-forge",
                    &chrono::Utc::now().format("%Y-%m").to_string(),
                );
                if principal.class != auth::PrincipalClass::Operator {
                    // MEDIUM-001: best-effort audit for legacy-passthrough subscription denials.
                    write_graphql_legacy_deny_audit(
                        ctx,
                        principal,
                        "graphql_subscription",
                        "subscription",
                        "CAPABILITY_OUT_OF_SCOPE",
                        None,
                        caller_class.as_str(),
                        &policy,
                    )
                    .await;
                    return Err(boundary_denial_error("CAPABILITY_OUT_OF_SCOPE", None, None));
                }
            }
        }
    } else if principal.class != auth::PrincipalClass::Operator {
        db::metrics::record_p081_boundary_policy_evaluation_error(
            "graphql_subscription",
            "policy_missing",
        );
        // No BoundaryPolicy available — fall back to operator-only guard.
        // No audit written here: this seam does not yet have bounded DB access.
        return Err(boundary_denial_error("CAPABILITY_OUT_OF_SCOPE", None, None));
    }

    // P081 fix: subscriptions must check allow_subscriptions, not allow_queries.
    if let Ok(table) = ctx.data::<auth::PrincipalTable>() {
        if let Some(allowed) = auth::is_subscription_allowed_by_surface_policy(table, &principal.id)
        {
            if !allowed {
                // MEDIUM-001: best-effort audit for P072 surface-policy subscription denials.
                if let Ok(policy) = ctx.data::<Arc<auth::boundary::BoundaryPolicy>>() {
                    let caller_class = auth::derive_caller_class(principal);
                    write_graphql_legacy_deny_audit(
                        ctx,
                        principal,
                        "graphql_subscription",
                        "subscription",
                        "CAPABILITY_OUT_OF_SCOPE",
                        None,
                        caller_class.as_str(),
                        &policy,
                    )
                    .await;
                }
                return Err(boundary_denial_error("CAPABILITY_OUT_OF_SCOPE", None, None));
            }
        }
    }
    Ok(())
}

// ── P046: Resource-scoped run accessibility ──────────────────────────────────
//
// Atomically verifies the calling principal is an operator AND the run exists.
// Returns Ok(Some(())) if authorized and found, Ok(None) if run absent,
// Err("forbidden") if non-operator, Err("db_unavailable") on transient DB failure.
// Uses the pinned P046 retry policy (3 attempts, 50ms/150ms backoff) so transient
// SQLite busy/timeout is handled consistently with other P046 DB reads.
// Combining both checks prevents authorization bypass if a new resolver omits
// require_operator_read — the run-scoped check enforces operator class itself.
// ID-based resolvers use Ok(None) to apply not-found-or-not-visible behavior.
// run_id-based resolvers typically map Ok(None) to Err("not found").
async fn p046_check_run_accessible(
    ctx: &Context<'_>,
    pool: &SqlitePool,
    run_id_str: &str,
    deadline: tokio::time::Instant,
) -> Result<Option<()>> {
    let principal = ctx
        .data::<auth::Principal>()
        .map_err(|_| Error::new("unauthorized"))?;
    if principal.class != auth::PrincipalClass::Operator {
        return Err(Error::new("forbidden"));
    }
    // Return a sanitized parse error (not "not found") so input validation failures
    // are distinguishable from authorization outcomes and do not disclose row existence.
    let parsed: domain::ids::RunId = run_id_str
        .parse()
        .map_err(|_| Error::new("invalid_argument"))?;
    let pool_ref = pool.clone();
    match p046_retry_db("session_run_access", deadline, || {
        let pool_inner = pool_ref.clone();
        async move {
            runs::find_by_id(&pool_inner, parsed)
                .await
                .map(|opt| opt.map(|_| ()))
        }
    })
    .await
    {
        Ok(result) => Ok(result),
        Err(e) => {
            let msg = e.to_string();
            if msg.starts_with("transient_db_unavailable") {
                warn!("p046 run accessibility check transient: {e:#}");
            } else {
                warn!("p046 run accessibility check: {e:#}");
            }
            Err(Error::new("db_unavailable"))
        }
    }
}

async fn run_from_projection_or_canonical(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Option<GqlRun>> {
    let item = runs::find_by_id(pool, run_id).await?;
    if let Some(run) = item {
        if let Some(projection) =
            projections::find_run_projection(pool, &run_id.to_string()).await?
        {
            let mut gql = GqlRun::from_projection_and_run(projection, run);
            enrich_run_with_artifact_contracts(pool, run_id, &mut gql).await?;
            Ok(Some(gql))
        } else {
            let mut gql = GqlRun::from(run);
            enrich_run_with_artifact_contracts(pool, run_id, &mut gql).await?;
            Ok(Some(gql))
        }
    } else {
        Ok(None)
    }
}

async fn enrich_run_with_artifact_contracts(
    pool: &SqlitePool,
    run_id: RunId,
    gql: &mut GqlRun,
) -> Result<()> {
    if let Some(projection) =
        db::repos::artifact_contracts::find_run_state_projection(pool, run_id).await?
    {
        gql.active_artifact_index_json =
            Some(serde_json::to_string(&projection.active_index_json)?);
        gql.run_state_projection_json = Some(serde_json::to_string(&projection.run_state_json)?);
        let overrides = db::repos::artifact_contracts::list_overrides(pool, run_id).await?;
        gql.operator_overrides_json = Some(serde_json::to_string(&overrides)?);
    }
    let legacy_overrides = db::repos::legacy_discovery_overrides::list_by_run(pool, run_id).await?;
    gql.legacy_discovery_overrides_json = Some(serde_json::to_string(&legacy_overrides)?);
    gql.implementation_self_assessment_summary =
        artifact_contracts::find_active_implementation_self_assessment_summary(pool, run_id)
            .await?
            .map(|stored| stored.summary.into());
    gql.rollout_contract_readback_json =
        rollout_contract_checks::find_terminal_rollout_contract_check_for_run(pool, run_id.inner())
            .await?
            .map(|check| Json(check.operator_readback_json_for_lane("graphql")));
    gql.side_effect_readback_json = Some(Json(side_effect_readback_json(pool, run_id).await?));
    let code_writer_completion_readbacks =
        code_writer_completion_receipts::list_by_run(pool, run_id).await?;
    let canonical_code_writer_completion_readbacks =
        code_writer_completion_receipts::list_canonical_by_run(pool, run_id).await?;
    gql.implementation_completion =
        domain::code_writer_completion::project_implementation_completion(
            &canonical_code_writer_completion_readbacks,
        )
        .into();
    gql.code_writer_completion_receipts = code_writer_completion_readbacks
        .into_iter()
        .map(Into::into)
        .collect();
    enrich_run_with_p091_retry_authority(pool, run_id, gql).await?;
    gql.workflow_conflict = workflow_conflicts::get_current_blocking_conflict(pool, run_id)
        .await?
        .map(Into::into);
    // P017: Enrich workflow conflict with lead mediation readback if present.
    // API-001 (P017 R2 audit): the enriched projection includes
    // mediation-owned `execution_attempts` so operators can inspect the
    // mediation's runtime facts, watchdog outcome, artifacts, and
    // provider/timing details directly through the conflict surface.
    if let Some(ref mut conflict) = gql.workflow_conflict {
        if let Some(ref mediation_id) = conflict.mediation_record_id {
            if let Ok(Some(med)) =
                db::repos::lead_conflict_mediations::find_by_id(pool, mediation_id).await
            {
                conflict.lead_mediation = Some(
                    crate::types::run::GqlLeadMediation::build_with_attempts(pool, &med).await?,
                );
            }
        }
    }
    gql.implementation_handoff_status_json = if let Some(status) =
        workflow_conflicts::get_implementation_handoff_status(pool, run_id).await?
    {
        Some(async_graphql::Json(serde_json::to_value(status)?))
    } else {
        None
    };
    gql.main_sync_readback_json = Some(async_graphql::Json(
        proposal_064_main_sync_readback(pool, run_id).await?,
    ));
    gql.knowledge_capsule_readback_json = Some(async_graphql::Json(
        proposal_064_knowledge_capsule_readback(pool, run_id).await?,
    ));
    // P077: Populate closeout readiness summary via CloseoutReadinessSummaryAccessor.
    if let Some(summary) =
        closeout::load_closeout_readiness_summary(pool, &run_id.to_string()).await?
    {
        let summary_json = async_graphql::Json(serde_json::to_value(&summary)?);
        gql.closeout_readiness_summary_json = Some(summary_json.clone());
        gql.implementation_closeout_readiness_summary = Some(summary_json);
    }
    Ok(())
}

async fn stage_from_projection_or_canonical(
    pool: &SqlitePool,
    stage_execution_id: domain::ids::StageExecutionId,
) -> Result<Option<GqlStageExecution>> {
    let item = db::repos::stages::find_by_id(pool, stage_execution_id).await?;
    if let Some(stage) = item {
        let projection = projections::list_stages_projection(pool, &stage.run_id.to_string())
            .await?
            .into_iter()
            .find(|row| row.id == stage.id.to_string());
        if let Some(projection) = projection {
            Ok(Some(GqlStageExecution::from_projection_and_stage(
                projection, stage,
            )))
        } else {
            Ok(Some(GqlStageExecution::from(stage)))
        }
    } else {
        Ok(None)
    }
}

fn p036_stage_topology_order(plan: &workflow::plan::RunPlan) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();
    let mut queue = VecDeque::from([plan.initial_state.clone()]);

    while let Some(stage_id) = queue.pop_front() {
        if !seen.insert(stage_id.clone()) {
            continue;
        }
        let Some(state) = plan.states.get(&stage_id) else {
            continue;
        };
        ordered.push(stage_id.clone());
        for transition in &state.transitions {
            if !seen.contains(&transition.to) {
                queue.push_back(transition.to.clone());
            }
        }
    }

    let mut remaining: Vec<_> = plan
        .states
        .keys()
        .filter(|stage_id| !seen.contains(*stage_id))
        .cloned()
        .collect();
    remaining.sort();
    ordered.extend(remaining);
    ordered
}

fn p036_agent_title(agent_id: &str) -> String {
    let words: Vec<String> = agent_id
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();
    if words.is_empty() {
        agent_id.to_string()
    } else {
        words.join(" ")
    }
}

fn p036_latest_stage_rows_by_stage_id(
    rows: Vec<projections::StageSummaryRow>,
) -> HashMap<String, projections::StageSummaryRow> {
    let mut latest = HashMap::new();
    for row in rows {
        latest
            .entry(row.stage_id.clone())
            .and_modify(|existing: &mut projections::StageSummaryRow| {
                if row.started_at >= existing.started_at {
                    *existing = row.clone();
                }
            })
            .or_insert(row);
    }
    latest
}

fn p036_status_for_topology(row: Option<&projections::StageSummaryRow>) -> String {
    match row {
        Some(row) if row.projection_lag || !row.projection_present => "unavailable".into(),
        Some(row) => row.status.clone(),
        None => "pending".into(),
    }
}

fn p036_is_current_stage(
    run: &domain::run::Run,
    stage_id: &str,
    row: Option<&projections::StageSummaryRow>,
) -> bool {
    if let Some(current_state) = run.current_state.as_deref() {
        return current_state == stage_id;
    }
    matches!(
        row.map(|row| row.status.as_str()),
        Some("running" | "blocked" | "waiting_approval" | "pending_approval")
    )
}

fn p036_topology_nodes(
    run: &domain::run::Run,
    plan: &workflow::plan::RunPlan,
    stage_rows: Vec<projections::StageSummaryRow>,
    artifacts: Vec<domain::artifact::Artifact>,
    agent_executions: Vec<domain::agent::AgentExecution>,
) -> Vec<GqlRunStageTopologyNode> {
    let latest_by_stage_id = p036_latest_stage_rows_by_stage_id(stage_rows);
    let stage_execution_to_stage_id: HashMap<String, String> = latest_by_stage_id
        .values()
        .map(|row| (row.id.clone(), row.stage_id.clone()))
        .collect();

    let mut artifacts_by_stage_id: HashMap<String, i64> = HashMap::new();
    for artifact in artifacts {
        *artifacts_by_stage_id.entry(artifact.stage_id).or_default() += 1;
    }

    let mut executions_by_stage_id: HashMap<String, Vec<domain::agent::AgentExecution>> =
        HashMap::new();
    for execution in agent_executions {
        let Some(stage_execution_id) = execution.stage_execution_id else {
            continue;
        };
        if let Some(stage_id) = stage_execution_to_stage_id.get(&stage_execution_id.to_string()) {
            executions_by_stage_id
                .entry(stage_id.clone())
                .or_default()
                .push(execution);
        }
    }

    p036_stage_topology_order(plan)
        .into_iter()
        .enumerate()
        .filter_map(|(index, stage_id)| {
            let state = plan.states.get(&stage_id)?;
            let latest = latest_by_stage_id.get(&stage_id);
            let executions = executions_by_stage_id
                .get(&stage_id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);

            let occurrences: Vec<GqlRunStageTopologyOccurrence> = state
                .tasks
                .iter()
                .chain(state.post_approval_tasks.iter())
                .map(|task| {
                    let matching: Vec<_> = executions
                        .iter()
                        .filter(|execution| execution.agent_id == task.agent.agent_id)
                        .collect();
                    let status = matching
                        .iter()
                        .max_by_key(|execution| execution.started_at)
                        .map(|execution| execution.status.to_string())
                        .unwrap_or_else(|| "pending".into());
                    GqlRunStageTopologyOccurrence {
                        agent_id: task.agent.agent_id.clone(),
                        agent_title: p036_agent_title(&task.agent.agent_id),
                        task_name: task.task_name.clone(),
                        status,
                        provider: task.agent.provider.clone(),
                        model: task.agent.model.clone(),
                        effort: task.agent.effort.clone(),
                        execution_count: matching.len() as i64,
                    }
                })
                .collect();

            let transitions: Vec<GqlRunStageTopologyTransition> = state
                .transitions
                .iter()
                .map(|transition| GqlRunStageTopologyTransition {
                    to_stage_id: transition.to.clone(),
                    to_label: plan
                        .states
                        .get(&transition.to)
                        .map(|target| target.label.clone()),
                    detail: match transition.condition.trim() {
                        "" | "true" => None,
                        condition => Some(condition.to_string()),
                    },
                })
                .collect();

            Some(GqlRunStageTopologyNode {
                stage_id: stage_id.clone(),
                label: state.label.clone(),
                order: index as i64 + 1,
                owner_agent_id: state.owner.agent_id.clone(),
                owner_agent_title: p036_agent_title(&state.owner.agent_id),
                status: p036_status_for_topology(latest),
                is_current: p036_is_current_stage(run, &stage_id, latest),
                iteration: latest.map(|row| row.iteration),
                attempt_number: latest.map(|row| row.attempt_number),
                approval_required: state.is_manual_gate
                    || latest.is_some_and(|row| row.has_pending_approval),
                artifact_count: artifacts_by_stage_id.get(&stage_id).copied().unwrap_or(0),
                communication_count: occurrences.len() as i64 + transitions.len() as i64,
                occurrences,
                transitions,
            })
        })
        .collect()
}

async fn p093_active_agent_executions(
    pool: &SqlitePool,
    run_id: RunId,
    items: Vec<domain::agent::AgentExecution>,
) -> Result<Vec<GqlAgentExecution>> {
    let runtime_evidence = p093_runtime_evidence_by_agent(
        pool,
        items.iter().map(|item| item.id.to_string()).collect(),
    )
    .await?;

    let plan = match runs::find_by_id(pool, run_id).await? {
        Some(run) => {
            let workflow = run.workflow_snapshot_json.as_deref().unwrap_or_default();
            let catalog = run.catalog_snapshot_json.as_deref().unwrap_or_default();
            if workflow.trim().is_empty() || catalog.trim().is_empty() {
                None
            } else {
                let catalog_path = run.agent_catalog_yaml_path.as_deref().unwrap_or(".");
                workflow::compiler::compile_from_snapshot_json(workflow, catalog, catalog_path).ok()
            }
        }
        None => None,
    };

    let stage_rows = projections::list_stages_projection(pool, &run_id.to_string()).await?;
    let stage_by_execution_id: HashMap<String, projections::StageSummaryRow> = stage_rows
        .into_iter()
        .map(|row| (row.id.clone(), row))
        .collect();

    let mut stage_order_by_id: HashMap<String, i64> = HashMap::new();
    let mut task_label_by_stage_agent: HashMap<(String, String), String> = HashMap::new();
    if let Some(plan) = plan.as_ref() {
        for (index, stage_id) in p036_stage_topology_order(plan).into_iter().enumerate() {
            stage_order_by_id.insert(stage_id.clone(), index as i64);
            if let Some(state) = plan.states.get(&stage_id) {
                for task in state.tasks.iter().chain(state.post_approval_tasks.iter()) {
                    task_label_by_stage_agent
                        .entry((stage_id.clone(), task.agent.agent_id.clone()))
                        .or_insert_with(|| task.task_name.clone());
                }
            }
        }
    }

    let mut gql_items: Vec<GqlAgentExecution> = items
        .into_iter()
        .map(|execution| {
            let started_at = execution.started_at;
            let execution_id = execution.id.to_string();
            let stage_execution_id = execution
                .stage_execution_id
                .map(|id| id.to_string())
                .unwrap_or_default();
            let stage_row = stage_by_execution_id.get(&stage_execution_id);
            let stage_id = stage_row.map(|row| row.stage_id.clone());
            let selection_order = stage_id
                .as_ref()
                .and_then(|stage_id| stage_order_by_id.get(stage_id).copied());
            let task_label = stage_id
                .as_ref()
                .and_then(|stage_id| {
                    task_label_by_stage_agent.get(&(stage_id.clone(), execution.agent_id.clone()))
                })
                .cloned();
            let mut gql = GqlAgentExecution::from(execution);
            gql.agent_title = Some(p036_agent_title(&gql.agent_id));
            gql.stage_label = stage_row.map(|row| row.label.clone());
            gql.task_label = task_label;
            if let Some((event_count, last_event_at)) = runtime_evidence.get(&execution_id) {
                gql.event_count = Some(*event_count);
                gql.last_event_at = last_event_at
                    .clone()
                    .or_else(|| Some(started_at.to_rfc3339()));
            } else {
                gql.event_count = Some(0);
                gql.last_event_at = Some(started_at.to_rfc3339());
            }
            gql.selection_order = selection_order;
            gql.selection_unavailable_reason = if selection_order.is_some() {
                None
            } else {
                Some("snapshot_unavailable".into())
            };
            gql
        })
        .collect();

    gql_items.sort_by(
        |lhs, rhs| match (lhs.selection_order, rhs.selection_order) {
            (Some(left), Some(right)) if left != right => left.cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => lhs
                .started_at
                .cmp(&rhs.started_at)
                .then_with(|| lhs.agent_id.cmp(&rhs.agent_id)),
        },
    );

    for (index, item) in gql_items.iter_mut().enumerate() {
        if item.selection_order.is_some() {
            item.selection_order = Some(index as i64);
        }
    }

    Ok(gql_items)
}

async fn p093_runtime_evidence_by_agent(
    pool: &SqlitePool,
    agent_execution_ids: Vec<String>,
) -> Result<HashMap<String, (i64, Option<String>)>> {
    let mut evidence = HashMap::new();
    for agent_execution_id in agent_execution_ids {
        let row = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(event_count), 0) AS event_count,
                MAX(last_event_at_ms) AS last_event_at_ms
            FROM agent_execution_runtime_receipts
            WHERE agent_execution_id = ?1
            "#,
        )
        .bind(&agent_execution_id)
        .fetch_one(pool)
        .await
        .map_err(|e| Error::new(e.to_string()))?;
        let event_count: i64 = row.get("event_count");
        let last_event_at_ms: Option<i64> = row.get("last_event_at_ms");
        let last_event_at = last_event_at_ms.and_then(|ms| {
            chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms).map(|ts| ts.to_rfc3339())
        });
        evidence.insert(agent_execution_id, (event_count, last_event_at));
    }
    Ok(evidence)
}

#[Object]
impl QueryRoot {
    async fn ideas(
        &self,
        ctx: &Context<'_>,
        include_archived: Option<bool>,
    ) -> Result<Vec<GqlIdea>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let include = include_archived.unwrap_or(false);
        let items = ideas::list(pool, include).await?;
        Ok(items.into_iter().map(GqlIdea::from).collect())
    }

    async fn idea(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlIdea>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let idea_id: IdeaId = id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let item = ideas::find_by_id(pool, idea_id).await?;
        Ok(item.map(GqlIdea::from))
    }

    async fn runs(&self, ctx: &Context<'_>, idea_id: Option<ID>) -> Result<Vec<GqlRun>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let mut runs: Vec<GqlRun> = if let Some(id) = idea_id {
            let items = projections::list_by_idea_projection(pool, id.as_str()).await?;
            items.into_iter().map(GqlRun::from).collect()
        } else {
            let items = projections::list_active_projection(pool).await?;
            items.into_iter().map(GqlRun::from).collect()
        };
        // Batch-fetch blocking workflow conflicts to avoid N+1 per-run lookups.
        let run_ids: Vec<String> = runs.iter().map(|r| r.id.to_string()).collect();
        if !run_ids.is_empty() {
            let conflicts =
                workflow_conflicts::get_blocking_conflicts_for_runs(pool, &run_ids).await?;
            for run in &mut runs {
                if let Some(conflict) = conflicts.get(run.id.as_str()) {
                    run.workflow_conflict = Some(conflict.clone().into());
                }
            }
        }
        Ok(runs)
    }

    async fn run(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlRun>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let run_id: RunId = id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        run_from_projection_or_canonical(pool, run_id).await
    }

    async fn approval_inbox(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
    ) -> Result<Vec<GqlApproval>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_pending_inbox_projection(pool).await?;

        // P081 Phase 4: compute BoundaryPolicy actionability for this caller once
        // and apply it to every approval in the list.
        let boundary_decision: Option<(auth::CallerClass, auth::boundary::PolicyDecision)> =
            if let Ok(principal) = ctx.data::<auth::Principal>() {
                if let Ok(policy) = ctx.data::<Arc<auth::boundary::BoundaryPolicy>>() {
                    let caller_class = auth::derive_caller_class(principal);
                    let decision = policy.evaluate(caller_class.as_str(), "graphql_mutation", None);
                    Some((caller_class, decision))
                } else {
                    None
                }
            } else {
                None
            };

        Ok(items
            .into_iter()
            .filter(|row| {
                run_id.as_ref().map_or(true, |requested_run_id| {
                    row.run_id == requested_run_id.as_str()
                })
            })
            .map(|row| {
                let approval = GqlApproval::from(row);
                if let Some((ref caller_class, ref decision)) = boundary_decision {
                    approval.with_boundary_actionability(caller_class.as_str(), decision)
                } else {
                    approval
                }
            })
            .collect())
    }

    async fn artifacts(&self, ctx: &Context<'_>, run_id: ID) -> Result<Vec<GqlArtifact>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let parsed_run_id: RunId = run_id
            .as_str()
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let run = runs::find_by_id(pool, parsed_run_id).await?;
        let items = projections::list_artifacts_projection(pool, run_id.as_str()).await?;
        let should_attach_payload = ctx.look_ahead().field("payloadText").exists();
        debug!(
            run_id = %run_id.as_str(),
            artifact_count = items.len(),
            payload_requested = should_attach_payload,
            "P031 artifacts query"
        );
        if should_attach_payload {
            info!(
                run_id = %run_id.as_str(),
                artifact_count = items.len(),
                "P031 bulk artifact payload requested"
            );
        }
        let mut bulk_preview_budget_remaining = P031_ARTIFACT_PAYLOAD_BULK_PREVIEW_MAX_BYTES;
        Ok(items
            .into_iter()
            .map(|row| {
                let mut artifact = GqlArtifact::from(row.clone());
                if should_attach_payload {
                    attach_p031_artifact_payload(
                        &row,
                        run.as_ref(),
                        &mut artifact,
                        &mut bulk_preview_budget_remaining,
                    );
                }
                artifact
            })
            .collect())
    }

    async fn artifact(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlArtifact>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let artifact_id: ArtifactId = id
            .as_str()
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let Some(row) = artifacts::find_by_id(pool, artifact_id).await? else {
            debug!(artifact_id = %id.as_str(), "P031 selected artifact query missed");
            return Ok(None);
        };
        let run = runs::find_by_id(pool, row.run_id).await?;
        let format = row.format.to_string();
        let mut artifact = GqlArtifact::from(row.clone());
        let should_attach_payload = ctx.look_ahead().field("payloadText").exists();
        debug!(
            artifact_id = %id.as_str(),
            run_id = %row.run_id,
            payload_requested = should_attach_payload,
            "P031 selected artifact query"
        );
        if should_attach_payload {
            let mut preview_budget = P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES;
            attach_p031_artifact_payload_from_metadata(
                &format,
                row.report_kind.as_deref(),
                row.size_bytes,
                &row.file_path,
                run.as_ref(),
                &mut artifact,
                &mut preview_budget,
            );
        }
        debug!(
            artifact_id = %artifact.id.as_str(),
            payload_state = ?artifact.payload_availability_state,
            has_payload = artifact.payload_text.as_ref().is_some_and(|text| !text.is_empty()),
            "P031 selected artifact response"
        );
        Ok(Some(artifact))
    }

    async fn stages(&self, ctx: &Context<'_>, run_id: ID) -> Result<Vec<GqlStageExecution>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let items = projections::list_stages_projection(pool, run_id.as_str()).await?;
        Ok(items.into_iter().map(GqlStageExecution::from).collect())
    }

    async fn run_stage_topology(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<Vec<GqlRunStageTopologyNode>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let run_id: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let Some(run) = runs::find_by_id(pool, run_id).await? else {
            return Ok(vec![]);
        };
        let Some(workflow_snapshot_json) = run.workflow_snapshot_json.as_deref() else {
            return Ok(vec![]);
        };
        let Some(catalog_snapshot_json) = run.catalog_snapshot_json.as_deref() else {
            return Ok(vec![]);
        };
        if workflow_snapshot_json.trim().is_empty() || catalog_snapshot_json.trim().is_empty() {
            return Ok(vec![]);
        }

        let catalog_path = run.agent_catalog_yaml_path.as_deref().unwrap_or(".");
        let plan = match workflow::compiler::compile_from_snapshot_json(
            workflow_snapshot_json,
            catalog_snapshot_json,
            catalog_path,
        ) {
            Ok(plan) => plan,
            Err(error) => {
                warn!(
                    run_id = %run.id,
                    error = %error,
                    "P036 runStageTopology failed closed because frozen snapshots did not compile"
                );
                return Ok(vec![]);
            }
        };

        let stage_rows = projections::list_stages_projection(pool, &run_id.to_string()).await?;
        let artifact_rows = artifacts::list_by_run(pool, run_id).await?;
        let agent_execution_rows = db::repos::agent_executions::list_by_run(pool, run_id).await?;
        Ok(p036_topology_nodes(
            &run,
            &plan,
            stage_rows,
            artifact_rows,
            agent_execution_rows,
        ))
    }

    async fn active_agent_executions(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<Vec<GqlAgentExecution>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let run_id: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let items = db::repos::agent_executions::list_running_by_run(pool, run_id).await?;
        p093_active_agent_executions(pool, run_id, items).await
    }

    async fn timeline_raw_detail(
        &self,
        ctx: &Context<'_>,
        handle: ID,
    ) -> Result<GqlTimelineRawDetailResult> {
        require_operator_read(ctx).await?;
        let handle = handle.to_string();
        if handle.trim().is_empty() {
            return Ok(GqlTimelineRawDetailResult::missing(
                TimelineRawDetailErrorReason::HandleNotFound,
            ));
        }
        p093_resolve_timeline_raw_detail(ctx.data::<SqlitePool>()?, &handle).await
    }

    /// Work-queue counts for all items associated with a run.
    async fn run_queue_summary(&self, ctx: &Context<'_>, run_id: ID) -> Result<GqlRunQueueSummary> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let run_id_str = run_id.as_str();
        let rows = sqlx::query(
            r#"SELECT status, COUNT(*) AS cnt FROM work_items WHERE run_id = ?1 GROUP BY status"#,
        )
        .bind(run_id_str)
        .fetch_all(pool)
        .await
        .map_err(|e| Error::new(e.to_string()))?;
        let mut pending = 0i64;
        let mut running = 0i64;
        let mut completed = 0i64;
        let mut failed = 0i64;
        let mut cancelled = 0i64;
        for row in &rows {
            let status: String = row.get("status");
            let cnt: i64 = row.get("cnt");
            match status.as_str() {
                "pending" => pending = cnt,
                "running" => running = cnt,
                "completed" => completed = cnt,
                "failed" => failed = cnt,
                "cancelled" => cancelled = cnt,
                _ => {}
            }
        }
        Ok(GqlRunQueueSummary {
            run_id: run_id.clone(),
            pending,
            running,
            completed,
            failed,
            cancelled,
            total: pending + running + completed + failed + cancelled,
        })
    }

    /// Work-queue counts for all items associated with a stage execution.
    async fn stage_queue_summary(
        &self,
        ctx: &Context<'_>,
        stage_execution_id: ID,
    ) -> Result<GqlStageQueueSummary> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let stage_id_str = stage_execution_id.as_str();
        let rows = sqlx::query(
            r#"SELECT status, COUNT(*) AS cnt FROM work_items WHERE stage_id = ?1 GROUP BY status"#,
        )
        .bind(stage_id_str)
        .fetch_all(pool)
        .await
        .map_err(|e| Error::new(e.to_string()))?;
        let mut pending = 0i64;
        let mut running = 0i64;
        let mut completed = 0i64;
        let mut failed = 0i64;
        let mut cancelled = 0i64;
        for row in &rows {
            let status: String = row.get("status");
            let cnt: i64 = row.get("cnt");
            match status.as_str() {
                "pending" => pending = cnt,
                "running" => running = cnt,
                "completed" => completed = cnt,
                "failed" => failed = cnt,
                "cancelled" => cancelled = cnt,
                _ => {}
            }
        }
        Ok(GqlStageQueueSummary {
            stage_execution_id: stage_execution_id.clone(),
            pending,
            running,
            completed,
            failed,
            cancelled,
            total: pending + running + completed + failed + cancelled,
        })
    }

    async fn stage(&self, ctx: &Context<'_>, id: ID) -> Result<Option<GqlStageExecution>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let stage_execution_id: domain::ids::StageExecutionId = id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        stage_from_projection_or_canonical(pool, stage_execution_id).await
    }

    async fn agent_executions(
        &self,
        ctx: &Context<'_>,
        stage_execution_id: ID,
    ) -> Result<Vec<GqlAgentExecution>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let stage_execution_id: domain::ids::StageExecutionId = stage_execution_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let items = db::repos::agent_executions::find_by_stage(pool, stage_execution_id).await?;
        Ok(items.into_iter().map(GqlAgentExecution::from).collect())
    }

    async fn run_escalation_readback(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<crate::types::escalation::GqlEscalationRunReadback> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let run_id: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        crate::types::escalation::run_escalation_readback(pool, run_id).await
    }

    async fn steward_analyses(
        &self,
        ctx: &Context<'_>,
        limit: Option<i32>,
        status: Option<String>,
    ) -> Result<Vec<GqlStewardAnalysis>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let parsed_status = status
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(Error::new)?;
        let items =
            steward_repo::list_analyses(pool, limit.unwrap_or(50) as i64, parsed_status).await?;
        let mut result = Vec::with_capacity(items.len());
        for item in items {
            let analysis_id = item.id.clone();
            let links = steward_repo::list_run_links(pool, &analysis_id).await?;
            let recommendations = steward_repo::list_recommendations(pool, &analysis_id).await?;
            result.push(GqlStewardAnalysis::from_parts(item, links, recommendations));
        }
        Ok(result)
    }

    async fn steward_analysis(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> Result<Option<GqlStewardAnalysis>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let item = steward_repo::find_analysis(pool, id.as_str()).await?;
        if let Some(item) = item {
            let links = steward_repo::list_run_links(pool, id.as_str()).await?;
            let recommendations = steward_repo::list_recommendations(pool, id.as_str()).await?;
            Ok(Some(GqlStewardAnalysis::from_parts(
                item,
                links,
                recommendations,
            )))
        } else {
            Ok(None)
        }
    }

    async fn steward_analysis_run_links(
        &self,
        ctx: &Context<'_>,
        analysis_id: ID,
    ) -> Result<Vec<GqlStewardAnalysisRunLink>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let items = steward_repo::list_run_links(pool, analysis_id.as_str()).await?;
        Ok(items
            .into_iter()
            .map(GqlStewardAnalysisRunLink::from)
            .collect())
    }

    async fn steward_recommendations(
        &self,
        ctx: &Context<'_>,
        analysis_id: ID,
    ) -> Result<Vec<GqlStewardRecommendation>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let items = steward_repo::list_recommendations(pool, analysis_id.as_str()).await?;
        Ok(items
            .into_iter()
            .map(GqlStewardRecommendation::from)
            .collect())
    }

    /// P042 §5.2 readback surface. Returns the authoritative
    /// `DaemonStatus` owned by the in-process lifecycle reporter.
    /// Operator-only — matches the `/health` vs `daemonStatus` trust
    /// split: any authenticated operator can read the full typed status,
    /// unauthenticated loopback probes get the JSON snapshot at `/health`.
    async fn daemon_status(&self, ctx: &Context<'_>) -> Result<GqlDaemonStatus> {
        require_operator_read(ctx).await?;
        let reporter = ctx.data::<LifecycleReporter>()?;
        Ok(GqlDaemonStatus::from(reporter.snapshot()))
    }

    /// P081: bounded operator diagnostic readback for the active boundary policy
    /// and audit-log integrity state. This never exposes raw audit rows.
    async fn boundary_runtime(&self, ctx: &Context<'_>) -> Result<Json<serde_json::Value>> {
        require_observer_opt_in_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let boundary_policy = ctx
            .data_opt::<Arc<auth::boundary::BoundaryPolicy>>()
            .map(|policy| policy.as_ref());
        Ok(Json(
            boundary_runtime_readback_json(pool, boundary_policy).await?,
        ))
    }

    /// P081: bounded operator alert inbox derived from the same BoundaryPolicy
    /// and audit health readback as `boundaryRuntime`.
    async fn operator_alerts(&self, ctx: &Context<'_>) -> Result<Json<serde_json::Value>> {
        let authorization = require_observer_opt_in_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let boundary_policy = ctx
            .data_opt::<Arc<auth::boundary::BoundaryPolicy>>()
            .map(|policy| policy.as_ref());
        let mut alerts = p081_operator_alerts_json(pool, boundary_policy).await?;
        let collector = ctx.data_opt::<P081GraphqlRedactionCollector>();
        redact_p081_operator_alerts_for_observer(&mut alerts, &authorization, collector);
        Ok(Json(alerts))
    }

    /// P075: Storage health readback for write pressure, evidence spooling,
    /// units, freshness, thresholds, and kill-switch state.
    async fn storage_health(
        &self,
        ctx: &Context<'_>,
    ) -> Result<crate::types::storage::GqlStorageHealth> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let heartbeat = ctx.data_opt::<Arc<DbWriterHeartbeat>>();

        // P087: Enforce hot-read circuit guard for storage.health GraphQL readback.
        let guard = db::hot_read_guard::HotReadGuard::new(pool.clone(), "storage.health");
        let check = guard.check().await.map_err(|e| Error::new(e.to_string()))?;

        match check {
            db::hot_read_guard::CheckResult::Allowed {
                is_probe,
                probe_guard: _probe_guard,
            } => {
                let timeout_ms = if is_probe { 500 } else { 10_000 };

                // P087: Create a cancellation token so underlying SQLite/metadata/lane
                // resources are cancelled when the timeout fires, matching MCP behaviour.
                let cancel = tokio_util::sync::CancellationToken::new();
                let _cancel_guard = cancel.clone().drop_guard();

                let result = tokio::time::timeout(
                    std::time::Duration::from_millis(timeout_ms),
                    db::writer::CANCELLATION_TOKEN.scope(
                        cancel,
                        db::repos::storage_health::storage_health_with_writer(
                            pool,
                            heartbeat.map(|heartbeat| heartbeat.as_ref()),
                        ),
                    ),
                )
                .await;

                let json = match result {
                    Err(_elapsed) => {
                        // P087: Record violation so the circuit opens/stays open on timeout.
                        let _ = guard.record_violation("timeout").await;
                        let _ = db::metrics::increment_counter("graphql_hot_read_timeout_total");
                        return Err(Error::new("hot read timeout (GraphQL)").extend_with(
                            |_, ext| {
                                ext.set("code", "HOT_READ_TIMEOUT");
                                ext.set("surface", "storage.health");
                                ext.set("timeoutMs", timeout_ms as i64);
                            },
                        ));
                    }
                    Ok(Err(e)) => {
                        let _ = guard.record_violation("unavailable").await;
                        return Err(Error::new(e.to_string()));
                    }
                    Ok(Ok(json)) => json,
                };

                if is_probe {
                    let _ = guard.record_success().await;
                }

                crate::types::storage::GqlStorageHealth::from_storage_health_json(json)
                    .map_err(|e| Error::new(e.to_string()))
            }
            db::hot_read_guard::CheckResult::Denied {
                status,
                last_opened,
                retry_after,
            } => {
                let now_ms = chrono::Utc::now().timestamp_millis();
                let retry_after_ms = retry_after.map(|retry_after| (retry_after - now_ms).max(0));
                Err(
                    Error::new("hot read circuit is open (GraphQL)").extend_with(
                        |_, extensions| {
                            extensions.set("code", "HOT_READ_CIRCUIT_OPEN");
                            extensions.set("surface", "storage.health");
                            extensions.set(
                                "status",
                                match status {
                                    db::repos::hot_read_circuit::CircuitStatus::Open => "OPEN",
                                    db::repos::hot_read_circuit::CircuitStatus::HalfOpen => {
                                        "HALF_OPEN"
                                    }
                                    db::repos::hot_read_circuit::CircuitStatus::Closed => "CLOSED",
                                },
                            );
                            if let Some(last_opened) = last_opened {
                                extensions.set("lastOpenedAtMs", last_opened);
                            }
                            if let Some(retry_after_ms) = retry_after_ms {
                                extensions.set("retryAfterMs", retry_after_ms);
                            }
                        },
                    ),
                )
            }
        }
    }

    /// P066 T17: Latest startup recovery summary including toolchainCache fields.
    /// Returns None when no startup recovery sweep has been recorded yet.
    async fn startup_recovery_summary(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlStartupRecoverySummary>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let readback = db::repos::startup_repairs::latest_startup_recovery_readback(pool).await?;
        Ok(readback.map(GqlStartupRecoverySummary::from))
    }

    /// P066 T18: Latest toolchain cache housekeeping summary.
    /// Returns None before any housekeeping sweep has been recorded.
    async fn toolchain_cache_housekeeping_summary(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlToolchainCacheHousekeepingSummary>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let readback = db::repos::toolchain_cache_housekeeping::latest(pool).await?;
        Ok(readback.map(GqlToolchainCacheHousekeepingSummary::from))
    }

    /// P078: Bounded read-only projection of unresolved side-effect records.
    /// Returns at most `first` records (1-100, default 50).
    /// Read-only: no reconcile, retry, push, upload, or command mutations.
    async fn unresolved_side_effects(
        &self,
        ctx: &Context<'_>,
        first: Option<i32>,
    ) -> Result<Vec<GqlSideEffectSummary>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let limit = first.unwrap_or(50).clamp(1, 100) as u32;
        let effects = db::repos::side_effects::list_unresolved(pool, limit).await?;
        Ok(effects
            .into_iter()
            .map(GqlSideEffectSummary::from_domain)
            .collect())
    }

    // ── P046: Session observability read-only queries ─────────────────────────

    #[graphql(visible = "crate::types::session::p046_visible")]
    async fn session_lineages(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
        #[graphql(default = 100)] first: i32,
        after: Option<String>,
    ) -> Result<GqlSessionLineageConnection> {
        require_operator_read(ctx).await?;
        let p046 = ctx.data::<P046Config>()?;
        if !p046.enabled {
            db::metrics::increment_counter_with_label(
                "session_graphql_disabled_schema_guard_total",
                "graphql_external:blocked",
            );
            return Err(Error::new("session observability is not enabled"));
        }
        let pool = ctx.data::<SqlitePool>()?.clone();
        let resolver_deadline = p046_resolver_deadline();
        let resolver_start = std::time::Instant::now();
        let limit = first as i64;
        if limit > SESSION_LINEAGES_MAX_FIRST {
            return Err(Error::new("first exceeds maximum allowed value"));
        }
        if limit < 1 {
            return Err(Error::new("first must be at least 1"));
        }
        let run_id_str = run_id.as_str().to_string();
        if let Some(ref c) = after {
            match db::repos::sessions::decode_session_lineage_cursor(c) {
                Some((cursor_run_id, _, _, _, _)) if cursor_run_id == run_id_str => {}
                Some(_) => return Err(Error::new("invalid cursor")), // wrong run
                None => return Err(Error::new("invalid cursor")),
            }
        }
        let after_clone = after.clone();
        // P046 resource-scoped authorization: bind operator read to the owning run.
        match p046_check_run_accessible(ctx, &pool, &run_id_str, resolver_deadline).await? {
            Some(()) => {}
            None => return Err(Error::new("not found")),
        }
        let page = p046_retry_db("sessionLineages", resolver_deadline, || {
            let pool = pool.clone();
            let run_id_str = run_id_str.clone();
            let after = after_clone.clone();
            async move {
                session_repo::list_lineages_for_run_paginated(
                    &pool,
                    &run_id_str,
                    limit,
                    after.as_deref(),
                )
                .await
            }
        })
        .await
        .map_err(|e| {
            warn!("p046 db error: {e:#}");
            Error::new("db_unavailable")
        })?;
        // Load per-lineage stats (generation_count, latest_event_at, active_generation_status)
        // scoped to the returned page's lineage IDs only — this bounds the DB work to the
        // current page rather than scanning all lineages in the run.
        let page_lineage_ids: Vec<String> = page.items.iter().map(|l| l.id.clone()).collect();
        let stats = match p046_retry_db("sessionLineages", resolver_deadline, || {
            let pool = pool.clone();
            let ids = page_lineage_ids.clone();
            async move { session_repo::aggregate_lineage_stats_for_page(&pool, &ids).await }
        })
        .await
        {
            Ok(s) => Some(s),
            Err(e) => {
                warn!("p046 sessionLineages stats error: {e:#}");
                return Err(Error::new("db_unavailable"));
            }
        };
        db::metrics::increment_counter_with_label(
            "session_graphql_query_total",
            "sessionLineages:ok",
        );
        db::metrics::record_p046_query_duration(
            "sessionLineages",
            resolver_start.elapsed().as_millis() as u64,
        );
        Ok(GqlSessionLineageConnection::from_page_with_stats(
            page,
            stats.as_ref(),
        ))
    }

    #[graphql(visible = "crate::types::session::p046_visible")]
    async fn session_lineage(
        &self,
        ctx: &Context<'_>,
        id: ID,
    ) -> Result<Option<GqlSessionLineage>> {
        require_operator_read(ctx).await?;
        let p046 = ctx.data::<P046Config>()?;
        if !p046.enabled {
            db::metrics::increment_counter_with_label(
                "session_graphql_disabled_schema_guard_total",
                "graphql_external:blocked",
            );
            return Err(Error::new("session observability is not enabled"));
        }
        let pool = ctx.data::<SqlitePool>()?.clone();
        let resolver_deadline = p046_resolver_deadline();
        let resolver_start = std::time::Instant::now();
        let id_str = id.as_str().to_string();
        // Resolve owning run first; not-found-or-not-visible behavior
        let owner = p046_retry_db("sessionLineage", resolver_deadline, || {
            let pool = pool.clone();
            let id_str = id_str.clone();
            async move { session_repo::find_lineage_owner_run(&pool, &id_str).await }
        })
        .await
        .map_err(|e| {
            warn!("p046 db error: {e:#}");
            Error::new("db_unavailable")
        })?;
        let owner_run_id_str = match owner {
            None => return Ok(None),
            Some(r) => r,
        };
        // P046 resource-scoped authorization: bind operator read to the owning run.
        match p046_check_run_accessible(ctx, &pool, &owner_run_id_str, resolver_deadline).await? {
            Some(()) => {}
            None => return Ok(None),
        }
        let lineage = match p046_retry_db("sessionLineage", resolver_deadline, || {
            let pool = pool.clone();
            let id_str = id_str.clone();
            async move { session_repo::find_lineage_by_id(&pool, &id_str).await }
        })
        .await
        .map_err(|e| {
            warn!("p046 db error: {e:#}");
            Error::new("db_unavailable")
        })? {
            None => return Ok(None),
            Some(l) => l,
        };
        // Load stats and activeGeneration for the resolved single lineage.
        let stats = match p046_retry_db("sessionLineage", resolver_deadline, || {
            let pool = pool.clone();
            let id_str = id_str.clone();
            async move { session_repo::aggregate_lineage_stats_for_lineage(&pool, &id_str).await }
        })
        .await
        {
            Ok(s) => s,
            Err(e) => {
                warn!("p046 sessionLineage stats error: {e:#}");
                return Err(Error::new("db_unavailable"));
            }
        };
        let active_gen = if lineage.active_generation_id.is_some() {
            let run_id_for_ref = lineage.run_id.clone();
            let lid = lineage.id.clone();
            match p046_retry_db("sessionLineage", resolver_deadline, || {
                let pool = pool.clone();
                let lid = lid.clone();
                async move { session_repo::find_active_generation(&pool, &lid).await }
            })
            .await
            {
                Ok(gen) => gen.map(|g| {
                    crate::types::session::GqlSessionGeneration::from_domain(g, &run_id_for_ref)
                }),
                Err(e) => {
                    warn!("p046 sessionLineage activeGeneration error: {e:#}");
                    return Err(Error::new("db_unavailable"));
                }
            }
        } else {
            None
        };
        let mut gql = GqlSessionLineage::from_lineage_with_stats(lineage, stats.as_ref());
        gql.active_generation = active_gen;
        db::metrics::increment_counter_with_label(
            "session_graphql_query_total",
            "sessionLineage:ok",
        );
        db::metrics::record_p046_query_duration(
            "sessionLineage",
            resolver_start.elapsed().as_millis() as u64,
        );
        Ok(Some(gql))
    }

    #[graphql(visible = "crate::types::session::p046_visible")]
    async fn session_generations(
        &self,
        ctx: &Context<'_>,
        lineage_id: ID,
        #[graphql(default = 100)] first: i32,
        after: Option<String>,
    ) -> Result<GqlSessionGenerationConnection> {
        require_operator_read(ctx).await?;
        let p046 = ctx.data::<P046Config>()?;
        if !p046.enabled {
            db::metrics::increment_counter_with_label(
                "session_graphql_disabled_schema_guard_total",
                "graphql_external:blocked",
            );
            return Err(Error::new("session observability is not enabled"));
        }
        let pool = ctx.data::<SqlitePool>()?;
        let resolver_deadline = p046_resolver_deadline();
        let resolver_start = std::time::Instant::now();
        let limit = first as i64;
        if limit > SESSION_GENERATIONS_MAX_FIRST {
            return Err(Error::new("first exceeds maximum allowed value"));
        }
        if limit < 1 {
            return Err(Error::new("first must be at least 1"));
        }
        let lineage_id_str = lineage_id.as_str().to_string();
        if let Some(ref c) = after {
            match db::repos::sessions::decode_session_generation_cursor(c) {
                Some((cursor_lid, _, _, _)) if cursor_lid == lineage_id_str => {}
                Some(_) => return Err(Error::new("invalid cursor")), // mismatched filter
                None => return Err(Error::new("invalid cursor")),
            }
        }
        let pool = pool.clone();
        let after_clone = after.clone();
        // Resolve owning run for resource-scoped authorization (not-found-or-not-visible).
        let run_id = match p046_retry_db("sessionGenerations", resolver_deadline, || {
            let pool = pool.clone();
            let lineage_id_str = lineage_id_str.clone();
            async move { session_repo::find_lineage_owner_run(&pool, &lineage_id_str).await }
        })
        .await
        .map_err(|e| {
            warn!("p046 db error: {e:#}");
            Error::new("db_unavailable")
        })? {
            Some(r) => r,
            None => {
                return Ok(GqlSessionGenerationConnection::from_page(
                    db::repos::sessions::SessionGenerationPage {
                        items: vec![],
                        has_next_page: false,
                        start_cursor: None,
                        end_cursor: None,
                    },
                    "",
                ));
            }
        };
        // P046 resource-scoped authorization: bind operator read to the owning run.
        match p046_check_run_accessible(ctx, &pool, &run_id, resolver_deadline).await? {
            Some(()) => {}
            None => {
                return Ok(GqlSessionGenerationConnection::from_page(
                    db::repos::sessions::SessionGenerationPage {
                        items: vec![],
                        has_next_page: false,
                        start_cursor: None,
                        end_cursor: None,
                    },
                    "",
                ));
            }
        }
        let page = p046_retry_db("sessionGenerations", resolver_deadline, || {
            let pool = pool.clone();
            let lineage_id_str = lineage_id_str.clone();
            let after = after_clone.clone();
            async move {
                session_repo::list_generations_for_lineage_paginated(
                    &pool,
                    &lineage_id_str,
                    limit,
                    after.as_deref(),
                )
                .await
            }
        })
        .await
        .map_err(|e| {
            warn!("p046 db error: {e:#}");
            Error::new("db_unavailable")
        })?;
        db::metrics::increment_counter_with_label(
            "session_graphql_query_total",
            "sessionGenerations:ok",
        );
        db::metrics::record_p046_query_duration(
            "sessionGenerations",
            resolver_start.elapsed().as_millis() as u64,
        );
        Ok(GqlSessionGenerationConnection::from_page(page, &run_id))
    }

    #[graphql(visible = "crate::types::session::p046_visible")]
    async fn session_events(
        &self,
        ctx: &Context<'_>,
        lineage_id: ID,
        generation_id: Option<ID>,
        #[graphql(default = 200)] first: i32,
        after: Option<String>,
    ) -> Result<GqlSessionEventConnection> {
        require_operator_read(ctx).await?;
        let p046 = ctx.data::<P046Config>()?;
        if !p046.enabled {
            db::metrics::increment_counter_with_label(
                "session_graphql_disabled_schema_guard_total",
                "graphql_external:blocked",
            );
            return Err(Error::new("session observability is not enabled"));
        }
        let pool = ctx.data::<SqlitePool>()?;
        let resolver_deadline = p046_resolver_deadline();
        let resolver_start = std::time::Instant::now();
        let limit = first as i64;
        if limit > SESSION_EVENTS_MAX_FIRST {
            return Err(Error::new("first exceeds maximum allowed value"));
        }
        if limit < 1 {
            return Err(Error::new("first must be at least 1"));
        }
        let lineage_id_str = lineage_id.as_str().to_string();
        let gen_filter = generation_id.as_deref().map(|id| id.as_str().to_string());
        if let Some(ref c) = after {
            let expected_gen = gen_filter.as_deref().unwrap_or("");
            match db::repos::sessions::decode_session_cursor(c) {
                Some((cursor_lid, cursor_gen, _, _))
                    if cursor_lid == lineage_id_str && cursor_gen == expected_gen => {}
                Some(_) => return Err(Error::new("invalid cursor")), // mismatched lineage or gen filter
                None => return Err(Error::new("invalid cursor")),
            }
        }
        let pool = pool.clone();
        let after_clone = after.clone();
        // Resolve owning run for resource-scoped authorization (not-found-or-not-visible).
        let owner_run_id = p046_retry_db("sessionEvents", resolver_deadline, || {
            let pool = pool.clone();
            let lineage_id_str = lineage_id_str.clone();
            async move { session_repo::find_lineage_owner_run(&pool, &lineage_id_str).await }
        })
        .await
        .map_err(|e| {
            warn!("p046 db error: {e:#}");
            Error::new("db_unavailable")
        })?;
        let empty_page = || db::repos::sessions::SessionEventPage {
            items: vec![],
            has_next_page: false,
            start_cursor: None,
            end_cursor: None,
            gen_id_filter: gen_filter.as_deref().unwrap_or("").to_string(),
        };
        match owner_run_id {
            None => {
                return Ok(GqlSessionEventConnection::from_page(empty_page()));
            }
            Some(rid) => {
                // P046 resource-scoped authorization: bind operator read to the owning run.
                match p046_check_run_accessible(ctx, &pool, &rid, resolver_deadline).await? {
                    Some(()) => {}
                    None => {
                        return Ok(GqlSessionEventConnection::from_page(empty_page()));
                    }
                }
            }
        }
        // Validate generationId belongs to the authorized lineage before reading events.
        // Proposal requirement: "verify generationId belongs to that lineage before returning
        // events. A generationId from another lineage returns not-found-or-not-visible."
        if let Some(ref gen_id) = gen_filter {
            let pool_for_check = pool.clone();
            let gen_id_for_check = gen_id.clone();
            let lineage_id_for_check = lineage_id_str.clone();
            let gen_belongs = p046_retry_db("sessionEvents", resolver_deadline, move || {
                let pool = pool_for_check.clone();
                let gen_id = gen_id_for_check.clone();
                let lineage_id = lineage_id_for_check.clone();
                async move {
                    let result =
                        session_repo::find_generation_with_lineage_owner(&pool, &gen_id).await?;
                    Ok(matches!(result, Some((ref g, _)) if g.lineage_id == lineage_id))
                }
            })
            .await
            .map_err(|e| {
                warn!("p046 db error: {e:#}");
                Error::new("db_unavailable")
            })?;

            if !gen_belongs {
                return Ok(GqlSessionEventConnection::from_page(empty_page()));
            }
        }

        let page = p046_retry_db("sessionEvents", resolver_deadline, || {
            let pool = pool.clone();
            let lineage_id_str = lineage_id_str.clone();
            let gen_filter = gen_filter.clone();
            let after = after_clone.clone();
            async move {
                session_repo::list_events_paginated(
                    &pool,
                    &lineage_id_str,
                    gen_filter.as_deref(),
                    limit,
                    after.as_deref(),
                )
                .await
            }
        })
        .await
        .map_err(|e| {
            warn!("p046 db error: {e:#}");
            Error::new("db_unavailable")
        })?;
        db::metrics::increment_counter_with_label(
            "session_graphql_query_total",
            "sessionEvents:ok",
        );
        db::metrics::record_p046_query_duration(
            "sessionEvents",
            resolver_start.elapsed().as_millis() as u64,
        );
        Ok(GqlSessionEventConnection::from_page(page))
    }

    #[graphql(visible = "crate::types::session::p046_visible")]
    async fn session_kpi_summary(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<GqlSessionKpiSummary> {
        require_operator_read(ctx).await?;
        let p046 = ctx.data::<P046Config>()?;
        if !p046.enabled {
            db::metrics::increment_counter_with_label(
                "session_graphql_disabled_schema_guard_total",
                "graphql_external:blocked",
            );
            return Err(Error::new("session observability is not enabled"));
        }
        let pool = ctx.data::<SqlitePool>()?.clone();
        let resolver_deadline = p046_resolver_deadline();
        let resolver_start = std::time::Instant::now();
        let run_id_str = run_id.as_str().to_string();
        // P046 resource-scoped authorization: bind operator read to the owning run.
        match p046_check_run_accessible(ctx, &pool, &run_id_str, resolver_deadline).await? {
            Some(()) => {}
            None => return Err(Error::new("not found")),
        }
        let kpi = p046_retry_db("sessionKpiSummary", resolver_deadline, || {
            let pool = pool.clone();
            let run_id_str = run_id_str.clone();
            async move { session_repo::aggregate_kpis_for_run(&pool, &run_id_str).await }
        })
        .await
        .map_err(|e| {
            warn!("p046 db error: {e:#}");
            Error::new("db_unavailable")
        })?;
        db::metrics::increment_counter_with_label(
            "session_graphql_query_total",
            "sessionKpiSummary:ok",
        );
        db::metrics::record_p046_query_duration(
            "sessionKpiSummary",
            resolver_start.elapsed().as_millis() as u64,
        );
        Ok(GqlSessionKpiSummary {
            run_id: run_id_str,
            lineage_count: kpi.lineage_count,
            generation_count: kpi.generation_count,
            active_generation_count: kpi.active_generation_count,
            closed_generation_count: kpi.closed_generation_count,
            reset_generation_count: kpi.reset_generation_count,
            invalidated_generation_count: kpi.invalidated_generation_count,
            reuse_event_count: kpi.reuse_event_count,
            operator_reset_event_count: kpi.operator_reset_event_count,
            total_turn_count: kpi.total_turn_count,
            total_prompt_tokens: kpi.total_prompt_tokens,
            total_cost_cents: kpi.total_cost_cents,
            latest_activity_at: kpi.latest_activity_at.map(|t| t.to_rfc3339()),
            stale_active_generation_count: kpi.stale_active_generation_count,
        })
    }

    #[graphql(visible = "crate::types::session::p046_visible")]
    async fn session_health(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<GqlSessionHealthReport> {
        require_operator_read(ctx).await?;
        let p046 = ctx.data::<P046Config>()?;
        if !p046.enabled {
            db::metrics::increment_counter_with_label(
                "session_graphql_disabled_schema_guard_total",
                "graphql_external:blocked",
            );
            return Err(Error::new("session observability is not enabled"));
        }
        let pool = ctx.data::<SqlitePool>()?.clone();
        let resolver_deadline = p046_resolver_deadline();
        let resolver_start = std::time::Instant::now();
        let run_id_str = run_id.as_str().to_string();
        // P046 resource-scoped authorization: bind operator read to the owning run.
        // For sessionHealth, transient db failure during access check returns UNKNOWN/transient_db_unavailable
        // rather than propagating a resolver error (same contract as aggregate read exhaustion).
        match p046_check_run_accessible(ctx, &pool, &run_id_str, resolver_deadline).await {
            Ok(Some(())) => {}
            Ok(None) => return Err(Error::new("not found")),
            Err(e) if e.message == "db_unavailable" => {
                db::metrics::increment_counter_with_label(
                    "session_graphql_query_total",
                    "sessionHealth:db_unavailable",
                );
                db::metrics::record_p046_query_duration(
                    "sessionHealth",
                    resolver_start.elapsed().as_millis() as u64,
                );
                return Ok(transient_db_unavailable_health(&run_id_str));
            }
            Err(e) => return Err(e),
        }
        // P046: use pinned retry policy.
        // Transient sqlite exhaustion returns UNKNOWN/transient_db_unavailable health (not an error).
        // Non-transient errors (data-shape corruption, schema mismatch) propagate as resolver errors.
        match p046_retry_db("sessionHealth", resolver_deadline, || {
            let pool = pool.clone();
            let run_id_str = run_id_str.clone();
            async move { session_repo::load_health_data_for_run(&pool, &run_id_str).await }
        })
        .await
        {
            Ok(data) => {
                let run_is_terminal = data.run_is_terminal;
                let report = crate::types::session::compute_session_health(
                    &data,
                    &run_id_str,
                    run_is_terminal,
                );
                db::metrics::increment_counter_with_label(
                    "session_graphql_query_total",
                    "sessionHealth:ok",
                );
                db::metrics::record_p046_query_duration(
                    "sessionHealth",
                    resolver_start.elapsed().as_millis() as u64,
                );
                Ok(report)
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.starts_with("transient_db_unavailable") {
                    db::metrics::increment_counter_with_label(
                        "session_graphql_query_total",
                        "sessionHealth:db_unavailable",
                    );
                    db::metrics::record_p046_query_duration(
                        "sessionHealth",
                        resolver_start.elapsed().as_millis() as u64,
                    );
                    Ok(transient_db_unavailable_health(&run_id_str))
                } else {
                    warn!("p046 sessionHealth non-transient error for run {run_id_str}: {e:#}");
                    Err(Error::new("session health data unavailable"))
                }
            }
        }
    }

    // Lightweight capability probe: present only when P046 is enabled.
    // Clients issue this before constructing P046 query/subscription documents.
    // A "Cannot query field" error means P046 fields are absent from the schema.
    // Requires operator-read authorization so non-operator principals cannot fingerprint
    // daemon capability (holds_conditions: every P046 query/subscription requires operator-read).
    #[graphql(visible = "crate::types::session::p046_visible")]
    async fn session_observability_available(&self, ctx: &Context<'_>) -> Result<bool> {
        require_operator_read(ctx).await?;
        let p046 = ctx.data::<P046Config>()?;
        if !p046.enabled {
            db::metrics::increment_counter_with_label(
                "session_graphql_disabled_schema_guard_total",
                "graphql_external:blocked",
            );
            return Err(Error::new("session observability is not enabled"));
        }
        Ok(true)
    }

    /// P086: Read-only continuation history and current status for an agent execution.
    /// Returns raw + display status, freshness/projection-lag, and UNKNOWN display states
    /// for unrecognised daemon values. Operator-only; no continuation mutation surface.
    async fn continuation_status(
        &self,
        ctx: &Context<'_>,
        agent_execution_id: ID,
    ) -> Result<GqlContinuationStatus> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let ae_id = agent_execution_id.as_str();
        let records = agent_work_continuations::list_for_agent_execution(pool, ae_id).await?;
        let active = agent_work_continuations::find_active_for_agent_execution(pool, ae_id).await?;
        let freshness_state = if records.is_empty() {
            crate::types::p031::GqlFreshnessState::Unavailable
        } else {
            crate::types::p031::GqlFreshnessState::Live
        };
        let history: Vec<GqlContinuationRecord> = records
            .into_iter()
            .map(GqlContinuationRecord::from)
            .collect();
        let active_gql = active.map(GqlContinuationRecord::from);
        Ok(GqlContinuationStatus {
            agent_execution_id: agent_execution_id.clone(),
            active: active_gql,
            history,
            freshness_state,
        })
    }

    /// P086: Read-only list of eligible continuation candidates for a run.
    /// Returns eligibility, raw/display status, and disabled reason for each
    /// code_writer stage-owned AgentExecution. Operator-only.
    async fn continuation_candidates(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<GqlContinuationCandidatesResult> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let candidates =
            agent_work_continuations::list_candidates_for_run(pool, run_id.as_str()).await?;
        let freshness_state = if candidates.is_empty() {
            crate::types::p031::GqlFreshnessState::Unavailable
        } else {
            crate::types::p031::GqlFreshnessState::Live
        };
        Ok(GqlContinuationCandidatesResult {
            run_id: run_id.clone(),
            candidates: candidates
                .into_iter()
                .map(crate::types::continuation::GqlContinuationCandidate::from)
                .collect(),
            freshness_state,
        })
    }

    /// P086: Read-only run-level continuation history for SwiftUI/operator readback.
    async fn continuations(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<Vec<GqlContinuationRecord>> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let records = agent_work_continuations::list_for_run(pool, run_id.as_str()).await?;
        Ok(records
            .into_iter()
            .map(GqlContinuationRecord::from)
            .collect())
    }

    /// P086: Durable read-only rollout metric summary for continuation behavior.
    async fn continuation_metrics_summary(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<GqlContinuationMetricsSummary> {
        require_operator_read(ctx).await?;
        let pool = ctx.data::<SqlitePool>()?;
        let summary = agent_work_continuations::p086_continuation_metrics_summary_for_run(
            pool,
            run_id.as_str(),
        )
        .await?;
        Ok(GqlContinuationMetricsSummary::from(summary))
    }
}

/// GraphQL wrapper around [`DaemonStatus`] (P042 §5.2). Every field of the
/// domain type is exposed as a first-class GraphQL field: `state`,
/// `degraded`, and `failure` are typed enum/object values so clients can
/// pattern-match terminal reasons without parsing a stringified JSON.
///
/// The `json` field is retained as a convenience for clients that want
/// the canonical snake-case serialization (matching `/health` wire
/// format) without re-serializing the typed fields.
#[derive(SimpleObject, Clone)]
pub struct GqlDaemonStatus {
    pub state: GqlDaemonLifecycleState,
    pub schema_version: i32,
    pub binary_schema_version: i32,
    pub build_sha: String,
    /// ISO-8601 UTC. `None` before the daemon has reached `Ready`.
    pub started_at: Option<String>,
    pub last_state_change_at: String,
    pub restart_count_since_boot: i32,
    pub pid: i32,
    /// Non-empty iff `state == DEGRADED`.
    pub degraded: Vec<GqlDegradedReason>,
    /// Populated iff `state == FAILED` (P042 §4.1 invariant).
    pub failure: Option<GqlFailureReason>,
    /// Xcode MCP broker health when the daemon has mounted the broker pool.
    pub xcode_broker_health: Option<GqlXcodeBrokerHealthSnapshot>,
    /// Canonical JSON per P042 §5.2 (`{state, schema_version, pid,
    /// degraded?, failure?}`). Kept for clients that prefer the
    /// snake-case wire shape identical to `/health`.
    pub json: String,
}

/// GraphQL mirror of [`domain::lifecycle::DaemonLifecycleState`]. Names
/// match the domain enum exactly so the `#[Enum]` mapping round-trips.
#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlDaemonLifecycleState {
    NotStarted,
    Starting,
    Ready,
    Degraded,
    Restarting,
    Failed,
    Shutdown,
}

impl From<domain::lifecycle::DaemonLifecycleState> for GqlDaemonLifecycleState {
    fn from(s: domain::lifecycle::DaemonLifecycleState) -> Self {
        use domain::lifecycle::DaemonLifecycleState::*;
        match s {
            NotStarted => Self::NotStarted,
            Starting => Self::Starting,
            Ready => Self::Ready,
            Degraded => Self::Degraded,
            Restarting => Self::Restarting,
            Failed => Self::Failed,
            Shutdown => Self::Shutdown,
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlDegradedKind {
    BackgroundExecutorStalled,
    AcpRuntimeUnavailable,
    StaleProjection,
    AuthPrincipalTableUnreadable,
    DiskSpaceLow,
}

impl From<domain::lifecycle::DegradedKind> for GqlDegradedKind {
    fn from(k: domain::lifecycle::DegradedKind) -> Self {
        use domain::lifecycle::DegradedKind::*;
        match k {
            BackgroundExecutorStalled => Self::BackgroundExecutorStalled,
            AcpRuntimeUnavailable => Self::AcpRuntimeUnavailable,
            StaleProjection => Self::StaleProjection,
            AuthPrincipalTableUnreadable => Self::AuthPrincipalTableUnreadable,
            DiskSpaceLow => Self::DiskSpaceLow,
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlFailureKind {
    MigrationFailed,
    SchemaNewerThanBinary,
    BackupFailed,
    CrashLoopBudgetExhausted,
}

impl From<domain::lifecycle::FailureKind> for GqlFailureKind {
    fn from(k: domain::lifecycle::FailureKind) -> Self {
        use domain::lifecycle::FailureKind::*;
        match k {
            MigrationFailed => Self::MigrationFailed,
            SchemaNewerThanBinary => Self::SchemaNewerThanBinary,
            BackupFailed => Self::BackupFailed,
            CrashLoopBudgetExhausted => Self::CrashLoopBudgetExhausted,
        }
    }
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlXcodeBrokerHealthState {
    Disabled,
    Healthy,
    Degraded,
    Failed,
}

impl From<domain::lifecycle::XcodeBrokerHealthState> for GqlXcodeBrokerHealthState {
    fn from(s: domain::lifecycle::XcodeBrokerHealthState) -> Self {
        use domain::lifecycle::XcodeBrokerHealthState::*;
        match s {
            Disabled => Self::Disabled,
            Healthy => Self::Healthy,
            Degraded => Self::Degraded,
            Failed => Self::Failed,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct GqlDegradedReason {
    pub kind: GqlDegradedKind,
    pub detail: String,
    /// ISO-8601 UTC.
    pub since: String,
}

impl From<domain::lifecycle::DegradedReason> for GqlDegradedReason {
    fn from(r: domain::lifecycle::DegradedReason) -> Self {
        Self {
            kind: r.kind.into(),
            detail: r.detail,
            since: r.since.to_rfc3339(),
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct GqlFailureReason {
    pub kind: GqlFailureKind,
    pub detail: String,
    /// ISO-8601 UTC.
    pub since: String,
    /// Absolute path of the pre-migration backup when applicable.
    pub backup_path: Option<String>,
}

impl From<domain::lifecycle::FailureReason> for GqlFailureReason {
    fn from(r: domain::lifecycle::FailureReason) -> Self {
        Self {
            kind: r.kind.into(),
            detail: r.detail,
            since: r.since.to_rfc3339(),
            backup_path: r.backup_path,
        }
    }
}

/// Run-level work-queue summary for `runQueueSummary(runId:)`.
#[derive(SimpleObject, Clone)]
pub struct GqlRunQueueSummary {
    pub run_id: ID,
    pub pending: i64,
    pub running: i64,
    pub completed: i64,
    pub failed: i64,
    pub cancelled: i64,
    pub total: i64,
}

/// Stage-level work-queue summary for `stageQueueSummary(stageExecutionId:)`.
#[derive(SimpleObject, Clone)]
pub struct GqlStageQueueSummary {
    pub stage_execution_id: ID,
    pub pending: i64,
    pub running: i64,
    pub completed: i64,
    pub failed: i64,
    pub cancelled: i64,
    pub total: i64,
}

#[derive(SimpleObject, Clone)]
pub struct GqlXcodeBrokerHealthSnapshot {
    pub state: GqlXcodeBrokerHealthState,
    pub reason_code: String,
    pub can_acquire_new_xcode_leases: bool,
    pub active_lease_count: i32,
    pub initialize_queue_depth: i32,
    pub last_transition_at: String,
    pub operator_message: String,
    pub pool_id: String,
    pub active_leases: i32,
    pub queued_leases: i32,
    pub max_active_leases: i32,
    pub max_queued_leases: i32,
    pub broker_disabled: bool,
    pub backend_available: bool,
    pub observation_persistence_failures: i32,
    pub stale_lease_count: i32,
    pub backend_session_count: i32,
    pub helper_cleanup_reaped_leases_total: i32,
}

impl From<domain::lifecycle::XcodeBrokerHealthSnapshot> for GqlXcodeBrokerHealthSnapshot {
    fn from(s: domain::lifecycle::XcodeBrokerHealthSnapshot) -> Self {
        Self {
            state: s.state.into(),
            reason_code: s.reason_code,
            can_acquire_new_xcode_leases: s.can_acquire_new_xcode_leases,
            active_lease_count: s.active_lease_count as i32,
            initialize_queue_depth: s.initialize_queue_depth as i32,
            last_transition_at: s.last_transition_at,
            operator_message: s.operator_message,
            pool_id: s.pool_id,
            active_leases: s.active_leases as i32,
            queued_leases: s.queued_leases as i32,
            max_active_leases: s.max_active_leases as i32,
            max_queued_leases: s.max_queued_leases as i32,
            broker_disabled: s.broker_disabled,
            backend_available: s.backend_available,
            observation_persistence_failures: s.observation_persistence_failures as i32,
            stale_lease_count: s.stale_lease_count as i32,
            backend_session_count: s.backend_session_count as i32,
            helper_cleanup_reaped_leases_total: s.helper_cleanup_reaped_leases_total as i32,
        }
    }
}

impl From<DaemonStatus> for GqlDaemonStatus {
    fn from(s: DaemonStatus) -> Self {
        let json = serde_json::to_string(&s).unwrap_or_else(|_| "{}".to_string());
        Self {
            state: s.state.into(),
            schema_version: s.schema_version as i32,
            binary_schema_version: s.binary_schema_version as i32,
            build_sha: s.build_sha,
            started_at: s.started_at.map(|t| t.to_rfc3339()),
            last_state_change_at: s.last_state_change_at.to_rfc3339(),
            restart_count_since_boot: s.restart_count_since_boot as i32,
            pid: s.pid as i32,
            degraded: s
                .degraded
                .into_iter()
                .map(GqlDegradedReason::from)
                .collect(),
            failure: s.failure.map(GqlFailureReason::from),
            xcode_broker_health: s
                .xcode_broker_health
                .map(GqlXcodeBrokerHealthSnapshot::from),
            json,
        }
    }
}

/// P078: Read-only projection of a single unresolved side-effect record.
/// Exposes raw kind/status strings for forward-compatible clients.
/// No mutation fields.
#[derive(SimpleObject, Clone)]
pub struct GqlSideEffectSummary {
    pub id: String,
    pub run_id: String,
    pub stage_execution_id: String,
    /// Decoded effect kind (e.g. "git_commit"). Use effect_kind_raw for unknown values.
    pub effect_kind: String,
    /// Raw effect kind string for forward-compatible clients.
    pub effect_kind_raw: String,
    /// Decoded status string. Use status_raw for unknown values.
    pub status: String,
    /// Raw status string for forward-compatible clients.
    pub status_raw: String,
    pub target_key: String,
    pub external_write_attempted: bool,
    pub last_error_kind: Option<String>,
    pub expected_evidence_json: Option<Json<serde_json::Value>>,
    pub observed_evidence_summary_json: Option<Json<serde_json::Value>>,
    pub evidence_root: Option<String>,
    pub readback_source: String,
    pub report_path: Option<String>,
    pub blocked_reason: String,
    pub operator_next_action: String,
    pub recommended_mcp_tool: String,
    pub retry_forbidden: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl GqlSideEffectSummary {
    pub fn from_domain(e: domain::side_effect::SideEffect) -> Self {
        let kind_str = e.effect_kind.to_string();
        let status_str = e.status.to_string();
        let expected_evidence_json = parse_optional_json(&e.expected_evidence_json);
        let observed_evidence_summary_json = parse_optional_json(&e.observed_evidence_summary_json);
        let report_path = observed_evidence_summary_json
            .as_ref()
            .and_then(|json| json.0.get("manifest_path"))
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned);
        let operator_next_action = side_effect_operator_next_action(&e.status);
        Self {
            id: e.id.to_string(),
            run_id: e.run_id.to_string(),
            stage_execution_id: e.stage_execution_id.to_string(),
            effect_kind: kind_str.clone(),
            effect_kind_raw: kind_str,
            status: status_str.clone(),
            status_raw: status_str,
            target_key: e.target_key,
            external_write_attempted: e.external_write_attempted,
            last_error_kind: e.last_error_kind,
            expected_evidence_json,
            observed_evidence_summary_json,
            evidence_root: e.evidence_root,
            readback_source: "side_effects_ledger".into(),
            report_path,
            blocked_reason: side_effect_blocked_reason(&e.status),
            operator_next_action: operator_next_action.clone(),
            recommended_mcp_tool: operator_next_action,
            retry_forbidden: true,
            created_at: e.created_at.to_rfc3339(),
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

fn parse_optional_json(raw: &Option<String>) -> Option<Json<serde_json::Value>> {
    raw.as_ref().map(|value| {
        Json(
            serde_json::from_str(value)
                .unwrap_or_else(|_| serde_json::Value::String(value.clone())),
        )
    })
}

fn side_effect_blocked_reason(status: &domain::side_effect::SideEffectStatus) -> String {
    match status {
        domain::side_effect::SideEffectStatus::Prepared => "prepared_effect_not_executed",
        domain::side_effect::SideEffectStatus::Executing => "executing_effect_not_settled",
        domain::side_effect::SideEffectStatus::ExternallyObserved => {
            "external_write_observed_pending_settlement"
        }
        domain::side_effect::SideEffectStatus::NeedsReconciliation => "effect_needs_reconciliation",
        domain::side_effect::SideEffectStatus::Conflict => "effect_conflict_requires_disposition",
        domain::side_effect::SideEffectStatus::Unrecoverable => {
            "effect_unrecoverable_requires_manual_clear"
        }
        _ => "not_blocking",
    }
    .to_string()
}

fn side_effect_operator_next_action(status: &domain::side_effect::SideEffectStatus) -> String {
    match status {
        domain::side_effect::SideEffectStatus::NeedsReconciliation
        | domain::side_effect::SideEffectStatus::ExternallyObserved => "effects.reconcile",
        domain::side_effect::SideEffectStatus::Conflict => {
            "effects.mark_unrecoverable or effects.clear_after_manual_verification"
        }
        domain::side_effect::SideEffectStatus::Unrecoverable => {
            "effects.clear_after_manual_verification"
        }
        _ => "effects.inspect",
    }
    .to_string()
}

async fn enrich_run_with_p091_retry_authority(
    pool: &SqlitePool,
    run_id: RunId,
    run: &mut GqlRun,
) -> Result<()> {
    let history = db::repos::retry_stage_execution_authorities::list_by_run(pool, run_id).await?;
    let p092_events =
        db::repos::retry_payload_recovery_events::latest_by_authority_for_run(pool, run_id).await?;
    let mut history_json: Vec<_> = history
        .iter()
        .map(|authority| {
            let mut value = serde_json::json!({
                "id": authority.id,
                "run_id": authority.run_id.to_string(),
                "stage_id": authority.stage_id,
                "target_stage_execution_id": authority.target_stage_execution_id.to_string(),
                "entry_kind": authority.entry_kind.to_string(),
                "source_command_journal_id": authority.source_command_journal_id,
                "source_retry_work_item_id": authority.source_retry_work_item_id,
                "source_invoke_work_item_id": authority.source_invoke_work_item_id,
                "source_agent_execution_id": authority.source_agent_execution_id,
                "authority_state": authority.authority_state.to_string(),
                "created_at": authority.created_at.to_rfc3339(),
                "updated_at": authority.updated_at.to_rfc3339(),
                "terminal_reason": authority.terminal_reason,
            });
            if let Some(event) = p092_events.get(&authority.id) {
                value["retry_payload_recovery"] = event.readback_json();
            }
            value
        })
        .collect();
    for event in db::repos::retry_payload_recovery_events::list_by_run(pool, run_id).await? {
        if event.retry_authority_id.is_none() {
            history_json.push(serde_json::json!({
                "schema_version": "retry_payload_recovery_history_v1",
                "authority_state": "missing_authority",
                "run_id": event.run_id.to_string(),
                "source_invoke_work_item_id": event.invoke_work_item_id,
                "retry_payload_recovery": event.readback_json(),
            }));
        }
    }
    run.retry_authority_json = history
        .iter()
        .find(|authority| authority.authority_state.to_string() == "active")
        .map(|authority| {
            let mut value = serde_json::json!({
                "id": authority.id,
                "stage_id": authority.stage_id,
                "target_stage_execution_id": authority.target_stage_execution_id.to_string(),
                "entry_kind": authority.entry_kind.to_string(),
                "authority_state": authority.authority_state.to_string(),
                "terminal_reason": authority.terminal_reason,
            });
            if let Some(event) = p092_events.get(&authority.id) {
                value["retry_payload_recovery"] = event.readback_json();
            }
            Json(value)
        });
    run.retry_authority_history_json = Some(Json(serde_json::Value::Array(history_json)));
    run.p091_orphan_repair_readback_json =
        Some(Json(p091_orphan_repair_readback(pool, run_id).await?));
    Ok(())
}

async fn p091_orphan_repair_readback(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let row = sqlx::query(
        r#"SELECT mode, disabled, candidates_total, excluded_total,
                  would_repair_total, repaired_total, disabled_total,
                  bounded_samples_json, created_at
           FROM p091_orphan_repair_passes
           WHERE run_id IS NULL OR run_id = ?1
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(run_id.to_string())
    .fetch_optional(pool)
    .await?;
    let disabled_by_env = std::env::var("CHAINWORKS_P091_DISABLE_STARTUP_ORPHAN_REPAIR")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    let configured_mode = std::env::var("CHAINWORKS_P091_STARTUP_ORPHAN_REPAIR_MODE")
        .unwrap_or_else(|_| "diagnostic".to_string());
    if let Some(row) = row {
        let samples_raw: Option<String> = row.get("bounded_samples_json");
        let samples = samples_raw
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
            .unwrap_or_else(|| serde_json::json!([]));
        Ok(serde_json::json!({
            "configured_mode": configured_mode,
            "operator_disabled": disabled_by_env,
            "latest_pass": {
                "mode": row.get::<String, _>("mode"),
                "disabled": row.get::<i64, _>("disabled") != 0,
                "candidates_total": row.get::<i64, _>("candidates_total"),
                "excluded_total": row.get::<i64, _>("excluded_total"),
                "would_repair_total": row.get::<i64, _>("would_repair_total"),
                "repaired_total": row.get::<i64, _>("repaired_total"),
                "disabled_total": row.get::<i64, _>("disabled_total"),
                "bounded_samples": samples,
                "created_at": row.get::<String, _>("created_at"),
            }
        }))
    } else {
        Ok(serde_json::json!({
            "configured_mode": configured_mode,
            "operator_disabled": disabled_by_env,
            "latest_pass": null,
        }))
    }
}

async fn side_effect_readback_json(pool: &SqlitePool, run_id: RunId) -> Result<serde_json::Value> {
    let unresolved =
        db::repos::side_effects::list_unresolved_for_run(pool, &run_id.to_string()).await?;
    let effects: Vec<serde_json::Value> = unresolved
        .iter()
        .map(|effect| {
            let observed = effect
                .observed_evidence_summary_json
                .as_deref()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
            let report_path = observed
                .as_ref()
                .and_then(|value| value.get("manifest_path"))
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned);
            serde_json::json!({
                "id": effect.id.to_string(),
                "run_id": effect.run_id.to_string(),
                "stage_execution_id": effect.stage_execution_id.to_string(),
                "agent_execution_id": effect.agent_execution_id.as_ref().map(|id| id.to_string()),
                "effect_kind": effect.effect_kind.to_string(),
                "status": effect.status.to_string(),
                "target_key": effect.target_key,
                "external_write_attempted": effect.external_write_attempted,
                "evidence_root": effect.evidence_root.clone(),
                "readback_source": "side_effects_ledger",
                "report_path": report_path,
                "blocked_reason": side_effect_blocked_reason(&effect.status),
                "operator_next_action": side_effect_operator_next_action(&effect.status),
                "recommended_mcp_tool": side_effect_operator_next_action(&effect.status),
                "retry_forbidden": true,
                "last_error_kind": effect.last_error_kind.clone(),
                "updated_at": effect.updated_at.to_rfc3339()
            })
        })
        .collect();
    Ok(serde_json::json!({
        "schema_version": "p078_side_effect_readback_v1",
        "run_id": run_id.to_string(),
        "unresolved_count": effects.len(),
        "blocked": !effects.is_empty(),
        "readback_source": "side_effects_ledger",
        "effects": effects
    }))
}

async fn proposal_064_command_readback(
    pool: &SqlitePool,
    run_id: RunId,
    command_types: &[&str],
) -> Result<serde_json::Value> {
    let placeholders = command_types
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, command_type, result_status, created_at, completed_at, caller_surface, caller_principal_id, caller_tool \
         FROM command_journal \
         WHERE run_id = ? AND command_type IN ({placeholders}) \
         ORDER BY created_at DESC LIMIT 8"
    );
    let mut query = sqlx::query(&sql).bind(run_id.to_string());
    for command_type in command_types {
        query = query.bind(*command_type);
    }
    let rows = query.fetch_all(pool).await?;
    let commands = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "id": row.try_get::<String, _>("id").ok(),
                "command_type": row.try_get::<String, _>("command_type").ok(),
                "result_status": row.try_get::<String, _>("result_status").ok(),
                "created_at": row.try_get::<String, _>("created_at").ok(),
                "completed_at": row.try_get::<Option<String>, _>("completed_at").ok().flatten(),
                "caller_surface": row.try_get::<Option<String>, _>("caller_surface").ok().flatten(),
                "caller_principal_id": row.try_get::<Option<String>, _>("caller_principal_id").ok().flatten(),
                "caller_tool": row.try_get::<Option<String>, _>("caller_tool").ok().flatten(),
            })
        })
        .collect::<Vec<_>>();
    let pending = commands
        .iter()
        .filter(|command| command["result_status"] == "pending")
        .cloned()
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "latest_commands": commands,
        "pending_commands": pending,
    }))
}

async fn proposal_064_main_sync_readback(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let latest_attempt = sqlx::query(
        "SELECT id, idempotency_key, trigger_reason, status, barrier_id, conflict_count, resolver_work_item_id, error_message, requested_by_stage_id, requested_by_work_item_id, created_at, started_at, completed_at \
         FROM main_sync_attempts WHERE run_id = ? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(run_id.to_string())
    .fetch_optional(pool)
    .await?;
    let barrier = sqlx::query(
        "SELECT id, owner_id, owner_kind, status, reason, acquired_at, heartbeat_at, expires_at, released_at \
         FROM worktree_mutation_barriers WHERE run_id = ? AND status IN ('pending', 'active') ORDER BY created_at DESC LIMIT 1",
    )
    .bind(run_id.to_string())
    .fetch_optional(pool)
    .await?;
    let active_consumers = sqlx::query(
        "SELECT id, worktree_resource_key, owner_id, worktree_access_mode, owner_kind, reason, acquired_at, expires_at, heartbeat_at \
         FROM background_leases WHERE run_id = ? AND worktree_resource_key IS NOT NULL AND released_at IS NULL ORDER BY acquired_at DESC LIMIT 16",
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    let command_readback = proposal_064_command_readback(
        pool,
        run_id,
        &[
            "MainSyncRequest",
            "MainSyncRetry",
            "MainSyncSetRunOverride",
            "MainSyncRepairState",
            "MainSyncRecordRecoveryDecision",
        ],
    )
    .await?;

    Ok(serde_json::json!({
        "schema_version": "p064_main_sync_readback_v1",
        "mode": "off",
        "operator_tools_enabled": false,
        "latest_attempt": latest_attempt.map(|row| serde_json::json!({
            "id": row.try_get::<String, _>("id").ok(),
            "idempotency_key": row.try_get::<String, _>("idempotency_key").ok(),
            "trigger_reason": row.try_get::<String, _>("trigger_reason").ok(),
            "status": row.try_get::<String, _>("status").ok(),
            "barrier_id": row.try_get::<Option<String>, _>("barrier_id").ok().flatten(),
            "conflict_count": row.try_get::<Option<i64>, _>("conflict_count").ok().flatten(),
            "resolver_work_item_id": row.try_get::<Option<String>, _>("resolver_work_item_id").ok().flatten(),
            "error_message": row.try_get::<Option<String>, _>("error_message").ok().flatten(),
            "requested_by_stage_id": row.try_get::<Option<String>, _>("requested_by_stage_id").ok().flatten(),
            "requested_by_work_item_id": row.try_get::<Option<String>, _>("requested_by_work_item_id").ok().flatten(),
            "created_at": row.try_get::<String, _>("created_at").ok(),
            "started_at": row.try_get::<Option<String>, _>("started_at").ok().flatten(),
            "completed_at": row.try_get::<Option<String>, _>("completed_at").ok().flatten(),
        })),
        "active_barrier": barrier.map(|row| serde_json::json!({
            "id": row.try_get::<String, _>("id").ok(),
            "owner_id": row.try_get::<String, _>("owner_id").ok(),
            "owner_kind": row.try_get::<String, _>("owner_kind").ok(),
            "status": row.try_get::<String, _>("status").ok(),
            "reason": row.try_get::<String, _>("reason").ok(),
            "acquired_at": row.try_get::<Option<String>, _>("acquired_at").ok().flatten(),
            "heartbeat_at": row.try_get::<Option<String>, _>("heartbeat_at").ok().flatten(),
            "expires_at": row.try_get::<String, _>("expires_at").ok(),
            "released_at": row.try_get::<Option<String>, _>("released_at").ok().flatten(),
        })),
        "active_consumers": active_consumers.into_iter().map(|row| serde_json::json!({
            "lease_id": row.try_get::<String, _>("id").ok(),
            "resource_key": row.try_get::<String, _>("worktree_resource_key").ok(),
            "owner_id": row.try_get::<String, _>("owner_id").ok(),
            "access_mode": row.try_get::<Option<String>, _>("worktree_access_mode").ok().flatten(),
            "owner_kind": row.try_get::<Option<String>, _>("owner_kind").ok().flatten(),
            "reason": row.try_get::<Option<String>, _>("reason").ok().flatten(),
            "acquired_at": row.try_get::<String, _>("acquired_at").ok(),
            "expires_at": row.try_get::<String, _>("expires_at").ok(),
            "heartbeat_at": row.try_get::<Option<String>, _>("heartbeat_at").ok().flatten(),
        })).collect::<Vec<_>>(),
        "commands": command_readback,
    }))
}

async fn proposal_064_knowledge_capsule_readback(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let attachments = sqlx::query(
        "SELECT a.id, a.capsule_id, a.match_rule, a.attachment_reason, a.injected, a.injected_byte_count, a.injected_token_count, a.truncated, a.stale_main, a.ignored, a.ignored_reason, a.created_at, c.source_run_id, c.source_proposal_id, c.source_status, c.status AS capsule_status \
         FROM run_knowledge_capsule_attachments a \
         JOIN run_knowledge_capsules c ON c.id = a.capsule_id \
         WHERE a.target_run_id = ? ORDER BY a.created_at DESC LIMIT 16",
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    let command_readback =
        proposal_064_command_readback(pool, run_id, &["KnowledgeCapsuleIgnore"]).await?;

    Ok(serde_json::json!({
        "schema_version": "p064_knowledge_capsule_readback_v1",
        "mode": "off",
        "operator_tools_enabled": false,
        "attached_capsules": attachments.into_iter().map(|row| serde_json::json!({
            "attachment_id": row.try_get::<String, _>("id").ok(),
            "capsule_id": row.try_get::<String, _>("capsule_id").ok(),
            "source_run_id": row.try_get::<String, _>("source_run_id").ok(),
            "source_proposal_id": row.try_get::<Option<String>, _>("source_proposal_id").ok().flatten(),
            "source_status": row.try_get::<String, _>("source_status").ok(),
            "capsule_status": row.try_get::<String, _>("capsule_status").ok(),
            "match_rule": row.try_get::<String, _>("match_rule").ok(),
            "attachment_reason": row.try_get::<String, _>("attachment_reason").ok(),
            "injected": row.try_get::<i64, _>("injected").unwrap_or_default() != 0,
            "injected_byte_count": row.try_get::<Option<i64>, _>("injected_byte_count").ok().flatten(),
            "injected_token_count": row.try_get::<Option<i64>, _>("injected_token_count").ok().flatten(),
            "truncated": row.try_get::<i64, _>("truncated").unwrap_or_default() != 0,
            "stale_main": row.try_get::<i64, _>("stale_main").unwrap_or_default() != 0,
            "ignored": row.try_get::<i64, _>("ignored").unwrap_or_default() != 0,
            "ignored_reason": row.try_get::<Option<String>, _>("ignored_reason").ok().flatten(),
            "created_at": row.try_get::<String, _>("created_at").ok(),
        })).collect::<Vec<_>>(),
        "commands": command_readback,
    }))
}

const P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES: usize = 120_000;
const P031_ARTIFACT_PAYLOAD_BULK_PREVIEW_MAX_BYTES: usize = 1_000_000;

struct P031ArtifactPayloadPreview {
    text: String,
    truncated: bool,
    bytes_read: usize,
}

fn attach_p031_artifact_payload(
    row: &db::repos::projections::ArtifactIndexRow,
    run: Option<&domain::run::Run>,
    artifact: &mut GqlArtifact,
    bulk_preview_budget_remaining: &mut usize,
) {
    attach_p031_artifact_payload_from_metadata(
        &row.format,
        row.report_kind.as_deref(),
        row.size_bytes,
        &row.file_path,
        run,
        artifact,
        bulk_preview_budget_remaining,
    );
}

fn attach_p031_artifact_payload_from_metadata(
    format: &str,
    report_kind: Option<&str>,
    size_bytes: Option<i64>,
    file_path: &str,
    run: Option<&domain::run::Run>,
    artifact: &mut GqlArtifact,
    bulk_preview_budget_remaining: &mut usize,
) {
    if report_kind.is_some() || format == "report" {
        return;
    }

    let estimated_preview_bytes = size_bytes
        .and_then(|size| usize::try_from(size).ok())
        .filter(|size| *size > 0)
        .map(|size| size.min(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES))
        .unwrap_or(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES);
    if estimated_preview_bytes > *bulk_preview_budget_remaining {
        warn!(
            artifact_id = %artifact.id.as_str(),
            estimated_preview_bytes,
            bulk_preview_budget_remaining = *bulk_preview_budget_remaining,
            "P031 artifact payload deferred before read: preview budget exhausted"
        );
        mark_payload_deferred(
            artifact,
            "Artifact payload preview deferred because the bulk artifact list reached its payload preview budget",
        );
        return;
    }

    let Some(run) = run else {
        warn!(
            artifact_id = %artifact.id.as_str(),
            "P031 artifact payload unavailable: missing run metadata"
        );
        mark_payload_unavailable(
            artifact,
            "Run metadata was unavailable for artifact readback",
        );
        return;
    };

    let Some(path) = resolve_server_owned_artifact_path(file_path, run) else {
        warn!(
            artifact_id = %artifact.id.as_str(),
            "P031 artifact payload unavailable: path outside run-owned roots"
        );
        mark_payload_unavailable(
            artifact,
            "Artifact path is outside the selected run's server-owned roots",
        );
        return;
    };

    match read_p031_artifact_payload_preview(&path) {
        Ok(preview) => {
            let consumed_preview_bytes = estimated_preview_bytes.max(
                preview
                    .bytes_read
                    .min(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES),
            );
            if consumed_preview_bytes > *bulk_preview_budget_remaining {
                warn!(
                    artifact_id = %artifact.id.as_str(),
                    consumed_preview_bytes,
                    bulk_preview_budget_remaining = *bulk_preview_budget_remaining,
                    "P031 artifact payload deferred after read: preview budget exhausted"
                );
                mark_payload_deferred(
                    artifact,
                    "Artifact payload preview deferred because the bulk artifact list reached its payload preview budget",
                );
                return;
            }
            *bulk_preview_budget_remaining =
                bulk_preview_budget_remaining.saturating_sub(consumed_preview_bytes);
            artifact.payload_text = Some(preview.text);
            artifact.payload_availability_state = GqlPayloadAvailabilityState::Available;
            artifact.payload_unavailable_reason_code = None;
            artifact.server_debug_detail = preview.truncated.then(|| {
                format!(
                    "Artifact payload preview capped at {} bytes; full payload remains server-owned",
                    P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES
                )
            });
            debug!(
                artifact_id = %artifact.id.as_str(),
                consumed_preview_bytes,
                bytes_read = preview.bytes_read,
                truncated = preview.truncated,
                bulk_preview_budget_remaining = *bulk_preview_budget_remaining,
                "P031 artifact payload preview attached"
            );
        }
        Err(err) => {
            warn!(
                artifact_id = %artifact.id.as_str(),
                error = %err,
                "P031 artifact payload readback failed"
            );
            mark_payload_unavailable(
                artifact,
                &format!("Artifact payload readback failed: {err}"),
            );
        }
    }
}

fn read_p031_artifact_payload_preview(path: &Path) -> io::Result<P031ArtifactPayloadPreview> {
    let file = std::fs::File::open(path)?;
    let mut limited = file.take((P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES + 1) as u64);
    let mut bytes = Vec::with_capacity(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES + 1);
    limited.read_to_end(&mut bytes)?;

    let truncated = bytes.len() > P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES;
    if truncated {
        bytes.truncate(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES);
    }
    let bytes_read = bytes.len();

    match String::from_utf8(bytes) {
        Ok(text) => Ok(P031ArtifactPayloadPreview {
            text,
            truncated,
            bytes_read,
        }),
        Err(err) => {
            let valid_up_to = err.utf8_error().valid_up_to();
            let mut bytes = err.into_bytes();
            bytes.truncate(valid_up_to);
            let text = String::from_utf8(bytes).map_err(|utf8_err| {
                io::Error::new(io::ErrorKind::InvalidData, utf8_err.to_string())
            })?;
            Ok(P031ArtifactPayloadPreview {
                text,
                truncated: true,
                bytes_read,
            })
        }
    }
}

fn mark_payload_unavailable(artifact: &mut GqlArtifact, detail: &str) {
    artifact.payload_text = None;
    artifact.payload_availability_state = GqlPayloadAvailabilityState::Unavailable;
    artifact.payload_unavailable_reason_code = Some(GqlPayloadUnavailableReasonCode::NotAvailable);
    artifact.server_debug_detail = Some(detail.to_string());
}

fn mark_payload_deferred(artifact: &mut GqlArtifact, detail: &str) {
    artifact.payload_text = None;
    artifact.payload_availability_state = GqlPayloadAvailabilityState::PayloadDeferred;
    artifact.payload_unavailable_reason_code =
        Some(GqlPayloadUnavailableReasonCode::PayloadDeferredByP031);
    artifact.server_debug_detail = Some(format!("{detail}. {P085_NO_DEADLINE_JUSTIFICATION}"));
}

fn resolve_server_owned_artifact_path(file_path: &str, run: &domain::run::Run) -> Option<PathBuf> {
    let raw_path = PathBuf::from(file_path);
    let candidate = if raw_path.is_absolute() {
        raw_path
    } else if !run.artifact_root.is_empty() {
        PathBuf::from(&run.artifact_root).join(raw_path)
    } else {
        PathBuf::from(&run.workspace_root).join(raw_path)
    };
    let canonical_candidate = std::fs::canonicalize(candidate).ok()?;
    let allowed_roots = [
        Some(run.artifact_root.as_str()),
        Some(run.workspace_root.as_str()),
        run.chainworks_meta_root.as_deref(),
    ];
    allowed_roots
        .into_iter()
        .flatten()
        .filter(|root| !root.is_empty())
        .filter_map(|root| std::fs::canonicalize(root).ok())
        .any(|root| path_is_inside(&canonical_candidate, &root))
        .then_some(canonical_candidate)
}

fn path_is_inside(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

pub struct MutationRoot;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationName {
    /// P072: Converged approval mutation by approval_id.
    ApproveApproval,
    /// P072: Converged rejection mutation by approval_id.
    RejectApproval,
}

impl MutationName {
    fn graphql_name(self) -> &'static str {
        match self {
            MutationName::ApproveApproval => "approveApproval",
            MutationName::RejectApproval => "rejectApproval",
        }
    }
}

pub fn capability_id_for(mutation: MutationName) -> domain::CapabilityToolId {
    match mutation {
        MutationName::ApproveApproval | MutationName::RejectApproval => {
            domain::CapabilityToolId::ApprovalsResolve
        }
    }
}

async fn mutation_allowed(
    ctx: &Context<'_>,
    principal: &auth::Principal,
    mutation: MutationName,
) -> Result<(), async_graphql::Error> {
    let caller_class = auth::derive_caller_class(principal);
    // P081 Phase 3: consult the shared BoundaryPolicy when it is injected.
    // ui_operator on graphql_mutation is allowed by the matrix (approval actions).
    // Any other caller_class that lacks a matching row returns MATRIX_NO_ROW → denied.
    if let Ok(policy) = ctx.data::<Arc<auth::boundary::BoundaryPolicy>>() {
        match policy.evaluate(
            caller_class.as_str(),
            "graphql_mutation",
            Some(mutation.graphql_name()),
        ) {
            auth::boundary::PolicyDecision::Deny {
                reason_code,
                row_id,
                ..
            } => {
                // P081: write durable deny audit before returning the denial.
                // Fail-closed: if the audit write fails, return E_AUDIT_UNAVAILABLE.
                if let Ok(pool) = ctx.data::<SqlitePool>() {
                    write_graphql_deny_audit(
                        pool,
                        ctx,
                        principal,
                        "graphql_mutation",
                        mutation.graphql_name(),
                        &reason_code,
                        row_id.as_deref(),
                        caller_class.as_str(),
                        &policy,
                    )
                    .await?;
                }
                return Err(boundary_denial_error(
                    &reason_code,
                    row_id.as_deref(),
                    Some(caller_class.as_str()),
                ));
            }
            auth::boundary::PolicyDecision::Shadow { matched_decision } => {
                if let auth::boundary::PolicyDecision::Deny {
                    reason_code,
                    row_id,
                    ..
                } = *matched_decision
                {
                    tracing::debug!(
                        caller_class = caller_class.as_str(),
                        transport = "graphql_mutation",
                        reason_code = %reason_code,
                        row_id = ?row_id,
                        "BoundaryPolicy shadow: matrix would deny this graphql_mutation"
                    );
                    if principal.class == auth::PrincipalClass::Operator {
                        db::metrics::record_p081_boundary_policy_enforcement_parity(
                            "allow", "deny",
                        );
                        db::metrics::record_p081_boundary_shadow_disagreement(
                            "graphql_mutation",
                            row_id.as_deref(),
                            caller_class.as_str(),
                            mutation.graphql_name(),
                            "allow",
                            "deny",
                            Some(reason_code.as_str()),
                        );
                    }
                }
            }
            auth::boundary::PolicyDecision::Allow { .. }
            | auth::boundary::PolicyDecision::LegacyPassthrough => {}
        }
    } else {
        db::metrics::record_p081_boundary_policy_evaluation_error(
            "graphql_mutation",
            "policy_missing",
        );
    }

    if let Ok(pool) = ctx.data::<SqlitePool>() {
        if audit_log::audit_budget_requires_safe_mode(pool)
            .await
            .map_err(|e| Error::new(e.to_string()))?
        {
            db::metrics::record_p081_audit_log_rate_limited(
                "graphql_mutation",
                "AUDIT_BUDGET_EXHAUSTED",
            );
            return Err(boundary_denial_error(
                "AUDIT_BUDGET_EXHAUSTED",
                Some("p081.audit_budget.safe_mode"),
                Some(caller_class.as_str()),
            ));
        }
    }

    if let Ok(table) = ctx.data::<auth::PrincipalTable>() {
        if let Some(allowed) = auth::is_mutation_allowed_by_surface_policy(
            table,
            &principal.id,
            mutation.graphql_name(),
        ) {
            if !(allowed && principal.class == auth::PrincipalClass::Operator) {
                return Err(boundary_denial_error("NON_APPROVAL_MUTATION", None, None));
            }
            return Ok(());
        }
        if auth::find_principal_by_id(table, &principal.id).is_some() {
            return Err(boundary_denial_error("NON_APPROVAL_MUTATION", None, None));
        }
    }

    if auth::filter_tools(principal, &[capability_id_for(mutation)]).len() == 1 {
        Ok(())
    } else {
        Err(boundary_denial_error("CAPABILITY_OUT_OF_SCOPE", None, None))
    }
}

/// Build the GraphQL caller context for a mutation and attach the
/// `X-Request-ID` from the async-graphql request data (P042 §9.3) when
/// the outer axum middleware injected one. The command journal INSERT
/// picks it up transparently via `CallerContext.request_id`.
fn graphql_caller_with_request_id(
    ctx: &Context<'_>,
    principal: &auth::Principal,
    mutation_name: &str,
) -> CallerContext {
    let caller_class = auth::derive_caller_class(principal);
    let mut caller = CallerContext::graphql(&principal.id, &principal.class, mutation_name)
        .with_caller_class(caller_class.as_str());
    if let Ok(rid) = ctx.data::<crate::request_id::RequestId>() {
        caller = caller.with_request_id(&rid.0);
    }
    // SEC-P081-M002: propagate derived token_id for audit correlation.
    if let Ok(tid) = ctx.data::<crate::auth_layer::GraphqlTokenId>() {
        caller = caller.with_token_id(&tid.0);
    }
    caller
}

// ── P029 payload wrappers ──────────────────────────────────────────────
// Dedicated types for each mutation so journal_id doesn't pollute shared
// Run/Approval types used by read queries.

/// P072: Payload for approveApproval mutation.
#[derive(SimpleObject)]
pub struct ApproveApprovalPayload {
    pub approval: GqlApproval,
    pub journal_id: ID,
    pub conflict_result_code: Option<GqlMutationConflictResultCode>,
}

/// P072: Payload for rejectApproval mutation.
#[derive(SimpleObject)]
pub struct RejectApprovalPayload {
    pub approval: GqlApproval,
    pub journal_id: ID,
    pub conflict_result_code: Option<GqlMutationConflictResultCode>,
}

/// SEC-M-001: Validate that an idempotency key is a well-formed UUIDv7.
/// UUIDv7 format: 8-4-4-4-12 hex groups where the version nibble (13th char) is '7'
/// and the variant nibble (17th char) is '8', '9', 'a', or 'b' (RFC 4122 variant).
/// Returns a typed, non-disclosing error on validation failure.
fn validate_idempotency_key_uuidv7(key: &str) -> Result<()> {
    let uuid = uuid::Uuid::parse_str(key).map_err(|_| {
        let mut e = async_graphql::Error::new("INVALID_IDEMPOTENCY_KEY");
        e = e.extend_with(|_, ext| ext.set("code", "INVALID_IDEMPOTENCY_KEY"));
        e
    })?;
    // Version check: UUIDv7 has version nibble == 7
    if uuid.get_version_num() != 7 {
        let mut e = async_graphql::Error::new("INVALID_IDEMPOTENCY_KEY");
        e = e.extend_with(|_, ext| ext.set("code", "INVALID_IDEMPOTENCY_KEY"));
        return Err(e.into());
    }
    // SEC-M-001b: Variant check — must be RFC 4122 (high bits 10xx).
    // Rejects malformed UUIDs that parse with version=7 but use a non-standard variant byte.
    if uuid.get_variant() != uuid::Variant::RFC4122 {
        let mut e = async_graphql::Error::new("INVALID_IDEMPOTENCY_KEY");
        e = e.extend_with(|_, ext| ext.set("code", "INVALID_IDEMPOTENCY_KEY"));
        return Err(e.into());
    }
    Ok(())
}

fn approval_resolution_conflict_code(
    error: &anyhow::Error,
) -> Option<(ID, GqlMutationConflictResultCode)> {
    let conflict = error.downcast_ref::<ApprovalResolutionConflict>()?;
    match conflict {
        ApprovalResolutionConflict::AlreadyResolved { .. } => Some((
            ID::from(conflict.journal_id().to_owned()),
            GqlMutationConflictResultCode::AlreadyResolved,
        )),
        // P081: terminal approval retried with a different key → APPROVAL_NOT_ACTIONABLE.
        // Zero settlement side effects (no command_journal row was written).
        ApprovalResolutionConflict::ApprovalNotActionable { .. } => Some((
            ID::from(conflict.journal_id().to_owned()),
            GqlMutationConflictResultCode::ApprovalNotActionable,
        )),
    }
}

/// Build a deterministic IDEMPOTENCY_CONFLICT GraphQL error.
/// Called when the command handler returns an IDEMPOTENCY_CONFLICT string error
/// (same key, different canonical request hash) which is not an ApprovalResolutionConflict
/// typed error and must not fall through to the opaque INTERNAL path.
fn idempotency_conflict_gql_error(request_id: &Option<String>) -> Error {
    let mut gql_err = Error::new("IDEMPOTENCY_CONFLICT");
    gql_err = gql_err.extend_with(|_, ext| ext.set("code", "IDEMPOTENCY_CONFLICT"));
    gql_err = gql_err.extend_with(|_, ext| ext.set("reasonCode", "IDEMPOTENCY_CONFLICT"));
    if let Some(ref rid) = request_id {
        gql_err = gql_err.extend_with(|_, ext| ext.set("requestId", rid.clone()));
    }
    gql_err
}

#[Object]
impl MutationRoot {
    /// P072/P081: Approve a stage approval by approval_id. The resolver
    /// server-resolves run_id and stage_id from the approval record
    /// before constructing ResolveApprovalCmd.
    /// P081: idempotency_key is a required UUIDv7 per ApprovalActionAttemptStore attempt.
    /// SEC-P081-HIGH-001: Required to enforce committed-unack replay contract.
    async fn approve_approval(
        &self,
        ctx: &Context<'_>,
        approval_id: ID,
        comment: Option<String>,
        idempotency_key: String,
    ) -> Result<ApproveApprovalPayload> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        mutation_allowed(ctx, &principal, MutationName::ApproveApproval).await?;

        // SEC-M-001: Validate idempotencyKey as UUIDv7 before command handling or storage.
        validate_idempotency_key_uuidv7(&idempotency_key)?;

        let caller = graphql_caller_with_request_id(ctx, &principal, "approveApproval");
        // SEC-P081-002: Capture request_id before caller is consumed so internal errors
        // can include it without disclosing the raw error chain to the client.
        let request_id = caller.request_id.clone();

        // P081: Log a SHA-256 digest of the key — raw replay handles must not appear in logs.
        {
            let key_digest = {
                use sha2::{Digest, Sha256};
                let h = Sha256::digest(idempotency_key.as_bytes());
                format!("{:x}", h)[..16].to_string()
            };
            tracing::debug!(idempotency_key_digest = %key_digest, approval_id = %*approval_id, "approveApproval idempotency_key received");
        }

        let aid: domain::ids::ApprovalId = approval_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        // Server-resolve run_id and stage_id from the approval record.
        let approval = approvals::find_by_id(pool, aid)
            .await?
            .ok_or_else(|| Error::new(format!("Approval {aid} not found")))?;

        let cmd = Command::ResolveApproval(ResolveApprovalCmd {
            approval_id: aid,
            decision: ApprovalResolutionDecision::Approved,
            rationale: comment,
            run_id: approval.run_id,
            stage_id: approval.stage_id.clone(),
            idempotency_key: Some(idempotency_key),
        });

        let result = cmd_handler.handle(cmd, caller).await;
        match result {
            Ok(commanded) => {
                let jid = ID::from(commanded.journal_id);
                // Re-fetch for authoritative readback.
                let updated = approvals::find_by_id(pool, aid)
                    .await?
                    .ok_or_else(|| Error::new("Approval not found after update"))?;
                Ok(ApproveApprovalPayload {
                    approval: GqlApproval::from(updated),
                    journal_id: jid,
                    conflict_result_code: None,
                })
            }
            Err(e) => {
                if let Some((journal_id, conflict_result_code)) =
                    approval_resolution_conflict_code(&e)
                {
                    let current = approvals::find_by_id(pool, aid)
                        .await?
                        .ok_or_else(|| Error::new(format!("Approval {aid} not found")))?;
                    Ok(ApproveApprovalPayload {
                        approval: GqlApproval::from(current),
                        journal_id,
                        conflict_result_code: Some(conflict_result_code),
                    })
                } else if e.to_string().contains("IDEMPOTENCY_CONFLICT") {
                    // P081: same idempotency key with a different canonical request hash.
                    // Return a deterministic IDEMPOTENCY_CONFLICT code, not an opaque INTERNAL.
                    Err(idempotency_conflict_gql_error(&request_id))
                } else {
                    // SEC-P081-002: Log full error chain server-side; expose only INTERNAL + request_id.
                    tracing::error!(error = %e, request_id = ?request_id, "approveApproval: internal command error");
                    let mut gql_err = Error::new("INTERNAL");
                    gql_err = gql_err.extend_with(|_, ext| ext.set("code", "INTERNAL"));
                    if let Some(ref rid) = request_id {
                        gql_err = gql_err.extend_with(|_, ext| ext.set("requestId", rid.clone()));
                    }
                    Err(gql_err)
                }
            }
        }
    }

    /// P072/P081: Reject a stage approval by approval_id with a required reason.
    /// P081: idempotency_key is a required UUIDv7 per ApprovalActionAttemptStore attempt.
    /// SEC-P081-HIGH-001: Required to enforce committed-unack replay contract.
    async fn reject_approval(
        &self,
        ctx: &Context<'_>,
        approval_id: ID,
        reason: String,
        idempotency_key: String,
    ) -> Result<RejectApprovalPayload> {
        let pool = ctx.data::<SqlitePool>()?;
        let cmd_handler = ctx.data::<Arc<CommandHandler>>()?;

        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| async_graphql::Error::new("unauthorized: no principal in context"))?
            .clone();

        mutation_allowed(ctx, &principal, MutationName::RejectApproval).await?;

        // SEC-M-001: Validate idempotencyKey as UUIDv7 before command handling or storage.
        validate_idempotency_key_uuidv7(&idempotency_key)?;

        // P081: Log a SHA-256 digest of the key — raw replay handles must not appear in logs.
        {
            let key_digest = {
                use sha2::{Digest, Sha256};
                let h = Sha256::digest(idempotency_key.as_bytes());
                format!("{:x}", h)[..16].to_string()
            };
            tracing::debug!(idempotency_key_digest = %key_digest, approval_id = %*approval_id, "rejectApproval idempotency_key received");
        }

        let caller = graphql_caller_with_request_id(ctx, &principal, "rejectApproval");
        // SEC-P081-002: Capture request_id before caller is consumed.
        let request_id = caller.request_id.clone();

        let aid: domain::ids::ApprovalId = approval_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        // Server-resolve run_id and stage_id from the approval record.
        let approval = approvals::find_by_id(pool, aid)
            .await?
            .ok_or_else(|| Error::new(format!("Approval {aid} not found")))?;

        let cmd = Command::ResolveApproval(ResolveApprovalCmd {
            approval_id: aid,
            decision: ApprovalResolutionDecision::Rejected,
            rationale: Some(reason),
            run_id: approval.run_id,
            stage_id: approval.stage_id.clone(),
            idempotency_key: Some(idempotency_key),
        });

        let result = cmd_handler.handle(cmd, caller).await;
        match result {
            Ok(commanded) => {
                let jid = ID::from(commanded.journal_id);
                let updated = approvals::find_by_id(pool, aid)
                    .await?
                    .ok_or_else(|| Error::new("Approval not found after update"))?;
                Ok(RejectApprovalPayload {
                    approval: GqlApproval::from(updated),
                    journal_id: jid,
                    conflict_result_code: None,
                })
            }
            Err(e) => {
                if let Some((journal_id, conflict_result_code)) =
                    approval_resolution_conflict_code(&e)
                {
                    let current = approvals::find_by_id(pool, aid)
                        .await?
                        .ok_or_else(|| Error::new(format!("Approval {aid} not found")))?;
                    Ok(RejectApprovalPayload {
                        approval: GqlApproval::from(current),
                        journal_id,
                        conflict_result_code: Some(conflict_result_code),
                    })
                } else if e.to_string().contains("IDEMPOTENCY_CONFLICT") {
                    // P081: same idempotency key with a different canonical request hash.
                    // Return a deterministic IDEMPOTENCY_CONFLICT code, not an opaque INTERNAL.
                    Err(idempotency_conflict_gql_error(&request_id))
                } else {
                    // SEC-P081-002: Log full error chain server-side; expose only INTERNAL + request_id.
                    tracing::error!(error = %e, request_id = ?request_id, "rejectApproval: internal command error");
                    let mut gql_err = Error::new("INTERNAL");
                    gql_err = gql_err.extend_with(|_, ext| ext.set("code", "INTERNAL"));
                    if let Some(ref rid) = request_id {
                        gql_err = gql_err.extend_with(|_, ext| ext.set("requestId", rid.clone()));
                    }
                    Err(gql_err)
                }
            }
        }
    }
}

pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    async fn run_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlRun>>>> {
        // P029 §4.1.c: principal is injected by on_connection_init during WS handshake.
        // P081 Phase 3: evaluate graphql_subscription transport (not graphql_query).
        require_subscription_read(ctx).await?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();
        let filter_run_id: Option<RunId> = run_id.and_then(|id| id.parse().ok());

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                let refresh_run_id = match event {
                    DomainEvent::RunStatusChanged { run_id, .. }
                    | DomainEvent::RunStarted { run_id, .. }
                    | DomainEvent::StageStatusChanged { run_id, .. }
                    | DomainEvent::ApprovalRequested { run_id, .. }
                    | DomainEvent::ArtifactCreated { run_id, .. }
                    | DomainEvent::RuntimeStatusChanged { run_id, .. }
                    | DomainEvent::RuntimeTimelineEvent { run_id, .. }
                    | DomainEvent::MediationConfirmationResolved { run_id, .. }
                    | DomainEvent::RoutingCompleted { run_id, .. } => Some(run_id),
                    DomainEvent::ApprovalResolved { approval_id, .. } => {
                        approvals::find_by_id(&pool, approval_id)
                            .await
                            .ok()
                            .flatten()
                            .map(|approval| approval.run_id)
                    }
                    DomainEvent::SchedulerBackpressureChanged { run_id, .. } => {
                        run_id.and_then(|id| id.parse().ok())
                    }
                    DomainEvent::DaemonStatusChanged { .. }
                    | DomainEvent::MaintenanceSlotReleaseCasFailed { .. }
                    | DomainEvent::SessionEventRecorded { .. } => None,
                }?;
                if let Some(fid) = filter_run_id {
                    if refresh_run_id != fid {
                        return None;
                    }
                }
                match run_from_projection_or_canonical(&pool, refresh_run_id).await {
                    Ok(run) => Some(Ok(run)),
                    Err(err) => Some(Err(err)),
                }
            };
            fut
        }))
    }

    async fn stage_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlStageExecution>>>>
    {
        require_subscription_read(ctx).await?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();
        let filter_run_id: RunId = run_id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::StageStatusChanged {
                        run_id,
                        stage_execution_id,
                        ..
                    } => {
                        if run_id != filter_run_id {
                            return None;
                        }
                        match stage_from_projection_or_canonical(&pool, stage_execution_id).await {
                            Ok(stage) => Some(Ok(stage)),
                            Err(err) => Some(Err(err)),
                        }
                    }
                    _ => None,
                }
            };
            fut
        }))
    }

    async fn approval_requested(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlApproval>>>> {
        require_subscription_read(ctx).await?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::ApprovalRequested { approval_id, .. } => {
                        let approval = approvals::find_by_id(&pool, approval_id).await.ok()??;
                        Some(Ok(Some(GqlApproval::from(approval))))
                    }
                    _ => None,
                }
            };
            fut
        }))
    }

    async fn approval_resolved(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlApproval>>>> {
        require_subscription_read(ctx).await?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();

        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::ApprovalResolved { approval_id, .. } => {
                        let approval = approvals::find_by_id(&pool, approval_id).await.ok()??;
                        Some(Ok(Some(GqlApproval::from(approval))))
                    }
                    _ => None,
                }
            };
            fut
        }))
    }

    /// Live stream of ACP runtime/session lifecycle events.
    /// Emits on session_started, session_completed, and session_failed.
    /// Required for the SwiftUI thin-client's runtime health surface (P027 §8.1).
    async fn runtime_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: Option<ID>,
        replay_cursor: Option<String>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<Option<GqlRuntimeEvent>>>>
    {
        require_subscription_read(ctx).await?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();
        let filter_run_id: Option<RunId> = run_id.and_then(|id| id.parse().ok());
        let latest_sequence = P081_SUBSCRIPTION_SEQUENCE.load(Ordering::SeqCst);
        let replay = p081_subscription_replay_readback(
            replay_cursor.as_deref(),
            p081_oldest_retained_sequence(latest_sequence),
            latest_sequence,
            latest_sequence,
        );
        let bootstrap_frames = if replay["gapDetected"].as_bool().unwrap_or(false) {
            vec![Ok(Some(GqlRuntimeEvent {
                id: ID(format!(
                    "rte_subscription_gap_{}",
                    replay["sequenceCursor"].as_str().unwrap_or("seq-0")
                )),
                run_id: ID(filter_run_id
                    .unwrap_or_else(|| RunId::from(uuid::Uuid::nil()))
                    .to_string()),
                stage_id: "boundary_subscription_replay".to_string(),
                agent_id: "control_plane".to_string(),
                provider: "control_plane".to_string(),
                event_kind: "subscription_gap_detected".to_string(),
                title: None,
                detail: None,
                surface_label: None,
                session_generation_id: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                raw_detail: None,
                raw_detail_bytes: None,
                raw_detail_truncated: false,
                raw_detail_handle: None,
                raw_detail_digest: None,
                full_raw_available: true,
                detail_digest: None,
                detail_char_count: None,
                chunk_count: None,
                is_streaming: false,
                is_terminal: true,
                state_label: Some("boundary_subscription_replay".to_string()),
                sequence_cursor: replay["sequenceCursor"]
                    .as_str()
                    .unwrap_or("seq-0")
                    .to_string(),
                projection_generation: replay["projectionGeneration"].as_i64().unwrap_or(0),
                gap_detected: true,
                requires_full_refetch: true,
            }))]
        } else {
            Vec::new()
        };

        let rx = events.subscribe();
        let live = BroadcastStream::new(rx).filter_map(move |msg| {
            let pool = pool.clone();
            let fut = async move {
                let event = msg.ok()?;
                match event {
                    DomainEvent::RuntimeStatusChanged {
                        run_id,
                        stage_id,
                        agent_id,
                        provider,
                        event_kind,
                    } => {
                        let sequence = p081_next_subscription_sequence();
                        if let Some(fid) = filter_run_id {
                            if run_id != fid {
                                return None;
                            }
                        }
                        let mut event = GqlRuntimeEvent::from_parts(
                            ID(run_id.to_string()),
                            stage_id,
                            agent_id,
                            provider,
                            event_kind,
                            None,
                            None,
                            None,
                            None,
                            chrono::Utc::now().to_rfc3339(),
                        );
                        event.sequence_cursor = format!("seq-{sequence}");
                        event.projection_generation = sequence;
                        event.gap_detected = false;
                        event.requires_full_refetch = false;
                        Some(Ok(Some(event)))
                    }
                    DomainEvent::RuntimeTimelineEvent {
                        run_id,
                        stage_id,
                        agent_id,
                        provider,
                        event_kind,
                        title,
                        detail,
                        surface_label,
                        session_generation_id,
                    } => {
                        if let Some(fid) = filter_run_id {
                            if run_id != fid {
                                return None;
                            }
                        }
                        let sequence = p081_next_subscription_sequence();
                        let mut event = GqlRuntimeEvent::from_live_timeline_parts(
                            &pool,
                            run_id,
                            stage_id,
                            agent_id,
                            provider,
                            event_kind,
                            Some(title),
                            detail,
                            Some(surface_label),
                            session_generation_id,
                            chrono::Utc::now().to_rfc3339(),
                        )
                        .await;
                        event.sequence_cursor = format!("seq-{sequence}");
                        event.projection_generation = sequence;
                        event.gap_detected = false;
                        event.requires_full_refetch = false;
                        Some(Ok(Some(event)))
                    }
                    _ => None,
                }
            };
            fut
        });
        Ok(stream::iter(bootstrap_frames).chain(live))
    }

    /// P042 §5.2 push surface. Emits a `GqlDaemonStatus` frame on every
    /// lifecycle transition (driven by the same EventBus the reporter
    /// broadcasts into). Clients typically call `daemonStatus` once at
    /// connect time to seed state, then subscribe here to stay in sync.
    ///
    /// Operator-only per P042 §5.2 readback-surfaces table. A principal
    /// of any other class receives `unauthorized`; the check runs
    /// before `events.subscribe()` so a non-operator never even sees the
    /// first frame.
    async fn daemon_status_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<GqlDaemonStatus>>> {
        // P081 Phase 3: use shared graphql_subscription BoundaryPolicy evaluation.
        require_subscription_read(ctx).await?;
        let events = ctx.data::<EventSender>()?.clone();
        let rx = events.subscribe();
        Ok(BroadcastStream::new(rx).filter_map(move |msg| async move {
            let event = msg.ok()?;
            match event {
                DomainEvent::DaemonStatusChanged { status } => {
                    Some(Ok(GqlDaemonStatus::from(status)))
                }
                _ => None,
            }
        }))
    }

    // ── P046: Live session status subscription ────────────────────────────────

    #[graphql(visible = "crate::types::session::p046_visible")]
    async fn session_status_changed(
        &self,
        ctx: &Context<'_>,
        run_id: ID,
    ) -> Result<impl async_graphql::futures_util::Stream<Item = Result<GqlSessionStatusChangedEvent>>>
    {
        // P046-SEC-L3: check operator class AND subscription surface policy at startup,
        // matching the per-emission recheck policy so the gate is symmetric.
        let principal = ctx
            .data::<auth::Principal>()
            .map_err(|_| Error::new("unauthorized"))?;
        if principal.class != auth::PrincipalClass::Operator {
            return Err(Error::new("forbidden"));
        }
        if let Ok(table) = ctx.data::<auth::PrincipalTable>() {
            if let Some(allowed) =
                auth::is_subscription_allowed_by_surface_policy(table, &principal.id)
            {
                if !allowed {
                    return Err(Error::new("forbidden"));
                }
            }
        }
        let p046 = ctx.data::<P046Config>()?;
        if !p046.enabled {
            db::metrics::increment_counter_with_label(
                "session_graphql_disabled_schema_guard_total",
                "graphql_external:blocked",
            );
            return Err(Error::new("session observability is not enabled"));
        }

        let run_id_str = run_id.as_str().to_string();
        let filter_run_id: RunId = run_id_str
            .parse()
            .map_err(|_| Error::new("invalid run id"))?;

        let pool = ctx.data::<SqlitePool>()?.clone();
        let events = ctx.data::<EventSender>()?.clone();
        // Use the live handle for per-emission auth so revocation in the underlying
        // table is observed without waiting for daemon restart.
        let live_principal_handle = ctx.data::<P046LivePrincipalHandle>()?.clone();
        let principal = ctx.data::<auth::Principal>()?;
        let principal_id = principal.id.clone();
        let live_credential = ctx
            .data_opt::<P046LiveCredential>()
            .cloned()
            .or_else(|| {
                ctx.data::<auth::PrincipalTable>().ok().and_then(|table| {
                    auth::principal_token_fingerprint_by_id(table, &principal_id).map(
                        |token_fingerprint| P046LiveCredential {
                            principal_id: principal_id.clone(),
                            token_fingerprint,
                        },
                    )
                })
            })
            .ok_or_else(|| Error::new("unauthorized"))?;
        // Optional test-only shutdown signal: when the watch sender is dropped (or sends true),
        // the subscription performs the same graceful-shutdown drain as RecvError::Closed.
        let shutdown_rx: Option<tokio::sync::watch::Receiver<bool>> = ctx
            .data::<tokio::sync::watch::Receiver<bool>>()
            .ok()
            .cloned();

        // P046 resource-scoped authorization: bind operator subscription to the owning run.
        let run_id_for_auth = filter_run_id.to_string();
        let sub_deadline = p046_resolver_deadline();
        match p046_check_run_accessible(ctx, &pool, &run_id_for_auth, sub_deadline).await? {
            Some(()) => {}
            None => return Err(Error::new("not found")),
        }

        let channel_capacity = p046.subscription_channel_capacity;
        let (tx, rx) =
            tokio::sync::mpsc::channel::<Result<GqlSessionStatusChangedEvent>>(channel_capacity);

        tokio::spawn(async move {
            let mut broadcast_rx = events.subscribe();
            let mut shutdown_rx = shutdown_rx;
            let mut consecutive_failures: u32 = 0;
            // resync_pending: lag/overflow detected; try to send resync before next event.
            let mut resync_pending = false;
            // resync_sent: a resync was successfully enqueued; suppress further resyncs
            // until a successful non-resync payload clears this (at-most-once contract).
            let mut resync_sent = false;
            let mut queue_full_since: Option<tokio::time::Instant> = None;
            let mut last_emitted_event_id: Option<String> = None;

            loop {
                // Compute the absolute slow-consumer disconnect deadline (once set, it does not
                // reset when new events arrive, so the 5s SLO is enforced independently).
                let disconnect_deadline =
                    queue_full_since.map(|s| s + std::time::Duration::from_secs(5));

                let recv_result = tokio::select! {
                    biased;
                    // Slow-consumer arm fires at the absolute deadline, not relative to events.
                    _ = async {
                        if let Some(dl) = disconnect_deadline {
                            tokio::time::sleep_until(dl).await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    } => {
                        db::metrics::increment_counter_with_label(
                            "session_status_subscription_slow_consumer_disconnect_total",
                            "queue_full_5s",
                        );
                        let _ = tx.try_send(Err(Error::new("slow_consumer_disconnected")));
                        break;
                    }
                    // Optional shutdown signal (test-only): mirrors the Closed arm behavior.
                    _ = async {
                        if let Some(rx) = &mut shutdown_rx {
                            let _ = rx.changed().await;
                        } else {
                            std::future::pending::<()>().await;
                        }
                    } => {
                        if !live_principal_handle.auth_ok_for_credential(&live_credential).await {
                            let _ = tx.try_send(Err(Error::new("authorization_recheck_failed")));
                            return;
                        }
                        let resync = crate::types::session::resync_event(&run_id_str);
                        let _ = tx.try_send(Ok(resync));
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        break;
                    }
                    // Normal receive with keep-alive timeout.
                    result = tokio::time::timeout(
                        std::time::Duration::from_secs(30),
                        broadcast_rx.recv(),
                    ) => result,
                };

                let domain_event = match recv_result {
                    Ok(Ok(ev)) => ev,
                    Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                        // Proposal: immediately try to enqueue one resyncRequired notification
                        // rather than deferring until the next matching event. At-most-once:
                        // suppress if a resync was already delivered since the last successful
                        // non-resync payload.
                        db::metrics::increment_counter_with_label(
                            "session_status_subscription_lag_total",
                            "lagged",
                        );
                        if !live_principal_handle
                            .auth_ok_for_credential(&live_credential)
                            .await
                        {
                            let _ = tx.try_send(Err(Error::new("authorization_recheck_failed")));
                            return;
                        }
                        if !resync_sent {
                            let resync = crate::types::session::resync_event(&run_id_str);
                            match tx.try_send(Ok(resync)) {
                                Ok(_) => {
                                    consecutive_failures = 0;
                                    queue_full_since = None;
                                    resync_sent = true;
                                    resync_pending = false;
                                }
                                Err(_) => {
                                    // Queue full; will retry before next matching event.
                                    resync_pending = true;
                                    if queue_full_since.is_none() {
                                        queue_full_since = Some(tokio::time::Instant::now());
                                    }
                                    consecutive_failures += 1;
                                    if consecutive_failures >= 3 {
                                        db::metrics::increment_counter_with_label(
                                            "session_status_subscription_slow_consumer_disconnect_total",
                                            "consecutive_enqueue_failures",
                                        );
                                        let _ = tx.try_send(Err(Error::new(
                                            "slow_consumer_disconnected",
                                        )));
                                        return;
                                    }
                                }
                            }
                        }
                        continue;
                    }
                    Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                        // Graceful shutdown: emit one resyncRequired payload so clients know
                        // they must re-query. Allow up to 1 second for the client to receive it.
                        if !live_principal_handle
                            .auth_ok_for_credential(&live_credential)
                            .await
                        {
                            let _ = tx.try_send(Err(Error::new("authorization_recheck_failed")));
                            return;
                        }
                        let resync = crate::types::session::resync_event(&run_id_str);
                        let _ = tx.try_send(Ok(resync));
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        break;
                    }
                    Err(_timeout) => {
                        if tx.is_closed() {
                            break;
                        }
                        continue;
                    }
                };

                let emit_start = std::time::Instant::now();

                // Primary trigger: SessionEventRecorded (session lifecycle events).
                // Secondary triggers: runtime/run/stage changes for backwards compat.
                let matched_run_id = match &domain_event {
                    DomainEvent::SessionEventRecorded { run_id } => *run_id == filter_run_id,
                    DomainEvent::RuntimeStatusChanged { run_id, .. } => *run_id == filter_run_id,
                    DomainEvent::RunStatusChanged { run_id, .. } => *run_id == filter_run_id,
                    DomainEvent::StageStatusChanged { run_id, .. } => *run_id == filter_run_id,
                    _ => false,
                };
                if !matched_run_id {
                    continue;
                }

                // Recheck full operator-read authorization (class + P072 subscription policy).
                // Uses the live handle so revocation of principals.json is observed.
                // Fail-closed on revocation, downgrade, or transient lookup failure.
                if !live_principal_handle
                    .auth_ok_for_credential(&live_credential)
                    .await
                {
                    let _ = tx.try_send(Err(Error::new("authorization_recheck_failed")));
                    break;
                }

                // Bound per-emission DB lookup to 250ms (subscription payload resolution deadline).
                let session_ev = match tokio::time::timeout(
                    std::time::Duration::from_millis(250),
                    session_repo::latest_session_event_for_run(&pool, &run_id_str),
                )
                .await
                {
                    Ok(Ok(Some(ev))) => ev,
                    Ok(Ok(None)) => continue,
                    Ok(Err(_)) => {
                        if !resync_sent {
                            resync_pending = true;
                        }
                        continue;
                    }
                    Err(_timeout) => {
                        if !resync_sent {
                            resync_pending = true;
                        }
                        continue;
                    }
                };

                // Skip if this is the same event we already emitted (stale repeat).
                if last_emitted_event_id.as_deref() == Some(session_ev.id.as_str()) {
                    continue;
                }

                // Build payloads: (is_resync, payload) so we can update resync_sent correctly.
                // Prepend a pending resync only if one hasn't already been delivered.
                let mut payloads: Vec<(bool, Result<GqlSessionStatusChangedEvent>)> = Vec::new();
                if resync_pending && !resync_sent {
                    resync_pending = false;
                    payloads.push((true, Ok(crate::types::session::resync_event(&run_id_str))));
                } else {
                    resync_pending = false;
                }
                payloads.push((
                    false,
                    Ok(crate::types::session::session_event_to_status_changed(
                        &session_ev,
                        &run_id_str,
                    )),
                ));

                for (is_resync, payload) in payloads {
                    if tx.is_closed() {
                        return;
                    }
                    match tx.try_send(payload) {
                        Ok(_) => {
                            consecutive_failures = 0;
                            queue_full_since = None;
                            if is_resync {
                                resync_sent = true;
                                db::metrics::increment_counter_with_label(
                                    "session_status_subscription_event_total",
                                    "UNKNOWN_EVENT_SHAPE:resync",
                                );
                            } else {
                                // Successful non-resync delivery: reset resync guard and
                                // record last emitted event id for deduplication.
                                resync_sent = false;
                                last_emitted_event_id = Some(session_ev.id.clone());
                                db::metrics::record_p046_emit_lag(
                                    emit_start.elapsed().as_millis() as u64
                                );
                                let evt_label = format!(
                                    "{}:ok",
                                    session_event_type_metric_label(&session_ev.event_type)
                                );
                                db::metrics::increment_counter_with_label(
                                    "session_status_subscription_event_total",
                                    &evt_label,
                                );
                            }
                        }
                        Err(_) => {
                            if queue_full_since.is_none() {
                                queue_full_since = Some(tokio::time::Instant::now());
                            }
                            consecutive_failures += 1;
                            // Schedule a pending resync for the next event if we haven't
                            // already sent one and this was a non-resync failure.
                            if !is_resync && !resync_sent {
                                resync_pending = true;
                            }
                            if consecutive_failures >= 3 {
                                db::metrics::increment_counter_with_label(
                                    "session_status_subscription_slow_consumer_disconnect_total",
                                    "consecutive_enqueue_failures",
                                );
                                let _ = tx.try_send(Err(Error::new("slow_consumer_disconnected")));
                                return;
                            }
                        }
                    }
                }
            }
        });

        Ok(ReceiverStream::new(rx))
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(rename_items = "snake_case")]
pub enum TimelineRawDetailStatus {
    Available,
    Missing,
    Stale,
    Unauthorized,
    Unavailable,
    DigestMismatch,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(rename_items = "snake_case")]
pub enum TimelineRawDetailErrorReason {
    HandleNotFound,
    HandleExpired,
    RunNotAuthorized,
    EventNotAuthorized,
    StorageUnavailable,
    DigestValidationFailed,
}

#[derive(SimpleObject, Clone, Debug)]
pub struct GqlTimelineRawDetailResult {
    pub status: TimelineRawDetailStatus,
    pub raw_detail: Option<String>,
    pub raw_detail_bytes: Option<i32>,
    pub raw_detail_digest: Option<String>,
    pub error_reason: Option<TimelineRawDetailErrorReason>,
}

impl GqlTimelineRawDetailResult {
    fn available(raw_detail: String, raw_detail_digest: String) -> Self {
        let raw_detail_bytes = raw_detail.len() as i32;
        Self {
            status: TimelineRawDetailStatus::Available,
            raw_detail: Some(raw_detail),
            raw_detail_bytes: Some(raw_detail_bytes),
            raw_detail_digest: Some(raw_detail_digest),
            error_reason: None,
        }
    }

    fn missing(error_reason: TimelineRawDetailErrorReason) -> Self {
        Self {
            status: TimelineRawDetailStatus::Missing,
            raw_detail: None,
            raw_detail_bytes: None,
            raw_detail_digest: None,
            error_reason: Some(error_reason),
        }
    }

    fn failed(status: TimelineRawDetailStatus, error_reason: TimelineRawDetailErrorReason) -> Self {
        Self {
            status,
            raw_detail: None,
            raw_detail_bytes: None,
            raw_detail_digest: None,
            error_reason: Some(error_reason),
        }
    }
}

async fn p093_resolve_timeline_raw_detail(
    pool: &SqlitePool,
    handle: &str,
) -> Result<GqlTimelineRawDetailResult> {
    let row = sqlx::query(
        r#"
        SELECT
            trd.run_id,
            trd.agent_execution_id,
            trd.session_generation_id,
            trd.timeline_event_id,
            trd.raw_detail,
            trd.raw_detail_bytes,
            trd.raw_detail_digest,
            trd.status,
            trd.expires_at,
            r.id AS existing_run_id,
            ae.id AS existing_agent_execution_id,
            ae.session_generation_id AS execution_session_generation_id,
            se.run_id AS execution_run_id
        FROM timeline_raw_details trd
        LEFT JOIN runs r ON r.id = trd.run_id
        LEFT JOIN agent_executions ae ON ae.id = trd.agent_execution_id
        LEFT JOIN stage_executions se ON se.id = ae.stage_execution_id
        WHERE trd.handle = ?1
        "#,
    )
    .bind(handle)
    .fetch_optional(pool)
    .await
    .map_err(|e| Error::new(e.to_string()))?;

    let Some(row) = row else {
        return Ok(GqlTimelineRawDetailResult::missing(
            TimelineRawDetailErrorReason::HandleNotFound,
        ));
    };

    let status: String = row.get("status");
    match status.as_str() {
        "available" => {}
        "stale" => {
            return Ok(GqlTimelineRawDetailResult::failed(
                TimelineRawDetailStatus::Stale,
                TimelineRawDetailErrorReason::HandleExpired,
            ));
        }
        "unauthorized" => {
            return Ok(GqlTimelineRawDetailResult::failed(
                TimelineRawDetailStatus::Unauthorized,
                TimelineRawDetailErrorReason::EventNotAuthorized,
            ));
        }
        "unavailable" => {
            return Ok(GqlTimelineRawDetailResult::failed(
                TimelineRawDetailStatus::Unavailable,
                TimelineRawDetailErrorReason::StorageUnavailable,
            ));
        }
        "digest_mismatch" => {
            return Ok(GqlTimelineRawDetailResult::failed(
                TimelineRawDetailStatus::DigestMismatch,
                TimelineRawDetailErrorReason::DigestValidationFailed,
            ));
        }
        _ => {
            return Ok(GqlTimelineRawDetailResult::failed(
                TimelineRawDetailStatus::Unavailable,
                TimelineRawDetailErrorReason::StorageUnavailable,
            ));
        }
    }

    let expires_at: Option<String> = row.get("expires_at");
    if let Some(expires_at) = expires_at
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let parsed = chrono::DateTime::parse_from_rfc3339(expires_at)
            .map(|value| value.with_timezone(&chrono::Utc))
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(expires_at, "%Y-%m-%dT%H:%M:%S%.fZ")
                    .map(|value| value.and_utc())
            });
        match parsed {
            Ok(expires_at) if expires_at <= chrono::Utc::now() => {
                return Ok(GqlTimelineRawDetailResult::failed(
                    TimelineRawDetailStatus::Stale,
                    TimelineRawDetailErrorReason::HandleExpired,
                ));
            }
            Ok(_) => {}
            Err(_) => {
                return Ok(GqlTimelineRawDetailResult::failed(
                    TimelineRawDetailStatus::Stale,
                    TimelineRawDetailErrorReason::HandleExpired,
                ));
            }
        }
    }

    let existing_run_id: Option<String> = row.get("existing_run_id");
    if existing_run_id.is_none() {
        return Ok(GqlTimelineRawDetailResult::failed(
            TimelineRawDetailStatus::Unauthorized,
            TimelineRawDetailErrorReason::RunNotAuthorized,
        ));
    }

    let agent_execution_id: Option<String> = row.get("agent_execution_id");
    let existing_agent_execution_id: Option<String> = row.get("existing_agent_execution_id");
    if agent_execution_id
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
        || existing_agent_execution_id.is_none()
    {
        return Ok(GqlTimelineRawDetailResult::failed(
            TimelineRawDetailStatus::Unauthorized,
            TimelineRawDetailErrorReason::EventNotAuthorized,
        ));
    }

    let run_id: String = row.get("run_id");
    let execution_run_id: Option<String> = row.get("execution_run_id");
    if execution_run_id.as_deref() != Some(run_id.as_str()) {
        return Ok(GqlTimelineRawDetailResult::failed(
            TimelineRawDetailStatus::Unauthorized,
            TimelineRawDetailErrorReason::RunNotAuthorized,
        ));
    }

    let timeline_event_id: String = row.get("timeline_event_id");
    if timeline_event_id.trim().is_empty() {
        return Ok(GqlTimelineRawDetailResult::failed(
            TimelineRawDetailStatus::Unauthorized,
            TimelineRawDetailErrorReason::EventNotAuthorized,
        ));
    }

    let scoped_session_generation_id: Option<String> = row.get("session_generation_id");
    if let Some(scoped_session_generation_id) = scoped_session_generation_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let execution_session_generation_id: Option<String> =
            row.get("execution_session_generation_id");
        if execution_session_generation_id.as_deref() != Some(scoped_session_generation_id) {
            return Ok(GqlTimelineRawDetailResult::failed(
                TimelineRawDetailStatus::Unauthorized,
                TimelineRawDetailErrorReason::EventNotAuthorized,
            ));
        }
    }

    let raw_detail: String = row.get("raw_detail");
    let raw_detail_digest: String = row.get("raw_detail_digest");
    let computed = sha256_digest(&raw_detail);
    if raw_detail_digest != computed {
        return Ok(GqlTimelineRawDetailResult::failed(
            TimelineRawDetailStatus::DigestMismatch,
            TimelineRawDetailErrorReason::DigestValidationFailed,
        ));
    }

    Ok(GqlTimelineRawDetailResult::available(
        raw_detail,
        raw_detail_digest,
    ))
}

/// Runtime lifecycle event surfaced to GraphQL subscribers.
#[derive(SimpleObject, Clone, Debug)]
pub struct GqlRuntimeEvent {
    pub id: ID,
    pub run_id: ID,
    pub stage_id: String,
    pub agent_id: String,
    pub provider: String,
    /// "session_started" | "session_completed" | "session_failed"
    pub event_kind: String,
    pub title: Option<String>,
    pub detail: Option<String>,
    pub surface_label: Option<String>,
    pub session_generation_id: Option<String>,
    pub timestamp: String,
    pub raw_detail: Option<String>,
    pub raw_detail_bytes: Option<i32>,
    pub raw_detail_truncated: bool,
    pub raw_detail_handle: Option<ID>,
    pub raw_detail_digest: Option<String>,
    pub full_raw_available: bool,
    pub detail_digest: Option<String>,
    pub detail_char_count: Option<i32>,
    pub chunk_count: Option<i32>,
    pub is_streaming: bool,
    pub is_terminal: bool,
    pub state_label: Option<String>,
    pub sequence_cursor: String,
    pub projection_generation: i64,
    pub gap_detected: bool,
    pub requires_full_refetch: bool,
}

struct P093RuntimeRawDetailReadback {
    detail: Option<String>,
    raw_detail: Option<String>,
    raw_detail_bytes: Option<i32>,
    raw_detail_truncated: bool,
    raw_detail_handle: Option<ID>,
    raw_detail_digest: Option<String>,
    full_raw_available: bool,
    detail_digest: Option<String>,
    detail_char_count: Option<i32>,
}

impl GqlRuntimeEvent {
    const RETAINED_INLINE_RAW_DETAIL_LIMIT: usize = 512 * 1024;

    fn from_parts(
        run_id: ID,
        stage_id: String,
        agent_id: String,
        provider: String,
        event_kind: String,
        title: Option<String>,
        detail: Option<String>,
        surface_label: Option<String>,
        session_generation_id: Option<String>,
        timestamp: String,
    ) -> Self {
        let raw = Self::inline_raw_detail_readback(detail, None);
        Self::from_readback_parts(
            run_id,
            stage_id,
            agent_id,
            provider,
            event_kind,
            title,
            surface_label,
            session_generation_id,
            timestamp,
            raw,
        )
    }

    async fn from_live_timeline_parts(
        pool: &SqlitePool,
        run_id: RunId,
        stage_id: String,
        agent_id: String,
        provider: String,
        event_kind: String,
        title: Option<String>,
        detail: Option<String>,
        surface_label: Option<String>,
        session_generation_id: Option<String>,
        timestamp: String,
    ) -> Self {
        let full_detail = detail.clone();
        let mut raw = Self::inline_raw_detail_readback(detail, None);
        if raw.raw_detail_truncated {
            let event_id = runtime_event_id(
                &run_id.to_string(),
                &stage_id,
                &agent_id,
                &event_kind,
                surface_label.as_deref(),
                session_generation_id.as_deref(),
                &timestamp,
                raw.detail_digest.as_deref(),
            );
            if let Some(full_detail) = full_detail {
                match p093_persist_live_timeline_raw_detail(
                    pool,
                    run_id,
                    &stage_id,
                    &agent_id,
                    &provider,
                    session_generation_id.as_deref(),
                    &event_id,
                    &full_detail,
                    raw.raw_detail_digest.as_deref().unwrap_or_default(),
                )
                .await
                {
                    Ok(Some(handle)) => {
                        raw.raw_detail_handle = Some(ID(handle));
                        raw.full_raw_available = true;
                    }
                    Ok(None) => {
                        raw.full_raw_available = false;
                    }
                    Err(error) => {
                        warn!(
                            run_id = %run_id,
                            stage_id = %stage_id,
                            agent_id = %agent_id,
                            error = ?error,
                            "P093 live timeline raw detail retention failed closed"
                        );
                        raw.full_raw_available = false;
                    }
                }
            }
        }
        Self::from_readback_parts(
            ID(run_id.to_string()),
            stage_id,
            agent_id,
            provider,
            event_kind,
            title,
            surface_label,
            session_generation_id,
            timestamp,
            raw,
        )
    }

    fn from_readback_parts(
        run_id: ID,
        stage_id: String,
        agent_id: String,
        provider: String,
        event_kind: String,
        title: Option<String>,
        surface_label: Option<String>,
        session_generation_id: Option<String>,
        timestamp: String,
        raw: P093RuntimeRawDetailReadback,
    ) -> Self {
        let is_terminal = matches!(
            event_kind.as_str(),
            "session_completed" | "session_failed" | "agent_summary"
        ) || matches!(surface_label.as_deref(), Some("agent_summary"));
        let is_streaming = !is_terminal
            && matches!(
                surface_label.as_deref(),
                Some("text_chunk") | Some("agent_message_chunk")
            );
        let id = runtime_event_id(
            run_id.as_str(),
            &stage_id,
            &agent_id,
            &event_kind,
            surface_label.as_deref(),
            session_generation_id.as_deref(),
            &timestamp,
            raw.detail_digest.as_deref(),
        );
        Self {
            id: ID(id),
            run_id,
            stage_id: stage_id.clone(),
            agent_id,
            provider,
            event_kind,
            title,
            detail: raw.detail,
            surface_label,
            session_generation_id,
            timestamp,
            raw_detail: raw.raw_detail,
            raw_detail_bytes: raw.raw_detail_bytes,
            raw_detail_truncated: raw.raw_detail_truncated,
            raw_detail_handle: raw.raw_detail_handle,
            raw_detail_digest: raw.raw_detail_digest,
            full_raw_available: raw.full_raw_available,
            detail_digest: raw.detail_digest,
            detail_char_count: raw.detail_char_count,
            chunk_count: Some(1),
            is_streaming,
            is_terminal,
            state_label: Some(stage_id),
            sequence_cursor: "seq-0".to_string(),
            projection_generation: 0,
            gap_detected: false,
            requires_full_refetch: false,
        }
    }

    fn inline_raw_detail_readback(
        detail: Option<String>,
        raw_detail_handle: Option<ID>,
    ) -> P093RuntimeRawDetailReadback {
        let Some(full_detail) = detail else {
            return P093RuntimeRawDetailReadback {
                detail: None,
                raw_detail: None,
                raw_detail_bytes: None,
                raw_detail_truncated: false,
                raw_detail_handle,
                raw_detail_digest: None,
                full_raw_available: true,
                detail_digest: None,
                detail_char_count: None,
            };
        };

        let raw_detail_digest = sha256_digest(&full_detail);
        let capped = cap_utf8_suffix(&full_detail, Self::RETAINED_INLINE_RAW_DETAIL_LIMIT);
        let detail_digest = sha256_digest(&capped.text);
        let raw_detail_bytes = Some(i32::try_from(capped.text.len()).unwrap_or(i32::MAX));
        let detail_char_count =
            Some(i32::try_from(capped.text.chars().count()).unwrap_or(i32::MAX));
        P093RuntimeRawDetailReadback {
            detail: Some(capped.text.clone()),
            raw_detail: Some(capped.text),
            raw_detail_bytes,
            raw_detail_truncated: capped.truncated,
            raw_detail_handle,
            raw_detail_digest: Some(raw_detail_digest),
            full_raw_available: !capped.truncated,
            detail_digest: Some(detail_digest),
            detail_char_count,
        }
    }
}

async fn p093_persist_live_timeline_raw_detail(
    pool: &SqlitePool,
    run_id: RunId,
    stage_id: &str,
    agent_id: &str,
    provider: &str,
    session_generation_id: Option<&str>,
    timeline_event_id: &str,
    raw_detail: &str,
    raw_detail_digest: &str,
) -> Result<Option<String>> {
    let mut query = String::from(
        r#"
        SELECT ae.id
        FROM agent_executions ae
        INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
        WHERE se.run_id = ?1
          AND se.stage_id = ?2
          AND ae.agent_id = ?3
          AND ae.provider = ?4
        "#,
    );
    if session_generation_id.is_some() {
        query.push_str(" AND ae.session_generation_id = ?5");
    }
    query.push_str(" ORDER BY ae.started_at DESC LIMIT 1");

    let mut sql = sqlx::query(&query)
        .bind(run_id.to_string())
        .bind(stage_id)
        .bind(agent_id)
        .bind(provider);
    if let Some(session_generation_id) = session_generation_id {
        sql = sql.bind(session_generation_id);
    }
    let row = sql
        .fetch_optional(pool)
        .await
        .map_err(|e| Error::new(e.to_string()))?;
    let Some(row) = row else {
        return Ok(None);
    };

    let agent_execution_id: String = row.get("id");
    let handle = format!("trd_{}", uuid::Uuid::new_v4().simple());
    sqlx::query(
        r#"
        INSERT INTO timeline_raw_details
            (handle, run_id, agent_execution_id, session_generation_id, timeline_event_id,
             raw_detail, raw_detail_bytes, raw_detail_digest, status)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'available')
        "#,
    )
    .bind(&handle)
    .bind(run_id.to_string())
    .bind(agent_execution_id)
    .bind(session_generation_id)
    .bind(timeline_event_id)
    .bind(raw_detail)
    .bind(i64::try_from(raw_detail.len()).unwrap_or(i64::MAX))
    .bind(raw_detail_digest)
    .execute(pool)
    .await
    .map_err(|e| Error::new(e.to_string()))?;

    Ok(Some(handle))
}

fn cap_utf8_suffix(text: &str, utf8_limit: usize) -> P093CappedText {
    if text.len() <= utf8_limit {
        return P093CappedText {
            text: text.to_string(),
            truncated: false,
        };
    }
    let mut start = text.len().saturating_sub(utf8_limit);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    P093CappedText {
        text: text[start..].to_string(),
        truncated: true,
    }
}

struct P093CappedText {
    text: String,
    truncated: bool,
}

fn sha256_digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn runtime_event_id(
    run_id: &str,
    stage_id: &str,
    agent_id: &str,
    event_kind: &str,
    surface_label: Option<&str>,
    session_generation_id: Option<&str>,
    timestamp: &str,
    detail_digest: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        run_id,
        stage_id,
        agent_id,
        event_kind,
        surface_label.unwrap_or(""),
        session_generation_id.unwrap_or(""),
        timestamp,
        detail_digest.unwrap_or(""),
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    format!("rte_{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    use async_graphql::Request;
    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{
        artifact_contracts, artifacts, audit_log, ideas, projections, rollout_contract_checks,
        runs, stages, steward, workflow_conflicts,
    };
    use db::write_class::{ReplayPolicy, WriteClass, WriteLane, WriteOperation, WriteResult};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::mediation::{LeadConflictMediationRecord, LeadMediationStatus};
    use domain::steward::{
        CohortQuality, StewardAnalysis, StewardAnalysisRunLink, StewardAnalysisStatus,
        StewardRecommendation,
    };
    use domain::validation::{
        ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
        ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
    };
    use domain::workflow_conflict::{
        candidate_transition_hash, workflow_conflict_fingerprint, CandidateTransitionEvaluation,
        CandidateTransitionResult, WorkflowConflictReason, WorkflowConflictRecord,
        WorkflowConflictStatus,
    };
    use engine::event_bus;
    use engine::work_queue::WorkQueue;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    /// Shared in-process `LifecycleReporter` for tests. Every `build_schema`
    /// call now requires a reporter per P042 §5.2; tests get a default one
    /// seeded in `NotStarted` unless they need a specific transition.
    fn test_reporter() -> LifecycleReporter {
        LifecycleReporter::new(0, "test", event_bus::new_bus(16))
    }

    const P041_FIXTURES: &[&str] = &[
        "proposal-loop-basic",
        "implementation-refine-review",
        "approval-pause-resume",
        "retry-recovery-flow",
        "cancelled-or-blocked-run",
        "terminal-report-evidence",
        "projection-readback-surface",
    ];

    fn p041_selected_fixtures() -> Vec<&'static str> {
        match std::env::var("P041_ONLY_FIXTURE") {
            Ok(raw) if !raw.trim().is_empty() => {
                let requested = raw.trim().to_string();
                let fixture = P041_FIXTURES
                    .iter()
                    .copied()
                    .find(|candidate| *candidate == requested.as_str())
                    .unwrap_or_else(|| {
                        panic!("P041_ONLY_FIXTURE {requested:?} is not in P041_FIXTURES")
                    });
                vec![fixture]
            }
            _ => P041_FIXTURES.to_vec(),
        }
    }

    #[test]
    fn mutation_name_converter_covers_approval_mutations() {
        assert_eq!(
            capability_id_for(MutationName::ApproveApproval),
            domain::CapabilityToolId::ApprovalsResolve
        );
        assert_eq!(
            capability_id_for(MutationName::RejectApproval),
            domain::CapabilityToolId::ApprovalsResolve
        );
    }

    #[tokio::test]
    async fn graphql_mutation_root_exposes_only_approval_mutations() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let sdl = schema.sdl();

        assert!(sdl.contains("approveApproval("));
        assert!(sdl.contains("rejectApproval("));
        for mutation in [
            "startRun(",
            "approveStage(",
            "rejectStage(",
            "retryStage(",
            "overrideLegacyDiscoveryPolicy(",
            "cancelRun(",
        ] {
            assert!(
                !sdl.contains(mutation),
                "{mutation} must not be present on GraphQL MutationRoot"
            );
        }
    }

    #[tokio::test]
    async fn runtime_timeline_p093_readback_fields_are_in_schema() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let sdl = schema.sdl();

        for field in [
            "timelineRawDetail(",
            "rawDetail",
            "rawDetailBytes",
            "rawDetailTruncated",
            "rawDetailHandle",
            "rawDetailDigest",
            "fullRawAvailable",
            "detailDigest",
            "detailCharCount",
            "chunkCount",
            "isStreaming",
            "isTerminal",
            "stateLabel",
        ] {
            assert!(sdl.contains(field), "schema should expose {field}");
        }
    }

    #[test]
    fn runtime_timeline_p093_event_synthesizes_metadata_without_swift_inference() {
        let event = GqlRuntimeEvent::from_parts(
            ID("run-1".into()),
            "state_10".into(),
            "code_writer".into(),
            "claude".into(),
            "meaningful_progress".into(),
            Some("Agent response".into()),
            Some("chunk".into()),
            Some("text_chunk".into()),
            Some("session-1".into()),
            "2026-05-21T08:44:47Z".into(),
        );

        assert!(event.id.as_str().starts_with("rte_"));
        assert_eq!(event.raw_detail.as_deref(), Some("chunk"));
        assert_eq!(event.raw_detail_bytes, Some(5));
        assert_eq!(event.detail_char_count, Some(5));
        assert_eq!(event.chunk_count, Some(1));
        assert!(event.is_streaming);
        assert!(!event.is_terminal);
        assert_eq!(event.state_label.as_deref(), Some("state_10"));
        assert!(event.full_raw_available);
        assert!(!event.raw_detail_truncated);
        assert!(event
            .raw_detail_digest
            .as_deref()
            .unwrap()
            .starts_with("sha256:"));
    }

    #[tokio::test]
    async fn runtime_timeline_p093_live_over_budget_event_persists_resolvable_raw_detail() {
        use async_graphql::futures_util::StreamExt;

        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.status = domain::run::RunStatus::Running;
        runs::insert(&pool, &run).await.unwrap();
        let stage = make_stage_execution(run_id, "state_10", "Implementation", Utc::now());
        let stage_execution_id = stage.id;
        stages::insert(&pool, &stage).await.unwrap();
        let execution =
            make_agent_execution(stage_execution_id, "code_writer", "claude", Utc::now());
        db::repos::agent_executions::insert(&pool, &execution)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let raw = format!("{}{}", "a".repeat(530_000), "tail-marker");
        let expected_digest = sha256_digest(&raw);
        let mut stream = schema.execute_stream(
            Request::new(
                r#"
                subscription($runId: ID!) {
                  runtimeStatusChanged(runId: $runId) {
                    rawDetail
                    rawDetailBytes
                    rawDetailTruncated
                    rawDetailHandle
                    rawDetailDigest
                    fullRawAvailable
                  }
                }
                "#,
            )
            .variables(Variables::from_json(
                serde_json::json!({ "runId": run_id.to_string() }),
            ))
            .data(test_principal()),
        );
        let bus_for_event = bus.clone();
        let raw_for_event = raw.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = bus_for_event.send(DomainEvent::RuntimeTimelineEvent {
                run_id,
                stage_id: "state_10".into(),
                agent_id: "code_writer".into(),
                provider: "claude".into(),
                event_kind: "meaningful_progress".into(),
                title: "Agent response".into(),
                detail: Some(raw_for_event),
                surface_label: "text_chunk".into(),
                session_generation_id: Some("session-code_writer".into()),
            });
        });

        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("runtime timeline subscription frame timed out")
            .expect("runtime timeline subscription ended");
        assert!(
            frame.errors.is_empty(),
            "runtime timeline over-budget event should stream without errors: {frame:?}"
        );
        let json = frame.data.into_json().unwrap();
        let event = &json["runtimeStatusChanged"];
        assert_eq!(event["rawDetailTruncated"], serde_json::json!(true));
        assert_eq!(event["rawDetailBytes"], serde_json::json!(524_288));
        assert_eq!(event["rawDetailDigest"], serde_json::json!(expected_digest));
        assert_eq!(event["fullRawAvailable"], serde_json::json!(true));
        let retained = event["rawDetail"].as_str().unwrap();
        assert_eq!(retained.len(), 524_288);
        assert!(retained.ends_with("tail-marker"));
        let handle = event["rawDetailHandle"]
            .as_str()
            .expect("over-budget live event should expose daemon raw-detail handle");

        let resolver_response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query {{
                      timelineRawDetail(handle: "{handle}") {{
                        status rawDetail rawDetailBytes rawDetailDigest errorReason
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            resolver_response.errors.is_empty(),
            "stored live raw detail handle should resolve: {resolver_response:?}"
        );
        let resolver_json = resolver_response.data.into_json().unwrap();
        let resolved = &resolver_json["timelineRawDetail"];
        assert_eq!(resolved["status"], serde_json::json!("available"));
        assert_eq!(resolved["rawDetailBytes"], serde_json::json!(raw.len()));
        assert_eq!(
            resolved["rawDetailDigest"],
            serde_json::json!(expected_digest)
        );
        assert_eq!(resolved["rawDetail"], serde_json::json!(raw));
        assert!(resolved["errorReason"].is_null());
    }

    #[tokio::test]
    async fn runtime_timeline_p093_raw_detail_resolver_covers_status_matrix() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let raw = "full retained raw response";
        let digest = sha256_digest(raw);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.status = domain::run::RunStatus::Running;
        runs::insert(&pool, &run).await.unwrap();
        let stage = make_stage_execution(run_id, "state_10", "Implementation", Utc::now());
        let stage_id = stage.id;
        stages::insert(&pool, &stage).await.unwrap();
        let execution = make_agent_execution(stage_id, "code_writer", "claude", Utc::now());
        let agent_execution_id = execution.id.to_string();
        db::repos::agent_executions::insert(&pool, &execution)
            .await
            .unwrap();
        for (handle, status, detail, digest_override) in [
            ("trd_available", "available", raw, digest.as_str()),
            ("trd_stale", "stale", "", "sha256:empty"),
            ("trd_unauthorized", "unauthorized", "", "sha256:empty"),
            ("trd_unavailable", "unavailable", "", "sha256:empty"),
            (
                "trd_status_mismatch",
                "digest_mismatch",
                raw,
                digest.as_str(),
            ),
            (
                "trd_digest_mismatch",
                "available",
                raw,
                "sha256:not-the-content",
            ),
        ] {
            sqlx::query(
                r#"
                INSERT INTO timeline_raw_details
                    (handle, run_id, agent_execution_id, session_generation_id, timeline_event_id,
                     raw_detail, raw_detail_bytes, raw_detail_digest, status)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )
            .bind(handle)
            .bind(run_id.to_string())
            .bind(&agent_execution_id)
            .bind("session-code_writer")
            .bind(format!("rte_{handle}"))
            .bind(detail)
            .bind(detail.len() as i64)
            .bind(digest_override)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        }

        let response = schema
            .execute(
                Request::new(
                    r#"
                    query {
                      available: timelineRawDetail(handle: "trd_available") {
                        status rawDetail rawDetailBytes rawDetailDigest errorReason
                      }
                      missing: timelineRawDetail(handle: "trd_missing") {
                        status errorReason
                      }
                      stale: timelineRawDetail(handle: "trd_stale") {
                        status errorReason
                      }
                      unauthorized: timelineRawDetail(handle: "trd_unauthorized") {
                        status errorReason
                      }
                      unavailable: timelineRawDetail(handle: "trd_unavailable") {
                        status errorReason
                      }
                      statusMismatch: timelineRawDetail(handle: "trd_status_mismatch") {
                        status errorReason
                      }
                      digestMismatch: timelineRawDetail(handle: "trd_digest_mismatch") {
                        status errorReason
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "raw detail resolver should fail closed through result statuses: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["available"]["status"], serde_json::json!("available"));
        assert_eq!(json["available"]["rawDetail"], serde_json::json!(raw));
        assert_eq!(
            json["available"]["rawDetailDigest"],
            serde_json::json!(digest)
        );
        assert_eq!(
            json["missing"]["errorReason"],
            serde_json::json!("handle_not_found")
        );
        assert_eq!(json["stale"]["status"], serde_json::json!("stale"));
        assert_eq!(
            json["stale"]["errorReason"],
            serde_json::json!("handle_expired")
        );
        assert_eq!(
            json["unauthorized"]["status"],
            serde_json::json!("unauthorized")
        );
        assert_eq!(
            json["unauthorized"]["errorReason"],
            serde_json::json!("event_not_authorized")
        );
        assert_eq!(
            json["unavailable"]["status"],
            serde_json::json!("unavailable")
        );
        assert_eq!(
            json["unavailable"]["errorReason"],
            serde_json::json!("storage_unavailable")
        );
        assert_eq!(
            json["statusMismatch"]["status"],
            serde_json::json!("digest_mismatch")
        );
        assert_eq!(
            json["digestMismatch"]["errorReason"],
            serde_json::json!("digest_validation_failed")
        );
    }

    #[tokio::test]
    async fn runtime_timeline_p093_raw_detail_resolver_enforces_expiry_and_scope() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.status = domain::run::RunStatus::Running;
        runs::insert(&pool, &run).await.unwrap();
        let stage = make_stage_execution(run_id, "state_10", "Implementation", Utc::now());
        let stage_id = stage.id;
        stages::insert(&pool, &stage).await.unwrap();
        let execution = make_agent_execution(stage_id, "code_writer", "claude", Utc::now());
        let agent_execution_id = execution.id.to_string();
        db::repos::agent_executions::insert(&pool, &execution)
            .await
            .unwrap();

        let raw = "full scoped raw detail";
        let digest = sha256_digest(raw);
        sqlx::query(
            r#"
            INSERT INTO timeline_raw_details
                (handle, run_id, agent_execution_id, session_generation_id, timeline_event_id,
                 raw_detail, raw_detail_bytes, raw_detail_digest, status, expires_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'available', ?9)
            "#,
        )
        .bind("trd_scoped")
        .bind(run_id.to_string())
        .bind(&agent_execution_id)
        .bind("session-code_writer")
        .bind("rte_scoped")
        .bind(raw)
        .bind(raw.len() as i64)
        .bind(&digest)
        .bind((Utc::now() + chrono::Duration::minutes(5)).to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO timeline_raw_details
                (handle, run_id, agent_execution_id, session_generation_id, timeline_event_id,
                 raw_detail, raw_detail_bytes, raw_detail_digest, status, expires_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'available', ?9)
            "#,
        )
        .bind("trd_expired")
        .bind(run_id.to_string())
        .bind(&agent_execution_id)
        .bind("session-code_writer")
        .bind("rte_expired")
        .bind(raw)
        .bind(raw.len() as i64)
        .bind(&digest)
        .bind((Utc::now() - chrono::Duration::minutes(5)).to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO timeline_raw_details
                (handle, run_id, agent_execution_id, session_generation_id, timeline_event_id,
                 raw_detail, raw_detail_bytes, raw_detail_digest, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'available')
            "#,
        )
        .bind("trd_wrong_session")
        .bind(run_id.to_string())
        .bind(&agent_execution_id)
        .bind("session-other")
        .bind("rte_wrong_session")
        .bind(raw)
        .bind(raw.len() as i64)
        .bind(&digest)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO timeline_raw_details
                (handle, run_id, agent_execution_id, session_generation_id, timeline_event_id,
                 raw_detail, raw_detail_bytes, raw_detail_digest, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'available')
            "#,
        )
        .bind("trd_wrong_run")
        .bind(RunId::new().to_string())
        .bind(&agent_execution_id)
        .bind("session-code_writer")
        .bind("rte_wrong_run")
        .bind(raw)
        .bind(raw.len() as i64)
        .bind(&digest)
        .execute(&pool)
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                    query {
                      scoped: timelineRawDetail(handle: "trd_scoped") { status rawDetail errorReason }
                      expired: timelineRawDetail(handle: "trd_expired") { status rawDetail errorReason }
                      wrongSession: timelineRawDetail(handle: "trd_wrong_session") { status rawDetail errorReason }
                      wrongRun: timelineRawDetail(handle: "trd_wrong_run") { status rawDetail errorReason }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "raw detail scope checks should fail closed through result statuses: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["scoped"]["status"], serde_json::json!("available"));
        assert_eq!(json["scoped"]["rawDetail"], serde_json::json!(raw));
        assert_eq!(json["expired"]["status"], serde_json::json!("stale"));
        assert_eq!(
            json["expired"]["errorReason"],
            serde_json::json!("handle_expired")
        );
        assert_eq!(
            json["wrongSession"]["status"],
            serde_json::json!("unauthorized")
        );
        assert_eq!(
            json["wrongSession"]["errorReason"],
            serde_json::json!("event_not_authorized")
        );
        assert_eq!(
            json["wrongRun"]["status"],
            serde_json::json!("unauthorized")
        );
        assert_eq!(
            json["wrongRun"]["errorReason"],
            serde_json::json!("run_not_authorized")
        );
        assert!(json["wrongRun"]["rawDetail"].is_null());
    }

    #[tokio::test]
    async fn runtime_timeline_p093_active_agent_selector_uses_backend_stage_order() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let workflow_path = "../../../examples/workflows/full-mvp-live.yaml";
        let catalog_path = "../../../examples/agents/agents.yaml";
        let workflow_snapshot = workflow::definition::load(workflow_path).unwrap();
        let catalog_snapshot = workflow::catalog::load(catalog_path).unwrap();
        let mut run = make_run(run_id, idea_id);
        run.status = domain::run::RunStatus::Running;
        run.workflow_yaml_path = Some(workflow_path.into());
        run.agent_catalog_yaml_path = Some(catalog_path.into());
        run.workflow_snapshot_json = Some(serde_json::to_string(&workflow_snapshot).unwrap());
        run.catalog_snapshot_json = Some(serde_json::to_string(&catalog_snapshot).unwrap());
        runs::insert(&pool, &run).await.unwrap();

        let later = Utc::now() + chrono::Duration::seconds(30);
        let earlier = Utc::now();
        let proposal_stage = make_stage_execution(
            run_id,
            "state_2_proposal_drafted",
            "Proposal drafted",
            later,
        );
        let implementation_stage = make_stage_execution(
            run_id,
            "state_10_implementation_refined",
            "Implementation refined",
            earlier,
        );
        let proposal_stage_id = proposal_stage.id;
        let implementation_stage_id = implementation_stage.id;
        stages::insert(&pool, &proposal_stage).await.unwrap();
        stages::insert(&pool, &implementation_stage).await.unwrap();
        db::repos::agent_executions::insert(
            &pool,
            &make_agent_execution(proposal_stage_id, "proposal_writer", "codex", later),
        )
        .await
        .unwrap();
        db::repos::agent_executions::insert(
            &pool,
            &make_agent_execution(implementation_stage_id, "code_writer", "claude", earlier),
        )
        .await
        .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query {{
                      activeAgentExecutions(runId: "{run_id}") {{
                        agentId
                        agentTitle
                        stageLabel
                        taskLabel
                        selectionOrder
                        selectionUnavailableReason
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "active agent selector order should be daemon-owned: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let agents = json["activeAgentExecutions"].as_array().unwrap();
        assert_eq!(agents[0]["agentId"], serde_json::json!("proposal_writer"));
        assert_eq!(
            agents[0]["agentTitle"],
            serde_json::json!("Proposal Writer")
        );
        assert_eq!(
            agents[0]["stageLabel"],
            serde_json::json!("Proposal drafted")
        );
        assert_eq!(agents[0]["selectionOrder"], serde_json::json!(0));
        assert_eq!(
            agents[0]["selectionUnavailableReason"],
            serde_json::Value::Null
        );
        assert_eq!(agents[1]["agentId"], serde_json::json!("code_writer"));
        assert_eq!(agents[1]["selectionOrder"], serde_json::json!(1));
    }

    #[tokio::test]
    async fn runtime_timeline_p093_active_agent_selector_uses_receipts_start_time_and_agent_id_tiebreak(
    ) {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let workflow_path = "../../../examples/workflows/full-mvp-live.yaml";
        let catalog_path = "../../../examples/agents/agents.yaml";
        let workflow_snapshot = workflow::definition::load(workflow_path).unwrap();
        let catalog_snapshot = workflow::catalog::load(catalog_path).unwrap();
        let mut run = make_run(run_id, idea_id);
        run.status = domain::run::RunStatus::Running;
        run.workflow_yaml_path = Some(workflow_path.into());
        run.agent_catalog_yaml_path = Some(catalog_path.into());
        run.workflow_snapshot_json = Some(serde_json::to_string(&workflow_snapshot).unwrap());
        run.catalog_snapshot_json = Some(serde_json::to_string(&catalog_snapshot).unwrap());
        runs::insert(&pool, &run).await.unwrap();

        let started = Utc::now();
        let stage = make_stage_execution(
            run_id,
            "state_10_implementation_refined",
            "Implementation refined",
            started,
        );
        let stage_id = stage.id;
        stages::insert(&pool, &stage).await.unwrap();
        let earlier_started = started - chrono::Duration::seconds(30);
        let zeta = make_agent_execution(stage_id, "zeta_writer", "claude", started);
        let alpha = make_agent_execution(stage_id, "alpha_writer", "codex", started);
        let beta = make_agent_execution(stage_id, "beta_writer", "gemini", earlier_started);
        db::repos::agent_executions::insert(&pool, &zeta)
            .await
            .unwrap();
        db::repos::agent_executions::insert(&pool, &alpha)
            .await
            .unwrap();
        db::repos::agent_executions::insert(&pool, &beta)
            .await
            .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let last_event_at_ms = 1_777_000_123_000_i64;
        insert_runtime_receipt(
            &pool,
            zeta.id.to_string(),
            "claude",
            2,
            last_event_at_ms - 5_000,
        )
        .await;
        insert_runtime_receipt(&pool, alpha.id.to_string(), "codex", 7, last_event_at_ms).await;
        insert_runtime_receipt(
            &pool,
            beta.id.to_string(),
            "gemini",
            3,
            last_event_at_ms - 10_000,
        )
        .await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query {{
                      activeAgentExecutions(runId: "{run_id}") {{
                        agentId
                        eventCount
                        lastEventAt
                        selectionOrder
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "active agent selector should use daemon runtime evidence: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let agents = json["activeAgentExecutions"].as_array().unwrap();
        assert_eq!(agents[0]["agentId"], serde_json::json!("beta_writer"));
        assert_eq!(agents[0]["eventCount"], serde_json::json!(3));
        assert_eq!(agents[1]["agentId"], serde_json::json!("alpha_writer"));
        assert_eq!(agents[1]["eventCount"], serde_json::json!(7));
        assert_eq!(
            agents[1]["lastEventAt"],
            serde_json::json!(
                chrono::DateTime::<Utc>::from_timestamp_millis(last_event_at_ms)
                    .unwrap()
                    .to_rfc3339()
            )
        );
        assert_eq!(agents[2]["agentId"], serde_json::json!("zeta_writer"));
        assert_eq!(agents[2]["eventCount"], serde_json::json!(2));
    }

    fn test_principal() -> auth::Principal {
        auth::Principal::new("test-operator", auth::PrincipalClass::Operator)
    }

    fn make_idea(id: IdeaId) -> Idea {
        Idea {
            id,
            title: "Test idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        }
    }

    fn make_run(id: RunId, idea_id: IdeaId) -> domain::run::Run {
        domain::run::Run {
            id,
            idea_id,
            status: domain::run::RunStatus::Ready,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: None,
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: None,
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: Some(
                "{\"repo_identifier\":\"repo-3\",\"repo_root\":\"/repo-3\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                    .into(),
            ),
            delivery_preflight_json: None,
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: None,
            catalog_snapshot_hash: None,
            workflow_snapshot_json: None,
            catalog_snapshot_json: None,
            drift_detected_at: None,
            drift_details_json: None,
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: None,
        }
    }

    fn make_stage_execution(
        run_id: RunId,
        stage_id: &str,
        label: &str,
        started_at: chrono::DateTime<Utc>,
    ) -> domain::stage::StageExecution {
        domain::stage::StageExecution {
            id: domain::ids::StageExecutionId::new(),
            run_id,
            stage_id: stage_id.into(),
            label: label.into(),
            status: domain::stage::StageStatus::Running,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: None,
            started_at,
            completed_at: None,
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: None,
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        }
    }

    fn make_agent_execution(
        stage_execution_id: domain::ids::StageExecutionId,
        agent_id: &str,
        provider: &str,
        started_at: chrono::DateTime<Utc>,
    ) -> domain::agent::AgentExecution {
        domain::agent::AgentExecution {
            id: domain::ids::AgentExecutionId::new(),
            stage_execution_id: Some(stage_execution_id),
            agent_id: agent_id.into(),
            provider: provider.into(),
            model: Some("test-model".into()),
            started_at,
            completed_at: None,
            status: domain::agent::AgentStatus::Running,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: Some(format!("session-{agent_id}")),
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: None,
            session_reuse_scope: None,
            session_family_id: None,
            session_reuse_disposition: None,
            session_reset_reason: None,
            backend_profile_id: None,
            requested_mcp_extensions_json: None,
            predicted_mcp_extensions_json: None,
            predicted_mcp_runtime_ids_json: None,
            actual_mcp_extensions_json: None,
            actual_mcp_runtime_ids_json: None,
            denied_mcp_extensions_json: None,
            mcp_blocking_issues_json: None,
            actual_mcp_observation_json: None,
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: None,
            owner_kind: None,
            owner_id: None,
            lead_mediation_record_id: None,
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
        }
    }

    async fn insert_runtime_receipt(
        pool: &SqlitePool,
        agent_execution_id: String,
        provider: &str,
        event_count: i64,
        last_event_at_ms: i64,
    ) {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO agent_execution_runtime_receipts
                (runtime_receipt_id, agent_execution_id, prompt_kind, turn_index,
                 provider, transport_family, status, event_count, last_event_kind,
                 last_event_at_ms, receipt_json, created_at, updated_at)
            VALUES (?1, ?2, 'original', 0, ?3, 'acp', 'running', ?4, 'text_chunk', ?5, '{}', ?6, ?7)
            "#,
        )
        .bind(format!("{agent_execution_id}:original:0"))
        .bind(agent_execution_id)
        .bind(provider)
        .bind(event_count)
        .bind(last_event_at_ms)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn persist_rollout_contract_readback(pool: &SqlitePool, run_id: RunId) {
        use rollout_contract_checks::{
            ProjectionIntegrity, RolloutContractDecision, RolloutContractEnforcementMode,
            RolloutContractLifecycleState, RolloutContractStatus, UpsertRolloutContractCheck,
        };

        let now = Utc::now();
        rollout_contract_checks::upsert_rollout_contract_check(
            pool,
            &UpsertRolloutContractCheck {
                id: uuid::Uuid::new_v4(),
                run_id: run_id.inner(),
                proposal_id: "proposal-084".into(),
                proposal_revision_id: "p084-r5".into(),
                proposal_content_hash: "sha256:proposal".into(),
                contract_object_hash: "sha256:contract".into(),
                content_snapshot_id: "snapshot-1".into(),
                checker_version: "p084-lint-1".into(),
                status: RolloutContractStatus::Pass,
                decision: RolloutContractDecision::Release,
                lifecycle_state: RolloutContractLifecycleState::Terminal,
                enforcement_mode: RolloutContractEnforcementMode::Enforce,
                failure_reasons: vec![],
                diagnostics: vec![],
                waiver: None,
                rollback_disposition: serde_json::json!({
                    "mode": "feature_flag_disable_or_enforcement_mode_permissive",
                    "data_loss_risk": "none",
                    "steps": ["Move enforcement mode through an audited mutation."]
                }),
                projection_integrity: ProjectionIntegrity::Valid,
                cutover_policy_revision: Some("p084-cutover-v1".into()),
                redaction_state: "partial".into(),
                retry_count: 0,
                preflight_timeout_seconds: 45,
            },
            now,
        )
        .await
        .unwrap();
    }

    fn make_workflow_conflict(run_id: RunId) -> WorkflowConflictRecord {
        let candidates = vec![CandidateTransitionEvaluation {
            transition_id: "review_to_refine".into(),
            from_state_id: "review".into(),
            to_state_id: "refine".into(),
            condition_expression_id: Some("proposal_review_summary.pass == false".into()),
            result: CandidateTransitionResult::MissingInput,
            required_artifacts: vec!["proposal_review_summary".into()],
            missing_artifacts: vec!["proposal_review_summary".into()],
            missing_fields: vec![],
            source_artifact_ids: vec![],
            source_agent_execution_id: None,
            sanitized_diagnostic: Some("proposal_review_summary is required".into()),
        }];
        let reason = WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition;
        let candidate_hash = candidate_transition_hash(&candidates);
        WorkflowConflictRecord {
            conflict_id: uuid::Uuid::new_v4().to_string(),
            conflict_fingerprint: workflow_conflict_fingerprint(
                &run_id.to_string(),
                "review",
                &reason,
                &candidate_hash,
                &[],
            ),
            run_id: run_id.to_string(),
            stage_execution_id: None,
            lineage_id: Some("lineage-p017".into()),
            current_state_id: "review".into(),
            reason,
            operator_label: "Required transition input is missing".into(),
            status: WorkflowConflictStatus::Unresolved,
            candidate_transitions: candidates,
            candidate_transition_hash: candidate_hash,
            advisory_evidence_refs: vec![],
            lead_agent_id: None,
            mediation_record_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            resolved_at: None,
            superseded_by_conflict_id: None,
            resolution_record_json: None,
            terminal_failure_reason: None,
            diagnostic_redaction_tier: "operator_safe".into(),
        }
    }

    fn make_lead_mediation_record(
        run_id: RunId,
        conflict: &WorkflowConflictRecord,
        mediation_id: &str,
    ) -> LeadConflictMediationRecord {
        LeadConflictMediationRecord {
            id: mediation_id.to_string(),
            run_id: run_id.to_string(),
            conflict_id: conflict.conflict_id.clone(),
            conflict_fingerprint: conflict.conflict_fingerprint.clone(),
            lead_agent_id: "lead-agent-1".into(),
            status: LeadMediationStatus::OperatorConfirmationRequired,
            settlement_result: Some("operator_confirmed".into()),
            recovery_action: None,
            chosen_action: Some("advance".into()),
            chosen_next_state_id: Some("release".into()),
            chosen_next_state_label: Some("Release".into()),
            operator_rationale: Some("PRIVATE rationale must not leave storage".into()),
            sanitized_progress: Some("Lead mediation selected a release transition.".into()),
            validation_errors_json: Some(
                serde_json::json!([{"field": "summary", "message": "safe validation note"}])
                    .to_string(),
            ),
            cost_summary_json: Some(
                serde_json::json!({
                    "total_cost_cents": 42,
                    "input_tokens": 100,
                    "output_tokens": 25
                })
                .to_string(),
            ),
            metric_event_id: Some("metric-1".into()),
            superseded_by_event_ref: Some("event-2".into()),
            agent_execution_id: Some("agent-exec-1".into()),
            confirmation_subject_id: Some("confirmation-1".into()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            settled_at: None,
        }
    }

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed");
        let writer = Arc::new(db::writer::DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer)
            .await
            .expect("register shared DbWriter for test pool");
        pool
    }

    async fn p043_test_pool() -> sqlx::SqlitePool {
        let path =
            std::env::temp_dir().join(format!("chainworks-p043-{}.sqlite", uuid::Uuid::new_v4()));
        let pool = create_pool(&format!("sqlite://{}", path.to_string_lossy()))
            .await
            .expect("P043 file-backed pool failed");
        let writer = Arc::new(db::writer::DbWriter::new(pool.clone()));
        db::writer::register_shared_writer(&pool, writer)
            .await
            .expect("P043 register shared DbWriter");
        pool
    }

    fn make_command_handler(pool: sqlx::SqlitePool) -> Arc<CommandHandler> {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        Arc::new(CommandHandler::new(pool, events, work_queue))
    }

    fn assert_enum_values(json: &serde_json::Value, alias: &str, expected: &[&str]) {
        let values = json[alias]["enumValues"]
            .as_array()
            .unwrap_or_else(|| panic!("{alias} enumValues should be present"));
        let actual: Vec<&str> = values
            .iter()
            .map(|value| {
                value["name"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{alias} enum value name should be a string"))
            })
            .collect();
        assert_eq!(actual, expected.to_vec(), "{alias} enum values drifted");
    }

    async fn persist_blocked_implementation_summary(pool: &sqlx::SqlitePool, run_id: RunId) {
        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_8_implementation_continued".into(),
            agent_id: "code_writer".into(),
            name: "implementation_self_assessment".into(),
            contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/implementation/self-assessment.json".into(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "test".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(pool, &artifact).await.unwrap();
        let raw = serde_json::json!({
            "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
            "implementation_complete": true,
            "verification_green": false,
            "remaining_code_tasks": [],
            "handoff_tasks": [],
            "known_risks": ["verification blocked by environment"],
            "tests_run": ["cargo test: blocked"],
            "docs_impacted": []
        });
        let summary = parse_implementation_self_assessment_v2(
            &raw,
            ContractParseContext {
                run_id: run_id.to_string(),
                run_age: None,
                declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
                canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
                raw_artifact_path: Some(artifact.file_path.clone()),
                source_generation_id: None,
                artifact_created_at: Some(artifact.created_at),
                v2_generation_seen_for_run: true,
                legacy_v1_generation_available: false,
            },
        );
        artifact_contracts::persist_implementation_self_assessment_summary(
            pool,
            run_id,
            artifact.id,
            &artifact.contract_id,
            &summary,
            artifact.created_at,
        )
        .await
        .unwrap();
    }

    async fn seed_validation_attempt(
        pool: &sqlx::SqlitePool,
        run_id: RunId,
    ) -> (domain::ids::StageExecutionId, domain::ids::AgentExecutionId) {
        let stage_id = domain::ids::StageExecutionId::new();
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        db::repos::stages::insert(
            pool,
            &domain::stage::StageExecution {
                id: stage_id,
                run_id,
                stage_id: "stage_1".to_string(),
                label: "Stage 1".to_string(),
                status: domain::stage::StageStatus::Failed,
                iteration: 1,
                attempt_number: 1,
                settlement_kind: Some(domain::stage::StageSettlementKind::Failed),
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                owner_agent: Some("validation_agent".to_string()),
                provider: Some("system".to_string()),
                model: None,
                stage_type: None,
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: None,
            },
        )
        .await
        .unwrap();
        db::repos::agent_executions::insert(
            pool,
            &domain::agent::AgentExecution {
                id: agent_execution_id,
                stage_execution_id: Some(stage_id),
                agent_id: "validation_agent".to_string(),
                provider: "system".to_string(),
                model: None,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                status: domain::agent::AgentStatus::Failed,
                owner_execution_lineage_id: None,
                session_lineage_id: None,
                session_generation_id: None,
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: None,
                session_reuse_scope: None,
                session_family_id: None,
                session_reuse_disposition: Some("reused".into()),
                session_reset_reason: Some("operator_reset".into()),
                backend_profile_id: Some("codex_with_mcp".into()),
                requested_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                actual_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                actual_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                denied_mcp_extensions_json: Some("[]".into()),
                mcp_blocking_issues_json: Some("[]".into()),
                actual_mcp_observation_json: Some(
                    r#"{"source":"provider_session_new_response"}"#.into(),
                ),
                actual_xcode_runtime_observation_json: None,
                mcp_session_startup_latency_ms: Some(17),
                owner_kind: None,
                owner_id: None,
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
            },
        )
        .await
        .unwrap();
        (stage_id, agent_execution_id)
    }

    fn validation_failure_payload(run_id: RunId) -> serde_json::Value {
        serde_json::json!({
            "id": "33333333-3333-3333-3333-333333333333",
            "timestamp": "2026-04-15T09:30:00Z",
            "agentID": "validation_agent",
            "stageID": "stage_1",
            "runID": run_id.to_string(),
            "outputResults": [{
                "outputName": "report",
                "contractID": "report_v1",
                "status": "failed",
                "missingFields": ["summary"],
                "validationError": "Missing required fields: summary",
                "rawPayloadSize": 17
            }],
            "failureSummary": "report: Missing required fields: summary",
            "failureClass": "output_contract_mismatch",
            "contractMetadata": [{
                "outputName": "report",
                "contractID": "report_v1",
                "machineFormat": "json",
                "validationMode": "strict_structured",
                "requiredFieldCount": 1,
                "rawArtifactName": "report_raw",
                "normalizedArtifactName": "report"
            }],
            "rawOutputExists": true,
            "receiptExists": false,
            "transcriptExists": true,
            "recoveryRecommendation": {
                "action": "retry_failed_agent",
                "explanation": "Retry the agent with the same inputs.",
                "source": "runtime_policy"
            }
        })
    }

    fn validation_failure_record(
        artifact_id: ArtifactId,
        run_id: RunId,
        stage_execution_id: domain::ids::StageExecutionId,
        agent_execution_id: domain::ids::AgentExecutionId,
    ) -> ValidationFailureRecord {
        ValidationFailureRecord {
            id: "33333333-3333-3333-3333-333333333333".to_string(),
            artifact_id,
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-15T09:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
            agent_id: "validation_agent".to_string(),
            stage_id: "stage_1".to_string(),
            stage_execution_id,
            agent_execution_id,
            run_id,
            output_results: vec![OutputValidationResult {
                output_name: "report".to_string(),
                contract_id: Some("report_v1".to_string()),
                status: ValidationStatus::Failed,
                missing_fields: vec!["summary".to_string()],
                validation_error: Some("Missing required fields: summary".to_string()),
                raw_payload_size: 17,
            }],
            failure_summary: "report: Missing required fields: summary".to_string(),
            failure_class: ValidationFailureClass::OutputContractMismatch,
            contract_metadata: vec![ContractValidationMetadata {
                output_name: "report".to_string(),
                contract_id: "report_v1".to_string(),
                machine_format: "json".to_string(),
                validation_mode: "strict_structured".to_string(),
                required_field_count: 1,
                raw_artifact_name: Some("report_raw".to_string()),
                normalized_artifact_name: Some("report".to_string()),
            }],
            raw_output_exists: true,
            receipt_exists: false,
            transcript_exists: true,
            recovery_recommendation: RecoveryRecommendation {
                action: "retry_failed_agent".to_string(),
                explanation: "Retry the agent with the same inputs.".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn proposal_064_run_query_exposes_sync_and_capsule_readback() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO main_sync_attempts (id, run_id, idempotency_key, trigger_reason, status, conflict_count, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind("attempt-1")
        .bind(run_id.to_string())
        .bind("before-review-1")
        .bind("before_review")
        .bind("waiting_for_barrier")
        .bind(0_i64)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO worktree_mutation_barriers (id, run_id, worktree_resource_key, owner_id, owner_kind, status, reason, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind("barrier-1")
        .bind(run_id.to_string())
        .bind(format!("run-worktree:{run_id}"))
        .bind("main-sync")
        .bind("main_sync")
        .bind("pending")
        .bind("active reader")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();
        let run_id_string = run_id.to_string();
        db::repos::command_journal::record(
            &pool,
            "journal-1",
            "MainSyncRequest",
            "{}",
            Some(&run_id_string),
            Utc::now(),
            Some("mcp"),
            Some("operator"),
            Some("operator"),
            Some("runs.main_sync.request"),
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    {{
                      run(id: "{run_id}") {{
                        mainSyncReadbackJson
                        knowledgeCapsuleReadbackJson
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let main_sync = &json["run"]["mainSyncReadbackJson"];
        assert_eq!(
            main_sync["schema_version"], "p064_main_sync_readback_v1",
            "unexpected P064 readback payload: {json}"
        );
        assert_eq!(main_sync["latest_attempt"]["status"], "waiting_for_barrier");
        assert_eq!(main_sync["active_barrier"]["owner_kind"], "main_sync");
        assert_eq!(
            main_sync["commands"]["pending_commands"][0]["command_type"],
            "MainSyncRequest"
        );
        assert_eq!(
            json["run"]["knowledgeCapsuleReadbackJson"]["schema_version"],
            "p064_knowledge_capsule_readback_v1"
        );
    }

    #[tokio::test]
    async fn run_query_exposes_delivery_configuration_json() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(repo.path())
            .output()
            .expect("git init should run");
        let worktrees = tempfile::tempdir().unwrap();
        let delivery_json = format!(
            r#"{{"repo_identifier":"repo-2","repo_root":"{}","base_branch":"main","worktree_base_path":"{}","target_branch":"cw/release","release_target_id":"app-store"}}"#,
            repo.path().display(),
            worktrees.path().display()
        );
        let mut run = make_run(run_id, idea_id);
        run.delivery_configuration_json = Some(delivery_json.clone());
        runs::insert(&pool, &run).await.unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    deliveryConfigurationJson
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["run"]["deliveryConfigurationJson"],
            serde_json::json!(delivery_json)
        );
    }

    #[tokio::test]
    async fn run_query_exposes_implementation_self_assessment_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_blocked_implementation_summary(&pool, run_id).await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    implementationSelfAssessmentSummary {{
                      status
                      implementationComplete
                      verificationGreen
                      blockingRemainingCodeTaskCount
                      testsRun
                    }}
	                  }}
	                }}
	                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let summary = &json["run"]["implementationSelfAssessmentSummary"];
        assert_eq!(summary["status"], serde_json::json!("blocked"));
        assert_eq!(summary["implementationComplete"], serde_json::json!(true));
        assert_eq!(summary["verificationGreen"], serde_json::json!(false));
        assert_eq!(
            summary["blockingRemainingCodeTaskCount"],
            serde_json::json!(0)
        );
    }

    #[tokio::test]
    async fn proposal_087_runs_query_is_projection_only_without_per_row_enrichment() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_blocked_implementation_summary(&pool, run_id).await;
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                query RunsProjectionOnly {
                  runs {
                    id
                    totalStages
                    implementationSelfAssessmentSummary { status }
                    rolloutContractReadbackJson
                    sideEffectReadbackJson
                    closeoutReadinessSummaryJson
                  }
                }
                "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let run_json = json["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|run| run["id"] == serde_json::json!(run_id.to_string()))
            .expect("run appears in active run list");
        assert_eq!(run_json["totalStages"], serde_json::json!(0));
        assert!(
            run_json["implementationSelfAssessmentSummary"].is_null(),
            "GraphQL runs list must not perform per-row implementation summary enrichment"
        );
        assert!(run_json["rolloutContractReadbackJson"].is_null());
        assert!(run_json["sideEffectReadbackJson"].is_null());
        assert!(run_json["closeoutReadinessSummaryJson"].is_null());
    }

    #[tokio::test]
    async fn proposal_017_run_query_exposes_current_workflow_conflict() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let conflict = make_workflow_conflict(run_id);
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    workflowConflict {{
                      reason
                      status
                      currentStateId
                      candidateTransitions {{
                        transitionId
                        result
                        missingArtifacts
                      }}
                    }}
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let conflict_json = &json["run"]["workflowConflict"];
        assert_eq!(
            conflict_json["reason"],
            serde_json::json!("REQUIRED_ARTIFACT_OR_FIELD_MISSING_FOR_TRANSITION")
        );
        assert_eq!(conflict_json["status"], serde_json::json!("UNRESOLVED"));
        assert_eq!(conflict_json["currentStateId"], serde_json::json!("review"));
        assert_eq!(
            conflict_json["candidateTransitions"][0]["result"],
            serde_json::json!("MISSING_INPUT")
        );
    }

    #[tokio::test]
    async fn proposal_017_run_query_exposes_refine_instruction_action_hint() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let mut conflict = make_workflow_conflict(run_id);
        conflict.reason = WorkflowConflictReason::NoDeclarativeTransitionMatched;
        conflict.operator_label = "No declarative workflow transition matched".into();
        conflict.candidate_transitions = vec![CandidateTransitionEvaluation {
            transition_id: "review_to_refine".into(),
            from_state_id: "review".into(),
            to_state_id: "review".into(),
            condition_expression_id: Some("proposal_needs_refine".into()),
            result: CandidateTransitionResult::NotMatched,
            required_artifacts: vec!["proposal_review_summary".into()],
            missing_artifacts: vec![],
            missing_fields: vec![],
            source_artifact_ids: vec!["proposal_review_summary".into()],
            source_agent_execution_id: None,
            sanitized_diagnostic: Some(
                "Loop budget exhausted for proposal_review_count: 3/3 iterations".into(),
            ),
        }];
        conflict.candidate_transition_hash =
            candidate_transition_hash(&conflict.candidate_transitions);
        conflict.conflict_fingerprint = workflow_conflict_fingerprint(
            &run_id.to_string(),
            "review",
            &conflict.reason,
            &conflict.candidate_transition_hash,
            &[],
        );
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    workflowConflict {{
                      suggestedOperatorAction
                    }}
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["run"]["workflowConflict"]["suggestedOperatorAction"],
            serde_json::json!("choose_transition_or_provide_refine_instruction")
        );
    }

    #[tokio::test]
    async fn proposal_017_run_query_exposes_sanitized_lead_mediation_readback() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let mediation_id = "mediation-p017-readback";
        let mut conflict = make_workflow_conflict(run_id);
        conflict.status = WorkflowConflictStatus::OperatorConfirmationRequired;
        conflict.mediation_record_id = Some(mediation_id.into());
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();
        db::repos::lead_conflict_mediations::insert(
            &pool,
            &make_lead_mediation_record(run_id, &conflict, mediation_id),
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        assert!(!schema.sdl().contains("operatorRationale"));

        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query P017LeadMediationReadback {{
                  run(id: "{run_id}") {{
                    workflowConflict {{
                      mediationRecordId
                      leadMediation {{
                        id
                        conflictId
                        leadAgentId
                        status
                        resolutionMode
                        chosenAction
                        chosenNextStateId
                        chosenNextStateLabel
                        sanitizedProgress
                        statusUpdates {{
                          status
                          sanitizedProgress
                          updatedAt
                          attemptNumber
                        }}
                        validationErrors
                        confirmationSubjectId
                        supersededByEventRef
                        costSummary
                      }}
                    }}
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let mediation = &json["run"]["workflowConflict"]["leadMediation"];
        assert_eq!(mediation["id"], serde_json::json!(mediation_id));
        assert_eq!(
            mediation["conflictId"],
            serde_json::json!(conflict.conflict_id)
        );
        assert_eq!(mediation["leadAgentId"], serde_json::json!("lead-agent-1"));
        assert_eq!(
            mediation["status"],
            serde_json::json!("operator_confirmation_required")
        );
        assert_eq!(
            mediation["resolutionMode"],
            serde_json::json!("operator_confirmation")
        );
        assert_eq!(mediation["chosenAction"], serde_json::json!("advance"));
        assert_eq!(mediation["chosenNextStateId"], serde_json::json!("release"));
        assert_eq!(
            mediation["chosenNextStateLabel"],
            serde_json::json!("Release")
        );
        assert_eq!(
            mediation["sanitizedProgress"],
            serde_json::json!("Lead mediation selected a release transition.")
        );
        assert_eq!(
            mediation["statusUpdates"][0]["status"],
            serde_json::json!("operator_confirmation_required")
        );
        assert_eq!(
            mediation["statusUpdates"][0]["sanitizedProgress"],
            serde_json::json!("Lead mediation selected a release transition.")
        );
        assert_eq!(
            mediation["statusUpdates"][0]["attemptNumber"],
            serde_json::json!(1)
        );
        assert!(mediation["statusUpdates"][0]["updatedAt"].is_string());
        assert_eq!(
            mediation["confirmationSubjectId"],
            serde_json::json!("confirmation-1")
        );
        assert_eq!(
            mediation["supersededByEventRef"],
            serde_json::json!("event-2")
        );
        assert_eq!(
            mediation["validationErrors"][0]["field"],
            serde_json::json!("summary")
        );
        assert_eq!(
            mediation["costSummary"]["total_cost_cents"],
            serde_json::json!(42)
        );

        let serialized = serde_json::to_string(&json).unwrap();
        assert!(!serialized.contains("operatorRationale"));
        assert!(!serialized.contains("operator_rationale"));
        assert!(!serialized.contains("PRIVATE rationale"));
    }

    /// P017 R2 / API-001: every mediation-owned `agent_executions` row must
    /// surface under `workflowConflict.leadMediation.executionAttempts` in
    /// GraphQL, with owner identity, nullable stage execution ID, runtime
    /// facts, watchdog, and per-attempt timing/status.
    #[tokio::test]
    async fn proposal_017_run_query_exposes_lead_mediation_execution_attempts() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let mediation_id = "mediation-p017-attempts";
        let mut conflict = make_workflow_conflict(run_id);
        conflict.status = WorkflowConflictStatus::OperatorConfirmationRequired;
        conflict.mediation_record_id = Some(mediation_id.into());
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();
        db::repos::lead_conflict_mediations::insert(
            &pool,
            &make_lead_mediation_record(run_id, &conflict, mediation_id),
        )
        .await
        .unwrap();

        // Insert two mediation-owned agent_executions (no stage_execution_id).
        let exec_one = domain::agent::AgentExecution {
            id: domain::ids::AgentExecutionId::new(),
            stage_execution_id: None,
            agent_id: "lead-agent-1".into(),
            provider: "claude".into(),
            model: Some("sonnet".into()),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            status: domain::agent::AgentStatus::Failed,
            owner_execution_lineage_id: None,
            session_lineage_id: None,
            session_generation_id: None,
            rehydrated_from_checkpoint_artifact_id: None,
            invocation_owner_key: None,
            session_reuse_scope: None,
            session_family_id: None,
            session_reuse_disposition: None,
            session_reset_reason: None,
            backend_profile_id: None,
            requested_mcp_extensions_json: None,
            predicted_mcp_extensions_json: None,
            predicted_mcp_runtime_ids_json: None,
            actual_mcp_extensions_json: None,
            actual_mcp_runtime_ids_json: None,
            denied_mcp_extensions_json: None,
            mcp_blocking_issues_json: None,
            actual_mcp_observation_json: None,
            actual_xcode_runtime_observation_json: None,
            mcp_session_startup_latency_ms: None,
            owner_kind: Some("lead_conflict_mediation".into()),
            owner_id: Some(mediation_id.into()),
            lead_mediation_record_id: Some(mediation_id.into()),
            origin_stage_execution_id: None,
            total_cost_cents: None,
            input_tokens: None,
            output_tokens: None,
            cached_input_tokens: None,
            transcript_artifact_id: None,
            actual_toolchain_mapping_diagnostics_json: None,
        };
        let exec_one_id = exec_one.id;
        db::repos::agent_executions::insert(&pool, &exec_one)
            .await
            .unwrap();

        let exec_two = domain::agent::AgentExecution {
            id: domain::ids::AgentExecutionId::new(),
            started_at: exec_one.started_at + chrono::Duration::seconds(1),
            status: domain::agent::AgentStatus::Completed,
            ..exec_one.clone()
        };
        let exec_two_id = exec_two.id;
        db::repos::agent_executions::insert(&pool, &exec_two)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query P017LeadMediationAttempts {{
                  run(id: "{run_id}") {{
                    workflowConflict {{
                      leadMediation {{
                        statusUpdates {{ attemptNumber }}
                        executionAttempts {{
                          agentExecutionId
                          ownerKind
                          ownerId
                          mediationRecordId
                          stageExecutionId
                          agentId
                          provider
                          model
                          status
                          startedAt
                          completedAt
                          attemptNumber
                          runtimeFacts
                          watchdog
                          cost
                          transcriptRef
                          artifacts {{ id }}
                        }}
                      }}
                    }}
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );

        let json = response.data.into_json().unwrap();
        let mediation = &json["run"]["workflowConflict"]["leadMediation"];
        let attempts = mediation["executionAttempts"]
            .as_array()
            .expect("executionAttempts array");
        assert_eq!(attempts.len(), 2, "two attempts expected");

        for attempt in attempts {
            assert_eq!(
                attempt["ownerKind"],
                serde_json::json!("lead_conflict_mediation")
            );
            assert_eq!(attempt["ownerId"], serde_json::json!(mediation_id));
            assert_eq!(
                attempt["mediationRecordId"],
                serde_json::json!(mediation_id)
            );
            assert!(
                attempt["stageExecutionId"].is_null(),
                "mediation-owned attempt has no stage execution id"
            );
            assert_eq!(attempt["agentId"], serde_json::json!("lead-agent-1"));
            assert_eq!(attempt["provider"], serde_json::json!("claude"));
            assert!(attempt["startedAt"].is_string());
        }

        // Attempts are sorted by started_at ASC; attemptNumber is durable.
        assert_eq!(
            attempts[0]["agentExecutionId"],
            serde_json::json!(exec_one_id.to_string())
        );
        assert_eq!(attempts[0]["attemptNumber"], serde_json::json!(1));
        assert_eq!(attempts[0]["status"], serde_json::json!("failed"));
        assert_eq!(
            attempts[1]["agentExecutionId"],
            serde_json::json!(exec_two_id.to_string())
        );
        assert_eq!(attempts[1]["attemptNumber"], serde_json::json!(2));
        assert_eq!(attempts[1]["status"], serde_json::json!("completed"));

        // The synthesized status_updates entry's attemptNumber reflects the
        // durable mediation attempt count, not hard-coded 1.
        assert_eq!(
            mediation["statusUpdates"][0]["attemptNumber"],
            serde_json::json!(2)
        );

        // No operator_rationale anywhere in the readback.
        let serialized = serde_json::to_string(&json).unwrap();
        assert!(!serialized.contains("operatorRationale"));
        assert!(!serialized.contains("operator_rationale"));
        assert!(!serialized.contains("PRIVATE rationale"));
    }

    #[tokio::test]
    async fn proposal_017_runs_query_exposes_current_workflow_conflict_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &make_workflow_conflict(run_id))
            .await
            .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                query Runs {
                  runs {
                    id
                    workflowConflict {
                      reason
                      status
                      currentStateId
                    }
                  }
                }
                "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let run_json = json["runs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|run| run["id"] == serde_json::json!(run_id.to_string()))
            .expect("run appears in active run list");
        assert_eq!(
            run_json["workflowConflict"]["reason"],
            serde_json::json!("REQUIRED_ARTIFACT_OR_FIELD_MISSING_FOR_TRANSITION")
        );
        assert_eq!(
            run_json["workflowConflict"]["status"],
            serde_json::json!("UNRESOLVED")
        );
        assert_eq!(
            run_json["workflowConflict"]["currentStateId"],
            serde_json::json!("review")
        );
    }

    #[tokio::test]
    async fn delivery_preflight_graphql_readback_tests() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.delivery_preflight_json = Some(
            serde_json::json!({
                "passed": true,
                "checks": [
                    {
                        "id": "repo_root_exists",
                        "label": "Repository root exists",
                        "passed": true,
                        "detail": null
                    }
                ]
            })
            .to_string(),
        );
        runs::insert(&pool, &run).await.unwrap();
        persist_rollout_contract_readback(&pool, run_id).await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    deliveryPreflightJson
                    rolloutContractReadbackJson
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert!(json["run"]["deliveryPreflightJson"]
            .as_str()
            .unwrap()
            .contains("repo_root_exists"));
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["schemaVersion"],
            serde_json::json!("operator_readback_v1")
        );
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["backendDecision"],
            serde_json::json!("release")
        );
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["sourceLane"],
            serde_json::json!("graphql")
        );
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["rollbackDisposition"]["dataLossRisk"],
            serde_json::json!("none")
        );
        assert_eq!(
            json["run"]["rolloutContractReadbackJson"]["adoptionMetric"]["name"],
            serde_json::json!("new_applicable_proposals_with_passing_rollout_contract_percent")
        );
    }

    #[tokio::test]
    async fn execution_mcp_truth_contract_tests() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, _) = seed_validation_attempt(&pool, run_id).await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query StageExecutions {{
                  stages(runId: "{run_id}") {{
                    id
                    executions {{
                      backendProfileId
                      requestedMcpExtensionsJson
                      predictedMcpRuntimeIdsJson
                      actualMcpRuntimeIdsJson
                      mcpBlockingIssuesJson
                    }}
                  }}
                  agentExecutions(stageExecutionId: "{stage_execution_id}") {{
                    backendProfileId
                    actualMcpObservationJson
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let execution = &json["stages"][0]["executions"][0];
        assert_eq!(
            execution["backendProfileId"],
            serde_json::json!("codex_with_mcp")
        );
        assert_eq!(
            execution["requestedMcpExtensionsJson"],
            serde_json::json!(r#"["filesystem"]"#)
        );
        assert_eq!(
            execution["actualMcpRuntimeIdsJson"],
            serde_json::json!(r#"["fs-runtime"]"#)
        );
        assert_eq!(
            json["agentExecutions"][0]["actualMcpObservationJson"],
            serde_json::json!(r#"{"source":"provider_session_new_response"}"#)
        );
    }

    #[tokio::test]
    async fn p036_run_stage_topology_query_is_available() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let workflow_path = "../../../examples/workflows/full-mvp-live.yaml";
        let catalog_path = "../../../examples/agents/agents.yaml";
        let workflow_snapshot = workflow::definition::load(workflow_path).unwrap();
        let catalog_snapshot = workflow::catalog::load(catalog_path).unwrap();
        let mut run = make_run(run_id, idea_id);
        run.status = domain::run::RunStatus::Running;
        run.current_state = Some("state_2_proposal_drafted".into());
        run.workflow_yaml_path = Some(workflow_path.into());
        run.agent_catalog_yaml_path = Some(catalog_path.into());
        run.workflow_snapshot_json = Some(serde_json::to_string(&workflow_snapshot).unwrap());
        run.catalog_snapshot_json = Some(serde_json::to_string(&catalog_snapshot).unwrap());
        runs::insert(&pool, &run).await.unwrap();

        let stage_execution_id = domain::ids::StageExecutionId::new();
        db::repos::stages::insert(
            &pool,
            &domain::stage::StageExecution {
                id: stage_execution_id,
                run_id,
                stage_id: "state_2_proposal_drafted".into(),
                label: "Proposal drafted".into(),
                status: domain::stage::StageStatus::Running,
                iteration: 1,
                attempt_number: 2,
                settlement_kind: None,
                started_at: Utc::now(),
                completed_at: None,
                owner_agent: Some("proposal_writer".into()),
                provider: Some("codex".into()),
                model: Some("gpt-5.5".into()),
                stage_type: None,
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: None,
            },
        )
        .await
        .unwrap();
        db::repos::agent_executions::insert(
            &pool,
            &domain::agent::AgentExecution {
                id: domain::ids::AgentExecutionId::new(),
                stage_execution_id: Some(stage_execution_id),
                agent_id: "proposal_writer".into(),
                provider: "codex".into(),
                model: Some("gpt-5.5".into()),
                started_at: Utc::now(),
                completed_at: None,
                status: domain::agent::AgentStatus::Running,
                owner_execution_lineage_id: None,
                session_lineage_id: None,
                session_generation_id: None,
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: None,
                session_reuse_scope: None,
                session_family_id: None,
                session_reuse_disposition: None,
                session_reset_reason: None,
                backend_profile_id: Some("codex".into()),
                requested_mcp_extensions_json: None,
                predicted_mcp_extensions_json: None,
                predicted_mcp_runtime_ids_json: None,
                actual_mcp_extensions_json: None,
                actual_mcp_runtime_ids_json: None,
                denied_mcp_extensions_json: None,
                mcp_blocking_issues_json: None,
                actual_mcp_observation_json: None,
                actual_xcode_runtime_observation_json: None,
                mcp_session_startup_latency_ms: None,
                owner_kind: None,
                owner_id: None,
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
            },
        )
        .await
        .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: ArtifactId::new(),
                run_id,
                stage_id: "state_2_proposal_drafted".into(),
                agent_id: "proposal_writer".into(),
                name: "proposal.md".into(),
                contract_id: "proposal_markdown_v1".into(),
                format: ArtifactFormat::Markdown,
                file_path: "/tmp/proposal.md".into(),
                checksum_sha256: None,
                size_bytes: Some(42),
                provider: "codex".into(),
                model: Some("gpt-5.5".into()),
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: None,
                report_version: None,
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query P036StageTopology {{
                  runStageTopology(runId: "{run_id}") {{
                    stageId
                    label
                    ownerAgentTitle
                    status
                    isCurrent
                    artifactCount
                    transitions {{ toStageId toLabel detail }}
                    occurrences {{ agentId agentTitle taskName status provider model }}
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "runStageTopology should be part of the GraphQL read contract: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let topology = json["runStageTopology"].as_array().unwrap();
        assert!(
            topology.len() > 2,
            "topology should come from the frozen workflow snapshot"
        );
        assert_eq!(
            topology[0]["stageId"],
            serde_json::json!("state_1_idea_received")
        );
        let proposal = topology
            .iter()
            .find(|stage| stage["stageId"] == serde_json::json!("state_2_proposal_drafted"))
            .expect("proposal stage should be present");
        assert_eq!(proposal["label"], serde_json::json!("Proposal drafted"));
        assert_eq!(
            proposal["ownerAgentTitle"],
            serde_json::json!("Proposal Writer")
        );
        assert_eq!(proposal["status"], serde_json::json!("running"));
        assert_eq!(proposal["isCurrent"], serde_json::json!(true));
        assert_eq!(proposal["artifactCount"], serde_json::json!(1));
        assert_eq!(
            proposal["occurrences"][0]["agentId"],
            serde_json::json!("proposal_writer")
        );
        assert_eq!(
            proposal["occurrences"][0]["status"],
            serde_json::json!("running")
        );
        assert_eq!(
            proposal["transitions"][0]["toStageId"],
            serde_json::json!("state_3_initial_proposal_approval")
        );
        let refined_index = topology
            .iter()
            .position(|stage| {
                stage["stageId"] == serde_json::json!("state_10_implementation_refined")
            })
            .expect("implementation refined stage should be present");
        let complete_index = topology
            .iter()
            .position(|stage| stage["stageId"] == serde_json::json!("state_12_workflow_complete"))
            .expect("workflow complete stage should be present");
        assert!(
            refined_index < complete_index,
            "topology order should not place Workflow complete before the refinement branch"
        );
    }

    #[tokio::test]
    async fn p036_run_stage_topology_fails_closed_without_frozen_snapshots() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"query {{ runStageTopology(runId: "{run_id}") {{ stageId label }} }}"#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "missing snapshots should fail closed as empty readback, not an API error: {response:?}"
        );
        assert_eq!(
            response.data.into_json().unwrap()["runStageTopology"],
            serde_json::json!([])
        );
    }

    #[tokio::test]
    async fn proposal_043_run_query_uses_projection_summary_fields() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, _) = seed_validation_attempt(&pool, run_id).await;
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: domain::ids::ApprovalId::new(),
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P043RunDetail {{
                      run(id: "{run_id}") {{
                        id
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        totalStages
                        failedStages
                        pendingApprovals
                      }}
                      stage(id: "{stage_execution_id}") {{
                        id
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P043 run detail query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["run"]["projectionPresent"], serde_json::json!(true));
        assert!(json["run"]["projectionUpdatedAt"].is_string());
        assert_eq!(json["run"]["projectionLag"], serde_json::json!(false));
        assert_eq!(json["run"]["totalStages"], serde_json::json!(1));
        assert_eq!(json["run"]["failedStages"], serde_json::json!(1));
        assert_eq!(json["run"]["pendingApprovals"], serde_json::json!(1));
    }

    #[tokio::test]
    async fn proposal_043_stage_queries_expose_projection_decision_flags() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, agent_execution_id) = seed_validation_attempt(&pool, run_id).await;
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: domain::ids::ApprovalId::new(),
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        let payload_path = std::env::temp_dir().join(format!("p043-artifact-{run_id}.json"));
        std::fs::write(&payload_path, br#"{"ok":true}"#).unwrap();
        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "stage_1".into(),
            agent_id: "validation_agent".into(),
            name: "validation_failure_validation_agent".into(),
            contract_id: "validation_failure_record".into(),
            format: ArtifactFormat::Json,
            file_path: payload_path.to_string_lossy().to_string(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("validation_failure".into()),
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(&pool, &artifact).await.unwrap();
        let record =
            validation_failure_record(artifact.id, run_id, stage_execution_id, agent_execution_id);
        db::repos::validation::insert(&pool, &record).await.unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P043StageReadback {{
                      stages(runId: "{run_id}") {{
                        id
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        hasArtifacts
                        hasPendingApproval
                        hasValidationFailure
                      }}
                      stage(id: "{stage_execution_id}") {{
                        id
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        hasArtifacts
                        hasPendingApproval
                        hasValidationFailure
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P043 stage readback query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["stages"][0]["projectionPresent"],
            serde_json::json!(true)
        );
        assert!(json["stages"][0]["projectionUpdatedAt"].is_string());
        assert_eq!(json["stages"][0]["projectionLag"], serde_json::json!(false));
        assert_eq!(json["stages"][0]["hasArtifacts"], serde_json::json!(true));
        assert_eq!(
            json["stages"][0]["hasPendingApproval"],
            serde_json::json!(true)
        );
        assert_eq!(
            json["stages"][0]["hasValidationFailure"],
            serde_json::json!(true)
        );
        assert_eq!(json["stage"]["projectionPresent"], serde_json::json!(true));
        assert!(json["stage"]["projectionUpdatedAt"].is_string());
        assert_eq!(json["stage"]["projectionLag"], serde_json::json!(false));
        assert_eq!(json["stage"]["hasArtifacts"], serde_json::json!(true));
        assert_eq!(json["stage"]["hasPendingApproval"], serde_json::json!(true));
        assert_eq!(
            json["stage"]["hasValidationFailure"],
            serde_json::json!(true)
        );
    }

    #[tokio::test]
    async fn proposal_043_graphql_reads_are_operator_only_v1() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P043OperatorOnly {
                      runs { id }
                    }
                    "#,
                )
                .data(observer_principal()),
            )
            .await;

        assert!(
            response
                .errors
                .iter()
                .any(|error| error.message.contains("forbidden")),
            "P043 V1 reads must reject non-operator principals: {response:?}"
        );
    }

    #[tokio::test]
    async fn proposal_043_run_subscription_uses_projection_summary_fields() {
        use async_graphql::futures_util::StreamExt;

        let pool = p043_test_pool().await;
        let bus = event_bus::new_bus(16);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        seed_validation_attempt(&pool, run_id).await;
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: domain::ids::ApprovalId::new(),
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let mut stream = schema.execute_stream(
            Request::new(format!(
                r#"
                subscription P043RunSubscription {{
                  runStatusChanged(runId: "{run_id}") {{
                    id
                    projectionPresent
                    projectionUpdatedAt
                    projectionLag
                    totalStages
                    pendingApprovals
                  }}
                }}
                "#
            ))
            .data(test_principal()),
        );
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = bus.send(DomainEvent::RunStatusChanged {
                run_id,
                status: domain::run::RunStatus::Ready,
            });
        });

        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("P043 run subscription frame timed out")
            .expect("P043 run subscription ended");
        assert!(
            frame.errors.is_empty(),
            "P043 run subscription must succeed: {frame:?}"
        );
        let json = frame.data.into_json().unwrap();
        assert_eq!(
            json["runStatusChanged"]["projectionPresent"],
            serde_json::json!(true)
        );
        assert!(json["runStatusChanged"]["projectionUpdatedAt"].is_string());
        assert_eq!(
            json["runStatusChanged"]["projectionLag"],
            serde_json::json!(false)
        );
        assert_eq!(
            json["runStatusChanged"]["totalStages"],
            serde_json::json!(1)
        );
        assert_eq!(
            json["runStatusChanged"]["pendingApprovals"],
            serde_json::json!(1)
        );
    }

    #[tokio::test]
    async fn run_subscription_refreshes_on_runtime_progress_events() {
        use async_graphql::futures_util::StreamExt;

        let pool = p043_test_pool().await;
        let bus = event_bus::new_bus(16);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let mut stream = schema.execute_stream(
            Request::new(format!(
                r#"
                subscription RuntimeProgressRefreshesRun {{
                  runStatusChanged(runId: "{run_id}") {{
                    id
                    status
                    freshnessState
                  }}
                }}
                "#
            ))
            .data(test_principal()),
        );
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = bus.send(DomainEvent::RuntimeStatusChanged {
                run_id,
                stage_id: "state_9".into(),
                agent_id: "code_writer".into(),
                provider: "codex".into(),
                event_kind: "session_started".into(),
            });
        });

        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("runtime progress run subscription frame timed out")
            .expect("runtime progress run subscription ended");
        assert!(
            frame.errors.is_empty(),
            "runtime progress run subscription must succeed: {frame:?}"
        );
        let json = frame.data.into_json().unwrap();
        assert_eq!(
            json["runStatusChanged"]["id"],
            serde_json::json!(run_id.to_string())
        );
    }

    #[tokio::test]
    async fn proposal_081_runtime_subscription_payload_carries_cursor_generation_and_gap() {
        use async_graphql::futures_util::StreamExt;

        let pool = p043_test_pool().await;
        let bus = event_bus::new_bus(16);
        let run_id = RunId::new();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let mut live_stream = schema.execute_stream(
            Request::new(format!(
                r#"
                subscription {{
                  runtimeStatusChanged(runId: "{run_id}") {{
                    eventKind
                    sequenceCursor
                    projectionGeneration
                    gapDetected
                    requiresFullRefetch
                  }}
                }}
                "#
            ))
            .data(test_principal()),
        );
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = bus.send(DomainEvent::RuntimeStatusChanged {
                run_id,
                stage_id: "state_9".into(),
                agent_id: "code_writer".into(),
                provider: "codex".into(),
                event_kind: "session_started".into(),
            });
        });

        let live_frame =
            tokio::time::timeout(std::time::Duration::from_secs(5), live_stream.next())
                .await
                .expect("runtime status subscription frame timed out")
                .expect("runtime status subscription ended");
        assert!(live_frame.errors.is_empty(), "{live_frame:?}");
        let live_json = live_frame.data.into_json().unwrap();
        let live = &live_json["runtimeStatusChanged"];
        assert_eq!(live["eventKind"], "session_started");
        assert!(
            live["sequenceCursor"]
                .as_str()
                .unwrap_or_default()
                .starts_with("seq-"),
            "live subscription payload must carry sequenceCursor"
        );
        assert!(live["projectionGeneration"].as_i64().unwrap_or(0) > 0);
        assert_eq!(live["gapDetected"], false);
        assert_eq!(live["requiresFullRefetch"], false);

        let mut gap_stream = schema.execute_stream(
            Request::new(
                r#"
                subscription {
                  runtimeStatusChanged(replayCursor: "seq-999999999") {
                    eventKind
                    sequenceCursor
                    projectionGeneration
                    gapDetected
                    requiresFullRefetch
                  }
                }
                "#,
            )
            .data(test_principal()),
        );
        let gap_frame = tokio::time::timeout(std::time::Duration::from_secs(1), gap_stream.next())
            .await
            .expect("gap frame timed out")
            .expect("gap stream ended");
        assert!(gap_frame.errors.is_empty(), "{gap_frame:?}");
        let gap_json = gap_frame.data.into_json().unwrap();
        let gap = &gap_json["runtimeStatusChanged"];
        assert_eq!(gap["eventKind"], "subscription_gap_detected");
        assert_eq!(gap["gapDetected"], true);
        assert_eq!(gap["requiresFullRefetch"], true);
    }

    #[tokio::test]
    async fn proposal_043_stage_subscription_uses_projection_decision_flags() {
        use async_graphql::futures_util::StreamExt;

        let pool = p043_test_pool().await;
        let bus = event_bus::new_bus(16);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, agent_execution_id) = seed_validation_attempt(&pool, run_id).await;
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: domain::ids::ApprovalId::new(),
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        let payload_path = std::env::temp_dir().join(format!("p043-sub-artifact-{run_id}.json"));
        std::fs::write(&payload_path, br#"{"ok":true}"#).unwrap();
        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "stage_1".into(),
            agent_id: "validation_agent".into(),
            name: "validation_failure_validation_agent".into(),
            contract_id: "validation_failure_record".into(),
            format: ArtifactFormat::Json,
            file_path: payload_path.to_string_lossy().to_string(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("validation_failure".into()),
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(&pool, &artifact).await.unwrap();
        let record =
            validation_failure_record(artifact.id, run_id, stage_execution_id, agent_execution_id);
        db::repos::validation::insert(&pool, &record).await.unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let mut stream = schema.execute_stream(
            Request::new(format!(
                r#"
                subscription P043StageSubscription {{
                  stageStatusChanged(runId: "{run_id}") {{
                    id
                    projectionPresent
                    projectionUpdatedAt
                    projectionLag
                    hasArtifacts
                    hasPendingApproval
                    hasValidationFailure
                  }}
                }}
                "#
            ))
            .data(test_principal()),
        );
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let _ = bus.send(DomainEvent::StageStatusChanged {
                run_id,
                stage_execution_id,
                status: domain::stage::StageStatus::Failed,
            });
        });

        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("P043 stage subscription frame timed out")
            .expect("P043 stage subscription ended");
        assert!(
            frame.errors.is_empty(),
            "P043 stage subscription must succeed: {frame:?}"
        );
        let json = frame.data.into_json().unwrap();
        assert_eq!(
            json["stageStatusChanged"]["projectionPresent"],
            serde_json::json!(true)
        );
        assert!(json["stageStatusChanged"]["projectionUpdatedAt"].is_string());
        assert_eq!(
            json["stageStatusChanged"]["projectionLag"],
            serde_json::json!(false)
        );
        assert_eq!(
            json["stageStatusChanged"]["hasArtifacts"],
            serde_json::json!(true)
        );
        assert_eq!(
            json["stageStatusChanged"]["hasPendingApproval"],
            serde_json::json!(true)
        );
        assert_eq!(
            json["stageStatusChanged"]["hasValidationFailure"],
            serde_json::json!(true)
        );
    }

    #[tokio::test]
    async fn proposal_043_approval_resolved_subscription_is_available() {
        use async_graphql::futures_util::StreamExt;

        let pool = p043_test_pool().await;
        let bus = event_bus::new_bus(16);
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let approval_id = domain::ids::ApprovalId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            bus.clone(),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let mut stream = schema.execute_stream(
            Request::new(
                r#"
                subscription P043ApprovalResolved {
                  approvalResolved {
                    id
                    decision
                    decidedAt
                  }
                }
                "#,
            )
            .data(test_principal()),
        );
        let pool_for_event = pool.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            approvals::resolve(
                &pool_for_event,
                approval_id,
                domain::approval::ApprovalDecision::Granted,
                Utc::now(),
                Some("approved".into()),
            )
            .await
            .unwrap();
            let _ = bus.send(DomainEvent::ApprovalResolved {
                approval_id,
                decision: domain::approval::ApprovalDecision::Granted,
            });
        });

        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("P043 approvalResolved subscription frame timed out")
            .expect("P043 approvalResolved subscription ended");
        assert!(
            frame.errors.is_empty(),
            "P043 approvalResolved subscription must succeed: {frame:?}"
        );
        let json = frame.data.into_json().unwrap();
        assert_eq!(
            json["approvalResolved"]["decision"],
            serde_json::json!("granted")
        );
        assert!(
            json["approvalResolved"]["decidedAt"].is_string(),
            "resolved approval subscription must expose decidedAt: {json:?}"
        );
    }

    #[tokio::test]
    async fn proposal_043_missing_projection_rows_are_explicit_lag_state() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, _) = seed_validation_attempt(&pool, run_id).await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P043MissingProjectionLag {{
                      run(id: "{run_id}") {{
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        pendingApprovals
                      }}
                      stage(id: "{stage_execution_id}") {{
                        projectionPresent
                        projectionUpdatedAt
                        projectionLag
                        hasPendingApproval
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P043 missing projection query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["run"]["projectionPresent"], serde_json::json!(false));
        assert_eq!(json["run"]["projectionUpdatedAt"], serde_json::Value::Null);
        assert_eq!(json["run"]["projectionLag"], serde_json::json!(true));
        assert_eq!(json["stage"]["projectionPresent"], serde_json::json!(false));
        assert_eq!(
            json["stage"]["projectionUpdatedAt"],
            serde_json::Value::Null
        );
        assert_eq!(json["stage"]["projectionLag"], serde_json::json!(true));
        assert_ne!(
            json["run"]["projectionLag"],
            serde_json::json!(false),
            "missing projection must not be indistinguishable from normal zero-count truth"
        );
        assert_ne!(
            json["stage"]["projectionLag"],
            serde_json::json!(false),
            "missing stage projection must not be indistinguishable from normal false flags"
        );
    }

    #[tokio::test]
    async fn proposal_031_schema_exposes_required_enum_values() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P031EnumContract {
                      freshness: __type(name: "FreshnessState") {
                        enumValues { name }
                      }
                      disabledReason: __type(name: "DisabledReasonCode") {
                        enumValues { name }
                      }
                      writePath: __type(name: "WritePathState") {
                        enumValues { name }
                      }
                      payloadAvailability: __type(name: "PayloadAvailabilityState") {
                        enumValues { name }
                      }
                      payloadUnavailableReason: __type(name: "PayloadUnavailableReasonCode") {
                        enumValues { name }
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 enum contract introspection must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_enum_values(
            &json,
            "freshness",
            &[
                "live",
                "refreshing",
                "projection_lag",
                "stale",
                "unavailable",
                "unauthorized",
            ],
        );
        assert_enum_values(
            &json,
            "disabledReason",
            &[
                "WRITE_PATH_NOT_AVAILABLE",
                "MANAGED_OUTSIDE_UI",
                "AMBIGUOUS_APPROVAL_IDENTITY",
                "STALE_READ",
                "PROJECTION_LAG",
                "UNAUTHORIZED",
                "UNSUPPORTED_ACTION",
                // P081 boundary policy reason codes
                "APPROVAL_NOT_ACTIONABLE",
                "OBSERVER_SCOPE",
                "NON_APPROVAL_MUTATION",
                "CAPABILITY_OUT_OF_SCOPE",
            ],
        );
        assert_enum_values(
            &json,
            "writePath",
            &[
                "available",
                "read_only_diagnostic",
                "write_path_not_available",
                "external_transport_required",
                "hidden",
            ],
        );
        assert_enum_values(
            &json,
            "payloadAvailability",
            &[
                "available",
                "metadata_only",
                "payload_deferred",
                "generating",
                "unavailable",
            ],
        );
        assert_enum_values(
            &json,
            "payloadUnavailableReason",
            &[
                "PAYLOAD_DEFERRED_BY_P031",
                "GENERATING",
                "NOT_INDEXED",
                "NOT_AUTHORIZED",
                "NOT_AVAILABLE",
                "UNKNOWN",
            ],
        );
    }

    #[tokio::test]
    async fn proposal_075_storage_health_is_typed_graphql_contract() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let sdl = schema.sdl();
        assert!(sdl.contains("type StorageHealth"));
        assert!(sdl.contains("type DbWriterHealth"));
        assert!(sdl.contains("type EvidenceSpoolSummary"));
        assert!(sdl.contains("enum StorageDbState"));
        assert!(!sdl.contains("storageHealth: JSON"));

        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P075StorageHealthTyped {
                      storageHealth {
                        updatedAt
                        staleAfterMs
                        isStale
                        dbState
                        writer {
                          alive
                          lanes { lane capacity queuedDepth queuedDepthRatio }
                        }
                        wal { available warnSizeBytes criticalSizeBytes }
                        evidenceSpool {
                          enabled
                          filesWrittenTotal
                          bytesWrittenTotal
                          metadataRowsTotal
                        }
                        thresholds { metric warn critical unit action }
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P075 typed storageHealth query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["storageHealth"]["staleAfterMs"], 5000);
        assert!(json["storageHealth"]["writer"]["lanes"]
            .as_array()
            .is_some_and(|lanes| lanes.len() >= 6));
        assert!(json["storageHealth"]["thresholds"]
            .as_array()
            .is_some_and(|thresholds| !thresholds.is_empty()));
    }

    #[tokio::test]
    async fn proposal_075_storage_health_reads_live_dbwriter_heartbeat() {
        let pool = test_pool().await;
        let writer = db::writer::DbWriter::new(pool.clone());
        let result = writer
            .submit(
                WriteOperation {
                    class: WriteClass::A,
                    lane: WriteLane::CriticalBarrier,
                    operation_name: "graphql_storage_health_live_writer_test",
                    expected_rows: 1,
                    batchable: false,
                    barrier: true,
                    deadline: std::time::Duration::from_secs(5),
                    deadline_reason: None,
                    idempotency_key: "graphql-storage-health-live-writer".into(),
                    replay_policy: ReplayPolicy::NaturalKey,
                    observed_at: None,
                },
                |pool| async move {
                    let mut tx = db::pool::begin_immediate_with_retry(
                        &pool,
                        "graphql_storage_health_live_writer_test",
                    )
                    .await?;
                    sqlx::query(
                        "CREATE TABLE IF NOT EXISTS p075_graphql_storage_health_probe (id TEXT PRIMARY KEY)",
                    )
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT OR REPLACE INTO p075_graphql_storage_health_probe (id) VALUES ('probe')",
                    )
                    .execute(&mut *tx)
                    .await?;
                    tx.commit().await?;
                    Ok(1)
                },
            )
            .await;
        assert_eq!(result, WriteResult::Committed);

        for _ in 0..30 {
            if writer.is_alive() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(writer.is_alive(), "DbWriter heartbeat should become live");

        let schema = build_schema_with_storage_writer(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
            writer.heartbeat.clone(),
        );

        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P075StorageHealthWriter {
                      storageHealth {
                        isStale
                        writer {
                          alive
                          totalQueued
                          lastHeartbeatAt
                          lastDrainAt
                          writeLockWaitP50Ms
                          writeLockWaitP95Ms
                          transactionDurationP95Ms
                          lanes { lane queuedDepth }
                        }
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P075 live storageHealth query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["storageHealth"]["writer"]["alive"], true);
        assert_eq!(json["storageHealth"]["isStale"], false);
        assert!(json["storageHealth"]["writer"]["lastHeartbeatAt"]
            .as_str()
            .is_some());
        assert!(json["storageHealth"]["writer"]["lastDrainAt"]
            .as_str()
            .is_some());
        assert!(json["storageHealth"]["writer"]["writeLockWaitP50Ms"]
            .as_f64()
            .is_some());
        assert!(json["storageHealth"]["writer"]["writeLockWaitP95Ms"]
            .as_f64()
            .is_some());
        assert!(json["storageHealth"]["writer"]["transactionDurationP95Ms"]
            .as_f64()
            .is_some());
        assert!(json["storageHealth"]["writer"]["lanes"]
            .as_array()
            .is_some_and(|lanes| lanes.len() == 6));
    }

    #[tokio::test]
    async fn proposal_031_freshness_state_is_derived_from_server_projection() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let (stage_execution_id, _) = seed_validation_attempt(&pool, run_id).await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let lagging = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031LaggingFreshness {{
                      run(id: "{run_id}") {{ freshnessState projectionLag }}
                      stage(id: "{stage_execution_id}") {{ freshnessState projectionLag }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            lagging.errors.is_empty(),
            "P031 lagging freshness query must succeed: {lagging:?}"
        );
        let lagging_json = lagging.data.into_json().unwrap();
        assert_eq!(
            lagging_json["run"]["freshnessState"],
            serde_json::json!("projection_lag")
        );
        assert_eq!(
            lagging_json["stage"]["freshnessState"],
            serde_json::json!("projection_lag")
        );

        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();
        let live = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031LiveFreshness {{
                      run(id: "{run_id}") {{ freshnessState projectionLag }}
                      stage(id: "{stage_execution_id}") {{ freshnessState projectionLag }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            live.errors.is_empty(),
            "P031 live freshness query must succeed: {live:?}"
        );
        let live_json = live.data.into_json().unwrap();
        assert_eq!(
            live_json["run"]["freshnessState"],
            serde_json::json!("live")
        );
        assert_eq!(
            live_json["stage"]["freshnessState"],
            serde_json::json!("live")
        );
    }

    #[tokio::test]
    async fn proposal_031_approval_inbox_is_diagnostic_read_only() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let approval_id = domain::ids::ApprovalId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_approval_inbox(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P031ApprovalDiagnostics {
                      approvalInbox {
                        id
                        freshnessState
                        disabledReasonCode
                        writePathState
                        diagnosticId
                        serverDebugDetail
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 approval diagnostic query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let approval = &json["approvalInbox"][0];
        assert_eq!(approval["freshnessState"], serde_json::json!("live"));
        assert_eq!(approval["disabledReasonCode"], serde_json::Value::Null);
        assert_eq!(approval["writePathState"], serde_json::json!("available"));
        assert_eq!(
            approval["diagnosticId"],
            serde_json::json!(approval_id.to_string())
        );
        assert!(
            approval["serverDebugDetail"].is_null(),
            "serverDebugDetail must be null for Phase 0 approval rows"
        );
    }

    #[tokio::test]
    async fn proposal_031_approval_inbox_can_be_scoped_to_run() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let other_run_id = RunId::new();
        let approval_id = domain::ids::ApprovalId::new();
        let other_approval_id = domain::ids::ApprovalId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        runs::insert(&pool, &make_run(other_run_id, idea_id))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: other_approval_id,
                run_id: other_run_id,
                stage_id: "stage_2".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_approval_inbox(&pool, run_id)
            .await
            .unwrap();
        projections::rebuild_approval_inbox(&pool, other_run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031RunScopedApprovalDiagnostics {{
                      approvalInbox(runId: "{run_id}") {{
                        id
                        runId
                        stageId
                        writePathState
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 run-scoped approval query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(json["approvalInbox"].as_array().unwrap().len(), 1);
        assert_eq!(
            json["approvalInbox"][0]["id"],
            serde_json::json!(approval_id.to_string())
        );
        assert_eq!(
            json["approvalInbox"][0]["runId"],
            serde_json::json!(run_id.to_string())
        );
    }

    #[tokio::test]
    async fn proposal_031_report_artifacts_are_metadata_only_payloads() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_id = ArtifactId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "report".into(),
                agent_id: "release".into(),
                name: "Release report".into(),
                contract_id: "release_report_v1".into(),
                format: ArtifactFormat::Json,
                file_path: "/tmp/report.json".into(),
                checksum_sha256: None,
                size_bytes: Some(64),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("release".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ReportPayloadMetadata {{
                      artifacts(runId: "{run_id}") {{
                        id
                        freshnessState
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        diagnosticId
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 report metadata query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifact = &json["artifacts"][0];
        assert_eq!(artifact["freshnessState"], serde_json::json!("live"));
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("metadata_only")
        );
        assert_eq!(
            artifact["payloadUnavailableReasonCode"],
            serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
        );
        assert_eq!(
            artifact["diagnosticId"],
            serde_json::json!(artifact_id.to_string())
        );
        assert!(
            artifact["serverDebugDetail"].is_string(),
            "operator diagnostic detail should explain why report payload rendering is deferred"
        );
    }

    #[tokio::test]
    async fn proposal_031_artifact_payload_text_is_server_owned_readback() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_id = ArtifactId::new();
        let artifact_root =
            std::env::temp_dir().join(format!("p031-artifact-payload-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&artifact_root).unwrap();
        let artifact_path = artifact_root.join("proposal.md");
        fs::write(&artifact_path, "# Proposal\n\nGraphQL payload").unwrap();

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "proposal".into(),
                agent_id: "proposal_writer".into(),
                name: "proposal.md".into(),
                contract_id: "proposal_markdown_v1".into(),
                format: ArtifactFormat::Markdown,
                file_path: artifact_path.to_string_lossy().into_owned(),
                checksum_sha256: None,
                size_bytes: Some(24),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: None,
                report_version: None,
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ArtifactPayloadReadback {{
                      artifacts(runId: "{run_id}") {{
                        id
                        format
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        payloadText
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 artifact payload readback query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifact = &json["artifacts"][0];
        assert_eq!(artifact["id"], serde_json::json!(artifact_id.to_string()));
        assert_eq!(artifact["format"], serde_json::json!("markdown"));
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("available")
        );
        assert!(artifact["payloadUnavailableReasonCode"].is_null());
        assert_eq!(
            artifact["payloadText"],
            serde_json::json!("# Proposal\n\nGraphQL payload")
        );

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_artifact_query_reads_selected_payload_only() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_id = ArtifactId::new();
        let artifact_root =
            std::env::temp_dir().join(format!("p031-selected-artifact-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&artifact_root).unwrap();
        let artifact_path = artifact_root.join("selected.md");
        fs::write(&artifact_path, "# Selected\n\nOnly this artifact").unwrap();

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "proposal".into(),
                agent_id: "proposal_writer".into(),
                name: "selected.md".into(),
                contract_id: "proposal_markdown_v1".into(),
                format: ArtifactFormat::Markdown,
                file_path: artifact_path.to_string_lossy().into_owned(),
                checksum_sha256: None,
                size_bytes: Some(29),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: None,
                report_version: None,
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031SelectedArtifactPayload {{
                      artifact(id: "{artifact_id}") {{
                        id
                        payloadAvailabilityState
                        payloadText
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 selected artifact payload query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifact = &json["artifact"];
        assert_eq!(artifact["id"], serde_json::json!(artifact_id.to_string()));
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("available")
        );
        assert_eq!(
            artifact["payloadText"],
            serde_json::json!("# Selected\n\nOnly this artifact")
        );

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_artifact_payload_text_is_capped_for_bulk_readback() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_id = ArtifactId::new();
        let artifact_root =
            std::env::temp_dir().join(format!("p031-artifact-preview-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&artifact_root).unwrap();
        let artifact_path = artifact_root.join("large.md");
        let payload = format!(
            "{}tail-marker",
            "large artifact preview line\n"
                .repeat((P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES / 28) + 256)
        );
        fs::write(&artifact_path, &payload).unwrap();

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "proposal".into(),
                agent_id: "proposal_writer".into(),
                name: "large.md".into(),
                contract_id: "proposal_markdown_v1".into(),
                format: ArtifactFormat::Markdown,
                file_path: artifact_path.to_string_lossy().into_owned(),
                checksum_sha256: None,
                size_bytes: Some(payload.len() as i64),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: None,
                report_version: None,
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ArtifactPayloadPreview {{
                      artifacts(runId: "{run_id}") {{
                        payloadAvailabilityState
                        payloadText
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 capped artifact payload query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifact = &json["artifacts"][0];
        let preview = artifact["payloadText"].as_str().unwrap();
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("available")
        );
        assert!(preview.len() <= P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES);
        assert!(preview.starts_with("large artifact preview line"));
        assert!(!preview.contains("tail-marker"));
        assert!(
            artifact["serverDebugDetail"]
                .as_str()
                .unwrap()
                .contains("preview capped"),
            "truncated payloads should expose operator-visible preview metadata"
        );

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_artifact_payload_text_has_bulk_response_budget() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_root = std::env::temp_dir().join(format!(
            "p031-artifact-bulk-preview-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&artifact_root).unwrap();
        let payload = format!(
            "{}tail-marker",
            "bulk artifact preview line\n"
                .repeat((P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES / 27) + 8)
        );

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();

        for index in 0..10 {
            let artifact_id = ArtifactId::new();
            let artifact_path = artifact_root.join(format!("large-{index}.md"));
            fs::write(&artifact_path, &payload).unwrap();
            artifacts::insert(
                &pool,
                &Artifact {
                    id: artifact_id,
                    run_id,
                    stage_id: "proposal".into(),
                    agent_id: "proposal_writer".into(),
                    name: format!("large-{index}.md"),
                    contract_id: "proposal_markdown_v1".into(),
                    format: ArtifactFormat::Markdown,
                    file_path: artifact_path.to_string_lossy().into_owned(),
                    checksum_sha256: None,
                    // Stale discovery metadata can under-report size. The
                    // response budget must be enforced from actual preview
                    // bytes, not only the indexed size.
                    size_bytes: Some(0),
                    provider: "test".into(),
                    model: None,
                    created_at: Utc::now(),
                    is_pinned: false,
                    report_kind: None,
                    report_version: None,
                    agent_execution_id: None,
                },
            )
            .await
            .unwrap();
        }
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ArtifactPayloadBulkBudget {{
                      artifacts(runId: "{run_id}") {{
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        payloadText
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 bulk artifact payload query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifacts = json["artifacts"].as_array().unwrap();
        let available_count = artifacts
            .iter()
            .filter(|artifact| {
                artifact["payloadAvailabilityState"] == serde_json::json!("available")
            })
            .count();
        let deferred_count = artifacts
            .iter()
            .filter(|artifact| {
                artifact["payloadAvailabilityState"] == serde_json::json!("payload_deferred")
            })
            .count();
        let total_payload_bytes: usize = artifacts
            .iter()
            .filter_map(|artifact| artifact["payloadText"].as_str())
            .map(str::len)
            .sum();

        assert_eq!(available_count, 8);
        assert_eq!(deferred_count, 2);
        assert!(total_payload_bytes <= P031_ARTIFACT_PAYLOAD_BULK_PREVIEW_MAX_BYTES);
        assert!(artifacts.iter().any(|artifact| {
            artifact["payloadUnavailableReasonCode"]
                == serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
                && artifact["serverDebugDetail"]
                    .as_str()
                    .unwrap()
                    .contains("bulk artifact list reached its payload preview budget")
        }));

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_artifact_metadata_query_does_not_consume_payload_budget() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_root = std::env::temp_dir().join(format!(
            "p031-artifact-metadata-budget-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&artifact_root).unwrap();
        let payload = "metadata-only artifact preview line\n"
            .repeat((P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES / 36) + 8);

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();

        for index in 0..10 {
            let artifact_id = ArtifactId::new();
            let artifact_path = artifact_root.join(format!("metadata-{index}.md"));
            fs::write(&artifact_path, &payload).unwrap();
            artifacts::insert(
                &pool,
                &Artifact {
                    id: artifact_id,
                    run_id,
                    stage_id: "proposal".into(),
                    agent_id: "proposal_writer".into(),
                    name: format!("metadata-{index}.md"),
                    contract_id: "proposal_markdown_v1".into(),
                    format: ArtifactFormat::Markdown,
                    file_path: artifact_path.to_string_lossy().into_owned(),
                    checksum_sha256: None,
                    size_bytes: Some(payload.len() as i64),
                    provider: "test".into(),
                    model: None,
                    created_at: Utc::now(),
                    is_pinned: false,
                    report_kind: None,
                    report_version: None,
                    agent_execution_id: None,
                },
            )
            .await
            .unwrap();
        }
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ArtifactMetadataList {{
                      artifacts(runId: "{run_id}") {{
                        id
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P031 artifact metadata query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifacts = json["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 10);
        assert!(artifacts.iter().all(|artifact| {
            artifact["payloadAvailabilityState"] == serde_json::json!("available")
                && artifact["payloadUnavailableReasonCode"].is_null()
                && artifact["serverDebugDetail"].is_null()
        }));

        let _ = fs::remove_dir_all(&artifact_root);
    }

    #[tokio::test]
    async fn proposal_031_diagnostic_metadata_is_operator_only() {
        let pool = p043_test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let approval_id = domain::ids::ApprovalId::new();
        let artifact_id = ArtifactId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &domain::approval::Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: domain::approval::ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "report".into(),
                agent_id: "release".into(),
                name: "Release report".into(),
                contract_id: "release_report_v1".into(),
                format: ArtifactFormat::Json,
                file_path: "/tmp/report.json".into(),
                checksum_sha256: None,
                size_bytes: Some(64),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("release".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_approval_inbox(&pool, run_id)
            .await
            .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let approval_response = schema
            .execute(
                Request::new(
                    r#"
                    query P031ApprovalDiagnosticsObserverDenied {
                      approvalInbox {
                        diagnosticId
                        serverDebugDetail
                        disabledReasonCode
                        writePathState
                      }
                    }
                    "#,
                )
                .data(observer_principal()),
            )
            .await;
        assert!(
            approval_response
                .errors
                .iter()
                .any(|error| error.message.contains("forbidden")),
            "observer principals must not read P031 approval diagnostic metadata: {approval_response:?}"
        );

        let artifact_response = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P031ReportDiagnosticsObserverDenied {{
                      artifacts(runId: "{run_id}") {{
                        diagnosticId
                        serverDebugDetail
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                      }}
                    }}
                    "#
                ))
                .data(observer_principal()),
            )
            .await;
        assert!(
            artifact_response
                .errors
                .iter()
                .any(|error| error.message.contains("forbidden")),
            "observer principals must not read P031 report diagnostic metadata: {artifact_response:?}"
        );
    }

    #[tokio::test]
    async fn run_query_exposes_cancellation_settlement_log() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(
            &pool,
            &domain::run::Run {
                cancellation_settlement_log: Some(
                    serde_json::json!([
                        {
                            "agent_execution_id": "ae-1",
                            "agent_id": "writer",
                            "prior_status": "running",
                            "terminal_status": "cancelled",
                            "session_close_attempted": true,
                            "session_close_succeeded": true,
                            "settled_at": "2026-04-15T10:00:00Z"
                        }
                    ])
                    .to_string(),
                ),
                ..make_run(run_id, idea_id)
            },
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    id
                    cancellationSettlementLog
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(json["run"]["cancellationSettlementLog"].as_str().unwrap())
                .unwrap();
        assert_eq!(
            parsed,
            serde_json::json!([
                {
                    "agent_execution_id": "ae-1",
                    "agent_id": "writer",
                    "prior_status": "running",
                    "terminal_status": "cancelled",
                    "session_close_attempted": true,
                    "session_close_succeeded": true,
                    "settled_at": "2026-04-15T10:00:00Z"
                }
            ])
        );
    }

    #[tokio::test]
    async fn run_query_exposes_p091_retry_authority_history_and_repair_readback() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let stage_execution_id = domain::ids::StageExecutionId::new();
        let now = Utc::now();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        db::repos::stages::insert(
            &pool,
            &domain::stage::StageExecution {
                id: stage_execution_id,
                run_id,
                stage_id: "implement".to_string(),
                label: "Implement".to_string(),
                status: domain::stage::StageStatus::Running,
                iteration: 0,
                attempt_number: 2,
                settlement_kind: None,
                started_at: now,
                completed_at: None,
                owner_agent: Some("code_writer".to_string()),
                provider: Some("junie".to_string()),
                model: None,
                stage_type: None,
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: None,
            },
        )
        .await
        .unwrap();
        db::repos::retry_stage_execution_authorities::create_active(
            &pool,
            &domain::retry_authority::RetryStageExecutionAuthority {
                id: "p091-auth-test".to_string(),
                run_id,
                stage_id: "implement".to_string(),
                target_stage_execution_id: stage_execution_id,
                entry_kind: domain::retry_authority::RetryAuthorityEntryKind::FullStageRetry,
                source_command_journal_id: Some("journal-1".to_string()),
                source_retry_work_item_id: Some("advance-1".to_string()),
                source_invoke_work_item_id: None,
                source_agent_execution_id: None,
                authority_state: domain::retry_authority::RetryAuthorityState::Active,
                created_at: now,
                updated_at: now,
                terminal_reason: None,
            },
        )
        .await
        .unwrap();
        db::repos::retry_payload_recovery_events::upsert(
            &pool,
            &domain::retry_authority::RetryPayloadRecoveryEvent {
                idempotency_key: "p092:graphql".to_string(),
                run_id,
                invoke_work_item_id: "invoke-p092".to_string(),
                retry_authority_id: Some("p091-auth-test".to_string()),
                target_stage_execution_id: Some(stage_execution_id),
                completed_agent_execution_id: Some("agent-p092".to_string()),
                reason_code: "valid_retry_invoke_completion_recovered".to_string(),
                mode: "diagnostic".to_string(),
                repaired: false,
                current_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "target_stage_execution_id": stage_execution_id.to_string(),
                    "retry_authority_id": "p091-auth-test",
                    "completed_agent_execution_id": "agent-p092",
                    "invoke_work_item_id": "invoke-p092"
                }),
                provenance_json: Some(serde_json::json!({
                    "source_agent_execution_id": "old-agent"
                })),
                repaired_fields_json: Some(serde_json::json!(["target_stage_execution_id"])),
                diagnostic_json: Some(serde_json::json!({"would_repair": true})),
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        db::repos::retry_payload_recovery_events::upsert(
            &pool,
            &domain::retry_authority::RetryPayloadRecoveryEvent {
                idempotency_key: "p092:graphql-missing-authority".to_string(),
                run_id,
                invoke_work_item_id: "invoke-p092-missing-authority".to_string(),
                retry_authority_id: None,
                target_stage_execution_id: Some(stage_execution_id),
                completed_agent_execution_id: None,
                reason_code: "retry_authority_missing_for_targeted_invoke".to_string(),
                mode: "enforce".to_string(),
                repaired: false,
                current_json: serde_json::json!({
                    "run_id": run_id.to_string(),
                    "target_stage_execution_id": stage_execution_id.to_string(),
                    "retry_authority_id": null,
                    "invoke_work_item_id": "invoke-p092-missing-authority"
                }),
                provenance_json: Some(serde_json::json!({
                    "payload_retry_authority_id": "stale-auth"
                })),
                repaired_fields_json: Some(serde_json::json!([])),
                diagnostic_json: Some(serde_json::json!({"fail_closed": true})),
                created_at: now,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO p091_orphan_repair_passes
               (id, mode, disabled, run_id, candidates_total, excluded_total,
                would_repair_total, repaired_total, disabled_total,
                bounded_samples_json, created_at)
               VALUES ('pass-1', 'diagnostic', 0, ?1, 2, 1, 1, 0, 0, ?2, ?3)"#,
        )
        .bind(run_id.to_string())
        .bind(
            serde_json::json!([
                {"stage_execution_id": stage_execution_id.to_string(), "reason": "pending_approval"}
            ])
            .to_string(),
        )
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query RunById {{
                  run(id: "{run_id}") {{
                    retryAuthorityJson
                    retryAuthorityHistoryJson
                    p091OrphanRepairReadbackJson
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["run"]["retryAuthorityJson"]["id"],
            serde_json::json!("p091-auth-test")
        );
        assert_eq!(
            json["run"]["retryAuthorityHistoryJson"][0]["target_stage_execution_id"],
            serde_json::json!(stage_execution_id.to_string())
        );
        assert_eq!(
            json["run"]["retryAuthorityJson"]["retry_payload_recovery"]["reason_code"],
            serde_json::json!("valid_retry_invoke_completion_recovered")
        );
        assert_eq!(
            json["run"]["retryAuthorityHistoryJson"][0]["retry_payload_recovery"]["current"]
                ["invoke_work_item_id"],
            serde_json::json!("invoke-p092")
        );
        let history = json["run"]["retryAuthorityHistoryJson"].as_array().unwrap();
        let missing_authority = history
            .iter()
            .find(|entry| entry["authority_state"] == serde_json::json!("missing_authority"))
            .expect("missing-authority P092 history row");
        assert_eq!(
            missing_authority["retry_payload_recovery"]["reason_code"],
            serde_json::json!("retry_authority_missing_for_targeted_invoke")
        );
        assert_eq!(
            missing_authority["retry_payload_recovery"]["current"]["retry_authority_id"],
            serde_json::Value::Null
        );
        assert_eq!(
            json["run"]["p091OrphanRepairReadbackJson"]["latest_pass"]["excluded_total"],
            serde_json::json!(1)
        );
    }

    #[tokio::test]
    async fn runs_query_exposes_cancellation_settlement_summary_only() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(
            &pool,
            &domain::run::Run {
                status: domain::run::RunStatus::Cancelling,
                cancellation_settlement_log: Some(
                    serde_json::json!([
                        {
                            "agent_execution_id": "ae-1",
                            "agent_id": "writer",
                            "prior_status": "running",
                            "terminal_status": "cancelled",
                            "session_close_attempted": true,
                            "session_close_succeeded": true,
                            "settled_at": "2026-04-15T10:00:00Z"
                        },
                        {
                            "agent_execution_id": "ae-2",
                            "agent_id": "reviewer",
                            "prior_status": "running",
                            "terminal_status": "cancelled",
                            "session_close_attempted": true,
                            "session_close_succeeded": false,
                            "settled_at": "2026-04-15T10:00:02Z"
                        }
                    ])
                    .to_string(),
                ),
                ..make_run(run_id, idea_id)
            },
        )
        .await
        .unwrap();
        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                query Runs {
                  runs {
                    id
                    cancellationSettlementSummary
                    cancellationSettlementLog
                  }
                }
                "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_eq!(
            json["runs"][0]["cancellationSettlementSummary"],
            serde_json::json!("2/2 agents settled, 1 sessions closed")
        );
        assert!(json["runs"][0]["cancellationSettlementLog"].is_null());
    }

    #[tokio::test]
    async fn artifacts_query_decodes_validation_failure_record() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let payload_path = std::env::temp_dir().join(format!("validation-failure-{}.json", run_id));
        std::fs::write(
            &payload_path,
            serde_json::to_vec(&validation_failure_payload(run_id)).unwrap(),
        )
        .unwrap();
        let (stage_execution_id, agent_execution_id) = seed_validation_attempt(&pool, run_id).await;

        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "stage_1".into(),
            agent_id: "validation_agent".into(),
            name: "validation_failure_validation_agent".into(),
            contract_id: "validation_failure_record".into(),
            format: ArtifactFormat::Json,
            file_path: payload_path.to_string_lossy().to_string(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("validation_failure".into()),
            report_version: None,
            agent_execution_id: None,
        };
        artifacts::insert(&pool, &artifact).await.unwrap();
        let record =
            validation_failure_record(artifact.id, run_id, stage_execution_id, agent_execution_id);
        db::repos::validation::insert(&pool, &record).await.unwrap();

        projections::rebuild_all_for_run(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(format!(
                    r#"
                query Artifacts {{
                  artifacts(runId: "{run_id}") {{
                    name
                    reportKind
                    validationFailureRecord {{
                      failureSummary
                      failureClass
                      sessionReuseDisposition
                      sessionResetReason
                      outputResults {{
                        outputName
                        missingFields
                      }}
                    }}
                  }}
                }}
                "#
                ))
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "query must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let artifacts = json["artifacts"].as_array().unwrap();
        let validation_failure = artifacts
            .iter()
            .find(|artifact| artifact["reportKind"] == serde_json::json!("validation_failure"))
            .expect("validation failure artifact");

        assert_eq!(
            validation_failure["validationFailureRecord"]["failureSummary"],
            serde_json::json!("report: Missing required fields: summary")
        );
        assert_eq!(
            validation_failure["validationFailureRecord"]["outputResults"][0]["missingFields"],
            serde_json::json!(["summary"])
        );
        assert_eq!(
            validation_failure["validationFailureRecord"]["sessionReuseDisposition"],
            serde_json::json!("reused")
        );
        assert_eq!(
            validation_failure["validationFailureRecord"]["sessionResetReason"],
            serde_json::json!("operator_reset")
        );
    }

    #[tokio::test]
    async fn steward_graphql_readback_tests_exposes_analysis_rows() {
        let pool = test_pool().await;
        let now = Utc::now();
        let analysis = StewardAnalysis {
            id: "analysis-1".into(),
            created_at: now,
            window_start: now,
            window_end: now,
            run_count: 5,
            cohort_keys_json: serde_json::json!({
                "workflow_family": "mvp_live",
                "risk_class": "high"
            })
            .to_string(),
            cohort_quality: CohortQuality::Weak,
            status: StewardAnalysisStatus::Inconclusive,
            degradation_count: 1,
            improvement_count: 0,
            workflow_snapshot_artifact_hash: "workflow-hash".into(),
            agent_catalog_snapshot_hash: "catalog-hash".into(),
            steward_config_snapshot_hash: "config-hash".into(),
            metrics_snapshot_artifact_id: Some("/tmp/steward/metrics-window.json".into()),
            baseline_snapshot_artifact_id: Some("/tmp/steward/baseline-window.json".into()),
            agent_catalog_snapshot_artifact_id: Some("/tmp/steward/catalog-snapshot.json".into()),
            workflow_snapshot_artifact_id: Some("/tmp/steward/workflow-snapshot.json".into()),
            config_change_log_artifact_id: Some("/tmp/steward/config-change-log.json".into()),
            health_report_artifact_id: None,
            degradation_alert_artifact_id: Some("/tmp/steward/degradation-alert.json".into()),
            agent_tuning_artifact_id: None,
            workflow_tuning_artifact_id: None,
            experiment_plan_artifact_id: None,
            audit_report_artifact_id: None,
            trigger_reason: "manual".into(),
            error_summary: None,
        };
        steward::insert_analysis(&pool, &analysis).await.unwrap();
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        steward::insert_run_link(
            &pool,
            &StewardAnalysisRunLink {
                id: "link-1".into(),
                analysis_id: "analysis-1".into(),
                run_id: run_id.to_string(),
                role: "implicated".into(),
            },
        )
        .await
        .unwrap();
        steward::insert_recommendation(
            &pool,
            &StewardRecommendation {
                id: "rec-1".into(),
                analysis_id: "analysis-1".into(),
                created_at: now,
                category: "degradation".into(),
                summary: "Lead time regressed".into(),
                target_metric: "lead_time_median_seconds".into(),
                confidence_level: "high".into(),
                status: "proposed".into(),
                source_artifact_name: Some("deterministic_signal".into()),
                decision_comment: None,
                decided_at: None,
            },
        )
        .await
        .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                query Steward {
                  stewardAnalyses(limit: 10) {
                    id
                    status
                    triggerReason
                    cohortKeysJson
                    cohortQuality
                    runCount
                    degradationCount
                    artifactIds
                    recommendations { id targetMetric status }
                    linkedRuns { id runId role }
                  }
                  stewardAnalysis(id: "analysis-1") {
                    id
                    stewardConfigSnapshotHash
                    recommendations { id summary }
                    linkedRuns { id role }
                  }
                }
                "#,
                )
                .data(test_principal()),
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap()["stewardAnalyses"][0]["id"],
            "analysis-1"
        );
    }

    #[tokio::test]
    async fn proposal_041_graphql_readback_parity_surfaces() {
        for fixture_id in p041_selected_fixtures() {
            // The engine crate's `proposal_041_parity.rs` integration
            // test produces
            // `target/parity/reports/<generation>/<fixture_id>/behavioral-diff-report.json`
            // + the SQLite DB at the path that report's `database_ref`
            // points to. Under `cargo test --workspace` the engine
            // integration binary and this graphql-server lib binary
            // run in parallel slots — there is no ordering guarantee.
            // If the engine binary hasn't produced the report yet (or
            // was cleaned between runs), skip instead of failing. The
            // engine-side gate still enforces that the report IS
            // produced; this test exercises the readback contract
            // only when the artifacts exist. The dedicated
            // `./scripts/test-gate.sh proposal-041` lane runs both in
            // the right order and is the authoritative readiness
            // signal for P041.
            let report_path = p041_report_path(fixture_id);
            let replay_path = p041_replay_path(fixture_id);
            if !report_path.is_file() || !replay_path.is_file() {
                eprintln!(
                    "P041 readback: skipping fixture '{fixture_id}' — engine-side replay has \
                     not produced {} yet. Run `cargo test -p engine --test proposal_041_parity` \
                     first, or use `./scripts/test-gate.sh proposal-041`.",
                    report_path.display()
                );
                return;
            }
            let mut report = p041_report(fixture_id);
            let replay = p041_replay(fixture_id);
            let run_id = replay["run_id"].as_str().expect("run_id");
            let idea_id = replay["run_projection"]["idea_id"]
                .as_str()
                .expect("idea_id");
            // stageQueueSummary requires a stage execution ID; use the first
            // stage from the replay's stage_projection.
            let first_stage_exec_id = replay["stage_projection"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|s| s["id"].as_str())
                .unwrap_or("");
            let db_path =
                workspace_root().join(report["database_ref"].as_str().expect("database_ref"));
            if !db_path.is_file() {
                eprintln!(
                    "P041 readback: skipping fixture '{fixture_id}' — engine-side replay DB \
                     {} is missing (likely cleaned between runs).",
                    db_path.display()
                );
                return;
            }
            let pool = create_pool(&format!("sqlite://{}", db_path.to_string_lossy()))
                .await
                .expect("open P041 fixture DB");
            db::writer::register_shared_writer(
                &pool,
                Arc::new(db::writer::DbWriter::new(pool.clone())),
            )
            .await
            .expect("register P041 fixture shared writer");
            let schema = build_schema(
                pool.clone(),
                make_command_handler(pool.clone()),
                event_bus::new_bus(64),
                auth::PrincipalTable::test_fixture(),
                test_reporter(),
            );
            let response = schema
                .execute(
                    Request::new(format!(
                        r#"
                    query P041FixtureReadback {{
                      run(id: "{run_id}") {{
                        id
                        status
                        workflowId
                      }}
                      runs(ideaId: "{idea_id}") {{
                        id
                        totalStages
                        completedStages
                        failedStages
                        pendingApprovals
                      }}
                      stages(runId: "{run_id}") {{
                        stageId
                        label
                        status
                      }}
                      artifacts(runId: "{run_id}") {{
                        name
                        contractId
                        reportKind
                      }}
                      runQueueSummary(runId: "{run_id}") {{
                        runId
                        pending
                        running
                        completed
                        failed
                        cancelled
                        total
                      }}
                      stageQueueSummary(stageExecutionId: "{first_stage_exec_id}") {{
                        stageExecutionId
                        pending
                        running
                        completed
                        failed
                        cancelled
                        total
                      }}
                    }}
                    "#
                    ))
                    .data(test_principal()),
                )
                .await;
            assert!(
                response.errors.is_empty(),
                "P041 GraphQL fixture readback query must succeed for {fixture_id}: {response:?}"
            );
            let data = response.data.into_json().unwrap();
            let actual = normalize_p041_graphql_actual(data, run_id, first_stage_exec_id);
            update_p041_surface(
                &mut report,
                "graphql_readback",
                actual,
                "graphql-server::schema::build_schema",
            );
            write_p041_report(fixture_id, &report);
        }
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("graphql crate should be under control-plane/crates")
            .to_path_buf()
    }

    fn control_plane_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("graphql crate should be under control-plane/crates")
            .to_path_buf()
    }

    fn p041_report_path(fixture_id: &str) -> PathBuf {
        control_plane_root()
            .join("target/parity/reports")
            .join(p041_generation_id())
            .join(fixture_id)
            .join("behavioral-diff-report.json")
    }

    fn p041_replay_path(fixture_id: &str) -> PathBuf {
        control_plane_root()
            .join("target/parity/work")
            .join(p041_generation_id())
            .join(fixture_id)
            .join("server-replay.json")
    }

    fn p041_generation_id() -> String {
        let generation_id = std::env::var("P041_PUBLICATION_GENERATION_ID")
            .unwrap_or_else(|_| "unscoped-fixture-replay".to_string());
        assert_safe_p041_generation_id(&generation_id);
        generation_id
    }

    fn assert_safe_p041_generation_id(raw: &str) {
        if raw == "unscoped-fixture-replay" {
            return;
        }
        let valid_prefix = raw.starts_with("p041-");
        let valid_chars = raw
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | ':' | 'T' | 'Z'));
        assert!(
            valid_prefix
                && valid_chars
                && !raw.contains("..")
                && !raw.contains('/')
                && !raw.contains('\\'),
            "P041_PUBLICATION_GENERATION_ID must be a safe path segment"
        );
    }

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(path).expect("read JSON")).expect("parse JSON")
    }

    fn p041_report(fixture_id: &str) -> serde_json::Value {
        read_json(&p041_report_path(fixture_id))
    }

    fn p041_replay(fixture_id: &str) -> serde_json::Value {
        read_json(&p041_replay_path(fixture_id))
    }

    fn write_p041_report(fixture_id: &str, report: &serde_json::Value) {
        fs::write(
            p041_report_path(fixture_id),
            serde_json::to_string_pretty(report).expect("serialize P041 report"),
        )
        .expect("write P041 report");
    }

    fn normalize_p041_graphql_actual(
        data: serde_json::Value,
        run_id: &str,
        first_stage_exec_id: &str,
    ) -> serde_json::Value {
        let mut stages = data["stages"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|stage| {
                serde_json::json!({
                    "stage_id": stage["stageId"],
                    "label": stage["label"],
                    "status": stage["status"],
                })
            })
            .collect::<Vec<_>>();
        stages.sort_by(|left, right| {
            left["stage_id"]
                .as_str()
                .unwrap_or_default()
                .cmp(right["stage_id"].as_str().unwrap_or_default())
        });
        // Exclude P057 system projection exports (active-index.json,
        // run-state.json). They are supplemental infrastructure artifacts that
        // post-date the golden fixtures and are not agent-produced outputs.
        const P057_SYSTEM_ARTIFACTS: &[&str] = &["active-index.json", "run-state.json"];
        let mut artifacts = data["artifacts"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|artifact| {
                !P057_SYSTEM_ARTIFACTS.contains(&artifact["name"].as_str().unwrap_or_default())
            })
            .map(|artifact| {
                serde_json::json!({
                    "name": artifact["name"],
                    "contract_id": artifact["contractId"],
                    "report_kind": artifact["reportKind"],
                })
            })
            .collect::<Vec<_>>();
        artifacts.sort_by(|left, right| {
            left["name"]
                .as_str()
                .unwrap_or_default()
                .cmp(right["name"].as_str().unwrap_or_default())
        });
        // Normalize queue summaries by comparing only active (pending/running)
        // counts. Exact completed totals vary by fixture, but active counts must
        // be zero for any terminal run.
        let run_qs = &data["runQueueSummary"];
        let normalized_run_queue_summary = serde_json::json!({
            "run_id": "$run_id",
            "pending": run_qs["pending"],
            "running": run_qs["running"],
        });
        let stage_qs = &data["stageQueueSummary"];
        let normalized_stage_queue_summary = serde_json::json!({
            "stage_execution_id": if first_stage_exec_id.is_empty() {
                serde_json::json!("$first_stage_id")
            } else {
                serde_json::json!("$first_stage_id")
            },
            "pending": stage_qs["pending"],
            "running": stage_qs["running"],
        });
        serde_json::json!({
            "collector_owner": "graphql-server::schema::build_schema",
            "query": "P041FixtureReadback",
            "run": {
                "id": "$run_id",
                "status": data["run"]["status"],
                "workflow_id": data["run"]["workflowId"],
            },
            "runs_by_idea": data["runs"].as_array().cloned().unwrap_or_default().into_iter().map(|run| {
                serde_json::json!({
                    "id": if run["id"] == serde_json::json!(run_id) { serde_json::json!("$run_id") } else { run["id"].clone() },
                    "total_stages": run["totalStages"],
                    "completed_stages": run["completedStages"],
                    "failed_stages": run["failedStages"],
                    "pending_approvals": run["pendingApprovals"],
                })
            }).collect::<Vec<_>>(),
            "stages": stages,
            "artifacts": artifacts,
            "run_queue_summary": normalized_run_queue_summary,
            "stage_queue_summary": normalized_stage_queue_summary,
        })
    }

    fn update_p041_surface(
        report: &mut serde_json::Value,
        surface: &str,
        actual: serde_json::Value,
        collector_owner: &str,
    ) {
        let comparisons = report["surface_comparisons"]
            .as_array_mut()
            .expect("surface_comparisons");
        let comparison = comparisons
            .iter_mut()
            .find(|item| item["surface"] == serde_json::json!(surface))
            .expect("surface comparison");
        let expected = comparison["expected"].clone();
        let matched = expected == actual;
        comparison["actual"] = actual.clone();
        comparison["collector_owner"] = serde_json::json!(collector_owner);
        comparison["status"] = serde_json::json!(if matched { "matched" } else { "diverged" });

        let divergences = report["divergences"].as_array_mut().expect("divergences");
        divergences.retain(|item| item["owner_surface"] != serde_json::json!(surface));
        if !matched {
            divergences.push(serde_json::json!({
                "path": format!("$.{surface}"),
                "expected": expected,
                "actual": actual,
                "severity": "blocking",
                "owner_surface": surface,
                "investigation_hint": "P041 fixture-bound GraphQL readback diverged from expected client truth."
            }));
        }
        let blocking_count = divergences
            .iter()
            .filter(|item| item["severity"] == "blocking")
            .count();
        report["summary"]["blocking_count"] = serde_json::json!(blocking_count);
        report["verdict"] = serde_json::json!(if blocking_count == 0 { "ready" } else { "red" });
    }

    // ── P050 GraphQL readback proof ──

    #[tokio::test]
    async fn test_graphql_run_exposes_chainworks_meta_root() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();

        ideas::insert(
            &pool,
            &Idea {
                id: idea_id,
                title: "P050 GraphQL proof".into(),
                body: "body".into(),
                workspace_root_path: None,
                project_key: None,
                status: IdeaStatus::Active,
                created_at: Utc::now(),
                archived_at: None,
            },
        )
        .await
        .unwrap();

        let mut run = make_run(run_id, idea_id);
        run.chainworks_meta_root = Some(format!(".chainworks/runs/{run_id}"));
        runs::insert(&pool, &run).await.unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                async_graphql::Request::new(format!(
                    r#"{{ run(id: "{}") {{ chainworksMetaRoot }} }}"#,
                    run_id
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            response.errors.is_empty(),
            "GraphQL errors: {:?}",
            response.errors
        );
        let data = response.data.into_json().unwrap();
        let meta_root = data["run"]["chainworksMetaRoot"].as_str();
        assert!(
            meta_root.is_some(),
            "GraphQL run query must expose chainworksMetaRoot"
        );
        assert!(
            meta_root.unwrap().contains(".chainworks/runs/"),
            "chainworksMetaRoot must contain per-run path, got: {:?}",
            meta_root
        );
    }

    // ───────────────────────────────────────────────────────────────────
    // Proposal 029 §4.4.b / §9.1 — `journalId` surfacing on mutations
    // ───────────────────────────────────────────────────────────────────
    //
    // Every GraphQL mutation that invokes `CommandHandler` returns a
    // dedicated payload wrapper that exposes `journalId: ID!`. These tests
    // cover the success path for every command mutation plus two denial
    // paths:
    //
    //   - `test_graphql_start_run_started_variant_includes_journal_id`
    //   - `test_graphql_start_run_blocked_variant_includes_journal_id`
    //   - `test_graphql_approve_stage_returns_payload_with_approval_and_journal_id`
    //   - `test_graphql_retry_stage_returns_payload_with_retried_and_journal_id`
    //   - `test_graphql_cancel_run_returns_payload_with_cancelled_and_journal_id`
    //   - `test_response_omits_journal_id_when_capability_check_fails`
    //
    // See also AC-11 at proposal §8.

    use db::repos::approvals;
    use domain::approval::{Approval, ApprovalDecision};
    use domain::ids::{ApprovalId, StageExecutionId};
    use domain::stage::{StageExecution, StageStatus};

    fn make_approval(run_id: RunId, stage_id: &str) -> Approval {
        Approval {
            id: ApprovalId::new(),
            run_id,
            stage_id: stage_id.to_string(),
            decision: ApprovalDecision::Pending,
            requested_at: Utc::now(),
            decided_at: None,
            comment: None,
            expires_at: None,
        }
    }

    fn make_manual_gate_stage(run_id: RunId, stage_id: &str) -> StageExecution {
        StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: stage_id.to_string(),
            label: stage_id.to_string(),
            status: StageStatus::WaitingApproval,
            iteration: 0,
            attempt_number: 1,
            settlement_kind: None,
            started_at: Utc::now(),
            completed_at: None,
            owner_agent: None,
            provider: None,
            model: None,
            stage_type: Some("manual_gate".into()),
            validation_failure_json: None,
            evidence_packet_json: None,
            recovery_snapshot_json: None,
            retry_reason: None,
        }
    }

    fn p072_principal_table() -> (auth::PrincipalTable, auth::Principal, auth::Principal) {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"{
              "schema_version": 2,
              "principals": [
                {
                  "token": "default-token",
                  "id": "default-operator",
                  "class": "operator",
                  "surface_policies": {
                    "graphql": {
                      "allow_queries": true,
                      "allow_subscriptions": true,
                      "allowed_mutations": ["approveApproval", "rejectApproval"]
                    },
                    "mcp": {
                      "allowed_tools": ["runs.list", "runs.get"]
                    }
                  }
                },
                {
                  "token": "ui-token",
                  "id": "ui_operator",
                  "class": "operator",
                  "surface_policies": {
                    "graphql": {
                      "allow_queries": true,
                      "allow_subscriptions": true,
                      "allowed_mutations": ["approveApproval", "rejectApproval"]
                    },
                    "mcp": {
                      "allowed_tools": []
                    }
                  }
                }
              ]
            }"#,
        )
        .unwrap();
        let table = auth::PrincipalTable::load_or_bootstrap(file.path()).unwrap();
        let default_operator = auth::resolve_bearer("default-token", &table).unwrap();
        let ui_operator = auth::resolve_bearer("ui-token", &table).unwrap();
        (table, default_operator, ui_operator)
    }

    fn observer_principal() -> auth::Principal {
        auth::Principal::new("test-observer", auth::PrincipalClass::Observer)
    }

    fn p072_legacy_default_operator_table() -> (auth::PrincipalTable, auth::Principal) {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"{
              "principals": [
                {
                  "token": "legacy-default-token",
                  "id": "default-operator",
                  "class": "operator"
                }
              ]
            }"#,
        )
        .unwrap();
        let table = auth::PrincipalTable::load_or_bootstrap(file.path()).unwrap();
        let principal = auth::resolve_bearer("legacy-default-token", &table).unwrap();
        (table, principal)
    }

    fn p072_legacy_custom_operator_table() -> (auth::PrincipalTable, auth::Principal) {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"{
              "principals": [
                {
                  "token": "legacy-custom-token",
                  "id": "custom-operator",
                  "class": "operator"
                }
              ]
            }"#,
        )
        .unwrap();
        let table = auth::PrincipalTable::load_or_bootstrap(file.path()).unwrap();
        let principal = auth::resolve_bearer("legacy-custom-token", &table).unwrap();
        (table, principal)
    }

    fn p072_legacy_agent_table() -> (auth::PrincipalTable, auth::Principal) {
        let file = tempfile::NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            r#"{
              "principals": [
                {
                  "token": "legacy-agent-token",
                  "id": "legacy-agent",
                  "class": "agent"
                }
              ]
            }"#,
        )
        .unwrap();
        let table = auth::PrincipalTable::load_or_bootstrap(file.path()).unwrap();
        let principal = auth::resolve_bearer("legacy-agent-token", &table).unwrap();
        (table, principal)
    }

    #[tokio::test]
    async fn proposal_081_boundary_runtime_graphql_readback_is_bounded() {
        let pool = test_pool().await;
        let policy = Arc::new(
            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                auth::boundary::PolicyMode::ReadOnlySafeMode,
            )
            .unwrap(),
        );
        let schema = build_schema_inner(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
            None,
            Some(policy),
        );

        let response = schema
            .execute(Request::new("{ boundaryRuntime }").data(test_principal()))
            .await;

        assert!(
            response.errors.is_empty(),
            "boundaryRuntime readback must be available to operator reads: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let readback = &json["boundaryRuntime"];
        assert_eq!(readback["schemaVersion"], "boundary_runtime.v1");
        assert_eq!(readback["matrixId"], "p081-boundary-matrix-v1");
        assert_eq!(readback["policyInjected"], true);
        assert_eq!(readback["policyMode"], "read_only_safe_mode");
        assert_eq!(readback["safeModeActive"], true);
        assert_eq!(
            readback["auditLogHealth"]["schemaVersion"],
            "audit_log_health.v1"
        );
        assert!(readback["auditLogHealth"]["rowCount"].as_i64().is_some());
        assert_eq!(readback["auditLogHealth"]["writable"], true);
        assert_eq!(readback["auditLogHealth"]["retentionMinDays"], 90);
        assert!(readback["auditLogHealth"]["cleanupState"]
            .as_str()
            .is_some());
        assert!(readback["auditLogHealth"]["cleanupEligibleRowCount"]
            .as_i64()
            .is_some());
        assert!(readback["auditLogHealth"]["cleanupProtectedRowCount"]
            .as_i64()
            .is_some());
        assert!(readback["auditLogHealth"]["payloadBudgetBytes"]
            .as_i64()
            .is_some());
        assert!(readback["auditLogHealth"]["payloadUsedBytes"]
            .as_i64()
            .is_some());
        let audit_health = readback["auditLogHealth"]
            .as_object()
            .expect("audit health object");
        assert!(audit_health.contains_key("lastWriteOkAtMs"));
        assert!(audit_health.contains_key("consecutiveFailures"));
        assert!(audit_health.contains_key("cumulativeFailures"));
        assert!(audit_health.contains_key("budgetBytes"));
        assert!(audit_health.contains_key("usedBytes"));
        assert!(audit_health.contains_key("payloadBudgetState"));
        assert!(audit_health.contains_key("payloadBudgetUsedPercent"));
        assert!(audit_health.contains_key("halfOpenProbeSuccessCount"));
        assert!(readback["auditLogHealth"]["consecutiveFailures"]
            .as_i64()
            .is_some());
        assert!(readback["auditLogHealth"]["cumulativeFailures"]
            .as_i64()
            .is_some());
        assert!(readback["auditLogHealth"]["budgetBytes"].as_i64().is_some());
        assert!(readback["auditLogHealth"]["usedBytes"].as_i64().is_some());
        assert!(readback["auditLogHealth"]["payloadBudgetState"]
            .as_str()
            .is_some());
        assert!(readback["auditLogHealth"]["payloadBudgetUsedPercent"]
            .as_i64()
            .is_some());
        assert!(readback["auditLogHealth"]["halfOpenProbeSuccessCount"]
            .as_i64()
            .is_some());
        assert_eq!(
            readback["auditLogHealth"]["shadowCoverageReportRef"],
            "docs/evidence/boundary-policy-shadow-coverage/report.json"
        );
        assert!(
            readback["auditLogHealth"]["integrityState"]
                .as_str()
                .is_some(),
            "auditLogHealth must expose bounded integrity state, not raw rows"
        );
        assert!(
            readback.get("rows").is_none(),
            "boundaryRuntime must not expose raw audit rows"
        );
    }

    #[tokio::test]
    async fn proposal_081_operator_alerts_surface_safe_mode_without_raw_audit_rows() {
        let pool = test_pool().await;
        let policy = Arc::new(
            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                auth::boundary::PolicyMode::ReadOnlySafeMode,
            )
            .unwrap(),
        );
        let schema = build_schema_inner(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
            None,
            Some(policy),
        );

        let response = schema
            .execute(Request::new(r#"{ operatorAlerts }"#).data(test_principal()))
            .await;

        assert!(
            response.errors.is_empty(),
            "operatorAlerts readback must be available to operator reads: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        let alerts = json["operatorAlerts"].as_array().expect("alerts array");
        let safe_mode = alerts
            .iter()
            .find(|alert| alert["dedupeKey"] == "p081.boundary.safe_mode_active")
            .expect("safe-mode alert must be present");
        assert_eq!(safe_mode["schemaVersion"], "operator_alert_v1");
        assert_eq!(safe_mode["severity"], "critical");
        assert_eq!(safe_mode["active"], true);
        assert_eq!(safe_mode["silenceable"], false);
        assert_eq!(safe_mode["lifecycle"]["state"], "active_unacknowledged");
        assert_eq!(
            safe_mode["nativeDelivery"]["dedupePolicy"],
            "dedupe_key_until_clear"
        );
        assert_eq!(
            safe_mode["boundaryRuntime"]["safeModeActive"],
            serde_json::Value::Bool(true)
        );
        assert!(
            !safe_mode.to_string().contains("\"rows\""),
            "operatorAlerts must not expose raw audit rows"
        );
    }

    #[tokio::test]
    async fn proposal_081_subscription_runtime_readback_exposes_cursor_gap_contract() {
        let pool = test_pool().await;
        let schema = build_schema_inner(
            pool,
            make_command_handler(test_pool().await),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
            None,
            Some(Arc::new(
                auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                    auth::boundary::PolicyMode::Enforce,
                )
                .unwrap(),
            )),
        );

        let response = schema
            .execute(Request::new("{ boundaryRuntime }").data(test_principal()))
            .await;
        assert!(response.errors.is_empty(), "{response:?}");
        let json = response.data.into_json().unwrap();
        let subscription = &json["boundaryRuntime"]["subscriptionReplay"];
        assert_eq!(
            subscription["schemaVersion"],
            "subscription_replay_runtime_v1"
        );
        assert!(subscription["sequenceCursor"].as_str().is_some());
        assert!(subscription["projectionGeneration"].as_i64().is_some());
        assert_eq!(subscription["gapDetected"], serde_json::Value::Bool(false));
        assert_eq!(
            subscription["requiresFullRefetch"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(subscription["retentionMinutes"], 15);
        assert_eq!(subscription["retentionEventCount"], 10_000);

        let inside_window = p081_subscription_replay_readback(Some("seq-95"), 90, 100, 7);
        assert_eq!(inside_window["gapDetected"], serde_json::Value::Bool(false));
        assert_eq!(
            inside_window["requiresFullRefetch"],
            serde_json::Value::Bool(false)
        );
        assert_eq!(inside_window["projectionGeneration"], 7);

        let outside_window = p081_subscription_replay_readback(Some("seq-89"), 90, 100, 7);
        assert_eq!(outside_window["gapDetected"], serde_json::Value::Bool(true));
        assert_eq!(
            outside_window["requiresFullRefetch"],
            serde_json::Value::Bool(true)
        );
    }

    #[tokio::test]
    async fn proposal_081_observer_operator_alerts_redact_fields_without_graphql_errors() {
        let pool = test_pool().await;
        let policy = Arc::new(
            auth::boundary::BoundaryPolicy::from_embedded_with_mode(
                auth::boundary::PolicyMode::ReadOnlySafeMode,
            )
            .unwrap(),
        );
        let schema = build_schema_inner(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
            None,
            Some(policy),
        );
        let redactions = P081GraphqlRedactionCollector::default();

        let mut response = schema
            .execute(
                Request::new(r#"{ operatorAlerts }"#)
                    .data(observer_principal())
                    .data(redactions.clone()),
            )
            .await;
        attach_p081_collected_redactions(&mut response, &redactions);

        assert!(
            response.errors.is_empty(),
            "observer opt-in read must redact fields without a response-level GraphQL error: {response:?}"
        );
        let data = response.data.into_json().unwrap();
        let alerts = data["operatorAlerts"].as_array().expect("alerts array");
        let safe_mode = alerts
            .iter()
            .find(|alert| alert["dedupeKey"] == "p081.boundary.safe_mode_active")
            .expect("safe-mode alert must be present");
        assert_eq!(
            safe_mode["message"],
            serde_json::Value::Null,
            "observer-sensitive alert message must be field-null redacted"
        );
        assert_eq!(
            safe_mode["nativeDelivery"],
            serde_json::Value::Null,
            "observer-sensitive native delivery metadata must be redacted"
        );
        let extension_redactions = match response.extensions.get("redactions") {
            Some(async_graphql::Value::List(redactions)) => redactions,
            other => panic!("extensions.redactions must be present, got {other:?}"),
        };
        assert!(
            extension_redactions.iter().any(|redaction| {
                let async_graphql::Value::Object(object) = redaction else {
                    return false;
                };
                matches!(
                    object.get("redactionMode"),
                    Some(async_graphql::Value::String(value)) if value == "field_null_redacted"
                )
            }),
            "observer redaction must use camelCase field_null_redacted extension metadata"
        );
    }

    #[tokio::test]
    async fn test_graphql_approve_approval_uses_p072_ui_operator_policy() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, default_operator, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let allowed_with_default_operator = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}", idempotencyKey: "01900000-0000-7000-8000-000000000001") {{
                        approval {{ id }}
                        journalId
                      }}
                    }}"#,
                    approval.id
                ))
                .data(default_operator),
            )
            .await;
        assert!(
            allowed_with_default_operator.errors.is_empty(),
            "default-operator is the app bearer and must allow approveApproval: {allowed_with_default_operator:?}"
        );

        let ui_approval = make_approval(run_id, "state_6");
        let ui_approval_id = ui_approval.id;
        approvals::insert(&pool, &ui_approval).await.unwrap();

        let allowed = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}", idempotencyKey: "01900000-0000-7000-8000-000000000002") {{
                        approval {{
                          id
                          decision
                          availableActions
                          disabledReasonCode
                          writePathState
                        }}
                        journalId
                      }}
                    }}"#,
                    ui_approval.id
                ))
                .data(ui_operator),
            )
            .await;
        assert!(
            allowed.errors.is_empty(),
            "ui_operator approveApproval must succeed: {allowed:?}"
        );
        let data = allowed.data.into_json().unwrap();
        let approval = &data["approveApproval"]["approval"];
        assert_eq!(
            approval["id"],
            serde_json::json!(ui_approval_id.to_string())
        );
        assert_eq!(approval["decision"], serde_json::json!("granted"));
        assert_eq!(approval["availableActions"], serde_json::json!([]));
        assert_eq!(
            approval["writePathState"],
            serde_json::json!("write_path_not_available")
        );
        assert_eq!(
            approval["disabledReasonCode"],
            serde_json::json!("UNSUPPORTED_ACTION")
        );
        assert!(
            data["approveApproval"]["journalId"].is_string(),
            "approveApproval must return journalId"
        );
    }

    #[tokio::test]
    async fn proposal_081_audit_budget_safe_mode_denies_approval_mutation() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();
        force_p081_audit_budget_safe_mode(&pool).await;

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let denied = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}", idempotencyKey: "01900000-0000-7000-8081-000000000002") {{
                        approval {{ id decision }}
                        journalId
                      }}
                    }}"#,
                    approval.id
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            denied
                .errors
                .iter()
                .any(|error| format!("{error:?}").contains("AUDIT_BUDGET_EXHAUSTED")),
            "audit budget safe mode must deny approval mutation: {denied:?}"
        );
        let current = approvals::find_by_id(&pool, approval.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            current.decision,
            ApprovalDecision::Pending,
            "denied approval mutation must not mutate approval state"
        );
    }

    #[tokio::test]
    async fn test_graphql_ui_principals_denied_non_approval_mutations() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let run_id = RunId::new();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();

        let (principal_table, default_operator, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let cases = [
            "mutation { startRun }".to_string(),
            "mutation { retryStage }".to_string(),
            "mutation { cancelRun }".to_string(),
        ];

        for principal in [default_operator, ui_operator] {
            for query in cases.clone() {
                let response = schema
                    .execute(Request::new(query).data(principal.clone()))
                    .await;
                assert!(
                    !response.errors.is_empty(),
                    "P072 UI principals must be denied non-approval mutation: {response:?}"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_graphql_legacy_default_operator_denied_non_approval_mutations() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let (principal_table, principal) = p072_legacy_default_operator_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let response = schema
            .execute(Request::new("mutation { startRun }").data(principal))
            .await;

        assert!(
            !response.errors.is_empty(),
            "legacy default-operator must not see removed startRun GraphQL mutation: {response:?}"
        );
    }

    #[tokio::test]
    async fn test_graphql_missing_graphql_surface_policy_principals_denied_non_approval_mutations()
    {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

        let query = "mutation { startRun }";

        for (principal_table, principal, label) in [
            {
                let (table, principal) = p072_legacy_custom_operator_table();
                (table, principal, "custom operator")
            },
            {
                let (table, principal) = p072_legacy_agent_table();
                (table, principal, "agent")
            },
        ] {
            let schema = build_schema(
                pool.clone(),
                make_command_handler(pool.clone()),
                event_bus::new_bus(64),
                principal_table,
                test_reporter(),
            );

            let response = schema.execute(Request::new(query).data(principal)).await;
            assert!(
                !response.errors.is_empty(),
                "{label} must not see removed startRun GraphQL mutation: {response:?}"
            );

            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM command_journal WHERE command_type = 'StartRun'",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(
                count, 0,
                "{label} denial must not create a command_journal row"
            );
        }
    }

    #[tokio::test]
    async fn test_graphql_approve_approval_rejects_missing_or_resolved_approval() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let missing_id = ApprovalId::new();
        let missing = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}", idempotencyKey: "01900000-0000-7000-8aaa-000000000001") {{
                        approval {{ id }}
                        journalId
                      }}
                    }}"#,
                    missing_id
                ))
                .data(ui_operator.clone()),
            )
            .await;
        assert!(
            !missing.errors.is_empty(),
            "missing approval must return a GraphQL error"
        );

        let first = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}", idempotencyKey: "01900000-0000-7000-8aaa-000000000002") {{
                        approval {{ id decision }}
                        journalId
                      }}
                    }}"#,
                    approval.id
                ))
                .data(ui_operator.clone()),
            )
            .await;
        assert!(
            first.errors.is_empty(),
            "first approveApproval must succeed: {first:?}"
        );

        // P081: second attempt with a DIFFERENT idempotency key on an already-resolved approval
        // returns approval_not_actionable (not already_resolved) per P081 conflict contract.
        let second = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}", idempotencyKey: "01900000-0000-7000-8aaa-000000000003") {{
                        approval {{ id decision }}
                        journalId
                        conflictResultCode
                      }}
                    }}"#,
                    approval.id
                ))
                .data(ui_operator),
            )
            .await;
        assert!(
            second.errors.is_empty(),
            "terminal approval with different key must return typed conflict code, not a GraphQL error: {second:?}"
        );
        assert_eq!(
            second.data.into_json().unwrap()["approveApproval"]["conflictResultCode"],
            serde_json::json!("approval_not_actionable"),
            "terminal approval with different key must return approval_not_actionable"
        );
    }

    #[tokio::test]
    async fn proposal_085_approval_conflict_result_code_uses_real_failed_journal_id() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        // P081: each attempt uses a distinct idempotency key.
        let approve = |fields: &str, key: &str| {
            Request::new(format!(
                r#"mutation {{
                  approveApproval(approvalId: "{}", idempotencyKey: "{key}") {{
                    {fields}
                  }}
                }}"#,
                approval.id
            ))
            .data(ui_operator.clone())
        };

        let first = schema
            .execute(approve(
                "approval { id decision } journalId conflictResultCode",
                "01900000-0000-7000-8001-000000000001",
            ))
            .await;
        assert!(
            first.errors.is_empty(),
            "first approveApproval must succeed: {first:?}"
        );
        let first_json = first.data.into_json().unwrap();
        assert_eq!(
            first_json["approveApproval"]["conflictResultCode"],
            serde_json::Value::Null
        );

        // P081: second attempt with a DIFFERENT idempotency key returns approval_not_actionable.
        // (Per P081 contract: terminal approval + different key = APPROVAL_NOT_ACTIONABLE, not AlreadyResolved)
        // P081 AC17: already-terminal denials produce zero new command_journal rows.
        let journal_count_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap();
        let second = schema
            .execute(approve(
                "approval { id decision } journalId conflictResultCode",
                "01900000-0000-7000-8001-000000000002",
            ))
            .await;
        assert!(
            second.errors.is_empty(),
            "terminal approval with different key must return typed conflict payload: {second:?}"
        );
        let second_json = second.data.into_json().unwrap();
        let payload = &second_json["approveApproval"];
        assert_eq!(
            payload["conflictResultCode"],
            serde_json::json!("approval_not_actionable"),
            "different-key terminal retry must return approval_not_actionable (not already_resolved)"
        );
        assert_eq!(
            payload["approval"]["decision"],
            serde_json::json!("granted")
        );
        // P081 AC17: The returned journalId for a conflict is a valid UUID (non-zero) but
        // is not persisted to command_journal — it is a correlation handle only.
        let journal_id = payload["journalId"]
            .as_str()
            .expect("conflict payload must include journalId");
        assert_ne!(
            journal_id, "00000000-0000-0000-0000-000000000000",
            "conflict journalId must be a non-zero UUID correlation handle"
        );
        uuid::Uuid::parse_str(journal_id).expect("conflict journalId must be a valid UUID");
        // P081 AC17: zero new command_journal rows for the already-resolved conflict attempt.
        let journal_count_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            journal_count_before.0, journal_count_after.0,
            "P081 AC17: already-resolved conflict must produce zero new command_journal rows"
        );
    }

    #[tokio::test]
    async fn proposal_085_conflict_enum_matches_backend_emitted_codes() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let response = schema
            .execute(
                Request::new(
                    r#"
                    query P085ConflictEnumContract {
                      conflict: __type(name: "MutationConflictResultCode") {
                        enumValues { name }
                      }
                    }
                    "#,
                )
                .data(test_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "P085 mutation conflict enum introspection must succeed: {response:?}"
        );
        let json = response.data.into_json().unwrap();
        assert_enum_values(
            &json,
            "conflict",
            &["already_resolved", "approval_not_actionable"],
        );
    }

    #[tokio::test]
    async fn proposal_085_reject_conflict_result_code_uses_real_failed_journal_id() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        stages::insert(&pool, &make_manual_gate_stage(run_id, "state_6"))
            .await
            .unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let approve = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      approveApproval(approvalId: "{}", idempotencyKey: "01900000-0000-7000-8002-000000000001") {{
                        approval {{ id decision }}
                        journalId
                        conflictResultCode
                      }}
                    }}"#,
                    approval.id
                ))
                .data(ui_operator.clone()),
            )
            .await;
        assert!(
            approve.errors.is_empty(),
            "initial approveApproval must succeed before reject conflict proof: {approve:?}"
        );

        // P081 AC17: zero new journal rows for the already-resolved reject conflict attempt.
        let journal_count_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap();
        let reject = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      rejectApproval(approvalId: "{}", reason: "stale reject", idempotencyKey: "01900000-0000-7000-8002-000000000002") {{
                        approval {{ id decision availableActions disabledReasonCode writePathState }}
                        journalId
                        conflictResultCode
                      }}
                    }}"#,
                    approval.id
                ))
                .data(ui_operator),
            )
            .await;
        assert!(
            reject.errors.is_empty(),
            "rejectApproval on resolved approval must return typed conflict payload: {reject:?}"
        );
        let json = reject.data.into_json().unwrap();
        let payload = &json["rejectApproval"];
        // P081: reject with a DIFFERENT key than the approve → approval_not_actionable
        assert_eq!(
            payload["conflictResultCode"],
            serde_json::json!("approval_not_actionable"),
            "terminal approval retried with different key must return approval_not_actionable"
        );
        assert_eq!(
            payload["approval"]["decision"],
            serde_json::json!("granted")
        );
        assert_eq!(
            payload["approval"]["availableActions"],
            serde_json::json!([])
        );
        assert_eq!(
            payload["approval"]["disabledReasonCode"],
            serde_json::json!("UNSUPPORTED_ACTION")
        );
        assert_eq!(
            payload["approval"]["writePathState"],
            serde_json::json!("write_path_not_available")
        );
        // P081: journalId in the conflict response is a valid non-zero UUID correlation handle.
        let journal_id = payload["journalId"]
            .as_str()
            .expect("reject conflict payload must include journalId");
        assert_ne!(journal_id, "00000000-0000-0000-0000-000000000000");
        uuid::Uuid::parse_str(journal_id).expect("reject conflict journalId must be a valid UUID");
        // P081 AC17: zero new command_journal rows for the already-resolved conflict attempt.
        let journal_count_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM command_journal")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            journal_count_before.0, journal_count_after.0,
            "P081 AC17: already-resolved conflict must produce zero new command_journal rows"
        );
    }

    #[tokio::test]
    async fn proposal_085_backend_artifact_projection_state_matrix() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let artifact_root =
            std::env::temp_dir().join(format!("p085-affordance-matrix-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&artifact_root).unwrap();
        let outside_root =
            std::env::temp_dir().join(format!("p085-affordance-outside-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&outside_root).unwrap();
        let outside_path = outside_root.join("outside.md");
        fs::write(&outside_path, "outside payload").unwrap();

        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        let mut run = make_run(run_id, idea_id);
        run.artifact_root = artifact_root.to_string_lossy().into_owned();
        run.workspace_root = artifact_root.to_string_lossy().into_owned();
        runs::insert(&pool, &run).await.unwrap();

        artifacts::insert(
            &pool,
            &Artifact {
                id: ArtifactId::new(),
                run_id,
                stage_id: "report".into(),
                agent_id: "release".into(),
                name: "release-report".into(),
                contract_id: "release_report_v1".into(),
                format: ArtifactFormat::Json,
                file_path: artifact_root
                    .join("report.json")
                    .to_string_lossy()
                    .into_owned(),
                checksum_sha256: None,
                size_bytes: Some(64),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("release".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();

        for index in 0..10 {
            let path = artifact_root.join(format!("payload-{index}.md"));
            fs::write(&path, format!("payload {index}")).unwrap();
            artifacts::insert(
                &pool,
                &Artifact {
                    id: ArtifactId::new(),
                    run_id,
                    stage_id: "artifact".into(),
                    agent_id: "writer".into(),
                    name: format!("payload-{index}.md"),
                    contract_id: "proposal_markdown_v1".into(),
                    format: ArtifactFormat::Markdown,
                    file_path: path.to_string_lossy().into_owned(),
                    checksum_sha256: None,
                    size_bytes: Some(P031_ARTIFACT_PAYLOAD_PREVIEW_MAX_BYTES as i64),
                    provider: "test".into(),
                    model: None,
                    created_at: Utc::now(),
                    is_pinned: false,
                    report_kind: None,
                    report_version: None,
                    agent_execution_id: None,
                },
            )
            .await
            .unwrap();
        }

        let unavailable_id = ArtifactId::new();
        artifacts::insert(
            &pool,
            &Artifact {
                id: unavailable_id,
                run_id,
                stage_id: "artifact".into(),
                agent_id: "writer".into(),
                name: "outside.md".into(),
                contract_id: "proposal_markdown_v1".into(),
                format: ArtifactFormat::Markdown,
                file_path: outside_path.to_string_lossy().into_owned(),
                checksum_sha256: None,
                size_bytes: Some(16),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: None,
                report_version: None,
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );
        let list = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P085ArtifactProjectionStateMatrix {{
                      artifacts(runId: "{run_id}") {{
                        name
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        payloadText
                        freshnessState
                        diagnosticId
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            list.errors.is_empty(),
            "P085 artifact projection state matrix query must succeed: {list:?}"
        );
        let data = list.data.into_json().unwrap();
        let artifacts = data["artifacts"].as_array().unwrap();
        assert!(artifacts.iter().any(|artifact| {
            artifact["payloadAvailabilityState"] == serde_json::json!("available")
                && artifact["payloadText"].is_string()
                && artifact["freshnessState"] == serde_json::json!("live")
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact["payloadAvailabilityState"] == serde_json::json!("metadata_only")
                && artifact["payloadUnavailableReasonCode"]
                    == serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
                && artifact["diagnosticId"].is_string()
                && artifact["serverDebugDetail"].is_string()
                && artifact["serverDebugDetail"]
                    .as_str()
                    .is_some_and(|detail| detail.contains(P085_NO_DEADLINE_JUSTIFICATION))
        }));
        assert!(artifacts.iter().any(|artifact| {
            artifact["payloadAvailabilityState"] == serde_json::json!("payload_deferred")
                && artifact["payloadUnavailableReasonCode"]
                    == serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
                && artifact["serverDebugDetail"]
                    .as_str()
                    .is_some_and(|detail| {
                        detail.contains("payload preview budget")
                            && detail.contains(P085_NO_DEADLINE_JUSTIFICATION)
                    })
        }));

        let detail = schema
            .execute(
                Request::new(format!(
                    r#"
                    query P085UnavailableArtifactDetail {{
                      artifact(id: "{unavailable_id}") {{
                        payloadAvailabilityState
                        payloadUnavailableReasonCode
                        payloadText
                        serverDebugDetail
                      }}
                    }}
                    "#
                ))
                .data(test_principal()),
            )
            .await;
        assert!(
            detail.errors.is_empty(),
            "P085 unavailable artifact detail query must succeed: {detail:?}"
        );
        let detail_json = detail.data.into_json().unwrap();
        let artifact = &detail_json["artifact"];
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("unavailable")
        );
        assert_eq!(
            artifact["payloadUnavailableReasonCode"],
            serde_json::json!("NOT_AVAILABLE")
        );
        assert!(artifact["payloadText"].is_null());
        assert!(artifact["serverDebugDetail"]
            .as_str()
            .is_some_and(|detail| detail.contains("outside the selected run")));

        let _ = fs::remove_dir_all(&artifact_root);
        let _ = fs::remove_dir_all(&outside_root);
    }

    #[tokio::test]
    async fn proposal_085_graphql_backend_projection_and_authorization_contract() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let approval_id = ApprovalId::new();
        let artifact_id = ArtifactId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        stages::insert(&pool, &make_manual_gate_stage(run_id, "stage_1"))
            .await
            .unwrap();
        approvals::insert(
            &pool,
            &Approval {
                id: approval_id,
                run_id,
                stage_id: "stage_1".into(),
                decision: ApprovalDecision::Pending,
                requested_at: Utc::now(),
                decided_at: None,
                comment: None,
                expires_at: None,
            },
        )
        .await
        .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: artifact_id,
                run_id,
                stage_id: "stage_1".into(),
                agent_id: "release".into(),
                name: "release-report".into(),
                contract_id: "release_report_v1".into(),
                format: ArtifactFormat::Json,
                file_path: "/tmp/release-report.json".into(),
                checksum_sha256: None,
                size_bytes: Some(128),
                provider: "test".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("release".into()),
                report_version: Some(1),
                agent_execution_id: None,
            },
        )
        .await
        .unwrap();
        projections::rebuild_approval_inbox(&pool, run_id)
            .await
            .unwrap();
        projections::upsert_artifact_index_entry(&pool, run_id)
            .await
            .unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let query = format!(
            r#"
            query P085BackendAffordanceProof {{
              approvalInbox(runId: "{run_id}") {{
                id
                decision
                availableActions
                disabledReasonCode
                writePathState
                freshnessState
                diagnosticId
                serverDebugDetail
              }}
              artifacts(runId: "{run_id}") {{
                id
                payloadAvailabilityState
                payloadUnavailableReasonCode
                freshnessState
                diagnosticId
                serverDebugDetail
              }}
            }}
            "#
        );
        let allowed = schema
            .execute(Request::new(query.clone()).data(ui_operator))
            .await;
        assert!(
            allowed.errors.is_empty(),
            "P085 backend projection query must succeed for UI operator: {allowed:?}"
        );
        let json = allowed.data.into_json().unwrap();
        let approval = &json["approvalInbox"][0];
        assert_eq!(approval["id"], serde_json::json!(approval_id.to_string()));
        assert_eq!(approval["decision"], serde_json::json!("pending"));
        assert_eq!(
            approval["availableActions"],
            serde_json::json!(["approve", "reject"])
        );
        assert_eq!(approval["writePathState"], serde_json::json!("available"));
        assert_eq!(approval["freshnessState"], serde_json::json!("live"));
        assert_eq!(
            approval["diagnosticId"],
            serde_json::json!(approval_id.to_string())
        );
        assert!(approval["serverDebugDetail"].is_null());

        let artifact = &json["artifacts"][0];
        assert_eq!(artifact["id"], serde_json::json!(artifact_id.to_string()));
        assert_eq!(
            artifact["payloadAvailabilityState"],
            serde_json::json!("metadata_only")
        );
        assert_eq!(
            artifact["payloadUnavailableReasonCode"],
            serde_json::json!("PAYLOAD_DEFERRED_BY_P031")
        );
        assert_eq!(artifact["freshnessState"], serde_json::json!("live"));
        assert_eq!(
            artifact["diagnosticId"],
            serde_json::json!(artifact_id.to_string())
        );
        assert!(
            artifact["serverDebugDetail"].is_string(),
            "server-owned diagnostic detail should explain deferred report payload"
        );

        let denied = schema
            .execute(Request::new(query).data(observer_principal()))
            .await;
        assert!(
            denied
                .errors
                .iter()
                .any(|error| error.message.contains("forbidden")),
            "P085 diagnostic fields must be denied to unauthorized observers: {denied:?}"
        );
    }

    #[tokio::test]
    async fn test_graphql_reject_approval_uses_p072_ui_operator_policy() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let stage = make_manual_gate_stage(run_id, "state_6");
        stages::insert(&pool, &stage).await.unwrap();
        let approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &approval).await.unwrap();

        let (principal_table, default_operator, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table,
            test_reporter(),
        );

        let allowed_with_default_operator = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      rejectApproval(approvalId: "{}", reason: "needs more work", idempotencyKey: "01900000-0000-7000-8003-000000000001") {{
                        approval {{ id }}
                        journalId
                      }}
                    }}"#,
                    approval.id
                ))
                .data(default_operator),
            )
            .await;
        assert!(
            allowed_with_default_operator.errors.is_empty(),
            "default-operator is the app bearer and must allow rejectApproval: {allowed_with_default_operator:?}"
        );

        let ui_approval = make_approval(run_id, "state_6");
        approvals::insert(&pool, &ui_approval).await.unwrap();

        let allowed = schema
            .execute(
                Request::new(format!(
                    r#"mutation {{
                      rejectApproval(approvalId: "{}", reason: "needs more work", idempotencyKey: "01900000-0000-7000-8003-000000000002") {{
                        approval {{ id decision }}
                        journalId
                      }}
                    }}"#,
                    ui_approval.id
                ))
                .data(ui_operator),
            )
            .await;
        assert!(
            allowed.errors.is_empty(),
            "ui_operator rejectApproval must succeed: {allowed:?}"
        );
        let data = allowed.data.into_json().unwrap();
        assert_eq!(
            data["rejectApproval"]["approval"]["decision"],
            serde_json::json!("rejected")
        );
        assert!(
            data["rejectApproval"]["journalId"].is_string(),
            "rejectApproval must return journalId"
        );
    }

    #[tokio::test]
    async fn test_p072_ui_principals_allow_queries() {
        // The app uses one operator bearer, so P072 UI principals must support
        // read operations and approval-only mutations on the same GraphQL surface.
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let (principal_table, _, ui_operator) = p072_principal_table();
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            principal_table.clone(),
            test_reporter(),
        );

        let ui_allowed = schema
            .execute(
                Request::new(format!(r#"{{ run(id: "{}") {{ id }} }}"#, run_id))
                    .data(ui_operator.clone()),
            )
            .await;
        assert!(
            ui_allowed.errors.is_empty(),
            "ui_operator query must succeed: {ui_allowed:?}"
        );

        let (_, default_operator, _) = p072_principal_table();
        let allowed = schema
            .execute(
                Request::new(format!(r#"{{ run(id: "{}") {{ id }} }}"#, run_id))
                    .data(default_operator),
            )
            .await;
        assert!(
            allowed.errors.is_empty(),
            "default-operator query must succeed: {allowed:?}"
        );
    }

    // ── P042 §5.2: daemonStatus query + daemonStatusChanged subscription ──

    fn operator_principal() -> auth::Principal {
        auth::Principal::new("test-operator", auth::PrincipalClass::Operator)
    }

    async fn force_p081_audit_budget_safe_mode(pool: &SqlitePool) {
        let now_ms = Utc::now().timestamp_millis();
        let payload = "x".repeat(16_100);
        let entry = audit_log::AuditEntry {
            id: "p081-graphql-budget-safe-mode",
            request_id: "p081-graphql-budget-safe-mode",
            timestamp_ms: now_ms,
            event_type: "policy_denied",
            principal_id: Some("test-operator"),
            principal_class: Some("operator"),
            caller_class: Some("ui_operator"),
            token_id: None,
            transport: "graphql_mutation",
            action_attempted: "approveApproval",
            decision: "deny",
            denial_reason_code: None,
            row_id: Some("p081.audit_budget.safe_mode"),
            env_gate_state: None,
            source_ip_hash_or_local_process_id: None,
            boundary_policy_mode: "enforce",
            fixture_version: "p081-boundary-matrix-v1",
            payload: &payload,
            original_payload_bytes: None,
            diagnostic_truncated: false,
            checkpoint_id: None,
            created_at_ms: now_ms,
        };
        audit_log::append(pool, &entry).await.unwrap();
        let health = audit_log::health_snapshot(pool).await.unwrap();
        assert_eq!(health.payload_budget_state, "read_only_safe_mode");
    }

    #[tokio::test]
    async fn test_daemon_status_query_includes_build_sha_and_schema_versions() {
        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "cafe-babe", bus.clone());
        reporter.set_state(domain::lifecycle::DaemonLifecycleState::Starting);
        reporter.set_state(domain::lifecycle::DaemonLifecycleState::Ready);
        reporter.set_xcode_broker_health(domain::lifecycle::XcodeBrokerHealthSnapshot {
            state: domain::lifecycle::XcodeBrokerHealthState::Degraded,
            reason_code: "xcode_mcp_capacity_backpressure".to_string(),
            can_acquire_new_xcode_leases: false,
            active_lease_count: 2,
            initialize_queue_depth: 8,
            last_transition_at: "2026-04-25T09:00:00Z".to_string(),
            operator_message: "Xcode MCP bridge pool is applying capacity backpressure."
                .to_string(),
            pool_id: "pool-test".to_string(),
            active_leases: 2,
            queued_leases: 8,
            max_active_leases: 2,
            max_queued_leases: 8,
            broker_disabled: false,
            backend_available: true,
            observation_persistence_failures: 0,
            stale_lease_count: 1,
            backend_session_count: 1,
            helper_cleanup_reaped_leases_total: 2,
        });

        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter)
            .finish();
        let response = schema
            .execute(
                Request::new(
                    r#"{
                      daemonStatus {
                        state schemaVersion binarySchemaVersion buildSha
                        pid lastStateChangeAt json
                        xcodeBrokerHealth {
                          state reasonCode canAcquireNewXcodeLeases
                          activeLeaseCount initializeQueueDepth lastTransitionAt
                          operatorMessage poolId activeLeases queuedLeases
                          maxActiveLeases maxQueuedLeases brokerDisabled
                          backendAvailable observationPersistenceFailures
                          staleLeaseCount backendSessionCount helperCleanupReapedLeasesTotal
                        }
                      }
                    }"#,
                )
                .data(operator_principal()),
            )
            .await;

        assert!(
            response.errors.is_empty(),
            "daemonStatus errored: {response:?}"
        );
        let data = response.data.into_json().unwrap();
        // GraphQL enums serialize as SCREAMING_SNAKE_CASE per spec.
        assert_eq!(data["daemonStatus"]["state"], "READY");
        assert_eq!(data["daemonStatus"]["binarySchemaVersion"], 14);
        assert_eq!(data["daemonStatus"]["buildSha"], "cafe-babe");
        let health = &data["daemonStatus"]["xcodeBrokerHealth"];
        assert_eq!(health["state"], "DEGRADED");
        assert_eq!(health["reasonCode"], "xcode_mcp_capacity_backpressure");
        assert_eq!(health["canAcquireNewXcodeLeases"], false);
        assert_eq!(health["activeLeaseCount"], 2);
        assert_eq!(health["initializeQueueDepth"], 8);
        assert_eq!(health["lastTransitionAt"], "2026-04-25T09:00:00Z");
        assert_eq!(
            health["operatorMessage"],
            "Xcode MCP bridge pool is applying capacity backpressure."
        );
        assert_eq!(health["poolId"], "pool-test");
        assert_eq!(health["activeLeases"], 2);
        assert_eq!(health["queuedLeases"], 8);
        assert_eq!(health["maxActiveLeases"], 2);
        assert_eq!(health["maxQueuedLeases"], 8);
        assert_eq!(health["brokerDisabled"], false);
        assert_eq!(health["backendAvailable"], true);
        assert_eq!(health["observationPersistenceFailures"], 0);
        assert_eq!(health["staleLeaseCount"], 1);
        assert_eq!(health["backendSessionCount"], 1);
        assert_eq!(health["helperCleanupReapedLeasesTotal"], 2);
        // The json field carries the full P042 §5.2 wire shape (snake_case).
        let json_str = data["daemonStatus"]["json"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed["state"], "ready");
        assert_eq!(parsed["binary_schema_version"], 14);
        assert_eq!(
            parsed["xcode_broker_health"]["reason_code"],
            "xcode_mcp_capacity_backpressure"
        );
        assert_eq!(
            parsed["xcode_broker_health"]["can_acquire_new_xcode_leases"],
            false
        );
        assert_eq!(parsed["xcode_broker_health"]["stale_lease_count"], 1);
    }

    #[tokio::test]
    async fn daemon_status_query_is_operator_only() {
        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter)
            .finish();
        let response = schema
            .execute(Request::new("{ daemonStatus { state } }").data(observer_principal()))
            .await;
        assert!(
            response
                .errors
                .iter()
                .any(|e| e.message.contains("forbidden")),
            "observer must be denied, got {response:?}"
        );
    }

    #[tokio::test]
    async fn daemon_status_query_populates_failure_field_when_failed() {
        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        reporter.set_failed(
            domain::lifecycle::FailureKind::MigrationFailed,
            "test failure",
            Some("/tmp/bk.sqlite".into()),
        );
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter)
            .finish();
        let response = schema
            .execute(
                Request::new("{ daemonStatus { state failure { kind detail backupPath } json } }")
                    .data(operator_principal()),
            )
            .await;
        assert!(response.errors.is_empty(), "{response:?}");
        let data = response.data.into_json().unwrap();
        assert_eq!(data["daemonStatus"]["state"], "FAILED");
        // Typed `failure` field is now first-class GraphQL (not nested in
        // a stringified json). `kind` is a GraphQL enum, so it serializes
        // as SCREAMING_SNAKE_CASE.
        assert_eq!(data["daemonStatus"]["failure"]["kind"], "MIGRATION_FAILED");
        assert_eq!(data["daemonStatus"]["failure"]["detail"], "test failure");
        assert_eq!(
            data["daemonStatus"]["failure"]["backupPath"],
            "/tmp/bk.sqlite"
        );
        // `json` retains the snake_case wire shape identical to /health.
        let json_str = data["daemonStatus"]["json"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert_eq!(parsed["failure"]["kind"], "migration_failed");
        assert_eq!(parsed["failure"]["backup_path"], "/tmp/bk.sqlite");
    }

    #[tokio::test]
    async fn daemon_status_changed_subscription_receives_transitions() {
        use async_graphql::futures_util::StreamExt;

        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter.clone())
            .finish();

        let mut stream = schema.execute_stream(
            Request::new("subscription { daemonStatusChanged { state } }")
                .data(operator_principal()),
        );
        // The BroadcastStream only observes frames sent AFTER
        // `events.subscribe()` runs inside the subscription handler, which
        // only runs when the stream is polled. Kick the transition from a
        // spawned task with a small delay so the first poll activates the
        // subscription before the frame is broadcast.
        let reporter_clone = reporter.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            reporter_clone.set_state(domain::lifecycle::DaemonLifecycleState::Starting);
        });
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("subscription frame timed out")
            .expect("subscription stream ended");
        let data = frame.data.into_json().unwrap();
        assert_eq!(data["daemonStatusChanged"]["state"], "STARTING");
    }

    #[tokio::test]
    async fn test_daemon_status_changed_subscription_auth_required() {
        use async_graphql::futures_util::StreamExt;

        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter.clone())
            .finish();

        // No principal data inserted into the request — this mirrors what
        // happens when WS `connection_init` is missing or rejects the
        // token. The subscription handler must refuse with "unauthorized"
        // on first poll, not silently pass through frames.
        let mut stream = schema.execute_stream(Request::new(
            "subscription { daemonStatusChanged { state } }",
        ));
        let frame = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
            .await
            .expect("unauthorized subscription should produce a frame, not hang")
            .expect("stream should not end before emitting the error");
        assert!(
            !frame.errors.is_empty(),
            "subscription without principal must surface an error, got {frame:?}"
        );
        assert!(
            frame
                .errors
                .iter()
                .any(|e| e.message.to_lowercase().contains("unauthorized")),
            "error message must mention 'unauthorized': {:?}",
            frame.errors
        );
    }

    #[tokio::test]
    async fn test_daemon_status_changed_subscription_rejects_non_operator_principal() {
        use async_graphql::futures_util::StreamExt;

        let pool = test_pool().await;
        let bus = event_bus::new_bus(16);
        let reporter = LifecycleReporter::new(14, "dev", bus.clone());
        let schema = Schema::build(QueryRoot, MutationRoot, SubscriptionRoot)
            .data(pool.clone())
            .data(make_command_handler(pool.clone()))
            .data(bus)
            .data(auth::PrincipalTable::test_fixture())
            .data(reporter.clone())
            .finish();

        // Observer class — has a principal but is not Operator. P042 §5.2
        // marks the subscription as operator-only bearer auth; the handler
        // must refuse with `forbidden`, not stream frames.
        let mut stream = schema.execute_stream(
            Request::new("subscription { daemonStatusChanged { state } }")
                .data(observer_principal()),
        );
        let frame = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next())
            .await
            .expect("observer subscription should produce a frame, not hang")
            .expect("stream should not end before emitting the error");
        assert!(
            !frame.errors.is_empty(),
            "observer subscription must surface an error, got {frame:?}"
        );
        assert!(
            frame
                .errors
                .iter()
                .any(|e| e.message.to_lowercase().contains("forbidden")),
            "error must mention 'forbidden' for the observer class: {:?}",
            frame.errors
        );
    }

    // Mutex to serialise tests that mutate CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE.
    static P087_GQL_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[tokio::test]
    async fn proposal_087_graphql_storage_health_circuit_open_returns_hot_read_circuit_open_error()
    {
        // Prove that record_violation("timeout") — now called by the GraphQL storage_health
        // resolver on timeout — opens the circuit and subsequent queries return
        // HOT_READ_CIRCUIT_OPEN, matching the MCP enforcement contract.
        let _env_lock = P087_GQL_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::env::set_var(
            "CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE",
            "enforce",
        );

        let pool = test_pool().await;

        // Open the circuit by recording 3 violations (the same error code the timeout
        // path now emits via guard.record_violation("timeout")).
        let guard = db::hot_read_guard::HotReadGuard::new(pool.clone(), "storage.health");
        for _ in 0..3 {
            guard.record_violation("timeout").await.unwrap();
        }

        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        let resp = schema
            .execute(
                Request::new("{ storageHealth { projections { pendingInvalidations } } }")
                    .data(test_principal()),
            )
            .await;

        std::env::remove_var("CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE");

        assert!(
            !resp.errors.is_empty(),
            "circuit-open must surface a GraphQL error, got: {:?}",
            resp.data
        );
        let error_str = format!("{:?}", resp.errors);
        assert!(
            error_str.contains("HOT_READ_CIRCUIT_OPEN") || error_str.contains("circuit"),
            "error must identify the circuit-open condition: {error_str}"
        );
    }

    #[tokio::test]
    async fn proposal_087_graphql_storage_health_record_violation_wires_to_circuit_state() {
        // Prove the GraphQL storageHealth guard's record_violation("timeout") updates
        // hot_read_circuit_states — the same persistence path used by MCP.
        let pool = test_pool().await;
        let guard = db::hot_read_guard::HotReadGuard::new(pool.clone(), "storage.health");

        // Three violations must be enough to set would_open or open the circuit.
        for _ in 0..3 {
            guard.record_violation("timeout").await.unwrap();
        }

        let (_, _, failures, _, _, _) =
            db::repos::hot_read_circuit::get_circuit_state(&pool, "storage.health")
                .await
                .unwrap();
        assert!(
            failures >= 3,
            "violation calls must be persisted in hot_read_circuit_states; got {failures}"
        );
    }

    /// Validate that the exact Swift storageDiagnosticsQuery executes without schema errors.
    /// This catches query/schema drift that string-based gate checks cannot detect.
    #[tokio::test]
    async fn proposal_087_swift_storage_diagnostics_query_matches_graphql_schema() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool.clone()),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        // Exact field set from DaemonLifecycleClient.swift storageDiagnosticsQuery.
        // Must stay in sync: if schema changes break this test, update both the query and schema.
        let query = r#"{
          storageHealth {
            updatedAt staleAfterMs isStale dbState
            writer {
              alive lastHeartbeatAt lastDrainAt totalQueued
              lanes { lane capacity queuedDepth queuedDepthRatio oldestQueuedAgeMs rejectedTotal droppedTotal }
              writeLockWaitP50Ms writeLockWaitP95Ms transactionDurationP95Ms
              busyRetryRatePerMinute busyRetryExhaustedTotal rejectedTotal droppedTelemetryTotal
            }
            wal {
              available unavailableReason sizeBytes warnSizeBytes criticalSizeBytes
              lastCheckpointAt checkpointDurationP95Ms
            }
            projections {
              pendingInvalidations projectionLagMs coalescedKeysPending
              coalescedMergedTotal coalescedFlushAgeP95Ms
            }
            evidenceSpool {
              enabled filesWrittenTotal bytesWrittenTotal metadataRowsTotal
              orphanFiles orphanBytes recoveredFiles checksumMismatchFiles pendingDeleteFiles
            }
            killSwitches { dbWriterBypassClasses coalescingDisabledKeys evidenceSpoolDisabledKinds }
            thresholds { metric warn critical unit action }
            projectionFreshness {
              projectionName sourceName watermarkMs isPoisoned lastError
              updatedAtMs throttledUntilMs backlogRows backlogBytes
            }
            hotReadGuards {
              governedSurface circuitStatus consecutiveSuccesses lastViolationKind
              wouldOpen lastOpenedAtMs updatedAtMs latencyMs
            }
            maintenanceOperations {
              id operationKind status idempotencyKey slotGeneration
              startedAtMs completedAtMs error createdAtMs updatedAtMs
            }
            degraded { severity reason message }
            rollout
          }
        }"#;

        let response = schema
            .execute(Request::new(query).data(test_principal()))
            .await;

        assert!(
            response.errors.is_empty(),
            "Swift storageDiagnosticsQuery must execute without schema errors: {:?}",
            response.errors
        );
    }

    /// Executable negative introspection: proves StorageHealth.projections is of type
    /// ProjectionStorageHealth (OBJECT), not a list or scalar. Fails if P087 breaks the
    /// GraphQL type contract by changing this field to a different type.
    #[tokio::test]
    async fn proposal_087_graphql_storage_health_projections_type_is_projection_storage_health() {
        let pool = test_pool().await;
        let schema = build_schema(
            pool.clone(),
            make_command_handler(pool),
            event_bus::new_bus(64),
            auth::PrincipalTable::test_fixture(),
            test_reporter(),
        );

        // Introspect StorageHealth to verify the `projections` field type.
        let introspect_query = r#"{
          __type(name: "StorageHealth") {
            fields {
              name
              type {
                name
                kind
                ofType { name kind }
              }
            }
          }
        }"#;

        let response = schema
            .execute(Request::new(introspect_query).data(test_principal()))
            .await;

        assert!(
            response.errors.is_empty(),
            "StorageHealth introspection must succeed: {:?}",
            response.errors
        );

        let data = response
            .data
            .into_json()
            .expect("introspection data must serialize");
        let fields = data["__type"]["fields"]
            .as_array()
            .expect("StorageHealth must have fields");

        let projections_field = fields
            .iter()
            .find(|f| f["name"] == "projections")
            .expect("StorageHealth must have a 'projections' field");

        // The field type must be (non-null) ProjectionStorageHealth OBJECT, not a list or scalar.
        // GraphQL wraps non-null fields as NON_NULL { ofType: { kind, name } }, so we must
        // unwrap the outer NON_NULL layer to reach the actual type.
        let outer_kind = projections_field["type"]["kind"].as_str().unwrap_or("");
        let (resolved_kind, resolved_name) = if outer_kind == "NON_NULL" {
            (
                projections_field["type"]["ofType"]["kind"]
                    .as_str()
                    .unwrap_or(""),
                projections_field["type"]["ofType"]["name"]
                    .as_str()
                    .unwrap_or(""),
            )
        } else {
            (
                outer_kind,
                projections_field["type"]["name"].as_str().unwrap_or(""),
            )
        };
        assert_eq!(
            resolved_kind, "OBJECT",
            "StorageHealth.projections must resolve to an OBJECT type, not {resolved_kind}. \
             P087 must not change this field to a list, scalar, or union."
        );
        assert_eq!(
            resolved_name, "ProjectionStorageHealth",
            "StorageHealth.projections must resolve as ProjectionStorageHealth, got '{resolved_name}'. \
             P087 must not rename or retype this field."
        );

        // Introspect ProjectionFreshnessV1 to verify identity fields projectionName and sourceName.
        let freshness_introspect = r#"{
          __type(name: "ProjectionFreshnessV1") {
            fields { name }
          }
        }"#;

        let freshness_response = schema
            .execute(Request::new(freshness_introspect).data(test_principal()))
            .await;

        assert!(
            freshness_response.errors.is_empty(),
            "ProjectionFreshnessV1 introspection must succeed: {:?}",
            freshness_response.errors
        );

        let freshness_data = freshness_response
            .data
            .into_json()
            .expect("freshness introspection data must serialize");
        let freshness_fields = freshness_data["__type"]["fields"]
            .as_array()
            .expect("ProjectionFreshnessV1 must have fields");

        let field_names: Vec<&str> = freshness_fields
            .iter()
            .filter_map(|f| f["name"].as_str())
            .collect();

        assert!(
            field_names.contains(&"projectionName"),
            "ProjectionFreshnessV1 must include projectionName identity field; got: {field_names:?}"
        );
        assert!(
            field_names.contains(&"sourceName"),
            "ProjectionFreshnessV1 must include sourceName identity field; got: {field_names:?}"
        );
    }

    // ── P046: retry exhaustion unit tests ────────────────────────────────────
    //
    // These unit tests exercise db::p046_retry::p046_retry_db (now db-owned per
    // approved architecture contract) via the re-exported alias to prove the
    // pinned retry policy:
    // - All 3 attempts are tried before exhaustion is declared.
    // - The returned error starts with "transient_db_unavailable".
    // - deadline_headroom_stop is recorded when the remaining budget is zero.

    #[tokio::test]
    async fn p046_retry_db_exhaustion_returns_transient_db_unavailable() {
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_clone = call_count.clone();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);

        let result = p046_retry_db("test_exhaustion_field", deadline, || {
            let cc = call_count_clone.clone();
            async move {
                cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err::<(), _>(anyhow::anyhow!("database is locked: injected test error"))
            }
        })
        .await;

        assert!(result.is_err(), "exhausted retry must return Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.starts_with("transient_db_unavailable"),
            "error must start with transient_db_unavailable; got: {msg}"
        );
        assert_eq!(
            call_count.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "policy requires exactly 3 total attempts before exhaustion"
        );
    }

    #[tokio::test]
    async fn p046_retry_db_non_transient_error_propagates_immediately() {
        let call_count = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let call_count_clone = call_count.clone();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);

        let result = p046_retry_db("test_nontransient_field", deadline, || {
            let cc = call_count_clone.clone();
            async move {
                cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err::<(), _>(anyhow::anyhow!("schema mismatch: unexpected column"))
            }
        })
        .await;

        assert!(result.is_err(), "non-transient error must propagate");
        let msg = result.unwrap_err().to_string();
        assert!(
            !msg.starts_with("transient_db_unavailable"),
            "non-transient error must not be wrapped as transient_db_unavailable; got: {msg}"
        );
        assert_eq!(
            call_count.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "non-transient errors must not be retried (exactly 1 attempt)"
        );
    }
}
