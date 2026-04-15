# Proposal 044: Post-Approval Task Execution and Release Gate Completion Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md |
| Repository Root | . |
| Git SHA | ddc5c0d |
| Working Tree | dirty (0 index, 4714 worktree, 95702 untracked, 0 ignored) |
| Audited At | 2026-04-15T08:03:44+03:00 |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Implemented |
| Overall Readiness | Ready with Risks |
| Audit Confidence | High |

## Executive Verdict

P044 is now implemented on the audited tree. The old R2 blockers are stale: the proposal-owned `proposal-044` gate is green on the same `HEAD`, the compiler/orchestrator/approval-path changes are landed, and the current proof inventory now covers the release-gate approval transition, N-phase ordering, retry semantics, simple-gate non-regression, end-state fallthrough, and release-path receipt persistence. The remaining concern is proof shape, not substrate correctness: the current evidence is strong but compositional, so readiness is `Ready with Risks` rather than a completely risk-free sign-off.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Implemented | Proof is split across focused integration and release tests rather than one monolithic state_11 -> state_12 fixture | High |
| Architecture | Acceptable | Terminal receipt proof spans orchestrator and executor surfaces | High |
| Product | Acceptable | End-state happy-path proof is compositional rather than one direct operator-flow test | Medium |
| UI | Acceptable | None identified; no direct UI surface is in scope | High |
| UX | Acceptable | None identified; no direct end-user interaction surface is in scope | High |
| Readiness | Ready with Risks | No single executed fixture walks the entire state_11 -> state_12 happy path in one shot | High |

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
- ACP-owned release side effects are explicitly out of scope; P044 proof may use fixture tasks and repo-backed deterministic services, but not free-form ACP release execution.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:57-70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:428-430`

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
- Add and pass the `proposal-044` Rust control-plane gate.
- Minimum proof inventory is the proposal-owned focused test slice plus full `cargo test --workspace`.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:334-341`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:370-422`

### Explicit Exclusions
- Deterministic release services are owned by P045.
- ACP-owned release execution is not permitted for P044 proof.
- Release receipt formatting is out of scope for orchestration.
  Source: `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:426-430`

## Proposal Fidelity / Divergence

### Matches
- `compile_run_block` now assigns incrementing phases for both `sequence` and multi-task `then`.
- `ApproveStage` now sends manual gates with post-approval tasks to `Running` instead of immediately settling `Completed`.
- `Orchestrator::advance_run` now resolves `effective_tasks`, kickstarts phase 0 post-approval work, and uses generalized N-phase gating.
- End states with tasks now fall through to compute-state handling instead of unconditional immediate completion.
- The canonical `proposal-044` gate exists in `docs/reference/test-gates.md` and `scripts/test-gate.sh`, and it now passes on the audited tree.
- The current focused proof inventory contains all 8 tests the user listed, and all 8 are exercised by the passing workspace gate.

### Divergences
- The landed focused proof inventory differs from the proposal’s original five-name sketch, but it materially strengthens the proof instead of weakening it.
- Terminal happy-path evidence is compositional across integration tests and release tests rather than one single fixture that executes `state_11 -> state_12` end to end.

### Ambiguities / Evidence Gaps
- I did not find one focused test that both drives the full state_11 approval path and directly asserts `run_report` creation on state_12 before completion.
- Phase-failure skip behavior is explicit in orchestrator code, but it is not isolated by a dedicated focused failure test name in the current inventory.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 9 |
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
  - `control-plane/crates/workflow/tests/integration.rs:113-190`
  - `./scripts/test-gate.sh proposal-044` (passed; includes `test_compile_n_phase_ordering`)
- Gap / Note: The compiler now assigns `(0,1)` to state_11 post-approval tasks and `(0,1,2,3)` to state_9 parallel+then ordering.

### REQ-002 Approval handling must use post-approval tasks as the effective owner for release gates
- Proposal Source: `§3c`, `§3d`, `§3f` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:205-245`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:266-280`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:182-221`
  - `control-plane/crates/engine/src/orchestrator.rs:201-242`
  - `control-plane/crates/engine/src/orchestrator.rs:1269-1280`
  - `control-plane/crates/engine/tests/integration.rs:793-900`
  - `./scripts/test-gate.sh proposal-044` (passed; includes `test_approve_manual_gate_with_post_approval_tasks_sets_running` and `test_approve_simple_manual_gate_settles_completed`)
