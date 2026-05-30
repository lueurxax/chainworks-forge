-- P081 Phase 5: Approval mutation idempotency table.
--
-- Stores one idempotency record per (idempotency_key, approval_id, action) tuple.
-- Allows approveApproval and rejectApproval to return the original committed result
-- on retry without creating duplicate command_journal rows or re-settling the approval.
--
-- Retention: at least 7 days (expires_at_ms enforced by cleanup job; not by DB constraint
-- so cleanup failures are non-fatal and do not corrupt the idempotency guarantee).

CREATE TABLE approval_mutation_idempotency (
    idempotency_key     TEXT NOT NULL PRIMARY KEY,
    approval_id         TEXT NOT NULL,
    action              TEXT NOT NULL CHECK(action IN ('approve', 'reject')),
    caller_fingerprint  TEXT NOT NULL,
    request_id          TEXT NULL,
    command_journal_id  TEXT NOT NULL,
    result_hash         TEXT NULL,
    committed_at_ms     INTEGER NOT NULL,
    expires_at_ms       INTEGER NOT NULL,
    created_at          TEXT NOT NULL
);

CREATE INDEX idx_ami_approval_id ON approval_mutation_idempotency(approval_id);
CREATE INDEX idx_ami_expires_at  ON approval_mutation_idempotency(expires_at_ms);
