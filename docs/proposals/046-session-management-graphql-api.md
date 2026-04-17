# Proposal 046: Session Management GraphQL API

| Field | Value |
|---|---|
| Date | 2026-04-17 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [043-query-projections-and-client-consumption-contract.md](043-query-projections-and-client-consumption-contract.md) |
| Scope | Expose session lineage, generation, and event data via GraphQL queries, add server-side KPI aggregation and proactive health indicators, and extend the resetSession mutation with audit context. |
| Goal | Make session lifecycle fully observable and debuggable through the GraphQL API, with pre-computed health signals that surface problems before they cause failures. |

---

## 1. Context and Motivation

The control-plane tracks session lineage, generations, and events in SQLite (tables: `session_lineages`, `session_generations`, `session_events`). It has a `resetSession` GraphQL mutation. But there are **no queries** to inspect sessions — the data is write-only from the API consumer's perspective.

The Swift app has `AgentSessionManager` with full inspection: session lineage browsing, generation status, event audit trails, and `SessionReuseKPIExporter` for metrics. None of this is accessible via the control-plane API.

An operator debugging a session-related failure must currently inspect the SQLite database directly. This proposal makes session data fully observable through GraphQL, with two improvements beyond Swift parity:

1. **Server-side KPI aggregation** — the Swift app exports raw KPI data for client-side computation. The control-plane should compute the aggregation server-side, which is more efficient and avoids redundant client logic.

2. **Proactive session health indicators** — instead of waiting for sessions to fail, surface warnings about sessions approaching budget limits, idle sessions, and binding fingerprint mismatches.

---

## 2. Product Questions This Proposal Must Answer

1. Can the operator list all session lineages for a given run?
2. Can the operator inspect every generation within a lineage, including token/cost tracking and status transitions?
3. Can the operator view the event audit trail for a session (created, reused, invalidated, reset, etc.)?
4. Can the system compute session reuse KPIs server-side (reuse rate, average lifespan, disposition breakdown)?
5. Can the system proactively surface session health warnings (budget exhaustion, idle sessions, fingerprint mismatches)?
6. Can the operator track session status changes in real-time via subscription?
7. Does the resetSession mutation record the operator's reason in the audit trail?

---

## 3. Scope

This proposal includes:

- 4 new GraphQL queries: `sessionLineages`, `sessionLineage`, `sessionGenerations`, `sessionEvents`.
- 2 new computed queries: `sessionKpiSummary`, `sessionHealth`.
- 1 new subscription: `sessionStatusChanged`.
- Enhancement to existing `resetSession` mutation (add `reason` field).
- Server-side aggregation logic for KPIs and health indicators.

This proposal does **not** include:

- MCP tool equivalents (session inspection is a UI/debugging concern best served by GraphQL).
- Changes to session persistence or reuse policy logic.
- Changes to the ACP runtime manager or provider adapters.

---

## 4. Problem Statement

### 4.1 Session data is write-only from the API

The `session_lineages`, `session_generations`, and `session_events` tables are populated during execution but no GraphQL query reads them. The only session-related API is the `resetSession` mutation.

### 4.2 Debugging session failures requires database access

When an agent fails due to session issues (budget exhaustion, binding mismatch, transport error), the operator must query SQLite directly to understand what happened. The `agent_executions` table has `session_reuse_disposition` and `session_reset_reason` fields but these are fragments — the full picture requires joining lineages, generations, and events.

### 4.3 KPI computation is duplicated client-side

The Swift app's `SessionReuseKPIExporter` computes reuse rate, disposition breakdown, and lifespan metrics by iterating over all session records for a run. Every client that wants these metrics must reimplement this computation.

### 4.4 Session problems are discovered only after failures

No mechanism warns the operator that a session is approaching budget limits, has been idle for an extended period, or has a stale binding fingerprint. These conditions are detectable but not surfaced.

---

## 5. Core Product Behavior

### 5.1 GraphQL Type Definitions

