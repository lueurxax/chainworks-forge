# Proposal 044: Post-Approval Task Execution and Release Gate Completion Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md |
| Repository Root | . |
| Git SHA | ddc5c0d |
| Working Tree | dirty (4717 tracked changes, 102562 untracked) |
| Audited At | 2026-04-15T10:24:33+03:00 |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Implemented |
| Overall Readiness | Ready with Risks |
| Audit Confidence | Medium |

## Executive Verdict

P044 is implemented on the audited tree. The workflow/compiler phase model, approval-path owner bridge, post-approval enqueue logic, end-state fallthrough, worktree safety, retry semantics, native release-agent routing, and the state_11 -> state_12 happy-path proof are all present in code and executed tests, and the canonical same-tree gate `./scripts/test-gate.sh proposal-044` passed on `HEAD ddc5c0d`. Readiness stays at `Ready with Risks` instead of `Ready` because the same canonical gate was not perfectly stable run-to-run during this audit: it failed once in `background_executor_persists_delivery_receipt_on_publish_failure`, then the isolated test passed, and the immediate full-gate rerun passed.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Implemented | Proposal-owned proof lane is currently reproducible enough to pass, but not yet perfectly stable run-to-run | High |
| Architecture | Acceptable | P044 now depends on adjacent release-path determinism for proof confidence | High |
| Product | Acceptable | Operator happy path is proven in executed fixtures rather than a live daemon walkthrough | Medium |
| UI | Acceptable | No direct UI surface is in scope | High |
| UX | Acceptable | No direct end-user interaction surface is in scope | High |
| Readiness | Ready with Risks | Back-to-back canonical gate executions were inconsistent before the final green run | Medium |

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
- P044 proof uses fixture tasks and deterministic native services, not ACP-owned release execution.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:57-70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:428-430`

### Primary User Flows
- Approve a simple manual gate and have the workflow continue without regression.
- Approve `state_11_manual_release` and have post-approval tasks run in strict declared order before transition evaluation.
- Reach `state_12_workflow_complete` and run `finalize_run_and_produce_receipts` before the run is marked complete.
- Retry a failed post-approval release stage and reacquire approval before attempting irreversible work again.

### UI Commitments
- No direct UI layout or screen commitments. The proposal is daemon orchestration only.

### UX Commitments
- Approval semantics must remain safe and understandable: release work starts only after approval, failures block later phases, and retry reacquires approval.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:65-70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`

### Acceptance Criteria
1. No regression on single-task `then`.
2. Multi-task `then` ordering.
3. No regression on simple gates.
4. Sequential ordering for state_11 post-approval tasks.
5. Happy path: state_11 approval -> Running -> phase 0 -> phase 1 -> Completed -> transition to state_12.
6. End-state task execution before run completion.
7. Phase failure propagation.
8. Retry requires re-approval.
9. Worktree guard.
10. DB correctness for `task_index` and phase mapping.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:345-356`

### Test / Evidence Requirements
- Add focused proof tests for post-approval and end-state behavior.
- Add the `proposal-044` Rust control-plane gate.
- P044 proof lane uses the full Rust workspace test suite via `./scripts/test-gate.sh proposal-044`.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:334-341`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:370-422`

### Explicit Exclusions
- Deterministic release services are owned by P045.
- ACP-owned release execution is not permitted for P044 proof.
- Release receipt formatting is out of scope for orchestration.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:426-430`

## Proposal Fidelity / Divergence

### Matches
- `compile_run_block` now assigns incrementing phases for both `sequence` and multi-task `then`.
- `ApproveStage` now sends manual gates with `post_approval_tasks` to `Running` instead of immediately settling `Completed`.
- `Orchestrator::advance_run` now resolves `effective_tasks`, kickstarts phase-0 post-approval work, uses generalized N-phase gating, and keeps end states with tasks on the compute path.
- Worktree gating now inspects `post_approval_tasks`.
- The proposal-owned `proposal-044` gate exists and passed on the audited `HEAD`.
- Release agents are routed natively rather than through ACP.

### Divergences
- No direct proposal-vs-implementation divergence was found on the current tree.

### Ambiguities / Evidence Gaps
- Platform scope is still `Ambiguous` because this proposal is daemon orchestration only, not an Apple-surface feature slice.
- The canonical `proposal-044` gate was inconsistent across two immediate executions during this audit, which is a readiness risk but not a proposal-conformance miss.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 N-phase compile ordering exists for `sequence` and multi-task `then`
- Proposal Source: `Scope`, `§3a` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:76-142`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:325-360`
  - `control-plane/crates/workflow/tests/integration.rs:113-200`
  - `cd control-plane && cargo test -p workflow --test integration -- --nocapture` (passed)
- Gap / Note: The compiler now assigns `(0, 1)` to `state_11_manual_release.post_approval_tasks` and `(0, 1, 2, 3)` to `state_9_implementation_reviewed`.

