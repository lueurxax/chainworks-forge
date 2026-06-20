-- P083-008: Add 'dispatching' to shutdown_signal_side_effects.intent_state CHECK constraint.
--
-- SEC-P083-HIGH-001: The dispatcher must atomically claim a planned signal row by
-- transitioning it to 'dispatching' before calling kill(). This prevents a concurrent
-- dispatcher or crash-recovery pass from re-issuing the same OS signal.
--
-- SQLite does not support ALTER TABLE ... MODIFY CONSTRAINT, so we recreate the table.

PRAGMA foreign_keys = OFF;

CREATE TABLE shutdown_signal_side_effects_new (
  signal_effect_id TEXT PRIMARY KEY,
  provider_session_id TEXT NOT NULL,
  shutdown_epoch INTEGER NOT NULL,
  process_id INTEGER NOT NULL CHECK(process_id > 0),
  process_start_identity TEXT NOT NULL,
  signal_kind TEXT NOT NULL CHECK(signal_kind IN ('graceful','kill')),
  generation INTEGER NOT NULL,
  intent_state TEXT NOT NULL CHECK(intent_state IN (
    'planned',
    'dispatching',
    'issued',
    'observed',
    'suppressed',
    'identity_mismatch'
  )),
  issued_at_monotonic_ms INTEGER,
  issued_at_wall_clock TEXT,
  observed_exit_at_monotonic_ms INTEGER,
  error_code TEXT,
  UNIQUE(provider_session_id, shutdown_epoch, signal_kind, generation)
);

INSERT INTO shutdown_signal_side_effects_new
  SELECT signal_effect_id, provider_session_id, shutdown_epoch,
         process_id, process_start_identity, signal_kind, generation,
         intent_state, issued_at_monotonic_ms, issued_at_wall_clock,
         observed_exit_at_monotonic_ms, error_code
    FROM shutdown_signal_side_effects;

DROP TABLE shutdown_signal_side_effects;

ALTER TABLE shutdown_signal_side_effects_new
  RENAME TO shutdown_signal_side_effects;

CREATE UNIQUE INDEX IF NOT EXISTS shutdown_signal_side_effect_unique
  ON shutdown_signal_side_effects(provider_session_id, shutdown_epoch, signal_kind, generation);

CREATE INDEX IF NOT EXISTS shutdown_signal_side_effects_session_idx
  ON shutdown_signal_side_effects(provider_session_id, shutdown_epoch);

PRAGMA foreign_keys = ON;
