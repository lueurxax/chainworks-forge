# P083 Implementation Audit R9 - Execution-Truth Ownership and Invariant Model

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/083-execution-truth-ownership-invariant-model.md` |
| Proposal id / revision | `P083` / `P083-r70-refined-r69-score-lift` |
| Audit report | `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R9.md` |
| Audit date | 2026-06-21 03:10 EEST |
| Implementation target | Current dirty working tree at `0e6482c82b588b74a76294a225e68286bfe37fa4` |
| Proposal file state | Modified in working tree during audit |
| Prior proposal-review reuse | Not reused: `discover_prior_review.py` found no sidecar proposal-review artifacts for this proposal path |
| Prior implementation audit consulted | R8, for stale-vs-current comparison only |
| Canonical gate | `./scripts/test-gate.sh proposal-083` passed |
| Security-sensitive diff | Triggered; manual pass completed for P083 auth, ingress, schema/parser, process-boundary, and redaction surfaces |

## Executive Verdict

Overall conformance: **Partially Implemented**.

Readiness: **Not Ready**.

The current tree implements a large share of P083. The canonical P083 gate passed, all eight physical migration files are present, GraphQL and MCP route lifecycle commands through the command handler with `CallerRequestId`, rollback/set-enforcement target values are wired through public APIs and idempotency, shutdown/identity-hold paths are durable, and the macOS Run menu/toolbar proof is present.

The proposal still cannot be closed out. The side-effect force-reconcile contract is internally inconsistent and not fully matched by implementation: the proposal SDL/MCP inventory omits `decision_json`, while implementation/reference schemas require it; the proposal GraphQL success shape still names `reconciliationId`, while the implementation returns `journal_id` and `request_id`; and the idempotency hash uses the internal key `effect_id` where the proposal contract says `side_effect_id`. Mandatory hardening items are also not fully closed: schema-version evolution policy evidence is missing, the H007/H011 command policy tables exclude `provider_session.mark_process_absent`, and H008 lacks the requested concurrent late-output writer proof. These are in-scope requirements under the proposal closure policy and no approved successor proposal was found to own them.

## Proposal State

State classification: **Implementation in progress; closeout blocked**.

Current proposal text permits implementation work (`implementation_may_start: true`) but says Ready/closeout requires a fresh aggregate implementation review against this revision with `decision=approve`, `blocker_count=0`, and current-revision evidence (`docs/proposals/083-execution-truth-ownership-invariant-model.md:43-46`). This audit is current-revision evidence, but it does not return an approve/zero-blocker result.

## Primary Flows Audited

1. Lifecycle GraphQL/MCP ingress using non-null `CallerRequestId`, principal mutation policy, and command idempotency.
2. Rollback execution and enforcement-mode changes carrying caller-supplied target modes through API, intent hash, audit, and readback.
3. Provider-session shutdown, durable cancellation intent, identity-ambiguous hold, and manual process-absent recovery.
4. Post-cancel late-output ignored-settlement, overflow latch persistence, and active projection protection.
5. P083 migration/readback/rollout evidence, metrics, and canonical gate aliases.
6. macOS read-only operator affordances for lifecycle commands and manual identity-check recovery.

## Reviewer Selection and Reuse

Reviewer artifacts reused: **None**. The prior-review discovery helper returned no proposal-review sidecar artifacts. The embedded R69 review basis was treated as proposal context only.

Selected reviewer lenses:

| Reviewer | Selected | Reason |
| --- | --- | --- |
| `chainworks_execution_truth_reviewer` | Yes | Repo-local durable execution-truth reviewer; P083 changes command ownership, recovery, projections, and readback. |
| `api_contract_reviewer` | Yes | GraphQL SDL, MCP schemas, denial/enums, and readback are explicit P083 contracts. |
| `rust_reliability_reviewer` | Yes | Command idempotency, shutdown, retry policy, monotonic deadlines, and late-output behavior are reliability-critical. |
| `rust_security_reviewer` | Yes | Security-sensitive diff triggered on auth, ingress, parser/schema, process-boundary, and redaction categories. |
| `observability_rollout_reviewer` | Yes | P083 defines rollout holds, migration descriptors, metrics, gate aliases, and rollback disposition. |

Not selected because of the five-reviewer cap:

| Reviewer | Disposition | Residual risk |
| --- | --- | --- |
| `macos_ui_reviewer` | Displaced | Manual identity banner and menu/toolbar were statically reviewed only. |
| `apple_ux_reviewer` | Displaced | No interactive accessibility or action-state proof was run. |
| `apple_arch_reviewer` | Displaced | SwiftData lifecycle-boundary claims were not independently built or runtime-tested. |
| `rust_performance_reviewer` | Displaced | The fingerprint helper triggered performance, but P083 has no primary throughput/latency acceptance path. |

Specialist coverage result: **Insufficient for Ready** because macOS UI/SwiftData scope remains only statically covered.

## Security-Sensitive Diff Scan

Result: **Triggered and manually reviewed**.

Reviewed surfaces included GraphQL lifecycle mutations, MCP lifecycle schemas/handlers, `CallerRequestId` validation, operator principal checks, provider-session shutdown, identity-ambiguous process handling, process-absent recovery, late-output overflow rows, and MCP/GraphQL denial output shaping. I did not confirm a new Critical or Major security vulnerability in the inspected P083 paths. The remaining blockers are API/reliability/hardening conformance blockers, not a confirmed auth bypass or secret exposure.

## Fidelity Summary

Implemented or strongly evidenced:

- `./scripts/test-gate.sh proposal-083` passed.
- The proposal evidence corpus check passed with 112 declared paths verified.
- DB migration test suite passed: 57 tests, 0 failures.
- Focused engine P083 tests passed: 21 P083/shutdown unit tests, 0 failures.
- GraphQL and MCP compile checks passed.
- Domain denial-code round-trip, GraphQL approval mutation, MCP P083 validation tests, and rollout-contract lint passed.
- All eight physical P083 migration files are present and readback descriptors include their filenames (`scripts/test-gate.sh:9591-9623`).
- GraphQL exposes `CallerRequestId`, `DenialPayload`, `DenialReason`, and P083 lifecycle payload unions (`control-plane/crates/graphql-server/src/types/p083.rs:13-103`, `control-plane/crates/graphql-server/src/schema.rs:8294-8349`).
- MCP P083 inputs use Draft 2020-12 schemas with `additionalProperties:false` and required `caller_request_id` fields, including rollback target and mark-process-absent schemas (`control-plane/crates/mcp-server/src/tools/runs.rs:380-545`).
- Provider-session shutdown validates caller id, uses command idempotency, records durable cancellation/signal facts, and handles identity-ambiguous holds (`control-plane/crates/engine/src/command_handler.rs:7350-8035`).
- `provider_session.shutdown` no longer hashes diagnostic `reason`; its hash is based on `command` and `provider_session_id` only (`control-plane/crates/engine/src/command_handler.rs:7370-7382`).
- Late-output ignored settlement keeps active projection unchanged in executor code and integration coverage (`control-plane/crates/engine/src/executor.rs:15677-15690`, `control-plane/crates/engine/tests/integration.rs:17989-18019`).

Material divergences and gaps:

- Proposal GraphQL SDL lists `sideEffectsForceReconcile(sideEffectId, callerRequestId)` and `SideEffectsForceReconcileSuccess { sideEffectId reconciliationId }`, but implementation requires `decision_json` and returns `side_effect_id`, `journal_id`, and `request_id` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:173-181`, `control-plane/crates/graphql-server/src/schema.rs:6334-6361`, `control-plane/crates/graphql-server/src/types/p083.rs:893-900`).
- Proposal MCP inventory lists `side_effects.force_reconcile.required_input` without `decision_json`, while reference/runtime schemas require it (`docs/proposals/083-execution-truth-ownership-invariant-model.md:195-210`, `docs/reference/mcp/p083/side_effects.force_reconcile.input.schema.json:21-25`, `control-plane/crates/mcp-server/src/tools/runs.rs:456-486`).
- `side_effects.force_reconcile` intent hash uses key `effect_id`, not proposal logical field `side_effect_id` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1125-1136`, `control-plane/crates/engine/src/command_handler.rs:9170-9180`).
- H007/H011 policy tables and tests cover eight commands and omit `provider_session.mark_process_absent`, while the proposal command contract covers nine commands including it (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1082-1087`, `:1108-1117`, `control-plane/crates/domain/src/commands.rs:857-900`, `:946-989`, `:1114-1164`).
- H004 schema-version evolution policy evidence was not found outside the proposal and prior audit commentary (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1333-1335`).
- H008 has cap constants, unit tests, and transactional latch writes, but no concurrent late-output writer proof matching the hardening requirement (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1343-1345`, `control-plane/crates/db/src/repos/cancel_late_output_overflow.rs:17-89`, `:168-228`, `:390-481`).
- The proposal migration contract describes `ALTER TABLE artifact_lineage ADD COLUMN report_kind`, but the implementation creates `artifact_lineage` as a new table; no explicit backfill/no-preexisting-row evidence closes H003 (`docs/proposals/083-execution-truth-ownership-invariant-model.md:360-378`, `control-plane/crates/db/migrations/087_p083_001_artifact_lineage_report_kind.sql:1-52`, `control-plane/crates/db/tests/proposal_083_migrations.rs:656-670`).

