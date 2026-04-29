# Proposal 031: Thin GraphQL-Only UI Rewrite Over Server Projections

Revision: `031-2026-04-24-r19-degraded-state-correction`
Source packet: prior run `8dd01a54-0791-43e0-b526-5ed92c95b34f`, r18  
Current run: `72409268-9dea-4ece-82f6-6ef29b4a446e`  
Status: stopped at GraphQL-only read-boundary stabilization. Do not continue P031 as the vehicle for visual/product polish; remaining visual, dogfood, and stabilization tails are handed off to P032/P036.

## Executive Summary

P031 cuts the macOS operator app from client-owned workflow truth to a thin, GraphQL-only read UI over server-owned projections. After P031, governed SwiftUI workflow surfaces render GraphQL read models and maintain only presentation state, server-derived caches, read-refresh state, and freshness handling.

P031-owned UI must not use MCP reads, MCP writes, non-approval GraphQL mutations, local workflow mutation fallback, command payload construction, command receipts, command correlation, or broad write-path implementation. P072 supersedes the original P031 all-mutation ban with one narrow exception: governed SwiftUI may use GraphQL only for `approveApproval` and `rejectApproval`. MCP remains supported for agents, CLI/operator diagnostics, automation, and debug/control workflows outside the governed macOS UI.

This restart keeps the prior r18 packet as the baseline and converts it into a single proposal document for clean aggregate re-review. It explicitly incorporates all prior reviewer feedback, including the stale r7 implementation approval rejection. The result is intentionally conservative: it does not restore UI writes, it does not add a second control plane, and it does not hide the operator trade-off that writes move outside the macOS UI until separate follow-up proposals restore approved write paths.

## Decision Summary

Stop P031 after landing the GraphQL-only read boundary and thin read surfaces. Treat P031 as the architectural cutover proposal, not the product-polish proposal. Do not use P031 to continue visual restoration, navigation redesign, dogfood sign-off, or write-path restoration work.

Follow-up ownership:

- P032 owns stabilization, release-readiness evidence, dogfood/sign-off, degraded drills/waivers, daemon lifecycle polish, freshness evidence, and documentation cleanup.
- P036 owns visual and navigation restoration over the GraphQL read model: richer run/stage cards, definitions/catalog ergonomics, idea/catalog surfaces, stage-transition visualization, artifact browsing ergonomics, and overall operator flow.
- Future write-path proposals own any interactive create/start/cancel/retry behavior. P072 owns the narrow approval-only GraphQL mutation exception for `approveApproval` / `rejectApproval`; P031 does not restore any other UI writes.

The old Swift-local UI remains useful only as a visual and ergonomic reference. Mutation affordances from that UI are explicitly out of scope unless a later proposal defines an approved non-MCP, non-GraphQL-mutation transport.

Hard decisions:

- Governed macOS UI reads workflow truth through GraphQL only.
- Governed macOS UI has no MCP calls, no non-approval GraphQL mutations, and no local mutation fallback.
- Approval rows are diagnostic-read-only in the original P031 stop-state, but P072 supersedes that clause: interactive approval decisions use only the `approveApproval` / `rejectApproval` GraphQL mutations.
- Full report payload rendering remains outside P031 and defaults to a P0 follow-up unless Phase 0d evidence proves metadata-only inspection is acceptable.
- P031 does not preserve or restore the old Swift-orchestrator path. Fail-closed behavior means disabling or degrading affected thin UI surfaces while the control-plane database and GraphQL projections remain the source of truth; no local workflow writes are restored.
- The stale r7 GraphQL+MCP implementation approval is non-authoritative. No further P031 implementation approval should be pursued unless P031 is explicitly reopened; product polish now belongs to P032/P036.

## Stop-State Handoff: Visual Diff and Deferred Tails

Visual baseline commit: `1cca56b9abd622ad7dc4e38304985cbf49e66780` (`2026-04-19T19:05:19+03:00`, `Add P060: lead-driven reviewer routing and expanded reviewer catalog`).

This is the last pre-control-plane visual/ergonomic baseline selected for P031 handoff. It is the parent of the first large control-plane land commit `a17b1cd04ac38f46f61111c647911f03844b4a33` from 2026-04-21. Use it only as a visual reference. It includes old Swift-local mutation paths that are not P031-compatible and must not be restored through P031.

Current P031 state:

- The governed workflow UI reads through GraphQL-backed presenters.
- The app exposes read-only Runs Home, run detail, stage transitions, artifacts, catalog context, approvals diagnostics, reports metadata, and daemon lifecycle.
- Artifact rendering now performs content-based markdown/JSON handling instead of trusting file extensions alone.
- Artifact browsing has a first-pass GraphQL-only filter/group/detail layout.
- Non-approval write controls remain unavailable or diagnostic-only. Approval rows may expose the P072 approval-only GraphQL actions.

Visual/ergonomic regressions to carry forward:

| Surface | Pre-control-plane visual behavior worth preserving | Current P031 state | Follow-up owner |
| --- | --- | --- | --- |
| Runs Home rows | Status-grouped rows with clear attention lanes, compact action affordances, provenance/status chips, and strong card scanability. | GraphQL rows are functionally readable but visually flatter and less informative. | P036 |
| Run detail | Dedicated panels for progress, workflow map, recovery, reports, comparisons, and artifact drill-in. | Single thin read-detail stack; useful for proof of read-boundary, not product-complete inspection. | P036 |
| Stage visualization | `WorkflowMapView` had horizontal stage cards, chevron flow, current-stage outline, hover/tap detail popover, loop progress, occurrences, and transition labels. | P031 has a vertical transition list. It is truthful but loses the map/card affordance and stage-level density. | P036 |
| Handoffs and agent panels | Old workflow map separated topology, handoffs, agents, telemetry, and timeline. | P031 exposes a simplified stage transition surface. | P036 |
| Artifact hierarchy | Old `RunArtifactHierarchyView` grouped by stage/agent/semantic bucket, had promoted artifacts, filters, search, badges, and artifact rows with timestamps. | P031 has partial GraphQL-only grouping/filtering and split list/detail, but should be polished against the old hierarchy behavior. | P036 |
| Artifact inspector | Old inspector had provenance chips, produced-by/consumed-by traceability, pin/open actions, proposal-loop summaries, and format-aware rendering. | P031 focuses on safe read-only payload rendering. Provenance/traceability/actions need GraphQL-backed replacements or explicit deferral. | P036 for UX, P032 for evidence/docs |
| Ideas | Old Ideas surface had sidebar cards, summary strip, archive/new idea flows, approval bar, and idea detail with run context. | P031 does not restore a full GraphQL-only idea/catalog experience. | P036 |
| Agent catalog | Old catalog retained a two-pane detail form with summary strip and validation issues, but was still flat. | Current thin UI only provides limited catalog context in run detail. Full definitions/catalog inspection belongs outside P031. | P036 |
| Workflow inspector/catalog | Old inspector showed workflow state details but sorted poorly; P036 already owns execution-order sorting and definitions consolidation. | P031 does not replace the full definitions experience. | P036 |
| Dogfood/readiness evidence | P031 audits identified missing dogfood/sign-off, VoiceOver spot check or waiver, degraded drill/waiver, and post-dogfood approval re-entry. | Stop P031 before forcing audit closure on an incomplete product surface. | P032 |

