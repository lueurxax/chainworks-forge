# P082 Implementation Audit R3: Recovery Retry State Machine Test Matrix

Date: 2026-06-04

Proposal: `docs/proposals/082-recovery-retry-state-machine-test-matrix.md`

Worktree: `.chainworks/worktrees/cw-implement-proposal-082-recover-a09a1918`

Branch: `cw/implement-proposal-082-recover/a09a1918`

Audited HEAD: `fe217cb67064f1050c744c9d027e879cdbdc309a`

Merge base with `main`: `94ceec201b5c14aef8a1118e935004fb69234051`

Reviewer reuse: not reused. No prior `proposal-review` artifacts were found for P082; prior implementation audit files were treated as history, not as specialist-review coverage.

## Verdict

Track 1 - Proposal conformance: **Partially Implemented**.

Track 2 - Implementation readiness: **Not Ready**.

The core P082 recovery matrix behavior is substantially present: the reference matrix, DB/engine/MCP/readback tests, rejection envelopes, readback redaction, no-migration posture, and canonical `proposal-082` gate all exist and pass in this worktree. The readiness verdict is blocked by security-sensitive auth drift on failed-serve surfaces, an explicit P082 metric emission-site mismatch, one untracked gate dependency, and a background runtime panic observed during the canonical gate.

## Evidence Summary

Canonical gate:

- `./scripts/test-gate.sh proposal-082` - **PASS**.
- Included DB matrix tests: 67 passed.
- Included engine matrix tests: 35 passed.
- Included MCP readback tests: 16 passed.
- Included GraphQL live revocation tests: 7 passed.
- Included MCP stdio P082 tests: 5 passed.
- Included auth/workflow/GraphQL focused tests: passed.
- Caveat: the gate output logged a background panic from `projection-rebuild-all-for-run` while the test process still returned success.

Additional security-sensitive checks run manually because the branch changed auth and ingress surfaces:

- `CARGO_TARGET_DIR=target/proposal-082-audit-extra cargo test -p mcp-server p082_mcp_http -- --nocapture` - **PASS**, 2 tests.
- `CARGO_TARGET_DIR=target/proposal-082-audit-extra cargo test -p graphql-server --test p082_ws_bearer_auth -- --nocapture` - **PASS**, 9 tests.
- `CARGO_TARGET_DIR=target/proposal-082-audit-extra cargo test -p graphql-server --test request_id_propagation -- --nocapture` - **PASS**, 3 tests.
- `CARGO_TARGET_DIR=target/proposal-082-audit-extra cargo test -p auth -- --nocapture` - **PASS**, 107 tests.
- `CARGO_TARGET_DIR=target/proposal-082-audit-extra cargo test -p graphql-server --test proposal_046_session_graphql -- --nocapture` - **PASS**, 83 tests.

Hard-gate helpers:

- `implementation_surface_fingerprint.py --root <worktree> --json` returned broad merge-base-to-HEAD lenses: API contract, Apple UI/UX, architecture, observability/rollout, performance, reliability, and security. P082-scoped reviewer coverage selected the maximum five relevant lenses: Rust architecture, Rust reliability, API contract, observability/rollout, and Rust security.
- `security_sensitive_diff.py --root <worktree> --json` triggered **true** for auth, public ingress, parser boundary, filesystem/subprocess boundary, DoS/resource limits, secrets/redaction/privacy, and unsafe crypto/dependency surface. Security review was therefore mandatory and is included below.

Rejected specialist lenses:

- Apple UI/UX: rejected for P082 scope. `rg` found no P082 Swift app implementation; the proposal explicitly makes Swift/UI diagnostic consumption optional/future-only.
- Rust performance: rejected as a standalone reviewer because P082 has no latency/throughput/benchmark acceptance contract. Resource-limit and DoS implications were covered under security and observability.
- Product reviewer: rejected because the proposal is a control-plane correctness and operator-readback contract, not a product-flow or copy review.

## Requirement Matrix

