# Operator Experience

Stable reference for the operator-facing baseline that was previously tracked as `P005-OPS`.

## Purpose

Chainworks is only useful if one engineer can return to the app and understand:

- what is happening now,
- what needs attention,
- what is safe to do next,
- and what evidence exists for the run.

This document describes the implemented operator spine for the current baseline. It is not a future-state proposal.

Related stable docs:

- [idea-lifecycle.md](idea-lifecycle.md)
- [live-workflow-map.md](live-workflow-map.md)
- [run-control.md](run-control.md)
- [provider-binding-truth.md](provider-binding-truth.md)

## Scope

This reference covers the read-only and repo-agnostic operator layer:

- `RunsHomeView` as the primary landing surface
- idea/archive visibility truth across operator surfaces
- immutable run reports plus mutable latest summaries
- safe recovery actions for non-destructive run states
- deterministic run comparison
- run-detail workflow topology and agent activity surfaces
- artifact inspection with provenance and traceability
- notifications, dock badge, and menu bar presence

It does not define repo-backed write/release recovery. That boundary belongs to [007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md](../proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md).

## Runs Home

`RunsHomeView` is the operator landing surface.

Runs are grouped into:

1. `Waiting Approval`
2. `Blocked`
3. `Running`
4. `Recently Completed`

Each row shows:

- idea title,
- workflow title,
- run status,
- current stage,
- elapsed time,
- total cost,
- last progress timestamp,
- attention level,
- runtime provenance.

Contextual actions are status-aware:

- `Open` is always available
- `Open gate` appears only for approval-blocked runs
- `Recover` appears only for blocked or failed runs
- `Compare` appears only when a compatible target exists
- `View report` appears only when report artifacts exist

The operator shell must not promise an action that cannot actually run from the current row.

If a run belongs to an archived idea, that archived parent state remains visible in the row/detail context even though the idea is hidden from the default active ideas list.

## Runtime provenance

Operator surfaces must expose runtime trust instead of hiding it behind generic success/failure labels.

The current trust states are:

- `Fixture / verified baseline`
- `Goose server / trust pending`
- `Goose server / verified`

That provenance appears in:

- `RunsHomeView`
- run reports
- run comparison
- artifact metadata

## Reports

Run reporting is intentionally split into immutable history and a movable latest summary.

Every stable checkpoint emits:

- `run_report_v{n}.md`
- `run_report_v{n}.json`
- `run_summary_latest.md`
- `run_summary_latest.json`

Rules:

- immutable reports are never overwritten,
- recovery and re-arm actions append a new immutable version,
- latest summary may advance to the newest state,
- UI must distinguish immutable history from the latest summary.

Report content includes:

- run header and timestamps,
- elapsed time and total cost,
- workflow/catalog provenance,
- runtime trust level,
- stage and approval summary,
- agent/provider/model/effort usage,
- pinned artifacts,
- recovery notes,
- deterministic outcome.

## Recovery

The implemented recovery toolkit covers non-destructive paths for the current baseline:

1. `Retry Agent`
2. `Retry Stage`
3. `Resume from Approval Gate`
4. `Clone Run (Frozen Snapshot)`
5. `Clone Run (Current Config)`

`RecoverySheet` shows:

- blocked reason,
- most recent stage,
- trust/provenance summary,
- suggested safe next action,
- only the actions allowed for the current run type.

Out of scope here:

- writable worktree recovery,
- git/release/publish recovery,
- repo-backed side-effect re-entry.

Those remain Proposal 007 territory.

## Workflow map in run detail

Run detail includes a workflow-topology surface rather than only a flat log/status view.

That surface follows the stable contract in [live-workflow-map.md](live-workflow-map.md):

- map stays inside existing run-detail ownership,
- primary and fallback states are both first-class,
- agent activity grouping is explicit,
- loop direction and handoff counts are visible.

## Run comparison

Comparison is deterministic and structural.

Compatible runs compare:

- workflow hash,
- catalog hash,
- drift metadata,
- runtime trust level,
- provider/model/effort bindings,
- stage status,
- duration,
- cost,
- loops,
- approvals,
- pinned artifact presence and content deltas.

Comparison does not claim repo-backed or release-specific diff support.

## Artifact inspector

The operator artifact surface supports:

- markdown rendering,
- JSON rendering,
- diff rendering,
- text rendering,
- provenance chips,
- produced-by / consumed-by traceability,
- pin / unpin,
- open-on-disk actions.

Displayed provenance includes:

- run,
- stage,
- agent,
- provider,
- model,
- effort,
- attempt,
- runtime trust level.

## Notifications and presence

The notification layer is intentionally conservative.

It covers:

- approval required,
- run blocked,
- run failed,
- run completed.

Presence surfaces include:

- dock badge,
- optional menu bar extra,
- foreground banner while the app is active.

## Contracts consumed by later work

Later proposals and features may build on this operator spine, but should not redefine these baseline rules:

- runtime provenance remains visible,
- reports remain immutable-history plus latest-summary,
- contextual actions remain truthful,
- recovery stays policy-bounded,
- comparison stays deterministic.

Repo-backed delivery work in Proposal 007 extends this operator shell rather than replacing it.
