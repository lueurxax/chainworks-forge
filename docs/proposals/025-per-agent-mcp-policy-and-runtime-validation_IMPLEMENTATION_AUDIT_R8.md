# Proposal 025: Per-Agent MCP Policy and Runtime Validation Multi-Lens Audit R8

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `d8ccf4b` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-07T12:13:50+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P025` is no longer blocked by the stale `preferredExampleURLPrefersRepositoryCopy()` basis from the prior audit, and the old `signing_args[@]` shell bug is also no longer the live issue. The fresh same-tree canonical gate is red on a different basis: `./scripts/test-gate.sh proposal-025` now launches cleanly, compiles the current MCP implementation and `Proposal025Tests.swift`, and then fails because the shared `Chainworks ForgeTests` target compile-breaks in `Proposal026Tests.swift`. As a result, no fresh MCP assertions execute on the current tree. The MCP implementation itself still appears wired in code, but the proposal-owned proof lane is presently blocked by shared regression, so the audit stays `Partial` / `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Fresh same-tree MCP proof is blocked by a neighboring test-target regression | `High` |
| Architecture | `Acceptable` | Repo-owned policy vs runtime-settlement wiring remains intact in code | `High` |
| Product | `At Risk` | No fresh same-tree MCP runtime assertions completed in this pass | `High` |
| UI | `Acceptable` | Shell-owned report/comparison readers still carry MCP truth in the implementation | `Medium` |
| UX | `Acceptable` | Requested vs actual vs denied MCP truth remains explicit in the model/report path | `Medium` |
| Readiness | `Not Ready` | Canonical proposal gate is red before any successful roll-up can be considered | `High` |

## Proposal Contract

### Scope

- Repo-owned `mcp_profile` mapping in the agent catalog.
- Separation of requested MCP intent, predicted capability, and settled session truth.
- Goose-side reconciliation before prompt submission.
- Persisted requested / actual / denied MCP truth on execution records.
- Burn telemetry in the canonical run-owned KPI/report lane.

### Locked Decisions

- Default deny / zero-MCP baseline.
- `agent.mcp_profile` is the runtime authority.
- Requested, predicted, actual, and denied MCP truth remain separately inspectable.
- Burn telemetry extends the existing run-owned KPI/report spine.

### Acceptance Criteria

- Explicit per-agent `mcp_profile`.
- Runtime capability stays separate from repo registry truth.
- Goose reconciliation happens before prompt submission.
- Actual enabled MCP truth persists on execution rows.
- Canonical KPI/report lane carries MCP-burn telemetry.
- Canonical same-tree `proposal-025` gate passes.

## Proposal Fidelity / Divergence

### Matches

- `mcp_profiles`, `agent.mcp_profile`, `MCPPolicyRuntime`, `WorkflowOrchestrator`, `RunReportBuilder`, `RunComparisonService`, and `SessionReuseKPIExporter` remain present on the current tree.
- The fresh canonical gate compiles `Proposal025Tests.swift` plus the current MCP implementation files on the same tree.
- The old direct-`Run` guard blocker remains closed.

### Divergences

- The fresh same-tree canonical gate is red before any Proposal 025 tests execute.
- The live blocker is now compile drift in `Chainworks ForgeTests/Proposal026Tests.swift`, not an identified MCP contract failure inside `P025`.

### Fresh Basis Delta vs R7

- `scripts/test-gate.sh proposal-025` now launches correctly and is no longer blocked by the old shell bug.
- The stale `preferredExampleURL` failure is no longer the active executed basis because the gate never reaches test execution.
- The new blocker is a shared test-target compile failure in Proposal 026 coverage.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 3 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 2 |

## Requirement Audit

### REQ-001 Per-agent `mcp_profile` intent remains explicit and runtime-authoritative
- Proposal Source: `§5.2`, `§6`, `§9 AC1-AC3`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/DSL/YAMLValidator.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - fresh local canonical gate build compiled the MCP catalog/runtime path on the current tree
- Gap / Note: No fresh code-level contradiction surfaced against the authority split.

### REQ-002 Requested, predicted, actual, and denied MCP truth stay separately inspectable
- Proposal Source: `§5.5-§5.6`, `§7`, `§9 AC4-AC7`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - fresh local canonical gate compiled these files on the current tree
- Gap / Note: The report/comparison persistence surfaces remain present; the current blocker is before execution.

