-- no-transaction
-- P017 Phase B/C: mediation-owned agent executions are not stage-owned.
-- SQLite cannot drop NOT NULL or FK constraints in place, so rebuild the table
-- with nullable stage_execution_id while keeping existing columns and data.

PRAGMA foreign_keys = OFF;

CREATE TABLE IF NOT EXISTS agent_executions_p017_nullable_stage (
  id TEXT PRIMARY KEY,
  stage_execution_id TEXT REFERENCES stage_executions(id),
  agent_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  provider_family TEXT,
  model TEXT,
  status TEXT NOT NULL DEFAULT 'running',
  started_at TEXT NOT NULL,
  completed_at TEXT,
  owner_execution_lineage_id TEXT,
  session_lineage_id TEXT,
  session_generation_id TEXT,
  rehydrated_from_checkpoint_artifact_id TEXT,
  invocation_owner_key TEXT,
  session_reuse_scope TEXT,
  session_family_id TEXT,
  session_reuse_disposition TEXT,
  session_reset_reason TEXT,
  backend_profile_id TEXT,
  requested_mcp_extensions_json TEXT,
  predicted_mcp_extensions_json TEXT,
  predicted_mcp_runtime_ids_json TEXT,
  actual_mcp_extensions_json TEXT,
  actual_mcp_runtime_ids_json TEXT,
  denied_mcp_extensions_json TEXT,
  mcp_blocking_issues_json TEXT,
  actual_mcp_observation_json TEXT,
  actual_xcode_runtime_observation_json TEXT,
  mcp_session_startup_latency_ms INTEGER,
  owner_kind TEXT NOT NULL DEFAULT 'stage_execution'
    CHECK (owner_kind IN ('stage_execution', 'lead_conflict_mediation')),
  owner_id TEXT,
  lead_mediation_record_id TEXT,
  origin_stage_execution_id TEXT,
  CHECK (
    (owner_kind = 'stage_execution' AND stage_execution_id IS NOT NULL AND owner_id = stage_execution_id)
    OR
    (owner_kind = 'lead_conflict_mediation' AND stage_execution_id IS NULL AND owner_id IS NOT NULL)
  )
);

INSERT INTO agent_executions_p017_nullable_stage (
  id, stage_execution_id, agent_id, provider, provider_family, model, status,
  started_at, completed_at, owner_execution_lineage_id, session_lineage_id,
  session_generation_id, rehydrated_from_checkpoint_artifact_id,
  invocation_owner_key, session_reuse_scope, session_family_id,
  session_reuse_disposition, session_reset_reason, backend_profile_id,
  requested_mcp_extensions_json, predicted_mcp_extensions_json,
  predicted_mcp_runtime_ids_json, actual_mcp_extensions_json,
  actual_mcp_runtime_ids_json, denied_mcp_extensions_json,
  mcp_blocking_issues_json, actual_mcp_observation_json,
  actual_xcode_runtime_observation_json, mcp_session_startup_latency_ms,
  owner_kind, owner_id, lead_mediation_record_id, origin_stage_execution_id
)
SELECT
  id, stage_execution_id, agent_id, provider, provider_family, model, status,
  started_at, completed_at, owner_execution_lineage_id, session_lineage_id,
  session_generation_id, rehydrated_from_checkpoint_artifact_id,
  invocation_owner_key, session_reuse_scope, session_family_id,
  session_reuse_disposition, session_reset_reason, backend_profile_id,
  requested_mcp_extensions_json, predicted_mcp_extensions_json,
  predicted_mcp_runtime_ids_json, actual_mcp_extensions_json,
  actual_mcp_runtime_ids_json, denied_mcp_extensions_json,
  mcp_blocking_issues_json, actual_mcp_observation_json,
  actual_xcode_runtime_observation_json, mcp_session_startup_latency_ms,
  owner_kind, COALESCE(owner_id, stage_execution_id), lead_mediation_record_id,
  origin_stage_execution_id
FROM agent_executions;

DROP TABLE agent_executions;
ALTER TABLE agent_executions_p017_nullable_stage RENAME TO agent_executions;

CREATE INDEX IF NOT EXISTS idx_agent_executions_stage
  ON agent_executions(stage_execution_id);
CREATE INDEX IF NOT EXISTS idx_agent_executions_owner
  ON agent_executions(owner_kind, owner_id);
CREATE INDEX IF NOT EXISTS idx_agent_executions_provider_family_status
  ON agent_executions(provider_family, status);
