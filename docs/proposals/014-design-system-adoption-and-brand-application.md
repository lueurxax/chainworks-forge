# Proposal 014: Design System Adoption and Brand Application

| Field | Value |
|---|---|
| Date | 2026-03-28 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/chainworks_forge_design_kit_v1.md](../reference/chainworks_forge_design_kit_v1.md), [012-ui-quality-audit-and-visual-polish.md](012-ui-quality-audit-and-visual-polish.md), [reference/operator-experience.md](../reference/operator-experience.md), [reference/live-workflow-map.md](../reference/live-workflow-map.md), [reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md) |
| Scope | Brand-token adoption, shared UI primitives, icon/logo application, and surface-by-surface migration of the current macOS app toward the approved Chainworks Forge design system |
| Goal | Bring the shipped app into visual, typographic, and interaction alignment with the Design Kit v1 so Chainworks Forge reads as a coherent orchestration tool with a distinctive brand rather than a collection of functional SwiftUI screens. |

---

## 1. Context

The app now has enough runtime maturity that visual inconsistency is no longer a side issue.
There is already a UI-quality proposal in [012-ui-quality-audit-and-visual-polish.md](012-ui-quality-audit-and-visual-polish.md), and a stable full-delivery runtime baseline in [reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md).
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

### 1.1 Relationship to Proposal 012

Proposal 012 remains the audit and readability baseline.
It already defines:

- the audited surface inventory,
- the current UI debt backlog,
- non-happy-path state coverage,
- bounded shared-primitives guardrails.

Proposal 014 does not replace that work.
It builds on it and answers a different question:

> How do we apply the approved Chainworks Forge visual system across the app without regressing clarity, accessibility, or operator trust?

Rule of thumb:

- Proposal 012 owns **readability, information hierarchy, and UI debt closure**.
- Proposal 014 owns **brand-system adoption, design tokens, icon/logo application, and surface migration into the design kit**.

When both proposals touch the same surface, Proposal 012 remains authoritative for interaction and state truth, while Proposal 014 remains authoritative for visual language.

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

### Layer Q: Brand Tokens and Assets

| Component | Responsibility |
|---|---|
| **ForgeColor** | Canonical color tokens derived from Design Kit v1 |
| **ForgeTypography** | Canonical type scale for screen title, section, label, body, and meta text |
| **ForgeSpacing / ForgeRadius** | Shared spacing and corner-radius primitives |
| **ForgeStatusColor** | Semantic status palette separate from generic action styling |
| **Brand Asset Set** | Logo variants, app icon assets, monochrome symbol assets, and usage rules |

### Layer R: Shared UI Primitives

| Component | Responsibility |
|---|---|
| **StatusCapsule** | Unified badge/chip primitive aligned to the design kit |
| **ForgePanel / ForgeCardStyle** | Shared surface styling for cards, panels, grouped content, and secondary containers |
| **ForgeSectionHeader** | Reusable section-title treatment with typographic consistency |
| **ForgeEmptyState** | Branded empty-state wrapper that avoids generic placeholder screens |
| **ForgeIconBridge** | Controlled mapping from product states/actions to the branded symbol system |

### Layer S: Surface Migration

| Component | Responsibility |
|---|---|
| **Shell Migration Pack** | App shell, runs list, ideas list, summary strips, and brand entry points |
| **Run-Centric Migration Pack** | Run detail, workflow map, approval gate, release gate, and delivery-preflight surfaces |
| **Setup Migration Pack** | Provider settings, pilot readiness, first-run setup, and Goose remediation assistant |
| **Secondary Surface Pack** | Archive, new-idea, recovery, banner, and supporting surfaces |

### Layer T: Verification and Rollout Evidence

| Component | Responsibility |
|---|---|
| **Preview Matrix** | Per-surface visual proof for the audited operator surfaces |
| **Brand Application Checklist** | Logo/icon/token/motion/accessibility verification gate |
| **Min-Window and Keyboard Proof** | Keep 1024×768 usability and keyboard behaviors intact |
| **Design Adoption Review Pack** | Screenshot set and checklists for final proposal-readiness review |

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

Proposal 014 adopts the design kit's suggested direction, with repo-local naming aligned to existing SwiftUI support code.

Initial target structure:

```text
Support/
  Design/
    ForgeColor.swift
    ForgeTypography.swift
    ForgeSpacing.swift
    ForgeRadius.swift
    ForgeTheme.swift
    StatusCapsule.swift
    ForgePanel.swift
    ForgeSectionHeader.swift
    ForgeEmptyState.swift
    ForgeIconBridge.swift
Assets.xcassets/
  Brand/
  AppIcon.appiconset/
Design/
  Brand/
  Icons/
  AppIcon/
```

Rules:

1. token files are the only source of truth for adopted surfaces,
2. asset-catalog integration may consume generated/finalized source assets from `Design/`,
3. no second ad-hoc token namespace is introduced in view files,
4. surfaces not yet migrated may temporarily keep old styling, but must not fork the new tokens.

---

## 6. Surface rollout plan

Proposal 014 uses the audited-surface inventory from Proposal 012 rather than inventing a new scope list.

### 6.1 Phase 1: Foundation and shell

First adopters:

- `ContentView`
- `RunsHomeView`
- `IdeaListView`
- `ForegroundBannerView`

Goals:

- establish app-level color and typography rhythm,
- introduce the new badge/panel tokens on the most visible shell surfaces,
- apply the design-kit hierarchy so the product immediately reads as `Run -> Stage -> Agent -> Artifact`,
- introduce bounded brand identity in the shell without turning it into a splash screen.

### 6.2 Phase 2: Run-centric and delivery surfaces

Second adopters:

- `RunDetailPanel`
- `WorkflowMapView`
- `ApprovalGateView`
- `ReleaseGateView`
- `DeliveryPreflightReportView`

Goals:

- align status chips, panels, and section hierarchy,
- make approval and release surfaces feel like trust-bearing product checkpoints,
- keep run-critical information legible while adopting the new token system,
- ensure workflow topology and delivery surfaces share one visual vocabulary.

### 6.3 Phase 3: Setup and provider surfaces

Third adopters:

- `ProviderSettingsView`
- `PilotReadinessView`
- `FirstRunSetupWizard`
- `GooseProviderConnectionAssistantView`

Goals:

- bring heavy operator/setup surfaces into the same system,
- keep important commands above the fold or in toolbars/sticky footers as required by the design kit,
- avoid turning dense setup forms into decorative layouts,
- keep troubleshooting and readiness semantics explicit.

### 6.4 Phase 4: Secondary and supporting surfaces

Final adopters:

- `ArchivedIdeasView`
- `NewIdeaSheetView`
- `RecoverySheet`
- supporting empty states and secondary panels touched by the earlier phases

Goals:

- finish consistency work,
- unify empty-state and helper surfaces,
- close remaining visual drift without reopening flow ownership.

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

### 7.3 Iconography adoption

The branded icon system should be introduced only where it materially improves product identity and clarity.

Rules:

1. small operational icons must remain legible at 16–20 px,
2. branded icons may not degrade recognition compared with SF Symbols in dense controls,
3. the icon system must stay monochrome-first for small UI elements,
4. the workflow/run/stage/approval vocabulary should feel related without becoming ornamental.

---

## 8. Accessibility and trust constraints

Proposal 014 must preserve the accessibility and operator-trust rules already established by Proposal 012.

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

### Phase 1: Token and asset foundation

- [ ] Introduce `ForgeColor`, `ForgeTypography`, `ForgeSpacing`, `ForgeRadius`, and `ForgeStatusColor`
- [ ] Introduce `StatusCapsule` and one shared panel/header primitive
- [ ] Create the bounded brand asset lane (`Design/` + asset-catalog integration path)
- [ ] Apply the corrected motion baseline where already known (`ForegroundBannerView` and other low-risk transitions)

### Phase 2: Shell adoption

- [ ] Migrate `ContentView`, `RunsHomeView`, and `IdeaListView` to the new tokens/primitives
- [ ] Rework summary strips, chips, and panels into the design-kit hierarchy
- [ ] Apply bounded brand identity to the app shell without harming density or clarity

### Phase 3: Run and delivery adoption

- [ ] Migrate `RunDetailPanel`, `WorkflowMapView`, `ApprovalGateView`, `ReleaseGateView`, and `DeliveryPreflightReportView`
- [ ] Unify run-centric status and stage affordances
- [ ] Verify approval/release checkpoints still read as high-trust operational surfaces

### Phase 4: Setup and remediation adoption

- [ ] Migrate setup/readiness/remediation surfaces
- [ ] Apply the design-kit hierarchy to form-heavy and diagnostics-heavy screens
- [ ] Keep advanced controls subordinate and above-the-fold actions obvious

### Phase 5: Secondary surfaces and final pass

- [ ] Finish archive/new-idea/recovery/empty-state migration
- [ ] Apply final icon/logo integration where approved
- [ ] Produce final screenshot and preview evidence pack

---

## 10. Verification criteria

Each migrated phase must be verified through:

1. **Preview proof** for the migrated surfaces listed in Proposal 012 Appendix A.
2. **Min-window proof** at `1024x768` for every migrated surface that declares min-window ownership.
3. **Cross-view consistency proof** that badges, spacing, panel styling, and typography now come from shared primitives rather than local ad-hoc styling.
4. **Brand application proof** that the logo/icon/accent usage follows the design-kit restraint rules.
5. **Accessibility proof** for Differentiate Without Color Alone, Increase Contrast, Reduce Transparency, VoiceOver labels/traits, and keyboard-only modal flows.
6. **No-regression interaction proof** for approval, release, setup, recovery, and above-the-fold action discoverability.
7. **Screenshot review pack** covering shell, run-progress, workflow-map, approval, release, provider/setup, and recovery surfaces.

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
| Design-kit rollout turns into broad UI churn | Phase-based surface migration using the audited-surface list from Proposal 012 |
| Branding competes with workflow clarity | Brand accents are bounded; runtime hierarchy and status semantics remain primary |
| Custom iconography hurts small-size readability | Keep small operational icons monochrome-first and allow SF Symbols where branded icons are weaker |
| Token extraction causes behavior regressions | No business-logic changes inside token/primitives work; verification includes unchanged interaction proof |
| Dense setup and provider screens become prettier but less usable | Above-the-fold command rule and Proposal 012 interaction constraints remain binding |

---

## 13. Acceptance criteria

Proposal 014 is complete only when all of the following are true:

1. the app has a real code-level token system derived from Design Kit v1,
2. the primary operator surfaces use shared typography, color, spacing, and badge primitives instead of local ad-hoc styling,
3. the shell and run-centric surfaces visibly reflect the approved Chainworks Forge brand language,
4. logo/app-icon/symbol application exists in bounded approved integration points,
5. no migrated surface regresses keyboard, accessibility, or operator-trust behaviors,
6. screenshot and preview evidence shows a coherent visual system across shell, run, setup, and recovery surfaces.

---

## 14. Final recommendation

Approve Proposal 014 as the bounded design-system rollout plan that follows Proposal 012.

Proposal 012 identifies what is visually and structurally wrong.
Proposal 014 defines how the app should actually look and how to migrate it there without turning visual work into uncontrolled product churn.
