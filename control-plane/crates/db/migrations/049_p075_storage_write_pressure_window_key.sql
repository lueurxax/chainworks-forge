-- P075 closeout: Class D telemetry_merge replay contract.
--
-- A write-pressure rollup bucket is keyed by its time window. Existing
-- duplicates from pre-closeout binaries are collapsed before the unique index
-- is added; runtime insert now merges duplicate-window payloads in repository
-- code before this index is exercised.

DELETE FROM storage_write_pressure_snapshots
WHERE rowid NOT IN (
  SELECT MIN(rowid)
  FROM storage_write_pressure_snapshots
  GROUP BY window_start, window_end
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_storage_write_pressure_window_unique
  ON storage_write_pressure_snapshots(window_start, window_end);
