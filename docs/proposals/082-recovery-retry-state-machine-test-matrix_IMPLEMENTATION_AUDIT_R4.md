# P082 Implementation Audit R4: Recovery and Retry State-Machine Test Matrix

## Metadata

| Field | Value |
| --- | --- |
| Audit date | 2026-06-04 |
| Proposal | `docs/proposals/082-recovery-retry-state-machine-test-matrix.md` |
| Proposal id / revision | P082 / `P082-r4-4d5cc83d-20260521` |
| Proposal state | Active: `approved_for_implementation_review` |
| Implementation target | `.chainworks/worktrees/cw-implement-proposal-082-recover-a09a1918` |
| Branch | `cw/implement-proposal-082-recover/a09a1918` |
| Audited HEAD | `fe217cb67064f1050c744c9d027e879cdbdc309a` |
| Compare base | `main...HEAD`, merge-base `94ceec201b5c14aef8a1118e935004fb69234051` |
| Audit mode | `auto` / implementation readiness |
| Report path | `docs/proposals/082-recovery-retry-state-machine-test-matrix_IMPLEMENTATION_AUDIT_R4.md` |

## Final Verdict

Overall conformance: **Implemented**.

Overall implementation readiness: **Ready with Risks**.

Reviewer-selection reuse: **Not reused**. The discovery helper found no prior proposal-review artifacts for P082. Prior `IMPLEMENTATION_AUDIT` reports were intentionally ignored for reviewer selection, per skill rules.

Audit confidence: **High** for Rust control-plane, MCP, GraphQL auth, DB/engine matrix, and readback lanes. **Medium** for broad merge readiness because the branch contains non-P082 historical surfaces beyond this proposal's scope.

Post-refinement docs sync note, 2026-06-15: the current `scripts/test-gate.sh` `proposal-082|p082` alias executes the Python static fixture/matrix checklist before the focused DB, engine, MCP, auth, MCP HTTP live-revocation, and daemon failed-serve revocation suites. It does not currently include GraphQL P082 readback tests because P082 GraphQL readback remains optional and unimplemented; workflow YAML and background-panic detector checks are not part of the alias. Canonical current gate semantics are in `docs/reference/test-gates.md`; treat stale gate-inventory statements below as superseded by this note and the corrected sections. Failed-serve GraphQL/MCP diagnostics now resolve bearer tokens through `auth::LivePrincipalSource`, so the prior R3 `SEC-001` static-auth finding is resolved in the current tree.

## Implementation Target / Compare Base

- Repo root: `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-082-recover-a09a1918`
- Current SHA: `fe217cb67064f1050c744c9d027e879cdbdc309a`
- Merge base: `94ceec201b5c14aef8a1118e935004fb69234051`
- Working tree: dirty by design for the implementation under audit. Post-refinement docs sync found no tracked `control-plane/crates/graphql-server/tests/p082_live_revocation.rs` entry, and GraphQL P082 readback remains optional/unimplemented. Prior local audit sidecars `R2` and `R3` remain untracked/non-implementation evidence and are ignored.

## Prior Review Reuse

No prior proposal-review artifacts were found beside the proposal, in repo-local review folders, or via `discover_prior_review.py`. Reuse state: **Not reused**.

Implementation audit files `R1`-`R3` were not used as prior reviewer-selection input. R3 was only used as local history to verify that its previously observed blockers were re-tested on the current tree.

## Selected Reviewers

| Reviewer | Lens | Why selected |
| --- | --- | --- |
| `rust_arch_reviewer` | Architecture | P082 changes DB repos, engine command paths, MCP/GraphQL shared auth handles, and runtime ownership boundaries. |
| `rust_reliability_reviewer` | Reliability | Proposal centers on retry, recovery, cancellation, idempotency, crash replay, late output, and provider/session cleanup. |
| `api_contract_reviewer` | API contract | MCP `runs.get`, `reports.get`, `report://`, run-report JSON, release receipt lanes, GraphQL optional boundary, and error/readback schemas are explicit contract surfaces. |
| `observability_rollout_reviewer` | Ops/rollout | Proposal mandates gate aliases, rollout fixtures, metrics, readback thresholds, and release-readiness evidence. |
| `rust_security_reviewer` | Security | Security-sensitive diff triggered auth, ingress, redaction/privacy, parser, subprocess/filesystem, DoS/resource-limit, and dependency/parser categories. |

