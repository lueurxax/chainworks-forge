# P061 Implementation Audit R2: SQLite write serialization and executor backpressure

- Proposal: `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md`
- Report: `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure_IMPLEMENTATION_AUDIT_R2.md`
- Date: 2026-04-23
- Audit mode: `proposal-implementation-audit`, auto mode
- Repository: `/Users/user/Documents/Chainworks Forge`
- Base commit observed: `7912de0db5e68c5ed52ee5cb340c015bda23fa41`
- Worktree state: dirty; this audit intentionally includes current uncommitted P061 implementation changes.
- Validation commands run by this audit: none. The audit did not run `./scripts/test-gate.sh proposal-061`.

## Verdict

| Field | Verdict |
|---|---|
| Overall Conformance | **Partial** |
| Overall Implementation Readiness | **Not Ready** |
| Source-level R1 Rust/API blockers | **Closed by current tree** |
| Remaining source blocker | **UI-001: incomplete** |
| Remaining readiness blocker | **READY-001: not verified in this audit** |

P061 is no longer blocked by the R1 source-level Rust/API divergences for Codex capacity, host-interruption cleanup, host-interruption readback, or housekeeping gate coverage. Those changes are present in the current tree.

P061 is still not ready for closeout because the current macOS UI implementation adds the Scheduler Health section but does not implement the proposal-required link from `DaemonLifecycleBanner` when sustained backpressure, stale projections, or DB-writer pressure is detected. The canonical `proposal-061` gate also was not executed in this R2 audit, so readiness remains unproven.

## Scope and method

The audit re-read targeted proposal slices and current implementation files for the R1 remediation claims:

- Capacity defaults and provider normalization.
- Host-interruption cleanup-before-retry semantics.
- Durable affected-execution cleanup/quota/readback fields.
- GraphQL/MCP scheduler-health and host-interruption parity.
- macOS Pilot Readiness scheduler-health UI surface.
- Canonical `proposal-061` gate membership and housekeeping proof coverage.

The audit did not restart the daemon, mutate run lifecycle, run validation commands, or modify implementation files.

## Prior R1 findings status

| R1 ID | Current status | Evidence |
|---|---|---|
| REL-001: Codex cap mismatch | **Closed** | Proposal now resolves Codex default cap as 10 at `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:106`, architecture default is `codex: 10` at `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:233`, acceptance uses Codex <=10 at `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:715`, and implementation default is `(ProviderFamily::Codex, 10)` at `control-plane/crates/domain/src/provider.rs:104`. |
| REL-002: retry enqueued despite cleanup failure | **Closed** | Cleanup failure now sets `settlement_status = retry_deferred_cleanup_failed` and increments deferred retry accounting at `control-plane/crates/engine/src/host_interruption.rs:357`; retry enqueue occurs only on the non-cleanup-failed branch at `control-plane/crates/engine/src/host_interruption.rs:376`; the focused test asserts no retry enqueue and deferred cleanup status at `control-plane/crates/engine/tests/proposal_061_backpressure.rs:2264` and `control-plane/crates/engine/tests/proposal_061_backpressure.rs:2302`. |
| API-001: affected-execution cleanup/quota evidence omitted | **Closed** | Migration `control-plane/crates/db/migrations/024_host_interruption_cleanup_evidence.sql:1` adds `previous_status`, `settlement_status`, `cleanup_status`, and `quota_budget_effect`; scheduler persistence binds those fields at `control-plane/crates/db/src/repos/scheduler.rs:514`; GraphQL exposes them at `control-plane/crates/graphql-server/src/types/scheduler.rs:189`; MCP report output includes them at `control-plane/crates/mcp-server/src/tools/reports.rs:298`. |
| UI-001: macOS operator backpressure surface missing | **Partially closed** | The Scheduler Health section now exists in Pilot Readiness at `Chainworks Forge/Views/PilotReadinessView.swift:178` and renders scheduler readback at `Chainworks Forge/Views/PilotReadinessView.swift:344`; GraphQL-only readback is implemented at `Chainworks Forge/Support/DaemonLifecycleClient.swift:338`. The required `DaemonLifecycleBanner` link is still missing; see current finding UI-001 below. |
| READY-001: canonical gate incomplete / not proven | **Source coverage improved, not verified** | `scripts/test-gate.sh:428` includes P061 focused tests, including housekeeping proof tests at `scripts/test-gate.sh:447`; the `proposal-061` case runs domain, workflow, DB, engine, GraphQL, and MCP tests at `scripts/test-gate.sh:2407`; `docs/reference/test-gates.md:1094` documents the expanded scope. This audit did not run the gate, so passing readiness is not established. |

## Current findings

### UI-001: `DaemonLifecycleBanner` does not link to Scheduler Health when scheduler pressure is detected

