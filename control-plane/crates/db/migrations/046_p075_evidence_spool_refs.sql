-- P075 Phase 1: Evidence spool reference metadata table.
--
-- EvidenceSpoolRef records a compact metadata pointer for evidence files written
-- to the local filesystem. Raw evidence bytes live in files; this table stores only
-- the metadata pointer, checksum, size, kind, and ownership.
--
-- File-before-metadata ordering (Class C discipline):
--   fsync(file) and fsync(parent_dir) must complete before metadata is enqueued
--   to DbWriter. Metadata-without-bytes is therefore impossible by construction;
--   bytes-without-metadata are orphan-recoverable by the startup sweep (Phase 3).
--
-- reader_status values:
--   available          - file present, checksum matches, readable
--   legacy_absent      - run predates P075; no spool metadata expected
--   missing_file       - metadata row exists but file is absent
--   checksum_mismatch  - file present but checksum does not match
--   recovered_orphan   - file recovered by startup orphan sweep; metadata backfilled
--   pending_delete     - terminal-run file scheduled for deletion after grace period

CREATE TABLE IF NOT EXISTS evidence_spool_refs (
  id                   TEXT    PRIMARY KEY,
  metadata_version     INTEGER NOT NULL DEFAULT 1
                         CHECK(metadata_version = 1),
  run_id               TEXT    NOT NULL,
  stage_execution_id   TEXT,
  stage_id             TEXT,
  agent_execution_id   TEXT,
  agent_id             TEXT,
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
                         CHECK(length(relative_path) > 0),
  size_bytes           INTEGER NOT NULL
                         CHECK(size_bytes >= 0),
  checksum_algorithm   TEXT    NOT NULL
                         CHECK(checksum_algorithm IN ('sha256')),
  checksum             TEXT    NOT NULL,
  producer_operation   TEXT    NOT NULL,
  content_type         TEXT,
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
