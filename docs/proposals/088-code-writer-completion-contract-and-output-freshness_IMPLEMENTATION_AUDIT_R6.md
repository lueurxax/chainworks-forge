# Proposal 088 Implementation Audit R6

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` |
| Proposal checksum | `4a2a5c99988d7991e967862158153c5438189d705a65eb1faf28c25a067cf690` |
| Audit timestamp UTC | `2026-05-12T04:22:34Z` |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| HEAD | `b18587e63e39913cd7cb611de88b090a8e8ad5ca` |
| Implementation target | Current worktree on `main` |
| Compare base | Implicit current worktree audit; no PR/range supplied |
| Worktree state | Dirty, with P088 implementation files, proposal file, prior R1-R5 audit reports, Swift UI changes, Rust control-plane changes, docs, fixtures, and unrelated local changes present |
| Overall Conformance | Partial |
| Overall Implementation Readiness | Not Ready |
| Reviewer Selection Reuse | Not reused |
| Audit Confidence | High |

## Implementation Target / Compare Base

The audit inspected the current worktree rather than a clean diff. The target is `main` at HEAD `b18587e63e39913cd7cb611de88b090a8e8ad5ca`, plus unstaged and untracked working-tree changes.

The proposal itself is currently untracked in this worktree. Prior implementation audit reports R1-R5 are also untracked and were ignored for reviewer selection because the skill only reuses prior proposal-review artifacts unless explicitly instructed otherwise.

## Prior Review Reuse

Prior proposal-review discovery returned no artifacts:

```json
{"artifacts":[],"proposal_path":"/Users/user/Documents/Chainworks Forge/docs/proposals/088-code-writer-completion-contract-and-output-freshness.md","repo_root":"/Users/user/Documents/Chainworks Forge"}
```

Reuse state: `Not reused`.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_reliability_reviewer` | P088 changes retry, repair, idempotency, receipt conflict, startup recovery, stale active terminalization, and workqueue behavior. |
| `api_contract_reviewer` | P088 adds public GraphQL, MCP, run-report, closed vocabulary, and forward-compatible unknown readback contracts. |
| `observability_rollout_reviewer` | P088 adds operator diagnostics, evidence paths, receipt artifacts, test gates, and recovery/readiness guarantees. |
| `chainworks_execution_truth_reviewer` | P088 changes durable run/stage/agent/output truth boundaries and separates transition evidence from transition truth. |
| `macos_ui_reviewer` | P088 includes operator readback/UI scope in the macOS SwiftUI app. |

Rejected close alternatives:

| Reviewer | Reason rejected |
|---|---|
| `rust_arch_reviewer` | Covered indirectly by reliability/API/data review; no new broad crate architecture question was needed beyond the P088 contract. |
| `apple_arch_reviewer` | Swift changes are readback/presentation-bound; no new app state ownership or navigation architecture contract dominates the audit. |
| `product_reviewer` | No proposal metrics or product decision checkpoints are central. |
| `security_reviewer` | No new auth, secrets, public capability, or unsafe boundary was identified in the inspected P088 slice. |
| `performance_reviewer` | No explicit performance target or hot-path benchmark commitment is part of P088. |
| Go reviewers | No real `go.mod` or Go implementation surface exists. |

## Proposal State And Contract Summary

Proposal state: `Ambiguous/Draft`. The proposal line 6 marks status as `Draft`, but the user requested implementation audit and the current worktree contains a broad P088 implementation. I treated the proposal as the active audit contract while preserving the Draft caveat.

Core contract:

- P088 targets the `code_writer` completion-handoff class where useful implementation work exists but fresh structured outputs for the current attempt do not settle.
- It explicitly does not weaken output contracts, accept stale files as fresh truth, add provider-specific hotfixes as the main contract, fix all non-`code_writer` missing-output cases, or silently repair historical blocked runs.
- It requires a durable `code_writer_completion_receipt_v1`, prompt-level runtime receipts, worktree fingerprint freshness proof, completion-text capture independent of transcript capture, typed absence reasons, GraphQL/MCP/run-report readback, and a focused `proposal-088|p088` gate.
- It requires targeted retries for current blocked runs to carry preserved completion evidence so operators do not need forensic digging to distinguish provider failure from completion-handoff failure.

