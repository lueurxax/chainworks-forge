# Proposal 082 Implementation Audit R2

Generated: 2026-06-03

Skill: `proposal-implementation-audit`

Proposal: `docs/proposals/082-recovery-retry-state-machine-test-matrix.md`

Target worktree: `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-082-recover-a09a1918`

Branch: `cw/implement-proposal-082-recover/a09a1918`

HEAD audited: `fe217cb67064f1050c744c9d027e879cdbdc309a`

Merge base with `main`: `94ceec201b5c14aef8a1118e935004fb69234051`

Report path: `docs/proposals/082-recovery-retry-state-machine-test-matrix_IMPLEMENTATION_AUDIT_R2.md`

## Verdict

Track 1 requirement conformance: **Implemented**

Track 2 specialist readiness: **Ready with Risks**

Same-tree canonical gate: **PASS**

Command run from the target worktree:

```bash
./scripts/test-gate.sh proposal-082
```

Gate result summary:

- Static P082 checks passed, including 17 scenario IDs, reason-code vocabulary, nested schema terms, lane placement, YAML parser advisory controls, metrics, and required test-name presence.
- `workflow` P082 YAML-boundary regression: 1 passed.
- `db --test proposal_082_recovery_retry_matrix`: 67 passed.
- `engine --test proposal_082_recovery_retry_matrix`: 35 passed.
- `engine --test integration p082_`: 2 passed.
- `engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager`: 1 passed.
- `mcp-server --test proposal_082_recovery_readback`: 16 passed.
- `graphql-server --test p082_live_revocation`: 7 passed.
- `mcp-server p082_mcp_stdio`: 5 passed.
- `db p082_sanitize_string_redacts_absolute_paths_after_punctuation_boundaries`: 1 passed.

Gate log caveat: the provider-cleanup test returned `ok`, but the same run printed a background Tokio shutdown panic:

```text
thread 'projection-rebuild-all-for-run' (...) panicked ... A Tokio 1.x context was found, but it is being shutdown.
```

This does not invalidate the gate exit status, but it is a readiness risk for final closeout.

## Reviewer Reuse

No prior specialist review artifacts were discovered by the audit helper for this target. This R2 audit did not use previous implementation audits for reviewer selection, per skill policy. Existing implementation-audit files were treated as historical artifacts only, not as evidence to copy forward.

## Track 1 Requirement Conformance

| Requirement | Evidence | Status |
|---|---|---|
| Canonical reference matrix with all P082-R01 through P082-R17 rows, required columns, durable owners, crash/replay proof, and observability thresholds | `docs/reference/recovery-retry-state-machine-test-matrix.md` defines the scenario convention, reason-code vocabulary, and the full canonical matrix. The gate statically checks all 17 IDs and required terms in `scripts/test-gate.sh`. | Implemented |
| Gate aliases `proposal-082` and `p082` | `scripts/test-gate.sh` contains the `proposal-082|p082` case and `docs/reference/test-gates.md` documents the gate. | Implemented |
| DB and engine tests for validation before mutation, unique ownership, idempotent replay, cancellation convergence, provider cleanup, late-output quarantine, and no blind retry | Gate ran DB 67 tests, engine 35 tests, engine integration 2 tests, and provider cleanup proof 1 test. | Implemented |
| MCP/report/run-report P082 lane placement and parity | `runs.get` wires singular and plural fields; `reports.get`, `report://`, and generated run report wire plural-only readbacks. MCP P082 readback suite passed 16 tests. | Implemented |
| Release side-effect retry fail-closed while unresolved ledger rows exist | Reference matrix and gate assert side-effect hold behavior; DB/engine/MCP tests include R07/R13 and release receipt lane coverage. | Implemented |
| Shared reason-code constants and nested schemas | `control-plane/crates/domain/src/recovery_matrix.rs` is required by the gate; gate checks constants and schema helpers. DB/engine tests validate schema shape and negative fixtures. | Implemented |
| Rejected command readback stored in `command_journal.error`, not `command_journal.payload_json` | Reference doc documents the typed envelope and payload non-mutation; gate statically checks this contract and DB tests cover malformed envelope rejection and payload non-mutation. | Implemented |
| Startup requeue exhausted held state | Matrix row P082-R16 is documented and gate requires named DB/engine proof tests for R16. DB tests passed R16 held-state and metric cases. | Implemented |
| Cancel-then-late-output quarantine | Matrix row P082-R17 is documented; engine and MCP tests passed cancelled provider late-output coverage. | Implemented |
| Provider subprocess cleanup proof | Gate requires `test_cancel_run_finalize_closes_live_session_via_runtime_manager`; the test passed, with the shutdown-panic caveat recorded below. | Implemented with readiness risk |
| GraphQL advisory readback | No P082 GraphQL readback fields (`p082RecoveryMatrixReadbackJson` or `p082RecoveryMatrixReadbacksJson`) were introduced, so the optional tolerant GraphQL readback obligation is not triggered. New GraphQL live-auth tests are outside readback lane semantics and passed. | Not applicable / no gap |
| Swift/macOS read-only boundary | Repository search found no Swift P082 consumer or app-side P082 recovery authority. The reference keeps future UI integration out of P082 scope. | Implemented |
| YAML parser advisory compensating controls | Reference doc documents the advisory boundary. `workflow::YAML_INPUT_MAX_BYTES` and bounded loader are implemented; the gate checks workflow/catalog/compiler/transition-lint use and `MCP_REGISTRY_YAML_MAX_BYTES`. Workflow regression passed. | Implemented |

