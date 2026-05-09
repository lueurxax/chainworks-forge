# Roadmap

Small planning index for active sequencing. This is not a proposal and does not replace the reference docs.

Detailed context:
- [2026-05-06 main roadmap update](roadmap/2026-05-06-main-roadmap-update.md)
- Current stabilization direction: close the UI/action boundary, protect SQLite from write pressure, make release side effects reconciliation-safe, then restore UI ergonomics over the GraphQL read model.

## Operating mode

- **P073 freeze mode** remains active.
- Do not add new ACP provider families, broad MCP tools, UI write surfaces, context-strategy experiments, agent roles, or speculative runtime features during this stabilization window.
- New implementation proposals must use the executable rollout-gate contract before merge.

## Completed / reference prerequisites

These are not active workstreams:

- **P066** provider toolchain cache mapping. Treat as completed/reference prerequisite; do not reopen it for settlement, retry, SQLite pressure, or control-plane boundary work.
- **P051** shared Xcode MCP bridge pool scoped closeout. Keep in maintenance/release-host validation mode; do not expand scope.

## Now

Highest priority work:

- **P084** minimal rollout-gate template and proposal readiness contract.
- **UI action boundary / P072 closeout gate**:
  - SwiftUI is GraphQL-only.
  - SwiftUI mutations are limited to `approveApproval` and `rejectApproval`.
  - All non-approval operator actions are MCP-only.
- **P081** boundary-first API/auth contract matrix.
- **Local persistence write-budget / evidence-spooling infrastructure** is implemented and remains the persistence safety baseline.
- **P078** durable side-effect ledger and retry blocking.
- **Configurable Agent Escalation Chains**:
  - keep scoped to contract-output repair/fallback;
  - do not include release agents;
  - do not bypass P078 side-effect safety.

## Parallel UI recovery lane

This lane may proceed in parallel with P078 as long as it preserves the implemented write-budget contract and does not add non-approval UI mutations.

- **P085** thin-client read-model parity and affordance contract.
- **P036** visual/navigation restoration over the GraphQL read model.
- **P032** productization, dogfood evidence, accessibility/readiness, and honest operator sign-off.

Goal:

> recover the pre-control-plane level of UI usefulness without reintroducing client-owned orchestration or broad UI write controls.

## Next

After the write-budget and P078 safety rails are in place:

- **P082** recovery/retry state-machine test matrix.
- **P076/P080** effect-aware recovery and stale execution reconciliation.
- **P079** contract-aware output repair and provider fallback.
- **P031** corrected GraphQL thin UI closeout, if not already closed by the UI recovery lane.
- **P046** session read/subscription-only behavior.

## Then

After the safety and UI recovery lanes stabilize:

- **P038** MCP-only run compaction.
  - Must follow the implemented write-budget contract.
  - Preferably follows P078 so compaction preserves side-effect reconciliation evidence.
- **P083** execution-truth ownership invariant model.
- **P070** typed-boundary consolidation.

## Backlog

- Future **P086** agent limit observatory / runtime budget dashboard.
- Future **P087** limit-aware session pool / runtime fallback policy.
- Additional ACP runtime/provider expansion only after the stabilization window.
- Additional UI polish only after P036/P032 restore stable operator ergonomics.

## Current critical path

```text
P073 freeze mode
→ P084 rollout-gate template
→ UI action boundary / P072 closeout
→ P081 boundary matrix
→ implemented SQLite write discipline
→ P078 side-effect ledger
→ P082 recovery/retry matrix
→ P076/P080 effect-aware recovery
→ P079 output repair/fallback
→ P038 compaction
→ P083 ownership invariants
→ P070 consolidation
```

Parallel UI lane:

```text
P081 stable enough
→ P085 read-model/affordance contract
→ P036 visual/navigation restoration
→ P032 dogfood/productization closeout
```
