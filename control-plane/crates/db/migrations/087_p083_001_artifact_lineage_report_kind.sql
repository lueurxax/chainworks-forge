-- P083-001: artifact_lineage table with bounded report_kind.
-- Creates execution-truth artifact provenance table.
-- Active report rows must carry a bounded report_kind value (enforced via
-- BEFORE INSERT/UPDATE triggers per the proposal, plus a unique partial index).

CREATE TABLE IF NOT EXISTS artifact_lineage (
  artifact_id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES runs(id),
  artifact_role TEXT NOT NULL,
  artifact_name TEXT,
  canonical_path TEXT,
  active INTEGER NOT NULL DEFAULT 1,
  report_kind TEXT NULL,
  created_at TEXT NOT NULL,
  superseded_by_artifact_id TEXT NULL
);

CREATE TRIGGER IF NOT EXISTS artifact_lineage_report_kind_required_insert
  BEFORE INSERT ON artifact_lineage
  WHEN NEW.artifact_role = 'report' AND NEW.active = 1
    AND (NEW.report_kind IS NULL
         OR NEW.report_kind NOT IN (
           'proposal_current', 'proposal_revision_summary',
           'proposal_feedback_coverage', 'review_summary',
           'run_report', 'release_receipt', 'evidence_pack'
         ))
BEGIN
  SELECT RAISE(ABORT, 'artifact_lineage: active report row requires valid bounded report_kind');
END;

CREATE TRIGGER IF NOT EXISTS artifact_lineage_report_kind_required_update
  BEFORE UPDATE ON artifact_lineage
  WHEN NEW.artifact_role = 'report' AND NEW.active = 1
    AND (NEW.report_kind IS NULL
         OR NEW.report_kind NOT IN (
           'proposal_current', 'proposal_revision_summary',
           'proposal_feedback_coverage', 'review_summary',
           'run_report', 'release_receipt', 'evidence_pack'
         ))
BEGIN
  SELECT RAISE(ABORT, 'artifact_lineage: active report row requires valid bounded report_kind');
END;

CREATE UNIQUE INDEX IF NOT EXISTS artifact_lineage_active_report_kind_uniq
  ON artifact_lineage(run_id, report_kind)
  WHERE active = 1 AND artifact_role = 'report' AND report_kind IS NOT NULL;

CREATE INDEX IF NOT EXISTS artifact_lineage_run_id_idx
  ON artifact_lineage(run_id);

CREATE INDEX IF NOT EXISTS artifact_lineage_role_active_idx
  ON artifact_lineage(artifact_role, active);
