# Proposal 046 Implementation Audit R1

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/046-session-management-graphql-api.md` |
| Proposal revision | `p046-session-graphql-read-subscription-r4` |
| Proposal state | Active for audit; source status is `draft_for_review` |
| Audit timestamp | 2026-05-25T21:41:52+03:00 |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-046-session-b4f4b41c` |
| Audited implementation target | Current worktree, including unstaged changes |
| Audited HEAD | `95060bcfbbd10bc72935f76233f9768bbd7539aa` |
| Branch | `cw/implement-proposal-046-session/b4f4b41c` |
| Compare base | `origin/main...HEAD` for committed implementation diff; unstaged worktree edits also included |
| Working tree status | Dirty: `control-plane/crates/db/src/p046_retry.rs`, `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/graphql-server/src/types/session.rs`, proposal doc, `scripts/test-gate.sh` |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready for release/closeout |
| Audit confidence | High for server/Swift guardrail behavior covered by the proposal gate; Medium for dogfood and cursor edge semantics |

## Prior Proposal-Review Reuse

Reviewer selection reuse: Not reused.

The discovery helper found no prior proposal-review artifacts for this proposal. Reviewer routing was selected from the proposal scope and implementation evidence.

Selected reviewers:

- `rust_arch_reviewer` — Rust db/graphql-server/domain ownership boundaries and no-write/read-only separation.
- `rust_reliability_reviewer` — retry budgets, deadlines, subscription backpressure, shutdown, and fail-closed behavior.
- `api_contract_reviewer` — GraphQL schema, pagination/cursors, disabled schema, redaction, and client/server compatibility.
- `observability_rollout_reviewer` — feature flag, rollout fixture, metrics, dogfood evidence, and gate coverage.
- `apple_arch_reviewer` — SwiftUI selected-run ownership, MainActor state, disabled-schema gating, no SwiftData/AppKit ownership.

Rejected close alternatives:

- `macos_ui_reviewer`: visible UI is compact readback and guidance only; the main risk is ownership/gating rather than platform visual design.
- `apple_ux_reviewer`: user-facing reset guidance is covered by explicit requirements and Apple architecture checks; no separate journey redesign was needed.
- `rust_security_reviewer`: auth and sensitive data redaction are material, but they are directly covered by API contract and Rust architecture lenses under the five-reviewer cap.
- `product_reviewer`: dogfood and metric evidence are covered by observability/rollout; there was no product experiment or value tradeoff beyond proposal acceptance.

## Proposal Contract Summary

P046 adds GraphQL read/subscription observability for run-scoped session lineage, generations, events, KPIs, health, and live status while explicitly preserving MCP-only ownership of session reset/control. It requires operator-read authorization scoped to the owning run, bounded pagination and resolver deadlines, default-deny redaction, derived non-secret replacements for sensitive generation fields, a feature flag with disabled-schema compatibility, bounded subscription backpressure, deterministic SQLite retry behavior, SwiftUI transient MainActor ownership, rollout fixtures, metrics, and a `proposal-046` proving gate.

Platform/product scope:

- Apple: macOS SwiftUI, selected-run detail readback only.
- Backend/service: Rust GraphQL API, SQLite read helpers, subscription stream, metrics, feature flag, rollout evidence.
- Cross-stack: SwiftUI GraphQL client capability gating and subscription re-query behavior.

Primary implementation flows audited:

1. Operator queries run-level session observability through `sessionLineages`, `sessionKpiSummary`, and `sessionHealth`.
2. Operator/API client drills into lineage/generation/event readback through ID-based resolvers with parent-run authorization.
3. Client subscribes to `sessionStatusChanged(runId)`, receives only run-scoped events, and re-queries on resync/close/lag.
4. GraphQL returns health/KPI projections, redacted event details, and derived sensitive references without adding control mutations.
5. SwiftUI selected-run surface discovers capability first, owns transient MainActor state, and shows readback/generic MCP reset guidance without SwiftData persistence.

## Fidelity Inventory

Matches:

