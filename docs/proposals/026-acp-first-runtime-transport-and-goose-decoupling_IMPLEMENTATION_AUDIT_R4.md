# Proposal 026: ACP-First Runtime Transport And Goose Decoupling Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md` |
| Proposal MD5 | `f1f9889b9d3521a8cc688a64f769fc3c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `9390eb0` |
| Working Tree | `Dirty (pre-existing implementation edits before audit report write)` |
| Audited At | `2026-04-07T19:00:52+0300` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P026` has closed several previously live blockers on the current tree:

- the canonical `proposal-026` lane exists in `test-gate.sh`
- the focused same-tree gate passes cleanly
- the runtime-settlement fields now persist and surface in the run/report path

Fresh focused proof:

- local `./scripts/test-gate.sh proposal-026`
- bundle:
  `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-185308.xcresult`
- summary:
  `69` passed, `0` failed

`P026` still lands `Not Implemented` because its strongest proposal-owned requirement remains missing: there is still no executed same-tree ACP-backed proof showing one canonical proposal loop and one implementation path completing without downgrading Forge’s canonical truth layers. The current repo has ACP runtime profiles and transport tests, but the available canonical proposal-loop evidence is still fixture-Goose or compile-shape oriented rather than live ACP-backed end-to-end proof.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Not Implemented` | The promised ACP-backed canonical flow proof still does not exist | `High` |
| Architecture | `Acceptable` | Transport-neutral runtime settlement and adapter-family persistence now look coherent | `High` |
| Product | `At Risk` | First-wave ACP value is still unproven in the exact user flows the proposal promised | `High` |
| UI | `Acceptable` | No fresh contradiction surfaced against shell-owned report / recovery readers | `Medium` |
| UX | `Acceptable` | Requested / predicted / actual runtime truth remains legible in reports and comparison | `Medium` |
| Readiness | `Not Ready` | A proposal-owned requirement is still missing, so successful roll-up is unavailable even before full regression | `High` |

## Fresh Basis Delta vs R3

- The old `ProviderPlatform` fixture-access blocker is closed on the current tree.
- The canonical `proposal-026` gate is now green at `69/69`.
- The remaining blocker is narrower and more meaningful: the proposal still lacks executed ACP-backed proof for the canonical proposal loop plus implementation path commitment.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 7 |
| Partially Implemented | 0 |
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
- Status: `Implemented`
- Evidence:
  - focused same-tree bundle above
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`

### REQ-005 Goose still works as the default runtime path after seam extraction
- Status: `Implemented`
- Evidence:
  - focused same-tree bundle above
  - `Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift`

### REQ-006 At least two ACP runtimes can be selected through backend/runtime profiles
- Status: `Implemented`
- Evidence:
  - `examples/agents/agents.yaml`
  - `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`

### REQ-007 Canonical proposal-owned focused gate exists and exercises the current seam
- Status: `Implemented`
- Evidence:
  - `scripts/test-gate.sh`
  - result bundle:
    `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-185308.xcresult`
  - roll-up:
    `69` tests, `0` failures

### REQ-008 ACP-backed runs complete one canonical proposal loop and one implementation path without downgrading canonical truth
- Status: `Missing`
- Evidence:
  - search basis:
    `rg -n "proposal loop|implementation path|claude_agent_acp|gemini_cli_acp|acp_native|proposal-026" examples docs/evidence 'Chainworks ForgeTests' 'Chainworks Forge' scripts/test-gate.sh`
  - ACP runtime profiles are present in:
    `examples/agents/agents.yaml`
  - canonical live proposal-loop fixtures remain documented in:
    `examples/workflows/proposal-loop-live.yaml`
  - live workflow tests still cover compile / shape or fixture-Goose paths in:
    `Chainworks ForgeTests/LiveProposalWorkflowTests.swift`
    `Chainworks ForgeTests/EndToEndTests.swift`
- Gap / Note: No executed same-tree proof artifact was found for the promised ACP-backed canonical proposal loop plus implementation path.

## Product Review

**Summary:** `At Risk`

### PROD-001 First-wave ACP value is still not proven in the canonical flows the proposal promised
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / REQs:
  - `REQ-008`
- Evidence:
  - missing ACP-backed proof above
  - focused same-tree `proposal-026` bundle above
- Why It Matters: `P026` is not only a transport refactor. It promised that ACP-backed runtimes would successfully carry a real proposal loop and a real implementation path without losing canonical Forge truth.
- Recommended Action: Add and execute a same-tree ACP-backed proof lane for one canonical proposal loop and one implementation path.

## Readiness Review

**Summary:** `Not Ready`

This audit is blocked before successful-roll-up questions. A proposal-owned requirement is still missing, so `P026` remains below the success bar even though the focused seam gate is now green.

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q 'docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md'`
- `./scripts/test-gate.sh proposal-026`
- `xcrun xcresulttool get test-results summary --path /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-185308.xcresult`
- `rg -n "proposal loop|implementation path|claude_agent_acp|gemini_cli_acp|acp_native|proposal-026" examples docs/evidence 'Chainworks ForgeTests' 'Chainworks Forge' scripts/test-gate.sh`

## Recommended Next Actions

1. Add a real ACP-backed proof for one canonical proposal loop and one implementation path.
2. Execute that proof on the same tree and preserve the resulting artifact/report evidence.
3. Rerun the `proposal-026` audit after the end-to-end ACP proof exists.
