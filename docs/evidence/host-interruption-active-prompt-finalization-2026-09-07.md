# Host Interruption and Late ACP Finalization

Date: 2026-09-07. Base: `4e7f39d6c57eb4c145234b30ca5df78bbfbaa3f8`.
Pre-close ordering and queued-transaction correction: 2026-09-08.
Late successful provider-result correction: 2026-09-08.

## Scope

This fixes ownership of an already claimed InvokeAgent when host recovery and
the original ACP prompt's finalization race. It does not change P049 admission,
input budgets, provider allocation, live runs, or provider entitlements.

## Reproduction Before the Patch

The engine fixture uses a real work queue, claim creation, host recovery service,
and worker finalization against a temporary test database. Notify barriers hold
the fixture provider's active prompt until host cleanup/requeue has committed.
There are no sleeps controlling the race. Queue due timestamps are made eligible
explicitly to respect the indexed scheduler's existing one-second cutoff.

Observed RED results before production changes:

- A pending host retry became `failed` when the old active prompt returned.
- An already running replacement was reset to `pending` by the old prompt.
- The same pending-to-failed race occurred at the ordinary auto-requeue limit.
- The generation remained `active` after successful host cleanup/requeue.

Independent review identified two additional cases, also reproduced before their
fixes: stage-wide requeue lost the second execution's retry evidence and requeued
a sibling whose cleanup failed; a diagnostic read failure sent the old worker
through generic failure finalization and failed the already running replacement.
Three further RED regressions released the old prompt after cleanup failure:
ordinary active-close requeued the held work item, the attempt-limit path failed
it, and a diagnostic read failure enqueued failure advance.

Parent review on 2026-09-08 identified the reverse ordering: cleanup signalled
the old prompt before host recovery acquired its transaction. Four new barrier
fixtures allowed that prompt to finish before host settlement and reproduced
`affected_executions = 0` instead of 1, including the auto-requeue limit and
cleanup failure. A shared-writer DB regression also observed one queued operation
after a supposedly finished no-op; dropping `QueuedTransaction` had not completed
its commit/rollback protocol.

Crash-gap fixtures then reproduced an `active` old generation after startup
requeue, both without a command journal and on a journal-backed P082-R15 replay.
These RED results preceded their corresponding production corrections.

A further parent review identified successful results during the phase-one
cleanup interval. Two full-worker barrier fixtures returned a valid declared
`implementation_self_assessment_v2` output while cleanup was paused. Before the
correction, each produced one active old generation, a closed source claim,
completed agent and stage, and two work items instead of one. Separate RED
checks showed that the import transaction accepted a fenced result and that an
ordinary successful import left the execution running until later queue settlement.

## Implemented Behavior

- Host recovery requeues and supersedes the exact execution/source work item;
  legacy unclaimed rows require a single matching stage/agent candidate.
- Phase one commits the exact affected set and `cleanup_pending/pending` evidence
  before close is requested. It retains running execution capacity until cleanup
  settles. Phase two uses that frozen set rather than a new running-only query.
- Retirement of the closed generation and exact retry/claim evidence commit
  before the replacement is claimable.
- Active-prompt recovery consults durable host supersession, not merely a payload
  marker. Ordinary requeue and failure facts use one writer transaction.
- Attempt-limit/P088 non-requeue settlement occurs under that ownership lock.
- Worker complete/fail/transient-requeue operations compare the captured execution
  with the current running preclaim and reject cancelled or cleanup-fenced
  executions inside their transaction. Failed cleanup retains its hold, claim and
  generation; the old worker cannot bypass that hold even when host-evidence diagnostics fail.
  Default queue APIs still support intentional pending-item settlement.
- The generic execution-failure fallback reads ownership/status and writes under
  one writer transaction. Normal no-op paths explicitly commit before returning.
- A provider result checks durable host ownership before discovery,
  repair or artifact persistence. Import and mediation completion recheck durable
  host ownership inside their settlement transactions. An affected execution never
  regains result authority just because its cleanup evidence changes status.
- Accepted declared output, source-claim closure and terminal agent status commit
  atomically. This closes the gap in which host phase one could select a running
  execution after its output was already active. Host-fenced imports exit before
  artifact/fact writes or claim closure; non-host P082 ignored-late-output handling
  retains its existing cancellation/quarantine semantics.
- The tested late active-close result does not mutate a pending, running or
  completed replacement, overwrite host cancellation, duplicate failure advance, consume quota retry
  budget, or revive the closed generation. The fresh retry produces a validated
  required output through normal artifact settlement.

