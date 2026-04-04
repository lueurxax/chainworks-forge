# Proposal 025 Implementation Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md` |
| Proposal MD5 | `ec820bc41594781712a416ba2571a432` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `2de983d` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-03T23:43:23+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P025`'s proposal-owned implementation contract is now functionally complete on the current tree. Fresh canonical `proposal-025` proof passed locally in `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260403-234514.xcresult` with `46` tests across `3` suites, and the old `R2` burn-telemetry gap is closed: the live execution row, KPI exporter, and report lane now carry startup latency, per-server tool/byte usage, prompt-context delta, blocked-run count, and zero-MCP counts. Track 1 therefore lands with all in-scope `REQ-*` items implemented.

The audit still fails closed overall. Under the current audit skill, any successful roll-up requires a passing same-tree full regression run. I synced this exact dirty tree to the approved host and launched `./scripts/test-gate.sh full`, but the run went red before completion. The live failure basis is broader repo portability, not a live `P025` contract hole: the remote same-tree full run failed multiple tests because example catalogs, preview fallbacks, and fixture helpers still hardcode `/Users/user/...` paths for external skills and repo fixtures. Because same-tree full regression is red, `Overall Conformance` cannot be reported as `Implemented` and `Overall Readiness` cannot be reported as successful.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | All proposal requirements are implemented, but successful roll-up fails closed without a passing same-tree full regression run | `High` |
| Architecture | `At Risk` | Repo/runtime portability still depends on workstation-specific absolute paths outside the proposal-owned MCP slice | `High` |
| Product | `At Risk` | Canonical example and preview-backed flows cannot be replayed reliably on the approved host | `High` |
| UI | `Acceptable` | Existing shell-owned report/comparison surfaces expose the MCP contract and telemetry promised by the proposal | `Medium` |
| UX | `Acceptable` | Preflight and runtime settlement remain explicit and fail closed on current evidence | `Medium` |
| Readiness | `Not Ready` | Same-tree full regression on the approved host is red | `High` |

## Proposal Contract

### Scope

- Repo-owned `mcp_server_registry`
- Per-agent `mcp_profile`
- Goose session reconciliation before prompt submission
- Preflight validation of registry truth, requested profile, and runtime capability truth
- Persisted requested / predicted / actual / denied MCP truth
- MCP burn telemetry in the existing run-owned KPI / report lane

### Locked Decisions

- Default deny / zero-MCP baseline
- `agents.*.mcp_profile` is runtime authority
- Permission-profile MCP is legacy metadata or ceiling-only, never a widening source
- Requested, predicted, actual, and denied MCP truths stay separately inspectable
- Burn telemetry extends the canonical run-owned KPI / report lane rather than a second metrics blob

### Primary User Flows

1. Declare explicit per-agent MCP intent in catalog YAML.
2. Run preflight and see whether the chosen runtime can honor that MCP contract.
3. Launch a Goose-backed session that reconciles extensions before the first prompt.
4. Inspect persisted requested, predicted, actual, denied, and burn-telemetry truths in the shell-owned reporting surfaces.

### UI Commitments

- Diagnostics show requested, predicted, actual, and denied MCP state
- Post-run readers use existing report / comparison surfaces rather than a side metrics blob

### UX Commitments

- Preflight fails closed when required MCP cannot be honored
- Empty MCP policy produces genuinely MCP-free sessions
- Operators can inspect the settled MCP contract after execution

### Acceptance Criteria

`AC1` explicit `mcp_profile`; `AC2` registry truth separate from machine-local runtime capability; `AC3` `mcp_profile` is runtime authority; `AC4` Goose honors policy before prompt submission; `AC5` preflight distinguishes installed / requested / predicted; `AC6` actual reconciled state persists on the execution truth path; `AC7` diagnostics show requested / predicted / actual / denied; `AC8` telemetry extends the run-owned KPI / report lane; `AC9` preflight fails when required MCP cannot be honored; `AC10` empty policy yields MCP-free sessions; `AC11` burn telemetry shows whether tightening MCP policy reduced overhead / tool chatter.

### Test / Evidence Requirements

- Persisted actual enabled MCP state on `AgentExecution`
- Post-run report / comparison readers show requested / predicted / actual / denied MCP truth
- KPI / report lane carries MCP telemetry
- Same-tree execution evidence for the focused proposal slice

### Explicit Exclusions

- Plugin marketplace / arbitrary extension authoring
- Interactive MCP policy editor
- Non-Goose runtime implementation beyond capability hooks
- Replacing existing provider / model validation logic

## Proposal Fidelity / Divergence

### Matches

- Catalog-level `mcp_server_registry`, `mcp_profiles`, and per-agent `mcp_profile` wiring is live.
- Run-start state freezes predicted MCP policy into immutable run truth.
- Goose-backed execution reconciles session extensions before prompt submission and persists settled runtime state.
- Report / comparison readers expose requested, predicted, actual, and denied MCP truth from persisted run / execution data.
- The run-owned KPI lane now carries the burn-telemetry fields promised by `§5.7`.
- Canonical `proposal-025` proof is green on the audited tree.

### Divergences

- No live proposal-owned functional divergence remains in the focused `P025` slice.
- The broader repository still fails same-tree sign-off because external-skill and fixture paths are not portable across approved-host environments.

### Ambiguities / Evidence Gaps

- The approved-host full gate was interrupted only after it had already gone red. The failure basis is sufficient and fresh, but the resulting bundle at `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/full-20260403-234820.xcresult` is incomplete rather than a clean finished artifact.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Explicit per-agent `mcp_profile`

- Proposal Source: `§9` AC1
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift:11-13`
  - `Chainworks Forge/DSL/AgentCatalog.swift:93-101`
  - `examples/agents/agents.yaml:71-82`
