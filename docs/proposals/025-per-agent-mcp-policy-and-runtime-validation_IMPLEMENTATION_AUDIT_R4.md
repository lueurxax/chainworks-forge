# Proposal 025 Implementation Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `2de983d` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-04T07:35:21+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P025`'s proposal-owned contract is implemented on the current tree, but the audit still fail-closes to `Partial` / `Not Ready`. The local canonical `proposal-025` gate passed `51/51` tests in `3` suites on the audited dirty tree, and the current repo clearly carries the promised MCP persistence and KPI telemetry surfaces. The blocker is the updated audit rule for successful verdicts: same-tree `full` regression must also pass. On the exact synced approved-host copy of this dirty tree, `full` went red with fresh failures in `ProviderPlatformTests`, a SwiftData crash during `ResumeManager` coverage, and repo-backed `FullMVPDeliveryTests`. Because same-tree `full` is red, the audit cannot land on `Implemented` / `Ready` even though every explicit `REQ-*` item below is implemented.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | All proposal-owned requirements are implemented, but successful roll-up is blocked by red same-tree `full` regression | `High` |
| Architecture | `At Risk` | Approved-host replay still hard-fails some repo-backed flows when Goose extension registry capability is unavailable | `High` |
| Product | `At Risk` | Repo-backed delivery and refine-loop flows still break in canonical `full` regression | `High` |
| UI | `Acceptable` | Shell-owned report / comparison readers expose the MCP contract and telemetry promised by the proposal | `Medium` |
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
- Requested, predicted, actual, and denied MCP truths stay separately inspectable.
- Burn telemetry extends the canonical run-owned KPI / report lane rather than creating a second metrics blob.

### Primary User Flows

1. Declare explicit per-agent MCP intent in catalog YAML.
2. Run preflight and see whether the chosen runtime can honor that MCP contract.
3. Launch a Goose-backed session that reconciles extensions before the first prompt.
4. Inspect persisted requested, predicted, actual, denied, and burn-telemetry truths in shell-owned reporting surfaces.

### UI Commitments

- Diagnostics show requested, predicted, actual, and denied MCP state.
- Existing report / comparison readers expose MCP telemetry without creating a parallel metrics surface.

### UX Commitments

- Preflight fails closed when required MCP cannot be honored.
- Empty MCP policy produces genuinely MCP-free sessions.
- Operators can inspect the settled MCP contract after execution.

### Acceptance Criteria

`AC1` explicit per-agent `mcp_profile`; `AC2` server mapping separate from machine-local runtime capability truth; `AC3` `mcp_profile` is runtime authority; `AC4` Goose honors policy before prompt submission; `AC5` preflight distinguishes installed / requested / predicted; `AC6` actual reconciled state persists on `AgentExecution`; `AC7` diagnostics show requested / predicted / actual / denied; `AC8` telemetry extends the run-owned KPI / report lane; `AC9` preflight fails when required MCP cannot be honored; `AC10` empty policy yields MCP-free sessions; `AC11` burn telemetry shows whether tighter MCP policy reduced overhead or tool chatter.

### Test / Evidence Requirements

- Persisted actual enabled MCP state on `AgentExecution`.
- Post-run readers show requested / predicted / actual / denied MCP truth.
- KPI / report lane carries MCP telemetry.
- Same-tree execution evidence for the focused proposal slice.

### Explicit Exclusions

- Plugin marketplace or arbitrary extension authoring.
- Interactive MCP policy editor.
- Non-Goose runtime implementation beyond capability hooks.
- Replacing existing provider / model validation logic.

## Proposal Fidelity / Divergence

### Matches

- Catalog-level `mcp_server_registry`, `mcp_profiles`, and per-agent `mcp_profile` wiring is live.
- Run-start state freezes predicted MCP policy into immutable run truth.
- Goose-backed execution reconciles session extensions before prompt submission and persists settled runtime state.
- Existing shell-owned report / comparison readers expose requested, predicted, actual, and denied MCP truth from persisted run / execution data.
- The run-owned KPI lane now carries the burn-telemetry fields promised by `§5.7`.
- The local canonical `proposal-025` gate is green with `51/51` tests.

### Divergences

