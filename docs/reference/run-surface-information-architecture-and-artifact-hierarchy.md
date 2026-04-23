# Run Surface Information Architecture and Artifact Hierarchy

Stable reference for the run-surface information architecture and artifact-hierarchy slice.

## Purpose

Chainworks Forge already had the necessary run truth, but the operator shell was asking the user to scan too much of it at once.

This document is the stable contract for:

- the segmented run-shell structure in `Runs Home` and `Idea`,
- deterministic pane routing for action-critical run states,
- the focused timeline inspector as a subordinate workflow-map surface,
- the shared canonical artifact hierarchy used for run browsing,
- and the continuity rule that demoted metadata must not orphan report, export, delivery, or recovery paths.

## Scope

This reference covers:

- `RunsHomeView` as the run-centric segmented inspection surface,
- idea-side run progress as the idea-centric segmented operation surface,
- `RunSurfacePaneRouting` and pane-priority behavior,
- `WorkflowMapView` plus `RunTimelineInspectorView`,
- `RunArtifactHierarchy`, `RunArtifactHierarchyBuilder`, and `RunArtifactHierarchyView`,
- and the surviving run-owned report/export path after metadata demotion.

It does not redefine:

- execution truth or recovery authority in [execution-truth-and-recovery.md](execution-truth-and-recovery.md),
- the broader operator shell baseline in [operator-experience.md](operator-experience.md),
- workflow-topology semantics in [live-workflow-map.md](live-workflow-map.md),
- or repo-backed delivery semantics in [full-mvp-delivery.md](full-mvp-delivery.md).

## Core Rules

### Two run surfaces remain, and they are intentionally different

The product still has two run surfaces:

- `Runs Home` for inspection, diagnostics, report/export, and recovery-oriented work,
- `Idea` for active progress watching, approvals, and quick operational control inside idea context.

They do not collapse into one unified run screen, and they do not share the same pane set.

### Segmented panes replace the old long run-detail stack

Both run surfaces now follow the same shell pattern:

- compact run header,
- compact shell-owned action row,
- segmented switcher,
- one active content pane at a time.

The segmented switcher is in-place content replacement, not a new navigation hierarchy.

### `Runs Home` owns the inspection-first pane set

The implemented `Runs Home` pane contract is:

- `Summary`
- `Flow`
- `Artifacts`
- `Diagnostics`

`Summary` keeps high-frequency run status, stage, cost, elapsed time, and shell-owned actions.

`Flow` owns the workflow-map-derived operator truth that used to be spread across the long stack:

- topology,
- summary chips and stage health counters,
- agent activity,
- handoff trail,
- loop/iteration telemetry,
- focused timeline entry.

`Artifacts` owns the canonical hierarchy browser plus promoted artifacts.

`Diagnostics` owns lower-frequency receipts, anomalies, recovery evidence, discovery settlement logs (P053), and other deep technical inspection paths.

### `Idea` owns the operation-first pane set

The implemented idea-side run progress pane contract is:

- `Summary`
- `Progress`
- `Artifacts`
- `Approvals`

`Summary` keeps the immediate run headline, next action, and run control.

`Progress` owns stage progression, active execution focus, recent handoff context, loop state, and focused timeline entry.

`Artifacts` reuses the same canonical hierarchy as `Runs Home`, but is consumed from the idea context.

`Approvals` keeps pending approval context, rationale, and decision history as a first-class pane rather than permanent summary clutter.

### Action-critical states override stale pane memory

Segment restoration is allowed for neutral states, but urgent states must stay foregrounded.

Current pane-priority routing rules:

- `running`, `pending`, `ready`: default to `Summary`
- `waitingApproval`: auto-select or deep-link into `Approvals`
- `blocked`, `failed`: foreground `Summary` with next action and one-step recovery entry

If the operator arrives from an approval-focused or recovery-focused affordance, that routing outranks stale remembered pane state.

### Focused timeline stays subordinate to the workflow-map owner path

`Live Timeline` is no longer a default inline run-detail block.

