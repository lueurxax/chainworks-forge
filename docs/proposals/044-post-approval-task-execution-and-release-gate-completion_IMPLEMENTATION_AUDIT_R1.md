# Proposal 044: Post-Approval Task Execution and Release Gate Completion Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md` |
| Repository Root | `.` |
| Git SHA | `2c3dfa1` |
| Working Tree | dirty (many modified and untracked files already present before this audit pass) |
| Audited At | `2026-04-14T23:18:43+0300` |
| Platform Scope | Ambiguous |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 044's core orchestration slice is materially but incompletely landed on the current tree. The compiler now assigns sequential phases for `sequence` and multi-task `then`, manual-gate approval with `post_approval_tasks` transitions the stage to `Running`, `effective_tasks(...)` exists, end states with tasks no longer short-circuit to immediate completion, and the narrow proposal-owned tests that do exist are green. This audit still fails closed because one proposal requirement is still missing in runtime truth: post-approval release tasks are not covered by the worktree/repo-safety guard path, so a missing `worktree_root` can still degrade into ACP execution on `workspace_root`. Proof ownership is also incomplete: the promised focused tests are only partially present, `docs/reference/test-gates.md` still has no `proposal-044` entry, and the same-tree `proposal-044` gate is currently red.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Post-approval worktree safety is still missing | High |
| Architecture | At Risk | Safety/provisioning still keys off `state.tasks`, not effective post-approval tasks | High |
| Product | At Risk | Manual release can fall back to the live workspace instead of failing closed on missing worktree | High |
| UI | Acceptable | No proposal-specific UI surface is directly in scope | Medium |
| UX | Acceptable | Approval/retry semantics are clear in code, but not end-to-end runtime-proved | Medium |
| Readiness | Not Ready | `proposal-044` proof lane is red and the canonical docs gate entry is absent | High |

## Proposal Contract

### Scope

- Implement `run_after_approval` task enqueuing with correct effective-task ownership.
- Generalize binary phase gating to N-phase sequential ordering for both `sequence` and multi-task `then`.
- Ensure end states with `run` blocks execute tasks before run completion.

### Locked Decisions

- `run_after_approval.sequence` tasks execute in strict declared order.
- Multi-task `then` blocks are sequential, not a single shared phase.
- For manual release failure, retry returns to `WaitingApproval` and requires fresh approval.
- Post-approval release tasks use the provisioned implementation worktree and must fail closed if it is missing.
- Proof is repo-owned through a `proposal-044` gate plus a `test-gates.md` entry.

### Primary User Flows

1. Operator approves `state_11_manual_release`, post-approval tasks execute in strict order, and the stage settles only after both phases complete.
2. The run transitions into `state_12_workflow_complete`, executes `finalize_run_and_produce_receipts`, and only then marks the run completed.
3. A failed post-approval release stage is retried as a fresh approval gate rather than silently resuming irreversible release work.

### UI Commitments

- No new screen/layout contract is introduced.
- Operator-facing proof for this slice lives in gate/test ownership and in the workflow state progression, not in new UI chrome.

### UX Commitments

- Approval must precede post-approval execution.
- Sequential release ordering must be deterministic and data-dependency safe.
- Missing worktree must block release execution rather than silently downgrading to a weaker path.
- Retry after post-approval failure must require fresh human approval.

### Acceptance Criteria

1. No regression on state_4 single-task `then`.
2. State_9 multi-task `then` executes auditor -> prepush -> aggregation in numeric phase order.
3. Simple manual gates still settle `Completed` after approval.
4. State_11 executes `commit_and_push` before `build_and_distribute`.
5. State_11 approval advances through Running and then transitions to state_12.
6. State_12 runs `finalize_run_and_produce_receipts` before run completion.
7. Failed phase skips later phases and blocks the run.
8. Retry after failed state_11 requires fresh approval.
9. Missing worktree is blocked by `RepoSafetyGuard`.
10. Work-item payloads retain correct `task_index` / phase ordering truth.

### Test / Evidence Requirements

- Add focused proof tests for post-approval settle/end-state completion.
- Add a repo-owned `proposal-044` gate.
- Add a matching `docs/reference/test-gates.md` entry.
- Same-tree gate execution should prove the slice on the audited tree.

### Explicit Exclusions

- No dependency on real release side effects from Proposal 045.
- No expansion into broader UI redesign or non-orchestration product work.

## Proposal Fidelity / Divergence

### Matches

