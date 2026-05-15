ALTER TABLE stage_executions ADD COLUMN terminal_reason TEXT;
ALTER TABLE stage_summaries ADD COLUMN terminal_reason TEXT;
ALTER TABLE stage_summaries ADD COLUMN retry_authority_id TEXT;
ALTER TABLE stage_summaries ADD COLUMN is_retry_authoritative INTEGER NOT NULL DEFAULT 0;
ALTER TABLE stage_summaries ADD COLUMN retry_authority_state TEXT;

CREATE TABLE IF NOT EXISTS retry_stage_execution_authorities (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_id TEXT NOT NULL,
  target_stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
  entry_kind TEXT NOT NULL,
  source_command_journal_id TEXT,
  source_retry_work_item_id TEXT,
  source_invoke_work_item_id TEXT,
  source_agent_execution_id TEXT,
  authority_state TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  terminal_reason TEXT
);

CREATE UNIQUE INDEX retry_stage_execution_authorities_one_active
  ON retry_stage_execution_authorities(run_id, stage_id)
  WHERE authority_state = 'active';

CREATE INDEX retry_stage_execution_authorities_target
  ON retry_stage_execution_authorities(target_stage_execution_id);

CREATE INDEX retry_stage_execution_authorities_run
  ON retry_stage_execution_authorities(run_id, stage_id, authority_state);

CREATE TABLE IF NOT EXISTS p091_orphan_repair_passes (
  id TEXT PRIMARY KEY,
  mode TEXT NOT NULL,
  disabled INTEGER NOT NULL DEFAULT 0,
  run_id TEXT,
  candidates_total INTEGER NOT NULL DEFAULT 0,
  excluded_total INTEGER NOT NULL DEFAULT 0,
  would_repair_total INTEGER NOT NULL DEFAULT 0,
  repaired_total INTEGER NOT NULL DEFAULT 0,
  disabled_total INTEGER NOT NULL DEFAULT 0,
  bounded_samples_json TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX p091_orphan_repair_passes_run
  ON p091_orphan_repair_passes(run_id, created_at);
