# Proposal 007: Full MVP Delivery Slice — Dedicated Worktrees, Implementation Loop, Manual Release, and Dogfooding Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` |
| Repository Root | `.` |
| Git SHA | `63f527054e871f9188e1d83ab5a07b70675f805d` |
| Reviewed At | `2026-03-26T23:19:00+0200` |
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
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
  - `docs/reference/live-provider-execution-slice.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/provider-platform.md`
- Freshness check:
  - the proposal source did not change since the previous written review
  - the relevant delivery/runtime code and tests changed materially, so the old absence-based review was stale
  - current-round build and targeted test evidence were refreshed instead of reused
- Build/run attempts used in this round:
  - fresh `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007p008-build-dd -resultBundlePath /tmp/p007p008-build.xcresult build` passed
  - fresh targeted Proposal 007 slice passed at [`/tmp/p007-review.xcresult`](/tmp/p007-review.xcresult)
  - the targeted UI portion closed stronger than the previous round:
    - `testApprovalGateViewSurface()` passed
    - `testRunProgressViewSurface()` passed
    - `testStartRunSheetUI()` passed
    - `testFullProductCheckpointCanonicalExecution()` skipped in the current headless macOS environment
- Screenshots / attachments in scope:
  - current-round UI attachments live in [`/tmp/p007-review.xcresult`](/tmp/p007-review.xcresult)
- Code areas inspected:
  - `examples/workflows/full-mvp-live.yaml`
  - `DeliveryConfiguration`, `DeliveryPreflightService`, `WorktreeProvisioner`, `RepoSafetyGuard`
  - `WorkflowOrchestrator`, `ReleaseOpsCoordinator`, `GitReleaseService`, `ConnectPublishService`, `DeliveryReceiptBuilder`
  - `IdeaListView`, `ReleaseGateView`, `RunsHomeView`, `EvidencePackBuilder`
- Remaining blockers:
  - no live proposal-text blockers remain
  - this is still not full sign-off evidence for Proposal 007 itself because the round does not include one authoritative happy-path repo-backed dogfood run, one non-happy-path recovery run, a release-gate screenshot set, and final exported delivery receipts/evidence pack from an in-app full session
  - the canonical product-checkpoint UI test still skips in this environment, so the review cannot claim a clean current-round end-to-end proof

## 1. Executive Summary

- Overall readiness: `Yellow`
- Confidence: `High`
- Remaining blockers to full sign-off:
  1. the old review is stale and its “feature absent” findings are no longer true on current `HEAD`
  2. the draft now rereads cleanly, but the proposal’s own sign-off standard still requires a full repo-backed happy-path and non-happy-path dogfood proof that this round does not provide
  3. the strongest current-round evidence is fixture/runtime/shell proof, not a complete exported full-loop delivery session
- Top risks:
  1. readers could mistake the newly green targeted runtime slices for full Proposal 007 completion
  2. release/runtime credibility can still be overstated until one real delivery run exports the exact receipts and evidence pack the proposal demands
  3. the skipped canonical checkpoint can hide final integration issues that narrower targeted slices do not expose
- Top opportunities:
  1. the proposal text itself no longer appears to be the bottleneck
  2. current `HEAD` now really contains the previously missing Proposal 007 building blocks: fixture, delivery configuration, worktree/runtime services, release gate, and evidence export
  3. the next rereview can focus on dogfood proof quality instead of proposal cleanup or basic owner-path reachability

Verdict: no live proposal-text findings surfaced in the current reread. Proposal 007 now reads handoff-ready as a draft, and the repo has advanced far beyond the older “future-state only” baseline. The review still stays `Evidence Gap Review` because the current round does not close Proposal 007’s own full repo-backed dogfood evidence gate.

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

- the current-round targeted UI slice is materially healthier than the previous review baseline and now passes the nearest owner-path surfaces in [`/tmp/p007-review.xcresult`](/tmp/p007-review.xcresult)

### UX

No live UX proposal-text findings surfaced in this reread.

Current evidence-level note:

- the draft remains consistent with the current operator and provider-platform references, and the `Start Run -> Run Progress -> Approval` owner path is no longer the blocker it was in the earlier round

### Architecture

No live architecture proposal-text findings surfaced in this reread.

Current evidence-level note:

- current repo inspection shows that the earlier “missing component” findings are closed at the code level, but the review still lacks the full dogfood evidence required to validate the whole repo-backed contract in one lived session

