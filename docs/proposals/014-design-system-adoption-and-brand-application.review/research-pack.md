# Proposal Research Pack

## 0. Review Target and Local Context Consumed

- Proposal: `docs/proposals/014-design-system-adoption-and-brand-application.md`
- Research round: `R1`
- Proposal evidence pack used: `docs/reviews/014-design-system-adoption-and-brand-application-evidence-pack.md`
- Current-system baseline used: `.review-baselines/current-system-baseline.md`
- Proposal-specific integration context used: none present
- Existing research pack reused: none; this is the first `P014` research pack
- Adjacent docs consumed:
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/chainworks_forge_design_kit_v1.md`
  - `docs/reference/ui-quality-and-polish.md`
  - `docs/reference/test-gates.md`
  - `docs/evidence/ui-quality-and-polish-proof.md`
- Current code / module mapping consumed:
  - `Chainworks Forge/Support/DesignTokens.swift`
  - `Chainworks Forge/Support/StatusCapsule.swift`
  - `Chainworks Forge/Support/EmptyStateView.swift`
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/WorkflowMapView.swift`
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `Chainworks Forge/Views/ProviderSettingsView.swift`
  - `Chainworks Forge/Views/PilotReadinessView.swift`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift`
  - `Chainworks Forge/Views/DeliveryPreflightReportView.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `scripts/test-gate.sh`
- Local evidence IDs that triggered research:
  - `REAL-01`: one shared token/primitive authority is now explicit and worth validating against platform conventions
  - `REAL-02`: proof ownership is now anchored to the current UI-quality lane and worth validating against Apple accessibility criteria
  - `REAL-03`: current adopted-slice rebaseline raises the question of how far custom brand treatment should go on already-operational surfaces
  - `REAL-05`: design-kit authority is aligned locally, but bounded icon/logo usage still benefits from platform guidance
  - `RSH-01`, `RSH-02`, `RSH-03`
- Notes on baseline freshness or local contradictions:
  - local proposal/doc/code evidence was already green before research
  - research is confirmatory and scope-sharpening, not rescue work for a red draft
  - no proposal-blocking contradiction surfaced between the updated draft and current repo reality

## 1. Research Questions Derived from Local Evidence

