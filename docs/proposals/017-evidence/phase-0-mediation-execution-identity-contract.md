# Phase 0 Mediation Execution Identity Contract

Every persistence, quota, retry, source-generation, transcript, runtime-facts, and readback path must key off durable owner identity.

## Durable Identity Keys
- run_id
- agent_execution_id
- owner_kind (stage_execution | lead_conflict_mediation)
- owner_id

## Compatibility Context
- origin_stage_id
- origin_stage_execution_id
- conflict_fingerprint
