-- P087: Projection invalidation lifecycle state
ALTER TABLE projection_invalidation_log ADD COLUMN is_consumed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projection_invalidation_log ADD COLUMN consumed_at_ms INTEGER;

-- Index for reaper and freshness readback
CREATE INDEX IF NOT EXISTS idx_projection_invalidation_log_lifecycle 
  ON projection_invalidation_log(projection_name, source_name, is_consumed, created_at_ms);
