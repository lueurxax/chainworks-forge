-- Standalone journal. No daemon, coordinator, provider or migration-of-live-data wiring.
CREATE TABLE xcode_effect_attempts (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
    attempt_id TEXT NOT NULL UNIQUE CHECK (
        length(attempt_id) = 36 AND substr(attempt_id, 9, 1) = '-'
        AND substr(attempt_id, 14, 1) = '-' AND substr(attempt_id, 19, 1) = '-'
        AND substr(attempt_id, 24, 1) = '-'
        AND length(replace(attempt_id, '-', '')) = 32
        AND replace(attempt_id, '-', '') NOT GLOB '*[^0-9a-f]*'),
    nonce TEXT NOT NULL UNIQUE CHECK (nonce = attempt_id),
    owner_lineage TEXT NOT NULL CHECK (length(CAST(owner_lineage AS BLOB)) BETWEEN 1 AND 256),
    project_key TEXT NOT NULL CHECK (json_valid(project_key) AND length(CAST(project_key AS BLOB)) <= 8192),
    operation_key TEXT NOT NULL CHECK (length(operation_key) = 36),
    -- Closed domain structs only; never arbitrary provider request/result JSON.
    intent_json TEXT NOT NULL CHECK (json_valid(intent_json) AND length(CAST(intent_json AS BLOB)) <= 8192),
    state TEXT NOT NULL CHECK (state IN ('prepared', 'dispatched', 'succeeded', 'failed', 'unknown', 'reconciled')),
    revision INTEGER NOT NULL CHECK (typeof(revision) = 'integer' AND revision >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    dispatched_at TEXT,
    completed_at TEXT,
    outcome_json TEXT CHECK (outcome_json IS NULL OR (json_valid(outcome_json) AND length(CAST(outcome_json AS BLOB)) <= 8192)),
    result_digest TEXT CHECK (result_digest IS NULL OR (length(result_digest) = 64 AND result_digest NOT GLOB '*[^0-9a-f]*')),
    uncertainty_reason TEXT CHECK (uncertainty_reason IN ('restart', 'transport_lost', 'partial_effect', 'async_in_flight', 'cancelled_after_dispatch')),
    settlement_proof_json TEXT CHECK (settlement_proof_json IS NULL OR (json_valid(settlement_proof_json) AND length(CAST(settlement_proof_json AS BLOB)) <= 2048)),
    reconciliation_json TEXT CHECK (reconciliation_json IS NULL OR (json_valid(reconciliation_json) AND length(CAST(reconciliation_json AS BLOB)) <= 16384)),
    reconciliation_evidence_ref TEXT CHECK (reconciliation_evidence_ref IS NULL OR length(CAST(reconciliation_evidence_ref AS BLOB)) BETWEEN 1 AND 256),
    UNIQUE (owner_lineage, project_key, operation_key),
    UNIQUE (project_key, attempt_id),
    CHECK (json_extract(intent_json, '$.owner_lineage') IS owner_lineage),
    CHECK (json_extract(intent_json, '$.operation_key') IS operation_key),
    CHECK (json_extract(intent_json, '$.project_key') IS project_key),
    CHECK (json_extract(intent_json, '$.origin') IN ('lifecycle', 'mcp', 'shim')),
    CHECK (json_extract(intent_json, '$.effect_class') IN ('mutate', 'execute')),
    CHECK (json_type(intent_json, '$.binding_digest') IS 'text' OR (
        json_type(intent_json, '$.binding_digest') IS 'null'
        AND json_extract(intent_json, '$.origin') IS 'lifecycle'
        AND json_extract(intent_json, '$.effect_class') IS 'mutate'
        AND json_extract(intent_json, '$.operation') IS 'workspace_open')),
    CHECK ((outcome_json IS NULL AND result_digest IS NULL) OR (
        outcome_json IS NOT NULL AND result_digest IS NOT NULL
        AND json_extract(outcome_json, '$.schema_version') IS 1
        AND json_extract(outcome_json, '$.result_digest') IS result_digest
        AND json_extract(outcome_json, '$.schema') IS json_extract(intent_json, '$.result_schema'))),
    CHECK (
        (state = 'prepared' AND dispatched_at IS NULL AND completed_at IS NULL AND outcome_json IS NULL AND uncertainty_reason IS NULL AND settlement_proof_json IS NULL)
        OR (state = 'dispatched' AND dispatched_at IS NOT NULL AND completed_at IS NULL AND outcome_json IS NULL AND uncertainty_reason IS NULL AND settlement_proof_json IS NULL)
        OR (state = 'unknown' AND dispatched_at IS NOT NULL AND completed_at IS NULL AND outcome_json IS NULL AND uncertainty_reason IS NOT NULL AND settlement_proof_json IS NULL)
        OR (state = 'succeeded' AND dispatched_at IS NOT NULL AND completed_at IS NOT NULL AND outcome_json IS NOT NULL AND json_extract(outcome_json, '$.payload.kind') IS 'result' AND uncertainty_reason IS NULL AND settlement_proof_json IS NOT NULL)
        OR (state = 'failed' AND completed_at IS NOT NULL AND outcome_json IS NOT NULL AND json_extract(outcome_json, '$.payload.kind') IS 'error' AND uncertainty_reason IS NULL AND (
            (dispatched_at IS NULL AND settlement_proof_json IS NULL AND json_extract(outcome_json, '$.payload.code') IS 'cancelled_before_dispatch')
            OR (dispatched_at IS NOT NULL AND settlement_proof_json IS NOT NULL AND json_extract(outcome_json, '$.payload.code') IS 'terminal_failure')))
        OR (state = 'reconciled' AND dispatched_at IS NOT NULL AND completed_at IS NOT NULL AND outcome_json IS NOT NULL AND settlement_proof_json IS NOT NULL AND uncertainty_reason IS NOT NULL)
    ),
    CHECK ((state = 'reconciled' AND reconciliation_json IS NOT NULL AND reconciliation_evidence_ref IS NOT NULL)
        OR (state <> 'reconciled' AND reconciliation_json IS NULL AND reconciliation_evidence_ref IS NULL))
);

CREATE INDEX xcode_effect_attempts_owner_sequence ON xcode_effect_attempts(owner_lineage, sequence);
CREATE INDEX xcode_effect_attempts_recovery ON xcode_effect_attempts(state, sequence);

CREATE TABLE xcode_project_holds (
    project_key TEXT PRIMARY KEY NOT NULL,
    attempt_id TEXT NOT NULL UNIQUE,
    uid INTEGER NOT NULL CHECK (uid IS json_extract(project_key, '$.uid')),
    canonical_path TEXT NOT NULL CHECK (canonical_path IS json_extract(project_key, '$.canonical_path')),
    device TEXT NOT NULL CHECK (device IS json_extract(project_key, '$.device')),
    inode TEXT NOT NULL CHECK (inode IS json_extract(project_key, '$.inode')),
    -- Pin both the path slot (replacement) and filesystem identity (aliases).
    UNIQUE (uid, canonical_path),
    UNIQUE (uid, device, inode),
    FOREIGN KEY (project_key, attempt_id) REFERENCES xcode_effect_attempts(project_key, attempt_id) ON DELETE RESTRICT
);

CREATE TRIGGER xcode_effect_identity_immutable
BEFORE UPDATE OF sequence, attempt_id, nonce, owner_lineage, project_key, operation_key, intent_json, created_at ON xcode_effect_attempts
BEGIN SELECT RAISE(ABORT, 'xcode_effect_identity_immutable'); END;

CREATE TRIGGER xcode_effect_transition_guard
BEFORE UPDATE ON xcode_effect_attempts
WHEN NEW.revision <> OLD.revision + 1 OR NOT (
    (OLD.state = 'prepared' AND NEW.state IN ('dispatched', 'failed'))
    OR (OLD.state = 'dispatched' AND NEW.state IN ('succeeded', 'failed', 'unknown'))
    OR (OLD.state = 'unknown' AND NEW.state = 'reconciled'))
BEGIN SELECT RAISE(ABORT, 'xcode_effect_invalid_transition'); END;

CREATE TRIGGER xcode_effect_hold_requires_uncertainty
BEFORE INSERT ON xcode_project_holds
WHEN NOT EXISTS (SELECT 1 FROM xcode_effect_attempts WHERE attempt_id = NEW.attempt_id AND project_key = NEW.project_key AND state IN ('dispatched', 'unknown'))
BEGIN SELECT RAISE(ABORT, 'xcode_effect_hold_requires_dispatch'); END;

CREATE TRIGGER xcode_effect_hold_immutable
BEFORE UPDATE ON xcode_project_holds
BEGIN SELECT RAISE(ABORT, 'xcode_effect_hold_immutable'); END;

CREATE TRIGGER xcode_effect_hold_requires_settlement
BEFORE DELETE ON xcode_project_holds
WHEN EXISTS (SELECT 1 FROM xcode_effect_attempts WHERE attempt_id = OLD.attempt_id AND state IN ('dispatched', 'unknown'))
BEGIN SELECT RAISE(ABORT, 'xcode_effect_hold_unsettled'); END;

-- Retention integration must explicitly preserve tombstones; there is no purge API.
CREATE TRIGGER xcode_effect_no_automatic_delete
BEFORE DELETE ON xcode_effect_attempts
BEGIN SELECT RAISE(ABORT, 'xcode_effect_retention_not_integrated'); END;
