# Proposal 044: Post-Approval Task Execution and Release Gate Completion Multi-Lens Audit R6

| Field | Value |
|---|---|
| Proposal | `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md` |
| Repository Root | `.` |
| Git SHA | `db7d51aa91f71f898c4e621c01523708ca7d3c1b` |
| Working Tree | dirty |
| Audited At | `2026-04-15T21:49:01+03:00` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

P044 is mostly landed on this HEAD: `run_after_approval` ownership, N-phase sequencing, post-approval kickstart, end-state task execution, and the repo-owned `proposal-044` gate are all present and green on the audited tree. The remaining gap is explicit and proposal-owned: retrying a failed `state_11_manual_release` attempt still stops at a new `Pending` stage and relies on a later lazy-create path, while the proposal commits to a fresh-approval return to `WaitingApproval` on the retried release gate. That keeps conformance at `Partial` and readiness at `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | `REQ-008` is only partially implemented | High |
| Architecture | At Risk | Retry lifecycle can fork stage lineage instead of cleanly re-entering the retried gate | High |
| Product | At Risk | Failed release-gate recovery does not yet deterministically hand control back through fresh approval on the retried attempt | High |
| UI | Acceptable | No proposal-owned UI surface is in scope for this Rust control-plane slice | Low |
| UX | At Risk | Operator recovery after post-approval failure remains indirect and under-proven | Medium |
| Readiness | Not Ready | Same-tree gate is green, but one explicit acceptance criterion still diverges | High |

## Proposal Contract

### Scope

- Implement `run_after_approval` task enqueuing with correct effective-task ownership. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8-10`
- Generalize binary phase `0/1` settlement to strict N-phase ordering for `sequence` and multi-task `then`. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:76-118`
- Ensure end states with `run` blocks execute their tasks before run completion. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:293-330`

### Locked Decisions

- Approval on `state_11_manual_release` executes `run_after_approval` before transition evaluation. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:65-70`
- Retry after failed post-approval work on a manual release gate requires fresh approval and returns to `WaitingApproval`. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:67`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`
- Post-approval release tasks require a provisioned worktree. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:68`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:328-330`
- ACP-owned release execution is out of scope; proof must use fixture tasks only. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:69`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:426-430`
- `sequence` and multi-task `then` are strict-order constructs. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:91-118`

### Primary User Flows

1. Operator approves `state_11_manual_release`, and the stage remains active long enough to execute post-approval release work before transition.
2. The release gate executes `commit_and_push` before `build_and_distribute`, then transitions to `state_12_workflow_complete`.
3. `state_12_workflow_complete` runs `finalize_run_and_produce_receipts` before the run settles completed.
4. If post-approval release work fails, retrying the gate requires fresh human approval before release work resumes.
5. Existing simple manual gates and existing `parallel + then` stages continue to behave as before.

### UI Commitments

- None proposal-owned. P044 is an orchestration-only Rust control-plane slice. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:372-422`

### UX Commitments

- The operator-facing release approval is a real checkpoint, not a cosmetic pause. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:65-70`
- Retry after failed release work must force a fresh approval checkpoint. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`
- End-state receipts must exist before the run appears complete. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:293-330`

### Acceptance Criteria

- No regression on single-task `then`. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:347`
- Multi-task `then` ordering. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:348`
- No regression on simple gates. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:349`
- Sequential post-approval ordering for `state_11`. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:350`
- `state_11` happy path transitions to `state_12`. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:351`
- `state_12` runs finalization before run completion. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:352`
- Failed phases skip later phases and block the run. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:353`
- Retry requires re-approval. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:354`
- Worktree guard blocks missing worktree. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:355`
- `task_index` maps correctly to the effective task list. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:356`

### Test / Evidence Requirements

- The repo-owned proof lane is `./scripts/test-gate.sh proposal-044`. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:370-422`
- The gate is allowed to be the full Rust workspace suite, not just a focused subset. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:411-422`

### Explicit Exclusions

