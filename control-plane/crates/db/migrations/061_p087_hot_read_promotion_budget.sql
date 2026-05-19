-- P087: Observe-to-enforce promotion budget counters
-- These support computing the per-surface would_open rate and flap-free window
-- required before an operator can safely promote from observe to enforce mode.
ALTER TABLE hot_read_circuit_states ADD COLUMN total_requests INTEGER NOT NULL DEFAULT 0;
ALTER TABLE hot_read_circuit_states ADD COLUMN total_would_open INTEGER NOT NULL DEFAULT 0;
ALTER TABLE hot_read_circuit_states ADD COLUMN last_state_change_at_ms INTEGER;
-- Tracks the first time a governed surface was ever observed (first request).
-- The 48-hour observation window is measured from this timestamp, not from
-- last_state_change_at_ms, which is NULL for surfaces that have always been closed.
ALTER TABLE hot_read_circuit_states ADD COLUMN first_observed_at_ms INTEGER;
