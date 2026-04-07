# Proposal 025: Per-Agent MCP Policy and Runtime Validation Multi-Lens Audit R6

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `a0bb075` |
| Working Tree | `Dirty (49 modified, 6 untracked)` |
| Audited At | `2026-04-07T10:34:28+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P025` regressed from the previous audit basis and is now `Partial` / `Not Ready` on the current tree. The proposal-owned MCP policy code is still present: catalog-level `mcp_profile` wiring, requested / actual / denied persistence, and KPI/report consumption all remain visible in code and targeted tests. But the fresh same-tree evidence is red in two separate ways. First, the canonical `proposal-025` gate now fails before test execution because the repo guard rejects direct `Run(` construction in `ArtifactInspectorView.swift`. Second, an independent targeted xcodebuild slice for `Proposal025Tests` plus Goose transport/session tests fails at compile time with `Cannot find 'RuntimeStreamEventMapper' in scope`. Same-tree `full` regression is also unavailable from this host, and the approved remote host was not reachable from this environment. That combination fails closed under the audit skill.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Fresh same-tree MCP proof is blocked by a gate guard and a shared Goose compile failure | `High` |
| Architecture | `At Risk` | The current runtime refactor leaves Goose transport in a broken intermediate state | `High` |
| Product | `At Risk` | Canonical proposal proof does not currently reach the MCP slice itself | `High` |
| UI | `Acceptable` | Shell-owned report/comparison surfaces still encode the intended MCP truth lanes | `Medium` |
| UX | `Acceptable` | Requested / actual / denied truth remains explicit in code | `Medium` |
| Readiness | `Not Ready` | No passing same-tree proposal proof or full regression evidence exists | `High` |

## Proposal Contract

### Scope

- Repo-owned installed-server registry mapping.
- Per-agent `mcp_profile` runtime intent.
- Preflight separation of requested profile, runtime capability, and predicted effective set.
- Goose session reconciliation before prompt submission.
- Persisted requested / predicted / actual / denied MCP truth.
- Burn telemetry in the canonical run-owned KPI / report lane.

### Locked Decisions

- Default deny / zero-MCP baseline.
- `agent.mcp_profile` is the runtime authority.
- Requested, predicted, actual, and denied MCP truth remain separately inspectable.
- Burn telemetry extends existing run-owned KPI / report surfaces.

### Primary User Flows

1. Declare per-agent MCP intent in catalog YAML.
2. Run preflight and see requested vs predicted capability truth.
3. Launch a Goose-backed session that reconciles extensions before prompt submission.
4. Inspect requested / predicted / actual / denied / telemetry truth in shell-owned report surfaces.

### UI Commitments

- Diagnostics show selected `mcp_profile` plus requested / predicted / actual / denied state.
- Existing shell-owned report / comparison readers expose MCP truth and telemetry.
- Empty MCP policy yields genuinely MCP-free sessions.

### UX Commitments

- Required MCP failure is explicit and fail-closed.
- Operators can distinguish repo intent from machine/runtime settlement.

### Acceptance Criteria

- Explicit per-agent `mcp_profile`.
- Runtime capability separation from repo registry truth.
- Goose reconciliation before prompt submission.
- Persisted actual enabled MCP truth on execution rows.
- Telemetry in canonical KPI/report lane.

### Test / Evidence Requirements

- Focused proposal proof for MCP resolution and persistence.
- Same-tree successful `full` regression for any successful audit.

### Explicit Exclusions

- No widening from legacy permission metadata.
- No second metrics lane outside the canonical run-owned KPI/report spine.

## Proposal Fidelity / Divergence

### Matches

- Current code still contains `mcp_profiles`, `agent.mcp_profile`, `MCPPolicyRuntime`, `Proposal025Tests`, `GooseServerTransportTests`, and `GooseSessionBridgeTests`.
- `RunReportBuilder`, `RunComparisonService`, and `SessionReuseKPIExporter` still encode the intended MCP truth and telemetry surfaces.
- `WorkflowOrchestrator` still writes requested / actual / denied MCP fields onto `AgentExecution`.

### Divergences

- Canonical `proposal-025` gate is now blocked by the repo’s direct-`Run` guard before test execution.
- Fresh same-tree targeted xcodebuild proof fails during compile because the shared Goose transport stack is broken.

### Ambiguities / Evidence Gaps

