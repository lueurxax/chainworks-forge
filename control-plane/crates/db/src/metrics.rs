use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const MAX_SAMPLES: usize = 1024;

#[derive(Debug, Default)]
pub struct Histogram {
    samples: VecDeque<u64>,
}

impl Histogram {
    pub fn record(&mut self, value: u64) {
        if self.samples.len() >= MAX_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(value);
    }

    pub fn sample_count(&self) -> u64 {
        self.samples.len() as u64
    }

    pub fn latest(&self) -> Option<u64> {
        self.samples.back().copied()
    }

    pub fn p50(&self) -> Option<u64> {
        self.percentile(50)
    }

    pub fn p95(&self) -> Option<u64> {
        self.percentile(95)
    }

    fn percentile(&self, p: usize) -> Option<u64> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sorted = self.samples.iter().copied().collect::<Vec<_>>();
        sorted.sort_unstable();
        let idx = ((sorted.len() - 1) * p).div_ceil(100);
        sorted.get(idx).copied()
    }
}

#[derive(Debug, Default)]
struct SystemMetrics {
    hot_read_latency: HashMap<String, Histogram>,
    projection_rebuild_duration: HashMap<String, Histogram>,
    projection_lag: HashMap<String, Histogram>,
    projection_backlog_rows: HashMap<String, u64>,
    projection_backlog_bytes: HashMap<String, u64>,
    counters: HashMap<String, u64>,
    gauges: HashMap<String, u64>,
    p058_metric_samples: HashMap<String, Histogram>,
    /// Keyed by "scenario_id:reason_code" to carry the {scenario_id,reason_code} label dimensions.
    p082_recovery_state_age_seconds: HashMap<String, u64>,
    mcp_liveness_gate_last_recorded_at_ms: Option<i64>,
    mcp_hot_read_error_total_by_code: HashMap<String, u64>,
    p046_query_duration_ms: HashMap<String, Histogram>,
    p046_emit_lag_ms: Histogram,
}

static METRICS: OnceLock<Mutex<SystemMetrics>> = OnceLock::new();

/// P046 bounded-label metric names required by rollout_contract_v1.
/// Used by gate inventory checks to verify metric emissions are present.
pub const P046_REQUIRED_METRICS: &[&str] = &[
    "session_graphql_query_total",
    "session_graphql_query_duration_seconds",
    "session_graphql_sqlite_retry_total",
    "session_graphql_sqlite_retry_exhausted_total",
    "session_status_subscription_event_total",
    "session_status_subscription_emit_lag_seconds",
    "session_status_subscription_slow_consumer_disconnect_total",
    "session_health_warning_total",
    "session_event_redaction_total",
    "session_graphql_disabled_schema_guard_total",
    "session_graphql_reset_mutation_guard_total",
    "session_graphql_observability_query_success_rate",
];

pub const P087_REQUIRED_METRICS: &[&str] = &[
    "db_writer_alive",
    "db_writer_queue_depth_by_lane",
    "db_writer_write_rejection_total_by_lane",
    "db_writer_write_wait_ms_p50",
    "db_writer_write_wait_ms_p95",
    "db_writer_write_wait_ms_p99",
    "db_writer_transaction_duration_ms_p50",
    "db_writer_transaction_duration_ms_p95",
    "db_writer_transaction_duration_ms_p99",
    "sqlite_wal_size_bytes",
    "sqlite_checkpoint_duration_ms",
    "sqlite_checkpoint_failed_total",
    "evidence_spool_bytes_written_total",
    "evidence_spool_orphan_count",
    "projection_lag_count",
    "projection_rebuild_duration_ms",
    "runs_list_read_latency_ms",
    "mcp_liveness_gate_duration_ms",
    "mcp_hot_read_violation_total",
    "storage_maintenance_reaper_sla_breach_total",
    "projection_invalidation_coalesce_near_capacity_total",
    "hot_read_latency_ms",
    "projection_invalidation_backlog_rows",
    "projection_invalidation_backlog_bytes",
    "projection_invalidation_backlog_exceeded_total",
    "mcp_hot_read_error_total_by_code",
    "graphql_projection_freshness_identity_missing_total",
    "storage_health_legacy_projection_field_compat_total",
    "maintenance_slot_release_cas_failed_total",
    "hot_read_circuit_would_open_total",
    "hot_read_circuit_open_total",
];

/// P082: Required metric names for the recovery/retry state-machine matrix proof gate.
/// These names are checked by the test gate to confirm all required metrics are declared
/// before emission wiring is added at the approved sites.
pub const P082_REQUIRED_METRICS: &[&str] = &[
    "p082_recovery_matrix_rows_with_db_engine_readback_coverage_percent",
    "p082_recovery_matrix_gate_result_total",
    "p082_recovery_reason_readback_total",
    "p082_recovery_mutation_rejected_total",
    "p082_release_side_effect_retry_block_total",
    "p082_late_output_quarantine_total",
    "p082_recovery_idempotency_replay_total",
    "p082_recovery_state_age_seconds",
];

