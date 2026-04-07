# Proposal 026: ACP-First Runtime Transport And Goose Decoupling Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md` |
| Proposal MD5 | `f1f9889b9d3521a8cc688a64f769fc3c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `d8ccf4b` |
| Working Tree | `Dirty` (`scripts/test-gate.sh`, `Chainworks ForgeTests/Proposal025Tests.swift`, plus 3 untracked prior audit reports) |
| Audited At | `2026-04-07T11:44:52+0300` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P026` is materially underway on the current tree. The canonical core seam is no longer Goose-shaped: `RuntimeTransportProtocol` exists, Goose now conforms as a compatibility adapter, catalog/backend resolution freezes `runtimeProfileID`, `ExecutionService` selects `ClaudeAgentACPTransport` and `GeminiCLIACPTransport`, and fresh same-tree proof is green for the current transport layer plus run-start freeze. The audit still lands `Not Implemented` because the requested-vs-predicted-vs-actual runtime split is only partially persisted on `AgentExecution`, and the proposal’s strongest proof bar is still missing: there is no same-tree evidence that an ACP-backed runtime completes one canonical proposal loop and one implementation path without downgrading canonical execution/report/MCP truth. `scripts/test-gate.sh` also still has no canonical `proposal-026` lane, so readiness remains `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Not Implemented` | ACP-backed end-to-end proof required by the proposal is still absent | `High` |
| Architecture | `At Risk` | Actual runtime settlement is under-modeled on `AgentExecution` and downstream readers | `High` |
| Product | `At Risk` | First-wave ACP value is still unproven in a canonical proposal/implementation flow | `High` |
| UI | `Acceptable` | Shell-owned report/recovery readers still remain the intended operator spine | `Medium` |
| UX | `Acceptable` | No fresh operator-flow contradiction surfaced beyond missing ACP proof | `Medium` |
| Readiness | `Not Ready` | No canonical `proposal-026` gate or ACP-backed proof lane exists on the current tree | `High` |

## Proposal Contract

### Scope

- Introduce an ACP-shaped canonical runtime vocabulary in core execution code.
- Replace Goose-shaped transport abstractions in core while preserving Goose as the first-wave default runtime.
- Add catalog-owned `runtime_profiles` and backend-profile runtime binding.
- Keep Forge-owned execution truth, report truth, recovery truth, and MCP truth canonical.
- Add first-wave ACP runtime selection for Claude Agent ACP and Gemini CLI ACP.

### Locked Decisions

- ACP is canonical at the runtime-vocabulary layer; Forge remains canonical at the product-semantics layer.
- `runtime_profile` is repo-owned runtime intent, not machine-local launch authority.
- Backend profiles freeze provider/model/effort/structured-output posture plus runtime profile.
- Goose remains the default runtime path in the first wave.
- Requested / predicted / actual runtime and MCP truth must remain split across catalog selection, preflight, `RunStartSnapshot`, `AgentExecution`, and shell-owned readers.

### Primary User Flows

1. Start a run whose backend profile freezes a runtime profile into run-start truth.
2. Execute through a transport-neutral core seam while keeping Goose functional as the default path.
3. Select a first-wave ACP runtime through backend/runtime-profile truth.
4. Inspect persisted execution, report, comparison, and recovery truth without a parallel diagnostics lane.
5. Complete at least one canonical proposal loop and one implementation path on ACP-backed runtimes.

### UI Commitments

- Shell-owned persisted-truth readers remain authoritative:
  - `RunReportView`
  - `RunComparisonView`
  - `RecoverySheet`
  - `BlockedRunRecoveryView`

### UX Commitments

- Operators can trust requested-vs-predicted-vs-actual runtime/MCP truth splits.
- ACP migration must not hide or reconstruct truth from adapter heuristics.
- First-wave Goose behavior must not be intentionally weakened by bridge concessions.

### Acceptance Criteria

