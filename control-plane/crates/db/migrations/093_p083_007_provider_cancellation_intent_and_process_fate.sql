-- P083-007: provider sessions table and cancellation intent tracking.
-- provider_sessions is the authoritative lifecycle record for provider
-- processes. process_fate tracks the durable outcome of process identity
-- checks during shutdown and recovery. provider_cancellation_intents
-- records durable cancellation requests; held+identity_ambiguous intents
-- are not auto-retried on restart.

CREATE TABLE IF NOT EXISTS provider_sessions (
  provider_session_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  agent_execution_id TEXT,
  provider TEXT NOT NULL,
  lifecycle_state TEXT NOT NULL DEFAULT 'registered' CHECK(lifecycle_state IN (
    'registered',
    'spawn_error_no_child',
    'launch_handshake',
    'live',
    'self_exit_observed',
    'terminated_graceful',
    'terminated_by_kill',
    'orphan_settled',
    'shutdown_interrupted',
    'backpressure_cutoff'
  )),
  process_id INTEGER CHECK(process_id IS NULL OR process_id > 0),
  process_start_identity TEXT,
  process_fate TEXT NOT NULL DEFAULT 'running' CHECK(process_fate IN (
    'running',
    'backpressure_cutoff_shutdown_pending',
    'absent_verified',
    'interrupted_receipt_recorded',
    'identity_ambiguous'
  )),
  process_fate_updated_at TEXT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS provider_sessions_process_fate_idx
  ON provider_sessions(process_fate);

CREATE INDEX IF NOT EXISTS provider_sessions_run_id_idx
  ON provider_sessions(run_id);

CREATE TABLE IF NOT EXISTS provider_cancellation_intents (
  provider_session_id TEXT NOT NULL,
  cancellation_epoch INTEGER NOT NULL,
  intent_state TEXT NOT NULL CHECK(intent_state IN (
    'requested',
    'shutdown_started',
    'settled',
    'held'
  )),
  reason TEXT NOT NULL CHECK(reason IN (
    'operator_cancel',
    'backpressure_cutoff',
    'shutdown_recovery'
  )),
  requested_at_monotonic_ms INTEGER NOT NULL,
  requested_at_wall_clock TEXT NOT NULL,
  baseline_sample_id TEXT,
  shutdown_epoch INTEGER NULL,
  shutdown_epoch_assigned_at TEXT NULL,
  PRIMARY KEY(provider_session_id, cancellation_epoch)
);

-- shutdown_started and settled rows must have a shutdown_epoch assigned.
-- (Verified by V7 verification query: SELECT provider_session_id FROM
-- provider_cancellation_intents WHERE intent_state IN ('shutdown_started','settled')
-- AND shutdown_epoch IS NULL → zero rows)

CREATE INDEX IF NOT EXISTS provider_cancellation_intents_shutdown_epoch_idx
  ON provider_cancellation_intents(provider_session_id, shutdown_epoch)
  WHERE shutdown_epoch IS NOT NULL;

CREATE INDEX IF NOT EXISTS provider_cancellation_intents_state_idx
  ON provider_cancellation_intents(intent_state, reason);