- Gap / Note: The approval path now matches the proposal’s owner bridge: simple gates settle immediately, release gates move to `Running` and switch to `post_approval_tasks`.

### REQ-003 Approved state_11 release gates must enqueue phase-0 work before transition evaluation
- Proposal Source: `Goal`, `§3e`, `AC-5` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:9-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:247-264`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:349-352`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:214-242`
  - `control-plane/crates/engine/tests/integration.rs:903-975`
  - `./scripts/test-gate.sh proposal-044` (passed; includes `test_post_approval_tasks_enqueued_after_approval`)
- Gap / Note: The executed proof confirms that approval alone does not short-circuit the stage; phase 0 work is enqueued first.

### REQ-004 The orchestrator must enforce strict numeric phase ordering and skip later phases after failure
- Proposal Source: `§3b`, `AC-2`, `AC-4`, `AC-7`, `AC-10` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:144-203`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:347-356`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:244-359`
  - `control-plane/crates/workflow/src/compiler.rs:325-360`
  - `control-plane/crates/engine/tests/integration.rs:1047-1115`
  - `./scripts/test-gate.sh proposal-044` (passed; includes `test_n_phase_sequence_ordering`)
- Gap / Note: The executed test proves phase-1 work is not enqueued before phase 0 completes. The failure-skip branch is explicit in orchestrator code and settles the stage as `Failed` without enqueueing later phases.

### REQ-005 End states with `run` blocks must execute tasks before completion and support terminal receipt production
- Proposal Source: `Goal`, `§3h`, `AC-6` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:9-10`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:293-326`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:352`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:404-435`
  - `control-plane/crates/engine/src/orchestrator.rs:839-847`
  - `control-plane/crates/engine/tests/integration.rs:977-1044`
  - `control-plane/crates/engine/tests/release.rs:306-409`
  - `control-plane/crates/engine/tests/release.rs:411-552`
  - `./scripts/test-gate.sh proposal-044` (passed; includes `test_end_state_with_tasks_does_not_short_circuit`, `background_executor_routes_release_agents_natively`, and `advance_run_backfills_delivery_receipt_when_terminal_release_lineage_exists`)
- Gap / Note: Current proof is compositional: one test proves end states do not short-circuit, and the release tests prove terminal receipt persistence and backfill once terminal release lineage exists.

### REQ-006 Retrying a failed post-approval release stage must reacquire approval
- Proposal Source: `§2 Product Questions`, `§3g`, `AC-8` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:59-70`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:282-291`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:354`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:304-344`
  - `control-plane/crates/engine/src/orchestrator.rs:437-473`
  - `control-plane/crates/engine/tests/integration.rs:1122-1159`
  - `./scripts/test-gate.sh proposal-044` (passed; includes `test_post_approval_retry_requires_fresh_approval`)
- Gap / Note: The executed test proves the retry creates a fresh stage attempt; the manual-gate path then recreates `WaitingApproval` plus a new approval record on the next `AdvanceRun`.

### REQ-007 Missing worktree must block post-approval release execution
- Proposal Source: `§2 Product Questions`, `§3i`, `AC-9` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:61-69`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:328-330`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:355`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:577-610`
- Gap / Note: `RepoSafetyGuard` now evaluates both `state.tasks` and `state.post_approval_tasks`, which closes the proposal’s post-approval worktree-safety seam.

