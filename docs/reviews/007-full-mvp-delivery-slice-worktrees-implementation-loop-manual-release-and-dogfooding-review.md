# Proposal 007: Full MVP Delivery Slice — Worktrees, Implementation Loop, Manual Release, and Dogfooding Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` |
| Repository Root | `.` |
| Git SHA | `e1655a6bd7547bb1a03f46a97258d108a11ac109` |
| Reviewed At | `2026-03-25T19:46:19+0200` |
| Proposal Source MD5 | `4b89cb30f9b5a30d3b32657391e117a2` |
| Proposal Source MTime | `2026-03-25 11:13:19 +0200` |
| Review Mode | `full-review` |
| Overall Status | `Evidence Gap Review` |
| Readiness | `Yellow` |
| Confidence | `High` |
| Evidence Completeness | `Partial` |

## Verdict

Current Proposal 007 rereads cleanly as a design draft. The earlier workflow-topology and delivery-configuration gaps remain closed in the current version, and I did not surface any new proposal-text inconsistencies in this pass.

This is still not a clean sign-off. Current repo reality remains substantially behind the proposal: there is still no `examples/workflows/full-mvp-live.yaml`, no `DeliveryConfiguration` implementation, and no repo-backed delivery/runtime services in current source. Fresh macOS UI evidence improved relative to the older round because the runner now starts and one baseline UI test passes, but the proof is still incomplete: `testApprovalGateViewSurface()` passed, `testFullProductCheckpointCanonicalExecution()` skipped in headless mode, and both `testStartRunSheetUI()` and `testRunProgressViewSurface()` failed because the Start Run sheet is not reachable for the seeded idea path. That is not enough to close a full-review gate for Proposal 007.

## Findings

No live proposal-text findings surfaced in the current reread.

## Evidence Summary

- `E-P007-001`: Fresh reread of the current Proposal 007 draft
  - previous topology/config-boundary issues remain closed
  - no new proposal-text inconsistencies surfaced
- `E-P007-002`: Fresh current-head source/fixture absence check for Proposal 007 runtime
  - no `full-mvp-live.yaml`
  - no `DeliveryConfiguration`
  - no `RepositoryProfile`
  - no `DeliveryPreflightService`
  - no `WorktreeProvisioner`
  - no `RepoSafetyGuard`
  - no `ReleaseOpsCoordinator`
  - no `GitReleaseService`
  - no `ConnectPublishService`
  - no `DeliveryReceiptBuilder`
  - no `ReleaseGateView`
- `E-P007-003`: Fresh macOS UI-baseline rerun on current HEAD
  - `1` passed
  - `2` failed
  - `1` skipped
  - xcresult: `/tmp/codex-p007-r3-ui.xcresult`
  - `testApprovalGateViewSurface()` passed and recorded an approval-gate attachment
  - `testStartRunSheetUI()` failed: `Start Run sheet must be reachable for seeded idea`
  - `testRunProgressViewSurface()` failed: `Start Run sheet must be reachable for seeded idea`
  - `testFullProductCheckpointCanonicalExecution()` skipped: headless toolbar path cannot create idea
- `E-P007-004`: Fresh current workspace state
  - review is anchored to current local proposal revision and current HEAD

## Missing Evidence

- No `examples/workflows/full-mvp-live.yaml` exists in the repo yet.
- No Proposal 007 runtime/services exist in current source:
  - no worktree provisioner
  - no repo safety guard
  - no delivery preflight service
  - no deterministic git/connect release services
  - no delivery receipt builder
  - no release-gate surface
- No repo-backed happy-path dogfood run exists on current HEAD.
- No repo-backed non-happy-path recovery run exists on current HEAD.
- No full Proposal 007 screenshot pack exists for:
  - Start Run preset
  - implementation loop
  - manual release gate
  - completed run with receipts

## What Can Still Be Said With Partial Confidence

- The current draft reads much closer to handoff-ready than earlier 007 revisions.
- Current repo evidence still says Proposal 007 is future-state:
  - the repo-backed workflow fixture is absent
  - the delivery runtime/services are absent
  - the current app shell cannot yet prove the full repo-backed delivery path
- The UI harness is healthier than in the older round because automation starts and the approval-gate surface is reachable, but the seeded Start Run path is still unstable.

## What Is Required To Finish The Full Review

- Implement the Proposal 007 slice:
  - `examples/workflows/full-mvp-live.yaml`
  - delivery configuration and delivery preflight services
  - worktree/repo safety runtime
  - deterministic release services
  - release-gate and evidence-pack export surfaces
- Make the Start Run seeded-idea path reliable in current macOS UI tests.
- Re-run the review with:
  - one happy-path repo-backed dogfood run
  - one non-happy-path recovery run
  - a complete screenshot/attachment set
  - final receipts and exported evidence pack
