# Roadmap

Small planning index for active sequencing. This is not a proposal and does not replace the reference docs.

Detailed context:
- [2026-05-06 main roadmap update](roadmap/2026-05-06-main-roadmap-update.md)
- Current stabilization direction: keep the UI boundary closed, preserve local storage liveness, prove retry/recovery behavior, and improve operator usability without reopening client-owned orchestration.

## Operating mode

- **P073 freeze mode** remains active.
- Do not add new ACP provider families, broad MCP tools, UI write surfaces, context-strategy experiments, agent roles, or speculative runtime features during this stabilization window.
- New implementation proposals must use the executable rollout-gate / observability contract before closeout.
- Prefer current reference docs over old proposal lineage once proposal truth has been promoted.

## Reference-owned / completed prerequisites

These are not active workstreams:

- **P051** shared Xcode MCP bridge pool scoped closeout.
  - Keep in maintenance / release-host validation mode.
  - Do not expand scope.
- **P066** provider toolchain cache mapping.
  - Treat as completed / reference prerequisite.
  - Do not reopen it for settlement, retry, SQLite pressure, or control-plane boundary work.
- **P073** freeze mode.
  - Operating mode, not a product feature.
- **P084** executable rollout-gate template and proposal readiness contract.
  - Treat as gate infrastructure.
- **UI action boundary / P072**.
  - Stable truth lives in [ui-action-boundary.md](reference/ui-action-boundary.md).
  - SwiftUI is GraphQL-only.
  - SwiftUI mutations are limited to `approveApproval` and `rejectApproval`.
  - All non-approval operator actions are MCP-only.
- **P081** boundary-first API/auth contract matrix.
  - Treat as reference/gate infrastructure if implementation proof is complete.
  - Remaining work should be boundary verification, not feature expansion.
- **P046** session observability via GraphQL.
  - Stable truth lives in [rust-control-plane.md](reference/rust-control-plane.md#graphql) and [test-gates.md](reference/test-gates.md#session-observability-graphql).
  - The GraphQL schema includes the P046 read/subscription surface by default; disabled-schema mode is retained only for rollback and compatibility proof.
- **Local persistence write-budget / evidence-spooling infrastructure**.
  - Implemented baseline in [rust-control-plane.md](reference/rust-control-plane.md).
  - SQLite remains compact canonical state; high-volume evidence is file-spooled.
- **Storage tiering / read-path liveness**.
  - Retained alias: **P087**.
  - Operational truth lives in [query-projections-and-client-consumption-contract.md](reference/query-projections-and-client-consumption-contract.md) and [rust-control-plane.md](reference/rust-control-plane.md).
  - No active P087 proposal file is expected.
- **Durable side-effect ledger / P078**.
  - Treated as implemented baseline in [rust-control-plane.md](reference/rust-control-plane.md).
  - Remaining issues should flow through recovery/test/ownership follow-ups, not "implement P078".
- **Retry authority payload target invariants and recovery**.
  - Retained historical alias: **P092**.
  - Operational truth lives in [rust-control-plane.md](reference/rust-control-plane.md#retry-payload-target-invariants-and-recovery) and [test-gates.md](reference/test-gates.md#proposal-092p092-retained-historical-alias).
- **macOS operator navigation / P036** and **thin-client affordance contract / P085**.
  - Treat as implemented UI/read-model baseline unless a new delta proposal explicitly says otherwise.

## Now

Active stabilization and correctness work:

- **P082** recovery/retry state-machine test matrix.
  - Turn recurring retry/recovery fixes into a shared proof suite.
  - Every recovery/retry change should add or satisfy a matrix row.
- **P076/P080** effect-aware recovery and stale execution reconciliation.
  - Use P082 as the proof harness.
  - Release/publish/git side-effect lanes remain fail-closed and route through durable side-effect reconciliation.
- **P079** contract-aware output repair and provider fallback.
  - Keep scoped to contract-output repair/fallback.
  - Do not include release agents.
  - Do not bypass durable side-effect safety.
- **P083** execution-truth ownership invariant model.
  - Move earlier than broad typed-boundary refactor.
  - Name authoritative records for run/stage/agent/approval/artifact/side-effect truth.
- **P088** code-writer completion contract, output freshness, and repair diagnostics.
  - Treat as active because code-writer handoff failures are now a major throughput and correctness problem.
  - Keep strict output contracts; do not accept stale files as fresh truth.
- **P095** two-phase agent invocation and deferred output settlement.
  - Normalizes `code_writer` execution as short work turn, server-owned readback, separate output collection, then settlement.
  - Reduces fresh retries and stale-output hazards without weakening P079/P088 contracts.
  - Keep safety in runtime permissions, path guards, and durable side-effect policy, not long prompt warnings.
- **P086** agent work continuation and lead-directed same-session resumption.
  - Implement only within continuation-capable non-release agent lanes.
  - Keep GraphQL read-only for continuation readback.
  - Do not use it to bypass P088 output freshness or side-effect safety.

## Next

After the current recovery/output/ownership block stabilizes:

- **P070** typed-boundary consolidation.
  - Refactor only after P082/P083/P088 stop moving the core contracts.
  - Do not use P070 to add product features.
- **P089** managed temporary artifact lifecycle.
  - Use if unmanaged temp roots / DerivedData / provider runtime homes continue to create disk pressure.
  - Keep active worktrees and failure evidence safe.
- **P093** live agent timeline UX and readability.
  - Improve active-agent timeline readability over existing control-plane readback.
  - Do not recreate Swift-local orchestration state.
- **P038** MCP-only run compaction, if artifact noise remains high.
  - Must respect implemented storage/write-budget and side-effect evidence preservation.
  - GraphQL is readback only; compaction is MCP-only.
- **P032** productization, dogfood evidence, accessibility/readiness, and honest operator sign-off over the implemented UI baseline.

## Backlog

- Future limit observatory / runtime budget dashboard - number TBD.
  - Do not use P086, P087, P088, P089, P092, or P093.
- Future limit-aware session pool / runtime fallback policy - number TBD.
- Additional ACP runtime/provider expansion only after the stabilization window.
- Additional UI polish only after P032/P036/P093 remain stable under dogfood.
- Additional storage migration work only if storage/read-path exit criteria fail under real runs.

## Current critical path

```text
P073 freeze mode
→ P082 recovery/retry matrix
→ P076/P080 effect-aware stale/retry reconciliation
→ P079 output repair/fallback
→ P083 ownership invariants
→ P088 code-writer completion/freshness
→ P095 work/output turn separation
→ P070 typed-boundary consolidation
```

Parallel throughput lane:

```text
P086 same-session work continuation
→ P095 readback/output collection after continuation work turns
→ guarded by durable side-effect safety
→ read back through GraphQL only
→ no release/publish/git-push/upload stages
```

Conditional operator/product lanes:

```text
P089 temp artifact lifecycle if disk pressure continues
P093 live timeline readability if active-agent visibility remains poor
P038 compaction if run artifact noise remains high
P032 productization/dogfood closeout when UI baseline is stable
```
