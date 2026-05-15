ALTER TABLE agent_execution_runtime_facts
  ADD COLUMN runtime_preflight_phase TEXT;

ALTER TABLE agent_execution_runtime_facts
  ADD COLUMN runtime_preflight_attempt_count INTEGER;

ALTER TABLE agent_execution_runtime_facts
  ADD COLUMN runtime_preflight_remediation TEXT;

ALTER TABLE agent_execution_runtime_facts
  ADD COLUMN runtime_preflight_provider_launched INTEGER;

ALTER TABLE agent_execution_runtime_facts
  ADD COLUMN runtime_preflight_json TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN engine_failure_envelope_json TEXT;

ALTER TABLE code_writer_completion_receipts
  ADD COLUMN repair_failure_envelope_json TEXT;
