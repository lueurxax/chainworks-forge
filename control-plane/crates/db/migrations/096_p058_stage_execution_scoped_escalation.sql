-- Escalation chains belong to a concrete stage execution, not just a logical stage.
-- Legacy rows remain readable with a NULL stage_execution_id and cannot match new attempts.
ALTER TABLE escalation_ledger ADD COLUMN stage_execution_id TEXT;

DROP INDEX IF EXISTS idx_escalation_ledger_unique_chain;

CREATE UNIQUE INDEX idx_escalation_ledger_unique_stage_execution_chain
    ON escalation_ledger(run_id, stage_id, stage_execution_id, agent_id, policy_id);

CREATE INDEX idx_escalation_ledger_stage_execution_id
    ON escalation_ledger(stage_execution_id);
