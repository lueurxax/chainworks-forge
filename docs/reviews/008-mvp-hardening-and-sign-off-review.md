# Proposal 008: MVP Hardening and Sign-Off Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
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
  - `docs/proposals/008-mvp-hardening-and-sign-off.md`
  - `docs/ps/chainworks-forge-mvp.md`
  - `docs/reference/runtime-contract.md`
  - current Proposal 007 review and current repo baseline for delivery/runtime shell ownership
- Freshness check:
  - the proposal source did not change since the previous written review
  - the dependency baseline changed materially: Proposal 007 code/runtime slices now exist and the old “future-state only” baseline is stale
  - current-round build and focused shell/UI evidence were refreshed instead of blindly reusing the old report
- Build/run attempts used in this round:
  - fresh `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007p008-build-dd -resultBundlePath /tmp/p007p008-build.xcresult build` passed
  - fresh targeted Proposal 008 shell pass succeeded at [`/tmp/p008-review.xcresult`](/tmp/p008-review.xcresult): `5` tests, `0` failures
  - the focused UI slice closed cleanly for:
    - `testApprovalInboxReachable()`
    - `testProviderSettingsTabReachable()`
    - `testPilotReadinessRefreshSurface()`
    - `testStartRunSheetUI()`
    - `testRunProgressViewSurface()`
- Screenshots / attachments in scope:
  - current-round UI attachments live in [`/tmp/p008-review.xcresult`](/tmp/p008-review.xcresult)
- Code areas inspected:
  - current operator shell: `RunsHomeView`, `RecoverySheet`, `RunReportView`, `RunComparisonView`, `ForegroundBannerView`
  - current provider/settings shell: `ProviderSettingsView`, `PilotReadinessView`, `ContentView`
  - current attachment/reference path: `Idea`, `IdeaListView`, `GooseSessionBridge`
  - current absence baseline for 008-specific benchmark/sign-off models, services, and views
- Remaining blockers:
  - no live proposal-text blockers remain
  - Proposal 008 still cannot move beyond a partial evidence review because its own benchmark/sign-off runtime slice is not yet implemented on current `HEAD`
  - section `1.1` correctly blocks 008 behind Proposal 007 current-head green repo-backed evidence, and the current 007 rereview remains partial rather than fully green

## 1. Executive Summary

- Overall readiness: `Yellow`
- Confidence: `High`
- Remaining blockers to full sign-off:
  1. the draft now rereads cleanly, but Proposal 008 is explicitly sequenced after a stronger Proposal 007 evidence state than the repo currently proves
  2. current-round evidence is strong for the shell/operator baseline that Proposal 008 extends, but the proposal’s own benchmark/sign-off entities and routes are still absent on `HEAD`
  3. this round can validate draft readiness and adjacent-shell fit, not implemented-flow behavior for `BenchmarkCohort`, `MVPSignOffEvaluator`, `CompletedRunExportHub`, or `MVPSignOffSummaryView`
- Top risks:
  1. a clean draft could be mistaken for immediate implementation readiness even though its hard prerequisite is still unmet
  2. later readers could reuse the old review’s stale “007 is future-state only” framing instead of the current subtler truth: 007 slices exist, but full dogfood proof is still incomplete
  3. adjacent-shell green proof can be overread as proof of 008-specific sign-off/runtime ownership
- Top opportunities:
  1. the old proposal-text findings appear closed in the current draft
  2. the current shell now aligns well with 008’s shell-ownership story
  3. once Proposal 007 closes its own dogfood evidence gate, Proposal 008 can move into an implementation audit on a much cleaner baseline

Verdict: no live proposal-text findings surfaced in the current reread. Proposal 008 now reads coherent and directionally handoff-ready as a draft. The review remains `Evidence Gap Review` only because the proposal is intentionally blocked behind stronger Proposal 007 evidence and because 008-specific benchmark/sign-off implementation surfaces do not yet exist on current `HEAD`.

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

- the current-round targeted shell/UI pass in [`/tmp/p008-review.xcresult`](/tmp/p008-review.xcresult) is clean, which strengthens the proposal’s shell-ownership claims but does not yet prove 008-specific export/sign-off subroutes

### UX

No live UX proposal-text findings surfaced in this reread.

Current evidence-level note:

- the draft’s reference-only attachment policy, approval relaunch rule, and shell-owned recovery/export framing now match current runtime reality much more closely than in the older review

### Architecture

No live architecture proposal-text findings surfaced in this reread.

Current evidence-level note:

- repo inspection still shows no landed 008-specific benchmark/sign-off entities, evaluator, recorder, or sign-off surfaces on current `HEAD`, so the review remains partial on implementation evidence rather than proposal quality

