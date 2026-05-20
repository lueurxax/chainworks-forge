# Proposal 036 UX Consolidation Evidence

Phase 1 foundational coverage is asserted by `Proposal036UXConsolidationTests`,
`Proposal031ThinGraphQLReadBoundaryTests`, and `RunTimelineInspectorViewTests`
under `./scripts/test-gate.sh proposal-036`. Phase 2.5 dogfood and Phase 4
remote UI/accessibility evidence are still pending; the standalone Approvals
top-level tab has been removed, and old `Approvals` routes redirect to Runs
focused on the waiting-approval lane.

## Navigation Shell Parity
- [x] Runs tab reachable
- [x] Ideas tab reachable
- [x] Definitions tab reachable
- [x] Settings tab reachable

## Definitions Segmented Wrapper
- [x] Agents segment functional
- [x] Workflows segment functional

## Runs Workbench Presentation Model
- [x] Attention lanes normalized
- [x] Inline approvals rendered
- [x] Deferred states handled

## Live Timeline
- [x] Dogfood flag respected
- [x] Motion reduction respected

## Pending Evidence
- [ ] Phase 2.5 dogfood readability rating (≥5 task-set runs, ≥2 operators)
- [ ] Phase 4 remote UI smoke + accessibility coverage
- [x] Approvals tab removal parity checklist (Phase 2c)

## Refinement Notes
- Old-route compatibility extends to `chainworks://` deep links (`runs`, `ideas`, `definitions`, `settings`, `approvals`). The `approvals` host posts `chainworksFocusWaitingApprovalLane` so Runs focuses the waiting-approval lane after routing.
- Cross-tab return from blocked/failed Runs to Settings System Readiness is implemented via the `chainworksOpenSystemReadiness` notification.
- Ideas surface no longer exposes `New Idea`, `Archive`, `Workspace`, or `Start New Run` write controls in Swift UI, matching the proposal's command-boundary resolution. The Ideas metadata "Workspace" row routes `workspaceRootPath` through the shared `redactedPath` helper so absolute filesystem paths outside `$HOME` collapse to `<redacted>` (P036-SEC-001/M4), and the legacy `completedExportHub` direct surface is gated behind `#if DEBUG` so release builds fall back to `RunsHomeView`.
- `RunsWorkbenchPresentationModel` maps P085 disabled reason codes (`unauthorized`, `staleRead`, `projectionLag`, `redacted`, `conflict`, `duplicate`, `alreadyResolved`, `writePathNotAvailable`, `managedOutsideUI`, `unsupportedAction`, `ambiguousApprovalIdentity`) to `P036DeferredState` values, and substitutes a generic "Redacted — details unavailable" message in redacted states.
- Settings System Readiness includes Diagnostics & Configuration Paths (with `$HOME` redaction and `<redacted>` fallback for non-`$HOME` absolute paths), Provider Health, an Actionable Runs deep link list, and a "Show Diagnostics Detail" toggle gating raw PID readout.
- Lane classification uses a typed `P036RunLane` derived from the typed `RunStatus` vocabulary plus pending-approval count; unknown server statuses fall through to a `deferred` "Status Unknown" lane rather than being heuristically bucketed as completed/blocked.
- UI-side operational counters (`P036UICounters`) accumulate `p036_tab_route_resolution_total`, `p036_inline_approval_render_total`, `p036_timeline_batch_flush_total`, `p036_artifact_payload_state_total`, `p036_projection_gap_deferred_total`, and `p036_global_attention_indicator_total` from the surfaces that emit them; counters are process-scoped and complement engine-side `MetricsCollector` rollups.
- Approval rows preserve P036-SEC-001/M2/M3/M4 redaction: when the deferred state is `redacted`, body text and the upstream accessibility label are replaced with a generic message so VoiceOver and visual surfaces cannot leak redacted approval detail.
- P036-SEC-004 fail-closed freshness mapping: `refreshing` falls through to `.unavailable` and `unknown(rawValue:)` falls through to `.unsupported`, so unknown server-side freshness values render an explicit banner with disabled approve/reject instead of an inert row.
- `testNavigationTabTargetParity` asserts that `ContentView.Tab` exposes exactly four cases (`runs`, `ideas`, `definitions`, `settings`) and that legacy `Approvals`/`approvals` deep links route to `.runs` while `Pilot Readiness` routes to `.settings`.
- `RunStatus.from(serverValue:)` parses both Swift `camelCase` and Rust `snake_case` (e.g. `waiting_approval`) so the typed `P036RunLane` vocabulary remains intact across the GraphQL boundary instead of falling back to substring heuristics.
- `P031GraphQLReadRequest.isAllowedApprovalMutationDocument` now rejects documents that declare more than one mutation operation and verifies the root selection set contains exactly one of `approveApproval` or `rejectApproval`, closing a multi-operation allowlist bypass against the approval mutation exception.
- `RunsWorkbenchPresentationModel` consolidates the additional read-model presentations it now publishes — `reportRows`, `approvalInbox`, `ideaContext`, `catalogContext`, `closeoutReadiness` (P077), `implementationCompletion` (P088), and `sideEffectReadback` (P078) — and enriches `SummaryHeader` (workflow, progress, pending approvals, rollout decision summary, refresh feedback, freshness) and `StageMap` cards (attempt text, started/completed labels, duration, evidence labels, artifact count) so SwiftUI views no longer interpret raw projection data.
- PC-003 routing race fix: in addition to posting `chainworksFocusWaitingApprovalLane`, `ContentView` sets `RunsWorkbenchPresentationModel.pendingFocusWaitingApprovalLane = true` before tab-switching from a legacy `Approvals` route. `RunsHomeView` consumes the flag on mount (`onChange(initial: true)`), so the focus survives the render cycle even when the notification fires before the view is in the hierarchy. This applies to the env-var initial-tab path, the `chainworksSelectTab` notification path, and the `chainworks://approvals` deep-link path.
- `CHAINWORKS_UI_TEST_INITIAL_TAB` segment routing: `workflowInspector`/`Workflow Inspector` initializes `DefinitionsView` on the Workflow segment; `agentCatalog`/`Agent Catalog` initializes the Agents segment. The same mapping applies to `chainworksSelectTab` notifications so test fixtures and runtime routing stay symmetric.
