# Proposal 012: UI Quality Audit and Visual Polish Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/012-ui-quality-audit-and-visual-polish.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `9b544e2` |
| Working Tree | `dirty (62 modified, 5 untracked)` |
| Audited At | `2026-03-29T12:00:25+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Draft` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Ready with Risks` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 012 is materially further along than R2. The current same-`HEAD` approved-host `ui-smoke` gate is green, the same-`HEAD` approved-host `proposal-006` gate is green for `ProviderSettingsView`, `PilotReadinessView`, and `FirstRunSetupWizard`, the local macOS build passes, and preview-backed core surfaces render cleanly under Xcode Preview. The remaining blockers are no longer host-policy problems or obvious surface regressions. They are proposal-sign-off gaps inside Section 6 itself: no executed `1024×768` min-window audit, no executed Differentiate Without Color Alone / Increase Contrast / Reduce Transparency / VoiceOver / focus-order proof for the bounded adopter slice, and no current runtime proof for some Appendix A secondary surfaces (`GooseProviderConnectionAssistantView`, `WorkflowMapView`, `ReleaseGateView`) beyond preview/code evidence.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Section 6 verification is still broader than the proof now on hand | High |
| Architecture | Acceptable | no new architecture-specific blocker surfaced beyond the rollout/proof sequencing gaps already captured in `REQ-005` and `REQ-006` | Medium |
| Product | Acceptable | the shell still exposes separate `Pilot Readiness` and `Settings` tabs despite the in-file merge comment | Medium |
| UI | Acceptable | primary shell/provider surfaces are runtime-proven, but some preview-owned secondary surfaces still lack same-round runtime proof | Medium |
| UX | At Risk | accessibility-settings and VoiceOver/focus-order proof for the bounded adopter slice are still missing | High |
| Readiness | Ready with Risks | approved-host runtime proof is now green where it exists, but the proposal’s explicit min-window and accessibility sign-off bar is still not fully executed | High |

## Proposal Contract

### Scope

- Improve readability, density, and visual hierarchy across the Appendix A macOS operator surfaces.
- Land a bounded shared design-system slice (`DesignTokens`, `StatusCapsule`, typography/spacing helpers, empty-state treatment).
- Standardize surface-local feedback and interaction treatment without broad engine/navigation changes.

### Locked Decisions

- Proposal 012 is a macOS-only UI quality slice, not a runtime-contract rewrite.
- Shared primitives are supposed to roll out through a bounded first-adopter slice before broader expansion.
- Existing keyboard behavior and `accessibilityIdentifier` stability must be preserved.
- Runtime screenshots and live interaction proof move into implementation evidence review rather than remaining implicit in proposal text.

### Primary User Flows

- Scan `RunsHomeView` without severe truncation and act from an above-the-fold `RunDetailPanel`.
- Configure providers/bootstrap via `ProviderSettingsView`, `PilotReadinessView`, and `FirstRunSetupWizard`.
- Operate approval/release/recovery/start-run flows with stable keyboard ownership and status semantics.
- Understand run topology and release state through `WorkflowMapView` and `ReleaseGateView`.

### UI Commitments

- Reduce sidebar truncation and density issues.
- Replace fragmented badge styling with shared status capsules and semantic tokens.
- Add clearer empty states, journey/progress indicators, hero banners, disclosure groups, and above-the-fold actions.
- Convert the New Idea sheet to a macOS-appropriate `Form`.

### UX Commitments

- Modal/operator flows must keep high-value confirm/dismiss behavior.
- Async loading/success/failure treatment should stay close to the initiating surface.
- Status communication must not rely on color alone.
- The bounded adopter slice must preserve labels, traits, focus order, and contrast behavior before broader rollout.

### Acceptance Criteria

- Current-`HEAD` readability issues on the named surfaces are corrected.
- Shared tokens and status-capsule primitives exist and are adopted.
- Secondary polish items land where promised.
- Section 6 verification and implementation-evidence handoff are actually executed, not merely assumed.

## Track 1: Objective Proposal-Conformance Audit

### REQ-001 Current-`HEAD` readability fixes on the named operator surfaces are implemented
- Proposal Source: `1. Context`, `Phase 1`, `Appendix A` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:20`, `docs/proposals/012-ui-quality-audit-and-visual-polish.md:490`, `docs/proposals/012-ui-quality-audit-and-visual-polish.md:563`)
- Status: Implemented
- Evidence Type: code, screenshot, tests-run
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:127`
  - `Chainworks Forge/Views/RunsHomeView.swift:526`
  - `Chainworks Forge/Views/IdeaListView.swift:173`
  - `Chainworks Forge/Views/IdeaListView.swift:379`
  - `Chainworks Forge/Views/ReleaseGateView.swift:153`
  - `Chainworks Forge/Views/ForegroundBannerView.swift:68`
  - Xcode Preview renders passed for `RunsHomeView`, `ProviderSettingsView`, `PilotReadinessView`, `FirstRunSetupWizard`, and `WorkflowMapView`
  - approved-host `./scripts/test-gate.sh ui-smoke` passed on same `HEAD`, bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/ui-smoke-20260329-115154.xcresult`
