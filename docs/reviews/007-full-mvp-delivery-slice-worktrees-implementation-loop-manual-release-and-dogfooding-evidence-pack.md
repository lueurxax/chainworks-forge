# Proposal 007 Evidence Pack

| Field | Value |
|---|---|
| Proposal | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` |
| Prepared At | `2026-03-25T19:46:19+0200` |
| Proposal MD5 | `4b89cb30f9b5a30d3b32657391e117a2` |
| Proposal MTime | `2026-03-25 11:13:19 +0200` |
| Repository SHA | `e1655a6bd7547bb1a03f46a97258d108a11ac109` |
| Evidence Completeness | `Partial` |

## Scope

Review target:
- current Proposal 007 draft readiness
- current-head baseline relevant to repo-backed delivery
- fresh current-round macOS UI-baseline attempt for the existing shell

Out of scope for live review:
- unimplemented Proposal 007 repo-backed runtime
- `full-mvp-live.yaml` dogfood execution
- real worktree provisioning and release side effects

## Evidence Items

### E-P007-001 — Proposal source reread

- File:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- Key sections reviewed:
  - context and definition of done
  - state map
  - delivery configuration boundary
  - repository profile schema
  - persisted runtime metadata
  - delivery preflight
  - Start Run and release-gate UI
  - DSL deltas
  - dogfooding pack
  - acceptance/testing strategy
- Current round conclusion:
  - explicit 12-state/manual-gate topology remains documented
  - explicit pre-run delivery configuration boundary remains documented
  - no new proposal-text inconsistencies surfaced in this reread

### E-P007-002 — Fresh source and fixture absence check for Proposal 007 runtime

- Commands:
  - `rg -n "full-mvp-live|DeliveryConfiguration|RepositoryProfile|DeliveryPreflightService|WorktreeProvisioner|RepoSafetyGuard|ReleaseOpsCoordinator|GitReleaseService|ConnectPublishService|DeliveryReceiptBuilder|ReleaseGateView|EvidencePackExport|Delivery" 'Chainworks Forge' 'examples' 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
  - `rg --files 'examples' 'Chainworks Forge' | rg 'full-mvp|worktree|release|delivery'`
- Result:
  - no runtime/service hits for the Proposal 007 terms above
  - only relevant workflow-like file found: `examples/workflows/proposal-to-release.yaml`
- Meaning:
  - Proposal 007 terminology exists in the document
  - but the actual repo-backed runtime slice is still absent in code and fixtures

### E-P007-003 — Fresh macOS UI-baseline rerun on current HEAD

- Command:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-p007-r3-ui -resultBundlePath /tmp/codex-p007-r3-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution'`
- Result:
  - `failed`
  - UI automation initialized successfully
  - build phase succeeded as part of the `xcodebuild test` run
- xcresult:
  - `/tmp/codex-p007-r3-ui.xcresult`
- Summary:
  - `1` passed
  - `2` failed
  - `1` skipped
  - total test count: `4`
- Detailed outcomes:
  - `testApprovalGateViewSurface()` passed
  - test log recorded attachment name: `REQ011_ApprovalGate`
  - `testStartRunSheetUI()` failed with `XCTAssertTrue failed - Start Run sheet must be reachable for seeded idea`
  - `testRunProgressViewSurface()` failed with `XCTAssertTrue failed - Start Run sheet must be reachable for seeded idea`
  - `testFullProductCheckpointCanonicalExecution()` skipped with `Skipping: cannot create idea in headless xcodebuild (toolbar not accessible)`
- Meaning for Proposal 007:
  - current-shell evidence is better than the old automation-timeout round
  - but the seeded Start Run path is still not reliable enough to close the review gate

### E-P007-004 — Fresh workspace-state check

- Commands:
  - `git rev-parse HEAD`
  - proposal file mtime/MD5
- Result:
  - reviewed SHA: `e1655a6bd7547bb1a03f46a97258d108a11ac109`
  - proposal mtime: `2026-03-25 11:13:19 +0200`
  - proposal MD5: `4b89cb30f9b5a30d3b32657391e117a2`
- Meaning:
  - this review is anchored to the current local proposal revision and current HEAD

## Attempted But Missing

- No Proposal 007 runtime screenshots were captured for the full repo-backed flow because the slice is still unimplemented.
- No `full-mvp-live.yaml` compile/run attempt was possible because the fixture still does not exist.
- No repo-backed happy-path or non-happy-path dogfood run was possible because the runtime slice remains unimplemented.
- No Proposal 007 evidence-pack export could be reviewed live because the feature does not exist yet.

## Evidence Gate Assessment

- repo/code inspection: `met`
- build/run attempt logged: `met`
- platform-appropriate UI screenshots/attachments: `not met for the full Proposal 007 flow`
- completed evidence pack with IDs: `met`

Why this remains `Partial`:
- proposal-local evidence is fresh and the draft is materially healthier
- but the reviewed slice itself is still largely unimplemented
- and the current-round UI attempt proves only baseline shell reachability, not the Proposal 007 repo-backed delivery flow
