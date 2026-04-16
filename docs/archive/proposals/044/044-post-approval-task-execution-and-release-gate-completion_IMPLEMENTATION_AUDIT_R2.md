# Proposal 044: Post-Approval Task Execution and Release Gate Completion Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md |
| Repository Root | . |
| Git SHA | ddc5c0d |
| Working Tree | dirty (282 modified, 4427 deleted, 88129 untracked) |
| Audited At | 2026-04-15T07:37:01+0300 |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

P044 is partially implemented on the current tree. The workflow/compiler and orchestrator substrate for N-phase ordering, post-approval effective-task ownership, and end-state fallthrough is present in code, and the workflow-side phase proof runs green. But the same-tree `proposal-044` gate is red, the `engine` crate does not currently compile because adjacent release-path work in `executor.rs` is incomplete, and the executed proof still stops short of the proposal’s strongest claims around strict runtime sequencing and terminal `state_12` receipt production. This is not ready for sign-off.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Same-tree proof lane is red and the strongest happy-path claims remain under-proven | High |
| Architecture | At Risk | The orchestration slice is now coupled to half-landed release integration in `engine/src/executor.rs` | High |
| Product | At Risk | The operator job “approve release, run sequenced post-approval work, get terminal receipts” is only partially proven | Medium |
| UI | Acceptable | No direct user-facing UI surface is in scope for this proposal | High |
| UX | Acceptable | No direct end-user interaction surface beyond approval lifecycle is in scope | High |
| Readiness | Not Ready | `./scripts/test-gate.sh proposal-044` fails on the audited tree | High |

## Proposal Contract

### Scope
- Implement `run_after_approval` task enqueuing with correct effective-task ownership.
- Generalize binary phase `0/1` orchestration to N-phase ordering for `sequence` and multi-task `then`.
- Fix `is_end` states with `run` blocks so they execute tasks before settling.  
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8-10`

### Locked Decisions
- Post-approval tasks execute after approval and before transition evaluation.
- Release-gate retry requires fresh human approval.
- Post-approval tasks require a worktree.
- ACP-owned release side effects are explicitly out of scope; P044 proof must rely on fixture/stub execution only.  
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:57-70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:428-430`

### Primary User Flows
- Approve a simple manual gate and have the workflow continue without regression.
- Approve `state_11_manual_release` and have post-approval tasks run in strict declared order before transition evaluation.
- Reach `state_12_workflow_complete` and run `finalize_run_and_produce_receipts` before the run is marked complete.
- Retry a failed post-approval release stage and reacquire approval before attempting irreversible work again.

### UI Commitments
- No direct UI layout or screen commitments. The proposal is daemon orchestration only.

### UX Commitments
- Approval semantics must remain understandable and safe: release work starts only after approval, failures block later phases, and retry reacquires approval.  
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:65-70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`

### Acceptance Criteria
- Preserve no-regression behavior for state_4 and simple gates.
- Enforce strict ordering for state_9 multi-task `then` and state_11 post-approval `sequence`.
- Execute `state_12` end-state work before completion.
- Skip later phases on failure.
- Require re-approval after failed post-approval retry.
- Block missing-worktree execution.
- Maintain correct `task_index` / phase bookkeeping.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:345-356`

### Test / Evidence Requirements
- Add focused proof tests for post-approval and end-state behavior.
- Add and pass `proposal-044` gate on the Rust control-plane workspace.
- The proposal’s minimum proof inventory names five focused tests plus the full `cargo test --workspace` gate.  
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:334-341`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:370-422`

