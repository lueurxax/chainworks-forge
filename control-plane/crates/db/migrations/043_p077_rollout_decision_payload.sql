-- P077: Make the rollout decision payload first-class.
--
-- Migration 042 created durable decision rows. This migration adds the
-- proposal-required decision-payload fields as columns so expansion and
-- rollback decisions cannot hide required release evidence inside opaque JSON.

ALTER TABLE p077_rollout_decisions
  ADD COLUMN decision_type TEXT NOT NULL DEFAULT 'continue_advisory'
  CHECK (
    decision_type IN (
      'continue_advisory',
      'limited_enforcement',
      'expand_enforcement',
      'hold',
      'rollback_to_advisory'
    )
  );

ALTER TABLE p077_rollout_decisions
  ADD COLUMN cohort TEXT NOT NULL DEFAULT 'unspecified';

ALTER TABLE p077_rollout_decisions
  ADD COLUMN eligible_closeouts INTEGER NOT NULL DEFAULT 0
  CHECK(eligible_closeouts >= 0);

ALTER TABLE p077_rollout_decisions
  ADD COLUMN primary_metric_values_json TEXT NOT NULL DEFAULT '{}';

ALTER TABLE p077_rollout_decisions
  ADD COLUMN diagnostic_metric_snapshot_json TEXT NOT NULL DEFAULT '{}';

ALTER TABLE p077_rollout_decisions
  ADD COLUMN dependency_checklist_snapshot_id TEXT NOT NULL DEFAULT 'unspecified';

ALTER TABLE p077_rollout_decisions
  ADD COLUMN fingerprint_p95_threshold_ms INTEGER NOT NULL DEFAULT 1
  CHECK(fingerprint_p95_threshold_ms > 0);

ALTER TABLE p077_rollout_decisions
  ADD COLUMN measurement_window TEXT NOT NULL DEFAULT 'unspecified';

ALTER TABLE p077_rollout_decisions
  ADD COLUMN waivers_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE p077_rollout_decisions
  ADD COLUMN next_review_date TEXT NOT NULL DEFAULT 'unspecified';

ALTER TABLE p077_rollout_decisions
  ADD COLUMN readiness_links_json TEXT NOT NULL DEFAULT '[]';

CREATE INDEX IF NOT EXISTS idx_p077_rollout_decisions_decision_type
  ON p077_rollout_decisions(decision_type);

CREATE INDEX IF NOT EXISTS idx_p077_rollout_decisions_cohort
  ON p077_rollout_decisions(cohort);
