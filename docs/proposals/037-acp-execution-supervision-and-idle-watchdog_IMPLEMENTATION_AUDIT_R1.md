# Proposal 037: ACP Execution Supervision and Idle-Hang Watchdog Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md` |
| Repository Root | `.` |
| Git SHA | `d3c5e22` |
| Working Tree | dirty (many modified and untracked files already present before this report) |
| Audited At | `2026-04-11T15:42:54+0300` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 037 is substantially landed on the current tree: the ACP-wide watchdog thresholds, classification model, durable `supervisionClassification` persistence, same-stage automatic retry lineage, and report/recovery/timeline surfaces are all present and backed by a passing same-tree `proposal-037` proof lane. The remaining blocker is specific and architectural: the watchdog failure path does not durably invalidate the old `AgentSessionGeneration` before returning failure truth, even though §7.2 and acceptance criterion 8 require stale session state invalidation as part of the automatic fresh-retry contract. Because of that gap, and because no same-tree full regression gate was run, this audit cannot report a successful implementation verdict.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Fresh-retry contract does not fully settle stale session generation | High |
| Architecture | At Risk | Watchdog failure returns before generation invalidation/closure | High |
| Product | Acceptable | Core supervision/recovery flow is present, but stale session truth can drift from operator expectations | Medium |
| UI | Acceptable | Timeline/report surfaces are proved by tests, not live runtime validation | Medium |
| UX | Acceptable | Recovery wording is durable, but operator behavior was not runtime-walked in app UI | Medium |
| Readiness | Not Ready | Focused gate is green, but REQ-008 is still partial and no full regression gate was run | High |

## Proposal Contract

### Scope

- Introduce one ACP-only execution supervision contract for idle hangs and mutation-integrity failures.
- Perform one automatic fresh retry for watchdog-triggered failures.
- Persist deterministic supervision truth into receipts, reports, recovery, and operator-facing history.

### Locked Decisions

- ACP families share one supervision contract rather than provider-specific behavior.
- `idle_hang_*` reasons live in `supervisionClassification`, not `AgentCanonicalOutcome`.
- The first automatic watchdog retry stays inside the same `StageExecution` and creates a new `AgentExecution` lineage entry.
- Automatic retry count is exactly one.
- Mutating-tool success is not durable progress unless filesystem side effects are observed.

### Primary User Flows

1. An ACP execution that never makes meaningful progress fails early with a watchdog-specific classification instead of drifting to the coarse outer timeout.
2. A watchdog-failed ACP attempt automatically retries once inside the same stage, then either succeeds cleanly or settles into explicit recoverable failure truth.
3. Operators inspecting reports, recovery actions, and timeline/history can see the watchdog reason and retry consumption without reconstructing it from transport logs.

### UI Commitments

- Operator-facing report/recovery/timeline surfaces must expose watchdog-specific reasons.
- Successful auto-retry should remain historical truth, not false blocked truth.

### UX Commitments

- Deterministic recovery wording instead of generic timeout/interruption phrasing.
- Clear same-run retry lineage before broader clone-run actions.

### Acceptance Criteria

- ACP-wide supervision parity.
- Thresholds: `120s` first progress, `300s` idle after progress, `120s` weak read-loop, `120s` first-edit silence, `30s` mutation-side-effect verification.
- Exactly one automatic fresh retry.
- Retry invalidates old session state and creates a new session.
- Reports/recovery surfaces preserve watchdog-specific reasons.
- No ACP execution remains indefinitely `running` after supervision failure patterns.

### Test / Evidence Requirements

- Repo-owned focused gate `proposal-037`.
- Focused suites named in `docs/reference/test-gates.md` for watchdog behavior, lineage, report/recovery truth, and timeline persistence.
- Long-run/full regression only required to support a successful implementation verdict.

### Explicit Exclusions

- No new `AgentCanonicalOutcome` enum cases for watchdog reasons.
- No executor-local hidden retries.
- No stage-level retry for the first automatic watchdog retry.

## Proposal Fidelity / Divergence

### Matches

- ACP watchdog thresholds and classifications are implemented in `RuntimeAgentExecutor`.
- Shared weak/strong/mutating progress taxonomy is implemented and aligned with `ExecutionEventBridge`.
- `supervisionClassification` is persisted on `AgentExecution` and used by report/recovery readers.
- Automatic watchdog retry creates durable same-stage `AgentExecution` lineage through `StageRetryCoordinator`.
- Repo-owned `proposal-037` proof lane exists and passed on this tree.