| Question ID | Derived From (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Research Question | Why Local Evidence Is Not Enough | Priority |
|---|---|---|---|---|---|
| RQ-01 | Unresolved tradeoff | `DOC-01`, `MAP-01`, `MAP-02`, `REAL-01` | For a macOS operator app that already has bounded shared owners, what do official Apple platform docs suggest about evolving a design system while preserving semantic consistency and familiar icon behavior? | Local evidence proves the proposal is internally coherent, but external Apple guidance helps validate the “extend the current authority, don’t fork it” strategy. | High |
| RQ-02 | Host-system integration risk | `DOC-01`, `DOC-04`, `MAP-07`, `MAP-12`, `REAL-05` | What official Apple guidance constrains where app icons, logos, and custom iconography belong in a utility-style macOS app versus dense operational UI? | The design kit defines brand intent, but platform guidance is needed to keep bounded brand application aligned with native expectations. | High |
| RQ-03 | Baseline constraint | `DOC-01`, `DOC-05`, `DOC-06`, `MAP-10`, `MAP-11`, `REAL-02` | Which official Apple accessibility and evaluation criteria should the existing proof lane continue to guarantee for this bounded rollout: non-color-only semantics, contrast/transparency, VoiceOver labels/order, and keyboard continuity? | Local proof owners are known, but primary accessibility guidance helps sharpen what those owners should keep proving. | High |

## 2. Source Ledger

| Source ID | Title | Publisher / Authority | URL or Reference | Published Date | Last Updated Date | Accessed / Verified Date | Why This Source Matters | Temporal Volatility / Freshness Risk | Confidence |
|---|---|---|---|---|---|---|---|---|---|
| SRC-01 | Human Interface Guidelines | Apple Developer | [https://developer.apple.com/design/human-interface-guidelines/](https://developer.apple.com/design/human-interface-guidelines/) | Not stated | Not stated | 2026-03-30 | Establishes platform-level hierarchy, harmony, and consistency principles that support one coherent design system. | Medium: HIG language evolves, but the principles are stable. | High |
| SRC-02 | Designing for macOS | Apple Developer | [https://developer.apple.com/design/human-interface-guidelines/designing-for-macos](https://developer.apple.com/design/human-interface-guidelines/designing-for-macos) | Not stated | Not stated | 2026-03-30 | Grounds macOS-specific expectations around large displays, comfortable density, toolbars, personalization, and keyboard-heavy work styles. | Medium. | High |
| SRC-03 | Accessibility | Apple Developer | [https://developer.apple.com/design/human-interface-guidelines/accessibility](https://developer.apple.com/design/human-interface-guidelines/accessibility) | Not stated | Not stated | 2026-03-30 | Primary Apple guidance on system colors, non-color-only meaning, and auditing accessibility. | Medium. | High |
| SRC-04 | VoiceOver | Apple Developer | [https://developer.apple.com/design/human-interface-guidelines/voiceover](https://developer.apple.com/design/human-interface-guidelines/voiceover) | Not stated | March 7, 2025 | 2026-03-30 | Primary Apple guidance on labels, grouping, reading order, and announcing layout changes. | Medium. | High |
| SRC-05 | Menus | Apple Developer | [https://developer.apple.com/design/human-interface-guidelines/menus](https://developer.apple.com/design/human-interface-guidelines/menus) | Not stated | Not stated | 2026-03-30 | Gives concrete icon-usage guidance: use familiar system icons for common actions and avoid ornamental custom icons. | Medium. | High |
| SRC-06 | What’s new in SF Symbols | Apple WWDC21 | [https://developer.apple.com/videos/play/wwdc2021/10097/](https://developer.apple.com/videos/play/wwdc2021/10097/) | 2021 | Not stated | 2026-03-30 | Official guidance that SF Symbols integrate with San Francisco, support accessibility features, and are meant for consistent iconography. | Low-Medium. | High |
| SRC-07 | Add an app icon | Apple App Store Connect Help | [https://developer.apple.com/help/app-store-connect/manage-app-information/add-an-app-icon](https://developer.apple.com/help/app-store-connect/manage-app-information/add-an-app-icon) | Not stated | Not stated | 2026-03-30 | Confirms the app icon is the app’s representation in key system locations and belongs in the asset-catalog/app-icon lane. | Medium. | High |
| SRC-08 | Differentiate Without Color Alone evaluation criteria | Apple App Store Connect Help | [https://developer.apple.com/help/app-store-connect/manage-app-accessibility/differentiate-without-color-alone-accessibility-evaluation-criteria](https://developer.apple.com/help/app-store-connect/manage-app-accessibility/differentiate-without-color-alone-accessibility-evaluation-criteria) | Not stated | Not stated | 2026-03-30 | Gives concrete official evaluation criteria for non-color-only differentiation in common tasks. | Medium. | High |
| SRC-09 | Sufficient Contrast evaluation criteria | Apple App Store Connect Help | [https://developer.apple.com/help/app-store-connect/manage-app-accessibility/sufficient-contrast-evaluation-criteria/](https://developer.apple.com/help/app-store-connect/manage-app-accessibility/sufficient-contrast-evaluation-criteria/) | Not stated | Not stated | 2026-03-30 | Gives concrete testing expectations for Increase Contrast, Reduce Transparency, dark mode, and non-text contrast. | Medium. | High |
| SRC-10 | VoiceOver evaluation criteria | Apple App Store Connect Help | [https://developer.apple.com/help/app-store-connect/manage-app-accessibility/voiceover-evaluation-criteria](https://developer.apple.com/help/app-store-connect/manage-app-accessibility/voiceover-evaluation-criteria) | Not stated | Not stated | 2026-03-30 | Gives concrete criteria for labels, logical order, modal focus, keyboard continuity, and status announcements under VoiceOver. | Medium. | High |

## 3. Findings by Theme

### Apple / iOS Platform Conventions

- Finding ID: `FIND-APPLE-01`
  Research question IDs: `RQ-01`
  Source IDs: `SRC-01`, `SRC-02`
  Source-backed finding: Apple’s HIG stresses hierarchy, harmony, and consistency, while the macOS guidance emphasizes comfortable information density, toolbar/menu-bar integration, keyboard shortcuts, and personalization rather than ornamental chrome.
  Model inference / host-system note: Proposal 014’s updated “extend the existing shared owners” strategy is better aligned with Apple’s platform conventions than a second token system would be. For Chainworks Forge, design-system work should continue to clarify hierarchy on existing operator surfaces, not add a parallel aesthetic layer.
  Host-system surface touched: `DesignTokens`, `ContentView`, `RunsHomeView`, `WorkflowMapView`, shell and run-centric hierarchy
  Time-sensitive: `No`
  Confidence: `High`

- Finding ID: `FIND-APPLE-02`
  Research question IDs: `RQ-01`, `RQ-02`
  Source IDs: `SRC-05`, `SRC-06`
  Source-backed finding: Apple’s menu guidance says to use familiar system icons for common actions, not every action needs an icon, and icons shouldn’t be added for ornamentation. Apple’s SF Symbols guidance says symbols integrate with San Francisco and support accessibility features like Dynamic Type and Bold Text.
  Model inference / host-system note: Proposal 014 is strongest when custom brand iconography stays bounded and SF Symbols remain the default for dense operational controls unless a branded symbol is equally legible and semantically obvious. That supports the draft’s “brand-safe iconography without replacing clear SF Symbols blindly” rule.
  Host-system surface touched: `StatusCapsule`, menu/toolbar actions, `ApprovalGateView`, `ReleaseGateView`, `ProviderSettingsView`, `GooseProviderConnectionAssistantView`
  Time-sensitive: `Medium`
  Confidence: `High`

- Finding ID: `FIND-APPLE-03`
  Research question IDs: `RQ-02`
  Source IDs: `SRC-07`
  Source-backed finding: Apple states that the app icon is the app’s representation and that it appears in key system locations, with asset-catalog / Icon Composer ownership in the app-icon pipeline.
  Model inference / host-system note: This reinforces Proposal 014’s bounded brand-lane rule: the strongest product identity anchor is the app icon and approved asset lane, not repeated full-logo treatment across operational screens.
  Host-system surface touched: `Assets.xcassets`, app icon pipeline, launch/setup-adjacent identity anchors
  Time-sensitive: `Medium`
  Confidence: `High`

### Accessibility / Dynamic Type / VoiceOver / Contrast

- Finding ID: `FIND-A11Y-01`
  Research question IDs: `RQ-03`
  Source IDs: `SRC-03`, `SRC-08`
  Source-backed finding: Apple’s accessibility guidance says to prefer system-defined colors and convey information with more than color alone. The App Store Connect “Differentiate Without Color Alone” criteria go further: common tasks shouldn’t rely on color as the sole differentiator, and shape, placement, order, icons, or labels should supplement color by default.
  Model inference / host-system note: Proposal 014’s decision to keep `StatusCapsule` textual and to anchor proof on adopter-slice owner surfaces remains well grounded. For Chainworks Forge, badge/chip styling should keep text/icon/shape cues first and brand accent second.
  Host-system surface touched: `StatusCapsule`, `RunsHomeView`, `WorkflowMapView`, `ReleaseGateView`, `DeliveryPreflightReportView`, touched `IdeaListView` chips
  Time-sensitive: `Medium`
  Confidence: `High`

- Finding ID: `FIND-A11Y-02`
  Research question IDs: `RQ-03`
  Source IDs: `SRC-09`
  Source-backed finding: Apple’s sufficient-contrast criteria recommend testing with Increase Contrast and Reduce Transparency, checking both light and dark interfaces, using Accessibility Inspector, and treating non-text state indicators as needing sufficient contrast too.
  Model inference / host-system note: Proposal 014’s current proof lane is right to keep `proposal-012` as the min-window and bounded accessibility owner. The most defensible brand rollout is one that keeps contrast/transparency checks attached to those same owner surfaces rather than inventing a separate aesthetic-only screenshot pack.
  Host-system surface touched: `proposal-012` proof lane, `StatusCapsule`, panel/background helpers, secondary surfaces with material/transparency treatment
  Time-sensitive: `Medium`
  Confidence: `High`

- Finding ID: `FIND-A11Y-03`
  Research question IDs: `RQ-03`
  Source IDs: `SRC-04`, `SRC-10`
  Source-backed finding: Apple’s VoiceOver guidance and evaluation criteria require concise labels, meaningful grouping, logical reading order, modal focus movement, timely status announcements, and keyboard operations that continue to work with VoiceOver active.
  Model inference / host-system note: This strongly supports Proposal 014’s current decision to keep the proof lane attached to existing owner-level runtime surfaces. For Chainworks Forge, status banners, modals, approvals, and recovery sheets should be judged by whether VoiceOver and keyboard users can complete common tasks without ambiguity, not by visual polish alone.
  Host-system surface touched: `ForegroundBannerView`, `ApprovalGateView`, `RecoverySheet`, modal/setup flows, `ui-smoke` and `proposal-012` proof owners
  Time-sensitive: `Medium`
  Confidence: `High`

### Testing Strategy

- Finding ID: `FIND-TEST-01`
  Research question IDs: `RQ-03`
  Source IDs: `SRC-03`, `SRC-08`, `SRC-09`, `SRC-10`
  Source-backed finding: Apple repeatedly frames accessibility claims in terms of “common tasks” and encourages auditing with system settings and Accessibility Inspector, not only static appearance review.
  Model inference / host-system note: Proposal 014’s updated verification contract is correctly shaped when it keeps preview-backed proof for visual drift, but leaves final accessibility/no-regression sign-off on the existing runtime owners (`proposal-012`, `proposal-006`, `ui-smoke`) that already exercise operator tasks.
  Host-system surface touched: preview-backed owner renders, `proposal-012`, `proposal-006`, `ui-smoke`
  Time-sensitive: `Medium`
  Confidence: `High`

## 4. Host-System Applicability Matrix

| Insight ID | Source IDs | Classification (`Adopt | Adapt | Watch | Reject`) | Proposal Area Affected | Host-System Surface Touched | Why It Applies or Does Not Apply | Concrete Recommended Change |
|---|---|---|---|---|---|---|
| APP-01 | `SRC-01`, `SRC-02` | Adopt | Layers `Q`/`R`, Sections `5`, `6`, `9` | shared token/primitive owners and shell hierarchy | Apple’s platform guidance favors coherent, consistent system evolution over per-surface style drift. | Keep the current “extend existing authority” model and resist any later temptation to reintroduce a second token namespace. |
| APP-02 | `SRC-05`, `SRC-06` | Adopt | Section `7`, Appendix `C` | icon usage rules across dense operational surfaces | Apple’s icon guidance strongly favors familiar system symbols for common actions and warns against ornamental icon usage. | Keep SF Symbols as the default for dense controls and use branded iconography only where it remains equally legible and semantically clear. |
| APP-03 | `SRC-07` | Adopt | Section `7`, asset-lane planning | `Assets.xcassets`, app icon, launch/setup-adjacent identity anchors | Apple treats the app icon as the app’s canonical system representation. | Keep full-brand emphasis concentrated in the app-icon and approved asset lane rather than repeated logos in operational panels. |
| APP-04 | `SRC-03`, `SRC-08`, `SRC-09`, `SRC-10` | Adopt | Section `8`, Section `10` | bounded adopter slice and proof owners | Apple’s accessibility and evaluation criteria map cleanly onto the current `proposal-012` / `ui-smoke` proof lane. | Keep proof anchored to common-task owner surfaces with system settings enabled, not to a separate screenshot-only lane. |
| APP-05 | `SRC-06` | Adapt | Section `7.4`, iconography rollout details | brand iconography and rendering modes | SF Symbols guidance is about Apple’s symbol system, not custom product branding directly. | Borrow the legibility, scale, alignment, and accessibility principles, but do not treat SF Symbols styling as a mandate to replace the product’s branded asset lane. |
| APP-06 | `SRC-07` | Watch | brand/logo placement language | full-logo usage in launch/setup/docs | Apple’s app icon guidance confirms the system representation role but does not directly answer every in-app branding placement question. | Recheck Apple HIG app-icons / icons pages if Proposal 014 later expands beyond bounded operator-surface branding into launch or onboarding identity design. |

## 5. Proposal Deltas / Recommended Updates

| Delta ID | Proposal Section / Decision | Recommended Update | Why It Helps | Supporting Source IDs | Supporting Local Evidence IDs | Priority |
|---|---|---|---|---|---|---|
| DELTA-01 | Section `7.4` iconography adoption | Optional: add one sentence that brand-safe iconography must preserve the familiarity of common operational actions and should defer to system symbols when the branded alternative is less obvious. | Makes the already-correct bounded icon rule even closer to Apple’s explicit guidance on familiar icons and non-ornamental use. | `SRC-05`, `SRC-06` | `REAL-05`, `MAP-07` | Medium |
| DELTA-02 | Section `10` verification criteria | Optional: add one sentence that the accessibility/no-regression gates are expected to cover “common tasks” on the bounded owner surfaces, consistent with Apple’s evaluation criteria. | Strengthens the connection between the current proof lane and Apple’s accessibility evaluation model. | `SRC-08`, `SRC-09`, `SRC-10` | `REAL-02`, `MAP-10`, `MAP-11` | Medium |
| DELTA-03 | Section `7.2` app integration points | Optional: add one sentence that the app icon remains the strongest system-level identity anchor, so repeated in-app logo treatment should stay exceptional and subordinate. | Aligns the draft’s bounded brand lane with Apple’s app-icon/system-representation framing. | `SRC-07` | `MAP-12`, `REAL-05` | Low |

## 6. Freshness Risks / Recheck Triggers

| Trigger ID | Claim / Recommendation | Why It Is Time-Sensitive | What Must Be Rechecked | Recheck Trigger / Window | Source IDs |
|---|---|---|---|---|---|
| FRESH-01 | Apple accessibility evaluation criteria continue to map cleanly onto the repo’s current proof owners | Apple updates App Store accessibility evaluation docs over time. | Differentiate Without Color, Sufficient Contrast, and VoiceOver evaluation pages. | Recheck when expanding the adopter slice or changing the proof lane. | `SRC-08`, `SRC-09`, `SRC-10` |
| FRESH-02 | SF Symbols guidance still supports the current bounded “use system icons where familiar actions matter most” posture | SF Symbols guidance and rendering capabilities continue to evolve. | SF Symbols HIG / WWDC guidance if Proposal 014 later expands custom iconography or symbol effects materially. | Recheck at implementation start for the iconography pass. | `SRC-05`, `SRC-06` |
| FRESH-03 | App icon remains the primary system identity anchor for the app | Apple’s app-icon pipeline and HIG guidance can evolve with tooling such as Icon Composer. | App-icon guidance in HIG and App Store Connect help. | Recheck when the asset-lane/app-icon phase starts. | `SRC-07` |

## 7. Remaining Open Questions

- QUESTION-01: If Proposal 014 later adds a stronger branded toolbar or titlebar moment, does the repo want a dedicated bounded checklist for those identity anchors, or should they remain covered only by the existing preview/runtime owners?
- QUESTION-02: Does the repo want an explicit “brand-safe icon fallback to SF Symbols” decision table for dense operational actions, or is the current prose sufficient for implementation?
