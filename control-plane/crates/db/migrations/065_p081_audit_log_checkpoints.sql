-- P081 Phase 1: audit_log_checkpoints table.
-- Checkpoints are written every 1000 audit_log rows and at clean shutdown
-- when the open window is non-empty. Startup verifies the latest checkpoint
-- window and reports verified, degraded, or tamper_suspected.

CREATE TABLE IF NOT EXISTS audit_log_checkpoints (
  checkpoint_id             TEXT    NOT NULL PRIMARY KEY
                                      CHECK(length(checkpoint_id) > 0 AND length(checkpoint_id) <= 256),
  checkpoint_seq            INTEGER NOT NULL UNIQUE,
  covered_start_id          TEXT    NOT NULL
                                      CHECK(length(covered_start_id) > 0 AND length(covered_start_id) <= 256),
  covered_end_id            TEXT    NOT NULL
                                      CHECK(length(covered_end_id) > 0 AND length(covered_end_id) <= 256),
  covered_row_count         INTEGER NOT NULL
                                      CHECK(covered_row_count > 0),
  previous_checkpoint_hash  TEXT    NULL
                                      CHECK(previous_checkpoint_hash IS NULL OR length(previous_checkpoint_hash) = 64),
  checkpoint_hash           TEXT    NOT NULL
                                      CHECK(length(checkpoint_hash) = 64),
  created_at_ms             INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS uniq_audit_log_checkpoints_seq
  ON audit_log_checkpoints(checkpoint_seq);
