# P039 I1 Carry-Forward Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans and test-driven-development. Execute the authorized implementation without another planning approval pause.

**Goal:** Build and verify the provider-free carry-forward core before exposing any production mutation.

**Architecture:** Domain types own immutable evidence and fresh approval identity. A workflow compiler entry validates the closed restart profile and freezes successor read roots. The engine inventories and materializes pinned source bytes into private, independent roots, with no daemon/API admission yet.

**Tech Stack:** Existing Rust domain/workflow/engine crates, serde, SHA-256, SQLite test fixtures and local Git repositories. No additional service or provider.

**Spec:** `docs/proposals/039-blocked-run-fork-and-canonical-carry-forward.md` and its `039/` children, revision `p039-r2`.

## Global Constraints

- Isolated worktree; preserve all unrelated dirty work in the primary checkout.
- No production DB, P095 files, daemon restart, Apple call or provider dispatch.
- No MCP admission until I2/I3 fences, durable ownership and acceptance tests exist.
- Unknown effects hold; no source edits/reset, silent data loss or effect retry.
- Preview is read-only. Writes stay in newly created, operation-owned roots.
- Use `scripts/cargo-managed`; source snapshots are immutable historical evidence.
- This plan covers I1 only. I2/I3 require their own implementation plan based on measured I1 results; I4 requires explicit live scope.

## Review Focus

1. A review-to-code edge bypasses the fresh approval gate: compiler rejects it.
2. Repository main differs from the source checkout: every reviewer reads successor roots.
3. Source changes after inventory, including only its index: materialization holds.
4. A symlink/special file or filter escapes the supported local file model: reject without following or executing it.
5. Old approvals/results appear among imported references: they cannot authorize dispatch.

## Tasks

### Task 1: Closed Compiler Entry

Files: `workflow/src/carry_forward.rs`, `workflow/src/lib.rs`, `workflow/tests/proposal_039_profile.rs`.

Interfaces: produce `compile_for_continuation_v1(workflow_path: &str, catalog_path: &str) -> Result<ContinuationTargetV1>` and `validate_continuation_target_v1(plan: RunPlan) -> Result<ContinuationTargetV1>`. The target owns the current compiled plan and strict profile; ordinary compiler paths remain unchanged.

- [x] Add tests using current bundled workflow/catalog snapshots and a temporary workflow with explicit profile metadata.
- [x] Observe missing entry/profile behavior fail before implementation.
- [x] Validate the declared review, manual approval and preparation states, reject pre-gate writers/bypasses, require explicit headless policy for Xcode tasks, and derive shared successor read roots.
- [x] Verify omitted/unknown profile, rejected-path loop, dynamic reviewers and catalog drift.

```rust
let target = validate_continuation_target_v1(plan)?;
assert_eq!(target.plan.initial_state, target.profile.review_state);
assert!(target.plan.states[&target.profile.approval_state].is_manual_gate);
```

Run: `../scripts/cargo-managed test -p workflow --test proposal_039_profile` from `control-plane`.
Expected: all positive and negative profile cases pass; no filesystem writes by validation.

### Task 2: Immutable Evidence Types

Files: `domain/src/run_carry_forward.rs`, `domain/src/lib.rs`, `domain/tests/proposal_039_contracts.rs`.

Interfaces: typed entry roles, workspace witness, plan/manifest digests and an evidence-bound approval entry. Canonical hashes bind exact input bytes, modes, HEAD/index and target hashes; references never become execution outputs.

- [x] Add round-trip and digest-change tests; run RED.
- [x] Implement strict schema/version types and canonical digest helpers.
- [x] Add approval binding tests for changed tuple, stage identity, source/successor distinction and single consumption.
- [x] Run domain regression tests.

```rust
assert_ne!(before.digest()?, after.digest()?);
assert!(!binding.authorizes(&changed_tuple, stage_execution_id));
```

Run: `../scripts/cargo-managed test -p domain`.
Expected: strict contracts and existing domain tests pass.

### Task 3: Read-Only Workspace Planner

Files: `engine/src/run_carry_forward/{mod,inventory,git}.rs`, `engine/src/lib.rs`, `engine/tests/proposal_039_inventory.rs`.