| Requirement | Status | Evidence |
| --- | --- | --- |
| Canonical reference matrix with P082-R01..R17, reason codes, and lane placement | Implemented | `docs/reference/recovery-retry-state-machine-test-matrix.md`; static gate checks pass. |
| No schema migration; use existing durable tables and `command_journal.error` readback | Implemented | `control-plane/crates/db/tests/proposal_082_recovery_retry_matrix.rs`; canonical gate pass. |
| Single projection/readback accessor shared by MCP/report/run-report/release lanes | Implemented | `control-plane/crates/mcp-server/src/tools/reports.rs:261`; `control-plane/crates/db/src/repos/p082_recovery_matrix.rs`. |
| `runs.get` singular + plural P082 fields; report lanes plural-only | Implemented | `control-plane/crates/mcp-server/src/tools/runs.rs:465`; MCP readback tests pass. |
| Operator-only readback and redaction of raw diagnostics, auth material, command payloads, paths | Implemented | `control-plane/crates/db/src/repos/p082_recovery_matrix.rs`; MCP readback tests pass. |
| Retry/cancel/recovery rejection envelopes and mutation-denial metrics | Implemented | Engine/DB P082 matrix tests pass. |
| Crash/restart, cancellation replay, late-output terminalization, startup requeue, and Xcode grace semantics | Implemented with readiness caveat | DB/engine matrix tests pass; canonical gate logs a background Tokio shutdown panic. |
| GraphQL P082 readback | Not implemented by design | Proposal makes GraphQL advisory optional; no P082 GraphQL diagnostic field is present, so optional GraphQL P082 readback tests are not required. |
| Swift app-facing P082 readback | Not implemented by design | No P082 Swift implementation was found; proposal makes this future/optional. |
| P082 metric emission sites | Partially implemented | See OPS-001. |
| Security-sensitive auth and ingress behavior introduced in this branch | Partially implemented | Main GraphQL/MCP paths use live auth; failed-serve paths still use a static principal table. See SEC-001. |

## Findings

### SEC-001 - Failed-serve GraphQL/MCP auth uses a static principal table

Severity: **Major**

The branch introduces `auth::LivePrincipalTable` with comments stating that all authenticated control-plane surfaces observe revocation, disable, expiry, and auth-source unavailability without a daemon restart (`control-plane/crates/auth/src/lib.rs:709`). Normal GraphQL HTTP, GraphQL WebSocket, subscriptions, MCP HTTP, and MCP stdio use that live handle and are covered by passing tests.

The failed-serve path does not use the live handle. `serve_failed` still accepts and clones a static `auth::PrincipalTable` (`control-plane/crates/daemon/src/main.rs:1563`), and `build_failed_serve_router` stores that snapshot in Axum extensions (`control-plane/crates/daemon/src/failed_serve.rs:33`). Both failed-serve `/graphql` and `/mcp` then call `auth::resolve_bearer` against that static table (`control-plane/crates/daemon/src/failed_serve.rs:90`, `control-plane/crates/daemon/src/failed_serve.rs:206`).

Impact: if the daemon enters failed-serve mode, a token revoked or expired after failed-serve startup can continue to access authenticated failure diagnostics until the process restarts. Auth-source reload failure also cannot make those surfaces fail closed. This is outside P082's original recovery-matrix contract, but it is a readiness blocker because the security-sensitive diff changed auth/public ingress behavior and the new live-auth claim is broader than the implementation.

Expected fix: thread `LivePrincipalTable` into failed-serve, start or reuse the principal-table reload loop for failed-serve mode, make failed-serve `/graphql` and `/mcp` call live resolution, and add failed-serve revocation/fail-closed tests.

### OPS-001 - `p082_recovery_matrix_gate_result_total` is emitted from runtime readback, not the gate assertion groups

Severity: **Major**

The proposal explicitly defines `p082_recovery_matrix_gate_result_total{scenario_id,status}` with emission site "proposal-082 gate harness after each scenario assertion group" (`docs/proposals/082-recovery-retry-state-machine-test-matrix.md:161`). The implementation emits that metric inside the production readback accessor loop (`control-plane/crates/db/src/repos/p082_recovery_matrix.rs:927`, `control-plane/crates/db/src/repos/p082_recovery_matrix.rs:963`).

The gate script also writes synthetic `.prom` rows for all 17 scenarios before running the cargo assertions (`scripts/test-gate.sh:10926`). That proves a file can be generated, but it is not "after each scenario assertion group" and it is disconnected from assertion success/failure.

Impact: runtime readback calls can inflate or skew a gate-result metric, while the actual gate does not emit the metric at the proposal's required assertion boundary. This breaks the observability contract even though the functional tests pass.