```graphql
type SessionLineage {
  id: ID!
  runId: ID!
  agentId: String!
  lineageId: String!
  sessionReuseScope: String
  sessionFamilyId: String
  activeGenerationId: String
  createdAt: DateTime!
  closedAt: DateTime
  generations: [SessionGeneration!]!
}

type SessionGeneration {
  id: ID!
  lineageId: ID!
  generation: Int!
  invocationOwnerKey: String
  providerSessionId: String
  bindingFingerprint: String
  status: SessionGenerationStatus!
  turnCount: Int!
  estimatedInputTokens: Int
  estimatedOutputTokens: Int
  cumulativeInputTokens: Int
  cumulativeOutputTokens: Int
  costCents: Float
  createdAt: DateTime!
  lastActivityAt: DateTime
  endedAt: DateTime
  endReason: String
}

enum SessionGenerationStatus {
  ACTIVE
  INVALIDATED
  CLOSED
  RESET
}

type SessionEvent {
  id: ID!
  lineageId: ID!
  generationId: String
  eventType: SessionEventType!
  recordedAt: DateTime!
  detailsJson: String
}

enum SessionEventType {
  CREATED
  REUSED
  INVALIDATED
  CLOSED
  OPERATOR_RESET
  BUDGET_EXCEEDED
  COMPACTED
}

type SessionKpiSummary {
  runId: ID!
  totalSessions: Int!
  totalReused: Int!
  totalReset: Int!
  totalInvalidated: Int!
  reuseRate: Float!
  averageGenerationLifespanSeconds: Float
  totalInputTokens: Int!
  totalOutputTokens: Int!
  totalCostCents: Float!
  dispositionBreakdown: [DispositionCount!]!
}

type DispositionCount {
  disposition: String!
  count: Int!
}

type SessionHealthReport {
  runId: ID!
  agents: [AgentSessionHealth!]!
  warnings: [SessionWarning!]!
}

type AgentSessionHealth {
  agentId: String!
  lineageId: ID!
  isAlive: Boolean!
  activeGenerationId: String
  budgetRemainingPercent: Float
  lastActivityAge: Duration
  bindingFingerprintValid: Boolean!
  turnCount: Int!
}

type SessionWarning {
  agentId: String!
  severity: WarningSeverity!
  kind: String!
  message: String!
}

enum WarningSeverity {
  INFO
  WARNING
  CRITICAL
}
```

### 5.2 Queries

```graphql
extend type Query {
  # List all session lineages for a run
  sessionLineages(runId: ID!): [SessionLineage!]!

  # Get single lineage with generations
  sessionLineage(id: ID!): SessionLineage

  # List generations for a lineage
  sessionGenerations(lineageId: ID!): [SessionGeneration!]!

  # List events, optionally filtered by generation
  sessionEvents(lineageId: ID!, generationId: ID): [SessionEvent!]!

  # Server-side aggregated KPIs
  sessionKpiSummary(runId: ID!): SessionKpiSummary!

  # Proactive health indicators
  sessionHealth(runId: ID!): SessionHealthReport!
}
```

### 5.3 Subscription

```graphql
extend type Subscription {
  # Fires on session create, reuse, invalidate, reset, close
  sessionStatusChanged(runId: ID!): SessionStatusEvent!
}

type SessionStatusEvent {
  lineageId: ID!
  generationId: String
  agentId: String!
  eventType: SessionEventType!
  timestamp: DateTime!
  details: String
}
```

### 5.4 Mutation enhancement

```graphql
extend type Mutation {
  # Enhanced: add reason for audit trail
  resetSession(
    runId: ID!
    stageId: String!
    reason: String  # NEW — recorded in session_events.details_json
  ): ResetSessionPayload!
}
```

### 5.5 KPI computation logic

`sessionKpiSummary` aggregates server-side via SQL:

```sql
SELECT
  COUNT(*) as total_sessions,
  COUNT(CASE WHEN status = 'closed' THEN 1 END) as closed,
  SUM(CASE WHEN generation > 1 THEN 1 ELSE 0 END) as reused,
  SUM(cumulative_input_tokens) as total_input_tokens,
  SUM(cumulative_output_tokens) as total_output_tokens,
  SUM(cost_cents) as total_cost_cents,
  AVG(JULIANDAY(COALESCE(ended_at, CURRENT_TIMESTAMP)) - JULIANDAY(created_at)) * 86400 as avg_lifespan_seconds
FROM session_generations g
JOIN session_lineages l ON g.lineage_id = l.id
WHERE l.run_id = ?;
```

Disposition breakdown from `agent_executions`:

```sql
SELECT session_reuse_disposition, COUNT(*) as count
FROM agent_executions
WHERE stage_execution_id IN (
  SELECT id FROM stage_executions WHERE run_id = ?
)
AND session_reuse_disposition IS NOT NULL
GROUP BY session_reuse_disposition;
```

### 5.6 Health computation logic

`sessionHealth` inspects live state:

1. For each lineage with `closed_at IS NULL`:
   - Find active generation (`status = 'active'`)
   - Compute `budgetRemainingPercent` from `cumulative_input_tokens / max_context_window * 100`
   - Compute `lastActivityAge` from `NOW() - last_activity_at`
   - Validate `bindingFingerprint` against current resolved agent binding

2. Generate warnings:
   - **CRITICAL**: `budgetRemainingPercent < 15%` → "Session approaching context budget limit"
   - **WARNING**: `lastActivityAge > 5 minutes` and session is active → "Session idle, may be stale"
   - **WARNING**: `bindingFingerprintValid = false` → "Agent binding changed, session will be invalidated on next use"
   - **INFO**: `turnCount > 20` → "High turn count, consider compaction"

---

## 6. Migration

### 6.1 Repository additions

Add to `db/repos/sessions.rs`:

- `list_lineages_by_run(pool, run_id)` → `Vec<SessionLineage>`
- `find_lineage(pool, id)` → `Option<SessionLineage>`
- `list_generations(pool, lineage_id)` → `Vec<SessionGeneration>`
- `list_events(pool, lineage_id, generation_id?)` → `Vec<SessionEvent>`
- `kpi_summary(pool, run_id)` → computed struct
- `disposition_breakdown(pool, run_id)` → `Vec<(String, i64)>`

### 6.2 GraphQL schema additions

Add to `graphql-server/src/schema.rs`:

- GQL types: `GqlSessionLineage`, `GqlSessionGeneration`, `GqlSessionEvent`, `GqlSessionKpiSummary`, `GqlSessionHealthReport`
- Query resolvers for all 6 queries
- Subscription resolver for `sessionStatusChanged`
- Filter `session_events` from `EventBus` domain events

### 6.3 Event bus extension

Add `SessionEventRecorded { lineage_id, generation_id, event_type }` variant to `DomainEvent` enum for subscription delivery.

### 6.4 resetSession mutation

Add `reason: Option<String>` to `ResetSessionCmd`. Record in `session_events.details_json` as `{ "reason": "..." }`.

---

## 7. Verification

- `sessionLineages(runId)` returns all lineages for a run with correct agent IDs and scopes.
- `sessionGenerations(lineageId)` returns generations sorted by generation number, with accurate token/cost tracking.
- `sessionEvents` returns events in chronological order, filtered by generation when specified.
- `sessionKpiSummary` matches manual computation from raw data (reuse rate, token totals, cost totals).
- `sessionHealth` flags sessions with budget < 15% as CRITICAL, idle > 5min as WARNING.
- `sessionStatusChanged` subscription fires within 1 second of session event recording.
- `resetSession` with `reason` persists the reason in session_events details_json.

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| KPI aggregation query slow on large runs | Low | Queries are bounded by run_id; add index on `session_lineages.run_id` if needed |
| Health check binding fingerprint validation requires agent resolution | Medium | Cache resolved bindings per run; fingerprint is a SHA-256 string comparison |
| Subscription volume high during parallel agent execution | Low | Sessions fire fewer events than runtime_status_changed; bounded by agent count per stage |
| Budget remaining % requires knowing max context window per model | Medium | Use backend_profile's model to look up context window from a static table; default to 128K if unknown |
