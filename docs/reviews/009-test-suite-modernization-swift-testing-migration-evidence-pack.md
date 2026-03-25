# Proposal 009 Evidence Pack

| Field | Value |
|---|---|
| Proposal | `docs/proposals/009-test-suite-modernization-swift-testing-migration.md` |
| Prepared At | `2026-03-25T11:57:38+0200` |
| Proposal MD5 | `6f4f7127bfd0cf24aa60874232fdad7f` |
| Proposal MTime | `2026-03-25 11:50:33 +0200` |
| Repository SHA | `3459c9471b16d3ff776c33ddf1829e117279fd87` |
| Evidence Completeness | `Partial` |

## Scope

Review target:
- current Proposal 009 draft readiness
- current-head baseline for test inventory, mock requirements, and gate tooling
- fresh current-round build/test proof against the repo's existing gate script

Out of scope for live review:
- user-facing app UI, because Proposal 009 is an internal test-suite modernization slice
- migrated implementation artifacts that do not yet exist in current source
- non-primary-source external claims beyond official Apple documentation about Swift Testing / Xcode workflows

## Evidence Items

### E-P009-001 — Proposal source reread

- File:
  - `docs/proposals/009-test-suite-modernization-swift-testing-migration.md`
- Key sections reviewed:
  - context and definition of done
  - current CI tooling baseline
  - shared infrastructure upgrade
  - two-lane Goose transport mock strategy
  - Xcode Test Plans gate model
  - file-by-file migration plan
  - acceptance criteria
- Current round conclusion:
  - the draft now consistently describes:
    - `17` executable XCTest unit files
    - `1` executable Swift Testing unit file
    - `2` helper files
  - the CI/tag path is now `.xctestplan` + `xcodebuild -testPlan`
  - the mock strategy now explicitly distinguishes stateless vs observation-heavy tests

### E-P009-002 — Fresh current-head unit-test inventory baseline

- Command:
  - Python inventory scan over `Chainworks ForgeTests/*.swift`
- Result:
  - `ALL_TESTS 20`
  - `HELPERS 2`
  - `EXECUTABLE_UNIT_FILES 18`
  - `XCTEST_EXECUTABLE_FILES 17`
  - `SWIFT_TESTING_EXECUTABLE_FILES 1`
- Meaning:
  - current repo reality matches the updated proposal framing
  - helper files are correctly separated from executable migration targets

### E-P009-003 — Fresh current-head mock-observation baseline

- Files:
  - `Chainworks ForgeTests/SharedMocks.swift`
  - `Chainworks ForgeTests/GooseAgentExecutorTests.swift`
  - `Chainworks ForgeTests/GooseSessionBridgeTests.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
- Key facts:
  - current tests assert on richer transport-side effects such as:
    - session closure
    - captured session request
    - propagated execution-policy fields
    - call counts
- Meaning:
  - the proposal's new two-lane strategy is now aligned with the current suite:
    - `StubGooseTransport` for pure stimulus-only cases
    - `ObservableGooseTransport` for observation-heavy cases

### E-P009-004 — Fresh external primary-source confirmation

- Sources:
  - Apple Swift Testing overview: [Swift Testing](https://developer.apple.com/documentation/testing)
  - Apple Swift Testing page: [Swift Testing - Xcode](https://developer.apple.com/xcode/swift-testing/)
  - WWDC24: [Go further with Swift Testing](https://developer.apple.com/videos/play/wwdc2024/10195/)
- Key facts used:
  - Swift Testing applies to Xcode projects, not only SwiftPM
  - Apple explicitly documents tags as an organization/run mechanism
  - WWDC24 explicitly shows saving tag preferences into Xcode Test Plans and filtering test plans by tags
- Meaning:
  - the proposal's move from `swift test --filter` to `.xctestplan` is now aligned with primary-source Xcode guidance

### E-P009-005 — Fresh current-round build gate

- Command:
  - `./scripts/test-gate.sh build`
- Result:
  - `passed`
  - `** BUILD SUCCEEDED **`
- Meaning:
  - the old network/package-resolution blocker from the previous round is no longer the current issue
  - this round has a valid green build baseline

### E-P009-006 — Fresh current-round fast gate

- Command:
  - `./scripts/test-gate.sh fast`
- Result:
  - `failed`
  - `result = Failed`
  - `passedTests = 63`
  - `failedTests = 1`
- Failure:
  - `RunTests/noDirectRunConstruction()`
  - failure text: `Caught error: lookbehind is not currently supported`
- Result artifact:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/fast-20260325-115350.xcresult`
- Meaning:
  - current-round proof is blocked by a real repository test failure, not by environment/package resolution

### E-P009-007 — Fresh current workspace-state check

- Commands:
  - `git rev-parse HEAD`
  - file mtime/MD5 for the proposal source
- Result:
  - repository SHA: `3459c9471b16d3ff776c33ddf1829e117279fd87`
  - proposal mtime: `2026-03-25 11:50:33 +0200`
  - proposal MD5: `6f4f7127bfd0cf24aa60874232fdad7f`
- Meaning:
  - this evidence pack is anchored to the current local proposal revision and current HEAD

## Attempted But Missing

- No green current-round `fast` proof exists because the fresh run failed in `RunTests/noDirectRunConstruction()`.
- No platform UI screenshots or XCUITest attachments were collected for Proposal 009 because this slice does not define a direct user-facing app flow.
- No `.xctestplan` implementation artifact exists in the repo yet; Proposal 009 is still at draft-review stage, not implementation-review stage.

## Evidence Gate Assessment

- repo/code inspection: `met`
- build/run attempt logged: `met`
- proposal-local reread against current draft: `met`
- completed evidence pack with IDs: `met`
- green current-round proof gate: `not met`

Why this remains `Partial`:
- the proposal text now reads cleanly
- but the current repository proof gate is still not green
- and Proposal 009 is an internal tooling slice with no direct runtime UI flow to sign off visually
