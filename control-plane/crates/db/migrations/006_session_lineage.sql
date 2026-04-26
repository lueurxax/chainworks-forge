ALTER TABLE session_lineages RENAME TO session_lineages_legacy;

CREATE TABLE session_lineages (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  agent_id TEXT NOT NULL,
  lineage_id TEXT NOT NULL,
  session_reuse_scope TEXT NOT NULL,
  session_family_id TEXT,
  active_generation_id TEXT,
  created_at TEXT NOT NULL,
  closed_at TEXT
);

CREATE TABLE session_generations (
  id TEXT PRIMARY KEY,
  lineage_id TEXT NOT NULL REFERENCES session_lineages(id),
  generation INTEGER NOT NULL,
  invocation_owner_key TEXT NOT NULL,
  provider_session_id TEXT,
  binding_fingerprint TEXT NOT NULL,
  rehydrated_from_checkpoint_artifact_id TEXT,
  working_directory TEXT NOT NULL,
  workspace_mode TEXT NOT NULL,
  runtime_provider TEXT NOT NULL,
  runtime_model TEXT NOT NULL,
  status TEXT NOT NULL,
  turn_count INTEGER NOT NULL DEFAULT 0,
  cumulative_prompt_tokens INTEGER NOT NULL DEFAULT 0,
  cumulative_cost_cents INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  ended_at TEXT,
  end_reason TEXT
);

CREATE TABLE session_events (
  id TEXT PRIMARY KEY,
  lineage_id TEXT NOT NULL REFERENCES session_lineages(id),
  generation_id TEXT NOT NULL REFERENCES session_generations(id),
  event_type TEXT NOT NULL,
  recorded_at TEXT NOT NULL,
  details_json TEXT
);

ALTER TABLE runs ADD COLUMN cancellation_settlement_log TEXT;
ALTER TABLE run_summaries ADD COLUMN cancellation_settlement_summary TEXT;

ALTER TABLE agent_executions ADD COLUMN session_lineage_id TEXT;
ALTER TABLE agent_executions ADD COLUMN session_generation_id TEXT;
ALTER TABLE agent_executions ADD COLUMN rehydrated_from_checkpoint_artifact_id TEXT;
ALTER TABLE agent_executions ADD COLUMN invocation_owner_key TEXT;
ALTER TABLE agent_executions ADD COLUMN session_reuse_scope TEXT;
ALTER TABLE agent_executions ADD COLUMN session_family_id TEXT;
ALTER TABLE agent_executions ADD COLUMN session_reuse_disposition TEXT;
ALTER TABLE agent_executions ADD COLUMN session_reset_reason TEXT;

CREATE INDEX idx_session_lineages_run_id ON session_lineages(run_id);
CREATE INDEX idx_session_generations_lineage_id ON session_generations(lineage_id);
CREATE INDEX idx_session_events_lineage_id ON session_events(lineage_id);