pub const P081_REQUIRED_METRICS: &[&str] = &[
    "p081_boundary_policy_enforcement_parity_percent",
    "boundary_policy_decisions_total",
    "boundary_policy_shadow_disagreement_total",
    "auth_ambiguous_caller_warn_total",
    "boundary_no_op_label_total",
    "audit_log_append_failure_total",
    "audit_log_rate_limited_total",
    "operator_alert_native_delivery_total",
    "operator_alert_clear_latency_ms",
    "mcp_command_idempotency_replay_total",
    "mcp_command_idempotency_conflict_total",
    "approval_idempotency_duplicate_total",
    "boundary_policy_evaluation_error_total",
    "approval_actionability_false_total",
    "graphql_redaction_extensions_total",
    "boundary_policy_decision_latency_ms",
    "boundary_commit_transaction_latency_ms",
    "audit_budget_cleanup_duration_ms",
];

pub const P058_REQUIRED_METRICS: &[&str] = &[
    "escalation_chains_started_total",
    "escalation_tier_success_rate",
    "time_to_success_after_escalation_seconds",
    "escalation_pause_total",
    "false_escalation_rate",
    "policy_compile_failure_total",
    "shadow_tier_selection_match_rate",
    "provider_session_kill_latency_seconds",
    "daemon_outage_credit_seconds_total",
    "fan_out_blocked_dwell_seconds",
    "launch_recycle_storm_total",
    "capacity_probe_failure_total",
    "escalation_drift_pending_ack_dwell_seconds",
    "tier_dwell_share_of_chain",
    "chain_exhausted_total_by_terminal_tier_kind",
    "escalation_repeated_digest_no_progress_total",
    "escalation_commit_contention_total",
    "escalation_retry_after_parse_anomaly_total",
    "escalation_provider_late_frame_after_detach_total",
];

fn metrics() -> &'static Mutex<SystemMetrics> {
    METRICS.get_or_init(|| Mutex::new(SystemMetrics::default()))
}

pub fn record_projection_backlog(projection: &str, source: &str, rows: u64, bytes: u64) {
    let mut m = metrics().lock().unwrap();
    let label = format!("{}:{}", projection, source);

    // Bounded labels: avoid memory blow-up from unbounded projection/source names.
    let key = if m.projection_backlog_rows.len() >= 256
        && !m.projection_backlog_rows.contains_key(&label)
    {
        "unbounded_overflow".to_string()
    } else {
        label
    };

    m.projection_backlog_rows.insert(key.clone(), rows);
    m.projection_backlog_bytes.insert(key, bytes);
}

pub fn increment_counter(name: &str) {
    let mut m = metrics().lock().unwrap();
    *m.counters.entry(name.to_string()).or_default() += 1;
}

pub fn increment_counter_with_label(name: &str, label: &str) {
    let mut m = metrics().lock().unwrap();
    let key = format!("{}:{}", name, label);
    *m.counters.entry(key).or_default() += 1;
}

fn metric_key(name: &str, labels: &[(&str, &str)]) -> String {
    if labels.is_empty() {
        return name.to_string();
    }
    let suffix = labels
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{name}:{suffix}")
}

fn record_labeled_histogram(name: &str, labels: &[(&str, &str)], value: u64) {
    let mut m = metrics().lock().unwrap();
    m.hot_read_latency
        .entry(name.to_string())
        .or_default()
        .record(value);
    m.hot_read_latency
        .entry(metric_key(name, labels))
        .or_default()
        .record(value);
}

pub fn reset_for_tests() {
    let mut m = metrics().lock().unwrap();
    *m = SystemMetrics::default();
}

pub fn record_p081_auth_ambiguous_caller_warn(
    principal_class: &str,
    surface_policy_hash: &str,
    transport: &str,
) {
    increment_counter("auth_ambiguous_caller_warn_total");
    increment_counter_with_label(
        "auth_ambiguous_caller_warn_total",
        &format!(
            "principal_class={principal_class},surface_policy_hash={surface_policy_hash},transport={transport}"
        ),
    );
}

pub fn record_p081_boundary_no_op_label(repo: &str, month: &str) {
    increment_counter("boundary_no_op_label_total");
    increment_counter_with_label(
        "boundary_no_op_label_total",
        &format!("repo={repo},month={month}"),
    );
}

pub fn record_p081_audit_log_rate_limited(transport: &str, reason_code: &str) {
    increment_counter("audit_log_rate_limited_total");
    increment_counter_with_label(
        "audit_log_rate_limited_total",
        &format!("transport={transport},reason_code={reason_code}"),
    );
}

pub fn record_p081_audit_log_append_failure(event_type: &str, transport: &str, mode: &str) {
    increment_counter("audit_log_append_failure_total");
    increment_counter_with_label(
        "audit_log_append_failure_total",
        &format!("event_type={event_type},transport={transport},mode={mode}"),
    );
}

