-- P066 Phase 0: Cleanup readback durability surfaces.
--
-- T17: Extend startup_recovery_readbacks with toolchainCache fields.
--      These columns are nullable so existing rows decode cleanly as "no sweep yet".
--
-- T18: New low-churn toolchain_cache_housekeeping_readbacks projection.
--      One row per housekeeping sweep. Used as the promotion gate for Phase 3.

ALTER TABLE startup_recovery_readbacks
  ADD COLUMN toolchain_session_scoped_roots_seen INTEGER;

ALTER TABLE startup_recovery_readbacks
  ADD COLUMN toolchain_session_scoped_roots_reclaimed INTEGER;

ALTER TABLE startup_recovery_readbacks
  ADD COLUMN toolchain_session_scoped_cleanup_failures INTEGER;

ALTER TABLE startup_recovery_readbacks
  ADD COLUMN toolchain_orphan_threshold_minutes INTEGER;

ALTER TABLE startup_recovery_readbacks
  ADD COLUMN toolchain_last_sweep_started_at TEXT;

CREATE TABLE IF NOT EXISTS toolchain_cache_housekeeping_readbacks (
  id                         TEXT    PRIMARY KEY,
  last_sweep_started_at      TEXT    NOT NULL,
  run_scoped_roots_pruned    INTEGER NOT NULL DEFAULT 0,
  run_scoped_prune_failures  INTEGER NOT NULL DEFAULT 0,
  oldest_eligible_root_age_days REAL,
  disk_pressure_blocks       INTEGER NOT NULL DEFAULT 0,
  quarantined_roots_created  INTEGER NOT NULL DEFAULT 0,
  created_at                 TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_toolchain_housekeeping_last_sweep
  ON toolchain_cache_housekeeping_readbacks(last_sweep_started_at DESC);
