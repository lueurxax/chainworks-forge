ALTER TABLE scheduler_health_snapshots
  ADD COLUMN fire_consecutive_snapshots INTEGER NOT NULL DEFAULT 0;

ALTER TABLE scheduler_health_snapshots
  ADD COLUMN clear_consecutive_snapshots INTEGER NOT NULL DEFAULT 0;
