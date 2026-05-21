# UI Quality and Visual Polish

Stable reference for the UI quality, readability, accessibility, and bounded design-system hardening slice.

## Purpose

Chainworks Forge is an operator-facing macOS control plane.
That means the UI must do more than render the right data.
It must make active work readable, keep high-value actions visible, communicate trust-bearing non-happy-path states clearly, and remain usable under bounded accessibility settings on the supported macOS shell.

This document defines the implemented contract for that slice.

## Scope

This reference covers:

- operator-surface readability and hierarchy fixes,
- bounded async feedback and non-happy-path state ownership,
- the first-adopter shared-primitives slice for status and semantic tokens,
- minimum-window and bounded accessibility expectations,
- the current audited owner surfaces that carry proof obligations.

It does not redefine runtime, provider transport, release semantics, or repo-backed delivery behavior.
Those remain owned by:

- [operator-experience.md](operator-experience.md),
- [provider-platform.md](provider-platform.md),
- [live-workflow-map.md](live-workflow-map.md),
- [design-system-and-brand-application.md](design-system-and-brand-application.md),
- [full-mvp-delivery.md](full-mvp-delivery.md),
- [mvp-sign-off.md](mvp-sign-off.md),
- [chainworks_forge_design_kit_v1.md](chainworks_forge_design_kit_v1.md).

## Core rules

The implemented UI-quality slice is built around six rules:

1. important operator surfaces must stay readable at supported window sizes,
2. high-value actions must remain above the fold or otherwise discoverable without deep scrolling,
3. non-happy-path states must be explained locally on the surface that initiated the work,
4. shared status styling must use one bounded semantic system instead of per-view badge drift,
5. status meaning must not rely on color alone,
6. the proving path for UI quality is preview plus approved-host runtime evidence, not code review alone.

## Implemented issue classes

The slice closed four categories of UI debt.

### Readability and density

Implemented readability fixes include:

- wider and more adaptive `RunsHomeView` list ownership,
- reduced row density for run sidebar content,
- clearer `IdeaListView` summary strip structure,
- improved release artifact semantics that distinguish expected absence from warning conditions,
- discoverable run-detail actions without requiring a full detail scroll.

### Hierarchy and async feedback

Implemented hierarchy/state work includes:

- structured provider settings rather than one undifferentiated settings wall,
- System Readiness summary treatment (formerly Pilot Readiness; now under Settings in the consolidated operator shell),
- step-oriented first-run setup presentation,
- visible journey/probing feedback in provider diagnostics and setup,
- surface-local loading, success, and retry copy for bounded async actions.

### Shared status semantics

Implemented shared-primitives work includes:

- a bounded adopter slice for `StatusCapsule` and semantic status tokens,
- aligned badge padding and font treatment across the adopter surfaces,
- semantic status colors separated from generic action colors,
- typography and spacing normalization on the surfaces touched by the slice.

### Accessibility and operator trust

The slice is not only a happy-path polish pass.
It also hardens:

- explicit keyboard-only confirm/dismiss ownership on high-value modal/operator flows,
- bounded adopter-slice proof under Differentiate Without Color, Increase Contrast, and Reduce Transparency,
- VoiceOver-readable labels/traits on owner-level status and summary surfaces,
- focus-order/focus-visibility expectations on the runtime-proof slice.

## State and feedback contract

The current surface-level contract is:

- validation errors stay inline and preserve local draft/input state,
- backend, degraded, offline, and auth-required states are surfaced distinctly when the owning view already has that upstream truth,
- retry stays local to the affected card/sheet/panel where possible,
- cancellation dismisses UI or ends the current action but does not imply engine-level rollback,
- destructive or irreversible actions keep explicit confirmation boundaries.

This contract is presentation-only.
It does not invent new persistence or engine semantics.

## First-adopter shared-primitives slice

The shared design-system rollout remains intentionally bounded.
The first-adopter surfaces are:

- Runs tab status, provenance, and archive badges,
- Definitions tab (Workflow segment) status badges,
- `ReleaseGateView` status and artifact-semantic badges,
- `DeliveryPreflightReportView` status badges,
- touched `IdeaListView` chips/summary indicators.

Guardrails:

1. no business-logic or navigation rewrites inside primitive extraction,
2. existing accessibility identifiers and keyboard bindings remain stable,
3. adoption outside the slice is a separate decision,
4. status/chip/card semantics must remain understandable without color alone,
5. expansion requires fresh preview, min-window, accessibility, and runtime proof.

## Current owner surfaces

These surfaces now define the stable proof-owning UI quality baseline:

| Surface | Primary proof type | Notes |
|---|---|---|
| `ContentView` shell | Preview + shell smoke | shell grouping, foreground attention, tab stability |
| `Runs tab` | Min-window + accessibility runtime proof | owner summary, list readability, grouped run lanes, and inline approval panel |
| `RunDetailPanel` | Interaction/runtime proof | above-the-fold contextual actions |
| `IdeaListView` | Preview + min-window proof | summary chips, ideas density, selection readability |
| `NewIdeaSheetView` | Preview | form structure and local validation treatment |
| `ProviderSettingsView` | Preview + provider gate | hierarchy and inline async feedback |
| `SettingsView` System Readiness segment | Preview + provider gate | verdict summary and grouped readiness state (successor to the former `PilotReadinessView`) |
| `FirstRunSetupWizard` | Preview + interaction/runtime proof | steps, validation, launch feedback |
| `ArchivedIdeasView` | Preview | empty-state/readability consistency |
| `DeliveryPreflightReportView` | Preview + adopter-slice accessibility proof | positive and negative status rendering |
| `ApprovalGateView` | Interaction/runtime proof | keyboard-only confirm/dismiss ownership |
| `ReleaseGateView` | Preview + runtime proof | artifact semantics and decision affordances |
| `RecoverySheet` | Interaction/runtime proof | recovery action discoverability and dismissal |
| `RunStartOverridesView` | Preview | bounded secondary-surface consistency |
| `WorkflowMapView` | Preview + runtime proof | stage-card affordance, status badges, topology readability |
| `ImplementationSelfAssessmentPanel` | Preview + interaction/runtime proof | 2x2 metric grid, status-mapped icons, detail disclosure groups, accessibility summary |

## Minimum-window contract

The current supported proof floor for this slice is `1024×768`.

At that size:

- `RunsHomeView` must keep owner summary and active lanes readable,
- `IdeaListView` must keep summary/readability and primary action ownership intact,
- action-heavy run-detail and approval/release surfaces must remain operable,
- no owner surface may silently depend on a larger window just to reveal the primary next action.

## Accessibility contract

The implemented bounded accessibility contract is intentionally narrow but real.

### Settings proof

On the adopter slice, the system must be exercised under:

- Differentiate Without Color,
- Increase Contrast,
- Reduce Transparency.

The required outcomes are:

- state remains distinguishable without color alone,
- status and artifact chips preserve text/icon/shape cues,
- focus remains visible,
- translucent or subdued surfaces do not become illegible.

### VoiceOver and focus proof

The proving path must also confirm:

- owner-level summary/status labels are spoken with useful meaning,
- traits/actions are exposed where interactive,
- traversal order across the bounded proof slice is deterministic enough for operator use,
- modal dismissal/confirmation paths stay keyboard reachable.

## Verification and proving path

This slice is only considered proven when all of the following exist on the same head under review:

1. Xcode Preview rendering for the preview-backed Appendix-A-equivalent surfaces,
2. green local macOS build,
3. green approved-host `proposal-006` gate for provider/setup-adjacent surfaces,
4. green approved-host `proposal-012` gate for min-window, adopter-slice accessibility, and secondary runtime surfaces,
5. green approved-host `ui-smoke` gate for shell-level operator continuity.

Canonical runtime commands are documented in:

- [test-gates.md](test-gates.md)
- [agent-ui-test-execution.md](agent-ui-test-execution.md)

## Adjacent references

Use:

- [operator-experience.md](operator-experience.md) for the broader operator shell contract,
- [design-system-and-brand-application.md](design-system-and-brand-application.md) for the implemented brand/token adoption layer,
- [provider-platform.md](provider-platform.md) for provider/settings/readiness semantics,
- [live-workflow-map.md](live-workflow-map.md) for topology truth in run detail,
- [chainworks_forge_design_kit_v1.md](chainworks_forge_design_kit_v1.md) for brand-system authority.