## Platform / Product Scope

Apple scope: `macOS`.

Backend/service scope: cross-stack service, worker, API, data, rollout, and operator-readback scope:

- Rust control-plane executor, command handler, ACP transport, DB persistence, GraphQL, MCP, run reports, session events, and test gate registration.
- macOS SwiftUI operator readback and timeline/presenter surfaces.

## Primary Flows Audited

1. Original `code_writer` attempt performs implementation work, misses required outputs, and is classified into the P088 completion failure family rather than generic missing output.
2. Eligible missing-output attempt runs a single `code_writer_completion_repair_v1` turn within the shared repair budget, records prompt/runtime evidence, and fails closed on unexpected non-output mutation.
3. Stale `implementation_active` attempt with real current-attempt diff is terminalized into P088 receipt/readback instead of active-prompt auto-requeue.
4. Operator/API readback exposes canonical receipt truth through run report, MCP, GraphQL, and macOS presentation with closed known values and unknown-value preservation.
5. Targeted retry of a historical P088 failure carries preserved failed-stage or receipt evidence so the retry can activate `operator_retry_completion_recovery`.

## Fidelity Inventory

Matches:

- P088 receipt schema and SQLite tables exist in `control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql`.
- DB repository writes receipt, text captures, output decisions, receipt links, and runtime receipts in one transaction through `upsert_with_runtime_receipts`.
- Receipt `completion_mode` now maps to proposal vocabulary: `provider_envelope`, `acp_final_text_chainworks_output`, `exact_path_current_attempt`, `code_writer_completion_repair_turn`, and `mixed`.
- Receipt `completion_status` now maps to `complete`, `partial`, and `missing_required_outputs`.
- Generic-repair-already-failed guard now blocks ineligible `code_writer` candidates with `generic_repair_already_failed_completion_contract_required`.
- Completion-text absence reasons now map to proposal vocabulary for provider no-text, terminal-response no-text, truncation, raw capture disabled, redaction/storage failures, and redacted storage failures.
- Focused P088 gate is registered and passed on this same worktree.
- macOS readback models and tests exist for P088 `implementationCompletion`, including unknown future enum preservation.

Divergences:

- `./scripts/test-gate.sh fast` fails on this same worktree, so the implementation is not ready even though the focused P088 gate passes.
- Provider-independence proof remains incomplete against proposal Section 11.4: P088 tests cover `claude` and `codex` fixtures, but no fixture-backed P088 completion-contract test covers `junie`.
- Targeted retry completion recovery has payload wiring and unit coverage, but no end-to-end operator retry proof showing the preserved evidence travels through enqueue, execution activation, receipt generation, and operator readback.
- Some test/preview fixtures still use non-contract vocabulary, such as `completion_failure`, `completion_status: "failed"`, `activationSource: "normal_settlement"`, `completionTurnResult: "failed"`, and `nextOperatorAction: "inspect_failed_stage_evidence"`.

Ambiguities / Evidence Gaps:

- The focused stale-active canary intentionally does not claim a full live ACP supervisor timeout end-to-end, per `docs/reference/test-gates.md` lines 1913-1915.
- `p088_transcript_status` production code emits `unavailable` with `provider_did_not_supply` or `storage_write_failed`, while enum helpers cover the full typed set. The audit did not find production paths proving `capture_disabled`, `capture_failed`, or `session_reuse_without_terminal_capture`.
- The worktree is dirty and includes unrelated changes; this audit did not attempt attribution beyond P088-relevant surfaces.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 20 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed REQ Audit

