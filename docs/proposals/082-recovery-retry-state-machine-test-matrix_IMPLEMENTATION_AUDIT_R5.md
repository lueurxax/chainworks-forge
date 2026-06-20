# Proposal 082 Implementation Audit R5

## Metadata

- Proposal: `docs/proposals/082-recovery-retry-state-machine-test-matrix.md`
- Proposal hash: `sha256:16ad41908c6f9e8640539d923141390651375e397cb1b872f436ce3b2baec43e`
- Audit timestamp: `2026-06-20T05:36:15Z`
- Audit base: `4c3dce2c5f70cd7dc540887979d399f7d354c59d`
- Audit report: `docs/proposals/082-recovery-retry-state-machine-test-matrix_IMPLEMENTATION_AUDIT_R5.md`
- Verdict: **Not Ready**
- Conformance summary: **Substantially implemented in inspected P082 surfaces, but not sign-off eligible in the current tree**
- Readiness reason: `./scripts/test-gate.sh proposal-082` fails in the final daemon phase because the current worktree contains unresolved merge conflicts that break Rust compilation.
- Prior-review reuse: **None.** No prior `proposal-review` artifacts were discovered for P082. Existing `IMPLEMENTATION_AUDIT_R1` through `R4` were intentionally not used for reviewer selection.

## Implementation Target And Base

The audit targeted the current local repository state for P082 closeout. During verification the tree was no longer a clean, merge-ready tree:

- `git status --short` reported unmerged files.
- `git ls-files -u` reported unresolved conflicts in:
  - `control-plane/Cargo.toml`
  - `control-plane/crates/auth/src/lib.rs`
  - `control-plane/crates/db/src/repos/mod.rs`
  - `control-plane/crates/domain/src/lib.rs`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/mcp-server/src/tools/reports.rs`
  - `docs/ROADMAP.md`
  - `docs/reference/rust-control-plane.md`
  - `docs/reference/test-gates.md`

This blocks a release-ready verdict even though the inspected P082 implementation surfaces show broad coverage.

## Proposal State And Contract

P082 requires a canonical recovery/retry state-machine test matrix and durable proof gate covering restart, retry, cancellation, stale startup, late output, side-effect, approval, session, and mediation boundaries.

Key contract points audited:

- Reference documentation must define P082-R01 through P082-R17, recovery reason codes, source owners, readback fields, crash/replay proof, observability thresholds, and live-principal enforcement.
- Gate aliases `proposal-082` and `p082` must exist and prove the implemented behavior.
- Rejected commands must use a typed `p082_rejected_command_error_v1` envelope in `command_journal.error`; `command_journal.payload_json` must not be mutated.
- Parsing must remain backward-compatible for legacy/untyped errors.
- GraphQL readback is optional and advisory only.
- SwiftUI/app-facing consumption is out of scope.
- Operator-only report/readback surfaces must be enforced through live/reloadable principal checks across MCP HTTP, MCP stdio, failed-serve diagnostics, and existing GraphQL HTTP/WebSocket auth boundaries.
- No migration is required for P082.

## Platform And Scope

- Primary codebase: Rust control-plane parity daemon.
- Swift app: explicitly out of scope for P082 behavior consumption.
- Northbound surfaces: MCP `runs.get`, `reports.get`, `report://`, run report artifact, release receipt, and adjacent auth checks.
- Storage surfaces: command journal error envelopes, startup repair notes, work item payloads, stage recovery snapshots, cancellation settlements, mediation/session diagnostics, and readback projection accessors.

## Prior Review Reuse

No prior proposal-review artifacts were found and no reviewer output was reused.

Implementation-audit reports R1-R4 were treated as historical audit records only. They were not used to select reviewers or satisfy specialist coverage.

## Selected Reviewers

| Reviewer | Why selected | Coverage |
| --- | --- | --- |
| `rust_arch_reviewer` | P082 adds shared storage/readback boundaries and ownership rules. | Domain contracts, DB projection ownership, no-migration posture, source owner allowlist. |
| `rust_reliability_reviewer` | P082 is primarily a retry/recovery/cancellation state-machine proposal. | R01-R17 behavior, idempotent replay, validation-before-mutation, provider cleanup, late-output quarantine. |
| `api_contract_reviewer` | P082 exposes operator readbacks through MCP/report lanes. | Singular/plural lane shape, release receipt/run report/report resource parity, optional GraphQL omission. |
| `rust_security_reviewer` | Operator-only diagnostics, live bearer principal reload, redaction, and bounded parsing are security-sensitive. | MCP HTTP/stdio, GraphQL auth layer, failed-serve diagnostics, secret/path redaction, typed envelope parsing. |
| `observability_rollout_reviewer` | P082 requires rollout fixtures, metrics, gate aliases, and rollback clarity. | Gate scripts, reference gate docs, positive/negative fixtures, metric names/labels, dependency-audit risk note. |