### REQ-003 Goose reconciliation and MCP telemetry remain wired into the current runtime/report path
- Proposal Source: `§5.4`, `§5.7`, `§9 AC4`, `§9 AC8-AC11`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift`
  - fresh local canonical gate compiled these files on the current tree
- Gap / Note: No fresh code-level contradiction surfaced against the reconciliation/telemetry contract.

### REQ-004 Portable proof prefers repo copies over bundled fallback when both exist
- Proposal Source: `§8`, `§9 proof expectations`
- Status: `Not Verifiable`
- Evidence Type: `code`, `blocked-tests-run`
- Evidence:
  - `Chainworks Forge/Support/AppConfiguration.swift`
  - `Chainworks ForgeTests/Proposal025Tests.swift`
  - fresh local invocation: `./scripts/test-gate.sh proposal-025`
- Gap / Note: `Proposal025Tests.swift` compiled, but the gate failed earlier in `Proposal026Tests.swift`, so this portability assertion did not execute on the current tree.

### REQ-005 Canonical proposal-owned proof lane passes on the same tree
- Proposal Source: `§9`, `§10`
- Status: `Partially Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - local invocation: `./scripts/test-gate.sh proposal-025`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260407-120343.xcresult`
  - failure text:
    `No exact matches in call to initializer`
    `Cannot infer contextual base in reference to member 'empty'`
    `Cannot infer contextual base in reference to member 'operatorGrade'`
    `Cannot infer contextual base in reference to member 'legacyOperatorGrade'`
  - failing compile unit:
    `Chainworks ForgeTests/Proposal026Tests.swift`
- Gap / Note: The canonical gate exists and starts correctly, but it is red before the Proposal 025 slice can execute.

### REQ-006 Same-tree successful full regression exists for a green audit roll-up
- Proposal Source: `Test / Evidence Requirements`
- Status: `Not Verifiable`
- Evidence Type: `audit-policy`
- Evidence:
  - focused canonical gate above is already red
  - current audit skill requires successful proposal-owned proof before any successful roll-up is relevant
- Gap / Note: I did not run `full` in this pass because the proposal-owned canonical gate is already red on the same tree.

## Architecture Review

**Summary:** `Acceptable`

No fresh architecture contradiction reopened the MCP contract. The live issue is proof execution blockage, not a newly discovered authority split or report-lane regression inside the P025 implementation.

## Product Review

**Summary:** `At Risk`

### PROD-001 Fresh MCP proof is masked by shared test-target regression
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-004`, `REQ-005`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - local canonical gate result bundle above
  - failing compile unit: `Chainworks ForgeTests/Proposal026Tests.swift`
- Why It Matters: Even if the MCP implementation is correct, this repo state cannot currently produce trustworthy same-tree proof for it.
- Recommended Action: Fix the shared Proposal 026 compile drift first, then rerun the same-tree `proposal-025` gate.

## UI Review

**Summary:** `Acceptable`

No fresh UI contradiction surfaced. The shell-owned report and comparison readers still appear aligned with the proposal in code.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Canonical `proposal-025` gate is fresh-red on the current tree
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-005`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - local invocation and result bundle above
  - failure text above
- Why It Matters: Under the current audit skill, no successful audit is possible while the canonical proposal gate itself is red.
- Recommended Action: Repair `Chainworks ForgeTests/Proposal026Tests.swift`, then rerun `./scripts/test-gate.sh proposal-025`.

### READY-002 Full regression was intentionally not attempted after the focused gate failed
- Severity: `Medium`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: `audit-policy`
- Evidence:
  - canonical gate above is already red
- Why It Matters: A green full regression would not rescue a red proposal-owned gate. The immediate blocker is earlier in the stack.
- Recommended Action: Fix the red canonical gate first; only then spend time on full-regression roll-up.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | Fresh local canonical gate completed app/test-target build until the shared compile failure. |
| Core user flow runtime-validated | `Fail` | No Proposal 025 assertions executed on the current tree. |
| Empty/loading/error states covered | `Not Checked` | Fresh execution did not reach MCP assertions. |
| Critical tests executed | `Fail` | Canonical same-tree `proposal-025` gate is red before test execution. |
| Full regression suite passed on same tree/HEAD | `Not Run` | Not attempted after the red focused gate. |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `./scripts/test-gate.sh proposal-025`
- `sed -n '1,220p' 'Chainworks ForgeTests/Proposal026Tests.swift'`
- `sed -n '260,320p' 'Chainworks ForgeTests/Proposal026Tests.swift'`
- `sed -n '1,220p' 'Chainworks Forge/Providers/BackendProfileResolverV2.swift'`
- `sed -n '1,120p' 'Chainworks Forge/Engine/RunStartOverrideResolver.swift'`
- `sed -n '1,120p' 'Chainworks Forge/DSL/AgentCatalog.swift'`

## Recommended Next Actions

1. Repair the shared compile drift in `Chainworks ForgeTests/Proposal026Tests.swift`.
2. Rerun `./scripts/test-gate.sh proposal-025` on the same tree once the test target is green again.
3. Only after the canonical gate passes, reassess whether a same-tree `full` regression run is required for successful roll-up.