pub fn record_p081_operator_alert_native_delivery(severity: &str, surface: &str, result: &str) {
    increment_counter("operator_alert_native_delivery_total");
    increment_counter_with_label(
        "operator_alert_native_delivery_total",
        &format!("severity={severity},surface={surface},result={result}"),
    );
}

pub fn record_p081_approval_idempotency_duplicate(action: &str, caller_class: &str) {
    increment_counter("approval_idempotency_duplicate_total");
    increment_counter_with_label(
        "approval_idempotency_duplicate_total",
        &format!("action={action},caller_class={caller_class}"),
    );
}

pub fn record_p081_boundary_decision(
    transport: &str,
    row_id: Option<&str>,
    caller_class: &str,
    action_kind: &str,
    decision: &str,
    denial_reason_code: Option<&str>,
    mode: &str,
) {
    increment_counter("boundary_policy_decisions_total");
    increment_counter_with_label(
        "boundary_policy_decisions_total",
        &format!(
            "transport={transport},row_id={},caller_class={caller_class},action_kind={action_kind},decision={decision},denial_reason_code={},shadow_or_enforce={mode}",
            row_id.unwrap_or("none"),
            denial_reason_code.unwrap_or("none")
        ),
    );
}

pub fn record_p081_boundary_decision_latency(
    transport: &str,
    caller_class: &str,
    mode: &str,
    duration: Duration,
) {
    record_labeled_histogram(
        "boundary_policy_decision_latency_ms",
        &[
            ("transport", transport),
            ("caller_class", caller_class),
            ("mode", mode),
        ],
        duration.as_millis() as u64,
    );
}

pub fn record_p081_boundary_commit_transaction_latency(
    transport: &str,
    action_kind: &str,
    decision: &str,
    duration: Duration,
) {
    record_labeled_histogram(
        "boundary_commit_transaction_latency_ms",
        &[
            ("transport", transport),
            ("action_kind", action_kind),
            ("decision", decision),
        ],
        duration.as_millis() as u64,
    );
}

pub fn record_p081_audit_budget_cleanup_duration(duration: Duration) {
    let mut m = metrics().lock().unwrap();
    m.hot_read_latency
        .entry("audit_budget_cleanup_duration_ms".to_string())
        .or_default()
        .record(duration.as_millis() as u64);
}

pub fn record_p081_operator_alert_clear_latency(
    alert_id: &str,
    severity: &str,
    duration: Duration,
) {
    record_labeled_histogram(
        "operator_alert_clear_latency_ms",
        &[("alert_id", alert_id), ("severity", severity)],
        duration.as_millis() as u64,
    );
}

pub fn record_p081_boundary_policy_enforcement_parity_percent(percent: u64) {
    let mut m = metrics().lock().unwrap();
    m.hot_read_latency
        .entry("p081_boundary_policy_enforcement_parity_percent".to_string())
        .or_default()
        .record(percent.min(100));
}

pub fn record_p081_boundary_policy_enforcement_parity(
    legacy_decision: &str,
    matrix_decision: &str,
) {
    let percent = if legacy_decision == matrix_decision {
        100
    } else {
        0
    };
    record_p081_boundary_policy_enforcement_parity_percent(percent);
}

pub fn record_escalation_chain_started(policy_id: &str, tier_kind: Option<&str>) {
    increment_counter("escalation_chains_started_total");
    record_labeled_histogram(
        "escalation_chains_started_total",
        &[
            ("policy_id", policy_id),
            ("tier_kind", tier_kind.unwrap_or("unknown")),
        ],
        1,
    );
}

