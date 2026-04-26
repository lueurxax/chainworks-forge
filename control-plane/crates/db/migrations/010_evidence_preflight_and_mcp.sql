ALTER TABLE stage_executions ADD COLUMN validation_failure_json TEXT;
ALTER TABLE stage_executions ADD COLUMN evidence_packet_json TEXT;
ALTER TABLE stage_executions ADD COLUMN recovery_snapshot_json TEXT;

ALTER TABLE runs ADD COLUMN delivery_preflight_json TEXT;

ALTER TABLE agent_executions ADD COLUMN backend_profile_id TEXT;
ALTER TABLE agent_executions ADD COLUMN requested_mcp_extensions_json TEXT;
ALTER TABLE agent_executions ADD COLUMN predicted_mcp_extensions_json TEXT;
ALTER TABLE agent_executions ADD COLUMN predicted_mcp_runtime_ids_json TEXT;
ALTER TABLE agent_executions ADD COLUMN actual_mcp_extensions_json TEXT;
ALTER TABLE agent_executions ADD COLUMN actual_mcp_runtime_ids_json TEXT;
ALTER TABLE agent_executions ADD COLUMN denied_mcp_extensions_json TEXT;
ALTER TABLE agent_executions ADD COLUMN mcp_blocking_issues_json TEXT;
ALTER TABLE agent_executions ADD COLUMN actual_mcp_observation_json TEXT;
ALTER TABLE agent_executions ADD COLUMN mcp_session_startup_latency_ms INTEGER;