## Requirement Summary

| Requirement area | Status | Evidence / gap |
| --- | --- | --- |
| Current-revision closeout review gate | Missing | This audit found blockers; proposal requires approve and `blocker_count=0` before closeout. |
| Durable SQLite execution-truth authority | Partially implemented | Migrations/repos/handlers are broad and gate passes; side-effect contract and hardening gaps keep full conformance partial. |
| CallerRequestId and command idempotency | Partially implemented | UUIDv4 validation and replay/alias paths exist; side-effect hash field name and policy coverage drift from proposal. |
| Rollback target reconciliation | Implemented for audited surfaces | Gate verifies GraphQL/MCP/idempotency/readback target mode alignment. |
| GraphQL SDL contract | Partially implemented | Core types exist; side-effect mutation signature/success shape diverges from proposal text. |
| MCP lifecycle inventory and schemas | Partially implemented | Runtime/reference require `decision_json`; proposal inventory omits it. |
| Migrations and readback descriptors | Partially implemented | Gate passes; H003 migration/backfill posture remains not explicitly closed against proposal text. |
| Shutdown/cancellation/identity hold | Implemented for audited Rust/API surfaces | Command handler and GraphQL/MCP paths are wired; no live daemon crash drill was run in this audit. |
| Durable monotonic clock | Implemented by static/gate evidence | Gate verifies source and baseline correlation. |
| Post-cancel late-output overflow | Partially implemented | Transactional latch/cap logic exists; concurrent-writer proof required by H008 is missing. |
| SwiftData/macOS operator boundary | Partially implemented / not Ready | Static source proof exists; no Swift build or interactive UI/accessibility proof was run. |
| Rollout contract, metrics, and gate aliases | Partially implemented | Gate and lint pass; readiness still blocked by current-review and hardening gaps. |
| Mandatory hardening H003/H004/H007/H008/H009/H011 | Partially implemented | H004 missing; H007/H011 omit one command; H008 lacks concurrent proof; H003 posture incomplete. |

