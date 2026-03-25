# Proposal 010: Operator Clarity - Idea Archive, Provider Troubleshooting, and Live Workflow Visualization Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/010-operator-clarity-idea-archive-provider-troubleshooting-and-live-workflow-visualization.md` |
| Repository Root | `.` |
| Git SHA | `e1655a6` |
| Reviewed At | `2026-03-25T21:46:29+0200` |
| Review Mode | `full-review` |
| Product Overlay | `omitted` |
| Overall Status | `Evidence Gap Review` |
| Readiness | `Yellow` |
| Confidence | `High` |
| Evidence Completeness | `Partial` |

## 0. Review Mode and Evidence Summary

- Mode used: `full-review`
- Evidence completeness: `Partial`
- Documents / repo inputs reviewed:
  - `docs/proposals/010-operator-clarity-idea-archive-provider-troubleshooting-and-live-workflow-visualization.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/provider-platform.md`
  - `docs/reference/runtime-contract.md`
- Build/run attempts:
  - authoritative runtime proof remains `RUN-01`: focused macOS UI rerun with valid xcresult, `6` tests total, `4` passed, `2` failed
  - current round added a fresh Xcode MCP preview render pass across the current shell, provider owner path, runs shell, idea list, start-run sheet, and override sheet
  - runtime evidence was reused only after a freshness check confirmed the relevant app baseline had not changed since `RUN-01`
- Screenshots captured:
  - `P006_Settings`
  - `P006_Wizard_Surface`
  - `P006_PilotReadiness_Refresh`
  - `REQ011_ApprovalGate`
  - `content-shell-seeded`
  - `runs-home-mixed-states`
  - `provider-settings-configured`
  - `pilot-readiness-seeded`
  - `first-run-setup-seeded`
  - `ideas-operator-list`
  - `start-new-run-live`
  - `override-list-8-agents`
- Code areas inspected:
  - idea lifecycle model and Ideas flow
  - app shell / tab ownership / preview coverage
  - provider settings, wizard, and pilot-readiness journey
  - current run-progress UI and runtime models
  - current SwiftUI preview surfaces rendered through Xcode MCP for shell and owner-path inspection
- Remaining assumptions:
  - passed XCUITests contain their expected screenshot attachments inside the reused authoritative xcresult
- Remaining blockers:
  - current HEAD still has no archive slice and no workflow-map slice
  - current `Ideas` -> `Start Run` / run-progress owner path is still unstable in focused UI proof

## 1. Executive Summary

- Overall readiness: `Yellow`
- Confidence: `High`
- Remaining blockers to full sign-off:
  1. archive and workflow-map slices are still absent on current HEAD, so the full triad evidence gate cannot close yet
  2. the current `Ideas` -> `Start Run` shell path is still UI-fragile in focused macOS proof
  3. runtime evidence remains partial because the new Proposal 010 surfaces do not exist yet
- Top risks:
  1. archive/map implementation can still drift from the proposal because there is no live feature evidence yet
  2. the unstable Start Run path can make future workflow-map proof noisier than it should be
  3. provider improvements may look complete while archive and visualization slices remain unimplemented
- Top opportunities:
  1. the draft now cleanly separates draft-readiness from post-implementation sign-off
  2. provider troubleshooting is now anchored to the existing settings/wizard/readiness owner path instead of inventing a parallel stack
  3. the archive and workflow-map slices are now framed as real post-implementation proof obligations rather than impossible pre-implementation gates
  4. fresh preview evidence now confirms that the current shell clutter and owner-path hierarchy issues the proposal wants to fix are real on current HEAD

Verdict: the live proposal-text blockers from the previous pass are closed. Proposal 010 now reads like a workable handoff document, and the fresh Xcode MCP preview pass strengthens confidence that its shell/provider clarity critique matches current UI reality. This round still stays `Evidence Gap Review` because the app does not yet implement the archive or workflow-map slices, and the current Start Run owner path remains UI-fragile, so the full runtime side of the triad is still unavailable.

## 2. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Yellow | High | Partial | 0 | 0 | 0 | 0 |
| UX | Yellow | High | Partial | 0 | 0 | 0 | 0 |
| iOS Architecture | Yellow | High | Partial | 0 | 0 | 0 | 0 |

## 3. Findings by Discipline

No live proposal-text findings surfaced in this reread.

