-- Workflow state machine support: track current state and YAML paths on runs,
-- and agent binding metadata on stages.

ALTER TABLE runs ADD COLUMN current_state TEXT;
ALTER TABLE runs ADD COLUMN workflow_yaml_path TEXT;
ALTER TABLE runs ADD COLUMN agent_catalog_yaml_path TEXT;

ALTER TABLE stage_executions ADD COLUMN owner_agent TEXT;
ALTER TABLE stage_executions ADD COLUMN provider TEXT;
ALTER TABLE stage_executions ADD COLUMN model TEXT;
ALTER TABLE stage_executions ADD COLUMN stage_type TEXT;