| ID | Requirement | Proposal source | Status | Evidence |
|---|---|---|---|---|
| REQ-001 | Materialize deterministic P088 fixtures before implementation. | Lines 60-70, 1013 | Implemented | `docs/evidence/088-code-writer-completion/*`; `scripts/test-gate.sh` lines 6186-6236 checks required fixtures. |
| REQ-002 | Distinct `code_writer` completion failure family. | Lines 577-589, 995 | Implemented | `executor.rs` lines 9071-9090 maps missing outputs to `terminal_response_completed_missing_required_outputs`, `work_completed_missing_current_attempt_outputs`, and the generic-repair guard class. |
| REQ-003 | Stale previous-attempt files cannot satisfy current attempt. | Lines 99-107, 996 | Implemented | P088 gate passed stale exact-path fixture checks; focused gate covers stale output negative cases. |
| REQ-004 | Completion eligibility based on original-prompt worktree fingerprints, with historical recovery exception. | Lines 342-359, 997 | Partially Implemented | Fingerprint fixtures and executor tests exist; targeted historical recovery payload exists, but no full end-to-end targeted retry proof. |
| REQ-005 | Pre-existing dirty work plus provider timeout is not current-attempt completion. | Lines 347-359, 998 | Implemented | P088 fixture `p087-70c9-dirty-worktree-timeout.fixture.json`; gate checks `preexisting_dirty_work` and `do_not_retry_preexisting_dirty_timeout`. |
| REQ-006 | Persist completion receipt plus transcript/runtime evidence or typed absence reasons. | Lines 221-267, 496-565, 999 | Implemented | Migration lines 65-162; DB repo lines 44-76 and 87-231; text absence mapper in `executor.rs` lines 9432-9478. |
| REQ-007 | `code_writer_completion_repair_v1` branch can recover eligible attempts without generic prompt reuse or extra budget. | Lines 269-326, 1000 | Implemented | Executor lines 5144-5222 chooses dedicated completion prompt for eligible attempts; session events and P088 gate cover branch behavior. |
| REQ-008 | Generic repair already failed: eligible runs use only completion branch, ineligible runs block with typed reason. | Lines 299-300, 905 | Implemented | Executor lines 5160-5215 and helper lines 9587-9593; unit test lines 11508-11522. |
| REQ-009 | Completion repair cannot mutate non-output repo files; unexpected mutation fails closed. | Lines 328-340, 1001 | Implemented | P088 fixture `completion-repair-mutation-negative.fixture.json`; focused gate validates `failed_unexpected_worktree_mutation`. |
| REQ-010 | Original and completion-repair runtime receipts are separately persisted and cannot overwrite. | Lines 500-508, 1002 | Implemented | DB repo `upsert_with_runtime_receipts` lines 44-76; persistence test lines 324-363. |
| REQ-011 | SQLite receipt writes are transactionally linked, idempotent, conflict-detecting, and canonical for readback. | Lines 717-729, 1003 | Implemented | DB repo lines 21-40, 44-76, 215-231, 267-285; persistence tests reference conflict and canonical readback. |
| REQ-012 | Terminal completion text is inspectable as redacted/raw/typed absence independent of transcript availability. | Lines 496-565, 1004 | Implemented | ACP absence enum lines 330-345; executor mapper lines 9432-9478; tests lines 11360-11475. |
| REQ-013 | Completion text capture records source, byte limits, truncation flags, extraction input SHA-256, and truncation failures. | Lines 511-549, 1005 | Implemented | Migration lines 113-130; GraphQL seed lines 207-223; P088 gate covers large-streamed prelude/tail and truncation fixtures. |
| REQ-014 | Prompt-side evidence records template/version/hash/redacted prompt/expected-output snapshot/repair reason. | Lines 747-752, 914, 1006 | Implemented | Runtime receipt migration lines 3-27; DB readback lines 303-324; persistence test lines 324-355. |
| REQ-015 | `worktree_fingerprint_v1` explains path-level inclusion/exclusion and count derivation. | Lines 361-365, 1007 | Implemented | `control-plane/crates/engine/src/worktree_fingerprint.rs`; fixture `worktree-fingerprint-v1.fixture.json`; gate lines 6252-6263. |
| REQ-016 | GraphQL/MCP/run-report readback has closed vocabulary plus unknown handling. | Lines 778-863, 1008 | Implemented | Domain vocabulary lines 200-240; GraphQL readback tests lines 159-240; Swift readback test lines 96-132. |
| REQ-017 | Operator readback explains fresh/stale/missing/control-plane outputs. | Lines 568-589, 849-860, 1009 | Implemented | Swift model lines 1369-1385; Swift test lines 50-56; GraphQL test seed lines 193-204. |
| REQ-018 | Terminal completed plus missing outputs is not `provider_active_without_terminal_response`. | Lines 584-586, 1010 | Implemented | Executor lines 9071-9078; P088 fixture `p087-terminal-completed-missing-outputs.fixture.json`; focused gate checks expected failure class. |
| REQ-019 | P087-like stale `implementation_active` terminalizes into P088 diagnosis/recovery. | Lines 591-611, 1011 | Implemented | Integration test lines 10383-10540 proves receipt path and no active-prompt auto-requeue. |
| REQ-020 | Usable current-attempt `CHAINWORKS_OUTPUT` materializes through normal settlement without completion repair. | Lines 126-130, 608, 1012 | Implemented | Fixture `normal-materialization-no-repair.fixture.json`; gate lines 6213-6216. |
| REQ-021 | Provider-independence fixture-backed tests cover `junie`, `claude`, and `codex`. | Lines 945-953 | Partially Implemented | P088 tests seed `claude` and `codex`; search found no P088 fixture-backed Junie completion-contract test. |
| REQ-022 | Register and document `proposal-088|p088` focused gate. | Lines 955-989, 1015 | Implemented | `scripts/test-gate.sh` lines 6180-6357; `docs/reference/test-gates.md` lines 1894-1919; gate passed. |
| REQ-023 | macOS operator readback/UI displays the new failure family. | Lines 566-611, 862 | Implemented | Swift model/query lines 1337-1385 and 2225-2255; timeline integration lines 164-184; Swift tests lines 5-134 and `RunTimelineInspectorViewTests.swift` lines 62-95. |
| REQ-024 | Targeted retries for current blocked runs no longer require forensic digging. | Lines 344-345, 1016 | Partially Implemented | Command handler attaches preserved evidence lines 1296-1314 and 4329-4407; unit tests lines 5611-5643 and executor parser test lines 11478-11506; no full retry/readback integration proof found. |