Rejected close alternatives:

- Apple UI/UX: rejected for P082 scope. The proposal explicitly excludes new Forge UI, Swift app-facing recovery authority, native notifications, Dock badges, keyboard/context-menu affordances, and new screens.
- Rust performance: rejected as a standalone reviewer. No latency, throughput, allocation, or benchmark target is committed; resource-limit and parser DoS controls were covered under security/ops.
- Product reviewer: rejected. P082 is a correctness/readback/gate proposal, not a product-flow or experiment decision proposal.

## Proposal Contract Summary

P082 creates the canonical recovery/retry state-machine matrix and proof gate for startup repair, retry validation, cancellation, stale startup, late output, side effects, approvals, duplicate sessions, mediation, crash replay, and operator readback. It preserves a no-migration posture, stores rejected-command readback in `command_journal.error`, adds defensive legacy parsing, keeps GraphQL and Swift consumption optional/tolerant/diagnostic-only, and forbids blind retry, approval auto-resolution, side-effect retry while unresolved, and app-side mutation authority.

Platform/product scope:

- Apple: macOS/SwiftUI is explicitly **out of implementation scope** except future read-only/tolerant consumption constraints.
- Backend/service: Rust control-plane DB, engine, MCP, GraphQL auth/diagnostics, workflow YAML loading, rollout fixtures, metrics, and gate scripts.
- Cross-stack contract: MCP/report/run-report/release readback shapes and optional GraphQL/Swift boundaries.

Primary service flows audited:

1. Startup repair/requeue converges once per idempotency key and exposes P082 readback.
2. Invalid retry/cancel/recovery commands reject before mutation and store typed, redacted `command_journal.error` envelopes.
3. Cancellation, late output, provider cleanup, approvals, mediation, side effects, and crash replay preserve durable ownership and fail closed.
4. Operator readback lanes expose exact singular/plural fields with redaction and non-operator gating.
5. Security-sensitive live principal changes propagate to normal GraphQL HTTP/WS/subscription/mutation/query guards, MCP HTTP/stdio, and failed-serve diagnostic surfaces.

## Proposal Fidelity Inventory

Matches:

- Reference matrix exists and covers P082-R01 through P082-R17.
- `proposal-082` and `p082` aliases exist and are documented.
- The current canonical gate executes the DB P082 matrix, engine P082 matrix, and MCP P082 readback suites.
- Rejected command readback uses `command_journal.error` typed envelope; `payload_json` is not used as post-validation owner.
- MCP `runs.get` exposes singular and plural readback; report lanes are plural-only.
- GraphQL P082 readback is not implemented, so optional GraphQL P082 readback tests are not required.
- Swift app-facing P082 readback is not implemented, consistent with explicit out-of-scope language.
- Runtime readback accessor no longer emits `p082_recovery_matrix_gate_result_total`.

Divergences:

- `p082_recovery_matrix_gate_result_total` is exercised by a DB harness test rather than emitted by the shell gate after each scenario assertion group. Runtime readback emission is fixed, but this is weaker than the literal "proposal-082 gate harness after each scenario assertion group" wording.

Ambiguities / evidence gaps:

- Broad helper output includes unrelated branch surfaces from older proposal work; this audit scopes readiness to P082 and directly adjacent security/reliability surfaces.

## Residual Scope / Follow-up Ownership

| Item | Owner | Blocks conformance/readiness? | Notes |
| --- | --- | --- | --- |
| Future Forge UI consumption of P082 readback | Separate future UI proposal required by P082 | No | P082 explicitly excludes UI implementation. |
| Advisory GraphQL P082 readback fields | Optional only if implementation adds them | No | No P082 GraphQL readback fields were found; GraphQL changes here are auth hardening. |
| Swift app-facing decode/tolerance tests | Optional only if Swift path is added | No | No P082 Swift implementation was found. |
| Strict per-assertion gate-result metric timing | P082 gate implementation | No, bounded risk | The current alias executes the static fixture/matrix checklist and focused Rust tests; the DB harness exercises gate-result metric emission, but the shell gate does not emit it after each scenario group. |