## 4. Cross-Discipline Conflicts and Decisions

- Conflict: the proposal draft is now cleaner than the dependency/runtime state it intentionally waits on.
  Tradeoff: mark Proposal 008 effectively green because the draft is coherent, versus keep the overall triad verdict partial until Proposal 007 closes and 008-specific runtime surfaces exist.
  Decision: no proposal-text findings remain, but the overall review stays `Evidence Gap Review` with `Readiness = Yellow` because the proposal correctly blocks itself behind a dependency and implementation state that are not yet fully closed.
  Owner: roadmap / implementation owner

## 5. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source |
|---|---|---|---|---|---|---|---|
| P1 | Close Proposal 007’s current-head repo-backed dogfood evidence prerequisite before starting 008 implementation | Architecture + UX | Implementation owner | Before 008 implementation | Proposal 007 rereview | Proposal 007 is no longer partial / evidence-gap based on repo-backed full-loop proof | Evidence gap only |
| P1 | Land the 008-specific persisted benchmark/sign-off model and services (`BenchmarkCohort`, `BenchmarkExecutionRecord`, `BenchmarkPair`, `MVPSignOffDecisionSnapshot`, `BenchmarkRunRecorder`, `MVPSignOffEvaluator`, `SignOffEvidencePackBuilder`) | Architecture | Implementation owner | 008 implementation phase | P1 above | repo search returns real implementation hits and tests exist | Evidence gap only |
| P1 | Land the 008-specific shell-owned routes/subviews (`BlockedRunRecoveryView`, `CompletedRunExportHub`, `MVPSignOffSummaryView`) | UI + UX | Implementation owner | 008 implementation phase | benchmark/sign-off model available | sign-off/export/recovery proof comes from real 008 surfaces, not adjacent shell baselines | Evidence gap only |
| P2 | Rerun the review with one happy-path and one recovered non-happy-path sign-off packet once the 008 runtime exists | UI + UX + Architecture | Implementation owner | Next rereview | previous items complete | review can cite real 008 benchmark/sign-off packets | Evidence gap only |

## 6. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Hold Criteria |
|---|---|---|---|---|---|
| Dependency truth | whether 008 starts only after 007 is really green on current `HEAD` | current 007 review status and dogfood evidence exist | do not start 008 on a partial 007 baseline | before 008 implementation | hold if 007 remains partial |
| Benchmark replayability | whether `GO/HOLD` can be recomputed from persisted benchmark records only | landed cohort/pair/decision models and evaluator tests | do not allow notebook-only arbitration | after 008 model landing | hold if sign-off still depends on external notes |
| Shell ownership | whether recovery/export/sign-off stay inside the current shell hierarchy | real 008 subroutes reachable from `RunsHomeView` / `RecoverySheet` / `RunReportView` | do not create parallel top-level destinations | next rereview | hold if routing fragments |
| Attachment truth | whether reference-only attachment behavior remains aligned with runtime and copy | no runtime ingestion path for `attachmentPath`, clear UI labeling | do not imply agent ingestion | next rereview | hold if product language outruns runtime |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: current `HEAD` still has no landed 008-specific benchmark/sign-off models, services, or views
- `GAP-02`: Proposal 008 is intentionally blocked behind stronger Proposal 007 evidence than the current repo can yet show
- `GAP-03`: this round can only prove adjacent current shell behavior, not the proposal’s own sign-off packet flow

### Open Questions

- No live proposal-text open questions remain from this reread.

## Evidence Gap Review Fallback

- What was attempted:
  - reread Proposal 008 against the PS, runtime contract, and current shell/runtime reality
  - refreshed current-round build and targeted shell/UI evidence
  - rechecked the old 008 findings against the current draft and current 007 baseline
- What is missing:
  - real 008 benchmark/sign-off runtime surfaces
  - one happy-path and one recovered non-happy-path sign-off packet
  - a fully closed Proposal 007 prerequisite
- Blockers:
  - Proposal 008 is intentionally sequenced after a stronger dependency proof than current `HEAD` yet provides
- Confidence: `High`
- What can still be said with partial confidence:
  - the current draft no longer surfaces live proposal-text blockers
  - the current shell/operator baseline now aligns with 008 much better than the older review claimed
  - the remaining limitation is dependency/runtime evidence, not document coherence
- What evidence is required to finish the full review:
  - Proposal 007 current-head green repo-backed dogfood proof
  - implemented 008 benchmark/sign-off/runtime surfaces
  - real sign-off packets and screenshot/attachment evidence from those flows
