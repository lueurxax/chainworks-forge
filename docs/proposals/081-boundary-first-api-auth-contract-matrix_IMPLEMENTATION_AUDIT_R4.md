# Proposal 081 Implementation Audit R4

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` |
| Proposal id / revision | `081` / `081-v6` |
| Proposal state | Active, status `revised_for_review_blocker_closure` |
| Audit report | `docs/proposals/081-boundary-first-api-auth-contract-matrix_IMPLEMENTATION_AUDIT_R4.md` |
| Audit timestamp | 2026-05-25T07:54:43+0300 |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-081-boundar-4dd7c886` |
| Branch | `cw/implement-proposal-081-boundar/4dd7c886` |
| Current SHA | `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa` |
| Compare base | `3a93e76332512fc07e8b7bec50882ee83d703c2f` (`git merge-base HEAD origin/main`) |
| Working tree status before report | Dirty: 10 modified implementation files, no audit report file yet |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer selection reuse | Not reused |
| Audit confidence | High for Rust/API/idempotency/readback evidence; medium for macOS runtime interaction and recovery behavior evidence |

## Implementation Target

The audited implementation target is the proposal branch at `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa`, including the uncommitted worktree modifications present at audit time. The worktree already contained implementation changes in `control-plane/crates/auth`, `control-plane/crates/db`, `control-plane/crates/engine`, `control-plane/crates/graphql-server`, and `control-plane/crates/mcp-server`; this audit did not modify those implementation files.

The implementation was compared against the active proposal contract in `docs/proposals/081-boundary-first-api-auth-contract-matrix.md`, not against prior implementation audits.

## Prior Review Reuse

No prior proposal-review artifacts were discovered by the review discovery helper. Existing `_IMPLEMENTATION_AUDIT_R1.md`, `_R2.md`, and `_R3.md` files were treated as historical context only and were not reused for reviewer selection.

Reuse status: Not reused.

## Selected Reviewer Lenses

| Reviewer | Reason selected |
| --- | --- |
| `rust_arch_reviewer` | P081 changes the Rust daemon boundary policy, idempotency, audit log, command journal, and startup contracts. |
| `rust_reliability_reviewer` | P081 has explicit reliability contracts for committed-unack recovery, SQLite contention, audit budget recovery, policy reload, and subscription gaps. |
| `api_contract_reviewer` | P081 defines GraphQL and MCP boundary behavior, redaction envelopes, schema readback, idempotency grammar, and error semantics. |
| `observability_rollout_reviewer` | P081 has rollout phases, canary fixtures, exact metric names, alerting, readback, and hold conditions. |
| `macos_ui_reviewer` | P081 includes Swift/macOS approval-only mutation UX, native alerts, redaction UI, keyboard command, and accessibility parity requirements. |

Rejected close alternatives: `rust_security_reviewer` was not selected as a separate lane because the reviewed security concerns are coupled to the Rust/API boundary and reliability contracts. `apple_arch_reviewer` was not selected because the Swift scope is operator UI behavior rather than app architecture. `product_reviewer` was not selected because P081 is an implementation contract with no new product decision surface beyond its already-defined rollout gates. `performance_reviewer` was not selected because latency and rollout performance checks are covered under observability/reliability for this audit.

## Proposal Contract Summary

P081 requires a boundary-first API/auth contract matrix that governs GraphQL, MCP, and Swift approval actionability through a shared immutable `BoundaryPolicy`. The policy must classify callers with trusted server-side `CallerClass` / `CallerContext` values, enforce route-level allow/deny/redact decisions from the checked-in matrix, emit durable audit rows, and keep the Swift app as an approval-only mutation surface.

The proposal's primary contract areas are:

- Boundary matrix documentation and JSON fixtures with required rows for GraphQL query/subscription/mutation, MCP initialize/list/call, approvals, and disabled debug break-glass behavior.
- Shared Rust auth boundary policy injection into GraphQL, MCP, and actionability paths.
- Principal table schema versions, caller class derivation, token handling, and fail-closed startup validation.
- Deterministic GraphQL and MCP responses for allowed, denied, redacted, unknown, and idempotent requests.
- Atomic state-changing MCP command journaling and idempotency replay/conflict handling.
- Audit log durability, checkpointing, retention, readback, and safe-mode behavior.
- Rollout canary validation, shadow coverage reporting, exact metrics, alerts, and hold conditions.
- macOS operator approval, redaction, native alert, keyboard, state restoration, and accessibility behavior.

