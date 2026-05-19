-- P087: Local Storage Tiering and Projection Liveness

-- Projection invalidation log (bounded by logic to 50k rows)
CREATE TABLE IF NOT EXISTS projection_invalidation_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  projection_name TEXT NOT NULL,
  source_name TEXT NOT NULL,
  primary_key TEXT NOT NULL,
  invalidation_kind TEXT NOT NULL, -- "upsert" | "delete"
  payload_json TEXT,               -- optional coalesced payload
  size_bytes INTEGER NOT NULL DEFAULT 0,
  created_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_projection_invalidation_log_lookup
  ON projection_invalidation_log(projection_name, source_name, created_at_ms);

-- Projection cursors (watermarks and liveness)
CREATE TABLE IF NOT EXISTS projection_cursors (
  projection_name TEXT NOT NULL,
  source_name TEXT NOT NULL,
  watermark_ms INTEGER NOT NULL,
  is_poisoned INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  updated_at_ms INTEGER NOT NULL,
  PRIMARY KEY (projection_name, source_name)
);

-- Maintenance operations and repair slots
CREATE TABLE IF NOT EXISTS maintenance_operations (
  id TEXT PRIMARY KEY,
  operation_kind TEXT NOT NULL,
  status TEXT NOT NULL,          -- "pending" | "running" | "completed" | "failed" | "orphaned"
  idempotency_key TEXT NOT NULL UNIQUE,
  slot_generation INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT,
  started_at_ms INTEGER,
  completed_at_ms INTEGER,
  error TEXT,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_maintenance_operations_status
  ON maintenance_operations(status);

-- Projection-backed compact readback fields for shared hot run-list
-- contracts are added by 056_p087_run_summary_hot_read_payloads.sql.

-- Storage health snapshots
CREATE TABLE IF NOT EXISTS storage_health_snapshots (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  snapshot_kind TEXT NOT NULL,
  health_json TEXT NOT NULL,
  captured_at_ms INTEGER NOT NULL
);

-- Hot read circuit state (tracking wouldOpen vs Open)
CREATE TABLE IF NOT EXISTS hot_read_circuit_states (
  governed_surface TEXT PRIMARY KEY, -- e.g. "runs.list", "artifacts.metadata"
  circuit_status TEXT NOT NULL,      -- "closed" | "open" | "half_open"
  consecutive_successes INTEGER NOT NULL DEFAULT 0,
  last_violation_kind TEXT,
  would_open INTEGER NOT NULL DEFAULT 0,
  last_opened_at_ms INTEGER,
  updated_at_ms INTEGER NOT NULL
);