- Fresh same-tree `full` proof could not be obtained from this host, and the approved remote host was unreachable from this environment.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 3 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Explicit per-agent `mcp_profile` exists in the catalog contract
- Proposal Source: `§5.2`, `§6`, `§9 AC1`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/DSL/YAMLValidator.swift`
- Gap / Note: The catalog still exposes `mcp_profiles` and per-agent `mcp_profile` fields.

### REQ-002 Registry truth stays separate from machine-local runtime capability truth
- Proposal Source: `§5.1`, `§5.5`, `§7`, `§9 AC2`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
- Gap / Note: The current code still separates repo YAML mapping from runtime capability evaluation.

### REQ-003 `mcp_profile` remains runtime authority
- Proposal Source: `§4`, `§5.2`, `§9 AC3`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/DSL/YAMLValidator.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
- Gap / Note: The runtime still resolves requested extensions from explicit per-agent MCP intent.

### REQ-004 Goose-backed sessions reconcile MCP policy before prompt submission
- Proposal Source: `§5.4`, `§9 AC4`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-found`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/GooseServerTransport.swift`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift`
  - `Chainworks ForgeTests/GooseSessionBridgeTests.swift`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-F0mJxm/Logs/Test/Test-Chainworks Forge-2026.04.07_10-32-49-+0300.xcresult`
- Gap / Note: Reconciliation code is still present, but the fresh same-tree proof did not build because `GooseServerTransport.swift` calls missing `RuntimeStreamEventMapper`.

### REQ-005 Preflight distinguishes requested profile, runtime capability, and predicted effective set
- Proposal Source: `§5.5`, `§9 AC5`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
- Gap / Note: The separation rule remains encoded in current preflight/runtime logic.

### REQ-006 Actual reconciled MCP state persists on execution truth
- Proposal Source: `§5.5`, `§5.6`, `§9 AC6`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
- Gap / Note: Requested, actual, and denied MCP sets are still persisted on `AgentExecution`.

### REQ-007 Diagnostics show requested / predicted / actual / denied MCP truth
- Proposal Source: `§5.6`, `§9 AC7`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
- Gap / Note: The intended shell-owned reporting path still exists in code and targeted tests.

### REQ-008 Burn telemetry extends the canonical run-owned KPI/report lane
- Proposal Source: `§5.7`, `§9 AC8`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
- Gap / Note: KPI export and report readers still consume MCP telemetry from the existing run-owned lane.

### REQ-009 Preflight fails when required MCP cannot be honored
- Proposal Source: `§5.3`, `§5.5`, `§9 AC9`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
- Gap / Note: Required-missing cases remain fail-closed in the current preflight/runtime contract.

### REQ-010 Empty MCP policy yields genuinely MCP-free sessions
- Proposal Source: `§5.4`, `§8.1`, `§9 AC10`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-found`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-F0mJxm/Logs/Test/Test-Chainworks Forge-2026.04.07_10-32-49-+0300.xcresult`
- Gap / Note: Zero-MCP policy is still encoded, but fresh execution proof could not be re-established because the focused slice failed to build.

### REQ-011 Burn telemetry shows whether tighter MCP policy reduced runtime overhead or chatter
- Proposal Source: `§5.7`, `§9 AC11`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
- Gap / Note: The current code still computes requested/actual/denied counts and MCP-specific KPI summaries.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 Shared runtime transport refactor is incomplete on the current tree
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-004`, `REQ-010`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift:293`
  - `Chainworks Forge/Engine/GooseStreamEventMapper.swift`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-F0mJxm/Logs/Test/Test-Chainworks Forge-2026.04.07_10-32-49-+0300.xcresult`
- Why It Matters: `P025` depends on a live Goose-backed reconciliation path. The current runtime transport stack does not compile cleanly, so the proposal’s execution contract is not operational.
- Recommended Action: Fix the mapper/name mismatch in the Goose transport layer before reevaluating MCP runtime conformance.

## Product Review

**Summary:** `At Risk`

