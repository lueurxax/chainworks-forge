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
    p058_metric_samples: HashMap<String, Histogram>,
    mcp_liveness_gate_last_recorded_at_ms: Option<i64>,
    mcp_hot_read_error_total_by_code: HashMap<String, u64>,
}

static METRICS: OnceLock<Mutex<SystemMetrics>> = OnceLock::new();

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

pub fn record_escalation_chain_started(policy_id: &str, tier_kind: Option<&str>) {
    increment_counter_with_label("escalation_chains_started_total", policy_id);
    if let Some(kind) = tier_kind {
        increment_counter_with_label("escalation_chains_started_total_by_tier_kind", kind);
    }
}

pub fn record_escalation_event(
    event_kind: &str,
    tier_kind: Option<&str>,
    pause_reason: Option<&str>,
    payload_json: Option<&str>,
) {
    let payload = payload_json.and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
    match event_kind {
        "escalation.tier_succeeded" => {
            increment_counter_with_label(
                "escalation_tier_success_rate",
                tier_kind.unwrap_or("unknown"),
            );
            record_p058_rate_from_payload("escalation_tier_success_rate", payload.as_ref());
        }
        "escalation.false_positive" => {
            increment_counter("false_escalation_rate");
            record_p058_rate_from_payload("false_escalation_rate", payload.as_ref());
        }
        "escalation.policy_compile_failed" => increment_counter("policy_compile_failure_total"),
        "escalation.shadow_selection_matched" => {
            increment_counter("shadow_tier_selection_match_rate");
            record_p058_rate_from_payload("shadow_tier_selection_match_rate", payload.as_ref());
        }
        "escalation.provider_session_force_detached" => {
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
        "escalation.fan_out_blocked" => {
            increment_counter("fan_out_blocked_dwell_seconds");
            record_p058_duration_from_payload("fan_out_blocked_dwell_seconds", payload.as_ref());
        }
        "escalation.launch_recycle_storm" => increment_counter("launch_recycle_storm_total"),
        "escalation.capacity_probe_failed" => increment_counter("capacity_probe_failure_total"),
        "escalation.policy_drift_pending_ack" => {
            increment_counter("escalation_drift_pending_ack_dwell_seconds");
            record_p058_duration_from_payload(
                "escalation_drift_pending_ack_dwell_seconds",
                payload.as_ref(),
            );
        }
        "escalation.tier_dwell_recorded" => {
            increment_counter("tier_dwell_share_of_chain");
            record_p058_rate_from_payload("tier_dwell_share_of_chain", payload.as_ref());
        }
        "escalation.chain_exhausted" => increment_counter_with_label(
            "chain_exhausted_total_by_terminal_tier_kind",
            tier_kind.unwrap_or("unknown"),
        ),
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

    if pause_reason.is_some() {
        increment_counter_with_label("escalation_pause_total", pause_reason.unwrap());
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
            let basis_points = numerator.saturating_mul(10_000) / denominator;
            record_p058_metric_sample(metric_name, basis_points.min(10_000));
        }
    }
}

pub fn get_counter(name: &str) -> u64 {
    let m = metrics().lock().unwrap();
    m.counters.get(name).copied().unwrap_or(0)
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
    }

    #[test]
    fn proposal_058_metric_samples_are_recorded_from_event_payloads() {
        reset_read_path_metrics_for_tests();
        record_escalation_event(
            "escalation.provider_session_force_detached",
            Some("backend_profile"),
            Some("provider_session_force_detached"),
            Some(r#"{"metric_sample_ms":4200}"#),
        );
        record_escalation_event(
            "escalation.tier_succeeded",
            Some("backend_profile"),
            None,
            Some(r#"{"metric_numerator":3,"metric_denominator":4}"#),
        );

        assert_eq!(
            get_p058_metric_latest("provider_session_kill_latency_seconds"),
            Some(4200)
        );
        assert_eq!(
            get_p058_metric_p95("provider_session_kill_latency_seconds"),
            Some(4200)
        );
        assert_eq!(
            get_p058_metric_latest("escalation_tier_success_rate"),
            Some(7500)
        );
        assert_eq!(
            get_p058_metric_sample_count("escalation_tier_success_rate"),
            1
        );
    }
}
