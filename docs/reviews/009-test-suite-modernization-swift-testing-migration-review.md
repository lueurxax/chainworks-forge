# Proposal 009: Test Suite Modernization — Swift Testing Migration, Parameterization, and Infrastructure Upgrade Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/009-test-suite-modernization-swift-testing-migration.md` |
| Repository Root | `.` |
| Git SHA | `25cb0b29ed52b7f4967a8edab9382551f78efb49` |
| Reviewed At | `2026-03-25T14:29:54+0200` |
| Proposal Source MD5 | `6f4f7127bfd0cf24aa60874232fdad7f` |
| Proposal Source MTime | `2026-03-25 11:50:33 +0200` |
| Review Mode | `full-review` |
| Overall Status | `Evidence Gap Review` |
| Readiness | `Yellow` |
| Confidence | `High` |
| Evidence Completeness | `Partial` |

## Verdict

Current Proposal 009 rereads cleanly as a draft. This was a no-delta repeat round on the proposal text: the current source MD5 still matches the version reviewed in the previous pass. The three live proposal-text gaps from the earlier round remain closed:

- the migration inventory is now re-baselined on `17` executable XCTest unit files, `1` executable Swift Testing unit file, and `2` helper files;
- the CI/tagging path is now Xcode-native, using `.xctestplan` files and `xcodebuild -testPlan` instead of a nonexistent SwiftPM path;
- the shared Goose transport strategy now explicitly separates lightweight stubs from observation-heavy observable mocks.

No new proposal-text findings surfaced in this reread.

This is still not a full sign-off. I refreshed the impacted proof slice on the new current HEAD, and the fresh `./scripts/test-gate.sh fast` run reproduced the same baseline failure. The blocker remains `RunTests/noDirectRunConstruction()` (`Caught error: lookbehind is not currently supported`). Proposal 009 also has no direct operator-facing app flow, so this review is necessarily anchored to repo/test evidence rather than runtime UI proof.

## Findings

No live proposal-text findings surfaced in the current draft.

## Evidence Summary

- `E-P009-001`: Fresh reread of the current Proposal 009 draft
  - inventory/count references are internally consistent
  - migration scope is explicitly `17` executable XCTest unit files
  - helper files are no longer counted as executable migration targets
- `E-P009-001A`: Repeat-round freshness check
  - current proposal MD5 matches the last reviewed revision
  - the proposal-text verdict is therefore unchanged
- `E-P009-002`: Fresh current-head inventory baseline
  - `17` executable XCTest unit files
  - `1` executable Swift Testing unit file
  - `2` helper files
  - current repo inventory matches the draft's updated framing
- `E-P009-003`: Fresh current-head mock/test baseline
  - current Goose transport tests still assert on request/session/close side effects
  - the proposal's new two-lane `StubGooseTransport` / `ObservableGooseTransport` split now matches those observation requirements
- `E-P009-004`: Fresh external primary-source confirmation from Apple
  - Swift Testing applies to Xcode projects, not only SwiftPM
  - WWDC24 `Go further with Swift Testing` explicitly shows saving tag preferences into Xcode Test Plans and filtering plans by tags
- `E-P009-005`: Fresh `./scripts/test-gate.sh fast` run
  - build phase succeeded
  - test phase failed with `1` failing test, `63` passing tests
  - failing test: `RunTests/noDirectRunConstruction()`
  - failure text: `Caught error: lookbehind is not currently supported`
- `E-P009-006`: Fresh current workspace-state check
  - review is anchored to current local draft and current HEAD

## Missing Evidence

- No green current-round proof exists for the repo's `fast` gate because the fresh run failed in `RunTests/noDirectRunConstruction()`.
- No proposal-specific runtime UI evidence exists:
  - Proposal 009 is an internal test-suite modernization slice
  - it does not define a direct app-facing operator flow to capture as screenshots

## What Can Still Be Said With Partial Confidence

- Proposal 009 is now textually handoff-ready.
- The migration boundary is clearer than in the prior round:
  - executable XCTest files vs already-migrated Swift Testing vs helper files
  - Xcode Test Plans vs nonexistent SwiftPM execution path
  - stateless vs observation-heavy Goose transport mocks
- The remaining blocker is current repo proof, not proposal design.

## What Is Required To Finish The Full Review

- Fix the current `fast` gate failure in `RunTests/noDirectRunConstruction()`.
- Re-run the review with a green current-round proof gate.
- If implementation work starts, attach any concrete `.xctestplan` and `test-gate.sh` migration evidence in a future round.
