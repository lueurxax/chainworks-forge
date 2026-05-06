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

The P077 cardElevated surface uses `accentColor.opacity(0.12)` as a
background tint and `accentColor.opacity(0.2)` as a border over the macOS
control background. The compactCapsule surface uses `accentColor.opacity(0.16)`
with a `0.35` border. Primary labels use system foreground color; supporting
text uses the same semantic foreground stack and keeps icon/text labels so color
is not the only state carrier.

Phase 0 contrast measurement was taken with AppKit semantic colors resolved to
sRGB for `.aqua`, `.darkAqua`, `.accessibilityHighContrastAqua`, and
`.accessibilityHighContrastDarkAqua`, compositing the actual tint alpha over
`NSColor.controlBackgroundColor` and applying WCAG relative luminance. Reduce
Transparency does not change these non-material tint surfaces; Differentiate
Without Color is satisfied by the explicit status text and SF Symbol icon in
the same measured surfaces.

| surface | mode | candidate | text_or_signal | measured_contrast_ratio | contrast_decision | evidence_status |
| --- | --- | --- | --- | --- | --- | --- |
| cardElevated | standard light | readyWithRisks / amber fallback | primary and supporting text over warning tint | `19.01:1` | passes `4.5:1` text threshold | passed |
| cardElevated | standard dark | readyWithRisks / amber fallback | primary and supporting text over warning tint | `13.51:1` | passes `4.5:1` text threshold | passed |
| cardElevated | High Contrast light | readyWithRisks / amber fallback | primary and supporting text over warning tint | `19.01:1` | passes `4.5:1` text threshold | passed |
| cardElevated | High Contrast dark | readyWithRisks / amber fallback | primary and supporting text over warning tint | `13.51:1` | passes `4.5:1` text threshold | passed |
| compactCapsule | standard light | readyWithRisks / amber fallback | compact signal text and icon over warning capsule | `18.38:1` | passes `4.5:1` text threshold | passed |
| compactCapsule | standard dark | readyWithRisks / amber fallback | compact signal text and icon over warning capsule | `12.46:1` | passes `4.5:1` text threshold | passed |
| compactCapsule | High Contrast light | readyWithRisks / amber fallback | compact signal text and icon over warning capsule | `18.38:1` | passes `4.5:1` text threshold | passed |
| compactCapsule | High Contrast dark | readyWithRisks / amber fallback | compact signal text and icon over warning capsule | `12.46:1` | passes `4.5:1` text threshold | passed |
| cardElevated + compactCapsule | Reduce Transparency | readyWithRisks / amber fallback | non-material tint surfaces | minimum measured ratio `12.46:1` | no material/transparency dependency; measured non-material pair passes | passed |
| cardElevated + compactCapsule | Differentiate Without Color | readyWithRisks / amber fallback | text + SF Symbol + measured warning tint | minimum measured ratio `12.46:1` | color is not sole signal and measured contrast passes | passed |

## Accessibility and Focus Evidence

Fixture-backed presentation requirements:

- compact signal label: `compactActivationAccessibilityLabel`
- full card label: `cardAccessibilityLabel`
- primary unblock label: `Primary unblock: ...`
- diagnostics entry label: `diagnosticsAccessibilityLabel`
- diagnostics sheet rows: decision, gate, audit, diagnostic reason, fingerprint,
  mode, generation
- focus return copy: `focusReturnLabel`
- copy failure fallback: `copyFailureFallbackText`
- VoiceOver announcement policy: `voiceOverAnnouncementPolicy`
- keyboard traversal proof order: `keyboardTraversalOrder`
- recovery lifecycle row: `recoveryLifecycleText`
- backlink/readback row: `backlinkRouteLabel` and
  `backlinkRouteAccessibilityLabel`

The surface remains read-only. It exposes copy, diagnostics/readback, recovery
guidance, and route labels, but it does not add local write/control buttons.
