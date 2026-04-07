# Proposal 026: ACP-First Runtime Transport And Goose Decoupling Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md` |
| Proposal MD5 | `f1f9889b9d3521a8cc688a64f769fc3c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `0e68bb2` |
| Working Tree | `Clean (before audit report write)` |
| Audited At | `2026-04-07T14:30:19+0300` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

The password issue is not the blocker for `P026`. The fresh same-tree `proposal-026` gate now builds and runs substantial proof on the current tree: the `Proposal026` slice passes, the `GooseServerTransport` slice passes, and the older actual-runtime-settlement / missing-gate blockers remain closed. The audit still lands `Not Implemented` for two reasons:

1. The focused gate is still red overall because the `ProviderPlatform` slice fails repeatedly while trying to copy `examples/workflows/workflow.yaml` from the repo root, surfacing `NSCocoaErrorDomain Code=513` / POSIX `Operation not permitted`.
2. The proposal’s strongest contract is still missing: there is still no executed same-tree ACP-backed proof of one canonical proposal loop and one implementation path that preserves canonical execution truth, report truth, and MCP truth.

So `P026` remains `Not Implemented` / `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Not Implemented` | ACP-backed end-to-end proof required by the proposal still does not exist | `High` |
| Architecture | `Acceptable` | Core seam, runtime settlement persistence, and Goose decoupling remain coherent in code | `High` |
| Product | `At Risk` | First-wave ACP value is still unproven in the canonical user flows the proposal promised | `High` |
| UI | `Acceptable` | No fresh contradiction surfaced against shell-owned report/recovery readers | `Medium` |
| UX | `Acceptable` | Requested / predicted / actual runtime truth remains legible in code and reports | `Medium` |
| Readiness | `Not Ready` | Canonical focused proof is still red before any success roll-up is eligible | `High` |

## Fresh Basis Delta vs R2

- The old `Proposal026Tests.swift` compile-drift basis is closed.
- The gate now runs real current-tree tests and proves that the `Proposal026` and `GooseServerTransport` slices are alive.
- The live blockers are now narrower and more meaningful: repo-fixture portability in `ProviderPlatform`, plus the still-missing ACP-backed canonical workflow proof.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 1 |
| Missing | 1 |

## Requirement Audit

### REQ-001 Core orchestration no longer imports Goose transport types or endpoint semantics as canonical truth
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift`

### REQ-002 The canonical runtime abstraction in core is ACP-shaped
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/ACPStreamEventMapper.swift`
  - `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`

### REQ-003 `RunStartSnapshot` and `AgentExecution` persist transport-neutral runtime truth
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`

### REQ-004 Runtime selection exists through catalog/runtime-profile and backend-profile truth
- Status: `Partially Implemented`
- Evidence:
  - result bundle:
    `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-142209.xcresult`
  - roll-up:
    `69` tests, `3` suites, `7` issues
  - failing slice:
    `ProviderPlatform`
  - failing fixture path:
    `/Users/user/Documents/Chainworks Forge/examples/workflows/workflow.yaml`
- Gap / Note: The selection path exists and much of it is live, but the focused provider-platform proof is still red because the fixture-copy path is not portable enough for the current execution environment.

### REQ-005 Goose still works as the default runtime path after seam extraction
- Status: `Implemented`
- Evidence:
  - same bundle above
  - `GooseServerTransport` slice passed on the current tree
  - `Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift`

### REQ-006 At least two ACP runtimes can be selected through backend/runtime profiles
- Status: `Implemented`
- Evidence:
  - `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`

### REQ-007 Canonical proposal-owned focused gate exists and exercises the current seam
- Status: `Implemented`
- Evidence:
  - `scripts/test-gate.sh`
  - same result bundle above

### REQ-008 ACP-backed runs complete one canonical proposal loop and one implementation path without downgrading canonical truth
- Status: `Missing`
- Evidence:
  - search basis:
    `rg -n "proposal loop|implementation path|claude_agent_acp|gemini_cli_acp|proposal-026" examples docs/evidence 'Chainworks ForgeTests' scripts/test-gate.sh`
  - no executed same-tree proof artifact was found for the promised ACP-backed canonical flows
- Gap / Note: This remains the key unresolved proposal-owned blocker even after the transport seam and runtime settlement work.

## Product Review

**Summary:** `At Risk`

### PROD-001 First-wave ACP value is still not proven in the canonical user flows
- Severity: `Critical`
- Confidence: `High`
- Evidence:
  - missing ACP-backed canonical workflow proof above
  - current focused gate bundle above
- Why It Matters: `P026` is not just a refactor proposal. It promised that ACP-backed runtimes would carry a real proposal loop and a real implementation path without losing Forge truth. That evidence still does not exist.
- Recommended Action: Add and execute a same-tree ACP-backed proof lane for one canonical proposal loop and one implementation path.

### PROD-002 Provider-platform proof still depends on a repo fixture path that is not execution-safe
- Severity: `Major`
- Confidence: `High`
- Evidence:
  - same bundle above
  - failing fixture path above
- Why It Matters: The runtime-selection story is only partially proven while the provider-platform slice is still red on local fixture-copy behavior.
- Recommended Action: Make the provider-platform proof independent of TCC-sensitive direct reads from the repo example path.

## Readiness Review

**Summary:** `Not Ready`

Even before full regression, the focused same-tree `proposal-026` gate is still red and the proposal’s strongest ACP-backed proof requirement is still missing. That keeps both conformance and readiness below the success bar.

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q 'docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md'`
- `./scripts/test-gate.sh proposal-026`
- `rg -n "proposal loop|implementation path|claude_agent_acp|gemini_cli_acp|proposal-026" examples docs/evidence 'Chainworks ForgeTests' scripts/test-gate.sh`

## Recommended Next Actions

1. Fix the `ProviderPlatform` fixture-access failures around `examples/workflows/workflow.yaml`.
2. Add and execute a real ACP-backed proof for one canonical proposal loop and one implementation path.
3. Rerun `./scripts/test-gate.sh proposal-026` before attempting any broader roll-up.
