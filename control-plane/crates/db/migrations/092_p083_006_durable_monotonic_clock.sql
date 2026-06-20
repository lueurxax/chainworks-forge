-- P083-006: durable monotonic clock samples.
-- Records monotonic/wall-clock correlation samples keyed by boot_id so
-- that recovery can compare elapsed time across daemon restarts or host
-- reboots. A baseline sample is written at daemon start; verification
-- expects at least one baseline row after startup.

CREATE TABLE IF NOT EXISTS durable_monotonic_clock_samples (
  sample_id TEXT PRIMARY KEY,
  boot_id TEXT NOT NULL,
  sample_state TEXT NOT NULL CHECK(sample_state IN (
    'baseline',
    'checkpoint',
    'rollback_detected',
    'stale_fallback'
  )),
  monotonic_ms INTEGER NOT NULL,
  observed_at_wall_clock TEXT NOT NULL,
  prior_sample_id TEXT,
  clock_delta_ms INTEGER,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS durable_monotonic_clock_samples_boot_idx
  ON durable_monotonic_clock_samples(boot_id, observed_at_wall_clock);

CREATE INDEX IF NOT EXISTS durable_monotonic_clock_samples_state_idx
  ON durable_monotonic_clock_samples(sample_state);
