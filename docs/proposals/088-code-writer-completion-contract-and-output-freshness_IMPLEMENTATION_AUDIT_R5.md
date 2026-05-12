# Proposal 088 Implementation Audit R5

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` |
| Proposal checksum | `4a2a5c99988d7991e967862158153c5438189d705a65eb1faf28c25a067cf690` |
| Report | `docs/proposals/088-code-writer-completion-contract-and-output-freshness_IMPLEMENTATION_AUDIT_R5.md` |
| Audit timestamp | `2026-05-11T19:56:19Z` |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main...origin/main` |
| HEAD | `b18587e63e39913cd7cb611de88b090a8e8ad5ca` |
| Implementation target | Current dirty worktree on `main` |
| Compare base | Implicit current worktree; no PR branch/range supplied |
| Proposal state | `Draft` per proposal line 6; no superseding proposal found during this audit |
| Overall conformance | **Not Implemented** by the skill rollup model because one atomic in-scope requirement is missing, with several additional partial requirements |
| Overall implementation readiness | **Not Ready** |
| Reviewer-selection reuse | **Not reused** |
| Audit confidence | Medium-high for Rust/API/data findings; medium for macOS UI because the app build failed before UI runtime validation |

## Implementation Target / Compare Base

The audit target is the current dirty worktree. The tree includes P088 Rust control-plane changes, DB migration/repo code, GraphQL/MCP readback, deterministic fixtures, gate docs, Swift read-model/UI changes, and unrelated dirty files from other proposal work. The proposal file and prior P088 implementation audits are untracked in this checkout.

`git status --short --branch` showed modified files across `Chainworks Forge/`, `Chainworks ForgeTests/`, `control-plane/`, `docs/reference/`, and `scripts/test-gate.sh`, plus untracked P088 files such as `control-plane/crates/domain/src/code_writer_completion.rs`, `control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql`, `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs`, `control-plane/crates/*/tests/proposal_088_*`, `docs/evidence/088-code-writer-completion/`, and `Chainworks ForgeTests/Proposal088OperatorReadbackTests.swift`.

## Prior Review Reuse