Interfaces: `WorkspacePlanner::preview(source, limits)` returns an immutable workspace inventory and witness; it makes no source or destination changes.

- [x] Create disposable Git fixtures with staged/unstaged binary/text changes, deletion, mode change and untracked input; run RED.
- [x] Inventory all tracked and selected nonignored files, hash bounded streams, record HEAD/index/patches and reject unsupported special/link/filter/submodule shapes.
- [x] Prove source files and index are byte-identical after repeated preview; deterministic digests change on every meaningful change.
- [x] Prove limits fail without truncation and external links are never dereferenced.

```rust
let preview = WorkspacePlanner::preview(source, limits)?;
assert_eq!(index_before, std::fs::read(index_path)?);
assert_eq!(preview.witness, WorkspacePlanner::preview(source, limits)?.witness);
```

Run: `../scripts/cargo-managed test -p engine --test proposal_039_inventory`.
Expected: local-only inventory tests pass.

### Task 4: Independent Materialization Experiment

Files: `engine/src/run_carry_forward/materialize.rs`, `engine/tests/proposal_039_materialize.rs`.

Interfaces: materializer consumes a verified inventory plus explicit owned destination, preserves independent bytes and produces a typed verification receipt. It cannot activate a Run, grant approval or retry a partial destination.

- [x] Add two-repository source/target tests and source-change/occupied-destination negatives; run RED.
- [x] Preserve files, patches and index; pin the source commit; create a distinct checkout at that commit with final source overlay and clean index.
- [x] Verify copied contents/modes and source witness again before publishing a receipt.
- [x] Verify source independence, no overwrite/retry, preserved deleted files, and no inherited runtime metadata.

```rust
let receipt = materialize(&preview, &destination)?;
assert_ne!(receipt.checkout_root, source);
assert_eq!(source_before, WorkspacePlanner::preview(source, limits)?.witness);
```

Run: `../scripts/cargo-managed test -p engine --test proposal_039_materialize`.
Expected: preservation and independence proven; changed/unknown inputs fail closed.

### Task 5: I1 Proof And Review

Files: `engine/tests/proposal_039_experiment.rs`, `scripts/test-gate.sh`, `docs/reference/test-gates.md`, `docs/evidence/p039-i1-carry-forward.md`.

- [x] Add isolated SQLite/run fixtures linking old source metadata to the compiled target and verified manifest, without production activation.
- [x] Verify current catalog differs from old policy, old decisions stay historical, and a fresh exact-tuple approval is needed in the core contract.
- [x] Register a clearly named `proposal-039-i1` gate, not a misleading complete `proposal-039` gate.
- [x] Run focused gates plus workspace regressions and fresh-context code review; fix substantive findings with regression tests. The broad suite is not green; outcomes are recorded in evidence.
- [x] Record measured supported/partial/refuted/blocked hypothesis result and exact remaining I2/I3 gaps. Do not call a fixture-only path a migrated run.

Run: `./scripts/test-gate.sh proposal-039-i1` and `scripts/cargo-managed test --manifest-path control-plane/Cargo.toml`.
Expected: I1 acceptance evidence, or named unrelated/environment failures without laundering them as green.

## Execution Record

- Baseline HEAD: `8c40f71a03cfebff413fbad584131e0bcbe9859b`.
- Worktree: `/Users/user/.codex/worktrees/p039-carry-forward/Chainworks Forge`.
- Baseline domain: 300 passed, 0 failed.
- Ruling: user's explicit implementation instruction authorizes execution after this plan; no second planning confirmation is needed.
- Ruling: no shared-branch merge/push or deployment is included in this implementation pass.
- Final I1 gate and alias: 37 passed each. Scoped formatting, shell syntax and diff checks passed.
- Domain/workflow regression: 559 passed, 1 failure reproduced on unchanged HEAD.
- Broad diagnostic workspace run: exit 101, 9 failing targets, not a final-tree sign-off; see evidence for baseline confirmations and unclassified failures.
- Final result: I1 core delivered under documented local/quiescent-writer assumptions; full H1 Partial. I2/I3/I4 remain unimplemented.
