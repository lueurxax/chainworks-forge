# P031 UX And Accessibility Sign-Off

Status: VALIDATED_WITH_ASSISTIVE_ACCESS_LIMITATION
Owner: P031 macOS thin UI owner
Blocking Phase: Phase 0d
Blocker Recorded: 2026-04-24
Last Updated: 2026-04-25T04:27:00Z

## Scope

- Syncing placement.
- Approval diagnostic callouts.
- First-run orientation.
- Report payload indicators.
- Density and compact row behavior.
- VoiceOver labels for diagnostic and disabled states.

## Local Evidence Status

Runtime screenshots were captured after restoring the operator database into the packaged daemon path:

- `docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-ready-2026-04-24.png`
- `docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-degraded-sanitized-2026-04-24.png`
- `docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-2026-04-24.png` (local audit context only; not release evidence)

Observed runtime state:

- Runs Home renders restored run rows from GraphQL.
- Run cards show `Live` freshness badges.
- Run detail renders server projection stage rows.
- External write-path guide remains visible and write controls remain marked unavailable/documented externally.
- A transient `Daemon unavailable` screenshot was captured immediately after app/daemon restart; after relaunch and daemon readiness, the app rendered run rows and live state.

Code-level accessibility evidence:

- `P031ApprovalInboxCard` now applies each server-derived approval diagnostic row `accessibilityLabel` to the rendered callout.
- Targeted test coverage asserts the approval diagnostic accessibility label for the thin workflow screen coordinator.
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests' -skip-testing:'Chainworks ForgeUITests'` passed on 2026-04-25 with 48 P031 thin-read tests.

Accessibility limitation:

- A local attempt to inspect the macOS accessibility tree via `System Events` failed because `osascript` lacks Assistive Access permission in this environment.
- No VoiceOver pass was completed.
- No human VoiceOver sign-off has been recorded.

Required owner action: a human VoiceOver spot check should still be completed in an environment with Assistive Access permission before Phase 3 release sign-off. Phase 0d code and visual evidence is attached.

## Result

Runtime visual evidence and code-level accessibility evidence are attached. Runtime VoiceOver tree inspection remains environment-blocked.
