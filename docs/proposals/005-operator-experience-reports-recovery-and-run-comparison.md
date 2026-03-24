# Proposal 005: Operator Experience

## Subtitle
Run reports, recovery, comparison, notifications, and artifact ergonomics for the proposal-loop live baseline

| Field | Value |
|---|---|
| Date | 2026-03-24 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Proposal Alias | `P005-OPS` |
| Depends on | Proposal 001, Proposal 002, Proposal 003, [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [005-goose-server-transport-adapter.md](005-goose-server-transport-adapter.md) |
| Goal | Make Chainworks calm to operate on the current proposal-loop live slice without expanding scope into writable worktrees, repo-backed implementation, or release recovery. |

## 1. Canonical sequencing contract

This document exists in a repo that currently contains two different Proposal 005 drafts.

To keep sequential implementation unambiguous until the repo is renumbered:

- this file is the operator proposal and should be referenced as `P005-OPS` or by filename,
- [005-goose-server-transport-adapter.md](005-goose-server-transport-adapter.md) is the transport proposal and should be referenced as `P005-TRANSPORT` or by filename,
- no downstream document should rely on the bare text `Proposal 005` to mean both.

The sequential chain for this document is:

1. Proposal 004 baseline: [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md)
2. Transport compatibility layer: [005-goose-server-transport-adapter.md](005-goose-server-transport-adapter.md)
3. Operator spine: this document (`P005-OPS`)
4. Repo-backed implementation and manual release: Proposal 007

This operator proposal does not redefine Proposal 004 safety guarantees and does not pull Proposal 007 scope forward.

## 2. Current baseline this proposal is allowed to assume

The canonical baseline is the implemented live proposal-loop slice from [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md).

That baseline is intentionally narrow:

- proposal-loop live execution only,
- read-only workspace policy,
- no writable worktrees,
- no git, publish, release, or distribution side effects,
- approval and artifact inspection for proposal-loop artifacts,
- safe resume for waiting-approval and other non-destructive states.

`P005-TRANSPORT` may swap the live transport from fixture-backed to Goose server backed, but that does not enlarge operator scope by itself.

This proposal must remain correct under both runtime sources:

- fixture-backed live mode from Proposal 004,
- real Goose server mode from `P005-TRANSPORT` once its own sign-off is complete.

## 3. What this proposal builds

This proposal adds the first operator spine for the current baseline:

| Component | Responsibility |
|---|---|
| `RunsHomeView` | Primary landing surface for runs that need attention |
| `RunReportBuilder` | Deterministic immutable reports plus latest summary view |
| `RecoveryCoordinator` | Safe recovery actions for proposal-loop runs |
| `RunComparisonService` | Deterministic comparison for compatible runs |
| `ArtifactInspectorV2` | Better rendering, provenance, pinning, and traceability |
| `NotificationService` | Local notifications, dock badge, optional menu bar presence |

This proposal is intentionally about operator surfaces and deterministic summaries, not new execution intelligence.

## 4. Primary design rules

The operator surface must be calmer than the engine.

Rules:

- contextual actions only; no row may promise an action that cannot be executed from that row,
- provenance before polish,
- deterministic summaries before LLM prose,
- immutable history plus mutable latest summary,
- no operator surface may imply repo-write or release capability before Proposal 007.

## 5. Runs Home

### 5.1 Purpose

`RunsHomeView` becomes the primary operator entry surface.

It answers:

> What needs my attention right now, and what safe action is available?

### 5.2 Sections

Runs are grouped into:

1. `Waiting Approval`
2. `Blocked`
3. `Running`
4. `Recently Completed`

### 5.3 Row contents

Each row shows:

- idea title,
- workflow title,
- run status,
- current stage label,
- elapsed time,
- total cost,
- last progress timestamp,
- attention level,
- runtime provenance badge.

Runtime provenance badge must be explicit about transport trust:

- `Fixture / verified baseline`
- `Goose server / trust pending`
- `Goose server / verified`

The operator surface must never hide the distinction between a fully verified safety path and a compatibility path.

### 5.4 Contextual row actions

Row actions are status-aware:

- `Open` is always available
- `Open gate` appears only for `waitingApproval`
- `Recover` appears only for `blocked` or `failed`
- `Compare` appears only when at least one compatible comparison target exists
- `View report` appears only when at least one report artifact exists

There is no universal always-visible action strip that can dead-end.

## 6. Run reports

### 6.1 Why

The operator needs one stable record of what happened without reconstructing the story from raw stage screens.

### 6.2 Report semantics

This proposal fixes report trust by separating immutable history from the latest summary.

Every report cycle produces:

- immutable history artifacts:
  - `run_report_v{n}.md`
  - `run_report_v{n}.json`
- mutable latest pointers:
  - `run_summary_latest.md`
  - `run_summary_latest.json`

Rules:

- immutable reports are never overwritten,
- recovery or re-arm actions append a new immutable report version,
- latest summary may move forward and point to the newest state,
- the UI must label clearly whether the operator is reading immutable history or latest summary.

### 6.3 When a report version is emitted

A new immutable report version is created when the run reaches a stable checkpoint:

- terminal state (`completed`, `failed`, `cancelled`, `blocked`),
- explicit recovery action that changes execution direction,
- explicit approval re-arm after a stored checkpoint.

This prevents same-run recovery from silently invalidating a previously trusted "final" report.

### 6.4 Report contents

Immutable report sections:

1. Header
   - idea title
   - workflow title
   - run ID
   - run status
   - report version
   - timestamps
   - elapsed time
   - total cost

2. Snapshot and runtime provenance
   - workflow hash
   - catalog hash
   - runtime mode
   - policy acknowledgement authority or trust level
   - current-vs-frozen drift note if applicable

3. Execution summary
   - stages completed / skipped / failed
   - loops entered
   - approvals requested / granted / rejected
   - retries performed
   - recovery actions taken

4. Stage timeline
   - stage label
   - status
   - iteration
   - attempt
   - duration

5. Agents used
   - agent ID
   - provider
   - model
   - effort
   - cost
   - duration
   - final status

6. Approvals
   - gate label
   - decision
   - comment
   - requested / decided timestamps

7. Key artifacts
   - pinned artifacts first
   - then report-worthy artifacts

8. Recovery notes
   - blocked reason
   - retry path
   - resume path
   - drift decision

9. Outcome
   - deterministic conclusion only

### 6.5 Model additions

```swift
// Added to Artifact
var isPinned: Bool = false
var displayRole: String?
var reportKind: String?          // "immutable_history" | "latest_summary"
var reportVersion: Int?
var supersedesArtifactID: UUID?

// Added to Run
var latestSummaryArtifactID: UUID?
var latestImmutableReportArtifactID: UUID?
var latestReportVersion: Int = 0
var runtimeTrustLevel: String?   // "fixture_verified" | "server_unverified" | "server_verified"
```

## 7. Recovery toolkit

### 7.1 Recovery actions in scope

This proposal supports recovery for the current proposal-loop baseline:

1. `Retry Agent`
2. `Retry Stage`
3. `Resume from Approval Gate`
4. `Clone Run (Frozen Snapshot)`
5. `Clone Run (Current Config)`

### 7.2 Safety boundary

This proposal does not own repo-backed or release-side recovery.

The current baseline has no writable worktree stages, no git stages, and no publish stages.
Therefore:

- proposal-loop compute stages may be retried when policy allows,
- approval gates may be resumed or re-armed,
- cloned reruns may be created,
- repo-write, implementation, release, publish, or distribute recovery stays deferred to Proposal 007.

### 7.3 Explicit deferral table

| Action | Proposal-loop read-only run | Repo-backed implementation run | Release / side-effect run |
|---|---|---|---|
| Retry Agent | In scope here | Deferred to Proposal 007 | Deferred to Proposal 007 |
| Retry Stage | In scope here | Deferred to Proposal 007 | Deferred to Proposal 007 |
| Resume Gate | In scope here | Proposal 007 may extend | Proposal 007 owns release re-entry |
| Clone Frozen | In scope here | Proposal 007 may extend | Proposal 007 may extend |
| Clone Current | In scope here | Proposal 007 may extend | Proposal 007 may extend |

If a future run type includes repo-backed or side-effect stages, `P005-OPS` may still show the blocked state and historical context, but it must not pretend to implement the actual recovery mechanics.

### 7.4 Recovery UI

Blocked and failed runs surface `RecoverySheet` with:

- reason,
- most recent stage,
- trust/provenance summary,
- suggested next safe action,
- list of actions that are actually allowed for this run type.

## 8. Run comparison

### 8.1 Scope

Comparison in this proposal is deterministic and structural.

It is limited to compatible runs:

- same idea,
- same high-level workflow family,
- current proposal-loop baseline,
- no assumption of writable worktree or release receipts.

### 8.2 Comparison dimensions

- workflow hash
- catalog hash
- drift metadata
- runtime trust level
- provider / model / effort bindings
- stage status delta
- duration delta
- total cost delta
- loop delta
- approval delta
- pinned artifact presence and content diff

### 8.3 Out of scope here

Not in scope for this proposal:

- worktree path comparison,
- git commit / push / release receipt comparison,
- side-effect recovery diffing,
- publish or distribute artifact comparison.

Those become meaningful only when Proposal 007 introduces the repo-backed slice.

## 9. Artifact inspector V2

This proposal upgrades the existing artifact inspector with:

1. format-aware rendering
   - markdown
   - JSON
   - diff
   - generic text

2. provenance chips
   - run
   - stage
   - agent
   - provider
   - model
   - effort
   - attempt
   - runtime trust level

3. produced-by / consumed-by traceability
   - producing `AgentExecution`
   - consuming attempts via `inputBindingsJSON`

4. pin / unpin
   - affects reports and comparison

5. open actions
   - reveal in Finder
   - open on disk
   - copy path

This proposal does not add repo-backed shortcuts such as opening a writable worktree or release manifest. Those stay with Proposal 007.

## 10. Notifications and presence

This proposal adds operator-facing notifications for:

- approval required,
- run blocked,
- run failed,
- run completed.

Presence surfaces:

- dock badge for waiting approvals plus blocked runs,
- optional menu bar extra,
- foreground banners while the app is active.

Notification policy remains intentionally conservative. The operator should not be spammed on every stage completion.

## 11. File structure

```text
Chainworks Forge/
  Engine/
    RunReportBuilder.swift
    RecoveryCoordinator.swift
    RunComparisonService.swift
    NotificationService.swift
    ArtifactDiffService.swift

  Models/
    Run.swift
    StageExecution.swift
    AgentExecution.swift
    Artifact.swift

  Views/
    RunsHomeView.swift
    RunReportView.swift
    RunComparisonView.swift
    RecoverySheet.swift
    ArtifactInspectorView.swift
    MenuBarStatusView.swift

  Support/
    NotificationPreferences.swift
```

## 12. Acceptance criteria

### Runs Home

- [ ] `RunsHomeView` is the primary operator landing surface
- [ ] Runs are grouped into `Waiting Approval`, `Blocked`, `Running`, and `Recently Completed`
- [ ] Each row shows current stage, elapsed time, cost, attention level, and runtime trust/provenance
- [ ] No row shows an action that cannot be executed from that row

### Reports

- [ ] Every stable checkpoint emits immutable `run_report_v{n}.md/json`
- [ ] Latest summary artifacts exist separately from immutable history
- [ ] Recovery never overwrites a historical report
- [ ] Reports include runtime trust/provenance and drift notes

### Recovery

- [ ] Proposal-loop read-only runs support retry/re-arm/clone actions defined in this proposal
- [ ] Recovery UI only exposes actions allowed for the current run type
- [ ] No repo-write, release, or publish recovery is implemented here

### Comparison

- [ ] Operator can compare compatible proposal-loop runs in the UI
- [ ] Comparison shows snapshot, timing, cost, approval, and trust deltas
- [ ] Comparison does not claim repo-backed or release-specific diff support

### Artifact Inspector V2

- [ ] Inspector renders markdown, JSON, diff, and text
- [ ] Inspector shows provenance, attempt metadata, and runtime trust level
- [ ] Inspector supports produced-by / consumed-by traceability
- [ ] Inspector supports pin / unpin and reveal-on-disk

### Notifications

- [ ] Local notifications fire for approval, blocked, failed, and completed
- [ ] Dock badge reflects runs requiring attention
- [ ] Menu bar extra is optional but supported

### Sequential implementation gates

- [ ] No Proposal 001 / 002 / 003 / 004 runtime or UI tests regress
- [ ] The targeted live/recovery baseline from Proposal 004 and `P005-TRANSPORT` compiles and passes before `P005-OPS` implementation starts
- [ ] `xcodebuild build && xcodebuild test` is green before sign-off

### Product checkpoint (`PROD-PA-005-OPS`)

- [ ] One engineer can leave the app, return later, and understand in under 30 seconds:
  - what happened,
  - what needs attention,
  - what trust level the run has,
  - and what safe action is available.
- [ ] One engineer can recover a proposal-loop blocked or failed run without touching raw files or database state.

## 13. What is explicitly not in scope

| Exclusion | Owner |
|---|---|
| Writable worktrees | Proposal 007 |
| Repo-backed implementation runs | Proposal 007 |
| Git commit / push / release recovery | Proposal 007 |
| Publish / distribution recovery | Proposal 007 |
| Release receipts and release comparison | Proposal 007 |
| Semantic LLM-written reports | Later, if ever |
| Team collaboration / shared cloud inbox | Later |

## 14. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| OPS-051 | This document is `P005-OPS` until repo renumbering is cleaned up | Avoid ambiguous downstream references |
| OPS-052 | Operator scope is anchored to the current Proposal 004 baseline, not to Proposal 007 capabilities | Prevent scope leak |
| OPS-053 | Reports use immutable history plus latest summary semantics | Preserve trust during recovery |
| OPS-054 | Runtime trust/provenance must be visible in Runs Home, reports, and comparison | Do not hide transport/safety ambiguity |
| OPS-055 | Row actions are contextual, never universal promises | Avoid dead-end UI |
| OPS-056 | Repo-backed and release recovery stay out of scope here | Keep sequential implementation composable |

## 15. Handoff conditions

This proposal unblocks later work only when all of the following are true:

1. operator surfaces are correct for the current proposal-loop live baseline,
2. report history is immutable and recovery-safe,
3. no section of the operator UI implies writable worktree or release capability,
4. runtime trust/provenance is visible rather than inferred,
5. Proposal 007 can reuse the operator spine instead of replacing it.

After that:

- `P005-TRANSPORT` owns the first real network-backed live transport sign-off,
- Proposal 007 extends this operator spine to repo-backed implementation and release runs,
- Proposal 006 can assume the app is operable, but not that Proposal 007 capabilities already exist.