## Reviewer / Lens Scorecard

| Lens | Score | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Provider-independence and targeted retry recovery are not fully proven. | High |
| Rust reliability | Partial | Retry recovery is payload/unit-level, not end-to-end; fast gate exposes run lifecycle regressions. | High |
| API contract | Mostly Pass | Public readback is implemented, but stale vocabulary remains in some fixtures/previews. | High |
| Observability/rollout | Partial | Focused gate is strong, broad fast gate fails on same tree. | High |
| Execution truth | Mostly Pass | Receipt/link/canonical truth is much improved; historical retry truth still lacks end-to-end proof. | High |
| macOS UI | Mostly Pass | Readback surfaces exist and P088 UI tests are present; full fast suite still red. | Medium |
| Readiness | Fail | Same-tree `fast` gate failed with 8 failing tests / 20 issues. | High |

## Routed Specialist Findings

### READY-001 - Same-tree `fast` gate fails

- Reviewer: `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related requirements: REQ-023, readiness gate
- Evidence types: tests-run, log-or-trace
- Evidence references:
  - `./scripts/test-gate.sh fast` exited 65.
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/fast-20260512-071807.xcresult`
  - `xcrun xcresulttool get test-results summary` reported 179 total tests, 8 failed tests, result `Failed`.
- Why it matters: The skill requires same-tree full regression or canonical gate evidence for any successful readiness verdict. P088 touches macOS UI/readback and run lifecycle surfaces, and the broader fast gate currently fails.
- Failed tests summarized:
  - `RunTests/currentStageIDDerived()`
  - `ProviderPlatformTests/sampleRunLauncherCreatesFrozenProviderBindingSnapshot()`
  - `OrchestratorTests/geminiCapacityFallbackRerunsParallelExecutionWithStableModel()`
  - `OrchestratorTests/geminiCapacityFallbackRerunsSequentialExecutionWithStableModel()`
  - `OrchestratorTests/implementationPartialArtifactSetRecoversFailedCodeWriter()`
  - `ResumeManagerTests/executionServiceReconcilesStalledRunningRunAfterSessionClosed()`
  - `ResumeManagerTests/executionServiceReconcilesStalledRunWithStaleRunningAgentRows()`
  - `ResumeManagerTests/executionServiceApprovalResolutionPersistsDecisionForFreshContextReads()`
