# Roadmap

Small planning index for active sequencing. This is not a proposal and does not replace the reference docs.

Detailed context:
- [2026-05-06 main roadmap update](roadmap/2026-05-06-main-roadmap-update.md) (historical snapshot; see its status note)
- Current stabilization direction: keep the UI boundary closed, preserve local storage liveness, prove retry/recovery behavior, and improve operator usability without reopening client-owned orchestration.

## Operating mode

- **P073 freeze mode** remains active.
- Do not add new ACP provider families, broad MCP tools, UI write surfaces, context-strategy experiments, agent roles, or speculative runtime features during this stabilization window.
- New implementation proposals must use the executable rollout-gate / observability contract before closeout.
- Prefer current reference docs over old proposal lineage once proposal truth has been promoted.
- Proposal numbers are never reused. Implemented/Ready proposals are retired into `docs/reference/` and the proposal file is deleted.

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
  - Implemented/Ready (audit R8); proposal retired.
  - Stable truth lives in [boundary-first-api-auth-contract.md](reference/boundary-first-api-auth-contract.md) and [swift-macos-boundary-contract.md](reference/swift-macos-boundary-contract.md).
  - Remaining work is boundary verification, not feature expansion.
- **P046** session observability via GraphQL.
  - Stable truth lives in [rust-control-plane.md](reference/rust-control-plane.md#graphql) and [test-gates.md](reference/test-gates.md#session-observability-graphql).
  - The GraphQL schema includes the P046 read/subscription surface by default; disabled-schema mode is retained only for rollback and compatibility proof.
- **P087** local storage tiering, read-path liveness, and SQLite exit criteria.
  - Operational truth lives in [query-projections-and-client-consumption-contract.md](reference/query-projections-and-client-consumption-contract.md) and [rust-control-plane.md](reference/rust-control-plane.md).
  - SQLite remains compact canonical state; high-volume evidence is file-spooled.
- **Durable side-effect ledger / P078**.
  - Treated as implemented baseline in [rust-control-plane.md](reference/rust-control-plane.md).
  - Release settlement, retry blocking, and reconciliation remain outside normal retry/continuation.
- **Configurable agent escalation chains (P058)**.
  - Implemented baseline: `escalation_policy_v1` schema, durable ledger/metadata/events tables, GraphQL `runEscalationReadback`, MCP `runs.get` parity, redaction tiers, safe kill-switch defaults, scheduler-owned tier advancement, and governed macOS read surfaces.
  - Operational truth lives in [escalation-policies.md](reference/escalation-policies.md).
  - Scheduler behavior remains scoped to contract-output repair/fallback and must not include release agents or bypass durable side-effect safety.
- **P076** auto-retry observation ledger.
  - Implemented/Ready with Risks (audit R1) as an observe-only contract.
  - Operational truth lives in [auto-retry-observation-ledger.md](reference/auto-retry-observation-ledger.md).
  - Active repair/retry dispatch is NOT owned here; it belongs to P080 slices behind durable side-effect safety.
- **P092** retry authority payload target invariants and recovery.
  - Operational truth lives in [rust-control-plane.md](reference/rust-control-plane.md#retry-payload-target-invariants-and-recovery) and [test-gates.md](reference/test-gates.md#proposal-092p092-retained-historical-alias).
- **P096** bounded tool output and safe search policy.
  - Implemented; proposal retired.
  - Operational truth lives in [bounded-tool-output-and-safe-search-policy.md](reference/bounded-tool-output-and-safe-search-policy.md); the `proposal-096|p096` gate alias is retained.
- **P094** workflow-owned quality-gate blocker boundaries.
  - Implemented; proposal retired.
  - Operational truth lives in [workflow-execution-engine.md](reference/workflow-execution-engine.md#quality-gate-blocker-boundary-transitions) and [output-contracts-failure-evidence-and-recovery.md](reference/output-contracts-failure-evidence-and-recovery.md#workflow-owned-quality-gate-boundary-contracts); the `proposal-094|p094` gate alias is retained.
- **P036/P085** macOS operator navigation and thin-client affordance baseline.
  - Treat as implemented UI/read-model baseline unless a new delta proposal explicitly says otherwise.

## Closeout pending

Implemented per audit; only closeout into reference truth remains. Closeout is a short gate-and-promote task, not feature work:

- **P086** agent work continuation and lead-directed same-session resumption.
  - Audit R6: Implemented / Ready with Risks.
  - Closeout: accept audit, promote to reference truth, fix or make explicit the metric summary cap before P093 soak evidence.
  - Unlocks the P093 expansion soak.
- **P088** code-writer completion contract and output freshness.
  - Audit R7: Implemented (22/22) / Ready with Risks.
  - Closeout: run `./scripts/test-gate.sh full` + remote ui-smoke, commit the implementation worktree, promote status from Draft, synchronize `completion_turn_result` into durable API/reference truth.

## Now

Active stabilization and correctness work, in order:

- **P082** recovery/retry state-machine test matrix.
  - Status: approved for implementation review but NOT yet implemented, while downstream recovery work (P076/P086/P088) already landed.
  - Reframed as a BACKFILL proof harness first: encode the already-landed retry/recovery behavior as matrix rows, then require every new recovery/retry change to add or satisfy a row.
- **P080** continuous stale execution reconciliation, implemented through its decomposition slices:
  - **P098** manual operator hold and clear-hold semantics.
  - **P099** read-only diagnostics window.
  - Use P082 as the proof harness.
  - Release/publish/git side-effect lanes remain fail-closed and route through durable side-effect reconciliation.
- **P079** contract-aware output repair and provider fallback.
  - Keep scoped to contract-output repair/fallback.
  - Do not include release agents.
  - Do not bypass durable side-effect safety.
- **P083** execution-truth ownership invariant model.
  - Move earlier than broad typed-boundary refactor.
  - Name authoritative records for run/stage/agent/approval/artifact/side-effect truth.
- **P095** two-phase agent invocation and deferred output settlement.
  - Normalizes `code_writer` execution as short work turn, server-owned readback, separate output collection, then settlement.
  - Reduces fresh retries and stale-output hazards without weakening P079/P088 contracts.
  - Keep safety in runtime permissions, path guards, and durable side-effect policy, not long prompt warnings.

## Next

After the current recovery/output/ownership block stabilizes:

- **P070** typed-boundary consolidation.
  - Refactor only after P082/P083/P088 stop moving the core contracts.
  - Includes residual legacy Swift engine cleanup (dead DSL/Engine code, orphaned fixtures, stale references) after the engine removal lands — see P070 G-9.
  - Do not use P070 to add product features.
- **P097** governed frozen snapshot retrofit.
  - Operator-only MCP retrofit of frozen snapshots, `escalation_policy_only` scope first.
  - Incident-driven; keep the governed/emergency slice narrow.
- **P089** managed temporary artifact lifecycle.
  - Use if unmanaged temp roots / DerivedData / provider runtime homes continue to create disk pressure.
  - Keep active worktrees and failure evidence safe.
- **P093** agent work continuation expansion soak.
  - Post-P086-closeout scale/soak evidence for continuation-capable lanes.
  - Requires the P086 metric summary cap fix noted in audit R6.
- **P100** P058 escalation release evidence and macOS runtime proof.
  - Renumbered from a colliding "096" file; the canonical P096 is bounded tool output.
  - Produce remote UI/accessibility/contrast/reduced-motion, multi-window/scene restoration, long-run metric trend, and operational drill artifacts for escalation broad-release decisions.
  - Do not reopen implemented escalation behavior; this is release proof only.
- **P038** MCP-only run compaction, if artifact noise remains high.
  - Must respect implemented storage/write-budget and side-effect evidence preservation.
  - GraphQL is readback only; compaction is MCP-only.
- **P032** productization, dogfood evidence, accessibility/readiness, and honest operator sign-off over the implemented UI baseline.

## Backlog

- **P101** agent limit observatory and runtime budget dashboard ([draft](proposals/101-agent-limit-observatory-and-runtime-budget-dashboard.md), parked until the freeze lifts).
- **P102** limit-aware session pool and runtime fallback policy ([draft](proposals/102-limit-aware-session-pool-and-runtime-fallback-policy.md), parked; depends on P101).
- Backup/restore/export of `.chainworks` state (SQLite + artifacts disaster recovery) — number TBD.
- App↔daemon version handshake and upgrade compatibility — number TBD.
- Additional ACP runtime/provider expansion only after the stabilization window.
- Additional UI polish only after P032/P036/P103 remain stable under dogfood.
- Additional storage migration work only if storage/read-path exit criteria fail under real runs.

## Parked (P073 freeze triage)

Explicitly parked, not scheduled. Each item leaves this list only by an explicit roadmap edit that retires, merges, or reactivates it — no silent reactivation:

- **P1000** Go/Temporal control-plane extraction — strategic architecture document; not active during the freeze; revisit only after stabilization closes.
- Old draft proposals parked during the freeze: P020, P021, P023, P028, P034, P037, P039, P044, P045, P047, P048, P049, P052, P055, P056, P059, P062, P063, P064, P065, P067, P069, P071, P074.
  - P037 (ACP execution supervision / idle watchdog) has two Partial audit rounds and is the first triage candidate when the freeze lifts.

## Current critical path

```text
P073 freeze mode
→ P086/P088 closeouts (promote implemented work to reference truth)
→ P082 recovery/retry matrix (backfill proof over landed work, then gate new work)
→ P080 slices (P098 hold semantics, P099 diagnostics window)
→ P079 output repair/fallback
→ P083 ownership invariants
→ P095 work/output turn separation
→ P070 typed-boundary consolidation (incl. legacy Swift engine residue cleanup)
```

Parallel throughput lane:

```text
P086 same-session work continuation (closeout)
→ P093 expansion soak
→ P095 readback/output collection after continuation work turns
→ guarded by durable side-effect safety
→ read back through GraphQL only
→ no release/publish/git-push/upload stages
```

Conditional operator/product lanes:

```text
P089 temp artifact lifecycle if disk pressure continues
P103 live timeline readability if active-agent visibility remains poor
P100 escalation release evidence before broad escalation default-enable
P038 compaction if run artifact noise remains high
P032 productization/dogfood closeout when UI baseline is stable
```