- Gap / Note: The primary readability/visual-hierarchy fixes are present in code and are now backed by current approved-host runtime proof on the shell surfaces that the canonical smoke gate exercises.

### REQ-002 Bounded shared design-system primitives exist and are adopted on the promised surfaces
- Proposal Source: `4.1 Proposed File Structure`, `4.4 First-Adopter Slice and Migration Guardrails`, `Phase 3` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:431`, `docs/proposals/012-ui-quality-audit-and-visual-polish.md:462`, `docs/proposals/012-ui-quality-audit-and-visual-polish.md:505`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Support/DesignTokens.swift`
  - `Chainworks Forge/Support/StatusCapsule.swift:40`
  - `Chainworks Forge/Support/EmptyStateView.swift:18`
  - `Chainworks Forge/Views/RunsHomeView.swift:309`
  - `Chainworks Forge/Views/WorkflowMapView.swift`
  - `Chainworks Forge/Views/ReleaseGateView.swift:174`
  - `Chainworks Forge/Views/DeliveryPreflightReportView.swift:19`
  - `Chainworks Forge/Views/IdeaListView.swift:215`
- Gap / Note: The promised primitives exist and are clearly adopted on the initial slice. The remaining proposal issue is not absence of the design system; it is rollout sequencing and proof, captured separately in `REQ-005`.

### REQ-003 Secondary polish and interaction fixes on the named surfaces are implemented
- Proposal Source: `Phase 2`, `Phase 4`, `Appendix A` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:497`, `docs/proposals/012-ui-quality-audit-and-visual-polish.md:511`, `docs/proposals/012-ui-quality-audit-and-visual-polish.md:563`)
- Status: Implemented
- Evidence Type: code, tests-run, screenshot
- Evidence:
  - `Chainworks Forge/Views/ProviderSettingsView.swift:44`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:92`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:194`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:422`
  - `Chainworks Forge/Views/PilotReadinessView.swift:40`
  - `Chainworks Forge/Views/PilotReadinessView.swift:391`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:59`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:254`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:268`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:29`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:90`
  - `Chainworks Forge/Views/WorkflowMapView.swift:212`
  - `Chainworks Forge/Views/WorkflowMapView.swift:218`
  - `Chainworks Forge/Views/ArchivedIdeasView.swift:47`
  - approved-host `./scripts/test-gate.sh proposal-006` passed on same `HEAD`, bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-006-20260329-115449.xcresult`
  - approved-host `./scripts/test-gate.sh ui-smoke` passed on same `HEAD`, bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/ui-smoke-20260329-115154.xcresult`
- Gap / Note: The named secondary polish items are clearly landed. Runtime proof is strongest for provider/pilot/wizard and operator-shell flows; some Appendix A preview-owned surfaces still rely more heavily on preview/code evidence than on dedicated runtime replay.

### REQ-004 Section 3 state/async feedback contracts are implemented or explicitly deferred on the touched surfaces
- Proposal Source: `Section 3`, `Phase 2`, `Verification Criteria #3` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:497`, `docs/proposals/012-ui-quality-audit-and-visual-polish.md:526`)
- Status: Partially Implemented
- Evidence Type: code, tests-run, screenshot, inference
- Evidence:
  - `Chainworks Forge/Views/ProviderSettingsView.swift:75`
  - `Chainworks Forge/Views/PilotReadinessView.swift:367`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:228`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:56`
  - `Chainworks Forge/Views/ReleaseGateView.swift:153`
  - approved-host `proposal-006` bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-006-20260329-115449.xcresult`
  - approved-host `ui-smoke` bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/ui-smoke-20260329-115154.xcresult`