### REQ-002 Approval handling must use post-approval tasks as the effective owner for release gates
- Proposal Source: `§3c`, `§3d`, `§3f` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:205-245`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:266-280`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:182-221`
  - `control-plane/crates/engine/src/command_handler.rs:417-464`
  - `control-plane/crates/engine/src/orchestrator.rs:225-266`
  - `control-plane/crates/engine/src/orchestrator.rs:1312-1324`
  - `control-plane/crates/engine/tests/integration.rs:793-893`
  - `./scripts/test-gate.sh proposal-044` (passed on rerun)
- Gap / Note: Simple manual gates still settle immediately; release gates with post-approval work now move to `Running` and switch to `post_approval_tasks`.

### REQ-003 Approved `state_11` release gates must enqueue phase-0 work before transition evaluation
- Proposal Source: `Goal`, `§3e`, `AC-5` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:9-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:247-264`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:349-352`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:225-266`
  - `control-plane/crates/engine/tests/integration.rs:900-971`
  - `./scripts/test-gate.sh proposal-044` (passed on rerun)
- Gap / Note: The executed proof shows approval does not short-circuit transition evaluation; phase 0 is enqueued first.

### REQ-004 The orchestrator must enforce strict numeric phase ordering and skip later phases after failure
- Proposal Source: `§3b`, `AC-2`, `AC-4`, `AC-7`, `AC-10` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:144-203`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:347-356`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:268-320`
  - `control-plane/crates/workflow/src/compiler.rs:325-360`
  - `control-plane/crates/engine/tests/integration.rs:1042-1115`
  - `cd control-plane && cargo test -p engine --test integration test_state_11_to_state_12_happy_path -- --nocapture` (passed)
- Gap / Note: Executed proof directly covers success-path phase ordering. The failure-skip branch is implemented in the orchestrator code path at `control-plane/crates/engine/src/orchestrator.rs:308-320`.

### REQ-005 End states with `run` blocks must execute tasks before completion and support terminal receipt production
- Proposal Source: `Goal`, `§3h`, `AC-6` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:9-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:293-326`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:352`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:446-450`
  - `control-plane/crates/engine/src/orchestrator.rs:888-897`
  - `control-plane/crates/engine/tests/integration.rs:973-1035`
  - `control-plane/crates/engine/tests/integration.rs:1205-1578`
  - `cd control-plane && cargo test -p engine --test integration test_state_11_to_state_12_happy_path -- --nocapture` (passed)
- Gap / Note: The current tree now includes the explicit contiguous `state_11 -> state_12` happy-path fixture that was still missing in earlier audit rounds.

### REQ-006 Retrying a failed post-approval release stage must reacquire approval
- Proposal Source: `§2 Product Questions`, `§3g`, `AC-8` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:59-70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:354`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/tests/integration.rs:1118-1155`
  - `./scripts/test-gate.sh proposal-044` (passed on rerun)
- Gap / Note: The executed proof shows the retry creates a fresh stage attempt; the next `AdvanceRun` re-enters the manual-gate path and requires fresh approval.

### REQ-007 Missing worktree must block post-approval release execution
- Proposal Source: `§2 Product Questions`, `§3i`, `AC-9` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:61-69`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:328-330`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:355`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:520-590`
- Gap / Note: `needs_git_worktree` now inspects both `state.tasks` and `state.post_approval_tasks`, which closes the proposal’s post-approval worktree-safety seam.

### REQ-008 The proposal-owned `proposal-044` proof lane must exist and pass on the audited tree
- Proposal Source: `§7 Test Gate` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:370-422`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `docs/reference/test-gates.md:501-519`
  - `scripts/test-gate.sh:1470-1476`
  - `./scripts/test-gate.sh proposal-044` (first run failed in `control-plane/crates/engine/tests/release.rs:563-686`, immediate rerun passed on the same `HEAD`)
- Gap / Note: Passing same-tree full regression evidence exists, so conformance remains implemented. The run-to-run inconsistency lowers readiness confidence and is tracked under `READY-001`.

### REQ-009 Focused proof inventory must cover the P044 substrate and no-regression bar
- Proposal Source: `§4 Files to Modify`, `§7 Test Gate` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:338-341`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:397-422`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `scripts/test-gate.sh:178-188`
  - `control-plane/crates/workflow/tests/integration.rs:113-200`
  - `control-plane/crates/engine/tests/integration.rs:793-1578`
  - `./scripts/test-gate.sh proposal-044` (passed on rerun)
- Gap / Note: The current inventory is broader than the original sketch and now includes the contiguous `test_state_11_to_state_12_happy_path` proof.

### REQ-010 Release agents must route natively rather than through ACP
- Proposal Source: `§8 Out of Scope` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:428-430`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:222-240`
  - `control-plane/crates/engine/src/executor.rs:459-479`
  - `control-plane/crates/engine/tests/release.rs:354-420`
  - `./scripts/test-gate.sh proposal-044` (passed on rerun)
- Gap / Note: The executor now explicitly bypasses ACP for `commit_and_push_to_github` and `build_archive_and_push_connect`.

## Architecture Review

**Summary:** Acceptable

- No architecture-level blocker remains. The compiler, approval path, orchestrator, and native release-agent routing are coherent with the proposal’s locked decisions.

## Product Review

**Summary:** Acceptable

- The operator job promised by P044 is materially achievable on the current tree: approval gates irreversible work, phase ordering is enforced, retries reacquire approval, and terminal receipts are produced before run completion.

## UI Review

**Summary:** Acceptable

- No direct UI findings. This proposal is daemon orchestration only.

## UX Review

**Summary:** Acceptable

- No direct UX-only finding remains. The safety and retry semantics in scope are implemented through backend state transitions rather than new interaction surfaces.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 The canonical `proposal-044` gate is still unstable run-to-run
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: Test / Evidence Requirements, `REQ-008`, `REQ-009`
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-044` on `HEAD ddc5c0d` (first run failed in `control-plane/crates/engine/tests/release.rs:563-686`)
  - `cd control-plane && cargo test -p engine --test release background_executor_persists_delivery_receipt_on_publish_failure -- --nocapture` (passed)
  - `./scripts/test-gate.sh proposal-044` on `HEAD ddc5c0d` (immediate rerun passed)