## Rejected Alternatives

- macOS UI/UX reviewers: rejected because SwiftUI consumption is out of P082 scope and deferred to a future proposal.
- Dedicated GraphQL API reviewer: rejected because P082 GraphQL readback fields are optional and not implemented; auth adjacency was still covered by API/security review.
- Performance reviewer: rejected because P082 adds bounded diagnostic readback, not a throughput or latency contract. Bounds were still checked under architecture/security review.
- Product reviewer: rejected because the proposal is an operator diagnostics and proof-gate contract, not a user workflow change.

## Track 1 Requirement Conformance

| Req | Contract | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Reference matrix documents all P082 rows, reason codes, schemas, owners, lane placement, thresholds, live-principal boundary, and gate ownership. | Implemented | `docs/reference/recovery-retry-state-machine-test-matrix.md` defines P082-R01 through R17, reason codes, schemas, lane placement, observability thresholds, Swift boundary, and gate ownership. |
| REQ-002 | Gate aliases `proposal-082` and `p082` exist and include static and focused runtime checks. | Implemented but current-tree gate failed | `scripts/test-gate.sh` contains the alias, static checklist, required scenario checks, and focused cargo suites. Final gate failed only when the current unmerged tree reached the daemon compile phase. |
| REQ-003 | No migration; rejected commands write typed envelopes to `command_journal.error` and do not mutate `payload_json`. | Implemented in inspected surfaces | No P082 migration found. `recovery_matrix.rs`, DB readback repo, and command handler use typed error envelopes and safe legacy fallback. |
| REQ-004 | Shared readback accessor with source owner allowlist and bounded projection. | Implemented | `control-plane/crates/db/src/repos/p082_recovery_matrix.rs` implements source-specific readers, allowlisted projection, redaction, row/string limits, plural/singular accessors, and lane metrics. |
| REQ-005 | DB/engine behavior covers restart, retry rejection, stale startup, side effects, approval, mediation, cancellation, crash/replay, and late output. | Implemented in inspected surfaces | DB, engine unit, and engine integration P082 suites passed before the later daemon compile failure. |
| REQ-006 | MCP/report/run-report/release receipt lanes expose operator readback parity and hide diagnostics from non-operators. | Implemented in inspected surfaces | MCP `runs.get`, `reports.get`, report resource, run report artifact, and release receipt tests passed before the later daemon compile failure. |
| REQ-007 | Live/reloadable principal enforcement across MCP HTTP/stdio, failed-serve diagnostics, and GraphQL auth boundaries. | Blocked for current-tree proof | MCP HTTP and auth live-principal tests passed. The daemon failed-serve live-principal proof could not complete because the current tree does not compile. |
| REQ-008 | Positive and negative rollout fixtures exist and validate all required lanes/failure cases. | Implemented | Positive fixture contains all required lanes, reason codes, and scenario IDs. Sixteen negative fixtures exist with expected P082 failure codes. |
| REQ-009 | Metrics and observability thresholds are documented and emitted. | Implemented in inspected surfaces | Required metric names are present in DB metrics and the gate emits P082 scenario result counters. Reference docs define warning/critical thresholds. |
| REQ-010 | Swift/macOS consumption remains out of scope; optional GraphQL readback omission is tolerated. | Implemented | No Swift P082 consumption was found. GraphQL P082 readback fields were not implemented, consistent with the optional diagnostic-only contract. |
| REQ-011 | Security/redaction/bounded parsing prevents sensitive diagnostic leakage. | Implemented in inspected surfaces | DB projection redacts paths/secrets, strips non-allowlisted keys, bounds row/string size, and non-operator lanes receive null/empty diagnostics. |
| REQ-012 | Rollout/rollback and dependency-risk documentation exist. | Implemented with residual risk | Reference docs describe gate ownership, evidence expectations, rollback posture, and the pinned `serde_yaml -> unsafe-libyaml` dependency risk. Dependency audit tooling was unavailable locally. |

## Flow Coverage

