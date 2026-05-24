-- P081 Phase 2: Add caller_class to command_journal.
-- Nullable: existing rows keep NULL until backfill; new boundary-aware command writes
-- supply the derived CallerClass value alongside caller_principal_class.
ALTER TABLE command_journal ADD COLUMN caller_class TEXT;
CREATE INDEX IF NOT EXISTS idx_command_journal_caller_class
    ON command_journal(caller_class);
