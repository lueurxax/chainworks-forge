-- Proposal 049: Steward analysis run ownership and persistence.

ALTER TABLE ideas ADD COLUMN project_key TEXT;

ALTER TABLE runs ADD COLUMN workflow_family TEXT;
ALTER TABLE runs ADD COLUMN project_key TEXT;
ALTER TABLE runs ADD COLUMN risk_class TEXT;
ALTER TABLE runs ADD COLUMN stack TEXT;
ALTER TABLE runs ADD COLUMN workflow_snapshot_hash TEXT;
ALTER TABLE runs ADD COLUMN catalog_snapshot_hash TEXT;
ALTER TABLE runs ADD COLUMN workflow_snapshot_json TEXT;
ALTER TABLE runs ADD COLUMN catalog_snapshot_json TEXT;
ALTER TABLE runs ADD COLUMN drift_detected_at TEXT;
ALTER TABLE runs ADD COLUMN drift_details_json TEXT;

ALTER TABLE stage_executions ADD COLUMN retry_reason TEXT;

CREATE TABLE IF NOT EXISTS steward_analyses (
  id TEXT PRIMARY KEY,
  created_at TEXT NOT NULL,
  window_start TEXT NOT NULL,
  window_end TEXT NOT NULL,
  run_count INTEGER NOT NULL,
  cohort_keys_json TEXT NOT NULL,
  cohort_quality TEXT NOT NULL,
  status TEXT NOT NULL,
  degradation_count INTEGER NOT NULL DEFAULT 0,
  improvement_count INTEGER NOT NULL DEFAULT 0,
  workflow_snapshot_artifact_hash TEXT NOT NULL,
  agent_catalog_snapshot_hash TEXT NOT NULL,
  steward_config_snapshot_hash TEXT NOT NULL,
  metrics_snapshot_artifact_id TEXT,
  baseline_snapshot_artifact_id TEXT,
  agent_catalog_snapshot_artifact_id TEXT,
  workflow_snapshot_artifact_id TEXT,
  config_change_log_artifact_id TEXT,
  health_report_artifact_id TEXT,
  degradation_alert_artifact_id TEXT,
  agent_tuning_artifact_id TEXT,
  workflow_tuning_artifact_id TEXT,
  experiment_plan_artifact_id TEXT,
  audit_report_artifact_id TEXT,
  trigger_reason TEXT NOT NULL,
  error_summary TEXT
);

CREATE TABLE IF NOT EXISTS steward_analysis_run_links (
  id TEXT PRIMARY KEY,
  analysis_id TEXT NOT NULL REFERENCES steward_analyses(id),
  run_id TEXT NOT NULL REFERENCES runs(id),
  role TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS steward_recommendations (
  id TEXT PRIMARY KEY,
  analysis_id TEXT NOT NULL REFERENCES steward_analyses(id),
  created_at TEXT NOT NULL,
  category TEXT NOT NULL,
  summary TEXT NOT NULL,
  target_metric TEXT NOT NULL,
  confidence_level TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'proposed',
  source_artifact_name TEXT,
  decision_comment TEXT,
  decided_at TEXT
);

CREATE TABLE IF NOT EXISTS steward_runtime_state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_runs_steward_cohort
  ON runs(workflow_family, risk_class, completed_at);
CREATE INDEX IF NOT EXISTS idx_steward_analyses_created_at
  ON steward_analyses(created_at);
CREATE INDEX IF NOT EXISTS idx_steward_analyses_status
  ON steward_analyses(status);
CREATE INDEX IF NOT EXISTS idx_steward_analysis_run_links_run_id
  ON steward_analysis_run_links(run_id);
CREATE INDEX IF NOT EXISTS idx_steward_recommendations_analysis_id
  ON steward_recommendations(analysis_id);
