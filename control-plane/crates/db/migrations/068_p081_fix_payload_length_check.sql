-- P081 SEC-007: Fix audit_log.payload CHECK to use byte length.
--
-- SQLite length(TEXT) counts characters; Rust enforces a byte limit.
-- For the all-ASCII payloads shipped so far the two agree, but a future
-- multibyte payload could pass the SQL CHECK while the Rust guard rejects it,
-- or vice versa, so both layers must agree on bytes.
--
-- SQLite does not support ALTER TABLE ... MODIFY CHECK. We rebuild the table
-- with the corrected constraint using the standard rename-and-recreate idiom.
-- All existing data is preserved; indexes are recreated.

-- 1. Rename old table.
ALTER TABLE audit_log RENAME TO audit_log_v1_backup;

-- 2. Create corrected table with BLOB-length payload CHECK.
CREATE TABLE audit_log (
  id                                    TEXT    NOT NULL PRIMARY KEY
                                                  CHECK(length(id) > 0 AND length(id) <= 256),
  request_id                            TEXT    NOT NULL
                                                  CHECK(length(request_id) > 0 AND length(request_id) <= 256),
  timestamp_ms                          INTEGER NOT NULL,
  event_type                            TEXT    NOT NULL
                                                  CHECK(length(event_type) > 0 AND length(event_type) <= 128),
  principal_id                          TEXT    NULL
                                                  CHECK(principal_id IS NULL OR length(principal_id) <= 256),
  principal_class                       TEXT    NULL
                                                  CHECK(principal_class IS NULL OR length(principal_class) <= 64),
  caller_class                          TEXT    NULL
                                                  CHECK(caller_class IS NULL OR caller_class IN (
                                                    'ui_operator',
                                                    'agent_operator',
                                                    'automation',
                                                    'observer',
                                                    'developer_break_glass'
                                                  )),
  token_id                              TEXT    NULL
                                                  CHECK(token_id IS NULL OR length(token_id) <= 256),
  transport                             TEXT    NOT NULL
                                                  CHECK(transport IN (
                                                    'graphql_query',
                                                    'graphql_subscription',
                                                    'graphql_mutation',
                                                    'mcp_initialize',
                                                    'mcp_tools_list',
                                                    'mcp_tools_call',
                                                    'debug_endpoint'
                                                  )),
  action_attempted                      TEXT    NOT NULL
                                                  CHECK(length(action_attempted) > 0 AND length(action_attempted) <= 512),
  decision                              TEXT    NOT NULL
                                                  CHECK(decision IN ('allow', 'deny', 'redact', 'drop_resource')),
  denial_reason_code                    TEXT    NULL
                                                  CHECK(denial_reason_code IS NULL OR denial_reason_code IN (
                                                    'UNAUTHENTICATED',
                                                    'AMBIGUOUS_CALLER',
                                                    'CAPABILITY_OUT_OF_SCOPE',
                                                    'NON_APPROVAL_MUTATION',
                                                    'APPROVAL_NOT_ACTIONABLE',
                                                    'OBSERVER_SCOPE',
                                                    'BREAK_GLASS_DISABLED',
                                                    'MATRIX_NO_ROW',
                                                    'E_AUDIT_UNAVAILABLE',
                                                    'E_FIXTURE_DIGEST_MISMATCH',
                                                    'SQLITE_CONTENTION_RETRY_EXHAUSTED',
                                                    'IDEMPOTENCY_CONFLICT'
                                                  )),
  row_id                                TEXT    NULL
                                                  CHECK(row_id IS NULL OR length(row_id) <= 256),
  env_gate_state                        TEXT    NULL
                                                  CHECK(env_gate_state IS NULL OR length(env_gate_state) <= 64),
  source_ip_hash_or_local_process_id    TEXT    NULL
                                                  CHECK(source_ip_hash_or_local_process_id IS NULL OR
                                                        length(source_ip_hash_or_local_process_id) <= 256),
  boundary_policy_mode                  TEXT    NOT NULL
                                                  CHECK(boundary_policy_mode IN (
                                                    'shadow',
                                                    'enforce',
                                                    'read_only_safe_mode',
                                                    'legacy_compat'
                                                  )),
  fixture_version                       TEXT    NOT NULL
                                                  CHECK(length(fixture_version) > 0 AND length(fixture_version) <= 128),
  payload_schema_version                INTEGER NOT NULL DEFAULT 1
                                                  CHECK(payload_schema_version >= 1),
  -- SEC-007: Use BLOB cast so length counts bytes, matching the Rust guard.
  payload                               TEXT    NOT NULL
                                                  CHECK(length(CAST(payload AS BLOB)) > 0 AND
                                                        length(CAST(payload AS BLOB)) <= 16384),
  payload_sha256                        TEXT    NOT NULL
                                                  CHECK(length(payload_sha256) = 64),
  diagnostic_truncated                  INTEGER NOT NULL DEFAULT 0
                                                  CHECK(diagnostic_truncated IN (0, 1)),
  prev_hash                             TEXT    NULL
                                                  CHECK(prev_hash IS NULL OR length(prev_hash) = 64),
  row_hash                              TEXT    NOT NULL
                                                  CHECK(length(row_hash) = 64),
  checkpoint_id                         TEXT    NULL
                                                  CHECK(checkpoint_id IS NULL OR length(checkpoint_id) <= 256),
  created_at_ms                         INTEGER NOT NULL
);

-- 3. Copy all existing rows.
INSERT INTO audit_log SELECT * FROM audit_log_v1_backup;

-- 4. Recreate indexes.
CREATE UNIQUE INDEX IF NOT EXISTS audit_log_row_hash_idx ON audit_log(row_hash);
CREATE INDEX IF NOT EXISTS audit_log_request_id_idx ON audit_log(request_id);
CREATE INDEX IF NOT EXISTS audit_log_time_idx ON audit_log(timestamp_ms);
CREATE INDEX IF NOT EXISTS audit_log_reason_idx ON audit_log(denial_reason_code, row_id);
CREATE INDEX IF NOT EXISTS audit_log_principal_idx ON audit_log(principal_id, caller_class);
CREATE INDEX IF NOT EXISTS audit_log_checkpoint_idx ON audit_log(checkpoint_id);

-- 5. Drop backup table.
DROP TABLE audit_log_v1_backup;
