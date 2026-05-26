-- P081 Phase 3: Add idempotency linkage columns to command_journal.
-- Nullable: existing rows keep NULL; new MCP state-changing writes supply the
-- derived idempotency_key so replay readback can bind to the durable command row.
-- boundary_row_id stores the boundary matrix row_id that allowed the command.
ALTER TABLE command_journal ADD COLUMN mcp_idempotency_key TEXT;
ALTER TABLE command_journal ADD COLUMN boundary_row_id TEXT;
CREATE INDEX IF NOT EXISTS idx_command_journal_mcp_idempotency_key
    ON command_journal(mcp_idempotency_key)
    WHERE mcp_idempotency_key IS NOT NULL;
