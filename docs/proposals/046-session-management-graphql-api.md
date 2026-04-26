# Proposal 046 Correction: Session Management GraphQL API Is Read/Subscription Only

| Field | Value |
|---|---|
| Date | 2026-04-25 |
| Status | Correction / Amendment |
| Corrects | `046-session-management-graphql-api.md` |
| Depends on | Proposal 072, Proposal 068 |
| Goal | Correct P046 so GraphQL exposes session lineage inspection, health, KPIs, and subscriptions, but does not expose session reset mutations. Reset and reset-agent-session remain MCP-only. |

---

## 1. Why this correction exists

P046 currently includes an enhancement to a `resetSession` GraphQL mutation.

That conflicts with the target boundary:

- SwiftUI uses GraphQL only,
- SwiftUI mutations are approval-only,
- reset/reset agent session are MCP-only.

Session data is still important to expose through GraphQL, but only as read and live status.

---

## 2. Corrected scope

P046 should include:

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
  sessionLineages(runId: ID!): [SessionLineage!]!
  sessionLineage(id: ID!): SessionLineage
  sessionGenerations(lineageId: ID!): [SessionGeneration!]!
  sessionEvents(lineageId: ID!, generationId: ID): [SessionEvent!]!
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

---

## 5. MCP ownership

Session reset operations must live in MCP:

- `sessions.reset`
- `sessions.reset_agent`
- or the canonical tool name already selected by the MCP control-plane implementation.

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
- suggested MCP command.

SwiftUI may not reset a session.

Example UI copy:

> “Session lineage appears stale. Suggested MCP action: `sessions.reset_agent`.”

---

## 7. Acceptance criteria correction

P046 is complete when:

1. session inspection works through GraphQL queries;
2. session live updates work through GraphQL subscriptions;
3. server-side session KPIs and health indicators are visible;
4. no GraphQL session reset mutation exists;
5. reset flows are documented as MCP-only;
6. UI does not expose reset controls.

---

## 8. Final recommendation

Session observability belongs in GraphQL.

Session control belongs in MCP.

This preserves a thin UI while still giving the operator enough visibility to understand session problems.
