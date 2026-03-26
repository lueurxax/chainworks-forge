# Proposal 011: Run Control, Working Directory Ownership, and Provider Binding Truth Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md` |
| Repository Root | `.` |
| Git SHA | `0b2ca31` |
| Reviewed At | `2026-03-26T22:22:44+0200` |
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
  - `docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/provider-platform.md`
- Freshness check:
  - the proposal was reread against current `HEAD`
  - the proposal changed since the previous written review
  - relevant runtime / provider / UI files changed, so current-round `build`, targeted unit, and targeted UI evidence were refreshed instead of reused
- Build/run attempts used in this round:
  - fresh `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' build` passed
  - fresh targeted unit slice passed at [`/tmp/p011-r3-unit.xcresult`](/tmp/p011-r3-unit.xcresult): `33` passed, `0` failed, `0` skipped
  - fresh targeted UI slice passed at [`/tmp/p011-r3-ui.xcresult`](/tmp/p011-r3-ui.xcresult): `5` passed, `0` failed, `0` skipped
- Screenshots / attachments in scope:
  - current-round UI attachments live in the UI xcresult above
- Code areas inspected:
  - `WorkflowDefinition`, `RunPlan`, `ExecutionService`, `WorkflowOrchestrator`
  - `BackendProfileResolverV2`, `RunReportBuilder`, `RunComparisonService`
  - `IdeaListView`, `Run`, `AgentExecution`
- Remaining blockers:
  - no live proposal-text blockers remain
  - evidence is still partial because current `HEAD` does not yet expose the full typed Proposal 011 runtime slice (`bindingProvenanceJSON`, `cancellationSettlementLog`, `workspaceRootPath`, shared `requiresProjectAccess` implementation seam, explicit stop/cancellation owner services)

## 1. Executive Summary

- Overall readiness: `Yellow`
- Confidence: `High`
- Remaining blockers to full sign-off:
  1. no live draft blocker remains, but the review still cannot claim full implemented-flow closure because the dedicated Proposal 011 runtime slice is not fully landed on current `HEAD`
  2. current-round proof is strong for the nearest owner path (`Ideas -> Start Run -> Run Progress -> Approval`), but it is still adjacent-shell proof rather than proof of fully realized 011-specific persistence / stop-settlement behavior
- Top risks:
  1. implementation can still drift from the now-clean draft if the missing typed seams land inconsistently
  2. operator-facing history can still remain partially implicit until the new 011 fields and stop/cancellation truth path exist in runtime
  3. a later review could overread the green owner-path proof as full implementation closure if the distinction between nearby shells and true 011 seams is not kept explicit
- Top opportunities:
  1. the old provenance finding is closed: section `6.5` now truthfully persists `.unverifiable` instead of inventing `.backendProfileDefault`
  2. current-round build, targeted unit, and targeted UI evidence are all green
  3. the next rereview can focus on implementation closure instead of further proposal editing

Verdict: no live proposal-text findings surfaced in the current reread. Proposal 011 now reads handoff-ready as a draft. The report stays `Evidence Gap Review` only because the implemented runtime slice on current `HEAD` is still narrower than the full Proposal 011 contract.

## 2. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Partial | 0 | 0 | 0 | 0 |
| UX | Green | High | Partial | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Partial | 0 | 0 | 0 | 0 |

## 3. Findings by Discipline

### UI

No live UI proposal-text findings surfaced in this reread.

Current evidence-level note:

- the focused macOS owner-path UI slice is now green: `testProviderSettingsTabReachable()`, `testPilotReadinessRefreshSurface()`, `testStartRunSheetUI()`, `testRunProgressViewSurface()`, and `testApprovalGateViewSurface()` all passed in [`/tmp/p011-r3-ui.xcresult`](/tmp/p011-r3-ui.xcresult)

### UX

No live UX proposal-text findings surfaced in this reread.

Current evidence-level note:

- the draft now truthfully distinguishes frozen provenance from unverifiable provenance instead of coercing unknown history into a false source

### Architecture

No live architecture proposal-text findings surfaced in this reread.

Current evidence-level note:

- repo inspection still shows that the typed runtime slice described by Proposal 011 is only partially implemented on current `HEAD`, so the report remains partial as implementation evidence even though the draft itself is now clean

## 4. Cross-Discipline Conflicts and Decisions

- Conflict: the proposal draft is now materially cleaner than the implemented runtime slice on current `HEAD`.
  Tradeoff: mark the proposal fully green on text quality versus keep the overall triad verdict partial until the runtime seams actually exist.
  Decision: no proposal-text findings remain, but the overall review stays `Evidence Gap Review` with `Readiness = Yellow` because the current review still cannot prove the full Proposal 011 runtime contract from live implementation evidence.
  Owner: implementation owner

## 5. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source |
|---|---|---|---|---|---|---|---|
| P2 | Land the typed Proposal 011 runtime seams (`bindingProvenanceJSON`, `cancellationSettlementLog`, `workspaceRootPath`, shared `requiresProjectAccess`, explicit stop/cancellation truth path`) | Architecture | Implementation owner | Next implementation slice | none | runtime fields / services exist and are consumed by owner surfaces | Evidence gap only |
| P2 | Rerun the review once the 011-specific owner surfaces exist, not just the adjacent `Ideas` shell | UI / UX | Implementation owner | Next rereview | 011 runtime slice implemented | review can rely on direct 011 flow evidence instead of adjacent-shell proof | Evidence gap only |

## 6. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Hold Criteria |
|---|---|---|---|---|---|
| Run control truth | whether `cancelled` only appears after real settlement propagation is recorded | new cancellation truth fields and settlement path land in runtime | do not regress to timestamp-only or local-flag semantics | next rereview | hold if settlement is still implicit |
| Working-directory ownership | whether project-backed runs share one explicit frozen directory contract | shared `requiresProjectAccess` path lands across start, preflight, compiler, and resume | do not let owner surfaces infer cwd implicitly | next rereview | hold if path truth is still ambient |
| Provider/model truth | whether frozen provenance is stored and displayed from run-start truth | provenance fields land and are used by history/report/comparison surfaces | do not derive old runs from mutable current settings | next rereview | hold if provenance remains display-time inference |
| Owner-path proof | whether dedicated 011 owner surfaces become reachable and stable | new stop / cancellation / provenance UI exists and is testable | do not treat adjacent-shell proof as full 011 closure | next rereview | hold if only adjacent shells are testable |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: the current review does not yet have direct live proof for dedicated Proposal 011 runtime seams, because those seams are still not fully implemented on current `HEAD`
- `GAP-02`: current green UI proof is for the nearest owner shell, not for a fully realized stop / cancellation-settlement / frozen-provenance surface

### Open Questions

- No live proposal-text open questions remain from this reread.

## Evidence Gap Review Fallback

- What was attempted:
  - reread Proposal 011 end-to-end against current reference docs and current code
  - reran fresh build, targeted unit, and targeted UI evidence for the current round
  - rechecked the old provenance finding against the updated section `6.5`
- What is missing:
  - full implementation evidence for the typed 011 runtime seams
  - direct owner-surface proof for stop / cancellation settlement / frozen binding provenance once those surfaces land
- Blockers:
  - current `HEAD` still does not expose the full Proposal 011 runtime slice
- Confidence: `High`
- What can still be said with partial confidence:
  - the current draft no longer has live proposal-text blockers
  - the closest owner-path UI proof is green in the current round
  - the remaining limitation is implementation evidence, not proposal quality
- What evidence is required to finish the full review:
  - runtime implementation of the 011-specific typed seams
  - a direct rerun against those dedicated owner surfaces once they exist
