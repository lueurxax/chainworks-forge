# Proposal 015: Skill Resolution and Runtime Injection Multi-Lens Audit R9

| Field | Value |
|---|---|
| Proposal | `docs/proposals/015-skill-resolution-and-runtime-injection.md` |
| Proposal MD5 | `162789b1c6a3b41439c7e4d6d72b436c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `9390eb0` |
| Working Tree | `Dirty (pre-existing implementation edits before audit report write)` |
| Audited At | `2026-04-07T19:00:52+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P015` is no longer blocked by password entry, shell bootstrap, or stale compile drift. On the fresh synced approved-host run, the canonical split gate reaches both required halves:

- non-UI proof passes `13/13`
- UI proof launches and fails `0/1`

The live blocker is now the UI automation lane itself:

`The test runner failed to initialize for UI testing. (Underlying Error: Timed out while enabling automation mode.)`

That keeps the proposal at `Partial` / `Not Ready`. The implementation remains broadly present, but the required approved-host UI proof is still red on the current same tree.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Required approved-host UI proof still does not complete on the same tree | `High` |
| Architecture | `Acceptable` | Resolver / injector / provenance owners still line up with proposal intent | `High` |
| Product | `At Risk` | The proposal-owned proof surface remains unproven in live UI execution | `High` |
| UI | `At Risk` | macOS UI automation for the required proof lane still times out before the surface is exercised | `High` |
| UX | `Acceptable` | Shell-owned report / comparison / artifact readers remain coherent in code | `Medium` |
| Readiness | `Not Ready` | Successful roll-up is not eligible while the proposal-owned UI proof lane is red | `High` |

## Fresh Basis Delta vs R8

- The old assertion-level basis from `R8` is no longer the active blocker.
- The synced approved-host gate now reaches the UI runner and produces a real XCTest failure summary instead of stalling on password or code-sign setup.
- The live blocker is narrower but still proposal-owned: UI automation cannot initialize for the required proof lane.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partially Implemented | 2 |
| Missing | 0 |

## Requirement Audit

### REQ-001 Skill references resolve from current builtin, inline, and external contracts
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillResolver.swift`
  - `Chainworks Forge/Engine/Skills/ExternalSkillLoader.swift`
  - synced approved-host canonical gate compiled and ran the current Proposal 015 slice

### REQ-002 Resolved skill content is injected into runtime execution packets
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Engine/Skills/SkillInjector.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`

### REQ-003 Raw and injected skill provenance freezes into immutable run-start truth
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/Run.swift`

### REQ-004 Shell-owned readers expose persisted execution-time skill truth
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`

### REQ-005 Approved-host UI proof surface executes successfully on the same tree
- Status: `Partially Implemented`
- Evidence:
  - synced approved-host non-UI bundle:
    `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260407-185248.xcresult`
  - synced approved-host UI bundle:
    `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260407-185328.xcresult`
  - non-UI summary:
    `13` passed, `0` failed
  - UI summary:
    `0` passed, `1` failed
  - failure text:
    `The test runner failed to initialize for UI testing. (Underlying Error: Timed out while enabling automation mode.)`
- Gap / Note: The proof lane exists and launches, but the required UI run still does not complete successfully.

### REQ-006 Canonical proposal-owned split gate passes on the same tree
- Status: `Partially Implemented`
- Evidence:
  - remote invocation:
    `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh proposal-015'`
  - non-UI half passes and advances into the UI half on the synced same tree
  - the overall gate remains red because the required UI automation lane times out before the proof test executes

## Product Review

**Summary:** `At Risk`

### PROD-001 Required Proposal 015 proof is still blocked by live UI automation failure
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs:
  - `REQ-005`
  - `REQ-006`
- Evidence:
  - same approved-host UI bundle above
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
- Why It Matters: The proposal promised execution-time skill visibility through a live proofable surface. That proof still cannot be completed on the required approved-host lane.
- Recommended Action: Stabilize the approved-host UI automation path for `proposal-015` and rerun the split gate.

## Readiness Review

**Summary:** `Not Ready`

This audit does not reach a successful roll-up. The current audit skill requires the proposal-owned proof lane itself to be green before any broader readiness claim can stand, and `proposal-015` remains red on the synced approved host.

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q 'docs/proposals/015-skill-resolution-and-runtime-injection.md'`
- `ssh -o BatchMode=yes test@SMacBook.local 'cd /Users/test/chainworks-remote && git rev-parse --short HEAD && git status --short'`
- `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh proposal-015'`
- `ssh -o BatchMode=yes test@SMacBook.local 'xcrun xcresulttool get test-results summary --path /var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260407-185248.xcresult'`
- `ssh -o BatchMode=yes test@SMacBook.local 'xcrun xcresulttool get test-results summary --path /var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260407-185328.xcresult'`

## Recommended Next Actions

1. Fix the approved-host UI automation initialization path for the `proposal-015` proof lane.
2. Rerun the synced approved-host `proposal-015` split gate.
3. Only after the UI half is green, consider any broader roll-up.