- No live proposal-owned functional divergence remains in the focused `P025` slice.
- The same-tree approved-host `full` run is red, so the audit cannot roll up to success under the current skill.

### Ambiguities / Evidence Gaps

- The approved-host red `full` bundle was interrupted only after fresh failures were already visible. The failure basis is sufficient and fresh, but the bundle is incomplete rather than cleanly finalized.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Explicit per-agent `mcp_profile`

- Proposal Source: `§9` `AC1`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `examples/agents/agents.yaml:71-82`
- Gap / Note: Per-agent MCP intent remains first-class in the catalog schema and examples.

### REQ-002 Registry truth stays separate from machine-local runtime capability truth

- Proposal Source: `§5.1`, `§7`, `§9` `AC2`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
- Gap / Note: Repo YAML owns server mapping; machine-local capability answers remain in runtime / preflight logic.

### REQ-003 `mcp_profile` is runtime authority

- Proposal Source: `§5.2`, `§9` `AC3`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/DSL/YAMLValidator.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
- Gap / Note: Legacy permission-profile MCP no longer widens runtime truth.

### REQ-004 Goose-backed sessions honor MCP policy before prompt submission

- Proposal Source: `§5.4`, `§9` `AC4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/GooseServerTransport.swift`
  - local focused bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-072808.xcresult`
- Gap / Note: The fresh local gate passed `51/51`.

### REQ-005 Preflight distinguishes registry truth, requested profile, and predicted effective set

- Proposal Source: `§5.5`, `§9` `AC5`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
- Gap / Note: Requested MCP truth is frozen separately from predicted settlement.

### REQ-006 Actual reconciled enabled MCP state persists on the execution truth path

- Proposal Source: `§8.2` step 6, `§9` `AC6`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift:61-66`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2969-2974`
  - `Chainworks Forge/Engine/GooseServerTransport.swift`
- Gap / Note: Execution rows now store requested, actual, denied, startup-latency, and per-server metrics.

### REQ-007 Diagnostics show requested, predicted, actual, and denied MCP state

- Proposal Source: `§5.6`, `§9` `AC7`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift:162-220`
- Gap / Note: The current shell-owned post-run readers consume persisted MCP truth rather than reconstructing it heuristically.

### REQ-008 MCP telemetry extends the existing run-owned KPI/report lane

- Proposal Source: `§5.7`, `§9` `AC8`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:31-79`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2835-2840`
  - `Chainworks Forge/Models/Run.swift`
- Gap / Note: MCP telemetry is exported through the canonical run-owned KPI JSON and report payload.

### REQ-009 Preflight fails when required MCP cannot be honored

- Proposal Source: `§5.3`, `§5.5`, `§9` `AC9`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
- Gap / Note: Required missing extensions remain blocking issues; optional ones degrade to warnings only when policy allows it.

### REQ-010 Empty MCP policy yields genuinely MCP-free sessions

- Proposal Source: `§4`, `§8.1`, `§9` `AC10`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
  - local focused bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-072808.xcresult`
- Gap / Note: Zero-MCP is both a declared policy and a measured runtime outcome.

### REQ-011 Burn telemetry shows whether tightening MCP policy reduced session overhead or tool chatter

- Proposal Source: `§5.7`, `§9` `AC11`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift:61-66`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:47-79`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift:162-220`
- Gap / Note: The live contract covers startup latency, per-server tool-call and byte usage, prompt-context delta, blocked-run count, and zero-MCP counts.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 Approved-host replay still has MCP-runtime capability fragility outside the focused slice

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: Primary flows 2-4, `REQ-005`, `REQ-009`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - red same-tree `full` failure in `Chainworks ForgeTests/ProviderPlatformTests.swift:1572`
  - failure text: `Goose extension registry is unavailable; cannot validate MCP profile ...`
- Why It Matters: The proposal-owned contract is implemented, but broader repo-backed flows on the approved host still assume registry capability that is not actually available there. That makes architecture-level replay less robust than the proposal’s clean ownership model suggests.
- Recommended Action: Decide whether approved-host `full` should provide a portable Goose extension-registry fixture or whether those repo-backed flows must degrade more narrowly when registry capability is absent.

## Product Review

**Summary:** `At Risk`

