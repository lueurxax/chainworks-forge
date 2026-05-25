-- P081: Add request_hash column to approval_mutation_idempotency.
--
-- The approval idempotency contract requires canonical_request_hash to detect
-- IDEMPOTENCY_CONFLICT when the same key is reused with a different request.
-- This column stores a sha256 of the canonical request fields so the server
-- can distinguish a replay (same hash → return original result) from a conflict
-- (same key, different hash → return IDEMPOTENCY_CONFLICT, no side effects).
--
-- Adding as nullable so existing rows remain valid; new writes must supply the value.

ALTER TABLE approval_mutation_idempotency ADD COLUMN request_hash TEXT NULL;
