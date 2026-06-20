-- P083-005: enforcement mode state, transition journal, and rollback audit.
-- These tables track the P083 enforcement lifecycle from disabled through
-- permissive burn-in to enforce mode. Rollback sets enforcement_mode to
-- permissive or disabled without dropping schema or evidence.

CREATE TABLE IF NOT EXISTS p083_enforcement_mode_state (
  state_id TEXT PRIMARY KEY,
  proposal_id TEXT NOT NULL DEFAULT 'P083',
  enforcement_mode TEXT NOT NULL CHECK(enforcement_mode IN ('disabled','permissive','enforce')),
  mode_reason TEXT NOT NULL,
  effective_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS p083_enforcement_mode_transition_journal (
  journal_id TEXT PRIMARY KEY,
  from_mode TEXT NOT NULL CHECK(from_mode IN ('disabled','permissive','enforce')),
  to_mode TEXT NOT NULL CHECK(to_mode IN ('disabled','permissive','enforce')),
  transition_state TEXT NOT NULL CHECK(transition_state IN ('transitioning','committed','failed')),
  principal_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  commit_marker TEXT,
  initiated_at TEXT NOT NULL,
  committed_at TEXT
);

-- Transitioning rows must not have a commit_marker; committed rows must have one.
-- (Verified by the V5 verification query: SELECT COUNT(*) FROM ... WHERE transition_state = 'transitioning' AND commit_marker IS NOT NULL.)

CREATE TABLE IF NOT EXISTS p083_enforcement_mode_audit (
  audit_id TEXT PRIMARY KEY,
  journal_id TEXT NOT NULL REFERENCES p083_enforcement_mode_transition_journal(journal_id),
  enforcement_mode TEXT NOT NULL CHECK(enforcement_mode IN ('disabled','permissive','enforce')),
  reason TEXT NOT NULL,
  audited_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS p083_rollback_audit (
  audit_id TEXT PRIMARY KEY,
  action TEXT NOT NULL CHECK(action IN (
    'disable_to_permissive',
    'permissive_to_enforce',
    'enforce_to_permissive',
    'rollback_disable',
    'reenable_after_rollback',
    'manual_process_identity_check'
  )),
  status TEXT NOT NULL,
  target_enforcement_mode TEXT NOT NULL CHECK(target_enforcement_mode IN ('permissive','disabled')),
  reason TEXT,
  principal_id TEXT NOT NULL,
  request_id TEXT NOT NULL,
  ttl_expires_at TEXT,
  audited_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS p083_enforcement_mode_transition_journal_state_idx
  ON p083_enforcement_mode_transition_journal(transition_state);

CREATE INDEX IF NOT EXISTS p083_rollback_audit_action_idx
  ON p083_rollback_audit(action, status);
