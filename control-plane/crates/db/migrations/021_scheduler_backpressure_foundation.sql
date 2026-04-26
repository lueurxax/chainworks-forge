-- P061 provider normalization, scheduler hot indexes, queue projections, and host interruption facts.

ALTER TABLE agent_executions ADD COLUMN provider_family TEXT;

UPDATE agent_executions
SET provider = CASE lower(trim(provider))
  WHEN 'claude' THEN 'claude'
  WHEN 'claude_acp' THEN 'claude'
  WHEN 'claude_agent' THEN 'claude'
  WHEN 'claude_agent_acp' THEN 'claude'
  WHEN 'gemini' THEN 'gemini'
  WHEN 'gemini_acp' THEN 'gemini'
  WHEN 'gemini_cli' THEN 'gemini'
  WHEN 'gemini_cli_acp' THEN 'gemini'
  WHEN 'codex' THEN 'codex'
  WHEN 'codex_acp' THEN 'codex'
  WHEN 'codex_cli' THEN 'codex'
  WHEN 'codex_cli_acp' THEN 'codex'
  WHEN 'openai_codex' THEN 'codex'
  WHEN 'auggie' THEN 'auggie'
  WHEN 'auggie_acp' THEN 'auggie'
  WHEN 'junie' THEN 'junie'
  WHEN 'junie_acp' THEN 'junie'
  ELSE provider
END;

UPDATE agent_executions
SET provider_family = CASE lower(trim(provider))
  WHEN 'claude' THEN 'claude'
  WHEN 'gemini' THEN 'gemini'
  WHEN 'codex' THEN 'codex'
  WHEN 'auggie' THEN 'auggie'
  WHEN 'junie' THEN 'junie'
  ELSE NULL
END;

CREATE INDEX IF NOT EXISTS idx_work_items_kind_status_scheduled_at
  ON work_items(kind, status, scheduled_at);

CREATE INDEX IF NOT EXISTS idx_work_items_run_status_kind_scheduled_at
  ON work_items(run_id, status, kind, scheduled_at);

CREATE INDEX IF NOT EXISTS idx_agent_executions_status_provider_family
  ON agent_executions(status, provider_family);

CREATE INDEX IF NOT EXISTS idx_agent_executions_status
  ON agent_executions(status);

CREATE INDEX IF NOT EXISTS idx_stage_executions_run_id_id
  ON stage_executions(run_id, id);

CREATE TABLE IF NOT EXISTS scheduler_service_state (
  scope TEXT NOT NULL,
  scope_id TEXT NOT NULL DEFAULT '',
  last_served_at TEXT,
  last_claimed_work_item_id TEXT,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (scope, scope_id)
);

CREATE TABLE IF NOT EXISTS scheduler_queue_summaries (
  scope TEXT NOT NULL,
  scope_id TEXT NOT NULL DEFAULT '',
  run_id TEXT,
  stage_execution_id TEXT,
  provider_family TEXT NOT NULL DEFAULT '',
  top_reason TEXT NOT NULL,
  queued_count INTEGER NOT NULL DEFAULT 0,
  oldest_queued_at TEXT,
  oldest_queued_age_ms INTEGER NOT NULL DEFAULT 0,
  global_queue_depth INTEGER NOT NULL DEFAULT 0,
  stale_after_ms INTEGER NOT NULL DEFAULT 60000,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (scope, scope_id, provider_family, top_reason)
);

CREATE INDEX IF NOT EXISTS idx_scheduler_queue_summaries_run
  ON scheduler_queue_summaries(run_id, top_reason);

CREATE INDEX IF NOT EXISTS idx_scheduler_queue_summaries_stage
  ON scheduler_queue_summaries(stage_execution_id, top_reason);

CREATE TABLE IF NOT EXISTS scheduler_health_snapshots (
  id TEXT PRIMARY KEY,
  queued_count INTEGER NOT NULL DEFAULT 0,
  oldest_queued_age_ms INTEGER NOT NULL DEFAULT 0,
  global_queue_depth INTEGER NOT NULL DEFAULT 0,
  active_agent_executions INTEGER NOT NULL DEFAULT 0,
  db_writer_wait_p95_ms INTEGER,
  command_latency_p95_ms_json TEXT,
  last_host_interruption_epoch_id TEXT,
  sustained_backpressure_state TEXT NOT NULL DEFAULT 'clear',
  stale_after_ms INTEGER NOT NULL DEFAULT 60000,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_scheduler_health_snapshots_updated_at
  ON scheduler_health_snapshots(updated_at);

CREATE TABLE IF NOT EXISTS scheduler_db_writer_observations (
  id TEXT PRIMARY KEY,
  operation TEXT NOT NULL,
  wait_ms INTEGER NOT NULL,
  observed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_scheduler_db_writer_observations_observed_at
  ON scheduler_db_writer_observations(observed_at);

CREATE TABLE IF NOT EXISTS host_interruption_epochs (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  monotonic_gap_ms INTEGER,
  wall_clock_gap_ms INTEGER,
  details_json TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS host_interruption_affected_executions (
  epoch_id TEXT NOT NULL REFERENCES host_interruption_epochs(id),
  agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
  run_id TEXT,
  stage_execution_id TEXT NOT NULL,
  provider_family TEXT,
  action TEXT NOT NULL,
  retry_enqueued_at TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (epoch_id, agent_execution_id)
);

CREATE INDEX IF NOT EXISTS idx_host_interruption_affected_execution
  ON host_interruption_affected_executions(agent_execution_id);