- P046 query fields and `sessionStatusChanged(runId)` are implemented behind `CHAINWORKS_GRAPHQL_SESSION_OBSERVABILITY`.
- No GraphQL reset/control mutation was found.
- Query resolvers enforce operator-read and parent-run lookup before returning data.
- Subscription setup filters by run, rechecks live principal credentials per emission, sends resync on lag/shutdown paths, and uses bounded per-subscriber queues.
- Event redaction is default-deny with typed safe details and closed vocabularies.
- Raw provider/session/workdir fields are replaced by derived references or redacted display values.
- SQLite retry is db-owned and bounded to three attempts with 50/150 ms backoff, jitter, sleep cap, and deadline headroom.
- SwiftUI adds a `P046SessionObservabilityModel` as transient MainActor state and gates full documents behind a capability probe.
- Reference docs, negative fixtures, rollout readback, and `proposal-046` gate wiring are present.

Divergences:

- Structurally valid base64 cursors are accepted when their timestamp tuple component is not an RFC3339 timestamp. This leaves part of the `malformed/wrong-type/expired cursor -> invalid cursor` contract unproven and likely unenforced.
- `session_graphql_sqlite_retry_total` records the `exhausted` outcome on each transient retry attempt, even if a later retry succeeds. The metric outcome vocabulary is therefore not semantically reliable.
- Dogfood/run-report/release receipt evidence remains waived or pending; the rollout fixture itself records Phase 3 handoff still pending.

Ambiguities / Evidence Gaps:

- The current auth model appears to express "resource-scoped operator-read" as operator class plus run existence/ownership lookup. No per-run ACL model was present to test finer-grained authorization.
- I did not run remote UI tests or capture screenshots. SwiftUI evidence is from targeted unit/guardrail tests and code inspection, which is sufficient for the proposal's ownership/gating requirements but not visual QA.

## Residual Scope / Follow-up Ownership

| Item | Owner | Blocking? | Status |
|---|---|---:|---|
| Validate all cursor tuple components, including RFC3339 timestamps and wrong-type/expired cursor cases, and add negative tests | P046 implementation | Yes for full conformance |
| Correct retry metric outcome semantics and add a success-after-retry metric test | P046 implementation | Yes for rollout readiness |
| Complete dogfood validation: at least 20 sessions with session data, p95 emit lag < 500 ms, zero cross-run emissions, populate run_report lane | P046 Phase 3 rollout | Blocks release/default-enable closeout, not the local code gate |
| Release receipt after enabling in a release build | P046 release process | Blocks final release closeout only |

No separate follow-up proposal owns the first two items, so they remain in-scope P046 residual work.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | Expose P046 GraphQL read fields and subscription when enabled | Implemented |
| REQ-002 | Expose no GraphQL session reset/control mutation | Implemented |
| REQ-003 | Enforce operator-read authorization for owning run | Implemented |
| REQ-004 | ID-based resolvers resolve parent ownership before data | Implemented |
| REQ-005 | Subscription filters by run and rechecks authorization per emission | Implemented |
| REQ-006 | Bound large reads with deterministic ordering, connection shape, and invalid-cursor behavior | Partially Implemented |
| REQ-007 | Compute KPI and health server-side from persisted rows | Implemented |
| REQ-008 | Redact event details by default-deny versioned allowlist | Implemented |
| REQ-009 | Replace raw sensitive generation fields with safe derived fields | Implemented |
| REQ-010 | Enforce pinned SQLite retry behavior and deterministic exhaustion outputs | Implemented |
| REQ-011 | Bound subscription backpressure, resync, slow-consumer, and shutdown behavior | Implemented |
| REQ-012 | Document MCP-only reset/control ownership | Implemented |
| REQ-013 | SwiftUI gates P046 documents and keeps transient MainActor state | Implemented |
| REQ-014 | Provide rollout fixtures and proposal gate coverage | Implemented |
| REQ-015 | Emit bounded, semantically correct P046 metrics | Partially Implemented |
| REQ-016 | Complete dogfood/release rollout evidence | Not Verifiable |

