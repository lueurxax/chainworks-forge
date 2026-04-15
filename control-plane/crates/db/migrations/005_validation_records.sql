CREATE TABLE IF NOT EXISTS validation_failure_records (
  id TEXT PRIMARY KEY,
  artifact_id TEXT NOT NULL REFERENCES artifacts(id),
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
  agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
  timestamp TEXT NOT NULL,
  failure_class TEXT NOT NULL,
  failure_summary TEXT NOT NULL,
  record_json TEXT NOT NULL,
  recovery_action TEXT
);

CREATE INDEX IF NOT EXISTS idx_vfr_run_id ON validation_failure_records(run_id);
CREATE INDEX IF NOT EXISTS idx_vfr_run_stage_execution ON validation_failure_records(run_id, stage_execution_id);
CREATE INDEX IF NOT EXISTS idx_vfr_run_stage_agent_execution ON validation_failure_records(run_id, stage_execution_id, agent_execution_id);
CREATE INDEX IF NOT EXISTS idx_vfr_artifact_id ON validation_failure_records(artifact_id);

ALTER TABLE stage_summaries ADD COLUMN has_validation_failure INTEGER NOT NULL DEFAULT 0;