### Explicit Exclusions
- Deterministic release services are owned by P045.
- ACP-owned release execution is not permitted for P044 proof.
- Release receipt formatting is out of scope for orchestration.  
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:426-430`

## Proposal Fidelity / Divergence

### Matches
- `CompiledState` already carries `post_approval_tasks`, and `compile_run_block` now assigns incrementing phases for `sequence` and multi-task `then`.
- `ApproveStage` now branches manual gates with post-approval tasks to `Running` instead of immediately settling `Completed`.
- `Orchestrator::advance_run` now resolves an effective task list for granted manual gates, kickstarts phase 0 post-approval work, and uses generalized next-phase gating.
- End states with tasks now fall through to compute-state handling instead of unconditional immediate completion.
- `proposal-044` exists in both `docs/reference/test-gates.md` and `scripts/test-gate.sh`.

### Divergences
- The same-tree `proposal-044` gate is red because `cargo test --workspace` fails on unresolved release integration in `control-plane/crates/engine/src/executor.rs` and `control-plane/crates/engine/tests/release.rs`.
- The current focused proof is weaker than the proposal’s own minimum proof inventory: the script only lists three P044-specific test names, and the landed engine tests prove kickstart / no-short-circuit behavior rather than the full “enqueued and settled” / “runs tasks before completion” claims.
- I found no executed same-tree proof for the strict runtime `started_at` ordering promised for state_9/state_11 or for actual `state_12` production of both `delivery_receipt` and `run_report` before run completion.

### Ambiguities / Evidence Gaps
- Fresh re-approval after failed `state_11` retry is supported by the generic retry + manual-gate code path, but I did not find direct state_11-specific executed proof on the audited tree.
- Worktree blocking logic includes `post_approval_tasks`, but I did not find a focused missing-worktree proof for the P044 release-gate path.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 5 |
| Partially Implemented | 4 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 N-phase compile ordering exists for `sequence` and multi-task `then`
- Proposal Source: `Scope`, `§3a` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:76-142`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:325-360`
  - `control-plane/crates/workflow/tests/integration.rs:113-190`
  - `cd control-plane && cargo test -p workflow test_compile_n_phase_ordering -- --nocapture` (passed)
- Gap / Note: This closes the compile-time phase-assignment seam for both `state_11` and `state_9`.

### REQ-002 Approval handling must use `post_approval_tasks` as the effective owner for release gates
- Proposal Source: `§3c`, `§3d`, `§3f` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:205-245`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:266-280`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:182-210`
  - `control-plane/crates/engine/src/orchestrator.rs:201-242`
  - `control-plane/crates/engine/src/orchestrator.rs:1269-1280`
  - `control-plane/crates/engine/tests/integration.rs:789-892`
- Gap / Note: The code matches the proposal, but the engine test target could not be executed on the current tree because the `engine` crate fails to compile.

### REQ-003 Approved post-release manual gates must enqueue phase-0 work before transition evaluation
- Proposal Source: `Goal`, `§3e`, `AC-5` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:9-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:247-264`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:349-352`)
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:214-242`
  - `control-plane/crates/engine/tests/integration.rs:900-971`
- Gap / Note: The landed focused engine test name is `test_post_approval_tasks_enqueued_after_approval`, not the stronger `...enqueued_and_settled` proof name promised by the proposal.

### REQ-004 Orchestrator must gate later phases strictly and skip them after failure
- Proposal Source: `§3b`, `AC-2`, `AC-4`, `AC-7`, `AC-10` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:144-203`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:347-356`)
- Status: Partially Implemented
- Evidence Type: code, tests-run, tests-found
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:244-320`
  - `control-plane/crates/workflow/tests/integration.rs:139-190`
  - `cd control-plane && cargo test -p workflow test_compile_n_phase_ordering -- --nocapture` (passed)
- Gap / Note: Compile-time phase metadata is proven, and the runtime gating code exists, but I found no executed same-tree proof for the promised runtime ordering by `work_item.started_at` or for phase-1 enqueue-after-phase-0 completion on the current broken `engine` target.

