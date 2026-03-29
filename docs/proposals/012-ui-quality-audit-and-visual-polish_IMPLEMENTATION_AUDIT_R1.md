# Proposal 012: UI Quality Audit and Visual Polish Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/012-ui-quality-audit-and-visual-polish.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `c387e38` |
| Working Tree | `dirty (18 modified, 5 untracked)` |
| Audited At | `2026-03-29T09:01:53+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Ready with Risks` |
| Audit Confidence | `Medium` |

## Executive Verdict

Proposal 012 is largely implemented in the current `HEAD`: the major readability fixes, shared primitives, summary-strip/form/panel restructures, and most of the named surface polish landed in code and the macOS app still builds cleanly enough to ship a debug build. It is not fully closed as a proposal contract, though, because the keyboard/accessibility verification layer remains incomplete, the explicit Section 6 proof bar is only partially satisfied, and the macOS UI evidence path is currently capped by a LocalAuthentication boot failure before the targeted UI assertions run.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Section 6 verification and accessibility proof are only partially closed | High |
| Architecture | Acceptable | compiler warnings around actor isolation and missing `SwiftData` imports remain in touched host-system seams | High |
| Product | Acceptable | configuration simplification value is diluted because shell-level grouping remains deferred | Medium |
| UI | Acceptable | visual claims are strong in code but still not runtime-proven under min-window / real macOS observation | Medium |
| UX | At Risk | keyboard-only and accessibility-safe interaction coverage is uneven across the final modal/operator flows | Medium |
| Readiness | Ready with Risks | targeted UI proof is blocked by LocalAuthentication cancellation before the Proposal 012 smoke assertions execute | High |

## Proposal Contract

### Scope

- Improve readability, density, and visual hierarchy of the current macOS operator surfaces listed in Appendix A.
- Land a bounded shared design-system foundation for status, spacing, corners, and typography.
- Standardize surface-level feedback and async treatment without changing core business logic or navigation ownership.

### Locked Decisions

- Proposal 012 is a macOS-only UI quality slice, not a runtime-contract or engine-behavior rewrite.
- Shared primitives must be introduced in a bounded adopter slice first, without business-logic changes or navigation changes.
- Existing `accessibilityIdentifier` values and keyboard behaviors must remain stable through the shared-primitive migration.
- Runtime screenshots and live interaction proof are intentionally deferred to the follow-up implementation evidence review rather than assumed by proposal review.

### Primary User Flows

- Review runs in `RunsHomeView` without severe truncation and take high-value actions from an always-visible detail surface.
- Configure providers and workspace/bootstrap settings through `ProviderSettingsView`, `PilotReadinessView`, and `FirstRunSetupWizard`.
- Create ideas, inspect readiness, and move through approval/release/recovery flows with consistent status, empty, and feedback treatment.
- Understand run topology and release state via `WorkflowMapView` and `ReleaseGateView`.

### UI Commitments

- Widen and simplify dense sidebars and rows (`RunsHomeView`, `RunsHomeRow`).
- Replace fragmented badge styling with shared status capsules and semantic tokens.
- Add clearer empty states, chips, hero banners, journey/progress indicators, group boxes, disclosure groups, and above-the-fold actions on named surfaces.
- Convert the New Idea sheet to a macOS-appropriate `Form`.

### UX Commitments

- Key modal/operator flows must have consistent high-value confirm/dismiss behavior, including keyboard ownership.
- Recoverable loading and error states should stay inline/local to the initiating surface.
- Accessibility-safe status differentiation must not rely on color alone.
- The bounded adopter slice must preserve labels, traits, focus order, and contrast behavior before wider rollout.

### Acceptance Criteria

- Current-HEAD readability issues in the named catalog are corrected on the audited surfaces.
- Shared tokens and shared status capsule primitives exist and are adopted on the bounded slice.
- Section 3 state/feedback semantics are implemented or explicitly deferred on the named surfaces.
- Section 6 verification criteria are satisfied after code lands.

### Test / Evidence Requirements

- Preview-backed surfaces listed in Appendix A should render without overflow.
- Minimum window checks should confirm usability at `1024x768`.
- Keyboard, accessibility-settings, and VoiceOver verification should be executed on the bounded adopter slice.
- Runtime screenshots and live interaction proof should exist in the follow-up implementation evidence review.