### REQ-008 The proposal-owned `proposal-044` proof lane must exist and pass on the audited tree
- Proposal Source: `§7 Test Gate` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:370-422`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `docs/reference/test-gates.md:501-518`
  - `scripts/test-gate.sh:178-186`
  - `scripts/test-gate.sh:1463-1468`
  - `./scripts/test-gate.sh proposal-044` (passed on `HEAD ddc5c0d`)
- Gap / Note: This closes the biggest R2 blocker. Same-tree full regression evidence now exists and is green.

### REQ-009 Focused proof inventory must cover the P044 substrate and no-regression bar
- Proposal Source: `§4 Files to Modify`, `§7 Test Gate` (`docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:338-341`, `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:397-422`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `scripts/test-gate.sh:178-186`
  - `control-plane/crates/workflow/tests/integration.rs:113-190`
  - `control-plane/crates/engine/tests/integration.rs:793-1192`
  - `./scripts/test-gate.sh proposal-044` (passed; executed all 8 focused proofs plus broader workspace coverage)
- Gap / Note: The current inventory differs from the proposal’s original sketch, but the landed 8-test slice is broader and materially stronger.

## Architecture Review

**Summary:** Acceptable

- No architecture-level blocker remains. The landed compiler, command-handler, and orchestrator changes are coherent and the same-tree gate is green.

## Product Review

**Summary:** Acceptable

- The operator job promised by P044 is now materially achievable: approval gates release work, phase ordering is enforced, retry reacquires approval, and terminal receipt persistence is covered by executed tests.

## UI Review

**Summary:** Acceptable

- No direct UI findings. This proposal is daemon orchestration only.

## UX Review

**Summary:** Acceptable

- No direct UX-only findings beyond the proof-shape readiness note. Approval/retry semantics in scope are implemented through backend state transitions rather than new interaction surfaces.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 Happy-path proof is strong but still compositional
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: REQ-005, REQ-009
- Evidence Type: tests-run, code
- Evidence:
  - `control-plane/crates/engine/tests/integration.rs:977-1044`
  - `control-plane/crates/engine/tests/release.rs:306-552`
  - `./scripts/test-gate.sh proposal-044` (passed)
- Why It Matters: The current gate proves the important pieces, but it still does so across multiple focused tests rather than one state_11 -> state_12 fixture that asserts approval, phase 0, phase 1, transition, finalizer artifacts, and run completion in one pass. That is not a proposal-conformance miss anymore, but it does leave a small regression-detection gap across boundaries.
- Recommended Action: Add one fixture-based end-to-end P044 happy-path test that asserts state_11 approval, ordered post-approval execution, state_12 entry, `delivery_receipt`/`run_report` emission, and terminal run completion in one shot.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `./scripts/test-gate.sh proposal-044` passed on `HEAD ddc5c0d` |
| Core user flow runtime-validated | Partial | Validated by executed Rust integration and release tests, not by a single live daemon walkthrough |
| Empty/loading/error states covered | Not Applicable | No direct UI surface in scope |
| Accessibility risk acceptable | Not Applicable | No direct UI surface in scope |
| Localization risk acceptable | Not Applicable | No direct UI surface in scope |
| Critical tests executed | Pass | Gate executed workflow, engine integration, engine release, GraphQL, MCP, DB, and ACP tests |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `./scripts/test-gate.sh proposal-044` passed on the audited tree |
| Privacy/permissions/entitlements reviewed | Not Applicable | No platform permission surface in scope |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md`
- `git rev-parse --short HEAD`
- `python3 - <<'PY' ... git status --short summary ... PY`
- `sed -n '1,340p' docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md`
- `sed -n '170,240p' control-plane/crates/engine/src/command_handler.rs`
- `sed -n '190,460p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '560,620p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '1258,1288p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '320,360p' control-plane/crates/workflow/src/compiler.rs`
- `sed -n '780,1195p' control-plane/crates/engine/tests/integration.rs`
- `sed -n '290,560p' control-plane/crates/engine/tests/release.rs`
- `sed -n '100,210p' control-plane/crates/workflow/tests/integration.rs`
- `./scripts/test-gate.sh proposal-044`

## Recommended Next Actions

1. Add one fixture-based end-to-end `state_11 -> state_12` happy-path proof so future regressions across approval, sequencing, transition, and finalization are caught by a single targeted test.
