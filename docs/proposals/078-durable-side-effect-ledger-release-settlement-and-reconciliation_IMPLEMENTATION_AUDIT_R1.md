# Proposal 078 Implementation Audit R1

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/078-durable-side-effect-ledger-release-settlement-and-reconciliation.md` |
| Proposal format audited | Current worktree file is JSON payload `schema_version=proposal_document_v1`, revision `p078-refined-2026-05-07-dfc2d583-r2`; the historical `HEAD` version is Markdown Draft |
| Audit timestamp | 2026-05-12T11:02:23+03:00 |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-078-durable-e49976db` |
| Implementation branch | `cw/implement-proposal-078-durable/e49976db` |
| Implementation HEAD | `8fb2afa96f8fb2ce9c26ae612a46fa26308147c4` |
| Compare base | Implicit current worktree, including staged/unstaged/unmerged changes |
| Proposal state | Ambiguous: current JSON has no `Status`; `HEAD` Markdown says `Draft` |
| Overall Conformance | Not Implemented |
| Overall Implementation Readiness | Not Ready |
| Audit Confidence | High for blockers and major gaps; Medium for UI readback gaps because Swift files are conflicted |
| Reviewer Selection Reuse | Not reused |

## Implementation Target

The audited target is not a clean implementation tree. `git status --short` reports unresolved merge conflicts in:

- `Chainworks Forge/Engine/ExecutionService.swift`
- `Chainworks Forge/Engine/RunStageSnapshotLoader.swift`
- `Chainworks ForgeTests/ResumeManagerTests.swift`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/mcp-server/src/tools/runs.rs`

The proposal path has also drifted from Markdown into a one-line JSON proposal document. I parsed that JSON as the current contract because it is the worktree proposal content supplied for this audit.

## Prior Review Reuse

No reusable prior proposal-review artifact was discovered beside this proposal. The current JSON proposal embeds `source_review_pass_id=c4f604a1-6537-4027-acc5-876c70013226` and a `reviewer_feedback_resolution` list, but there is no adjacent `*.review/` package or reviewer-selection file to reuse.

Selected reviewers:

- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`
- `rust_security_reviewer`

Rejected close alternatives:

- `apple_arch_reviewer`: Swift surfaces are touched and conflicted, but the durable contract is owned by Rust control-plane semantics; Swift issues are captured as readiness/readback evidence gaps.
- `macos_ui_reviewer`: no new visual UI implementation could be validated in this conflicted tree.
- `product_reviewer`: proposal metrics exist, but the blocking issues are implementation, API, reliability, and rollout proof failures rather than product decision quality.

## Proposal Contract Summary

P078 requires a durable side-effect ledger for externally visible release operations. The current JSON contract makes these commitments:

- durable intent before every wired external operation;
- first wired paths for `git_commit`, `git_push`, `build_archive`, and `connect_upload`;
- schema-supported deferred kinds for `tag_create` and `artifact_publish`;
- deterministic idempotency keys, request fingerprints, and version cutover blocking;
- compact SQLite lifecycle rows plus P075-style file-spooled evidence;
- fail-closed retry, targeted retry, cancellation, startup recovery, and scheduler requeue before canonical mutation;
- MCP-only reconciliation command/control through `effects.*`;
- GraphQL and governed SwiftUI as read-only projections;
- P084 rollout contract coverage, metrics/log definitions, fixtures, rollback disposition, and `proposal-078|p078` gate aliases;
- local validation with fake adapters and no live external side effects.

Platform/product scope:

- Apple: macOS read-only projection/presenter scope only.
- Backend/service: Rust control-plane service, worker/release execution, DB migration, MCP, GraphQL, rollout/observability.
- Overall scope: cross-stack control-plane/API/readback contract.

Primary implementation flows:

1. Release execution prepares and leases durable side-effect intent before invoking git/archive/upload adapters.
2. Executor, watchdog/reaper, and settlement CAS race over the same effect row without duplicate external writes.
3. Retry, targeted retry, cancellation, startup recovery, and scheduler requeue check unresolved effects before canonical mutation.
4. Operator inspects and reconciles unresolved side effects through MCP `effects.*` tools.
5. GraphQL, run reports, release receipts, and SwiftUI expose read-only lifecycle diagnostics and next actions.

