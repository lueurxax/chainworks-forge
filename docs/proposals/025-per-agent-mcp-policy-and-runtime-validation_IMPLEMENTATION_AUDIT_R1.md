# Proposal 025 Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `2de983d` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-03T21:40:43+0300` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P025` has landed substantial real implementation on the current dirty tree: the catalog now carries repo-owned MCP policy/registry/profile data, preflight evaluates requested vs predicted MCP state, and Goose-backed execution reconciles session extensions before prompt submission. Focused same-tree Goose proof is strong: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-goose-20260403-213939.xcresult` passed `43/43` tests across `GooseSessionBridge` and `GooseServerTransport`.

The proposal is still not fully implemented because two acceptance criteria remain unfulfilled and two more are only partially landed. The largest gap is truth ownership: `AgentExecution` persists fields named like settled MCP execution truth, but those fields are still populated from the predicted pre-session resolution rather than from a post-reconciliation runtime read. On top of that, the promised MCP KPI/report lane is not implemented, and current shell-owned report/comparison surfaces do not expose the full requested-vs-predicted-vs-actual contract.

## Primary User Flows

1. Define per-agent MCP intent in the catalog with explicit `mcp_profile` assignments and registry-backed server IDs.
2. Run preflight and understand whether the selected runtime on this machine can honor the requested MCP contract.
3. Launch a Goose-backed execution that reconciles session extensions before the first prompt is submitted.
4. Inspect persisted run/execution truth and determine what MCP contract was requested, predicted, actually enabled, denied, and whether policy tightening reduced burn.

## Track 1: Proposal-Conformance Audit

### Proposal Contract Summary

- Scope:
  - repo-owned `mcp_server_registry`
  - per-agent `mcp_profile`
  - Goose session reconciliation before prompt submission
  - preflight validation of registry truth, requested profile, and runtime capability
  - persisted execution truth for actual enabled MCP state
  - run-owned KPI/report telemetry for MCP burn
- Locked decisions:
  - default deny / zero-MCP baseline
  - `agents.*.mcp_profile` is runtime authority
  - permission-profile MCP is legacy metadata or ceiling-only, never widening runtime truth
  - installed-registry truth, requested truth, predicted truth, and actual execution truth must remain separately inspectable
- Evidence requirements:
  - actual reconciled enabled state must persist on `AgentExecution`
  - diagnostics must show requested, predicted, actual, and denied/dropped
  - burn telemetry must land in the existing KPI/report lane
- Non-goals:
  - generic plugin marketplace
  - interactive MCP policy editor
  - non-Goose runtime implementation beyond capability hooks

### Proposal Fidelity / Divergence

**Matches**

- Catalog model now includes `mcp_policy`, `mcp_server_registry`, `mcp_profiles`, and per-agent `mcp_profile` fields in both schema and live example catalog.
- YAML validation now enforces MCP profile existence, fallback semantics, registry references, and warns on legacy permission-profile allowances that do not map to runtime registry truth.
- Run-start snapshot freezes predicted MCP resolution into `Run.resolvedMCPPoliciesJSON`.
- Goose-backed execution resolves MCP policy before session start and reconciles session extensions before prompt submission.
- Zero-MCP baseline is real for Goose transport: current extensions are removed when the requested set is empty.

**Divergences**

- `AgentExecution` settles `effectiveMCPRuntimeExtensionIDsJSON` from `predictedEffectiveRuntimeExtensionIDs`, not from a post-reconciliation read of what the session actually ended up with.
- Current shell-owned report/comparison surfaces do not consume MCP execution truth or MCP KPI summaries.
- MCP burn telemetry is not present in the run-owned KPI/export lane at all.

**Ambiguities / Evidence Gaps**

- Runtime capability remains Goose-specific and transport-derived rather than coming from a structured provider/runtime capability model.
- No proposal-scoped canonical `proposal-025` gate exists in `scripts/test-gate.sh`, so there is no repeatable same-tree audit lane for this proposal.

### Requirement Summary

