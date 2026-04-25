# P051 Dependency Audit

Date: 2026-04-25

Purpose: record the upstream readiness posture required by P051 before treating the shared Xcode MCP bridge pool as schedulable/closeable. This audit is based on current reference docs and registered gates; it does not rerun the dependency gates.

## Summary

P051 scaffold can proceed on the current documentation/gate posture. P026 and P029 have registered gates and stable reference coverage sufficient for P051 fixture work. No dependency row below is a current scheduling blocker for fixture-level P051 closeout.

Pre-ship P051 remains blocked by live dogfood/sign-off, tracked separately in [dogfood-signoff.md](dogfood-signoff.md).

## Dependency Matrix

| Proposal | Owner Surface | Current Gate Status | Remaining Gaps For P051 | Parallel vs Sequential | Blocking Threshold |
|---|---|---|---|---|---|
| P025 | Per-agent MCP policy and runtime validation | `proposal-025|p025` is registered in `scripts/test-gate.sh`; stable reference exists in `docs/reference/per-agent-mcp-policy-and-runtime-validation.md`. | No P051-specific blocker found. P051 extends requested/predicted/actual/denied MCP truth with brokered Xcode observations. | Parallel: P051 can build on the existing MCP truth chain. | Block only if requested/predicted/actual/denied execution truth or MCP report readback is removed or its gate becomes unavailable. |
| P026 | ACP runtime transport | `proposal-026|p026` is registered in `scripts/test-gate.sh`; stable reference exists in `docs/reference/acp-runtime-transport.md`. | No scaffold blocker found. P051 relies on ACP runtime profiles and adapter capability boundaries, then adds HTTP MCP capability proof. | Sequential prerequisite satisfied for scaffold; future adapter refactors must preserve P026 runtime selection truth. | Hard blocker if ACP runtime profile binding, adapter-family selection, or launch/readback truth is incomplete or unregistered, because P051 cannot safely attach broker leases before provider startup without it. |
| P029 | Second-wave ACP runtime profiles and northbound MCP | `proposal-029|p029` and `proposal-029-mcp|p029-mcp` are registered; P029-MCP evidence exists at `docs/evidence/029-mcp-northbound-control-plane-server/README.md`; northbound reference exists in `docs/reference/mcp-northbound-control-plane-server.md`. | No scaffold blocker found. P051 must keep capability filtering and MCP report/resource readback compatible with P029 northbound ownership. | Sequential prerequisite satisfied for scaffold; P051 can proceed in parallel with later northbound tool expansion as long as it does not bypass P029 capability policy. | Hard blocker if MCP bearer auth, capability filtering, command journal/readback, or `reports.get` ownership is absent, because P051 observations and broker policy need that northbound contract. |
| P037 | ACP execution supervision and idle watchdog | `proposal-037|p037` is registered and documented in `docs/reference/test-gates.md`; R2 audit records same-tree focused gate evidence but notes full regression was not available in that audit context. | No P051 scaffold blocker found. Live dogfood should still watch for watchdog/session-retry interactions with broker leases. | Parallel: P051 fixture work can proceed while live dogfood validates runtime interaction. | Block P051 release if watchdog retry/session invalidation regresses broker lease cleanup, stale session rejection, or durable failure evidence. |
| P049 | Steward analysis system | `proposal-049|p049` is registered and documented; stable reference exists in `docs/reference/steward-analysis-system.md`. | No P051 scaffold blocker found. P051 does not require a Steward dashboard or dedicated P049 UI gate. | Parallel: Steward analysis can consume P051 run truth later without blocking broker scaffold/readback. | Block only if P051 changes frozen catalog/run-start snapshot fields in a way that breaks P049 cohort/snapshot hashing or active-catalog Steward IO. |

## Gate Posture Notes

- P026 and P029 are the only P051 hard scheduling prerequisites named by the proposal. Current docs/gates show both are implemented enough for scaffold closeout.
- P025, P037, and P049 remain compatibility dependencies, not current sequential blockers.
- This artifact is not dogfood proof and does not claim operator sign-off.
