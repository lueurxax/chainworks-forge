CREATE TABLE IF NOT EXISTS agent_execution_discovery_diagnostics (
  agent_execution_id TEXT PRIMARY KEY REFERENCES agent_executions(id),
  discovery_diagnostics_json TEXT NOT NULL,
  discovery_schema_version TEXT NOT NULL,
  legacy_broad_discovery_used INTEGER NOT NULL DEFAULT 0,
  missing_required_output_count INTEGER NOT NULL DEFAULT 0,
  rejected_output_count INTEGER NOT NULL DEFAULT 0,
  stale_output_count INTEGER NOT NULL DEFAULT 0,
  meta_discovery_truncated INTEGER NOT NULL DEFAULT 0,
  git_manifest_status TEXT,
  resume_warning_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_discovery_diag_schema
  ON agent_execution_discovery_diagnostics(discovery_schema_version);

CREATE INDEX IF NOT EXISTS idx_agent_discovery_diag_legacy_used
  ON agent_execution_discovery_diagnostics(legacy_broad_discovery_used);

CREATE INDEX IF NOT EXISTS idx_agent_discovery_diag_output_counts
  ON agent_execution_discovery_diagnostics(missing_required_output_count, rejected_output_count);

CREATE INDEX IF NOT EXISTS idx_agent_discovery_diag_meta_truncated
  ON agent_execution_discovery_diagnostics(meta_discovery_truncated);

CREATE TABLE IF NOT EXISTS legacy_discovery_overrides (
  override_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_id TEXT NOT NULL,
  workflow_id TEXT NOT NULL,
  target_stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
  target_attempt_number INTEGER NOT NULL,
  actor_id TEXT NOT NULL,
  reason TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expires_at_attempt INTEGER NOT NULL,
  requested_policy TEXT NOT NULL CHECK (requested_policy IN ('workflow_opt_in')),
  from_policy TEXT NOT NULL CHECK (from_policy IN ('disabled', 'workflow_opt_in')),
  approval_source TEXT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  consumed_at TEXT,
  status TEXT NOT NULL CHECK (status IN ('pending', 'consumed', 'rejected', 'expired')),
  journal_id TEXT NOT NULL REFERENCES command_journal(id)
);

CREATE INDEX IF NOT EXISTS idx_legacy_discovery_overrides_run
  ON legacy_discovery_overrides(run_id, created_at);

CREATE INDEX IF NOT EXISTS idx_legacy_discovery_overrides_target
  ON legacy_discovery_overrides(target_stage_execution_id, target_attempt_number, status);
