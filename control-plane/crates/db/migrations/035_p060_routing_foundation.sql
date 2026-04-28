-- P060: Deterministic reviewer routing foundation tables.
-- Adds review_routing_json to runs, creates system_executions and routing_receipts.

ALTER TABLE runs ADD COLUMN review_routing_json TEXT;

-- SystemExecution: lifecycle record for system.routing tasks (no AgentExecution).
CREATE TABLE IF NOT EXISTS system_executions (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id),
    stage_id TEXT NOT NULL,
    attempt_id INTEGER NOT NULL,
    task_id TEXT NOT NULL,
    task_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    started_at TEXT NOT NULL,
    completed_at TEXT,
    receipt_id TEXT,
    plan_hash TEXT,
    failure_kind TEXT
);

CREATE INDEX IF NOT EXISTS idx_system_executions_run_id ON system_executions(run_id);
CREATE INDEX IF NOT EXISTS idx_system_executions_stage ON system_executions(run_id, stage_id, attempt_id);

-- RoutingReceipt: every terminal routing outcome (success or failure).
CREATE TABLE IF NOT EXISTS routing_receipts (
    receipt_id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL REFERENCES runs(id),
    stage_id TEXT NOT NULL,
    attempt_id INTEGER NOT NULL,
    system_execution_id TEXT NOT NULL REFERENCES system_executions(id),
    status TEXT NOT NULL,
    failure_kind TEXT,
    plan_hash TEXT,
    input_snapshot_hashes_json TEXT,
    operator_actions_json TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_routing_receipts_run_id ON routing_receipts(run_id);
CREATE INDEX IF NOT EXISTS idx_routing_receipts_stage ON routing_receipts(run_id, stage_id, attempt_id);