Expected fix: move `p082_recovery_matrix_gate_result_total` emission into the actual P082 gate assertion groups, after each scenario group has passed or failed. If runtime readback status is useful, emit it under a separate metric name. Keep `p082_recovery_reason_readback_total{reason_code,lane}` in the shared readback accessor, which matches the proposal.

### READY-001 - Canonical P082 gate depends on an untracked GraphQL test file

Severity: **Major**

`scripts/test-gate.sh` runs `cargo test -p graphql-server --test p082_live_revocation` as part of `proposal-082` (`scripts/test-gate.sh:10965`), and the local gate passes. However, `control-plane/crates/graphql-server/tests/p082_live_revocation.rs` is untracked in this worktree (`git ls-files --stage -- control-plane/crates/graphql-server/tests/p082_live_revocation.rs` returned no entry).

Impact: the proof gate is not reproducible from the committed branch state. A reviewer or CI runner that checks out the branch without untracked files will either fail to find the test target or silently lose the intended security proof if the script is changed later.

Expected fix: add the test file to version control, or remove the gate dependency and replace it with committed tests that cover the same live-revocation behavior.

### REL-001 - Canonical gate passes while logging a background Tokio shutdown panic

Severity: **Medium**

During `./scripts/test-gate.sh proposal-082`, the focused engine integration test `test_cancel_run_finalize_closes_live_session_via_runtime_manager` passed, but the output included a panic from a background thread named `projection-rebuild-all-for-run`:

`A Tokio 1.x context was found, but it is being shutdown.`

Impact: this does not currently fail the gate, but it weakens the P082 reliability proof. P082's reliability section requires crash/restart and cancellation cleanup evidence; a background projection task panicking during the proof path is exactly the kind of lifecycle leak the gate should either prevent or surface as a failed test.

Expected fix: ensure the projection rebuild task is joined, aborted, or drained before runtime shutdown, or make the test harness fail on background panics so the cleanup contract is enforceable.

### READY-002 - Canonical gate omits MCP HTTP live-revocation tests

Severity: **Minor**

The branch changes `control-plane/crates/mcp-server/src/http.rs` and contains two focused P082 MCP HTTP tests (`p082_mcp_http_rejects_token_after_live_revocation` and `p082_mcp_http_fails_closed_when_auth_source_unavailable` at `control-plane/crates/mcp-server/src/http.rs:519` and `control-plane/crates/mcp-server/src/http.rs:559`). They pass when run manually, but `scripts/test-gate.sh proposal-082` only runs MCP readback tests and MCP stdio tests, not the MCP HTTP live-auth tests.

Impact: this audit covers the surface manually, but the durable proposal gate will not catch future regressions in MCP HTTP live revocation/fail-closed behavior.

Expected fix: add the MCP HTTP live-revocation test filter to `proposal-082` or to another documented security gate that is required for this branch.

## Security Review Notes

Security-sensitive categories were all inspected:

- Auth/public ingress: normal GraphQL HTTP, GraphQL WebSocket, subscriptions, MCP HTTP, and MCP stdio use `LivePrincipalTable` and pass focused tests. Failed-serve remains static and is the blocking finding.
- Secrets/redaction/privacy: P082 readbacks use operator-only gating and DB-side allowlist/redaction; non-operator MCP lanes return `null` or `[]`.
- Parser boundary/DoS: workflow YAML and MCP registry YAML loaders enforce byte caps before parse.
- Filesystem/subprocess boundary: ACP/provider cleanup is covered by the P082 cancellation proof, with the background-panic caveat above.
- Unsafe dependency/parser risk: `unsafe-libyaml` exposure is constrained to bounded local YAML config inputs; no public YAML parse ingress was found.

## Closeout Recommendation

Do not close out P082 yet. Fix SEC-001, OPS-001, READY-001, and REL-001 first; then rerun:

- `./scripts/test-gate.sh proposal-082`
- `CARGO_TARGET_DIR=target/proposal-082-audit-extra cargo test -p mcp-server p082_mcp_http -- --nocapture`
- the GraphQL/auth focused tests listed in this report, unless they are incorporated into the canonical gate

After those pass from tracked files only and without background panic output, P082 can be re-audited for a Ready verdict.