1. Core orchestration code does not import Goose transport types or Goose endpoint semantics.
2. The canonical runtime abstraction in core is ACP-shaped.
3. MCP intent in core is not expressed in Goose extension IDs.
4. `RunStartSnapshot` and `AgentExecution` persist transport-neutral truth.
5. Runtime selection exists through catalog/runtime-profile and backend-profile truth.
6. Goose still works as the default runtime path after seam extraction.
7. At least two ACP runtimes can be selected through backend profiles:
   - Claude Agent ACP
   - Gemini CLI ACP
8. ACP-backed runs complete at least one canonical proposal loop and one implementation path without downgrading canonical execution truth, report truth, or MCP truth.

### Test / Evidence Requirements

- Phase 3 proof must show run snapshot freezing of runtime profile selection.
- Reports and recovery must stay grounded in persisted Forge truth through the current shell-owned readers.
- ACP-backed runs must prove at least one canonical proposal loop and one implementation path.
- A successful audit roll-up would additionally require same-tree full regression, but this pass does not reach that bar because live proposal-owned gaps remain.

### Explicit Exclusions

- No Forge-owned replacement protocol for ACP.
- No forced first-wave cutover away from Goose.
- No machine-local launch authority in repo-owned `runtime_profiles`.
- No parallel runtime-diagnostics truth lane outside persisted shell-owned readers.

## Proposal Fidelity / Divergence

### Matches

- Core runtime vocabulary is ACP-shaped in `RuntimeTransportProtocol`.
- Goose is now a compatibility adapter under the canonical transport contract.
- Backend/runtime-profile resolution and run-start freezing exist on the current tree.
- Claude Agent ACP and Gemini CLI ACP transports are present and selectable in execution code.
- Fresh same-tree proof is green for the current transport seam and for run-start frozen provider binding snapshot creation.

### Divergences

- `AgentExecution` still does not persist actual `runtimeProfileID`, `adapterFamily`, or `capabilityClass` for a concrete attempt.
- Shell-owned report/comparison payloads do not expose runtime-profile / adapter-family / capability-class truth for executed attempts.
- There is still no canonical `proposal-026` gate or ACP-backed proof lane in `scripts/test-gate.sh`.
- No same-tree test or gate proves that Claude Agent ACP or Gemini CLI ACP complete a canonical proposal loop and an implementation path.

### Ambiguities / Evidence Gaps

- The current repo contains the first-wave ACP adapters, but no executed same-tree artifact proves them in end-to-end proposal/implementation flows.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 1 |
| Missing | 1 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Core orchestration no longer imports Goose transport types or endpoint semantics
- Proposal Source: `§3`, `§4.5`, `§5.1`, `§9.1`, `§11(1)`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - focused result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-audit-tests-20260407-114041.xcresult`
- Gap / Note: Core transport vocabulary now flows through `RuntimeTransportProtocol`, and the fresh same-tree transport slice is green.

### REQ-002 The canonical runtime abstraction in core is ACP-shaped
- Proposal Source: `§5.1`, `§6.1`, `§7.1`, `§11(2)`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/ACPStreamEventMapper.swift`
  - `Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift`
  - focused result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-audit-tests-20260407-114041.xcresult`
- Gap / Note: The live transport seam uses session lifecycle, prompt submission, stream events, close, and runtime-state reads in ACP-shaped terms.

### REQ-003 MCP intent in core is not expressed as Goose extension IDs
- Proposal Source: `§4.7`, `§8.1-§8.2`, `§11(3)`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift`
  - focused result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-audit-tests-20260407-114041.xcresult`
- Gap / Note: Core code carries requested extensions and runtime-settled MCP truth without reintroducing Goose naming as canonical intent.

### REQ-004 `RunStartSnapshot` and `AgentExecution` persist transport-neutral truth
- Proposal Source: `§5.5`, `§8.1`, `§9.3 proof focus`, `§11(4)`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift`
  - focused result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-binding-test-20260407-114254.xcresult`
- Gap / Note: `RunStartSnapshot` does freeze provider/runtime binding via `providerBindingSnapshotJSON`, and fresh proof confirms run-start snapshot creation. `AgentExecution` does persist runtime session ID, runtime provider/model, and actual MCP settlement, but it still does not persist actual `runtimeProfileID`, `adapterFamily`, or `capabilityClass`, and shell-owned report/comparison payloads do not expose those actual runtime-settlement fields.

### REQ-005 Runtime selection exists through catalog/runtime-profile and backend-profile truth
- Proposal Source: `§4.6`, `§5.2-§5.3`, `§9.3`, `§11(5)`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/Engine/RunPlan.swift`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift`
  - focused result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-binding-test-20260407-114254.xcresult`
