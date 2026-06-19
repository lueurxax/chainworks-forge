# Proposal 103: Live Agent Timeline UX and Readability

| Field | Value |
|---|---|
| Date | 2026-06-09 |
| Status | Draft / Conditional lane — activate only if active-agent visibility remains poor under dogfood |
| Author | Roadmap triage 2026-06-09 |
| Depends on | P046 session observability (implemented), P036/P085 macOS operator navigation and thin-client read-model baseline, UI action boundary (P072) |
| Related | `docs/reference/macos-operator-navigation.md`, `docs/reference/run-surface-information-architecture-and-artifact-hierarchy.md`, `docs/reference/escalation-policies.md`, `docs/reference/ui-action-boundary.md` |
| Scope | Improve readability of the live agent timeline in the macOS operator UI strictly over existing control-plane readback: clearer per-stage/per-agent activity grouping, current-turn vs settled-output distinction, escalation/attention cues, and time/idle indication. |
| Non-goal | Read-only: no new mutations, no new MCP tools, no Swift-local orchestration state reconstruction, no new GraphQL write surface. |

---

Provenance note: this work was previously referred to as "P093" in
`docs/ROADMAP.md`; number 093 actually belongs to the agent work continuation
expansion soak proposal, so the timeline UX work takes number 103.

## 1. Problem

During a live run the operator's main question is "what is each agent doing
right now, and is it stuck?" The current run surface answers this poorly:
activity is interleaved across stages and agents, a live in-progress turn looks
similar to settled history, idle time is not visually distinct from active
streaming, and attention/escalation states require navigating away from the
timeline. The data already exists in GraphQL readback (P046 session
observability, escalation readback, run/stage projections); this is a
presentation problem, not a data problem.

## 2. Goals

- G-1: Group timeline entries per stage and per agent execution with collapse/
  expand, so parallel fan-out reads as parallel lanes rather than interleaved
  noise.
- G-2: Visually distinguish a live current turn (streaming, unsettled) from
  settled outputs and superseded attempts, consistent with execution-truth
  identity (attempt/generation).
- G-3: Surface idle/elapsed indication per active execution (time since last
  activity event) so a stalled agent is visible at a glance, aligned with
  watchdog semantics rather than reimplementing them.
- G-4: Inline escalation/attention cues on the timeline rows, reusing the
  implemented escalation adapter read surface (status capsule, attention
  aggregation) instead of new queries where possible.
- G-5: Keep accessibility parity: VoiceOver labels, Full Keyboard Access
  traversal, contrast and reduced-motion behavior consistent with the P058
  escalation read-surface conventions.

## 3. Non-Goals

- No new orchestration state in Swift; everything renders from GraphQL
  reads/subscriptions already exposed by the control plane.
- No mutations of any kind; approvals remain the only UI write path.
- No timeline persistence or local caching beyond existing view-model state.
- No new control-plane fields unless a strictly read-only projection gap is
  proven during design; any such gap becomes a scoped delta, not silent scope.

## 4. Design Sketch

- A timeline view-model adapter composes existing projections: run/stage state,
  agent execution lineage (attempt, generation, supersede), session
  observability activity events, and escalation readback.
- Lane layout: stage → agent execution rows; each row shows provider/model
  binding, current state, last-activity age, and settlement status.
- Live-turn rendering uses the existing subscription stream; settled rows
  render from projection queries, so re-subscribing after window restore stays
  cheap.

## 5. Rollout Gates and Observability Contract

- Gate: `./scripts/test-gate.sh proposal-103` — view-model adapter unit tests
  over fixture readback (parallel fan-out, supersede, idle, escalation
  co-occurrence), plus boundary scan proving no new mutation call sites.
- Remote UI proof: timeline readability cases added to the remote `ui-smoke`
  flow per repo policy (UI tests are remote-only).
- Metrics: none server-side (read-only UI); client render health is covered by
  existing UI gates.
- Hold conditions: any new GraphQL mutation usage, any Swift-local
  orchestration state, or accessibility regression in the escalation read
  surface contracts.
- Rollback disposition: view-layer change only; revert restores the current
  timeline with no data migration.

## 6. Acceptance

- Fixture-driven previews/tests show: two parallel agents render as separate
  lanes; a superseded attempt is visually demoted; an idle execution shows
  elapsed-time emphasis; an escalation-paused row carries the attention cue.
- Remote ui-smoke passes with the new timeline cases.
- Boundary scan confirms read-only behavior.

## 7. Open Questions

- Whether last-activity age should come from an existing session observability
  field or needs a small read-only projection addition (scoped delta if so).
- How much history to render before collapsing into "earlier activity" —
  fixed window vs. per-stage budget.
- Whether MenuBarExtra should mirror a compact "agents active/idle" summary in
  this proposal or stay out of scope.
