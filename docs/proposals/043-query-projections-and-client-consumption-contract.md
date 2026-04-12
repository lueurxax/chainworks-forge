# Proposal 043: Query Projections and Client Consumption Contract

| Field | Value |
|---|---|
| Date | 2026-04-11 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 027 |
| Goal | Define the GraphQL read-model and query contract that the thin client consumes once workflow truth moves behind the local server. |

## 1. Why this proposal exists

Northbound MCP is the right control plane.
GraphQL is the target read path for the UI.

If the repo does not explicitly separate command/control from query/projection reads, the thin-client rewrite will drift into an unstable mix of:

- MCP for some reads,
- ad hoc direct queries for others,
- and client-owned reconstruction for the hard cases.

This proposal fixes that by defining the GraphQL-first query contract up front.

## 2. Outcome

After Proposal 043:

- the server exposes explicit read models and query surfaces for the client,
- GraphQL becomes the canonical read/query plane for the client,
- the client knows what it may render and what it must never reconstruct,
- MCP remains the mutation/control plane,
- UI refresh, detail views, and operator summaries have a stable read contract.

## 3. Architectural rule

The system should separate:

- MCP for commands and control mutations
- GraphQL query/projection APIs for read surfaces

The UI should not be forced to tunnel all list/detail rendering through MCP.

## 4. Scope

This proposal includes:

- required server-owned projections
- GraphQL query surfaces for the client
- optional GraphQL subscription surfaces for live UI
- projection invariants
- streaming or refresh expectations
- ownership rules for client consumption

This proposal does **not** include:

- MCP tool design,
- workflow semantics redesign,
- or UI layout redesign.

## 5. Required read surfaces

The client will likely need at least:

- runs home summary
- run detail
- stage detail
- approval inbox
- artifact metadata and retrieval
- report summaries and comparisons
- runtime health views

For each surface, the repo should define:

- the projection owner,
- the query contract,
- freshness expectations,
- and what the client is forbidden to infer on its own.

## 6. Projection invariants

The projections must be explicit enough that the client does not:

- compute next stage,
- infer recovery legality,
- guess run terminality,
- reconstruct approval truth,
- or stitch together runtime state from artifact fragments.

If a UI needs a fact, the server must publish it as part of a read model or query surface.

## 7. Risks

### 7.1 MCP becomes an accidental read bus

Risk:
- UI performance and clarity degrade because high-frequency reads are squeezed through command-oriented surfaces.

Mitigation:
- keep MCP northbound for mutation and control,
- define GraphQL as the explicit read contract for UI consumption.

### 7.2 Client-owned truth leaks back in

Risk:
- the thin client quietly reconstructs state from partial records.

Mitigation:
- explicit projection invariants,
- server-owned read models,
- no heuristic UI truth.

## 8. Relationship to other proposals

- Proposal 027 provides the server-side parity replica.
- Proposal 029 provides the northbound MCP command plane.
- Proposal 031 depends on this proposal for its read path.

## 9. Acceptance criteria

Proposal 043 is complete when:

1. required UI read surfaces are enumerated,
2. each surface has a defined server-owned projection or query contract,
3. GraphQL is explicitly documented as the canonical client read plane,
4. client reconstruction responsibilities are explicitly prohibited,
5. the separation between MCP control path and GraphQL read path is documented,
6. thin-client cutover can depend on this proposal without inventing new read semantics.

## 10. Final recommendation

Proposal 043 should land before thin-client cutover.

Without it, the UI rewrite will mix command and read concerns and recreate client-owned truth by accident.
