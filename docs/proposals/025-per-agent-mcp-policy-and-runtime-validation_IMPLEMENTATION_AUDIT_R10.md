# Proposal 025: Per-Agent MCP Policy and Runtime Validation Multi-Lens Audit R10

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
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

`P025` is no longer failing on proposal-owned MCP assertions. The fresh same-tree canonical gate passes cleanly:

- local `./scripts/test-gate.sh proposal-025`
- bundle:
  `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260407-185308.xcresult`
- summary:
  `36` passed, `0` failed

That closes the old portability-specific `Proposal025Tests` failures. The audit still cannot roll up to success because the updated audit skill requires passing same-tree full regression for any successful verdict, and the synced approved-host `full` run is red on this exact tree:

- command:
  `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh full'`
- live roll-up emitted by the run:
  `Test run with 557 tests in 49 suites failed after 37.454 seconds with 20 issues`
- current bundle directory:
  `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/full-20260407-185603.xcresult`

So proposal-owned conformance is complete, but the audit must fail closed to `Partial` / `Not Ready` on readiness.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | All proposal-owned requirements are implemented, but successful roll-up is blocked by same-tree full regression | `High` |
| Architecture | `Acceptable` | Requested / predicted / actual / denied MCP truth remains correctly split in code | `High` |
| Product | `Acceptable` | The focused MCP contract is now fully proven on the current tree | `High` |
| UI | `Acceptable` | Shell-owned report / comparison readers surface MCP truth coherently | `Medium` |
| UX | `Acceptable` | Runtime inspection and post-run comparison remain legible | `Medium` |
| Readiness | `Not Ready` | Same-tree full regression is red, which blocks any successful verdict under the current audit policy | `High` |

## Fresh Basis Delta vs R9

- The old portability-sensitive Proposal 025 failures are closed on the current tree.
- The canonical same-tree `proposal-025` lane is green.
- The remaining blocker is not proposal-owned conformance. It is the audit policy’s fail-closed full-regression requirement.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 0 |
| Missing | 0 |

## Requirement Audit

### REQ-001 Per-agent `mcp_profile` intent remains explicit and runtime-authoritative
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`

### REQ-002 Requested, predicted, actual, and denied MCP truth stay separately inspectable
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`

### REQ-003 Goose reconciliation and MCP telemetry remain wired into the canonical runtime/report path
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift`

### REQ-004 Portability-sensitive runtime sources avoid workstation-specific absolute paths
- Status: `Implemented`
- Evidence:
  - focused same-tree bundle above
  - `Chainworks ForgeTests/Proposal025Tests.swift`
  - the previously red portability assertions now pass in live execution on the current tree

### REQ-005 Repo-backed seed surfaces avoid cwd-derived repository roots
- Status: `Implemented`
- Evidence:
  - focused same-tree bundle above
  - `Chainworks ForgeTests/Proposal025Tests.swift`
  - the previously red repo-root assertion now passes in live execution on the current tree

### REQ-006 Canonical proposal-owned proof lane passes on the same tree
- Status: `Implemented`
- Evidence:
  - local invocation:
    `./scripts/test-gate.sh proposal-025`
  - result bundle:
    `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260407-185308.xcresult`
  - roll-up:
    `36` tests, `0` failures

## Readiness Review

**Summary:** `Not Ready`

Under the current audit skill, a successful verdict requires passing same-tree full regression. That bar is not met here. The synced approved-host `full` run is red on the exact audited tree and emitted a failing roll-up before the bundle finalized cleanly.

### READY-001 Same-tree full regression is red, so success must fail closed
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs:
  - `REQ-006`
- Evidence:
  - remote invocation above
  - emitted roll-up:
    `Test run with 557 tests in 49 suites failed after 37.454 seconds with 20 issues`
  - partial bundle directory:
    `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/full-20260407-185603.xcresult`
- Why It Matters: The proposal-owned MCP work is green, but the audit policy explicitly forbids successful roll-up while the same-tree canonical full gate is red.
- Recommended Action: Fix the broader full-regression failures on the synced approved-host tree, then rerun `full`.

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q 'docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md'`
- `./scripts/test-gate.sh proposal-025`
- `xcrun xcresulttool get test-results summary --path /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260407-185308.xcresult`
- `ssh -o BatchMode=yes test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh full'`

## Recommended Next Actions

1. Fix the broader same-tree approved-host `full` failures.
2. Rerun `full` on the synced approved-host tree.
3. If full turns green without reopening the focused MCP slice, `P025` can roll up from `Partial` / `Not Ready` to a successful audit verdict.
