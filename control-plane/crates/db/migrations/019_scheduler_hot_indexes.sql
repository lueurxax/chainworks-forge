CREATE INDEX IF NOT EXISTS idx_work_items_status_kind_scheduled
ON work_items(status, kind, scheduled_at);

CREATE INDEX IF NOT EXISTS idx_agent_executions_status
ON agent_executions(status);

CREATE INDEX IF NOT EXISTS idx_agent_executions_status_provider
ON agent_executions(status, provider);

CREATE INDEX IF NOT EXISTS idx_agent_executions_status_stage
ON agent_executions(status, stage_execution_id);
