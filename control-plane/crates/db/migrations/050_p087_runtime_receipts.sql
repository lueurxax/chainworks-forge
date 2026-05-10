CREATE TABLE IF NOT EXISTS agent_execution_runtime_receipts (
  agent_execution_id TEXT PRIMARY KEY REFERENCES agent_executions(id) ON DELETE CASCADE,
  provider TEXT NOT NULL,
  transport_family TEXT NOT NULL,
  status TEXT NOT NULL,
  failure_phase TEXT,
  event_count INTEGER NOT NULL DEFAULT 0,
  last_event_kind TEXT,
  last_event_at_ms INTEGER,
  receipt_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_execution_runtime_receipts_provider_status
  ON agent_execution_runtime_receipts(provider, status);

CREATE INDEX IF NOT EXISTS idx_agent_execution_runtime_receipts_last_event_kind
  ON agent_execution_runtime_receipts(last_event_kind);
