-- P086: Tighten resurrection_deadline_at invariants.
--
-- Migration 079/081 only require resurrection_deadline_at to be NULL or belong to
-- a provider_session_resurrection row. This migration adds two additional constraints:
--   1. Active (non-terminal) resurrection rows MUST have a non-null deadline.
--   2. Terminal resurrection phases (completed, failed_closed) MUST have a null deadline.
--
-- SQLite cannot ALTER an existing CHECK constraint. We use the standard table-rename
-- pattern: create a replacement table with the widened constraints, copy rows, drop
-- original, and rename. All indexes are recreated verbatim from migration 081.
--
-- Foreign-key enforcement is disabled by the migrate.rs pool (foreign_keys=false).

CREATE TABLE agent_work_continuations_new2 (
  id                              TEXT    PRIMARY KEY,
  run_id                          TEXT    NOT NULL REFERENCES runs(id),
  stage_execution_id              TEXT    NOT NULL REFERENCES stage_executions(id),
  agent_execution_id              TEXT    NOT NULL REFERENCES agent_executions(id),
  command_journal_id              TEXT    REFERENCES command_journal(id),
  session_generation_id           TEXT,
  provider_session_id             TEXT,

  mode                            TEXT    NOT NULL
                                    CHECK (mode IN (
                                      'live_handle_continuation',
                                      'provider_session_resurrection'
                                    )),
  trigger_kind                    TEXT    NOT NULL
                                    CHECK (trigger_kind IN ('operator_mcp', 'lead_auto')),

  status                          TEXT    NOT NULL DEFAULT 'accepted'
                                    CHECK (status IN (
                                      'accepted', 'queued', 'starting', 'running',
                                      'preflight_passed', 'prompt_sent', 'observing',
                                      'worktree_observed',
                                      'needs_continuation_reconciliation',
                                      'finalizing', 'cancelling',
                                      'succeeded', 'no_progress', 'failed', 'cancelled'
                                    )),

  failure_reason                  TEXT,
  reconciliation_status           TEXT,

  idempotency_scope               TEXT    NOT NULL,
  idempotency_key                 TEXT    NOT NULL,
  request_fingerprint_sha256      TEXT    NOT NULL
                                    CHECK (length(request_fingerprint_sha256) = 64
                                      AND request_fingerprint_sha256 NOT GLOB '*[^0-9a-f]*'),

  canonical_request_artifact_id   TEXT,
  attach_receipt_artifact_id      TEXT,
  evidence_bundle_artifact_id     TEXT,
  worktree_readback_artifact_id   TEXT,
  continuation_report_artifact_id TEXT,
  response_fingerprint_sha256     TEXT
                                    CHECK (response_fingerprint_sha256 IS NULL
                                      OR (length(response_fingerprint_sha256) = 64
                                        AND response_fingerprint_sha256 NOT GLOB '*[^0-9a-f]*')),
  response_artifact_id            TEXT,
  result_or_no_progress_artifact_id TEXT,

  conflict_count                  INTEGER NOT NULL DEFAULT 0,

  lead_decision_artifact_id       TEXT,
  lead_decision_artifact_sha256   TEXT
                                    CHECK (lead_decision_artifact_sha256 IS NULL
                                      OR (length(lead_decision_artifact_sha256) = 64
                                        AND lead_decision_artifact_sha256 NOT GLOB '*[^0-9a-f]*')),
  continuation_instruction_sha256 TEXT
                                    CHECK (continuation_instruction_sha256 IS NULL
                                      OR (length(continuation_instruction_sha256) = 64
                                        AND continuation_instruction_sha256 NOT GLOB '*[^0-9a-f]*')),

  budget_json                     TEXT,
  metadata_json                   TEXT,

  created_at                      TEXT    NOT NULL,
  updated_at                      TEXT    NOT NULL,

  resurrection_phase              TEXT
                                    CHECK (
                                      resurrection_phase IS NULL
                                      OR (
                                        mode = 'provider_session_resurrection'
                                        AND resurrection_phase IN (
                                          'admitted',
                                          'launching',
                                          'launched',
                                          'attaching',
                                          'attached_unprompted',
                                          'prompting',
                                          'settling',
                                          'cancelling',
                                          'completed',
                                          'failed_closed'
                                        )
                                      )
                                    ),

  -- Tightened deadline invariant:
  --   1. Only provider_session_resurrection rows may have a non-null deadline (unchanged).
  --   2. Terminal phases (completed, failed_closed) MUST have a null deadline.
  --   3. Non-terminal resurrection rows MUST have a non-null deadline (active progress tracked).
  resurrection_deadline_at        TEXT
                                    CHECK (
                                      resurrection_deadline_at IS NULL
                                      OR (
                                        mode = 'provider_session_resurrection'
                                        AND resurrection_phase IS NOT NULL
                                        AND resurrection_phase NOT IN ('completed', 'failed_closed')
                                      )
                                    )
                                    CHECK (
                                      mode != 'provider_session_resurrection'
                                      OR resurrection_phase IS NULL
                                      OR resurrection_phase IN ('completed', 'failed_closed')
                                      OR resurrection_deadline_at IS NOT NULL
                                    ),

  resurrection_last_heartbeat_at  TEXT
                                    CHECK (
                                      resurrection_last_heartbeat_at IS NULL
                                      OR mode = 'provider_session_resurrection'
                                    ),
  resurrection_timeout_class      TEXT
                                    CHECK (
                                      resurrection_timeout_class IS NULL
                                      OR (
                                        mode = 'provider_session_resurrection'
                                        AND resurrection_timeout_class IN (
                                          'dequeue_timeout',
                                          'launch_timeout',
                                          'attach_timeout',
                                          'unprompted_timeout',
                                          'prompt_timeout',
                                          'settle_timeout',
                                          'cancel_timeout'
                                        )
                                      )
                                    ),
  timeout_settled_at              TEXT NULL
                                    CHECK (
                                      timeout_settled_at IS NULL
                                      OR mode = 'provider_session_resurrection'
                                    )
);

