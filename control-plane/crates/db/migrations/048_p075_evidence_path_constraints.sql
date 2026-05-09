-- P075-SEC-002: Strengthen evidence_spool_refs relative_path containment constraints.
-- P075-SEC-001: Add SQL-level identity field and checksum length bounds.
--
-- SQLite does not support ALTER TABLE ADD CONSTRAINT, so this migration recreates
-- the table with the stronger constraints in a single atomic operation.
--
-- New constraints added to relative_path (mirrors validate_relative_path in Rust):
--   - length <= 2048
--   - NOT absolute (no leading '/')
--   - NO backslash separators (char(92))
--   - NO '..' traversal segments (exact, leading, trailing, middle)
--   - NO '.' path segments (exact, leading, trailing, middle)
--   - NO empty segments (no '//')
--
-- New constraints added to identity fields (run_id, stage_*, agent_*):
--   - length <= 512 (producer-controlled; bounded before indexing)
--
-- New constraints added to other stored fields:
--   - checksum: length > 0 AND length <= 256
--   - producer_operation: length > 0 AND length <= 1024
--   - content_type: length <= 1024 (already capped in Rust; mirrored here)

CREATE TABLE evidence_spool_refs_v2 (
  id                   TEXT    PRIMARY KEY
                         CHECK(length(id) > 0 AND length(id) <= 256),
  metadata_version     INTEGER NOT NULL DEFAULT 1
                         CHECK(metadata_version = 1),
  run_id               TEXT    NOT NULL
                         CHECK(length(run_id) > 0 AND length(run_id) <= 512),
  stage_execution_id   TEXT
                         CHECK(stage_execution_id IS NULL OR (length(stage_execution_id) > 0 AND length(stage_execution_id) <= 512)),
  stage_id             TEXT
                         CHECK(stage_id IS NULL OR (length(stage_id) > 0 AND length(stage_id) <= 512)),
  agent_execution_id   TEXT
                         CHECK(agent_execution_id IS NULL OR (length(agent_execution_id) > 0 AND length(agent_execution_id) <= 512)),
  agent_id             TEXT
                         CHECK(agent_id IS NULL OR (length(agent_id) > 0 AND length(agent_id) <= 512)),
  kind                 TEXT    NOT NULL
                         CHECK(kind IN (
                           'transcript',
                           'tool_trace',
                           'stdout',
                           'stderr',
                           'receipt',
                           'runtime_event',
                           'model_delta',
                           'delivery_readback'
                         )),
  relative_path        TEXT    NOT NULL
                         CHECK(
                           length(relative_path) > 0 AND
                           length(relative_path) <= 2048 AND
                           relative_path NOT LIKE '/%' AND
                           instr(relative_path, char(92)) = 0 AND
                           relative_path != '..' AND
                           relative_path NOT LIKE '../%' AND
                           relative_path NOT LIKE '%/..' AND
                           relative_path NOT LIKE '%/../%' AND
                           relative_path != '.' AND
                           relative_path NOT LIKE './%' AND
                           relative_path NOT LIKE '%/.' AND
                           relative_path NOT LIKE '%/./%' AND
                           relative_path NOT LIKE '%//%'
                         ),
  size_bytes           INTEGER NOT NULL
                         CHECK(size_bytes >= 0),
  checksum_algorithm   TEXT    NOT NULL
                         CHECK(checksum_algorithm IN ('sha256')),
  checksum             TEXT    NOT NULL
                         CHECK(length(checksum) > 0 AND length(checksum) <= 256),
  producer_operation   TEXT    NOT NULL
                         CHECK(length(producer_operation) > 0 AND length(producer_operation) <= 1024),
  content_type         TEXT
                         CHECK(content_type IS NULL OR length(content_type) <= 1024),
  summary_json         TEXT
                         CHECK(summary_json IS NULL OR length(summary_json) <= 8192),
  created_at           TEXT    NOT NULL,
  status               TEXT    NOT NULL DEFAULT 'available'
                         CHECK(status IN (
                           'available',
                           'legacy_absent',
                           'missing_file',
                           'checksum_mismatch',
                           'recovered_orphan',
                           'pending_delete'
                         ))
);

INSERT INTO evidence_spool_refs_v2
  SELECT id, metadata_version, run_id,
         stage_execution_id, stage_id, agent_execution_id, agent_id,
         kind, relative_path, size_bytes,
         checksum_algorithm, checksum, producer_operation,
         content_type, summary_json, created_at, status
  FROM evidence_spool_refs;

DROP TABLE evidence_spool_refs;

ALTER TABLE evidence_spool_refs_v2 RENAME TO evidence_spool_refs;

CREATE INDEX IF NOT EXISTS idx_evidence_spool_refs_run_created
  ON evidence_spool_refs(run_id, created_at);

CREATE INDEX IF NOT EXISTS idx_evidence_spool_refs_stage_execution
  ON evidence_spool_refs(stage_execution_id);

CREATE INDEX IF NOT EXISTS idx_evidence_spool_refs_agent_execution
  ON evidence_spool_refs(agent_execution_id);

CREATE INDEX IF NOT EXISTS idx_evidence_spool_refs_kind
  ON evidence_spool_refs(kind);

CREATE UNIQUE INDEX IF NOT EXISTS uniq_evidence_spool_refs_run_relative_path
  ON evidence_spool_refs(run_id, relative_path);
