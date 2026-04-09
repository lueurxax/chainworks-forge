# Proposal 027: Go + Temporal Control Plane Extraction

| Field | Value |
|---|---|
| Date | 2026-04-01 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Current product semantics already established in the client: execution truth, contract truth, session lineage, proposal-loop fidelity, MCP policy, ACP direction |
| Goal | Move all orchestration and domain logic out of the client into a Go service backed by Temporal, while preserving current product semantics and keeping the client functional during the transition. |

## 1. Why this proposal exists

The client currently owns too much:
- run lifecycle
- stage transitions
- retries
- approvals
- report truth
- recovery logic
- session lineage behavior
- proposal-loop orchestration
- runtime integration glue

That makes the UI heavy, the architecture hard to evolve, and every runtime change more expensive than it should be.

The next architectural step is to move product logic out of the client and into a proper control-plane service.

This proposal does **not** try to redesign product semantics.
It moves existing semantics into a better home.

## 2. Outcome

After Proposal 027:

- the client is no longer the owner of orchestration logic,
- the Go service becomes the owner of workflow truth,
- Temporal becomes the durable orchestration substrate,
- the client becomes a thin consumer of projections and commands,
- no existing run/report/recovery semantics should regress.

## 3. Core architectural decision

### 3.1 Temporal owns durable workflow execution

Temporal should own:
- workflow progression,
- waiting on approvals,
- retries,
- restart-safe orchestration,
- long-running state transitions,
- cancellation,
- signals/updates/queries.

Temporal should **not** own:
- business semantics definition,
- artifact rendering,
- UI presentation,
- provider-specific transport details,
- MCP northbound interface.

### 3.2 Go service owns domain truth and read models

The Go service should own:
- canonical run/stage/agent state projections,
- workflow and agent catalog loading,
- runtime adapter selection,
- artifact metadata and report metadata,
- reconciliation of Temporal state into queryable read models,
- command validation before workflow signals/updates are sent.

### 3.3 UI stops owning logic

The client should stop deciding:
- what stage comes next,
- how retries work,
- how approvals mutate state,
- how session resets are applied,
- what recovery paths are legal.

The client should eventually become:
- query renderer,
- artifact viewer,
- approval/retry/reset command initiator.

## 4. Proposed system shape

```text
Client UI
  -> query/read API
  -> command API

Go Control Plane
  -> Temporal workflows
  -> activities
  -> projections/read models
  -> artifact metadata layer
  -> runtime adapter layer

Southbound runtimes
  -> Goose legacy adapter
  -> future ACP adapters
  -> provider-specific activities
```

## 5. Temporal model

### 5.1 Top-level workflows

Initial top-level workflows:

- `RunWorkflow`
- `ProposalLoopWorkflow`
- `ImplementationLoopWorkflow`
- `ReleaseWorkflow`
- `StewardExperimentWorkflow` (later)

Initial guidance:
- keep `RunWorkflow` as the primary parent workflow,
- split child workflows only where the boundary is already meaningful in product semantics,
- do **not** create one Temporal workflow per agent invocation.

### 5.2 Activities

All nondeterministic work must live in activities, including:
- agent/runtime invocation
- artifact persistence
- Git and release operations
- MCP/runtime preflight
- validation passes
- proposal backlog generation
- report materialization

### 5.3 Signals / updates / queries

Temporal-facing control surface should support at least:

Signals / updates:
- approve
- reject
- retry stage
- retry agent
- reset session lineage
- cancel run
- clone from snapshot

Queries:
- run summary
- stage state
- pending approvals
- unresolved proposal backlog
- active runtime adapter info
- session lineage summary

### 5.4 History control

Proposal 027 must assume that some runs will be long and noisy.

Therefore:
- `Continue-As-New` should be planned from the beginning,
- large loop-heavy runs must not rely on unbounded Temporal history,
- read models must not depend on replaying full workflow history in the client.

## 6. Read-model strategy

Temporal history is not the UI database.

The Go service should maintain explicit read models for:
- runs
- stages
- agent executions
- approvals
- artifacts
- proposal-loop quality metrics
- recovery suggestions
- runtime/session state

These projections should be queryable without requiring the client to understand Temporal internals.

## 7. Migration strategy

### Phase 1 — carve out domain and transport seams
- define service-owned domain contracts
- isolate runtime adapters
- stop adding new business logic to the client

### Phase 2 — implement Go orchestration service
- temporal workflows
- activities
- read models
- command/query API
- bridge to existing runtime adapters

### Phase 3 — dual-run / feature flag path
- selected slices run through Go/Temporal
- current client path remains available as fallback
- validate parity on live runs

### Phase 4 — client stops owning orchestration
- all new execution commands route through service
- client becomes projection/command consumer

## 8. Non-goals

Proposal 027 does **not**:
- add MCP northbound server support,
- rewrite the UI,
- force ACP migration,
- remove Goose,
- solve runtime-provider comparison,
- package a final production deployment topology.

Those belong to later proposals.

## 9. Risks

### 9.1 Temporal misuse
Risk:
- putting nondeterministic logic in workflow code,
- replay pain,
- hard-to-debug workflow versioning issues.

Mitigation:
- strict activity boundaries,
- one workflow rulebook,
- versioning policy from day one.

### 9.2 Split-brain truth during migration
Risk:
- client and service both think they own run truth.

Mitigation:
- explicit authority transfer plan,
- feature-flagged slices,
- no new product semantics implemented in the client once migration begins.

### 9.3 Projection drift
Risk:
- read models drift from workflow truth.

Mitigation:
- projections treated as derived state,
- rebuild/reconciliation tooling,
- durable audit linking workflow events to read-model updates.

## 10. Acceptance criteria

Proposal 027 is complete when:

1. orchestration logic is executable in the Go service,
2. Temporal owns run progression for at least one real workflow slice,
3. the client can render run/stage/approval/artifact state entirely from service projections,
4. approval and retry flows work via service commands,
5. current product semantics do not regress,
6. the system can survive process restart without losing orchestration truth,
7. the client is no longer the owner of workflow decisions.

## 11. Final recommendation

Proposal 027 should be treated as the structural turning point of the system.

The goal is not to “rewrite in Go because Go is nicer.”
The goal is to put workflow truth, retries, approvals, and durable orchestration into a substrate that is built for long-running, restart-safe execution.

The client should stop being the brain.
It should become a well-behaved view layer over a real control plane.
