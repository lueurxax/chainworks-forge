-- P077: Persist typed accepted risk lineage on the active closeout readiness
-- generation so GraphQL, MCP, run-state projections, and macOS readback do
-- not rely on transient command payloads.

ALTER TABLE closeout_gate_generations ADD COLUMN accepted_risks_json TEXT;
