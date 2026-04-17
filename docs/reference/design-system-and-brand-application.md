# Design System and Brand Application

Stable reference for the design-system adoption and brand-application slice.

## Purpose

Chainworks Forge is not only a runtime engine.
It is an operator-facing macOS product, and its shell, run surfaces, setup views, and recovery flows now share one bounded visual system instead of ad-hoc per-view styling.

This document is the stable contract for:

- the Forge token and primitive lane,
- bounded brand-asset integration,
- surface adoption across shell, run, setup, and recovery views,
- and the rule that brand language must stay subordinate to runtime/status truth.

## Scope

This reference covers:

- shared design tokens and visual primitives,
- compatibility facades that preserve a single authority,
- bounded logo/app-icon usage,
- the current adopter surfaces in the macOS app,
- and the approved proof path for this slice.

It does not redefine:

- operator-state semantics in [operator-experience.md](operator-experience.md),
- the broader readability/accessibility baseline in [ui-quality-and-polish.md](ui-quality-and-polish.md),
- or the upstream visual authority in [chainworks_forge_design_kit_v1.md](chainworks_forge_design_kit_v1.md).

## Core Rules

### One visual authority

The visual system is anchored on the Design Kit plus one concrete implementation lane in the app:

- `ForgeColor`
- `ForgeTypography`
- `ForgeSpacing`
- `ForgeRadius`
- `ForgeStatusColor`
- `ForgePanel`
- `ForgeSectionHeader`
- `ForgeEmptyState`
- `ForgeIconBridge`
- `StatusCapsule`

View-level styling should flow through this lane rather than growing new token namespaces inside feature views.

### Compatibility facades do not create a second system

Some existing surfaces still route through compatibility wrappers such as:

- `DesignTokens`
- `StyledEmptyState`

That is allowed only because those wrappers now point back to the Forge lane.
They are compatibility facades, not an independent design authority.

### Runtime/status truth outranks decoration

Brand accents are intentionally bounded.

Rules:

1. status meaning must remain readable without color alone,
2. badge/status semantics outrank decorative accent use,
3. shell branding must not compete with approval, recovery, warning, or failure truth,
4. keyboard behavior, accessibility, and operator trust remain binding during visual migration.

## Brand Assets and Integration Points

The current bounded brand lane includes:

- `Assets.xcassets/Brand/`
- `Assets.xcassets/AppIcon.appiconset/`
- `ForgeIconBridge` brand asset accessors

Approved integration points include:

- shell/header branding in `ContentView`,
- bounded foreground attention treatment in `ForegroundBannerView`,
- app icon assets in the asset catalog,
- symbol selection that still uses SF Symbols where semantic clarity is stronger than ornamental replacement.

The app is intentionally not trying to replace every product symbol with custom artwork.

## Surface Adoption Map

### Shell and run surfaces

The implemented design-system slice now owns visual consistency across:

- `ContentView`
- `RunsHomeView`
- `IdeaListView`
- `WorkflowMapView`
- `ApprovalGateView`
- `ReleaseGateView`
- `RunProgressView`
- `DeliveryPreflightReportView`

### Setup and readiness surfaces

The same system is carried into:

- `ProviderSettingsView`
- `PilotReadinessView`
- `FirstRunSetupWizard`
- `ProviderSetupEvidencePanel`
- `ProviderTroubleshootingPanel`

### Recovery and supporting surfaces

The visual lane also covers:

- `RecoverySheet`
- `BlockedRunRecoveryView`
- foreground warning/attention surfaces
- shared empty states and section headers

## Relationship to the UI Quality Baseline

The design-system slice does not replace the UI quality baseline.

The split is:

- [ui-quality-and-polish.md](ui-quality-and-polish.md) owns readability, bounded accessibility, and owner-surface proof expectations,
- this document owns brand-system adoption, token authority, and bounded shell/run/setup/recovery visual rollout,
- [chainworks_forge_design_kit_v1.md](chainworks_forge_design_kit_v1.md) remains the upstream visual authority.

## Verification and Proof Path

This slice is proved by a mix of local and approved-host evidence:

1. green local macOS build on the current tree,
2. preview-backed owner renders for the migrated surfaces,
3. green approved-host `proposal-014` gate.

The gate name keeps its original label for reproducibility.

## Adjacent References

Use:

- [chainworks_forge_design_kit_v1.md](chainworks_forge_design_kit_v1.md) for the upstream visual authority,
- [ui-quality-and-polish.md](ui-quality-and-polish.md) for readability/accessibility proof rules,
- [operator-experience.md](operator-experience.md) for functional surface semantics,
- [test-gates.md](test-gates.md) and [agent-ui-test-execution.md](agent-ui-test-execution.md) for canonical proof execution.
