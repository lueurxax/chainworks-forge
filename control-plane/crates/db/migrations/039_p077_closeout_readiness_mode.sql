-- P077: Add closeout_readiness_mode column to runs.
-- Per R14 §architecture.readiness_mode_storage:
--   - Nullable, populated from workflow snapshot metadata at run admission.
--   - Frozen for the run; survives workflow edits.
--   - NULL means legacy/missing; accessor returns advisory unless an enforcement
--     migration record exists.
--   - Allowed values: advisory | enforcement | NULL.

ALTER TABLE runs ADD COLUMN closeout_readiness_mode TEXT;

-- Explicit enforcement-migration records for runs that must switch to enforcement
-- despite having NULL in the column (governed admin decision only).
CREATE TABLE IF NOT EXISTS closeout_readiness_mode_overrides (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id),
    mode TEXT NOT NULL CHECK(mode IN ('advisory', 'enforcement')),
    reason TEXT NOT NULL,
    principal TEXT NOT NULL,
    journal_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_closeout_readiness_mode_overrides_run_id
    ON closeout_readiness_mode_overrides(run_id);

-- Stores active generations for proposal_gate_result_v1 and
-- implementation_closeout_readiness_v1 as artifact-contract truth.
-- No transition is evaluated between gate activation and readiness activation.
CREATE TABLE IF NOT EXISTS closeout_gate_generations (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id),
    stage_id TEXT NOT NULL,
    contract_id TEXT NOT NULL,
    status TEXT NOT NULL,
    decision TEXT,
    generation_id TEXT NOT NULL,
    readiness_mode TEXT,
    diagnostic_reason TEXT,
    primary_unblock TEXT,
    code_blocker_count INTEGER NOT NULL DEFAULT 0,
    handoff_owner TEXT,
    risk_settlement_required INTEGER NOT NULL DEFAULT 0,
    fingerprint_json TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    superseded_by_generation_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_closeout_gate_generations_run_id
    ON closeout_gate_generations(run_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_closeout_gate_generations_active_generation
    ON closeout_gate_generations(run_id, contract_id, generation_id);

CREATE INDEX IF NOT EXISTS idx_closeout_gate_generations_active
    ON closeout_gate_generations(run_id, contract_id, active);