## Track 2 Specialist Findings

### READY-P082-R2-001 - Passing gate still emits a background Tokio shutdown panic

Severity: Medium

Track: Reliability / release readiness

Evidence:

- `./scripts/test-gate.sh proposal-082` exited successfully.
- During `test_cancel_run_finalize_closes_live_session_via_runtime_manager`, the log printed: `thread 'projection-rebuild-all-for-run' (...) panicked ... A Tokio 1.x context was found, but it is being shutdown.`
- The test result immediately following the panic was `ok`.

Impact:

The P082 provider-cleanup proof exists and passes, so Track 1 conformance is satisfied. However, a background projection task panicking during runtime shutdown is not clean release evidence. It can hide real teardown ordering defects, produce noisy CI logs, and make future regression triage harder.

Recommendation:

Before final closeout or merge sign-off, either fix the teardown ordering so the projection rebuild task is joined/cancelled before the runtime shuts down, or add a focused proof showing this panic cannot affect durable cancellation/provider-cleanup state and eliminate the panic from gate output.

### READY-P082-R2-002 - The passing gate depends on a dirty worktree, including an untracked test file

Severity: Medium

Track: Merge readiness / reproducibility

Evidence:

- `git status --short` after the audit shows many modified files plus untracked `control-plane/crates/graphql-server/tests/p082_live_revocation.rs`.
- The P082 gate invokes `cargo test -p graphql-server --test p082_live_revocation` in `scripts/test-gate.sh`.
- The audited same-tree gate passed because that untracked file exists in the current worktree.

Impact:

The current worktree is conformant, but the committed branch HEAD alone may not reproduce the R2 gate result if the untracked test is omitted. This is a release-process risk, not a behavior gap in the live audited tree.

Recommendation:

Before closeout, stage/commit every file required by the P082 gate or rerun `./scripts/test-gate.sh proposal-082` from a clean worktree/commit that exactly matches the merge candidate.

### READY-P082-R2-003 - Branch scope includes broad non-P082 changes

Severity: Low

Track: Scope control / reviewability

Evidence:

- `git diff --name-status main...HEAD` includes P082 implementation files, but also broad unrelated-looking artifacts: P079 and P086 rollout fixtures, deleted P058 proposal/audit files, Swift UI files, helper scripts (`aggregate.py`, `generate_artifacts.py`, `summarize.py`), and unrelated proposal/reference docs.
- The latest commit is named `Sync main worktree changes into P082`, which indicates this branch absorbed more than proposal-scoped P082 work.

Impact:

This does not block P082 behavioral conformance because the same-tree P082 gate passed. It does make merge review and rollback riskier: reviewers must separate P082 recovery/readback changes from other proposal or workspace synchronization changes.

Recommendation:

For closeout/merge, isolate P082 changes or explicitly document which non-P082 changes are intentional prerequisites versus incidental workspace contamination.

## Notes

- No Swift app-facing P082 consumer was found.
- No GraphQL P082 readback field was found; GraphQL remains outside the P082 readback lane contract in this implementation.
- The audited tree expands P082 gate coverage beyond the original proposal commands to include YAML-boundary proof, GraphQL live principal revocation, MCP stdio live-auth rechecks, and sanitizer regressions. Those additions passed in the same-tree gate.

