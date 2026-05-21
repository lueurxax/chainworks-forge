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

## Recently Stabilized

- **Storage tiering/read-path liveness (retained alias: P087)**. Preserves existing GraphQL `StorageHealth.projections` while exposing identity-bearing `ProjectionFreshnessV1` data through additive GraphQL fields such as `StorageHealth.projectionFreshness` and `StorageHealth.projectionFreshnessBySource`. Operational truth lives in [query-projections-and-client-consumption-contract.md](reference/query-projections-and-client-consumption-contract.md) and [rust-control-plane.md](reference/rust-control-plane.md).
- **Retry authority payload target invariants and recovery (retained historical alias: P092)**. Targeted retry payloads keep current target routing separate from source provenance, valid completed retry invokes can be recovered through startup/live reconciliation, and durable `retry_payload_recovery_events` back GraphQL/MCP/report readback. Operational truth lives in [rust-control-plane.md](reference/rust-control-plane.md#retry-payload-target-invariants-and-recovery) and [test-gates.md](reference/test-gates.md#proposal-092p092-retained-historical-alias).
- **P073 freeze mode**.
- **P084** minimal rollout-gate template and proposal readiness contract: template, linter, run-start preflight, authoritative storage, and four-lane operator readback.
- **UI action boundary / P072 closeout gate**:
  - SwiftUI is GraphQL-only.
  - SwiftUI mutations are limited to `approveApproval` and `rejectApproval`.
  - All non-approval operator actions are MCP-only.
- **P081** boundary-first API/auth contract matrix.
- **Local persistence write-budget / evidence-spooling infrastructure** is implemented and remains the persistence safety baseline.
- **Durable side-effect ledger**: release settlement, retry blocking, and reconciliation.
- **Configurable Agent Escalation Chains**:
  - keep scoped to contract-output repair/fallback;
  - do not include release agents;
  - do not bypass durable side-effect safety.

## Parallel UI recovery lane

This lane may proceed in parallel with durable side-effect stabilization as long as it preserves the implemented write-budget contract and does not add non-approval UI mutations.

- **P085** thin-client read-model parity and affordance contract.
- Implemented macOS operator navigation baseline over the GraphQL read model ([reference](reference/macos-operator-navigation.md)).
- **P032** productization, dogfood evidence, accessibility/readiness, and honest operator sign-off.

Goal:

> recover the pre-control-plane level of UI usefulness without reintroducing client-owned orchestration or broad UI write controls.

## Now

After the write-budget and durable side-effect safety rails are in place:

- **P081** boundary-first API/auth contract matrix.
- **P082** recovery/retry state-machine test matrix.
- **P076/P080** effect-aware recovery and stale execution reconciliation.
- **P079** contract-aware output repair and provider fallback.
- **P031** corrected GraphQL thin UI closeout, if not already closed by the UI recovery lane.
- **P046** session read/subscription-only behavior.

## Next

After the safety and UI recovery lanes stabilize:

- **P038** MCP-only run compaction.
  - Must follow the implemented write-budget contract.
  - Preferably follows the durable side-effect ledger so compaction preserves side-effect reconciliation evidence.
- **P083** execution-truth ownership invariant model.
- **P070** typed-boundary consolidation.

## Backlog

- Future **P086** agent limit observatory / runtime budget dashboard.

- Additional ACP runtime/provider expansion only after the stabilization window.
- Additional UI polish builds on the implemented macOS operator navigation baseline and the P032 productization lane.

## Current critical path

```text
P073 freeze mode
→ P084 rollout-gate template
→ UI action boundary / P072 closeout
→ P081 boundary matrix
→ implemented SQLite write discipline
→ durable side-effect ledger
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
→ implemented macOS operator navigation baseline
→ P032 dogfood/productization closeout
```
