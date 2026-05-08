-- P058 Phase 0-1: Escalation policy persistence for configurable agent escalation chains.
-- Tables: escalation_ledger, escalation_execution_metadata, escalation_events.
-- Columns added to agent_executions (attribution) and agent_execution_runtime_facts (shadow).
-- Phase 0 = schema only; no scheduler behavior change.

-- One row per escalation chain (keyed by run+stage+agent+policy).
CREATE TABLE IF NOT EXISTS escalation_ledger (
    id                    TEXT NOT NULL PRIMARY KEY,
    run_id                TEXT NOT NULL REFERENCES runs(id),
    stage_id              TEXT NOT NULL,
    agent_id              TEXT NOT NULL,
    policy_id             TEXT NOT NULL,
    policy_hash           TEXT NOT NULL,
    -- Raw status: active | paused | exhausted | cancelled
    status_raw            TEXT NOT NULL DEFAULT 'active',
    current_tier_id       TEXT,
    current_tier_kind_raw TEXT,
    chain_attempt_index   INTEGER NOT NULL DEFAULT 0,
    trigger_raw           TEXT,
    pause_reason_raw      TEXT,
    operator_action_hint  TEXT,
    runbook_anchor        TEXT,
    created_at            TEXT NOT NULL,
    updated_at            TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_escalation_ledger_run_id
    ON escalation_ledger(run_id);

CREATE INDEX IF NOT EXISTS idx_escalation_ledger_status
    ON escalation_ledger(status_raw);

-- Per-execution escalation attribution.
CREATE TABLE IF NOT EXISTS escalation_execution_metadata (
    agent_execution_id      TEXT NOT NULL PRIMARY KEY REFERENCES agent_executions(id),
    escalation_ledger_id    TEXT NOT NULL REFERENCES escalation_ledger(id),
    tier_id                 TEXT NOT NULL,
    tier_kind_raw           TEXT NOT NULL,
    tier_attempt_index      INTEGER NOT NULL DEFAULT 0,
    trigger_raw             TEXT,
    digest_version          TEXT,
    capacity_probe_counter  INTEGER NOT NULL DEFAULT 0,
    created_at              TEXT NOT NULL,
    updated_at              TEXT NOT NULL
);

-- Ordered event journal for the chain.
CREATE TABLE IF NOT EXISTS escalation_events (
    id                   TEXT NOT NULL PRIMARY KEY,
    escalation_ledger_id TEXT NOT NULL REFERENCES escalation_ledger(id),
    event_kind_raw       TEXT NOT NULL,
    tier_id              TEXT,
    tier_kind_raw        TEXT,
    trigger_raw          TEXT,
    pause_reason_raw     TEXT,
    payload_json         TEXT,
    created_at           TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_escalation_events_ledger
    ON escalation_events(escalation_ledger_id, created_at);

-- Escalation attribution columns on agent_executions.
ALTER TABLE agent_executions ADD COLUMN escalation_policy_id      TEXT;
ALTER TABLE agent_executions ADD COLUMN escalation_policy_hash    TEXT;
ALTER TABLE agent_executions ADD COLUMN escalation_tier_id        TEXT;
ALTER TABLE agent_executions ADD COLUMN escalation_tier_kind_raw  TEXT;
ALTER TABLE agent_executions ADD COLUMN escalation_trigger_raw    TEXT;
ALTER TABLE agent_executions ADD COLUMN escalation_digest_version TEXT;
ALTER TABLE agent_executions ADD COLUMN escalation_ledger_id      TEXT;

-- Shadow selection columns on agent_execution_runtime_facts (Phase 1b).
ALTER TABLE agent_execution_runtime_facts
    ADD COLUMN would_select_tier_id       TEXT;
ALTER TABLE agent_execution_runtime_facts
    ADD COLUMN would_select_trigger_raw   TEXT;
ALTER TABLE agent_execution_runtime_facts
    ADD COLUMN would_select_decision_json TEXT;