pub fn record_escalation_event(
    event_kind: &str,
    pause_reason: Option<&str>,
    terminal_tier_kind: Option<&str>,
    payload_json: Option<&str>,
) {
    let payload =
        payload_json.and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());
    match event_kind {
        "escalation.chain_started" => increment_counter("escalation_chains_started_total"),
        "escalation.tier_success" => {
            increment_counter("escalation_tier_success_rate");
            record_p058_rate_from_payload("escalation_tier_success_rate", payload.as_ref());
        }
        "escalation.false_positive" => {
            increment_counter("false_escalation_rate");
            record_p058_rate_from_payload("false_escalation_rate", payload.as_ref());
        }
        "escalation.policy_compile_failure" => increment_counter("policy_compile_failure_total"),
        "escalation.shadow_match" => {
            increment_counter("shadow_tier_selection_match_rate");
            record_p058_rate_from_payload("shadow_tier_selection_match_rate", payload.as_ref());
        }
        "escalation.provider_force_detach" => {
            increment_counter("provider_session_kill_latency_seconds");
            record_p058_duration_from_payload(
                "provider_session_kill_latency_seconds",
                payload.as_ref(),
            );
        }
        "escalation.daemon_outage_credit" => {
            increment_counter("daemon_outage_credit_seconds_total");
            record_p058_duration_from_payload(
                "daemon_outage_credit_seconds_total",
                payload.as_ref(),
            );
        }
        "escalation.fan_out_blocked_dwell" => {
            increment_counter("fan_out_blocked_dwell_seconds");
            record_p058_duration_from_payload("fan_out_blocked_dwell_seconds", payload.as_ref());
        }
        "escalation.launch_recycle_storm" => increment_counter("launch_recycle_storm_total"),
        "escalation.capacity_probe_failure" => increment_counter("capacity_probe_failure_total"),
        "escalation.drift_pending_ack_dwell" => {
            increment_counter("escalation_drift_pending_ack_dwell_seconds");
            record_p058_duration_from_payload(
                "escalation_drift_pending_ack_dwell_seconds",
                payload.as_ref(),
            );
        }
        "escalation.tier_dwell_share" => {
            increment_counter("tier_dwell_share_of_chain");
            record_p058_rate_from_payload("tier_dwell_share_of_chain", payload.as_ref());
        }
        "escalation.chain_exhausted" => {
            increment_counter_with_label(
                "chain_exhausted_total_by_terminal_tier_kind",
                terminal_tier_kind.unwrap_or("unknown"),
            );
        }
        "escalation.commit_contention" => increment_counter("escalation_commit_contention_total"),
        "escalation.retry_after_parse_anomaly" => {
            increment_counter("escalation_retry_after_parse_anomaly_total")
        }
        "escalation.provider_late_frame_after_detach" => {
            increment_counter("escalation_provider_late_frame_after_detach_total")
        }
        "escalation.time_to_success_recorded" => {
            increment_counter("time_to_success_after_escalation_seconds");
            record_p058_duration_from_payload(
                "time_to_success_after_escalation_seconds",
                payload.as_ref(),
            );
        }
        _ => {}
    }

    if let Some(reason) = pause_reason {
        increment_counter_with_label("escalation_pause_total", reason);
    }
    if pause_reason == Some("escalation_repeated_digest_no_progress") {
        increment_counter("escalation_repeated_digest_no_progress_total");
    }
}

pub fn record_p058_metric_sample(metric_name: &str, sample_value: u64) {
    if !P058_REQUIRED_METRICS.contains(&metric_name) {
        return;
    }
    let mut m = metrics().lock().unwrap();
    m.p058_metric_samples
        .entry(metric_name.to_string())
        .or_default()
        .record(sample_value);
}

pub fn get_p058_metric_sample_count(metric_name: &str) -> u64 {
    let m = metrics().lock().unwrap();
    m.p058_metric_samples
        .get(metric_name)
        .map(Histogram::sample_count)
        .unwrap_or(0)
}

pub fn get_p058_metric_latest(metric_name: &str) -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.p058_metric_samples
        .get(metric_name)
        .and_then(Histogram::latest)
}

pub fn get_p058_metric_p95(metric_name: &str) -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.p058_metric_samples
        .get(metric_name)
        .and_then(Histogram::p95)
}

fn record_p058_duration_from_payload(metric_name: &str, payload: Option<&serde_json::Value>) {
    if let Some(sample) = payload
        .and_then(|value| value.get("metric_sample_ms"))
        .and_then(serde_json::Value::as_u64)
    {
        record_p058_metric_sample(metric_name, sample);
    }
}

fn record_p058_rate_from_payload(metric_name: &str, payload: Option<&serde_json::Value>) {
    let Some(payload) = payload else {
        return;
    };
    let numerator = payload
        .get("metric_numerator")
        .and_then(serde_json::Value::as_u64);
    let denominator = payload
        .get("metric_denominator")
        .and_then(serde_json::Value::as_u64);
    if let (Some(numerator), Some(denominator)) = (numerator, denominator) {
        if denominator > 0 {
            record_p058_metric_sample(metric_name, numerator.saturating_mul(10_000) / denominator);
        }
    }
}

pub fn record_p081_boundary_shadow_disagreement(
    transport: &str,
    row_id: Option<&str>,
    caller_class: &str,
    action_kind: &str,
    legacy_decision: &str,
    matrix_decision: &str,
    denial_reason_code: Option<&str>,
) {
    increment_counter("boundary_policy_shadow_disagreement_total");
    increment_counter_with_label(
        "boundary_policy_shadow_disagreement_total",
        &format!(
            "transport={transport},row_id={},caller_class={caller_class},action_kind={action_kind},legacy_decision={legacy_decision},matrix_decision={matrix_decision},denial_reason_code={}",
            row_id.unwrap_or("none"),
            denial_reason_code.unwrap_or("none")
        ),
    );
}

pub fn record_p081_boundary_policy_evaluation_error(transport: &str, mode: &str) {
    increment_counter("boundary_policy_evaluation_error_total");
    increment_counter_with_label(
        "boundary_policy_evaluation_error_total",
        &format!("transport={transport},mode={mode}"),
    );
}