## Detailed REQ Audit

### REQ-001: Expose P046 GraphQL read fields and subscription when enabled

- Proposal source: Goals lines 33-34; Acceptance Criteria line 763.
- Status: Implemented.
- Evidence: `control-plane/crates/graphql-server/src/schema.rs` P046 query resolvers and subscription; tests `proposal_046_schema_fields_present_when_enabled` and `proposal_046_subscription_field_present_when_enabled`; `./scripts/test-gate.sh proposal-046` passed.
- Notes: Enabled schema is covered by the Rust proposal test suite.

### REQ-002: Expose no GraphQL session reset/control mutation

- Proposal source: Non Goals lines 45-47; Forbidden Schema lines 196-206; Acceptance Criteria line 764.
- Status: Implemented.
- Evidence: `rg` found no reset/control mutation resolver in `graphql-server/src`; tests `proposal_046_no_reset_mutation_in_schema`; gate source guard for `fn reset_session`.

### REQ-003: Enforce operator-read authorization for owning run

- Proposal source: Goals line 35; GraphQL Authorization lines 145-153; Acceptance Criteria line 765.
- Status: Implemented.
- Evidence: `require_operator_read` plus `p046_check_run_accessible` in `control-plane/crates/graphql-server/src/schema.rs`; authorization tests in `proposal_046_session_graphql.rs`; gate passed.
- Note: Current resource scope is run existence/ownership plus operator class/surface policy; no finer per-run ACL was available.

### REQ-004: ID-based resolvers resolve parent ownership before data

- Proposal source: GraphQL Authorization lines 149-151; Acceptance Criteria line 766.
- Status: Implemented.
- Evidence: `find_lineage_owner_run`, `find_generation_with_lineage_owner`, `sessionLineage`, `sessionGenerations`, and `sessionEvents` resolver paths; cross-lineage generation test passed.

### REQ-005: Subscription filters by run and rechecks authorization per emission

- Proposal source: Subscription Lifecycle lines 154-158; Acceptance Criteria line 767.
- Status: Implemented.
- Evidence: `session_status_changed` filters `DomainEvent` by `run_id` before payload resolution and uses `P046LivePrincipalHandle.auth_ok_for_credential` on each emission; tests cover run filtering, revocation, token rotation, and auth-source unavailable; gate passed.

### REQ-006: Bound large reads with deterministic ordering, connection shape, and invalid-cursor behavior

- Proposal source: Goals line 36; Connection Schema lines 159-174; query bounds lines 207-223; Acceptance Criteria line 768.
- Status: Partially Implemented.
- Implemented evidence: `first` caps, stable SQL ordering, `limit+1`, connection DTOs, cursor mismatch rejection, SDL tests, and invalid non-base64 cursor tests all exist and passed.
- Gap: Cursor decoders validate base64/part count and some filter dimensions, but do not parse timestamp tuple fields. `decode_session_cursor` returns `parts[2]` unchecked, `decode_session_lineage_cursor` returns `created_at` unchecked, and `decode_session_generation_cursor` parses generation but not `created_at`. Resolvers accept any decoded cursor with matching run/lineage/filter dimensions before SQL compares the unchecked timestamp string. Tests only cover `NOTACURSOR`, not structurally valid malformed timestamp/wrong-type/expired cursor payloads.

### REQ-007: Compute KPI and health server-side from persisted rows

- Proposal source: Implementation Slice 4 lines 127-133; Acceptance Criteria line 769; Health Thresholds lines 746-759.
- Status: Implemented.
- Evidence: `aggregate_kpis_for_run`, `load_health_data_for_run`, health computation helpers, and health/KPI tests for empty/no-data/stale/context-pressure/orphan/repeated-reset cases; gate passed.

### REQ-008: Redact event details by default-deny versioned allowlist

- Proposal source: Redaction Contract lines 232-262; Acceptance Criteria line 770.
- Status: Implemented.
- Evidence: `redact_event_details`, `extract_typed_details`, closed vocabularies, size guards, unknown event fail-closed behavior, and redaction tests for safe/unsafe/unknown/malformed/oversized shapes; gate passed.