## Proposal Fidelity Inventory

### Matches

- `EffectKind` and `SideEffectStatus` domain enums include the required initial kinds and lifecycle statuses in `control-plane/crates/domain/src/side_effect.rs`.
- Migration file `control-plane/crates/db/migrations/049_p078_side_effect_ledger.sql` defines `side_effects`, `side_effect_attempts`, and `side_effect_settlements` with CHECK constraints and idempotency indexes.
- `DurableEffectCoordinator` exists with `prepare_effect`, `prepare_and_lease`, `mark_write_started`, `settle_success`, `settle_failure`, `retry_preflight`, `retry_preflight_within_tx`, `run_cancel_preflight`, and `watchdog_pass`.
- `CommandHandler::RetryStage` calls `retry_preflight_within_tx` before replacement stage creation.
- MCP `effects.list`, `effects.inspect`, `effects.reconcile`, `effects.mark_unrecoverable`, and `effects.clear_after_manual_verification` code is present.
- GraphQL has a bounded top-level read-only `unresolved_side_effects(first)` query.
- `proposal-078|p078` aliases exist in `scripts/test-gate.sh` and `docs/reference/test-gates.md`.

### Divergences

- The proposal requires migration `046_p078_side_effect_ledger.sql`; implementation added `049_p078_side_effect_ledger.sql`, colliding with existing `049_p075_storage_write_pressure_window_key.sql`. SQLx migration application fails with duplicate version `049`.
- Release execution wraps only `git_push` and `connect_upload` in side-effect rows. It does not create separate `git_commit` or `build_archive` effect rows for the first wired paths required by the proposal.
- Release side-effect intents use `expected_evidence_json: None` and do not materialize proposal-required evidence manifests, checksums, or readback payloads.
- `effects.reconcile` returns local ledger state only; it does not perform git/upload/archive readback, write a reconciliation report, or complete settlement from evidence.
- Startup recovery does not call the P078 side-effect watchdog/reaper path; existing startup recovery code is still focused on work items, projections, toolchain/cache, and P088 receipt recovery.
- Settlement of side effects is not the same transaction as canonical release artifacts, runtime facts, agent/stage state, workflow cursor, queue advance, and projection invalidation.
- GraphQL exposes a global `unresolved_side_effects(first)` query, not the proposal's run-attached read model with `blockedReason`, `retryForbidden`, recommended MCP command, evidence summary, and reconciliation-report availability.
- The rollout contract fixture exists, but `proposal-078` gate does not validate the P084 readback fixture, negative fixtures, metric/log cardinality budgets, or rollout hold conditions.
- `control-plane/crates/mcp-server/src/tools/mod.rs` has a syntax-level match-arm issue around the added `effects.*` arms, and `control-plane/crates/mcp-server/src/tools/runs.rs` contains conflict markers.

### Ambiguities / Evidence Gaps

- The current proposal file is JSON in a `.md` path. The audit used it as source of truth, but this does not satisfy the skill's normal Markdown input expectation.
- Swift/macOS read-only diagnostics could not be validated because Swift files are unmerged.
- The gate stopped in DB tests before engine and MCP test phases; later phases remain unverified in this exact tree.
- There is no executed evidence for crash-after-git-push, crash-after-upload, cancellation, startup recovery, GraphQL SDL, Swift accessibility, or no-live-credential guard assertions.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | `proposal-078|p078` gate aliases pass locally without live side effects | Partially Implemented |
| REQ-002 | P084 rollout contract coverage, fixtures, metrics/logs, rollback, hold conditions | Partially Implemented |
| REQ-003 | Additive side-effect ledger migration and compact schema | Partially Implemented |
| REQ-004 | Domain lifecycle, idempotency, request fingerprint, version cutover policy | Partially Implemented |
| REQ-005 | DurableEffectCoordinator and CAS lifecycle | Partially Implemented |
| REQ-006 | Durable intent before every first wired release operation | Partially Implemented |
| REQ-007 | At most one external-write attempt per side_effect row | Partially Implemented |
| REQ-008 | Retry, targeted retry, cancellation, startup recovery, scheduler requeue preflight | Partially Implemented |
| REQ-009 | MCP-only reconciliation command/control with auditable dispositions | Partially Implemented |
| REQ-010 | GraphQL, reports, receipts, and SwiftUI read-only readback | Partially Implemented |
| REQ-011 | Startup recovery and watchdog classify unresolved effects | Partially Implemented |
| REQ-012 | Single barrier settlement transaction for ledger and canonical state | Missing |
| REQ-013 | P075-style evidence spooling and high-volume evidence discipline | Partially Implemented |
| REQ-014 | Metrics, structured logs, rollout readback, and rollback observability | Partially Implemented |
| REQ-015 | Required validation plan and same-tree gate evidence | Missing |

