# Proposal 101: Agent Limit Observatory and Runtime Budget Dashboard

| Field | Value |
|---|---|
| Date | 2026-06-09 |
| Status | Draft / Parked until P073 freeze lifts |
| Author | Roadmap triage 2026-06-09 |
| Depends on | Implemented SQLite write-budget contract and evidence-spooling baseline (P087), durable side-effect ledger (P078), UI action boundary (P072) |
| Related | `docs/reference/rust-control-plane.md`, `docs/reference/query-projections-and-client-consumption-contract.md`, `docs/reference/ui-action-boundary.md`, `docs/reference/executable-rollout-gate-template.md` |
| Scope | Durable, read-only accounting of provider usage and limit pressure per run/stage/agent execution: tokens, request counts, rate-limit and quota hits, and cost estimates where the provider exposes them, surfaced through GraphQL readback for the UI and MCP readback for agents/operators. |
| Non-goal | No scheduling, throttling, fallback, or session-pool behavior (that is P102). No new UI write surfaces. No new MCP command tools. No per-chunk row persistence in SQLite. |

---

## 1. Problem

Chainworks runs consume provider budgets (tokens, requests, rate limits,
session quotas) with no durable, queryable accounting. When a run slows down or
an agent stalls against a provider limit, the operator has no readback that
answers: which binding hit which limit, how much budget a run consumed, and
which provider/model is trending toward exhaustion. Limit work has been on the
roadmap backlog as "number TBD" since the 2026-05-06 roadmap update; this
proposal reserves the number and fixes honest scope.

## 2. Goals

- G-1: Record per-agent-execution usage observations (tokens in/out where the
  provider reports them, request counts, rate-limit/quota events, wall-clock)
  keyed by run/stage/execution identity from the execution-truth model.
- G-2: Aggregate observations per run, per provider binding, and per day into
  compact SQLite rows; spool high-volume raw samples to evidence files under
  the existing spooling convention.
- G-3: Expose read-only GraphQL queries/subscriptions for the operator UI
  (run budget summary, provider limit pressure, recent limit events).
- G-4: Expose read-only MCP readback (`limits.inspect`-shaped) for operators
  and agents, consistent with the mixed-inbox readback style.
- G-5: Emit typed limit events (`limit_observed`, `quota_exhausted_observed`)
  into the existing event stream for downstream consumers, observation-only.

## 3. Non-Goals

- No policy reactions: no pausing, retrying, rerouting, or provider fallback.
- No release/side-effect lane interaction of any kind.
- No SwiftUI mutations; the UI remains GraphQL read/subscription plus the two
  approval mutations.
- No provider-API polling beyond what ACP sessions already surface.

## 4. Design Sketch

- The ACP transport and adapters already see provider stream metadata and
  rate-limit error shapes; a thin observation tap maps them to a
  `usage_observation` domain event.
- A `limit_observatory` projection owns aggregates: `run_budget_summary`,
  `provider_limit_pressure`, `limit_event_recent`. Writes go through the
  DbWriter lane under an explicit write class; raw samples spool to
  `.chainworks` evidence files with manifest pointers.
- GraphQL adds query/subscription fields mirroring the projection; MCP adds a
  read-only inspection tool. Both reuse existing principal/boundary policy:
  `ui_operator` gets reads only.

## 5. Rollout Gates and Observability Contract

- Gate: `./scripts/test-gate.sh proposal-101` — projection unit tests, write-class
  conformance, GraphQL/MCP readback fixtures, boundary denial fixtures.
- Metrics: `limit_observations_total`, `limit_projection_lag_seconds`,
  `limit_spool_bytes_total`.
- Readback: `operator_readback_v1` lanes for run budget summary and provider
  limit pressure; fields versioned from first release.
- Hold conditions: write-budget regression (observation writes exceeding the
  declared write class), projection lag breaching threshold, or any mutation
  surfacing on the UI principal.
- Rollback disposition: observation tap and projection are additive; disable by
  feature flag, projection tables drop cleanly, no domain writes depend on them.

## 6. Acceptance

- A completed run shows a budget summary with non-zero usage via GraphQL and
  MCP readback in the canonical gate fixtures.
- A simulated provider rate-limit error produces a typed `limit_observed`
  event and appears in the recent-events readback.
- Boundary tests prove no new mutations exist for UI/default principals.

## 7. Open Questions

- Which providers expose token counts reliably over ACP today, and what is the
  honest "unknown" representation for those that do not?
- Whether cost estimation belongs in v1 or behind a follow-up once token truth
  per provider is proven.
- Retention windows for raw spooled samples vs. SQLite aggregates.
