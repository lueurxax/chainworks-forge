# Proposal 025 Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `2de983d` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-03T22:11:42+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P025` moved forward materially on the current dirty tree. Fresh same-tree `proposal-025` proof is now real: `./scripts/test-gate.sh proposal-025` passed `45` tests in `3` suites and wrote `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260403-220920.xcresult`. Compared with `R1`, the old blockers around actual MCP execution truth, report/comparison visibility, KPI-lane wiring, and canonical gate existence are closed.

The proposal is still not fully implemented. Section `5.7` and `AC11` require burn telemetry that proves whether tighter MCP policy reduced overhead or tool chatter. The live KPI lane now carries requested/predicted/actual/denied counts plus reduction/drift summaries, but it still does not capture startup-latency attribution, tool-call counts by server, bytes by server, prompt/context delta attributable to MCP output, or blocked-run counts. Because at least one in-scope requirement remains only partially implemented, `Overall Conformance = Partial`. `Overall Readiness` also stays `Not Ready` because no same-tree full regression run was executed in this pass, and this audit skill now fail-closes any successful readiness verdict without full-regression proof.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Burn telemetry is still too coarse for `§5.7` / `AC11` | `High` |
| Architecture | `Acceptable` | Runtime capability truth is still transport-inferred and Goose-shaped | `High` |
| Product | `At Risk` | Operators still cannot prove MCP overhead/tool-chatter reduction from canonical telemetry | `High` |
| UI | `Acceptable` | No material proposal-owned UI gap remains on current evidence | `Medium` |
| UX | `Acceptable` | No material proposal-owned UX gap remains on current evidence | `Medium` |
| Readiness | `Not Ready` | Focused gate is green, but no same-tree full regression evidence exists | `High` |

## Proposal Contract

### Scope

- Repo-owned `mcp_server_registry`
- Per-agent `mcp_profile`
- Goose session reconciliation before prompt submission
- Preflight validation of registry truth, requested profile, and runtime capability truth
- Persisted requested/predicted/actual MCP execution truth
- MCP telemetry in the existing run-owned KPI/report lane

### Locked Decisions

- Default deny / zero-MCP baseline
- `agents.*.mcp_profile` is runtime authority
- Permission-profile MCP is legacy metadata or ceiling-only, never widening runtime truth
- Requested, predicted, actual, and denied MCP truth must stay separately inspectable
- Burn telemetry extends the existing KPI/report lane rather than a second metrics blob

### Primary User Flows

1. Declare explicit per-agent MCP intent in catalog YAML.
2. Run preflight and see whether the chosen runtime can honor the requested MCP contract.
3. Launch a Goose-backed session that reconciles extensions before the first prompt.
4. Inspect persisted run/execution truth and answer what was requested, predicted, actually enabled, denied, and whether policy tightening reduced burn.

### UI Commitments

- Diagnostics show requested, predicted, actual, and denied MCP state
- Post-run readers use existing report/comparison surfaces rather than a side metrics blob

### UX Commitments

- Preflight fails closed when required MCP cannot be honored
- Zero-MCP sessions remain genuinely MCP-free
- Operators can inspect the settled MCP contract after execution

### Acceptance Criteria

`AC1` explicit `mcp_profile`; `AC2` registry truth separate from machine-local runtime capability; `AC3` `mcp_profile` is runtime authority; `AC4` Goose honors policy before prompt; `AC5` preflight distinguishes installed/requested/predicted; `AC6` actual reconciled state persists on execution truth path; `AC7` diagnostics show requested/predicted/actual/denied; `AC8` telemetry extends the run-owned KPI/report lane; `AC9` preflight fails when required MCP cannot be honored; `AC10` empty policy yields MCP-free sessions; `AC11` burn telemetry shows whether tightening policy reduced overhead/tool chatter.

### Test / Evidence Requirements

- Persisted actual enabled MCP state on `AgentExecution`
- Post-run report/comparison consumers show requested/predicted/actual/denied MCP truth
- KPI/report lane carries MCP telemetry
- Same-tree execution evidence for the focused proposal slice

### Explicit Exclusions

