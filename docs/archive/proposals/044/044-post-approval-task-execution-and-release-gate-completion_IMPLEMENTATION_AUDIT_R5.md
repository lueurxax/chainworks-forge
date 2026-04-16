# Proposal 044: Post-Approval Task Execution and Release Gate Completion Multi-Lens Audit R5

| Field | Value |
|---|---|
| Proposal | docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md |
| Repository Root | . |
| Git SHA | ddc5c0d |
| Working Tree | dirty (0 index, 110458 worktree, 105736 untracked, 0 ignored) |
| Audited At | 2026-04-15T11:52:11+0300 |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

P044's implementation slice is still present on the audited tree: compiler phase assignment, approval-path owner transfer, post-approval enqueue logic, N-phase orchestration, end-state fallthrough, retry semantics, worktree safety, and native release-agent routing all remain in code and focused same-tree tests. The audit still fails closed overall because the canonical proof lane `./scripts/test-gate.sh proposal-044` is red on the current tree and never finished green in this pass, so a successful conformance/readiness verdict is not available even though the proposal-owned behavior itself still looks implemented.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Same-tree canonical gate is red, so the audit cannot lock a successful roll-up | High |
| Architecture | Acceptable | No live P044-owned architecture gap found in the current compiler / orchestrator / approval path | High |
| Product | Acceptable | Happy-path and retry semantics are proven only through focused control-plane tests because the full proposal gate is red | High |
| UI | Acceptable | No direct UI surface is in scope | High |
| UX | Acceptable | No direct operator UI/interaction surface is introduced by this proposal | High |
| Readiness | Not Ready | `proposal-044` gate fails in adjacent workflow/db compile paths on the audited tree | High |

## Proposal Contract

### Scope

- Implement `run_after_approval` task enqueuing with correct effective-task ownership.
- Generalize the binary phase `0/1` model to N-phase sequential ordering for both `sequence` and multi-task `then`.
- Fix `is_end` states with `run` blocks so they execute tasks before settling.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:8-10`

### Locked Decisions

- Post-approval tasks execute after approval and before transition evaluation.
- Retry after a failed manual release requires fresh approval.
- Post-approval release tasks require a provisioned worktree.
- P044 proof uses fixture tasks; ACP-owned release execution is out of scope.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:57-70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:426-430`

### Primary User Flows

- Approve a simple manual gate and have the workflow continue without regression.
- Approve `state_11_manual_release` and have post-approval tasks run in strict declared order before transition evaluation.
- Reach `state_12_workflow_complete` and execute `finalize_run_and_produce_receipts` before the run is marked complete.
- Retry a failed post-approval release stage and reacquire approval before attempting irreversible work again.

### UI Commitments

- No direct UI commitments. This proposal is daemon orchestration only.

### UX Commitments

- Approval semantics remain safe: release work begins only after approval, failures block later phases, and retry requires a fresh decision before irreversible work resumes.
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

- Add focused proof tests for post-approval, ordering, and end-state behavior.
- Add the `proposal-044` Rust control-plane gate.
- Use `./scripts/test-gate.sh proposal-044` as the proposal-owned same-tree proof lane.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:370-422`

### Explicit Exclusions

- Deterministic release services are owned by P045.
- ACP-owned release execution is explicitly out of scope.
- Release receipt formatting is not part of the orchestration slice.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:426-430`

## Proposal Fidelity / Divergence

### Matches

- `compile_run_block` now assigns incrementing phases for `sequence` and multi-task `then`.
- `ApproveStage` moves manual gates with `post_approval_tasks` to `Running` instead of immediately settling `Completed`.
- The orchestrator now resolves `effective_tasks`, kickstarts post-approval phase 0, enforces generalized N-phase gating, and lets end states with tasks fall through to compute execution.
- Worktree gating now inspects `post_approval_tasks`.
- The focused proof inventory named by the proposal exists in repo tests.
- Release agents are still routed natively rather than through ACP.

### Divergences

- No live P044-owned implementation divergence was found on the current tree.

### Ambiguities / Evidence Gaps

