-- P086: Durable audit table for provider_session_resurrection attach receipt access.
--
-- Every raw receipt read (Operator MCP) and every access denial (wrong-run or
-- not-found) is recorded here so operators can trace who fetched what and when.
-- Metric events remain as a secondary read path; this table is the durable source.
--
-- Fields follow the proposal-required audit schema (approved_proposal:459):
--   principal_id, receipt_id (continuation_id), run_id, requested_at,
--   source_channel, denial_reason (NULL for successful reads).

CREATE TABLE p086_receipt_access_audit (
  id               TEXT    PRIMARY KEY,          -- UUID, client-generated
  principal_id     TEXT    NOT NULL,
  principal_class  TEXT    NOT NULL
                     CHECK (principal_class IN ('operator', 'observer', 'agent', 'unknown')),
  continuation_id  TEXT    NOT NULL,             -- the receipt_id per proposal
  run_id           TEXT    NOT NULL,
  requested_at     TEXT    NOT NULL,             -- ISO-8601 UTC
  source_channel   TEXT    NOT NULL DEFAULT 'mcp',
  outcome          TEXT    NOT NULL
                     CHECK (outcome IN ('raw_read', 'denied', 'reviewer_projection')),
  denial_reason    TEXT,                         -- NULL for outcome='raw_read' or 'reviewer_projection'
  created_at       TEXT    NOT NULL
);

CREATE INDEX idx_p086_receipt_audit_continuation
  ON p086_receipt_access_audit(continuation_id, requested_at DESC);

CREATE INDEX idx_p086_receipt_audit_principal
  ON p086_receipt_access_audit(principal_id, requested_at DESC);