Unowned residual scope blocking conformance/readiness: **none**.

## Specialist Coverage Matrix

| Triggered surface | Required lens | Completed pass | Result |
| --- | --- | --- | --- |
| Retry/recovery/cancellation/crash/session lifecycle | Reliability | `rust_reliability_reviewer` | Pass with no blocking findings. |
| MCP/report/run-report/release/GraphQL boundary shapes | API contract | `api_contract_reviewer` | Pass. |
| Metrics, gates, rollout fixtures, docs | Observability/rollout | `observability_rollout_reviewer` | Pass with bounded OPS-001 risk. |
| Crate/module/runtime/auth handle ownership | Architecture | `rust_arch_reviewer` | Pass. |
| Auth, public ingress, redaction, YAML parse, MCP/GraphQL/failed-serve boundaries | Security | `rust_security_reviewer` | Pass for P082 readback scope; prior adjacent failed-serve static-auth risk resolved. |
| Swift/macOS UI | Apple UI/UX | Rejected | No P082 UI implementation in scope. |
| Hot-path/benchmark performance | Performance | Rejected | No proposal performance target; resource limits reviewed under security. |

No mandatory lens is missing for the P082 implementation slice.

## Requirement Summary

| Req | Title | Status |
| --- | --- | --- |
| REQ-001 | Reference recovery/retry matrix | Implemented |
| REQ-002 | Gate aliases and documentation | Implemented |
| REQ-003 | DB/engine scenario proof | Implemented |
| REQ-004 | MCP/report/run-report/release readback lane contract | Implemented |
| REQ-005 | No migration and durable owner mapping | Implemented |
| REQ-006 | Rejected command typed error envelope and legacy fallback | Implemented |
| REQ-007 | Positive/negative rollout fixtures | Implemented |
| REQ-008 | No blind retry, no approval auto-resolution, side-effect fail-closed | Implemented |
| REQ-009 | Reliability crash/replay/cancel/late-output/session cleanup semantics | Implemented |
| REQ-010 | GraphQL and Swift optional diagnostic boundaries | Implemented |
| REQ-011 | Metrics and observability thresholds | Implemented with risk |
| REQ-012 | Security/redaction/parser compensating controls introduced by implementation | Implemented |

## Detailed Requirement Audit

### REQ-001 - Reference recovery/retry matrix

Proposal source: Goals and Architecture / Documentation.

Status: **Implemented**.

Evidence: `docs/reference/recovery-retry-state-machine-test-matrix.md`; `./scripts/test-gate.sh proposal-082`; focused DB, engine, and MCP gate suites.

Mapping: The reference doc contains all 17 scenario rows, required storage owner/projection/readback/crash/observability columns, reason codes, nested schemas, and future extension guidance.

### REQ-002 - Gate aliases and documentation

Proposal source: Metadata gate aliases; Gate section.

Status: **Implemented**.

Evidence: `scripts/test-gate.sh`; `docs/reference/test-gates.md:2220`.

Mapping: Both `proposal-082` and `p082` aliases resolve to the P082 gate. The current alias executes the static fixture/matrix checklist, DB and engine P082 suites, engine integration P082 checks, MCP P082 readback suites, auth live-principal revalidation, MCP HTTP live-revocation, and daemon failed-serve revocation checks. GraphQL P082 readback tests remain optional because GraphQL P082 readback is not implemented.

### REQ-003 - DB/engine scenario proof

Proposal source: Expected commands, fail-closed conditions, reliability semantics.

Status: **Implemented**.

Evidence: `cargo test -p db --test proposal_082_recovery_retry_matrix` 67 tests passed inside the gate; `cargo test -p engine --test proposal_082_recovery_retry_matrix` 35 tests passed; engine integration P082 tests passed.

Mapping: Tests cover validation before mutation, scenario vocabulary, crash replay, startup requeue, late-output settlement, cancellation, approval holds, side-effect holds, and Xcode grace.

### REQ-004 - MCP/report/run-report/release readback lane contract

Proposal source: Operator Surfaces; P082 Recovery Matrix Readback V1.