### REQ-005 End states with `run` blocks must execute tasks before completion and produce terminal receipts
- Proposal Source: `Goal`, `§3h`, `AC-6` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:9-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:293-326`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:352`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:404-435`
  - `control-plane/crates/engine/tests/integration.rs:973-1034`
  - `examples/workflows/full-mvp-live.yaml:299-320`
- Gap / Note: The short-circuit fix is present and the focused test proves “enter compute path, not immediate completion,” but I found no executed same-tree proof that `finalize_run_and_produce_receipts` actually emits both `delivery_receipt` and `run_report` before the run becomes `Completed`.

### REQ-006 Retrying a failed post-approval release stage must reacquire approval
- Proposal Source: `§2 Product Questions`, `§3g`, `AC-8` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:59-70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:354`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:304-349`
  - `control-plane/crates/engine/src/orchestrator.rs:437-473`
  - `control-plane/crates/engine/tests/integration.rs:275-324`
- Gap / Note: Generic retry semantics and manual-gate approval creation exist, but I found no state_11-specific executed proof showing `Failed -> RetryStage -> WaitingApproval + new ApprovalRequested`.

### REQ-007 Missing worktree must block post-approval release execution
- Proposal Source: `§2 Product Questions`, `§3i`, `AC-9` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:61-69`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:328-330`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:355`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:577-610`
  - `control-plane/crates/engine/src/orchestrator.rs:580-588`
- Gap / Note: Direct code evidence is strong; I did not find a focused executed test for the P044-specific missing-worktree case.

### REQ-008 The proposal-owned `proposal-044` proof lane must exist and pass
- Proposal Source: `§7 Test Gate` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:370-422`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `docs/reference/test-gates.md:501-518`
  - `scripts/test-gate.sh:1463-1468`
  - `./scripts/test-gate.sh proposal-044` (failed)
- Gap / Note: The lane exists, but the same-tree gate is red on the audited tree.

### REQ-009 Minimum focused proof should cover the full P044 happy path, not only substrate smoke
- Proposal Source: `§4 Files to Modify`, `§7 Test Gate` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:338-341`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:397-422`)
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/engine/tests/integration.rs:900-1034`
  - `scripts/test-gate.sh:178-182`
- Gap / Note: Landed focused tests cover `...enqueued_after_approval` and `...does_not_short_circuit`, but I did not find direct focused proofs for the stronger proposal claims `...enqueued_and_settled`, `...runs_tasks_before_completion`, or runtime `started_at` ordering.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Orchestration-only scope is now coupled to half-landed release integration
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Scope, Explicit Exclusions, REQ-008, REQ-009
- Evidence Type: code, tests-run
- Evidence:
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8`
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:428-430`
  - `control-plane/crates/engine/src/executor.rs:11-22`
  - `control-plane/crates/engine/src/executor.rs:221-225`
  - `control-plane/crates/engine/tests/release.rs:13-19`
  - `./scripts/test-gate.sh proposal-044` (failed)
- Why It Matters: P044 is explicitly scoped as orchestration-only and explicitly excludes ACP-owned release execution. On the audited tree, `BackgroundExecutor` now pulls in release-path imports and calls undefined release helpers, which breaks compilation before the P044 engine proofs can even run. That means the orchestration slice is no longer isolated enough to audit or sign off independently.
- Recommended Action: Either finish the adjacent release-path integration coherently, or isolate it behind a compiling boundary so the P044 orchestration slice and its proof lane can compile and run independently on the same tree.

## Product Review

**Summary:** At Risk

### PROD-001 The main operator job is only partially demonstrated end-to-end
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: Goal, Primary User Flows, REQ-004, REQ-005, REQ-006
- Evidence Type: code, tests-run, tests-found
- Evidence:
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:9-10`
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:347-356`
  - `control-plane/crates/workflow/tests/integration.rs:113-190`
  - `control-plane/crates/engine/tests/integration.rs:900-1034`
  - `./scripts/test-gate.sh proposal-044` (failed)
- Why It Matters: The product promise is simple: approve the release gate, run sequenced post-approval work, reach `state_12`, emit terminal receipts, and complete. The current tree proves only part of that story. The compile-time ordering substrate is real, but the executed evidence still does not show the full operator outcome on the audited tree.
- Recommended Action: Add or repair same-tree proof that covers the whole `state_11 -> state_12` happy path with strict phase ordering, terminal artifacts, and completion.

## UI Review

**Summary:** Acceptable

- No direct UI findings. This proposal is daemon orchestration only and does not commit to screen/layout work.

## UX Review

**Summary:** Acceptable

- No additional UX-only findings beyond the conformance/readiness issues already recorded. The operator-facing semantics in scope are approval, retry, and failure gating rather than visual interaction design.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The canonical proposal gate is red on the audited tree
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-008
- Evidence Type: tests-run
- Evidence:
  - `docs/reference/test-gates.md:501-518`
  - `scripts/test-gate.sh:1463-1468`
  - `./scripts/test-gate.sh proposal-044` (failed)
- Why It Matters: The proposal itself names `proposal-044` as the proof lane. That lane currently fails on the audited tree, so this audit cannot honestly call the slice ready or signed off.
- Recommended Action: Restore same-tree gate health first, then rerun the audit.

### READY-002 The `engine` target cannot currently compile its P044-focused tests
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-002, REQ-003, REQ-004, REQ-005, REQ-006, REQ-009
- Evidence Type: tests-run, code
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:11-22`
  - `control-plane/crates/engine/src/executor.rs:221-225`
  - `cd control-plane && cargo test -p engine --test integration test_approve_manual_gate_with_post_approval_tasks_sets_running -- --nocapture` (failed)
  - `cd control-plane && cargo test -p engine --test integration test_post_approval_tasks_enqueued_after_approval -- --nocapture` (failed)
  - `cd control-plane && cargo test -p engine --test integration test_end_state_with_tasks_does_not_short_circuit -- --nocapture` (failed)
