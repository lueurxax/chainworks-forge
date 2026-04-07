# Proposal 026: ACP-First Runtime Transport And Goose Decoupling Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md` |
| Proposal MD5 | `f1f9889b9d3521a8cc688a64f769fc3c` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `d8ccf4b` |
| Working Tree | `Dirty` (`Chainworks Forge/Engine/RunComparisonService.swift`, `Chainworks Forge/Engine/RunReportBuilder.swift`, `Chainworks Forge/Engine/WorkflowOrchestrator.swift`, `Chainworks Forge/Models/AgentExecution.swift`, `Chainworks ForgeTests/Proposal025Tests.swift`, `scripts/test-gate.sh`, untracked `Chainworks ForgeTests/Proposal026Tests.swift`, plus prior audit reports) |
| Audited At | `2026-04-07T12:23:20+0300` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P026` is materially further along than `R1`, but it is still not across the proposal finish line. The old persistence blocker is now closed on the current tree: `AgentExecution` persists actual `runtimeProfileID`, `actualAdapterFamily`, and `actualCapabilityClass`, and both run-report and run-comparison payload builders now expose those fields. The old gate-existence blocker is also closed: `scripts/test-gate.sh` now has a canonical `proposal-026` lane. The audit still lands `Not Implemented` because the proposal’s strongest requirement remains missing: there is still no executed same-tree proof that an ACP-backed runtime completes one canonical proposal loop and one implementation path without downgrading execution/report/MCP truth. Readiness stays `Not Ready` because the fresh canonical `proposal-026` gate is red on the current tree: build succeeds, but the targeted test phase cancels after `Proposal026Tests.swift` compile-drift failures.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Not Implemented` | ACP-backed end-to-end proof required by `REQ-008` is still absent | `High` |
| Architecture | `Acceptable` | Core runtime seam and actual-settlement persistence now align, but lack of executed ACP proof still leaves the migration under-exercised | `High` |
| Product | `At Risk` | The proposal promises real first-wave ACP value in canonical flows, and that proof is still missing | `High` |
| UI | `Acceptable` | No fresh contradiction surfaced against the shell-owned report/recovery reader contract | `Medium` |
| UX | `Acceptable` | No fresh operator-flow regression surfaced beyond missing ACP-backed proof | `Medium` |
| Readiness | `Not Ready` | Canonical `proposal-026` gate is now present but currently red on `Proposal026Tests.swift` compile drift | `High` |

## Proposal Contract

### Scope

- Introduce an ACP-shaped canonical runtime vocabulary in core execution code.
- Decouple core orchestration from Goose-shaped transport semantics while keeping Goose as the first-wave default runtime.
- Freeze runtime intent through catalog/runtime-profile and backend-profile truth.
- Keep Forge-owned execution truth, report truth, recovery truth, and MCP truth canonical.
- Prove first-wave ACP runtime selection for Claude Agent ACP and Gemini CLI ACP.

### Locked Decisions

- ACP is the canonical runtime-vocabulary layer; Forge remains canonical at the product-semantics layer.
- `runtime_profile` is repo-owned runtime intent, not machine-local launch authority.
- Requested, predicted, frozen, and actual runtime/MCP truth must remain split across catalog selection, preflight, `RunStartSnapshot`, `AgentExecution`, and shell-owned readers.
- Goose remains the default runtime path in the first wave.

### Primary User Flows

1. Start a run whose backend profile freezes runtime-profile selection into immutable run-start truth.
2. Execute through a transport-neutral core seam while keeping Goose as the default compatibility path.
3. Select Claude Agent ACP or Gemini CLI ACP through backend/runtime-profile truth.
4. Inspect persisted execution/report/comparison/recovery truth without a parallel diagnostics lane.
5. Complete one canonical proposal loop and one implementation path on ACP-backed runtimes.

### UI Commitments

- Shell-owned persisted-truth readers remain authoritative:
  - `RunReportView`
  - `RunComparisonView`
  - `RecoverySheet`
  - `BlockedRunRecoveryView`

### UX Commitments

- Operators can trust requested vs predicted vs actual runtime/MCP truth.
- ACP migration must not hide truth behind adapter heuristics.
- First-wave Goose behavior must not be intentionally weakened by bridge concessions.

### Acceptance Criteria