## Detailed Requirement Audit

### REQ-001 - Gate aliases pass locally without live side effects

- Source: `acceptance_criteria[0]`, `validation_plan[0]`, `rollout_contract.gate_aliases`
- Status: Partially Implemented
- Evidence: `scripts/test-gate.sh:2181`, `scripts/test-gate.sh:6181`, `docs/reference/test-gates.md:1086`
- Tests run: `./scripts/test-gate.sh proposal-078`
- Notes: Aliases are registered and the gate uses local Rust cargo tests. The gate does not pass: domain P078 tests passed, then DB P078 tests failed 18/18 due duplicate SQLx migration version `049`.

### REQ-002 - P084 rollout contract coverage

- Source: `acceptance_criteria[1]`, `rollout_contract`, `rollout_contract_v1`
- Status: Partially Implemented
- Evidence: `docs/evidence/rollout-contract/operator-readback/p078-full-surface.fixture.json`, `docs/evidence/rollout-contract/negative/p078-missing-side-effect-readback.json`
- Gap: Fixture files exist, but `scripts/test-gate.sh proposal-078` does not validate them. Search found no gate references to `p078-full-surface` or `p078-missing-side-effect-readback`. Required metrics/log definitions are proposal text and sparse log strings, not executable rollout proof.

### REQ-003 - Additive side-effect ledger migration and compact schema

- Source: `data_model`, `validation_plan[1]`
- Status: Partially Implemented
- Evidence: `control-plane/crates/db/migrations/049_p078_side_effect_ledger.sql:11`, `:79`, `:106`
- Gap: The schema file exists and includes expected tables and constraints, but the proposal requires migration `046_p078_side_effect_ledger.sql`. The implemented migration uses version `049`, colliding with `049_p075_storage_write_pressure_window_key.sql`. Gate output: SQLx migration apply failed with `UNIQUE constraint failed: _sqlx_migrations.version`.

### REQ-004 - Domain lifecycle, idempotency, fingerprint, and version cutover

- Source: `scope`, `lifecycle_model`, `idempotency_and_attempt_policy`
- Status: Partially Implemented
- Evidence: `control-plane/crates/domain/src/side_effect.rs:103`, `:177`, `:213`, `:362`
- Tests run: domain P078 tests in `./scripts/test-gate.sh proposal-078` passed 10/10.
- Gaps: Version cutover behavior is only partially visible. Executor-local key derivation in `control-plane/crates/engine/src/executor.rs:12272` diverges from the domain helper by formatting keys as `p078:v1:{effect_kind}:{hash}` instead of the proposal/domain `p078:v{intent_version}:{hex}` shape.

### REQ-005 - DurableEffectCoordinator and CAS lifecycle

- Source: `architecture.components`, `reaper_and_cas_contract`
- Status: Partially Implemented
- Evidence: `control-plane/crates/engine/src/side_effects.rs:30`, `:62`, `:165`, `:241`, `:302`, `:346`, `:371`, `:419`
- Gap: The core API exists, but DB tests proving CAS and repository behavior fail before execution because migrations cannot apply. `watchdog_pass` exists but no startup/recovery caller was found.

### REQ-006 - Durable intent before every first wired release operation

- Source: `goals[1]`, `acceptance_criteria[2]`, `architecture.components`, `validation_plan[5]`
- Status: Partially Implemented
- Evidence: `control-plane/crates/engine/src/executor.rs:6732`, `:6737`, `:6767`, `:6774`, `:6923`, `:6959`, `:6966`
- Gap: Release execution prepares and leases rows for `git_push` and `connect_upload`, but the proposal requires first wired paths for `git_commit`, `git_push`, `build_archive`, and `connect_upload`. `GitReleaseService::commit_and_push(...)` is still invoked as one operation under a `git_push` effect, and `ConnectPublishService::build_and_distribute(...)` is still invoked under a `connect_upload` effect.

