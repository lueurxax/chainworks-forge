-- P073: Stability budget snapshot storage.
-- One authoritative durable snapshot family owned by the Rust control plane.
-- Each row represents one metric reading within a named snapshot.
-- The latest snapshot is identified by the highest captured_at for each snapshot_id.

CREATE TABLE IF NOT EXISTS stability_budget_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    snapshot_id TEXT NOT NULL,
    captured_at TEXT NOT NULL,
    phase TEXT NOT NULL,
    metric_id TEXT NOT NULL,
    metric_classification TEXT NOT NULL,
    blocking_mode TEXT NOT NULL,
    measurement_status TEXT NOT NULL,
    current_value REAL,
    baseline_value REAL,
    target_threshold TEXT NOT NULL,
    latest_by_instrumentation_date TEXT,
    missing_data_policy TEXT NOT NULL,
    notes TEXT NOT NULL DEFAULT ''
);

-- Fast lookup of the latest snapshot (all metrics for a given snapshot_id).
CREATE INDEX IF NOT EXISTS idx_stability_budget_snapshot_id
    ON stability_budget_snapshots(snapshot_id);

-- Chronological ordering for latest-snapshot queries.
CREATE INDEX IF NOT EXISTS idx_stability_budget_captured_at
    ON stability_budget_snapshots(captured_at);

-- Unique constraint: one row per metric per snapshot.
CREATE UNIQUE INDEX IF NOT EXISTS idx_stability_budget_snapshot_metric
    ON stability_budget_snapshots(snapshot_id, metric_id);