`discover_prior_review.py` returned no proposal-review artifacts for this proposal. Existing sibling `*_IMPLEMENTATION_AUDIT_R1.md` through `R4.md` were not reused for reviewer selection because the skill explicitly excludes prior implementation audits from reviewer-selection reuse unless requested. Current reviewer routing is based on fresh proposal extraction, current code inspection, and same-tree gate results.

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_reliability_reviewer` | P088 changes retry/repair eligibility, stale active terminalization, receipt recovery, idempotency, and startup repair. |
| `api_contract_reviewer` | P088 defines receipt fields, closed public vocabularies, GraphQL/MCP/run-report readback, and no new mutations. |
| `observability_rollout_reviewer` | P088 has migration, focused gate, evidence fixtures, startup reconciliation, operator diagnostics, and rollout order commitments. |
| `chainworks_execution_truth_reviewer` | P088 changes durable Run/Stage/Agent/output truth, runtime receipts, failed-stage evidence, and completion receipt ownership. |
| `macos_ui_reviewer` | P088 explicitly includes operator readback/UI, and Swift operator surfaces were changed. |

Rejected close alternatives:

| Reviewer | Reason rejected |
|---|---|
| `rust_arch_reviewer` | Rust module boundaries are touched, but the decisive risks are reliability, execution truth, and API contract. Hard cap kept this out. |
| `apple_arch_reviewer` | Swift work is primarily read-model/presentation/UI display; deeper Apple state ownership was not the highest-risk lens. |
| `product_reviewer` | The proposal has no central product metric or experiment decision checkpoint. |
| `security_reviewer` | No new auth, secrets, privacy, public mutation, unsafe, or permission expansion surface was found. |
| `performance_reviewer` | No latency/throughput/benchmark claim is central to P088. |
| Go reviewers | No real `go.mod` or Go implementation surface is present. |

## Proposal Contract Summary

P088 is a code-writer completion-handoff containment proposal. It does not weaken output contracts and does not make receipt artifacts transition authority. It requires the engine to diagnose the case where `code_writer` changes implementation-owned files but fails to publish fresh required outputs for the current attempt.

Locked proposal decisions:

- Transition truth remains declared-output import, validation, and materialization, not agent-authored receipts or exact-path files.
- `changed_files_manifest` becomes supporting evidence only.
- Freshness is proven by engine-owned pre/post exact-output and worktree fingerprints, provider-envelope settlement, ACP final text extraction, and schema validation.
- Eligible `code_writer` failures use one same-session `code_writer_completion_repair_v1` finalization turn instead of the generic repair prompt, without expanding the one-turn budget.
- Completion repair may publish missing structured outputs only; unexpected non-output repo mutation fails closed.
- Runtime evidence must be durable and prompt-level, including original prompt and completion repair runtime receipts.
- Operator readback must expose actionable status/failure classes and cross-surface `implementationCompletion` through run report, MCP, GraphQL, and macOS UI.

## Platform / Product Scope

| Scope Type | Value |
|---|---|
| Apple | macOS operator app |
| Backend/service | Rust control-plane engine, ACP transport, DB persistence, startup recovery, GraphQL, MCP, run report |
| Data | SQLite migration and receipt/output/text-capture tables |
| Rollout | Focused `proposal-088|p088` gate and `docs/reference/test-gates.md` |
| Product/operator | Diagnose and recover `code_writer` completion-handoff failures without manual forensic digging |

## Primary Implementation Flows

1. A `code_writer` original attempt changes implementation-owned paths, misses required outputs, writes a completion receipt, preserves prompt-level evidence, and surfaces `work_completed_missing_current_attempt_outputs` or `terminal_response_completed_missing_required_outputs`.
2. A stale P087-like `implementation_active` attempt is terminalized by P037 supervision into P088 receipt/readback rather than bypassing evidence through active-prompt auto-requeue.
3. A current-attempt terminal final response with usable `CHAINWORKS_OUTPUT` materializes through normal declared-output settlement without invoking completion repair.
4. A completion receipt persists through SQLite, canonical receipt links, GraphQL, MCP, run-report JSON, and macOS operator readback.
5. A crash after receipt artifact write but before DB persistence is recovered or marked as `completion_receipt_partial_write` during startup repair.

## Implementation Fingerprint

Stack tags:

- Rust control-plane: `control-plane/crates/acp`, `db`, `domain`, `engine`, `graphql-server`, `mcp-server`.
- macOS SwiftUI operator app: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`, `Views/RunsHomeView.swift`, `Views/RunTimelineInspectorView.swift`, related tests.
- SQLite migration: `control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql`.
- Evidence/gates/docs: `docs/evidence/088-code-writer-completion/`, `docs/reference/test-gates.md`, `scripts/test-gate.sh`.

Risk tags:

- API vocabulary drift.
- Repair lifecycle and retry eligibility.
- Receipt idempotency/startup recovery.
- Cross-surface readback parity.
- macOS UI build and display coverage.
- Gate completeness across Rust and Swift surfaces.

## Proposal Fidelity Inventory

### Matches

- Deterministic P088 fixture files exist under `docs/evidence/088-code-writer-completion/`.
- The Rust focused gate is registered as `proposal-088|p088` in `scripts/test-gate.sh` and documented in `docs/reference/test-gates.md`.
- The focused gate passed on the audited tree.
- `worktree_fingerprint_v1` exists with sorted paths, inclusion/exclusion reasons, path statuses, content digests, and derived counts.
- Completion receipt persistence has dedicated receipt, text capture, output decision, and canonical link tables.
- Receipt persistence now performs transactional upsert with runtime receipt rows and conflict checks.
- Canonical readback now joins through `code_writer_completion_receipt_links` instead of falling back to latest created receipt.
- Startup repair can recover an orphan P088 receipt artifact and mark it `completion_receipt_partial_write`.
- P037 stale `implementation_active` now has an integration test proving P088 receipt path entry rather than active-prompt auto-requeue.
- GraphQL/MCP/run-report surfaces expose `implementationCompletion` and receipt details with public enum readback metadata.
- macOS read-model and view code exists for a sidebar signal, overview card, and timeline entry.

### Divergences