- Gap / Note: Surface-local progress/error/success treatment is visibly present, and several key paths are runtime-proven. I am still holding this at `Partially Implemented` because the proposal’s full Section 3 state matrix is broader than the currently executed runtime surface set, especially for preview-owned secondary flows.

### REQ-005 The Phase 3 bounded-adopter guardrails and accessibility-safe rollout are proven before broader expansion
- Proposal Source: `4.4 First-Adopter Slice and Migration Guardrails` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:462`)
- Status: Partially Implemented
- Evidence Type: code, inference
- Evidence:
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:467`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:478`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:479`
  - `Chainworks Forge/Support/StatusCapsule.swift:40`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:304`
  - `Chainworks Forge/Views/PilotReadinessView.swift:405`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:383`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:62`
  - `Chainworks Forge/Views/ApprovalGateView.swift:94`
  - `Chainworks Forge/Views/RecoverySheet.swift:152`
  - repo-wide search across app/tests/scripts found no executed Differentiate Without Color Alone / Increase Contrast / Reduce Transparency / VoiceOver / focus-order proof artifact beyond comments in `StatusCapsule.swift`
- Gap / Note: The shared primitives now encode better semantics, but the proposal explicitly says expansion beyond the adopter slice is allowed only after previews, min-window checks, VoiceOver labels/traits, focus order, and non-text contrast verification pass unchanged. The rollout has already expanded beyond the original adopter slice without that proof being present in executable evidence.

### REQ-006 Section 6 verification and implementation-evidence handoff are executed
- Proposal Source: `6. Verification Criteria` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:520`)
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime, screenshot, inference
- Evidence:
  - local build passed, bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//proposal012-build-result.10M8Eg/proposal012-build.xcresult`
  - approved-host `ui-smoke` passed, bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/ui-smoke-20260329-115154.xcresult`
  - approved-host `proposal-006` passed, bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-006-20260329-115449.xcresult`
  - screenshot-bearing attachments from `ui-smoke`: `REQ016_ExportHub_Ready`, `REQ016_ExportHub_Exported`, `REQ011_RunProgress_Entry`, `REQ011_RunProgress_Overview`, `REQ011_RunProgress_Sections`
  - Xcode Preview renders passed for `RunsHomeView`, `ProviderSettingsView`, `PilotReadinessView`, `FirstRunSetupWizard`, and `WorkflowMapView`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:524`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:525`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:529`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:530`
- Gap / Note: Implementation-evidence handoff is now real, not theoretical. But Section 6 is still not fully executed end-to-end: there is no recorded `1024×768` min-window audit, no recorded bounded-slice accessibility-settings audit, no VoiceOver/focus-order proof artifact, and no current-round runtime proof for every Appendix A secondary surface.

## Architecture Review

**Summary:** Acceptable

No new architecture-specific blocker surfaced beyond the proposal-owned rollout/proof sequencing already captured in `REQ-005` and `REQ-006`. The current gaps are overwhelmingly verification/guardrail gaps, not missing UI infrastructure.

## Product Review

**Summary:** Acceptable

