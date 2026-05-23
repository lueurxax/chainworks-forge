-- P086: Agent Work Continuation and Lead-Directed Same-Session Resumption.
--
-- Adds three tables:
--   agent_work_continuations        -- one row per continuation turn
--   agent_external_side_effect_ledger -- durable ordered side-effect log
--   supervised_workers_continuation -- worker supervision with heartbeat
--
-- SHA-256 TEXT columns follow: c IS NULL OR (length(c)=64 AND c NOT GLOB '*[^0-9a-f]*')
-- NOT NULL SHA-256 columns omit the c IS NULL OR prefix.

CREATE TABLE IF NOT EXISTS agent_work_continuations (
  -- identity
  id                              TEXT    PRIMARY KEY,
  run_id                          TEXT    NOT NULL REFERENCES runs(id),
  stage_execution_id              TEXT    NOT NULL REFERENCES stage_executions(id),
  agent_execution_id              TEXT    NOT NULL REFERENCES agent_executions(id),
  command_journal_id              TEXT    REFERENCES command_journal(id),
  session_generation_id           TEXT,
  provider_session_id             TEXT,

  -- mode + trigger
  -- mode: live_handle_continuation | provider_session_resurrection
  mode                            TEXT    NOT NULL
                                    CHECK (mode IN (
                                      'live_handle_continuation',
                                      'provider_session_resurrection'
                                    )),
  -- trigger_kind: operator_mcp | lead_auto
  trigger_kind                    TEXT    NOT NULL
                                    CHECK (trigger_kind IN ('operator_mcp', 'lead_auto')),

  -- lifecycle status (terminal: succeeded, no_progress, failed, cancelled)
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

  -- idempotency
  idempotency_scope               TEXT    NOT NULL,
  idempotency_key                 TEXT    NOT NULL,
  request_fingerprint_sha256      TEXT    NOT NULL
                                    CHECK (length(request_fingerprint_sha256) = 64
                                      AND request_fingerprint_sha256 NOT GLOB '*[^0-9a-f]*'),

  -- artifacts
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

  -- conflict counter for idempotency_conflict audit events
  conflict_count                  INTEGER NOT NULL DEFAULT 0,

  -- lead auto: decision artifact binding
  lead_decision_artifact_id       TEXT,
  lead_decision_artifact_sha256   TEXT
                                    CHECK (lead_decision_artifact_sha256 IS NULL
                                      OR (length(lead_decision_artifact_sha256) = 64
                                        AND lead_decision_artifact_sha256 NOT GLOB '*[^0-9a-f]*')),
  continuation_instruction_sha256 TEXT
                                    CHECK (continuation_instruction_sha256 IS NULL
                                      OR (length(continuation_instruction_sha256) = 64
                                        AND continuation_instruction_sha256 NOT GLOB '*[^0-9a-f]*')),

  -- budget and metadata
  budget_json                     TEXT,
  metadata_json                   TEXT,

  created_at                      TEXT    NOT NULL,
  updated_at                      TEXT    NOT NULL
);

-- Proposal index aliases (must match rollout_contract migration_contract.indexes exactly)
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

-- Unique idempotency: one accepted/active row per scope+key (enforced
-- by admission logic, not DB unique constraint, because terminal rows
-- are retained for replay).
CREATE INDEX IF NOT EXISTS idx_awc_idempotency
  ON agent_work_continuations(idempotency_scope, idempotency_key, status);

-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS agent_external_side_effect_ledger (
  -- composite primary: one row per (continuation_id, sequence_number)
  continuation_id           TEXT    NOT NULL
                              REFERENCES agent_work_continuations(id),
  idempotency_key           TEXT    NOT NULL,
  -- side_effect_kind: runtime_lease | worktree_lease | provider_send | provider_cancel | provider_session_attach
  side_effect_kind          TEXT    NOT NULL
                              CHECK (side_effect_kind IN (
                                'runtime_lease',
                                'worktree_lease',
                                'provider_send',
                                'provider_cancel',
                                'provider_session_attach'
                              )),
  sequence_number           INTEGER NOT NULL,
  -- status: planned | started | committed | released | failed
  status                    TEXT    NOT NULL DEFAULT 'planned'
                              CHECK (status IN ('planned', 'started', 'committed', 'released', 'failed')),
  payload_hash              TEXT
                              CHECK (payload_hash IS NULL
                                OR (length(payload_hash) = 64
                                  AND payload_hash NOT GLOB '*[^0-9a-f]*')),
  request_fingerprint_sha256 TEXT
                              CHECK (request_fingerprint_sha256 IS NULL
                                OR (length(request_fingerprint_sha256) = 64
                                  AND request_fingerprint_sha256 NOT GLOB '*[^0-9a-f]*')),
  response_fingerprint_sha256 TEXT
                              CHECK (response_fingerprint_sha256 IS NULL
                                OR (length(response_fingerprint_sha256) = 64
                                  AND response_fingerprint_sha256 NOT GLOB '*[^0-9a-f]*')),
  committed_at              TEXT,

  PRIMARY KEY (continuation_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS idx_ledger_cont_seq
  ON agent_external_side_effect_ledger(continuation_id, sequence_number);

CREATE INDEX IF NOT EXISTS idx_ledger_unresolved
  ON agent_external_side_effect_ledger(committed_at, status)
  WHERE status IN ('planned', 'started') AND committed_at IS NULL;

-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS supervised_workers_continuation (
  continuation_id               TEXT    NOT NULL
                                  REFERENCES agent_work_continuations(id),
  worker_pid                    INTEGER NOT NULL,
  daemon_generation_id          TEXT    NOT NULL,
  session_generation_id         TEXT    NOT NULL,
  registered_at                 TEXT    NOT NULL,
  last_heartbeat_at             TEXT,
  released_at                   TEXT,
  -- release_reason: completed | cancelled | crashed | stale_generation | stale_generation_reaped | stale_generation_reap_unverified | forced
  release_reason                TEXT,
  stale_generation_evidence_json TEXT,

  PRIMARY KEY (continuation_id, worker_pid, daemon_generation_id)
);

CREATE INDEX IF NOT EXISTS idx_swc_heartbeat
  ON supervised_workers_continuation(last_heartbeat_at)
  WHERE released_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_swc_generation
  ON supervised_workers_continuation(daemon_generation_id, continuation_id)
  WHERE released_at IS NULL;

-- At most one active (unreleased) supervised worker per continuation.
CREATE UNIQUE INDEX IF NOT EXISTS uniq_swc_active_continuation
  ON supervised_workers_continuation(continuation_id)
  WHERE released_at IS NULL;