- Engine-written receipt `completion_mode` uses `normal_settlement`, `completion_repair`, and `completion_failure`, not proposal values `provider_envelope`, `acp_final_text_chainworks_output`, `exact_path_current_attempt`, `code_writer_completion_repair_turn`, and `mixed`.
- Engine-written receipt `completion_status` uses public-ish status values such as `succeeded`, `partial_evidence`, `failed`, and `skipped_no_live_session`, not receipt values `complete`, `partial`, and `missing_required_outputs`.
- The exact block reason `generic_repair_already_failed_completion_contract_required` is absent from the implementation.
- Completion-text and transcript typed absence vocabularies are narrower than the proposal and include a `redacted_only` absence reason that the proposal did not list.
- `operator_retry_completion_recovery` is detected from payload flags plus a preserved evidence path, but no command-level operator retry wiring or integration test was found.
- Swift tests use non-server contract values such as `repair_failed`, `retry_completion_repair`, and `inspect_failed_stage_evidence`.
- `proposal-088` gate is Rust/control-plane only; it does not build or run the macOS UI tests even though UI is in scope.
- `./scripts/test-gate.sh fast` and `./scripts/test-gate.sh build` fail the macOS app build on this tree.

### Ambiguities / Evidence Gaps

- No live blocked-run retry was executed, so "targeted retries no longer require forensic digging" is inferred from readback surfaces rather than proven end to end.
- No macOS runtime screenshot, accessibility pass, or UI smoke test was possible because the app build failed.
- The build failure log captured the failed SwiftCompile group but not the underlying compiler diagnostic line.
- Provider-independence is mostly shown by provider-agnostic control-plane code and generic ACP tests, but this audit did not find P088-specific fixture-backed coverage for `junie`, `claude`, and `codex` as a set.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 16 |
| Partially Implemented | 7 |
| Missing | 1 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

Because one atomic in-scope requirement is `Missing`, the skill rollup is **Overall Conformance = Not Implemented**. Practically, the implementation is substantial and many core Rust flows are implemented, but it is not proposal-conformant yet.

## Detailed REQ Audit

