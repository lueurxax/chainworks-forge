# Proposal 012: UI Quality Audit and Visual Polish Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/012-ui-quality-audit-and-visual-polish.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `c387e38` |
| Working Tree | `dirty (18 modified, 8 untracked)` |
| Audited At | `2026-03-29T09:26:33+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Ready with Risks` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 012 moved materially forward from R1. The clean macOS build passed, the focused `WorkflowMapProjectionTests` slice passed, the explicit `FirstRunSetupWizard` keyboard-ownership gap is now closed, and the provider-settings smoke selector drift is fixed. The proposal is still not fully closed because the bounded-slice accessibility proof remains unexecuted and the targeted macOS UI smoke path is blocked on this host by the repo’s remote-only UI-test policy before the touched surfaces can be exercised.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Section 6 accessibility/runtime proof is still incomplete | High |
| Architecture | Acceptable | touched seams still emit actor-isolation and missing-import warnings in clean builds | High |
| Product | Acceptable | configuration simplification value is still diluted by the unresolved `Pilot Readiness` vs `Settings` shell split | Medium |
| UI | Acceptable | the direct macOS smoke path is blocked by host policy, so the visual/runtime proof remains thinner than the proposal expects | Medium |
| UX | At Risk | keyboard bindings are now encoded, but keyboard-only plus VoiceOver/accessibility-settings proof is still unexecuted | Medium |
| Readiness | Ready with Risks | local UI smoke cannot reach the Proposal 012 surfaces on this host because UI tests are remote-only | High |

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

- The highest-priority structural fixes remain landed across `RunsHomeView`, `ProviderSettingsView`, `PilotReadinessView`, and `FirstRunSetupWizard`.
- Shared `DesignTokens`, `StatusCapsule`, and `StyledEmptyState` primitives exist and are adopted on the named operator surfaces.
- The `FirstRunSetupWizard` now exposes explicit `Escape` dismiss and `⌘S` save bindings, closing the keyboard-ownership gap called out in R1.
- The provider-settings UI smoke selector now matches the current surface contract: the test expects `provider-settings-export`, and the view still exposes `provider-settings-export`.

### Divergences

- `ContentView` still keeps separate `Pilot Readiness` and `Settings` tabs even though the in-file Proposal 012 comment claims the shell was reduced from seven tabs to six by merging them.
- Section 6’s runtime/accessibility proof bar is still not reproducible in this local audit environment, so the implementation remains short of the proposal’s full sign-off posture.

### Ambiguities / Evidence Gaps

- `.review-baselines/current-system-baseline.md` is still absent, so this audit again relied on direct code mapping rather than a reusable baseline.
- I found no executed Differentiate Without Color Alone, Increase Contrast, Reduce Transparency, or VoiceOver proof artifact for the bounded adopter slice.
- The targeted macOS UI smoke tests are blocked on this host by the repo’s remote-only UI-test policy before the Proposal 012 assertions execute.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 3 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Current-HEAD readability and hierarchy fixes land on the named operator surfaces
- Proposal Source: `C-01`, `H-01`, `H-02`, `H-03`, `H-04` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:66`, `:83`, `:97`, `:111`, `:125`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:127`
  - `Chainworks Forge/Views/RunsHomeView.swift:526`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:65`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:194`
  - `Chainworks Forge/Views/PilotReadinessView.swift:40`
  - `Chainworks Forge/Views/PilotReadinessView.swift:391`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:36`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:79`
- Gap / Note: The core readability and hierarchy changes remain present in current code on the named surfaces.

### REQ-002 Shared design primitives and semantic styling foundation exist and are adopted on the bounded slice
- Proposal Source: `M-01`, `M-02`, `M-03`, `M-04`, `4.1`, `4.2`, `4.3` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:140`, `:183`, `:218`, `:241`, `:431`, `:441`, `:451`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/DesignTokens.swift:6`
  - `Chainworks Forge/Support/DesignTokens.swift:15`
  - `Chainworks Forge/Support/DesignTokens.swift:33`
  - `Chainworks Forge/Support/StatusCapsule.swift:11`
  - `Chainworks Forge/Support/StatusCapsule.swift:35`
  - `Chainworks Forge/Support/EmptyStateView.swift:18`
  - `Chainworks Forge/Views/ReleaseGateView.swift:105`
  - `Chainworks Forge/Views/WorkflowMapView.swift:250`
