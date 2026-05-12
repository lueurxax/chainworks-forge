PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS agent_execution_runtime_receipts_p088 (
  runtime_receipt_id TEXT NOT NULL,
  agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id) ON DELETE CASCADE,
  prompt_kind TEXT NOT NULL DEFAULT 'original',
  turn_index INTEGER NOT NULL DEFAULT 0,
  prompt_template_id TEXT,
  prompt_template_version INTEGER,
  prompt_sha256 TEXT,
  redacted_prompt_artifact_path TEXT,
  expected_output_contract_snapshot_sha256 TEXT,
  expected_output_contract_snapshot_path TEXT,
  repair_or_settlement_reason TEXT,
  provider TEXT NOT NULL,
  transport_family TEXT NOT NULL,
  status TEXT NOT NULL,
  failure_phase TEXT,
  event_count INTEGER NOT NULL DEFAULT 0,
  last_event_kind TEXT,
  last_event_at_ms INTEGER,
  receipt_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (agent_execution_id, prompt_kind, turn_index),
  UNIQUE (runtime_receipt_id)
);

INSERT OR IGNORE INTO agent_execution_runtime_receipts_p088 (
  runtime_receipt_id, agent_execution_id, prompt_kind, turn_index,
  provider, transport_family, status, failure_phase, event_count,
  last_event_kind, last_event_at_ms, receipt_json, created_at, updated_at
)
SELECT
  agent_execution_id || ':original:0',
  agent_execution_id,
  'original',
  0,
  provider,
  transport_family,
  status,
  failure_phase,
  event_count,
  last_event_kind,
  last_event_at_ms,
  receipt_json,
  created_at,
  updated_at
FROM agent_execution_runtime_receipts;

DROP TABLE agent_execution_runtime_receipts;
ALTER TABLE agent_execution_runtime_receipts_p088 RENAME TO agent_execution_runtime_receipts;

PRAGMA foreign_keys = ON;

CREATE INDEX IF NOT EXISTS idx_agent_execution_runtime_receipts_provider_status
  ON agent_execution_runtime_receipts(provider, status);

CREATE INDEX IF NOT EXISTS idx_agent_execution_runtime_receipts_last_event_kind
  ON agent_execution_runtime_receipts(last_event_kind);

CREATE INDEX IF NOT EXISTS idx_agent_execution_runtime_receipts_execution_prompt
  ON agent_execution_runtime_receipts(agent_execution_id, prompt_kind, turn_index);

CREATE TABLE IF NOT EXISTS code_writer_completion_receipts (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id) ON DELETE CASCADE,
  agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id) ON DELETE CASCADE,
  session_generation_id TEXT,
  original_runtime_receipt_id TEXT,
  completion_repair_runtime_receipt_id TEXT,
  provider TEXT NOT NULL,
  model TEXT,
  completion_mode TEXT,
  published_at TEXT,
  activation_source TEXT NOT NULL,
  ingestion_boundary_failure TEXT,
  work_change_kind TEXT,
  pre_prompt_worktree_fingerprint_path TEXT,
  post_prompt_worktree_fingerprint_path TEXT,
  pre_prompt_worktree_fingerprint_sha256 TEXT,
  post_prompt_worktree_fingerprint_sha256 TEXT,
  current_attempt_changed_path_count INTEGER NOT NULL DEFAULT 0,
  preexisting_dirty_path_count INTEGER NOT NULL DEFAULT 0,
  completion_status TEXT NOT NULL,
  failure_class TEXT,
  terminal_response_status TEXT,
  completion_turn_attempted INTEGER NOT NULL DEFAULT 0,
  completion_turn_result TEXT,
  completion_text_capture_count INTEGER NOT NULL DEFAULT 0,
  completion_text_absence_count INTEGER NOT NULL DEFAULT 0,
  completion_repair_text_status TEXT,
  completion_repair_raw_text_artifact_path TEXT,
  completion_repair_redacted_text_artifact_path TEXT,
  completion_repair_text_absence_reason TEXT,
  fresh_required_output_count INTEGER NOT NULL DEFAULT 0,
  stale_required_output_count INTEGER NOT NULL DEFAULT 0,
  missing_required_output_count INTEGER NOT NULL DEFAULT 0,
  control_plane_output_count INTEGER NOT NULL DEFAULT 0,
  completion_repair_turn_count INTEGER NOT NULL DEFAULT 0,
  generic_repair_turn_count INTEGER NOT NULL DEFAULT 0,
  missing_outputs TEXT NOT NULL DEFAULT '[]',
  stale_outputs TEXT NOT NULL DEFAULT '[]',
  transcript_status TEXT,
  transcript_absence_reason TEXT,
  receipt_artifact_path TEXT,
  failed_stage_evidence_path TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (agent_execution_id)
);

CREATE TABLE IF NOT EXISTS code_writer_completion_text_captures (
  receipt_id TEXT NOT NULL REFERENCES code_writer_completion_receipts(id) ON DELETE CASCADE,
  prompt_kind TEXT NOT NULL,
  turn_index INTEGER NOT NULL DEFAULT 0,
  terminal_response_status TEXT,
  completion_text_status TEXT NOT NULL,
  completion_text_capture_source TEXT,
  completion_text_raw_byte_limit INTEGER,
  completion_text_captured_byte_count INTEGER,
  completion_text_truncated INTEGER NOT NULL DEFAULT 0,
  extraction_input_truncated INTEGER NOT NULL DEFAULT 0,
  extraction_input_sha256 TEXT,
  raw_text_artifact_path TEXT,
  redacted_text_artifact_path TEXT,
  text_absence_reason TEXT,
  created_at TEXT NOT NULL,
  PRIMARY KEY (receipt_id, prompt_kind, turn_index)
);

CREATE TABLE IF NOT EXISTS code_writer_completion_output_decisions (
  receipt_id TEXT NOT NULL REFERENCES code_writer_completion_receipts(id) ON DELETE CASCADE,
  output_name TEXT NOT NULL,
  contract_id TEXT,
  canonical_path TEXT NOT NULL,
  pre_prompt_sha256 TEXT,
  post_prompt_sha256 TEXT,
  content_sha256 TEXT,
  settlement_source TEXT,
  validation_status TEXT,
  rejection_reason TEXT,
  PRIMARY KEY (receipt_id, output_name)
);

CREATE INDEX IF NOT EXISTS idx_code_writer_completion_receipts_run
  ON code_writer_completion_receipts(run_id);

CREATE INDEX IF NOT EXISTS idx_code_writer_completion_receipts_agent_execution
  ON code_writer_completion_receipts(agent_execution_id);

CREATE TABLE IF NOT EXISTS code_writer_completion_receipt_links (
  agent_execution_id TEXT PRIMARY KEY REFERENCES agent_executions(id) ON DELETE CASCADE,
  receipt_id TEXT NOT NULL UNIQUE REFERENCES code_writer_completion_receipts(id) ON DELETE CASCADE,
  run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id) ON DELETE CASCADE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_code_writer_completion_receipt_links_run
  ON code_writer_completion_receipt_links(run_id);