| ID | Requirement | Proposal Source | Status | Evidence | Implementation Mapping | Gap / Note |
|---|---|---|---|---|---|---|
| REQ-001 | Materialize deterministic P088 evidence fixtures before implementation. | Lines 60-70, 991-1016 AC19 | Implemented | Code, tests-run | `docs/evidence/088-code-writer-completion/*`; `scripts/test-gate.sh:6186-6236`; gate passed. | Fixtures exist and static checks validate expected scenarios. |
| REQ-002 | Classify `code_writer` changed-work/missing-output attempts as a distinct completion failure family. | Lines 577-589, AC1 | Implemented | Code, tests-found, tests-run | `executor.rs:9008-9015`; `engine/tests/integration.rs:10520-10528`; GraphQL test lines 393-399. | Distinct failure classes are persisted and exposed. |
| REQ-003 | Stale previous-attempt files cannot satisfy current-attempt settlement. | Lines 99-107, 417-454, AC2 | Implemented | Code, tests-run | `executor.rs` stale-output decisions are counted at `9000-9005`; P088 fixtures and gate cover stale exact-path output. | No blocker found. |
| REQ-004 | Completion eligibility is based on original-prompt fingerprints, with preserved historical evidence as the only exception; manifests are supporting evidence only. | Lines 342-359, AC3 | Partially Implemented | Code, tests-found, tests-run | `executor.rs:5141-5148`; `worktree_fingerprint.rs:115-170`, `232-359`; operator retry payload detection at `9422-9452`. | Fingerprint eligibility exists. Historical recovery is only payload/path detection, with no end-to-end operator retry test. The deterministic `docs/evidence/088-code-writer-completion/` path is not included by the active-proposal-id helper, though `docs/reference/**` is. |
| REQ-005 | Pre-existing dirty work plus timeout/no terminal text is not treated as current-attempt completion. | Lines 347-359, AC4 | Implemented | Code, tests-run | `worktree_fingerprint.rs:183-229`, `337-347`; P088 fixture `p087-70c9-dirty-worktree-timeout.fixture.json`; gate passed. | No blocker found. |
| REQ-006 | Persist completion receipt plus transcript/runtime/completion-text evidence or typed absence reasons. | Lines 221-265, 496-565, AC5 | Partially Implemented | Code, migration, tests-run | Domain record fields at `code_writer_completion.rs:6-52`; migration at `051...sql:65-162`; text captures at `code_writer_completion.rs:54-71`; receipt construction at `executor.rs:9058-9144`. | Fields exist, but receipt `completion_mode`/`completion_status` values diverge from proposal and absence vocab is incomplete. |
| REQ-007 | Use distinct one-turn `code_writer_completion_repair_v1` without generic prompt reuse for eligible attempts and without extra budget. | Lines 269-326, AC6 | Partially Implemented | Code, tests-run | Eligible branch selects `code_writer_completion_repair_prompt` at `executor.rs:5156-5161`; prompt evidence at `5175-5197`; prompt receipt uses `code_writer_completion_repair_v1` at `5304-5310`; session events at `5225-5239`, `5507-5525`, `5570-5591`. | Eligible branch exists, but the generic-repair-already-failed guard is missing as a separate explicit requirement. |
| REQ-008 | If generic repair already failed, ineligible attempts block with `generic_repair_already_failed_completion_contract_required`. | Lines 299-300, 904-906 | Missing | Code search | `rg` found no implementation of `generic_repair_already_failed_completion_contract_required`; ineligible attempts still fall to `output_contract_repair_prompt` at `executor.rs:5157-5161`. | This is an explicit missing contract and a release blocker. |
| REQ-009 | Completion repair cannot mutate non-output repo files; unexpected mutation fails closed with typed evidence. | Lines 328-340, AC7 | Implemented | Code, tests-run | Pre/post repair fingerprints at `executor.rs:5242-5282`, `5340-5389`; mutation failure at `5376`, `5394-5437`; completion result mapping at `9333-9342`; fixture and gate passed. | No blocker found. |
| REQ-010 | Original and completion-repair runtime receipts are separately persisted and cannot overwrite each other. | Lines 498-508, 935-943, AC8 | Implemented | Code, migration, tests-run | Runtime receipt migration at `051...sql:3-63`; transaction API at `code_writer_completion_receipts.rs:44-76`; prompt receipt rows at `executor.rs:5296-5337`; DB test `proposal_088_runtime_receipts_preserve_prompt_level_rows_per_execution`. | No blocker found. |
| REQ-011 | SQLite receipt writes are transactional, idempotent, conflict-detecting, canonically selected, and startup-recoverable after partial writes. | Lines 717-729, AC9 | Implemented | Code, migration, tests-run | Transaction/conflict upsert at `code_writer_completion_receipts.rs:15-76`; canonical link readback at `267-285`; link table at `051...sql:152-162`; tests at `proposal_088_persistence.rs:591-688`; startup recovery at `recovery.rs:500-575`, `integration.rs:10543-10691`. | R4 blocker resolved: canonical readback no longer falls back to unlinked/latest rows, and startup recovery is tested. |
| REQ-012 | Original and completion-repair terminal text are inspectable as redacted/raw text or typed absence reasons independent of transcript availability. | Lines 498-565, AC10 | Partially Implemented | Code, tests-run | ACP capture metadata at `acp/src/lib.rs:337-368`; engine capture records at `executor.rs:9263-9331`; transcript status at `9345-9361`; tests at `11177-11230`. | Capture exists, but proposal-listed absence reasons are not all representable. |
| REQ-013 | Completion text capture records source, byte limits, truncation flags, extraction input SHA-256, and typed truncation failures. | Lines 511-549, AC11 | Implemented | Code, tests-run | ACP source selection at `transport.rs:1014-1078`; proposal source mapping at `executor.rs:9412-9419`; ingestion boundary mapping at `9364-9391`; ACP tests passed. | Capped stream is mapped to proposal public value `session_update_stream`. |
| REQ-014 | Prompt-side evidence records template id/version, prompt hash, redacted prompt artifact, expected-output snapshot hash, and repair/settlement reason. | Lines 731-766, 935-943, AC12 | Implemented | Code, tests-run | Prompt artifact/snapshot persistence at `executor.rs:5175-5197`; prompt receipt construction at `5304-5317`; summary projection at `code_writer_completion.rs:299-329`; GraphQL/MCP tests passed. | No blocker found. |
| REQ-015 | `worktree_fingerprint_v1` artifacts explain inclusion/exclusion, path status, digest, and deterministic count derivation. | Lines 361-416, AC13 | Implemented | Code, tests-run | Schema and capture at `worktree_fingerprint.rs:10-170`; classification/counts at `232-370`; tests at `526-725`; gate passed. | Minor caveat noted in REQ-004 for exact proposal-owned evidence folder matching. |
| REQ-016 | Public GraphQL/MCP/run-report readback uses closed vocabularies with unknown handling and adds no retry/repair/continue mutation. | Lines 778-863, AC14 | Implemented | Code, tests-run | Public enum wrapper at `code_writer_completion.rs:109-140`; values at `198-243`; projection at `245-350`; GraphQL type at `types/run.rs:214-359`; tests at `proposal_088_code_writer_completion_readback.rs:260-410`; MCP tests at `245-381`. | Public summary contract is implemented; this does not cover internal receipt vocab drift in REQ-006. |
| REQ-017 | Run report, MCP, and GraphQL can explain fresh, stale, missing, and control-plane outputs. | Lines 830-861, AC15 | Implemented | Code, tests-run | Summary fields at `code_writer_completion.rs:180-187`, `330-337`; GraphQL fields at `types/run.rs:236-243`, `395-405`; MCP report projection at `reports.rs:548-676`; tests passed. | No blocker found. |
| REQ-018 | Completed terminal responses with missing required outputs are `terminal_response_completed_missing_required_outputs`, not `provider_active_without_terminal_response`. | Lines 577-586, AC16 | Implemented | Code, tests-run | `executor.rs:9008-9015`; GraphQL/MCP tests assert the failure class. | No blocker found. |
| REQ-019 | P087-like stale `implementation_active` attempts enter P088 diagnosis/recovery instead of remaining active or auto-requeueing. | Lines 118-122, 591-611, AC17 | Implemented | Code, tests-run | Integration test `proposal_088_code_writer_stale_implementation_active_enters_receipt_path_not_auto_requeue` at `integration.rs:10384-10541`; gate passed. | R4 blocker resolved. |
| REQ-020 | Usable current-attempt final `CHAINWORKS_OUTPUT` materializes through normal declared-output settlement without completion repair. | Lines 126-130, 607-609, 880-882, AC18 | Implemented | Code, fixture, tests-run | Fixture `normal-materialization-no-repair.fixture.json`; gate static check at `scripts/test-gate.sh:6213-6216`; ACP capture tests passed. | No live provider run was executed, but the focused gate covers the contract path. |
| REQ-021 | Provider-independence tests cover the same completion contract for `junie`, `claude`, and `codex`. | Lines 945-953 | Partially Implemented | Code search, tests-found | Generic control-plane code is provider-agnostic; P088 GraphQL/MCP tests seed `codex`; DB tests seed `claude`. | This audit did not find P088-specific fixture-backed coverage for all three named providers as a set, especially `junie`. |
| REQ-022 | `proposal-088|p088` gate is registered, documented, runnable, and covers required fixture/readback cases. | Lines 955-989, AC21 | Implemented | Config, docs, tests-run | Gate registered at `scripts/test-gate.sh:6180-6357`; docs at `docs/reference/test-gates.md:1894-1908`; `./scripts/test-gate.sh proposal-088` passed. | The gate is valid for Rust/control-plane P088, but readiness still requires Swift build/UI evidence because UI is in proposal scope. |
| REQ-023 | Operator readback/UI displays the new failure family. | Lines 862-883 | Partially Implemented | Code, tests-found, tests-run failed | Swift read model at `P031ThinGraphQLReadBoundary.swift:1337-1538`; run row wiring at `1557-1650`, `4728-4750`, `4908-4934`; UI card at `RunsHomeView.swift:193-195`, `1202-1279`; timeline entry at `RunTimelineInspectorView.swift:164-185`; Swift tests exist. | The app build fails under `./scripts/test-gate.sh fast/build`, so this UI cannot be accepted as build-verified. Swift tests also use stale/non-contract values. |
| REQ-024 | Targeted retries for blocked runs no longer require forensic digging to distinguish provider failure from completion-handoff failure. | Lines 867-884, AC22 | Partially Implemented | Code, tests-run, inference | Readback and next-action projection exist at `code_writer_completion.rs:430-469`; operator retry recovery payload detection exists at `executor.rs:9422-9452`; GraphQL/MCP/run-report surfaces pass. | No live blocked-run retry or command-level operator retry integration test proves this end to end; UI build failure blocks operator-facing validation. |