| ID | Requirement | Source | Status |
|---|---|---|---|
| REQ-001 | Agents can declare an explicit `mcp_profile` in the catalog | `§9` AC1 | `Implemented` |
| REQ-002 | Catalog keeps installed-server registry separate from machine-local runtime capability truth | `§9` AC2, `§5.1`, `§7` | `Implemented` |
| REQ-003 | `mcp_profile` is the runtime authority for session extension selection | `§9` AC3, `§5.2` | `Implemented` |
| REQ-004 | Goose-backed sessions honor MCP policy before prompt submission | `§9` AC4, `§5.4` | `Implemented` |
| REQ-005 | Preflight distinguishes installed registry truth, requested MCP profile, and predicted effective set | `§9` AC5, `§5.5` | `Implemented` |
| REQ-006 | Actual reconciled enabled MCP state persists on the execution truth path | `§9` AC6, `§8.2` step 6 | `Partially Implemented` |
| REQ-007 | Diagnostics show requested, predicted, actual, and dropped/denied MCP state | `§9` AC7, `§5.6` | `Partially Implemented` |
| REQ-008 | MCP telemetry extends the existing run-owned KPI/report lane | `§9` AC8, `§5.7` | `Missing` |
| REQ-009 | Preflight fails when required MCP cannot be honored | `§9` AC9, `§5.3`, `§5.5` | `Implemented` |
| REQ-010 | Empty MCP policy yields genuinely MCP-free sessions | `§9` AC10, `§4`, `§8.1` | `Implemented` |
| REQ-011 | Burn telemetry shows whether tighter MCP policy reduced overhead/tool chatter | `§9` AC11, `§5.7` | `Missing` |

Conformance roll-up:

- `Implemented`: `7`
- `Partially Implemented`: `2`
- `Missing`: `2`
- `Not Verifiable`: `0`

Because at least one in-scope requirement is `Missing`, `Overall Conformance = Not Implemented`.

### Requirement Audit

#### REQ-001 — Explicit per-agent `mcp_profile`

- Status: `Implemented`
- Proposal source: `§9` AC1 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:462`)
- Evidence types: `code`, `tests-found`
- Evidence references:
  - `Chainworks Forge/DSL/AgentCatalog.swift:11-13`
  - `Chainworks Forge/DSL/AgentCatalog.swift:95-127`
  - `examples/agents/agents.yaml:1076-1104`
- Note: The catalog schema and live example both support explicit agent-level `mcp_profile`.

#### REQ-002 — Registry truth stays separate from machine-local runtime capability truth

- Status: `Implemented`
- Proposal source: `§9` AC2 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:463`)
- Evidence types: `code`
- Evidence references:
  - `Chainworks Forge/DSL/AgentCatalog.swift:325-380`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:71-98`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:206-232`
- Note: Repo YAML owns server mapping and policy metadata; machine-local truth is read from the local Goose registry snapshot and provider transport context, not stored back into the YAML schema.

#### REQ-003 — `mcp_profile` is runtime authority

- Status: `Implemented`
- Proposal source: `§9` AC3 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:464`)
- Evidence types: `code`
- Evidence references:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:101-133`
  - `Chainworks Forge/DSL/YAMLValidator.swift:155-212`
  - `Chainworks Forge/DSL/AgentCatalog.swift:291-349`
- Note: Resolution starts from `agent.mcpProfileID` or the catalog default profile; permission-profile MCP survives only as legacy metadata/warnings and does not widen runtime selection.

#### REQ-004 — Goose-backed sessions honor policy before prompt submission

- Status: `Implemented`
- Proposal source: `§9` AC4 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:465`)
- Evidence types: `code`, `tests-run`
- Evidence references:
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:59-87`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:137-223`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:466-482`
  - `Chainworks ForgeTests/GooseSessionBridgeTests.swift:232-245`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:236-360`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-goose-20260403-213939.xcresult`
- Note: The bridge resolves MCP before session creation, passes requested extensions into `GooseSessionRequest`, and the transport reconciles extensions before prompt submission.

#### REQ-005 — Preflight distinguishes registry truth, requested profile, and predicted effective set

