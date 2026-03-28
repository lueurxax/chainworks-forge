# Proposal 012 Research Pack

## 0. Review Target and Local Context Consumed
- Proposal: `docs/proposals/012-ui-quality-audit-and-visual-polish.md`
- Research round: `R1` on `2026-03-28`
- Proposal evidence pack used: `docs/reviews/012-ui-quality-audit-and-visual-polish-evidence-pack.md`
- Current-system baseline used: `none` (`.review-baselines/current-system-baseline.md` is missing)
- Proposal-specific integration context used: `none`
- Existing research pack reused: `none`
- Adjacent docs consumed:
  - `docs/reference/idea-lifecycle.md`
  - `docs/reference/live-workflow-map.md`
  - current 012 review artifact
  - current 012 evidence pack
- Current code / module mapping consumed:
  - `ContentView.swift`
  - `Views/RunsHomeView.swift`
  - `Views/IdeaListView.swift`
  - `Views/ProviderSettingsView.swift`
  - `Views/PilotReadinessView.swift`
  - `Views/FirstRunSetupWizard.swift`
  - `Views/GooseProviderConnectionAssistantView.swift`
  - `Views/WorkflowMapView.swift`
  - `Views/ReleaseGateView.swift`
  - `Views/ForegroundBannerView.swift`
  - `Views/DeliveryPreflightReportView.swift`
  - `Views/ApprovalGateView.swift`
  - `Views/RecoverySheet.swift`
  - `Views/ArchivedIdeasView.swift`
  - `Views/RunStartOverridesView.swift`
- Local evidence IDs that triggered research:
  - `RSH-01`
  - `RSH-02`
  - `RSH-03`
  - `RSH-04`
  - `MAP-01`–`MAP-11`
  - `REAL-02`
  - `REAL-03`
  - `REAL-04`
  - `TEST-01`
- Notes on baseline freshness or local contradictions:
  - Proposal 012 is already `Green` in proposal-readiness.
  - This research round is additive. It does not reopen the closed local blockers.
  - The host-system baseline artifact is still missing, so applicability was derived from direct local code mapping rather than reusable baseline reuse.

