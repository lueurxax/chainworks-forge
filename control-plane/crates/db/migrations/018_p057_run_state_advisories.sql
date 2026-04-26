CREATE TABLE IF NOT EXISTS artifact_contract_advisories (
  advisory_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  artifact_id TEXT NOT NULL,
  contract_id TEXT NOT NULL,
  advisory_path TEXT NOT NULL,
  advisory_kind TEXT NOT NULL,
  superseded_by TEXT NOT NULL,
  source_agent_execution_id TEXT,
  source_stage_execution_id TEXT,
  source_session_generation_id TEXT,
  source_work_item_id TEXT,
  warnings_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artifact_contract_advisories_run ON artifact_contract_advisories(run_id);