- The canonical `proposal-044` gate is red on the audited tree, so the audit cannot close with a successful same-tree proof roll-up.
- Platform scope remains `Ambiguous` because this slice is control-plane orchestration rather than an Apple-surface feature.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 N-phase compile ordering exists for `sequence` and multi-task `then`
- Proposal Source: `Scope`, `§3a`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:366-403`
  - `control-plane/crates/workflow/tests/integration.rs:138-215`
  - `cargo test -p workflow --test integration test_compile_n_phase_ordering -- --exact --nocapture` (passed)
- Gap / Note: The compiler now assigns `(0, 1)` to `state_11_manual_release.post_approval_tasks` and `(0, 1, 2, 3)` to `state_9_implementation_reviewed`.

### REQ-002 Approval handling transfers manual-release stages onto the post-approval owner path
- Proposal Source: `§3c`, `§3d`, `§3f`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:182-235`
  - `control-plane/crates/engine/src/orchestrator.rs:225-266`
  - `control-plane/crates/engine/src/orchestrator.rs:1313-1324`
  - `control-plane/crates/engine/tests/integration.rs:793-971`
  - `cargo test -p engine --test integration -- --nocapture` (passed)
- Gap / Note: Simple manual gates still settle directly; manual release gates now move to `Running` and use `post_approval_tasks` as the effective task set.

### REQ-003 Approved `state_11` release gates enqueue phase-0 work before transition evaluation
- Proposal Source: `Goal`, `§3e`, `AC-5`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:238-266`
  - `control-plane/crates/engine/tests/integration.rs:900-971`
  - `cargo test -p engine --test integration -- --nocapture` (passed)
- Gap / Note: Approval does not short-circuit to transition evaluation; phase 0 is explicitly enqueued first.

### REQ-004 The orchestrator enforces strict N-phase ordering and failure short-circuiting
- Proposal Source: `§3b`, `AC-2`, `AC-4`, `AC-7`, `AC-10`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:268-335`
  - `control-plane/crates/engine/tests/integration.rs:1042-1115`
  - `cargo test -p engine --test integration -- --nocapture` (passed)
- Gap / Note: Success-path phase ordering is directly executed; later-phase skip-on-failure remains explicit in the orchestration branch at `control-plane/crates/engine/src/orchestrator.rs:308-320`.

### REQ-005 End states with `run` blocks execute tasks before run completion
- Proposal Source: `Goal`, `§3h`, `AC-6`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:447-478`
  - `control-plane/crates/engine/src/orchestrator.rs:888-895`
  - `control-plane/crates/engine/tests/integration.rs:977-1035`
  - `control-plane/crates/engine/tests/integration.rs:1224-1575`
  - `cargo test -p engine --test integration test_state_11_to_state_12_happy_path -- --exact --nocapture` (passed)
- Gap / Note: The contiguous `state_11 -> state_12` happy-path fixture still proves finalizer task execution before terminal run completion.

### REQ-006 Retrying a failed post-approval release stage reacquires approval
- Proposal Source: `§2 Product Questions`, `§3g`, `AC-8`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/tests/integration.rs:1118-1155`
  - `cargo test -p engine --test integration -- --nocapture` (passed)
- Gap / Note: Retry creates a new pending attempt, forcing the orchestrator back through the manual-gate path.

### REQ-007 Worktree safety covers post-approval release tasks
- Proposal Source: `§2 Product Questions`, `§3i`, `AC-9`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:519-540`
- Gap / Note: `needs_git_worktree` now inspects both `state.tasks` and `state.post_approval_tasks`.

### REQ-008 The proposal-owned `proposal-044` gate exists as the canonical proof lane
- Proposal Source: `§7 Test Gate`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `docs/reference/test-gates.md:501-519`
  - `scripts/test-gate.sh:1470-1476`
- Gap / Note: The gate exists and is still the canonical same-tree sign-off lane, even though it is red on the current tree.

### REQ-009 Focused proof inventory exists and the current P044-focused slice still executes successfully
- Proposal Source: `§4 Files to Modify`, `§7 Test Gate`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `scripts/test-gate.sh:178-188`
  - `control-plane/crates/workflow/tests/integration.rs:138-215`
  - `control-plane/crates/engine/tests/integration.rs:793-1575`
  - `cargo test -p workflow --test integration test_compile_n_phase_ordering -- --exact --nocapture` (passed)
  - `cargo test -p engine --test integration -- --nocapture` (passed; 16 tests)
- Gap / Note: The focused proof inventory is now nine tests, including the contiguous `test_state_11_to_state_12_happy_path`.

### REQ-010 Release agents route natively rather than through ACP
- Proposal Source: `§8 Out of Scope`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/tests/release.rs:354-420`
  - `cargo test -p engine --test release background_executor_routes_release_agents_natively -- --exact --nocapture` (passed)
