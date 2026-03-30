# Proposal 014: Design System Adoption and Brand Application

| Field | Value |
|---|---|
| Date | 2026-03-28 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/chainworks_forge_design_kit_v1.md](../reference/chainworks_forge_design_kit_v1.md), [reference/ui-quality-and-polish.md](../reference/ui-quality-and-polish.md), [reference/operator-experience.md](../reference/operator-experience.md), [reference/live-workflow-map.md](../reference/live-workflow-map.md), [reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md) |
| Scope | Brand-token adoption, shared UI primitives, icon/logo application, and surface-by-surface migration of the current macOS app toward the approved Chainworks Forge design system |
| Goal | Bring the shipped app into visual, typographic, and interaction alignment with the Design Kit v1 so Chainworks Forge reads as a coherent orchestration tool with a distinctive brand rather than a collection of functional SwiftUI screens. |

---

## 1. Context

The app now has enough runtime maturity that visual inconsistency is no longer a side issue.
There is already a stable UI-quality baseline in [reference/ui-quality-and-polish.md](../reference/ui-quality-and-polish.md), and a stable full-delivery runtime baseline in [reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md).
What was missing until now was one explicit visual authority for the brand and the UI language.

[chainworks_forge_design_kit_v1.md](../reference/chainworks_forge_design_kit_v1.md) now provides that authority:

- brand metaphor,
- color system,
- typography rules,
- iconography direction,
- UI hierarchy rules,
- motion constraints,
- suggested token structure.

Without a dedicated implementation proposal, that design kit will remain advisory.
The repo will keep accumulating:

- one-off badge styles,
- inconsistent color semantics,
- surfaces that look like internal tooling rather than a product,
- and partial visual changes that never add up to a coherent system.

Proposal 014 is the bounded implementation slice that turns the design kit into real app behavior.

### 1.1 Relationship to the UI quality baseline

The implemented UI-quality baseline remains the audit and readability authority.
It already defines:

- the audited surface inventory,
- the current UI debt backlog,
- non-happy-path state coverage,
- bounded shared-primitives guardrails.

Proposal 014 does not replace that work.
It builds on it and answers a different question:

> How do we apply the approved Chainworks Forge visual system across the app without regressing clarity, accessibility, or operator trust?

Rule of thumb:

- The UI quality baseline owns **readability, information hierarchy, and UI debt closure**.
- Proposal 014 owns **brand-system adoption, design tokens, icon/logo application, and surface migration into the design kit**.

When both slices touch the same surface, the UI quality baseline remains authoritative for interaction and state truth, while Proposal 014 remains authoritative for visual language.

### 1.2 What this proposal is not

Proposal 014 is **not**:

- a marketing-site redesign,
- a new workflow or information-architecture proposal,
- a rewrite of provider/run/recovery semantics,
- a light-mode expansion proposal,
- an excuse to add decorative motion or ornamental graphics.

This is a product-UI system rollout, not a branding exercise detached from runtime truth.

---

## 2. Product questions this proposal must answer

After Proposal 014, the engineer should be able to answer `yes` to all of these:

1. Does the app consistently look like Chainworks Forge rather than a generic prototype?
2. Do key surfaces reflect the design-kit hierarchy of `Run -> Stage -> Agent -> Artifact`?
3. Are color, typography, spacing, and badges driven by shared semantic tokens rather than ad-hoc per-view decisions?
4. Are logo, icon, and brand accents applied in bounded places that strengthen product identity without competing with workflow state?
5. Can the brand system be adopted without breaking keyboard behavior, accessibility, and operator trust?

Proposal 014 is done only when all five answers are explicit in code, previews, and verification evidence.

---

## 3. What we build

Proposal 014 delivers four tightly coupled layers.

### Layer Q: Existing design-system authority plus bounded extension

| Component | Responsibility |
|---|---|
| **DesignTokens** | Existing canonical token authority for status, action, spacing, radius, and typography |
| **StatusCapsule** | Existing bounded shared badge primitive |
| **StyledEmptyState** | Existing shared empty-state wrapper |
| **DesignTokens extension** | Add only the missing brand-aligned tokens that are not already represented in `DesignTokens` |
| **Brand Asset Set** | Logo variants, app icon assets, monochrome symbol assets, and usage rules |

### Layer R: Primitive completion, not primitive replacement