1. Core orchestration code does not import Goose transport types or Goose endpoint semantics.
2. The canonical runtime abstraction in core is ACP-shaped.
3. MCP intent in core is not expressed in Goose extension IDs.
4. `RunStartSnapshot` and `AgentExecution` persist transport-neutral truth.
5. Runtime selection exists through catalog/runtime-profile and backend-profile truth.
6. Goose still works as the default runtime path after seam extraction.
7. At least two ACP runtimes can be selected through backend/runtime profiles.
8. ACP-backed runs complete at least one canonical proposal loop and one implementation path without downgrading canonical execution truth, report truth, or MCP truth.

### Test / Evidence Requirements

- Run-start proof must show frozen runtime-profile selection.
- Reports and recovery must remain grounded in persisted Forge truth via current shell-owned readers.
- ACP-backed runs must prove one canonical proposal loop and one implementation path.
- Same-tree full regression is required only for a successful audit roll-up; this pass never reaches that bar because live proposal-owned gaps remain.

## Proposal Fidelity / Divergence

### Matches

- Core runtime vocabulary is ACP-shaped in `RuntimeTransportProtocol`.
- Goose now sits behind the compatibility adapter path rather than owning the canonical transport seam.
- Backend/runtime-profile resolution and execution-time runtime selection exist on the current tree.
- `AgentExecution`, `RunReportBuilder`, and `RunComparisonService` now carry actual runtime settlement fields.
- `scripts/test-gate.sh` now contains a canonical `proposal-026` lane.

### Divergences

- No same-tree ACP-backed proof completes a canonical proposal loop and an implementation path.
- The fresh canonical `proposal-026` gate cancels before tests run because `Proposal026Tests.swift` no longer matches current APIs.

### Ambiguities / Evidence Gaps

- The repo contains first-wave ACP transports and selection wiring, but no executed proof artifact demonstrates that those transports preserve canonical execution/report/MCP truth through end-to-end user flows.

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
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift`
- Gap / Note: Core transport vocabulary now flows through `RuntimeTransportProtocol`; Goose remains an adapter implementation rather than the canonical runtime contract.

### REQ-002 The canonical runtime abstraction in core is ACP-shaped
- Proposal Source: `§5.1`, `§6.1`, `§7.1`, `§11(2)`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/ACPStreamEventMapper.swift`
  - `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`
- Gap / Note: The live transport seam is ACP-shaped around session creation, prompt submission, stream events, close, and settled runtime-state reads.

### REQ-003 MCP intent in core is not expressed in Goose extension IDs
- Proposal Source: `§4.7`, `§8.1-§8.2`, `§11(3)`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
- Gap / Note: Core runtime requests carry canonical requested-extension truth through runtime-facing structures instead of Goose-specific extension-ID ownership.

### REQ-004 `RunStartSnapshot` and `AgentExecution` persist transport-neutral truth
- Proposal Source: `§5.5`, `§8.1`, `§9.3`, `§11(4)`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
- Gap / Note: This is the biggest delta from `R1`. The current tree now persists actual `runtimeProfileID`, `actualAdapterFamily`, and `actualCapabilityClass` on `AgentExecution`, and run-level report/comparison payloads expose the same actual runtime-settlement truth.

### REQ-005 Runtime selection exists through catalog/runtime-profile and backend-profile truth
- Proposal Source: `§4.6`, `§5.2-§5.3`, `§9.3`, `§11(5)`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
- Gap / Note: Runtime profile identity is resolved from catalog/backend truth and consumed by execution-time transport selection instead of ad hoc transport overrides.

### REQ-006 Goose still works as the default runtime path after seam extraction
- Proposal Source: `§5.4`, `§9.2`, `§10.2`, `§11(6)`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - fresh canonical gate bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-122054.xcresult`
- Gap / Note: The default path is still Goose in code when no ACP runtime profile is selected, but the fresh same-tree `proposal-026` gate cancelled during test-target compilation before the targeted Goose bridge and executor suites could run. The compatibility path remains wired, but this pass could not re-prove runtime behavior end-to-end.

### REQ-007 At least two ACP runtimes can be selected through backend/runtime profiles
- Proposal Source: `§5.6`, `§9.3`, `§11(7)`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
- Gap / Note: The first-wave selection wiring for `claude_agent_acp` and `gemini_cli_acp` is live on the current tree.

### REQ-008 ACP-backed runs complete one canonical proposal loop and one implementation path without downgrading canonical execution/report/MCP truth
- Proposal Source: `§9.3`, `§10.2`, `§11(8)`
- Status: `Missing`
- Evidence Type: `code`, `tests-found`, `tests-run`
- Evidence:
  - `scripts/test-gate.sh`
  - `Chainworks ForgeTests/Proposal026Tests.swift`
  - search result: `rg -n "proposal loop|implementation path|claude_agent_acp|gemini_cli_acp|proposal-026" examples docs/evidence 'Chainworks ForgeTests' scripts/test-gate.sh`
  - fresh canonical gate bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-122054.xcresult`
