-- P083-003: shutdown interrupted receipts and signal side effects.
-- shutdown_interrupted_receipts tracks when provider sessions could not be
-- signaled before a deadline expired. queue_rank is non-null only for
-- queued_no_signal rows; all other interrupted states store NULL queue_rank.
-- shutdown_signal_side_effects tracks planned/issued/observed/suppressed
-- graceful and kill signals with generation replay semantics.

CREATE TABLE IF NOT EXISTS shutdown_interrupted_receipts (
  receipt_id TEXT PRIMARY KEY,
  provider_session_id TEXT NOT NULL,
  shutdown_epoch INTEGER NOT NULL,
  receipt_generation INTEGER NOT NULL,
  interrupted_state TEXT NOT NULL CHECK(interrupted_state IN (
    'grace_deadline_expired',
    'kill_signal_issued',
    'kill_pid_exit_observed',
    'queued_no_signal',
    'shutdown_interrupted'
  )),
  queue_rank INTEGER NULL,
  created_at TEXT NOT NULL,
  recovered_at TEXT,
  UNIQUE(provider_session_id, shutdown_epoch, receipt_generation),
  -- queue_rank must be non-null exactly when interrupted_state = 'queued_no_signal'.
  CHECK(
    (interrupted_state = 'queued_no_signal' AND queue_rank IS NOT NULL)
    OR (interrupted_state <> 'queued_no_signal' AND queue_rank IS NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS shutdown_interrupted_receipts_epoch_generation_uniq
  ON shutdown_interrupted_receipts(provider_session_id, shutdown_epoch, receipt_generation);

CREATE INDEX IF NOT EXISTS shutdown_interrupted_receipts_session_idx
  ON shutdown_interrupted_receipts(provider_session_id, shutdown_epoch);

CREATE TABLE IF NOT EXISTS shutdown_signal_side_effects (
  signal_effect_id TEXT PRIMARY KEY,
  provider_session_id TEXT NOT NULL,
  shutdown_epoch INTEGER NOT NULL,
  process_id INTEGER NOT NULL CHECK(process_id > 0),
  process_start_identity TEXT NOT NULL,
  signal_kind TEXT NOT NULL CHECK(signal_kind IN ('graceful','kill')),
  generation INTEGER NOT NULL,
  intent_state TEXT NOT NULL CHECK(intent_state IN (
    'planned',
    'issued',
    'observed',
    'suppressed',
    'identity_mismatch'
  )),
  issued_at_monotonic_ms INTEGER,
  issued_at_wall_clock TEXT,
  observed_exit_at_monotonic_ms INTEGER,
  baseline_sample_id TEXT,
  error_code TEXT,
  UNIQUE(provider_session_id, shutdown_epoch, signal_kind, generation)
);

CREATE UNIQUE INDEX IF NOT EXISTS shutdown_signal_side_effect_unique
  ON shutdown_signal_side_effects(provider_session_id, shutdown_epoch, signal_kind, generation);

CREATE INDEX IF NOT EXISTS shutdown_signal_side_effects_session_idx
  ON shutdown_signal_side_effects(provider_session_id, shutdown_epoch);