### REQ-009: Replace raw sensitive generation fields with safe derived fields

- Proposal source: Field Sensitivity lines 175-195; Acceptance Criteria line 771.
- Status: Implemented.
- Evidence: GraphQL DTOs expose `hasProviderSession`, `providerSessionRef`, `bindingProfileRef`, `invocationOwnerKind`, `invocationOwnerRef`, `workingDirectoryDisplay`; raw field grep guard and derived-reference tests passed.

### REQ-010: Enforce pinned SQLite retry behavior and deterministic exhaustion outputs

- Proposal source: Implementation Slice 1 lines 104-108; Reliability Contract lines 897-921; Acceptance Criteria line 772.
- Status: Implemented.
- Evidence: `db::p046_retry::p046_retry_db` implements three attempts, 50/150 ms backoff, jitter <= 25 ms, total sleep cap, per-attempt timeout, and 250 ms headroom; retry tests passed.
- Note: Metric semantics are covered separately in REQ-015.

### REQ-011: Bound subscription backpressure, resync, slow-consumer, and shutdown behavior

- Proposal source: Implementation Slice 3 lines 118-126; Subscription lines 263-284; Acceptance Criteria line 773.
- Status: Implemented.
- Evidence: `session_status_changed` uses bounded mpsc capacity, resync guards, lag handling, 5s/3-failure slow-consumer disconnect, shutdown resync, and dedupe inputs; tests cover lag, shutdown, slow consumer, auth revocation, and event ID dedupe; gate passed.

### REQ-012: Document MCP-only reset/control ownership

- Proposal source: Goals line 40; Acceptance Criteria line 774.
- Status: Implemented.
- Evidence: `docs/reference/rust-control-plane.md`, `docs/reference/session-lineage-reuse-and-operator-reset.md`, `docs/reference/test-gates.md`, and negative reset fixture updates document P046 as read/subscription-only with MCP-only reset.

### REQ-013: SwiftUI gates P046 documents and keeps transient MainActor state

- Proposal source: UX/UI lines 61-81; Acceptance Criteria line 775.
- Status: Implemented.
- Evidence: `P046SessionObservabilityModel` is `@MainActor`, checks capability before full documents, cancels previous selected-run observation, re-queries on resync/close/error, deduplicates events, and has no SwiftData dependency. `Proposal046Tests` passed 11 Swift guardrail tests.

### REQ-014: Provide rollout fixtures and proposal gate coverage

- Proposal source: Rollout Contract lines 442-585; Acceptance Criteria line 776; Test Plan lines 778-795.
- Status: Implemented.
- Evidence: `docs/evidence/rollout-contract/operator-readback/p046-session-graphql-full-surface.fixture.json`, all required negative fixtures, `scripts/test-gate.sh proposal-046`, and gate pass.

### REQ-015: Emit bounded, semantically correct P046 metrics

- Proposal source: Metrics lines 496-509, 646-744; bounded outcome vocabulary lines 671-675; Reliability Contract lines 916-918.
- Status: Partially Implemented.
- Implemented evidence: Required metric names are present, labels are bounded to fixed field/status/reason vocabularies in source, and the proposal gate checks metric inventory.
- Gap: `p046_retry_db` increments `session_graphql_sqlite_retry_total` with label `{field}:exhausted` for every transient error before the retry loop is actually exhausted. A query that succeeds on a later retry records both `exhausted` and `success_after_retry`, which makes the metric outcome misleading and can trigger false dogfood alerts.

### REQ-016: Complete dogfood/release rollout evidence

- Proposal source: Rollout lines 587-644; Dogfood Validation lines 714-744; Phase 3 exit line 630.
- Status: Not Verifiable.
- Evidence: The rollout readback fixture declares GraphQL lane `ready_for_phase3` / `pending_phase3_validation`, run_report lane waived pending dogfood, and release_receipt waived pending release.
- Gap: No dogfood run-report evidence was present for >=20 sessions, p95 emit lag < 500 ms, or release receipt. This blocks final release/default-enable closeout but not the local code gate.