pub fn record_p081_approval_actionability_false(
    caller_class: &str,
    row_id: Option<&str>,
    reason_code: &str,
) {
    increment_counter("approval_actionability_false_total");
    increment_counter_with_label(
        "approval_actionability_false_total",
        &format!(
            "caller_class={caller_class},row_id={},reason_code={reason_code}",
            row_id.unwrap_or("none")
        ),
    );
}

pub fn get_counter(name: &str) -> u64 {
    let m = metrics().lock().unwrap();
    m.counters.get(name).copied().unwrap_or(0)
}

pub fn get_counter_with_label(name: &str, label: &str) -> u64 {
    let m = metrics().lock().unwrap();
    let key = format!("{}:{}", name, label);
    m.counters.get(&key).copied().unwrap_or(0)
}

pub fn set_gauge(name: &str, value: u64) {
    let mut m = metrics().lock().unwrap();
    m.gauges.insert(name.to_string(), value);
}

pub fn get_gauge(name: &str) -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.gauges.get(name).copied()
}

pub fn record_p082_recovery_matrix_coverage_percent(rows_with_readback: usize, total_rows: usize) {
    let percent = if total_rows == 0 {
        0
    } else {
        ((rows_with_readback as u64) * 100) / (total_rows as u64)
    };
    set_gauge(
        "p082_recovery_matrix_rows_with_db_engine_readback_coverage_percent",
        percent,
    );
}

/// Emit `p082_recovery_matrix_gate_result_total{scenario_id,status}`.
/// The compound label `"scenario_id:status"` preserves the required two label dimensions.
pub fn record_p082_recovery_matrix_gate_result(scenario_id: &str, status: &str) {
    increment_counter_with_label(
        "p082_recovery_matrix_gate_result_total",
        &format!("{scenario_id}:{status}"),
    );
}

/// Emit `p082_recovery_state_age_seconds{scenario_id,reason_code}`.
/// Stores the latest age per compound key so callers can query by both dimensions.
pub fn record_p082_recovery_state_age_seconds(
    scenario_id: &str,
    reason_code: &str,
    age_seconds: u64,
) {
    let mut m = metrics().lock().unwrap();
    let key = format!("{scenario_id}:{reason_code}");
    m.p082_recovery_state_age_seconds.insert(key, age_seconds);
}

/// Returns the maximum age across all labeled scenario/reason entries, or `None` when empty.
pub fn get_p082_recovery_state_age_seconds_latest() -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.p082_recovery_state_age_seconds.values().copied().max()
}

/// Returns the age for a specific `{scenario_id,reason_code}` pair, or `None` if not recorded.
pub fn get_p082_recovery_state_age_seconds_for(
    scenario_id: &str,
    reason_code: &str,
) -> Option<u64> {
    let m = metrics().lock().unwrap();
    let key = format!("{scenario_id}:{reason_code}");
    m.p082_recovery_state_age_seconds.get(&key).copied()
}

/// Returns the sum of all counters whose key starts with `prefix:`.
/// Used in tests to verify that a labelled counter family was incremented
/// without enumerating every possible label combination.
pub fn get_counter_prefix_sum(prefix: &str) -> u64 {
    let m = metrics().lock().unwrap();
    let prefix_colon = format!("{prefix}:");
    m.counters
        .iter()
        .filter(|(k, _)| k.starts_with(&prefix_colon))
        .map(|(_, v)| v)
        .sum()
}

/// Record P046 query resolver duration (session_graphql_query_duration_seconds).
/// Stored as milliseconds internally; the metric name uses _seconds per the proposal vocabulary.
pub fn record_p046_query_duration(field: &str, millis: u64) {
    let mut m = metrics().lock().unwrap();
    let key =
        if m.p046_query_duration_ms.len() >= 32 && !m.p046_query_duration_ms.contains_key(field) {
            "unbounded_overflow"
        } else {
            field
        };
    m.p046_query_duration_ms
        .entry(key.to_string())
        .or_default()
        .record(millis);
    // Also increment success-rate adoption counter (session_graphql_observability_query_success_rate).
    *m.counters
        .entry("session_graphql_observability_query_success_rate".to_string())
        .or_default() += 1;
}

pub fn get_p046_query_duration_p95(field: &str) -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.p046_query_duration_ms.get(field).and_then(|h| h.p95())
}

/// Record P046 subscription emit lag (session_status_subscription_emit_lag_seconds).
/// Stored as milliseconds internally.
pub fn record_p046_emit_lag(millis: u64) {
    let mut m = metrics().lock().unwrap();
    m.p046_emit_lag_ms.record(millis);
}

pub fn get_p046_emit_lag_p95() -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.p046_emit_lag_ms.p95()
}

