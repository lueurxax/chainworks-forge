# Proposal 024 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Proposal 024 so `Runs Home` and `Idea` use segmented run shells, workflow-map truth is explicitly assigned, live timeline becomes a focused subordinate surface, and artifact browsing uses one canonical hierarchy without creating a second report/export authority lane.

**Architecture:** Add shared run-surface infrastructure first: a canonical artifact hierarchy builder plus reusable hierarchy/timeline views and pane-routing helpers. Then refactor `RunsHomeView` and `WorkflowRunProgressView` to consume those shared primitives with context-specific pane sets and state-priority routing. Existing persisted `Artifact` / `Run` report authority remains unchanged; new hierarchy is a projection for browsing only.

**Tech Stack:** SwiftUI, SwiftData, Swift Testing, existing `WorkflowMapProjectionService`, existing report/export/recovery surfaces.

---

### Task 1: Shared Artifact Hierarchy And Timeline Infrastructure

**Files:**
- Create: `Chainworks Forge/Models/RunArtifactHierarchy.swift`
- Create: `Chainworks Forge/Engine/RunArtifactHierarchyBuilder.swift`
- Create: `Chainworks Forge/Views/RunArtifactHierarchyView.swift`
- Create: `Chainworks Forge/Views/RunTimelineInspectorView.swift`
- Modify: `Chainworks Forge/Views/WorkflowMapView.swift`
- Test: `Chainworks ForgeTests/RunArtifactHierarchyBuilderTests.swift`

- [ ] Define canonical hierarchy value types that preserve artifact authority metadata (`reportKind`, `reportVersion`, supersession, promoted state).
- [ ] Write failing builder tests covering stage grouping, agent grouping, semantic buckets, promoted artifacts, and report metadata preservation.
- [ ] Implement deterministic hierarchy builder over persisted `Artifact` + `Run` truth.
- [ ] Extract a reusable artifact hierarchy browser view with promoted row, filters, and leaf selection callback.
- [ ] Extract a focused timeline inspector view that renders timeline only and is subordinate to workflow-map usage.
- [ ] Refactor `WorkflowMapView` so topology, agents, handoffs, telemetry, and timeline can be shown selectively instead of always stacked.

**Verification:**
- Run: `xcodebuild test -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -only-testing:Chainworks_ForgeTests/RunArtifactHierarchyBuilderTests`
- Run: `xcodebuild test -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -only-testing:Chainworks_ForgeTests/WorkflowMapProjectionTests`

### Task 2: `Runs Home` Segmented Run Detail Shell

**Files:**
- Modify: `Chainworks Forge/Views/RunsHomeView.swift`
- Maybe Create: `Chainworks Forge/Views/RunSurfaceSharedComponents.swift`
- Test: `Chainworks ForgeTests/Proposal024RunSurfaceTests.swift`

- [ ] Keep the existing run list and operator badges intact.
- [ ] Replace `RunDetailPanel` long-scroll layout with pane-based shell: `Summary`, `Flow`, `Artifacts`, `Diagnostics`.
- [ ] Make `Flow` own workflow-map-derived topology, chips, handoffs, telemetry, and detached timeline entry.
- [ ] Keep `Summary` compact and action-oriented, with compare/report/recovery/stop-cancel entry points.
- [ ] Route delivery/export actions to one explicit shell-owned path after metadata demotion.
- [ ] Use shared artifact hierarchy view in `Artifacts` without changing report/export authority.
- [ ] Use the focused timeline surface from `Flow` / `Diagnostics`, not inline by default.

**Verification:**
- Run: `xcodebuild test -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -only-testing:Chainworks_ForgeTests/Proposal024RunSurfaceTests`

### Task 3: `Idea` Run Progress Segmented Shell And Priority Routing

**Files:**
- Modify: `Chainworks Forge/Views/IdeaListView.swift`
- Test: `Chainworks ForgeTests/Proposal024RunSurfaceTests.swift`

- [ ] Replace the current list-oriented `WorkflowRunProgressView` with context-specific panes: `Summary`, `Progress`, `Artifacts`, `Approvals`.
- [ ] Keep next action and run control above the fold in `Summary`.
- [ ] Make `Progress` own workflow-map-derived active execution context without inlining all sections.
- [ ] Make `Approvals` the foreground path for `waitingApproval`, including direct deep links from approval-driven opens.
- [ ] Ensure blocked/failed states foreground recovery-critical summary context and preserve one-step recovery entry.
- [ ] Reuse the shared artifact hierarchy browser for `Artifacts`.

**Verification:**
- Run: `xcodebuild test -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -only-testing:Chainworks_ForgeTests/Proposal024RunSurfaceTests`

### Task 4: End-To-End Acceptance Proof

**Files:**
- Modify as needed based on failed verification

- [ ] Re-read Proposal 024 acceptance criteria and map each item to concrete code.
- [ ] Run targeted tests for hierarchy, workflow-map, and proposal 024 shell behavior.
- [ ] Run a macOS build for the app target.
- [ ] If verification fails, fix the specific regression and re-run the failing command before moving on.

**Verification:**
- Run: `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" build`
- Run: `xcodebuild test -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -only-testing:Chainworks_ForgeTests/RunArtifactHierarchyBuilderTests -only-testing:Chainworks_ForgeTests/WorkflowMapProjectionTests -only-testing:Chainworks_ForgeTests/Proposal024RunSurfaceTests`