- `compile_run_block(...)` now assigns incrementing phases for `sequence` and multi-task `then`.
- Manual-gate approval checks `post_approval_tasks` and leaves release gates `Running`.
- The orchestrator now resolves `effective_tasks(...)` for post-approval contexts and uses generalized N-phase settlement logic.
- End states with tasks now fall through to the compute path and complete after `evaluate_and_transition(...)`.
- Retry scheduling still recreates a fresh manual-gate attempt through the normal pending-stage path.

### Divergences

- Post-approval worktree provisioning and `RepoSafetyGuard` still inspect `state.tasks` / `state.owner`, not `post_approval_tasks` through `effective_tasks(...)`.
- The proposal-promised focused proof inventory is only partially present.
- `docs/reference/test-gates.md` still has no `proposal-044` entry.
- Same-tree `proposal-044` gate is currently red.

### Ambiguities / Evidence Gaps

- No direct executed same-tree happy-path proof was found for `state_11 -> state_12 -> finalize_run_and_produce_receipts`.
- No dedicated executed proof was found for runtime multi-task `then` started-at ordering; current evidence is compile-time and code-path based.
- No UI/runtime walkthrough was performed because this slice is daemon-only.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 7 |
| Partially Implemented | 1 |
| Missing | 3 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Sequential phase assignment for `sequence` and multi-task `then`
- Proposal Source: §3a; AC-1, AC-2, AC-4
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:256`
  - `control-plane/crates/workflow/tests/integration.rs:113`
  - `cargo test -p workflow --test integration test_compile_n_phase_ordering -- --exact` -> `1 passed`
- Gap / Note: Compile-time proof covers state_11, state_9, and state_4 phase assignment.

### REQ-002 Approving a manual release gate with `post_approval_tasks` leaves the stage `Running`
- Proposal Source: §3d; §3f; AC-5
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:185`
  - `control-plane/crates/engine/tests/integration.rs:755`
  - `cargo test -p engine --test integration test_approve_ -- --nocapture` -> `3 passed`
- Gap / Note: Narrow executed proof exists for the manual-gate approval state transition.

### REQ-003 Post-approval execution resolves `effective_tasks(...)` and kickstarts phase 0
- Proposal Source: §3c; §3e
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:176`
  - `control-plane/crates/engine/src/orchestrator.rs:1238`
- Gap / Note: No dedicated executed proof was found for the zero-invoke post-approval kickstart path, but the runtime branch is present and wired to `effective_tasks(...)`.

### REQ-004 Runtime N-phase settlement uses `task_index` / `phase` ordering truth
- Proposal Source: §3b; AC-2, AC-4, AC-10
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:205`
  - `control-plane/crates/engine/src/orchestrator.rs:632`
- Gap / Note: The runtime N-phase algorithm is present, and work-item payloads include `task_index`, but the proposal-promised runtime ordering test is still missing.

### REQ-005 Simple manual gates remain a no-regression `Completed` path
- Proposal Source: §3f; AC-3
- Status: Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:198`
  - `control-plane/crates/engine/tests/integration.rs:805`
  - `cargo test -p engine --test integration test_approve_ -- --nocapture` -> `3 passed`
- Gap / Note: The narrow regression proof currently exists only through the generic `test_approve_` run.

### REQ-006 End states with tasks execute before run completion
- Proposal Source: §3h; AC-6
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/orchestrator.rs:378`
  - `control-plane/crates/engine/src/orchestrator.rs:811`
- Gap / Note: Code now clearly falls through for end states with tasks, but the proposal-promised focused runtime test is still absent.

### REQ-007 Retrying a failed post-approval stage re-enters a fresh approval gate
- Proposal Source: §3g; AC-8
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:304`
  - `control-plane/crates/engine/src/orchestrator.rs:411`
- Gap / Note: Retry scheduling creates a new pending stage attempt, and the normal manual-gate path recreates `WaitingApproval`; no dedicated executed proof was found for the state_11 retry case.

### REQ-008 Missing worktree must be blocked for post-approval release tasks
- Proposal Source: §3i; AC-9
- Status: Missing
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:256`
  - `examples/workflows/full-mvp-live.yaml:345`
  - `examples/agents/agents.yaml:1015`
  - `examples/agents/agents.yaml:1305`
  - `examples/agents/agents.yaml:1368`
  - `control-plane/crates/engine/src/orchestrator.rs:451`
  - `control-plane/crates/engine/src/orchestrator.rs:552`
  - `control-plane/crates/acp/src/transport.rs:440`