- Gap / Note: Runtime profile identity is frozen through catalog/backend truth rather than ad hoc per-agent transport settings.

### REQ-006 Goose still works as the default runtime path after seam extraction
- Proposal Source: `§5.4`, `§9.2`, `§10.2`, `§11(6)`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/GooseAdapter/GooseTransport.swift`
  - `Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift`
  - focused result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-audit-tests-20260407-114041.xcresult`
- Gap / Note: Resolver and execution defaults still fall back to Goose, and the fresh transport slice stays green.

### REQ-007 At least two ACP runtimes can be selected through backend/runtime profiles
- Proposal Source: `§5.6`, `§9.3`, `§11(7)`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
- Gap / Note: Selection logic for `claude_agent_acp` and `gemini_cli_acp` exists in the current execution path. The missing gap is not selection wiring; it is end-to-end proof.

### REQ-008 ACP-backed runs complete one canonical proposal loop and one implementation path without downgrading canonical execution/report/MCP truth
- Proposal Source: `§9.3`, `§10.2`, `§11(8)`
- Status: `Missing`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `scripts/test-gate.sh`
  - search result with no ACP proof hits in tests/gates:
    `rg -n 'claude_agent_acp|gemini_cli_acp|ClaudeAgentACPTransport|GeminiCLIACPTransport|ACPStreamEventMapper|runtime_profiles|runtime_profile' 'Chainworks ForgeTests' examples docs/reference docs/evidence scripts/test-gate.sh`
  - current execution code only:
    `Chainworks Forge/Engine/ExecutionService.swift`
    `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
    `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`
- Gap / Note: The repo contains the first-wave ACP adapters, but there is still no same-tree proof lane showing a canonical proposal loop and an implementation path completing on ACP-backed runtimes while preserving persisted execution/report/MCP truth.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 Actual runtime settlement remains under-modeled below the requested/predicted/actual split
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-004`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
- Why It Matters: The proposal explicitly says `AgentExecution` owns actual runtime settlement. Today the concrete attempt row stores runtime session/provider/model plus MCP settlement, but not actual runtime profile identity, adapter family, or capability class. That leaves the requested-vs-predicted-vs-actual runtime split incomplete and makes downstream readers weaker than the contract.
- Recommended Action: Persist actual runtime profile / adapter-family / capability-class settlement on `AgentExecution` and thread that truth into report/comparison payloads.

## Product Review

**Summary:** `At Risk`

