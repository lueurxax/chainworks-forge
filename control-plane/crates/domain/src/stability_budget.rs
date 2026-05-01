/// P073: Stability budget v1 domain types.
///
/// The Rust control plane is the sole authoritative owner of stability_budget.v1.
/// GraphQL, MCP, Steward, and SwiftUI consume the latest durable snapshot only
/// and may not compute a competing authoritative budget.
use serde::{Deserialize, Serialize};

/// Classification of how a stability metric is measured.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricClassification {
    /// Derived from control-plane owned data.
    Derived,
    /// Measured directly by server-native instrumentation.
    ServerNative,
    /// Observed on the client side; not a blocking gate source without an audited ingest.
    ClientObserved,
}

/// Whether a metric blocks gate progression.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlockingMode {
    /// Blocks gate exit unconditionally.
    Blocking,
    /// Blocks only after a named condition is met (e.g. instrumentation landing).
    BlockingAfterCondition,
    /// Advisory only for the entire P073 window.
    Advisory,
    /// Advisory until the P038 compaction seam is implemented.
    AdvisoryUntilP038,
}

/// Measurement presence status.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementStatus {
    /// A value is present and current.
    Present,
    /// Data is absent. The missing_data_policy determines whether this blocks.
    Missing,
    /// Measurement is stale beyond the instrumentation deadline.
    Stale,
}

/// A single metric row within a stability budget snapshot.
/// All 14 normalized DTO fields from the P073 contract are present.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StabilityBudgetRow {
    /// Unique row id (uuid).
    pub id: String,
    /// Groups all metric rows captured together into one snapshot.
    pub snapshot_id: String,
    /// ISO-8601 timestamp when the snapshot was captured.
    pub captured_at: String,
    /// P073 phase label (e.g. "phase_0", "phase_1").
    pub phase: String,
    /// Metric identifier (e.g. "SB-01").
    pub metric_id: String,
    pub metric_classification: MetricClassification,
    pub blocking_mode: BlockingMode,
    pub measurement_status: MeasurementStatus,
    /// Current measured value; None when measurement is absent.
    pub current_value: Option<f64>,
    /// Baseline value from the first durable snapshot; None before baseline capture.
    pub baseline_value: Option<f64>,
    /// Human-readable threshold description.
    pub target_threshold: String,
    /// ISO-8601 date by which instrumentation must exist; None for client-observed metrics.
    pub latest_by_instrumentation_date: Option<String>,
    /// Policy for how missing data is treated.
    pub missing_data_policy: String,
    pub notes: String,
}

/// The twelve P073 stability metrics.
pub const METRIC_IDS: &[&str] = &[
    "SB-01", "SB-02", "SB-03", "SB-04", "SB-05", "SB-06",
    "SB-07", "SB-08", "SB-09", "SB-10", "SB-11", "SB-12",
];

/// P073 snapshot writer identity — the single authoritative materializer.
pub const SNAPSHOT_WRITER: &str = "control-plane-stability-budget-materializer";