Platform and product scope: Rust control-plane daemon, SQLite persistence, GraphQL/MCP northbound APIs, Swift/macOS operator shell. The product user flow remains operator approval of runs; the agent/operator boundary is the service contract under audit.

Primary flows reviewed:

1. Principal loads from auth storage, derives `CallerClass`, and routes through `BoundaryPolicy`.
2. GraphQL query/subscription/mutation requests produce allowed, denied, or redacted contract responses.
3. MCP initialize/list/call requests expose only allowed tools and enforce idempotency for state-changing calls.
4. Audit, alert, readback, and safe-mode data move through SQLite and runtime diagnostics.
5. Swift operator approvals preserve idempotency keys, decode redactions, expose actionability, and surface alerts/accessibility metadata.

## Fidelity And Divergence Inventory

Implemented fidelity:

- The canonical `proposal-081` gate passed in the audited worktree.
- A structured canary validator now checks canary rows, row ids, expected decisions, shadow report coverage, unknown fields, duplicate keys, and redaction proof requirements.
- GraphQL and MCP runtime readback now expose bounded `auditLogHealth` fields including writability, retention/cleanup state, payload budget/usage, integrity, and shadow coverage reference.
- MCP idempotency hashing now canonicalizes nested JSON and includes tool name, normalized args, caller class, principal id, token id, and boundary row id.
- Multiple direct MCP write units now claim pending idempotency inside the durable command transaction, including projection invalidation, storage maintenance, and effects mutation paths.
- Swift-focused P081 tests pass for redaction decoding, approval attempt storage, alert lifecycle/native-delivery metadata, hidden-window alert lifecycle naming, and accessibility metadata naming.

Divergences and residual gaps:

- Several required rollout metrics are declared and recordable but still lack production emission paths.
- Reliability coverage remains partly token/inventory based and does not yet prove the full runtime recovery contract for subscription gap detection or audit budget backpressure/safe-mode thresholds.
- Accessibility mode coverage appears source/model focused for Full Keyboard Access, Increase Contrast, and Reduce Motion rather than behavioral across rendered macOS states.
- Some release fixtures lag the richer runtime readback schema, even though direct runtime tests cover the new fields.

## Residual Scope And Follow-Up Ownership

| Item | Status | Owner / follow-up |
| --- | --- | --- |
| Production emission for all P081 exact metrics | Blocks P081 full conformance | No concrete follow-up found; should remain in P081 closure scope. |
| Subscription cursor/gap behavior and audit budget recovery thresholds | Blocks P081 full conformance | No concrete follow-up found; should remain in P081 closure scope or be explicitly split with proposal update. |
| Behavioral accessibility evidence for FKA, Increase Contrast, and Reduce Motion | Blocks strict P081 conformance | No concrete follow-up found; macOS/UI tests should be strengthened or the acceptance criterion narrowed. |
| `storage.reconcile_evidence_orphans` non-dry-run path remains fail-closed | Residual adjacent storage scope | Current implementation prevents mutation. It only blocks P081 if this disabled path is treated as a required successful state-changing MCP command. |
| Operator readback fixture schema lag | Non-blocking evidence gap | Update fixture/gate evidence so release artifacts match runtime schema. |

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 20 |
| Partially Implemented | 5 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed Requirement Audit

