-- P065: Operator Retry Instruction Contract
-- Parent binding row — one per instructed retry command

CREATE TABLE IF NOT EXISTS retry_operator_instruction_bindings (
    binding_id TEXT PRIMARY KEY NOT NULL,
    journal_id TEXT NOT NULL REFERENCES command_journal(id),
    run_id TEXT NOT NULL REFERENCES runs(id),
    stage_id TEXT NOT NULL,
    source_stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
    retry_stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
    retry_attempt_number INTEGER NOT NULL,
    target_agent_execution_id TEXT,
    scope_kind TEXT NOT NULL CHECK (scope_kind IN (
        'full_stage_retry',
        'targeted_retry',
        'targeted_retry_fallback_full_stage'
    )),
    instruction_text TEXT NOT NULL,
    instruction_sha256 TEXT NOT NULL,
    created_by_principal_id TEXT NOT NULL,
    created_by_principal_class TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_retry_instruction_bindings_run
    ON retry_operator_instruction_bindings(run_id, created_at);

CREATE INDEX IF NOT EXISTS idx_retry_instruction_bindings_retry_stage
    ON retry_operator_instruction_bindings(retry_stage_execution_id);

-- Child delivery row — one per InvokeAgent work item that receives the instruction

CREATE TABLE IF NOT EXISTS retry_operator_instruction_deliveries (
    delivery_id TEXT PRIMARY KEY NOT NULL,
    binding_id TEXT NOT NULL REFERENCES retry_operator_instruction_bindings(binding_id),
    work_item_id TEXT,
    agent_execution_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'delivered', 'failed')),
    failure_reason TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    delivered_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_retry_instruction_deliveries_binding
    ON retry_operator_instruction_deliveries(binding_id);

-- Replay-safe: at most one delivery row per (binding, work_item) pair
CREATE UNIQUE INDEX IF NOT EXISTS idx_retry_instruction_deliveries_binding_work_item
    ON retry_operator_instruction_deliveries(binding_id, work_item_id)
    WHERE work_item_id IS NOT NULL;
