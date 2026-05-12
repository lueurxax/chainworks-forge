# Proposal 078 Implementation Audit R2

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/078-durable-side-effect-ledger-release-settlement-and-reconciliation.md` |
| Proposal document shape | JSON payload in a `.md` file, `schema_version=proposal_document_v1` |
| Proposal revision | `p078-refined-2026-05-07-dfc2d583-r2` |
| Source review pass | `c4f604a1-6537-4027-acc5-876c70013226` recorded in proposal JSON, no local prior review artifacts found |
| Audit report | `docs/proposals/078-durable-side-effect-ledger-release-settlement-and-reconciliation_IMPLEMENTATION_AUDIT_R2.md` |
| Implementation target | Worktree `.chainworks/worktrees/cw-implement-proposal-078-durable-e49976db` |
| Branch | `cw/implement-proposal-078-durable/e49976db` |
| HEAD | `8fb2afa96f8fb2ce9c26ae612a46fa26308147c4` |
| Audit time | 2026-05-12 14:38:34 EEST |
| Overall Conformance | Not Implemented |
| Overall Implementation Readiness | Not Ready |
| Audit Confidence | High for Rust control-plane contract gaps; Medium for Swift/macOS readback gaps because only source search, not Swift gate/runtime, was run |
| Reviewer Selection Reuse | Not reused |

## Implementation Target

This R2 audit supersedes the R1 blocker profile for this same worktree. The prior R1 report found unresolved conflicts, a duplicate migration version, and a failing P078 gate. Those specific blockers are no longer present:

- `rg` found no conflict markers in the audited implementation surfaces.
- `git diff --check` returned clean.
- The P078 migration has been renamed to `control-plane/crates/db/migrations/052_p078_side_effect_ledger.sql`.
- `./scripts/test-gate.sh proposal-078` passed on this same worktree.
- `cargo check -p engine` passed on this same worktree.

The target remains a dirty implementation worktree with many modified files and several unrelated untracked escalation files. I did not treat unrelated files as P078 evidence unless they were directly referenced by the proposal contract.

## Prior Proposal-Review Reuse

Discovery command:

```bash
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py \
  "/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-078-durable-e49976db/docs/proposals/078-durable-side-effect-ledger-release-settlement-and-reconciliation.md"