### PROD-001 Shell regrouping remains only partially realized
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `L-08`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/ContentView.swift:15`
  - `Chainworks Forge/ContentView.swift:22`
  - `Chainworks Forge/ContentView.swift:115`
  - `Chainworks Forge/ContentView.swift:120`
- Why It Matters: The in-file comment says Proposal 012 reduced the shell from seven tabs to six by merging `Pilot Readiness` into configuration, but the current shell still renders separate `Pilot Readiness` and `Settings` tabs. This does not fail the proposal, because `L-08` is explicitly secondary and deferred, but it dilutes the claimed simplification payoff and leaves misleading code commentary behind.
- Recommended Action: Either actually merge the shell surfaces in a follow-on slice or delete/update the stale merge comment so the product posture is truthful.

## UI Review

**Summary:** Acceptable

### UI-001 Secondary preview-owned surfaces still have thinner runtime proof than the primary shell/provider flows
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-003`, `REQ-004`, `REQ-006`
- Evidence Type: code, screenshot, runtime, inference
- Evidence:
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:29`
  - `Chainworks Forge/Views/WorkflowMapView.swift:212`
  - `Chainworks Forge/Views/ReleaseGateView.swift:153`
  - Xcode Preview renders passed for `WorkflowMapView`
  - approved-host `ui-smoke` and `proposal-006` are green on current `HEAD`
  - diagnostic direct approved-host `xcodebuild test` for Goose/workflow/release surfaces failed in a noncanonical path with bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/proposal012-r3-uiresult.7vNfgR/proposal012-r3-ui.xcresult`
- Why It Matters: The repo’s canonical gates now prove the main shell/provider/user-entry flows well, which is a big improvement over R2. But the proposal’s Appendix A inventory is broader, and some of the remaining surfaces are still supported mainly by preview/code evidence rather than direct same-round runtime replay.
- Recommended Action: Add a canonical approved-host Proposal 012 surface suite for `GooseProviderConnectionAssistantView`, `WorkflowMapView`, and `ReleaseGateView`, rather than relying on ad hoc direct `xcodebuild` replay.

## UX Review

**Summary:** At Risk

### UX-001 Accessibility-settings and VoiceOver/focus-order proof is still absent on the bounded adopter slice
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-005`, `REQ-006`
- Evidence Type: code, inference
- Evidence:
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:478`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:479`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:529`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:530`
  - `Chainworks Forge/Support/StatusCapsule.swift:9`
  - `Chainworks Forge/Support/StatusCapsule.swift:37`
  - `Chainworks Forge/Support/StatusCapsule.swift:40`
  - repo-wide search across app/tests/scripts found no executed proof artifact for Differentiate Without Color Alone, Increase Contrast, Reduce Transparency, VoiceOver labels/traits, or focus order
- Why It Matters: This is now the most important remaining proposal risk. The surfaces look substantially better, and keyboard ownership is stronger, but Proposal 012 explicitly defines accessibility-settings and VoiceOver/focus-order verification as rollout gates. Without that proof, the slice is improved but not fully signed off.
- Recommended Action: Run the bounded-slice accessibility audit on the approved host and capture explicit evidence for status cards/badges/chips under Differentiate Without Color Alone, Increase Contrast, Reduce Transparency, VoiceOver readout, and focus-order traversal.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 Section 6 sign-off is still incomplete even though the old remote-host blocker is gone
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: tests-run, runtime, inference
- Evidence:
  - approved-host `./scripts/test-gate.sh ui-smoke` passed on same `HEAD`
  - approved-host `./scripts/test-gate.sh proposal-006` passed on same `HEAD`
  - local build passed
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:525`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:529`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:530`
- Why It Matters: The main R2 blocker was “cannot prove on this host.” That blocker is now closed. The remaining readiness issue is subtler and more important: the executed proof does not yet cover every explicit verification item the proposal claims is required for sign-off.
- Recommended Action: Treat Proposal 012 as a near-complete UI slice that still needs a formal min-window plus accessibility proof pass before final closure.

