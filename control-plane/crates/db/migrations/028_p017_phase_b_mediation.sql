-- P017 Phase B/C: Owner-aware execution identity, mediation lifecycle,
-- and operator confirmation tables.

-- ── Owner-aware columns on agent_executions ────────────────────────────
ALTER TABLE agent_executions ADD COLUMN owner_kind TEXT NOT NULL DEFAULT 'stage_execution'
    CHECK (owner_kind IN ('stage_execution', 'lead_conflict_mediation'));
ALTER TABLE agent_executions ADD COLUMN owner_id TEXT;
ALTER TABLE agent_executions ADD COLUMN lead_mediation_record_id TEXT;
ALTER TABLE agent_executions ADD COLUMN origin_stage_execution_id TEXT;

-- Backfill: existing rows are stage-owned; owner_id = stage_execution_id
UPDATE agent_executions SET owner_id = stage_execution_id WHERE owner_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_agent_executions_owner
    ON agent_executions(owner_kind, owner_id);

-- ── Owner-aware columns on agent_retry_budget_ledger ───────────────────
ALTER TABLE agent_retry_budget_ledger ADD COLUMN owner_kind TEXT NOT NULL DEFAULT 'stage_execution'
    CHECK (owner_kind IN ('stage_execution', 'lead_conflict_mediation'));
ALTER TABLE agent_retry_budget_ledger ADD COLUMN owner_id TEXT;

-- Backfill: existing rows are stage-owned
UPDATE agent_retry_budget_ledger SET owner_id = stage_execution_id WHERE owner_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_retry_budget_owner
    ON agent_retry_budget_ledger(owner_kind, owner_id);

-- ── Owner-aware columns on artifact_source_generation_claims ───────────
ALTER TABLE artifact_source_generation_claims ADD COLUMN owner_kind TEXT NOT NULL DEFAULT 'stage_execution'
    CHECK (owner_kind IN ('stage_execution', 'lead_conflict_mediation'));
ALTER TABLE artifact_source_generation_claims ADD COLUMN owner_id TEXT;

-- Backfill: existing rows are stage-owned
UPDATE artifact_source_generation_claims SET owner_id = stage_execution_id WHERE owner_id IS NULL;

-- ── Lead conflict mediations table ─────────────────────────────────────
CREATE TABLE IF NOT EXISTS lead_conflict_mediations (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL,
    conflict_id TEXT NOT NULL,
    conflict_fingerprint TEXT NOT NULL,
    lead_agent_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN (
            'pending', 'queued', 'running', 'awaiting_output_validation',
            'operator_confirmation_required', 'settled', 'terminal_unverifiable',
            'canceled', 'superseded'
        )),
    settlement_result TEXT,
    recovery_action TEXT,
    chosen_action TEXT,
    chosen_next_state_id TEXT,
    chosen_next_state_label TEXT,
    operator_rationale TEXT,
    sanitized_progress TEXT,
    validation_errors_json TEXT,
    cost_summary_json TEXT,
    metric_event_id TEXT,
    superseded_by_event_ref TEXT,
    agent_execution_id TEXT,
    confirmation_subject_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    settled_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_lead_mediations_run
    ON lead_conflict_mediations(run_id);
CREATE INDEX IF NOT EXISTS idx_lead_mediations_conflict
    ON lead_conflict_mediations(conflict_id);
CREATE INDEX IF NOT EXISTS idx_lead_mediations_status
    ON lead_conflict_mediations(status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_lead_mediations_active_fingerprint
    ON lead_conflict_mediations(run_id, conflict_fingerprint)
    WHERE status NOT IN ('settled', 'terminal_unverifiable', 'canceled', 'superseded');

-- ── Lead mediation confirmations table (separate store from approvals) ─
CREATE TABLE IF NOT EXISTS lead_mediation_confirmations (
    id TEXT PRIMARY KEY NOT NULL,
    mediation_record_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    conflict_id TEXT NOT NULL,
    conflict_fingerprint TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'resolved', 'superseded', 'expired', 'canceled')),
    suggested_action TEXT,
    requested_at TEXT NOT NULL,
    deadline_at TEXT,
    readback_ref TEXT,
    idempotency_scope_key TEXT,
    resolved_at TEXT,
    resolved_by_principal_id TEXT,
    resolution_decision TEXT,
    resolution_comment TEXT,
    FOREIGN KEY (mediation_record_id) REFERENCES lead_conflict_mediations(id)
);

CREATE INDEX IF NOT EXISTS idx_mediation_confirmations_run
    ON lead_mediation_confirmations(run_id);
CREATE INDEX IF NOT EXISTS idx_mediation_confirmations_status
    ON lead_mediation_confirmations(status);
CREATE INDEX IF NOT EXISTS idx_mediation_confirmations_mediation
    ON lead_mediation_confirmations(mediation_record_id);
-- At most one pending confirmation per mediation record
CREATE UNIQUE INDEX IF NOT EXISTS idx_mediation_confirmations_pending_unique
    ON lead_mediation_confirmations(mediation_record_id)
    WHERE status = 'pending';