## Transaction Tradeoff and Interruption Recovery

Cleanup does not hold an open SQLite transaction. `request_close_session` signals
the prompt without waiting for its lock, but also releases Xcode leases, whose
observation sink writes through the DB writer. Holding the writer during that
call would risk reentrant waits, timeout-driven false cleanup failures, and
serial five-second cleanup waits per affected generation. The two short writer
transactions instead expose a durable, capacity-preserving `cleanup_pending`
interval, which every captured-attempt finalizer treats as non-owning.

The existing startup repair is the recovery path after interruption between
phases. A successful startup requeue, including P082-R15 journal replay, resolves
the pending fence to `retry_recovered_on_startup/unknown_after_restart` and
invalidates the old generation in that same transaction. It does not assert
that interrupted cleanup succeeded. Existing P082 retry limits, exhaustion holds
and operator recovery rules are unchanged; no new background retry loop was added.
Fixtures abort only the test recovery task, invoke real startup repair against
the fixture DB, then prove a distinct generation and a completed required output.

## Verification

Commands below run from `control-plane/` using the managed Cargo policy.
The final late-success checks use this dedicated managed gate cache:

```bash
export CARGO_TARGET_DIR="$HOME/Library/Caches/Chainworks Forge/cargo-target/gates/host-late-success-20260908"
```

Host/ordinary/P088 integration selections, full P061, the scoped DB rerun,
all-targets compilation and formatting were rechecked on 2026-09-08. The ACP
adjacent close fixture and P091 clean-base comparison were checked on 2026-09-07;
the additional import-test baseline comparison was checked on 2026-09-08.
ACP source was not changed by this correction.

| Command | Result |
|---|---|
| `../scripts/cargo-managed test -p engine --test integration active_prompt -- --nocapture` | 8 passed |
| `../scripts/cargo-managed test -p engine --test integration host_interruption_ -- --nocapture` | 18 passed |
| `../scripts/cargo-managed test -p engine --test integration p082_ -- --nocapture` | 12 passed, including cancellation quarantine and ordinary successful import |
| `../scripts/cargo-managed test -p engine --test proposal_061_backpressure host_interruption_ -- --nocapture` | 6 passed initially; also included in the final full P061 run |
| `../scripts/cargo-managed test -p engine --test integration proposal_088_code_writer_stale_implementation_active_enters_receipt_path_not_auto_requeue -- --exact --nocapture` | 1 passed |
| `../scripts/cargo-managed test -p db repos::work_items::tests:: -- --test-threads=1` | Initial 2026-09-07 result: 30 passed; 2 P091 queue-claim failures, reproduced on clean base |
| `../scripts/cargo-managed test -p engine --test proposal_061_backpressure -- --test-threads=1` | 28 passed on final code |
| `../scripts/cargo-managed test -p acp --test integration test_runtime_manager_closes_inflight_one_shot_session_by_generation_id -- --exact --nocapture` | 1 passed |
| `../scripts/cargo-managed check --workspace --all-targets` | Passed; warnings in unchanged code |

Final DB rerun passed 30 tests with exactly the two baseline failures excluded:

```bash
../scripts/cargo-managed test -p db --lib repos::work_items::tests:: -- --test-threads=1 \
  --skip p091_claim_next_quarantines_malformed_typed_advance_and_claims_next \
  --skip p091_claim_next_quarantines_source_work_item_only_retry_advance
```

Formatting and whitespace checks passed from the repository root:

```bash
rustfmt --edition 2021 --check \
  control-plane/crates/db/src/repos/scheduler.rs \
  control-plane/crates/db/src/repos/work_items.rs \
  control-plane/crates/engine/src/executor.rs \
  control-plane/crates/engine/src/host_interruption.rs \
  control-plane/crates/engine/src/work_queue.rs \
  control-plane/crates/engine/tests/integration.rs \
  control-plane/crates/engine/tests/proposal_061_backpressure.rs
git diff --check
```

The `active_prompt` and `host_interruption_` selections overlap. Together they
cover eighteen host-race/recovery cases and two ordinary-close cases, not
twenty-six distinct tests. The full P061 suite includes the six separately
selected host tests.