- Deterministic release services are deferred to P045. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:428`
- ACP-owned release execution is not permitted. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:429`
- Release receipt formatting is not part of this proposal. Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:430`

## Proposal Fidelity / Divergence

### Matches

- `run_after_approval` is compiled into `post_approval_tasks`. Evidence: `control-plane/crates/workflow/src/compiler.rs:304-307`
- `sequence` and multi-task `then` now compile to incrementing phases. Evidence: `control-plane/crates/workflow/src/compiler.rs:386-413`, `control-plane/crates/workflow/tests/integration.rs:161-248`
- `ApproveStage` keeps `state_11_manual_release` in `Running` when post-approval tasks exist. Evidence: `control-plane/crates/engine/src/command_handler.rs:202-241`, `control-plane/crates/engine/tests/integration.rs:1308-1360`
- The orchestrator resolves `effective_tasks`, kickstarts post-approval phase 0, advances phases generically, and completes `state_12` only after tasks finish. Evidence: `control-plane/crates/engine/src/orchestrator.rs:225-405`, `control-plane/crates/engine/src/orchestrator.rs:448-480`, `control-plane/crates/engine/src/orchestrator.rs:1359-1370`
- Worktree safety checks now include `post_approval_tasks`. Evidence: `control-plane/crates/engine/src/orchestrator.rs:622-655`
- The repo-owned `proposal-044` gate exists and passed on the audited HEAD. Evidence: `docs/reference/test-gates.md:501-519`, `scripts/test-gate.sh:1470-1476`

### Divergences

- Retry after failed `state_11_manual_release` is not yet proven or implemented as a clean return to `WaitingApproval` on the retried gate. `RetryStage` creates a new `Pending` stage, and `advance_run` later enters the general manual-gate lazy-create path, which creates another fresh stage with `attempt_number: 1` rather than explicitly restoring the retried execution to `WaitingApproval`. Evidence: `control-plane/crates/engine/src/command_handler.rs:324-367`, `control-plane/crates/engine/src/orchestrator.rs:82-88`, `control-plane/crates/engine/src/orchestrator.rs:415-518`, `control-plane/crates/engine/src/orchestrator.rs:816-849`, `control-plane/crates/db/src/repos/stages.rs:64-68`

### Ambiguities / Evidence Gaps

- No executed end-to-end proof was found that retries a failed `state_11_manual_release`, runs `advance_run`, asserts a new `ApprovalRequested`, and verifies the latest stage returns to `WaitingApproval`. The current focused retry test stops earlier at `Pending`. Evidence: `control-plane/crates/engine/tests/integration.rs:1637-1674`

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 `run_after_approval` compiles into `post_approval_tasks`

- Proposal Source: Scope and owner bridge, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:205-221`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:304-307`
  - `control-plane/crates/workflow/tests/integration.rs:167-185`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p workflow test_compile_n_phase_ordering -- --exact`
- Gap / Note: None.

### REQ-002 Approving a manual release gate with post-approval tasks keeps the stage active for execution

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:65-66`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:223-245`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:351`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:202-241`
  - `control-plane/crates/engine/src/command_handler.rs:440-488`
  - `control-plane/crates/engine/tests/integration.rs:1308-1360`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_approve_manual_gate_with_post_approval_tasks_sets_running -- --exact`
- Gap / Note: None.

### REQ-003 `sequence` tasks execute in strict declared N-phase order

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:76-103`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:350`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:386-392`
  - `control-plane/crates/engine/src/orchestrator.rs:268-360`
  - `control-plane/crates/workflow/tests/integration.rs:167-185`
  - `control-plane/crates/engine/tests/integration.rs:1561-1635`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p workflow test_compile_n_phase_ordering -- --exact`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_n_phase_sequence_ordering -- --exact`
- Gap / Note: None.

### REQ-004 Multi-task `then` blocks execute in strict declared order

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:104-142`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:348`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:404-413`
  - `control-plane/crates/engine/src/orchestrator.rs:268-360`
  - `control-plane/crates/workflow/tests/integration.rs:187-248`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' ./scripts/test-gate.sh proposal-044`
- Gap / Note: I did not find a dedicated runtime `state_9` fixture asserting `started_at` ordering, but the shared N-phase runtime path is explicit in code and the compiler-level state_9 phase proof ran on this tree.

### REQ-005 Post-approval task ownership, `task_index` mapping, and phase kickstart use the effective task list before transition evaluation

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:205-264`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:356`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:225-266`
  - `control-plane/crates/engine/src/orchestrator.rs:268-360`
  - `control-plane/crates/engine/src/orchestrator.rs:1359-1370`
  - `control-plane/crates/engine/tests/integration.rs:1420-1490`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_post_approval_tasks_enqueued_after_approval -- --exact`
