# Headless Journal Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect the existing effect journal to the admitted engine writer and startup recovery without enabling Apple dispatch.

**Architecture:** ACP defines an IO-neutral async journal boundary using the existing domain records; engine implements it with the shared DbWriter and a fixed run/owner scope. Startup recovery is a separate pre-admission operation, never part of the live StartupRepair work item.

**Tech Stack:** Existing Rust/domain/ACP/engine/db crates, SQLite, managed Cargo. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-19-xcode-headless-runtime-contract.md`, Durable Effect Attempts and One Workspace Access Owner.

## Global Constraints

- The user explicitly requested continuation to full migration. This plan continues an unchanged dependency of the written runtime contract; it does not decide the pending trust-model question.
- No production dispatch, provider launch, daemon restart, database migration on the installed service, permission change, commit, merge or push.
- Existing uncertain effects remain held. Never replay or issue a replacement operation key automatically.
- Every mutation consumes a registered Class A writer-admitted transaction. No pool fallback, nested transaction or transaction held over an Apple call.
- Existing journal outcomes are bounded historical summaries and proof assertions, not raw tool results or independently verified completion.
- Use the existing isolated worktree and managed Cargo, with an isolated shared gate target. Preserve historical evidence files.

## Review Focus

1. The same lineage string in a different run must not expose or dispatch another run's attempt.
2. A repeated nonce must return Existing rather than authorize another send.
3. Writer failure or shutdown must stop new preparation/dispatch; settlement may use only explicitly admitted drain operations.
4. Startup recovery must finish before provider/request admission, and must not be invoked during live StartupRepair.
5. More than 100 interrupted attempts must recover in bounded batches without dropping project holds or looping on a failed batch.

### Task 1: Scoped Engine Journal Adapter

**Files:** Create `control-plane/crates/acp/src/xcode_effect_journal.rs`, `control-plane/crates/engine/src/xcode_effect_journal.rs`, `control-plane/crates/engine/tests/xcode_effect_journal.rs`; update module exports, `db/write-operation-registry.toml` and the writer shutdown allowlist.

**Interfaces:** ACP owns `XcodeEffectJournal`, `XcodeJournalError` and `XcodeDispatchDecision::{Dispatch,Existing}`. Trait methods are `prepare(&NormalizedIntent)`, `dispatch(Uuid, &str, i64)`, `complete(AttemptRevision, &Completion)`, `cancel(AttemptRevision, &HistoricalResult)` and `get(Uuid)`, returning existing domain records or an ACP error. Engine owns `DbXcodeEffectJournal::new(pool, writer, run_id, owner_lineage)`. The fixed scope is checked before every read or mutation.

- [x] Write temporary-DB tests for a successful prepare/dispatch/unknown cycle, same-nonce dedupe, scope mismatch (including same owner/different run), missing/corrupt record, failed writer admission and settlement during drain.
- [x] Run `../scripts/cargo-managed test --locked --offline -p engine --test xcode_effect_journal -- --test-threads=1`; expect missing new API initially, then behavior failures before implementation.
- [x] Implement the trait and adapter by calling `class_a_operation(name, CriticalBarrier, key)` and `DbWriter::begin_immediate_transaction`, then the existing repository function. The adapter must preserve Dispatch versus Existing and typed conflict/hold/revision/storage errors.
- [x] Register `xcode_effect.prepare`, `.dispatch`, `.complete`, `.cancel` as Class A / caller_guarded with stable identity/revision keys and real duplicate-application tests. Add only complete/cancel to shutdown-admitted operations.
- [x] Run focused adapter, journal repository and writer/registry tests; do not enable a provider route or reconciliation endpoint.

### Task 2: Bounded Startup Recovery

**Files:** Update `db/src/repos/xcode_effect_attempts.rs`, its integration tests, `engine/src/recovery.rs`, `db/write-operation-registry.toml`, and `daemon/src/main.rs` startup-recovery boundary; add focused recovery tests.

**Interfaces:** DB adds `list_dispatched_revisions(&SqlitePool, u32) -> Result<Vec<AttemptRevision>>`, ordered by creation sequence with a 1..100 limit. Engine adds `RecoveryService::recover_xcode_effects_before_admission() -> anyhow::Result<usize>`; it calls the existing `recover_dispatched(tx, revisions, now)` until no dispatched rows remain. Register `xcode_effect.recover_dispatched` as a Class A barrier.

- [x] Add failing tests for zero attempts, 101 attempts across distinct synthetic projects, retained holds, invalid limits, repeated recovery and a failed batch.
- [x] Observe RED using the focused DB and engine tests.
- [x] Implement the bounded query and recovery loop. Stop on the first error; count only committed transitions. Do not add a transport retry or deadline extension. Hash the ordered batch identity to respect the existing writer's 1,024-byte bookkeeping-key bound.
- [x] Await recovery inside daemon startup before executor start and full request serving. Do not put the pass inside `run_startup_repair()` because live work items also invoke it.
- [x] Run focused recovery, DB and daemon tests; inspect startup failure propagation and confirm no production service is launched. Six recovery tests pass after review additions; the broader run includes all daemon tests. Combined daemon failure/admission and adapter-during-drain timing remain explicit coverage gaps.

### Task 3: Verification And Checkpoint

- [x] Run affected complete crate tests, managed workspace check, formatting and diff checks. Record baseline failures by name rather than skipping them. Broad run: 2,623 passed, 65 baseline failures, three existing ignored tests; two later review-added tests pass separately.
- [x] Obtain one independent component review, fix actionable findings with RED/GREEN tests, and record remaining integration limits. No code defect found; two suggested recovery tests added and passed.
- [x] Add a new dated checkpoint and source/log identities without overwriting September 19 evidence. Do not call the full migration complete from this journal integration.
