-- P081 Phase 4: MCP command idempotency table.
-- Stores one record per idempotency_key so state-changing MCP tool retries
-- return the original committed result without duplicating command_journal writes.
-- Retention: at least 7 days per the P081 mcp_idempotency_contract.

CREATE TABLE IF NOT EXISTS mcp_command_idempotency (
    idempotency_key TEXT NOT NULL PRIMARY KEY,
    tool_name TEXT NOT NULL,
    caller_fingerprint TEXT NOT NULL,
    canonical_request_hash TEXT NOT NULL,
    row_id TEXT NULL,
    command_journal_id TEXT NULL,
    result_json TEXT NOT NULL DEFAULT '{}',
    result_hash TEXT NULL,
    committed_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS mcp_idempotency_tool_caller_idx
    ON mcp_command_idempotency (tool_name, caller_fingerprint);

CREATE INDEX IF NOT EXISTS mcp_idempotency_expires_idx
    ON mcp_command_idempotency (expires_at_ms);