- Plugin marketplace / arbitrary extension authoring
- Interactive MCP policy editor
- Non-Goose runtime implementation beyond capability hooks
- Replacing existing provider/model validation logic

## Proposal Fidelity / Divergence

### Matches

- Catalog-level `mcp_server_registry`, `mcp_profiles`, and per-agent `mcp_profile` are live in schema and validation.
- Run-start state freezes predicted MCP policy into `Run.resolvedMCPPoliciesJSON`.
- Goose-backed execution reconciles session extensions before prompt submission.
- Post-reconciliation runtime state is now read back from Goose and carried into execution truth.
- Run report/comparison paths now surface requested, predicted, actual, and denied MCP data.
- MCP telemetry now extends the run-owned KPI/report lane instead of living outside it.
- Canonical `proposal-025` gate now exists and passed on the audited tree.

### Divergences

- Burn telemetry still under-fills the minimum contract in `§5.7`: no startup-latency attribution, tool-call counts by server, bytes by server, prompt/context delta, or blocked-run counts.
- Machine-local runtime capability is still inferred from current Goose transport context rather than exposed as a broader structured provider/runtime capability model.

### Ambiguities / Evidence Gaps

- No same-tree full regression run was executed in this pass, so the audit cannot land on a successful readiness roll-up even though the focused proposal gate is green.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Explicit per-agent `mcp_profile`

- Proposal Source: `§9` AC1
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift:13`
  - `Chainworks Forge/DSL/AgentCatalog.swift:99`
  - `examples/agents/agents.yaml`
- Gap / Note: The catalog schema and live example both carry agent-level `mcp_profile`.

### REQ-002 Registry truth stays separate from machine-local runtime capability truth

- Proposal Source: `§5.1`, `§7`, `§9` AC2
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift:23-24`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:206-232`
  - `Chainworks Forge/Engine/PreflightService.swift:548-582`
- Gap / Note: Repo YAML owns server mapping, while machine-local runtime answers still come from the runtime/preflight lane rather than being written back into the catalog.

### REQ-003 `mcp_profile` is runtime authority

- Proposal Source: `§5.2`, `§9` AC3
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift:99-119`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:133-203`
  - `Chainworks Forge/DSL/YAMLValidator.swift:156-208`
- Gap / Note: Resolution starts from agent-level `mcp_profile`; permission-profile MCP remains legacy ceiling-only metadata.

### REQ-004 Goose-backed sessions honor MCP policy before prompt submission

- Proposal Source: `§5.4`, `§9` AC4
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:42-64`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:137-236`
  - `Chainworks ForgeTests/GooseSessionBridgeTests.swift`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:318-416`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260403-220920.xcresult`
- Gap / Note: Fresh focused proof passed on the audited tree.

### REQ-005 Preflight distinguishes registry truth, requested profile, and predicted effective set

- Proposal Source: `§5.5`, `§9` AC5
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:540-619`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:191-203`
  - `Chainworks Forge/Engine/RunStartSnapshot.swift:11`
  - `Chainworks Forge/Models/Run.swift:53`
- Gap / Note: Preflight emits registry-level status plus per-agent requested/effective/denied summaries, and run-start state freezes predicted MCP policy separately from execution truth.

### REQ-006 Actual reconciled enabled MCP state persists on the execution truth path

- Proposal Source: `§8.2` step 6, `§9` AC6
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseTransport.swift:314-325`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:224-236`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:359`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:109-111`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:160-164`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:554`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:707`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:290-305`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:390-415`
- Gap / Note: The old `R1` prediction-shaped persistence gap is closed. The executor now persists post-reconciliation `actualEnabledExtensions`.

### REQ-007 Diagnostics show requested, predicted, actual, and denied MCP state

