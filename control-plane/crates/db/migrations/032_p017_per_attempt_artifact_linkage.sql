-- 032_p017_per_attempt_artifact_linkage.sql
--
-- P017 R5 closure (API-003):
--
-- Adds `artifacts.agent_execution_id` so the executor can link an
-- output artifact directly to the `AgentExecution` attempt that
-- produced it. MCP/GraphQL `execution_attempts.artifacts` then prefer
-- this direct link over the prior best-effort `agent_id` correlation,
-- which over-included artifacts across retries by the same lead agent.
--
-- The column is NULL-able for backwards compatibility — pre-R5 rows
-- and stage-owned executions can still rely on the legacy correlation
-- path. New mediation completions populate it inline.
--
-- An index on (agent_execution_id) speeds the per-attempt readback
-- query that MCP/GraphQL run for every conflict mediation projection.

ALTER TABLE artifacts ADD COLUMN agent_execution_id TEXT REFERENCES agent_executions(id);

CREATE INDEX IF NOT EXISTS idx_artifacts_agent_execution_id
  ON artifacts(agent_execution_id);