- Gap / Note: Native routing remains in place for `commit_and_push_to_github` and `build_archive_and_push_connect`.

## Architecture Review

**Summary:** Acceptable

- No live P044-owned architecture finding remained after current-tree code inspection and focused proof re-runs.

## Product Review

**Summary:** Acceptable

- The operator job promised by P044 is still achievable in focused proof: approval gates irreversible work, phases stay ordered, retries reacquire approval, and terminal receipts are produced before run completion.

## UI Review

**Summary:** Acceptable

- No direct UI finding. This proposal does not introduce a new UI surface.

## UX Review

**Summary:** Acceptable

- No direct UX finding. The user-facing contract in scope is safety/retry semantics, and those remain proven in the current focused control-plane tests.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The canonical `proposal-044` gate is red on the current tree, so successful audit sign-off is blocked
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Test / Evidence Requirements, `REQ-008`, `REQ-009`
- Evidence Type: tests-run, code
- Evidence:
  - `./scripts/test-gate.sh proposal-044` (failed during `workflow` integration-test compilation with `OutputSchema` field errors at `control-plane/crates/workflow/tests/integration.rs:284-407`)
  - `cargo test -p workflow --test integration test_compile_n_phase_ordering -- --exact --nocapture` (passed)
  - `./scripts/test-gate.sh proposal-044` (failed again on `db` compilation: orphan-rule error at `control-plane/crates/db/src/repos/validation.rs:99`)
- Why It Matters: The audit can use focused same-tree proof to show that P044 behavior still exists, but the skill’s success bar requires a passing same-tree canonical full gate for any `Implemented` / `Ready` style roll-up. That proof lane is currently red, and it is red in adjacent workflow/db slices before the proposal-owned workspace sign-off can complete.
- Recommended Action: Repair the current `workflow` / `db` compile breaks on the audited tree, then rerun `./scripts/test-gate.sh proposal-044`. If the project intentionally wants a narrower sign-off lane for P044 than full-workspace `cargo test --workspace`, change the repo-owned gate contract explicitly instead of treating focused proof as a substitute.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Focused workflow/engine/release tests compile and pass, but the canonical workspace gate fails in adjacent `workflow` / `db` slices |
| Core user flow runtime-validated | Pass | `cargo test -p engine --test integration test_state_11_to_state_12_happy_path -- --exact --nocapture` passed |
| Empty/loading/error states covered | Not Applicable | No direct UI surface in scope |
| Accessibility risk acceptable | Not Applicable | No direct UI surface in scope |
| Localization risk acceptable | Not Applicable | No direct UI surface in scope |
| Critical tests executed | Pass | Focused workflow ordering, engine integration, and native release-routing proofs all ran successfully on the audited tree |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail | `./scripts/test-gate.sh proposal-044` failed twice and never finished green in this audit pass |
| Privacy/permissions/entitlements reviewed | Not Applicable | No platform permission surface in scope |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md`
- `git rev-parse --short HEAD`
- `python3 - <<'PY' ... git status --short summary ... PY`
- `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md docs/proposals docs/reviews docs/reference -g '*.md'`
- `./scripts/test-gate.sh proposal-044` (failed: `workflow` integration-test compile errors)
- `cargo test -p engine --test integration test_state_11_to_state_12_happy_path -- --exact --nocapture` (passed)
- `cargo test -p engine --test integration -- --nocapture` (passed; 16 tests)
- `cargo test -p engine --test release background_executor_routes_release_agents_natively -- --exact --nocapture` (passed)
- `cargo test -p workflow --test integration test_compile_n_phase_ordering -- --exact --nocapture` (passed)
- `./scripts/test-gate.sh proposal-044` (failed: `db/src/repos/validation.rs:99` orphan-rule compile error)

## Recommended Next Actions

1. Repair the current-tree compile breaks that keep `./scripts/test-gate.sh proposal-044` red, then rerun the canonical gate on the same tree.
2. Once the gate is green again, rerun this implementation audit and upgrade the roll-up from `Partial / Not Ready` if the focused P044 proofs remain unchanged.
