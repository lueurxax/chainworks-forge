-- P060: Dynamic materialization records for idempotent reviewer execution.
-- Tracks which reviewer bindings have been materialized for each routing attempt.

CREATE TABLE IF NOT EXISTS dynamic_materialization_records (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id),
    stage_id TEXT NOT NULL,
    attempt_id INTEGER NOT NULL,
    phase_id TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    binding_id TEXT NOT NULL,
    agent_execution_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_dynamic_materialization_idempotency
    ON dynamic_materialization_records(run_id, stage_id, attempt_id, phase_id, plan_hash, binding_id);

CREATE INDEX IF NOT EXISTS idx_dynamic_materialization_run_id
    ON dynamic_materialization_records(run_id);