### PROD-001 Canonical `proposal-025` proof is blocked before the MCP slice even starts
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-004`, `REQ-007`
- Evidence Type: `tests-run`, `code`
- Evidence:
  - `./scripts/test-gate.sh proposal-025`
  - output: `Direct Run construction found outside RunRepository: Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift:488`
- Why It Matters: Even before the compile failure, the proposal’s canonical gate is red because the current tree violates the repository’s run-construction invariant. That prevents repeatable proposal proof.
- Recommended Action: Move the preview/test helper off direct `Run(` construction or route it through `RunRepository`, then rerun the canonical gate.

## UI Review

**Summary:** `Acceptable`

### UI-001 Shell-owned report/comparison readers still carry the intended MCP truth
- Severity: `Note`
- Confidence: `Medium`
- Related Proposal Items / Requirements: `REQ-007`, `REQ-008`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
- Why It Matters: The current problem is not loss of UI ownership or duplicated telemetry lanes. It is build/proof readiness.
- Recommended Action: Revalidate these same shell-owned surfaces once the gate and build are green again.

## UX Review

**Summary:** `Acceptable`

### UX-001 Requested / actual / denied truth remains explicit in the current design
- Severity: `Note`
- Confidence: `Medium`
- Related Proposal Items / Requirements: `REQ-005`, `REQ-006`, `REQ-007`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
- Why It Matters: The proposal’s operator-facing clarity model remains intact in code.
- Recommended Action: No UX redesign action is needed until execution proof is restored.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Canonical `proposal-025` gate is currently red
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-004`, `REQ-007`
- Evidence Type: `tests-run`
- Evidence:
  - `./scripts/test-gate.sh proposal-025`
- Why It Matters: The proposal has a canonical local gate, and it now fails immediately on the current tree. That is a fresh regression against the previous audit basis.
- Recommended Action: Fix the direct-`Run` guard failure first, then rerun the canonical gate.

### READY-002 Focused same-tree xcodebuild proof is red
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-004`, `REQ-010`
- Evidence Type: `tests-run`
- Evidence:
  - focused xcodebuild command above
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p025-audit-F0mJxm/Logs/Test/Test-Chainworks Forge-2026.04.07_10-32-49-+0300.xcresult`
- Why It Matters: The MCP slice cannot currently produce fresh same-tree proof because the build fails first.
- Recommended Action: Repair the shared Goose/runtime compile path and rerun the targeted proof.

### READY-003 Same-tree `full` regression is unavailable from this host
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-004`-`REQ-011`
- Evidence Type: `runtime`
- Evidence:
  - `./scripts/test-gate.sh full`
  - output: `error: UI tests are remote-only and may not run on this host.`
  - `ssh -o BatchMode=yes -o ConnectTimeout=5 test@SMacBook.local 'hostname && pwd'`
  - output: `ssh: Could not resolve hostname smacbook.local`
- Why It Matters: Even if the focused MCP slice were green, the audit skill still forbids a successful verdict without passing same-tree `full` regression.
- Recommended Action: Restore approved-host reachability and rerun `full` after fixing the current gate and build regressions.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Fail` | Focused MCP xcodebuild slice failed with `Cannot find 'RuntimeStreamEventMapper' in scope` |
| Core user flow runtime-validated | `Fail` | Canonical gate and focused proof both red |
| Empty/loading/error states covered | `Partial` | Static report/comparison surfaces still exist; runtime not revalidated |
| Accessibility risk acceptable | `Not Checked` | No fresh UI/runtime validation in this pass |
| Localization risk acceptable | `Not Checked` | Out of scope for this pass |
| Critical tests executed | `Pass` | Canonical gate and focused xcodebuild proof were both executed, both red |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail` | `full` unavailable from this host and approved host unreachable |
| Privacy/permissions/entitlements reviewed | `Not Checked` | Not proposal-critical in this pass |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md`
- `rg -n "mcp_profile|requested.*MCP|actual.*MCP|denied.*MCP|sessionKPIExportJSON|Proposal025Tests|GooseServerTransportTests|GooseSessionBridgeTests|MCPPolicyRuntime|effectiveMCP|actualEnabledExtensionIDsJSON|deniedExtensionIDsJSON" ...`
- `./scripts/test-gate.sh proposal-025`
- `xcodebuild -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath "$DERIVED_DATA" test -only-testing:'Chainworks ForgeTests/Proposal025Tests' -only-testing:'Chainworks ForgeTests/GooseServerTransportTests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests'`
- `./scripts/test-gate.sh full`
- `ssh -o BatchMode=yes -o ConnectTimeout=5 test@SMacBook.local 'hostname && pwd'`

## Recommended Next Actions

1. Fix the direct `Run(` construction in `ArtifactInspectorView` so the canonical `proposal-025` gate can start.
2. Repair the `RuntimeStreamEventMapper` / `GooseStreamEventMapper` mismatch in the shared Goose transport compile path.
3. Rerun the canonical `proposal-025` gate and the focused MCP xcodebuild slice on the same tree.
4. Restore approved-host reachability and run same-tree `full` regression before attempting a successful audit verdict.
