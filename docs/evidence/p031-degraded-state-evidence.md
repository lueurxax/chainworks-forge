# P031 Degraded-State Evidence

Status: PARTIAL_RUNTIME_EVIDENCE
Owner: P031 release owner
Blocking Phase: Phase 0d
Blocker Recorded: 2026-04-24
Last Updated: 2026-04-24T20:26:04Z

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
  - `docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-2026-04-24.png`
  - `docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-ready-2026-04-24.png`
- Live GraphQL evidence:
  - `docs/evidence/p031-runtime/live-graphql-probe-2026-04-24.json`

Observed against criteria:

- The UI visibly entered disabled/unavailable state during daemon unavailability.
- No local orchestrator, MCP UI call, GraphQL mutation, or local UI write control became visible.
- After recovery, restored run rows were read through GraphQL and displayed as live server projections.

Limitations:

- This was an incidental restart/degraded sequence, not a scripted fault-injection drill.
- No signed release-owner waiver is present.
- No operator dogfood confirmation has been recorded.

Required owner action: P031 release owner must either accept this partial degraded-state evidence with a dated waiver or run a scripted degraded-state drill during Phase 3 dogfood.

## Results

Partial degraded-state runtime evidence is attached. Release-owner acceptance/waiver or scripted dogfood drill remains required before Phase 3 closeout.
