/// P073: GraphQL types for the stability budget readback surface.
use async_graphql::*;

/// GraphQL representation of a single stability metric row from the latest snapshot.
#[derive(Clone, Debug, SimpleObject)]
pub struct GqlStabilityBudgetRow {
    pub snapshot_id: String,
    pub captured_at: String,
    pub phase: String,
    pub metric_id: String,
    pub metric_classification: String,
    pub blocking_mode: String,
    pub measurement_status: String,
    pub current_value: Option<f64>,
    pub baseline_value: Option<f64>,
    pub target_threshold: String,
    pub latest_by_instrumentation_date: Option<String>,
    pub missing_data_policy: String,
    pub notes: String,
}

/// The stability budget response — the full set of metric rows from the latest snapshot.
#[derive(Clone, Debug, SimpleObject)]
pub struct GqlStabilityBudget {
    /// The named snapshot writer (always "control-plane-stability-budget-materializer").
    pub snapshot_writer: String,
    /// All metric rows in the latest durable snapshot.
    pub metrics: Vec<GqlStabilityBudgetRow>,
}
