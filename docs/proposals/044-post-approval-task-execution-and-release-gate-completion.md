# Proposal 044: Post-Approval Task Execution and Release Gate Completion

| Field | Value |
|---|---|
| Date | 2026-04-14 |
| Status | Draft |
| Author | Claude |
| Depends on | None. P044 is orchestration-only; its proof uses fixture tasks that do not perform real release side effects. P045 depends on P044, not the reverse. |
| Scope | (A) Implement `run_after_approval` task enqueuing with correct effective-task ownership. (B) Generalize the binary phase 0/1 model to N-phase sequential ordering so both `sequence` and multi-task `then` blocks execute in declared order (fixing state_9 auditor→prepush→aggregation and state_11 commit→publish). (C) Fix end-state task execution so `is_end` states with a `run` block execute their tasks before settling. |
| Goal | A workflow run that reaches state_11, receives human approval, executes post-approval tasks **in strict declared sequence**, and transitions to state_12 where the end-state `finalize_run_and_produce_receipts` task executes before run completion. |

---

## 1. Context and Motivation

The Rust daemon handles manual gates by creating an Approval record and pausing the run in `WaitingApproval`. When approval is granted (`command_handler.rs:159-233`), the current code:

1. Resolves the approval as `Granted`
2. **Immediately settles the stage as `Completed`** (line 191-197 — `stages::settle(... Completed)`)
3. Enqueues `AdvanceRun`
4. Orchestrator sees Completed stage → `evaluate_and_transition`
5. Transition requires `exists('git_push_receipt')` — artifact doesn't exist
6. **No transition matches → run blocks forever**

State_11 in `full-mvp-live.yaml` defines a `run_after_approval` block with two **sequential** agents:

```yaml
state_11_manual_release:
  label: Manual release
  type: manual_gate
  owner: lead_orchestrator
  approval: required
  approval_policy: manual_release
  run_after_approval:
    sequence:
      - agent: commit_and_push_to_github
        task: commit_and_push
        inputs: [approved_proposal, implementation_review_summary, docs_report, prepush_review_report]
        outputs: [release_manifest, git_push_receipt]
      - agent: build_archive_and_push_connect
        task: build_and_distribute
        inputs: [git_push_receipt, release_manifest]
        outputs: [release_bundle_manifest, connect_upload_receipt]
  transitions:
    - to: state_12_workflow_complete
      when: exists('git_push_receipt')
```

Two independent bugs block this path:
1. **No post-approval task enqueuing.** `CompiledState.post_approval_tasks` is populated by the compiler but never consumed at runtime.
2. **No sequence ordering.** The compiler assigns all `sequence` tasks `phase = 0` (`compiler.rs:332-338`), and the orchestrator enqueues all phase-0 tasks simultaneously. `build_and_distribute` would race with `commit_and_push` instead of waiting for `git_push_receipt`.

**Note:** state_12 is not a bare end state — it has a `run` block where `lead_orchestrator` executes `finalize_run_and_produce_receipts`.

---

## 2. Product Questions

1. When an approval is granted on a `manual_release` gate, do the `run_after_approval` tasks execute before or after transition evaluation?
2. If a post-approval task fails, does the run block or retry? Does retry require fresh approval?
3. Do post-approval tasks require a provisioned worktree?
4. Can release side effects execute through ACP before deterministic services exist?
5. Must `sequence` tasks within `run_after_approval` execute in declared order?

**Answers (matching Swift `WorkflowOrchestrator.resumeAfterApproval` and stable delivery contract):**
1. **After approval, before transition.** Swift calls `executeRunBlock(runAfterApproval)` first (line 252-260), then evaluates transitions (line 271-280). If `runAfterApproval` fails, `handleFailure` is called and transitions are never evaluated.
2. Run blocks. **Retry on a `manual_release` gate returns the stage to `WaitingApproval` and requires fresh human approval.**
3. **Yes.** Both agents have `worktree_policy.strategy: dedicated`.
4. **No.** The stable delivery contract requires deterministic services. P044 proof uses fixture tasks.
5. **Yes.** `build_and_distribute` declares `git_push_receipt` as input — it cannot execute before `commit_and_push` produces it. The compiler and orchestrator must enforce this ordering.

