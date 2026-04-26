# Proposal 037: ACP Execution Supervision and Idle-Hang Watchdog Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md` |
| Repository Root | `.` |
| Git SHA | `d3c5e22` |
| Working Tree | dirty (many modified files already present before this audit pass) |
| Audited At | `2026-04-11T15:52:35+0300` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 037's implementation contract is now materially landed on the current tree: the ACP-wide watchdog thresholds, persistent `supervisionClassification`, session-generation invalidation on watchdog failure, same-stage automatic retry lineage, and report/recovery/timeline truth are all implemented and backed by a passing same-tree `proposal-037` proof lane. This audit still cannot issue a successful roll-up because the repository's canonical full regression gate was not available on the audited tree/host: the repo defines `full` as the authoritative remote UI sign-off gate, and the local invocation failed immediately while app/test processes were already running. Per audit policy, that forces a fail-closed downgrade even though the proposal requirements themselves are implemented.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Successful verdict blocked by unavailable same-tree full regression proof | High |
| Architecture | Acceptable | No active proposal-specific architecture gap found on this tree | High |
| Product | Acceptable | Core watchdog/recovery operator flow is implemented and covered by executed proof | Medium |
| UI | Acceptable | Timeline/report surfaces are proved through tests rather than a live app walkthrough | Medium |
| UX | Acceptable | Recovery and retry semantics are explicit, but not runtime-walked in the app shell | Medium |
| Readiness | Not Ready | Canonical `full` regression gate not completed on this tree/host | High |

## Proposal Contract

### Scope

- Introduce one ACP-only supervision contract for idle hangs and mutation-integrity failures.
- Perform one automatic fresh retry for watchdog-triggered failures.
- Surface deterministic supervision truth through receipts, reports, recovery, and operator-facing history.

### Locked Decisions

- All ACP runtime families share the same supervision model.
- Watchdog reasons refine execution truth through `supervisionClassification`, not new canonical-outcome enum cases.
- The first automatic retry stays inside the same `StageExecution` as a new `AgentExecution`.
- Automatic retry count is exactly one.
- Mutating-tool success is not durable progress without a verified filesystem side effect.

### Primary User Flows

1. An ACP execution with no meaningful progress fails early under a watchdog-specific classification instead of drifting to the coarse global timeout.
2. A watchdog-triggered failure automatically retries once inside the same stage and either succeeds or settles into explicit recoverable supervision truth.
3. Operators inspecting recovery, reports, and timeline/history can see the watchdog reason and retry consumption without reconstructing it from transport logs.

### UI Commitments

- Recovery/report/timeline surfaces must show supervision-specific reasons.
- Successful auto-retries should remain historical truth rather than false blocked truth.

### UX Commitments

- Recovery wording must stay deterministic and supervision-specific.
- Same-run retry lineage must be clear before broader clone-run actions.

### Acceptance Criteria

- ACP-wide supervision parity.
- Thresholds: `120s` first progress, `300s` idle-after-progress, `120s` weak read-loop, `120s` first-edit silence, `30s` mutating-side-effect verification.
- Exactly one automatic fresh retry.
- Retry invalidates stale session state and creates a new session.
- Retry exhaustion yields explicit supervision truth rather than generic timeout.
- Reports/recovery surfaces show watchdog-specific reasons.
- Successful auto-retry keeps the run alive without leaving false blocked truth.
- No ACP execution remains indefinitely `running` after supervision silence or false mutating success.

### Test / Evidence Requirements

- Repo-owned `proposal-037` proof lane.
- Focused suites covering watchdog classification, same-stage retry lineage, recovery/report truth, and timeline persistence.
- Same-tree full regression/canonical full gate is required for a successful audit verdict.

### Explicit Exclusions

- No new watchdog cases added to `AgentCanonicalOutcome`.
- No hidden executor-local retries.
- No new `StageExecution` for the first watchdog retry.

## Proposal Fidelity / Divergence

### Matches

- ACP-wide watchdog thresholds and classifications are implemented in the runtime executor.
- Session-generation invalidation now occurs on watchdog-classified failures.
- Automatic watchdog retry is durably represented as same-stage `AgentExecution` lineage.
- Recovery and report readers prefer supervision truth over generic timeout wording.
- Persisted timeline/history surfaces preserve watchdog retry truth.
- Repo-owned `proposal-037` proof lane exists and passes on this tree.