- Proposal Source: `§5.6`, `§9` AC7
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:160-179`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:521-532`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:561-570`
  - `Chainworks Forge/Engine/RunComparisonService.swift:170-174`
  - `Chainworks Forge/Views/RunComparisonView.swift:233-237`
  - `Chainworks Forge/Views/RunComparisonView.swift:371-383`
  - `Chainworks ForgeTests/Proposal025Tests.swift:9-70`
  - `Chainworks ForgeTests/Proposal025Tests.swift:73-172`
- Gap / Note: Preflight summaries still exist, but the important closure is post-run shell/report visibility.

### REQ-008 MCP telemetry extends the existing run-owned KPI/report lane

- Proposal Source: `§5.7`, `§9` AC8
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/Run.swift:79`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2835-2839`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:43-58`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:152`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:298`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:521-532`
  - `Chainworks ForgeTests/Proposal025Tests.swift:54-70`
- Gap / Note: MCP telemetry is now part of the canonical run-owned KPI payload and report payload rather than a side metrics blob.

### REQ-009 Preflight fails when required MCP cannot be honored

- Proposal Source: `§5.3`, `§5.5`, `§9` AC9
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:144-149`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:153-171`
  - `Chainworks Forge/Engine/PreflightService.swift:598-607`
- Gap / Note: Required missing extensions become blocking issues; optional ones degrade to warnings only when the fallback policy allows it.

### REQ-010 Empty MCP policy yields genuinely MCP-free sessions

- Proposal Source: `§4`, `§8.1`, `§9` AC10
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:120-129`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:219-236`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift:290-315`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260403-220920.xcresult`
- Gap / Note: The zero-MCP baseline is now explicit and tested.

### REQ-011 Burn telemetry shows whether tightening MCP policy reduced session overhead or tool chatter

- Proposal Source: `§5.7`, `§9` AC11
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:47-58`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:254-313`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:521-532`
  - `Chainworks ForgeTests/Proposal025Tests.swift:65-70`
  - repo search: `rg -n "startup latency|tool-call count|bytes returned|prompt/context delta|preflight-blocked|blocked by MCP" 'Chainworks Forge/Engine' 'Chainworks Forge/Views' 'Chainworks ForgeTests/Proposal025Tests.swift'`
- Gap / Note: Live telemetry now proves requested/predicted/actual/denied counts plus policy-reduction and prediction-drift executions, but it still does not measure startup latency, per-server tool-call counts, per-server bytes, prompt/context delta attributable to MCP output, or blocked-run counts. That leaves the proposal’s overhead/tool-chatter proof incomplete.

## Architecture Review

**Summary:** `Acceptable`

### ARCH-001 Runtime capability truth is still transport-inferred

- Severity: `Minor`
- Confidence: `High`
- Related Proposal Items / Requirements: `§7`, `REQ-002`, `REQ-005`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:144-149`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:206-232`
  - `Chainworks Forge/Engine/PreflightService.swift:548-569`
- Why It Matters: The repo has separated catalog mapping truth from machine-local runtime truth, but the runtime-capability answer is still mostly inferred from Goose transport shape plus registry availability. That is weaker than the proposal’s structured capability model and makes future non-Goose parity harder.
- Recommended Action: Promote runtime capability into explicit provider/runtime metadata with first-class answers for session-scoped reconciliation, zero-extension support, add/remove support, extension enumeration, and installed-vs-disabled distinction.

## Product Review

**Summary:** `At Risk`

### PROD-001 Burn telemetry is still too coarse to prove value

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `§5.7`, `REQ-011`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:47-58`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:254-313`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:521-532`
  - `Chainworks ForgeTests/Proposal025Tests.swift:65-70`
- Why It Matters: The product promise is not only policy correctness; it is measurable burn reduction. Current telemetry can show fewer enabled extensions and some reduction/drift counts, but it still cannot answer whether session startup got cheaper or whether MCP-driven tool chatter actually dropped.
- Recommended Action: Extend the KPI lane with startup latency attributable to reconciliation, tool-call and bytes-by-server metrics, prompt/context delta from MCP output, and blocked-run counts tied to MCP preflight.

## UI Review

**Summary:** `Acceptable`

No material proposal-owned UI findings on current evidence. Requested/predicted/actual/denied MCP truth is now represented in existing report/comparison readers instead of a sidecar surface.

## UX Review

**Summary:** `Acceptable`

No material proposal-owned UX findings on current evidence. The user-facing contract is now clearer at preflight and post-run inspection time than it was in `R1`.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 No same-tree full regression proof exists for a successful roll-up

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: audit readiness
- Evidence Type: `tests-run`
- Evidence:
  - `xcodebuild build -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=`
  - `./scripts/test-gate.sh proposal-025`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260403-220920.xcresult`
