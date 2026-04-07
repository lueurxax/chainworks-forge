# Proposal 015: Skill Resolution and Runtime Injection Multi-Lens Audit R8

| Field | Value |
|---|---|
| Proposal | `docs/proposals/015-skill-resolution-and-runtime-injection.md` |
| Proposal MD5 | `162789b1c6a3b41439c7e4d6d72b436c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `0e68bb2` |
| Working Tree | `Clean (before audit report write)` |
| Audited At | `2026-04-07T14:30:19+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

The password-gated approved-host lane now runs again, so the old "tests never really started" basis is gone. On the fresh synced same-tree run, `./scripts/test-gate.sh proposal-015` passes the non-UI half and reaches the required UI proof half, but the proposal-owned UI proof still fails on a real assertion:

`Chainworks_ForgeUITests.testProposal015SkillVisibilityProofSurface`

`Proof surface must render the real agent catalog owner surface`

That is materially stricter than the earlier shell/bootstrap/codesign failures. `P015` therefore stays `Partial` / `Not Ready`: the implementation remains broadly present, but the required approved-host UI proof is still red on the current tree.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Required same-tree UI proof still fails on a live Proposal 015 assertion | `High` |
| Architecture | `Acceptable` | Resolver / injector / provenance owners still match the proposal in code | `High` |
| Product | `At Risk` | The user-facing proof surface does not yet show the real catalog-owned truth the proposal promised | `High` |
| UI | `At Risk` | Approved-host proof lane is red in the exact UI surface the proposal owns | `High` |
| UX | `Acceptable` | Shell-owned inspection/report paths remain coherent in code | `Medium` |
| Readiness | `Not Ready` | Proposal-owned proof is still red, so no successful roll-up is eligible | `High` |

## Fresh Basis Delta vs R7

- The old shared compile blocker in `Proposal026Tests.swift` is no longer the active reason this audit is red.
- The password/code-sign gating problem is no longer the active reason either; the approved-host split gate now runs into the actual UI assertion set.
- The live blocker is now proposal-owned and product-facing: the UI proof surface does not render the real agent-catalog owner surface expected by `P015`.

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
    `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260407-142236.xcresult`
  - synced approved-host UI bundle:
    `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-ui-20260407-142306.xcresult`
  - failing assertion:
    `Chainworks_ForgeUITests.swift:1244`
    `Proof surface must render the real agent catalog owner surface`
- Gap / Note: The gate now reaches the proof surface, but the proof surface itself is not yet correct.

### REQ-006 Canonical proposal-owned split gate passes on the same tree
- Status: `Partially Implemented`
- Evidence:
  - remote invocation:
    `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh proposal-015'`
  - non-UI half passes and advances into the UI half on the synced same tree
  - the overall gate remains red because the required UI proof assertion fails

## Product Review

**Summary:** `At Risk`

### PROD-001 The required UI proof now fails on a real product assertion
- Severity: `Major`
- Confidence: `High`
- Evidence:
  - UI bundle above
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1244`
- Why It Matters: The proposal promised that execution-time skill truth would be visible through a real operator-facing surface. The current proof says the shown surface is still not the real catalog-owned one.
- Recommended Action: Fix the proof surface wiring so it renders the real agent catalog owner surface, then rerun the approved-host split gate.

## Readiness Review

**Summary:** `Not Ready`

The audit skill only allows successful verdicts after live proposal-owned proof is green. That bar is not met here: the fresh approved-host split gate now runs correctly, but the required UI proof still fails on the synced same tree.

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q 'docs/proposals/015-skill-resolution-and-runtime-injection.md'`
- `ssh -o BatchMode=yes test@SMacBook.local 'cd /Users/test/chainworks-remote && git rev-parse --short HEAD && git status --short'`
- `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh proposal-015'`

## Recommended Next Actions

1. Fix the proof-surface wiring behind `testProposal015SkillVisibilityProofSurface`.
2. Rerun the synced approved-host `proposal-015` split gate.
3. Only after the proposal-owned UI proof is green, consider full-regression roll-up.
