-- P086: Extend continuation_terminal_idempotency_ledger to accept
-- needs_continuation_reconciliation as a valid terminal_status.
--
-- The original migration 079 only allowed succeeded/no_progress/failed/cancelled.
-- When a resurrection settling-phase timeout resolves to needs_continuation_reconciliation
-- (ambiguous outputs may exist, prompt_related=1), the settlement code must write a ledger
-- row to prevent duplicate re-admission with the same idempotency key.
--
-- SQLite cannot modify CHECK constraints with ALTER TABLE; we use the standard
-- table-rename pattern to recreate with the extended constraint.
--
-- All indexes from migration 079 are recreated verbatim below.

CREATE TABLE continuation_terminal_idempotency_ledger_new (
  id                           TEXT    PRIMARY KEY,
  run_id                       TEXT    NOT NULL REFERENCES runs(id),
  idempotency_key              TEXT    NOT NULL,
  request_fingerprint_sha256   TEXT    NOT NULL
                                  CHECK (length(request_fingerprint_sha256) = 64
                                    AND request_fingerprint_sha256 NOT GLOB '*[^0-9a-f]*'),
  prompt_related               INTEGER NOT NULL CHECK (prompt_related IN (0, 1)),
  continuation_id              TEXT    NOT NULL REFERENCES agent_work_continuations(id),
  terminal_status              TEXT    NOT NULL
                                  CHECK (terminal_status IN (
                                    'succeeded', 'no_progress', 'failed', 'cancelled',
                                    'needs_continuation_reconciliation'
                                  )),
  terminal_at                  TEXT    NOT NULL,
  retention_until              TEXT    NOT NULL,
  created_at                   TEXT    NOT NULL
);

INSERT INTO continuation_terminal_idempotency_ledger_new
SELECT id, run_id, idempotency_key, request_fingerprint_sha256,
       prompt_related, continuation_id, terminal_status,
       terminal_at, retention_until, created_at
FROM continuation_terminal_idempotency_ledger;

DROP TABLE continuation_terminal_idempotency_ledger;
ALTER TABLE continuation_terminal_idempotency_ledger_new
  RENAME TO continuation_terminal_idempotency_ledger;

CREATE UNIQUE INDEX IF NOT EXISTS idx_terminal_idempotency_ledger_uniq_prompt_related_run_key
  ON continuation_terminal_idempotency_ledger(run_id, idempotency_key)
  WHERE prompt_related = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_terminal_idempotency_ledger_uniq_pre_prompt_run_key
  ON continuation_terminal_idempotency_ledger(run_id, idempotency_key)
  WHERE prompt_related = 0;