### Divergences

- Watchdog-triggered failure does not durably invalidate the prior session generation before returning failure truth, even though §7.2 requires invalidation as part of fresh-retry semantics.

### Ambiguities / Evidence Gaps

- No same-tree full regression gate was executed, so a successful audit verdict is unavailable by policy.
- Operator UI/report/timeline behavior was proved through executed tests, not a live app walkthrough.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 ACP-wide supervision contract
- Proposal Source: Scope/Goal; §6; AC-1
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1031`
  - `Chainworks Forge/Engine/ExecutionEventBridge.swift:430`
  - `scripts/test-gate.sh proposal-037` -> `114 tests in 9 suites passed`
- Gap / Note: ACP watchdog monitoring is keyed from runtime namespace presence, while the progress classifier is shared rather than family-specific.

### REQ-002 No-first-progress executions fail at 120s
- Proposal Source: §6.1; AC-2
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:70`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:885`
  - `Chainworks ForgeTests/RuntimeAgentExecutorTests.swift:3167`
  - `scripts/test-gate.sh proposal-037` -> `TEST SUCCEEDED`
- Gap / Note: None.

### REQ-003 Early-progress then silence fails at 300s
- Proposal Source: §6.2; AC-3
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:71`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:945`
  - `scripts/test-gate.sh proposal-037` -> `TEST SUCCEEDED`
- Gap / Note: Direct code evidence is clear even though the focused test lane proves the broader watchdog suite rather than one named per-threshold case in the captured console output.

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
  - `Chainworks ForgeTests/RuntimeAgentExecutorTests.swift:3227`
  - `scripts/test-gate.sh proposal-037` -> `TEST SUCCEEDED`
- Gap / Note: None.

### REQ-007 First watchdog or mutation-integrity failure triggers exactly one automatic retry
- Proposal Source: §7.1; AC-7
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:1092`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2183`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2202`
  - `Chainworks ForgeTests/OrchestratorTests.swift:630`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift:367`
- Gap / Note: Automatic retry is correctly single-consumption at the stage/agent lineage layer.

### REQ-008 Automatic retry invalidates old session state and creates a new session
- Proposal Source: §7.2; §11.2 slice 2; AC-8
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1115`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1147`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1424`
  - `Chainworks Forge/Engine/AgentSessionManager.swift:165`
  - `Chainworks Forge/Engine/SessionReusePolicy.swift:31`
  - `scripts/test-gate.sh proposal-037` -> `114 tests in 9 suites passed`
- Gap / Note: The watchdog failure path closes the provider session, but it does not call `invalidateGeneration(...)` or `closeGeneration(...)` before returning failure truth unless the error also qualifies as session-missing or after-output transport failure. Because `settleCompletedGenerationIfNeeded(...)` is only called on the non-throwing completion path, watchdog-triggered failures can leave the old `AgentSessionGeneration` active in durable session truth even though the proposal requires explicit invalidation before the fresh retry.

### REQ-009 Watchdog retry is persisted as same-stage durable lineage
- Proposal Source: §7.3; AC-7; AC-11
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift:27`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2202`
  - `Chainworks ForgeTests/OrchestratorTests.swift:630`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift:317`
- Gap / Note: None.

### REQ-010 `supervisionClassification` is the durable refinement field, not a new canonical outcome
- Proposal Source: §8.1-§8.3; AC-9
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift:53`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:3524`
  - `docs/reference/execution-truth-and-recovery.md:93`
  - `Chainworks ForgeTests/Proposal013Tests.swift:1916`
- Gap / Note: None.

### REQ-011 Reports and recovery surfaces show supervision-specific reasons
- Proposal Source: §8.4; §9; AC-9; AC-10
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:273`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:379`
  - `Chainworks ForgeTests/Proposal013Tests.swift:2067`
  - `Chainworks ForgeTests/RecoveryCoordinatorTests.swift:550`
  - `scripts/test-gate.sh proposal-037` -> `TEST SUCCEEDED`
- Gap / Note: None.

### REQ-012 Timeline/history surfaces preserve watchdog retry truth and prevent false indefinite running
- Proposal Source: §9.3; §10.3; AC-11; AC-12
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/RunTimelineInspectorView.swift`
  - `Chainworks ForgeTests/WorkflowMapProjectionTests.swift:124`
  - `Chainworks ForgeTests/WorkflowMapProjectionTests.swift:175`
  - `Chainworks ForgeTests/RunTimelineInspectorViewTests.swift:1`
  - `scripts/test-gate.sh proposal-037` -> `Projection persists watchdog supervision and automatic retry history into the focused timeline data path`
