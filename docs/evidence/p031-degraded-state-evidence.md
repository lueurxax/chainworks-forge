# P031 Degraded-State Evidence

Status: READY_SCRIPTED_DRILL_VERIFIED
Owner: P031 release owner
Blocking Phase: Phase 0d
Blocker Recorded: 2026-04-24
Last Updated: 2026-05-05T21:02:39+03:00

## Evidence Criteria
- Affected thin UI surfaces visibly enter disabled/degraded state within 60 seconds from the triggering degraded condition.
- No projection data loss is caused by entering degraded state.
- No stale GraphQL-only truth remains visible as authoritative after entering degraded state.
- No local orchestrator, local workflow truth, MCP UI call, GraphQL mutation, or local UI write becomes reachable.
- Participating operator confirms Runs Home/Run Detail remain usable as read-only/degraded control-plane views or clearly unavailable with actionable external guidance.

## Local Evidence Status

Runtime evidence captured one restart/degraded sequence:

- Packaged daemon initially used an empty DB path, then was stopped while the repo-local operator DB was copied into `~/Library/Application Support/Chainworks Forge/control-plane.db`.
- During app/daemon restart, the UI rendered `Daemon unavailable` / `Runs unavailable` rather than exposing local workflow truth or write fallbacks.
- After daemon readiness, the UI recovered to GraphQL-backed live run rows with `Live` freshness badges.
- Runtime screenshots:
  - `docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-degraded-sanitized-2026-04-24.png` (release-suitable cropped Forge-window evidence)
  - `docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-ready-2026-04-24.png`
  - `docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-2026-04-24.png` (local audit context only; includes unrelated desktop background and must not be used as release evidence)
- Live GraphQL evidence:
  - `docs/evidence/p031-runtime/live-graphql-probe-2026-04-24.json`

Observed against criteria:

- The UI visibly entered disabled/unavailable state during daemon unavailability.
- No local orchestrator, MCP UI call, GraphQL mutation, or local UI write control became visible.
- After recovery, restored run rows were read through GraphQL and displayed as live server projections.

## Scripted Remote UI Drill

Scripted degraded-state drill passed on 2026-05-05 against the remote macOS UI runner:

- Remote host: `SMacBook.local`
- Remote workspace: `/Users/test/chainworks-p031-drill`
- Test: `Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal031DegradedStateDrillShowsReadOnlyUnavailableAndRecovers`
- Result bundle: `/tmp/p031-degraded-drill-signed.xcresult`
- Evidence receipt: `docs/evidence/p031-runtime/p031-degraded-state-remote-ui-drill-2026-05-05.json`
- Seed run: `bb1620b5-bb27-46dc-96a5-9c817fcde47f`, `P031 Degraded-State Drill`
- Result: 1 test executed, 0 failures, test duration 21.999 seconds.

The drill verified:

- Runs Home opens against the packaged daemon with local in-memory fixture data disabled.
- The seeded active run renders from GraphQL-backed projections with live freshness.
- The packaged daemon is stopped and `/health` becomes unavailable.
- After operator refresh, the UI reaches `Daemon unavailable` within 60 seconds.
- No local run-detail fallback remains visible during the outage.
- No `Start Run`, `Cancel Run`, `Retry Stage`, `Approve`, or `Reject` write controls are enabled during the outage.
- `command_journal` remains unchanged at 0 rows.
- After app relaunch, the packaged daemon returns to `/health` ready and the seeded run returns as a live GraphQL-backed projection.

Critical write-path readiness:

- The release owner confirmed on 2026-05-05 that the P031 thin macOS UI is accepted as a GraphQL read surface for release.
- Local workflow/control writes must remain unavailable from the P031 UI.
- Command/control operations remain outside the thin UI, with approval-only GraphQL mutations allowed only for approval decisions.
- This sign-off confirms the write-path readiness item. It does not waive the separate degraded-state scripted drill or degraded-state release waiver requirement.

Limitations:

- The release-suitable degraded screenshot is sanitized to the Forge window; the original full-desktop capture is retained only as local audit context.
- Full dogfood workflow-completion signoff remains tracked separately in `docs/evidence/p031-dogfood-signoff.md`.

## Results

Restart/degraded-state runtime evidence and a scripted remote UI drill are attached. The degraded-state evidence item is ready for release closeout.