## Reviewer / Lens Scorecard

| Lens | Conformance | Top risk | Confidence |
|---|---|---|---|
| Rust architecture | Pass with notes | Resource-scoped auth depends on current operator/run-existence model | High |
| Rust reliability | Pass with notes | Retry behavior is bounded; cancellation metric is not represented | High |
| API contract | Partial | Cursor tuple validation gap | High |
| Observability/rollout | Partial | Retry metric outcome semantics and dogfood evidence | High |
| Apple architecture | Pass | No runtime visual/UI proof beyond guardrail tests | Medium |
| Readiness | Not Ready | API/OPS residuals plus dogfood/release evidence pending | High |

## Routed Specialist Findings

### API-001: Cursor validation accepts structurally decoded but semantically malformed cursors

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-006
- Evidence types: proposal, code, tests-found, tests-run
- Evidence references:
  - Proposal invalid cursor contract: `docs/proposals/046-session-management-graphql-api.md:169`
  - Cursor decoders: `control-plane/crates/db/src/repos/sessions.rs:740`, `:772`, `:795`, `:820`
  - Resolver acceptance of decoded cursors: `control-plane/crates/graphql-server/src/schema.rs:1010`, `:1201`, `:1309`
  - Existing invalid cursor tests only cover `NOTACURSOR`: `control-plane/crates/graphql-server/tests/proposal_046_session_graphql.rs:973`
- Why it matters: The proposal promises malformed, expired, wrong-type, and mismatched-filter cursors return sanitized `invalid cursor`. Today a caller can base64-encode the right number of cursor parts with a matching run/lineage/filter and a non-RFC3339 timestamp. That cursor is accepted and used in SQL lexical comparisons, producing undefined pagination behavior rather than the contract error.
- Recommended action: Make cursor decoding validate every typed tuple component: RFC3339 timestamp, non-empty stable IDs, generation integer, and cursor type/version. If "expired" is not implemented, either add cursor expiry semantics or remove that claim from the contract before closeout.
- Acceptance criteria: Tests fail before the fix and pass after for base64 structurally valid cursors with invalid timestamps, wrong cursor tuple type with matching dimensions, mismatched filters, and expired cursor semantics if retained. All return sanitized `invalid cursor` with no raw echo.

### OPS-001: SQLite retry metric records `exhausted` before exhaustion

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-015
- Evidence types: proposal, telemetry, code, tests-run
- Evidence references:
  - Proposal retry metric: `docs/proposals/046-session-management-graphql-api.md:727`
  - Outcome vocabulary: `docs/proposals/046-session-management-graphql-api.md:671`
  - Alert on retry exhaustion: `docs/proposals/046-session-management-graphql-api.md:719`
  - Implementation emits `exhausted` per transient attempt: `control-plane/crates/db/src/p046_retry.rs:95`
  - Implementation emits `success_after_retry` on later success: `control-plane/crates/db/src/p046_retry.rs:83`
- Why it matters: Dogfood alerting treats retry exhaustion as operationally meaningful. A one-time transient SQLite busy that succeeds on retry will still increment the `exhausted` outcome, making rollout metrics and alerts less trustworthy.
- Recommended action: Record `exhausted` only after the retry loop terminates unsuccessfully. If per-attempt telemetry is useful, use a distinct bounded outcome such as `retry_attempt` or a separate internal counter. Add a test where the first attempt fails transiently and the second succeeds, asserting `success_after_retry` increments and `exhausted` does not.
- Acceptance criteria: `session_graphql_sqlite_retry_total{field,outcome=exhausted}` increments only on actual exhausted retry outcomes; `session_graphql_sqlite_retry_exhausted_total{field}` remains the terminal exhaustion counter; success-after-retry does not create false exhaustion telemetry.

### READY-001: Phase 3 dogfood and release evidence remain pending

