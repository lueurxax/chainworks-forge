# Proposal 025 Implementation Audit R5

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `73e4169` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-04T12:23:56+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P025`'s proposal-owned contract is still implemented on the current tree, but the audit must again fail-close to `Partial` / `Not Ready`. The local canonical `proposal-025` gate passed `51/51` tests in `3` suites at `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-121229.xcresult`, and the current implementation still persists requested, predicted, actual, denied, and telemetry-rich MCP truth on the intended run/execution owner paths. The blocker is now purely readiness: on the exact synced approved-host copy of this dirty tree, same-tree `full` went red with fresh failures in `ProviderPlatformTests.testSampleRunLauncherCreatesFrozenProviderBindingSnapshot`, `ExecutionServiceTests` end-to-end coverage (`Full canonical workflow executes through all states`), and `MVPGoldenRunTests.testFullMVPLiveReachesWorkflowCompleteWithFixtureTransport`. Because successful audit verdicts require a green same-tree `full` regression, `P025` cannot currently roll up to `Implemented` / `Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | All proposal-owned requirements are implemented, but successful roll-up is blocked by red same-tree `full` regression | `High` |
| Architecture | `At Risk` | Approved-host replay still fails when Goose extension-registry capability is unavailable | `High` |
| Product | `At Risk` | Canonical repo-backed end-to-end flows are still red in same-tree `full` | `High` |
| UI | `Acceptable` | Shell-owned report / comparison readers still expose the promised MCP contract and telemetry | `Medium` |
| UX | `Acceptable` | Requested vs predicted vs actual vs denied MCP truth remains explicit and fail-closed | `Medium` |
| Readiness | `Not Ready` | Same-tree approved-host `full` is red | `High` |

## Proposal Contract

### Scope

- Repo-owned `mcp_server_registry`.
- Per-agent `mcp_profile`.
- Goose session reconciliation before prompt submission.
- Preflight validation of registry truth, requested profile, and runtime capability truth.
- Persisted requested / predicted / actual / denied MCP truth.
- MCP burn telemetry in the existing run-owned KPI / report lane.

### Locked Decisions

- Default deny / zero-MCP baseline.
- `agents.*.mcp_profile` is runtime authority.
- Permission-profile MCP is legacy metadata or ceiling-only, never a widening source.
- Requested, predicted, actual, and denied truths stay separately inspectable.
- Burn telemetry extends the canonical run-owned KPI / report lane rather than creating a new metrics blob.

### Primary User Flows

1. Declare explicit per-agent MCP intent in catalog YAML.
2. Run preflight and see whether the chosen runtime can honor that MCP contract.
3. Launch a Goose-backed session that reconciles extensions before the first prompt.
4. Inspect persisted requested, predicted, actual, denied, and burn telemetry truths in shell-owned reporting surfaces.

### UI / UX Commitments

- Diagnostics show selected `mcp_profile` plus requested, predicted, actual, and denied MCP state.
- Existing report / comparison readers expose the MCP contract and telemetry without creating a parallel surface.
- Preflight fails closed when required MCP cannot be honored.
- Empty MCP policy yields genuinely MCP-free sessions.

### Test / Evidence Requirements

- Persisted actual enabled MCP state on `AgentExecution`.
- Post-run readers show requested / predicted / actual / denied MCP truth.
- KPI / report lane carries MCP telemetry.
- Same-tree focused proposal proof and, for any successful audit, same-tree `full` regression.

## Proposal Fidelity / Divergence

### Matches

- Catalog-level `mcp_server_registry`, `mcp_profiles`, and per-agent `mcp_profile` wiring remain live.
- Frozen run-start truth still captures requested / predicted MCP state separately from execution rows.
- Goose-backed execution still reconciles extensions before prompt submission and persists settled runtime truth.
- Existing shell-owned report / comparison readers still expose requested, predicted, actual, denied, and telemetry data from persisted run / execution records.
- The local canonical `proposal-025` gate is green with `51/51`.

### Divergences

- No focused proposal-owned MCP gap remains on the current tree.
- Same-tree approved-host `full` is red, so the audit cannot roll up to success under the current skill contract.

### Ambiguities / Evidence Gaps

- The approved-host `full` run was interrupted after multiple fresh failures were already visible. The red basis is current and sufficient, but the partial `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/full-20260404-122221.xcresult` bundle is not readable as a clean finished result bundle.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Explicit per-agent `mcp_profile`

- Proposal Source: `§5.2`, `§9` `AC1`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `examples/agents/agents.yaml`
- Gap / Note: Per-agent MCP intent remains first-class in the catalog schema and fixtures.

### REQ-002 Registry truth stays separate from machine-local runtime capability truth

- Proposal Source: `§5.1`, `§5.5`, `§7`, `§9` `AC2`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
- Gap / Note: Repo YAML still owns server mapping while preflight/runtime own machine-local capability truth.

### REQ-003 `mcp_profile` is runtime authority

- Proposal Source: `§4`, `§5.2`, `§9` `AC3`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/DSL/YAMLValidator.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
- Gap / Note: Current tree still treats permission-profile MCP as legacy metadata / ceiling-only rather than widening runtime truth.

