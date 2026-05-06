-- P077: Durable rollout metric, go/no-go decision, and advisory rollback truth.
--
-- This is the executable counterpart to docs/reference/p077-rollout-dependency-evidence.md:
-- expansion or rollback decisions are backed by metric rows and governed
-- release-owner decisions, not by static advisory prose alone.

CREATE TABLE IF NOT EXISTS p077_rollout_metric_events (
  id TEXT PRIMARY KEY NOT NULL,
  metric TEXT NOT NULL CHECK (
    metric IN (
      'false_ready_prevented',
      'post_release_closeout_gap_reversals',
      'false_blocks',
      'pause_to_action',
      'code_writer_loops_avoided'
    )
  ),
  run_id TEXT REFERENCES runs(id),
  numerator INTEGER NOT NULL CHECK(numerator >= 0),
  denominator INTEGER NOT NULL CHECK(denominator >= 0),
  threshold TEXT NOT NULL,
  owner TEXT NOT NULL,
  source TEXT NOT NULL,
  go_no_go_action TEXT NOT NULL,
  evidence_json TEXT NOT NULL DEFAULT '{}',
  recorded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_p077_rollout_metric_events_metric
  ON p077_rollout_metric_events(metric);

CREATE INDEX IF NOT EXISTS idx_p077_rollout_metric_events_run_id
  ON p077_rollout_metric_events(run_id);

CREATE TABLE IF NOT EXISTS p077_rollout_decisions (
  id TEXT PRIMARY KEY NOT NULL,
  decision_scope TEXT NOT NULL,
  decision TEXT NOT NULL CHECK (
    decision IN (
      'continue_advisory',
      'limited_enforcement',
      'expand_enforcement',
      'hold',
      'rollback_to_advisory'
    )
  ),
  principal TEXT NOT NULL,
  reason TEXT NOT NULL,
  metric_snapshot_json TEXT NOT NULL,
  rollback_trigger TEXT,
  rollback_action TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_p077_rollout_decisions_decision
  ON p077_rollout_decisions(decision);

CREATE TABLE IF NOT EXISTS p077_rollout_advisory_migrations (
  id TEXT PRIMARY KEY NOT NULL,
  decision_id TEXT NOT NULL REFERENCES p077_rollout_decisions(id),
  run_id TEXT NOT NULL REFERENCES runs(id),
  previous_mode TEXT CHECK(previous_mode IN ('advisory', 'enforcement') OR previous_mode IS NULL),
  new_mode TEXT NOT NULL CHECK(new_mode = 'advisory'),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_p077_rollout_advisory_migrations_decision_id
  ON p077_rollout_advisory_migrations(decision_id);

CREATE INDEX IF NOT EXISTS idx_p077_rollout_advisory_migrations_run_id
  ON p077_rollout_advisory_migrations(run_id);
