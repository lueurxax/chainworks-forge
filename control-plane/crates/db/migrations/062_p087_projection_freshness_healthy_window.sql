-- P087: Track when each projection cursor first achieved healthy state.
-- first_healthy_at_ms is set when a cursor first enters healthy state (not poisoned,
-- watermark being advanced) and reset to NULL when the cursor becomes poisoned or
-- recovers from an unhealthy event. The rollout promotion check requires that all
-- cursors have first_healthy_at_ms set and that at least 48 hours have elapsed.
ALTER TABLE projection_cursors ADD COLUMN first_healthy_at_ms INTEGER;
