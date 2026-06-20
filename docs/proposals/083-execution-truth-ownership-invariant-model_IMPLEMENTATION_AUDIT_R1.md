# Proposal 083 Implementation Audit R1

Audit date: 2026-06-20

Proposal: `docs/proposals/083-execution-truth-ownership-invariant-model.md`

Proposal revision audited: `P083-r70-refined-r69-score-lift`

Workspace HEAD: `97bea6d580ee9de8954071662ed528153f125afd`

Audit mode: implementation audit only. This report is the only P083 file created by the audit.

## Verdict

Overall conformance: **Not Implemented**

Overall readiness: **Not Ready**

The implementation contains substantial P083 work across SQLite migrations, command idempotency repos, shutdown recovery, GraphQL/MCP mutations, rollout fixtures, and SwiftUI identity-hold views. It does not yet satisfy the proposal contract. The blocking mismatches are not cosmetic: the public rollback API still uses string `rollback_mode`/`request_id` shapes instead of the proposal's non-null rollback target and `CallerRequestId` contract, command idempotency hashes the wrong logical fields, durable monotonic clock baseline correlation is missing from deadline-bearing rows, native macOS command placement is absent, rollout evidence still contains placeholders, and the canonical P083 gate currently fails.

Readiness is also blocked by the proposal itself. The proposal status is `Revise-required`, and its active readiness narrative says `implementation_may_start=false` until a human implementation approval gate and fresh aggregate review approve the revision with `blocker_count=0` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:7`, `:43-46`).

## Reviewer Reuse

Prior review discovery: **Not reused**

The audit helper found no existing P083 implementation-audit artifacts beside this proposal. No prior implementation report was reused.

Selected specialist coverage, capped at five reviewers:

| Reviewer | Why selected | Coverage result |
| --- | --- | --- |
| `chainworks_execution_truth_reviewer` | Repo-local execution-truth owner lens is directly triggered by P083. | Used for ownership, durable truth, readback, and rollback invariants. |
| `rust_reliability_reviewer` | P083 changes durable state machines, restart recovery, idempotency, shutdown, and late-output handling. | Used for command idempotency, monotonic clock, recovery, and proof-gate findings. |
| `api_contract_reviewer` | P083 has explicit GraphQL SDL and MCP JSON Schema contracts. | Used for GraphQL/MCP shape, enum, caller-request, and denial-vocabulary findings. |
| `observability_rollout_reviewer` | P083 readiness depends on rollout readback, fixtures, metrics, migration hashes, and hold conditions. | Used for rollout evidence, placeholder fixture, migration/readback, and metric-label checks. |
| `rust_security_reviewer` | Lifecycle commands, process signaling, auth/caller identity, and rollback are security-sensitive. | Used for boundary, process identity, command auth, schema, and signal dispatch review. |

Coverage limitation: P083 also triggers macOS UI, Apple UX, and Apple architecture lenses. Those were manually spot-checked in this audit, but no dedicated UI/UX/Apple specialist pass fit within the five-reviewer cap. Because UI/macOS contracts are explicit acceptance criteria, this remains a readiness blocker rather than a waived scope.

## Security-Sensitive Assessment

Security-sensitive diff: **Triggered**

Relevant categories included auth/caller identity, public ingress/API schema, subprocess/process boundary, resource limits, redaction/privacy, and schema parsing. The security pass focused on P083-owned surfaces:

- GraphQL and MCP lifecycle mutations.
- Caller request id validation and principal-derived caller identity.
- Command idempotency leases and aliasing.
- Rollback and enforcement-mode state.
- Provider-session shutdown, process identity checks, and OS signal dispatch.
- Late-output overflow and readback/fixture redaction.
- SQLite migrations and rollout readback.

No separate exploit proof was needed to block readiness: the security-critical public boundary still accepts and hashes a different rollback shape than the proposal requires, and the proof gate fails. The shutdown dispatcher does include important hardening, including process identity verification and a durable `planned -> dispatching` claim before `kill()` (`control-plane/crates/engine/src/shutdown_service.rs:669-691`, `:909-955`), but missing baseline correlation and a failing gate prevent a release-ready security verdict.

## Implementation Surfaces Audited

Primary surfaces:

- Proposal contract: `docs/proposals/083-execution-truth-ownership-invariant-model.md`.
- Rust GraphQL: `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/graphql-server/src/types/p083.rs`.
- Rust MCP: `control-plane/crates/mcp-server/src/tools/runs.rs`.
- Rust engine: `control-plane/crates/engine/src/command_handler.rs`, `control-plane/crates/engine/src/shutdown_service.rs`, executor late-output/recovery paths.
- SQLite migrations: `control-plane/crates/db/migrations/087_p083_*.sql` through `094_p083_008_signal_dispatching_state.sql`.
- DB repos/tests: `control-plane/crates/db/src/repos/*p083*`, `control-plane/crates/db/tests/proposal_083_migrations.rs`.
- Rollout/readback evidence: `docs/evidence/083/**`, `docs/evidence/rollout-contract/**/p083*.json`.
- Swift/macOS UI: `Chainworks Forge/Chainworks_ForgeApp.swift`, `Chainworks Forge/Views/ManualProcessIdentityCheckBanner.swift`, `Chainworks Forge/Views/P083IdentityAmbiguousInboxView.swift`, `Chainworks Forge/Models/P083IdentityHoldSessionsModel.swift`.
- Canonical gate: `scripts/test-gate.sh`.

## Requirement Coverage

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Current revision readiness gate and approval posture. | Missing | Proposal remains `Revise-required`; `implementation_may_start=false` until fresh aggregate approve and human gate (`proposal.md:7`, `:43-46`). |
| REQ-002 | Durable ownership model and caller identity classification. | Partially implemented | Command context and idempotency repos exist, but public API field names and rollback target authority do not match the model. |
| REQ-003 | GraphQL lifecycle SDL with `CallerRequestId`, closed enums, denial union, and `targetEnforcementMode`. | Missing | Proposal requires enum/union SDL and `p083RollbackExecution(targetEnforcementMode: P083RollbackTargetMode!, callerRequestId: CallerRequestId!)` (`proposal.md:151-181`). Implementation exposes `rollback_mode: String`, `reason: String`, `request_id: String` (`schema.rs:5795-5825`) and `GqlP083RollbackExecutionPayload.rollback_mode: String` (`types/p083.rs:757-768`). |
| REQ-004 | MCP Draft 2020-12 schemas with proposal field names and parity vocabulary. | Partially implemented | The tool schemas use Draft 2020-12 and `additionalProperties:false`, but `p083.rollback_execution` requires `rollback_mode`, `reason`, and `request_id`, not `target_enforcement_mode` and `caller_request_id` (`runs.rs:321-343`, `:1216-1242`). |
| REQ-005 | Command idempotency for lifecycle commands with canonical logical fields. | Partially implemented | Repo and command wiring exist, but rollback intent hashes `command`, `reason`, and `rollback_mode` (`command_handler.rs:7881-7891`), while the proposal requires only `target_enforcement_mode` and excludes diagnostic metadata (`proposal.md:1099-1114`). Set-enforcement hashes `reason` and `enforcement_mode` instead of only `target_mode` (`command_handler.rs:8237-8247`). |
| REQ-006 | Rollback target is non-null and audited end-to-end. | Missing | Proposal requires `target_enforcement_mode` in rollback audit and readback (`proposal.md:426-430`). Migration `091` creates `p083_rollback_audit` without `target_enforcement_mode` (`091_p083_005_enforcement_and_rollback.sql:38-54`), and the command inserts no such column (`command_handler.rs:8122-8135`). |
| REQ-007 | Seven additive P083 migrations with readback SHA/verification. | Partially implemented | Proposal lists seven migrations (`proposal.md:353-466`). The gate expects eight physical migrations, including `094_p083_008_signal_dispatching_state.sql` (`scripts/test-gate.sh:9306-9322`). The extra hardening migration may be useful, but it is not reconciled with the proposal's migration/readback contract. |
| REQ-008 | Durable monotonic clock baseline generation and baseline correlation. | Missing | Proposal requires `baseline_generation`, `wall_clock_iso8601`, `clock_skew_ms`, `periodic`, `fallback_wall_only`, and `baseline_sample_id` on deadline-bearing rows (`proposal.md:517-538`). Migration `092` instead has `checkpoint`, `rollback_detected`, `stale_fallback`, no `baseline_generation`, no `wall_clock_iso8601`, and no `clock_skew_ms` (`092_p083_006_durable_monotonic_clock.sql:7-27`). |
| REQ-009 | Shutdown signal side effects include baseline correlation and restart-safe dispatch. | Partially implemented | Dispatcher hardening exists with identity verification and `planned -> dispatching` before OS signal (`shutdown_service.rs:909-955`). The schema still lacks `baseline_sample_id` on `shutdown_signal_side_effects` (`089_p083_003_shutdown_receipts_and_signals.sql:37-57`, `094_p083_008_signal_dispatching_state.sql:11-32`), contrary to proposal (`proposal.md:397-400`, `:523`). |
| REQ-010 | Provider cancellation intents and identity-ambiguous holds. | Partially implemented | Tables and UI/model surfaces exist, but `provider_cancellation_intents` lacks `baseline_sample_id` despite proposal DDL (`093_p083_007_provider_cancellation_intent_and_process_fate.sql:45-64`, `proposal.md:455`). |
| REQ-011 | Post-cancel late-output overflow latch and projection protection. | Partially implemented | Late-output fixtures and implementation surfaces exist, but the canonical P083 gate fails before full acceptance; release evidence is incomplete. |
| REQ-012 | Rollout readback API parity across run report, MCP, release receipt, and GraphQL. | Partially implemented | Readback/fixture files exist, but the operator readback fixture is explicitly a placeholder and still references an older revision (`docs/evidence/rollout-contract/operator-readback/p083-full-surface.fixture.json:1-35`). |
| REQ-013 | Rollout contract negative fixtures and current security/observability review gates. | Missing | Multiple negative fixtures still contain `placeholder_fixture_kind`, and operator readback contains `placeholder_pending_implementation_evidence`. |
| REQ-014 | Bounded operational metrics labels. | Partially implemented | Metrics fixtures exist under `docs/evidence/083/metrics`, but the rollout-contract negative unbounded-label fixture is still a placeholder and the canonical gate fails. |
| REQ-015 | SwiftData lifecycle boundary and concurrency isolation. | Not verifiable | Swift evidence fixtures exist, but no dedicated Swift/static acceptance path was reached by the canonical gate; readiness cannot rely on static fixtures alone. |
| REQ-016 | Native macOS Run menu, focused values, toolbar parity, accessibility parity. | Missing | Proposal requires a top-level `Run` menu with Lifecycle/Recovery submenus and `@FocusedValue` parity (`proposal.md:1263-1297`). The app currently defines only a `Navigation` command menu in `Chainworks_ForgeApp.swift:48-84`; search found no `CommandMenu("Run")` or focused-value lifecycle command wiring. |
| REQ-017 | `ManualProcessIdentityCheckBanner` hierarchy, states, confirmation, overflow, duplicate rollup. | Partially implemented | A banner exists with retry/copy/mark/evidence buttons (`ManualProcessIdentityCheckBanner.swift:81-126`), but the action order/style, confirmation dialog, loading/error/success states, overflow menu, and grouped duplicate-session rule required by proposal are absent (`proposal.md:1152-1221`). |
| REQ-018 | Graceful AppKit termination vs abrupt external shutdown semantics. | Partially implemented | Termination/recovery fixtures exist, and a later duplicate P083 gate branch checks `AppTerminationCoordinator`, but that branch is unreachable because the first `proposal-083|p083)` case wins (`scripts/test-gate.sh:9283`, `:9732`, `:10000`). |
| REQ-019 | Canonical proof gate exists and passes. | Missing | `./scripts/test-gate.sh proposal-083` failed during the engine step. See verification log. |
| REQ-020 | Mandatory hardening requirements. | Partially implemented | Some hardening exists, including atomic signal dispatch state, TTL constants, and late-output fixtures, but schema-version policy, failed-terminal policy, rollback audit target, migration/readback evidence, and external side-effect proof are not complete. Proposal marks these mandatory (`proposal.md:1298-1328`). |

## Key Findings

### BLOCKER-001: P083 cannot be marked ready while the proposal revision itself is still not implementation-ready

The proposal text says implementation may start only after a human approval gate and a fresh aggregate review returns approve with `blocker_count=0` (`proposal.md:7`, `:43-46`). This audit found no current implementation-review artifact that overrides that state.

Impact: even a technically complete implementation would still be administratively not ready. The current implementation is also technically incomplete.

Required fix: obtain and archive the required current review/approval evidence, then wire rollout readiness to that concrete artifact.

### BLOCKER-002: The canonical P083 gate fails

Executed command: `./scripts/test-gate.sh proposal-083`

Observed result:

- DB migration integration tests completed: 53 passed.
- `cargo test -p db --lib p083_` selected 0 tests and the gate accepted the zero-test result.
- The engine step failed to compile/test with duplicate test definitions in `crates/engine/src/session/policy.rs` and missing struct fields in `crates/engine/src/executor.rs`.
- Exit status: 101.

Representative failures:

- Duplicate test names:
  - `tool_output_budget_failure_requires_session_invalidation`
  - `claude_long_context_credits_failure_invalidates_generation_before_reuse`
  - `p082_duplicate_active_session_generations_converge_to_single_survivor`
- Missing fields:
  - `provider` and `provider_family` in `ContinuationEligibilityInfo`
  - `provider_runtime_home` in `ExecutionRequest`

Impact: no readiness claim is supportable. The gate also under-proves P083 because it accepts a zero-test DB filter and has duplicate unreachable `proposal-083|p083)` branches (`scripts/test-gate.sh:9283`, `:9732`, `:10000`).

Required fix: make the first canonical branch comprehensive, remove or merge duplicate branches, fail on zero selected tests where selection is intended, and get the gate green on a clean tree.

### BLOCKER-003: GraphQL and MCP do not implement the R70 rollback-target contract

Proposal R70 exists to resolve the R69 blocker by making rollback target a first-class non-null caller input. It requires GraphQL `targetEnforcementMode: P083RollbackTargetMode!` and MCP `target_enforcement_mode` (`proposal.md:151-181`, `:195-209`).

Implementation evidence:

- GraphQL accepts `rollback_mode: String`, `reason: String`, and `request_id: String` (`schema.rs:5795-5801`).
- GraphQL validates `rollback_mode` with a string `matches!` check instead of SDL enum enforcement (`schema.rs:5814-5817`).
- GraphQL payload returns `rollback_mode: String` (`types/p083.rs:757-768`).
- MCP schema requires `rollback_mode`, `reason`, and `request_id` (`runs.rs:321-343`).
- MCP runtime reads `params["rollback_mode"]` and `params["request_id"]` (`runs.rs:1216-1242`).

Impact: this reintroduces the exact class of cross-surface contradiction that R70 was written to remove. API clients cannot use the proposal's field names or typed rollback target contract.

Required fix: update GraphQL SDL/resolvers/payloads and MCP schemas/runtime to use `callerRequestId`/`caller_request_id`, `targetEnforcementMode`/`target_enforcement_mode`, closed enum domains, and shared denial vocabulary. Remove free-form lifecycle strings from caller-supplied lifecycle targets.

### BLOCKER-004: Rollback command idempotency hashes the wrong intent

Proposal requires `p083.rollback_execution` intent hash fields to be exactly `["target_enforcement_mode"]` and excludes caller request id, timestamps, principal display names, and diagnostic metadata (`proposal.md:1099-1114`).

Implementation hashes:

- `command = "p083.rollback_execution"`
- `reason`
- `rollback_mode`

Evidence: `control-plane/crates/engine/src/command_handler.rs:7881-7891`.

Set enforcement similarly hashes `command`, `enforcement_mode`, and `reason` instead of only `target_mode` (`command_handler.rs:8237-8247`).

Impact: same rollback target with a different operator rationale becomes a different intent, so same-intent aliasing and replay semantics violate the proposal.

Required fix: centralize per-command logical field selection, use proposal field names, exclude diagnostic `reason`, and add fixtures for same-request replay, same-intent aliasing across a new request id, and target mismatch denial.

### BLOCKER-005: Durable monotonic clock schema does not satisfy baseline correlation

Proposal requires a baseline generation model and baseline correlation for deadline-bearing rows (`proposal.md:517-538`). The implemented clock table omits required fields and uses different states (`092_p083_006_durable_monotonic_clock.sql:7-27`).

Missing or mismatched:

- No `baseline_generation`.
- No `wall_clock_iso8601`.
- No `clock_skew_ms`.
- `sample_state` lacks `periodic` and `fallback_wall_only`.
- `sample_state` adds non-proposal values `checkpoint`, `rollback_detected`, and `stale_fallback`.
- No unique baseline index for `(boot_id, baseline_generation)`.
- `shutdown_signal_side_effects` lacks `baseline_sample_id`.
- `provider_cancellation_intents` lacks `baseline_sample_id`.

Impact: recovery cannot prove which baseline converted deadline-bearing monotonic timestamps, so restart/reboot/skew semantics remain under-specified and under-implemented.

Required fix: reconcile schema with the proposal, add `baseline_sample_id` to all required rows, implement nearest-at-or-before fallback, and prove reboot/skew/rollback behavior with executable tests.

### BLOCKER-006: Native macOS command contract is absent

Proposal requires a top-level `Run` menu between `View` and `Window`, fixed Lifecycle and Recovery submenus, focused-value command routing, toolbar parity, and byte-equal accessibility/help strings (`proposal.md:1263-1297`).

Implementation evidence:

- `Chainworks_ForgeApp.swift` defines `CommandMenu("Navigation")` only (`Chainworks_ForgeApp.swift:48-84`).
- Repository search found no `CommandMenu("Run")`, no lifecycle command `@FocusedValue` wiring, and no toolbar/menu parity implementation.

Impact: operators do not get the native lifecycle/recovery command surface P083 requires, and keyboard/menu/toolbar denial parity is not proved.

Required fix: implement the `Run` menu, focused values, enabled-state/denial help, toolbar parity, and lint/test fixtures in the canonical gate.

### MAJOR-001: Manual process identity UI is only a partial banner

Proposal requires a `ManualProcessIdentityCheckBanner` with primary retry action, secondary confirmed destructive mark-absent action, tertiary copy diagnostic icon, overflow evidence action, loading/success/error states, no automatic spinner, and grouped duplicate-session behavior (`proposal.md:1152-1221`).

Implementation evidence:

- Banner action row is a simple horizontal set of copy, retry, mark absent, and open evidence buttons (`ManualProcessIdentityCheckBanner.swift:81-126`).
- Copy feedback exists, but confirmation, loading/error/success flows, overflow placement, primary action prominence, and duplicate-session rollup are not visible in the audited implementation.

Impact: the UI does not yet meet the proposal's operator safety and accessibility requirements for ambiguous process identity recovery.

Required fix: implement the required action hierarchy and states, then prove the three required surfaces and duplicate rollup with Swift/UI fixtures.

### MAJOR-002: Rollout evidence still contains placeholders

Placeholder evidence remains in the required rollout-contract area:

- `docs/evidence/rollout-contract/operator-readback/p083-full-surface.fixture.json:1-35`
- `docs/evidence/rollout-contract/negative/p083-force-quit-host-budget-claim.json`
- `docs/evidence/rollout-contract/negative/p083-migration-sha256-missing.json`
- `docs/evidence/rollout-contract/negative/p083-final-readback-rank-stored.json`
- `docs/evidence/rollout-contract/negative/p083-stale-security-review.json`
- `docs/evidence/rollout-contract/negative/p083-rollback-disposition-missing-schema-version.json`
- `docs/evidence/rollout-contract/negative/p083-stale-observability-rollout-review.json`
- `docs/evidence/rollout-contract/negative/p083-unbounded-metric-label.json`

Impact: P083's own rollout contract cannot distinguish concrete same-tree release evidence from proposal-freeze placeholders.

Required fix: replace placeholders with generated same-tree evidence, include current security/observability review artifacts, and make the rollout lint fail on stale revision ids or placeholder sentinel strings.

### MAJOR-003: Migration contract drift is unresolved

The proposal lists seven P083 migrations (`proposal.md:353-466`). The gate expects eight physical migration files and prints "all 8 P083 physical migration files present" (`scripts/test-gate.sh:9306-9322`).

The eighth migration adds a useful `dispatching` state for signal dispatch safety (`094_p083_008_signal_dispatching_state.sql:1-32`), but it also recreates `shutdown_signal_side_effects` without the proposal-required `baseline_sample_id`.

Impact: release/readback claims cannot be compared cleanly to the approved proposal. Operators will see a different migration set than the proposal promised.

Required fix: either revise the proposal/readback contract to own the eighth migration or fold the dispatching-state change into the approved migration plan, while preserving baseline correlation.

## Product and Platform Scope

Product scope: operator-facing execution-truth and recovery workflow for Chainworks Forge runs. This is not just backend infrastructure; P083 explicitly changes operator recovery behavior, native commands, accessibility, and readback.

Platform scope:

- Rust daemon/control-plane: write authority and durable execution truth.
- GraphQL and MCP: public lifecycle command surfaces.
- SQLite: authoritative persistence for lifecycle, idempotency, shutdown, rollout, and readback rows.
- macOS SwiftUI app: read-only/projection operator shell plus native lifecycle command affordances.

Out-of-scope items preserved by this audit:

- No new RBAC/keychain/auth system beyond existing principal helpers.
- No YAML workflow changes.
- No historical artifact deletion.
- No native macOS write path for `side_effects.force_reconcile`.
- No destructive rollback migration.

## Verification Log

Commands and checks run:

| Check | Result |
| --- | --- |
| Prior review discovery helper | No P083 implementation-audit artifacts found. |
| Report path helper | Selected `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R1.md`. |
| `find docs/proposals -maxdepth 1 -name '*083*IMPLEMENTATION_AUDIT*.md' -print` before report | No existing P083 audit report. |
| `./scripts/test-gate.sh proposal-083` | Failed with exit 101 during engine compile/test after DB migration tests passed. |
| `git diff --check` | Passed with no whitespace errors. |
| Placeholder scan for P083 rollout-contract evidence | Found placeholder sentinels in operator readback and multiple negative fixtures. |
| GraphQL/MCP shape searches | Found `rollback_mode`/`request_id` string surfaces; no implemented proposal SDL enum shape for rollback target. |
| macOS command search | Found no `CommandMenu("Run")` or lifecycle focused-value command wiring. |

The working tree was already dirty before this report, including unrelated P079/P080/P082/P086 audit and implementation files. This audit did not revert or normalize those changes.

## Closeout Readiness

Closeout-ready: **No**

Release-ready: **No**

Ready after:

1. Proposal status and review/approval evidence are current and allow implementation.
2. Canonical P083 gate is single, comprehensive, and green.
3. GraphQL and MCP implement the exact R70 rollback target/caller request/enum/denial contracts.
4. Command idempotency uses centralized proposal-defined logical fields.
5. Migrations/readback match the proposal, including baseline clock correlation.
6. Rollout placeholders are replaced with concrete same-tree evidence.
7. Native macOS Run menu, toolbar parity, focused values, and identity banner UX are implemented and tested.
8. Dedicated UI/UX/Apple specialist coverage or equivalent executable acceptance evidence is attached.