- Gap / Note: The shared primitives and semantic tokens are present and actively used on the bounded adopter slice.

### REQ-003 Secondary polish and interaction updates land on the named surfaces
- Proposal Source: `L-01`, `L-03`, `L-04`, `L-05`, `L-06`, `L-07`, `L-09`, `L-10`, `L-12` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:253`, `:273`, `:286`, `:296`, `:306`, `:319`, `:354`, `:368`, `:378`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/EmptyStateView.swift:18`
  - `Chainworks Forge/Views/IdeaListView.swift:173`
  - `Chainworks Forge/Views/IdeaListView.swift:379`
  - `Chainworks Forge/Views/IdeaListView.swift:428`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:29`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:51`
  - `Chainworks Forge/Views/WorkflowMapView.swift:123`
  - `Chainworks Forge/Views/WorkflowMapView.swift:217`
  - `Chainworks Forge/Views/ReleaseGateView.swift:152`
  - `Chainworks Forge/Views/ApprovalGateView.swift:83`
  - `Chainworks Forge/Views/ReleaseGateView.swift:78`
  - `Chainworks Forge/Views/RecoverySheet.swift:152`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:254`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:268`
  - `Chainworks Forge/Views/RunsHomeView.swift:595`
- Gap / Note: The previously open wizard shortcut gap is now closed, so the named secondary polish and interaction updates audit as implemented in code.

### REQ-004 Surface state and async feedback contracts are implemented or explicitly deferred on the named surfaces
- Proposal Source: `3.1 Surface State Matrix`, `3.2 Async Feedback by Surface`, `L-12` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:398`, `:410`, `:378`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Views/ProviderSettingsView.swift:75`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:284`
  - `Chainworks Forge/Views/PilotReadinessView.swift:367`
  - `Chainworks Forge/Views/PilotReadinessView.swift:391`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:228`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:244`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:56`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift:78`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:571`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:609`
- Gap / Note: Inline progress, retry, and local feedback are present on the named surfaces, but the broader Section 3 matrix across degraded/offline, auth-required, retry persistence, and cancellation semantics is still not fully executed in tests or runtime evidence.

### REQ-005 Accessibility-safe bounded-slice guardrails are proven before broader expansion
- Proposal Source: `4.4 First-Adopter Slice and Migration Guardrails` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:462`)
- Status: Not Verifiable
- Evidence Type: code, tests-found, inference
- Evidence:
  - `Chainworks Forge/Support/StatusCapsule.swift:7`
  - `Chainworks Forge/Support/StatusCapsule.swift:35`
  - `Chainworks Forge/Views/ReleaseGateView.swift:174`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:571`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:590`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:609`
- Gap / Note: The shared primitive now encodes stronger text and VoiceOver semantics, but I still found no executed proof for Differentiate Without Color Alone, Increase Contrast, Reduce Transparency, VoiceOver labels/traits, or focus order on the bounded adopter slice.

### REQ-006 Post-code verification and implementation-evidence handoff from Section 6 are executed
- Proposal Source: `6. Verification Criteria` (`docs/proposals/012-ui-quality-audit-and-visual-polish.md:520`)
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:25`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:571`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:590`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:609`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/proposal012-r2-build.AcDesV' build` (passed)
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/proposal012-r2-unit.SZfGzP' test -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests'` (passed)
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/proposal012-r2-ui.rRspZd' test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface'` (failed: repo-enforced remote-only UI host policy)
- Gap / Note: Clean build and focused unit proof are green, but the local macOS UI proof path still does not execute the touched Proposal 012 surfaces on this host, so the full Section 6 verification bar remains incomplete.

## Architecture Review

**Summary:** Acceptable

