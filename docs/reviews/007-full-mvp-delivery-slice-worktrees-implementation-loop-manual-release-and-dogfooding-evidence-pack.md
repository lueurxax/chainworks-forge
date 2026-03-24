# Proposal 007 Evidence Pack

| Field | Value |
|---|---|
| Proposal | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` |
| Prepared At | `2026-03-24T21:12:04+0200` |
| Proposal MD5 | `6ae2f8e1e84999b0029ee9acfd6f7b64` |
| Proposal MTime | `2026-03-24 20:58:05 +0200` |
| Repository SHA | `e63d440` |
| Evidence Completeness | `Partial` |

## Scope

Review target:
- current Proposal 007 draft readiness
- current-head baseline relevant to repo-backed delivery
- fresh build and fresh macOS UI-attempt evidence for the current shell

Out of scope for live review:
- unimplemented Proposal 007 repo-backed runtime
- `full-mvp-live.yaml` dogfood execution
- real worktree provisioning and release side effects

## Evidence Items

### E-P007-001 — Proposal source reread

- File:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- Key sections reviewed:
  - product question and definition of done
  - scope and state map
  - delivery configuration boundary
  - repository profile schema
  - persisted runtime metadata
  - release/preflight rules
  - Start Run and release-gate UI
  - DSL deltas
  - acceptance criteria
- Current round conclusion:
  - explicit 12-state/manual-gate topology is now documented
  - explicit pre-run delivery configuration boundary is now documented
  - no new proposal-text inconsistencies surfaced in this reread

### E-P007-002 — Fresh source and fixture absence check for Proposal 007 runtime

- Command:
  - `rg -n "full-mvp-live|DeliveryConfiguration|RepositoryProfile|DeliveryPreflightService|WorktreeProvisioner|RepoSafetyGuard|ReleaseOpsCoordinator|GitReleaseService|ConnectPublishService|DeliveryReceiptBuilder|ReleaseGateView" 'Chainworks Forge' 'examples' 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- Result:
  - no hits in current repo source
- Meaning:
  - Proposal 007 terminology now exists in the document
  - but the actual runtime slice is still absent in code and fixtures

### E-P007-003 — Fresh build on current HEAD

- Command:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-p007-r2-build build`
- Result:
  - `passed`
- Derived data:
  - `/tmp/codex-dd-p007-r2-build`
- Observed nuance:
  - build is green
  - Swift warnings remain in parser/validator/runtime/test code, but they did not block this build

### E-P007-004 — Fresh macOS UI-baseline rerun on current HEAD

- Command:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-p007-r2-ui -resultBundlePath /tmp/codex-p007-r2-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution'`
- Result:
  - `failed before UI tests actually ran`
- xcresult:
  - `/tmp/codex-p007-r2-ui.xcresult`
- Summary:
  - `0` passed
  - `1` failed
  - total test count reported: `1`
- Failure:
  - `Chainworks ForgeUITests-Runner (...) encountered an error`
  - `The test runner failed to initialize for UI testing. (Underlying Error: Timed out while enabling automation mode.)`
- Meaning for Proposal 007:
  - this round does not provide fresh UI screenshots/attachments
  - current-shell macOS UI evidence regressed relative to the previous 007 pass

### E-P007-005 — Fresh workspace-state check

- Commands:
  - `git rev-parse --short HEAD`
  - `git status --short`
- Result:
  - reviewed SHA: `e63d440`
  - worktree is dirty
- Relevant current-round nuance:
  - the proposal file changed since the previous 007 review
  - relevant runtime/UI files also changed, including app shell and UI tests
  - because of that, prior-round UI attachments were not considered fresh enough to reuse as current-round primary UI evidence

## Attempted But Missing

- No Proposal 007 runtime screenshots were captured in this round because UI automation failed before tests initialized.
- No `full-mvp-live.yaml` compile/run attempt was possible because the fixture still does not exist.
- No repo-backed happy-path or non-happy-path dogfood run was possible because the runtime slice remains unimplemented.
- No Proposal 007 evidence-pack export could be reviewed live because the feature does not exist yet.

## Evidence Gate Assessment

- repo/code inspection: `met`
- build/run attempt logged: `met`
- platform-appropriate UI screenshots/attachments: `not met in current round`
- completed evidence pack with IDs: `met`

Why this remains `Partial`:
- proposal-local evidence is fresh and the main draft issues are closed
- but the reviewed slice itself is still unimplemented
- and the current-round UI evidence attempt failed before it produced live attachments
