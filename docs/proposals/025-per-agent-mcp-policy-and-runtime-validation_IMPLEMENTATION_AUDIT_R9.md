# Proposal 025: Per-Agent MCP Policy and Runtime Validation Multi-Lens Audit R9

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
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

`P025` is no longer blocked by the stale `preferredExampleURLPrefersRepositoryCopy()` basis or by the earlier shell/password noise. The fresh same-tree canonical gate now executes live Proposal 025 assertions, and most of the MCP contract slice passes. The audit is still red because two proposal-owned portability tests fail on the current tree with file-access errors against workstation-specific source paths:

- `Proposal025Tests.swift:100` `Portability-sensitive runtime sources avoid workstation-specific absolute paths`
- `Proposal025Tests.swift:234` `Repo-backed seed surfaces avoid cwd-derived repository roots`

Both failures surface `NSCocoaErrorDomain Code=257` (`Operation not permitted`) while opening repo source files by absolute path. That keeps `P025` at `Partial` / `Not Ready`: the MCP truth/report path is largely implemented, but the portability and repo-root proof promised by the proposal is still not clean on the current tree.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Proposal-owned portability proof remains red on current same-tree execution | `High` |
| Architecture | `Acceptable` | Requested / predicted / actual / denied MCP truth remains correctly split in code | `High` |
| Product | `At Risk` | Repo-backed portability guarantees are not yet actually proven | `High` |
| UI | `Acceptable` | Shell-owned report/comparison readers still carry MCP truth | `Medium` |
| UX | `Acceptable` | The runtime truth model remains inspectable and legible | `Medium` |
| Readiness | `Not Ready` | Canonical `proposal-025` proof slice is red before any success roll-up is eligible | `High` |

## Fresh Basis Delta vs R8

- The old shared `Proposal026Tests.swift` compile blocker is no longer the active reason this audit is red.
- The old `preferredExampleURL` failure basis is also closed; that test now passes in the focused MCP slice.
- The live blocker is now narrower and proposal-owned: portability-sensitive source lookup still relies on direct repo file access that is not safe under the current runtime constraints.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 3 |
| Partially Implemented | 3 |
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
- Status: `Partially Implemented`
- Evidence:
  - result bundle:
    `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260407-142207.xcresult`
  - failing test:
    `Chainworks ForgeTests/Proposal025Tests.swift:100`
  - failure:
    `Error Domain=NSCocoaErrorDomain Code=257 "(null)" ... Operation not permitted`
  - blocked file:
    `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Support/PreviewSupport.swift`
- Gap / Note: The proof still depends on source-root file access that is not reliably portable under the current execution constraints.

### REQ-005 Repo-backed seed surfaces avoid cwd-derived repository roots
- Status: `Partially Implemented`
- Evidence:
  - same result bundle above
  - failing test:
    `Chainworks ForgeTests/Proposal025Tests.swift:234`
  - failure:
    `Error Domain=NSCocoaErrorDomain Code=257 "(null)" ... Operation not permitted`
  - blocked file:
    `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Chainworks_ForgeApp.swift`
- Gap / Note: The repo-root proof still reaches directly into a workstation path instead of using a portability-safe owner path.

### REQ-006 Canonical proposal-owned proof lane passes on the same tree
- Status: `Partially Implemented`
- Evidence:
  - local invocation:
    `./scripts/test-gate.sh proposal-025`
  - result bundle:
    `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260407-142207.xcresult`
  - roll-up:
    `36` tests, `2` suites, `2` issues
- Gap / Note: The gate is alive and mostly green, but the two proposal-owned portability assertions still fail.

## Product Review

**Summary:** `At Risk`

### PROD-001 Portability proof is still coupled to workstation-specific source access
- Severity: `Major`
- Confidence: `High`
- Evidence:
  - failing tests above
  - source files above
- Why It Matters: The proposal specifically tightened runtime and report truth around portability-safe MCP policy behavior. Direct dependence on local source paths undermines that proof.
- Recommended Action: Move the proof inputs onto a repo-backed or bundled owner path that does not require direct TCC-sensitive source-file reads.

## Readiness Review

**Summary:** `Not Ready`

This audit does not even reach the full-regression question. The canonical same-tree `proposal-025` slice itself is still red on live proposal-owned assertions, so successful roll-up is not yet allowed.

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q 'docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md'`
- `./scripts/test-gate.sh proposal-025`

## Recommended Next Actions

1. Remove the direct workstation-path dependency from the two failing portability tests or from the implementation path they exercise.
2. Rerun `./scripts/test-gate.sh proposal-025`.
3. Only after the focused MCP slice is green, consider full-regression roll-up.