- Recommended action: Fix or explicitly isolate the fast-gate failures, then rerun `./scripts/test-gate.sh fast` on the same tree.
- Acceptance criteria: `./scripts/test-gate.sh proposal-088` and `./scripts/test-gate.sh fast` both pass on the same audited HEAD/worktree.

### REL-001 - Targeted retry recovery is not proven end-to-end

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-004, REQ-024
- Evidence types: code, tests-found
- Evidence references:
  - `control-plane/crates/engine/src/command_handler.rs` lines 1296-1314 adds the P088 retry payload.
  - `control-plane/crates/engine/src/command_handler.rs` lines 4329-4407 retrieves receipt/evidence and attaches the payload.
  - `control-plane/crates/engine/src/command_handler.rs` lines 5611-5643 tests payload construction.
  - `control-plane/crates/engine/src/executor.rs` lines 11478-11506 tests payload parser shape.
- Why it matters: The proposal's operator-facing closure is not just "payload contains evidence"; it is that current blocked runs no longer require forensic digging. That needs proof across command handling, queued retry execution, activation source, receipt readback, and operator/API visibility.
- Recommended action: Add a focused integration test that seeds a P088 receipt/evidence for a blocked `code_writer` execution, triggers targeted retry through `CommandHandler`, processes the retry, and asserts `activation_source=operator_retry_completion_recovery`, preserved evidence path, and readback-visible next action.
- Acceptance criteria: A P088 gate test fails without the end-to-end retry recovery path and passes with the full retry/readback chain.

### READY-002 - Provider-independence proof is incomplete

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-021
- Evidence types: proposal, tests-found, code search
- Evidence references:
  - Proposal lines 945-953 requires fixture-backed tests for `junie`, `claude`, and `codex`.
  - `rg` over P088 tests found `claude` and `codex` fixtures, but no P088 completion-contract fixture for `junie`.
  - `executor.rs` line 2794 maps `junie_acp` to `junie`, but mapping is not the same as fixture-backed completion-contract proof.
- Why it matters: The proposal explicitly argues the failure reproduces across providers and forbids provider-specific truth branches. Without a Junie P088 fixture, the implementation cannot prove the contract is provider-independent.
- Recommended action: Add a P088 fixture-backed test covering the same completion contract for Junie, and include it in `proposal-088|p088`.
- Acceptance criteria: The focused gate proves the same required-output missing/completion-repair/readback behavior for Junie, Claude, and Codex without provider-specific truth branches.

### API-001 - Some fixtures/previews still use non-contract P088 vocabulary

- Reviewer: `api_contract_reviewer`
- Severity: Minor
- Confidence: High
- Related requirements: REQ-006, REQ-016, REQ-023
- Evidence types: code, tests-found
- Evidence references:
  - `control-plane/crates/engine/tests/integration.rs` lines 10584-10595 seeds `completion_mode: "completion_failure"` and `completion_status: "failed"`.
  - `Chainworks Forge/Views/RunsHomeView.swift` lines 681-700 uses preview values `activationSource: "normal_settlement"`, `completionTurnResult: "failed"`, and `nextOperatorAction: "inspect_failed_stage_evidence"`.
  - `control-plane/crates/graphql-server/tests/proposal_088_code_writer_completion_readback.rs` lines 201-202 seeds transcript values outside the proposal's typed transcript vocabulary.