- Why It Matters: Same-tree passing regression evidence exists, but a proposal-owned gate that flips red and green across immediate reruns is still a delivery risk. It reduces CI trust, complicates sign-off, and can hide shared-state or ordering issues in the adjacent release suite.
- Recommended Action: Investigate cross-test interference in `control-plane/crates/engine/tests/release.rs`, especially around publish-failure setup and shared temporary repo state, and make the canonical gate deterministic before treating `proposal-044` as routine release-grade proof.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `./scripts/test-gate.sh proposal-044` passed on `HEAD ddc5c0d` |
| Core user flow runtime-validated | Pass | Executed Rust integration proofs cover approval -> phase 0 -> phase 1 -> state_12 -> terminal completion, including `test_state_11_to_state_12_happy_path` |
| Empty/loading/error states covered | Not Applicable | No direct UI surface in scope |
| Accessibility risk acceptable | Not Applicable | No direct UI surface in scope |
| Localization risk acceptable | Not Applicable | No direct UI surface in scope |
| Critical tests executed | Pass | Workflow integration, engine integration, engine release, GraphQL, MCP, DB, domain, and ACP tests were executed through the canonical gate |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `./scripts/test-gate.sh proposal-044` passed on `HEAD ddc5c0d`; one prior red run in the same audit lowers confidence but does not remove passing same-tree evidence |
| Privacy/permissions/entitlements reviewed | Not Applicable | No platform permission surface in scope |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md`
- `git rev-parse --show-toplevel`
- `git rev-parse HEAD`
- `git rev-parse --short=7 HEAD`
- `git status --porcelain=v1`
- `date -Iseconds`
- `rg -n "superseded|deprecated|replaced by|obsolete|P044|044-post-approval-task-execution-and-release-gate-completion" docs/proposals docs/reviews docs/reference -g '*.md'`
- `rg -n "run_after_approval|post_approval_tasks|phase == 1|phase 0|effective_tasks|finalize_run_and_produce_receipts|manual_gate|check_has_post_approval_tasks|StageStatus::Running|sequence:" control-plane/crates -g '!target/**'`
- `sed -n '1,220p' /Users/user/.agents/skills/proposal-implementation-audit/references/example-implementation-audit-report.md`
- `sed -n '1,520p' docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md`
- `sed -n '325,395p' control-plane/crates/workflow/src/compiler.rs`
- `sed -n '176,240p' control-plane/crates/engine/src/command_handler.rs`
- `sed -n '417,465p' control-plane/crates/engine/src/command_handler.rs`
- `sed -n '220,320p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '396,450p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '520,615p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '650,760p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '1300,1335p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '100,210p' control-plane/crates/workflow/tests/integration.rs`
- `sed -n '780,1585p' control-plane/crates/engine/tests/integration.rs`
- `sed -n '340,420p' control-plane/crates/engine/tests/release.rs`
- `sed -n '563,686p' control-plane/crates/engine/tests/release.rs`
- `sed -n '501,520p' docs/reference/test-gates.md`
- `sed -n '1460,1485p' scripts/test-gate.sh`
- `./scripts/test-gate.sh proposal-044` (first run failed in `background_executor_persists_delivery_receipt_on_publish_failure`)
- `cd control-plane && cargo test -p workflow --test integration -- --nocapture` (passed)
- `cd control-plane && cargo test -p engine --test integration test_state_11_to_state_12_happy_path -- --nocapture` (passed)
- `cd control-plane && cargo test -p engine --test release background_executor_persists_delivery_receipt_on_publish_failure -- --nocapture` (passed)
- `./scripts/test-gate.sh proposal-044` (second run passed)

## Recommended Next Actions

1. Stabilize the canonical `proposal-044` gate by isolating the intermittent `background_executor_persists_delivery_receipt_on_publish_failure` behavior in the release suite.
2. Keep the contiguous `state_11 -> state_12` happy-path fixture as the primary proof lane and add any future release-path assertions there before broadening adjacent suites.
