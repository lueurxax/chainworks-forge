ALTER TABLE code_writer_completion_receipts
  ADD COLUMN provider_runtime_family TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN completion_boundary_subtype TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN final_payload_status TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN progress_before_handoff TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN runtime_preflight_phase TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN runtime_tool_path_preflight_json TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN final_completion_payload_capture_json TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN repair_materialization_summary_json TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN repair_materialization_mode TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN strict_final_payload_enabled INTEGER NOT NULL DEFAULT 0;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN staged_repair_settlement_enabled INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS code_writer_output_settlement_rows (
  id TEXT PRIMARY KEY,
  receipt_id TEXT NOT NULL REFERENCES code_writer_completion_receipts(id) ON DELETE CASCADE,
  run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  stage_id TEXT NOT NULL,
  stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id) ON DELETE CASCADE,
  agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id) ON DELETE CASCADE,
  session_generation_id TEXT NOT NULL,
  repair_attempt INTEGER NOT NULL DEFAULT 0,
  output_name TEXT NOT NULL,
  contract_id TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  source_generation_owner TEXT NOT NULL,
  candidate_digest TEXT,
  staging_path TEXT,
  canonical_path TEXT NOT NULL,
  canonical_before_sha256 TEXT,
  canonical_after_sha256 TEXT,
  decision TEXT NOT NULL,
  rejection_reason TEXT,
  materialization_state TEXT NOT NULL,
  active_pointer_generation_id TEXT,
  created_at TEXT NOT NULL,
  committed_at TEXT,
  UNIQUE(receipt_id, output_name)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_code_writer_output_settlement_idempotency
  ON code_writer_output_settlement_rows(agent_execution_id, repair_attempt, output_name, candidate_digest)
  WHERE candidate_digest IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_code_writer_output_settlement_receipt
  ON code_writer_output_settlement_rows(receipt_id);

CREATE INDEX IF NOT EXISTS idx_code_writer_output_settlement_agent_execution
  ON code_writer_output_settlement_rows(agent_execution_id);
