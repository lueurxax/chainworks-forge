-- P086: durable continuation rollout metrics.
--
-- These rows are intentionally stored separately from in-memory process
-- counters so rollout/readback can survive daemon restart and can be joined by
-- run/stage/agent/continuation without unbounded metric labels.

CREATE TABLE IF NOT EXISTS p086_continuation_metric_events (
  id                    TEXT    PRIMARY KEY,
  run_id                TEXT    REFERENCES runs(id),
  stage_execution_id    TEXT    REFERENCES stage_executions(id),
  agent_execution_id    TEXT    REFERENCES agent_executions(id),
  continuation_id       TEXT    REFERENCES agent_work_continuations(id),
  metric_name           TEXT    NOT NULL,
  labels_json           TEXT    NOT NULL DEFAULT '{}',
  value                 INTEGER NOT NULL DEFAULT 1,
  occurred_at           TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_p086_metric_run_time
  ON p086_continuation_metric_events(run_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_p086_metric_continuation
  ON p086_continuation_metric_events(continuation_id, metric_name);

CREATE INDEX IF NOT EXISTS idx_p086_metric_name_time
  ON p086_continuation_metric_events(metric_name, occurred_at DESC);