- Gap / Note: None.

### REQ-006 End states with tasks execute their run block before run completion

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:293-330`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:352`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:383-401`
  - `control-plane/crates/engine/src/orchestrator.rs:448-480`
  - `control-plane/crates/engine/tests/integration.rs:1492-1554`
  - `control-plane/crates/engine/tests/integration.rs:1724-1743`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_end_state_with_tasks_does_not_short_circuit -- --exact`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_state_11_to_state_12_happy_path -- --exact`
- Gap / Note: None.

### REQ-007 Simple manual gates remain unchanged

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:268-272`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:349`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:210-241`
  - `control-plane/crates/engine/tests/integration.rs:1362-1413`
  - `control-plane/crates/engine/tests/integration.rs:1676-1718`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_approve_simple_manual_gate_settles_completed -- --exact`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_simple_manual_gate_no_regression -- --exact`
- Gap / Note: None.

### REQ-008 Retrying a failed `state_11_manual_release` attempt returns the retried gate to fresh approval / `WaitingApproval`

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:67`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:354`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:324-367`
  - `control-plane/crates/engine/src/orchestrator.rs:82-88`
  - `control-plane/crates/engine/src/orchestrator.rs:415-518`
  - `control-plane/crates/engine/src/orchestrator.rs:816-849`
  - `control-plane/crates/db/src/repos/stages.rs:64-68`
  - `control-plane/crates/engine/tests/integration.rs:1637-1674`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_post_approval_retry_requires_fresh_approval -- --exact`
- Gap / Note: The current test only proves `Skipped` old stage + new `Pending` stage. The proposal promises more: a new approval record and a return to `WaitingApproval` on the retried release gate. The current `advance_run` path does not explicitly restore that state on the retried execution.

### REQ-009 Post-approval release work is subject to worktree safety checks

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:68`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:328-330`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:355`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:622-655`
  - `docs/reference/test-gates.md:507-514`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' ./scripts/test-gate.sh proposal-044`
- Gap / Note: I did not isolate a single named worktree-guard fixture inside the workspace run, but the proposal-owned gate explicitly scopes this behavior and the code path is direct.

### REQ-010 The `state_11` happy path transitions to `state_12`, and the `state_12` finalizer runs before run completion

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:9-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:351-352`
- Status: Implemented
- Evidence Type: tests-run, code
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:362-405`
  - `control-plane/crates/engine/src/orchestrator.rs:448-480`
  - `control-plane/crates/engine/tests/integration.rs:1724-1743`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_state_11_to_state_12_happy_path -- --exact`
- Gap / Note: None.

### REQ-011 The repo-owned `proposal-044` proof lane exists and passes on the audited tree

- Proposal Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:370-422`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `docs/reference/test-gates.md:501-519`
  - `scripts/test-gate.sh:1470-1476`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' ./scripts/test-gate.sh proposal-044`
- Gap / Note: None.

## Architecture Review

**Summary:** At Risk