### Divergences

- None found at the proposal-contract level on the audited tree.

### Ambiguities / Evidence Gaps

- No live app walkthrough was performed for operator UI surfaces; UI proof in this audit comes from executed tests and code inspection.
- The repository's canonical `full` regression gate was not completed on this tree/host, so successful audit statuses remain unavailable by policy.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 13 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 ACP-wide supervision contract
- Proposal Source: Scope / Goal; §6; AC-1
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1031`
  - `Chainworks Forge/Engine/ExecutionEventBridge.swift:430`
  - `scripts/test-gate.sh proposal-037` -> `114 tests in 9 suites passed`
- Gap / Note: ACP watchdog monitoring is enabled off runtime namespace presence, and the classifier is shared rather than provider-specific.

### REQ-002 No-first-progress ACP executions fail at 120s
- Proposal Source: §6.1; AC-2
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:70`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:885`
  - `Chainworks ForgeTests/RuntimeAgentExecutorTests.swift:3167`
  - `scripts/test-gate.sh proposal-037` -> `TEST SUCCEEDED`
- Gap / Note: None.

### REQ-003 Early-progress silence fails at 300s
- Proposal Source: §6.2; AC-3
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:71`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:945`
  - `scripts/test-gate.sh proposal-037` -> `TEST SUCCEEDED`
- Gap / Note: None.

### REQ-004 Weak read-loop churn fails under the 120s weak-progress policy
- Proposal Source: §6.3; AC-4
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:72`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:899`
  - `Chainworks ForgeTests/RuntimeAgentExecutorTests.swift:2142`
  - `scripts/test-gate.sh proposal-037` -> `ACP proposal reviewer read-loop stall fails early with durable failure evidence`
- Gap / Note: None.

### REQ-005 First-edit silence fails as `idle_hang_after_first_edit`
- Proposal Source: §6.5; AC-5
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:73`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:932`
  - `Chainworks ForgeTests/OrchestratorTests.swift:630`
  - `scripts/test-gate.sh proposal-037` -> `Sequential watchdog failures create durable same-stage retry lineage before succeeding`
- Gap / Note: None.

### REQ-006 Mutating-tool success without filesystem delta fails as `mutation_side_effect_missing`
- Proposal Source: §5.5; §6.6; AC-6
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:74`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:913`
  - `Chainworks ForgeTests/RuntimeAgentExecutorTests.swift:3347`
  - `scripts/test-gate.sh proposal-037` -> `TEST SUCCEEDED`
- Gap / Note: None.

### REQ-007 The first watchdog or mutation-integrity failure triggers exactly one automatic fresh retry
- Proposal Source: §7.1; AC-7
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:1092`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2183`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2202`
  - `Chainworks ForgeTests/OrchestratorTests.swift:630`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift:367`
- Gap / Note: None.

### REQ-008 The retry invalidates old session state and creates a new session
- Proposal Source: §7.2; §11.2 slice 2; AC-8
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1161`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1172`
  - `Chainworks Forge/Engine/AgentSessionManager.swift:165`
  - `Chainworks ForgeTests/RuntimeAgentExecutorTests.swift:3322`
  - `scripts/test-gate.sh proposal-037` -> `TEST SUCCEEDED`
- Gap / Note: Executed proof now verifies that a watchdog-failed generation becomes `.invalidated`, clears `activeGenerationID`, and records the invalidation event.

### REQ-009 Retry exhaustion produces explicit supervision failure truth rather than generic timeout
- Proposal Source: §7.4; AC-9
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:27`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:417`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift:550`
  - `Chainworks ForgeTests/Proposal013Tests.swift:2238`
- Gap / Note: None.

### REQ-010 `supervisionClassification` is the durable refinement field rather than a new canonical outcome
- Proposal Source: §8.1-§8.3
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift:53`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:3524`
  - `docs/reference/execution-truth-and-recovery.md:93`
  - `Chainworks ForgeTests/Proposal013Tests.swift:1916`
- Gap / Note: None.