| Req | Proposal requirement | Status | Evidence and notes |
| --- | --- | --- | --- |
| REQ-001 | Boundary contract matrix exists as docs/JSON and covers all required surfaces. | Implemented | Matrix rows and fixture-backed gate remain present; canary/report validation now confirms the expected row set. |
| REQ-002 | Matrix/action schema grammar is executable and validated. | Implemented | `scripts/validate-p081-canaries.py` rejects duplicate keys, unknown fields, malformed canaries, missing/extra rows, and report drift. |
| REQ-003 | Required GraphQL/MCP/approval/break-glass rows are represented. | Implemented | Validator and shadow coverage report cover the 11 required rows, including redacted observer read and disabled debug break-glass. |
| REQ-004 | Embedded fallback fixture and startup fail-safe behavior are wired. | Implemented | Gate and runtime readback show embedded fixture digest/policy injection and safe-mode fields. |
| REQ-005 | Audit log/checkpoint/readback/retention/fail-closed persistence exists. | Implemented | `AuditLogHealthSnapshot` now includes integrity, writability, retention, cleanup, and payload fields; GraphQL/MCP readback tests cover bounded health output. |
| REQ-006 | Principal table v1/v2/v3 compatibility and strict schema handling. | Implemented | Focused auth tests passed for legacy normalization and v3 unknown schema rejection; test helpers now enforce secure temp principal file permissions. |
| REQ-007 | `CallerClass` / `CallerContext` are derived server-side and not caller supplied. | Implemented | Boundary request paths derive class from loaded principals and reject unsupported principal schema/unknown caller fields in auth tests. |
| REQ-008 | Shared immutable `BoundaryPolicy` gates GraphQL, MCP, and approval actionability. | Implemented | Runtime readback exposes policy injection; gate includes policy/row validation and surface tests. |
| REQ-009 | GraphQL query/subscription/mutation contract is deterministic and redacts predictably. | Implemented | P081 gate passed GraphQL runtime/readback/redaction tests; Swift decoder tests preserve typed redaction extensions. |
| REQ-010 | MCP initialize/list/call contract returns correct allow/deny/unknown behavior. | Implemented | MCP boundary gate passed; `is_state_changing_tool`, read-only classification, and idempotency precheck cover the call surface. |
| REQ-011 | MCP idempotency includes canonical hash dimensions and committed-unack handling. | Implemented | Focused tests passed for nested canonical hash sorting and storage/effects write-unit idempotency claims. |
| REQ-012 | Approval idempotency is scoped and preserved through retry/success. | Implemented | Swift tests passed for one retry key until success and approval-action scoping. |
| REQ-013 | State-changing MCP commands require idempotency keys; read-only calls reject them. | Implemented | MCP server classifies read-only vs state-changing tools, enforces key precheck, and denies missing/conflicting keys. |
| REQ-014 | Denied calls create no side effects across all surfaces. | Partially Implemented | Deny/audit behavior is covered by gates, but the current evidence is not a complete cross-surface side-effect sweep for every denied GraphQL/MCP/approval path and command-journal mutation. |
| REQ-015 | Boundary guardrails prevent route drift and caller-supplied boundary spoofing. | Implemented | Gate includes matrix/schema/readback validation and source checks for expected boundary contract tokens. |
| REQ-016 | Security hardening covers token grammar, expiration, constant-time digest compare, break-glass disabled, and audit DoS controls. | Partially Implemented | Auth schema and principal permission evidence improved, but audit DoS/rate-limit/budget threshold behavior is not fully implemented/proven and required rate-limit metric emission is absent. |
| REQ-017 | Canary rollout and shadow coverage are schema-equivalent and machine validated. | Implemented | Structured validator checks canary/report agreement, required test ids, redaction proof, disagreements, and row coverage; proposal gate invokes it. |
| REQ-018 | Reliability runtime covers SIGTERM, policy reload, SQLite contention, committed-unack, subscription gaps, audit budget recovery, and tamper-safe startup. | Partially Implemented | Some evidence exists for committed-unack, SQLite contention, policy reload, and bounded readback. Subscription cursor/gap semantics and audit budget warning/safe-mode/half-open recovery remain unproven or absent. |
| REQ-019 | Operator alert lifecycle and native delivery behavior are covered. | Implemented | Swift P081 tests passed for operator alert lifecycle, native attention lifecycle, silencing, and hidden-window alert lifecycle naming. |
| REQ-020 | Runtime readback exposes P081 boundary/audit health without leaking raw rows. | Implemented | GraphQL and MCP tests assert bounded `auditLogHealth` fields including writability, retention/cleanup, payload budget/usage, integrity, and shadow report reference. |
| REQ-021 | Swift approval-only mutation surface preserves idempotency and decodes typed redaction. | Implemented | Swift P081 tests passed for approval action attempts and typed redaction extension handling. |
| REQ-022 | Accessibility parity covers Full Keyboard Access, Increase Contrast, Reduce Motion, redactions, and disabled controls. | Partially Implemented | Swift tests now cover accessibility metadata naming, but evidence does not demonstrate behavioral parity across the listed macOS accessibility modes. |
| REQ-023 | Baseline docs distinguish implemented vs proposed behavior and keep P081 active until closed. | Implemented | Proposal remains active; gate validates required reference/evidence tokens. |
| REQ-024 | Runtime request paths use injected policy, not fixture/docs file reads. | Implemented | Runtime readback and gate evidence support injected policy/digest behavior; no request-path fixture read evidence was found in audited changes. |
| REQ-025 | Exact rollout metrics are emitted or observable with expected names. | Partially Implemented | `P081_REQUIRED_METRICS` declares the required names and recordability tests pass, but several required metrics have no production call sites. |