### PROD-001 Canonical repo-backed delivery and refine-loop flows are still broken in same-tree `full`

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: successful audit roll-up gate
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:830-833`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:1083-1084`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:1209-1216`
  - red same-tree `full` terminal output showing missing delivery receipts / release manifest and blocked refine-loop completion
- Why It Matters: Even though `P025` itself is implemented, the repo’s canonical product-level replay is still too unstable to support a successful handoff verdict.
- Recommended Action: Repair the repo-backed evidence-pack / partial-delivery export path and refine-loop fixture before claiming readiness from the full system perspective.

## UI Review

**Summary:** `Acceptable`

No material proposal-owned UI finding remains on current evidence. The MCP contract and telemetry promised by `P025` are rendered through the existing shell-owned report / comparison surfaces rather than a parallel UI lane.

## UX Review

**Summary:** `Acceptable`

No material proposal-owned UX finding remains on current evidence. Requested vs predicted vs actual MCP state is explicit, and the runtime still fail-closes when required MCP cannot be honored.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Same-tree approved-host `full` regression is red

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: successful audit roll-up gate
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - local focused gate: `./scripts/test-gate.sh proposal-025`
  - focused bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-072808.xcresult`
  - approved-host sync root: `/Users/test/chainworks-audit-2de983d-dual-20260404`
  - approved-host `full` command: `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-dual-20260404' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD='***' ./scripts/test-gate.sh full"`
  - fresh red failures visible in `ProviderPlatformTests.swift:1572`, `FullMVPDeliveryTests.swift:1083-1084`, and `FullMVPDeliveryTests.swift:1209-1216`
  - incomplete red bundle path: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/full-20260404-073234.xcresult`
- Why It Matters: The updated audit skill explicitly forbids `Implemented`, `Ready`, and `Ready with Risks` without passing same-tree `full` regression.
- Recommended Action: Clear the fresh repo-level `full` failures first, then rerun this implementation audit on the same tree.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | Local focused `proposal-025` gate built and passed |
| Core user flow runtime-validated | `Pass` | Local `proposal-025` gate passed `51/51` in `3` suites |
| Empty/loading/error states covered | `Partial` | Proposal-owned preflight / reconciliation failure handling is exercised; broader app-state coverage is not the limiting factor here |
| Accessibility risk acceptable | `Not Checked` | No dedicated accessibility audit was run |
| Localization risk acceptable | `Not Checked` | Not reviewed in this pass |
| Critical tests executed | `Pass` | Focused `proposal-025` gate plus approved-host same-tree `full` gate were both executed |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail` | Approved-host same-tree `full` went red before completion |
| Privacy/permissions/entitlements reviewed | `Not Checked` | Not part of the proposal-owned acceptance contract |

## Verification Log

- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md'`
- `git rev-parse --show-toplevel && git rev-parse --short HEAD && git status --short`
- `date '+%Y-%m-%dT%H:%M:%S%z'`
- `md5 -q 'docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md'`
- `./scripts/test-gate.sh proposal-025`
- `xcrun xcresulttool get test-results summary --path '/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260404-072808.xcresult'`
- `tar czf - -C '/Users/user/Documents/Chainworks Forge' --exclude .git --exclude .codex . | ssh test@SMacBook.local "rm -rf '/Users/test/chainworks-audit-2de983d-dual-20260404' && mkdir -p '/Users/test/chainworks-audit-2de983d-dual-20260404' && tar xzf - -C '/Users/test/chainworks-audit-2de983d-dual-20260404'"`
- same-tree parity spot-checks:
  - local / remote proposal MD5 = `ec820bc41594781712a416ba2571a432`
  - local / remote `md5 examples/agents/agents.yaml` = `308643ff946473f9c390fcc7c7b35711`
  - local / remote `md5 'Chainworks ForgeTests/TestSupport.swift'` = `fa991bcf537ecf180d2b6520fe91fb8a`
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-dual-20260404' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD='***' ./scripts/test-gate.sh full"`

## Recommended Next Actions

1. Fix the fresh same-tree `full` failures around Goose extension-registry availability, repo-backed evidence export, and refine-loop completion.
2. Rerun the approved-host canonical `full` gate on the same synced tree.
3. Once `full` is green, rerun this audit to unlock a successful roll-up for `P025`.
