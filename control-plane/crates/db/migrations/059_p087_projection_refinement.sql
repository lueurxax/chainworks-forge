-- P087: Run summary projection refinements and new health projections

-- 1. Refine run_summaries to include all fields needed for projection-only lists
ALTER TABLE run_summaries ADD COLUMN workflow_id TEXT;
ALTER TABLE run_summaries ADD COLUMN workspace_root TEXT;
ALTER TABLE run_summaries ADD COLUMN artifact_root TEXT;
ALTER TABLE run_summaries ADD COLUMN completed_at TEXT;
ALTER TABLE run_summaries ADD COLUMN cancellation_requested_at TEXT;
ALTER TABLE run_summaries ADD COLUMN cancellation_settled_at TEXT;
ALTER TABLE run_summaries ADD COLUMN chainworks_meta_root TEXT;

-- 2. Artifact noise summary projection
CREATE TABLE IF NOT EXISTS artifact_noise_summary (
  run_id TEXT PRIMARY KEY REFERENCES runs(id),
  artifact_count INTEGER NOT NULL DEFAULT 0,
  superseded_count INTEGER NOT NULL DEFAULT 0,
  duplicate_candidate_count INTEGER NOT NULL DEFAULT 0,
  archive_eligible_count INTEGER NOT NULL DEFAULT 0,
  updated_at_ms INTEGER NOT NULL
);

-- 3. Runtime health summary projection (global single-row)
CREATE TABLE IF NOT EXISTS runtime_health_summary (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  active_sessions INTEGER NOT NULL DEFAULT 0,
  open_hot_read_circuits INTEGER NOT NULL DEFAULT 0,
  side_effect_unresolved_count INTEGER NOT NULL DEFAULT 0,
  continuation_active_count INTEGER NOT NULL DEFAULT 0,
  runtime_families_json TEXT NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

-- Initialize the single row for runtime health
INSERT OR IGNORE INTO runtime_health_summary (id, runtime_families_json, updated_at_ms)
VALUES (1, '[]', strftime('%s', 'now') * 1000);