- Severity: Major
- Status: Open
- Violated requirement: P061 `surfaces[Scheduler Health]` requires: "Add a Scheduler Health section to PilotReadinessView and link to it from DaemonLifecycleBanner when sustained backpressure, stale projections, or DB writer pressure is detected" at `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:186`. This also supports the operator-visible pressure goals at `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:78` and `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:79`.
- Evidence: The Scheduler Health section itself exists at `Chainworks Forge/Views/PilotReadinessView.swift:178` and uses `schedulerHealthSection` at `Chainworks Forge/Views/PilotReadinessView.swift:344`. The daemon banner is still instantiated with only `DaemonLifecycleBanner(viewModel: daemonStatus)` at `Chainworks Forge/ContentView.swift:90`. `DaemonLifecycleBanner` renders lifecycle phases through `phaseView(for:)` and `row(...)` at `Chainworks Forge/Views/DaemonLifecycleSurface.swift:143` and `Chainworks Forge/Views/DaemonLifecycleSurface.swift:180`, and its concrete actions are limited to crash-budget reset / diagnostics in `failedStateActions` at `Chainworks Forge/Views/DaemonLifecycleSurface.swift:198`. No `Scheduler Health`, `schedulerHealth`, or backpressure navigation/action path is present in `DaemonLifecycleSurface.swift`.
- Why blocking, not backlog: The link is not an optional polish item in P061; it is part of the named Scheduler Health UI surface. Without it, sustained backpressure, stale projections, and DB-writer pressure can be visible only after the operator manually discovers Pilot Readiness. That fails the proposal's stated operator-visible pressure path for detected pressure states.
- Minimal fix: Add a scheduler-health readback or derived alert model to the banner composition layer, then show a `NavigationLink`, button, or equivalent app route to Pilot Readiness / Scheduler Health when `sustainedBackpressureState` is non-clear, `isStale` is true, or DB-writer pressure crosses the configured threshold. Add a focused Swift test or view assertion that the banner exposes this affordance for each detected condition.

### READY-001: Same-tree `proposal-061` gate pass was not produced in this audit

- Severity: Major
- Status: Open / Not Verifiable
- Violated requirement: P061 test plan names `./scripts/test-gate.sh proposal-061` as the canonical gate at `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:733`; the gate constraints require fake providers and in-process SQLite fixtures at `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:735`.
- Evidence: This audit did not run `./scripts/test-gate.sh proposal-061`, and no fresh R2 gate log was produced or read. Source registration exists at `scripts/test-gate.sh:2407`, but registration is not equivalent to a passing gate.
- Why blocking, not backlog: P061 spans scheduler capacity, SQLite write serialization, host-interruption recovery, GraphQL/MCP parity, and housekeeping safety. The proposal explicitly makes the canonical gate the closure proof for these interactions, so closeout without a same-tree pass risks accepting unexecuted cross-crate behavior.
- Minimal fix: Run `./scripts/test-gate.sh proposal-061` on the same tree after UI-001 is fixed, capture the full output, and attach or reference the timestamped log. If it fails, fix the focused failure before closeout.

## Requirements traceability summary

| Proposal area | Status | Evidence |
|---|---|---|
| Goals: 5 active runs without surfaced SQLite lock errors / 10 active runs backpressured | **Not verified in R2** | Covered by canonical gate registration, but no R2 gate run was performed. |
| Capacity defaults: global 20, per-run 4, Claude 8, Gemini 4, Codex 10, Auggie 1, Junie 1 | **Implemented** | Proposal lines `76`, `106`, `233`, `715`; implementation `control-plane/crates/domain/src/provider.rs:104`; tests `control-plane/crates/domain/src/provider.rs:208` and `control-plane/crates/engine/src/capacity.rs:80`. |
| Capacity-aware pending/backpressured claim behavior | **Implemented, not revalidated** | Existing P061 gate includes capacity tests at `scripts/test-gate.sh:431` through `scripts/test-gate.sh:442`; not executed by R2. |
| Scheduler readback freshness, GraphQL and MCP parity | **Implemented, not revalidated** | Swift GraphQL readback uses `schedulerHealthSummary`, `activeExecutionCountsByProvider`, and `queuedBackpressuredCountsByProviderAndReason` at `Chainworks Forge/Support/DaemonLifecycleClient.swift:396`; gate registers GraphQL/MCP P061 tests at `scripts/test-gate.sh:2417` and `scripts/test-gate.sh:2418`. |
| Host interruption classification and affected-execution cleanup/quota evidence | **Implemented, not revalidated** | Schema migration at `control-plane/crates/db/migrations/024_host_interruption_cleanup_evidence.sql:1`; persistence at `control-plane/crates/engine/src/host_interruption.rs:425`; GraphQL/MCP mappings at `control-plane/crates/graphql-server/src/types/scheduler.rs:189` and `control-plane/crates/mcp-server/src/tools/reports.rs:298`. |
| Host-interruption cleanup before retry enqueue | **Implemented, not revalidated** | Cleanup failure defers retry at `control-plane/crates/engine/src/host_interruption.rs:357`; retry branch is skipped on cleanup failure at `control-plane/crates/engine/src/host_interruption.rs:376`; focused test assertions at `control-plane/crates/engine/tests/proposal_061_backpressure.rs:2264`. |
| macOS Scheduler Health operator surface | **Partially implemented** | Section and GraphQL readback are present at `Chainworks Forge/Views/PilotReadinessView.swift:178` and `Chainworks Forge/Support/DaemonLifecycleClient.swift:338`; banner link required by proposal is missing. |
| Generated-state housekeeping safety | **Implemented, not revalidated** | Gate includes housekeeping tests at `scripts/test-gate.sh:447`; tests assert worktrees/source/artifacts/databases are preserved at `control-plane/crates/engine/src/housekeeping.rs:481`; reference docs updated at `docs/reference/test-gates.md:1106`. |
| Canonical gate pass | **Not verifiable** | Gate registered at `scripts/test-gate.sh:2407`; not run by R2. |

## Closeout recommendation

Do not close P061 yet. The current tree should first implement the missing `DaemonLifecycleBanner` to Scheduler Health affordance and then produce a same-tree passing `./scripts/test-gate.sh proposal-061` log.

After those two items, R3 can likely promote P061 to Implemented/Ready if the gate passes and no new source divergence appears.