Status: **Implemented**.

Evidence: `control-plane/crates/mcp-server/src/tools/runs.rs`; `control-plane/crates/mcp-server/src/tools/reports.rs`; `control-plane/crates/mcp-server/tests/proposal_082_recovery_readback.rs`; 16 MCP readback tests passed.

Mapping: `runs.get` exposes singular and plural fields; `reports.get`, `report://`, run-report, and release receipt lanes use plural readbacks and operator-only redacted data.

### REQ-005 - No migration and durable owner mapping

Proposal source: Durable Storage Mapping.

Status: **Implemented**.

Evidence: DB tests; no migration files added for P082; `control-plane/crates/db/src/repos/p082_recovery_matrix.rs`.

Mapping: The implementation uses existing durable owners and accessors, with rejected command rows owned by `command_journal.error`.

### REQ-006 - Rejected command typed error envelope and legacy fallback

Proposal source: P082 Rejected Command Error V1.

Status: **Implemented**.

Evidence: DB tests for typed envelope, malformed envelope, legacy plain-text fallback, and no raw JSON exposure; MCP readback tests pass.

Mapping: `p082_rejected_command_error_v1` is parsed defensively; legacy text falls back to safe unavailable/stale summaries without panics or raw display JSON.

### REQ-007 - Positive/negative rollout fixtures

Proposal source: Fixtures and rollout contract.

Status: **Implemented**.

Evidence: `docs/evidence/rollout-contract/operator-readback/p082-full-surface.fixture.json`; 16 `docs/evidence/rollout-contract/negative/p082-*.json` files; retained inert static checklist in `scripts/test-gate.sh`.

Mapping: Positive fixture covers all lanes and nested subcontracts. Negative fixture inventory is listed, but current alias does not execute the retained Python fixture-shape checklist; active behavioral proof is provided by focused Rust suites.

### REQ-008 - No blind retry, no approval auto-resolution, side-effect fail-closed

Proposal source: Non Goals; fail-closed conditions.

Status: **Implemented**.

Evidence: Engine/DB P082 tests and gate pass.

Mapping: Retry eligibility validates before mutation, approval pending states are not auto-resolved, and unresolved side-effect ledger entries block retry/release work.

### REQ-009 - Reliability crash/replay/cancel/late-output/session cleanup semantics

Proposal source: Reliability Semantics.

Status: **Implemented**.

Evidence: DB crash-boundary tests, engine integration tests, and `test_cancel_run_finalize_closes_live_session_via_runtime_manager`.

Mapping: The historical audit recorded a pass without the previous background Tokio shutdown panic. Post-refinement docs sync found no separate panic-output detector in the current `proposal-082|p082` alias.

### REQ-010 - GraphQL and Swift optional diagnostic boundaries

Proposal source: Operator Surfaces; Swift Macos Contract.

Status: **Implemented**.

Evidence: `rg` found no P082 GraphQL readback fields or Swift P082 implementation; GraphQL auth tests pass.

Mapping: P082 GraphQL readback and Swift consumption were not added, so optional diagnostic-only/tolerant tests are not required. GraphQL changes are live-auth hardening, not P082 readback authority.

### REQ-011 - Metrics and observability thresholds

Proposal source: Metrics section and observability thresholds.

Status: **Implemented with risk**.

Evidence: DB metric tests; `control-plane/crates/db/src/repos/p082_recovery_matrix.rs`; `docs/reference/test-gates.md:2229`.

Mapping: Runtime readback emits coverage, reason/readback lane, and state-age metrics, and no longer emits the gate-result metric. A DB harness test exercises `p082_recovery_matrix_gate_result_total{scenario_id,status}` for all 17 scenarios, but the current shell gate does not write a `.prom` artifact after each scenario assertion group.

Gap/risk: the shell gate does not emit the gate-result metric after each scenario assertion group. This is weaker than the proposal's literal observability timing language.

### REQ-012 - Security/redaction/parser compensating controls introduced by implementation

Proposal source: P082 readback redaction; security-sensitive implementation diff.

Status: **Implemented for P082 readback; adjacent failed-serve auth risk resolved**.

