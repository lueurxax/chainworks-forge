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
- **P087** local storage tiering, read-path liveness, and SQLite exit criteria.
  - Operational truth lives in [query-projections-and-client-consumption-contract.md](reference/query-projections-and-client-consumption-contract.md) and [rust-control-plane.md](reference/rust-control-plane.md).
  - SQLite remains compact canonical state; high-volume evidence is file-spooled.
- **Durable side-effect ledger / P078**.
  - Treated as implemented baseline in [rust-control-plane.md](reference/rust-control-plane.md).
  - Release settlement, retry blocking, and reconciliation remain outside normal retry/continuation.
- **P058** configurable agent escalation chains.
  - Implemented baseline: `escalation_policy_v1` schema, durable ledger/metadata/events tables, GraphQL `runEscalationReadback`, MCP `runs.get` parity, redaction tiers, safe kill-switch defaults, scheduler-owned tier advancement, and governed macOS read surfaces.
  - Operational truth lives in [escalation-policies.md](reference/escalation-policies.md).
  - Scheduler behavior remains scoped to contract-output repair/fallback and must not include release agents or bypass durable side-effect safety.
- **P092** retry authority payload target invariants and recovery.
  - Operational truth lives in [rust-control-plane.md](reference/rust-control-plane.md#retry-payload-target-invariants-and-recovery) and [test-gates.md](reference/test-gates.md#proposal-092p092-retained-historical-alias).
- **P079** contract-aware output repair and provider fallback.
  - Partially implemented. Wired pieces include the SQLite migration, domain types, repair-event/lease repos, GraphQL/MCP readback, Swift DTOs, MCP runtime receipt sanitization, crash-consistent materialization, Junie plan-evidence capture/redaction, bounded transcript-recovery evidence that fails closed without transport attribution, deterministic-fixture same-session repair, and the current P079 security/settlement hardening.
  - Accepted transcript/provider-envelope recovery, controlled provider fallback dispatch, full projection rebuild + recovery sweep, Swift macOS inspector UI, P079 operational metric emission, and the full `proposal-079`/`p079` acceptance gate remain deferred.
  - Operational truth lives in [output-contracts-failure-evidence-and-recovery.md](reference/output-contracts-failure-evidence-and-recovery.md#p079-output-contract-repair-and-fallback-details).
  - Keep scoped to contract-output repair/fallback.
  - Do not include release agents or bypass durable side-effect safety.
- **P036/P085** macOS operator navigation and thin-client affordance baseline.
  - Treat as implemented UI/read-model baseline unless a new delta proposal explicitly says otherwise.

## Now

Active stabilization and correctness work:

- **P082** recovery/retry state-machine test matrix.
  - Turn recurring retry/recovery fixes into a shared proof suite.
  - Every recovery/retry change should add or satisfy a matrix row.
- **P076/P080** effect-aware recovery and stale execution reconciliation.
  - Use P082 as the proof harness.
  - Release/publish/git side-effect lanes remain fail-closed and route through durable side-effect reconciliation.

  - P080 implementation refined and underway; active repair remains rollout-gated.
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
- **P096** P058 release evidence and macOS runtime proof.
  - Produce remote UI/accessibility/contrast/reduced-motion, multi-window/scene restoration, long-run metric trend, and operational drill artifacts for P058 broad-release decisions.
  - Do not reopen P058 implementation behavior; this is release proof only.
- **P038** MCP-only run compaction, if artifact noise remains high.
  - Must respect implemented storage/write-budget and side-effect evidence preservation.
  - GraphQL is readback only; compaction is MCP-only.
- **P032** productization, dogfood evidence, accessibility/readiness, and honest operator sign-off over the implemented UI baseline.

## Backlog

- Future limit observatory / runtime budget dashboard - number TBD.
  - Do not reuse existing proposal numbers.
- Future limit-aware session pool / runtime fallback policy - number TBD.
- Additional ACP runtime/provider expansion only after the stabilization window.
- Additional UI polish only after P032/P036/P093 remain stable under dogfood.
- Additional storage migration work only if storage/read-path exit criteria fail under real runs.

## Current critical path

```text
P073 freeze mode
→ P082 recovery/retry matrix
→ P076/P080 effect-aware stale/retry reconciliation

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
