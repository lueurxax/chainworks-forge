# Frozen Effective Worktree Strategy on Retry

Date: 2026-09-19
Base: `4983ddfbbf15e946d4a5b758a0da90c2aedf5568`
Scope: a separate engine fix, not an extension of the frozen Green main
test-only run or P082/P070/P095 work.

## Failure and Cause

Ops reported Green main run `108e6cdc-533b-45d6-84ed-459ea7d992e4`
at `state_9_implementation_reviewed`, attempt 2, with a failed security
preflight (`xcode_target_not_found`), completed docs, and failed advance items:

```text
frozen_snapshot_contract_incompatible: V1 retry payload field 'worktree_strategy' differs from frozen authority
```

This implementation task did not inspect or modify the live DB. The incident
state and Xcode diagnosis above are Ops-provided observations, not a new live
acceptance claim.

Source tracing and provider-free execution confirmed this chain:

1. Static-task enqueue derives `shared_implementation_worktree` for the
   existing read-only implementation reviewer roles when no strategy is set.
2. The copied-payload validator compared this value with the raw resolved
   agent strategy, which is `None` for `security_checker`.
3. After a failed invocation, the worker processes `advance_run`; failed-stage
   settlement first tries P058 escalation retry, then contract-output retry.
   Both validate the persisted V1 source before mutation. The mismatch escapes
   as an error before stage settlement/retry creation, so an exhausted advance
   queue can coexist with unchanged `running` run/stage state.
4. The operator-targeted retry handler and deadline-resume path share the V1
   validator. There is no separate continuation or stale-running repair in
   this patch.

`docs_guardian` is distinct: the current catalog explicitly declares
`worktree_policy.strategy: shared_implementation_worktree` with write enabled.
A missing top-level catalog `worktree_strategy` is not evidence that its
resolved strategy is absent. The regression covers this explicit docs binding
as well as the derived read-only security binding.

## Fix and Boundaries

The existing pure strategy/read-root helpers moved from `orchestrator.rs` to
`worktree.rs`. Enqueue, prompt composition, and V1 static-task validation now
share their unchanged semantics. Validation resolves the exact frozen state
and task, including post-approval tasks, after checking assignment identity,
phase, parallel shape, outputs, and consumers. Explicit strategy takes
precedence. No arbitrary string, null substitution, or cross-assignment
derivation is admitted. Dynamic, owner, and mediation strategy behavior is
unchanged. No schema, catalog, workflow, or snapshot mutation was added.

## Regression Proof

All commands run from `control-plane` with this dedicated managed target:

```bash
export CARGO_TARGET_DIR="$HOME/Library/Caches/Chainworks Forge/cargo-target/gates/worktree-strategy-20260919"
../scripts/cargo-managed test -p engine --lib worktree_strategy_v1_ -- --nocapture --test-threads=1
```

Before the production fix, the new enqueued-payload roundtrip and real
`BackgroundExecutor::process_next_item` advance/P058-retry test both failed
with the exact incident error; the owner-isolation negative control passed
(1 passed, 2 failed, exit 101). No fixture changes were needed to obtain GREEN
(3 passed, exit 0).

The roundtrip table covers all three derived reviewer roles, explicit docs,
explicit `meta_only`/`dedicated` precedence, post-approval tasks, and a
non-reviewer static task. Each case rejects non-authoritative null/shared/
meta-only/dedicated/arbitrary substitutions. The owner control proves the same
reviewer agent cannot borrow static-task derivation for an owner assignment.
The fourth regression added after independent review verifies that an
allowlisted reviewer with write enabled and no explicit strategy keeps `null`
and rejects the read-only shared-worktree fallback.
The final isolated rerun of `worktree_strategy_v1_` passed all 4 tests
(exit 0, 568 filtered), after all source and test edits.

The worker regression uses an in-memory DB and temporary artifact roots,
production enqueue, a failed pre-provider security execution, completed docs
work, and a durable escalation tier. Only a due advance item is processed;
the ACP runtime has no provider adapters. GREEN requires completed advance,
skipped source stage, a running retry target, and exactly one pending invoke
carrying the derived strategy and the selected frozen fallback profile.

## Adjacent Verification

Using the same final-code managed cache above:

| Command | Result |
| --- | --- |
| `../scripts/cargo-managed test -p engine --test agent_context_skills -- --test-threads=1` | 26 passed, 1 intentionally ignored fixture-regeneration test |
| `../scripts/cargo-managed test -p engine --lib -- worktree_strategy p058_ proposal_058_ implementation_review_ implementation_summary_orchestrator auto_contract_output_retry --test-threads=1` | 36 passed, 1 baseline failure; all four new regressions passed |
| `../scripts/cargo-managed check -p engine --all-targets` | Passed |

The adjacent failure is
`executor::tests::targeted_p058_retry_refreshes_stale_tier_from_the_durable_ledger`.
It fails at the unchanged `executor.rs:24051` with `parse run idea_id` /
`invalid character: found i at 0`; the fixture inserts `idea-p058` where a UUID
is required. The exact test reproduced the same failure against an unmodified
`git archive` of the base commit in a temporary directory, with a separate
managed target `gates/worktree-strategy-baseline-20260919`:

```bash
../scripts/cargo-managed test -p engine --lib \
  executor::tests::targeted_p058_retry_refreshes_stale_tier_from_the_durable_ledger \
  -- --exact --nocapture
```

That baseline run had 1 failure and 567 filtered tests; the final working-tree
selection had 37 selected and 535 filtered. Baseline artifacts never shared
the final-code target directory. The unrelated fixture was not modified.

Independent read-only review found no production blockers. Its one
nonblocking coverage suggestion produced the write-enabled negative
regression above. The reviewer did not execute tests or access live state.

Final formatting and whitespace checks passed from the repository root:

```bash
rustfmt --edition 2021 --config skip_children=true --check \
  control-plane/crates/engine/src/orchestrator.rs \
  control-plane/crates/engine/src/agent_mission_context.rs \
  control-plane/crates/engine/src/worktree.rs \
  control-plane/crates/engine/src/orchestrator/tests/worktree_strategy.rs
git diff --check
```

## Changed Files

- `control-plane/crates/engine/src/worktree.rs`: shared unchanged strategy helpers.
- `control-plane/crates/engine/src/orchestrator.rs`: helper imports and test module registration.
- `control-plane/crates/engine/src/agent_mission_context.rs`: exact frozen static-task strategy comparison.
- `control-plane/crates/engine/src/orchestrator/tests/worktree_strategy.rs`: four provider-free regressions.
- `docs/reference/skill-resolution-and-runtime-integration.md`: canonical copy-validation rule.
- This evidence record.

## Limits

This is provider-free regression evidence, not live recovery or acceptance.
No daemon deployment/restart, app rebuild/install, Xcode interaction, live
retry, live DB write, commit, push, or publication was performed. Existing
run worktrees and the unrelated `scripts/__pycache__/` were left untouched.
The reported Xcode target failure is a separate operational prerequisite.
The fix does not reschedule already failed advance items automatically.
The full Rust workspace test suite and Swift/UI gates were not run. A green
engine compilation or focused test selection is not a green full suite.