- Gap / Note: Agent-level MCP intent is first-class in the catalog schema and live example.

### REQ-002 Registry truth stays separate from machine-local runtime capability truth

- Proposal Source: `§5.1`, `§7`, `§9` AC2
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift:11-13`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:133-149`
  - `Chainworks Forge/Engine/PreflightService.swift:548-582`
- Gap / Note: Repo YAML owns server mapping while machine-local runtime answers stay in runtime / preflight logic.

### REQ-003 `mcp_profile` is runtime authority

- Proposal Source: `§5.2`, `§9` AC3
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/DSL/AgentCatalog.swift:98-100`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:108-145`
  - `Chainworks Forge/DSL/YAMLValidator.swift:155-205`
- Gap / Note: Resolution starts from `agents.*.mcp_profile`; legacy permission-profile MCP no longer widens runtime truth.

### REQ-004 Goose-backed sessions honor MCP policy before prompt submission

- Proposal Source: `§5.4`, `§9` AC4
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:66-115`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:137-239`
  - `Chainworks ForgeTests/GooseServerTransportTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260403-234514.xcresult`
- Gap / Note: Fresh canonical focused proof passed on the current tree.

### REQ-005 Preflight distinguishes registry truth, requested profile, and predicted effective set

- Proposal Source: `§5.5`, `§9` AC5
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:548-619`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:191-203`
  - `Chainworks Forge/Engine/RunStartSnapshot.swift:5-12`
  - `Chainworks Forge/Engine/RunStartSnapshot.swift:74-80`
- Gap / Note: Requested MCP truth is frozen at run start, while preflight exposes predicted settlement separately.

### REQ-006 Actual reconciled enabled MCP state persists on the execution truth path

- Proposal Source: `§8.2` step 6, `§9` AC6
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/AgentExecutor.swift:150-161`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:220-239`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:361-373`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:556-561`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2969-2974`
- Gap / Note: The execution row now stores actual enabled runtime IDs, denied extensions, startup latency, and per-server metrics.

### REQ-007 Diagnostics show requested, predicted, actual, and denied MCP state

- Proposal Source: `§5.6`, `§9` AC7
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:151-179`
  - `Chainworks Forge/Engine/RunComparisonService.swift:156-174`
  - `Chainworks Forge/Views/RunComparisonView.swift:364-386`
  - `Chainworks ForgeTests/Proposal025Tests.swift:9-105`
  - `Chainworks ForgeTests/Proposal025Tests.swift:107-206`