## Detailed Findings

### API-001 - `side_effects.force_reconcile` public contract is internally inconsistent and not matched end to end

Severity: **Major**
Reviewer lens: `api_contract_reviewer`
Track: Track 1 conformance

The proposal's GraphQL SDL says `sideEffectsForceReconcile` accepts only `sideEffectId` and `callerRequestId`, and says success returns `sideEffectId` plus `reconciliationId` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:173-181`). The implementation requires `decision_json` and returns `side_effect_id`, `journal_id`, and `request_id` (`control-plane/crates/graphql-server/src/schema.rs:6334-6361`, `control-plane/crates/graphql-server/src/types/p083.rs:893-900`).

The MCP inventory has the same stale input shape: proposal `required_input` omits `decision_json` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:195-210`), but the reference and runtime schemas require it (`docs/reference/mcp/p083/side_effects.force_reconcile.input.schema.json:21-25`, `control-plane/crates/mcp-server/src/tools/runs.rs:456-486`).

Impact: generated clients and auditors cannot know which side-effect reconcile contract is authoritative. The implementation shape may be more correct because `decision_json_digest` is part of the idempotency contract, but P083 cannot be marked conformant while its active SDL/MCP inventory contradict the implemented API.

Required action: update the proposal/reference/fixtures and GraphQL SDL contract to include `decisionJson`/`decision_json` and the actual success envelope, or change implementation to match the current proposal contract. Add a gate that compares proposal SDL/MCP inventory against generated GraphQL SDL and runtime/reference MCP schemas.

### REL-001 - `side_effects.force_reconcile` intent hash uses `effect_id` instead of `side_effect_id`

Severity: **Major**
Reviewer lens: `rust_reliability_reviewer` / `api_contract_reviewer`
Track: Track 1 conformance

P083 requires `side_effects.force_reconcile` intent hash fields `side_effect_id` and `decision_json_digest` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1125-1136`). The command handler hashes `decision_json_digest` and `effect_id` (`control-plane/crates/engine/src/command_handler.rs:9170-9180`).