- Gap / Note: `state_11_manual_release` stores its release work in `post_approval_tasks`, but worktree provisioning and `RepoSafetyGuard` still inspect only `state.tasks` and `state.owner`. `lead_orchestrator` has no worktree policy, while the two post-approval release agents are `strategy: dedicated`. If `run.worktree_root` is missing, ACP transport falls back to `workspace_root` instead of blocking. That violates the proposal's fail-closed worktree contract.

### REQ-009 Proposal-owned focused proof tests are landed
- Proposal Source: §4; §7 `PROPOSAL_044_TESTS`
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:341`
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:401`
  - `scripts/test-gate.sh:178`
  - `cargo test -p engine --test integration test_approve_ -- --nocapture` -> `3 passed`
  - `cargo test -p workflow --test integration test_compile_n_phase_ordering -- --exact` -> `1 passed`
- Gap / Note: Only the approval regression tests and compile-time phase test exist. The proposal-promised focused tests for post-approval enqueue/settle, end-state completion, and runtime multi-task `then` ordering were not found.

### REQ-010 `docs/reference/test-gates.md` contains a canonical `proposal-044` entry
- Proposal Source: §7 `test-gates.md Entry`
- Status: Missing
- Evidence Type: `code`
- Evidence:
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:374`
  - `rg -n "proposal-044" docs/reference/test-gates.md` -> exit `1` (no match)
- Gap / Note: The gate exists in `scripts/test-gate.sh`, but the canonical reference doc was not updated.

### REQ-011 Same-tree `proposal-044` gate passes
- Proposal Source: §7; Test / Evidence Requirements
- Status: Missing
- Evidence Type: `tests-run`
- Evidence:
  - `scripts/test-gate.sh:1463`
  - `bash 'scripts/test-gate.sh' proposal-044` -> exit `101`
  - failing test: `control-plane/crates/engine/tests/integration.rs::test_startup_repair_skips_clean_runs`
- Gap / Note: The failing test is adjacent rather than obviously proposal-specific, but the repo-owned proof lane is still red on the audited tree, so the proposal's proof contract is not yet satisfied.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Post-approval worktree safety still bypasses the effective-task model
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: §3c, §3i; REQ-008
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:256`
  - `control-plane/crates/engine/src/orchestrator.rs:451`
  - `control-plane/crates/engine/src/orchestrator.rs:552`
  - `control-plane/crates/acp/src/transport.rs:440`
- Why It Matters: The proposal explicitly moved release execution onto `post_approval_tasks`, but worktree provisioning and safety validation are still attached to pre-approval `state.tasks` and owner metadata. That leaves the most safety-sensitive release step outside the same fail-closed boundary model used by write-enabled implementation stages.
- Recommended Action: Derive both worktree provisioning and `RepoSafetyGuard` checks from `effective_tasks(state, is_post_approval)` for manual-gate post-approval contexts, then add a focused regression test where `state_11` runs with `worktree_root = None` and blocks before agent launch.

## Product Review

**Summary:** At Risk

### PROD-001 Manual release can degrade to the live workspace instead of failing closed
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Goal; §2 answer 3; §3i; REQ-008
- Evidence Type: `code`
- Evidence:
  - `examples/workflows/full-mvp-live.yaml:345`
  - `examples/agents/agents.yaml:1311`
  - `examples/agents/agents.yaml:1374`
  - `control-plane/crates/acp/src/transport.rs:445`
- Why It Matters: The operator-facing product promise is that approved release work runs against the isolated implementation worktree. Quietly falling back to `workspace_root` undermines the release mental model and risks committing or publishing the wrong tree if prior provisioning drifted or was lost.
- Recommended Action: Make missing-worktree failure explicit in the release path, surface it as blocked run truth, and prove it with a repo-owned test before treating this slice as shippable.

## UI Review

**Summary:** Acceptable

No standalone UI finding was generated in this pass. Proposal 044 is a control-plane orchestration slice; it does not introduce new window, screen, or component commitments beyond the correctness of the underlying workflow progression.

## UX Review

**Summary:** Acceptable

No standalone UX finding was generated beyond the readiness issues already called out. Approval, retry, and end-state semantics are clear in code, but they are not yet fully backed by the proposal-promised focused proof set.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The repo-owned `proposal-044` proof lane is still red on the audited tree
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: §7; REQ-011
- Evidence Type: `tests-run`
- Evidence:
  - `scripts/test-gate.sh:1463`
  - `bash 'scripts/test-gate.sh' proposal-044` -> exit `101`
