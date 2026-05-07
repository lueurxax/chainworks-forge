-- P084: Authoritative rollout contract check storage.
--
-- Stores run-start rollout contract preflight results with hash binding,
-- enforcement mode, waiver state, and projection integrity tracking.
-- Projection is emitted atomically to .chainworks/runs/<run-id>/readiness/rollout-contract-check.json.
--
-- The scheduler trusts this table, not the projection file.
-- Missing, stale, hash-mismatched, tamper-suspect, partial, timed-out,
-- or cancelled records hold work enqueue under enforce mode.

CREATE TABLE IF NOT EXISTS rollout_contract_checks (
  id                         TEXT    PRIMARY KEY,
  run_id                     TEXT    NOT NULL,
  proposal_id                TEXT    NOT NULL,
  proposal_revision_id       TEXT    NOT NULL,
  proposal_content_hash      TEXT    NOT NULL,
  contract_object_hash       TEXT    NOT NULL,
  content_snapshot_id        TEXT    NOT NULL,
  checker_version            TEXT    NOT NULL,
  -- status: pass | fail | waived | not_applicable | timeout | cancelled | missing_contract | tamper_detected | stale
  status                     TEXT    NOT NULL,
  -- decision: release | hold | waive | not_applicable
  decision                   TEXT    NOT NULL,
  -- lifecycle_state: running | terminal | partial
  lifecycle_state            TEXT    NOT NULL,
  -- enforcement_mode: enforce | permissive | disabled
  enforcement_mode           TEXT    NOT NULL,
  failure_reasons_json       TEXT    NOT NULL DEFAULT '[]',
  diagnostics_json           TEXT    NOT NULL DEFAULT '[]',
  waiver_json                TEXT,
  -- projection_integrity: valid | tamper_detected | stale
  projection_integrity       TEXT    NOT NULL DEFAULT 'valid',
  cutover_policy_revision    TEXT,
  -- redaction_state: none | partial | full
  redaction_state            TEXT    NOT NULL DEFAULT 'none'
                                CHECK (redaction_state IN ('none', 'partial', 'full')),
  retry_count                INTEGER NOT NULL DEFAULT 0,
  preflight_timeout_seconds  INTEGER NOT NULL DEFAULT 45,
  created_at                 TEXT    NOT NULL,
  updated_at                 TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_rollout_contract_checks_run_id
  ON rollout_contract_checks(run_id);

CREATE INDEX IF NOT EXISTS idx_rollout_contract_checks_status_lifecycle
  ON rollout_contract_checks(run_id, status, lifecycle_state);

CREATE TABLE IF NOT EXISTS rollout_contract_metric_events (
  event_id                   TEXT PRIMARY KEY,
  run_id                     TEXT NOT NULL,
  rollout_contract_check_id  TEXT NOT NULL,
  metric_name                TEXT NOT NULL CHECK (
    metric_name IN (
      'rollout_contract_lint_total',
      'rollout_contract_run_start_block_total',
      'rollout_contract_waiver_total',
      'late_rollout_evidence_followup_total',
      'rollout_contract_enforcement_mode_total',
      'rollout_contract_partial_write_recovered_total',
      'rollout_contract_hash_drift_total',
      'rollout_contract_preflight_cancelled_total',
      'rollout_contract_retry_exhausted_total',
      'rollout_contract_permissive_dogfood_total',
      'rollout_contract_tamper_or_stale_projection_total'
    )
  ),
  labels_json                TEXT NOT NULL,
  value                      REAL NOT NULL,
  unit                       TEXT NOT NULL CHECK (unit IN ('count', 'seconds', 'ratio')),
  occurred_at                TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_rollout_contract_metric_events_run_id
  ON rollout_contract_metric_events(run_id);

CREATE INDEX IF NOT EXISTS idx_rollout_contract_metric_events_check_id
  ON rollout_contract_metric_events(rollout_contract_check_id);

CREATE INDEX IF NOT EXISTS idx_rollout_contract_metric_events_name
  ON rollout_contract_metric_events(metric_name);
