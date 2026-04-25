# Proposal 051 Dependency Audit (Pre-scheduling)

Status: **required before `p051-scaffold` scheduling**
Date: 2026-04-25
Owner: Chainworks operator planning

## Artifact purpose
The rows below resolve P051 dependency gating and sequencing before implementation starts. Each row includes a concrete canonical artifact link, owner, current gate/status, remaining gaps, and explicit parallel-vs-sequential decision.

## Required dependency table

| Proposal | Canonical artifact (checked-in) | Owner | Gate / status | Remaining gaps | Parallel-vs-sequential decision | Blocking threshold |
|---|---|---|---|---|---|---|
| P025 | _Not checked in_ (no `docs/proposals/025-*` file found) | Unclear (artifact missing in checked-in proposal tree) | **Missing canonical artifact** | Cannot verify readiness without a durable contract | **Blocked** (must locate or explicitly narrow scope) | Blocks P051 scaffold scheduling until resolved |
| P026 | _Not checked in_ (no `docs/proposals/026-*` file found) | Unclear (artifact missing in checked-in proposal tree) | **Missing canonical artifact** | Cannot verify readiness without a durable contract | **Blocked** (must locate or explicitly narrow scope) | Blocks P051 scaffold scheduling until resolved |
| P029 | `docs/reference/mcp-northbound-control-plane-server.md` | Control-plane auth/runtime authors | **Stable implemented reference** | Verify whether new Xcode broker MCP/GraphQL call patterns require any follow-up security-owner exceptions | **Parallel** with P051 prep once call-visibility and command-journal surfaces are confirmed | Does not block if audit confirms contract surfaces are current |
| P037 | `docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md` (Draft) | Codex | **Draft** | Core execution supervision behavior is not yet finalized in this proposal; scheduling should treat P051 as dependent on watchdog readiness for any production-like rollout assumptions | **Sequential** for rollout assumptions; some scaffolding can proceed in parallel only if evidence shows no direct contract conflict | Blocks broad `shim_enforced`/release rollout until readiness is confirmed |
| P049 | `docs/proposals/049-context-strategy-management-mcp-tools.md` (Draft) | Andrey Khasanov | **Draft** | Context strategy controls are not yet implemented, leaving a gap for adaptive budget-driven run behavior during future operator routing | **Sequential** for rollout assumptions; allow scaffolding if scope is narrowed to fixed strategy profile assumptions only | May block production rollout evidence for adaptive operators; can continue scaffold on fixed defaults |

## Audit status

- `P026`: **hard blocker** until a canonical artifact is recovered or an explicit exception scope is approved.\
- `P029`: **must pass** with explicit call-surface verification before broad rollout states (`shim_enforced`).\
- `P037` / `P049`: can be parallelized only for isolated P051 scaffolding if no shared release-goal assumptions are taken; otherwise treated as sequencing dependencies for rollout readiness.

