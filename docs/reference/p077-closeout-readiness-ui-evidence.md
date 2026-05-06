# P077 Closeout Readiness UI Evidence

This document is the durable token, contrast, and accessibility mapping for the
P077 closeout-readiness read-only macOS surface.

Proposal source: `docs/proposals/077-bounded-implementation-closeout-readiness-gates.md`
Implementation: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`,
`Chainworks Forge/Views/RunsHomeView.swift`
Fixture coverage: `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`

## Token Mapping

| readiness_state | tone_token | icon | typography | surface | breakpoint_behavior | interaction |
| --- | --- | --- | --- | --- | --- | --- |
| `ready` | semantic success (`Color.green`) | `checkmark.seal` | `.headline`, `.callout`, `.caption` | `P031CalloutCard` success accent | detail card remains full-width in the run-detail stack; compact signal stays single-line with accessibility label | copy generation id, diagnostics sheet |
| `ready_with_risks` | semantic warning (`Color.orange`) | `exclamationmark.triangle` | `.headline`, `.callout`, `.caption` | `P031CalloutCard` warning accent | secondary risk rows wrap inside card; compact signal remains available | copy generation id, diagnostics sheet |
| `handoff_required` | semantic warning (`Color.orange`) | `exclamationmark.triangle` | `.headline`, `.callout`, `.caption` | `P031CalloutCard` warning accent | handoff owner row wraps before mode explainer | diagnostics sheet, handoff route label |
| `not_ready` | semantic blocking (`Color.red`) | `xmark.octagon` | `.headline`, `.callout`, `.caption` | `P031CalloutCard` blocking accent | blocker rows stack vertically; no horizontal-only blocker layout | diagnostics sheet, code-refine recovery row |
| `blocked` | semantic blocking (`Color.red`) | `xmark.octagon` | `.headline`, `.callout`, `.caption` | `P031CalloutCard` blocking accent | blocker and diagnostic text wrap in the constrained detail column | diagnostics sheet, recovery row |
| `invalid` | semantic blocking (`Color.red`) | `xmark.octagon` | `.headline`, `.callout`, `.caption` | `P031CalloutCard` blocking accent | invalid evidence text remains in the card body, not only the sheet | diagnostics sheet, recovery row |
| `unknown` / first generation pending | semantic warning (`Color.orange`) | `exclamationmark.triangle` | `.headline`, `.callout`, `.caption` | `P031CalloutCard` warning accent | awaiting text is exposed through compact and card accessibility labels | diagnostics sheet |
| not applicable | semantic neutral (`Color.secondary`) | `minus.circle` | `.headline`, `.callout`, `.caption` | `P031CalloutCard` neutral accent | no action rows are exposed beyond diagnostics/readback | diagnostics sheet |

The UI intentionally uses the existing P031 card, semantic `Color` tokens, SF
Symbols, and presentation-layer strings instead of adding a parallel P077 design
system. That keeps the P077 surface inside the current thin GraphQL read
boundary and preserves the read-only control rule.

## Contrast Evidence

The P077 card uses `accentColor.opacity(0.12)` as a background tint and
`accentColor.opacity(0.2)` as a border over the existing macOS control
background. Primary labels use system foreground color; explanatory text uses
system secondary foreground color. These are dynamic system colors, so contrast
is owned by AppKit/SwiftUI semantic color adaptation across light, dark, and
increased-contrast modes.

Spot-check result for Phase 0 advisory rollout:

| surface | text_or_signal | color_source | contrast_decision | evidence_status |
| --- | --- | --- | --- | --- |
| card title/status | primary foreground over semantic accent tint | SwiftUI primary foreground + accent tint | passes by semantic dynamic color pairing | passed |
| primary unblock | primary/callout foreground over semantic accent tint | SwiftUI foreground + semantic status accent | passes by semantic dynamic color pairing | passed |
| secondary blockers | caption foreground over semantic accent tint | SwiftUI foreground + semantic status accent | passes by semantic dynamic color pairing | passed |
| mode/diagnostic/recovery text | secondary foreground over semantic accent tint | SwiftUI secondary foreground + semantic status accent | acceptable for advisory/supporting text; primary status remains foreground | passed |
| diagnostics sheet rows | primary foreground over sheet background | SwiftUI primary foreground | passes by platform semantic color pairing | passed |

## Accessibility and Focus Evidence

Fixture-backed presentation requirements:

- compact signal label: `compactActivationAccessibilityLabel`
- full card label: `cardAccessibilityLabel`
- primary unblock label: `Primary unblock: ...`
- diagnostics entry label: `diagnosticsAccessibilityLabel`
- diagnostics sheet rows: decision, gate, audit, diagnostic reason, fingerprint,
  mode, generation
- focus return copy: `focusReturnLabel`
- recovery lifecycle row: `recoveryLifecycleText`
- backlink/readback row: `backlinkRouteLabel`

The surface remains read-only. It exposes copy, diagnostics/readback, recovery
guidance, and route labels, but it does not add local write/control buttons.