- Gap / Note: The current repo contains transport selection and adapter scaffolding, but no same-tree proof shows Claude Agent ACP or Gemini CLI ACP completing a canonical proposal loop and an implementation path while preserving canonical execution/report/MCP truth. The only proposal-specific gate is currently red before any such runtime proof can execute.

## Architecture Review

**Summary:** `Acceptable`

No new live architecture finding remains after the current tree changes. The `R1` architecture blocker is closed by the now-persisted actual runtime settlement fields and their exposure in report/comparison payloads.

## Product Review

**Summary:** `At Risk`

### PROD-001 First-wave ACP value is still unproven in canonical user flows
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: `code`, `tests-found`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-122054.xcresult`
- Why It Matters: `P026` is not only a seam-extraction proposal; it explicitly promises that ACP-backed runtimes complete a real proposal loop and a real implementation path without weakening canonical truth. Until that proof exists, the product value of the migration is still asserted rather than demonstrated.
- Recommended Action: Add one canonical ACP-backed proposal-loop proof and one ACP-backed implementation-path proof, both anchored to persisted execution/report/MCP truth and wired into a stable proposal-owned gate.

## UI Review

**Summary:** `Acceptable`

No fresh UI-level contradiction surfaced against the proposal’s shell-owned report/recovery continuity contract.

## UX Review

**Summary:** `Acceptable`

No fresh UX-level contradiction surfaced beyond the missing ACP end-to-end proof and the red canonical gate.

## Readiness Review

**Summary:** `Not Ready`

### READY-001 Canonical `proposal-026` gate is live but red on the current tree
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-006`, `REQ-008`
- Evidence Type: `tests-run`
- Evidence:
  - `scripts/test-gate.sh`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-122054.xcresult`
  - `Chainworks ForgeTests/Proposal026Tests.swift`
- Why It Matters: `R1` said the proposal had no canonical gate. That is no longer true. The new problem is worse operationally: the proposal-owned gate exists, but it fails before targeted tests run because `Proposal026Tests.swift` no longer matches current APIs (`ProviderSettings` initializer drift plus unresolved `.empty` / capability-enum inference). That blocks stable same-tree proof and leaves the proposal without a trustworthy delivery lane.
- Recommended Action: Repair `Proposal026Tests.swift` to compile against current APIs, then extend the gate with executed ACP-backed proposal-loop and implementation-path proof instead of selection-only coverage.

## Readiness Checklist

| Check | Result | Evidence |
|---|---|---|
| Canonical `proposal-026` gate exists | `Pass` | `scripts/test-gate.sh` |
| Canonical `proposal-026` gate passes on current tree | `Fail` | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-122054.xcresult` |
| Actual runtime settlement persists on `AgentExecution` and shell-owned readers | `Pass` | `AgentExecution.swift`, `WorkflowOrchestrator.swift`, `RunReportBuilder.swift`, `RunComparisonService.swift` |
| ACP-backed proposal-loop proof exists | `Fail` | no matching same-tree proof found |
| ACP-backed implementation-path proof exists | `Fail` | no matching same-tree proof found |
| Same-tree full regression required for successful roll-up | `Not Run` | not eligible because `REQ-008` is `Missing` and the canonical proposal gate is red |

## Verification Log

- `git rev-parse --short HEAD` → `d8ccf4b`
- `git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md`
- `md5 -q docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md`
- `./scripts/test-gate.sh proposal-026`
  - build phase: `BUILD SUCCEEDED`
  - targeted test phase: `TEST FAILED`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-026-20260407-122054.xcresult`
  - dominant failures:
    - `No exact matches in call to initializer`
    - `Cannot infer contextual base in reference to member 'empty'`
    - `Cannot infer contextual base in reference to member 'operatorGrade'`
    - `Cannot infer contextual base in reference to member 'legacyOperatorGrade'`
    - `Testing cancelled because the build failed`

## Recommended Next Actions

1. Repair `Chainworks ForgeTests/Proposal026Tests.swift` so the canonical `proposal-026` gate compiles and runs again.
2. Add executed ACP-backed proof for one canonical proposal loop and one canonical implementation path.
3. Keep those proofs anchored to persisted Forge truth by asserting runtime settlement, execution/report payloads, and MCP truth after ACP-backed runs.
