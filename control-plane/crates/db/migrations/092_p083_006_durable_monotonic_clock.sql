-- P083-006: durable monotonic clock samples.
-- Records monotonic/wall-clock correlation samples keyed by boot_id so
-- that recovery can compare elapsed time across daemon restarts or host
-- reboots. A baseline sample is written at daemon start; verification
-- expects at least one baseline row after startup.

CREATE TABLE IF NOT EXISTS durable_monotonic_clock_samples (
  sample_id TEXT PRIMARY KEY,
  boot_id TEXT NOT NULL,
  baseline_generation INTEGER NOT NULL CHECK(baseline_generation > 0),
  sample_state TEXT NOT NULL CHECK(sample_state IN (
    'baseline',
    'periodic',
    'fallback_wall_only'
  )),
  monotonic_ms INTEGER NOT NULL,
  observed_at_wall_clock TEXT NOT NULL,
  wall_clock_iso8601 TEXT NOT NULL,
  clock_skew_ms INTEGER NOT NULL DEFAULT 0,
  prior_sample_id TEXT,
  clock_delta_ms INTEGER,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS durable_monotonic_clock_samples_boot_idx
  ON durable_monotonic_clock_samples(boot_id, observed_at_wall_clock);

CREATE UNIQUE INDEX IF NOT EXISTS durable_monotonic_clock_samples_baseline_generation_uniq
  ON durable_monotonic_clock_samples(boot_id, baseline_generation);

CREATE INDEX IF NOT EXISTS durable_monotonic_clock_samples_state_idx
  ON durable_monotonic_clock_samples(sample_state);