- Reviewer: `observability_rollout_reviewer`
- Severity: Major for release closeout; Minor for implementation iteration
- Confidence: High
- Related requirements: REQ-016
- Evidence types: proposal, config, tests-run
- Evidence references:
  - Dogfood success metrics: `docs/proposals/046-session-management-graphql-api.md:737`
  - Phase 3 exit includes dogfood validation: `docs/proposals/046-session-management-graphql-api.md:630`
  - Rollout fixture says GraphQL lane is `ready_for_phase3` / `pending_phase3_validation`: `docs/evidence/rollout-contract/operator-readback/p046-session-graphql-full-surface.fixture.json:7`
  - Run report and release receipt lanes are waived pending dogfood/release: `docs/evidence/rollout-contract/operator-readback/p046-session-graphql-full-surface.fixture.json:50`, `:73`
- Why it matters: The code gate passed, but the proposal's rollout contract does not yet prove dogfood emit-lag, success-rate, zero cross-run live emissions, or release receipt criteria.
- Recommended action: Complete the dogfood window, populate run_report readback, and produce release receipt before declaring P046 fully ready for closeout/default-enable release.
- Acceptance criteria: Fixture lanes move from waived/pending to pass or explicit dated waivers with evidence; dogfood metrics show >=20 sessions, p95 emit lag < 500 ms, p99 < 2 s, zero cross-run emissions, and successful query rate >= 95%.

## Readiness Checklist

| Check | Status | Evidence |
|---|---|---|
| Canonical proposal gate on same tree | Passed | `./scripts/test-gate.sh proposal-046` |
| Rust GraphQL proposal tests | Passed | 81 tests passed |
| Retry unit tests | Passed | graphql-server P046 lib tests 4 passed; db P046 retry test 1 passed |
| Swift guardrail tests | Passed | 11 `Proposal046Tests` passed |
| Feature flag / disabled schema | Passed with API note | Rust and Swift tests passed |
| Authorization and sensitive data redaction | Passed | Rust tests and source inspection |
| Cursor contract | Partial | API-001 |
| Metrics/rollout semantics | Partial | OPS-001, READY-001 |
| UI runtime screenshot / remote UI tests | Not run | Not required by proposal gate; residual visual QA risk |
| Accessibility/localization/privacy/entitlements | No new entitlement/privacy surface found; UI accessibility not runtime-verified | Code inspection only |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-046-session-b4f4b41c/docs/proposals/046-session-management-graphql-api.md` -> selected `docs/proposals/046-session-management-graphql-api_IMPLEMENTATION_AUDIT_R1.md`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py .../docs/proposals/046-session-management-graphql-api.md` -> no prior review artifacts found.
- `git status --short --branch` -> dirty worktree on `cw/implement-proposal-046-session/b4f4b41c`, audited as-is.
- `git diff --stat origin/main...HEAD` and focused file reads -> GraphQL/db/Swift/docs/gate implementation surfaces inspected.
- `rg` for forbidden reset/control mutation symbols in `control-plane/crates/graphql-server/src` -> no P046 reset/control mutation resolver found.
- `./scripts/test-gate.sh proposal-046` -> passed. Covered Rust GraphQL P046 test suite (81 tests), graphql-server P046 lib tests (4), db retry test (1), Swift `Proposal046Tests` (11), rollout fixture lane validation, negative fixture checks, metric inventory, and source guardrails.

## Final Verdict

The implementation satisfies the main P046 behavior and passes the same-tree canonical proposal gate, but it is not fully conformant for closeout. Two in-scope edge contracts remain: cursor tuple validation is incomplete for semantically malformed cursors, and retry metric outcomes can falsely report exhaustion. Phase 3 dogfood/release evidence is also pending by the implementation's own rollout fixture.

Recommended next actions:

1. Tighten cursor decoding/validation and add negative tests for structurally valid malformed cursor payloads.
2. Fix retry metric outcome semantics and add a success-after-retry telemetry regression.
3. Rerun `./scripts/test-gate.sh proposal-046`.
4. Complete dogfood/run_report/release receipt evidence before proposal closeout or default-enable release.