### Explicit Exclusions

- No engine/session/provider transport rewrite.
- No broad shell redesign beyond bounded UI polish.
- No expansion of the shared-primitives rollout past the adopter slice before the guardrails are proven.

## Proposal Fidelity / Divergence

### Matches

- The highest-priority structural fixes landed across `RunsHomeView`, `ProviderSettingsView`, `PilotReadinessView`, and `FirstRunSetupWizard`.
- Shared `DesignTokens`, `StatusCapsule`, and `StyledEmptyState` primitives now exist and are adopted across the named operator surfaces.
- `GooseProviderConnectionAssistantView`, `WorkflowMapView`, `ReleaseGateView`, `ApprovalGateView`, and `NewIdeaSheetView` all show the intended polish direction in code.
- `ForegroundBannerView` now animates from the bottom edge as required.

### Divergences

- `L-09` is only partially closed: `ApprovalGateView`, `ReleaseGateView`, `RecoverySheet`, and `NewIdeaSheetView` have explicit bindings, but `FirstRunSetupWizard` still exposes `Close` / `Save` without explicit keyboard shortcuts.
- `L-08` remains deferred in practice: `ContentView.swift` still renders separate `Pilot Readiness` and `Settings` tabs even though the code comment says the shell was reduced by merging them.
- The implementation-proof layer is weaker than the proposal contract asks for: the targeted macOS UI run failed during test-runner initialization, so the runtime portion of Section 6 is not reproducible in this audit.

### Ambiguities / Evidence Gaps

- `.review-baselines/current-system-baseline.md` is still missing, so this audit relied on direct code mapping rather than a reusable baseline.
- I found no executed VoiceOver, Differentiate Without Color Alone, Increase Contrast, or Reduce Transparency proof artifact for the bounded adopter slice.
- UI tests exist for most touched surfaces, but the targeted macOS UI run did not reach the actual surface assertions because the test runner failed on LocalAuthentication initialization.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 2 |
| Partially Implemented | 3 |
| Missing | 0 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Current-HEAD readability and hierarchy fixes land on the named operator surfaces
- Proposal Source: `C-01`, `H-01`, `H-02`, `H-03`, `H-04` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:66`, `:83`, `:97`, `:111`, `:125`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:127`
  - `Chainworks Forge/Views/RunsHomeView.swift:257`
  - `Chainworks Forge/Views/RunsHomeView.swift:595`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:56`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:194`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:298`
  - `Chainworks Forge/Views/PilotReadinessView.swift:35`
  - `Chainworks Forge/Views/PilotReadinessView.swift:40`
  - `Chainworks Forge/Views/PilotReadinessView.swift:364`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:28`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:59`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:371`
- Gap / Note: The high-priority structural changes are present in code on the named surfaces.

### REQ-002 Shared design primitives and semantic styling foundation exist and are adopted on the bounded slice
- Proposal Source: `M-01`, `M-02`, `M-03`, `M-04`, `4.1`, `4.2`, `4.3` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:140`, `:183`, `:218`, `:241`, `:431`, `:441`, `:451`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/DesignTokens.swift:6`
  - `Chainworks Forge/Support/DesignTokens.swift:15`
  - `Chainworks Forge/Support/DesignTokens.swift:33`
  - `Chainworks Forge/Support/DesignTokens.swift:48`
  - `Chainworks Forge/Support/DesignTokens.swift:71`
  - `Chainworks Forge/Support/StatusCapsule.swift:11`
  - `Chainworks Forge/Support/StatusCapsule.swift:35`
  - `Chainworks Forge/Support/EmptyStateView.swift:18`
  - `Chainworks Forge/Support/EmptyStateView.swift:28`
  - `Chainworks Forge/Views/ForegroundBannerView.swift:68`
  - `Chainworks Forge/Views/ReleaseGateView.swift:105`
  - `Chainworks Forge/Views/WorkflowMapView.swift:250`
- Gap / Note: The shared primitives and token families are in place and visibly adopted on the bounded operator surfaces.

