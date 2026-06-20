-- P086 SEC-HIGH-001 fix: DB-backed raw receipt storage.
--
-- Raw provider_session_attach_receipt_v2 bodies are stored here instead of
-- the filesystem. A same-UID ACP subprocess cannot access this table because
-- DATABASE_URL is never passed to child processes (env_clear() + allowlist).
-- Filesystem storage under private-receipts/ was reachable via CHAINWORKS_META_ROOT
-- traversal (../../private-receipts/<continuation_id>/raw.json) since both paths
-- share the same .chainworks parent directory and the ACP process runs as the same UID.

CREATE TABLE p086_resurrection_raw_receipts (
  continuation_id TEXT PRIMARY KEY,
  raw_receipt_json TEXT NOT NULL,
  written_at TEXT NOT NULL
);
