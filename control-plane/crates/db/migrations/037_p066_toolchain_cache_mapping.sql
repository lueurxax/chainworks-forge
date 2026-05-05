-- 037_p066_toolchain_cache_mapping.sql
--
-- P066 Phase 0: Add actual_toolchain_mapping_diagnostics_json column to
-- agent_executions. This column stores the bounded toolchain mapping
-- diagnostics document for each execution attempt. Pre-migration rows remain
-- NULL and are synthesized northbound as mapping_state=legacy_row_unavailable.
-- No historical rows are rewritten.

ALTER TABLE agent_executions
  ADD COLUMN actual_toolchain_mapping_diagnostics_json TEXT;