### REQ-003 Secondary polish and interaction updates land on the named surfaces
- Proposal Source: `L-01`, `L-03`, `L-04`, `L-05`, `L-06`, `L-07`, `L-09`, `L-10`, `L-12` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:253`, `:273`, `:286`, `:296`, `:306`, `:319`, `:354`, `:368`, `:378`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/EmptyStateView.swift:18`
  - `Chainworks Forge/Views/IdeaListView.swift:174`
  - `Chainworks Forge/Views/IdeaListView.swift:369`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:36`
  - `Chainworks Forge/Views/WorkflowMapView.swift:212`
  - `Chainworks Forge/Views/WorkflowMapView.swift:218`
  - `Chainworks Forge/Views/ReleaseGateView.swift:157`
  - `Chainworks Forge/Views/RunsHomeView.swift:595`
  - `Chainworks Forge/Views/ApprovalGateView.swift:83`
  - `Chainworks Forge/Views/ReleaseGateView.swift:78`
  - `Chainworks Forge/Views/RecoverySheet.swift:150`
  - `Chainworks Forge/Views/IdeaListView.swift:428`
- Gap / Note: Most named secondary fixes landed, but `L-09` is not uniformly closed because `FirstRunSetupWizard` still has `Close` / `Save` actions without explicit keyboard shortcut bindings.

### REQ-004 Surface state and async feedback contracts are implemented or explicitly deferred on the named surfaces
- Proposal Source: `3.1 Surface State Matrix`, `3.2 Async Feedback by Surface`, `L-12` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:398`, `:410`, `:378`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Views/ProviderSettingsView.swift:24`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:277`
  - `Chainworks Forge/Views/PilotReadinessView.swift:294`
  - `Chainworks Forge/Views/PilotReadinessView.swift:364`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:228`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:238`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:56`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:78`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:652`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:780`
- Gap / Note: Inline loading and some local error/retry treatment exist, but the full Section 3 matrix across validation, degraded/offline, auth-expiry, and rollback/cancellation is not comprehensively proven in tests or runtime evidence.

### REQ-005 Accessibility-safe bounded-slice guardrails are proven before broader expansion
- Proposal Source: `4.4 First-Adopter Slice and Migration Guardrails` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:462`)
- Status: Not Verifiable
- Evidence Type: code, tests-found, inference
- Evidence:
  - `Chainworks Forge/Support/StatusCapsule.swift:9`
  - `Chainworks Forge/Support/StatusCapsule.swift:10`
  - `Chainworks Forge/Support/StatusCapsule.swift:35`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:571`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:590`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:609`
- Gap / Note: The code comments and shared primitive design point in the right direction, but I found no executed proof for Differentiate Without Color Alone, Increase Contrast, Reduce Transparency, VoiceOver labels/traits, or focus order on the bounded adopter slice.

### REQ-006 Post-code verification and implementation-evidence handoff from Section 6 are executed
- Proposal Source: `6. Verification Criteria` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:520`)
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:388`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:537`
  - `Chainworks Forge/Views/PilotReadinessView.swift:435`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:468`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:310`
  - `Chainworks Forge/Views/WorkflowMapView.swift:568`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:571`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:780`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:814`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1169`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/chainworks-proposal012-audit-derived' build` (passed)
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/chainworks-proposal012-audit-derived' test -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests'` (passed)
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/chainworks-proposal012-audit-derived' test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface'` (failed during UI-test bootstrap with `Authentication cancelled`)
- Gap / Note: Preview-backed and test-backed intent exists, but the explicit macOS runtime/accessibility proof bar is not fully reproducible yet.

## Architecture Review

**Summary:** Acceptable

### ARCH-001 Touched host-system seams still carry compiler-warning debt
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-004`, `REQ-006`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RunStartOverrideResolver.swift:12`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:80`
  - `Chainworks Forge/Models/RunRepository.swift:104`
  - `Chainworks Forge/Views/RecoverySheet.swift:12`
  - `Chainworks Forge/Views/RunComparisonView.swift:11`
  - `xcodebuild ... build` emitted warnings for actor-isolated `.empty` defaults and missing `SwiftData` imports
- Why It Matters: Proposal 012 is a UI slice, but the touched operator flows still sit on compiler-fragile seams. That increases the chance that a future Swift/Xcode upgrade turns a polished UI slice into a broken build boundary.
- Recommended Action: Clear the current warnings before calling the slice fully hardened, especially the actor-isolation defaults and `SwiftData` import omissions.

## Product Review

**Summary:** Acceptable

### PROD-001 Configuration simplification value is only partially realized because the shell still splits Settings and Pilot Readiness
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `L-08`, `REQ-003`
- Leading Metric: reduced tab switching to complete provider/workspace setup
- Guardrail Metric: no drop in direct reachability for provider diagnostics, wizard launch, or support/export actions
- Decision Checkpoint: revisit after the first post-landing dogfood round if operators still bounce between `Pilot Readiness` and `Settings` to complete one setup job
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/ContentView.swift:15`
  - `Chainworks Forge/ContentView.swift:22`
  - `Chainworks Forge/ContentView.swift:115`
  - `Chainworks Forge/ContentView.swift:122`