---

## 3. Design

### 3a. N-Phase Sequential Ordering for `sequence` Blocks

**Problem:** The current compiler assigns all `sequence` tasks `phase = 0`:

```rust
// compiler.rs:332-338 — current behavior
if let Some(seq) = &rb.sequence {
    for at in seq {
        let mut t = compile_agent_task(at, agents, contracts, false)?;
        t.phase = 0;  // BUG: all sequence tasks get the same phase
        tasks.push(t);
    }
}
```

**Fix:** Assign `sequence` tasks incrementing phase numbers so they execute in declared order:

```rust
// compiler.rs — new behavior
if let Some(seq) = &rb.sequence {
    for (idx, at) in seq.iter().enumerate() {
        let mut t = compile_agent_task(at, agents, contracts, false)?;
        t.phase = idx as u32;  // 0, 1, 2, ... — strict ordering
        tasks.push(t);
    }
}
```

**`parallel` tasks** remain at phase 0 (unchanged).

**`then` tasks** also get incrementing phases — they are sequential-by-contract per the DSL spec (`then: [AgentTask]?` — "sequential tasks after parallel fan-out completes"). The current workflow depends on this: state_9's `then` block has `proposal_implementation_auditor` → `prepush_code_reviewer` (consumes `audit_report`) → `lead_orchestrator` aggregation (consumes both). Assigning a single shared phase would enqueue them together, violating the declared data dependency chain.

```rust
let mut next_phase = tasks.iter().map(|t| t.phase).max().unwrap_or(0) + 1;
if let Some(then) = &rb.then {
    for at in then {
        let mut t = compile_agent_task(at, agents, contracts, false)?;
        t.phase = next_phase;
        next_phase += 1;  // each then task gets its own phase — strict ordering
        tasks.push(t);
    }
}
```

**Phase assignment summary for `full-mvp-live.yaml` state_9:**

| Task | Block | Phase |
|---|---|---|
| `security_checker` | parallel | 0 |
| `docs_guardian` | parallel | 0 |
| `proposal_implementation_auditor` | then[0] | 1 |
| `prepush_code_reviewer` | then[1] | 2 |
| `lead_orchestrator` (aggregation) | then[2] | 3 |

Execution: security+docs run concurrently (phase 0) → auditor (phase 1) → prepush (phase 2) → aggregation (phase 3).

**Phase assignment for state_4 (existing parallel+then):**

| Task | Block | Phase |
|---|---|---|
| `proposal_reviewer_product_owner` | parallel | 0 |
| `proposal_reviewer_ux` | parallel | 0 |
| `proposal_reviewer_ui` | parallel | 0 |
| `proposal_reviewer_architect` | parallel | 0 |
| `lead_orchestrator` (aggregation) | then[0] | 1 |

Execution: 4 reviewers concurrently (phase 0) → aggregation (phase 1). Identical to current behavior.

### 3b. Generalized Phase Gating in Orchestrator

**Problem:** The orchestrator's settlement logic is binary — it knows about phase 0 and phase 1 only:

```rust
// current: hard-coded phase 1 check
let has_phase1 = state.tasks.iter().any(|t| t.phase == 1);
```

**Fix:** Generalize to "find the next unenqueued phase":

