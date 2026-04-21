# P053 Worktree Loss Retro

Date: 2026-04-21
Status: Open
Incident: P053 run cancellation removed the run-owned implementation worktree before the implementation diff had been made durable.

## Summary

During P053 follow-up work, the assistant moved from discussion into execution too early. It cancelled the P053 run and archived the idea before first preserving the dirty implementation worktree as a durable git object, patch bundle, or snapshot. The run-owned worktree was later gone; the branch still existed, but pointed at the base commit and did not contain the expensive implementation work.

Manual reconstruction of the lost P053 implementation is explicitly out of scope. The correct recovery direction is process and orchestrator hardening, not hand-recreating work that the orchestration system should preserve.

This incident must not make manual closeout of unfinished implementation runs a normal workflow. P053 was an exception path caused by orchestration gaps. The target state is to make the orchestrator robust enough that valuable dirty implementation runs are completed, reviewed, preserved, or explicitly rejected through workflow states rather than manually cancelled and cleaned up.

## Impact

- The dirty P053 implementation worktree was lost as an editable source tree.
- The approved P053 proposal and run artifacts survived.
- The original implementation diff was not available as a patch, commit, or durable snapshot.
- Significant agent time and token spend were wasted.
- Operator trust in destructive lifecycle actions was damaged.

## What Happened

1. The operator asked to proceed "analogously to P054" in a high-context conversation.
2. The assistant interpreted that as authorization to execute lifecycle actions, not only to propose a plan.
3. The assistant cancelled the P053 run before preserving the dirty worktree.
4. The cancellation path cleaned up the run-owned worktree.
5. The assistant later discovered that the branch still existed but only pointed at the base commit.

## Root Causes

1. The assistant treated ambiguous high-level operator language as permission for destructive lifecycle actions.
2. The assistant assumed that not manually deleting a worktree was equivalent to preserving it.
3. The process did not require a durable worktree snapshot before `cancel`, `archive`, cleanup, retry supersession, or any action that may trigger run cleanup.
4. The orchestration system allowed cancellation of a run with dirty worktree state without first preserving or warning about uncommitted implementation work.

## Contributing Factors

1. Operator commands were high-context and shorthand-heavy. That is normal for live orchestration, but it makes implicit authorization risky.
2. The phrase "like P054" was underspecified because P054 and P053 had materially different risk: P053 had a large dirty worktree with conflicts and no durable patch.
3. The assistant did not stop to ask whether "like P054" meant "prepare the same decision" or "execute the same lifecycle cleanup now."
4. The assistant over-prioritized momentum and cleanup over preserving evidence.
5. The run-owned worktree was treated as durable even though it was lifecycle-managed and therefore ephemeral.

## Blame Handling

This is not a "user is stupid" incident. The useful finding is not that the operator gave unclear commands; the useful finding is that operator commands can be terse, ambiguous, and context-dependent, and the assistant must design for that reality.

The assistant owns the safety failure because it acted on ambiguity where the downside was irreversible loss of work. The correct behavior is to treat ambiguity as a stop condition for destructive actions.

## Permanent Rules

1. Do not treat manual closeout of unfinished implementation runs as a routine path.
2. First diagnose and fix orchestration, retry, transition, approval, or recovery gaps so the run can proceed through the intended workflow.
3. Before any emergency run `cancel`, idea `archive`, worktree cleanup, retry supersession, or lifecycle action that may cleanup a dirty worktree, the assistant must first prove that implementation work is durable outside the run lifecycle.
4. Acceptable durability proofs are: a git commit, a named branch containing the work, a `git diff --binary` patch bundle stored outside the run-owned tree, or a tar snapshot stored outside the run-owned tree.
5. `git status --short`, untracked file inventory, and `git diff --binary` must be checked before lifecycle actions touching implementation runs.
6. A run-owned worktree is ephemeral. It is not preserved merely because the assistant did not delete it directly.
7. High-level instructions such as "like the previous one", "clean it up", "kill the run", or "do the same" do not authorize destructive actions when dirty work may exist.
8. The assistant must restate the orchestration-first path, the emergency destructive action checklist, and the preservation plan, then wait for explicit confirmation when work loss is possible.
9. Do not manually reconstruct lost implementation work unless the operator explicitly asks. The system must be fixed to preserve or regenerate work through orchestration.

## Required System Fixes

1. Fix loop-budget, retry, recovery, and transition behavior so a run that has produced complete implementation output can continue to review/release states without manual closeout.
2. Add a cancellation preflight that detects dirty run-owned worktrees.
3. Block or require explicit operator override for cancellation when dirty implementation work exists.
4. Automatically export a patch bundle and untracked-file archive before cancellation cleanup.
5. Store the preservation artifact path in the run cancellation settlement log.
6. Surface dirty-worktree preservation status in GraphQL and MCP run readback.
7. Add a proposal/process item for worktree lifecycle safety as an emergency guardrail, not as the target operating model.

## Current P053 Handling Rule

P053 implementation must not be manually recreated from memory. Further P053 implementation work should proceed only through a deliberate orchestrated process that preserves generated work before lifecycle cleanup.
