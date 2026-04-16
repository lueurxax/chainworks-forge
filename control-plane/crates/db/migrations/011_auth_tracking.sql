-- P029: Auth tracking columns on command_journal
ALTER TABLE command_journal ADD COLUMN caller_surface TEXT;
ALTER TABLE command_journal ADD COLUMN caller_principal_id TEXT;
ALTER TABLE command_journal ADD COLUMN caller_principal_class TEXT;
ALTER TABLE command_journal ADD COLUMN caller_tool TEXT;
