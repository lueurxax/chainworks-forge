-- P075 Phase 1: Optional write pressure snapshot history.
--
-- Compact ring buffer of storage health snapshots for the storageHealth GraphQL
-- and MCP diagnostics surfaces. Retention: 24 hours or the latest 288 five-minute
-- windows, whichever is smaller.
--
-- Producers and retention-purge logic are wired in Phase 6; this table is empty
-- until then. GraphQL reports migration_empty until producers write spool metadata.

CREATE TABLE IF NOT EXISTS storage_write_pressure_snapshots (
  id           TEXT NOT NULL PRIMARY KEY,
  window_start TEXT NOT NULL,
  window_end   TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at   TEXT NOT NULL,
  CHECK(length(payload_json) <= 65536)
);

CREATE INDEX IF NOT EXISTS idx_storage_write_pressure_window_start
  ON storage_write_pressure_snapshots(window_start);

CREATE INDEX IF NOT EXISTS idx_storage_write_pressure_created_at
  ON storage_write_pressure_snapshots(created_at);
