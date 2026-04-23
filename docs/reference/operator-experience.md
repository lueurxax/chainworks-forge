# Operator Experience

Stable reference for the operator-facing baseline.

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
- [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md)
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

It does not define repo-backed write/release recovery. That boundary belongs to [full-mvp-delivery.md](full-mvp-delivery.md).

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
- queued agent count (when > 0),
- runtime provenance.

Contextual actions are status-aware:

- `Open` is always available
- `Open gate` appears only for approval-blocked runs
- `Recover` appears only for blocked or failed runs
- `Compare` appears only when a compatible target exists
- `View report` appears only when report artifacts exist

## Scheduler Health and Backpressure

Backpressure is treated as normal scheduling state, not failure. When the system
reaches capacity (global, provider, or run-local), work remains queued and is
visible as "Queued" or "Waiting for provider slot".

Surfaces for backpressure visibility:
- **Sidebar Badge**: Shows queued agent count next to the run status.
- **Run Detail**: Displays active agents, queued agents, oldest queued age, and
  top backpressure reason.
- **Stage Detail**: Includes a "Backpressured Agents" disclosure showing pending
  agents by provider and reason.
- **Scheduler Health**: A dedicated section in PilotReadinessView showing system-wide
  capacity, write pressure, and command latency.
- **Sustained Backpressure Alerts**: Notifications trigger when work remains queued
  longer than the configured threshold (default 5 minutes).

### Workflow Conflict Details

When a run blocks due to a workflow conflict, a dedicated **Conflict Details** 
GroupBox appears immediately after the Blocker Summary. It provides:

- **Reason & Status**: Plain-language explanation (e.g., "Ambiguous next step") 
  plus status capsule.
- **Current State**: The authoritative graph state where the run is anchored.
- **Lead & Mediation**: The system lead agent assigned to the conflict and 
  active mediation progress.
- **Advisory Suggestion**: Redacted summary of the rejected agent hint.
- **Terminal Failure**: Detailed reason if the conflict reached `terminal_unverifiable`.

### Recovery

The implemented recovery toolkit covers non-destructive paths for the current baseline:

1. `Retry Agent`
2. `Retry Stage`
3. `Resume from Approval Gate`
4. `Clone Run (Frozen Snapshot)`
5. `Clone Run (Current Config)`

**Workflow Conflict Actions:**
- **Request lead mediation**: Escalate a conflict to the system lead for 
  automated resolution.
- **Inspect lead mediation**: View sanitized live status updates (queued, 
  running, validating) while mediation is active.
- **Manual Resolution**: For terminal conflicts, provides direct actions like 
  `Clone Run` or `Open editable recovery artifact`.

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

Those remain in the repo-backed delivery slice. See [full-mvp-delivery.md](full-mvp-delivery.md).

## Workflow map in run detail

Run detail includes a workflow-topology surface rather than only a flat log/status view.

That surface follows the stable contract in [live-workflow-map.md](live-workflow-map.md):

- map stays inside existing run-detail ownership,
- primary and fallback states are both first-class,
- agent activity grouping is explicit,
- loop direction and handoff counts are visible.

The exact segmented-pane placement, focused timeline inspector, and hierarchical artifact browsing rules for current run surfaces belong to [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md).

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

It does include current runtime-facing explanation lanes such as:

- skill truth drift,
- runtime profile drift,
- MCP requested/predicted/actual drift.

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
- runtime profile when available,
- skill reference / role / frozen snapshot hash when available,
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

The repo-backed delivery slice ([full-mvp-delivery.md](full-mvp-delivery.md)) extends this operator shell rather than replacing it.