- Gap / Note: The shell-owned post-run readers now consume persisted MCP truth instead of reconstructing it heuristically.

### REQ-008 MCP telemetry extends the existing run-owned KPI/report lane

- Proposal Source: `§5.7`, `§9` AC8
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/Run.swift:76-81`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2835-2839`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:31-45`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:521-547`
  - `Chainworks ForgeTests/Proposal025Tests.swift:64-92`
- Gap / Note: MCP telemetry is now exported through the canonical run-owned KPI JSON and report payload.

### REQ-009 Preflight fails when required MCP cannot be honored

- Proposal Source: `§5.3`, `§5.5`, `§9` AC9
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:144-171`
  - `Chainworks Forge/Engine/PreflightService.swift:598-607`
- Gap / Note: Required missing extensions remain blocking issues; optional ones degrade to warnings only when policy allows it.

### REQ-010 Empty MCP policy yields genuinely MCP-free sessions

- Proposal Source: `§4`, `§8.1`, `§9` AC10
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:113-115`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:220-239`
  - `Chainworks ForgeTests/Proposal025Tests.swift:95-105`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260403-234514.xcresult`
- Gap / Note: Zero-MCP is both a declared policy and a measured runtime outcome.

### REQ-011 Burn telemetry shows whether tightening MCP policy reduced session overhead or tool chatter

- Proposal Source: `§5.7`, `§9` AC11
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift:61-66`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2969-2974`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:47-79`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:275-349`
  - `Chainworks Forge/Engine/SessionReuseKPIExporter.swift:382-420`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:521-547`
  - `Chainworks ForgeTests/Proposal025Tests.swift:9-105`
- Gap / Note: The live contract now covers startup latency, per-server tool-call and byte usage, prompt-context delta, blocked-run count, and zero-MCP counts.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 Host-specific path assumptions still leak into canonical runtime inputs

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-001`, `REQ-004`, `REQ-005`, `REQ-007`
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `examples/agents/agents.yaml:71-82`
  - `examples/agents/agents_mcp_profiles_v2.yaml:71-82`
  - `Chainworks Forge/Support/PreviewSupport.swift:626-649`
  - `Chainworks ForgeTests/Chainworks_ForgeTests.swift:59-64`
  - approved-host same-tree `./scripts/test-gate.sh full` live output
- Why It Matters: The proposal-owned MCP contract is implemented, but the surrounding repository still hardcodes workstation-specific roots for skills and fixtures. That makes canonical remote replay dependent on one developer machine layout rather than the audited tree alone.
- Recommended Action: Remove `/Users/user/...` assumptions from example catalogs, preview fallback resolution, and fixture helpers; prefer repo-relative or home-relative resolution with test-localized overrides.

## Product Review

**Summary:** `At Risk`

### PROD-001 Canonical example flows are not portable enough for approved-host replay

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: Primary flows 1-4
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `examples/agents/agents.yaml:71-82`
  - `examples/agents/agents_mcp_profiles_v2.yaml:71-82`
  - approved-host same-tree full-gate failures in `ResumeManagerTests` and `Proposal022Tests` due `.skillResolutionFailed(... /Users/user/.codex/skills/proposal-review-triad)`
- Why It Matters: Operators cannot rely on the repo’s shipped examples and preview-backed harnesses to reproduce behavior on the approved host, which weakens the end-to-end value of the implementation beyond the narrow `P025` slice.
- Recommended Action: Make shipped examples self-contained or host-portable before treating remote sign-off as trustworthy.

## UI Review

**Summary:** `Acceptable`

No material proposal-owned UI finding remains on current evidence. The MCP contract and telemetry promised by `P025` are rendered through the existing shell-owned report / comparison surfaces rather than a parallel UI lane.

## UX Review