-- Copy all existing rows, nullifying deadline for terminal phases (satisfy new constraint).
INSERT INTO agent_work_continuations_new2
SELECT
  id, run_id, stage_execution_id, agent_execution_id, command_journal_id,
  session_generation_id, provider_session_id, mode, trigger_kind, status,
  failure_reason, reconciliation_status, idempotency_scope, idempotency_key,
  request_fingerprint_sha256, canonical_request_artifact_id, attach_receipt_artifact_id,
  evidence_bundle_artifact_id, worktree_readback_artifact_id, continuation_report_artifact_id,
  response_fingerprint_sha256, response_artifact_id, result_or_no_progress_artifact_id,
  conflict_count, lead_decision_artifact_id, lead_decision_artifact_sha256,
  continuation_instruction_sha256, budget_json, metadata_json, created_at, updated_at,
  resurrection_phase,
  -- Enforce: terminal phases must have null deadline; admitted phase may have null deadline.
  CASE
    WHEN resurrection_phase IN ('completed', 'failed_closed') THEN NULL
    ELSE resurrection_deadline_at
  END,
  resurrection_last_heartbeat_at,
  resurrection_timeout_class,
  timeout_settled_at
FROM agent_work_continuations;

DROP TABLE agent_work_continuations;
ALTER TABLE agent_work_continuations_new2 RENAME TO agent_work_continuations;

-- Recreate all indexes from migrations 065, 079, and 081.
CREATE INDEX IF NOT EXISTS idx_awc_agent_created_at
  ON agent_work_continuations(agent_execution_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_awc_run_status
  ON agent_work_continuations(run_id, status);

CREATE INDEX IF NOT EXISTS idx_awc_stage
  ON agent_work_continuations(stage_execution_id, status);

CREATE INDEX IF NOT EXISTS idx_awc_recon
  ON agent_work_continuations(reconciliation_status)
  WHERE reconciliation_status IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_awc_admission
  ON agent_work_continuations(status, created_at)
  WHERE status IN ('accepted', 'queued', 'starting');

CREATE INDEX IF NOT EXISTS idx_awc_recovery
  ON agent_work_continuations(status, updated_at)
  WHERE status NOT IN ('succeeded', 'no_progress', 'failed', 'cancelled');

CREATE INDEX IF NOT EXISTS idx_awc_idempotency
  ON agent_work_continuations(idempotency_scope, idempotency_key, status);

CREATE UNIQUE INDEX IF NOT EXISTS uniq_continuations_active_idempotency_run_scoped
  ON agent_work_continuations(run_id, idempotency_key)
  WHERE status IN (
    'accepted',
    'queued',
    'starting',
    'running',
    'preflight_passed',
    'prompt_sent',
    'observing',
    'worktree_observed',
    'needs_continuation_reconciliation',
    'finalizing',
    'cancelling'
  );

CREATE UNIQUE INDEX IF NOT EXISTS uniq_continuations_single_active_resurrection
  ON agent_work_continuations(agent_execution_id)
  WHERE mode = 'provider_session_resurrection'
    AND status IN (
      'accepted',
      'queued',
      'starting',
      'running',
      'preflight_passed',
      'prompt_sent',
      'observing',
      'worktree_observed',
      'needs_continuation_reconciliation',
      'finalizing',
      'cancelling'
    );

CREATE INDEX IF NOT EXISTS idx_agent_work_continuations_resurrection_deadline_active
  ON agent_work_continuations(resurrection_deadline_at)
  WHERE mode = 'provider_session_resurrection'
    AND status IN (
      'accepted',
      'queued',
      'starting',
      'running',
      'preflight_passed',
      'prompt_sent',
      'observing',
      'worktree_observed',
      'needs_continuation_reconciliation',
      'finalizing',
      'cancelling'
    );
