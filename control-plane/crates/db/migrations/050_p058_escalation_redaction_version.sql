-- P058 Phase 1 follow-up: add redaction_version to escalation_events.
-- Proposal mandates redaction_version stamps each escalation projection and report write.
-- NOT NULL with DEFAULT ensures defense-in-depth at the DB layer; the repo layer
-- also enforces the known-versions allowlist before any INSERT.
ALTER TABLE escalation_events ADD COLUMN redaction_version TEXT NOT NULL DEFAULT 'redaction_v1';
