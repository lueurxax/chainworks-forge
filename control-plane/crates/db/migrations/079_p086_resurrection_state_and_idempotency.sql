-- P086: Provider-session resurrection durable state and run-scoped idempotency.
--
-- Existing live_handle_continuation rows keep NULL resurrection fields. New
-- provider_session_resurrection rows start in resurrection_phase='admitted'
-- through repository admission code and later phases are driven by the engine.

ALTER TABLE agent_work_continuations
  ADD COLUMN resurrection_phase TEXT
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
          'completed',
          'failed_closed'
        )
      )
    );

ALTER TABLE agent_work_continuations
  ADD COLUMN resurrection_deadline_at TEXT
    CHECK (
      resurrection_deadline_at IS NULL
      OR mode = 'provider_session_resurrection'
    );

ALTER TABLE agent_work_continuations
  ADD COLUMN resurrection_last_heartbeat_at TEXT
    CHECK (
      resurrection_last_heartbeat_at IS NULL
      OR mode = 'provider_session_resurrection'
    );

ALTER TABLE agent_work_continuations
  ADD COLUMN resurrection_timeout_class TEXT
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
    );

CREATE TABLE IF NOT EXISTS continuation_terminal_idempotency_ledger (
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
                                    'succeeded', 'no_progress', 'failed', 'cancelled'
                                  )),
  terminal_at                  TEXT    NOT NULL,
  retention_until              TEXT    NOT NULL,
  created_at                   TEXT    NOT NULL
);

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

CREATE UNIQUE INDEX IF NOT EXISTS idx_terminal_idempotency_ledger_uniq_prompt_related_run_key
  ON continuation_terminal_idempotency_ledger(run_id, idempotency_key)
  WHERE prompt_related = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_terminal_idempotency_ledger_uniq_pre_prompt_run_key
  ON continuation_terminal_idempotency_ledger(run_id, idempotency_key)
  WHERE prompt_related = 0;

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