- Why It Matters: The focused `proposal-025` gate is green, which is strong local evidence for the slice itself. But this audit skill now requires passing same-tree full regression before any successful readiness verdict. That evidence does not exist in this pass.
- Recommended Action: If you want a green audit, run the repository’s full regression suite or canonical full gate on this exact tree/HEAD after addressing any remaining telemetry work.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | `xcodebuild build -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' ...` passed on this tree. |
| Core user flow runtime-validated | `Partial` | Focused `proposal-025` gate passed `45` tests in `3` suites, but this pass did not execute a live operator/runtime walkthrough. |
| Empty/loading/error states covered | `Not Checked` | Out of scope for this proposal slice. |
| Accessibility risk acceptable | `Not Checked` | No accessibility-specific validation was run in this pass. |
| Localization risk acceptable | `Not Checked` | No localization-specific validation was run in this pass. |
| Critical tests executed | `Pass` | `./scripts/test-gate.sh proposal-025` passed and wrote `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260403-220920.xcresult`. |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail Closed` | Not executed in this pass. |
| Privacy/permissions/entitlements reviewed | `Not Checked` | Not a proposal-owned focus area here. |

## Verification Log

- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md'`
- `git rev-parse --show-toplevel && git rev-parse --short HEAD && git status --short`
- `date '+%Y-%m-%dT%H:%M:%S%z' && md5 -q 'docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md'`
- `rg -n "superseded|deprecated|replaced by|obsolete" -S 'docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md' 'docs/proposals' 'docs/reviews'`
- `rg -n "proposal-025|PROPOSAL_025_TESTS|Proposal025Tests|GooseSessionBridgeTests|GooseServerTransportTests" 'scripts/test-gate.sh' 'Chainworks ForgeTests/Proposal025Tests.swift'`
- `rg -n "actualEnabledExtensions|readSessionRuntimeState|GooseSessionExecution|effectiveMCPRuntimeExtensionIDs|requestedMCPExtensionsJSON|deniedMCPExtensionsJSON" 'Chainworks Forge/Engine/GooseTransport.swift' 'Chainworks Forge/Engine/GooseServerTransport.swift' 'Chainworks Forge/Engine/GooseSessionBridge.swift' 'Chainworks Forge/Engine/GooseAgentExecutor.swift' 'Chainworks Forge/Models/AgentExecution.swift'`
- `rg -n "MCPTelemetrySummary|mcpTelemetry|totalExecutionsWithMCPProfile|totalPredictionDriftExecutions|totalPolicyReductionExecutions|requestedMCPExtensions|effectiveMCPRuntimeExtensionIDs|deniedMCPExtensions" 'Chainworks Forge/Engine/SessionReuseKPIExporter.swift' 'Chainworks Forge/Engine/RunReportBuilder.swift' 'Chainworks Forge/Engine/RunComparisonService.swift' 'Chainworks Forge/Views/RunComparisonView.swift' 'Chainworks Forge/Views/RunReportView.swift'`
- `xcodebuild build -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=`
- `./scripts/test-gate.sh proposal-025`
- `rg -n "startup latency|tool-call count|bytes returned|prompt/context delta|preflight-blocked|blocked by MCP" 'Chainworks Forge/Engine' 'Chainworks Forge/Views' 'Chainworks ForgeTests/Proposal025Tests.swift'`

## Recommended Next Actions

1. Extend MCP burn telemetry with the missing `§5.7` signals: startup latency attribution, per-server tool-call counts, per-server bytes, prompt/context delta, and blocked-run counts.
2. Promote machine-local runtime capability into explicit provider/runtime metadata instead of transport inference only.
3. If you want a successful audit verdict, run full same-tree regression after the telemetry contract is complete.