The successful-result barrier fixtures cover both successful and failed cleanup.
They assert that the old result creates no artifact rows or active generation,
does not close its source claim, and leaves agent/stage/work-item truth unchanged
before phase two. A fresh retry alone activates its valid output. The separate
import-boundary fixture bypasses the early result check to exercise the transaction
fence directly. The ordinary-success control checks terminal execution immediately
after import and verifies that subsequent host recovery selects no affected attempt.

Review also caught an overbroad early current-attempt filter: a non-host
cancellation committed between the initial running-status read and the new result
gate skipped P082 quarantine. A WAL/shared-writer barrier reproduced settlement
`None` instead of `IgnoredLateOutputs` before the filter was narrowed to durable
host evidence only. The fixture injects the same writer into the executor and waits
for both the held transaction and queued result gate before committing cancellation.
Non-host cancellation/supersession continues through claim CAS.

The stale-attempt DB test also verifies no-op complete/fail/transient-requeue for
replaced ownership and retains current-owner failure and general `fail(pending)`.
It checks shared-writer queue depth is zero immediately after each no-op returns.
The diagnostic failure injection only renames a table in the in-memory fixture
database, restoring it before the replacement finishes.

Independent read-only review rechecked the initial ownership cases and the
subsequent pre-close, fallback, no-op, P082-R15 and late-success corrections,
including the narrowed host-only early gate after the non-host quarantine finding.
The final review found no remaining concrete blockers in the scoped diff; this
is separate from test execution and does not establish live acceptance.

## Baseline and Rerun Notes

In a clean detached worktree at the base above,
`../scripts/cargo-managed test -p db --lib p091_claim_next_quarantines_ -- --test-threads=1`
reproduced both queue-claim tests failing at `Option::unwrap()` on `None`.
These unrelated P091 tests were not changed. The owned temporary worktree was
verified clean and removed after testing.

An initial 2026-09-07 P061 run had 27 passes and one `policy_bytes_mismatch` failure claiming
a 1,479-byte limit. The exact StartRun test passed on clean base, and two rebuilt
current-tree full P061 runs passed 28/28, including the final cancellation patch.
The initial mismatch's cause was not established; it is not classified as a
confirmed pre-existing source defect. No model-policy files were changed here.

Two additional unchanged import tests selected by
`../scripts/cargo-managed test -p engine --test integration imports_ -- --nocapture`
failed before provider execution at `assert!(executor.process_next_item().await.unwrap())`:
`proposal_057_invoke_agent_imports_declared_contract_output_into_active_index`
and `test_invoke_agent_imports_implementation_self_assessment_summary`.
Both failures reproduced on a clean detached base checkout on 2026-09-08.
That owned checkout was verified clean and removed; these tests were not modified.

After that comparison, an attempted current-tree P082 rerun reused the baseline
binary from the shared target (193 total tests rather than the then-current 212).
The final suite also includes the non-host cancellation regression, totaling 213.
That rerun is discarded as proof of the correction. Final late-success checks use
the dedicated managed gate cache above; no shared cache was deleted or live binary replaced.

## Changed Files

- `control-plane/crates/db/src/repos/work_items.rs`: exact host requeue and
  transaction-scoped captured-attempt finalizers, plus interrupted-fence retirement
  on successful existing startup requeue/replay and durable result-ownership lookup.
- `control-plane/crates/db/src/repos/scheduler.rs`: compare-and-set settlement of
  phase-one cleanup evidence.
- `control-plane/crates/engine/src/host_interruption.rs`: exact execution recovery
  with a durable pre-close fence and atomic closed-generation retirement.
- `control-plane/crates/engine/src/executor.rs`: atomic active-close settlement
  and captured ownership in both worker entry paths; host-only result fences and
  atomic accepted-output/terminal-execution settlement.
- `control-plane/crates/engine/src/work_queue.rs`: attempt-aware queue wrappers.
- `control-plane/crates/engine/tests/integration.rs`: deterministic race fixtures,
  cancellation/diagnostic failure cases, late-success and non-host quarantine
  ordering, and ordinary-close/import controls.
- `control-plane/crates/engine/tests/proposal_061_backpressure.rs`: same-stage
  fan-out recovery and failed-sibling cleanup tests.
- `docs/reference/execution-truth-and-recovery.md`: implemented ownership rules.
- This evidence document.

## Limits

No deployment, install, running app/daemon restart, live process kill, live MCP
retry, direct live DB mutation, commit, or push was performed. Tests are
provider-free fixtures, not live acceptance. The already blocked incident run
is not repaired by changing source code. Swift/UI suites and the full Rust test
workspace were not run; the all-targets check is compilation, not test execution.