Evidence: `security_sensitive_diff.py` triggered; manual security pass; GraphQL/MCP/auth/workflow tests passed. Post-refinement docs sync inspected `control-plane/crates/daemon/src/main.rs` and `control-plane/crates/daemon/src/failed_serve.rs` and found failed-serve now threads `auth::LivePrincipalSource` through GraphQL and MCP diagnostic handlers.

Mapping: `LivePrincipalSource` gates normal GraphQL HTTP/WS/query/mutation/subscription paths, MCP HTTP, MCP stdio, and failed-serve GraphQL/MCP diagnostics. YAML loaders perform bounded opened-file reads before `serde_yaml` parse. P082 readback strips unknown/sensitive keys and redacts paths/raw diagnostics. The current `proposal-082|p082` alias includes focused auth, MCP HTTP, and failed-serve revocation checks; GraphQL P082 readback tests remain unnecessary because P082 GraphQL readback is not implemented.

## Reviewer / Lens Scorecard

| Lens | Conformance | Readiness | Top risk | Confidence |
| --- | --- | --- | --- | --- |
| Rust architecture | Pass | Ready | Broad branch contains unrelated proposal surfaces | Medium |
| Reliability | Pass | Ready | Long-running reload tasks are intentionally detached; no shutdown contract claimed | High |
| API contract | Pass | Ready | Optional GraphQL P082 readback intentionally absent | High |
| Observability/rollout | Pass | Ready with risk | Gate-result artifact timing is pre-cargo rather than post-group | Medium |
| Security | Pass for P082 scope | Ready | Prior failed-serve static-auth revocation gap is covered by live-principal source wiring and tests | High |

## Security-Sensitive Diff Scan Summary

Hard gate triggered: **yes**.

Helper categories:

- auth
- public ingress
- parser boundary
- secrets/redaction/privacy
- filesystem/subprocess boundary
- DoS/resource limits
- unsafe crypto/dependency/parser surface

Reviewed security surfaces:

- `auth::LivePrincipalSource` in `control-plane/crates/auth/src/lib.rs`.
- GraphQL HTTP auth middleware, WebSocket `connection_init`, query/mutation/subscription guards, and per-emission subscription rechecks.
- MCP HTTP and stdio initialize/per-request auth.
- Failed-serve `/graphql` and `/mcp` diagnostic surfaces, which currently use `auth::LivePrincipalSource` for bearer-principal resolution.
- P082 MCP/report readback redaction and principal-class gating.
- Workflow and MCP-registry YAML bounded file loaders.
- Projection rebuild background runtime lifecycle after the prior panic.

Security verdict: **Pass for P082 scope; adjacent failed-serve auth risk resolved**.

The R3 `SEC-001` blocker is resolved in the current tree: failed-serve exposes `build_failed_serve_router_with_principal_source`, resolves GraphQL and MCP diagnostic auth through `auth::LivePrincipalSource`, and the current `proposal-082|p082` alias includes the failed-serve revocation test.

No unresolved Critical or Major security findings remain for the P082 readback contract itself.

## Routed Specialist Findings

### OPS-001 - Gate-result metric proof is not emitted by the shell gate after scenario groups

Reviewer: `observability_rollout_reviewer`

Severity: **Minor**

Confidence: **High**

Related requirement: REQ-011.

Evidence type: code, tests-run, telemetry.

Evidence references:

- Proposal metric site: `docs/proposals/082-recovery-retry-state-machine-test-matrix.md:161`.
- Current gate ownership: `scripts/test-gate.sh` executes the static fixture/matrix checklist, focused Rust suites, and focused auth/revocation checks.
- Runtime readback no longer emits gate-result metric: `control-plane/crates/db/src/repos/p082_recovery_matrix.rs:927`.
- DB harness test exercises `record_p082_recovery_matrix_gate_result` for all scenario IDs.

Why it matters:

The proposal says `p082_recovery_matrix_gate_result_total{scenario_id,status}` is emitted by the proposal-082 gate harness after each scenario assertion group. The implementation correctly removes runtime readback emission, and a DB harness test exercises the metric for all scenario IDs, but the current shell gate does not emit the metric after each scenario assertion group.

Recommended action:

Move gate-result emission into the actual shell gate assertion flow, or keep the DB harness as the documented owner and update the proposal/reference language accordingly. Keep the regression that forbids runtime readback from emitting this metric.

Acceptance criteria:

- The documented owner and implemented owner for `p082_recovery_matrix_gate_result_total` match.
- A failing/interrupted gate cannot leave evidence that looks like a successful scenario proof.

## Readiness Checklist

| Item | Status | Evidence |
| --- | --- | --- |
| Canonical proposal gate on audited tree/HEAD | Historical pass; not re-run during docs sync | `./scripts/test-gate.sh proposal-082` was recorded by the original audit; current alias semantics include static fixture/matrix checks plus DB, engine, MCP, auth, MCP HTTP, and daemon failed-serve revocation checks |
| Core service flows validated | Pass for P082 active gate scope | Static fixture/matrix checks and DB, engine, MCP P082 suites are the core gate-owned flows |
| Security-sensitive diff pass | Pass for P082 scope | Helper triggered; manual pass completed for P082 readback; failed-serve live auth is covered by current source wiring and a focused revocation test |
| Specialist coverage hard gate | Pass | Required lenses selected or explicitly rejected with scope rationale |
| UI empty/loading/error/accessibility states | Out of scope | No P082 UI implementation |
| Privacy/redaction | Pass | Operator-only readbacks; non-operator null/empty; DB-side sanitization tests |
| Permissions/auth | Pass for P082 scope | Live auth tests across normal GraphQL and MCP were inspected historically; failed-serve now uses `auth::LivePrincipalSource`, and the current P082 alias includes the focused failed-serve revocation test |
| Localization/entitlements | Out of scope | No UI/entitlement changes in P082 scope |
| Full regression/canonical gate evidence | Historical pass; current docs sync did not rerun | Canonical P082 gate contents changed from the broad inventory claimed below |

## Verification Log

Commands run during this historical audit. Post-refinement docs sync found that the "Canonical gate contents observed" inventory below is stale for the current `proposal-082|p082` alias; see `docs/reference/test-gates.md` for current gate contents.

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py .../docs/proposals/082-recovery-retry-state-machine-test-matrix.md` -> selected R4 report path.
- `git rev-parse --show-toplevel`, `git rev-parse HEAD`, `git merge-base main HEAD`, `git branch --show-current` -> target metadata recorded above.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py .../082-recovery-retry-state-machine-test-matrix.md` -> no prior proposal-review artifacts.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/implementation_surface_fingerprint.py --root <worktree> --json` -> required lenses: API contract, Apple UI/UX, architecture, observability/rollout, performance, reliability, security; scoped selection recorded above.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/security_sensitive_diff.py --root <worktree> --json` -> triggered security hard gate.
- `./scripts/test-gate.sh proposal-082` -> **PASS**.
- Focused file inspection with `rg`, `git diff`, and line reads for P082 proposal commitments, failed-serve auth, metrics, MCP/GraphQL lanes, and tracked test status.

Historical gate contents recorded by this audit, now superseded for current gate-inventory purposes:

- Auth live-principal invalidity-window test: 1 passed.
- Workflow bounded YAML loader test: 1 passed.
- Daemon failed-serve live-auth revocation test: included in the current `proposal-082|p082` alias.
- GraphQL P046 same-principal invalidity-window subscription test: 1 passed.
- DB P082 matrix: 67 passed.
- Engine P082 matrix: 35 passed.
- Engine P082 integration: 2 passed.
- Cancel finalize live-session cleanup: 1 passed, with no background panic output.
- MCP P082 readback: 16 passed.
- GraphQL P082 live revocation: 7 passed.
- MCP HTTP P082 live auth: 2 passed.
- MCP stdio P082 live auth: 5 passed.
- DB P082 path-redaction sanitizer: 1 passed.

## Recommended Next Actions

1. Use `docs/reference/test-gates.md` as the current source of truth for `proposal-082|p082` gate contents.
2. Resolve the bounded proof gap by either re-enabling/migrating the retained static checklist or documenting that focused Rust tests are the sole active fixture proof.
3. Align the gate-result metric owner language with implementation: either emit from the shell gate after assertion groups or document the DB harness as the owner.
