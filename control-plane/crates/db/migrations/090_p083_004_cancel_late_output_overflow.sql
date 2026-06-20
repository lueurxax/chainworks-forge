-- P083-004: cancel late output overflow latch table.
-- Records overflow latch activations when provider output arrives after
-- cancellation. normalized_run_id and normalized_provider_session_id are
-- stored generated columns used in the unique latch key to prevent
-- case/whitespace divergence. projection_mutation_blocked confirms that
-- active projections cannot be mutated by quarantined output.

CREATE TABLE IF NOT EXISTS cancel_late_output_overflow (
  overflow_id TEXT PRIMARY KEY,
  scope TEXT NOT NULL CHECK(scope IN ('session','run','global')),
  run_id TEXT,
  provider_session_id TEXT,
  cancellation_epoch INTEGER NOT NULL,
  overflow_kind TEXT NOT NULL CHECK(overflow_kind IN (
    'message_count',
    'session_bytes',
    'elapsed_time',
    'run_bytes',
    'global_bytes'
  )),
  dropped_message_count INTEGER NOT NULL DEFAULT 0,
  dropped_byte_count INTEGER NOT NULL DEFAULT 0,
  quarantine_uri TEXT,
  reservation_release_state TEXT NOT NULL DEFAULT 'active',
  projection_mutation_blocked INTEGER NOT NULL DEFAULT 0,
  latched_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  normalized_run_id TEXT GENERATED ALWAYS AS (
    LOWER(TRIM(COALESCE(run_id, '')))
  ) STORED,
  normalized_provider_session_id TEXT GENERATED ALWAYS AS (
    LOWER(TRIM(COALESCE(provider_session_id, '')))
  ) STORED
);

CREATE UNIQUE INDEX IF NOT EXISTS cancel_late_output_overflow_latch_uniq
  ON cancel_late_output_overflow(
    scope,
    normalized_run_id,
    normalized_provider_session_id,
    cancellation_epoch,
    overflow_kind
  );

CREATE INDEX IF NOT EXISTS cancel_late_output_overflow_scope_idx
  ON cancel_late_output_overflow(scope, normalized_run_id, overflow_kind);