pub fn get_p046_emit_lag_p99() -> Option<u64> {
    let m = metrics().lock().unwrap();
    // Approximate p99 from the sorted sample.
    if m.p046_emit_lag_ms.samples.is_empty() {
        return None;
    }
    let mut sorted: Vec<u64> = m.p046_emit_lag_ms.samples.iter().copied().collect();
    sorted.sort_unstable();
    let idx = ((sorted.len() - 1) * 99).div_ceil(100);
    sorted.get(idx).copied()
}

pub fn record_hot_read_latency(tool: &str, duration: Duration) {
    let mut m = metrics().lock().unwrap();
    let ms = duration.as_millis() as u64;
    m.hot_read_latency
        .entry(tool.to_string())
        .or_default()
        .record(ms);
    m.hot_read_latency
        .entry("hot_read_latency_ms".to_string())
        .or_default()
        .record(ms);
    if tool == "runs.list" {
        m.hot_read_latency
            .entry("runs_list_read_latency_ms".to_string())
            .or_default()
            .record(ms);
    }
}

pub fn record_mcp_liveness_gate_duration(duration: Duration) {
    let mut m = metrics().lock().unwrap();
    m.hot_read_latency
        .entry("mcp_liveness_gate_duration_ms".to_string())
        .or_default()
        .record(duration.as_millis() as u64);
    m.mcp_liveness_gate_last_recorded_at_ms = Some(chrono::Utc::now().timestamp_millis());
}

pub fn record_projection_rebuild(projection: &str, duration: Duration) {
    let mut m = metrics().lock().unwrap();
    let label = projection.to_string();

    // Bounded labels: avoid memory blow-up from unbounded projection names.
    let key = if m.projection_rebuild_duration.len() >= 256
        && !m.projection_rebuild_duration.contains_key(&label)
    {
        "unbounded_overflow".to_string()
    } else {
        label
    };

    m.projection_rebuild_duration
        .entry(key)
        .or_default()
        .record(duration.as_millis() as u64);
}

pub fn get_hot_read_p95(tool: &str) -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.hot_read_latency.get(tool).and_then(|h| h.p95())
}

pub fn get_hot_read_sample_count(tool: &str) -> u64 {
    let m = metrics().lock().unwrap();
    m.hot_read_latency
        .get(tool)
        .map(Histogram::sample_count)
        .unwrap_or(0)
}

pub fn record_mcp_hot_read_error(code: i32) {
    let mut m = metrics().lock().unwrap();
    let key = code.to_string();
    *m.mcp_hot_read_error_total_by_code
        .entry(key.clone())
        .or_default() += 1;
    *m.counters
        .entry("mcp_hot_read_error_total_by_code".to_string())
        .or_default() += 1;
    // Also record by specific code for finer readback if needed
    *m.counters
        .entry(format!("mcp_hot_read_error_total_{}", key))
        .or_default() += 1;
}

pub fn get_hot_read_latest(tool: &str) -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.hot_read_latency.get(tool).and_then(Histogram::latest)
}

pub fn get_mcp_hot_read_error_total(code: Option<i32>) -> u64 {
    let m = metrics().lock().unwrap();
    if let Some(c) = code {
        m.mcp_hot_read_error_total_by_code
            .get(&c.to_string())
            .copied()
            .unwrap_or(0)
    } else {
        m.counters
            .get("mcp_hot_read_error_total_by_code")
            .copied()
            .unwrap_or(0)
    }
}

pub fn get_runs_list_read_latency_p95() -> Option<u64> {
    get_hot_read_p95("runs_list_read_latency_ms")
}

pub fn get_mcp_liveness_gate_duration_p95() -> Option<u64> {
    get_hot_read_p95("mcp_liveness_gate_duration_ms")
}

pub fn get_mcp_liveness_gate_last_recorded_at_ms() -> Option<i64> {
    let m = metrics().lock().unwrap();
    m.mcp_liveness_gate_last_recorded_at_ms
}

pub fn reset_read_path_metrics_for_tests() {
    let mut m = metrics().lock().unwrap();
    m.hot_read_latency.clear();
    m.p058_metric_samples.clear();
    m.mcp_liveness_gate_last_recorded_at_ms = None;
}

pub fn get_projection_rebuild_p95(projection: &str) -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.projection_rebuild_duration
        .get(projection)
        .and_then(|h| h.p95())
}

pub fn record_projection_lag(projection: &str, lag: Duration) {
    let mut m = metrics().lock().unwrap();
    let label = projection.to_string();

    // Bounded labels: avoid memory blow-up from unbounded projection names.
    let key = if m.projection_lag.len() >= 256 && !m.projection_lag.contains_key(&label) {
        "unbounded_overflow".to_string()
    } else {
        label
    };

    m.projection_lag
        .entry(key)
        .or_default()
        .record(lag.as_millis() as u64);
}