- Why it matters: Production mapping appears corrected, but stale test/preview values can normalize invalid vocabulary and weaken regression coverage for the public readback contract.
- Recommended action: Replace stale fixture/preview values with proposal values or intentionally mark them as unknown/future raw values with `known=false` where the test is about forward compatibility.
- Acceptance criteria: P088 fixtures, Swift previews, and public readback tests use only proposal-known values or explicit unknown wrappers.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Focused P088 gate | Pass | `./scripts/test-gate.sh proposal-088` passed. |
| Same-tree broad fast gate | Fail | `./scripts/test-gate.sh fast` failed with exit 65. |
| Build evidence | Partial | Fast gate built Rust and Xcode targets far enough to run tests, but final test result failed. |
| Core service flow integration | Partial | P088 stale-active integration and receipt persistence tests pass in focused gate; targeted retry recovery lacks end-to-end proof. |
| API contract validation | Mostly pass | GraphQL/MCP P088 tests exist and focused gate passes; stale fixture vocabulary remains. |
| macOS UI/readback validation | Partial | P088 Swift tests exist and are not among `xcresult` failures; full fast gate still fails. |
| Empty/loading/error/offline/permission UI states | Not central | P088 UI scope is diagnostic readback, not broader UI state design. |
| Accessibility/localization/privacy/permissions | No blocking P088-specific issue found | Not deeply runtime-validated. |
| Provider-independence tests | Partial | Claude/Codex covered; Junie missing for P088 completion contract. |
| Full regression/canonical gate for successful verdict | Fail | Broad fast gate failed; no successful readiness verdict allowed. |

## Verification Log

| Command / Inspection | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...` | Returned this report path: `docs/proposals/088-code-writer-completion-contract-and-output-freshness_IMPLEMENTATION_AUDIT_R6.md`. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...` | No prior proposal-review artifacts found. |
| `git rev-parse HEAD` | `b18587e63e39913cd7cb611de88b090a8e8ad5ca`. |
| `git status --short --branch` | Dirty `main...origin/main`, with P088 implementation files and unrelated local changes. |
| `shasum -a 256 docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` | `4a2a5c99988d7991e967862158153c5438189d705a65eb1faf28c25a067cf690`. |
| `./scripts/test-gate.sh proposal-088` | Passed. Covered static fixture checks, ACP/domain/DB/engine/GraphQL/MCP P088 Rust tests, stale-active integration, startup recovery, text/transcript vocabulary helpers, retry payload unit tests, and generic-repair guard unit tests. |
| `./scripts/test-gate.sh fast` | Failed with exit 65. Console reported 179 tests across 6 suites failed after 20.926s with 20 issues. |
| `xcrun xcresulttool get test-results summary --path /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/fast-20260512-071807.xcresult` | Result `Failed`, 179 total tests, 171 passed, 8 failed tests. |
| Targeted file inspections with `rg`, `nl`, and `sed` | Confirmed P088 receipt schema, completion mode/status vocabulary, completion-text absence vocabulary, generic-repair guard, retry payload wiring, UI readback models/tests, and stale fixture vocabulary. |

## Final Verdict

Overall Conformance: Partial.

Overall Implementation Readiness: Not Ready.

The implementation has materially improved and now satisfies most P088 contract items, including the previously important generic-repair-already-failed guard, proposal vocabulary for receipt status/mode, prompt-level completion-text absence reasons, focused P088 gate registration, stale-active canary coverage, and macOS readback surfaces.

It is not ready to close because the same-tree `fast` gate fails, provider-independence coverage is incomplete for Junie, and targeted retry recovery is not proven end-to-end from preserved evidence through retry execution and readback.

Recommended next actions:

1. Fix the failing `./scripts/test-gate.sh fast` tests or isolate non-P088 failures with explicit evidence, then rerun `fast`.
2. Add P088 fixture-backed provider-independence coverage for Junie alongside Claude and Codex.
3. Add an end-to-end targeted retry recovery test that proves `operator_retry_completion_recovery` from preserved historical evidence through retry activation and readback.
4. Clean stale P088 vocabulary from test fixtures and Swift previews, or mark future values through explicit unknown wrappers.