- Status: `Implemented`
- Proposal source: `§9` AC5 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:466-469`)
- Evidence types: `code`
- Evidence references:
  - `Chainworks Forge/Engine/PreflightService.swift:168-176`
  - `Chainworks Forge/Engine/PreflightService.swift:540-619`
  - `Chainworks Forge/Views/IdeaListView.swift:2180-2227`
  - `Chainworks Forge/Models/Run.swift:47-53`
- Note: Preflight emits a registry check plus per-agent summaries derived from `MCPPolicyResolver`, and run-start snapshots freeze the predicted MCP resolution into `resolvedMCPPoliciesJSON`.

#### REQ-006 — Actual reconciled enabled MCP state persists on execution truth path

- Status: `Partially Implemented`
- Proposal source: `§9` AC6 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:470`)
- Evidence types: `code`
- Evidence references:
  - `Chainworks Forge/Models/AgentExecution.swift:61-64`
  - `Chainworks Forge/Engine/GooseTransport.swift:300-305`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:552-555`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:705-708`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2963-2966`
- Gap / note: The data model exists, but the persisted `effectiveMCPRuntimeExtensionIDs` comes from `sessionInfo.mcpResolution.predictedEffectiveRuntimeExtensionIDs`, not from a post-reconciliation runtime read. `GooseSessionResponse` does not carry settled extension state, so the proposal’s promised actual-execution truth is not fully implemented yet.

#### REQ-007 — Diagnostics show requested, predicted, actual, and dropped/denied state

- Status: `Partially Implemented`
- Proposal source: `§9` AC7 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:471`)
- Evidence types: `code`, `inference`
- Evidence references:
  - `Chainworks Forge/Engine/PreflightService.swift:584-619`
  - `Chainworks Forge/Views/IdeaListView.swift:2180-2227`
  - repo search: `rg -n "resolvedMCPPoliciesJSON|requestedMCPExtensionsJSON|effectiveMCPRuntimeExtensionIDsJSON|deniedMCPExtensionsJSON" 'Chainworks Forge/Views' 'Chainworks Forge/Engine/RunReportBuilder.swift' 'Chainworks Forge/Engine/RunComparisonService.swift'` returned no report/comparison consumer hits
- Gap / note: Requested, predicted, and denied state exist in preflight and frozen run-start data, but actual runtime state is not truly settled and shell-owned run/report surfaces do not expose the full contract.

#### REQ-008 — MCP telemetry extends the existing run-owned KPI/report lane

- Status: `Missing`
- Proposal source: `§9` AC8 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:472`)
- Evidence types: `code`
- Evidence references:
  - `Chainworks Forge/Models/Run.swift:76-81`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2835-2845`
  - repo search: `rg -n "MCP|mcp|zero-MCP|startup latency|tool-call count|bytes returned" 'Chainworks Forge/Engine/SessionReuseKPIExporter.swift' 'Chainworks Forge/Engine/RunReportBuilder.swift' 'Chainworks Forge/Engine/RunComparisonService.swift' 'Chainworks Forge/Views/RunReportView.swift' 'Chainworks Forge/Views/RunComparisonView.swift'`
- Gap / note: The run-owned KPI/report lane exists, but it currently exports session-reuse/strategy data only. No normalized MCP KPI payload is produced or consumed there.

#### REQ-009 — Preflight fails when required MCP cannot be honored

- Status: `Implemented`
- Proposal source: `§9` AC9 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:473`)
- Evidence types: `code`
- Evidence references:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:133-203`
  - `Chainworks Forge/Engine/PreflightService.swift:598-607`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:59-61`
- Note: Required missing extensions and unsupported session-scoped reconciliation produce blocking issues, which preflight marks as failures and live session creation rejects.

#### REQ-010 — Empty MCP policy yields genuinely MCP-free sessions

- Status: `Implemented`
- Proposal source: `§9` AC10 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:474`)
- Evidence types: `code`, `tests-run`
- Evidence references:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:22-34`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:219-223`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:466-476`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:236-294`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-goose-20260403-213939.xcresult`
- Note: Zero requested extensions cause the transport to remove existing session extensions and add none back.

#### REQ-011 — Burn telemetry proves whether policy tightening reduced overhead/tool chatter

- Status: `Missing`
- Proposal source: `§9` AC11 (`docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md:475`)
- Evidence types: `code`
- Evidence references:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2835-2845`
  - `Chainworks Forge/Views/RunReportView.swift:453-462`
  - repo search: `rg -n "startup latency|tool-call count by extension|bytes returned by each extension|prompt/context delta attributable to extension output|zero-MCP execution count|preflight-blocked run count due to MCP incompatibility" 'Chainworks Forge' 'Chainworks ForgeTests'`