### ARCH-001 Touched host-system seams still carry compiler-warning debt
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-004`, `REQ-006`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:48`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:80`
  - `Chainworks Forge/Models/RunRepository.swift:104`
  - `Chainworks Forge/Views/RecoverySheet.swift:12`
  - `Chainworks Forge/Views/RunComparisonView.swift:11`
  - clean `xcodebuild ... build` and `xcodebuild ... test` runs emitted actor-isolation `.empty` warnings and missing `SwiftData` import warnings
- Why It Matters: Proposal 012 is a UI polish slice, but the touched operator flows still sit on compiler-fragile seams. That weakens long-term maintainability and raises the chance of future toolchain regressions in otherwise-finished UI work.
- Recommended Action: Clear the current actor-isolation and missing-`SwiftData` warnings before calling the slice fully hardened.

## Product Review

**Summary:** Acceptable

### PROD-001 Configuration simplification value is still diluted by the unresolved shell split
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `L-08`
- Leading Metric: reduced tab switching to complete provider/workspace setup
- Guardrail Metric: no drop in direct reachability for readiness diagnostics, wizard launch, or settings transfer/export actions
- Decision Checkpoint: revisit after the next operator-heavy proposal if users still bounce between `Pilot Readiness` and `Settings` to complete one setup job
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/ContentView.swift:15`
  - `Chainworks Forge/ContentView.swift:22`
  - `Chainworks Forge/ContentView.swift:114`
  - `Chainworks Forge/ContentView.swift:119`
- Why It Matters: This does not break the proposal’s conformance bar, because `L-08` is explicitly non-breaking, but it means the practical product payoff on “less overwhelming configuration” is smaller than the in-file comment suggests.
- Recommended Action: Decide explicitly whether shell regrouping is still wanted, and either land it later or remove the misleading merge comment.

## UI Review

**Summary:** Acceptable

### UI-001 Direct macOS surface proof is now blocked by host policy rather than by selector drift
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-003`, `REQ-006`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:25`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:641`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:425`
  - `Chainworks Forge/Views/RunsHomeView.swift:127`
  - `Chainworks Forge/Views/PilotReadinessView.swift:391`
  - `xcodebuild ... -derivedDataPath '/tmp/proposal012-r2-ui.rRspZd' test ...` (failed on remote-only UI host policy before the touched assertions ran)
- Why It Matters: The visual system is strong in code, and the old provider-settings selector drift is gone, but the proposal still wants real macOS proof under actual host rendering and that evidence is missing in this local environment.
- Recommended Action: Run the targeted UI smoke slice on an approved remote UI host, or loosen the host-policy gate for controlled local audit runs.

## UX Review

**Summary:** At Risk