**Summary:** `Acceptable`

No material proposal-owned UX finding remains on current evidence. Requested vs predicted vs actual MCP state is explicit, and the runtime still fail-closes when required MCP cannot be honored.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Same-tree full regression on the approved host is red

- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: Roll-up gate for successful verdicts
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - local focused gate: `./scripts/test-gate.sh proposal-025`
  - focused bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260403-234514.xcresult`
  - approved-host sync root: `/Users/test/chainworks-audit-2de983d-p025`
  - approved-host full command: `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-p025' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD='***' ./scripts/test-gate.sh full"`
  - live full-gate output: `Test run with 116 tests in 26 suites failed after 3.728 seconds with 15 issues`
  - incomplete red bundle path: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/full-20260403-234820.xcresult`
- Why It Matters: The updated audit skill explicitly forbids a successful audit verdict without passing same-tree full regression. That bar is not met here.
- Recommended Action: Fix the host-portability failures first, then rerun the canonical approved-host `full` gate on the same tree.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | Local `proposal-025` gate built and passed; approved-host `full` build progressed into unit and UI test execution |
| Core user flow runtime-validated | Pass | `./scripts/test-gate.sh proposal-025` passed `46` tests in `3` suites |
| Empty/loading/error states covered | Partial | Proposal-owned preflight / reconciliation failure handling is exercised; broader app-state coverage is not the limiting factor here |
| Accessibility risk acceptable | Partial | Existing UI surfaces stayed reachable in the focused slice, but no new accessibility audit was run in this pass |
| Localization risk acceptable | Not Checked | Not in scope for this audit pass |
| Critical tests executed | Pass | Focused `proposal-025` gate plus approved-host same-tree `full` gate were both executed |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail | Approved-host same-tree `full` went red with 15 issues before completion |
| Privacy/permissions/entitlements reviewed | Not Checked | Not part of the proposal-owned acceptance contract |

## Verification Log

- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md'`
- `git rev-parse --show-toplevel && git rev-parse --short HEAD && git status --short`
- `date '+%Y-%m-%dT%H:%M:%S%z' && md5 -q 'docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md'`
- `rg -n "superseded|deprecated|replaced by|obsolete" -S 'docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md' docs/proposals docs/reviews docs/reference`
- targeted code reads with `sed`, `nl`, and `rg` across MCP runtime, preflight, KPI, report, comparison, model, example, preview, and test surfaces
- `./scripts/test-gate.sh proposal-025`
- `ssh -o BatchMode=yes test@SMacBook.local 'hostname && pwd'`
- `ssh -o BatchMode=yes test@SMacBook.local "xcodebuild -version && xcode-select -p"`
- `tar czf - -C '/Users/user/Documents/Chainworks Forge' --exclude .git --exclude .codex . | ssh test@SMacBook.local "rm -rf '/Users/test/chainworks-audit-2de983d-p025' && mkdir -p '/Users/test/chainworks-audit-2de983d-p025' && tar xzf - -C '/Users/test/chainworks-audit-2de983d-p025'"`
- `tar czf - -C '/Users/user/.codex/skills' proposal-review-triad proposal-implementation-audit | ssh test@SMacBook.local "mkdir -p '/Users/test/.codex/skills' && tar xzf - -C '/Users/test/.codex/skills'"`
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-audit-2de983d-p025' && CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD='***' ./scripts/test-gate.sh full"`
- `ssh test@SMacBook.local "pkill -f '/Users/test/chainworks-audit-2de983d-p025' || true; pkill -f 'xcodebuild -project /Users/test/chainworks-audit-2de983d-p025/Chainworks Forge.xcodeproj' || true"`

## Recommended Next Actions

1. Remove `/Users/user/...` absolute skill and fixture roots from shipped examples, preview fallbacks, and test helpers so same-tree approved-host replay no longer depends on one workstation layout.
2. Rerun the approved-host canonical `full` gate on the same audited tree after those portability fixes land.
3. Only after the full gate is green, rerun this implementation audit to unlock a successful roll-up.
