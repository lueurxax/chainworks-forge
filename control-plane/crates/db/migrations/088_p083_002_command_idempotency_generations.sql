-- P083-002: command idempotency tracking tables.
-- Provides lease-generation idempotency for all lifecycle commands with
-- CallerRequestId. command_request_aliases maps same-intent replacement
-- request ids to the canonical committed request for replay without
-- creating duplicate lifecycle side effects.

CREATE TABLE IF NOT EXISTS command_idempotency (
  principal_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  command TEXT NOT NULL,
  intent_hash TEXT NOT NULL,
  lease_generation INTEGER NOT NULL,
  lease_state TEXT NOT NULL CHECK(lease_state IN ('pending','committed','failed','abandoned')),
  acquired_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  committed_at TEXT NULL,
  outcome_json TEXT NULL,
  failure_code TEXT NULL,
  PRIMARY KEY(principal_id, request_id, lease_generation)
);

CREATE TABLE IF NOT EXISTS command_request_aliases (
  principal_id TEXT NOT NULL,
  command TEXT NOT NULL,
  intent_hash TEXT NOT NULL,
  request_id TEXT NOT NULL,
  canonical_request_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY(principal_id, command, intent_hash, request_id)
);

-- At most one active (pending/committed/failed) lease per (principal, request).
CREATE UNIQUE INDEX IF NOT EXISTS command_request_active_uniq
  ON command_idempotency(principal_id, request_id)
  WHERE lease_state IN ('pending','committed','failed');

-- At most one active (pending/committed) lease per (principal, command, intent).
CREATE UNIQUE INDEX IF NOT EXISTS command_intent_active_uniq
  ON command_idempotency(principal_id, command, intent_hash)
  WHERE lease_state IN ('pending','committed');