CREATE INDEX IF NOT EXISTS idx_agent_executions_session_generation
  ON agent_executions(session_generation_id);

CREATE TABLE IF NOT EXISTS agent_retry_budget_ledger_p017_owner_keyed (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  owner_kind TEXT NOT NULL DEFAULT 'stage_execution'
    CHECK (owner_kind IN ('stage_execution', 'lead_conflict_mediation')),
  owner_id TEXT NOT NULL,
  stage_execution_id TEXT REFERENCES stage_executions(id),
  agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
  failure_kind TEXT NOT NULL,
  retry_after TEXT,
  normal_budget_consumed INTEGER NOT NULL DEFAULT 0,
  early_retry_journal_id TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (
    (owner_kind = 'stage_execution' AND stage_execution_id IS NOT NULL AND owner_id = stage_execution_id)
    OR
    (owner_kind = 'lead_conflict_mediation' AND stage_execution_id IS NULL)
  )
);

INSERT INTO agent_retry_budget_ledger_p017_owner_keyed (
  id, run_id, owner_kind, owner_id, stage_execution_id, agent_execution_id,
  failure_kind, retry_after, normal_budget_consumed, early_retry_journal_id,
  idempotency_key, state, created_at, updated_at
)
SELECT
  id, run_id, owner_kind, COALESCE(owner_id, stage_execution_id), stage_execution_id,
  agent_execution_id, failure_kind, retry_after, normal_budget_consumed,
  early_retry_journal_id, idempotency_key, state, created_at, updated_at
FROM agent_retry_budget_ledger;

DROP TABLE agent_retry_budget_ledger;
ALTER TABLE agent_retry_budget_ledger_p017_owner_keyed RENAME TO agent_retry_budget_ledger;

CREATE INDEX IF NOT EXISTS idx_agent_retry_budget_execution
  ON agent_retry_budget_ledger(agent_execution_id, failure_kind);
CREATE INDEX IF NOT EXISTS idx_retry_budget_owner
  ON agent_retry_budget_ledger(owner_kind, owner_id);

CREATE TABLE IF NOT EXISTS artifact_source_generation_claims_p017_owner_keyed (
  run_id TEXT NOT NULL REFERENCES runs(id),
  owner_kind TEXT NOT NULL DEFAULT 'stage_execution'
    CHECK (owner_kind IN ('stage_execution', 'lead_conflict_mediation')),
  owner_id TEXT NOT NULL,
  stage_execution_id TEXT REFERENCES stage_executions(id),
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
  PRIMARY KEY (run_id, owner_kind, owner_id, agent_execution_id, source_work_item_id),
  CHECK (
    (owner_kind = 'stage_execution' AND stage_execution_id IS NOT NULL AND owner_id = stage_execution_id)
    OR
    (owner_kind = 'lead_conflict_mediation' AND stage_execution_id IS NULL)
  )
);

INSERT INTO artifact_source_generation_claims_p017_owner_keyed (
  run_id, owner_kind, owner_id, stage_execution_id, agent_execution_id,
  source_work_item_id, current_session_generation_id, claim_state,
  superseding_work_item_id, superseded_by_agent_execution_id,
  supersession_journal_id, superseded_at, closed_at, created_at, updated_at
)
SELECT
  run_id, owner_kind, COALESCE(owner_id, stage_execution_id), stage_execution_id,
  agent_execution_id, source_work_item_id, current_session_generation_id,
  claim_state, superseding_work_item_id, superseded_by_agent_execution_id,
  supersession_journal_id, superseded_at, closed_at, created_at, updated_at
FROM artifact_source_generation_claims;

DROP TABLE artifact_source_generation_claims;
ALTER TABLE artifact_source_generation_claims_p017_owner_keyed RENAME TO artifact_source_generation_claims;

CREATE INDEX IF NOT EXISTS idx_artifact_source_claim_generation
  ON artifact_source_generation_claims(current_session_generation_id);
CREATE INDEX IF NOT EXISTS idx_artifact_source_claim_state
  ON artifact_source_generation_claims(run_id, claim_state);
CREATE INDEX IF NOT EXISTS idx_artifact_source_claim_superseding_work_item
  ON artifact_source_generation_claims(superseding_work_item_id);
CREATE INDEX IF NOT EXISTS idx_artifact_source_claim_superseded
  ON artifact_source_generation_claims(superseded_by_agent_execution_id);
CREATE INDEX IF NOT EXISTS idx_artifact_source_claim_owner
  ON artifact_source_generation_claims(owner_kind, owner_id);

PRAGMA foreign_keys = ON;
