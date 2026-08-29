-- P058 operator recovery: append-only deadline windows for elapsed escalation chains.
-- The original escalation_ledger.created_at remains immutable. Each explicit resume
-- opens a new bounded window linked to the ledger and, when present, the prior window.
CREATE TABLE escalation_deadline_windows (
    id                       TEXT NOT NULL PRIMARY KEY,
    escalation_ledger_id     TEXT NOT NULL REFERENCES escalation_ledger(id),
    previous_window_id       TEXT REFERENCES escalation_deadline_windows(id),
    tier_id                  TEXT NOT NULL,
    tier_kind_raw            TEXT NOT NULL,
    policy_hash              TEXT NOT NULL,
    source_pause_reason_raw  TEXT NOT NULL
        CHECK(source_pause_reason_raw = 'escalation_deadline_elapsed'),
    source_deadline_at       TEXT NOT NULL,
    opened_by_principal_id   TEXT NOT NULL,
    command_journal_id       TEXT NOT NULL UNIQUE REFERENCES command_journal(id),
    resume_idempotency_key   TEXT NOT NULL UNIQUE,
    resume_request_hash      TEXT NOT NULL,
    source_stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
    source_agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
    retry_stage_execution_id TEXT NOT NULL UNIQUE REFERENCES stage_executions(id),
    work_item_id             TEXT NOT NULL UNIQUE REFERENCES work_items(id),
    target_backend_profile_id TEXT NOT NULL,
    target_provider          TEXT NOT NULL,
    starts_at                TEXT NOT NULL,
    expires_at               TEXT NOT NULL,
    created_at               TEXT NOT NULL,
    CHECK(expires_at > starts_at)
);

CREATE INDEX idx_escalation_deadline_windows_ledger
    ON escalation_deadline_windows(escalation_ledger_id, created_at, id);