- Why It Matters: Proposal 044 explicitly owns a repo-level proof lane. On the current tree, that lane does not produce a green same-tree artifact, which means the slice cannot be signed off as ready even if some feature code has landed.
- Recommended Action: Make `proposal-044` green on the same tree, either by fixing the failing workspace test or by narrowing the lane to the proposal-owned proof inventory if that is the intended contract.

### READY-002 Proof ownership is still incomplete in both script inventory and canonical docs
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: §4; §7; REQ-009; REQ-010
- Evidence Type: `code`
- Evidence:
  - `scripts/test-gate.sh:178`
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:341`
  - `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md:374`
  - `rg -n "proposal-044" docs/reference/test-gates.md` -> exit `1`
  - `rg -n "test_post_approval_tasks_enqueued_and_settled|test_end_state_runs_tasks_before_completion|test_multi_task_then_sequential_ordering|test_simple_manual_gate_no_regression|test_n_phase_sequence_ordering" control-plane/crates` -> exit `1`
- Why It Matters: Future auditors and implementers cannot reliably reproduce or defend this slice if the promised tests and the canonical gate reference doc never land. The current tree forces too much inference from code.
- Recommended Action: Land the missing focused tests, align `PROPOSAL_044_TESTS` with the proposal text, and add the `proposal-044` section to `docs/reference/test-gates.md`.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Rust control-plane crates built as part of targeted test execution; no Apple app build was run because this slice is daemon-only. |
| Core user flow runtime-validated | Partial | Narrow approval/compile proofs passed, but no executed end-to-end `state_11 -> state_12` happy-path proof was found. |
| Empty/loading/error states covered | Not Checked | No UI scope in this proposal. |
| Accessibility risk acceptable | Not Checked | No UI scope in this proposal. |
| Localization risk acceptable | Not Checked | No operator copy/UI surface was audited. |
| Critical tests executed | Partial | Targeted approval + compile tests passed, but same-tree `proposal-044` gate is red. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not run after the audit already failed closed on proposal-specific gaps. |
| Privacy/permissions/entitlements reviewed | Not Checked | Not applicable to this daemon orchestration slice. |

## Verification Log

- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md'`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "RepoSafetyGuard|worktree|effective_tasks|post_approval_tasks|finalize_run_and_produce_receipts|task_index|proposal-044|PROPOSAL_044_TESTS" control-plane/crates scripts docs/reference/test-gates.md`
- `sed -n '160,275p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '445,575p' control-plane/crates/engine/src/orchestrator.rs`
- `sed -n '730,900p' control-plane/crates/engine/tests/integration.rs`
- `sed -n '100,203p' control-plane/crates/workflow/tests/integration.rs`
- `sed -n '260,420p' docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md`
- `sed -n '1298,1382p' examples/agents/agents.yaml`
- `sed -n '330,390p' examples/workflows/full-mvp-live.yaml`
- `sed -n '210,245p' control-plane/crates/engine/src/executor.rs`
- `sed -n '432,450p' control-plane/crates/acp/src/transport.rs`
- `sed -n '198,220p' control-plane/crates/engine/src/worktree.rs`
- `rg -n "proposal-044" docs/reference/test-gates.md`
- `rg -n "test_post_approval_tasks_enqueued_and_settled|test_end_state_runs_tasks_before_completion|test_multi_task_then_sequential_ordering|test_simple_manual_gate_no_regression|test_n_phase_sequence_ordering" control-plane/crates`
- `bash 'scripts/test-gate.sh' proposal-044` -> failed with exit `101` (`test_startup_repair_skips_clean_runs`)
- `cargo test -p engine --test integration test_approve_ -- --nocapture` -> `3 passed`
- `cargo test -p workflow --test integration test_compile_n_phase_ordering -- --exact` -> `1 passed`

## Recommended Next Actions

1. Fix the real runtime gap first: move worktree provisioning and `RepoSafetyGuard` evaluation to the effective post-approval task set, then add a focused missing-worktree regression for `state_11`.
2. Land the proposal-promised focused tests for post-approval enqueue/settle, end-state completion, and runtime multi-task `then` ordering.
3. Add the canonical `proposal-044` section to `docs/reference/test-gates.md` and make the repo-owned gate green on the same tree before claiming proposal completion.