What changed since the previous pass:

- the simulator-only wording is gone
- the old impossible pre-implementation gate is now split into:
  - section `1.5` draft-readiness gate
  - section `1.6` post-implementation sign-off gate
- the general acceptance section now mirrors that same split
- fresh Xcode MCP previews now confirm the current-state critique the proposal is built on:
  - `ContentView` / `RunsHomeView` still flatten too many peer destinations and leave a large dead detail canvas when nothing is selected
  - `ProviderSettingsView`, `FirstRunSetupWizard`, and `PilotReadinessView` still read as dense admin/configuration surfaces rather than guided operational tasks
  - `Start New Run` still leaves too much inert space and does not make compile/preflight feel like evidence-bearing checkpoints
  - `RunStartOverridesView` is still overwhelmingly repetitive and validates keeping overrides off the default launch path

## 4. Cross-Discipline Conflicts and Decisions

- Conflict: the proposal is now structurally cleaner than the current app baseline, but the new archive/map surfaces still do not exist.
  Tradeoff: honest partial sign-off versus pretending the full triad is closed.
  Decision: keep the draft at `Yellow` with no live text findings, but retain `Evidence Gap Review` status until the new surfaces exist and are evidenced.
  Owner: reviewer / proposal author

## 5. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Keep Proposal 010 text as-is for gating, and implement the archive slice under the existing `Ideas` owner path | UX + Architecture | Implementation owner | During implementation | None | Archive flow exists and can produce fresh macOS UI proof | Evidence gap only |
| P1 | Implement workflow-map / agent-activity / loop-visualization surfaces inside the existing run-detail owner path | UI + Architecture | Implementation owner | During implementation | P0 not required | Workflow-map primary and fallback proofs become reachable | Evidence gap only |
| P2 | Stabilize the current `Ideas` -> `Start Run` path before relying on it for final Proposal 010 sign-off screenshots | UI | Implementation owner | Before final sign-off | None | Focused UI proof no longer fails on seeded idea reachability | Evidence gap only |

## 6. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Draft quality | Whether Proposal 010 remains internally consistent as implementation starts | no reopening of gate/ownership contradictions | keep draft-readiness and post-implementation sign-off separated | next proposal rereview | hold if the document collapses those gates again |
| Archive slice | Reachability and correctness of archive lifecycle | archive action only appears under valid lifecycle states | no new top-level destination | first archive implementation review | hold if archive hides active/approval-blocked work |
| Workflow map | Topology visibility and fallback states | primary-state and fallback-state proofs become reachable | no parallel shell tab | first workflow-map implementation review | hold if map truth is not derivable from runtime state |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: current HEAD still has no archive model/service/view slice, so archive-flow proof cannot be captured yet.
- `GAP-02`: current HEAD still has no workflow-map / agent-activity / loop-visualization slice, so workflow-map primary/fallback proof cannot be captured yet.
- `GAP-03`: authoritative reused runtime evidence still shows the current `Ideas` -> `Start Run` path failing in focused proof.

### Open Questions

- `QUESTION-01`: should the first implementation round land archive and workflow-map together, or should Proposal 010 be delivered in two staged slices under the same owner-path contract?

## Evidence Gap Review Fallback

- What was attempted:
  - reread the updated proposal after the latest text edits
  - rechecked the relevant reference contracts and current code baseline
  - performed a repeat-round freshness check against the immediately prior review artifacts
  - rendered fresh Xcode MCP previews for the current shell, runs home, provider settings, wizard, readiness, ideas list, start-run sheet, and override sheet
- What is missing:
  - archive-flow proof
  - workflow-map primary-state proof
  - workflow-map fallback-state proof
- Blockers:
  - archive slice is not implemented on current HEAD
  - workflow-map slice is not implemented on current HEAD
  - current `Ideas` owner path is still UI-fragile for Start Run / run-progress proof
- Confidence: `High`
- What can still be said with partial confidence:
  - the previous live proposal-text findings are now closed
  - provider ownership is directionally coherent with current shell reality
  - the remaining gaps are implementation/evidence-level, not proposal-text-level
- What evidence is required to finish the full review:
  - fresh macOS UI proof for archive flow once implemented
  - fresh macOS UI proof for workflow-map primary and fallback states once implemented
  - a stabilized Start Run / run-progress owner path for the final proof round
