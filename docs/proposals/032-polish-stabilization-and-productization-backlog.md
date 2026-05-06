# Proposal 032: Polish, Stabilization, and Productization Backlog

| Field | Value |
|---|---|
| Date | 2026-04-01 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposals 027, 029, 042, the server parity harness contracts in [p041-generated-artifact-schemas.md](../reference/p041-generated-artifact-schemas.md) and [test-gates.md#proposal-041p041](../reference/test-gates.md#proposal-041p041), and the canonical thin UI read contract in [query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md) |
| Goal | Collect and organize all remaining polish, stabilization, productization, and follow-on improvements after parity extraction, MCP exposure, query contracts, daemon lifecycle, and thin-client cutover land. |

## 1. Why this proposal exists

After the structural rewrite, there will still be many important but non-blocking pieces left:

- observability gaps
- docs
- design polish
- onboarding
- packaging
- auth hardening
- performance tuning
- more runtime adapters
- experiments
- release quality improvements

Proposal 032 is the intentional backlog bucket for that work.

It is supposed to stay partially open and grow over time.

## 2. Backlog buckets

### 2.1 Product polish
- approval UX polish
- artifact viewer polish
- report readability
- better operator summaries
- notifications and presence

### 2.2 Reliability
- retry ergonomics
- better recovery explanations
- projection rebuild tooling
- service health diagnostics
- runtime adapter health checks

### 2.3 Observability
- richer metrics
- burn telemetry
- adapter comparison dashboards
- experiment reporting
- trace/log correlation

### 2.4 Runtime expansion
- more ACP adapters
- better capability matrix
- adapter fallback policy
- runtime A/B infrastructure

### 2.5 Security / auth
- auth model hardening
- audit trail expansion
- role-based MCP tool exposure
- operator/admin separation

### 2.6 Packaging / deployment
- local dev topology
- desktop packaging strategy
- daemon lifecycle
- Temporal deployment story
- service distribution options

### 2.7 Documentation
- architecture docs
- operator docs
- runtime adapter docs
- migration docs
- internal troubleshooting docs

### 2.8 Thin UI productization and dogfood

The GraphQL-only read boundary is implemented and documented as repository truth in [query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md). The technical closeout evidence is complete; this backlog owns the remaining productization and operator-acceptance work over that boundary.

- Honest operator dogfood after P036 restores enough inspection ergonomics to evaluate real workflows.
- Follow-up workflow-completion notes from the release-owner sign-off.
- Release-candidate readiness for daemon lifecycle behavior, schema mismatch detection, and the operator-facing update-daemon flow.
- Productization of the read-only write-path guidance so operators understand which actions are external, unavailable, or approval-only.
- Coordination with later write-path proposals when create/start/cancel/retry/reset/clone/recover/context actions need approved transports.

These items are stabilization/productization work. They must not reintroduce MCP reads/writes, non-approval GraphQL mutations, local workflow truth, or old Swift-local execution paths into the governed macOS UI.

## 3. How to use this proposal

Proposal 032 is not expected to be “finished” in one pass.
It should be treated as a structured backlog home for everything left after the big architectural transition.

Each new leftover item should be assigned into a bucket.
If a bucket grows into a coherent large slice, it can be split into a dedicated later proposal.

## 4. Non-goals

Proposal 032 is intentionally not a tightly bounded implementation slice.
It is a managed backlog and stabilization container.

## 5. Acceptance criteria

Proposal 032 is useful when:

1. the remaining work is organized rather than scattered,
2. polish and stabilization items have a clear home,
3. later follow-on proposals can be split out cleanly,
4. the team does not lose track of important non-structural work after the big rewrite.

## 6. Final recommendation

Keep Proposal 032 intentionally light now.

It should serve as the living place for all remaining “this still needs care” work after the parity replica, MCP server, daemon lifecycle, query/read contract, and thin-client cutover land.
