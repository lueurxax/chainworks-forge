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

pub fn record_p081_boundary_decision_latency(duration: Duration) {
    let mut m = metrics().lock().unwrap();
    m.hot_read_latency
        .entry("boundary_policy_decision_latency_ms".to_string())
        .or_default()
        .record(duration.as_millis() as u64);
}

pub fn record_p081_boundary_commit_transaction_latency(duration: Duration) {
    let mut m = metrics().lock().unwrap();
    m.hot_read_latency
        .entry("boundary_commit_transaction_latency_ms".to_string())
        .or_default()
        .record(duration.as_millis() as u64);
}

pub fn record_p081_audit_budget_cleanup_duration(duration: Duration) {
    let mut m = metrics().lock().unwrap();
    m.hot_read_latency
        .entry("audit_budget_cleanup_duration_ms".to_string())
        .or_default()
        .record(duration.as_millis() as u64);
}

pub fn record_p081_operator_alert_clear_latency(duration: Duration) {
    let mut m = metrics().lock().unwrap();
    m.hot_read_latency
        .entry("operator_alert_clear_latency_ms".to_string())
        .or_default()
        .record(duration.as_millis() as u64);
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
        record_p081_boundary_decision_latency(Duration::from_millis(2));
        record_p081_boundary_commit_transaction_latency(Duration::from_millis(3));
        record_p081_audit_budget_cleanup_duration(Duration::from_millis(4));
        record_p081_operator_alert_clear_latency(Duration::from_millis(5));
        for counter in [
            "boundary_policy_shadow_disagreement_total",
            "auth_ambiguous_caller_warn_total",
            "boundary_no_op_label_total",
            "audit_log_append_failure_total",
            "audit_log_rate_limited_total",
            "operator_alert_native_delivery_total",
            "mcp_command_idempotency_replay_total",
            "mcp_command_idempotency_conflict_total",
            "approval_idempotency_duplicate_total",
            "boundary_policy_evaluation_error_total",
            "approval_actionability_false_total",
            "graphql_redaction_extensions_total",
        ] {
            increment_counter(counter);
        }
        assert!(get_counter("boundary_policy_decisions_total") > 0);
        assert_eq!(
            get_hot_read_latest("boundary_policy_decision_latency_ms"),
            Some(2)
        );
        assert_eq!(
            get_hot_read_latest("boundary_commit_transaction_latency_ms"),
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
    }
}