### PROD-001 First-wave ACP value is still unproven in the product’s canonical end-to-end flows
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `scripts/test-gate.sh`
  - no ACP-backed proposal or implementation proof hits in `Chainworks ForgeTests`
  - current ACP adapters only:
    `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
    `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`
- Why It Matters: The proposal’s user-visible promise is not just “adapter code exists.” It is that ACP-backed runtimes can carry at least one canonical proposal loop and one implementation path without downgrading truth. That remains unproven on the current tree.
- Recommended Action: Add an executed proposal-scoped ACP proof lane that runs one canonical proposal loop and one implementation path through the first-wave ACP runtimes.

## UI Review

**Summary:** `Acceptable`

No fresh UI-specific contradiction surfaced. The shell-owned operator spine named in the proposal still exists in the current app:

- `Chainworks Forge/Views/RunReportView.swift`
- `Chainworks Forge/Views/RunComparisonView.swift`
- `Chainworks Forge/Views/RecoverySheet.swift`
- `Chainworks Forge/Views/BlockedRunRecoveryView.swift`

## UX Review

**Summary:** `Acceptable`

No fresh UX contradiction surfaced beyond the missing ACP proof. The current issue is evidence and settlement completeness, not a discovered operator-flow regression.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 There is still no canonical `proposal-026` gate in `scripts/test-gate.sh`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `code`
- Evidence:
  - `scripts/test-gate.sh`
  - current gate list includes `proposal-015`, `proposal-018`, `proposal-019`, `proposal-022`, `proposal-024`, `proposal-025`, and `full`, but no `proposal-026`
- Why It Matters: The proposal claims a first-wave proof bar, but the repo still lacks a reproducible proposal-scoped lane for that proof.
- Recommended Action: Add a canonical `proposal-026` gate that exercises transport-neutral core plus ACP-backed runtime proof.

### READY-002 Successful audit roll-up is blocked before full regression because proposal-owned ACP proof is still missing
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `runtime`, `tests-found`
- Evidence:
  - local green focused proof:
    `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-audit-tests-20260407-114041.xcresult`
    `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-binding-test-20260407-114254.xcresult`
  - no ACP-backed canonical-loop / implementation-path proof lane found on the current tree
- Why It Matters: Under the updated audit skill, same-tree full regression is only required for a successful verdict. This audit never reaches that stage because the proposal still lacks the required ACP-backed proof itself.
- Recommended Action: Land the missing ACP-backed proof first, then rerun the audit and, only if all in-scope REQs are green, run same-tree full regression.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | Fresh same-tree focused runtime slices built and passed. |
| Core user flow runtime-validated | `Partial` | Transport-neutral Goose seam and run-start freeze are validated; ACP-backed end-to-end flows are not. |
| Empty/loading/error states covered | `Not Checked` | Not reassessed in this pass. |
| Accessibility risk acceptable | `Not Checked` | Not reassessed in this pass. |
| Localization risk acceptable | `Not Checked` | Not reassessed in this pass. |
| Critical tests executed | `Pass` | Fresh local result bundles passed `25/25` and `37/37`. |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Not Run` | Not attempted because live proposal-owned conformance gaps already block a successful roll-up. |
| Privacy/permissions/entitlements reviewed | `Not Checked` | Not reassessed in this pass. |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q 'docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md'`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py '/Users/user/Documents/Chainworks Forge/docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md'`
- `rg -n "proposal-026|p026|RuntimeTransport|ClaudeAgentACPTransport|GeminiCLIACPTransport|runtimeProfileID|adapterFamily|capabilityClass" scripts/test-gate.sh 'Chainworks Forge' 'Chainworks ForgeTests'`
- `rg -n 'claude_agent_acp|gemini_cli_acp|ClaudeAgentACPTransport|GeminiCLIACPTransport|ACPStreamEventMapper|runtime_profiles|runtime_profile' 'Chainworks ForgeTests' examples docs/reference docs/evidence scripts/test-gate.sh`
- `xcodebuild -list -project 'Chainworks Forge.xcodeproj'`
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-audit-tests-20260407-114041.xcresult -only-testing:'Chainworks ForgeTests/GooseServerTransportTests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests' -only-testing:'Chainworks ForgeTests/GooseAgentExecutorTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=`
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p026-binding-test-20260407-114254.xcresult -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=`

## Recommended Next Actions

1. Persist actual runtime profile / adapter-family / capability-class settlement on `AgentExecution`, then surface it through report/comparison payloads.
2. Add a canonical `proposal-026` gate to `scripts/test-gate.sh`.
3. Land executed same-tree proof for one ACP-backed proposal loop and one ACP-backed implementation path.
4. After the missing ACP proof is green, rerun the audit and only then escalate to same-tree full regression.
