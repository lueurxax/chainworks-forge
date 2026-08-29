-- Stage summary rebuilds ask whether each stage has at least one artifact.
-- The prior run-only index forced SQLite to scan every artifact in a large run
-- for each stage, exceeding the coalesced projection deadline.
CREATE INDEX IF NOT EXISTS idx_artifacts_run_stage_id
    ON artifacts(run_id, stage_id);