## Reviewer Scorecard

| Lens | Score | Notes |
| --- | --- | --- |
| Rust architecture | Mostly aligned | Shared policy/readback/idempotency shape is coherent, and direct write-unit atomicity improved. |
| Rust reliability | Partial | Recovery/token evidence exists, but subscription gap and audit budget recovery are not yet executable end-to-end behavior. |
| API contract | Mostly aligned | GraphQL/MCP contract and idempotency behavior are strong; denied side-effect sweep and fixture drift need tightening. |
| Observability/rollout | Partial | Structured canary validation is a significant closure, but metric production emission remains incomplete. |
| macOS UI | Partial | Approval, redaction, and alert model tests pass; accessibility mode behavior still needs stronger proof. |
| Implementation readiness | Not Ready | Canonical proposal gate passes, but P081 still has partial requirements that block full proposal closeout. |

## Routed Findings

### OPS-001: Required P081 metrics are not fully emitted from production paths

Severity: Major  
Confidence: High  
Related requirements: REQ-025, REQ-016

The implementation now declares the exact P081 metric names and has a recordability test, which closes the previous "metric name absent" class of defect. However, repository search found no production emission path for several required metrics, including `auth_ambiguous_caller_warn_total`, `boundary_no_op_label_total`, `audit_log_rate_limited_total`, `operator_alert_native_delivery_total`, `approval_idempotency_duplicate_total`, `boundary_commit_transaction_latency_ms`, and `operator_alert_clear_latency_ms`.

This matters because P081's rollout/hold conditions depend on observing these exact signals during shadow, canary, and enforce phases. A metric that is declared but never emitted cannot satisfy the rollout contract.

Recommended action: wire production increments/observations for every required metric, including duplicate approval handling and transaction latency, or update P081 to explicitly remove or reclassify metrics that are no longer intended to be produced.

### REL-001: Reliability runtime contract remains partially token-based and incomplete

Severity: Major  
Confidence: High  
Related requirements: REQ-018, REQ-016

The proposal requires executable reliability behavior for audit budget recovery, policy reload, SQLite contention, subscription cursor/gap detection, committed-unack recovery, and tamper/safe-mode startup behavior. The current gate still includes an inventory-style check for reliability evidence, and direct searches did not find implemented GraphQL subscription fields/behavior for `sequence_cursor`, `projection_generation`, or `gap_detected`. Audit budget readback and cleanup exist, but the proposed warning at 80%, safe mode at 95%, cleanup cadence, and half-open write recovery were not proven as runtime scenarios.

Recommended action: add scenario tests that exercise subscription gap reconnect behavior and audit budget threshold transitions, then replace token inventory proof with those executable assertions.

### UI-001: Accessibility acceptance is not yet behaviorally proven

Severity: Major  
Confidence: Medium  
Related requirements: REQ-022

The Swift P081 test suite now passes accessibility-oriented tests, including redaction metadata and accessibility parity naming. The current evidence still appears to prove presence/naming/model metadata rather than actual behavior across Full Keyboard Access, Increase Contrast, and Reduce Motion states with disabled approval controls and redaction surfaces.

Recommended action: add focused Swift tests that exercise the relevant view/service state under those accessibility modes, or narrow the proposal acceptance criterion if metadata-only coverage is the intended contract.

### API-001: Operator readback fixtures lag the richer runtime schema

