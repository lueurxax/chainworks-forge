# Proposal 051 Dependency Audit (Pre-scheduling)

Status: **fixture dependency posture reconciled; broad rollout still requires dogfood/sign-off**
Date: 2026-04-25
Owner: Chainworks operator planning

## Artifact purpose
The rows below resolve P051 dependency gating and sequencing before implementation starts. Each row includes a concrete implemented-system artifact link, owner, current gate/status, remaining gaps, and explicit parallel-vs-sequential decision.

Repo policy prefers current `docs/reference/` truth over old proposal lineage. The old `docs/proposals/025-*` and `docs/proposals/026-*` proposal files are not checked in, but P025/P026 implemented-system references and gates are present. The release stop sign for P051 is now live dogfood/sign-off, not missing proposal-lineage files.

## Required dependency table

| Proposal | Canonical artifact (checked-in) | Owner | Gate / status | Remaining gaps | Parallel-vs-sequential decision | Blocking threshold |
|---|---|---|---|---|---|---|
| P025 | `docs/reference/per-agent-mcp-policy-and-runtime-validation.md` plus `scripts/test-gate.sh proposal-025|p025` | MCP policy/runtime authors | **Implemented reference and registered gate present** | Historical `docs/proposals/025-*` lineage artifact is absent; no current P051 fixture blocker found because requested/predicted/actual/denied MCP truth is now reference-owned. | **Parallel** with P051 fixture/readback work. | Blocks P051 only if MCP execution truth or report readback is removed or the P025 gate disappears. |
| P026 | `docs/reference/acp-runtime-transport.md` plus `scripts/test-gate.sh proposal-026|p026` | ACP runtime transport authors | **Implemented reference and registered gate present** | Historical `docs/proposals/026-*` lineage artifact is absent; no current P051 fixture blocker found because adapter-family/runtime binding is now reference-owned. | **Sequential prerequisite satisfied for fixture work**; preserve ACP runtime selection truth when attaching broker leases. | Blocks P051 if ACP runtime profile binding, adapter-family selection, or launch/readback truth is incomplete. |
| P029 | `docs/reference/mcp-northbound-control-plane-server.md`; `docs/evidence/029-mcp-northbound-control-plane-server/README.md`; `scripts/test-gate.sh proposal-029|p029` and `proposal-029-mcp|p029-mcp` | Control-plane auth/runtime authors | **Stable implemented reference and evidence present** | P051 must preserve bearer auth, capability filtering, command-journal/readback, and `reports.get` ownership. | **Parallel** with P051 prep; P051 adds Xcode broker observations without bypassing P029 ownership. | Blocks P051 if MCP bearer auth, capability filtering, command journal/readback, or `reports.get` ownership is absent. |
| P037 | `docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md` (Draft) | Codex | **Draft** | Core execution supervision behavior is not yet finalized in this proposal; scheduling should treat P051 as dependent on watchdog readiness for any production-like rollout assumptions | **Sequential** for rollout assumptions; some scaffolding can proceed in parallel only if evidence shows no direct contract conflict | Blocks broad `shim_enforced`/release rollout until readiness is confirmed |
| P049 | `docs/proposals/049-context-strategy-management-mcp-tools.md` (Draft) | Andrey Khasanov | **Draft** | Context strategy controls are not yet implemented, leaving a gap for adaptive budget-driven run behavior during future operator routing | **Sequential** for rollout assumptions; allow scaffolding if scope is narrowed to fixed strategy profile assumptions only | May block production rollout evidence for adaptive operators; can continue scaffold on fixed defaults |

## Audit status

- `P025` / `P026`: **not current fixture blockers** because implemented reference docs and gate registrations exist; the absent historical proposal files remain noted but are not the source of truth for implemented behavior.\
- `P029`: **satisfied for fixture/readback work** by stable reference, evidence package, and gate registrations; keep call-surface compatibility under review before broad rollout.\
- `P037` / `P049`: can be parallelized only for isolated P051 scaffolding if no shared release-goal assumptions are taken; otherwise treated as sequencing dependencies for rollout readiness.

## P029 call-surface verification evidence

| check | status | owner | location |
|---|---|---|---|
| `xcode_mcp` call/response shape in ACP HTTP transport | **Covered by fixture gate** | control-plane auth/runtime | `control-plane/crates/acp/tests/integration.rs`; `./scripts/test-gate.sh proposal-051` |
| `GraphQL::reports.get` exposure for `xcode_broker_health` and observation streams | **Covered by compile/readback checks** | API contract owner | `control-plane/crates/graphql-server/src/schema.rs`; `docs/reference/mcp-northbound-control-plane-server.md`; `./scripts/test-gate.sh proposal-051` |
| `mcpbridge` command-journal path and broker failure handling | **Covered by fixture gate** | runtime platform owner | `control-plane` P051 tests under `./scripts/test-gate.sh p051-scaffold` and `proposal-051` |

This evidence package is sufficient for fixture/readback implementation closeout. Broad `shim_enforced` rollout still requires live dogfood/sign-off in `docs/evidence/051-shared-xcode-mcp-bridge-pool/dogfood-signoff.md`.