### REQ-007 - At most one external-write attempt per side_effect row

- Source: `acceptance_criteria[3]`, `idempotency_and_attempt_policy.external_write_rule`
- Status: Partially Implemented
- Evidence: `control-plane/crates/db/src/repos/side_effects.rs:952`, `control-plane/crates/engine/src/side_effects.rs:342`, DB tests in `control-plane/crates/db/tests/proposal_078_side_effects.rs:306`
- Gap: Code and tests exist, but the DB tests fail due migration collision, so this cannot be accepted as passing same-tree behavior.

### REQ-008 - Retry, targeted retry, cancellation, startup recovery, scheduler requeue preflight

- Source: `retry_recovery_and_circuit_breakers.preflight_scope`, `acceptance_criteria[6]`
- Status: Partially Implemented
- Evidence: `control-plane/crates/engine/src/command_handler.rs:2461`, `:2477`, `:4305`, `control-plane/crates/engine/src/side_effects.rs:471`, `:518`
- Gaps: `RetryStage` has ledger preflight plus heuristic guard. `RetryAgentExecution` evidence shows heuristic release-agent guard but not the same ledger-backed `retry_preflight_within_tx` check. `run_cancel_preflight` exists and is called, but startup recovery and scheduler requeue preflight were not found. Circuit breaker behavior for repeated ledger readback errors is not implemented.

### REQ-009 - MCP-only reconciliation command/control

- Source: `mcp_contract`, `acceptance_criteria[8]`, `acceptance_criteria[9]`
- Status: Partially Implemented
- Evidence: `control-plane/crates/mcp-server/src/tools/effects.rs:26`, `:138`, `:186`, `:237`, `:373`, `:466`, `:656`
- Gaps: Tool handlers exist, including operator dispositions and decision validation. But `effects.reconcile` is local ledger readback only (`readback_source: "local_ledger"`). It does not perform git `ls-remote`, upload/archive readback, evidence validation, reconciliation report creation, or settlement completion. `control-plane/crates/mcp-server/src/tools/mod.rs` also contains a syntax issue around added `effects.*` match arms.

### REQ-010 - GraphQL, reports, receipts, and SwiftUI read-only readback

- Source: `graphql_contract`, `ux_ui_notes`, `acceptance_criteria[10]`
- Status: Partially Implemented
- Evidence: `control-plane/crates/graphql-server/src/schema.rs:622`, `:899`
- Gaps: GraphQL exposes a top-level bounded `unresolved_side_effects(first)` query, but not the proposal's run-attached `blockedReason`, `unresolvedSideEffects`, `retryForbidden`, recommended MCP command, evidence summary, and reconciliation report availability. No GraphQL SDL/scalar tests were executed. Swift readback evidence is blocked by unmerged Swift files.

### REQ-011 - Startup recovery and watchdog classify unresolved effects

- Source: `reaper_and_cas_contract`, `acceptance_criteria[5]`, `validation_plan[6]`
- Status: Partially Implemented
- Evidence: `control-plane/crates/engine/src/side_effects.rs:241`, `control-plane/crates/db/src/repos/side_effects.rs:982`
- Gap: The watchdog/reaper function exists, but no caller was found from `control-plane/crates/engine/src/recovery.rs` or startup recovery paths. No crash/restart test reached execution.

### REQ-012 - Single barrier settlement transaction for ledger and canonical state

- Source: `architecture.coordinator_ownership`, historical Markdown section `16. Settlement transaction`, `acceptance_criteria[4]`, `acceptance_criteria[6]`
- Status: Missing
- Evidence: `control-plane/crates/db/src/repos/side_effects.rs:471`, `control-plane/crates/engine/src/executor.rs:6783`, `:6790`, `:6821`, `:6828`, `:6976`, `:6983`, `:7035`, `:7042`, `:7056`
- Gap: `executor_settle_cas` updates only side-effect rows and settlement rows. Release artifacts, agent execution status, stage status, projection rebuild, and queue advance happen afterward in separate calls. A crash after side-effect settlement but before artifact/canonical updates still leaves a cross-store inconsistency, which P078 was meant to close.

### REQ-013 - P075-style evidence spooling and high-volume evidence discipline