Impact: the canonical intent bytes are not byte-for-byte derived from the proposal field names. That matters because P083 explicitly makes canonical hash composition a cross-surface contract, not an internal implementation detail.

Required action: rename the hash key to `side_effect_id`, or revise P083 to make `effect_id` the canonical logical field name and update GraphQL/MCP/reference fixtures accordingly.

### REL-002 - H007/H011 command policy tables omit `provider_session.mark_process_absent`

Severity: **Major**
Reviewer lens: `rust_reliability_reviewer`
Track: Track 1 conformance

P083 covers nine idempotent lifecycle commands, including `provider_session.mark_process_absent` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1082-1087`). Its TTL map also assigns that command a 120-second TTL (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1108-1117`).

The implemented H011 TTL table omits `provider_session.mark_process_absent` (`control-plane/crates/domain/src/commands.rs:857-900`). The implemented H007 failed-terminal retry policy table also omits it (`control-plane/crates/domain/src/commands.rs:946-989`). The tests encode the old assumption by naming and checking "all eight commands" (`control-plane/crates/domain/src/commands.rs:1114-1164`).

Impact: one P083 lifecycle command lacks the mandatory per-command TTL and failed-terminal retry policy evidence. That command is specifically used to clear identity-ambiguous shutdown holds, so leaving it out weakens the recovery/idempotency contract.

Required action: add `provider_session.mark_process_absent` to both policy tables, update tests to cover all nine commands, and add a fixture for its failed-terminal retry behavior.

### OPS-001 - H004 schema-version evolution policy is missing

Severity: **Major**
Reviewer lens: `api_contract_reviewer` / `observability_rollout_reviewer`
Track: Track 1 conformance

P083-HARDEN-004 requires append-only `schema_version` semantics, same-version additive-safe field policy, version bump rules, prior-version readability, and unknown-schema diagnostic behavior (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1333-1335`). A repository search found only the proposal and prior implementation-audit commentary, not an implemented reference policy, executable fixture, or lint rule.

Impact: P083 introduces multiple public JSON/GraphQL/MCP/readback contracts. Without an evolution policy, compatibility decisions remain ad hoc and closeout would violate the proposal's mandatory hardening closure policy.

Required action: add the schema-version evolution policy to implemented reference docs or code-backed validation, plus fixtures for same-version additive fields, version bump behavior, prior-version readability, and unknown-schema diagnostics.

### REL-003 - H008 lacks concurrent late-output writer proof

Severity: **Major**
Reviewer lens: `rust_reliability_reviewer`
Track: Track 1 conformance

The implementation has useful pieces: cap constants and cap unit tests (`control-plane/crates/db/src/repos/cancel_late_output_overflow.rs:17-89`, `:390-481`), transactional latch writes (`control-plane/crates/db/src/repos/cancel_late_output_overflow.rs:168-228`), and executor logic that refuses active projection dirtiness for ignored late outputs (`control-plane/crates/engine/src/executor.rs:15677-15690`). Existing migration tests prove table existence, unique latch key, generated normalized columns, and fresh-DB verification (`control-plane/crates/db/tests/proposal_083_migrations.rs:342-395`, `:708-720`).

P083-HARDEN-008 is stricter: it requires specifying and testing atomic counter increment and cap enforcement for concurrent late-output writers, including overflow latch behavior (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1343-1345`). I did not find a concurrent-writer test for this repository path.

