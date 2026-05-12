-- P078: Durable side-effect ledger.
--
-- Additive migration: creates side_effects, side_effect_attempts, and
-- side_effect_settlements tables. Existing release receipts remain readable.
-- No filesystem-dependent backfill is performed here.
--
-- P078-SEC-HIGH-001 fix: partial unique index on (run_id, target_key) for
-- unresolved rows enforces DB-level atomicity so two concurrent callers
-- cannot both prepare distinct idempotency keys for the same target.

CREATE TABLE IF NOT EXISTS side_effects (
  id                              TEXT    PRIMARY KEY,
  run_id                          TEXT    NOT NULL,
  stage_execution_id              TEXT    NOT NULL,
  agent_execution_id              TEXT,
  effect_kind                     TEXT    NOT NULL
                                    CHECK (effect_kind IN (
                                      'git_commit','git_push','build_archive',
                                      'connect_upload','tag_create','artifact_publish'
                                    )),
  target_key                      TEXT    NOT NULL,
  idempotency_key                 TEXT    NOT NULL,
  idempotency_key_version         INTEGER NOT NULL DEFAULT 1,
  request_fingerprint             TEXT    NOT NULL,
  request_fingerprint_version     INTEGER NOT NULL DEFAULT 1,
  -- status: prepared | executing | externally_observed | needs_reconciliation |
  --         settled | reconciled | conflict | unrecoverable
  status                          TEXT    NOT NULL DEFAULT 'prepared'
                                    CHECK (status IN (
                                      'prepared','executing','externally_observed',
                                      'needs_reconciliation','settled','reconciled',
                                      'conflict','unrecoverable'
                                    )),
  owner_instance_id               TEXT,
  lease_acquired_at               TEXT,
  lease_renewed_at                TEXT,
  lease_expires_at                TEXT,
  deadline_at                     TEXT,
  external_write_started_at       TEXT,
  external_write_attempted        INTEGER NOT NULL DEFAULT 0
                                    CHECK (external_write_attempted IN (0, 1)),
  attempt_budget_remaining        INTEGER NOT NULL DEFAULT 3,
  expected_evidence_json          TEXT,
  observed_evidence_summary_json  TEXT,
  evidence_root                   TEXT,
  last_error_kind                 TEXT,
  last_error                      TEXT,
  settlement_txn_id               TEXT,
  created_at                      TEXT    NOT NULL,
  updated_at                      TEXT    NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_side_effects_idempotency_key
  ON side_effects(idempotency_key);

-- P078-SEC-HIGH-001: DB-enforced guard — at most one unresolved row per
-- (run_id, target_key). Terminal statuses (settled, reconciled) are excluded
-- so that a new intent for the same target can be prepared after closeout.
CREATE UNIQUE INDEX IF NOT EXISTS idx_side_effects_unresolved_run_target_key
  ON side_effects(run_id, target_key)
  WHERE status IN (
    'prepared','executing','externally_observed',
    'needs_reconciliation','conflict','unrecoverable'
  );

CREATE INDEX IF NOT EXISTS idx_side_effects_run_id
  ON side_effects(run_id);

CREATE INDEX IF NOT EXISTS idx_side_effects_status
  ON side_effects(status);

CREATE INDEX IF NOT EXISTS idx_side_effects_target_key_status
  ON side_effects(target_key, status);

CREATE INDEX IF NOT EXISTS idx_side_effects_lease_expires
  ON side_effects(lease_expires_at)
  WHERE status IN ('executing');

CREATE TABLE IF NOT EXISTS side_effect_attempts (
  id                              TEXT    PRIMARY KEY,
  side_effect_id                  TEXT    NOT NULL REFERENCES side_effects(id),
  attempt_number                  INTEGER NOT NULL,
  -- attempt_kind: initial | retry | readback | local_only
  attempt_kind                    TEXT    NOT NULL DEFAULT 'initial'
                                    CHECK (attempt_kind IN ('initial','retry','readback','local_only')),
  owner_instance_id               TEXT    NOT NULL,
  started_at                      TEXT    NOT NULL,
  completed_at                    TEXT,
  deadline_at                     TEXT,
  -- exit_status: succeeded | failed | aborted | timeout | unknown
  exit_status                     TEXT
                                    CHECK (exit_status IS NULL OR exit_status IN (
                                      'succeeded','failed','aborted','timeout','unknown'
                                    )),
  stdout_path                     TEXT,
  stderr_path                     TEXT,
  trace_path                      TEXT,
  observed_evidence_summary_json  TEXT,
  error_kind                      TEXT,
  error                           TEXT
);

CREATE INDEX IF NOT EXISTS idx_side_effect_attempts_side_effect_id
  ON side_effect_attempts(side_effect_id);

CREATE TABLE IF NOT EXISTS side_effect_settlements (
  id                              TEXT    PRIMARY KEY,
  side_effect_id                  TEXT    NOT NULL REFERENCES side_effects(id),
  -- settlement_status: settled | reconciled | conflict | unrecoverable
  settlement_status               TEXT    NOT NULL
                                    CHECK (settlement_status IN (
                                      'settled','reconciled','conflict','unrecoverable'
                                    )),
  -- settlement_source: executor | mcp_operator | startup_recovery | watchdog
  settlement_source               TEXT    NOT NULL
                                    CHECK (settlement_source IN (
                                      'executor','mcp_operator','startup_recovery','watchdog'
                                    )),
  receipt_artifact_id             TEXT,
  reconciliation_report_artifact_id TEXT,
  applied_at                      TEXT    NOT NULL,
  applied_by                      TEXT,
  disposition_id                  TEXT,
  settlement_attempt_id           TEXT    REFERENCES side_effect_attempts(id),
  decision_json                   TEXT,
  decision_json_hash              TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_side_effect_settlements_side_effect_id
  ON side_effect_settlements(side_effect_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_side_effect_settlements_disposition_id
  ON side_effect_settlements(disposition_id)
  WHERE disposition_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_side_effect_settlements_receipt_artifact_id
  ON side_effect_settlements(receipt_artifact_id)
  WHERE receipt_artifact_id IS NOT NULL;