Severity: Minor  
Confidence: Medium  
Related requirements: REQ-020, REQ-017

Runtime GraphQL and MCP tests now assert richer `auditLogHealth` fields, including writability, retention/cleanup, payload budget/usage, and shadow coverage reference. At least one rollout readback fixture still appears narrower than the runtime schema. This is less severe because direct runtime tests cover the new shape, but fixture drift weakens the release evidence story for operators.

Recommended action: refresh operator readback fixtures and gate assertions so fixture evidence matches the runtime readback contract.

## Readiness Checklist

| Check | Result |
| --- | --- |
| Same-tree canonical proposal gate | Passed: `./scripts/test-gate.sh proposal-081` |
| Full repository regression gate | Not run; not required for a Not Ready verdict |
| Focused MCP idempotency tests | Passed for storage clear-backlog, nested canonical hash, and effects conflict write-unit claim |
| Focused auth compatibility/security tests | Passed for legacy default operator normalization and v3 unknown schema rejection |
| GraphQL/MCP runtime readback | Passed in proposal gate and inspected in source/tests |
| Structured canary validation | Passed in proposal gate and inspected in script |
| Rollout metrics production emission | Partial |
| Reliability runtime scenarios | Partial |
| macOS native alert/accessibility evidence | Partial |
| Prior review reuse | Not reused |

## Verification Log

| Command / inspection | Result |
| --- | --- |
| `git rev-parse HEAD` | `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa` |
| `git merge-base HEAD origin/main` | `3a93e76332512fc07e8b7bec50882ee83d703c2f` |
| `git status --short --branch` | Dirty worktree with 10 modified implementation files before this audit report. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` | No prior proposal-review artifacts discovered. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` | Selected this R4 audit report path. |
| `./scripts/test-gate.sh proposal-081` | Passed. Final output: `Proposal 081 boundary-first API/auth gate passed`. |
| Swift P081 tests inside gate | Passed 10 tests, including redaction decoding, approval attempt storage, operator alert lifecycle/native delivery, and accessibility metadata naming. |
| Focused MCP/auth rerun | Passed after correcting an initial `cargo test` filter syntax mistake. |
| Metrics production-hook search | Found declarations/recordability and several emissions, but no production hooks for multiple required P081 metrics listed in OPS-001. |
| Reliability contract search | No direct P081 implementation evidence found for GraphQL subscription `sequence_cursor`, `projection_generation`, or `gap_detected`; audit budget thresholds not proven. |

Focused rerun command that passed:

```bash
cd control-plane && \
RUST_MIN_STACK=8388608 cargo test -p mcp-server p081_storage_clear_backlog_claims_idempotency_in_write_unit -- --nocapture && \
RUST_MIN_STACK=8388608 cargo test -p mcp-server p081_canonical_request_hash_sorts_nested_argument_objects -- --nocapture && \
RUST_MIN_STACK=8388608 cargo test -p mcp-server --test proposal_078_effects_tools proposal_081_effects_mark_conflict_claims_mcp_idempotency_in_write_unit -- --nocapture && \
cargo test -p auth legacy_default_operator_file_is_normalized_to_p072_ui_policy -- --nocapture && \
cargo test -p auth v3_principal_table_rejects_unknown_schema_version -- --nocapture
```

The first focused test attempt used multiple `cargo test` filters in one invocation and failed before running tests with `error: unexpected argument 'p081_canonical_request_hash_sorts_nested_argument_objects' found`; the corrected command above passed.

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

The current worktree substantially improves P081 and closes major prior gaps around structured canary validation, runtime readback, and MCP direct write-unit idempotency. The canonical `proposal-081` gate now passes. However, the proposal cannot be closed as fully implemented because metrics emission, reliability runtime behavior, and accessibility-mode proof remain partial against the active P081 contract.

Recommended next actions:

1. Wire production emission for every exact P081 rollout metric or revise the proposal contract before closeout.
2. Implement and test subscription gap detection plus audit budget warning/safe-mode/recovery scenarios.
3. Strengthen macOS accessibility-mode behavior tests for Full Keyboard Access, Increase Contrast, Reduce Motion, disabled actions, and redaction surfaces.
4. Refresh release/readback fixtures so they match the richer runtime schema already covered by direct tests.