- Gap / note: None of the proposal’s promised MCP burn KPIs are exported today, so the system cannot prove whether the tighter policy model materially reduced overhead or tool chatter.

## Track 2: Expert Multi-Lens Review

### Lens Scorecard

| Lens | Status | Summary |
|---|---|---|
| Architecture | `Amber` | MCP ownership is improved, but actual runtime truth still collapses back into prediction on the execution row. |
| Product | `Amber` | Core safety path works, but operators still cannot answer the full MCP contract question from standard post-run surfaces. |
| UI | `Amber` | No dedicated shell-owned MCP inspection/report surface is wired yet. |
| UX | `Amber` | Discoverability is preflight-heavy; runtime inspection degrades after launch. |
| Delivery / Readiness | `Red` | No canonical `proposal-025` gate exists, and the broader same-tree audit slice is currently red. |

### Findings

#### ARCH-001 — Execution truth is still prediction-shaped

- Severity: `Major`
- Confidence: `High`
- Related proposal items: `REQ-006`
- Evidence types: `code`
- Evidence references:
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:552-555`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:705-708`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2963-2966`
  - `Chainworks Forge/Engine/GooseTransport.swift:300-305`
- Why It Matters: The proposal explicitly separated predicted vs actual MCP truth. Persisting predicted runtime IDs into the `AgentExecution` row under an “effective” field recreates the exact parallel-authority ambiguity the design was supposed to remove.
- Recommended Action: Extend transport/session creation to return or query the settled enabled extension set after reconciliation, then persist that settled set separately from predicted preflight output.

#### PROD-001 — Post-run operator truth is incomplete

- Severity: `Major`
- Confidence: `High`
- Related proposal items: `REQ-007`, `REQ-008`, `REQ-011`
- Evidence types: `code`
- Evidence references:
  - `Chainworks Forge/Engine/PreflightService.swift:584-619`
  - `Chainworks Forge/Views/IdeaListView.swift:2180-2227`
  - repo search: `rg -n "resolvedMCPPoliciesJSON|requestedMCPExtensionsJSON|effectiveMCPRuntimeExtensionIDsJSON|deniedMCPExtensionsJSON" 'Chainworks Forge/Views' 'Chainworks Forge/Engine/RunReportBuilder.swift' 'Chainworks Forge/Engine/RunComparisonService.swift'`
- Why It Matters: Preflight can currently explain requested/predicted/denied state, but the product promise was broader: an operator should still be able to answer the MCP question after the run from persisted run/execution truth and report surfaces.
- Recommended Action: Add shell-owned run/report consumers for MCP contract data and wire them to persisted `Run`/`AgentExecution` truth rather than raw receipts or transient preflight summaries.

#### UI-001 — No shell-owned MCP inspection surface has landed

- Severity: `Minor`
- Confidence: `High`
- Related proposal items: `REQ-007`
- Evidence types: `code`
- Evidence references:
  - repo search: `rg -n "MCP|mcp" 'Chainworks Forge/Views/RunReportView.swift' 'Chainworks Forge/Views/RunComparisonView.swift' 'Chainworks Forge/Views/ArtifactInspectorView.swift' 'Chainworks Forge/Views/RecoverySheet.swift' 'Chainworks Forge/Views/PilotReadinessView.swift' 'Chainworks Forge/Views/AgentCatalogView.swift'`
- Why It Matters: Even when persisted truth exists, the current UI gives operators no first-class way to inspect it from the standard shell/report spine.
- Recommended Action: Surface MCP contract rows in existing report/comparison/inspection views instead of inventing a sidecar debug blob.

#### UX-001 — MCP diagnostics lose continuity after launch

- Severity: `Minor`
- Confidence: `Medium`
- Related proposal items: `REQ-007`
- Evidence types: `code`, `inference`
- Evidence references:
  - `Chainworks Forge/Engine/PreflightService.swift:577-619`
  - repo search: `rg -n "requestedMCPExtensionsJSON|effectiveMCPRuntimeExtensionIDsJSON|deniedMCPExtensionsJSON" 'Chainworks Forge/Views'`
- Why It Matters: The user can see MCP readiness during preflight, but once execution starts, there is no equally discoverable place to verify what actually happened. That weakens recovery/debuggability for denied or dropped extensions.
- Recommended Action: Preserve the same requested/predicted/actual vocabulary in post-run and recovery surfaces so operators do not have to infer runtime truth from earlier preflight snapshots.

#### READY-001 — No canonical proposal gate and current broader audit slice is red

- Severity: `Major`
- Confidence: `High`
- Related proposal items: delivery readiness
- Evidence types: `tests-run`, `code`
- Evidence references:
  - `scripts/test-gate.sh:732-739`
  - `scripts/test-gate.sh:807-916`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-tests-20260403-213826.xcresult`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:756-758`
- Why It Matters: This repo now requires same-tree full regression for any successful audit. `P025` has no canonical gate in `test-gate.sh`, and even the broader focused audit slice used here is not green on the current tree because `YAMLParserTests` contains an outdated artifact-contract count assertion.
- Recommended Action: Add a canonical `proposal-025` gate and clean up the current parser expectation drift before asking for a successful audit verdict.

## Readiness Checklist

| Check | Status | Notes |
|---|---|---|
| Proposal state is active and in scope | `Pass` | No supersession or replacement markers found. |
| Same-tree focused Goose MCP proof exists | `Pass` | `43/43` tests passed in `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-goose-20260403-213939.xcresult`. |
| Same-tree broader audit slice is green | `Fail` | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-tests-20260403-213826.xcresult` failed in `YAMLParserTests`. |
| Canonical `proposal-025` gate exists | `Fail` | `scripts/test-gate.sh` lists proposal gates through `proposal-024`, but not `proposal-025`. |
| Full regression proof on exact tree/HEAD exists | `Fail Closed` | Not available; also not required for this unsuccessful verdict. |
| Persisted execution truth matches proposal semantics | `Fail` | `AgentExecution` still records predicted, not actual, effective MCP runtime IDs. |
| Run-owned KPI/report lane carries MCP telemetry | `Fail` | No MCP KPI export or report consumer exists. |