- Why It Matters: This does not break the proposal, because `L-08` is framed as a softer shell-level consideration, but it means the product payoff on “less overwhelming configuration” is smaller than the in-file comment suggests.
- Recommended Action: Leave this out of the current conformance bar, but decide explicitly whether shell regrouping is still wanted before the next configuration-heavy proposal lands.

## UI Review

**Summary:** Acceptable

### UI-001 The visual system landed strongly in code, but min-window and real-surface visual proof is still thinner than the proposal expects
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-001`, `REQ-002`, `REQ-006`
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:127`
  - `Chainworks Forge/Views/IdeaListView.swift:105`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:298`
  - `Chainworks Forge/Views/PilotReadinessView.swift:364`
  - `Chainworks Forge/Views/WorkflowMapView.swift:212`
  - `Chainworks Forge/Views/ReleaseGateView.swift:157`
  - preview declarations at `Chainworks Forge/Views/RunsHomeView.swift:388`, `Chainworks Forge/Views/ProviderSettingsView.swift:537`, `Chainworks Forge/Views/PilotReadinessView.swift:435`, `Chainworks Forge/Views/FirstRunSetupWizard.swift:468`, `Chainworks Forge/Views/WorkflowMapView.swift:568`
- Why It Matters: The design work is visibly real, but a UI-polish proposal is only as strong as its visual proof under actual window constraints and real host rendering. Right now the runtime portion of that proof remains incomplete.
- Recommended Action: Complete the Section 6 min-window and direct-surface runtime checks once the UI test host can boot without the LocalAuthentication cancellation path.

## UX Review

**Summary:** At Risk

### UX-001 Keyboard ownership is improved but not yet uniform across the final operator flows
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `L-09`, `REQ-003`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/ApprovalGateView.swift:83`
  - `Chainworks Forge/Views/ApprovalGateView.swift:97`
  - `Chainworks Forge/Views/ReleaseGateView.swift:78`
  - `Chainworks Forge/Views/ReleaseGateView.swift:90`
  - `Chainworks Forge/Views/RecoverySheet.swift:150`
  - `Chainworks Forge/Views/IdeaListView.swift:428`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:252`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:256`
- Why It Matters: The proposal explicitly tries to normalize high-value confirm/dismiss behavior. The biggest modal/operator flows are closer now, but the wizard still depends on default toolbar behavior rather than explicit shortcut ownership, and there is no executed keyboard-only proof for the final macOS experience.
- Recommended Action: Decide and encode explicit `Close` / `Save` shortcut ownership for `FirstRunSetupWizard`, then validate the final modal flows with keyboard-only traversal.

### UX-002 Accessibility-safe status behavior is designed, not yet proven
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-005`, `REQ-006`
- Evidence Type: code, inference
- Evidence:
  - `Chainworks Forge/Support/StatusCapsule.swift:9`
  - `Chainworks Forge/Support/StatusCapsule.swift:10`
  - `Chainworks Forge/Support/StatusCapsule.swift:35`
  - `Chainworks Forge/Views/ReleaseGateView.swift:174`
  - `Chainworks Forge/Views/ReleaseGateView.swift:176`
- Why It Matters: The proposal and research deltas correctly treat non-color differentiation, contrast, and VoiceOver semantics as part of operator trust. Without executed settings/VoiceOver proof, the UX bar is still aspirational rather than closed.
- Recommended Action: Run and capture the explicit accessibility-settings and VoiceOver checks the proposal calls for before calling the polish slice finished.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 The targeted macOS UI-proof path is currently blocked by LocalAuthentication during test-runner initialization
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: tests-run
- Evidence:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/chainworks-proposal012-audit-derived' test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface'` (failed)
  - xcresult summary at `/tmp/chainworks-proposal012-audit-derived/Logs/Test/Test-Chainworks Forge-2026.03.29_09-00-26-+0300.xcresult` reported `The test runner failed to initialize for UI testing ... Authentication cancelled`