## Reviewer / Lens Scorecard

| Lens | Conformance | Top risk | Confidence |
|---|---|---|---|
| Objective proposal conformance | Not Implemented | Missing generic-repair-already-failed guard plus partial receipt/absence vocab compliance | High |
| `rust_reliability_reviewer` | Partial | Retry eligibility and missing generic-repair-failed path still leave a failure/retry edge unproven | Medium-high |
| `api_contract_reviewer` | Partial | Internal receipt vocab does not match proposal contract; absence vocab is incomplete | High |
| `observability_rollout_reviewer` | Not Ready | Focused Rust gate passes, but app build fails and UI is not covered by the focused gate | High |
| `chainworks_execution_truth_reviewer` | Mostly implemented | Receipt ownership/canonical/startup paths are much stronger, but receipt values drift from contract | Medium-high |
| `macos_ui_reviewer` | Not Ready | UI code exists but the app does not build, and Swift tests use stale public values | Medium |
| Readiness | Not Ready | Failed macOS build plus unresolved major/critical findings | High |

## Routed Specialist Findings

### READY-001 - macOS build is red while UI is in P088 scope

| Field | Value |
|---|---|
| Reviewer | `macos_ui_reviewer`, `observability_rollout_reviewer` |
| Severity | Critical |
| Confidence | High |
| Related requirements | REQ-022, REQ-023, REQ-024 |
| Evidence types | tests-run, code, config |
| Evidence references | `./scripts/test-gate.sh fast` exited 65 with `** BUILD FAILED **`; filtered build output showed failed SwiftCompile group including `FailedStageEvidencePanel.swift`, `RunsHomeView.swift`, `RunTimelineInspectorView.swift`, and `SettingsView.swift`, with 3 failures; P088 focused gate runs Rust packages only at `scripts/test-gate.sh:6348-6355`; UI code is in `RunsHomeView.swift:1202-1279` and `RunTimelineInspectorView.swift:164-185`. |