| Component | Responsibility |
|---|---|
| **StatusCapsule** | Keep as the canonical badge/chip primitive and extend only if design-kit coverage is missing |
| **Panel / card helpers** | Add shared panel styling only where current adopted surfaces still drift visually |
| **Section-header helpers** | Fill gaps in section-title consistency without creating a second typography authority |
| **StyledEmptyState enhancement** | Extend the existing wrapper if brand treatment needs to improve |
| **Icon usage rules** | Bounded mapping from product states/actions to brand-safe iconography without replacing clear SF Symbols blindly |

### Layer S: Current adoption rebaseline plus remaining migration

| Component | Responsibility |
|---|---|
| **Current adopted slice** | Record which shell, run-centric, and setup surfaces are already on `DesignTokens` / `StatusCapsule` |
| **Remaining migration pack** | Focus only on surfaces still visually outside the adopted slice or still missing design-kit behaviors |
| **Brand application pack** | Apply icons/logo/asset usage in bounded locations without reopening already-complete UI-quality work |
| **Secondary completion pack** | Finish archive, recovery, banner, and supporting surfaces that still visibly drift |

### Layer T: Existing proof-lane extension

| Component | Responsibility |
|---|---|
| **Preview-backed owners** | Per-surface visual proof for audited owner surfaces already defined by the UI-quality baseline |
| **`proposal-012` gate extension** | Canonical min-window, adopter-slice accessibility, and secondary runtime proof lane |
| **`ui-smoke` continuity proof** | Canonical shell-level no-regression lane |
| **Brand application checklist** | Bounded additional checks for logo/icon/token application attached to the existing proof owners |

---

## 4. Canonical design-system contract

The design authority for this proposal is:

- [reference/chainworks_forge_design_kit_v1.md](../reference/chainworks_forge_design_kit_v1.md)

Proposal 014 operationalizes that document with these implementation rules.

### 4.1 Visual hierarchy

At key operator surfaces, the UI must preserve the product hierarchy:

```text
Run -> Stage -> Agent -> Artifact
```

What that means in the app:

- `Run` remains the dominant unit in lists, detail views, and hero summaries,
- `Stage` remains the main context inside a run,
- `Agent` remains subordinate to stage context rather than competing with it,
- `Artifact` remains inspectable output rather than visual noise mixed into the main navigation layer.

### 4.2 Brand application rules

The brand system should communicate:

- orchestration,
- coordinated movement,
- control over a complex process,
- engineering clarity.

It must not make the product feel like:

- a chat app,
- a fantasy-AI brand exercise,
- a glossy marketing shell disconnected from runtime truth,
- a neon-on-black terminal parody.

### 4.3 Color rules

The design-kit palette becomes semantic code tokens, not decorative references.

Rules:

1. `ForgeAccent` is sparse and intentional.
2. Large surfaces do not become orange.
3. Status colors remain more important than decorative brand accents.
4. Action colors, status colors, and neutral surface colors must be separate namespaces.
5. Status meaning must never rely on color alone.

### 4.4 Typography rules

The product remains on the Apple system stack.

Rules:

1. screen and section hierarchy use the design-kit scale instead of per-view font improvisation,
2. uppercase remains limited to small utility labels and status chips,
3. spacing, weight, and grouping carry hierarchy before color does,
4. body and supporting text sizes stay readable in dense operator views.

### 4.5 Motion rules

Allowed motion stays restrained:

- subtle fade/slide for stage and surface transitions,
- subtle pulse for active/running indications,
- short pop-in for approval-gate emphasis,
- restrained badge/status transitions.

Disallowed:

- decorative looping motion,
- heavy spring animation for utility UI,
- recurring logo theatrics,
- motion that competes with workflow state.

---

## 5. Design-system file structure

Proposal 014 adopts the design kit's direction, but it does so by extending the current repo-local authority instead of creating a parallel `Forge*` stack beside it.

Initial target structure:

```text
Support/
  DesignTokens.swift
  StatusCapsule.swift
  StyledEmptyState.swift
  Design/
    DesignTokenExtensions.swift
    PanelStyles.swift
    SectionHeaderStyles.swift
    IconUsageRules.swift
Assets.xcassets/
  Brand/
  AppIcon.appiconset/
Design/
  Brand/
  Icons/
  AppIcon/
```

Rules:

1. `DesignTokens`, `StatusCapsule`, and `StyledEmptyState` remain the current canonical owners unless explicitly superseded in place.
2. Any new helper files extend that system; they do not create a second long-lived token namespace.
3. Asset-catalog integration may consume generated/finalized source assets from `Design/`.
4. No ad-hoc token namespace is introduced directly in view files.
5. Surfaces not yet migrated may temporarily keep old styling, but must not fork the shared owners.

Implementation guard:

- reject new view-local token namespaces on adopter surfaces,
- reject new ad-hoc badge/card primitives that duplicate `StatusCapsule` or the shared panel recipe,
- prefer lightweight drift guards for patterns such as `Color(red:`, local `Font.system(...)`, new capsule badges outside `StatusCapsule`, and local panel background/shadow recipes,
- treat temporary wrappers as migration shims only if they point back to the canonical owners and carry an explicit removal plan.

---

## 6. Surface rollout plan

Proposal 014 uses the audited-surface inventory from [reference/ui-quality-and-polish.md](../reference/ui-quality-and-polish.md) rather than inventing a new scope list.

### 6.1 Current adopted slice at `HEAD`

The current tree is not a blank slate.
These surfaces already use the bounded shared system in meaningful ways:

- `RunsHomeView`
- `IdeaListView`
- `WorkflowMapView`
- `ReleaseGateView`
- `DeliveryPreflightReportView`
- `ProviderSettingsView`
- `PilotReadinessView`
- `FirstRunSetupWizard`
- `GooseProviderConnectionAssistantView`

Proposal 014 therefore treats them as **rebaseline owners**, not untouched future adopters.

### 6.2 Phase 1: Canonical authority cleanup and token completion

First work items:

- `RunsHomeView`
- `ForegroundBannerView`
- `ContentView`
- shared support files under `Support/`

Goals:

- keep one explicit authority for tokens and primitives,
- add any missing panel/section/icon rules into the current shared system,
- remove residual visual drift in the app shell and banner path,
- keep the product hierarchy legible while introducing bounded brand identity.

### 6.3 Phase 2: Already-adopted surfaces that still need brand-level completion

Rebaseline-and-complete surfaces:

- `WorkflowMapView`
- `ApprovalGateView`
- `ReleaseGateView`
- `DeliveryPreflightReportView`
- `RunDetailPanel`
- `IdeaListView`

Goals:

- preserve the current `DesignTokens` / `StatusCapsule` ownership,
- finish design-kit hierarchy, panel, and brand-accent alignment on run-centric surfaces,
- make approval and release surfaces feel like trust-bearing product checkpoints,
- ensure workflow topology and delivery surfaces share one visual vocabulary.

### 6.4 Phase 3: Setup and remediation completion

Rebaseline-and-complete surfaces:

- `ProviderSettingsView`
- `PilotReadinessView`
- `FirstRunSetupWizard`
- `GooseProviderConnectionAssistantView`

Goals:

- bring already-adopted heavy setup surfaces into full design-kit alignment,
- keep important commands above the fold or in toolbars/sticky footers as required by the design kit,
- avoid turning dense setup forms into decorative layouts,
- keep troubleshooting and readiness semantics explicit.

### 6.5 Phase 4: Secondary and supporting surfaces

Final adopters:

- `ArchivedIdeasView`
- `NewIdeaSheetView`
- `RecoverySheet`
- supporting empty states and secondary panels touched by the earlier phases

Goals:

- finish consistency work,
- unify empty-state and helper surfaces,
- close remaining visual drift without reopening flow ownership.

### 6.6 Behavioral boundary for recovery and failed-stage surfaces

Proposal 014 does not own recovery behavior.
For `RecoverySheet`, `BlockedRunRecoveryView`, failed-stage evidence surfaces, and repair panels:

- Proposal 014 owns styling only:
  - spacing,
  - typography,
  - panel hierarchy,
  - icon/logo application,
  - bounded badge/empty-state treatment.
- Behavioral ownership remains with the recovery/runtime proposals and references:
  - Proposal 013 output-contract and recovery truth work,
  - current recovery/runtime reference docs.
- Proposal 014 must not redefine:
  - retry/resume semantics,
  - stage-settlement truth,
  - failed-stage evidence truth,
  - blocked-run repair logic,
  - recovery-action availability rules.

---

## 7. Brand asset adoption

### 7.1 Required asset outputs

Proposal 014 is not complete until the app has a bounded brand-asset lane:

- primary horizontal logo,
- square app icon version,
- monochrome symbol variant for small UI use,
- dark-background and light-background safe variants where needed.

### 7.2 App integration points

Bounded application points:

- app icon,
- toolbar/sidebar/title-bar brand usage where appropriate,
- first-run/setup or splash-adjacent surfaces if they already exist,
- docs/readme handoff surfaces if they consume product branding.

Non-goals:

- repeated logo use on every screen,
- large decorative hero art in operator views,
- replacing core runtime status indicators with branding.

### 7.3 Canonical asset naming and integration contract

| Canonical asset | Asset/catalog name | Allowed surfaces | Not allowed |
|---|---|---|---|
| Primary horizontal logo | `chainworks-forge-logo-horizontal` | README/docs, launch-adjacent setup surfaces, bounded shell/header moments that already carry product identity | dense operator panels, approval/release bodies, repeated per-screen branding |
| Square app icon master | `chainworks-forge-app-icon` | app icon pipeline, installer/export handoff, docs/app-icon references | inline content decoration inside operational screens |
| Monochrome product symbol | `chainworks-forge-symbol-monochrome` | compact branded shell affordances, small supporting marks where a product symbol is clearer than text | replacing legible operational SF Symbols in dense controls |
| Dark-safe hero/logo variant | `chainworks-forge-hero-dark` | README/docs and dark marketing-adjacent handoff surfaces | runtime operator panels |
| Light-safe hero/logo variant | `chainworks-forge-hero-light` | README/docs and light marketing-adjacent handoff surfaces | runtime operator panels |

Integration rules:

- full logo may be used only on docs, README, app-icon/launch-adjacent surfaces, and other explicitly approved product-identity anchors,
- symbol-only variants may be used for compact branded shell moments, but not as decoration across every screen,
- dense operational controls and workflow panels should keep SF Symbols unless a branded symbol is equally legible at operational sizes,
- approval, recovery, release, run-progress, and evidence panels must not introduce large decorative logo treatment,
- toolbars should prefer symbol-only branding if branding is needed at all; they must not become repeated horizontal-logo environments.

Orange accent rules:

- allowed for bounded brand details, logo accents, and sparse product-identity emphasis,
- forbidden as a large-surface fill,
- forbidden as a substitute for status semantics,
- forbidden as the default color for operational controls that already carry action or status meaning.

### 7.4 Iconography adoption

The branded icon system should be introduced only where it materially improves product identity and clarity.

Rules:

1. small operational icons must remain legible at 16–20 px,
2. branded icons may not degrade recognition compared with SF Symbols in dense controls,
3. the icon system must stay monochrome-first for small UI elements,
4. the workflow/run/stage/approval vocabulary should feel related without becoming ornamental.

---

## 8. Accessibility and trust constraints

Proposal 014 must preserve the accessibility and operator-trust rules already established by the UI quality baseline.

Required constraints:

1. status information must remain readable under Differentiate Without Color Alone,
2. badges, chips, and cards must preserve non-text contrast under Increase Contrast,
3. reduced-transparency mode must remain legible,
4. keyboard-only approval, dismissal, and recovery flows must remain intact,
5. existing `accessibilityIdentifier` values should remain stable on migrated surfaces unless a deliberate migration note says otherwise,
6. important operator commands stay above the fold or in toolbar/sticky-footer positions.

Proposal 014 is a visual-system rollout, not permission to hide or reframe critical actions.

---

## 9. Implementation plan

### Phase 1: Current-owner rebaseline and authority cleanup

- [ ] Confirm `DesignTokens`, `StatusCapsule`, and `StyledEmptyState` as the canonical bounded shared system
- [ ] Add only the missing token/panel/header/icon helpers around that existing system
- [ ] Create the bounded brand asset lane (`Design/` + asset-catalog integration path)
- [ ] Apply the corrected motion baseline where already known (`ForegroundBannerView` and other low-risk transitions)

Exit gate:

- token authority is singular and explicit,
- no parallel token or primitive namespace exists on the bounded slice,
- shell/banner drift is reduced without changing behavioral ownership,
- brand asset naming and integration rules are committed into the shared contract.

### Phase 2: Shell and run-surface completion