P031 close rule:

- Keep the GraphQL-only boundary and tests as durable repository truth.
- Do not spend more P031 work on restoring visual richness or write-path viability.
- Move all remaining work into P032/P036 or later write-path proposals before launching through Chainworks.

## Problem

The macOS operator app still has UI-facing paths that can read or infer workflow truth from SwiftData, local compiled plans, recovery services, local execution services, raw artifacts, and prior control affordances. Reviewers agree with the GraphQL-only cutover direction. The remaining risks are implementation drift and release timing:

- Stale GraphQL+MCP handoff artifacts can still imply UI command/control behavior.
- P043/P031 reference language can still assign command-completion, command receipts, or MCP command-control responsibilities to P031 UI.
- The P031 gate and UI ownership inventory need to be executable, not prose-only.
- New disabled/report/approval/freshness metadata fields must be server-owned or explicitly deferred.
- Operators need a validated external write workflow while UI writes are removed.
- Degraded/fail-closed states must not restore the old local orchestrator, local workflow truth, or local UI writes.

## Desired State

After P031:

- Visible workflow truth comes from GraphQL projections.
- Swift-local state is limited to presentation, server-derived caches, read-refresh state, and freshness handling.
- Missing GraphQL read fields block the affected UI migration or are represented as disabled/deferred states.
- Every governed UI file and generated GraphQL location is covered by a machine-readable inventory consumed by the P031 gate.
- Every removed write control is represented in an operator write-path guide row before dogfood.
- Dogfood evidence proves both technical compliance and operator workflow viability.
- Degraded/fail-closed UI states keep control-plane-owned truth authoritative and never re-enable local UI writes.

## Goals

- Replace P031-owned SwiftUI workflow reads with GraphQL projection read models and freshness metadata.
- Make the macOS UI GraphQL-only: queries, subscriptions, bounded polling, and targeted read refresh only.
- Strictly prohibit MCP usage from P031-owned macOS UI code.
- Remove or replace Create Idea, Start Run, Cancel Run, Stage Retry, Steward, runtime-health, session, clone, compare, experiment, and approval write affordances from governed screens.
- Preserve operator inspection ergonomics for Runs Home, Run Detail, stages, approvals, artifacts, report metadata, daemon lifecycle, and recovery/evidence readback.
- Render approvals as actionable only through the P072 `approveApproval` / `rejectApproval` GraphQL exception; all other write controls remain diagnostic-only guidance unless a separately approved transport exists.
- Ship report metadata inspection with list-level payload availability indicators.
- Reconcile P043/P031 reference and gate language so command-completion refresh, command receipts, command correlation, and MCP control rules are outside P031 UI.
- Define concrete GraphQL fields, enum cases, nullability, redaction, Swift presenter ownership, and tests for disabled/report/approval metadata.
- Define a machine-readable P031 UI file/type inventory.
- Publish a pre-dogfood operator write-path guide mapping every removed UI write control to an external workflow or unavailable follow-up.
- Capture user-outcome evidence during dogfood, not only static compliance.
- Define fail-closed rollout states, degraded-state evidence, freshness measurement, hold criteria, and sign-off authority.

## Non-Goals

- P031 does not redefine workflow execution semantics.
- P031 does not create a second control plane.
- P031 does not make MCP available to the macOS UI for reads or writes.
- P031 does not add broad GraphQL mutations or any other UI write transport. P072 supersedes this for the two approval-only GraphQL mutations.
- P031 does not route UI actions through MCP command tools.
- P031 does not add command journaling, CommandHandler wiring, command receipt recovery, ActionInvocationIdentity, CommandLegality, Check Status, Reissue Command, `client_command_id` command correlation, or MCP parameter mapping for UI writes.
- P031 does not implement Create Idea, Start Run, Cancel Run, Stage Retry, reset-session, resume, clone, comparison, experiment launch, runtime-health actions, agent reset, Steward actions, or second-wave MCP tools in the UI.
- P031 does not ship full report payload rendering unless a server-owned GraphQL report payload query lands first and is added to the P031 gate.
- P031 does not expand non-operator GraphQL read authorization.
- P031 does not declare external CLI/MCP command recipes complete; it defines the guide schema and blocks dogfood until recipes are named and validated.
- P031 does not restore the old local-orchestrator UI path as a recovery mechanism.

## Scope

In-scope reads:

- Runs Home reads runs from GraphQL run projections.
- Run Detail reads `run(id:)` and `runStatusChanged(runID:)` from GraphQL.
- Stage surfaces read stages and stage detail read models from GraphQL.
- Artifacts surfaces read artifacts from GraphQL.
- Reports surfaces read report metadata, list-level payload availability, and payload unavailable reasons from GraphQL.
- Approvals queue reads approval rows, ambiguity state, write-path state, and diagnostic identifiers from GraphQL.
- Daemon lifecycle and projection freshness are read from server-owned GraphQL/lifecycle surfaces.

In-scope operator controls:

- Targeted read refresh controls for projection-backed surfaces.
- Copy Diagnostic ID and Technical Details affordances.
- First-run dogfood orientation explaining read-only UI and external write workflows.

Removed or diagnostic-only writes:

- `ideas.create`
- `runs.start`
- `runs.cancel`
- `stages.retry`
- `approvals.resolve`
- `steward.run_analysis`
- reset, resume, clone, compare, experiment launch, runtime-health, and session actions
- local Swift recovery or execution mutation paths
- UI MCP command paths
- UI GraphQL mutation paths, except P072 `approveApproval` / `rejectApproval`

## Architecture

### Read Plane

GraphQL is the only macOS UI data plane for workflow truth.

Rules:

- The UI may query, subscribe, and poll GraphQL read models.
- The UI may trigger targeted read refreshes that refetch GraphQL data.
- The UI must not use GraphQL mutations except P072 `approveApproval` / `rejectApproval`.
- The UI must not use MCP clients, MCP tools, MCP read helpers, or MCP write helpers.
- The UI must not read workflow truth from SwiftData, local compiled plans, local recovery services, raw artifact directories, raw report files, or local execution services.
- MCP read/control tools remain allowed for agents, CLI/operator tooling, automation, and diagnostics outside the macOS UI contract.
- If a visible workflow field is missing from GraphQL, implementation must add the field to the server read model or disable/defer the UI surface.

### UI Write Prohibition

P031-owned UI is read-only except for read refreshes and diagnostic copy affordances.

Static guard requirements:

- Fail if governed UI imports or instantiates `MCPCommandClient`, `MCPPolicyRuntime`, MCP transport, or any MCP tool wrapper.
- Fail if governed UI contains GraphQL mutation operations, generated mutation calls, or mutation client types other than P072 `approveApproval` / `rejectApproval`.
- Fail if governed UI calls `ideas.create`, `runs.start`, `runs.cancel`, `stages.retry`, `approvals.resolve`, `steward.run_analysis`, session/reset, clone, compare, experiment, runtime-health, local recovery, or local execution mutation paths.
- Fail if governed UI constructs MCP parameter dictionaries, `ActionInvocationIdentity` payloads, `client_command_id` command correlation, command receipt state, or command invocation adapters.

### P043/P031 Reconciliation

P043 remains the GraphQL projection read contract. Any P043 language assigning MCP command-control, command receipts, command-completion refresh, or command correlation to P031-owned UI must be amended or scoped to non-P031/non-UI surfaces before Phase 1.

Phase 0a must:

- Amend `docs/reference/query-projections-and-client-consumption-contract.md` or add a checked-in addendum stating that P031-owned UI consumes GraphQL reads only.
- Remove P031 UI ownership of command-completion refresh behavior, command receipt display, and MCP command-control rules from composed P031 gate language.
- Keep MCP command behavior documented for non-UI agents, CLI, automation, and diagnostics.
- Register `proposal-031` and `p031` gates.
- Mark stale GraphQL+MCP idea-brief acceptance criteria as superseded by this GraphQL-only contract.

### Schema Contract

Every visible P031 workflow field must have one named GraphQL source or one explicit disabled/deferred state before the affected UI migrates.

Required schema-or-defer decisions:

| Field | Source surface | Nullability | Redaction default | Swift owner |
| --- | --- | --- | --- | --- |
| `freshnessState` | run, run row, stage, approval, artifact, report metadata | non-null | operator-visible; unauthorized denied/unavailable | `WorkflowFreshnessReducer` |
| `disabledReasonCode` | approvals and deferred action metadata | nullable | operator-visible; unauthorized generic/omitted | `DisabledReasonPresenter` |
| `writePathState` | approval rows | non-null | operator-visible; unauthorized omitted | `DisabledReasonPresenter`, `ApprovalDiagnosticPresenter` |
| `diagnosticId` | approvals, report metadata where needed | nullable | operator-only by default | `ApprovalDiagnosticPresenter` |
| `payloadAvailabilityState` | report metadata | non-null | authorized report readers only | `PayloadUnavailableReasonPresenter` |
| `payloadUnavailableReasonCode` | report metadata | nullable unless unavailable/deferred | precise for operator, generic/omitted otherwise | `PayloadUnavailableReasonPresenter` |
| `serverDebugDetail` | diagnostic extensions only | nullable | operator-only, never primary copy | `DiagnosticDetailsPresenter` |

Required enum values:

- `freshnessState`: `live`, `refreshing`, `projection_lag`, `stale`, `unavailable`, `unauthorized`
- `disabledReasonCode`: `WRITE_PATH_NOT_AVAILABLE`, `MANAGED_OUTSIDE_UI`, `AMBIGUOUS_APPROVAL_IDENTITY`, `STALE_READ`, `PROJECTION_LAG`, `UNAUTHORIZED`, `UNSUPPORTED_ACTION`
- `writePathState`: `available`, `read_only_diagnostic`, `write_path_not_available`, `external_transport_required`, `hidden`
- `payloadAvailabilityState`: `available`, `metadata_only`, `payload_deferred`, `generating`, `unavailable`
- `payloadUnavailableReasonCode`: `PAYLOAD_DEFERRED_BY_P031`, `GENERATING`, `NOT_INDEXED`, `NOT_AUTHORIZED`, `NOT_AVAILABLE`, `UNKNOWN`

Tests must cover operator reads, unauthorized denial/redaction, observer deferral where applicable, no Swift status inference, report payload indicator rendering, approval diagnostics, and no raw file probing.

### UI Ownership Inventory

Phase 0b must create `docs/reference/p031-thin-ui-inventory.json` or `docs/reference/p031-thin-ui-inventory.yaml`. The P031 gate must consume this artifact directly.

Inventory requirements:

- Governed Swift views, presenters, reducers, stores, and checked-in GraphQL documents.
- Generated GraphQL client output locations.
- Degraded/fail-closed UI files, if any, and explicit exclusions.
- Forbidden pattern groups for MCP, non-approval GraphQL mutations, command plumbing, local writes, raw truth probing, and enabled removed controls.
- Fail-closed rule for adding a governed Swift view or GraphQL operation without inventory coverage.
- Degraded/fail-closed exclusions must be explicitly inventoried and must prove they do not restore local workflow truth, local orchestration, local UI writes, MCP UI calls, or non-approval GraphQL mutations.