Why it matters: P088 explicitly includes operator readback/UI. A passing Rust focused gate cannot prove the proposal-ready operator experience if the macOS app cannot build on the audited tree.

Recommended action: Fix the Swift build failure first, then run `./scripts/test-gate.sh fast` or a broader canonical gate. Add P088 Swift readback tests to the validation path used for this proposal, or explicitly document that UI validation is covered by the canonical fast/full gate.

Acceptance criteria: `./scripts/test-gate.sh fast` passes on the same tree; P088 Swift readback tests compile and run; the P088 report can cite macOS build/test evidence rather than source inspection only.

### API-001 - Receipt `completion_mode` and `completion_status` values drift from the proposal contract

| Field | Value |
|---|---|
| Reviewer | `api_contract_reviewer`, `chainworks_execution_truth_reviewer` |
| Severity | Major |
| Confidence | High |
| Related requirements | REQ-006, REQ-011, REQ-016 |
| Evidence types | proposal, code, tests-found |
| Evidence references | Proposal requires `completion_mode` values at lines 229-234 and `completion_status` values at lines 261-264. Engine writes `succeeded`, `partial_evidence`, `failed`, `skipped_no_live_session` and modes `normal_settlement`, `completion_repair`, `completion_failure` at `executor.rs:8970-8986`, persisted at `9070-9100`. Tests seed the same drifted values in `proposal_088_persistence.rs`, GraphQL tests, and MCP tests. |

Why it matters: P088 separates internal receipt truth from public `implementationCompletion` summary. The public summary vocab is implemented, but the receipt artifact/schema values no longer match the proposal's minimum receipt contract. Any operator tooling or future migration that reads receipt rows/artifacts directly will see non-proposal values.

Recommended action: Either change the engine/DB fixtures/tests to use the proposal receipt vocab or explicitly amend the proposal/reference docs and migration contract. Do not leave public summary values and receipt values implicitly mixed.

Acceptance criteria: Receipt rows/artifacts use `provider_envelope`, `acp_final_text_chainworks_output`, `exact_path_current_attempt`, `code_writer_completion_repair_turn`, `mixed` for `completion_mode`, and `complete`, `partial`, `missing_required_outputs` for receipt `completion_status`, or the proposal is updated and all clients/tests/docs agree on the new contract.

### REL-001 - The generic-repair-already-failed block reason is absent

| Field | Value |
|---|---|
| Reviewer | `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer` |
| Severity | Major |
| Confidence | High |
| Related requirements | REQ-007, REQ-008 |
| Evidence types | proposal, code search, code |
| Evidence references | Proposal requires ineligible attempts to block with `generic_repair_already_failed_completion_contract_required` when generic repair already failed at lines 299-300 and 904-906. `rg` found no implementation of that string. Current repair selection still falls back to `output_contract_repair_prompt` for ineligible P088 candidates at `executor.rs:5157-5161`. |

Why it matters: The proposal's budget model depends on not adding another generic repair turn after the generic path already failed. Without the explicit block, the system can misdiagnose the exact "already spent generic repair" edge P088 is supposed to make actionable.