### UX-001 Keyboard ownership is now encoded, but keyboard-only and accessibility-safe proof is still missing
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-003`, `REQ-005`, `REQ-006`
- Evidence Type: code, inference
- Evidence:
  - `Chainworks Forge/Views/ApprovalGateView.swift:83`
  - `Chainworks Forge/Views/ApprovalGateView.swift:97`
  - `Chainworks Forge/Views/ReleaseGateView.swift:78`
  - `Chainworks Forge/Views/ReleaseGateView.swift:90`
  - `Chainworks Forge/Views/RecoverySheet.swift:152`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:254`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:268`
  - `Chainworks Forge/Support/StatusCapsule.swift:35`
- Why It Matters: The biggest modal/operator flows are closer to the intended macOS convention set now, but Proposal 012 explicitly calls for keyboard-only, VoiceOver, and accessibility-settings verification. Until that proof exists, the UX bar is improved in code but not actually closed.
- Recommended Action: Execute the bounded-slice keyboard-only and accessibility audit on an approved host, including Escape dismissal, primary confirm actions, VoiceOver labels/traits, and contrast/non-color checks.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 Local macOS UI smoke is blocked by the repo’s remote-only host policy
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: tests-run, code
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:25`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:26`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:27`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/proposal012-r2-ui.rRspZd' test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface'` (failed)
  - xcresult at `/tmp/proposal012-r2-ui.rRspZd/Logs/Test/Test-Chainworks Forge-2026.03.29_09-22-46-+0300.xcresult`
- Why It Matters: This is the main reason the audit cannot honestly call Proposal 012 fully ready. The blocker is now environmental/test-policy related, not a surface-selector regression, but it still caps runtime confidence for the verification contract.
- Recommended Action: Execute the smoke suite on an approved host or introduce an explicit audit-safe override for controlled local UI-proof runs.

### READY-002 Bounded-slice accessibility proof is still absent
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-005`, `REQ-006`
- Evidence Type: code, inference
- Evidence:
  - `Chainworks Forge/Support/StatusCapsule.swift:7`
  - `Chainworks Forge/Support/StatusCapsule.swift:35`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:528`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:529`
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md:530`
- Why It Matters: The proposal’s last meaningful sign-off risk is no longer button wiring. It is the absence of executed Differentiate Without Color Alone, Increase Contrast, Reduce Transparency, and VoiceOver proof on the shared adopter slice.
- Recommended Action: Add the missing accessibility-settings and VoiceOver evidence before treating the slice as fully signed off.

### READY-003 Reusable baseline context is still missing
- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: none
- Evidence Type: inference
- Evidence:
  - `.review-baselines/current-system-baseline.md` was absent during this audit
- Why It Matters: This did not block the current audit, but it keeps implementation reviews more expensive and more brittle than they need to be.
- Recommended Action: Refresh the repo-level current-system baseline after the current UI-polish branch stabilizes.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `xcodebuild ... build` for the macOS target passed with a clean unique DerivedData path |
| Core user flow runtime-validated | Partial | focused unit proof passed; targeted macOS UI smoke is blocked by remote-only host policy before surface assertions execute |
| Empty/loading/error states covered | Partial | strong code evidence for inline/local loading and retry on named surfaces, but not fully runtime-proven across the whole Section 3 matrix |
| Accessibility risk acceptable | Partial | shared primitives now encode stronger semantics, but Differentiate Without Color Alone / Increase Contrast / Reduce Transparency / VoiceOver proof is still missing |
| Localization risk acceptable | Not Checked | no localization-specific evidence gathered in this audit |
| Critical tests executed | Partial | clean macOS build passed; `WorkflowMapProjectionTests` passed; targeted UI smoke failed on host policy |
| Privacy/permissions/entitlements reviewed | Not Checked | no broader permissions or entitlements review was run in this audit |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/012-ui-quality-audit-and-visual-polish.md`
- `git rev-parse --show-toplevel`
- `git rev-parse --short HEAD`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `sed -n '1,620p' docs/proposals/012-ui-quality-audit-and-visual-polish.md`
- targeted `rg` / `sed` inspection across `ContentView.swift`, `Support/StatusCapsule.swift`, `Support/EmptyStateView.swift`, `Views/FirstRunSetupWizard.swift`, `Views/ApprovalGateView.swift`, `Views/ReleaseGateView.swift`, `Views/RecoverySheet.swift`, `Views/WorkflowMapView.swift`, `Views/GooseProviderConnectionAssistantView.swift`, `Views/IdeaListView.swift`, `Views/RunsHomeView.swift`, `Views/ProviderSettingsView.swift`, `Views/PilotReadinessView.swift`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/proposal012-r2-build.AcDesV' build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/proposal012-r2-unit.SZfGzP' test -only-testing:'Chainworks ForgeTests/WorkflowMapProjectionTests'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/proposal012-r2-ui.rRspZd' test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface'`

## Recommended Next Actions

- Run the targeted Proposal 012 UI smoke suite on an approved remote UI host, or add an explicit audit-safe override for controlled local runs.
- Capture the missing accessibility-settings and VoiceOver proof for the bounded adopter slice.
- Clear the remaining actor-isolation and missing-`SwiftData` warnings in touched seams.
- Decide whether `Pilot Readiness` should really merge into `Settings`, or delete the misleading in-file merge comment.
- Refresh `.review-baselines/current-system-baseline.md` after the branch stabilizes.
