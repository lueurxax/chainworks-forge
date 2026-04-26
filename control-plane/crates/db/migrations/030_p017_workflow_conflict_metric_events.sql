CREATE TABLE IF NOT EXISTS workflow_conflict_metric_events (
  event_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  conflict_id TEXT,
  metric_name TEXT NOT NULL CHECK (
    metric_name IN (
      'workflow_conflict_time_to_resolution_seconds',
      'conflict_reason_to_action_outcome_total',
      'recovery_action_chosen_total',
      'phase_c_validation_outcome_total',
      'external_catalog_warning_total',
      'phase_b_dogfood_mediation_completion_rate',
      'phase_b_dogfood_operator_guidance_sufficient_total',
      'lead_mediation_attempt_total',
      'advisory_rejection_total',
      'invalid_next_stage_hint_non_blocking_total'
    )
  ),
  labels_json TEXT NOT NULL,
  value REAL NOT NULL,
  unit TEXT NOT NULL CHECK (unit IN ('count', 'seconds', 'ratio')),
  occurred_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workflow_conflict_metric_events_run_id
  ON workflow_conflict_metric_events(run_id);
CREATE INDEX IF NOT EXISTS idx_workflow_conflict_metric_events_conflict_id
  ON workflow_conflict_metric_events(conflict_id);
CREATE INDEX IF NOT EXISTS idx_workflow_conflict_metric_events_name
  ON workflow_conflict_metric_events(metric_name);