- Source: `evidence_spooling`, `acceptance_criteria[11]`
- Status: Partially Implemented
- Evidence: migration fields `expected_evidence_json`, `observed_evidence_summary_json`, `evidence_root`; MCP redacts `last_error` in `control-plane/crates/mcp-server/src/tools/effects.rs:16`
- Gaps: No implementation evidence was found for `{artifact_root}/evidence/side-effects/{effect_id}/`, `evidence-manifest.json`, fsync/rename ordering, checksum/size verification, disk backpressure refusal, or startup verification of partial evidence. Release intents currently pass `expected_evidence_json: None` and `evidence_root: None`.

### REQ-014 - Metrics, structured logs, rollout readback, and rollback observability

- Source: `metrics_and_logs`, `rollout_contract`, `rollout_contract_v1`
- Status: Partially Implemented
- Evidence: structured log strings such as `side_effect_transition`, `requires_effect_reconciliation_denied`, and `side_effect_cas_lost` exist in `control-plane/crates/engine/src/side_effects.rs`.
- Gaps: Required metric definitions such as `side_effect_intent_total`, `side_effect_retry_block_total`, `side_effect_unresolved`, and `p078_release_side_effects_with_durable_intent_percent` were not found in executable telemetry. Rollout fixture files are not validated by the P078 gate.

### REQ-015 - Required validation plan and same-tree gate evidence

- Source: `validation_plan`, successful-verdict rule in audit skill
- Status: Missing
- Evidence: `./scripts/test-gate.sh proposal-078` failed; `cargo check -p engine` failed.
- Gap: Same-tree canonical gate evidence is red. Domain P078 tests passed 10/10, but DB P078 tests failed 18/18 due migration collision. Engine compilation fails on conflict markers in `control-plane/crates/engine/src/executor.rs:86`.

## Reviewer / Lens Scorecard

| Lens | Conformance | Top Risk | Confidence |
|---|---|---|---|
| Rust architecture | Partial | Release effect boundaries do not match first wired operations; settlement is not a unified barrier | High |
| Rust reliability | Not Ready | Duplicate migration version, missing startup/recovery wiring, missing external readback reconciliation | High |
| API contract | Partial | GraphQL/MCP readback shape is incomplete and MCP module has source-level syntax/conflict issues | High |
| Observability/rollout | Not Ready | Gate fails; rollout fixtures and metrics are not enforced | High |
| Rust security | Partial | Operator-only MCP intent is present, but unbuildable code and incomplete redacted evidence/report path block trust | Medium |
| Readiness | Not Ready | Unresolved merge conflicts and failed canonical gate | High |

## Routed Specialist Findings

### READY-001 - Critical - Unresolved merge conflicts make the target unbuildable

- Reviewer: readiness
- Confidence: High
- Related REQs: REQ-001, REQ-015
- Evidence: `git status --short`, `rg "<<<<<<<|=======|>>>>>>>"`, `cargo check -p engine`
- Evidence references:
  - `control-plane/crates/engine/src/executor.rs:86`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:5`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:1546`
- Why it matters: The implementation cannot be merged, built, or audited as executable behavior while conflict markers remain.
- Recommended action: Resolve all unmerged files, then rerun `./scripts/test-gate.sh proposal-078`.
- Acceptance criteria: `git status --short` has no `UU` entries; no conflict markers under implementation/test files; engine and MCP compile.

### OPS-001 - Critical - P078 migration collides with existing migration version 049

- Reviewer: observability_rollout_reviewer
- Confidence: High
- Related REQs: REQ-001, REQ-003, REQ-015
- Evidence: migration files, gate output
- Evidence references:
  - `control-plane/crates/db/migrations/049_p078_side_effect_ledger.sql:1`
  - existing `control-plane/crates/db/migrations/049_p075_storage_write_pressure_window_key.sql`
  - `./scripts/test-gate.sh proposal-078`: DB P078 tests failed 18/18 with `UNIQUE constraint failed: _sqlx_migrations.version`
- Why it matters: Fresh DB migration cannot apply. Every repository/CAS test using migrated in-memory SQLite fails before exercising P078 behavior.
- Recommended action: Rename/resequence the P078 migration to the next unused version and update docs/gate references from the stale expected migration number.
- Acceptance criteria: SQLx migration preflight applies cleanly; `cargo test -p db proposal_078_` passes.

