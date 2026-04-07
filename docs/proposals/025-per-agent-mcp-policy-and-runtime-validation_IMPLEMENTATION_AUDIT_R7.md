# Proposal 025: Per-Agent MCP Policy and Runtime Validation Multi-Lens Audit R7

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `a0bb075` |
| Working Tree | `Clean` |
| Audited At | `2026-04-07T11:30:43+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P025` is no longer blocked by the old direct-`Run` guard finding or by the old shared Goose compile break. The fresh same-tree MCP slice now builds and executes, but it is still red: `Proposal025Tests` + Goose transport/session proof ran `36` tests and failed one proposal-owned assertion, `preferredExampleURLPrefersRepositoryCopy()`. The failure shows that portable repo-copy resolution now falls through to the bundled fallback instead of preferring the repository copy on the current tree. The synced approved-host `full` gate is also unavailable because `scripts/test-gate.sh` aborts with `signing_args[@]: unbound variable`, so the audit cannot roll up to success even beyond the live proposal failure.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | One fresh proposal-owned MCP proof assertion is failing | `High` |
| Architecture | `At Risk` | Portable example/source resolution drift now breaks proof-host independence | `High` |
| Product | `At Risk` | Runtime proof portability regressed on the current tree | `High` |
| UI | `Acceptable` | Shell-owned report/comparison readers still encode requested / actual / denied MCP truth | `Medium` |
| UX | `Acceptable` | Requested vs settled MCP truth is still explicit in code and tests | `Medium` |
| Readiness | `Not Ready` | Proposal slice is red and same-tree full regression is unavailable | `High` |

## Proposal Contract

### Scope

- Repo-owned `mcp_profile` mapping in the agent catalog.
- Separation of requested MCP intent, runtime capability, and settled session truth.
- Goose-side reconciliation before prompt submission.
- Persisted requested / actual / denied MCP truth on execution records.
- Burn telemetry in the canonical run-owned KPI/report lane.

### Locked Decisions

- Default deny / zero-MCP baseline.
- `agent.mcp_profile` is the runtime authority.
- Requested, predicted, actual, and denied MCP truth remain separately inspectable.
- Burn telemetry extends the existing run-owned KPI/report spine.

### Primary User Flows

1. Declare per-agent MCP intent in YAML.
2. Run preflight and inspect requested vs predicted capability truth.
3. Launch a Goose-backed session that reconciles extensions before prompt submission.
4. Inspect requested / predicted / actual / denied / telemetry truth from shell-owned readers.

### UI Commitments

- Diagnostics show selected `mcp_profile` plus requested / predicted / actual / denied state.
- Existing report/comparison readers expose MCP truth and telemetry.
- Empty MCP policy yields genuinely MCP-free sessions.

### UX Commitments

- Required MCP failure is explicit and fail-closed.
- Operators can distinguish repo intent from machine/runtime settlement.

### Acceptance Criteria

- Explicit per-agent `mcp_profile`.
- Runtime capability stays separate from repo registry truth.
- Goose reconciliation happens before prompt submission.
- Actual enabled MCP truth persists on execution rows.
- Canonical KPI/report lane carries MCP-burn telemetry.

### Test / Evidence Requirements

- Focused proposal proof for MCP resolution, reconciliation, persistence, and telemetry.
- Passing same-tree `full` regression for any successful audit.

### Explicit Exclusions

- No second MCP metrics blob outside the canonical run-owned KPI/report lane.
- No widening from legacy permission metadata.

## Proposal Fidelity / Divergence

### Matches

- `mcp_profiles`, `agent.mcp_profile`, `MCPPolicyRuntime`, `WorkflowOrchestrator`, `RunReportBuilder`, and `SessionReuseKPIExporter` remain present and wired.
- The fresh focused slice still proves most MCP runtime/report behavior.
- The old direct-`Run` guard blocker is gone from `ArtifactInspectorView.swift`.

### Divergences

- The fresh focused slice now fails one portability-sensitive proposal-owned assertion in `Proposal025Tests`.
- The canonical `full` gate on the synced approved host aborts before a usable regression run.

### Ambiguities / Evidence Gaps

- No passing same-tree `full` regression evidence exists because the canonical `full` gate is currently broken.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Per-agent `mcp_profile` intent remains explicit and runtime-authoritative
- Proposal Source: `§5.2`, `§6`, `§9 AC1-AC3`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/DSL/YAMLValidator.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - focused result bundle: `/tmp/p025-audit.btMmG2/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-52-+0300.xcresult`
- Gap / Note: Catalog and runtime authority wiring are present and exercised in the fresh slice.

### REQ-002 Requested, predicted, actual, and denied MCP truth stay separately inspectable
- Proposal Source: `§5.5-§5.6`, `§7`, `§9 AC4-AC7`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - focused result bundle: `/tmp/p025-audit.btMmG2/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-52-+0300.xcresult`
- Gap / Note: The fresh focused slice passes the report/comparison MCP truth checks except for the separate portability test below.

### REQ-003 Goose reconciliation and MCP telemetry remain wired into the current runtime/report path
- Proposal Source: `§5.4`, `§5.7`, `§9 AC4`, `§9 AC8-AC11`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseServerTransport.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift`
  - focused result bundle: `/tmp/p025-audit.btMmG2/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-52-+0300.xcresult`
  - `xcrun xcresulttool get test-results summary --path '/tmp/p025-audit.btMmG2/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-52-+0300.xcresult'`
- Gap / Note: The fresh focused run passed `35` tests around Goose transport, session reconciliation, and report/KPI handling.

### REQ-004 Portable proposal proof prefers repo copies over bundled fallback when both exist
- Proposal Source: `§8`, `§9 proof expectations`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks ForgeTests/Proposal025Tests.swift:122-145`
  - `Chainworks Forge/Support/AppConfiguration.swift:185-229`
  - xcresult summary failure:
    `Proposal025Tests/preferredExampleURLPrefersRepositoryCopy()`
  - failure text:
    `resolved?.path == "/.../agents.bundle.yaml"` instead of `"/.../examples/agents/agents.yaml"`