## Verification Log

1. Resolved report path with the bundled helper:
   - `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md`
2. Captured reproducibility metadata:
   - `git rev-parse --show-toplevel`
   - `git rev-parse --short HEAD`
   - `git status --short`
   - `date '+%Y-%m-%dT%H:%M:%S%z'`
3. Checked proposal state markers:
   - `rg -n "superseded|deprecated|replaced by|obsolete" -S docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md docs/proposals docs/reviews`
4. Inspected proposal contract and implementation surfaces with `sed`, `nl`, and `rg`.
5. Ran focused Goose MCP proof on the same dirty tree:
   - `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-goose-20260403-213939.xcresult -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests' -only-testing:'Chainworks ForgeTests/GooseServerTransportTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=`
   - Result: `43/43` tests passed across `2` suites.
6. Ran a broader same-tree audit slice for readiness context:
   - `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-tests-20260403-213826.xcresult -only-testing:'Chainworks ForgeTests/YAMLParserTests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests' -only-testing:'Chainworks ForgeTests/GooseServerTransportTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=`
   - Result: `64` tests in `3` suites, failed because `YAMLParserTests.testParseArtifactContracts()` still expects `18` contracts while the current catalog exposes `28`.

## Recommended Next Actions

1. Make `AgentExecution` persist the actual post-reconciliation enabled extension set, not the predicted one.
2. Add shell-owned report/comparison/inspection readers for MCP requested/predicted/actual/denied truth.
3. Implement normalized MCP KPI export on `Run.sessionKPIExportJSON` and consume it in report/comparison surfaces.
4. Add a canonical `proposal-025` gate to `scripts/test-gate.sh`.
5. Fix the current `YAMLParserTests.testParseArtifactContracts()` expectation drift so the broader same-tree audit slice is green again.
