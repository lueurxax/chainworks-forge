-- Migration 102: P039/P095 Expanded History
-- Rebuild run_continuation_inputs to support up to 512 reference inputs + 1 execution seed (ordinal 0..512).
-- Preserves all existing rows, columns, foreign keys, unique constraints, and indexes.

-- 1. Rename existing table to backup
ALTER TABLE run_continuation_inputs RENAME TO run_continuation_inputs_v1_backup;

-- 2. Drop the old partial index so the name can be reused on the rebuilt table
DROP INDEX IF EXISTS run_continuation_input_seed;

-- 3. Create reconstructed table with expanded ordinal check (0..512)
CREATE TABLE run_continuation_inputs (
    input_id TEXT PRIMARY KEY NOT NULL,
    operation_id TEXT NOT NULL REFERENCES run_continuations(operation_id),
    ordinal INTEGER NOT NULL CHECK(ordinal>=0 AND ordinal<=512),
    successor_run_id TEXT REFERENCES runs(id),
    role TEXT NOT NULL CHECK(role IN ('execution_seed','reference_only')),
    source_kind TEXT NOT NULL CHECK(source_kind IN ('artifact','carried_input')),
    source_id TEXT NOT NULL,
    source_run_id TEXT NOT NULL REFERENCES runs(id),
    original_artifact_id TEXT NOT NULL REFERENCES artifacts(id),
    source_schema TEXT NOT NULL CHECK(length(source_schema) BETWEEN 1 AND 256),
    content_sha256 TEXT NOT NULL CHECK(length(content_sha256)=64 AND content_sha256 NOT GLOB '*[^0-9a-f]*'),
    target_logical_name TEXT NOT NULL CHECK(length(target_logical_name) BETWEEN 1 AND 256),
    target_relative_path TEXT NOT NULL CHECK(length(CAST(target_relative_path AS BLOB)) BETWEEN 1 AND 4096),
    installed INTEGER NOT NULL DEFAULT 0 CHECK(installed IN (0,1)),
    historical_relation_json TEXT NOT NULL CHECK(json_valid(historical_relation_json) AND length(CAST(historical_relation_json AS BLOB))<=65536),
    UNIQUE(operation_id,ordinal),
    UNIQUE(operation_id,target_relative_path),
    CHECK((installed=0 AND successor_run_id IS NULL) OR (installed=1 AND successor_run_id IS NOT NULL))
);

-- 4. Copy all existing rows from backup into rebuilt table
INSERT INTO run_continuation_inputs (
    input_id,
    operation_id,
    ordinal,
    successor_run_id,
    role,
    source_kind,
    source_id,
    source_run_id,
    original_artifact_id,
    source_schema,
    content_sha256,
    target_logical_name,
    target_relative_path,
    installed,
    historical_relation_json
)
SELECT
    input_id,
    operation_id,
    ordinal,
    successor_run_id,
    role,
    source_kind,
    source_id,
    source_run_id,
    original_artifact_id,
    source_schema,
    content_sha256,
    target_logical_name,
    target_relative_path,
    installed,
    historical_relation_json
FROM run_continuation_inputs_v1_backup;

-- 5. Recreate partial unique index for execution_seed
CREATE UNIQUE INDEX run_continuation_input_seed ON run_continuation_inputs(operation_id)
    WHERE role='execution_seed';

-- 6. Drop backup table
DROP TABLE run_continuation_inputs_v1_backup;