- Why It Matters: The current tree cannot execute the engine-side proof for the very behaviors P044 is supposed to land. That blocks both engineering confidence and audit reproducibility.
- Recommended Action: Repair the `engine` crate compile break, then rerun the focused engine tests and the proposal gate on the same tree.

### READY-003 The landed focused proof is weaker than the proposal’s own acceptance bar
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-004, REQ-005, REQ-009
- Evidence Type: code, tests-found
- Evidence:
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:338-341`
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:397-422`
  - `control-plane/crates/engine/tests/integration.rs:900-1034`
  - `scripts/test-gate.sh:178-182`
- Why It Matters: Even after the compile break is repaired, the current focused tests still do not fully prove the proposal’s strongest claims about strict started-at ordering and terminal finalizer outputs. That leaves the shipped evidence below the proposal contract.
- Recommended Action: Strengthen the focused proof to cover actual phase sequencing, phase-failure skip behavior, and `state_12` receipt/report production before completion.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Fail | `./scripts/test-gate.sh proposal-044` and targeted `cargo test -p engine ...` both fail during `engine` compilation |
| Core user flow runtime-validated | Partial | Workflow compile proof is green; engine-side happy path could not be executed on the audited tree |
| Empty/loading/error states covered | Not Applicable | No direct UI state surface in this orchestration-only proposal |
| Accessibility risk acceptable | Not Applicable | No direct UI scope |
| Localization risk acceptable | Not Applicable | No direct UI scope |
| Critical tests executed | Partial | `cargo test -p workflow test_compile_n_phase_ordering` passed; engine-side focused tests were blocked by compile failure |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail | `./scripts/test-gate.sh proposal-044` failed on `HEAD ddc5c0d` |
| Privacy/permissions/entitlements reviewed | Not Applicable | No platform permission surface in scope |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md`
- `git rev-parse --show-toplevel && git rev-parse --short HEAD && git status --short`
- `rg -n "post_approval_tasks|run_after_approval|effective_tasks|is_post_approval|task_index|is_end|proposal-044|manual_gate" control-plane scripts docs/reference docs/reviews -S`
- `./scripts/test-gate.sh proposal-044`
- `cd control-plane && cargo test -p workflow test_compile_n_phase_ordering -- --nocapture`
- `cd control-plane && cargo test -p engine --test integration test_approve_manual_gate_with_post_approval_tasks_sets_running -- --nocapture`
- `cd control-plane && cargo test -p engine --test integration test_post_approval_tasks_enqueued_after_approval -- --nocapture`
- `cd control-plane && cargo test -p engine --test integration test_end_state_with_tasks_does_not_short_circuit -- --nocapture`

## Recommended Next Actions

1. Repair the current `engine` compile break in `executor.rs` / release integration so the same-tree `proposal-044` gate can run.
2. Rerun `./scripts/test-gate.sh proposal-044` on the repaired tree and treat that as the minimum sign-off bar for this proposal.
3. Strengthen focused proof for the still-underproven claims: strict runtime phase ordering, state_11 happy-path completion, phase-failure skip behavior, and `state_12` receipt/report production before run completion.
4. Add direct state_11 retry-with-reapproval and missing-worktree proofs so the safety semantics are not only inferred from generic code paths.
