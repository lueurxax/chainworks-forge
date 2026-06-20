# Proposal 086: Agent Continuation API Contracts

This document summarizes the API contracts and schema materialization introduced by Proposal 086, focusing on the agent continuation mechanisms.

## Overview

Proposal 086 defines the core contracts for managing agent work continuations, including GraphQL read models and Model Context Protocol (MCP) commands. These contracts ensure durable state tracking, explicit continuation points, and predictable behavior for long-running agent workflows.

## API Contracts

### GraphQL

The GraphQL interface provides read-only access to continuation-related data. It exposes raw enum companions, display projections, freshness fields, projection lag, and `UNKNOWN` display states for unknown daemon values. There are no GraphQL mutations for continuation.

Implemented operator-read queries (see `control-plane/crates/graphql-server/src/schema.rs`):

- `continuationStatus(agentExecutionId: ID!)` — returns the active continuation record (if any), full history, and freshness state for an `AgentExecution`.
- `continuationCandidates(runId: ID!)` — returns eligible continuation candidates for a `Run`, with eligibility, raw/display status, and disabled reason per stage-owned `code_writer` `AgentExecution`.

Both queries require operator read scope; non-operator callers receive an authorization error rather than partial data.

### Model Context Protocol (MCP)

The MCP defines the command surface for interacting with agent continuations. Key commands include:

- `agents.continue_work`: The primary command for advancing agent work.
  - Required request fields: `agent_execution_id`, `mode`, `trigger_kind`, and `idempotency_key`.
  - `lead_auto` mode additionally requires `lead_decision_artifact_id` and `lead_decision_artifact_sha256`, and verifies `continuation_instruction_sha256`.
- `agents.continuation_status`: Provides direct read access to continuation status. Unauthorized rows are omitted with no existence leak.
- `agents.continuation_candidates`: Provides direct read access to available continuation candidates. Unauthorized rows are omitted with no existence leak.

MCP enum strings are canonical raw daemon values. `continuation_status.response_schema` defines `response_schema.$defs.continuation_history_item_v1`, and `history.items` references `#/$defs/continuation_history_item_v1`.

## Schema Materialization

The following JSON Schema artifacts are materialized as part of Proposal 086, defining the precise structure and validation rules for the API contracts:

### Artifact Schemas

- [`docs/reference/p086/schemas/artifacts/continuation_canonical_request_v1.schema.json`](./p086/schemas/artifacts/continuation_canonical_request_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/lead_continuation_decision_v1.schema.json`](./p086/schemas/artifacts/lead_continuation_decision_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/continuation_response_snapshot_v1.schema.json`](./p086/schemas/artifacts/continuation_response_snapshot_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/continuation_result_v1.schema.json`](./p086/schemas/artifacts/continuation_result_v1.schema.json)
- [`docs/reference/p086/schemas/artifacts/continuation_no_progress_report_v1.schema.json`](./p086/schemas/artifacts/continuation_no_progress_report_v1.schema.json)

### MCP Schemas

- [`docs/reference/p086/schemas/mcp/agents.continue_work.request.schema.json`](./p086/schemas/mcp/agents.continue_work.request.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json`](./p086/schemas/mcp/agents.continue_work.response.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continuation_status.request.schema.json`](./p086/schemas/mcp/agents.continuation_status.request.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continuation_status.response.schema.json`](./p086/schemas/mcp/agents.continuation_status.response.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continuation_candidates.request.schema.json`](./p086/schemas/mcp/agents.continuation_candidates.request.schema.json)
- [`docs/reference/p086/schemas/mcp/agents.continuation_candidates.response.schema.json`](./p086/schemas/mcp/agents.continuation_candidates.response.schema.json)

## Rules

Schemas adhere to Draft 2020-12 JSON Schema specifications, with `additionalProperties=false` unless a bounded versioned extension map is explicitly declared.

## Implementation status (current head)

The MCP command surface, atomic admission, idempotency conflict handling (`-32044`), saturation backpressure (`-32051`), and the request/response/artifact schemas listed above are implemented in the Rust control plane. Persistence lives in SQLite migration `control-plane/crates/db/migrations/057_p086_agent_work_continuations.sql` (tables `agent_work_continuations`, `agent_external_side_effect_ledger`, `supervised_workers_continuation`).

Phase gating at admission:

- `live_handle_continuation` with `trigger_kind=operator_mcp` is the enabled admission path.
- `lead_auto` is admission-blocked behind a Phase 3 enablement gate; the decision-artifact and `continuation_instruction_sha256` verification path is wired and ready, but no admission accepts `lead_auto` until phase enablement.
- `provider_session_resurrection` is admission-blocked behind a Phase 4 per-adapter gate and is rejected unconditionally for all adapters until enablement.

Background worker behaviors — provider prompt send, side-effect ledger commit, terminal artifact settlement, supervised-worker registration/heartbeat, structured-log correlation, cancellation lifecycle, admission-timeout sweeper, and pre-prompt crash recovery — land with the Phase 2 worker. Negative fixtures under `docs/evidence/rollout-contract/p086/negative/` that depend on those behaviors carry `status="deferred_pending_phase2_worker"` until Phase 2 lands; fixtures that can be settled from the Phase 0 admission and schema surface (schema lint, malformed hashes, fingerprint conflict, lead-decision validation, resurrection ordering, unsupported-adapter resurrection, saturation drain) already carry `status="pass"`.