### REQ-011 Reports and recovery surfaces show supervision-specific reasons
- Proposal Source: §8.4; §9; AC-10
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:273`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:379`
  - `Chainworks ForgeTests/Proposal013Tests.swift:2067`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift:550`
- Gap / Note: None.

### REQ-012 Successful auto-retry keeps the run alive and does not leave false blocked truth behind
- Proposal Source: §9.1-§9.3; AC-11
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2202`
  - `Chainworks ForgeTests/OrchestratorTests.swift:630`
  - `Chainworks ForgeTests/WorkflowMapProjectionTests.swift:124`
  - `Chainworks ForgeTests/WorkflowMapProjectionTests.swift:175`
- Gap / Note: None.

### REQ-013 Repo-owned `proposal-037` proof lane exists and maps to the watchdog slice
- Proposal Source: §10.3; §11.1; Test / Evidence Requirements
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `docs/reference/test-gates.md:427`
  - `scripts/test-gate.sh:1436`
  - `bash 'scripts/test-gate.sh' proposal-037` -> `114 tests in 9 suites passed`
- Gap / Note: The focused lane is present and aligned with the proposal-owned slice.

## Architecture Review

**Summary:** Acceptable

No additional architecture finding remains after the watchdog invalidation path landed on this tree.

## Product Review

**Summary:** Acceptable

No additional product-specific finding remains. The primary operator job promised by the proposal is implemented and covered by executed proof.

## UI Review

**Summary:** Acceptable

No separate UI-specific blocker found. Timeline/report visibility is proved through focused tests rather than a live app walkthrough.

## UX Review

**Summary:** Acceptable

No separate UX-specific blocker found. Recovery and retry semantics are explicit in the persisted/read-model surfaces validated by the gate.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Successful audit is blocked by unavailable same-tree canonical full regression proof
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Test / Evidence Requirements; REQ-013
- Evidence Type: `tests-run`, `code`
- Evidence:
  - `scripts/test-gate.sh proposal-037` -> `114 tests in 9 suites passed`
  - `docs/reference/test-gates.md:465`
  - `scripts/test-gate.sh:1447`
  - `bash 'scripts/test-gate.sh' full` -> `Refusing to start gate while test/app processes are already running`
- Why It Matters: The skill's audit policy does not allow a successful roll-up from focused proof alone. This repository defines `full` as the canonical sign-off regression gate and explicitly treats it as remote-only. On the audited host/tree, the local invocation did not reach execution because active app/test processes were already present, so the required same-tree full-regression evidence was unavailable.
- Recommended Action: Re-run the audit on an idle host that can execute the canonical `full` lane, or run the documented remote `full` gate on the same tree/HEAD and attach that result before asking for a successful implementation verdict.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `proposal-037` gate built successfully on macOS |
| Core user flow runtime-validated | Partial | Validated through executed XCTest/fixture flows rather than a live app walkthrough |
| Empty/loading/error states covered | Partial | Recovery/report/timeline and failure settlement are covered in focused tests |
| Accessibility risk acceptable | Not Checked | Not part of this focused runtime audit |
| Localization risk acceptable | Not Checked | No localization-specific verification in this audit |
| Critical tests executed | Pass | `proposal-037` gate passed with `114 tests in 9 suites` |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail | Local `full` invocation did not complete; canonical `full` is remote-only and the local host was not idle |
| Privacy/permissions/entitlements reviewed | Not Checked | Outside the scope of this watchdog-focused audit |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md`
- `git rev-parse HEAD`
- `git status --short`
- `rg -n "handleStreamFailure|invalidateGeneration|closeGeneration|proposal-037|full" ...`
- `bash 'scripts/test-gate.sh' proposal-037`
  - result: `TEST SUCCEEDED`
  - detail: `114 tests in 9 suites passed after 4.763 seconds`
- `bash 'scripts/test-gate.sh' full`
  - result: failed closed before execution
  - detail: `Refusing to start gate while test/app processes are already running`

## Recommended Next Actions

1. Run the canonical `full` regression gate on an idle same-tree host, preferably using the repo's documented remote form if that is the only supported path.
2. Re-run this implementation audit after the same-tree `full` gate passes.
3. If a live operator UI proof is required for sign-off, add it after the full gate rather than replacing the gate with ad hoc targeted evidence.
