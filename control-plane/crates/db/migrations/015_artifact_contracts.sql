CREATE TABLE IF NOT EXISTS artifact_contract_generations (
  generation_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  artifact_id TEXT NOT NULL,
  contract_id TEXT NOT NULL,
  canonical_path TEXT NOT NULL,
  raw_path TEXT NOT NULL,
  raw_status TEXT NOT NULL,
  canonical_status TEXT NOT NULL,
  source_agent_execution_id TEXT,
  valid INTEGER NOT NULL,
  partial INTEGER NOT NULL DEFAULT 0,
  warnings_json TEXT NOT NULL DEFAULT '[]',
  validation_errors_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS active_artifact_contracts (
  run_id TEXT NOT NULL REFERENCES runs(id),
  contract_id TEXT NOT NULL,
  generation_id TEXT NOT NULL REFERENCES artifact_contract_generations(generation_id),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (run_id, contract_id)
);

CREATE TABLE IF NOT EXISTS run_state_projections (
  run_id TEXT PRIMARY KEY REFERENCES runs(id),
  active_index_json TEXT NOT NULL,
  run_state_json TEXT NOT NULL,
  exported_active_index_path TEXT,
  exported_run_state_path TEXT,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS artifact_contract_overrides (
  override_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  contract_id TEXT NOT NULL,
  override_type TEXT NOT NULL,
  from_status TEXT NOT NULL,
  to_status TEXT NOT NULL,
  reason TEXT NOT NULL,
  owner TEXT NOT NULL,
  source_artifacts_json TEXT NOT NULL DEFAULT '[]',
  expires_at_stage TEXT NOT NULL,
  journal_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  expired_at TEXT,
  active INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_artifact_contract_generations_run ON artifact_contract_generations(run_id);
CREATE INDEX IF NOT EXISTS idx_artifact_contract_overrides_run ON artifact_contract_overrides(run_id);
