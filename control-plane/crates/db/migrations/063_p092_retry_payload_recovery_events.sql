CREATE TABLE IF NOT EXISTS retry_payload_recovery_events (
    idempotency_key TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES runs(id),
    invoke_work_item_id TEXT NOT NULL,
    retry_authority_id TEXT,
    target_stage_execution_id TEXT,
    completed_agent_execution_id TEXT,
    reason_code TEXT NOT NULL,
    mode TEXT NOT NULL,
    repaired INTEGER NOT NULL DEFAULT 0,
    current_json TEXT NOT NULL,
    provenance_json TEXT,
    repaired_fields_json TEXT,
    diagnostic_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_retry_payload_recovery_events_run_authority
    ON retry_payload_recovery_events(run_id, retry_authority_id, updated_at);

CREATE INDEX IF NOT EXISTS idx_retry_payload_recovery_events_run_invoke
    ON retry_payload_recovery_events(run_id, invoke_work_item_id);

CREATE INDEX IF NOT EXISTS idx_retry_payload_recovery_events_run_target
    ON retry_payload_recovery_events(run_id, target_stage_execution_id);