## 1. Research Questions Derived from Local Evidence
| Question ID | Derived From (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Research Question | Why Local Evidence Is Not Enough | Priority |
|---|---|---|---|---|---|
| RQ-01 | `Unresolved tradeoff` | `RSH-01`, `MAP-01`, `MAP-02`, `MAP-04`, `MAP-05`, `MAP-06`, `INT-01`, `INT-02` | What current Apple macOS guidance should shape Proposal 012's density, hierarchy, and "important actions stay visible" decisions on split views, settings walls, onboarding, and below-the-fold detail actions? | Local evidence shows where the pain is, but not the current Apple-preferred boundary between comfortable density, progressive disclosure, and command surfacing on macOS. | High |
| RQ-02 | `Proposal gap` | `RSH-02`, `MAP-04`, `MAP-05`, `MAP-06`, `MAP-07`, `MAP-09`, `DATA-01`, `DATA-03` | What current Apple guidance should govern inline loading/success/failure feedback versus modal alerts on recoverable operator surfaces? | Section 3.2 assigns ownership, but local evidence alone does not show whether the proposal's local-feedback bias fully matches current Apple guidance. | High |
| RQ-03 | `Unresolved tradeoff` | `RSH-03`, `MAP-06`, `MAP-09`, `MAP-11`, `REAL-03`, `REAL-04` | What current Apple guidance should govern keyboard-only workflows, confirm/dismiss bindings, and shortcut collision avoidance on macOS operator flows? | Local evidence shows which flows lack shortcuts today, but not which convention set Apple currently recommends for keyboard-first operation and modal dismissal. | High |
| RQ-04 | `Proposal gap` | `RSH-04`, `MAP-08`, `MAP-09`, `MAP-10`, `MAP-11`, `REAL-02` | What current Apple and WCAG guidance should shape status capsules, non-color differentiation, contrast, VoiceOver state announcements, and focus visibility for custom UI polish work? | Proposal 012 now aligns internally, but local evidence alone does not define current external expectations for contrast, non-color cues, or VoiceOver/state semantics on custom or semi-custom status surfaces. | High |

## 2. Source Ledger
| Source ID | Title | Publisher / Authority | URL or Reference | Published Date | Last Updated Date | Accessed / Verified Date | Why This Source Matters | Temporal Volatility / Freshness Risk | Confidence |
|---|---|---|---|---|---|---|---|---|---|
| SRC-01 | Designing for macOS | Apple Developer Documentation | https://developer.apple.com/design/human-interface-guidelines/designing-for-macos | not stated | not stated | 2026-03-28 | Defines current macOS principles for density, modality, menu-bar commands, keyboard shortcuts, and personalization. | `Medium` — revisit after WWDC or major HIG refresh. | High |
| SRC-02 | Loading | Apple Developer Documentation | https://developer.apple.com/design/human-interface-guidelines/loading | 2024-06-10 | 2025-06-09 | 2026-03-28 | Provides current Apple guidance for loading placeholders, background loading, and when to show progress indicators. | `Medium` — loading guidance evolves with HIG updates. | High |
| SRC-03 | Alerts | Apple Developer Documentation | https://developer.apple.com/design/human-interface-guidelines/alerts | not stated | not stated | 2026-03-28 | Establishes when alerts are appropriate, when they are too interruptive, and what cancel/default action patterns people expect. | `Medium` — recheck if alert guidance changes in HIG. | High |
| SRC-04 | Accessibility | Apple Developer Documentation | https://developer.apple.com/design/human-interface-guidelines/accessibility | not stated | 2025-06-09 | 2026-03-28 | Defines current Apple accessibility heuristics for keyboard-alone use, non-color cues, familiar interactions, contrast, and audits. | `Medium` — recheck after Apple accessibility guidance updates. | High |
| SRC-05 | VoiceOver evaluation criteria | App Store Connect Help / Apple Developer | https://developer.apple.com/help/app-store-connect/manage-app-accessibility/voiceover-evaluation-criteria | not stated | not stated | 2026-03-28 | Gives concrete Apple evaluation criteria for common-task completion, labels, state announcements, modal dismissal, and custom controls. | `Medium` — App Store criteria can evolve. | High |
| SRC-06 | Differentiate Without Color Alone evaluation criteria | App Store Connect Help / Apple Developer | https://developer.apple.com/help/app-store-connect/manage-app-accessibility/differentiate-without-color-alone-evaluation-criteria | not stated | not stated | 2026-03-28 | Gives concrete Apple guidance on non-color differentiation and grayscale testing for common tasks. | `Medium` — App Store criteria can evolve. | High |
| SRC-07 | Sufficient Contrast evaluation criteria | App Store Connect Help / Apple Developer | https://developer.apple.com/help/app-store-connect/manage-app-accessibility/sufficient-contrast-evaluation-criteria | not stated | not stated | 2026-03-28 | Gives concrete Apple guidance on text contrast, non-text contrast, Increase Contrast, Reduce Transparency, and Dark Mode checks. | `Medium` — App Store criteria can evolve. | High |
| SRC-08 | Understanding SC 1.4.1: Use of Color | W3C WAI | https://www.w3.org/WAI/WCAG22/Understanding/use-of-color | not stated | not stated | 2026-03-28 | Supplies normative accessibility reasoning for not conveying meaning through color alone. | `Low` — stable standard explanatory guidance. | High |
| SRC-09 | Understanding SC 1.4.11: Non-text Contrast | W3C WAI | https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast | not stated | not stated | 2026-03-28 | Supplies normative accessibility reasoning for 3:1 contrast on controls, states, and meaningful graphics. | `Low` — stable standard explanatory guidance. | High |
| SRC-10 | Scroll views | Apple Developer Documentation | https://developer.apple.com/design/human-interface-guidelines/scroll-views | not stated | not stated | 2026-03-28 | Provides Apple guidance on scroll discoverability, keyboard shortcuts, and bringing relevant content into view. | `Medium` — recheck if scroll-view guidance changes materially. | Medium |

## 3. Findings by Theme

### Apple / iOS Platform Conventions
- Finding ID: `RES-PLAT-01`
  Research question IDs: `RQ-01`
  Source IDs: `SRC-01`, `SRC-10`
  Source-backed finding:
  - Apple says macOS should leverage large displays to present more content in fewer nested levels and with less need for modality, while maintaining comfortable information density.
  - Apple also notes that scroll indicators are not always visible and that automatic scrolling should help bring relevant hidden content into view when context changes.
  Model inference / host-system note:
  - Proposal 012 is aligned directionally, but implementation should treat "high-value actions remain visible" as a macOS convention issue, not just a polish preference.
  - `RunDetailPanel`, `ProviderSettingsView`, `PilotReadinessView`, and `FirstRunSetupWizard` should favor always-discoverable commands, grouped sections, and minimal nesting over deep scroll-only discovery.
  Host-system surface touched:
  - `RunsHomeView`
  - `RunDetailPanel`
  - `ProviderSettingsView`
  - `PilotReadinessView`
  - `FirstRunSetupWizard`
  Time-sensitive: `Medium`
  Confidence: `High`

- Finding ID: `RES-PLAT-02`
  Research question IDs: `RQ-03`
  Source IDs: `SRC-01`
  Source-backed finding:
  - Apple explicitly calls out the menu bar and keyboard shortcuts as part of the familiar macOS command surface, and says apps should help people accelerate actions and use keyboard-only work styles.
  Model inference / host-system note:
  - Proposal 012 should keep shortcut ownership intentionally small and stable, but the implementation should also think in terms of command exposure, not only button bindings.
  - If `RunDetailPanel` actions move out of the scroll body, a toolbar or command path may be a better macOS fit than simply adding more inline buttons.
  Host-system surface touched:
  - `RunDetailPanel`
  - `ApprovalGateView`
  - `ReleaseGateView`
  - `RecoverySheet`
  - `FirstRunSetupWizard`
  Time-sensitive: `Medium`
  Confidence: `High`

### Accessibility / Dynamic Type / VoiceOver / Contrast
- Finding ID: `RES-A11Y-01`
  Research question IDs: `RQ-03`, `RQ-04`
  Source IDs: `SRC-04`, `SRC-05`
  Source-backed finding:
  - Apple says accessible interfaces must not rely on a single interaction method, should support Full Keyboard Access, and should avoid overriding system-defined keyboard shortcuts.
  - Apple’s VoiceOver criteria require common tasks to be completable using only VoiceOver; controls need concise labels and state/value semantics; modal views should support dismissal with Escape or `accessibilityPerformEscape`; and custom controls should provide native-equivalent accessibility.
  Model inference / host-system note:
  - Proposal 012 already asks for keyboard and VoiceOver verification, but current external guidance supports making that more concrete in implementation review: check Full Keyboard Access traversal, Escape dismissal on modal operator surfaces, and VoiceOver labels/traits on any custom badges, cards, or workflow states.
  Host-system surface touched:
  - `ApprovalGateView`
  - `ReleaseGateView`
  - `RecoverySheet`
  - `FirstRunSetupWizard`
  - `WorkflowMapView`
  - shared status components
  Time-sensitive: `Medium`
  Confidence: `High`

- Finding ID: `RES-A11Y-02`
  Research question IDs: `RQ-04`
  Source IDs: `SRC-06`, `SRC-08`
  Source-backed finding:
  - Apple says users should not have to rely on color alone to distinguish states or values; add text labels or icons and test with grayscale.
  - WCAG explains that color cannot be the only means of conveying information, indicating an action, prompting a response, or distinguishing a visual element.
  Model inference / host-system note:
  - `StatusCapsule`, `WorkflowMapStatusBadge`, release artifact states, and recovery/approval status indicators should preserve a textual or iconographic distinction in addition to tint.
  - Proposal 012 already moves in this direction; research says this should be treated as a required implementation quality bar, not a stylistic bonus.
  Host-system surface touched:
  - `StatusCapsule`
  - `WorkflowMapView`
  - `ReleaseGateView`
  - `DeliveryPreflightReportView`
  - `ApprovalGateView`
  - `RecoverySheet`
  Time-sensitive: `Medium`
  Confidence: `High`

- Finding ID: `RES-A11Y-03`
  Research question IDs: `RQ-04`
  Source IDs: `SRC-07`, `SRC-09`, `SRC-04`
  Source-backed finding:
  - Apple recommends checking text contrast at about `4.5:1`, non-text state indicators at about `3:1`, and testing Dark Mode together with Increase Contrast and Reduce Transparency.
  - WCAG requires `3:1` contrast for UI component states and other meaningful non-text cues against adjacent colors.
  Model inference / host-system note:
  - Proposal 012’s bounded design-system rollout should add explicit contrast/focus verification for badges, chips, cards, hover states, and any translucent materials introduced or restyled in Phases 1–3.
  Host-system surface touched:
  - `StatusCapsule`
  - `WorkflowMapView`
  - `ReleaseGateView`
  - `DeliveryPreflightReportView`
  - `IdeaListView` summary chips
  - custom focus or hover states
  Time-sensitive: `Medium`
  Confidence: `High`

### Testing Strategy
- Finding ID: `RES-TEST-01`
  Research question IDs: `RQ-03`, `RQ-04`
  Source IDs: `SRC-05`, `SRC-06`, `SRC-07`
  Source-backed finding:
  - Apple’s App Store accessibility evaluation pages repeatedly frame VoiceOver, Differentiate Without Color Alone, and Sufficient Contrast as criteria that should be re-evaluated every time an app is updated.
  Model inference / host-system note:
  - Proposal 012 is a good candidate for an implementation-review checklist that explicitly reruns VoiceOver, grayscale/non-color checks, and contrast checks after each Phase 1–3 landing, not only at the very end of the slice.
  Host-system surface touched:
  - all audited surfaces in Appendix A that adopt new hierarchy, status, or feedback treatment
  Time-sensitive: `Medium`
  Confidence: `High`

### Consumer-Finance Trust / Transparency / Recovery
- Finding ID: `RES-TRUST-01`
  Research question IDs: `RQ-02`
  Source IDs: `SRC-02`, `SRC-03`
  Source-backed finding:
  - Apple’s loading guidance says to show something as soon as possible, let people do other things while waiting when possible, and use progress indicators only when the delay is long enough to need reassurance.
  - Apple’s alerts guidance says to use alerts sparingly, avoid alerts that are merely informative, and prefer contextual communication; for startup or network problems, Apple explicitly suggests cached or placeholder data with a nonintrusive label.
  Model inference / host-system note:
  - Proposal 012’s local-feedback bias is externally supported.
  - This is especially relevant for `ProviderSettingsView`, `PilotReadinessView`, `GooseProviderConnectionAssistantView`, `FirstRunSetupWizard`, and `ReleaseGateView`, where false-alarm warning states or modal interruption would undermine trust more than help it.
  Host-system surface touched:
  - `ProviderSettingsView`
  - `PilotReadinessView`
  - `GooseProviderConnectionAssistantView`
  - `FirstRunSetupWizard`
  - `ReleaseGateView`
  Time-sensitive: `Medium`
  Confidence: `High`

## 4. Host-System Applicability Matrix
| Insight ID | Source IDs | Classification (`Adopt | Adapt | Watch | Reject`) | Proposal Area Affected | Host-System Surface Touched | Why It Applies or Does Not Apply | Concrete Recommended Change |
|---|---|---|---|---|---|---|
| APP-01 | `SRC-01`, `SRC-10` | `Adopt` | `C-01`, `H-01`, `H-02`, `H-03`, `L-10` | `RunsHomeView`, `RunDetailPanel`, `ProviderSettingsView`, `PilotReadinessView`, `FirstRunSetupWizard` | The proposal already targets density/hierarchy issues on macOS productivity surfaces; Apple directly endorses fewer nested levels, less modality, and clearer visibility of relevant content. | Keep important commands visible above the fold or in a toolbar/sticky footer; move rare advanced options behind disclosure or a secondary destination instead of leaving them inline. |
| APP-02 | `SRC-02`, `SRC-03` | `Adopt` | Section `3.2`, `L-06`, `L-12` | `ProviderSettingsView`, `PilotReadinessView`, `GooseProviderConnectionAssistantView`, `FirstRunSetupWizard`, `ReleaseGateView` | Apple strongly favors contextual loading/error communication and warns against alerts that merely provide information. This matches the proposal’s trust-bearing non-happy-path goals. | Treat inline or section-local progress/error UI as the default. Reserve alerts for uncommon destructive or non-undoable confirmations. Use neutral placeholder/nonintrusive states for "not yet produced" or connectivity issues. |
| APP-03 | `SRC-01`, `SRC-04`, `SRC-05`, `SRC-03` | `Adapt` | `L-09`, Section `6` item `5` | `ApprovalGateView`, `ReleaseGateView`, `RecoverySheet`, `FirstRunSetupWizard`, `RunDetailPanel` | External guidance supports keyboard-only work styles and quick dismissal paths, but it does not dictate a one-size-fits-all binding set for these specific operator actions. | Document a small shortcut convention set in implementation: preserve system-defined shortcuts, support Escape/Cancel where appropriate, and expose primary commands through native macOS command surfaces when useful. |
| APP-04 | `SRC-06`, `SRC-08` | `Adopt` | `M-01`, `M-02`, `L-06`, Section `6` accessibility | `StatusCapsule`, `WorkflowMapView`, `ReleaseGateView`, `DeliveryPreflightReportView`, `ApprovalGateView`, `RecoverySheet` | Proposal 012 introduces shared status semantics. Apple and WCAG both say color can reinforce meaning, but cannot be the only carrier. | Require text/icon/shape differentiation for status and artifact states; explicitly test grayscale and distinguish "not yet produced" from warning/error states without color alone. |
| APP-05 | `SRC-07`, `SRC-09`, `SRC-04` | `Adopt` | `M-01`, `M-02`, `M-03`, Section `6` accessibility | shared badges, chips, cards, focus/hover states, translucent surfaces | The proposal’s bounded design-system slice increases the chance that one contrast/focus bug gets propagated across multiple surfaces. | Add acceptance checks for `4.5:1` text contrast, `3:1` non-text state contrast, and Dark Mode + Increase Contrast + Reduce Transparency combinations on adopter-slice surfaces. |
| APP-06 | `SRC-05` | `Adopt` | Section `6` accessibility, Appendix A interaction checklist surfaces | `ApprovalGateView`, `RecoverySheet`, `ReleaseGateView`, `WorkflowMapView`, shared custom status elements | VoiceOver criteria map directly to the proposal’s custom or semi-custom operator surfaces. | Verify every common task is completable with VoiceOver only; add concise labels/traits; ensure modal dismissal via Escape or equivalent; keep custom elements at native-equivalent accessibility fidelity. |
| APP-07 | `SRC-01`, `SRC-04` | `Watch` | `L-08` shell grouping and future command/personalization work | `ContentView` tab shell, future toolbar/command surfaces | Apple encourages personalization and command exposure, but Proposal 012 intentionally keeps shell regrouping as a non-breaking, lower-priority note. | Do not broaden Proposal 012 scope now. Revisit after Phase 1–3 if actions still feel buried or shell density remains problematic. |
| APP-08 | `SRC-06`, `SRC-04` | `Reject` | any proposal extension that adds a custom app-specific accessibility mode or custom shortcut subsystem | whole app | Current external guidance favors default accessible design plus system accessibility features. Proposal 012 is a bounded polish slice, not an accessibility-settings subsystem proposal. | Do not add bespoke "color-blind mode", app-specific shortcut framework, or new accessibility settings in Proposal 012. Improve the default UI and system-feature compatibility first. |

## 5. Proposal Deltas / Recommended Updates
| Delta ID | Proposal Section / Decision | Recommended Update | Why It Helps | Supporting Source IDs | Supporting Local Evidence IDs | Priority |
|---|---|---|---|---|---|---|
| DELTA-01 | Section `6` accessibility audit | Add explicit grayscale / Differentiate Without Color Alone, Increase Contrast, and Reduce Transparency checks for shared badges, chips, cards, and other restyled non-text state indicators. | Converts the proposal’s generic accessibility audit into a stronger, externally grounded acceptance check for the exact custom/status surfaces it changes. | `SRC-04`, `SRC-06`, `SRC-07`, `SRC-08`, `SRC-09` | `REAL-02`, `MAP-08`, `MAP-09`, `MAP-10`, `MAP-11`, `TEST-01` | `P1` |
| DELTA-02 | `L-09`, Section `6` item `5` | Add one sentence that custom bindings must avoid system-defined shortcuts and that modal confirm/dismiss flows must work for keyboard-only operation, including Escape-based dismissal where appropriate. | Keeps the shortcut work macOS-native and prevents the proposal from encouraging arbitrary or conflicting bindings. | `SRC-01`, `SRC-03`, `SRC-04`, `SRC-05` | `REAL-03`, `REAL-04`, `MAP-06`, `MAP-09`, `MAP-11` | `P1` |
| DELTA-03 | Section `3.2`, `L-06`, `L-12` | Add an implementation note that recoverable loading or connectivity issues should default to inline/local feedback or nonintrusive labels, while alerts are reserved for uncommon destructive or non-undoable confirmations. | Strengthens the proposal’s trust-bearing feedback model with current Apple guidance and reduces the chance of modal overuse. | `SRC-02`, `SRC-03` | `DATA-01`, `DATA-03`, `MAP-04`, `MAP-05`, `MAP-07`, `MAP-09` | `P1` |
| DELTA-04 | `H-01`, `H-02`, `H-03`, `L-10` | Add a macOS-specific implementation heuristic: main commands remain above the fold or in a toolbar/sticky footer; rare configuration or advanced details move behind disclosure or a secondary destination. | Turns a set of local polish fixes into one consistent macOS command-surfacing rule, reducing the chance of partial or inconsistent implementation. | `SRC-01`, `SRC-10` | `MAP-02`, `MAP-04`, `MAP-05`, `MAP-06`, `INT-02` | `P2` |
| DELTA-05 | Phase `3` adopter-slice guardrails | Add an explicit requirement that shared primitives preserve VoiceOver labels/traits and non-text contrast across the bounded adopter slice before rollout expands. | This prevents the new design-system slice from centralizing visual consistency while accidentally centralizing accessibility regressions. | `SRC-05`, `SRC-06`, `SRC-07`, `SRC-09` | `FLAG-01`, `REAL-02`, `TEST-01`, `MAP-08`, `MAP-09`, `MAP-10` | `P1` |

## 6. Freshness Risks / Recheck Triggers
| Trigger ID | Claim / Recommendation | Why It Is Time-Sensitive | What Must Be Rechecked | Recheck Trigger / Window | Source IDs |
|---|---|---|---|---|---|
| FRESH-01 | macOS density, modality, loading, alert, and accessibility heuristics | Apple HIG guidance changes across WWDC cycles and periodic HIG refreshes. | `Designing for macOS`, `Loading`, `Alerts`, and `Accessibility` pages | before implementation kickoff if work slips past the next WWDC or major HIG refresh | `SRC-01`, `SRC-02`, `SRC-03`, `SRC-04`, `SRC-10` |
| FRESH-02 | VoiceOver / Differentiate Without Color Alone / Sufficient Contrast evaluation use as sign-off criteria | App Store Connect accessibility criteria can change as Accessibility Nutrition Labels evolve. | current evaluation wording and any new platform-specific caveats | before using these checks as a formal sign-off or public accessibility claim | `SRC-05`, `SRC-06`, `SRC-07` |
| FRESH-03 | 3:1 / 4.5:1 contrast-derived recommendations for custom status elements | WCAG Understanding docs are stable, but examples and interpretation notes can evolve. | contrast guidance and any Apple-specific interpretation that supersedes it | if Proposal 012 turns the research into hard numeric acceptance criteria or expands the custom component surface | `SRC-07`, `SRC-08`, `SRC-09` |
| FRESH-04 | applicability of current deltas to the host system | Proposal 012 currently assumes the same shell, split view, and operator surfaces mapped in this round. | applicability matrix and affected-surface mapping | if a new baseline refresh materially changes shell/navigation ownership before implementation starts | `SRC-01`, `SRC-10` plus local `MAP-*` / `INT-*` |

## 7. Remaining Open Questions
- QUESTION-01: Should Proposal 012 absorb `DELTA-01` through `DELTA-05` now, or should they live as implementation-review criteria attached to the next execution slice?
- QUESTION-02: If `RunDetailPanel` actions move, should the canonical location be a toolbar, a sticky footer, or duplicated command exposure plus an inline action cluster?
- QUESTION-03: For `StatusCapsule`, is text plus tint sufficient across all surfaces, or do approval/release/recovery states need explicit icon or shape variants too?
- QUESTION-04: Should `ReleaseGateView` adopt a neutral iconography pattern for "Not yet produced" that is distinct from warning/error semantics even with color disabled?