- Why It Matters: This is the main reason the audit cannot honestly say the runtime side of Proposal 012 is closed. The blocker is environmental/bootstrapping rather than missing view identifiers, but it still caps ship confidence for the verification contract.
- Recommended Action: Remove or bypass the LocalAuthentication dependency for automated UI-test bootstrap, then rerun the targeted surface tests and capture the actual macOS evidence.

### READY-002 The provider-settings UI smoke suite still contains selector drift from the proposal’s new placement
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: `H-01`, `REQ-006`
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:641`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:425`
- Why It Matters: Proposal 012 intentionally moved Settings Transfer out of the old inline wall. The test suite still expects `provider-settings-toolbar-export`, while the current view exposes `provider-settings-export` in the secondary section. That weakens the trustworthiness of the smoke suite even before it reaches the auth blocker.
- Recommended Action: Update the UI smoke tests to the new surface contract once the UI-test bootstrap issue is fixed.

### READY-003 Reusable baseline context is still missing, which keeps audits expensive and more brittle than they need to be
- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: none
- Evidence Type: code, inference
- Evidence:
  - `.review-baselines/current-system-baseline.md` was absent during this audit
- Why It Matters: This did not prevent the current implementation audit, but it forces each proposal/implementation pass to rebuild host-system context from scratch and makes future rounds noisier.
- Recommended Action: Refresh the repo-level current-system baseline after the current UI-polish branch stabilizes.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `xcodebuild ... build` for the macOS target passed |
| Core user flow runtime-validated | Partial | one targeted logic slice passed; targeted macOS UI run failed during runner bootstrap before surface assertions |
| Empty/loading/error states covered | Partial | strong code evidence for inline/local loading and retry on named surfaces, but not fully runtime-proven across the whole Section 3 matrix |
| Accessibility risk acceptable | Partial | design intent exists in shared primitives, but Differentiate Without Color Alone / Increase Contrast / Reduce Transparency / VoiceOver proof is still missing |
| Localization risk acceptable | Not Checked | no localization-specific evidence gathered in this audit |
| Critical tests executed | Partial | `WorkflowMapProjectionTests` passed; targeted UI test run failed during LocalAuthentication bootstrap |
| Privacy/permissions/entitlements reviewed | Partial | UI-test failure exposed an auth dependency in the host runtime path, but no broader permissions review was run |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/012-ui-quality-audit-and-visual-polish.md`
- `git rev-parse --show-toplevel`
- `git rev-parse --short HEAD`
- `git status --short`
- `sed -n '1,220p' docs/proposals/012-ui-quality-audit-and-visual-polish.md`
- `rg -n '^#|^##|^###|^####' docs/proposals/012-ui-quality-audit-and-visual-polish.md`
- targeted `rg` and `sed` reads across `ContentView.swift`, `Support/DesignTokens.swift`, `Support/StatusCapsule.swift`, `Support/EmptyStateView.swift`, and the touched `Views/*.swift` surfaces
- `rg -n "012-ui-quality-audit-and-visual-polish|supersed|deprecated|replaced|obsolete" docs -S`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/chainworks-proposal012-audit-derived' build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/chainworks-proposal012-audit-derived' test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface'`
- `xcrun xcresulttool get --legacy --path '/tmp/chainworks-proposal012-audit-derived/Logs/Test/Test-Chainworks Forge-2026.03.29_09-00-26-+0300.xcresult' --format json`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/chainworks-proposal012-audit-derived' test -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests'`

## Recommended Next Actions

- Fix the explicit keyboard shortcut gap in `FirstRunSetupWizard` and rerun the keyboard-only modal-flow proof.
- Unblock macOS UI-test bootstrap from the LocalAuthentication cancellation path, then rerun the targeted Proposal 012 surface tests.
- Add the missing accessibility-settings and VoiceOver verification evidence for the bounded adopter slice.
- Clear the current compiler warnings so the polished UI branch is not carrying avoidable toolchain debt into the next slice.
- Refresh `.review-baselines/current-system-baseline.md` after the UI-polish branch stabilizes.