| Flow | Status | Notes |
| --- | --- | --- |
| Startup requeue and exhausted startup repair | Covered | R01/R15/R16 code paths and tests were present; DB/engine suites passed before current-tree daemon compile failure. |
| Invalid retry and retry identifier mismatch | Covered | R02/R08 command-handler rejection paths write typed error envelopes before mutation. |
| Late output and cancel-then-late output | Covered | R03/R17 quarantine readbacks and settlement data are implemented and tested. |
| Side-effect retry block and cancellation with side effects | Covered | R07/R13 fail-closed/held behavior is implemented and tested. |
| Approval and mediation restart/cancel boundaries | Covered | R09/R10/R12 paths are represented in readback logic and tests. |
| Provider cleanup proof | Covered | Focused provider cleanup test passed before the later daemon compile failure. |
| Live failed-serve auth proof | Blocked | Could not complete because daemon compilation failed on unresolved conflicts. |

## Fidelity And Divergence

Implemented fidelity:

- P082 uses existing storage surfaces rather than adding a migration.
- Readback contracts are JSON-envelope based, bounded, and tolerant of legacy/untyped data.
- GraphQL P082 readback is omitted, which is compliant because the proposal makes it optional/advisory.
- Swift app consumption is not implemented, which is compliant because the proposal excludes it.
- Non-operator surfaces are fail-closed for P082 diagnostics.

Divergence:

- The current tree is not in a merge-resolved, compilable state. This is a readiness blocker, not a documented P082 contract alternative.

## Residual Scope

No P082 functional follow-up is required based on the inspected implementation surfaces.

Current residual work before closeout:

1. Resolve the unmerged conflict state in the current tree.
2. Re-run `./scripts/test-gate.sh proposal-082` on the resolved tree.
3. Re-check dependency audit tooling or keep the documented pinned-risk acceptance explicit in closeout material.

## Coverage Matrix

| Scenario | Status | Audit note |
| --- | --- | --- |
| P082-R01 restart mid command/startup requeue once | Covered | DB and engine suites emitted passing scenario metrics before final gate failure. |
| P082-R02 reject non-retryable stage retry | Covered | Command-handler rejection envelope and tests present. |
| P082-R03 late output after supersede | Covered | Quarantine settlement and readback path present. |
| P082-R04 duplicate session/startup claim | Covered | Session duplicate readback and tests present. |
| P082-R05 stale ACP startup | Covered | Startup work-item readback and Xcode grace logic present. |
| P082-R06 stale scheduler ownership | Covered | Held-state side-effect evidence gating present. |
| P082-R07 release side-effect drift | Covered | Retry preflight blocks side-effect drift. |
| P082-R08 retry identifier mismatch | Covered | Retry guidance envelope present. |
| P082-R09 pending human approval restart | Covered | Pending approval readback derived without synthetic decisions. |
| P082-R10 duplicate mediation attempt | Covered | Duplicate mediation readback path present. |
| P082-R11 cancel interleaved active stage/retry | Covered | Cancellation settlement readback path present. |
| P082-R12 cancel pending approval | Covered | Approval-preserving cancellation path present. |
| P082-R13 cancel unresolved side effects | Covered | Side-effect hold path present. |
| P082-R14 cancel startup repair | Covered | Engine integration test passed before final gate failure. |
| P082-R15 daemon crash during repair | Covered | Startup repair replay readback path present. |
| P082-R16 startup requeue exhausted held state | Covered | Exhausted held-state readback path present. |
| P082-R17 cancel then late provider output | Covered | Late-output quarantine/cancel settlement path present. |

## Security Scan

Security-sensitive review was required and performed because P082 touches:

- bearer principal reload and operator-only authorization,
- MCP HTTP/stdio/report resources,
- GraphQL auth adjacency,
- diagnostic JSON parsing/projection,
- path and secret redaction,
- command rejection envelopes.

Findings:

- No P082-specific unresolved security finding was identified in the inspected implementation surfaces.
- The current-tree conflict state prevents final security sign-off because it blocks the full proposal gate.
- `cargo-audit` and `cargo-deny` were unavailable locally, so dependency scanning was not run. The reference documentation explicitly records the pinned `serde_yaml -> unsafe-libyaml` dependency risk.

## Routed Findings

### BLOCKER-001: Current tree has unresolved merge conflicts and fails the canonical P082 gate

- Severity: Blocker
- Files: `control-plane/crates/engine/src/executor.rs` plus other unmerged files listed above.
- Evidence: `./scripts/test-gate.sh proposal-082` exited `101` in the final daemon phase.
- Failure:
  - `cargo test -p daemon sec_high_001_failed_serve_observes_live_principal_revocation -- --nocapture`
  - `error: mismatched closing delimiter: '}'`
  - `control-plane/crates/engine/src/executor.rs:10338:101`
  - `error: this file contains an unclosed delimiter`