```rust
// Determine which phase just completed.
let current_phase = stage_invokes.iter()
    .filter_map(|w| {
        serde_json::from_str::<serde_json::Value>(&w.payload_json).ok()
            .and_then(|v| v.get("task_index")?.as_u64())
            .and_then(|idx| effective.get(idx as usize))
            .map(|t| t.phase)
    })
    .max()
    .unwrap_or(0);

// Find the next phase (if any).
let next_phase = effective.iter()
    .map(|t| t.phase)
    .filter(|p| *p > current_phase)
    .min();

let next_phase_enqueued = next_phase.map_or(true, |np| {
    stage_invokes.iter().any(|w| {
        serde_json::from_str::<serde_json::Value>(&w.payload_json).ok()
            .and_then(|v| v.get("task_index")?.as_u64())
            .map(|idx| effective.get(idx as usize).map_or(false, |t| t.phase == np))
            .unwrap_or(false)
    })
});

if let Some(np) = next_phase {
    if !next_phase_enqueued {
        if failed > 0 {
            warn!("Phase {current_phase} had failures — skipping phase {np}, settling as Failed");
        } else {
            info!("Phase {current_phase} complete — enqueuing phase {np} tasks");
            for (i, task) in effective.iter().enumerate() {
                if task.phase != np { continue; }
                let prompt = build_task_prompt(task, &plan, run, idea_opt.as_ref(), None);
                self.enqueue_invoke_agent(run_id, stage, &task.agent, &prompt, i, effective.len()).await?;
            }
            return Ok(());
        }
    }
}
```

This replaces the binary `has_phase1 / phase1_enqueued` check with a general N-phase gating loop. Phase 0 tasks are enqueued first; when all complete, phase 1 tasks are enqueued; when those complete, phase 2, etc.

**For state_4's existing parallel+then pattern (phases 0 and 1):** behavior is identical — phase 0 tasks run first, then phase 1.

**For state_11's sequence pattern (phases 0, 1):** `commit_and_push` (phase 0) runs first; when complete, `build_and_distribute` (phase 1) is enqueued.

### 3c. Effective Task List Resolution — The Owner Bridge

When AdvanceRun finds a Running manual_gate stage with a granted approval, the orchestrator must use `post_approval_tasks` as the effective task list:

```rust
fn effective_tasks(state: &CompiledState, is_post_approval: bool) -> &[CompiledTask] {
    if is_post_approval && !state.post_approval_tasks.is_empty() {
        &state.post_approval_tasks
    } else {
        &state.tasks
    }
}
```

**How `is_post_approval` is determined:** When AdvanceRun finds a Running manual_gate stage, it checks if the approval for this stage is `Granted`. If yes and `post_approval_tasks` is non-empty, this is a post-approval execution.

**All downstream accounting uses the effective list:** phase detection, task_index mapping, completion counting, total_tasks, and the N-phase gating loop.

### 3d. Change: command_handler.rs — Don't Settle Manual Gates Immediately

**Current behavior** (line 190-197):
```rust
if stage.stage_type.as_deref() == Some("manual_gate") {
    stages::settle(&self.pool, stage.id, StageSettlementKind::Completed, now).await?;
}
```

**New behavior:** If the manual_gate state has `post_approval_tasks`, set stage to `Running` instead of settling as `Completed`:

```rust
if stage.stage_type.as_deref() == Some("manual_gate") {
    let has_post_tasks = self.check_has_post_approval_tasks(c.run_id, &c.stage_id).await;
    if has_post_tasks {
        stages::update_status(&self.pool, stage.id, StageStatus::Running).await?;
    } else {
        stages::settle(&self.pool, stage.id, StageSettlementKind::Completed, now).await?;
    }
}
```

**AdvanceRun is already enqueued** (line 218-225) — no change needed there.

### 3e. Change: orchestrator.rs — Post-Approval Task Enqueuing

When AdvanceRun finds a Running manual_gate stage with zero InvokeAgent work items and `is_post_approval` is true, enqueue phase 0 tasks from the effective list:

```rust
if total == 0 && is_post_approval {
    info!(run_id = %run_id, state = %current_state_id, "Enqueuing post-approval phase 0 tasks");
    let idea_opt = ideas::find_by_id(&self.pool, run.idea_id).await.ok().flatten();
    let effective_total = effective.len();
    for (i, task) in effective.iter().enumerate().filter(|(_, t)| t.phase == 0) {
        let prompt = build_task_prompt(task, &plan, run, idea_opt.as_ref(), None);
        self.enqueue_invoke_agent(run_id, stage, &task.agent, &prompt, i, effective_total).await?;
    }
    return Ok(());
}
```

When phase 0 completes, the N-phase gating logic (3b) automatically enqueues phase 1, etc.

### 3f. Stage Status Lifecycle