```

Result: no local review artifacts were found. The proposal JSON includes `source_review_pass_id=c4f604a1-6537-4027-acc5-876c70013226` and `reviewer_feedback_resolution`, but without the prior reviewer-selection report I cannot reuse the selection.

Reuse status: `Not reused`.

## Selected Reviewers

- `rust_reliability_reviewer`: ledger lifecycle, retries, leases, crash/restart, CAS races.
- `rust_arch_reviewer`: crate boundaries, executor/DB/MCP integration, transaction authority.
- `api_contract_reviewer`: MCP and GraphQL read/write surfaces, typed envelopes, schema drift.
- `observability_rollout_reviewer`: P084 rollout contract, metrics, logs, gates, rollback/readback.
- `apple_arch_reviewer`: governed SwiftUI/read-model exposure and command/control exclusion.

Rejected close alternatives:

- `rust_security_reviewer`: auth-sensitive MCP redaction and operator gating are present, but security is not the dominant unresolved gap.
- `performance_reviewer`: no performance SLA or hot-path benchmark is central to P078 beyond bounded queries and limits.
- `product_reviewer`: operator value is affected, but the unresolved issues are primarily reliability/API/rollout contract failures.

## Proposal State And Contract Summary

Proposal state: `Ambiguous`. The file is not Markdown despite its `.md` suffix and does not include a `status` field. I treated the JSON as the current active contract because it is the checked-out proposal input.

Scope:

- Backend/service/data scope: Rust control-plane durable ledger, release executor, command retry/cancel/recovery path, MCP, GraphQL, SQLite migration, rollout contract.
- Apple/macOS scope: governed SwiftUI remains read-only; no command/control affordances for side-effect reconciliation.
- Non-goals: no live external pushes/uploads/notarization/UI smoke in the P078 gate; no exactly-once external side-effect guarantee; no broad P075 implementation beyond needed evidence pointers.

Primary implementation flows:

1. Release executor prepares durable intent, acquires a CAS lease, marks the external write started, performs `git_commit`, `git_push`, `build_archive`, or `connect_upload`, then settles with a receipt.
2. Retry, targeted retry, and cancellation query unresolved ledger state and fail closed with `requires_effect_reconciliation` before mutating canonical state.
3. Crash/restart/watchdog/reaper should move stale or partially observed effects to `needs_reconciliation` and block queue advancement until operator disposition.
4. Operator inspects and reconciles unresolved effects through MCP `effects.*` tools only.
5. Operators see side-effect lifecycle, retry-forbidden state, evidence summary, rollback/readback status, and next actions through run reports, release receipts, MCP, GraphQL, and governed SwiftUI read models.

## Proposal Fidelity Inventory

### Matches

- Domain model includes P078 effect kinds, wired/deferred kind split, status round-trips, terminal/unresolved status helpers, deterministic idempotency key derivation, and the feature flag constant.
- Migration `052_p078_side_effect_ledger.sql` creates `side_effects`, `side_effect_attempts`, and `side_effect_settlements`, with CHECK constraints, unique idempotency key, unresolved `(run_id, target_key)` uniqueness, and settlement/disposition uniqueness.
- Release executor now wires separate ledger rows for `git_commit`, `git_push`, `build_archive`, and `connect_upload`, and refuses release execution when `CHAINWORKS_RELEASE_SIDE_EFFECTS_ENABLED` is not enabled.
- Retry stage, targeted release retry, and run cancellation have ledger-backed fail-closed preflight checks.
- MCP exposes `effects.list`, `effects.inspect`, `effects.reconcile`, `effects.mark_unrecoverable`, and `effects.clear_after_manual_verification`.
- GraphQL exposes a bounded read-only `unresolved_side_effects(first)` projection.
- `proposal-078` and `p078` gate aliases exist and the canonical P078 gate passes locally without live external side effects.

### Divergences

- The proposal JSON still names migration `046_p078_side_effect_ledger.sql`; the implementation uses `052_p078_side_effect_ledger.sql`. Given existing P075 migration numbering, `052` may be the right repo sequencing, but the proposal and implementation are no longer textually aligned.
- `effects.mark_conflict` exists as an internal handler but is deliberately not public; the proposal acceptance criteria say MCP reconciliation can mark conflict.
- `prepare_and_lease` does not set `deadline_at`, and no engine caller invokes `lease_renew_cas`. Long `build_archive` and `connect_upload` operations receive a 60-second lease TTL but have proposal deadlines of 7200 and 3600 seconds.
- `watchdog_pass` exists but I found no scheduler/startup caller. The proposal requires startup recovery and watchdog/reaper behavior, not only a callable helper.
- Side-effect evidence roots are set to `None` for all four wired release operations, and release executor settlement uses ordinary JSON artifacts as receipts rather than P078 evidence manifests under `{artifact_root}/evidence/side-effects/{effect_id}/`.
- The P078 gate does not execute rollout-contract validation for the P078 `rollout_contract_v1` fixtures or assert the proposal's operational metric/log definitions.
- Swift/macOS read-only side-effect lifecycle surfaces are not visible in the implementation search results; Swift references are legacy mutation-side-effect supervision and ResumeManager heuristics, not the P078 ledger read model.

### Ambiguities / Evidence Gaps

- The proposal is a one-line JSON document, so source anchors are key names rather than Markdown line references.
- I did not run the full Swift app gate or UI tests. Because the implementation is not ready by Rust/API/rollout evidence, this audit did not attempt a release-grade full regression roll-up.
- The target worktree is dirty and includes unrelated untracked escalation files; unrelated changes were ignored for conformance.

## Requirement Summary

| ID | Requirement | Status |
| --- | --- | --- |
| REQ-001 | Add durable side-effect ledger schema and domain lifecycle model | Implemented |
| REQ-002 | Support deterministic idempotency keys, request fingerprints, and wired/deferred effect kinds | Implemented |
| REQ-003 | Persist intent before every wired external operation executes | Partially Implemented |
| REQ-004 | Enforce one external-write attempt per row and mutually exclusive executor/reaper CAS | Partially Implemented |
| REQ-005 | Fail closed before retry, targeted retry, cancellation, startup recovery, scheduler requeue, and queue advancement | Partially Implemented |
| REQ-006 | Crash/restart/watchdog recovery produces `needs_reconciliation` and blocks ordinary retry | Missing |
| REQ-007 | MCP `effects.*` command/control surface supports inspect, reconcile, conflict, unrecoverable, clear, and idempotent disposition | Partially Implemented |
| REQ-008 | GraphQL remains read-only and exposes bounded lifecycle/readback projection | Partially Implemented |
| REQ-009 | Release receipts, run reports, MCP, GraphQL, and governed SwiftUI expose side-effect lifecycle and operator next actions | Partially Implemented |
| REQ-010 | Preserve P075 evidence discipline: spool large evidence to files with manifest/checksum/size pointers | Missing |
| REQ-011 | Provide P084 rollout-contract coverage, fixtures, metrics, logs, rollback disposition, and negative fixtures | Partially Implemented |
| REQ-012 | Validate locally without live git pushes, Connect uploads, notarization, simulator runs, or UI smoke tests | Implemented |

## Detailed REQ Audit

### REQ-001: Ledger Schema And Lifecycle Model

Status: `Implemented`

Evidence:

- `control-plane/crates/db/migrations/052_p078_side_effect_ledger.sql:11` creates `side_effects`.
- `control-plane/crates/db/migrations/052_p078_side_effect_ledger.sql:79` creates `side_effect_attempts`.
- `control-plane/crates/db/migrations/052_p078_side_effect_ledger.sql:106` creates `side_effect_settlements`.
- `control-plane/crates/domain/src/side_effect.rs` defines effect kinds, statuses, unresolved/terminal helpers, lease/deadline constants, and the feature flag key.
- `./scripts/test-gate.sh proposal-078` passed domain and DB P078 tests.

Note: proposal JSON still names migration `046_p078_side_effect_ledger.sql`; implementation and `docs/reference/test-gates.md` now use migration 052.

### REQ-002: Idempotency, Fingerprints, Wired And Deferred Kinds

Status: `Implemented`

Evidence:

- Domain kind split: `GitCommit`, `GitPush`, `BuildArchive`, and `ConnectUpload` are wired; `TagCreate` and `ArtifactPublish` remain deferred.
- Deterministic idempotency derivation exists in `domain/src/side_effect.rs:359` and release-local derivation exists in `engine/src/executor.rs:12955`.
- Release executor computes separate idempotency keys and request fingerprints for all four wired release operations.
- P078 domain tests for idempotency stability, SHA-256 format, and kind distinction passed.

### REQ-003: Durable Intent Before Wired External Writes

Status: `Partially Implemented`

Evidence:

- `commit_and_push_to_github` prepares and leases `git_commit` before `commit_changes`, then prepares and leases `git_push` before `push_commit`.
- `build_archive_and_push_connect` prepares and leases `build_archive` before `build_archive`, then prepares and leases `connect_upload` before `upload_archive`.
- Each path calls `mark_write_started` before invoking the external adapter.

Gap:

- All four `PrepareEffectIntent` values set `expected_evidence_json: None`, `evidence_root: None`, and `deadline_at: None`. The proposal requires expected evidence/readback linkage, evidence roots, and deadlines as part of the durable intent model.

### REQ-004: Single External Attempt And CAS Race Safety

Status: `Partially Implemented`

Evidence:

- `executor_start_cas` requires `status='prepared'` and `external_write_attempted=0` before inserting the attempt row.
- `mark_external_write_started` requires `status='executing'`, owner match, and `external_write_attempted=0`.
- `executor_settle_cas_tx` and `reaper_transition_cas` implement mutually exclusive predicates.
- DB tests cover one-winner start CAS, single external attempt, second settlement blocking, and executor/reaper race behavior.

Gap:

- `lease_renew_cas` exists only as a repository function; no engine path calls it.
- `prepare_and_lease` sets `deadline_at: None`, despite proposal deadlines of 7200 seconds for `build_archive` and 3600 seconds for `connect_upload`.
- Because `executor_settle_cas_tx` requires a live lease for `executing`, long release work can expire the 60-second lease and then fail settlement even though the external work may have completed.

### REQ-005: Fail-Closed Retry, Targeted Retry, Cancel, Recovery, Scheduler

Status: `Partially Implemented`

Evidence:

- `RetryStage` calls `retry_preflight_within_tx` before the retry mutation.
- `CancelRun` calls `run_cancel_preflight` before canonical cancellation settlement.
- P078-focused engine tests cover retry release stage, manual release gate retry, and targeted release retry failing closed.

Gaps:

- I found no startup recovery queue-advancement integration for P078 unresolved effects.
- I found no scheduler requeue integration for P078 unresolved effects.
- The proposal's ledger readback circuit breaker after repeated readback errors is not implemented.
- Metrics such as `side_effect_retry_block_total` and `side_effect_ledger_readback_error_total` are not implemented as metrics, only some warning log strings exist.

### REQ-006: Crash/Restart And Watchdog Recovery

Status: `Missing`

Evidence:

- `DurableEffectCoordinator::watchdog_pass` exists and uses `list_expired_executing` plus `reaper_transition_cas`.
- Search found no caller outside tests/helper code for `watchdog_pass`.
- Search found no P078 startup recovery flow that scans unresolved/stale side effects and blocks queue advancement.

Gap:

- The proposal requires crash/restart between external effect and settlement to produce `needs_reconciliation` or equivalent fail-closed readback, and startup recovery must preserve canonical state. A helper function without daemon scheduling/startup integration does not satisfy that contract.

### REQ-007: MCP Reconciliation Command/Control

Status: `Partially Implemented`

Evidence:

- Public capability IDs cover `effects.list`, `effects.inspect`, `effects.reconcile`, `effects.mark_unrecoverable`, and `effects.clear_after_manual_verification`.
- `effects.reconcile` is read-only and writes/returns a reconciliation report path.
- Disposition idempotency and payload mismatch behavior are implemented and tested.
- Public effects tools are operator-gated through the MCP server capability path.

Gaps:

- The proposal acceptance criteria include "mark conflict"; implementation has `handle_effects_mark_conflict`, but tests assert `effects.mark_conflict` is not publicly callable.
- `effects.reconcile` remains primarily local ledger/evidence-root readback. It does not implement concrete external readback for git remote state, Connect upload state, or archive evidence beyond local evidence paths.

### REQ-008: GraphQL Read-Only Projection

Status: `Partially Implemented`

Evidence:

- `unresolved_side_effects(first)` is bounded 1..100 and read-only.
- `GqlSideEffectSummary` exposes raw kind/status strings and `retry_forbidden`.
- Tests assert GraphQL does not expose an effects mutation.

Gaps:

- The proposal requires typed bounded `EvidenceSummary`; GraphQL summary exposes no evidence summary, evidence root, readback source, report path, blocked reason, or operator next-action detail beyond a constant recommended MCP tool string.
- The projection is top-level only; it is not clearly attached to run reports or governed run rows.

### REQ-009: Release Receipts, Run Reports, MCP, GraphQL, SwiftUI Read Models

Status: `Partially Implemented`

Evidence:

- MCP exposes side-effect tools.
- GraphQL exposes `unresolved_side_effects`.
- Release executor links settlement rows to receipt artifact IDs for successful release steps.

Gaps:

- Release receipt content was not updated to carry the full side-effect lifecycle/readback contract.
- `mcp-server/src/tools/runs.rs` exposes rollout-contract readback but no P078 side-effect lifecycle summary in run report output.
- Swift/macOS searches found legacy mutation-side-effect supervision and ResumeManager heuristics, not a governed P078 read-only ledger projection.
- No evidence was found for accessibility/view scans proving absence of forbidden command controls.

### REQ-010: P075 Evidence Spooling Discipline

Status: `Missing`

Evidence:

- P078 tables include evidence pointer columns.
- MCP reconciliation can create a file-backed report under an existing `evidence_root` or fallback path.

Gaps:

- The four wired release intents set `evidence_root: None`.
- There is no P078 evidence manifest generation with `stdout.log`, `stderr.log`, `git-ls-remote.json`, `upload-readback.json`, `archive-summary.json`, checksums, sizes, fsync ordering, and manifest-last semantics.
- Startup recovery does not verify `evidence-manifest.json`, referenced evidence files, checksum mismatch, size mismatch, or partial fsync evidence for P078.
- Storage backpressure behavior for side-effect evidence is not implemented.

### REQ-011: Rollout Contract, Fixtures, Metrics, Logs, Rollback

Status: `Partially Implemented`

Evidence:

- Proposal JSON includes `rollout_contract_v1`.
- P078 readback and negative fixtures exist:
  - `docs/evidence/rollout-contract/operator-readback/p078-full-surface.fixture.json`
  - `docs/evidence/rollout-contract/negative/p078-missing-side-effect-readback.json`
- `proposal-078|p078` gate aliases are registered and documented.

Gaps:

- `scripts/test-gate.sh proposal-078` only runs Rust cargo tests; it does not run rollout-contract validation against P078's `rollout_contract_v1`, readback fixture, or negative fixture.
- Operational metrics listed in the proposal are not implemented as emitted metrics.
- Log coverage is partial; some structured warning strings exist, but not the full metric/log contract.
- The gate does not scan governed SwiftUI for forbidden side-effect command controls.

### REQ-012: Local Validation Without Live External Side Effects

Status: `Implemented`

Evidence:

- The P078 gate comment explicitly forbids live git pushes, Connect uploads, notarization, production daemon startup, simulator runs, and UI smoke tests.
- The same-tree P078 gate passed locally. The release tests use local temporary git repositories and fake/local release paths.

## Reviewer / Lens Scorecard

| Lens | Conformance | Top Risk | Confidence |
| --- | --- | --- | --- |
| Rust reliability | Partial | Lease renewal/deadline/watchdog/startup recovery are not wired end-to-end | High |
| Rust architecture | Partial | Durable ledger exists, but recovery/settlement authority is incomplete across executor, daemon, and report surfaces | High |
| API contract | Partial | MCP conflict disposition and GraphQL evidence/readback projection do not satisfy the full contract | High |
| Observability/rollout | Partial | P078 gate passes but does not validate rollout contract, metrics, logs, or Swift forbidden-control scans | High |
| Apple/macOS read model | Not verifiable / likely missing | No P078 governed SwiftUI read-only lifecycle surface found | Medium |
| Readiness | Not Ready | Passing P078 gate is necessary but not sufficient; explicit contract gaps remain | High |

## Routed Specialist Findings

### REL-001: Long Release Operations Can Outlive The 60-Second Lease Without Renewal

Reviewer: `rust_reliability_reviewer`
Severity: `Critical`
Confidence: `High`
Related requirements: `REQ-004`, `REQ-006`

Evidence:

- `EffectKind::BuildArchive` and `EffectKind::ConnectUpload` use 60-second lease TTLs but proposal deadlines of 7200 and 3600 seconds.
- `prepare_and_lease` sets `deadline_at: None`.
- `lease_renew_cas` exists only in the DB repository; no engine caller invokes it.
- Release executor settlement passes `observed_lease_renewed_at: <lease>.lease_acquired_at` for all four operations.

Why it matters:

The proposal's long-running external operations are expected to survive for minutes or hours under a renewable lease. In the current implementation, a build or upload that takes longer than 60 seconds can complete externally but lose settlement CAS because the lease expired. That is exactly the crash/duplicate/reconciliation risk P078 is meant to control.

Recommended action:

Wire a lease renewal task around release operations, set `deadline_at` from `EffectKind::deadline_seconds()`, and add a fake long-running build/upload test that proves renewal allows settlement while an expired unrenewed lease is reaped.

Acceptance criteria:

- Long-running `build_archive` and `connect_upload` fake tests pass beyond one TTL.
- Lost renewal self-aborts without a second external write.
- Reaper and executor settlement remain one-winner under renewal races.

### REL-002: Watchdog And Startup Recovery Are Helpers, Not A Running Recovery System

Reviewer: `rust_reliability_reviewer`
Severity: `Critical`
Confidence: `High`
Related requirements: `REQ-005`, `REQ-006`

Evidence:

- `DurableEffectCoordinator::watchdog_pass` exists.
- Search found no production caller for `watchdog_pass`.
- Search found no P078 startup recovery flow that scans unresolved side effects before queue advancement or scheduler requeue.

Why it matters:

The proposal explicitly targets crash/restart between external write and settlement. Without daemon startup integration and a running reaper/watchdog, stale `executing` rows may not be moved into operator reconciliation, and startup queue advancement is not proven fail-closed.

Recommended action:

Integrate a P078 startup recovery pass into the daemon/engine recovery path and schedule the watchdog at the proposal's interval. Add tests for restart with stale `executing`, prepared-without-attempt, and externally-observed records.

Acceptance criteria:

- Startup recovery blocks queue advancement when unresolved side effects exist.
- Stale executing effects transition to `needs_reconciliation`.
- Prepared-without-attempt cancellation/recovery follows the P078 policy.

### REL-003: Retry/Cancellation Preflight Is Incomplete For Circuit Breaker And Scheduler Scope

Reviewer: `rust_reliability_reviewer`
Severity: `Major`
Confidence: `High`
Related requirements: `REQ-005`

Evidence:

- `RetryStage` and `CancelRun` preflights are implemented.
- No implementation was found for the proposal's repeated `ledger_readback_error` circuit breaker.
- No scheduler requeue or startup queue advancement preflight was found.
- Metrics for retry blocks and ledger readback errors are absent.

Why it matters:

Fail-closed retry and cancel are only part of the proposal. Scheduler and startup mutation paths can still advance canonical state unless they are explicitly guarded. Repeated ledger read failures also need the proposed circuit breaker so operators can distinguish transient DB errors from safe retry state.

Recommended action:

Add call-site coverage for scheduler requeue and startup recovery, implement the readback-error circuit breaker, and emit the proposed metrics.

Acceptance criteria:

- Tests fail before canonical mutation on scheduler/startup unresolved-effect cases.
- Three repeated readback errors open the fail-closed circuit breaker.
- `side_effect_retry_block_total` and `side_effect_ledger_readback_error_total` are emitted or otherwise queryable by the repo's metrics system.

### API-001: MCP Conflict Disposition Is Not Public Despite The Proposal Acceptance Criteria

Reviewer: `api_contract_reviewer`
Severity: `Major`
Confidence: `High`
Related requirements: `REQ-007`

Evidence:

- The proposal acceptance criteria say MCP reconciliation can "mark conflict".
- Implementation has `handle_effects_mark_conflict`.
- Tests assert `effects.mark_conflict` has no `CapabilityToolId` and is not publicly callable.

Why it matters:

Operators need a durable conflict disposition when readback evidence contradicts the expected outcome. If conflict is intentionally not public, the proposal must be amended; otherwise the implementation omits a required operator action.

Recommended action:

Either expose `effects.mark_conflict` with the same operator/auth/idempotency rules as other disposition tools, or update the proposal contract and acceptance criteria before closeout.

Acceptance criteria:

- Public MCP capability behavior matches the proposal.
- Disposition idempotency tests cover conflict or the proposal explicitly removes conflict as a command.

### API-002: GraphQL And Run Report Readback Are Too Thin For The Required Operator Surface

Reviewer: `api_contract_reviewer`
Severity: `Major`
Confidence: `High`
Related requirements: `REQ-008`, `REQ-009`

Evidence:

- GraphQL exposes only `GqlSideEffectSummary` fields: IDs, kind/status raw strings, target key, external attempt flag, last error kind, recommended tool, retry forbidden, timestamps.
- No GraphQL evidence summary, readback source, report path, blocked reason, or operator next-action detail was found.
- `mcp-server/src/tools/runs.rs` exposes rollout-contract readback, not a P078 side-effect lifecycle summary.

Why it matters:

P078 is explicitly an operator trust/readback proposal. A top-level list of unresolved rows is useful, but it does not satisfy the committed release receipts/run reports/GraphQL/Swift read model contract.

Recommended action:

Add a bounded typed evidence summary and next-action/readback fields to GraphQL and run reports, and connect them to release receipt and governed UI read models.

Acceptance criteria:

- GraphQL exposes `effectKindRaw`, `statusRaw`, typed bounded evidence/readback summary, retry-forbidden state, recommended MCP tool, and report availability.
- Run report output includes unresolved side-effect lifecycle and operator next actions for affected runs.

### OPS-001: P078 Rollout Contract Exists But Is Not Enforced By The P078 Gate

Reviewer: `observability_rollout_reviewer`
Severity: `Major`
Confidence: `High`
Related requirements: `REQ-011`

Evidence:

- Proposal JSON includes `rollout_contract_v1`.
- P078 full-surface and negative rollout fixtures exist.
- `scripts/test-gate.sh proposal-078` only runs Rust cargo tests for domain, db, engine, release, and mcp-server.
- Search found no `p078-full-surface` or `p078-missing-side-effect-readback` validation in the P078 gate.

Why it matters:

The proposal made rollout-contract coverage an acceptance criterion. The current passing gate can pass even if the P078 operator readback fixture or negative fixture is stale, incomplete, or disconnected from implementation.

Recommended action:

Extend `proposal-078` to execute the rollout-contract preflight against the P078 proposal/fixtures and fail on missing readback fields, missing negative fixture behavior, or forbidden command/control drift.

Acceptance criteria:

- `proposal-078` fails if `p078-full-surface.fixture.json` loses required readback fields.
- `proposal-078` fails if the P078 negative fixture no longer detects missing side-effect readback.
- Gate documentation states that rollout-contract validation is part of the P078 proof path.

### OPS-002: Operational Metrics Are Named But Not Implemented

Reviewer: `observability_rollout_reviewer`
Severity: `Major`
Confidence: `High`
Related requirements: `REQ-011`

Evidence:

- Proposal lists metrics including `p078_release_side_effects_with_durable_intent_percent`, `side_effect_intent_total`, `side_effect_transition_total`, `side_effect_retry_block_total`, `side_effect_settlement_latency_seconds`, `side_effect_unresolved`, and others.
- Search found those metric names only in proposal/fixtures or warning strings, not in emitted metric code.

Why it matters:

The rollback and operator decision model depends on measurable adoption, retry blocks, unresolved count/age, readback errors, evidence storage, and prepare denials. Without actual metrics, the rollout contract cannot support an operational decision.

Recommended action:

Wire metrics at prepare, transition, retry-block, settlement, unresolved-age, recovery, evidence-spool, and prepare-denied points.

Acceptance criteria:

- Metrics are emitted or queryable through the repo's established telemetry path.
- P078 tests assert at least the critical counters/gauges are updated for prepare, retry block, transition, and recovery.

### REL-004: Evidence Spooling Contract Is Not Implemented For Side Effects

Reviewer: `rust_reliability_reviewer`
Severity: `Critical`
Confidence: `High`
Related requirements: `REQ-010`

Evidence:

- All wired release intents set `evidence_root: None`.
- The P078 migration supports evidence pointer fields, but release execution does not populate a P078 evidence manifest.
- MCP reconcile can write a local report, but startup recovery does not validate manifest/checksum/size/partial fsync evidence.

Why it matters:

The ledger is useful only if operators can prove what happened outside the database after a crash. The proposal requires durable evidence files and compact DB pointers; current behavior records ledger rows and receipt artifacts, but not the P078 evidence-spooling contract.

Recommended action:

Create side-effect evidence roots per effect, spool required evidence files with manifest/checksum/size metadata, and make recovery verify the manifest before settlement or reconciliation.

Acceptance criteria:

- Each wired release effect has a populated `evidence_root`.
- Evidence manifest is written last and verified during recovery.
- Missing/corrupt/partial evidence transitions affected effects to `needs_reconciliation`.

### UI-001: Governed SwiftUI Read-Only P078 Surface Is Not Proven

Reviewer: `apple_arch_reviewer`
Severity: `Major`
Confidence: `Medium`
Related requirements: `REQ-009`, `REQ-011`

Evidence:

- Swift searches found legacy mutation-side-effect supervision and ResumeManager side-effect-state heuristics.
- No Swift model/view/readback surface for P078 ledger lifecycle, retry-forbidden state, evidence summary, or recommended MCP action was found.
- No Swift/view scan or UI test was run by the P078 gate.

Why it matters:

The proposal explicitly says governed SwiftUI must remain read-only while exposing lifecycle and operator next actions. Without a visible read model and a forbidden-command scan, the operator shell cannot be considered proposal-compliant.

Recommended action:

Add a read-only Swift model/projection for unresolved side effects and operator next actions, with tests proving no command/control affordances are exposed.

Acceptance criteria:

- Swift read model decodes P078 side-effect summaries.
- UI/view tests or source scans prove no reconcile/push/upload/manual-settlement controls exist in governed SwiftUI.

### READY-001: Passing The P078 Gate Is Necessary But Not Sufficient For Closeout

Reviewer: `observability_rollout_reviewer`
Severity: `Major`
Confidence: `High`
Related requirements: all

Evidence:

- Same-tree `./scripts/test-gate.sh proposal-078` passed.
- Same-tree `cargo check -p engine` passed.
- Full Swift gate and full repo regression were not run.
- Several explicit P078 requirements remain missing or partial.

Why it matters:

R2 confirms substantial progress: the implementation now builds and the focused P078 cargo gate passes. But proposal closeout would convert design intent into repository truth; doing that now would retire missing recovery, evidence, readback, and rollout obligations.

Recommended action:

Keep P078 open until the missing requirements above are implemented and covered by the canonical gate. After that, run the full relevant regression path before closeout.

Acceptance criteria:

- No `REQ-*` item is `Missing` or `Partially Implemented`.
- `proposal-078` covers rollout fixtures, recovery/watchdog, evidence spooling, metrics, and Swift read-only scans.
- Full relevant same-tree gate evidence is available before closeout.

## Verification Log

Commands run from `.chainworks/worktrees/cw-implement-proposal-078-durable-e49976db`:

| Check | Result |
| --- | --- |
| Resolve worktree/branch/HEAD/status | Passed; branch `cw/implement-proposal-078-durable/e49976db`, HEAD `8fb2afa96f8fb2ce9c26ae612a46fa26308147c4` |
| Report path helper | Returned `_IMPLEMENTATION_AUDIT_R2.md` |
| Prior review discovery helper | Returned no local artifacts |
| Proposal JSON extraction | Parsed `schema_version=proposal_document_v1`, revision `p078-refined-2026-05-07-dfc2d583-r2` |
| Conflict marker search | No merge conflict markers found in audited surfaces |
| `git diff --check` | Passed |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-078-audit-check cargo check -p engine` | Passed with warnings only |
| `./scripts/test-gate.sh proposal-078` | Passed |

P078 gate observed coverage:

- Domain P078 tests: 10 passed.
- DB P078 tests: 18 passed.
- Engine lib P078 tests: 5 passed.
- Engine `proposal_058_claim_start` P078 retry tests: 3 passed.
- Engine release tests: 12 passed.
- MCP server lib effects redaction tests: 7 passed.
- MCP server P078 effects tool tests: 10 passed.

## Final Verdict

The implementation is a substantial partial implementation and is materially improved from R1. It now has a durable ledger schema, wired release-side effect rows for the four first paths, retry/cancel fail-closed checks, MCP tools, GraphQL read-only basics, and a passing focused P078 gate.

It still does not close Proposal 078. The remaining gaps are not polish: lease renewal/deadline behavior, startup recovery/watchdog wiring, side-effect evidence spooling, rollout-contract enforcement, metrics, conflict disposition public API alignment, richer GraphQL/run-report/Swift readback, and governed SwiftUI absence-of-command proof are all explicit parts of the proposal contract.

Formal roll-up: `Overall Conformance = Not Implemented`; `Overall Implementation Readiness = Not Ready`.
