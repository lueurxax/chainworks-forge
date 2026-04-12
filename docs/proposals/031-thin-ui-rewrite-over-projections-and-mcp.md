# Proposal 031: Thin UI Rewrite Over Projections and MCP

| Field | Value |
|---|---|
| Date | 2026-04-01 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 027, Proposal 029, Proposal 041, Proposal 042, Proposal 043 |
| Goal | Make the first user-visible cutover from client-owned logic to a thin UI over GraphQL projections and MCP-driven control commands. |

## 1. Why this proposal exists

Once server parity is proven, daemon lifecycle is productized, GraphQL projections are defined, and MCP is available as the control surface, the UI should stop pretending to be a semi-independent runtime.

The UI should become:
- a renderer of read models,
- an initiator of commands,
- a place for operator ergonomics,
- not a place where workflow truth is decided.

## 2. Outcome

After Proposal 031:

- the UI no longer owns business logic,
- all operator actions route through MCP-backed control commands,
- read surfaces come from service projections,
- UI state becomes disposable,
- the client becomes easier to rewrite, swap, or replace.

Proposal 031 is the first proposal in this roadmap that changes the user-visible ownership boundary.

## 3. Core decision

### 3.1 Commands through MCP
All operator mutations should flow through MCP:
- start run
- approve/reject
- retry
- reset session
- cancel
- compare
- run experiments

### 3.2 Reads from projections
View surfaces should render from:
- GraphQL projections
- GraphQL queries
- optional GraphQL subscriptions or equivalent server-owned streaming updates

MCP remains the control path.
GraphQL is the canonical read path.

## 4. UI surface plan

### 4.1 Primary views
- Runs home
- Run detail
- Stage detail
- Approval inbox
- Artifact viewer
- Report viewer
- Experiment comparison view
- Runtime health / adapter state

### 4.2 Required behavioral simplification
The UI must not:
- compute the next stage,
- decide retry legality,
- infer settlement truth,
- reconstruct runtime state from partial artifacts,
- or improvise recovery actions.

It should only render and invoke.

## 5. Data flow

```text
UI
  -> GraphQL queries/subscriptions
  -> invoke MCP control commands
  -> render results and status
```

## 6. Migration strategy

### Phase 1
- build thin screens against the new read models
- mirror current key surfaces

### Phase 2
- route control actions through MCP
- remove client-owned orchestration logic

### Phase 3
- remove obsolete local state stores and orchestration code
- keep only UI-local presentation state

This cutover must not happen before:

- parity proof is green (Proposal 041)
- daemon lifecycle is dependable (Proposal 042)
- the query/projection contract is explicit (Proposal 043)

## 7. Design goals

The UI rewrite should optimize for:
- clarity
- operator confidence
- fast run inspection
- easy retry/recovery
- consistency across runtime backends

It should not optimize for:
- clever client-side orchestration
- hidden local caches that become alternate truth
- protocol-specific UI hacks

## 8. Non-goals

Proposal 031 does **not**:
- redefine workflow semantics,
- add new orchestration behavior,
- decide runtime backends,
- or create a second control plane.

## 9. Risks

### 9.1 Thin UI becomes too passive
Risk:
- UI loses usability because all intelligence moved out.

Mitigation:
- keep rich projections,
- keep operator-focused summaries,
- keep report and evidence surfaces strong.

### 9.2 UI talks to too many transports
Risk:
- client complexity returns through multiple adapters.

Mitigation:
- one read path,
- one control path,
- explicit client boundary.

## 10. Acceptance criteria

Proposal 031 is complete when:

1. the UI renders run/stage/approval/artifact/report state from service projections,
2. GraphQL is the canonical read/query surface for the client,
3. operator actions are executed through MCP-backed commands,
4. the client no longer owns orchestration logic,
5. removing the client would not destroy workflow truth,
6. the product remains usable and debuggable from the UI.

## 11. Final recommendation

Proposal 031 should make the UI intentionally smaller in responsibility and stronger in clarity.

The system should no longer depend on the client being alive, correct, and stateful in order for the workflow engine to behave.
