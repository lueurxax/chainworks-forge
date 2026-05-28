# Per-Agent MCP Policy and Runtime Validation

Stable reference for the Rust control-plane MCP owner chain.

This document describes how per-agent MCP intent is compiled, resolved against the machine-local registry, persisted on executions, and exposed northbound.

## Purpose

MCP access is explicit execution truth, not an adapter side effect.

For any execution, the system must be able to answer:

- which backend profile owned the MCP request
- which MCP server IDs were requested
- which runtime entries were predicted to become active
- which ones actually became active
- which ones were denied
- and which northbound readers expose that truth

## Scope

This reference covers:

- `AgentEntry.backend_profile`
- `backend_profile.mcp`
- compilation into `ResolvedAgent`
- executor-time resolution against the machine-local MCP registry
- requested / predicted / actual / denied persistence on `AgentExecution`
- northbound report and GraphQL readers

It does not define:

- provider credentials
- generic transport selection outside MCP
- external MCP server authoring
- or export-only naming for evidence packs

## Canonical ownership

The canonical Rust owner chain is:

```text
AgentEntry.backend_profile
  -> backend_profile.mcp
  -> ResolvedAgent.backend_profile_id
  -> ResolvedAgent.requested_mcp_server_ids
  -> executor MCP resolver
  -> AgentExecution MCP provenance fields
  -> GraphQL / MCP northbound readers
```

Key rules:

- `backend_profile.mcp` is the only requested-intent owner for this slice.
- `required_tools` does not own MCP intent.
- legacy `mcp_profile` wording is not canonical for the Rust control-plane implementation.
- executable command/args/env definitions come from the machine-local MCP registry, not from the workflow catalog.

## Resolution model

The system tracks four distinct layers:

1. `requested`
2. `predicted`
3. `actual`
4. `denied`

These layers must not be collapsed.

### Requested

What the compiled agent binding asked for via `ResolvedAgent.requested_mcp_server_ids`.

### Predicted

What the executor expects to be realizable before ACP session startup after resolving against the local MCP registry and runtime binding.

### Actual

What the runtime session actually settled on.

### Denied

What was missing, disabled, unsupported, or blocked by runtime-specific rules.

## Resolution pipeline

The implemented Rust path is:

```text
workflow compiler
  -> ResolvedAgent
  -> executor-side MCP resolver
  -> ACP transport session/new
  -> AgentExecution settlement
  -> report / GraphQL readers
```

### Compiler ownership

The workflow compiler is responsible for freezing MCP request intent into the run plan:

- `ResolvedAgent.backend_profile_id`
- `ResolvedAgent.requested_mcp_server_ids`

That compiler output is the only MCP request intent the executor should consume.

### Runtime realization

Runtime realization resolves executable server definitions from the machine-local MCP registry:

- canonical path: `~/.config/mcp/config.yaml`
- explicit override: `CHAINWORKS_CODEX_CONFIG_PATH`
- one-time legacy migration source when canonical file is absent: `~/.config/goose/config.yaml`

Runtime/provider binding decides whether a requested entry is valid for the selected session family, including `stdio` versus `platform` filtering.

## Persistence

The durable MCP truth lives on `AgentExecution`.

Current persisted fields for this slice are expected to carry:

- requested MCP extension IDs
- predicted effective extension IDs
- predicted runtime IDs
- actual effective extension IDs
- actual runtime IDs
- denied extension IDs
- MCP session startup latency

These rows are the source of truth for northbound readers.

## Northbound readers

Northbound readers must consume persisted execution truth rather than reconstructing MCP state from raw transport payloads.

### MCP reports

`reports.get` and `report://{run_id}` should expose:

- requested
- predicted
- actual
- denied

for each execution that persisted MCP truth.

### GraphQL

GraphQL should expose:

- run-owned summaries at the run layer
- execution-owned MCP truth at an execution reader
- artifact-owned validation failure payloads at the artifact layer

The stage summary reader may expose coarse booleans, but it is not the owner for full execution MCP payloads.

## Integration with adjacent slices

This slice depends on and composes with:

- [workflow-execution-engine.md](workflow-execution-engine.md)
- [structured-output-envelope-and-contract-validation.md](structured-output-envelope-and-contract-validation.md)
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md)
- [acp-runtime-transport.md](acp-runtime-transport.md)

## Escalation policy integration

Per-agent policy/runtime validation feeds the escalation classifier: validation failures (missing permission, unsafe side-effect binding, etc.) surface as escalation pause reasons rather than bypassing this slice. The escalation chain's frozen `policy_hash` and binding data are produced by the workflow compiler alongside the per-agent policy resolution described above. Schema, pause-reason catalog, and policy-drift behavior live in [escalation-policies.md](escalation-policies.md).

## Current implementation anchors

- `control-plane/crates/workflow/src/compiler.rs`
- `control-plane/crates/workflow/src/plan.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/mcp-server/src/server.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `control-plane/crates/graphql-server/src/types/stage.rs`
- `control-plane/crates/graphql-server/src/types/artifact.rs`
- `control-plane/crates/db/src/repos/escalation.rs`
