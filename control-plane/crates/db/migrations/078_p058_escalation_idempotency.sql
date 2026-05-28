-- P058 Phase 1 follow-up: enforce the overlap-free active-tier invariant.
--
-- Index 1 (chain-level): at most one escalation_ledger row per (run_id, stage_id, agent_id, policy_id).
-- This is the chain-level uniqueness guard — (run_id, stage_id, agent_id, policy_id) identifies
-- one escalation chain; the ledger.id is the chain handle. Prevents duplicate chain creation
-- from racing writers or replayed inserts.
CREATE UNIQUE INDEX IF NOT EXISTS idx_escalation_ledger_unique_chain
    ON escalation_ledger(run_id, stage_id, agent_id, policy_id);

-- Index 2 (tier-attempt-level idempotency): enforce the proposal idempotency key at the
-- execution-metadata layer. Proposal §architecture.idempotency_key = run_id, stage_id,
-- ledger_id, tier_id, tier_attempt_index. The run_id and stage_id are captured by the
-- ledger FK (escalation_ledger_id → escalation_ledger.id → run_id/stage_id), so enforcing
-- uniqueness on (escalation_ledger_id, tier_id, tier_attempt_index) is equivalent to the
-- full proposal key. This prevents duplicate scheduler transactions from inserting two
-- metadata rows for the same (chain, tier, attempt) tuple.
CREATE UNIQUE INDEX IF NOT EXISTS idx_escalation_exec_meta_idempotency
    ON escalation_execution_metadata(escalation_ledger_id, tier_id, tier_attempt_index);