### READY-002 Reusable baseline context is still missing
- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: none
- Evidence Type: inference
- Evidence:
  - `.review-baselines/current-system-baseline.md` was absent during this audit
- Why It Matters: This did not block the current audit, but it continues to make proposal-vs-implementation reviews slower and more fragile than they need to be.
- Recommended Action: Refresh the repo-level current-system baseline after the current UI-polish branch stabilizes.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | local macOS build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//proposal012-build-result.10M8Eg/proposal012-build.xcresult` |
| Core user flow runtime-validated | Partial | approved-host `ui-smoke` and `proposal-006` are green on same `HEAD`, but not every Appendix A secondary surface has current runtime proof |
| Empty/loading/error states covered | Partial | strong code evidence and focused runtime proof exist for provider/wizard/pilot/shell flows, but not for the whole Section 3 state matrix |
| Accessibility risk acceptable | Partial | `StatusCapsule` semantics improved, but proposal-required accessibility-settings and VoiceOver/focus-order proof is still missing |
| Localization risk acceptable | Not Checked | no localization-specific evidence gathered in this audit |
| Critical tests executed | Partial | local build passed; approved-host `ui-smoke` passed; approved-host `proposal-006` passed; no executed `1024×768` audit |
| Privacy/permissions/entitlements reviewed | Not Checked | no broader permissions or entitlements review was run in this audit |

## Verification Log

- `git rev-parse --short HEAD`
- `python3` summary over `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `md5 -q docs/proposals/012-ui-quality-audit-and-visual-polish.md`
- `stat -f '%Sm' -t '%Y-%m-%dT%H:%M:%S%z' docs/proposals/012-ui-quality-audit-and-visual-polish.md`
- targeted `rg`, `nl`, `sed`, and `perl` inspection across:
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks Forge/Support/StatusCapsule.swift`
  - `Chainworks Forge/Support/EmptyStateView.swift`
  - `Chainworks Forge/Views/ForegroundBannerView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/ProviderSettingsView.swift`
  - `Chainworks Forge/Views/PilotReadinessView.swift`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift`
  - `Chainworks Forge/Views/WorkflowMapView.swift`
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `Chainworks Forge/Views/ArchivedIdeasView.swift`
  - `Chainworks Forge/Views/DeliveryPreflightReportView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/ApprovalGateView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `Chainworks ForgeTests/WorkflowMapProjectionTests.swift`
  - `scripts/test-gate.sh`
- Xcode MCP `RenderPreview` for:
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/ProviderSettingsView.swift`
  - `Chainworks Forge/Views/PilotReadinessView.swift`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift`
  - `Chainworks Forge/Views/WorkflowMapView.swift`
- local macOS build produced: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//proposal012-build-result.10M8Eg/proposal012-build.xcresult`
- approved-host canonical replay:
  - `ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD='Test123' ./scripts/test-gate.sh ui-smoke"`
  - `ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD='Test123' ./scripts/test-gate.sh proposal-006"`
- diagnostic-only approved-host direct `xcodebuild test` for Goose/workflow/release surfaces produced noncanonical failure bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/proposal012-r3-uiresult.7vNfgR/proposal012-r3-ui.xcresult`

## Recommended Next Actions

- Run and record the explicit `1024×768` min-window audit for all Appendix A surfaces that declare min-window proof ownership.
- Execute the bounded-adopter accessibility audit on the approved host and attach evidence for Differentiate Without Color Alone, Increase Contrast, Reduce Transparency, VoiceOver labels/traits, and focus order.
- Add a canonical approved-host Proposal 012 runtime suite for `GooseProviderConnectionAssistantView`, `WorkflowMapView`, and `ReleaseGateView`.
- Either complete the `Pilot Readiness` into `Settings` regrouping later or remove the stale merge comment in `ContentView.swift`.
- Refresh `.review-baselines/current-system-baseline.md` after the current UI-polish branch stabilizes.
