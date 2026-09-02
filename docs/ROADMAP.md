# Roadmap

Small planning index for active sequencing. This is not a proposal and does not replace the reference docs.

Detailed context:
- [2026-05-06 main roadmap update](roadmap/2026-05-06-main-roadmap-update.md)
- [Agent mission context and skills hardening program](roadmap/2026-08-30-agent-mission-context-and-skills-hardening-program.md)
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
- **P082** recovery/retry state-machine proof matrix.
  - The shared cancellation, startup repair, retry-authority, ownership, side-effect hold, and late-output quarantine proofs are reference-owned in [recovery-retry-state-machine-test-matrix.md](reference/recovery-retry-state-machine-test-matrix.md).
- **P080** stale-execution reconciliation, current phase-scoped baseline.
  - Detection/readback plus promoted repair for `acp_startup_stale` and `scheduler_ownership_drift` are implemented.
  - Helper reap, side-effect-adjacent repair, manual hold, and permanent-hold clear remain fail-closed future slices; they are not unfinished behavior in the current baseline.
- **P083** execution-truth ownership invariants.
  - Durable run/stage/agent/approval/artifact/side-effect ownership and recovery truth lives in [execution-truth-and-recovery.md](reference/execution-truth-and-recovery.md).
- **P079** contract-aware output repair and provider fallback.
  - Implemented for the current safe repair/recovery scope. Wired pieces include the SQLite migration, domain types, repair-event/lease repos, GraphQL/MCP/run-report readback, Swift DTO/presenter support, read-only macOS inspector surfacing, MCP runtime receipt sanitization, crash-consistent materialization, Junie plan-evidence capture/redaction, bounded transcript/provider-envelope recovery, deterministic-fixture same-session repair, and the retained `proposal-079`/`p079` gate aliases.
  - Controlled provider fallback dispatch, projection artifact rebuild/recovery sweep, independent plan-evidence purge, and production same-session repair for advisory-only providers remain future work.
  - Operational truth lives in [output-contracts-failure-evidence-and-recovery.md](reference/output-contracts-failure-evidence-and-recovery.md#p079-output-contract-repair-and-fallback-details).
  - Keep scoped to contract-output repair/fallback.
  - Do not include release agents or bypass durable side-effect safety.
- **Retained historical alias P086** agent work continuation and lead-directed same-session resumption.
  - Implemented baseline: MCP continuation admission/readback, GraphQL/macOS read-only surfaces, lead-directed continuation, provider-session resurrection durable state/readback, guarded adapter capability boundaries, and retained `proposal-086`/`p086` gate aliases.
  - Operational truth lives in [agent-work-continuation.md](reference/agent-work-continuation.md).
  - Post-implementation expansion and soak evidence lives in P093.
- **P036/P085** macOS operator navigation and thin-client affordance baseline.
  - Treat as implemented UI/read-model baseline unless a new delta proposal explicitly says otherwise.
- **P088** code-writer completion freshness and repair diagnostics.
  - Current-attempt worktree evidence, bounded same-session completion publication, canonical receipt recovery, and `implementationCompletion` readback are reference-owned.
  - Operational truth lives in [output-contracts-failure-evidence-and-recovery.md](reference/output-contracts-failure-evidence-and-recovery.md#code-writer-completion-freshness-and-repair-p088-retained-alias); `proposal-088`/`p088` are retained proof aliases only.
- **P094** workflow-owned quality-gate blocker boundaries.
  - Canonical routing and readback live in [workflow-execution-engine.md](reference/workflow-execution-engine.md#quality-gate-blocker-boundary-transitions) and [output-contracts-failure-evidence-and-recovery.md](reference/output-contracts-failure-evidence-and-recovery.md#workflow-owned-quality-gate-boundary-contracts).
- **P096** bounded tool output and safe search.
  - Runtime preflight, provider wrappers, poisoned-session handling, and health readback are implemented in [bounded-tool-output-and-safe-search-policy.md](reference/bounded-tool-output-and-safe-search-policy.md).
- **Default-on mission context and frozen Agent Skills baseline**.
  - Every new Rust-owned invocation receives bounded compiler-owned mission context, and persisted copy/retry paths validate exact task, owner, P017, and P058 authority before mutation.
  - Proposal review, implementation audit, code implementation, security review, and pre-push review use strict frozen external bundles.
  - Stable truth lives in [skill-resolution-and-runtime-integration.md](reference/skill-resolution-and-runtime-integration.md); the retained provider-free gate is `agent-context-skills`.
  - Remaining procedure migration, authority overlays, resource brokerage, and eval infrastructure are new bounded work, not incomplete baseline acceptance.

## Now

Keep this queue small and execute it in order:

- **Maintain a green default branch and explicit release health.**
  - Keep the local default build/gate status green.
  - If external CI or App Store Connect is red and logs are unavailable, record it as unresolved external release health rather than treating release status as green.
- **P070-A** narrow typed invocation/settlement seam.
  - Extract only the typed boundary needed by P095 while preserving external behavior.
  - Do not implement the full P070 consolidation and do not add more P079/P086/P088/P095 branching to central `executor.rs` or `orchestrator.rs`.
- **P095** two-phase agent invocation and deferred output settlement.
  - This remains proposal-level design and starts only after, or directly on, P070-A.
  - First implementation scope is `code_writer` only: work turn, deterministic server readback, output collection, then settlement.
  - Output collection is neither an ordinary retry nor P079 repair. No release, publish, git-push, upload, new SwiftUI mutation, or loosened output contract is in scope.
- **P032** natural dogfood, productization, and operator sign-off.
  - Observe ordinary runs after P095 rather than spending provider budget on a special validation run.
  - Use those runs to prioritize bounded usability and reliability fixes over new surfaces.

## Next

After the current typed-seam and code-writer slice stabilizes:

- **Remaining P070** typed-boundary consolidation phases.
  - Continue only after P070-A and P095 prove the seam in natural runs.
  - Do not use P070 to add product features.
- **Bounded low-authority Agent Skills migration.**
  - Take one small slice from the [program note](roadmap/2026-08-30-agent-mission-context-and-skills-hardening-program.md) behind a fresh reviewed proposal.
  - Keep deterministic evals in PR gates and repeated live-provider evals in Steward/nightly operation; high-authority bundles require separate proposals.
- **P093** [agent work continuation expansion and soak](proposals/093-agent-work-continuation-expansion-soak.md).
  - This number belongs to continuation scale/soak work, not timeline UX.
- **P038** MCP-only run compaction, only if natural-run artifact noise remains high.
  - Preserve storage/write-budget and side-effect evidence; GraphQL remains readback-only.
- **P103** [live-agent timeline UX and readability](proposals/103-live-agent-timeline-ux-and-readability.md), only if dogfood still shows poor active-agent visibility.
- **P100** [P058 release evidence and macOS runtime proof](proposals/100-p058-release-evidence-and-macos-runtime-proof.md), only when release-host evidence is actually needed.

## Parked / Conditional

Do not start these without explicit authorization and a new bounded review:

- **P105** deterministic GitHub pull-request publication; this is high-authority external side-effect work and waits for P070/P095 plus release-safety review.
- destructive temporary-artifact cleanup;
- mutable authority overlays;
- skill resource/script brokerage;
- live-provider eval promotion UI;
- additional ACP providers;
- new broad UI surfaces.

Existing conditional baseline:

- **P089** managed temporary artifact inventory (read-only smoke slice implemented).
  - Advisory read-only, dry-run-only managed temporary artifact inventory slice, `disabled` by default.
  - Keeps active worktrees and failure evidence safe by design with no cleanup mutations.
  - MCP, GraphQL, run-report, release-receipt, and packaged Swift lanes all share one scan path; `operator_visible` promotion is still held on redaction-key initialization reconciliation, contract-fixture reconciliation, and packaged remote UI/accessibility evidence. Deletion and cleanup remain future work - see [reference/managed-temporary-artifact-inventory.md](reference/managed-temporary-artifact-inventory.md#11-implementation-status-by-lane-current-slice).

## Backlog

- **Deferred provider truth, containment, and runtime-readback program**.
  - These eight source inventories were decomposed from the planned
    model-variant/UI-label slice. They are roadmap inputs only: every item
    remains `not implementation-approved` until it has a fresh bounded
    proposal, review, focused gate, implementation, and closeout cycle.
  - Entry prerequisite: preserve the current planned-label behavior and wait
    for the relevant P083 ownership and P070/P081 typed-boundary contracts to
    stabilize. This program does not block or enlarge the planned-label slice.
  - Foundation lane 1: [Provider configuration migration and
    reconciliation](superpowers/specs/2026-08-31-provider-configuration-migration-and-reconciliation-design.md)
    owns durable storage, bootstrap, and restart reconciliation before
    accepted provider truth can become authoritative.
  - Foundation lane 2: [P031 bounded runtime
    readback](superpowers/specs/2026-08-31-p031-bounded-runtime-readback-design.md)
    owns bounded protocol operations, paging, error vocabulary, and Swift
    reduction contracts. It may be refined in parallel with lane 1.
  - Foundation lane 3: [Provider egress and diagnostics
    containment](superpowers/specs/2026-08-31-provider-egress-and-diagnostics-containment-design.md)
    owns the network and diagnostic security boundary required before broader
    resurrection work.
  - Independent recovery lane: [P079 repair output materialization and
    recovery](superpowers/specs/2026-08-31-p079-repair-output-materialization-design.md)
    may proceed under its own bounded proposal and does not gate planned-label
    closeout.
  - After foundation lane 1: [Provider accepted truth and prompt
    authority](superpowers/specs/2026-08-31-provider-accepted-truth-and-prompt-authority-design.md)
    may define authoritative accepted model/effort, occurrence ownership,
    prompt permits, terminal receipts, and exact failure recovery.
  - After provider accepted truth plus foundation lane 3: [P086 resurrection
    containment](superpowers/specs/2026-08-31-p086-resurrection-containment-design.md)
    may add provider attach/resurrection containment and output-only recovery.
  - After foundation lane 2: [Frozen run replacement and input
    repair](superpowers/specs/2026-08-31-frozen-run-replacement-and-input-repair-design.md)
    may add the operator-only replacement API and repair workspace without
    mutating the original snapshot.
  - Final presentation lane: [Verified provider truth
    UI](superpowers/specs/2026-08-31-verified-provider-truth-ui-design.md)
    starts only after accepted provider truth, bounded P031 readback, and
    stable P083-compatible occurrence/event identity exist.
- Future limit observatory / runtime budget dashboard - number TBD.
  - Do not reuse existing proposal numbers.
- Future limit-aware session pool / runtime fallback policy - number TBD.
- Additional ACP runtime/provider expansion only after the stabilization window.
- Additional UI polish only after P032/P036/P093 remain stable under dogfood.
- Additional storage migration work only if storage/read-path exit criteria fail under real runs.
- Deferred Agent Skills and eval hardening only through the bounded [program sequence](roadmap/2026-08-30-agent-mission-context-and-skills-hardening-program.md); the preserved full design is source inventory, not one implementation proposal.

## Current critical path

```text
P073 freeze mode
-> restore local build/fast gate green (completed 2026-09-02; external CI pending publication)
-> P088 current-head closeout (completed 2026-09-02; reference-owned)
-> P070-A typed invocation/settlement seam
-> P095 code_writer two-phase invocation
-> observe natural runs
-> remaining P070 consolidation
-> bounded low-authority skill migration
```

Parallel throughput lane:

```text
implemented continuation baseline
-> P095 readback/output collection after continuation work turns
-> P093 post-implementation soak/scale evidence
-> guarded by durable side-effect safety
-> read back through GraphQL only
-> no release/publish/git-push/upload stages
```

Conditional operator/product lanes:

```text
P089 managed temp artifact inventory read-only smoke slice; lifecycle deletion remains parked
P103 live timeline readability if active-agent visibility remains poor
P038 compaction if natural-run artifact noise remains high
P100 P058 release evidence when release-host proof is needed
P032 dogfood/productization runs in the main lane
```