### ARCH-001 - Major - Release path does not create separate effects for all first wired operations

- Reviewer: rust_arch_reviewer
- Confidence: High
- Related REQs: REQ-006, REQ-013
- Evidence: code inspection
- Evidence references:
  - `control-plane/crates/engine/src/executor.rs:6732`
  - `control-plane/crates/engine/src/executor.rs:6774`
  - `control-plane/crates/engine/src/executor.rs:6923`
  - `control-plane/crates/engine/src/executor.rs:6966`
- Why it matters: The proposal explicitly wires `git_commit`, `git_push`, `build_archive`, and `connect_upload`. The implementation wraps commit+push as one `git_push` effect and archive+upload as one `connect_upload` effect, leaving crash windows and evidence ambiguity inside each composite adapter call.
- Recommended action: Split release adapters into primitive effect executions or explicitly revise the proposal before implementation closeout.
- Acceptance criteria: Fake-adapter tests prove separate durable intents and evidence for `git_commit`, `git_push`, `build_archive`, and `connect_upload`.

### REL-001 - Major - Reconciliation is local ledger inspection, not external readback/settlement

- Reviewer: rust_reliability_reviewer
- Confidence: High
- Related REQs: REQ-009, REQ-011, REQ-013
- Evidence: code inspection
- Evidence references:
  - `control-plane/crates/mcp-server/src/tools/effects.rs:237`
  - `control-plane/crates/mcp-server/src/tools/effects.rs:277`
  - `control-plane/crates/engine/src/recovery.rs:268`
- Why it matters: P078's safety claim depends on proving whether a push/upload/archive happened after a crash. Returning local ledger state cannot distinguish externally completed, missing, conflicting, or inaccessible effects.
- Recommended action: Implement readback strategies for git push, archive/upload, and evidence-manifest validation; have MCP reconciliation produce a report and complete settlement when evidence proves completion.
- Acceptance criteria: Tests cover remote branch match, remote conflict, inaccessible remote, upload evidence present/missing, and reconciliation report persistence without external mutation.

### REL-002 - Major - Side-effect settlement is not atomic with canonical release state

- Reviewer: rust_reliability_reviewer
- Confidence: High
- Related REQs: REQ-012
- Evidence: code inspection
- Evidence references:
  - `control-plane/crates/db/src/repos/side_effects.rs:471`
  - `control-plane/crates/engine/src/executor.rs:6783`
  - `control-plane/crates/engine/src/executor.rs:6790`
  - `control-plane/crates/engine/src/executor.rs:6821`
  - `control-plane/crates/engine/src/executor.rs:7042`
- Why it matters: The current order can settle the ledger and then crash before receipts, agent/stage state, projections, or queue advance are updated. That creates the inverse inconsistency of the original problem.
- Recommended action: Move release artifact metadata, runtime facts, agent/stage state, workflow cursor/queue advance, and projection invalidation into one settlement service or transactional boundary.
- Acceptance criteria: A crash-injection test proves no state where `side_effects.status='settled'` exists without the corresponding canonical release receipt/stage/agent/projection invalidation state.

### API-001 - Major - GraphQL readback contract is incomplete

- Reviewer: api_contract_reviewer
- Confidence: High
- Related REQs: REQ-010
- Evidence: code inspection
- Evidence references:
  - `control-plane/crates/graphql-server/src/schema.rs:622`
  - `control-plane/crates/graphql-server/src/schema.rs:899`
- Why it matters: A global unresolved-effects list does not provide the proposal's run-level `blockedReason`, retry legality, recommended MCP command, evidence summary, or reconciliation report availability. Thin clients cannot reliably present blocked-run state from this shape.
- Recommended action: Add the run-scoped read model and SDL tests promised by the proposal, or update the proposal to accept a global-only query.
- Acceptance criteria: GraphQL tests assert exact SDL/scalars and run-attached side-effect fields, with bounded list behavior and raw kind/status preservation.

### OPS-002 - Major - Rollout metrics/readback fixtures are present but not enforced