- [ ] Rebaseline `ContentView`, `RunsHomeView`, `IdeaListView`, and `RunDetailPanel` against current adoption status
- [ ] Finish summary-strip, chip, panel, and hierarchy alignment where the current token slice is still incomplete
- [ ] Apply bounded brand identity to the app shell without harming density or clarity

Exit gate:

- shell and run-centric hierarchy read as one visual system,
- summary strips, chips, and panels come from shared owners rather than local recipes,
- above-the-fold commands remain visible and trustworthy,
- bounded brand application is present without repeated-logo drift.

### Phase 3: Run-centric, delivery, and setup completion

- [ ] Finish design-kit alignment for `WorkflowMapView`, `ApprovalGateView`, `ReleaseGateView`, `DeliveryPreflightReportView`, `ProviderSettingsView`, `PilotReadinessView`, `FirstRunSetupWizard`, and `GooseProviderConnectionAssistantView`
- [ ] Unify remaining run-centric and setup-surface status/stage affordances under the current shared authority
- [ ] Verify approval/release checkpoints still read as high-trust operational surfaces

Exit gate:

- run-centric, delivery, and setup surfaces share one status/action/panel vocabulary,
- remediation and setup screens look like part of the same product system rather than a separate tool lane,
- proof-owner gates remain green for affected surfaces,
- no setup or release surface loses command clarity to decorative treatment.

### Phase 4: Secondary surfaces and final pass

- [ ] Finish archive/new-idea/recovery/empty-state migration
- [ ] Apply final icon/logo integration where approved
- [ ] Produce the final screenshot and preview evidence set through the existing proof owners

Exit gate:

- archive, new-idea, recovery, and empty-state surfaces no longer drift visually,
- recovery/report seams adopt only visual changes and do not absorb runtime ownership,
- preview and screenshot artifacts read as one coherent product system,
- no adopter surface remains on ad-hoc styling once the phase is declared done.

---

## 10. Verification criteria

Proposal 014 does not create a second proof lane.
It extends the current canonical proof owners:

- preview-backed owner surfaces from the UI-quality baseline,
- approved-host `proposal-012` for min-window, adopter-slice accessibility, and secondary runtime surfaces,
- approved-host `ui-smoke` for shell continuity,
- approved-host `proposal-006` where provider/setup surfaces are affected.

Each migrated phase must be verified through:

1. **Preview proof** for migrated owner surfaces already named by the UI-quality baseline.
2. **Min-window proof** through the existing `proposal-012` owner checks at `1024x768`.
3. **Cross-view consistency proof** that badges, spacing, panel styling, and typography come from the current shared owners instead of local ad-hoc styling.
4. **Brand application proof** that logo/icon/accent usage follows the design-kit restraint rules and is attached to the same owner surfaces under review.
5. **Accessibility proof** through the bounded current contract first:
   - `proposal-012` for adopter-slice Differentiate Without Color, Increase Contrast, Reduce Transparency, VoiceOver labels/traits, and focus order,
   - `ui-smoke` for shell continuity and no-regression interaction reachability.
6. **Provider/setup proof** through `proposal-006` whenever the migrated surfaces include settings, readiness, onboarding, or Goose remediation.
7. **Screenshot/review artifact** may be added, but only as a bounded supplement to the canonical proof owners above, not as a replacement for them.

For Proposal 014, the bounded adopter slice that must stay anchored to those proof owners is:

- `RunsHomeView`
- `WorkflowMapView`
- `ReleaseGateView`
- `DeliveryPreflightReportView`
- touched `IdeaListView` chips and supporting run-list affordances

And the accessibility settings that must be evidenced for that slice are:

- Differentiate Without Color,
- Increase Contrast,
- Reduce Transparency,
- VoiceOver labels/traits,
- focus-order continuity on the same owner surfaces.

---

## 11. Out of scope

- Rewriting product flows or navigation ownership outside already-approved baselines.
- A full light-mode design pass.
- Marketing site or external website design.
- Animated mascots, decorative splash scenes, or repeated logo motion.
- New persistence/runtime contracts unrelated to visual-system adoption.
- Repo-wide replacement of every SF Symbol regardless of clarity cost.

---

## 12. Risk assessment

