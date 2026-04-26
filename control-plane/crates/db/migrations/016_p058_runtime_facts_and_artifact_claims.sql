CREATE TABLE IF NOT EXISTS agent_execution_runtime_facts (
  agent_execution_id TEXT PRIMARY KEY REFERENCES agent_executions(id),
  failure_kind TEXT,
  failure_kind_raw_debug TEXT,
  failure_kind_version INTEGER NOT NULL DEFAULT 1,
  failure_message_redacted TEXT,
  failure_message_redaction_version INTEGER NOT NULL DEFAULT 1,
  retry_after TEXT,
  operator_action_hint TEXT,
  provider_exit_status INTEGER,
  transport_error_code TEXT,
  supervision_classification TEXT,
  output_settlement TEXT NOT NULL DEFAULT 'none',
  valid_required_outputs INTEGER NOT NULL DEFAULT 0,
  late_output_count INTEGER NOT NULL DEFAULT 0,
  ignored_late_output_count INTEGER NOT NULL DEFAULT 0,
  session_reuse_reason TEXT,
  quota_ledger_id TEXT REFERENCES agent_retry_budget_ledger(id),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_runtime_facts_failure_kind
  ON agent_execution_runtime_facts(failure_kind);

CREATE INDEX IF NOT EXISTS idx_agent_runtime_facts_retry_after
  ON agent_execution_runtime_facts(retry_after)
  WHERE retry_after IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_agent_runtime_facts_output_settlement
  ON agent_execution_runtime_facts(output_settlement);

CREATE TABLE IF NOT EXISTS agent_retry_budget_ledger (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
  agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
  failure_kind TEXT NOT NULL,
  retry_after TEXT,
  normal_budget_consumed INTEGER NOT NULL DEFAULT 0,
  early_retry_journal_id TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_retry_budget_execution
  ON agent_retry_budget_ledger(agent_execution_id, failure_kind);

CREATE TABLE IF NOT EXISTS artifact_source_generation_claims (
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
  agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
  source_work_item_id TEXT NOT NULL,
  current_session_generation_id TEXT,
  claim_state TEXT NOT NULL DEFAULT 'active',
  superseding_work_item_id TEXT,
  superseded_by_agent_execution_id TEXT,
  supersession_journal_id TEXT,
  superseded_at TEXT,
  closed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (run_id, stage_execution_id, agent_execution_id, source_work_item_id)
);

CREATE INDEX IF NOT EXISTS idx_artifact_source_claim_generation
  ON artifact_source_generation_claims(current_session_generation_id);

CREATE INDEX IF NOT EXISTS idx_artifact_source_claim_state
  ON artifact_source_generation_claims(run_id, claim_state);

CREATE INDEX IF NOT EXISTS idx_artifact_source_claim_superseding_work_item
  ON artifact_source_generation_claims(superseding_work_item_id);

CREATE INDEX IF NOT EXISTS idx_artifact_source_claim_superseded
  ON artifact_source_generation_claims(superseded_by_agent_execution_id);

ALTER TABLE artifact_contract_generations
  ADD COLUMN source_stage_execution_id TEXT;

ALTER TABLE artifact_contract_generations
  ADD COLUMN source_session_generation_id TEXT;

ALTER TABLE artifact_contract_generations
  ADD COLUMN source_work_item_id TEXT;

ALTER TABLE artifact_contract_generations
  ADD COLUMN supersedes_generation_id TEXT;

ALTER TABLE artifact_contract_generations
  ADD COLUMN output_settlement TEXT NOT NULL DEFAULT 'none';

ALTER TABLE artifact_contract_generations
  ADD COLUMN source_generation_verified INTEGER NOT NULL DEFAULT 0;