- Reviewer: observability_rollout_reviewer
- Confidence: High
- Related REQs: REQ-002, REQ-014, REQ-015
- Evidence: fixture files and gate script
- Evidence references:
  - `docs/evidence/rollout-contract/operator-readback/p078-full-surface.fixture.json`
  - `docs/evidence/rollout-contract/negative/p078-missing-side-effect-readback.json`
  - `scripts/test-gate.sh:6181`
- Why it matters: The proposal makes P084 rollout proof part of completion. The canonical P078 gate currently runs only cargo test slices and does not prove rollout contract fields, negative fixtures, metric/log cardinality budgets, or no-live-credential behavior.
- Recommended action: Extend the P078 gate with static rollout fixture validation and metric/log presence checks, or delegate to an existing rollout-contract gate with explicit P078 fixture coverage.
- Acceptance criteria: `./scripts/test-gate.sh proposal-078` fails if P078 rollout readback fixture, negative fixtures, metrics/log definitions, or no-live-side-effect assertions are missing.

## Readiness Checklist

| Item | Status | Notes |
|---|---|---|
| Canonical gate | Failed | `./scripts/test-gate.sh proposal-078` failed in DB tests |
| Build/compile | Failed | `cargo check -p engine` failed on conflict marker in `executor.rs` |
| Core service flow validation | Failed | Gate stopped before engine/MCP phases |
| Migration validation | Failed | Duplicate SQLx migration version `049` |
| Retry/cancel/recovery validation | Partial | RetryStage/cancel code exists; targeted retry/startup/scheduler coverage incomplete |
| MCP readback validation | Partial | Tool code exists; not reached by gate; reconcile is local ledger only |
| GraphQL contract validation | Partial | Top-level query exists; run-scoped contract and SDL tests missing |
| macOS read-only UI validation | Not Verifiable | Swift files are conflicted; no UI/snapshot/accessibility proof run |
| Accessibility/localization/privacy/permissions | Not Verifiable | No runnable UI proof in this tree |
| Rollout/rollback/telemetry | Partial | Fixtures exist but gate does not validate them; metrics not found |
| Full regression/canonical gate on audited tree | Failed | Required for any successful verdict, unavailable |

## Verification Log

| Command | Result |
|---|---|
| `git rev-parse HEAD` | `8fb2afa96f8fb2ce9c26ae612a46fa26308147c4` |
| `git status --short` | Dirty worktree with five `UU` unmerged files |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` | Selected this report path as R1 |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` | No prior review artifacts found |
| JSON parse of proposal file | Parsed `proposal_document_v1`, revision `p078-refined-2026-05-07-dfc2d583-r2`; no `Status` field |
| `./scripts/test-gate.sh proposal-078` | Failed. Domain P078 tests passed 10/10; DB P078 tests failed 18/18 due duplicate migration version `049` |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-078-audit-check cargo check -p engine` | Failed on conflict marker in `control-plane/crates/engine/src/executor.rs:86` |
| `rg "<<<<<<<|=======|>>>>>>>" ...` | Conflict markers found in Swift tests, engine executor, and MCP runs tool |
| Search for P078 rollout fixture references in gate/code | No executable gate references found for `p078-full-surface` or `p078-missing-side-effect-readback` |

## Final Verdict

P078 is not implemented to the current proposal contract and is not ready for handoff.

The implementation has meaningful pieces of the durable side-effect ledger: domain models, schema draft, coordinator code, retry preflight for `RetryStage`, release wrapping for two operations, MCP tool handlers, and a GraphQL list projection. However, the audited tree is unbuildable, the canonical P078 gate fails, the migration version collides, reconciliation does not perform external readback or settlement, startup recovery is not wired, release operations are not split into all first wired effects, and canonical settlement remains outside a single barrier transaction.

Recommended next actions:

1. Resolve merge conflicts and restore a buildable target.
2. Resequence the P078 migration away from duplicate version `049`, then rerun DB P078 tests.
3. Split release effects into `git_commit`, `git_push`, `build_archive`, and `connect_upload` with expected evidence and fake-adapter tests.
4. Implement external readback reconciliation, startup/reaper wiring, and settlement-report persistence.
5. Move canonical release state updates into a single settlement boundary.
6. Complete GraphQL/run-report/release-receipt/Swift readback and rollout contract gate validation.
7. Rerun `./scripts/test-gate.sh proposal-078` on the same resolved tree before requesting another implementation audit.