| Risk | Mitigation |
|---|---|
| Design-kit rollout turns into broad UI churn | Phase-based surface migration using the audited-surface list from the UI quality baseline |
| Branding competes with workflow clarity | Brand accents are bounded; runtime hierarchy and status semantics remain primary |
| Custom iconography hurts small-size readability | Keep small operational icons monochrome-first and allow SF Symbols where branded icons are weaker |
| Token extraction causes behavior regressions | No business-logic changes inside token/primitives work; verification includes unchanged interaction proof |
| Dense setup and provider screens become prettier but less usable | Above-the-fold command rule and UI quality interaction constraints remain binding |

---

## 13. Acceptance criteria

Proposal 014 is complete only when all of the following are true:

1. the app has a real code-level token system derived from Design Kit v1,
2. that token system is an explicit evolution of `DesignTokens` / `StatusCapsule` / `StyledEmptyState`, not a parallel authority,
3. the primary operator surfaces use shared typography, color, spacing, and badge primitives instead of local ad-hoc styling,
4. the rollout plan and current adoption status match current-head reality,
5. canonical proof remains anchored to preview-backed owner surfaces plus `proposal-012`, `proposal-006`, and `ui-smoke`,
6. the shell and run-centric surfaces visibly reflect the approved Chainworks Forge brand language,
7. logo/app-icon/symbol application exists in bounded approved integration points,
8. no migrated surface regresses keyboard, accessibility, or operator-trust behaviors,
9. screenshot and preview evidence shows a coherent visual system across shell, run, setup, and recovery surfaces.

---

## Appendix A. Token mapping and drift rules

Proposal 014 extends the current authority instead of inventing a new design dictionary.
The mapping below is the migration contract.

| Category | Current owner | Mapping type | Notes |
|---|---|---|---|
| Surface/background neutrals | `DesignTokens` neutrals and panel backgrounds | kept plus bounded extension | add missing semantic neutrals in place; do not create per-surface gray palettes |
| Status colors | `DesignTokens.Status` | kept as canonical | status meaning stays separate from brand accent and action color |
| Action colors | `DesignTokens.Action` | kept plus bounded extension | action affordances remain distinct from status semantics |
| Brand accent | `DesignTokens` extension | new in-place extension | sparse brand accent only; not a second semantic color lane |
| Typography scale | `DesignTokens.Typography` | kept plus bounded extension | hierarchy changes must route through the shared typography owner |
| Spacing | `DesignTokens.Spacing` | kept as-is unless proven incomplete | no per-view spacing ladders |
| Radius | `DesignTokens.CornerRadius` | kept as-is unless proven incomplete | no alternative card/chip radius namespace |
| Shadow/panel treatment | shared panel helpers layered on current authority | bounded extension | panel recipes may be added, but only as shared helpers |
| Motion | shared motion baselines attached to current surfaces | bounded extension | restrained motion only; no decorative secondary motion lane |

Forbidden local replacement patterns:

- view-local color constants that duplicate semantic tokens,
- local font hierarchies that bypass `DesignTokens.Typography`,
- new capsule or badge primitives that duplicate `StatusCapsule`,
- local panel/shadow/background recipes on adopter surfaces,
- use of brand accent as a replacement for status or action semantics.

## Appendix B. Canonical naming cleanup

The canonical shared empty-state primitive is `StyledEmptyState`.

- Proposal 014 should refer to the primitive as `StyledEmptyState`.
- If the repo still temporarily stores that type in `EmptyStateView.swift`, that file is treated as the owner of `StyledEmptyState` until the file rename lands.
- Proposal 014 must not introduce a second long-lived empty-state primitive name during migration.

## Appendix C. Brand-safe surface application

Bounded application rules:

- use the full logo only on documentation, README, app-icon/launch-adjacent surfaces, and other explicitly approved identity anchors,
- use symbol-only branding only where compact product identity adds value without reducing operational clarity,
- keep SF Symbols for dense workflow controls, status actions, and operator-critical controls unless a branded symbol is equally legible,
- keep orange accent sparse and bounded to brand moments rather than broad operational UI fill.

Recovery-surface reminder:

- on `RecoverySheet`, `BlockedRunRecoveryView`, failed-stage evidence surfaces, and repair panels, Proposal 014 owns visual treatment only; behavioral ownership remains with the recovery/runtime lane.

## 14. Final recommendation

Approve Proposal 014 as the bounded design-system rollout plan that follows the implemented UI quality baseline.

The UI quality baseline identifies what is visually and structurally wrong.
Proposal 014 defines how the app should actually look and how to migrate it there without turning visual work into uncontrolled product churn.
