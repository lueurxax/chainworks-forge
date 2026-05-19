-- P087: Hot-read circuit breaker refinements (3-failure open and backoff)
ALTER TABLE hot_read_circuit_states ADD COLUMN consecutive_failures INTEGER NOT NULL DEFAULT 0;
ALTER TABLE hot_read_circuit_states ADD COLUMN retry_after_ms INTEGER;

-- P087: Projection invalidation refinements (throttling)
ALTER TABLE projection_cursors ADD COLUMN throttled_until_ms INTEGER;