### REQ-004 Goose-backed sessions honor MCP policy before prompt submission

- Proposal Source: `§5.4`, `§9` `AC4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/GooseServerTransport.swift`
  - local focused bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-121229.xcresult`
- Gap / Note: Fresh same-tree proof still shows reconciliation before prompt submission.

### REQ-005 Preflight distinguishes registry truth, requested profile, and predicted effective set

- Proposal Source: `§5.5`, `§9` `AC5`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
- Gap / Note: Current tree still freezes normalized requested truth on run start and keeps prediction separate from execution settlement.

### REQ-006 Actual reconciled enabled MCP state persists on the execution truth path

- Proposal Source: `§5.5`, `§8.2`, `§9` `AC6`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
- Gap / Note: Execution rows still persist requested, effective, denied, startup-latency, and per-server telemetry truth after reconciliation.

### REQ-007 Diagnostics show requested, predicted, actual, and denied MCP state

- Proposal Source: `§5.6`, `§9` `AC7`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
- Gap / Note: Current shell-owned readers still consume persisted MCP truth rather than reconstructing it from receipts alone.

### REQ-008 MCP telemetry extends the existing run-owned KPI/report lane

- Proposal Source: `§5.7`, `§9` `AC8`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Models/Run.swift`
  - local focused bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-121229.xcresult`
- Gap / Note: MCP telemetry is still written into the canonical run-owned KPI/report JSON lane.

### REQ-009 Preflight fails when required MCP cannot be honored

- Proposal Source: `§5.3`, `§5.5`, `§9` `AC9`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
- Gap / Note: Required missing extensions remain blocking errors; optional entries still degrade only under explicit fallback policy.

### REQ-010 Empty MCP policy yields genuinely MCP-free sessions

- Proposal Source: `§4`, `§5.4`, `§8.1`, `§9` `AC10`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
  - local focused bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-121229.xcresult`
- Gap / Note: Zero-MCP remains both a declared policy and a measured runtime outcome.

### REQ-011 Burn telemetry shows whether tighter MCP policy reduced session overhead or tool chatter

- Proposal Source: `§5.7`, `§9` `AC11`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
  - local focused bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-121229.xcresult`
- Gap / Note: Current tree still reports startup latency, per-server usage, prompt/context delta, blocked-run count, and zero-MCP counts.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 Approved-host replay still depends on Goose extension-registry capability that is missing there

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-005`, `REQ-009`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - approved-host terminal output from `./scripts/test-gate.sh full` on `/Users/test/chainworks-audit-73e4169-dual-20260404`
  - failure in `ProviderPlatformTests.testSampleRunLauncherCreatesFrozenProviderBindingSnapshot`
  - failure text: `Goose extension registry is unavailable; cannot validate MCP profile ...`
- Why It Matters: The proposal-owned contract is implemented, but the broader architecture is still not robust when approved-host replay lacks Goose extension-registry capability. That makes same-tree ship/readiness proof fail even though the focused MCP slice is green.
- Recommended Action: Decide whether approved-host `full` should ship a portable Goose registry fixture or whether runtime/preflight must narrow this failure more gracefully for repo-backed tests.

## Product Review

**Summary:** `At Risk`

### PROD-001 Canonical repo-backed end-to-end flows are still broken in same-tree `full`

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: Primary flows 2-4
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - approved-host terminal output from `./scripts/test-gate.sh full` on `/Users/test/chainworks-audit-73e4169-dual-20260404`
  - failure in `ExecutionServiceTests`: `Full canonical workflow executes through all states`
  - failure in `MVPGoldenRunTests`: `full-mvp-live reaches workflow_complete with fixture transport`
- Why It Matters: These are not proposal-owned MCP deltas, but they block the product from demonstrating a healthy same-tree execution baseline. That is enough to stop a successful audit roll-up under the current rules.
- Recommended Action: Repair the repo-backed workflow-completion failures, then rerun `full` on the same synced tree before claiming `P025` ready.

## UI Review

**Summary:** `Acceptable`

No new proposal-owned UI gap surfaced on the current tree. Shell-owned report/comparison surfaces still expose the MCP contract and telemetry promised by the proposal.

## UX Review

**Summary:** `Acceptable`

No new proposal-owned UX gap surfaced on the current tree. Requested vs predicted vs actual vs denied MCP truth is still explicit, fail-closed, and inspectable through existing shell-owned surfaces.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Successful verdict is blocked solely by red same-tree `full` regression

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: all `REQ-*` roll-up
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - local focused bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-121229.xcresult`
  - approved-host terminal output from `./scripts/test-gate.sh full` on `/Users/test/chainworks-audit-73e4169-dual-20260404`
  - partial bundle path: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/full-20260404-122221.xcresult`
- Why It Matters: The focused `P025` slice is green, but the audit skill now requires passing same-tree `full` before any successful roll-up. Current tree does not meet that bar.
- Recommended Action: Fix the approved-host `full` blockers and rerun the exact same synced tree before re-auditing `P025`.