### ARCH-001 RetryStage and `advance_run` do not preserve a single retried release-gate lineage

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-008`; `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:324-367`
  - `control-plane/crates/engine/src/orchestrator.rs:82-88`
  - `control-plane/crates/engine/src/orchestrator.rs:415-518`
  - `control-plane/crates/engine/src/orchestrator.rs:816-849`
  - `control-plane/crates/db/src/repos/stages.rs:64-68`
  - `control-plane/crates/engine/tests/integration.rs:1637-1674`
- Why It Matters: P044 makes fresh re-approval on retry part of the release-safety contract. The current implementation retries by inserting a new `Pending` stage execution, then later falls back to the generic manual-gate creation path. That means the recovery flow is not anchored to a single retried execution and is not explicitly modeled as "the retried release gate is back in `WaitingApproval`."
- Recommended Action: Change retry handling so the retried `state_11_manual_release` attempt deterministically re-enters `WaitingApproval` with a new approval record on the same logical retry path, then add a proof that asserts both the latest stage state and the new approval record after `advance_run`.

## Product Review

**Summary:** At Risk

The happy path is present and the release gate now actually performs post-approval work before transition. The remaining product-level gap is still `REQ-008`: a failed release gate does not yet prove the proposal’s promised operator checkpoint on retry, so the recovery contract is weaker than the design the proposal committed to.

## UI Review

**Summary:** Acceptable

No proposal-owned UI surface is in scope for P044. This audit did not identify a distinct UI-specific blocker beyond the control-plane retry divergence already captured in conformance and architecture.

## UX Review

**Summary:** At Risk

P044 explicitly treats manual release approval as a meaningful operator checkpoint before irreversible side effects. Because the retry path is not yet explicitly restored to `WaitingApproval` on the retried release gate, the recovery experience remains indirect and under-proven even though the normal approval and happy-path execution flows are in place.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 A green `proposal-044` gate still overstates proposal readiness because retry recovery is underimplemented

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-008`, `REQ-011`
- Evidence Type: tests-run, code
- Evidence:
  - `docs/reference/test-gates.md:501-519`
  - `scripts/test-gate.sh:1470-1476`
  - `control-plane/crates/engine/tests/integration.rs:1637-1674`
  - `control-plane/crates/engine/src/command_handler.rs:324-367`
  - `control-plane/crates/engine/src/orchestrator.rs:415-518`
  - `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' ./scripts/test-gate.sh proposal-044`
- Why It Matters: The repo-owned gate is necessary and it passed on this HEAD, but its green status does not by itself prove the proposal’s release-safety retry contract. Handing this slice off as ready would hide a still-open operator recovery gap inside a nominally green proof lane.
- Recommended Action: Fix the retry lifecycle first, then extend the proposal-owned proof inventory so the gate asserts retry-to-`WaitingApproval` with a fresh approval record on the same audited tree.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `./scripts/test-gate.sh proposal-044` passed on the audited macOS host / HEAD |
| Core user flow runtime-validated | Partial | Happy path is covered by `test_state_11_to_state_12_happy_path`; failed-release retry back to `WaitingApproval` is not fully proved and does not yet match the proposal-owned contract |
| Empty/loading/error states covered | Not Checked | No proposal-owned UI surface; failure propagation is visible in code but not audited as a UI flow |
| Accessibility risk acceptable | Not Checked | No UI in scope |
| Localization risk acceptable | Not Checked | No localized strings in scope |
| Critical tests executed | Pass | 9 focused tests were run successfully, including approval, sequencing, retry, simple-gate regression, and full happy path |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `./scripts/test-gate.sh proposal-044` passed on this HEAD |
| Privacy/permissions/entitlements reviewed | Not Checked | Not applicable to this Rust daemon-only slice |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py '/Users/user/Documents/Chainworks Forge/docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md'`
- `rg -n -i "superseded|deprecated|replaced by|obsolete" docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md docs/proposals docs/reference docs/reviews -g '*.md' -g '!**/*IMPLEMENTATION_AUDIT_*.md' -g '!**/*REVIEW_TRIAD_*.md'`
- `git rev-parse HEAD`
- `git status --short --untracked-files=no | wc -l`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p workflow test_compile_n_phase_ordering -- --exact`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_approve_manual_gate_with_post_approval_tasks_sets_running -- --exact`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_approve_simple_manual_gate_settles_completed -- --exact`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_post_approval_tasks_enqueued_after_approval -- --exact`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_end_state_with_tasks_does_not_short_circuit -- --exact`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_n_phase_sequence_ordering -- --exact`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_post_approval_retry_requires_fresh_approval -- --exact`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_simple_manual_gate_no_regression -- --exact`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' cargo test -p engine test_state_11_to_state_12_happy_path -- --exact`
- `CARGO_TARGET_DIR='/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//p044-audit-target.RzX3yo' ./scripts/test-gate.sh proposal-044`

## Recommended Next Actions

1. Fix `RetryStage` / `advance_run` so failed `state_11_manual_release` retries deterministically return the retried gate to `WaitingApproval` with a fresh approval record, instead of relying on a later generic lazy-create path.
2. Add a focused executed proof that retries a failed `state_11_manual_release`, runs `advance_run`, and asserts both `ApprovalRequested` recreation and latest-stage `WaitingApproval`.
3. Keep `proposal-044` as the canonical gate, but do not treat green gate status as proposal-complete until the retry recovery contract is part of that proof lane.