- Gap / Note: The audit did not perform a live macOS walkthrough, but executed tests prove the persisted timeline path and focused view composition.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Watchdog failure leaves stale active generation in durable session truth
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: §7.2; §11.2 slice 2; REQ-008
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1115`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1147`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1424`
  - `Chainworks Forge/Engine/AgentSessionManager.swift:165`
  - `Chainworks Forge/Engine/SessionReusePolicy.swift:31`
  - `scripts/test-gate.sh proposal-037` -> `114 tests in 9 suites passed`
- Why It Matters: Proposal 037 promises both a fresh retry and explicit invalidation of the failed attempt's session state. The current code only closes the provider session on watchdog failure; it does not durably settle the old generation unless the failure also hits a narrower transport path. That leaves session lineage truth inconsistent with the proposal's retry contract and makes future session-forensics/reuse semantics harder to trust.
- Recommended Action: On watchdog-classified failure, invalidate or close the current `AgentSessionGeneration` before returning `AgentResult`, then add a focused test that asserts the failed watchdog attempt leaves no active generation behind and that the retry creates a new generation deterministically.

## Product Review

**Summary:** Acceptable

No additional product-specific finding beyond the conformance gap. The primary operator job is present: hangs classify early, retry lineage persists, and recovery/report wording is specific.

## UI Review

**Summary:** Acceptable

No new UI-specific defect was found. The audit relied on executed timeline/report tests rather than a live macOS walkthrough, so UI confidence is lower than the backend confidence.

## UX Review

**Summary:** Acceptable

No separate UX-specific blocker was found. Recovery messaging and retry history are explicit in the tested durable surfaces, but the audit did not runtime-walk operator interactions end-to-end.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Successful audit blocked by partial retry-settlement truth and focused-only proof
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-008; §10.3; §12
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `scripts/test-gate.sh proposal-037` -> `114 tests in 9 suites passed`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1115`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1424`
- Why It Matters: The focused proposal gate is green, which is strong evidence that the watchdog slice is materially landed. But the remaining session-settlement gap means the implementation is not yet faithful to the full proposal contract, and no same-tree full regression gate was run, so this audit cannot be promoted to a successful readiness verdict.
- Recommended Action: Fix REQ-008 first, then rerun `proposal-037` and the repository's canonical full regression gate on the same tree before seeking a green implementation audit.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `scripts/test-gate.sh proposal-037` built and ran the focused macOS test lane successfully |
| Core user flow runtime-validated | Partial | Core behavior was validated through executed XCTest/fixture flows, not a live app walkthrough |
| Empty/loading/error states covered | Partial | Failure/report/recovery/timeline states are covered in focused tests; no live operator UI pass |
| Accessibility risk acceptable | Not Checked | Proposal scope is runtime/recovery centric; no accessibility audit run here |
| Localization risk acceptable | Not Checked | No localization-specific verification run |
| Critical tests executed | Pass | `proposal-037` gate passed with `114 tests in 9 suites` |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | No full regression gate was executed in this audit |
| Privacy/permissions/entitlements reviewed | Not Checked | Not part of this focused runtime audit |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md`
- `git rev-parse HEAD`
- `git status --short`
- `rg -n "supervisionClassification|watchdog|proposal-037|automatic_watchdog_retry|invalidateGeneration|closeGeneration" ...`
- `bash '/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh' proposal-037`
  - result: `TEST SUCCEEDED`
  - detail: `114 tests in 9 suites passed after 5.919 seconds`

## Recommended Next Actions

1. Fix REQ-008 in `RuntimeAgentExecutor`: watchdog-classified failures must invalidate or close the current `AgentSessionGeneration` before the automatic retry path begins.
2. Add a focused test proving the stale watchdog generation is no longer active after failure and that the retry creates a new generation on the same lineage.
3. After the fix, rerun `./scripts/test-gate.sh proposal-037`.
4. If the focused gate stays green, run the repository's canonical full regression gate on the same tree to qualify for a successful implementation verdict.