The ownership contract is:

- `WorkflowMapView` remains the shell-owned topology owner,
- `RunTimelineInspectorView` is opened from `Flow` or `Progress`,
- the focused timeline is a subordinate inspection extension, not a second run overview,
- timeline truth must still resolve back to the workflow-map/run-detail spine.

### Artifact hierarchy is a browsing projection, not a second truth lane

`RunArtifactHierarchy` is canonical for grouping and navigation only.

It does not replace:

- persisted `Artifact` identity and lineage,
- immutable report authority,
- latest-summary authority,
- evidence-pack or sign-off export authority,
- comparison or recovery readers.

The hierarchy builder must preserve the persisted metadata already required by those readers, including:

- `reportKind`,
- `reportVersion`,
- `supersedesArtifactID`,
- stable artifact identifiers and stage/agent lineage.

### Artifact grouping is centralized and deterministic

Artifact browsing must not be recomputed ad hoc inside each view.

The implemented browsing lane is shared through one builder and one view model:

- stage groups,
- stage iteration,
- agent groups,
- semantic buckets,
- promoted artifacts.

This keeps `Runs Home`, `Idea`, and shell-owned readers on the same grouping truth.

### Promoted artifacts stay first-class

Promoted artifacts remain a first-class operator slice.

They may appear above the main tree or as a dedicated promoted group, but they must not disappear into deep nested stage buckets.

Promoted status affects browsing priority only. It does not create a second interpretation layer for reports, exports, or sign-off packets.

### Metadata demotion cannot orphan repo-backed continuity

Low-signal metadata such as repository details, delivery configuration, raw worktree information, and verbose receipts no longer dominate the default summary path.

That demotion is allowed only because one explicit shell-owned run path still preserves the repo-backed actions the operator already had:

- worktree reveal,
- release manifest access,
- git push receipt access,
- upload receipt access,
- evidence-pack export,
- sign-off export,
- report and comparison entry points.

In the current implementation, that continuity path is anchored by the run-owned export/report hub rather than by the old always-visible summary stack.

## Current Implementation Owners

The main implementation owners for this slice are:

- `RunsHomeView`
- `IdeaListView` / `WorkflowRunProgressView`
- `RunSurfacePaneRouting`
- `WorkflowMapView`
- `RunTimelineInspectorView`
- `RunArtifactHierarchy`
- `RunArtifactHierarchyBuilder`
- `RunArtifactHierarchyView`
- `CompletedRunExportHub`

These components implement the stable contract above; proposal-era wording should no longer be treated as the canonical source of truth.

## Relationship To Adjacent Baselines

The split of authority is:

- [operator-experience.md](operator-experience.md) owns the broader operator shell baseline and truthful action semantics,
- this document owns the detailed segmented run-shell IA, focused timeline placement, hierarchy browsing rules, and metadata-demotion continuity,
- [live-workflow-map.md](live-workflow-map.md) owns workflow-map state vocabulary and topology expectations,
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) owns run truth, reports, and recovery precedence,
- [full-mvp-delivery.md](full-mvp-delivery.md) owns repo-backed delivery semantics, not the shell placement details for how their artifacts/actions are reached.

## Verification and Proof Path

This slice is proved by a mix of focused local verification and approved-host UI execution:

1. green local macOS build on the current tree,
2. green focused non-UI suites for pane routing and hierarchy building,
3. green approved-host targeted UI proof for the surviving export hub and focused timeline owner path,
4. green approved-host `proposal-024` gate.

The gate keeps its original label for reproducibility.

## Adjacent References

Use:

- [operator-experience.md](operator-experience.md) for the wider operator-shell baseline,
- [live-workflow-map.md](live-workflow-map.md) for topology and activity semantics,
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) for report/recovery/read authority,
- [full-mvp-delivery.md](full-mvp-delivery.md) for repo-backed run actions and evidence semantics,
- [test-gates.md](test-gates.md) and [agent-ui-test-execution.md](agent-ui-test-execution.md) for canonical proof execution.