**Simple manual gate (no post_approval_tasks, e.g. state_3, state_6):**
```
Pending → WaitingApproval → [approval] → Completed → evaluate_and_transition
```
No change — same as today.

**Release gate with post_approval_tasks (state_11):**
```
Pending → WaitingApproval → [approval] → Running
  → phase 0: commit_and_push → [completes]
  → phase 1: build_and_distribute → [completes]
  → Completed → evaluate_and_transition
```

### 3g. Retry After Post-Approval Failure

When a post-approval task fails, the stage settles as Failed and the run blocks. On `RetryStage`:

- The stage is reset (new attempt, `StageStatus::Pending`)
- Because `state.is_manual_gate` is true, the orchestrator re-enters the manual gate path
- A **new Approval record** is created with `Requested` decision
- The stage goes to `WaitingApproval` — **operator must approve again**

This is intentional. Release side effects are irreversible. Fresh human approval ensures the operator has reviewed the failure.

### 3h. Fix: End-State Task Execution (`is_end` with `run` Block)

**Problem:** `orchestrator.rs:286-311` short-circuits `is_end` states to immediate completion:

```rust
if state.is_end {
    self.create_stage_for_state(run_id, &current_state_id, state).await?;
    stages::settle(&self.pool, end_stage.id, StageSettlementKind::Completed, now).await?;
    runs::mark_completed(&self.pool, run_id, now).await?;
    return Ok(());
}
```

State_12 (`state_12_workflow_complete`) is `type: end` but has a `run.sequence` block where `lead_orchestrator` executes `finalize_run_and_produce_receipts` producing `delivery_receipt`, `run_report`, and `run_state`. The current code never runs these tasks.

**Fix:** If an end state has tasks, treat it as a regular compute state first — create stage, enqueue tasks, wait for completion, then settle and mark completed:

```rust
if state.is_end {
    if state.tasks.is_empty() {
        // No tasks — settle immediately (current behavior for bare end states)
        self.create_stage_for_state(run_id, &current_state_id, state).await?;
        stages::settle(&self.pool, end_stage.id, Completed, now).await?;
        runs::mark_completed(&self.pool, run_id, now).await?;
        self.cleanup_worktree_if_needed(&run).await;
        return Ok(());
    }
    // End state with tasks — fall through to regular compute-state handling.
    // The evaluate_and_transition path will see is_end + no transitions
    // and mark the run completed after tasks finish.
}
```

The existing `evaluate_and_transition` already handles the case where `state.transitions.is_empty() || state.is_end` by marking the run completed (line ~654). So the fix is: remove the early return for end states with tasks, let them fall through to the regular task enqueue path.

### 3i. Worktree Safety

Post-approval tasks for state_11 include agents with `worktree_policy.strategy: dedicated`. The existing `RepoSafetyGuard.validate_worktree_ready` check covers this — `any_agent_needs_worktree` now operates on the effective task list.

---

## 4. Files to Modify

| File | Change | Lines (approx.) |
|---|---|---|
| `workflow/src/compiler.rs` | (1) `compile_run_block`: assign `sequence` tasks incrementing phases (0, 1, 2...) instead of all phase 0. (2) `then` tasks also get incrementing phases (`next_phase += 1` per task) instead of a single shared phase — fixing state_9 multi-task `then` ordering (auditor→prepush→aggregation). | ~10 LOC |
| `engine/src/orchestrator.rs` | (1) Add `effective_tasks` resolution. (2) Generalize binary phase 0/1 check to N-phase gating. (3) Add post-approval task enqueuing when `total == 0 && is_post_approval`. (4) Worktree safety uses effective list. (5) Fix `is_end` handler: end states with tasks fall through to compute path instead of short-circuiting. | ~70 LOC |
| `engine/src/command_handler.rs` | In `ApproveStage`, conditional settlement: `post_approval_tasks` exist → `Running`, else → `Completed`. Add `check_has_post_approval_tasks` helper. | ~25 LOC |
| `engine/tests/integration.rs` | Add `test_post_approval_tasks_enqueued_and_settled` and `test_end_state_runs_tasks_before_completion` focused proof tests. | ~100 LOC |

