# Proposal 027: Unified Read-Only JSON And Markdown Rendering Multi-Lens Audit R6

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-unified-read-only-json-and-markdown-rendering.md` |
| Proposal MD5 | `829a3ab472c3cb6b95a509870d0df882` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `0e68bb2` |
| Working Tree | `Clean (before audit report write)` |
| Audited At | `2026-04-07T14:30:19+0300` |
| Overall Conformance | `Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P027` is the one proposal in this batch whose focused same-tree proof is now green. The canonical renderer slice passed `17/17` in:

`/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-027-20260407-141115.xcresult`

That closes the earlier missing-gate and shared-compile-drift basis. On current evidence, the proposal-owned renderer contract is implemented. The audit still cannot roll up to success because the current audit skill requires passing same-tree full regression for any successful verdict, and the latest completed approved-host `full` run on the same synced tree is still red:

`/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/full-20260407-141601.xcresult`

That full run failed with live issues in broader repo regression, including:

- `OrchestratorTests.swift:1361` (`Helloworld` vs `Hello world`)
- remote UI automation timeout while enabling automation mode

So the correct roll-up is `Implemented` / `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Implemented` | Proposal-owned renderer slice is now green on the current tree | `High` |
| Architecture | `Acceptable` | Shared renderer ownership remains coherent and centralized | `High` |
| Product | `Acceptable` | No live renderer-specific contradiction remains | `High` |
| UI | `Acceptable` | Artifact/report/comparison surfaces all inherit the shared renderer path | `Medium` |
| UX | `Acceptable` | Local-only fail-closed rendering model remains intact | `Medium` |
| Readiness | `Not Ready` | Same-tree full regression is still red, which blocks any successful audit roll-up | `High` |

## Fresh Basis Delta vs R5

- The old shared `Proposal026Tests.swift` compile-drift blocker is closed for the focused renderer slice.
- The canonical `proposal-027` lane is now real and green on the current tree.
- The remaining blocker is no longer proposal-owned conformance; it is audit-readiness only, because full same-tree regression is still red.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 7 |
| Partially Implemented | 0 |
| Missing | 0 |

## Requirement Audit

### REQ-001 Shared read-only renderer foundation exists and is used by primary artifact/report surfaces
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`

### REQ-002 Artifact-backed rendering follows canonical format truth and structured JSON rescue behavior
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`

### REQ-003 Markdown uses the required AppKit/TextKit-backed document surface
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - focused same-tree proof bundle above

### REQ-004 JSON renders as a collapsible tree with deterministic fallback behavior
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - focused same-tree proof bundle above

### REQ-005 Markdown image handling is local-only and fail-closed
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - focused same-tree proof bundle above

### REQ-006 Rendering remains read-only and presentation-only
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - focused same-tree proof bundle above

### REQ-007 Canonical proposal-owned `proposal-027` gate passes on the same tree
- Status: `Implemented`
- Evidence:
  - local invocation:
    `./scripts/test-gate.sh proposal-027`
  - result bundle above
  - roll-up:
    `17` tests, `1` suite, `0` failures

## Readiness Review

**Summary:** `Not Ready`

The current audit skill requires passing same-tree full regression before any successful verdict can be issued. That bar is not met on the current synced tree:

- approved-host parity was rechecked on `SMacBook.local`
- the latest completed same-tree `full` bundle is still red

So `P027` is implemented, but it is not yet release-ready under the audit policy.

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q 'docs/proposals/027-unified-read-only-json-and-markdown-rendering.md'`
- `./scripts/test-gate.sh proposal-027`
- `ssh -o BatchMode=yes test@SMacBook.local 'cd /Users/test/chainworks-remote && git rev-parse --short HEAD && git status --short'`
- `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh full'`

## Recommended Next Actions

1. Fix the broader full-regression failures on the synced approved-host tree.
2. Rerun the same-tree approved-host `full` gate.
3. If full turns green without reopening the focused renderer slice, `P027` can roll up from `Implemented` / `Not Ready` to a successful audit verdict.