- Impact: The implementation cannot be marked Ready because the same-tree canonical gate does not pass.
- Required action: Resolve the merge conflicts, preserve P082 behavior, and rerun `./scripts/test-gate.sh proposal-082`.

### RISK-001: Dependency audit tooling unavailable

- Severity: Risk
- Evidence: `cargo-audit` and `cargo-deny` were not installed in the local environment.
- Impact: The dependency-risk check could not be independently refreshed during this audit.
- Mitigation: The P082 reference doc records the pinned `serde_yaml -> unsafe-libyaml` risk. Re-run dependency audit tooling before final release closeout if available.

## Readiness Checklist

| Check | Status |
| --- | --- |
| Proposal requirements mapped | Pass |
| Prior proposal-review artifacts discovered/reused | Not applicable; none found |
| Specialist coverage selected | Pass |
| Security-sensitive hard gate applied | Pass |
| P082 static gate checklist | Pass |
| DB P082 suite | Pass |
| Engine P082 unit suite | Pass |
| Engine P082 integration filter | Pass |
| Provider cleanup proof | Pass |
| Auth live-principal unit proof | Pass |
| MCP P082/readback proof | Pass |
| MCP HTTP live-principal proof | Pass |
| Daemon failed-serve live-principal proof | **Fail: current tree does not compile** |
| Full `./scripts/test-gate.sh proposal-082` | **Fail** |
| `git diff --check` | Pass, but does not clear unmerged conflict state |
| `git ls-files -u` | **Fail: unresolved conflicts present** |

## Verification Log

| Command | Result |
| --- | --- |
| `./scripts/test-gate.sh proposal-082` | Failed with exit `101` during daemon live-principal test compilation. |
| P082 static checklist inside gate | Passed: `P082 gate: all static checks passed`. |
| `cargo test -p db --test proposal_082_recovery_retry_matrix -- --nocapture` | Passed: 87 tests. |
| `cargo test -p engine --test proposal_082_recovery_retry_matrix -- --nocapture` | Passed: 36 tests. |
| `cargo test -p engine --test integration p082_ -- --nocapture` | Passed: 11 tests. |
| `cargo test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --nocapture` | Passed: 1 test. |
| `cargo test -p auth live_principal_source_revalidates_revoked_disabled_and_rescoped_credentials -- --nocapture` | Passed: 1 lib test; bootstrap test target had no matching tests. |
| `cargo test -p mcp-server p082_ -- --nocapture` | Passed: 18 lib tests plus 17 `proposal_082_recovery_readback` tests. |
| `cargo test -p mcp-server sec_high_001_mcp_http_observes_live_principal_revocation -- --nocapture` | Passed: 1 test. |
| `cargo test -p daemon sec_high_001_failed_serve_observes_live_principal_revocation -- --nocapture` | Failed to compile `engine` because of unresolved conflict markers/delimiter mismatch in `executor.rs`. |
| `git diff --check` | Passed. |
| `git ls-files -u` | Failed readiness: unresolved conflicts present. |

## Scorecard

| Dimension | Score | Notes |
| --- | --- | --- |
| Proposal fidelity | 4/5 | Inspected P082 behavior closely matches the proposal, but current-tree conflicts prevent final proof. |
| Runtime reliability | 4/5 | DB/engine recovery scenarios passed before the final compile blocker. |
| API/readback contract | 4/5 | MCP/report lanes passed; daemon failed-serve proof blocked by compile failure. |
| Security | 3/5 | Operator-only and redaction design looked correct; final sign-off blocked and dependency audit tooling unavailable. |
| Operability/observability | 4/5 | Gate, metrics, fixtures, and docs are present; full gate did not pass in this tree. |
| Release readiness | 1/5 | Unresolved conflicts and failing canonical gate block readiness. |

## Final Verdict And Actions

P082 is **Not Ready** for implementation closeout in the current tree.

The inspected implementation surfaces substantially implement the proposal contract, including the matrix, readback schemas, MCP/report lanes, rejection envelopes, fixture set, metrics, and most runtime proofs. However, the audit cannot issue a Ready verdict because the exact tree under review is unmerged and the canonical `proposal-082` gate fails during daemon compilation.

Required before re-audit or closeout:

1. Resolve the unmerged files without regressing the P082 surfaces.
2. Re-run `./scripts/test-gate.sh proposal-082` and capture a clean pass.
3. Re-run or explicitly accept dependency audit risk if `cargo-audit`/`cargo-deny` remain unavailable.

