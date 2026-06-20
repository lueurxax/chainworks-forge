# Proposal 082 Implementation Audit R6

Proposal: `docs/proposals/082-recovery-retry-state-machine-test-matrix.md`
Commit audited: `3168e9d93d3c7ddcb1c578c9c72953a29efff844`
Audit date: 2026-06-20
Worktree note: verification ran against the current working tree at this commit. Existing modified source files were observed at final status and were left untouched by this audit.

## Verdict

Overall Conformance: Partially Implemented
Overall Readiness: Not Ready

The core P082 recovery/retry matrix implementation is substantially present: the canonical reference matrix, reason-code/schema constants, shared DB readback accessor, MCP/report/run-report/release receipt lanes, fixtures, and DB/engine/MCP proof slices all pass the canonical `proposal-082` gate. The implementation is not ready for closeout because the required live-principal boundary is not green on all proposal-required surfaces, and the gate can pass while those surfaces fail.

Reviewer-selection reuse: Not reused. `discover_prior_review.py` found no prior proposal-review artifacts for P082, and implementation-audit reports were intentionally ignored for reviewer selection per the audit skill.

Selected reviewers:
- `chainworks_execution_truth_reviewer` for durable execution truth, recovery ownership, retry identity, and readback authority.
- `rust_reliability_reviewer` for restart/retry/cancellation/convergence behavior.
- `api_contract_reviewer` for MCP/report/run-report/release receipt lane shape.
- `observability_rollout_reviewer` for proof gate, metrics, and rollout readback evidence.
- `rust_security_reviewer` for operator-only diagnostic surfaces, live principal reload, redaction, and fail-closed auth behavior.

Rejected reviewers:
- Apple/macOS UI reviewers: no SwiftUI P082 consumer or recovery authority was added; `rg` found no P082 app-side path in `Chainworks Forge/`, `Chainworks ForgeTests/`, or `Chainworks ForgeUITests/`.
- Rust performance reviewer: no benchmarked performance claim or hot-path target is introduced by the proposal.

## Findings

### P082-SEC-AUTH-001 - Major Security - MCP stdio does not pass the required live-revocation regression

P082 requires revoked, disabled, and rescoped principals to be rejected after principal-table reload on MCP stdio. The checked-in regression exists at `control-plane/crates/daemon/tests/mcp_stdio.rs:257`, rewrites the principals file at line 291, waits for reload, then expects `tools/list` to return `error.code == -32000` at lines 303-305.

Direct verification failed:

```bash
CARGO_TARGET_DIR='/Users/user/Library/Caches/Chainworks Forge/cargo-target/gates/proposal-082-stdio-extra' \
cargo test -p daemon --test mcp_stdio \
  sec_high_001_mcp_stdio_revalidates_session_after_principal_revocation -- --nocapture
```

Observed result: test failed because `response["error"]["code"]` was `Null`, not `-32000`.

Impact: a stdio session can remain usable after the principal table is revoked or changed, violating the P082 operator-only diagnostic boundary in proposal lines 32 and 107-111. This blocks implementation closeout even though the core matrix tests pass.

Required fix: make the stdio request path revalidate the current live principal source after reload, then include this exact stdio regression in `proposal-082|p082`.

### P082-GQL-001 - Major Security/Proof - GraphQL bearer-policy guard tests do not compile

P082 requires adjacent GraphQL HTTP/WebSocket bearer-policy guards to resolve the current principal table after reload. The checked-in P082 GraphQL WebSocket test file does not compile: `control-plane/crates/graphql-server/tests/p082_ws_bearer_auth.rs:14` calls `PrincipalTable::test_fixture_with_token`, which no longer exists.

Direct verification failed:

```bash
CARGO_TARGET_DIR='/Users/user/Library/Caches/Chainworks Forge/cargo-target/gates/proposal-082' \
cargo test -p graphql-server \
  sec_high_001_graphql_http_observes_live_principal_updates -- --nocapture
```

Observed compile failures:
- `p082_ws_bearer_auth.rs:14`: missing `PrincipalTable::test_fixture_with_token`.
- `graphql-server/src/server.rs:820`: missing `PrincipalTable::test_fixture_graphql_query_only` in a live-principal HTTP regression.
- `graphql-server/src/schema.rs:13473`: non-exhaustive `PrincipalClass` match omits `ReadOnlyOperator`.

Impact: the required GraphQL-adjacent live-auth proof is not runnable, and the broader `graphql-server` test target is red for the P082-auth surface. GraphQL P082 readback is optional, but GraphQL bearer-policy/live-principal guard proof is explicitly required by proposal lines 32 and 107-111.

Required fix: update the GraphQL tests/helpers for the current `PrincipalTable` API and cover `ReadOnlyOperator` in the affected match, then add the passing GraphQL auth regression to the P082 proof gate or document why another gate is the canonical owner.

### P082-GATE-001 - Major - The canonical P082 gate can pass while proposal-required live-auth surfaces fail