---

## 5. Acceptance Criteria

1. **No regression on single-task `then`:** State_4's fan-out (4 parallel reviewers phase 0 + 1 aggregator phase 1) still works identically.
2. **Multi-task `then` ordering:** State_9's `then` block: auditor (phase 1) completes **before** prepush_reviewer (phase 2) is enqueued, prepush_reviewer completes **before** aggregation (phase 3) is enqueued. Verified by work_item `started_at` timestamps.
3. **No regression on simple gates:** State_6 (simple manual gate, no `run_after_approval`) still works: approval → Completed → transition.
4. **Sequential ordering:** State_11 post-approval: `commit_and_push` (phase 0) completes **before** `build_and_distribute` (phase 1) is enqueued.
5. **Happy path:** State_11 approval → Running → phase 0 → phase 1 → stage Completed → `exists('git_push_receipt')` = true → transition to state_12.
6. **End-state task execution:** State_12 (`is_end` with `run` block) executes `finalize_run_and_produce_receipts` and produces `delivery_receipt` + `run_report` before run is marked Completed.
7. **Phase failure propagation:** If any phase fails → all later phases skipped → stage Failed → run blocks. Artifacts from completed earlier phases are already persisted.
8. **Retry requires re-approval:** Retrying failed state_11 returns to `WaitingApproval`.
9. **Worktree guard:** Missing worktree → `RepoSafetyGuard` blocks.
10. **DB correctness:** `work_items` for state_11 have `task_index` values mapping to `post_approval_tasks`; phases execute in strict numeric order.

---

## 6. Relationship to Other Proposals

| Proposal | Direction | Relationship |
|---|---|---|
| **P045 (Deterministic Release Ops)** | P045 depends on P044 | P044 provides the orchestration (enqueuing, effective-task resolution, N-phase ordering, settlement). P045 adds native Rust execution for release agents. **P044 does not depend on P045** — P044 proof uses non-side-effectful fixture tasks. |
| **P007 (Worktree Provisioning)** | P044 uses P007 | Post-approval release agents need a provisioned worktree from state_7. Already implemented. |
| **P046 (Output Envelope + Contracts)** | Independent | P044's proof does not depend on P046. |

---

## 7. Test Gate

P044's proof lane follows the P027 pattern (Rust daemon-only, `cargo test`).

### test-gates.md Entry

```
### `proposal-044`

Post-approval task execution and release gate completion gate.

Scope:

- N-phase sequential ordering for `sequence` and multi-task `then` blocks
- post-approval effective-task resolution and N-phase enqueuing
- end-state task execution before run completion
- multi-task `then` ordering (state_9: auditor → prepush → aggregation)
- no regression on single-task `then` settlement (state_4)
- no regression on simple manual gates (state_3, state_6)

Command:

\`\`\`bash
./scripts/test-gate.sh proposal-044
\`\`\`
```

### test-gate.sh Entry

```bash
PROPOSAL_044_TESTS=(
  "engine::tests::test_post_approval_tasks_enqueued_and_settled"
  "engine::tests::test_end_state_runs_tasks_before_completion"
  "engine::tests::test_n_phase_sequence_ordering"
  "engine::tests::test_multi_task_then_sequential_ordering"
  "engine::tests::test_simple_manual_gate_no_regression"
)
```

Gate runner:

```bash
proposal-044|p044)
  log "Proposal 044 control-plane gate: post-approval + N-phase + end-state"
  (
    cd "$ROOT_DIR/control-plane"
    cargo test --workspace 2>&1
  )
  log "Proposal 044 control-plane gate passed"
  ;;
```

**Note:** The focused test names above are the minimum proof. The gate runs the full Rust workspace test suite (same as P027) to catch regressions in existing orchestrator/settlement logic.

---

## 8. Out of Scope

- **Deterministic release services**: Covered by P045. P044 does not define how release agents produce their artifacts.
- **ACP-owned release execution**: Explicitly **not permitted**. P044 proof uses fixture tasks or stub agents.
- **Release receipt formatting**: Agent responsibility; out of scope for orchestration.
