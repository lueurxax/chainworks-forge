# Headless Offline Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement independently verifiable migration prerequisites while the operator is unavailable, without admitting an unverified production Xcode capability.

**Architecture:** A shared root-selection function removes inconsistent worktree choices; a typed, filesystem-pinned root is available to later preparation. A separate SQLite effect journal implements durable dispatch fencing and recovery without performing any Apple operation. These are dependencies of the existing runtime contract, not a replacement for binding, coordinator, facade or release integration.

**Tech Stack:** Rust, existing domain/ACP/engine crates, SQLite/sqlx, managed Cargo, synthetic tests only.

**Spec:** `docs/superpowers/specs/2026-09-19-xcode-headless-runtime-contract.md` and its parent/wire/release contracts.

## Global Constraints

- Latest user instruction authorizes continuing implementation without interaction; live acceptance is deferred until the operator returns.
- No IDE, Apple MCP, service restart, permission administration, provider run, commit, merge or deployment.
- No inferred filesystem-confinement or workspace-ID guarantee. Missing production admission proof remains blocking.
- Keep historical I1 source identities and generated reports immutable. New source is a new revision.
- Use managed Cargo with `--locked --offline`; preserve unrelated work and use the existing isolated worktree.
- Do not claim full replacement from these prerequisites. The old production path is not switched by this plan.

## Review Focus

1. Read-only shared-worktree tasks must use the worktree in all root consumers.
2. Explicit worktree strategies with missing roots must fail, not use repository fallback.
3. Historical/default requests retain their documented repository fallback.
4. A different operation key cannot bypass an unresolved effect on the same project.
5. Historical outcomes retain schema identity independently of future tool admission.

### Task 1: Shared Execution Root Authority

**Files:** Add `domain/src/execution_root.rs`, `acp/src/execution_root.rs`; update exports and existing ACP/engine root consumers. Paths are under `control-plane/crates/`.

**Interfaces:**
- `domain::execution_root::select_execution_root<'a>(repository: &'a str, worktree: Option<&'a str>, write_enabled: bool, strategy: Option<&str>) -> Result<&'a str, ExecutionRootError>` is pure selection.
- `acp::execution_root::resolve_execution_root(...) -> anyhow::Result<ResolvedExecutionRoot>` canonicalizes and pins the selected directory; `revalidate_execution_root(&ResolvedExecutionRoot)` checks replacement.
- Domain root data has version, repository/effective canonical paths, decimal-string filesystem identity, frozen strategy and root kind; it is not a workspace binding or permission grant.

- [x] Add failing selection tests, including:

```rust
assert_eq!(select_execution_root("/repo", Some("/tree"), false,
    Some("shared_implementation_worktree")).unwrap(), "/tree");
assert!(select_execution_root("/repo", None, false, Some("dedicated")).is_err());
assert_eq!(select_execution_root("/repo", None, true, None).unwrap(), "/repo");
```

- [x] Run `../scripts/cargo-managed test --locked --offline -p domain --test execution_root` and observe the missing module/function failure.
- [x] Implement the single selector and typed root resolution; reject empty selected paths, missing required directories and changed filesystem identity.
- [x] Replace duplicated root decisions in engine policy, ACP process/session setup and Xcode execution context. Preserve non-Xcode legacy behavior and explicit provider preflight overrides.
- [x] Add ACP real temporary-directory tests for canonical aliases, missing roots and path replacement; test actual session/new cwd and broker context, not only the pure selector.
- [x] Run focused domain/ACP/engine tests and read every failure before adapting expectations. Record changed semantics explicitly; no commit.

Task 1 verification: final ACP/example suite has 356 passes and one existing ignored test;
two new domain tests and 11 engine MCP tests pass. Full engine has 568 passes
and four failures reproduced identically on clean HEAD. Filesystem pinning is
a tested preparation primitive, not yet a manager ticket or binding digest.

### Task 2: Durable Effect Journal

**Files:** Add `domain/src/xcode_effect.rs`, `db/src/repos/xcode_effect_attempts.rs`, one next-number migration and `db/tests/xcode_effect_attempts.rs`; update only required module/migration registration.

**Interfaces:** Domain owns `AttemptState`, normalized intent, stored attempt and historical result envelope. DB owns `prepare`, `dispatch`, `complete`, `recover_dispatched`, `reconcile`, `get` and `list`. All state transitions use expected revisions; no function sends host bytes.

- [x] Write failing temporary SQLite tests for prepare dedupe, changed digest rejection, one successful dispatch CAS, new-key denial while held and restart conversion to unknown.
- [x] Run `../scripts/cargo-managed test --locked --offline -p db --test xcode_effect_attempts` and record RED.
- [x] Add constrained tables, uniqueness by owner/project/operation key, persisted UUID nonce, revision CAS, project hold in the same transaction as dispatch and retained schema-versioned outcome. Only lifecycle open may omit binding digest.
- [x] Complete only proven terminal outcomes; uncertain/partial results preserve holds. Reconciliation requires an explicit evidence-bearing input with expected revision and settled-operation proof. The DB layer is not an authorization endpoint.
- [x] Test cancellation before dispatch, outcome persistence failure, concurrent dispatch, on-disk DB reopen, held-project new nonce, malformed inputs and historical result readback after an unrelated manifest change. No automatic purge or retry API. Different-database coordinator exclusion remains in the runtime integration scope, not a property a standalone DB repository can enforce.
- [x] Run the focused DB tests and affected domain tests. No daemon wiring or live database migration; no commit.

Task 2 verification: 26 journal tests pass in implementation and parent rerun;
341 domain tests pass. Full DB library regression has 441 passes and two failures
reproduced on clean HEAD. Production Class A operation registration, authorization,
coordinator startup/recovery and actual proof validation remain separate work.

### Task 3: Verify And Record Integration Boundaries

- [x] Run the serial ACP suite with live smoke unset, focused engine tests, domain/DB tests, managed check, formatting and diff checks.
- [x] Independently review the complete new patch for root regressions, journal CAS/hold mistakes and accidental production exposure; fix actionable findings with focused regression tests.
- [x] Record exactly which prerequisites are wired versus standalone, tests actually run, and remaining broker/coordinator/prepare-before-reuse/facade/release work. Preserve the I1 live success separately.

Task 3 disposition: no actionable blocking component finding. The suggested
end-to-end root/reuse/resurrection fixture was added and passed. Final ACP has
356 passes/one manual test ignored; domain has 341 passes; journal has 26 passes.
Engine has 568 passes/four baseline failures, DB library 441 passes/two baseline
failures. Final workspace check and formatting pass. See the
[checkpoint](../../evidence/xcode-headless-offline-2026-09-19.md) for failed
diagnostic runs, source/log identities and remaining production integration.
Completion applies only to this bounded offline plan, not the user's full
headless replacement objective or I2 admission.

## Remaining Migration Work

These tasks do not implement the production host controller, coordinated broker/shim permits, prepare/commit tickets, typed admitted facade, privileged readback, legacy-daemon cutover or release receipt. In particular, no available evidence establishes the strict upstream ID/confinement guarantee required by the existing contract. Production admission cannot be enabled by this plan, and tomorrow's live check alone is not guaranteed to resolve that architectural requirement.