`./scripts/test-gate.sh proposal-082` passed. It runs DB, engine, engine integration, auth, MCP HTTP, daemon failed-serve, and MCP report-readback slices at `scripts/test-gate.sh:11476-11484`.

The same gate does not run:
- `cargo test -p daemon --test mcp_stdio sec_high_001_mcp_stdio_revalidates_session_after_principal_revocation`
- any `graphql-server` live-principal or P082 WebSocket bearer-policy test

During the gate run, `daemon/tests/mcp_stdio.rs` was compiled under the daemon package, but the current filter selected zero tests from that file. The gate still reported `Proposal 082 gate passed`.

Impact: the proof gate does not enforce the live-auth surface list that P082 itself requires, so it can produce a false green result. This violates the proposal goal to make `proposal-082|p082` the focused proof gate for the full P082 contract.

Required fix: extend `proposal-082|p082` with explicit stdio and GraphQL live-auth commands and keep the zero-test guard effective for those exact commands.

### P082-DOC-001 - Minor - `docs/reference/test-gates.md` does not document the P082 gate alias

The proposal goal requires adding `proposal-082` and `p082` aliases to `scripts/test-gate.sh` and documenting them in `docs/reference/test-gates.md`. The script alias exists, and `docs/README.md:105` links the P082 matrix reference, but `rg "proposal-082|p082|082"` in `docs/reference/test-gates.md` found no P082 gate section.

Impact: operators can discover the matrix reference from the docs README, but the canonical test-gate reference omits the P082 gate semantics, command, and host policy.

Required fix: add a `proposal-082|p082` section to `docs/reference/test-gates.md` after the gate is corrected to include all required live-auth surfaces.

## Conformance Notes

- Reference matrix: Implemented. `docs/reference/recovery-retry-state-machine-test-matrix.md` exists and the P082 gate verifies all `P082-R01` through `P082-R17` scenario IDs, schema constants, fixtures, negative fixtures, and required lane names.
- Durable storage/readback ownership: Implemented for the audited matrix paths. The accessor derives readbacks from existing owners, reads rejected retry/command evidence from `command_journal.error`, handles legacy plaintext safely, and redacts/sanitizes nested fields.
- MCP/report/run-report/release receipt lanes: Implemented for the audited P082 lanes. The gate and `proposal_082_recovery_readback` tests prove singular/plural `runs.get` placement, plural-only report lanes, operator-only gating, parity, legacy fallback rows, and no command affordances in release receipts.
- DB/engine recovery semantics: Implemented for the core matrix. The gate passed 87 DB tests, 36 engine matrix tests, and 11 engine integration tests covering validation before mutation, unique ownership, crash replay, cancellation convergence, side-effect holds, late-output quarantine, startup requeue exhaustion, and reason-code coverage.
- SwiftUI/macOS scope: Implemented by absence. No app-side P082 consumer was found; the Swift app remains read-only/tolerant by not consuming the new lanes.
- Live principal boundary: Not implemented/proven. MCP HTTP, auth core, and daemon failed-serve pass inside the gate, but MCP stdio fails when run directly and GraphQL tests do not compile.
- Gate/docs: Partial. The canonical gate passes the core matrix but misses required auth surfaces; the test-gate reference docs omit the P082 section.

## Verification

Passed:

```bash
./scripts/test-gate.sh proposal-082
```

Key passing slices observed:
- DB matrix tests: 87 passed.
- Engine matrix tests: 36 passed.
- Engine integration P082 slice: 11 passed.
- Auth live principal source regression: passed.
- MCP P082/readback slices: 7 + 17 + 19 tests passed across the gate invocations.
- Daemon failed-serve live revocation: passed.

Failed:

```bash
CARGO_TARGET_DIR='/Users/user/Library/Caches/Chainworks Forge/cargo-target/gates/proposal-082' \
cargo test -p graphql-server \
  sec_high_001_graphql_http_observes_live_principal_updates -- --nocapture
```

```bash
CARGO_TARGET_DIR='/Users/user/Library/Caches/Chainworks Forge/cargo-target/gates/proposal-082-stdio-extra' \
cargo test -p daemon --test mcp_stdio \
  sec_high_001_mcp_stdio_revalidates_session_after_principal_revocation -- --nocapture
```

Other checks:
- `security_sensitive_diff.py` and `implementation_reviewer_fingerprint.py` were treated as routing floors only because current untracked audit reports caused broad documentation-only triggers.
- Placeholder scan over P082 docs, fixtures, DB/engine/MCP tests found no placeholder/TODO/TBD markers.
- Prior-review discovery returned no reusable proposal-review artifacts.

## Closure Requirements

P082 should not be closed until all of the following are true:

1. The MCP stdio revocation regression passes and is run by `proposal-082|p082`.
2. The GraphQL P082/auth tests compile and the live-principal/bearer-policy regression passes.
3. The canonical P082 gate includes explicit stdio and GraphQL live-auth checks so it cannot pass while those surfaces fail.
4. `docs/reference/test-gates.md` documents the corrected `proposal-082|p082` gate.
5. `./scripts/test-gate.sh proposal-082` passes after the gate is corrected.
