-- 031_p017_metric_inventory_and_attempt_attribution.sql
--
-- P017 R4 closure:
-- 1. Extend `workflow_conflict_metric_events` allowed `metric_name` set so all
--    P017 operational metrics committed at proposal §operational_metrics can
--    be emitted without a CHECK violation (closes OPS-002 inventory gap).
-- 2. Make `run_id` nullable so daemon-level Phase C validation failures
--    that happen during workflow compile (before `runs::insert`) can still
--    emit a metric event without an FK violation. Existing rows are
--    backfilled and the index stays the same.
-- 3. Add per-attempt cost/transcript columns on `agent_executions`
--    (closes API-002 cost/transcript gap). Mediation-owned executions
--    populate these so MCP/GraphQL `execution_attempts` can return
--    non-null cost and transcript_ref entries.

-- ── 1+2. Recreate workflow_conflict_metric_events with extended CHECK ─

CREATE TABLE workflow_conflict_metric_events_new (
  event_id TEXT PRIMARY KEY,
  run_id TEXT REFERENCES runs(id),
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
      'invalid_next_stage_hint_non_blocking_total',
      'duplicate_mediation_session_total',
      'report_readback_completeness',
      'phase_c_lead_inventory_external_catalog_total',
      'mediation_late_output_ignored_total',
      'mediation_retry_budget_exhausted_total',
      'workflow_conflict_current_total',
      'terminal_unverifiable_total'
    )
  ),
  labels_json TEXT NOT NULL,
  value REAL NOT NULL,
  unit TEXT NOT NULL CHECK (unit IN ('count', 'seconds', 'ratio')),
  occurred_at TEXT NOT NULL
);

INSERT INTO workflow_conflict_metric_events_new (
  event_id, run_id, conflict_id, metric_name, labels_json, value, unit, occurred_at
)
SELECT event_id, run_id, conflict_id, metric_name, labels_json, value, unit, occurred_at
FROM workflow_conflict_metric_events;

DROP TABLE workflow_conflict_metric_events;
ALTER TABLE workflow_conflict_metric_events_new RENAME TO workflow_conflict_metric_events;

CREATE INDEX IF NOT EXISTS idx_workflow_conflict_metric_events_run_id
  ON workflow_conflict_metric_events(run_id);
CREATE INDEX IF NOT EXISTS idx_workflow_conflict_metric_events_conflict_id
  ON workflow_conflict_metric_events(conflict_id);
CREATE INDEX IF NOT EXISTS idx_workflow_conflict_metric_events_name
  ON workflow_conflict_metric_events(metric_name);

-- ── 3. Per-attempt cost/transcript on agent_executions ───────────────

ALTER TABLE agent_executions ADD COLUMN total_cost_cents INTEGER;
ALTER TABLE agent_executions ADD COLUMN input_tokens INTEGER;
ALTER TABLE agent_executions ADD COLUMN output_tokens INTEGER;
ALTER TABLE agent_executions ADD COLUMN cached_input_tokens INTEGER;
ALTER TABLE agent_executions ADD COLUMN transcript_artifact_id TEXT REFERENCES artifacts(id);

CREATE INDEX IF NOT EXISTS idx_agent_executions_transcript_artifact_id
  ON agent_executions(transcript_artifact_id);