Initial governed surfaces include:

- `Chainworks Forge/Views/RunsHomeView.swift`
- `Chainworks Forge/Views/RunDetailPanel.swift` if present, otherwise Run Detail surface inside `RunsHomeView.swift`
- `Chainworks Forge/Views/StageDetailView.swift`
- `Chainworks Forge/Views/ApprovalGateView.swift`
- `Chainworks Forge/Views/ArtifactInspectorView.swift`
- `Chainworks Forge/Views/RunArtifactHierarchyView.swift`
- `Chainworks Forge/Views/RunReportView.swift`
- `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- `Chainworks Forge/Views/RecoverySheet.swift`
- `Chainworks Forge/Views/RunComparisonView.swift`
- `Chainworks Forge/Views/WorkflowInspectorView.swift`
- `Chainworks Forge/Views/WorkflowMapView.swift`
- `Chainworks Forge/Views/DaemonLifecycleSurface.swift`
- New P031 GraphQL read stores/reducers/presenters under `Chainworks Forge/Support` or `Chainworks Forge/Views`

### Approval Diagnostic Contract

Approval decisions are binary in P031:

- If the P072 approval-only GraphQL transport is unavailable, approval rows render diagnostic-only guidance.
- Do not render permanently disabled primary Approve or Reject buttons as the main state.
- Render an inline diagnostic banner or terminal-styled callout.
- Show `Execute via CLI` only when the operator write-path guide names CLI as the approved external workflow.
- Include copy affordances for `run_id`, `stage_id`, `approval_id`, or `diagnosticId` as available.
- If no external workflow is documented, show `Approval write path unavailable` and link to `P031-FOLLOWUP-APPROVAL-WRITE-PATH`.
- Swift must not infer approval write availability from local status strings, recovery state, or local services.

### Read Refresh Contract

The r7 Check Status recovery model is superseded because P031-owned UI issues no commands. The remaining operator-triggered refresh is a targeted GraphQL read refresh.

Rules:

- Read refresh may refetch the current run, reports list/detail, approvals queue, artifacts, stages, or visible read surface.
- Refresh feedback uses stable wording such as `Checking latest data`, `Refreshing reports`, or `Updating approvals`.
- Read refresh cannot execute MCP, GraphQL mutation, local recovery, daemon-control, or workflow mutation paths.
- If refresh returns no newer projection, the UI keeps last authoritative server values and updates freshness timestamp or stale reason.

### Source Artifact Governance

Before Swift screen migration, implementation handoff must point to exactly one checked-in governing artifact:

- either a synchronized checked-in P031 proposal containing this r19 corrected contract,
- or a checked-in implementation addendum that copies the Phase 0 obligations and explicitly supersedes stale GraphQL+MCP handoff text.

The short checked-in proposal must not be the sole implementation contract while it omits these obligations. Implementation tickets must not mix stale GraphQL+MCP acceptance text with this GraphQL-only scope.

### Phase 0 Artifact Manifest

Phase 0 must create `docs/reference/p031-phase-0-artifact-manifest.json` before Phase 0d exit and dogfood start. The manifest gives reviewers, implementers, and the P031 gate one auditable handoff list.

Required entries:

- `governing_contract`: checked-in r19 synchronized proposal or addendum
- `p043_reconciliation_evidence`: reference and gate language no longer assigning command behavior to P031 UI
- `p031_gate_evidence`: registered `proposal-031` and `p031` aliases and fail-closed checks
- `ui_inventory`: gate-consumed UI inventory JSON/YAML
- `schema_decision_record`: every visible field mapped to GraphQL or disabled/deferred state
- `operator_write_path_guide`: guide JSON with one row per removed control
- `degraded_state_evidence`: fail-closed/degraded-state runtime evidence or dated waiver
- `report_payload_priority_decision`: default P0 or evidence-backed downgrade
- `dogfood_signoff_template`: Phase 3 checklist including trigger review and critical write-path readiness or waiver status

Each row must include artifact path, revision or commit identifier when available, owner role, validation status, and blocking phase.

## UX/UI Notes

Information hierarchy:

- Runs Home remains the entry screen with run list, status, selection, filters, freshness, first-run orientation, and drill-in.
- Run Detail keeps status summary, stage progress, approvals, artifacts, report metadata, and recovery/evidence context.
- Reports list rows show payload availability before drill-in.
- Approvals queue is read/diagnostic, not an incomplete action center.

Syncing placement:

| Surface | Placement |
| --- | --- |
| Runs Home rows | Fixed transparent slot immediately after `StatusCapsule` |
| Stage rows | Fixed transparent slot immediately after `StatusCapsule` |
| Run Detail | Fixed header slot at top trailing edge, aligned with status summary |
| Artifacts | Fixed toolbar/header slot at top trailing edge |
| Reports | Fixed list header slot plus per-row reserved payload/status slot |
| Approvals | Fixed header slot; row diagnostics do not shift primary columns |

Report payload indicators:

- Reserve a 96 point trailing payload-status slot.
- Use title-case labels.
- Truncate middle only after 96 points.
- Do not wrap in compact rows.
- Use SF Symbols:
  - `available`: `doc.text.fill`, label `Payload`
  - `metadata_only`: `doc.text`, label `Metadata`
  - `payload_deferred`: `clock.badge.exclamationmark`, label `Deferred`
  - `generating`: `arrow.triangle.2.circlepath`, label `Generating`
  - `unavailable`: `exclamationmark.triangle`, label `Unavailable`

Approval diagnostics:

- Use informational diagnostic treatment, distinct from primary error alerts.
- Terminal-styled callouts may use monospace text but must match surrounding body density.
- VoiceOver labels describe diagnostic guidance, not unavailable buttons.
- The UI must avoid copy implying `Retry`, `Reissue`, `Start`, `Cancel`, `Approve`, `Reject`, or `Create` are available in-app commands.

First-run orientation:

- Dogfood mode shows a dismissible Runs Home banner on first thin-read launch.
- Banner states that the build is read-only and control actions use external workflows in the operator guide.
- Guide affordance is directly clickable or copyable.
- Dismissal is local presentation state only.

Accessibility:

- Report payload state is read as complete sentences, for example `Report payload is currently generating.`
- Disabled/deferred states expose localized operator-friendly VoiceOver hints.
- Syncing motion is subtle, only active during refresh/projection lag, and honors reduced-motion preferences.

## Implementation Plan

### Phase 0a: Dependency, Reference, Schema, and Source Governance

Owner: P031 Rust control-plane owner and P031 macOS thin UI owner  
Estimate: 1.5-2.5 days

Required work:

- Reconcile P043/P031 reference and gate language.
- Confirm every visible field has a GraphQL read path or disabled/deferred state.
- Add or confirm schema fields from the schema matrix.
- Add operator and unauthorized redaction tests for metadata, diagnostic, payload, and debug fields.
- Check in the governing r19 implementation addendum or synchronized proposal.
- Create the Phase 0 artifact manifest.

Exit: P043/P031 conflict is resolved, server read schema/redaction decisions are executable, and one governing artifact is linked before Swift migration.

### Phase 0b: UI Inventory and Write-Path Removal Guards

Owner: P031 macOS thin UI owner  
Estimate: 1-1.5 days

Required work:

- Check in the machine-readable P031 UI inventory.
- Add static guards for UI MCP imports/calls, non-approval GraphQL mutations, local writes, command receipts, command correlation, and identity-to-MCP mapping.
- Add one negative test per removed write control.
- Prove any degraded/fail-closed UI code remains read-only, keeps control-plane-owned truth authoritative, and cannot restore local orchestration or local writes.

Exit: P031/P072 gates fail closed for UI MCP usage, non-approval GraphQL mutations, command plumbing, local write fallback, raw truth probing, and out-of-inventory governed surfaces.

### Phase 0c: Swift GraphQL-Only Boundary and Test Doubles

Owner: P031 macOS thin UI owner  
Estimate: 1-2 days

Required work:

- Introduce GraphQL read clients, subscription clients, stores, reducers, presenters, freshness constants, and read-only test doubles.
- Register `proposal-031` and `p031` aliases in `scripts/test-gate.sh`.
- Add reducer tests for freshness, targeted read refresh, disabled/deferred reasons, projection lag, authorization, report payload availability, approval diagnostics, and first-run orientation.

Exit: Thin-read mode fails closed unless GraphQL read contracts and UI write-removal guards are green.

### Phase 0d: Operator Guide, UX Sign-Off, Degraded-State Evidence, and Freshness Baseline

Owner: P031 release owner  
Estimate: 1-1.5 days

Required work:

- Publish `docs/reference/p031-operator-write-path-guide.json`.
- Validate one approval diagnostic and one non-approval removed-control workflow against copied UI identifiers.
- Complete UX review of Syncing placement, approval diagnostics, first-run orientation, report payload indicators, density, and accessibility.
- Measure representative GraphQL projection freshness p50/p95.
- Capture fail-closed/degraded-state runtime evidence or attach a dated release-owner waiver.
- Record report payload priority as default P0 or evidence-backed downgrade.
- Add the Phase 3 sign-off checklist.

Exit: Operator guide, UX sign-off, freshness measurement, degraded-state evidence/waiver, and Phase 1 go/no-go are attached.

### Phase 1: Read-Only Thin Screens

Owner: P031 macOS thin UI owner  
Estimate: 3-5 days

Required work:

- Add GraphQL-backed stores for Runs Home, Run Detail, stages, approvals, artifacts, report metadata, and daemon lifecycle banner.
- Render freshness and projection lag with the shared Syncing pattern.
- Add active targeted read-refresh feedback.
- Add Reports list payload availability indicators.
- Replace approval primary buttons with diagnostic banner/callout.
- Add first-run dogfood orientation banner.
- Keep any degraded/fail-closed path read-only; do not retain or restore the old local execution path.

Exit: Read surfaces render from GraphQL or are explicitly disabled/deferred; no governed thin-mode screen reads or writes workflow truth locally.

### Phase 2: Local Truth and Write-Control Teardown

Owner: P031 macOS thin UI owner  
Estimate: 1-2 days

Required work:

- Remove direct local service calls from P031-owned screens.
- Remove SwiftData production truth from P031-owned screens.
- Remove MCP command clients, command receipt paths, identity-to-MCP adapters, and GraphQL mutation paths from governed UI code.
- Retain only presentation state, read-refresh state, and server-derived caches.

Exit: No P031-owned production screen can decide or mutate workflow truth locally, through MCP, or through GraphQL mutation.

### Phase 3: Dogfood, Release, and Flag Removal

Owner: P031 release owner  
Estimate: 1-2 days

Required work:

- Run same-tree prerequisite gates for P027, P041, P042, reconciled P043, and P031.
- Run two full-mvp-live dogfood runs in GraphQL-only thin UI mode for the assumed one-to-three-operator internal population.
- Capture operator workflow-completion notes after each run.
- Capture degraded-state recovery and approval diagnostic evidence at least once.
- Capture targeted refresh feedback, Reports payload indicators, metadata inspection, accessibility spot check, projection correctness, freshness, degraded-state evidence/waiver, and fail-closed readiness.
- Review additional-evidence triggers before sign-off.
- Do not treat degraded-state evidence as optional while critical write paths remain outside the macOS UI.

Exit: Release handoff includes gate results, dogfood evidence, operator outcome notes, sign-off, hold/degraded-state status, metrics, and any dated waiver or follow-up needed for unavailable write workflows.

## Rollout

Modes:

| Mode | Reads | Writes | Use |
| --- | --- | --- | --- |
| `thin-read` | GraphQL read models | Removed, hidden, or diagnostic-only | P031 release mode |
| `dogfood` | GraphQL read models | Same as thin-read | Two-run internal dogfood |

Degraded behavior is a UI state, not a separate truth owner or runtime mode. Affected surfaces may show stale/unavailable diagnostics or hide unavailable controls, but they still treat control-plane-owned GraphQL readback as authoritative and keep writes external or unavailable.

Operator write-path guide:

- Required before Phase 0d exit and dogfood start.
- Covers 100 percent of removed write controls.
- Gate-consumed JSON is the source of truth; Markdown may be generated for operators.
- Each row includes:
  - `removed_control_id`
  - `removed_control_label`
  - `external_workflow_kind`: MCP terminal, CLI, automation, non-P031 UI, or temporarily unavailable
  - `external_workflow_name_or_tool`
  - required identifiers exposed by GraphQL/UI copy affordances
  - minimum parameter shape or unavailable reason
  - expected success output or follow-up id
  - operator notes and validation status

Dogfood evidence minimum:

- Two full-mvp-live runs in dogfood mode.
- Per-run operator workflow-completion note.
- Approval queue readback plus diagnostic-only comprehension observation.
- At least one degraded-state recovery such as daemon restart or projection lag.
- Targeted read-refresh active feedback.
- Reports list payload availability evidence and report metadata inspection.
- Accessibility spot check.
- Projection correctness and GraphQL freshness p50/p95.
- Degraded-state/fail-closed evidence or dated waiver.
- Operator guide rows validated against copied UI identifiers for at least one approval diagnostic and one removed-control workflow.
- Report payload priority decision.
- Phase 3 trigger review.

Additional-evidence triggers:

- More than three distinct operators use the thin UI before Phase 3 sign-off.
- Dogfood covers more than one workflow family beyond full-mvp-live.
- A new approval shape, degraded daemon state, report payload state, or projection lag failure appears.
- Release owner expands availability beyond the initial internal group.

Hold criteria:

- Any prerequisite P027/P041/P042/reconciled-P043 gate is red on the same tree.
- P043/P031 reference language still assigns command behavior to P031-owned UI.
- Governing r19 addendum or synchronized proposal is not checked in before implementation handoff.
- Governed UI imports/invokes MCP, defines/executes GraphQL mutation, calls local mutation paths, constructs command plumbing, or probes raw truth.
- Any removed write control remains enabled without a separate approved transport.
- Reports list lacks payload availability status.
- Operator write-path guide is missing before dogfood.
- UI inventory is missing or not consumed by the gate.
- Degraded-state/fail-closed evidence has no quantitative result or waiver before dogfood evidence acceptance.
- Report payload priority is not recorded by Phase 0d.
- Phase 3 sign-off does not review additional-evidence triggers.

Degraded-state criteria:

- GraphQL read model diverges from server projection/canonical truth.
- Daemon lifecycle causes repeated unavailable state on normal launch.
- Operator can trigger a local mutation, MCP command, or GraphQL mutation from a governed screen.
- App is continuously unavailable for two minutes in normal dogfood conditions.
- Targeted read refresh fails to update freshness state or visibly complete under normal daemon conditions.
- Dogfood run is blocked by missing write-path guidance or misunderstood approval diagnostics.
- Degraded-state handling exceeds 60 seconds to visible affected-surface disablement/degradation, restores local workflow truth, or leaves stale/conflicting truth authoritative.
- Copied UI diagnostic identifiers do not match the external workflow guide.

Fail-closed action: degrade affected thin UI surfaces, hide unavailable write affordances, preserve last authoritative control-plane/GraphQL readback only when clearly marked stale, and direct operators to the write-path guide or unavailable follow-up. Fail-closed behavior must not restore the old Swift orchestrator, local workflow truth, MCP UI calls, non-approval GraphQL mutations, or local UI writes.

Degraded-state simplification:

- Degraded/fail-closed behavior may be simplified after Phase 3 sign-off only if dogfood shows affected surfaces fail closed, operators can continue through documented external workflows or explicit unavailable follow-ups, and the P031 release owner records the decision.
- Critical write-path readiness means merged, reviewed, gate-green restoration or replacement of approval resolution and at least one operationally critical run-control workflow, without using P031-owned UI MCP or non-approval GraphQL mutations.
- A follow-up proposal being drafted is not sufficient to claim operator viability.
- Waiver must name unavailable paths, accept the operator gap, and set a hard write-restoration deadline.

Degraded-state evidence success:

- Affected thin UI surfaces visibly enter disabled/degraded state within 60 seconds from the triggering degraded condition under normal local dogfood conditions.
- No projection data loss is caused by entering degraded state.
- No stale GraphQL-only truth remains visible as authoritative after entering degraded state.
- No local orchestrator, local workflow truth, MCP UI call, GraphQL mutation, or local UI write becomes reachable.
- Participating operator confirms Runs Home/Run Detail remain usable as read-only/degraded control-plane views or clearly unavailable with actionable external guidance.
- Failure blocks dogfood sign-off unless a dated waiver and mitigation are attached.

## Metrics

Core compliance:

- GraphQL read ownership coverage: 100 percent of P031 visible workflow fields source from GraphQL or are disabled/deferred.
- P043/P031 reconciliation: 0 P031 UI command-completion, command receipt, command correlation, or MCP control obligations remain.
- Projection correctness: 0 parity divergences in gate runs or dogfood.
- UI MCP usage: 0 governed UI MCP imports, clients, wrappers, tool calls, command receipts, command correlation, or MCP serializers.
- GraphQL mutation usage: 0 non-approval GraphQL mutations defined or invoked by governed UI code; P072 `approveApproval` / `rejectApproval` are the only allowed exception.
- Removed write controls: 0 enabled removed write controls in governed screens unless a separate approved transport exists.

Operator viability:

- Operator guide coverage: 100 percent removed-control mapping before dogfood.
- Operator guide contract completeness: 100 percent rows include workflow/tool or follow-up, identifiers, parameter shape/unavailable reason, expected output, and validation status.
- Operator workflow viability: 2 of 2 dogfood runs include workflow-completion notes stating whether the run completed without improvised workarounds.
- Approval diagnostic comprehension: at least one approval encounter shows the operator understood diagnostic-only guidance without external help.
- Dogfood edge coverage: degraded-state recovery, approval diagnostics, Reports payload status, targeted read refresh, and metadata-only report inspection.

Experience quality:

- GraphQL freshness baseline: dogfood includes p50 and p95 projection freshness.
- Targeted read-refresh feedback: 100 percent refreshes visibly enter active state and complete without mutation paths.
- Report payload visibility: 100 percent Reports rows show payload availability or metadata-only/deferred state before drill-in.
- Time to usable Runs Home: 95 percent foreground launches reach live or stale-but-visible Runs Home within 2 seconds under normal daemon conditions.
- Syncing visual stability: 0 per-field spinners and 0 layout shifts from refresh/projection lag.
- Disabled reason accessibility: 100 percent visible diagnostic/disabled states expose localized VoiceOver hints.

Release safety:

- Degraded-state readiness: evidence shows affected surfaces degrade within 60 seconds, pass consistency assertions, prove no local orchestration/write path is restored, and record operator confirmation or a dated waiver.
- Local-orchestrator non-regression: 0 fail-closed paths restore local workflow truth, local UI writes, MCP UI calls, or non-approval GraphQL mutations.
- Report payload priority decision: Phase 0d records default P0 or evidence-backed downgrade.
- Phase 3 trigger review: 100 percent additional-evidence triggers reviewed at sign-off.
- Phase 0 artifact manifest completeness: 100 percent required entries have path, owner role, validation status, and blocking phase.

## Risks and Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| P043 reference language keeps imposing command/control obligations on P031 UI | Implementers satisfy stale docs while violating GraphQL-only scope | P043/P031 reconciliation is a Phase 0a exit gate and P031 gate requirement |
| UI grows hidden MCP control path | App becomes a second control surface | Static guards fail on MCP imports/calls, identity mapping, receipts, correlation, and payload construction |
| Non-approval GraphQL mutations are added to compensate for removed MCP UI writes | UI still mutates workflow truth outside P072 | P031/P072 gates fail on forbidden mutation definitions/invocations in governed UI |
| Static guards miss governed UI surfaces | Local truth remains reachable | Machine-readable UI inventory is gate-consumed and fail-closed |
| Approval diagnostics look like broken primary actions | Operators stall or lose trust | Diagnostic banner/callout replaces disabled primary buttons and dogfood captures comprehension |
| Operators cannot complete write workflows during dogfood | Dogfood validates rendering but not viability | Operator write-path guide maps every removed control and dogfood validates copied identifiers |
| Report metadata-only behavior is a regression | Operators leave the UI for frequent report inspection | Payload availability visible before drill-in; full payload follow-up defaults to P0 unless evidence supports downgrade |
| Implementation follows a shorter checked-in proposal | Phase 0 obligations are skipped | One checked-in governing addendum or synchronized proposal is required before handoff |
| Degraded state is interpreted as restoring the old Swift orchestrator | App regains a second workflow-truth owner and reintroduces local write risk | P031 defines degraded state as read-only control-plane-owned behavior only; static guards continue to reject local orchestration and UI writes |
| Degraded-state evidence becomes a checkbox | Release claims readiness without proof | 60-second target, consistency assertions, no-local-write assertions, and operator confirmation are pass/fail criteria |
| Operator guide rows and UI identifiers drift | External workflows fail in practice | Guide JSON is versioned and validated against copied identifiers |
| Observer diagnostic scope expands accidentally | Diagnostic/debug data leaks | Diagnostic/debug fields are operator-only by default; observer behavior is deferred to separate auth policy |

## Feedback Resolution

This section resolves reviewer feedback explicitly. No disagreement is hidden.

### Cross-Reviewer Disagreements

| Topic | Decision | Trade-off |
| --- | --- | --- |
| Restore UI writes through MCP? | No. P031 UI is GraphQL-read-only. MCP remains outside governed UI. | Operators temporarily use external workflows, but P031 avoids an ambiguous second control surface. |
| Approval decisions in P031? | Diagnostic-only unless a separately approved non-MCP, non-GraphQL transport lands. | Approval completion is deferred, but the UI no longer teases unavailable actions. |
| Does P031 retain an old local execution path after the control-plane DB becomes authoritative? | No. P031 degraded states are read-only behavior over control-plane-owned truth, with external write workflows or explicit unavailable follow-ups. | Operators do not regain old in-app local writes, but the architecture avoids reintroducing a second workflow-truth owner. |
| Report payload priority? | Full payload rendering is outside P031 but defaults to P0 follow-up unless evidence supports downgrade. | Proposal stays read-migration focused while preventing silent deprioritization. |
| Observer-visible diagnostics? | Operator-only by default; observer behavior deferred. | Avoids accidental auth expansion; future observer diagnostics need a separate decision. |
| Prose-only readiness? | No. Phase 0 requires gate-consumed inventory, guide JSON, manifest, and schema decisions. | More Phase 0 artifacts, less implementation drift. |

### Issue Matrix

| Issue | Status | Resolution |
| --- | --- | --- |
| `OPERATOR-REJECTION-STATE-6-01` | addressed | r7 approval is stale; implementation approval requires aggregate re-review of the corrected GraphQL-only scope. |
| `ARCH-R9-01`, `LIFT-R9-001` | addressed | P043/P031 reconciliation removes command-completion, command receipts, command correlation, and MCP command-control from P031 UI. |
| `ARCH-R9-02`, `LIFT-R9-003`, `ARCH-R10-03` | addressed | Schema matrix must become executable GraphQL or explicit disabled/deferred state before affected migration. |
| `ARCH-R9-03`, `LIFT-R9-004`, `ARCH-R10-04` | addressed | Machine-readable UI inventory is required and consumed by the P031 gate. |
| `ARCH-R9-04`, `UX-R9-01`, `UI-02`, `PO-R9-04`, `LIFT-R9-002`, `LIFT-R9-007` | addressed with explicit deferral | Approval UI is diagnostic-only; interactive decisions move to a separate follow-up transport. |
| `ARCH-R9-05`, `ARCH-R10-01` | addressed | One checked-in governing artifact must supersede stale GraphQL+MCP handoff text before migration. |
| `ARCH-R10-02` | addressed | P031 gate registration and P043 reconciliation are Phase 0 blockers. |
| `ARCH-R10-05`, `PO-R9-01`, `UX-R9-02`, `PO-R10-04`, `LIFT-R9-005`, `LIFT-R9-008` | addressed with open dependency | Operator guide JSON is a gate-consumed contract; exact external recipes remain OQ-031-01 and block dogfood until authored. |
| `ARCH-R10-06` | addressed | Diagnostic/debug fields are operator-only by default; observer behavior is deferred. |
| `PO-R9-02`, `PO-R9-05`, `PO-R10-05`, `LIFT-R9-006` | addressed | Dogfood includes workflow-completion notes, edge coverage, approval comprehension, and trigger review. |
| `PO-R9-03`, `LIFT-R9-012` | addressed | Follow-ups have priorities and start expectations; operator viability still requires evidence or waiver. |
| `PO-R10-01` | addressed | The old local Swift-orchestrator path is removed from scope; degraded states remain control-plane-owned and read-only. |
| `PO-R10-02` | addressed | Report payload follow-up defaults to P0 unless Phase 0d evidence supports downgrade. |
| `PO-R10-03` | addressed | Degraded-state evidence has quantitative 60-second, consistency, no-local-write, and operator-confirmation criteria. |
| `UI-01`, `LIFT-R9-009` | addressed | Syncing placement is fixed for list and detail surfaces. |
| `UI-03`, `LIFT-R9-010` | addressed | Report payload indicators specify SF Symbols, labels, stable width, and truncation rules. |
| `UX-R10` | addressed | Guide access is clickable/copyable, identifiers align with guide rows, and Reports VoiceOver labels are complete sentences. |
| `UI-04` | addressed | Approval diagnostics are visually distinct from primary errors. |
| `UI-05` | addressed | Syncing motion is subtle and reduced-motion-aware. |
| `UI-06` | addressed | First-run banner has clear dismissal and local presentation-state persistence. |

## Open Questions

| ID | Question | Why Open | Blocking Phase |
| --- | --- | --- | --- |
| `OQ-031-01` | Which exact external workflows should the operator write-path guide name for each removed write control? | Input artifacts define the guide schema but not the actual CLI/MCP/automation recipes. | Phase 0d exit and dogfood start |
| `OQ-031-02` | Who are the named P031 macOS thin UI owner and P031 release owner? | Staffing assignments are not present in input artifacts. | Phase 0 start |
| `OQ-031-03` | What measured p95 GraphQL projection freshness should be used for dogfood readiness? | Must be captured from representative runtime conditions. | Phase 0d exit and dogfood sign-off |
| `OQ-031-04` | Will the checked-in proposal be expanded or receive a concise implementation addendum? | P031 requires one checked-in governing artifact but this run-local output does not choose the repo documentation strategy. | Implementation handoff before Phase 1 |
| `OQ-031-05` | Does usage evidence justify keeping report payload restoration below P0? | No report-inspection frequency data exists yet; default is P0. | Phase 0d exit and Phase 3 flag removal decision |

## Follow-Ups

These historical P031 follow-ups are retained for traceability. New work should be launched through P032, P036, or a dedicated write-path proposal rather than by reopening P031.

| ID | Priority | Expected Start | Description |
| --- | --- | --- | --- |
| `P031-FOLLOWUP-APPROVAL-WRITE-PATH` | P0 immediate next proposal | Before P031 Phase 3 flag removal decision | Define approved non-MCP, non-GraphQL-mutation approval decision transport if interactive approvals must return to macOS UI. |
| `P031-FOLLOWUP-UI-CONTROL-SURFACE` | P1 | Draft before restoring any in-app write affordance | Propose any future start/cancel/retry/create UI control surface with explicit transport and safety model. |
| `P031-FOLLOWUP-REPORT-PAYLOAD` | P0 by default; downgrade only with Phase 0d evidence | Priority recorded before Phase 0d exit | Add server-owned GraphQL report payload readback and full payload UI rendering. |

## Acceptance Packets

Implementation approval re-entry:

- Active proposal revision states GraphQL-read-mostly UI with no UI MCP, no non-approval GraphQL mutations, no local writes, no command receipts, and no command correlation.
- Feedback coverage includes rejected `state_6_implementation_approval` context.
- Aggregate re-review completed against this GraphQL-only scope.
- Any implementation approval references the new re-review decision, not stale r7 approval.

Phase 1 Swift migration entry:

- One checked-in r19 addendum or synchronized proposal supersedes stale GraphQL+MCP handoff text.
- `proposal-031` and `p031` gates are registered, documented, and fail closed.
- P043/P031 text no longer assigns command behavior to P031 UI.
- Machine-readable UI inventory is consumed by the gate.
- Every visible field for the target screen has GraphQL source or disabled/deferred state.

Dogfood start:

- Operator guide JSON has 100 percent removed-control coverage and validation status.
- Approval diagnostics and one non-approval removed-control workflow are validated against copied identifiers.
- First-run orientation, report indicators, Syncing slots, diagnostic banners, and complete-sentence VoiceOver labels are implemented or explicitly outside the dogfood surface.
- Degraded-state/fail-closed evidence is attached or a dated waiver exists.
- Report payload priority is recorded.

Post-dogfood write-path readiness:

- Critical write-path readiness is merged, reviewed, gate-green, and documented for approval resolution plus at least one operationally critical run-control workflow, or a dated waiver exists.
- Waiver names unavailable paths, accepts the gap, and sets a hard write-restoration deadline.
- Phase 3 sign-off reviews all additional-evidence triggers.
- Phase 0 artifact manifest links sign-off, degraded-state, report-priority, and readiness/waiver evidence.

## Final Recommendation

Stop P031 here. Treat the landed GraphQL-only read boundary and read-model guardrails as the durable P031 outcome, then move the remaining work to follow-up proposals:

- P032: stabilization, dogfood/readiness evidence, degraded-state drills or waivers, daemon/schema lifecycle polish, freshness metrics, and documentation cleanup.
- P036: visual parity and navigation consolidation over the GraphQL read model.
- Separate write-path proposals: any future in-app create/start/cancel/retry/approve controls.

Do not request another P031 audit to close product polish gaps. Do not restore the old local Swift-orchestrator behavior, local UI writes, UI MCP calls, or non-approval GraphQL mutations.
