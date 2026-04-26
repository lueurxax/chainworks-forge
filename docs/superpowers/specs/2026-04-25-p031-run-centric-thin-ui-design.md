# P031 Run-Centric Thin UI Restoration Design

Date: 2026-04-25
Status: Approved design, pending implementation plan
Owner: P031 macOS thin UI implementation

## Decision

Restore P031 operator inspection around a run-centric workspace.

The selected shape is B: a primary Runs workspace with the selected run as the center of the product. Ideas, agent catalog, and workflow catalog return as read-only context panels attached to the selected run instead of as old local-write tabs. This lets dogfood validate the surfaces P031 requires without reviving the legacy SwiftData/local-orchestrator UI.

## Goals

- Make every run row identifiable by idea/proposal title, not only workflow title.
- Restore useful Run Detail inspection: status, progress, freshness, approvals, stage list, transition visualization, artifacts, reports, and diagnostics.
- Restore stage transition visualization from server-owned readback.
- Restore artifact/report inspection with Markdown, JSON, diff, and plain-text rendering only when payload content is available from a server-owned GraphQL read path.
- Restore idea and catalog context for the selected run as read-only inspectors.
- Keep P031-owned UI GraphQL-only: queries, subscriptions, bounded polling, and targeted read refresh.

## Non-Goals

- No SwiftData workflow truth in governed P031 UI.
- No local workflow compiler, local recovery coordinator, local execution service, or raw artifact filesystem scan as UI truth.
- No MCP calls from P031 UI.
- No GraphQL mutations.
- No in-app write controls for create idea, start run, retry, approve/reject, cancel, clone, compare, experiment, or session reset.
- No full report payload rendering unless the control-plane exposes a server-owned GraphQL payload read path.

## User Experience

The first screen is a run workspace.

The left rail lists active/recent runs. Each row shows:

- idea/proposal title as the primary label,
- workflow title as secondary context,
- status,
- stage progress,
- freshness state,
- approval count when present.

The main detail area for the selected run contains:

- run summary and diagnostics,
- stage transition visualization,
- stage detail list,
- approval diagnostic rows,
- artifact/report list,
- payload renderer when available,
- selected idea context,
- selected workflow/agent catalog context.

Write actions appear only as external write-path guidance or explicit unavailable rows.

## Architecture

### Read Store

Extend the existing P031 GraphQL read boundary rather than adding a new local store.

Required read models:

- `P031RunRowReadModel` and `P031RunDetailReadModel` continue to own run identity and summary.
- Add or extend read models for idea context, workflow/catalog context, stage transition graph, artifact payload availability, and artifact payload content.
- Any server gap must be represented as an explicit unavailable/deferred state or implemented as a read-only GraphQL query.

### Run Workspace

`RunsHomeView` becomes the primary composition root for the run workspace. It should remain a thin presentation layer over P031 presenters and read stores.

The old pre-P031 views may be used as layout references only. Their data and actions must not be restored if they depend on SwiftData, local services, MCP, or filesystem truth.

### Stage Transition Visualization

The stage graph renders from GraphQL-backed stage rows plus server-owned workflow snapshot/readback. If the server cannot expose transitions yet, the UI shows a stage timeline and an explicit "transition topology unavailable" diagnostic rather than rebuilding the workflow locally.

### Artifacts And Reports

The artifact list renders from GraphQL artifact/report metadata.

Payload rendering rules:

- Markdown renderer is enabled only for GraphQL payload content.
- JSON tree renderer is enabled only for GraphQL payload content.
- Diff/plain text renderers follow the same rule.
- Metadata-only artifacts show payload state and unavailable reason.
- The UI must not open raw local artifact paths to fill missing server payloads.

### Ideas And Catalogs

Idea context is read-only and scoped to the selected run's `ideaId`.

Agent/workflow catalog context is read-only and scoped to the selected run's server-owned snapshot or catalog metadata. If catalog snapshot readback is not exposed through GraphQL, the panel shows an explicit unavailable state with the required follow-up field/query named in diagnostics.

## Error Handling

- Initial GraphQL failure renders unavailable state.
- Subscription disconnect renders stale/refreshing state and bounded reconnect behavior.
- Missing read fields render disabled/deferred states.
- Unauthorized readback renders unauthorized state without local fallback.
- Payload unavailable rows remain inspectable as metadata.

## Tests And Gates

Implementation must include focused Swift tests for:

- run rows show idea/proposal title and workflow secondary label,
- run detail preserves the same identity,
- stage transition visualization renders from supplied read models,
- artifact renderer selects Markdown/JSON/diff/plain text only from GraphQL payload models,
- metadata-only payloads do not open local files,
- idea/catalog context panels render read-only states,
- forbidden write/local paths remain rejected by P031 static guard.

Verification path:

- targeted P031 Swift test suite,
- `./scripts/test-gate.sh proposal-031`,
- `./scripts/test-gate.sh proposal-031-readiness` only after Phase 3 evidence is complete.

## Acceptance Criteria

- The app can identify which idea/proposal each visible run belongs to.
- A user can inspect a run, its stages, transition progress, approvals, artifacts, reports, idea context, and catalog context without leaving the read-only UI.
- Every visible value comes from GraphQL readback or an explicit unavailable/deferred state.
- P031 static guard still rejects MCP, GraphQL mutations, local workflow writes, raw truth probing, and enabled removed controls.
- Dogfood can validate the actual intended P031 experience instead of a single-screen placeholder.
