# Proposal 064: Run Worktree Main Sync and Cross-Run Knowledge Transfer

| Field | Value |
|---|---|
| Date | 2026-04-21 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [../reference/per-run-workspace-isolation.md](../reference/per-run-workspace-isolation.md), [045-run-recovery-and-granular-retry-mcp-tools.md](045-run-recovery-and-granular-retry-mcp-tools.md), [rust-control-plane.md#capacity-aware-scheduling-and-backpressure](../reference/rust-control-plane.md#capacity-aware-scheduling-and-backpressure), [062-implementation-approval-rejection-loopback.md](062-implementation-approval-rejection-loopback.md) |
| Scope | Make implementation run worktrees stay current with `main` without blocking or losing work, and make completed-run lessons available to unfinished runs through durable orchestration artifacts rather than manual operator memory. |
| Goal | The orchestrator can safely synchronize `main` into active implementation branches, preserve dirty work before any merge attempt, route merge conflicts as normal work, and inject relevant knowledge from completed or failed runs into later run prompts. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-064|p064`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context and Motivation

Parallel implementation runs are now a normal Chainworks Forge operating mode. During dogfooding on 2026-04-21, several implementation worktrees drifted behind `main` while other proposals landed fixes, proposal documents, review-router updates, and engine behavior changes.

Manual sync exposed three problems:

- dirty run worktrees cannot safely fast-forward when `main` touched the same files;
- conflict handling currently becomes an operator side task instead of a first-class run task;
- useful lessons from completed or manually closed runs are not automatically handed to still-running proposals that touch the same code paths.

The immediate operator need is simple: keep work progressing without blocking. The system need is stronger: preserve work first, sync `main` deliberately, and turn conflicts or prior-run knowledge into durable inputs for the next agent pass.

### 1.1 Dogfood evidence from 2026-04-21

A manual sync attempt against active implementation worktrees showed the exact shape this proposal must automate:

| Branch | Result | Evidence |
|---|---|---|
| `cw/implement-proposal-053-bounded/4b3a582a` | Preserved dirty work, then merged `main` successfully after resolving the two loop-budget conflicts through the already-landed `main` version. | `main` became an ancestor of the branch; worktree clean. |
| `cw/implement-proposal-051-shared/43460545` | Preserved dirty work, merge produced 18 conflicted files, merge attempt was aborted cleanly. | Needs a follow-up conflict-resolution work item rather than a blocked operator chore. |
| `cw/implement-proposal-061-sqlite/b57f18ef` | Preserved dirty work, merge produced 16 conflicted files, merge attempt was aborted cleanly. | Needs a follow-up conflict-resolution work item rather than a blocked operator chore. |

This is the desired manual fallback behavior, but it should be orchestrated: preserve first, attempt sync, record conflict evidence, abort failed merge, and schedule focused resolution.

---

## 2. Problem Statement

### 2.1 Run worktrees drift from `main`

Long-running implementation loops may run for hours or days. Meanwhile `main` can gain:

- engine bug fixes;
- proposal or reference document corrections;
- test-gate changes;
- agent catalog and reviewer-routing changes;
- database, GraphQL, MCP, or workflow-contract changes.

If a run keeps working from an old base, later reviews and merges pay the integration cost all at once.

### 2.2 Dirty worktree merge is not currently orchestrated

The safe sequence is not "just merge main":

1. capture current dirty state;
2. preserve untracked files;
3. make the preservation durable;
4. attempt the merge;
5. if conflicts happen, retain conflict evidence and route resolution as work;
6. never use destructive reset/checkout cleanup as the default path.

Today this sequence is manual and inconsistent.

### 2.3 Completed-run knowledge is not reused by unfinished runs

A completed, cancelled, or manually harvested run may contain important operational facts:

- reviewer findings that changed the intended implementation;
- score trajectory and unresolved blockers;
- files touched and integration hazards;
- acceptance-test evidence;
- incident retrospectives and explicit guardrails;
- decisions to move work into a follow-up proposal.

Those facts should be available to related active runs without relying on the operator to repeat them in chat.

---

## 3. Scope

P064 includes:

- an orchestrator-owned `main_sync` operation for run-owned worktrees;
- dirty-state preservation before any merge attempt;
- conflict detection, conflict evidence, and conflict-resolution work item routing;
- compact run knowledge capsules emitted by terminal or manually harvested runs;
- relevance matching from knowledge capsules into active runs;
- prompt/context injection of relevant capsules for implementation, review, retry, and refinement stages;
- GraphQL readback for sync status and knowledge-capsule provenance;
- compact MCP inspection for agents and debugging.

P064 does not include:

- UI use of MCP tools. The macOS UI remains GraphQL-only for reads, and approval-only for write controls when those are separately implemented.
- New GraphQL write paths.
- Manual reconstruction of lost implementation work.
- Treating manual closeout of unfinished implementation runs as the normal lifecycle.
- Deleting run worktrees, `.chainworks`, build outputs, or artifacts as part of sync.
- Auto-resolving semantic merge conflicts by blindly preferring `main` or a run branch.

---

## 4. Proposed Behavior

### 4.1 Main sync trigger points

The orchestrator should run `main_sync` for implementation worktrees:

- before first implementation work item starts;
- before retrying an implementation work item after a provider failure or laptop sleep/wake interruption;
- before implementation review if the branch base is stale;
- after an operator explicitly requests "sync main into active runs";
- before final implementation approval or merge readiness scoring.

The sync operation is idempotent. If `main` is already an ancestor of the run branch and the worktree has no conflicts, it records a no-op result.

### 4.2 Preservation contract

Before any merge attempt, the orchestrator must persist:

- current branch and commit;
- `git status --short`;
- binary tracked diff;
- list of untracked files;
- archive of untracked files;
- timestamp and triggering command/work item id.

The preservation artifact must be written outside the run worktree's tracked tree and indexed in run artifacts. It is not a substitute for Git history, but it gives recovery evidence if a merge tool, agent, or host interruption fails mid-operation.

### 4.3 Dirty work handling

If the worktree is dirty, the orchestrator must first create a preservation commit on the run branch:

```text
chore: preserve run work before main sync
```

That commit makes the run's current work durable before integrating `main`. The commit must be explicitly marked as orchestrator-generated metadata in the run artifact index and final implementation handoff so reviewers understand its purpose.

### 4.4 Merge strategy

The merge operation uses normal Git semantics:

- fast-forward when possible;
- normal merge commit when the branch has run commits;
- no `git reset --hard`;
- no `git checkout -- <path>`;
- no `-X ours` or `-X theirs` for broad conflict hiding;
- no fake "ours strategy" merge that marks `main` integrated without content.

If the merge succeeds, the orchestrator records:

- previous branch head;
- new branch head;
- `main` head integrated;
- merge commit id when applicable;
- preservation artifact id;
- files changed by the merge.

### 4.5 Conflict behavior

Conflicts are not a terminal block by themselves. They become normal orchestrated work.

On conflict, the orchestrator must:

1. record conflicted file paths and Git conflict diagnostics;
2. abort the merge attempt cleanly, returning to the preserved pre-merge commit;
3. create a `main_sync_conflict_resolution` work item for the lead or code writer;
4. include the conflict artifact and relevant knowledge capsules in that work item;
5. keep the run status as running/backpressured unless retry budgets or workflow rules say otherwise.

The conflict-resolution agent may then perform a focused merge, resolve conflicts, run the relevant gate, and hand back a normal implementation artifact.

### 4.6 Knowledge capsules

Every terminal, cancelled, manually harvested, or implementation-audited run should emit a compact `RunKnowledgeCapsule` artifact.

Minimum fields:

```yaml
schema_version: 1
source_run_id: uuid
source_proposal_id: string
source_status: completed|cancelled|blocked|manual_harvest|audit_failed
created_at: timestamp
main_head_at_capture: commit
summary: string
decisions:
  - string
guardrails:
  - string
changed_files:
  - path
tests_or_gates:
  - command: string
    result: passed|failed|not_run
unresolved_risks:
  - string
follow_up_proposals:
  - proposal_id: string
    path: string
relevance_tags:
  - rust-engine
  - graphql
  - mcp
  - workflow
```

Capsules must be compact enough to inject into prompts directly. Heavy evidence remains in linked artifacts.

### 4.7 Relevance matching

The orchestrator attaches capsules to active runs when any of these match:

- same proposal id or explicit successor/predecessor link;
- overlapping changed files or directories;
- shared reference document;
- shared workflow state or gate alias;
- same subsystem tags (`rust-engine`, `sqlite`, `graphql`, `mcp`, `macos-ui`, `xcode-mcp`, etc.);
- operator-pinned capsule.

The prompt should distinguish facts from suggestions. A capsule is evidence from another run, not an instruction to overwrite the current proposal.

### 4.8 Readback

GraphQL must expose:

- latest main-sync status per run;
- whether `main` is an ancestor of the implementation branch;
- last sync attempt result;
- conflict file list when present;
- attached knowledge capsule ids and provenance.

MCP may expose compact inspection for agents and debugging. MCP must not become a UI dependency.

---

## 5. Implementation Inventory

Likely implementation surfaces:

- `control-plane/crates/engine/src/orchestrator.rs`
- `control-plane/crates/engine/src/work_queue.rs`
- `control-plane/crates/engine/src/recovery.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/domain/src/run.rs`
- `control-plane/crates/domain/src/events.rs`
- `control-plane/crates/db/src/repos/runs.rs`
- `control-plane/crates/db/src/repos/work_items.rs`
- new DB migration for main-sync attempts and knowledge capsules
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/mcp-server/src/tools/runs.rs`
- `docs/reference/execution-truth-and-recovery.md`
- `docs/reference/per-run-workspace-isolation.md`
- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`

---

## 6. Tests and Proof Gate

Add canonical gate aliases:

- `proposal-064`
- `p064`

Required proof:

- Clean worktree fast-forward sync records a successful no-op or fast-forward result.
- Dirty worktree sync creates a preservation artifact and preservation commit before attempting merge.
- Successful merge records previous head, main head, new head, and preservation artifact id.
- Conflict path records conflicted files, aborts the failed merge, leaves the worktree clean at the preservation commit, and creates `main_sync_conflict_resolution`.
- The orchestrator never uses destructive reset/checkout cleanup in the sync path.
- Knowledge capsule is emitted from a completed run and is compact enough for prompt injection.
- Relevance matching attaches a capsule to an active run with overlapping files/tags and does not attach unrelated capsules.
- GraphQL readback exposes sync status and capsule provenance.
- MCP compact inspection stays within the response budget and remains explicitly non-UI.

---

## 7. Rollout

1. Implement persistence for sync attempts and knowledge capsules.
2. Add preservation artifact writer and preservation commit flow.
3. Add safe merge attempt and conflict-work-item routing.
4. Add capsule emission for terminal and manually harvested runs.
5. Add relevance matching and prompt injection.
6. Add GraphQL readback and compact MCP inspection.
7. Register `proposal-064|p064` gate.
8. Update reference docs once the gate passes.

---

## 8. Acceptance Criteria

- Active implementation runs can be synced with `main` without losing dirty work.
- A failed merge attempt leaves the worktree clean at a durable preservation commit and creates a focused conflict-resolution work item.
- Completed-run decisions, guardrails, unresolved risks, and follow-up proposal links become durable capsules.
- Related unfinished runs receive relevant capsules automatically in their next agent context.
- Operators can inspect sync status and knowledge provenance through GraphQL.
- The macOS UI does not call MCP for this feature.
- `./scripts/test-gate.sh proposal-064` passes.
