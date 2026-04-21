CREATE TABLE IF NOT EXISTS artifact_contract_summaries (
  artifact_id TEXT PRIMARY KEY REFERENCES artifacts(id),
  run_id TEXT NOT NULL REFERENCES runs(id),
  contract_id TEXT NOT NULL,
  artifact_path TEXT NOT NULL,
  source_contract_id TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  source_precedence INTEGER NOT NULL,
  status TEXT NOT NULL,
  implementation_complete INTEGER,
  verification_green INTEGER,
  remaining_code_task_count INTEGER,
  blocking_remaining_code_task_count INTEGER,
  handoff_task_count INTEGER,
  blocking_review_handoff_task_count INTEGER,
  owner_class_counts_json TEXT NOT NULL,
  target_stage_summaries_json TEXT NOT NULL,
  remaining_code_tasks_json TEXT NOT NULL,
  handoff_tasks_json TEXT NOT NULL,
  known_risks_json TEXT NOT NULL,
  tests_run_json TEXT NOT NULL,
  docs_impacted_json TEXT NOT NULL,
  validation_errors_json TEXT NOT NULL,
  warnings_json TEXT NOT NULL,
  raw_artifact_available INTEGER NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artifact_contract_summaries_run_contract
  ON artifact_contract_summaries(run_id, contract_id);

CREATE INDEX IF NOT EXISTS idx_artifact_contract_summaries_status
  ON artifact_contract_summaries(status);

CREATE UNIQUE INDEX IF NOT EXISTS idx_artifact_contract_summaries_one_active
  ON artifact_contract_summaries(run_id, contract_id)
  WHERE is_active = 1;
