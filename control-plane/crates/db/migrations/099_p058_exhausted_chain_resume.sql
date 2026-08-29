-- P058 operator recovery: permit an audited one-shot window after a terminal
-- escalation chain pause while preserving the original deadline-window rows.
CREATE TABLE escalation_deadline_windows_v99 (
    id                        TEXT NOT NULL PRIMARY KEY,
    escalation_ledger_id      TEXT NOT NULL REFERENCES escalation_ledger(id),
    previous_window_id        TEXT REFERENCES escalation_deadline_windows_v99(id),
    tier_id                   TEXT NOT NULL,
    tier_kind_raw             TEXT NOT NULL,
    policy_hash               TEXT NOT NULL,
    source_pause_reason_raw   TEXT NOT NULL CHECK(source_pause_reason_raw IN (
        'escalation_deadline_elapsed',
        'escalation_chain_exhausted'
    )),
    source_deadline_at        TEXT NOT NULL,
    opened_by_principal_id    TEXT NOT NULL,
    command_journal_id        TEXT NOT NULL UNIQUE REFERENCES command_journal(id),
    resume_idempotency_key    TEXT NOT NULL UNIQUE,
    resume_request_hash       TEXT NOT NULL,
    source_stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id),
    source_agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
    retry_stage_execution_id  TEXT NOT NULL UNIQUE REFERENCES stage_executions(id),
    work_item_id              TEXT NOT NULL UNIQUE REFERENCES work_items(id),
    target_backend_profile_id TEXT NOT NULL,
    target_provider           TEXT NOT NULL,
    starts_at                 TEXT NOT NULL,
    expires_at                TEXT NOT NULL,
    created_at                TEXT NOT NULL,
    CHECK(expires_at > starts_at)
);

INSERT INTO escalation_deadline_windows_v99 (
    id, escalation_ledger_id, previous_window_id, tier_id, tier_kind_raw,
    policy_hash, source_pause_reason_raw, source_deadline_at,
    opened_by_principal_id, command_journal_id, resume_idempotency_key,
    resume_request_hash, source_stage_execution_id, source_agent_execution_id,
    retry_stage_execution_id, work_item_id, target_backend_profile_id,
    target_provider, starts_at, expires_at, created_at
)
SELECT
    id, escalation_ledger_id, previous_window_id, tier_id, tier_kind_raw,
    policy_hash, source_pause_reason_raw, source_deadline_at,
    opened_by_principal_id, command_journal_id, resume_idempotency_key,
    resume_request_hash, source_stage_execution_id, source_agent_execution_id,
    retry_stage_execution_id, work_item_id, target_backend_profile_id,
    target_provider, starts_at, expires_at, created_at
FROM escalation_deadline_windows;

DROP TABLE escalation_deadline_windows;
ALTER TABLE escalation_deadline_windows_v99 RENAME TO escalation_deadline_windows;

CREATE INDEX idx_escalation_deadline_windows_ledger
    ON escalation_deadline_windows(escalation_ledger_id, created_at, id);