Impact: the single-writer/static evidence is not enough to close the explicit H008 hardening item.

Required action: add a deterministic concurrent-writer test that races late-output latch writes for the same normalized latch key and proves one row, atomic counter accumulation, cap/latch behavior, and `projection_mutation_blocked=1`.

### OPS-002 - H003 backfill/no-preexisting-row posture is incomplete against the proposal text

Severity: **Major**
Reviewer lens: `observability_rollout_reviewer`
Track: Track 1 conformance

P083 describes migration `p083_001_artifact_lineage_report_kind` as `ALTER TABLE artifact_lineage ADD COLUMN report_kind TEXT NULL` plus triggers/indexes and a zero-row verification query (`docs/proposals/083-execution-truth-ownership-invariant-model.md:360-378`). The implementation creates `artifact_lineage` as a new table with `report_kind` already present (`control-plane/crates/db/migrations/087_p083_001_artifact_lineage_report_kind.sql:1-52`). The executable verification only proves zero rows on a fresh test DB (`control-plane/crates/db/tests/proposal_083_migrations.rs:656-670`).

If `artifact_lineage` is truly new in every supported deployed store, this can be closed by explicit executable no-preexisting-table/no-row evidence. As written, the proposal still requires either an additive backfill for pre-existing active report rows or executable evidence that no such rows exist before enforcement (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1328-1330`), and the current evidence does not make that deployment posture explicit.

Required action: either align the migration with the proposal's `ALTER TABLE` posture and add backfill evidence, or add a migration/readback proof that no supported pre-P083 store contains `artifact_lineage` active report rows before the triggers enforce bounded `report_kind`.

### READY-001 - Ready/closeout gate cannot pass until this current-revision audit returns zero blockers

Severity: **Critical for readiness**
Reviewer lens: `chainworks_execution_truth_reviewer` / `observability_rollout_reviewer`
Track: Track 1 conformance and readiness

P083 says Ready/closeout may be claimed only after a fresh aggregate implementation review against this revision returns `decision=approve`, `blocker_count=0`, and current-revision evidence (`docs/proposals/083-execution-truth-ownership-invariant-model.md:43-46`). This R9 audit is a current-revision implementation audit, but it identifies blockers above.

Impact: the implementation is not closeout-ready even with the canonical gate passing.

Required action: fix or explicitly re-scope the blockers through an approved successor proposal, then rerun the implementation audit and canonical gate.

### READY-002 - macOS UI/SwiftData specialist coverage is not enough for Ready

Severity: **Major**
Reviewer lens: `macos_ui_reviewer` / `apple_ux_reviewer` / `apple_arch_reviewer`
Track: Track 2 specialist coverage

P083 includes macOS menu/toolbar placement, manual identity-check UX, and SwiftData lifecycle-boundary claims. The P083 gate statically verifies the Run menu and toolbar strings, and source review confirmed `ManualProcessIdentityCheckBanner` and `P083IdentityHoldSessionsModel` are present (`Chainworks Forge/Chainworks_ForgeApp.swift:505-550`, `Chainworks Forge/Views/ManualProcessIdentityCheckBanner.swift:6-158`, `Chainworks Forge/Models/P083IdentityHoldSessionsModel.swift:6-41`). No Swift build, UI interaction test, accessibility pass, or SwiftData runtime proof was run in this audit.

Impact: static source coverage is useful but not enough to mark the UI/SwiftData portions Ready.

Required action: run or add a Swift/macOS proof gate for these surfaces, or explicitly move them to an approved follow-up proposal before closeout.

## Routed Findings

| Finding | Owner lens | Disposition |
| --- | --- | --- |
| API-001 | `api_contract_reviewer` | Blocks full conformance; side-effect API contract is internally inconsistent. |
| REL-001 | `rust_reliability_reviewer` / `api_contract_reviewer` | Blocks exact idempotency conformance; hash field name drifts. |
| REL-002 | `rust_reliability_reviewer` | Blocks H007/H011 completion; one lifecycle command omitted. |
| OPS-001 | `api_contract_reviewer` / `observability_rollout_reviewer` | Blocks H004 completion. |
| REL-003 | `rust_reliability_reviewer` | Blocks H008 completion. |
| OPS-002 | `observability_rollout_reviewer` | Blocks H003 completion unless deployment posture is proved. |
| READY-001 | `chainworks_execution_truth_reviewer` / `observability_rollout_reviewer` | Blocks Ready/closeout. |
| READY-002 | macOS/Apple reviewer lenses | Blocks Ready for UI/SwiftData scope. |

## Readiness Checklist

| Check | Status |
| --- | --- |
| Proposal current-revision implementation audit approve/zero blockers | Fail |
| Prior proposal-review artifacts reused or explicitly unavailable | Pass; unavailable |
| Canonical proposal gate passes on audited tree | Pass |
| Security-sensitive diff independently reviewed | Pass, with no confirmed Critical/Major security bug in inspected P083 paths |
| API contracts match proposal/reference/runtime | Fail |
| Full-implementation tail gate satisfied | Fail |
| Mandatory hardening items closed | Fail |
| Specialist coverage hard gate satisfied | Fail for macOS UI/SwiftData runtime coverage |
| Ready / Ready with Risks eligible | No |

## Verification Log

Commands and checks run:

- Read `/Users/user/.codex/skills/proposal-implementation-audit/SKILL.md` completely.
- Read proposal `docs/proposals/083-execution-truth-ownership-invariant-model.md`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py docs/proposals/083-execution-truth-ownership-invariant-model.md`
  - Result: selected `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R9.md`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py docs/proposals/083-execution-truth-ownership-invariant-model.md`
  - Result: no proposal-review artifacts found.
- Read `.codex/review-router.yaml`, `.codex/reviewers/chainworks-execution-truth.yaml`, and the built-in reviewer registry.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/security_sensitive_diff.py --root /Users/user/Documents/Chainworks\ Forge --json`
  - Result: triggered; categories included auth, public ingress, parser/schema boundary, filesystem/subprocess/process boundary, denial/redaction privacy, and dependency/crypto-sensitive surfaces.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/implementation_surface_fingerprint.py --root /Users/user/Documents/Chainworks\ Forge --json`
  - Result: API, architecture, reliability, security, observability/rollout, Apple UI/UX, and performance lenses triggered.
- Focused source/evidence review of GraphQL SDL/types, MCP tool schemas/handlers, command handler idempotency paths, domain hardening tables, migrations/tests, late-output repository/executor code, Swift menu/banner/model code, and reference MCP schemas.
- `./scripts/test-gate.sh proposal-083`
  - Result: passed.
  - Included evidence corpus verification, DB P083 migration tests, `cargo check -p db`, focused engine P083/shutdown tests, `cargo check -p daemon`, `cargo check -p graphql-server`, `cargo check -p mcp-server`, domain denial-code round trip, GraphQL approval mutation tests, MCP P083 tests, rollout contract lint, migration/readback static checks, monotonic-clock static checks, rollback target checks, and macOS Run menu/toolbar static proof.

Additional validation still required before Ready:

- Exact proposal SDL/MCP inventory comparison against generated GraphQL SDL and runtime/reference MCP schemas, especially `side_effects.force_reconcile`.
- H004 schema-version policy implementation and fixtures.
- H007/H011 all-nine-command policy coverage.
- H008 concurrent late-output writer proof.
- H003 deployment posture proof for `artifact_lineage.report_kind`.
- Swift/macOS build or UI proof for the P083 menu/banner/SwiftData claims.
