CREATE TABLE IF NOT EXISTS startup_recovery_readbacks (
  id TEXT PRIMARY KEY,
  recovered_item_count INTEGER NOT NULL,
  queued_under_startup_recovery_backpressure_count INTEGER NOT NULL,
  oldest_recovered_queued_age_ms INTEGER,
  affected_run_count INTEGER NOT NULL,
  next_retry_or_backoff_time TEXT,
  stale_after_ms INTEGER NOT NULL DEFAULT 60000,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_startup_recovery_readbacks_updated_at
  ON startup_recovery_readbacks(updated_at DESC);
