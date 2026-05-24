# Proposal 046: Session Management GraphQL API Is Read/Subscription Only

| Field | Value |
|---|---|
| Date | 2026-05-21 |
| Status | Approved R4 / implementation contract |
| Corrects | Earlier P046 text that allowed `resetSession` |
| Depends on | [UI action boundary](../reference/ui-action-boundary.md), Proposal 068 |
| Goal | Expose bounded GraphQL session lineage, event, KPI, health, and live-status observability without adding any GraphQL session reset or control mutation. Reset and reset-agent-session remain MCP-only. |

---

## 1. Why this correction exists

Earlier P046 text included an enhancement to a `resetSession` GraphQL mutation.

That conflicts with the implemented UI action boundary:

- SwiftUI uses GraphQL only,
- SwiftUI mutations are approval-only,
- reset/reset agent session are MCP-only.

Session data is still important to expose through GraphQL, but only as bounded readback and live status notification.

---

## 2. Corrected scope

P046 should include:

- `sessionObservabilityAvailable` capability probe
- `sessionLineages`
- `sessionLineage`
- `sessionGenerations`
- `sessionEvents`
- `sessionKpiSummary`
- `sessionHealth`
- `sessionStatusChanged`

P046 should remove:

- `resetSession` GraphQL mutation
- mutation audit context for reset
- any UI reset operation through GraphQL

---

## 3. Corrected product questions

Keep:

1. Can the operator list session lineages for a given run?
2. Can the operator inspect generations and events?
3. Can the system compute KPIs server-side?
4. Can the system proactively surface session health warnings?
5. Can the operator track session status changes in real time via subscription?

Replace:

> Does the resetSession mutation record the operator's reason?

with:

> Does the UI clearly route reset actions to MCP-only operator flows without exposing a GraphQL mutation?

---

## 4. Corrected GraphQL schema intent

Allowed:

```graphql
extend type Query {
  sessionObservabilityAvailable: Boolean!
  sessionLineages(runId: ID!, first: Int, after: String): SessionLineageConnection!
  sessionLineage(id: ID!): SessionLineage
  sessionGenerations(lineageId: ID!, first: Int, after: String): SessionGenerationConnection!
  sessionEvents(lineageId: ID!, generationId: ID, first: Int, after: String): SessionEventConnection!
  sessionKpiSummary(runId: ID!): SessionKpiSummary!
  sessionHealth(runId: ID!): SessionHealthReport!
}

extend type Subscription {
  sessionStatusChanged(runId: ID!): SessionStatusChangedEvent!
}
```

Forbidden:

```graphql
extend type Mutation {
  resetSession(...)
}
```

or any equivalent session reset mutation.

The connection fields use `nodes`, `edges`, and `pageInfo`; resolvers apply deterministic ordering, opaque cursors, default limits, maximum limits, and sanitized `invalid cursor` errors. `sessionLineages` and `sessionGenerations` default to 100 rows and cap at 500. `sessionEvents` defaults to 200 rows and caps at 1000.

Every P046 GraphQL field is gated by `CHAINWORKS_GRAPHQL_SESSION_OBSERVABILITY` and absent from the schema when disabled. Governed clients must probe capability/schema availability before constructing P046 documents.

All P046 resolvers require operator-read authorization for the owning run. ID-based resolvers first resolve parent lineage/generation ownership, then return not-found-or-not-visible behavior for absent or unauthorized rows. Event details use the `p046_event_details_redaction_v1` default-deny allowlist, and raw `providerSessionId`, `bindingFingerprint`, `invocationOwnerKey`, and absolute working directories are replaced by derived operator-safe fields.

---

## 5. MCP ownership

Session reset operations must live in MCP:

- use the canonical MCP session reset capability selected by the MCP control-plane implementation.
- P046 does not add, rename, or document a new MCP reset tool name.

MCP reset tools must own:

- reason,
- caller identity,
- journal/receipt,
- reset target,
- idempotency/retry semantics,
- recovery/report evidence.

---

## 6. UI behavior

SwiftUI may show:

- session lineages,
- session generations,
- session events,
- session health warnings,
- reset recommendation,
- generic suggested MCP action.

SwiftUI may not reset a session.
SwiftUI must gate P046 documents on capability/schema availability, keep P046 readback as transient MainActor UI state, and avoid SwiftData persistence or AppKit-owned GraphQL tasks for this proposal.

Example UI copy:

> “Session lineage appears stale. Suggested MCP action: use the MCP session reset capability.”

---

## 7. Acceptance criteria correction

P046 is complete when:

1. session inspection works through bounded GraphQL connection queries;
2. session live updates work through the run-scoped `sessionStatusChanged` subscription;
3. server-side session KPIs and health indicators are visible;
4. every P046 query and subscription emission enforces operator-read authorization for the owning run;
5. no GraphQL session reset or equivalent session-control mutation exists;
6. reset flows are documented as MCP-only;
7. UI does not expose reset controls and does not issue P046 documents when the schema/capability probe says fields are absent;
8. rollout-contract fixtures and the `proposal-046` gate cover authorization, pagination, redaction, disabled schema behavior, retry exhaustion, subscription backpressure, and absence of reset/control mutations.

---

## 8. Final recommendation

Session observability belongs in GraphQL.

Session control belongs in MCP.

This preserves a thin UI while still giving the operator enough visibility to understand session problems.