Recommended action: Add the generic-repair-already-failed detection keyed by `agent_execution_id` and generation. For eligible `code_writer` attempts, allow only `code_writer_completion_repair_v1`; for ineligible attempts, block with the exact diagnostic and persisted evidence.

Acceptance criteria: A focused test proves that after a failed generic repair for the same execution/generation, an ineligible attempt does not run generic repair again and persists `generic_repair_already_failed_completion_contract_required`.

### API-002 - Completion-text and transcript absence reasons do not cover the proposal vocabulary

| Field | Value |
|---|---|
| Reviewer | `api_contract_reviewer`, `observability_rollout_reviewer` |
| Severity | Major |
| Confidence | High |
| Related requirements | REQ-006, REQ-012, REQ-013 |
| Evidence types | proposal, code, tests-found |
| Evidence references | Proposal completion-text reasons are listed at lines 532-542 and transcript reasons at lines 551-564. ACP only models `NoTerminalOrStreamText` and `EmptyAfterSanitization` at `acp/src/lib.rs:330-335`. Engine maps absence at `executor.rs:9316-9330` and transcript status at `9345-9360`, producing only `provider_did_not_emit_text`, `redacted_only`, `storage_write_failed`, and `provider_did_not_supply` in normal paths. Tests at `executor.rs:11177-11230` assert that reduced set. |

Why it matters: P088's operator value is typed diagnosis at the boundary. Missing reasons such as `terminal_response_without_text`, `raw_capture_disabled`, `redaction_failed`, `redacted_storage_write_failed`, `capture_disabled`, `capture_failed`, and `session_reuse_without_terminal_capture` collapse distinct remediation paths back into generic evidence gaps.

Recommended action: Expand the ACP/engine absence enums and persistence mappings to the proposal vocabulary, and add targeted tests for each reason or an explicit "not observable in this runtime" mapping approved in reference docs.

Acceptance criteria: Completion-text and transcript absence reason tests cover every proposal-listed value or a documented, proposal-approved subset; no runtime path emits `redacted_only` as an absence reason unless the proposal/reference contract includes it.

### REL-002 - Historical operator retry recovery is only partially wired

| Field | Value |
|---|---|
| Reviewer | `rust_reliability_reviewer`, `observability_rollout_reviewer` |
| Severity | Major |
| Confidence | Medium |
| Related requirements | REQ-004, REQ-024 |
| Evidence types | proposal, code search, code |
| Evidence references | Proposal allows `operator_retry_completion_recovery` only for explicit retry of a historical run that references preserved evidence at lines 271-277 and 342-345. Implementation detects payload markers and preserved evidence path at `executor.rs:9422-9452`, and eligibility accepts that boolean at `5144-5148`. Search found no P088-specific operator retry integration test or command-level wiring that proves the retry command populates this shape. |

Why it matters: One of the proposal's operator promises is targeted retry without forensic digging. A low-level payload escape hatch is useful, but it does not prove the actual operator retry flow can invoke the new completion recovery path safely and repeatably.

Recommended action: Wire the operator retry command/path that creates targeted retries to populate the P088 preserved evidence reference. Add an integration test from failed historical evidence to retry payload to P088 completion eligibility and readback.

Acceptance criteria: A test exercises an explicit operator retry of a historical completion-handoff failure and asserts `activation_source=operator_retry_completion_recovery`, preserved evidence path, completion repair eligibility, and operator readback.

### UI-001 - Swift P088 tests use stale/non-contract public values

| Field | Value |
|---|---|
| Reviewer | `macos_ui_reviewer`, `api_contract_reviewer` |
| Severity | Minor |
| Confidence | High |
| Related requirements | REQ-016, REQ-023 |
| Evidence types | code, tests-found |
| Evidence references | Swift tests use `repair_failed`, `retry_completion_repair`, and `inspect_failed_stage_evidence` at `Proposal088OperatorReadbackTests.swift:11-25`, `70-87`, and `RunTimelineInspectorViewTests.swift:66-81`. Server public vocab is `status` values at `code_writer_completion.rs:198-207` and `next_operator_action` values at `231-243`. |

Why it matters: The Swift read-model is intentionally string-preserving, so these tests can pass while no server can emit the "known" values they assert. That weakens the UI parity signal and can hide a real client/server contract drift.

Recommended action: Update Swift tests to use server-known values, plus a separate unknown-value test that sets `known=false` and checks the UI still degrades gracefully.

