ALTER TABLE host_interruption_affected_executions
  ADD COLUMN previous_status TEXT NOT NULL DEFAULT 'running';

ALTER TABLE host_interruption_affected_executions
  ADD COLUMN settlement_status TEXT NOT NULL DEFAULT 'retry_enqueued';

ALTER TABLE host_interruption_affected_executions
  ADD COLUMN cleanup_status TEXT NOT NULL DEFAULT 'not_required';

ALTER TABLE host_interruption_affected_executions
  ADD COLUMN quota_budget_effect TEXT NOT NULL DEFAULT 'not_consumed';