pub fn get_projection_lag_p95(projection: &str) -> Option<u64> {
    let m = metrics().lock().unwrap();
    m.projection_lag.get(projection).and_then(|h| h.p95())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal_087_hot_read_latency_p95_is_recorded() {
        record_hot_read_latency("runs.list", Duration::from_millis(10));
        record_hot_read_latency("runs.list", Duration::from_millis(20));
        record_hot_read_latency("runs.list", Duration::from_millis(40));
        record_mcp_liveness_gate_duration(Duration::from_millis(55));

        assert_eq!(get_hot_read_p95("runs.list"), Some(40));
        assert_eq!(get_runs_list_read_latency_p95(), Some(40));
        assert_eq!(get_mcp_liveness_gate_duration_p95(), Some(55));
    }

    #[test]
    fn proposal_087_required_metric_names_are_declared() {
        for metric in [
            "db_writer_alive",
            "db_writer_queue_depth_by_lane",
            "db_writer_write_rejection_total_by_lane",
            "db_writer_write_wait_ms_p95",
            "db_writer_transaction_duration_ms_p95",
            "sqlite_wal_size_bytes",
            "sqlite_checkpoint_failed_total",
            "evidence_spool_bytes_written_total",
            "evidence_spool_orphan_count",
            "projection_lag_count",
            "projection_rebuild_duration_ms",
            "runs_list_read_latency_ms",
            "mcp_liveness_gate_duration_ms",
        ] {
            assert!(
                P087_REQUIRED_METRICS.contains(&metric),
                "missing required P087 metric declaration: {metric}"
            );
        }
    }

    #[test]
    fn proposal_058_required_metric_names_are_declared() {
        for metric in [
            "escalation_chains_started_total",
            "escalation_tier_success_rate",
            "time_to_success_after_escalation_seconds",
            "escalation_pause_total",
            "false_escalation_rate",
            "policy_compile_failure_total",
            "shadow_tier_selection_match_rate",
            "provider_session_kill_latency_seconds",
            "daemon_outage_credit_seconds_total",
            "fan_out_blocked_dwell_seconds",
            "launch_recycle_storm_total",
            "capacity_probe_failure_total",
            "escalation_drift_pending_ack_dwell_seconds",
            "tier_dwell_share_of_chain",
            "chain_exhausted_total_by_terminal_tier_kind",
            "escalation_repeated_digest_no_progress_total",
            "escalation_commit_contention_total",
            "escalation_retry_after_parse_anomaly_total",
            "escalation_provider_late_frame_after_detach_total",
        ] {
            assert!(
                P058_REQUIRED_METRICS.contains(&metric),
                "missing required P058 metric declaration: {metric}"
            );
        }

        let chains_started_before = get_counter("escalation_chains_started_total");
        let tier_success_before = get_counter("escalation_tier_success_rate");
        let tier_success_samples_before =
            get_p058_metric_sample_count("escalation_tier_success_rate");
        let time_to_success_samples_before =
            get_p058_metric_sample_count("time_to_success_after_escalation_seconds");
        let kill_latency_samples_before =
            get_p058_metric_sample_count("provider_session_kill_latency_seconds");
        let pause_before = get_counter("escalation_pause_total:provider_session_force_detached");
        let exhausted_before =
            get_counter("chain_exhausted_total_by_terminal_tier_kind:same_backend_retry");
        let no_progress_before = get_counter("escalation_repeated_digest_no_progress_total");

        record_escalation_chain_started("policy-p058", Some("same_backend_retry"));
        record_escalation_event(
            "escalation.tier_success",
            None,
            None,
            Some(r#"{"metric_numerator": 1, "metric_denominator": 1}"#),
        );
        record_escalation_event(
            "escalation.time_to_success_recorded",
            None,
            None,
            Some(r#"{"metric_sample_ms": 1200}"#),
        );
        record_escalation_event(
            "escalation.provider_force_detach",
            Some("provider_session_force_detached"),
            None,
            Some(r#"{"metric_sample_ms": 30}"#),
        );
        record_escalation_event(
            "escalation.chain_exhausted",
            Some("escalation_repeated_digest_no_progress"),
            Some("same_backend_retry"),
            None,
        );

        assert!(get_counter("escalation_chains_started_total") > chains_started_before);
        assert!(get_counter("escalation_tier_success_rate") > tier_success_before);
        assert!(
            get_p058_metric_sample_count("escalation_tier_success_rate")
                > tier_success_samples_before
        );
        assert!(
            get_p058_metric_sample_count("time_to_success_after_escalation_seconds")
                > time_to_success_samples_before
        );
        assert!(
            get_p058_metric_sample_count("provider_session_kill_latency_seconds")
                > kill_latency_samples_before
        );
        assert!(
            get_counter("escalation_pause_total:provider_session_force_detached") > pause_before
        );
        assert!(
            get_counter("chain_exhausted_total_by_terminal_tier_kind:same_backend_retry")
                > exhausted_before
        );
        assert!(get_counter("escalation_repeated_digest_no_progress_total") > no_progress_before);
    }

    #[test]
    fn proposal_081_required_metric_names_are_declared_and_recordable() {
        for metric in [
            "p081_boundary_policy_enforcement_parity_percent",
            "boundary_policy_decisions_total",
            "boundary_policy_shadow_disagreement_total",
            "auth_ambiguous_caller_warn_total",
            "boundary_no_op_label_total",
            "audit_log_append_failure_total",
            "audit_log_rate_limited_total",
            "operator_alert_native_delivery_total",
            "operator_alert_clear_latency_ms",
            "mcp_command_idempotency_replay_total",
            "mcp_command_idempotency_conflict_total",
            "approval_idempotency_duplicate_total",
            "boundary_policy_evaluation_error_total",
            "approval_actionability_false_total",
            "graphql_redaction_extensions_total",
            "boundary_policy_decision_latency_ms",
            "boundary_commit_transaction_latency_ms",
            "audit_budget_cleanup_duration_ms",
        ] {
            assert!(
                P081_REQUIRED_METRICS.contains(&metric),
                "missing required P081 metric declaration: {metric}"
            );
        }
        record_p081_boundary_decision(
            "graphql_query",
            Some("p081.observer.graphql_query.read_only_opt_in"),
            "observer",
            "graphql.read_only",
            "allow_redacted",
            None,
            "enforce",
        );
        record_p081_boundary_decision_latency(
            "graphql_query",
            "observer",
            "enforce",
            Duration::from_millis(2),
        );
        record_p081_boundary_commit_transaction_latency(
            "graphql_mutation",
            "approve",
            "committed",
            Duration::from_millis(3),
        );
        record_p081_audit_budget_cleanup_duration(Duration::from_millis(4));
        record_p081_operator_alert_clear_latency(
            "p081-boundary-safe-mode-active",
            "critical",
            Duration::from_millis(5),
        );
        record_p081_boundary_policy_enforcement_parity_percent(100);
        record_p081_boundary_shadow_disagreement(
            "graphql_query",
            Some("p081.observer.graphql_query.read_only_opt_in"),
            "observer",
            "graphql.read_only",
            "allow",
            "deny",
            Some("OBSERVER_SCOPE"),
        );
        record_p081_boundary_policy_evaluation_error("graphql_query", "enforce");
        record_p081_approval_actionability_false(
            "observer",
            Some("p081.observer.graphql_mutation.none"),
            "OBSERVER_SCOPE",
        );
        for counter in [
            "auth_ambiguous_caller_warn_total",
            "boundary_no_op_label_total",
            "audit_log_rate_limited_total",
            "mcp_command_idempotency_replay_total",
            "mcp_command_idempotency_conflict_total",
            "approval_idempotency_duplicate_total",
            "graphql_redaction_extensions_total",
        ] {
            increment_counter(counter);
        }
        record_p081_audit_log_append_failure("boundary_decision_deny", "graphql_query", "enforce");
        record_p081_operator_alert_native_delivery("critical", "macos", "delivered");
        assert!(get_counter("boundary_policy_decisions_total") > 0);
        assert!(get_counter("boundary_policy_shadow_disagreement_total") > 0);
        assert!(get_counter("boundary_policy_evaluation_error_total") > 0);
        assert!(get_counter("approval_actionability_false_total") > 0);
        assert_eq!(
            get_hot_read_latest("boundary_policy_decision_latency_ms"),
            Some(2)
        );
        assert_eq!(
            get_hot_read_latest(
                "boundary_policy_decision_latency_ms:transport=graphql_query,caller_class=observer,mode=enforce"
            ),
            Some(2)
        );
        assert_eq!(
            get_hot_read_latest("p081_boundary_policy_enforcement_parity_percent"),
            Some(100)
        );
        assert_eq!(
            get_hot_read_latest("boundary_commit_transaction_latency_ms"),
            Some(3)
        );
        assert_eq!(
            get_hot_read_latest(
                "boundary_commit_transaction_latency_ms:transport=graphql_mutation,action_kind=approve,decision=committed"
            ),
            Some(3)
        );
        assert_eq!(
            get_hot_read_latest("audit_budget_cleanup_duration_ms"),
            Some(4)
        );
        assert_eq!(
            get_hot_read_latest("operator_alert_clear_latency_ms"),
            Some(5)
        );
        assert_eq!(
            get_hot_read_latest(
                "operator_alert_clear_latency_ms:alert_id=p081-boundary-safe-mode-active,severity=critical"
            ),
            Some(5)
        );
        assert!(
            get_counter(
                "audit_log_append_failure_total:event_type=boundary_decision_deny,transport=graphql_query,mode=enforce"
            ) > 0
        );
        assert!(
            get_counter(
                "operator_alert_native_delivery_total:severity=critical,surface=macos,result=delivered"
            ) > 0
        );
    }
}