Acceptance criteria: Swift tests cover at least one server-known failed P088 readback and one unknown future value, using `known=false` for the unknown case.

## Readiness Checklist

| Check | Result | Evidence |
|---|---|---|
| Focused P088 gate | Pass | `./scripts/test-gate.sh proposal-088` passed on this tree; static fixtures passed; ACP 3 tests, domain 1 test, DB 5 tests, engine unit 8 tests, engine integration 2 tests, GraphQL 1 test, MCP 2 tests all passed in the P088 filter. |
| macOS app build / fast gate | Fail | `./scripts/test-gate.sh fast` failed with exit 65 and `** BUILD FAILED **`; follow-up build grep also ended in `** BUILD FAILED **` with failed SwiftCompile group and 3 failures. |
| Full regression / canonical full gate | Not run | Not required for a failing readiness verdict; cannot claim Ready/Ready with Risks. |
| Core Rust service flow integration | Partial pass | P087 stale active canary and startup partial-write recovery integration tests pass. No live targeted retry flow was run. |
| Public API/readback contract | Partial pass | GraphQL/MCP focused tests pass, but internal receipt vocab drifts from the proposal. |
| DB migration/replay/canonical readback | Pass | P088 DB tests cover prompt-level rows, conflict, canonical link selection, and empty canonical result when links are missing. |
| UI runtime/screenshot validation | Not run | App build fails. |
| Empty/loading/error/offline/permission states for UI | Not verified | Source-only UI inspection; no runtime. |
| Accessibility/localization/privacy/permissions/entitlements risk | Medium | UI adds accessibility labels/identifiers, but no build/runtime/accessibility validation; no new privacy/permission/entitlement surface found. |
| Rollout/gate coverage | Partial | Focused Rust gate passes and is documented, but UI scope is not covered by `proposal-088`. |

## Verification Log

| Command / inspection | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...088...md` | Selected `docs/proposals/088-code-writer-completion-contract-and-output-freshness_IMPLEMENTATION_AUDIT_R5.md`. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...088...md` | Returned no prior proposal-review artifacts. |
| `git status --short --branch` | Dirty `main...origin/main`; many P088 and non-P088 modified/untracked files. |
| `git rev-parse HEAD` | `b18587e63e39913cd7cb611de88b090a8e8ad5ca`. |
| `shasum -a 256 docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` | `4a2a5c99988d7991e967862158153c5438189d705a65eb1faf28c25a067cf690`. |
| `./scripts/test-gate.sh proposal-088` | Passed. Static fixture checks passed; P088-filtered Rust tests passed across ACP/domain/DB/engine/GraphQL/MCP. |
| `./scripts/test-gate.sh fast` | Failed, exit 65, macOS build failure before tests. |
| `./scripts/test-gate.sh build` filtered for failure lines | Failed with `** BUILD FAILED **`; failed SwiftCompile group included P088-touched UI files and `SettingsView.swift`; final summary reported 3 failures. |
| Focused `rg`/`nl` inspections | Checked proposal lines, P088 receipt domain/migration/repo, executor repair/receipt logic, startup recovery, GraphQL/MCP readback, fixtures, gate, Swift read model/UI/tests, and absence/retry vocabularies. |

## Final Verdict

**Overall conformance: Not Implemented.** The implementation contains a large, useful Rust/control-plane P088 slice and resolves several earlier blockers, especially P037 stale active routing, canonical receipt readback, and startup partial-write recovery. However, one explicit atomic requirement is missing (`generic_repair_already_failed_completion_contract_required`), and several proposal-contract requirements remain partial.

**Overall readiness: Not Ready.** The focused Rust gate passes, but the macOS app build fails on the audited tree while P088 includes operator UI scope. API/receipt vocabulary drift, incomplete typed absence reasons, and unproven historical operator retry recovery also block closeout.

Recommended next actions:

1. Fix the macOS build failure and rerun `./scripts/test-gate.sh fast`.
2. Implement and test `generic_repair_already_failed_completion_contract_required`.
3. Align receipt `completion_mode` and `completion_status` values with the proposal, or formally amend the proposal/reference contract.
4. Expand completion-text/transcript absence reason vocabularies and tests to the proposal-listed values.
5. Add a P088 operator retry integration test proving `operator_retry_completion_recovery` from historical evidence through readback.
6. Update Swift P088 tests to use server-known values plus explicit unknown-value cases.