## 4. Cross-Discipline Conflicts and Decisions

- Conflict: the proposal draft is now cleaner than the evidence story for full end-to-end repo-backed proof.
  Tradeoff: mark Proposal 007 effectively green because the fixture/runtime slices exist, versus keep the overall verdict partial until the proposal’s own dogfood evidence gate is met.
  Decision: no proposal-text findings remain, but the overall review stays `Evidence Gap Review` with `Readiness = Yellow` because the current round still does not prove one complete happy-path and one complete non-happy-path repo-backed delivery session from inside the app.
  Owner: implementation / evidence owner

## 5. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source |
|---|---|---|---|---|---|---|---|
| P1 | Execute one current-head happy-path repo-backed dogfood run and export the full evidence pack | Architecture + UX | Implementation owner | Next rereview | current runtime remains green | one completed run yields release receipts, report, and exported pack from inside the app | Evidence gap only |
| P1 | Execute one current-head non-happy-path release/recovery run with preserved receipts and operator-visible recovery path | Architecture + UX | Implementation owner | Next rereview | same as above | one blocked/recovered (or intentionally cancelled) repo-backed run is evidenced end-to-end | Evidence gap only |
| P1 | Capture explicit release-gate and final-receipts screenshot/attachment proof from the repo-backed flow | UI | Implementation owner | Next rereview | happy/non-happy runs available | the review can cite release-gate and final receipt attachments directly instead of inferring them from targeted shell tests | Evidence gap only |
| P2 | Rerun the canonical product-checkpoint flow in an environment where the headless skip does not apply | UI + UX | Implementation owner | Next rereview | UI harness environment stable | `testFullProductCheckpointCanonicalExecution()` closes without skip | Evidence gap only |

## 6. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Hold Criteria |
|---|---|---|---|---|---|
| Repo-backed dogfood proof | whether one full happy-path run finishes with real receipts and exportable evidence | completed run, receipts, exported pack, release-gate attachments | do not substitute fixture-only proof for full lived-session evidence | next rereview | hold if full repo-backed pack still does not exist |
| Recovery credibility | whether one non-happy-path release/recovery run is operator-complete | preserved receipts and visible recovery context | do not accept raw-log-only recovery evidence | next rereview | hold if recovery still requires guesswork |
| End-to-end checkpoint | whether the canonical checkpoint is green on current `HEAD` | non-skipped checkpoint UI test or equivalent direct evidence | do not treat headless skip as proof | next rereview | hold if full checkpoint remains skipped or absent |
| Release-gate trust | whether the operator can inspect release context before approval | release-gate screenshot plus receipts/report from the same run | do not infer release context from adjacent shells | next rereview | hold if release-gate proof remains indirect |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: this round does not include a full current-head happy-path repo-backed dogfood run with exported evidence pack
- `GAP-02`: this round does not include a full current-head non-happy-path recovery run with preserved release receipts
- `GAP-03`: the canonical end-to-end product-checkpoint UI flow is still skipped in the current headless environment
- `GAP-04`: the round does not yet provide explicit release-gate and final-receipts attachments from a real repo-backed delivery session

### Open Questions

- No live proposal-text open questions remain from this reread.

## Evidence Gap Review Fallback

- What was attempted:
  - reread Proposal 007 end-to-end against current adjacent reference docs
  - refreshed build and targeted Proposal 007 runtime/UI evidence on current `HEAD`
  - rechecked the old review’s “missing runtime” claims against actual current source
- What is missing:
  - one happy-path repo-backed dogfood run
  - one non-happy-path repo-backed recovery run
  - exported evidence pack and release-gate/final-receipts attachments from those runs
  - a non-skipped canonical product checkpoint
- Blockers:
  - current round is still narrower than the proposal’s own end-to-end evidence requirement
- Confidence: `High`
- What can still be said with partial confidence:
  - the old “Proposal 007 runtime is absent” review is no longer accurate on current `HEAD`
  - no live proposal-text blockers remain in the draft
  - targeted build/runtime/shell evidence is green enough to treat the proposal as draft-ready
- What evidence is required to finish the full review:
  - full repo-backed happy-path proof
  - full repo-backed non-happy-path proof
  - exported evidence pack
  - release-gate and final-receipt screenshots / attachments from the real flow