- Gap / Note: `AppConfiguration.preferredExampleURL(...)` still appends the bundled fallback after source-root candidates, but it no longer treats the current portable repo copy as authoritative in the failing fixture shape.

### REQ-005 Canonical proposal-owned proof lane passes on the same tree
- Proposal Source: `§9`, `§10`
- Status: `Partially Implemented`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - local invocation: `./scripts/test-gate.sh proposal-025`
  - focused invocation:
    `xcodebuild -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p025-audit.btMmG2 test -only-testing:'Chainworks ForgeTests/Proposal025Tests' -only-testing:'Chainworks ForgeTests/GooseServerTransportTests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests'`
  - focused result bundle: `/tmp/p025-audit.btMmG2/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-52-+0300.xcresult`
- Gap / Note: The proposal slice executes on the current tree, but it is still red with one live failure, so the canonical proof lane is not passing.

### REQ-006 Same-tree successful full regression exists for a green audit roll-up
- Proposal Source: `Test / Evidence Requirements`
- Status: `Partially Implemented`
- Evidence Type: `runtime`
- Evidence:
  - synced approved-host invocation: `./scripts/test-gate.sh full`
  - output: `./scripts/test-gate.sh: line 563: signing_args[@]: unbound variable`
  - local script reference: `scripts/test-gate.sh:562-568`
- Gap / Note: The required same-tree full regression evidence is currently unavailable.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 Portable example-resolution logic drifted away from the repo-first proof contract
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-004`, `REQ-005`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Support/AppConfiguration.swift:185-229`
  - `Chainworks ForgeTests/Proposal025Tests.swift:122-145`
  - focused result bundle: `/tmp/p025-audit.btMmG2/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-52-+0300.xcresult`
- Why It Matters: `P025` depends on portable, same-tree proof. Once repo-copy preference regresses, the proof host can silently read bundled fallback inputs instead of the synced repo state.
- Recommended Action: Restore repo-copy preference in `preferredExampleURL(...)` when the requested repo-relative file exists beside the synced tree.

## Product Review

**Summary:** `At Risk`

### PROD-001 The MCP feature mostly works, but the portable proof contract is no longer trustworthy
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-004`, `REQ-005`
- Evidence Type: `tests-run`
- Evidence:
  - focused slice summary: `35 passed / 1 failed`
  - failing test: `Preferred example URL resolves repo copy before bundled fallback`
- Why It Matters: The user-facing MCP runtime path can appear healthy while the proof system is accidentally reading bundled fallbacks. That weakens trust in the proposal’s runtime-validation story.
- Recommended Action: Fix repo-first example resolution and rerun the focused MCP slice before re-auditing.

## UI Review

**Summary:** `Acceptable`

No fresh UI contradiction surfaced. Shell-owned report and comparison surfaces still expose the intended requested / actual / denied MCP truth.

## UX Review

**Summary:** `Acceptable`

No fresh UX contradiction surfaced. The remaining gap is proof portability and sign-off readiness, not operator interaction design.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Same-tree full regression is unavailable, so the audit cannot roll up green
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: `runtime`
- Evidence:
  - synced approved-host invocation: `./scripts/test-gate.sh full`
  - output: `./scripts/test-gate.sh: line 563: signing_args[@]: unbound variable`
- Why It Matters: The updated audit skill forbids successful verdicts without passing same-tree full regression. That proof is currently unavailable.
- Recommended Action: Fix the `full` gate shell expansion bug, then rerun the full regression on the same synced tree.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | Focused local MCP slice built and ran. |
| Core user flow runtime-validated | `Partial` | Most MCP proof tests passed, but one proposal-owned portability assertion failed. |
| Empty/loading/error states covered | `Partial` | Covered indirectly by proposal-owned tests and shell-owned report logic. |
| Accessibility risk acceptable | `Not Checked` | Not reassessed in this pass. |
| Localization risk acceptable | `Not Checked` | Not reassessed in this pass. |
| Critical tests executed | `Pass` | Focused local result bundle executed `36` tests with `1` live failure. |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail` | Synced approved-host `full` gate aborts before launching regression. |
| Privacy/permissions/entitlements reviewed | `Not Checked` | Not reassessed in this pass. |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py '/Users/user/Documents/Chainworks Forge/docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md'`
- `./scripts/test-gate.sh proposal-025`
- `xcodebuild -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p025-audit.btMmG2 test -only-testing:'Chainworks ForgeTests/Proposal025Tests' -only-testing:'Chainworks ForgeTests/GooseServerTransportTests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests'`
- `xcrun xcresulttool get test-results summary --path '/tmp/p025-audit.btMmG2/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-52-+0300.xcresult'`
- `rsync -az --delete --exclude='.git' --exclude='DerivedData' --exclude='.build' --exclude='*.xcresult' '/Users/user/Documents/Chainworks Forge/' 'test@SMacBook.local:/Users/test/chainworks-remote/'`
- `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh full'`

## Recommended Next Actions

1. Fix `AppConfiguration.preferredExampleURL(...)` so synced repo copies outrank bundled fallbacks in the failing fixture shape.
2. Rerun the local focused `Proposal025Tests` / Goose transport slice until it is green.
3. Fix the canonical `full` gate shell bug before attempting another successful audit roll-up.
