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
- [ui-action-boundary.md](ui-action-boundary.md)
- [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md)

## Scope

This reference covers the primarily **read-side** and repo-agnostic operator layer (per the [UI action boundary](ui-action-boundary.md)):

- `RunsHomeView` as the primary landing surface (GraphQL-only reads)
- idea/archive visibility truth across operator surfaces
- immutable run reports plus mutable latest summaries (metadata inspection only)
- diagnostic-only guidance for non-approval actions; in-app resolution for approvals
- deterministic run comparison (read-only)
- run-detail workflow topology and agent activity surfaces
- artifact inspection with provenance and traceability
- notifications, dock badge, and menu bar presence

It does **NOT** define broad in-app write/recovery. Recovery, retry, reset, compact, run start/cancel, clone, experiment, runtime, and context actions remain external MCP-only actions. Approval resolution is the only exception, supported via governed GraphQL mutations.

## Runs Home

`RunsHomeView` is the operator landing surface. It consumes workflow truth exclusively through GraphQL projections.

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
- runtime provenance,
- **Freshness state** (Live, Refreshing, Stale, etc.).

### Proposal Review Routing

Reviewer routing is artifact/readback truth rather than a bespoke routing dashboard.
For dynamically routed proposal-review stages, operator surfaces read:

- `agent_selection_plan_v1` for selected, rejected, and ineligible reviewers,
- `routing_receipt` for terminal routing status, rationale, warnings, and hashes,
- `SystemExecution` for system-task lifecycle state,
- `ReviewCorpusBundle` for the selected reviewer outputs consumed by aggregate
  review and proposal refinement.

The macOS app keeps typed parity DTOs for these payloads and redacts raw repository
evidence by default. Dedicated UI affordances must consume these artifacts and
projections; they are not the source of routing truth.

**Actions are primarily diagnostic:**
- `Open` is available for drill-down.
- `Open gate` is an active in-app control for resolving approvals via governed GraphQL mutations.
- Primary buttons for `Recover` or `Start` are replaced with diagnostic banners or technical details for use in external MCP/CLI workflows.

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
- **Routing Evidence**: For deterministic routing, the plan shows selected
  reviewers, rejected alternatives, warnings such as
  `mandatory_overflow_pruned`, and the evidence IDs behind those decisions.
- **Current State**: The authoritative graph state where the run is anchored.
- **Lead & Mediation**: The system lead agent assigned to the conflict and
  active mediation progress.
- **Advisory Suggestion**: Redacted summary of the rejected agent hint.
- **Terminal Failure**: Detailed reason if the conflict reached `terminal_unverifiable`.

### Evidence Projection

Repo-backed evidence references (path, symbol, span) in routing rationale default
to hash-only projection for privacy and security.
- **Raw Projection**: Requires the `operator_debug_routing_evidence` capability.
- **Authenticated Access**: Raw evidence is visible only in authenticated operator
  sessions (CLI/MCP/App Debug).
- **Restricted Readback**: Unauthenticated, unknown, or report/export readers
  always receive hash-only evidence.

If a run belongs to an archived idea, that archived parent state remains visible in the row/detail context even though the idea is hidden from the default active ideas list.

### Guided Retries (P065)

Operators can attach a short instruction to the `stages.retry` command. While the macOS UI remains read-only for this feature in v1, the read anchors are explicit:

- `RunTimelineInspectorView`: show a compact `Guided Retry` badge on retry attempts with instruction provenance.
- `StageDetailView`: add a `Retry Instruction` group with scope kind, journal id, actor, created-at timestamp, and aggregate delivery status.
- `FailedStageEvidencePanel`: mirror `Retry Instruction` evidence whenever the failed or blocked attempt came from an instructed retry.
- `RunReportView`: include `Retry Instruction Provenance` with fallback marker and delivery rows.

**Redaction Pattern:** Non-operator readers see a restricted marker ("Instruction Present (Restricted to Operators)") and provenance, but never raw instruction text.

## Runtime provenance

Operator surfaces must expose runtime trust instead of hiding it behind generic success/failure labels.

The current trust states are:

- `Fixture / verified baseline`
- `Legacy / unverified`
- `Legacy / verified`
- `Runtime / unverified`
- `Runtime / verified`

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
- runtime profile / adapter-family truth,
- skill provenance,
- MCP requested / predicted / actual / denied truth,
- stage and approval summary,
- agent/provider/model/effort usage,
- **Guided Retry Provenance (P065)**: include retry-instruction actor, timestamp, scope kind, and delivery status.
- pinned artifacts,
- recovery notes,
- deterministic outcome.

### Recovery

The governed thin UI does not execute non-approval recovery actions. Instead, it provides diagnostic identifiers to assist operators in executing MCP-owned workflows. See the [Operator Write-Path Guide (P031)](p031-operator-write-path-guide.md) for a complete mapping of removed controls to external workflows.

Diagnostic guidance is provided for:
1. `Retry Agent`
2. `Retry Stage`
3. `Resume from Approval Gate`
4. `Clone Run`

Approval resolution is the exception: approval surfaces are operator-actionable
through GraphQL approval mutations. Recovery, retry, reset, compact, run
start/cancel, clone, and runtime/context changes remain MCP-only.

**Workflow Conflict Actions (Read-Only):**
- **Mediation Progress**: View sanitized live status updates (queued,
  running, validating) while mediation is active.
- **Diagnostic Details**: For terminal conflicts, provides `run_id` and conflict
  identifiers for use in external resolution workflows.
- **Manual Resolution Guidance**: In-app actions like `Clone Run` or `Open editable
  recovery artifact` are replaced with diagnostic identifiers and suggested
  external commands.

`RecoverySheet` and diagnostic banners show:

- `run_id`, `stage_id`, or `approval_id` for copy-paste,
- suggested CLI / MCP command strings,
- `writePathState`: `read_only_diagnostic`,
- `disabledReasonCode`: `WRITE_PATH_NOT_AVAILABLE`.

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

### Bounded Startup Latency

The system enforces bounded artifact discovery to prevent broad local filesystem scanning from delaying ACP session initialization. Operators should notice significantly faster startup times in large workspaces compared to legacy implicit discovery models. Discovery diagnostics are available in the `Diagnostics` pane for technical inspection of settlement decisions.

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
