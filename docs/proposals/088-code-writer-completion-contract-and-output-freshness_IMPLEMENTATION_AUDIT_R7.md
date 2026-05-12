# Proposal 088 Implementation Audit R7

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` |
| Proposal checksum | `4a2a5c99988d7991e967862158153c5438189d705a65eb1faf28c25a067cf690` |
| Audit report | `docs/proposals/088-code-writer-completion-contract-and-output-freshness_IMPLEMENTATION_AUDIT_R7.md` |
| Audit timestamp | `2026-05-12T05:12:19Z` |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| HEAD | `b18587e63e39913cd7cb611de88b090a8e8ad5ca` |
| Implementation target | Current worktree |
| Compare base | Implicit current branch/HEAD; no PR or commit range supplied |
| Proposal state | `Draft` in the proposal metadata; treated as the requested audit contract for this run |
| Overall Conformance | `Implemented` |
| Overall Implementation Readiness | `Ready with Risks` |
| Reviewer Selection Reuse | `Not reused` |
| Audit Confidence | `High` |

## Implementation Target / Compare Base

The audit target is the current dirty worktree on `main` at HEAD `b18587e63e39913cd7cb611de88b090a8e8ad5ca`. The user supplied only the proposal path, so the compare base is implicit. The implementation spans Rust control-plane crates, SQLite migration/repositories, GraphQL, MCP/run report readback, Swift read models/presentation, deterministic evidence fixtures, reference gate documentation, and `scripts/test-gate.sh`.

The worktree was already dirty before this report was written. Notable P088 surfaces include:

- `control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql`
- `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs`
- `control-plane/crates/domain/src/code_writer_completion.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/engine/src/worktree_fingerprint.rs`
- `control-plane/crates/graphql-server/src/types/run.rs`
- `control-plane/crates/mcp-server/src/tools/runs.rs`
- `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
- `Chainworks Forge/Views/RunTimelineInspectorView.swift`
- `Chainworks Forge/Views/RunsHomeView.swift`
- `docs/evidence/088-code-writer-completion/`
- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`

## Prior Proposal-Review Reuse

No prior proposal-review artifacts were discovered by the bundled helper:

```json
{"artifacts":[],"proposal_path":"/Users/user/Documents/Chainworks Forge/docs/proposals/088-code-writer-completion-contract-and-output-freshness.md","repo_root":"/Users/user/Documents/Chainworks Forge"}
```

Prior `IMPLEMENTATION_AUDIT` reports were not used for reviewer selection, per the audit skill rules. Current routing was derived from the proposal, repo-local routing expectations, and the implementation surfaces.

## Selected Reviewers

| Reviewer | Why Selected |
|---|---|
| `chainworks_execution_truth_reviewer` | P088 changes durable Run/Stage/Agent/receipt/artifact/recovery truth. |
| `rust_reliability_reviewer` | The core risk is repair lifecycle, idempotency, retry, terminalization, and startup recovery. |
| `api_contract_reviewer` | P088 adds GraphQL/MCP/run-report readback, closed public vocabularies, and SQLite persistence contracts. |
| `observability_rollout_reviewer` | The proposal requires deterministic fixtures, gates, migration behavior, failed-stage evidence, and support/debuggability. |
| `macos_ui_reviewer` | Operator readback is surfaced in the macOS app run row/timeline presentation. |

## Rejected Close Alternatives

| Reviewer | Reason Rejected |
|---|---|
| `rust_arch_reviewer` | Relevant, but covered by execution-truth, reliability, and API-contract lenses under the hard cap. |
| `apple_arch_reviewer` | Swift work is read-model/presentation integration, not new Apple state ownership or navigation architecture. |
| `product_reviewer` | No proposal metrics, experiment gates, or product decision checkpoints are central. |
| `rust_security_reviewer` | No auth, secret handling, unsafe/FFI, or public trust boundary expansion was identified. |
| `performance_reviewer` | No latency/throughput benchmark target is part of P088; bounded capture behavior is covered by tests and gates. |

## Proposal Contract Summary

Proposal 088 is a code-writer completion handoff contract. It explicitly keeps strict output validation intact while adding current-attempt completion receipts, worktree fingerprint freshness proof, a dedicated one-turn `code_writer_completion_repair_v1` branch, prompt-level runtime/text evidence, SQLite ownership, GraphQL/MCP/run-report/operator readback, deterministic fixtures, and a focused gate.

Key proposal sources:

- Scope/non-goals: lines 5-11 and 144-152.
- Goals and provider independence: lines 135-142.
- Receipt authority and minimum fields: lines 197-267.
- Repair lifecycle, eligibility, mutation guard, and work-change classification: lines 269-359.
- Worktree fingerprint schema/classifier: lines 361-415.
- Freshness, manifest advisory behavior, and repair ordering: lines 417-494.
- Completion text and transcript diagnostics: lines 496-565.
- Operator readback and canary closure: lines 566-611.
- SQLite/runtime facts/session events/readback shape: lines 613-863.
- Tests, focused gate, and acceptance criteria: lines 900-1016.

## Platform / Product Scope

| Scope | Classification |
|---|---|
| Apple | `macOS`; operator app readback/presentation only. No iOS scope. |
| Backend/service | Cross-stack Rust control-plane worker/API/data/rollout scope: ACP capture, engine repair lifecycle, SQLite persistence, GraphQL, MCP, run-report JSON, startup recovery, and test gates. |
| Product | Operator diagnostics and recovery trust for blocked implementation runs. No metrics or experiment checkpoint. |

## Primary Implementation Flows

1. Normal `code_writer` final text contains usable current-attempt `CHAINWORKS_OUTPUT`; declared-output settlement materializes outputs without completion repair.
2. A `code_writer` attempt changes implementation-owned files but required structured outputs are missing; the engine records fingerprints, writes a P088 completion receipt, and uses one `code_writer_completion_repair_v1` turn when eligible.
3. A P087-shaped stale `implementation_active` handoff with real current-attempt diff is terminalized into P088 diagnosis/readback instead of bypassing receipt persistence through active-prompt auto-requeue.
4. Operator-targeted retry for a historical blocked run carries preserved evidence into the retry payload, activation source, and readback.
5. Operators inspect the failure through GraphQL, MCP/run report, and the macOS run row/timeline readback with raw/known public enum handling and evidence paths.

## Fidelity / Divergence Inventory

### Matches

- Deterministic P088 evidence fixtures exist under `docs/evidence/088-code-writer-completion/`, including terminal-completed missing outputs, 70c9-shaped inherited dirty work, provider independence, normal materialization, mutation guard, truncation, prompt-side evidence, partial-write recovery, and public enum round-trip fixtures.
- SQLite migration and repositories add prompt-level runtime receipts, canonical code-writer completion receipts, text captures, output decisions, and receipt links.
- Engine code captures pre/post worktree fingerprints, classifies current-attempt vs preexisting/control-plane/generated changes, routes eligible attempts through `code_writer_completion_repair_v1`, and persists receipt/evidence/readback.
- The repair branch uses the existing one-turn repair budget and records dedicated `code_writer_completion_*` session events.
- GraphQL, MCP, run-report, and Swift read models expose `implementationCompletion` and preserve unknown public enum values.
- Provider-independence is now fixture-backed and test-backed for `claude`, `codex`, and `junie`.
- Targeted retry recovery now has payload plus GraphQL activation/readback coverage.
- The focused P088 gate and the broad `fast` gate both passed on the audited tree.

### Divergences

- `control-plane/crates/domain/src/code_writer_completion.rs` treats `generic_repair_already_failed_completion_contract_required` as a known `completion_turn_result` value. The proposal names that value as a required block reason in Section 11.1, but Section 9.4's `completion_turn_result` vocabulary list does not include it. This is a documentation/API-source alignment risk, not a conformance blocker.
- The `implementation_active` canary is an executable engine canary for the InvokeAgent/P037-style stale handoff branch; `docs/reference/test-gates.md` explicitly says it is not a full live ACP supervisor timeout end-to-end proof.
- The proposal itself is still marked `Draft`, and the audited implementation remains uncommitted/untracked in the working tree.

### Ambiguities / Evidence Gaps

- No remote UI smoke test was run. Repository policy says UI tests are remote-only unless explicitly requested.
- `./scripts/test-gate.sh full` was not run. The same-tree focused P088 gate and same-tree `fast` gate did pass.
- No live external provider session was exercised during this audit; provider behavior is covered by deterministic fixtures and focused ACP/engine tests.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 22 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed REQ Audit

| REQ | Proposal Source | Status | Evidence | Implementation Mapping / Gap |
|---|---|---|---|---|
| REQ-001 Distinct code-writer completion failure family | AC 1, lines 995; readback classes lines 566-589 | Implemented | code, tests-run | `executor.rs` derives `terminal_response_completed_missing_required_outputs` and `work_completed_missing_current_attempt_outputs`; domain/readback projects those classes; focused gate passed. |
| REQ-002 Stale previous-attempt files cannot satisfy current-attempt settlement | AC 2, lines 996; freshness rules lines 417-433 | Implemented | code, tests-found, tests-run, fixture | Output decisions preserve stale output names/counts; worktree fingerprint and fixture checks cover stale/preexisting cases; P088 gate passed. |
| REQ-003 Completion eligibility requires current-attempt fingerprints, except explicit historical recovery | AC 3, lines 997; eligibility lines 342-359 | Implemented | code, tests-run | `worktree_fingerprint.rs` derives `current_attempt_diff` vs `preexisting_dirty_work`; `executor.rs` allows `operator_retry_completion_recovery` only with preserved evidence; GraphQL targeted retry test covers activation/readback. |
| REQ-004 Preexisting dirty timeout is not classified as current-attempt completion | AC 4, line 998; lines 347-359 | Implemented | fixture, tests-run | `p087-70c9-dirty-worktree-timeout.fixture.json`, worktree fingerprint tests, and gate static checks prove inherited dirty paths stay `preexisting_dirty_work`. |
| REQ-005 Every such failure persists completion receipt plus runtime/transcript evidence or typed absence | AC 5, line 999; diagnostics lines 496-565 | Implemented | migration, code, tests-run | Migration creates receipt/text/output tables; executor persists receipt, text captures, failed-stage evidence, transcript absence; DB and engine tests passed. |
| REQ-006 Dedicated `code_writer_completion_repair_v1` can recover eligible attempts without full retry/generic prompt/budget expansion | AC 6, line 1000; lines 269-326, 455-481 | Implemented | code, tests-run | `executor.rs` swaps eligible repair prompt to `code_writer_completion_repair_v1`, reuses the same session/generation and one repair count, and records success/failure events. |
| REQ-007 Completion repair cannot mutate non-output repo files | AC 7, line 1001; mutation guard lines 328-340 | Implemented | code, fixture, tests-run | `executor.rs` captures pre/post repair fingerprints and fails closed with mutation guard evidence; fixture/gate cover unexpected mutation. |
| REQ-008 Original and completion-repair runtime receipts are separate and non-overwriting | AC 8, line 1002; lines 630-646, 725-727 | Implemented | migration, code, tests-run | Prompt-level runtime receipt primary key is `(agent_execution_id, prompt_kind, turn_index)`; DB test preserves original plus `code_writer_completion_repair` rows. |
| REQ-009 SQLite receipt writes are transactional, idempotent, conflict-detecting, canonical, and recover partial writes | AC 9, line 1003; transaction rules lines 717-729 | Implemented | migration, code, tests-run | `upsert_with_runtime_receipts` uses one DB transaction; replay drift returns `completion_receipt_conflict`; canonical readback uses receipt links; startup repair recovers partial artifact-only receipt. |
| REQ-010 Terminal text is inspectable independent of transcript availability | AC 10, line 1004; lines 498-565 | Implemented | code, tests-run | ACP capture metadata, executor text artifact persistence, receipt text capture rows, and tests cover captured/unavailable text independent of transcript status. |
| REQ-011 Completion text capture records source, limits, truncation flags, SHA-256, and typed truncation failures | AC 11, line 1005; lines 511-549 | Implemented | code, tests-run | ACP transport tests cover terminal final response preference, streamed tail capture, truncation metadata, and empty terminal text classification. |
| REQ-012 Prompt-side evidence records template/version/hash/redacted prompt/output snapshot/reason | AC 12, line 1006; lines 628-646, 733-765 | Implemented | code, tests-run, fixture | Runtime prompt receipt rows include prompt template metadata and expected-output snapshot fields; P088 prompt-side fixture and DB tests passed. |
| REQ-013 `worktree_fingerprint_v1` explains path inclusion/status/digest/count derivation | AC 13, line 1007; lines 361-415 | Implemented | code, fixture, tests-run | `worktree_fingerprint.rs` implements schema, classifier, deterministic path ordering, content SHA, derived summary counts, and proposal-owned evidence handling. |
| REQ-014 Public GraphQL/MCP/run-report readback has closed vocabularies with unknown handling | AC 14, line 1008; lines 778-863 | Implemented | code, tests-run | Domain `PublicEnumReadback`, GraphQL `PublicEnumReadback`, MCP tests, and Swift tests preserve raw unknowns with `known=false`. See API-001 note for one doc-alignment risk. |
| REQ-015 Operator readback explains fresh, stale, missing, and control-plane-generated outputs | AC 15, line 1009; lines 830-861 | Implemented | code, tests-run | Receipt records counts and output decisions; GraphQL/MCP/run-report/Swift presenter tests expose counts, paths, failure class, action, and capture rows. |
| REQ-016 Completed terminal responses with missing outputs are not `provider_active_without_terminal_response` | AC 16, line 1010; lines 577-586 | Implemented | code, tests-run | Executor uses `terminal_response_completed_missing_required_outputs` when runtime receipt status is `completed`; stale vocabulary search found no `inspect_failed_stage_evidence`/`normal_settlement` residue. |
| REQ-017 P087-like stale `implementation_active` attempts enter P088 diagnosis/recovery | AC 17, line 1011; canary lines 591-611 | Implemented | code, integration tests-run | Engine integration test `proposal_088_code_writer_stale_implementation_active_enters_receipt_path_not_auto_requeue` verifies `p037_idle_terminalization`, `current_attempt_diff`, receipt readback, and no active-prompt auto-requeue. |
| REQ-018 Usable final `CHAINWORKS_OUTPUT` materializes normally without completion repair | AC 18, line 1012; lines 464-477, 604-611 | Implemented | fixture, code, tests-run | `normal-materialization-no-repair.fixture.json`, ACP capture behavior, and gate checks cover successful normal settlement with `completion_turn_attempted=false`. |
| REQ-019 P087 terminal-completed and 70c9 dirty-worktree fixtures exist before implementation | AC 19, line 1013; lines 60-70 | Implemented | fixture, tests-run | Both `p087-terminal-completed-missing-outputs.fixture.json` and `p087-70c9-dirty-worktree-timeout.fixture.json` exist and are checked by the focused gate. |
| REQ-020 Readback does not claim to close non-`code_writer` failures | AC 20, line 1014; non-goals lines 150, 479-481 | Implemented | code, tests-run | Executor gates P088 candidate logic on `agent_id == "code_writer"`; non-code-writer repair lifecycle remains generic. |
| REQ-021 `proposal-088|p088` gate is registered and documented | AC 21, line 1015; focused gate lines 955-989 | Implemented | code, docs, tests-run | `scripts/test-gate.sh` contains the `proposal-088|p088` gate and logs "Proposal 088 gate passed"; `docs/reference/test-gates.md` documents the gate and scope. |
| REQ-022 Targeted retries no longer require forensic digging | AC 22, line 1016; activation source lines 271-276 | Implemented | code, tests-run | `command_handler.rs` attaches P088 retry payload with preserved historical evidence; GraphQL test verifies payload, activation source, preserved evidence path, and readback. |

## Reviewer / Lens Scorecard

| Lens | Conformance | Top Risk | Confidence |
|---|---|---|---|
| Proposal conformance | Implemented | Draft proposal state and dirty worktree mean handoff still needs closeout. | High |
| Chainworks execution truth | Pass | Canonical readback depends on receipt links; tests cover this. | High |
| Rust reliability | Pass | Live provider timeout proof is fixture/integration-backed, not live-provider runtime. | High |
| API contract | Pass with note | One public known enum value needs closeout documentation alignment. | High |
| Observability/rollout | Pass with note | Full gate and remote UI smoke were not run. | Medium |
| macOS UI | Pass with note | Presenter/unit coverage exists; no runtime screenshot/UI smoke was executed. | Medium |
| Readiness | Ready with Risks | Uncommitted target and non-full validation scope. | High |

## Routed Specialist Findings

### READY-001 Dirty/uncommitted audit target should be closed out before handoff

- Reviewer: `observability_rollout_reviewer`
- Severity: `Minor`
- Confidence: `High`
- Related REQs: all
- Evidence types: `diff`, `tests-run`
- Evidence references: `git status --short --branch`; current target includes many modified/untracked implementation files plus untracked proposal/evidence/reports.
- Why it matters: The implementation is conformant in the audited worktree, but handoff remains fragile until the proposal, migration, evidence fixtures, Rust/Swift changes, gate docs, and generated audit report are made durable on the intended branch.
- Recommended action: Run proposal closeout/branch hygiene after this audit: ensure the proposal is tracked, include deterministic fixtures and tests, commit or otherwise preserve the work, and record the green gate evidence.
- Acceptance criteria: A durable branch/commit contains the P088 implementation, proposal, evidence fixtures, gate docs, and this audit; `git status --short` contains only intentional local noise or is clean.

### API-001 Align the extra known `completion_turn_result` value with the public vocabulary source

- Reviewer: `api_contract_reviewer`
- Severity: `Note`
- Confidence: `High`
- Related REQs: REQ-006, REQ-014
- Evidence types: `proposal`, `code`, `tests-run`
- Evidence references: Proposal line 905 requires `generic_repair_already_failed_completion_contract_required`; Section 9.4 lines 804-814 omits it from `implementationCompletion.completion_turn_result`; `control-plane/crates/domain/src/code_writer_completion.rs` includes it as a known completion-turn result.
- Why it matters: The behavior is justified by the proposal's generic-repair guard, but Section 9.4 is the public readback vocabulary source. Leaving the source list behind the implementation can confuse client authors and future audits.
- Recommended action: During closeout, update the durable API/reference truth or proposal text so the public known value list includes this diagnostic value, or document its mapping to an existing public value.
- Acceptance criteria: The public readback vocabulary source and implementation agree, and GraphQL/MCP/Swift unknown-handling tests remain green.

## Readiness Checklist

| Item | Status | Evidence / Note |
|---|---|---|
| Canonical P088 gate | Passed | `./scripts/test-gate.sh proposal-088` exited 0; static fixture checks passed; ACP/domain/DB/engine/GraphQL/MCP P088 tests passed; final log: `Proposal 088 gate passed`. |
| Broad same-tree regression | Passed | `./scripts/test-gate.sh fast` exited 0; Xcode result summary: `result=Passed`, `totalTestCount=179`, `failedTests=0`, `skippedTests=0`. |
| Build status | Passed | Fast gate build phase printed `** BUILD SUCCEEDED **` before tests. |
| Core service flow validation | Passed | Rust P088 focused tests cover ACP capture, DB persistence, engine repair/canary/recovery, GraphQL targeted retry/readback, and MCP/run-report readback. |
| Operator readback UI validation | Passed with bounded evidence | Swift unit tests cover decode/presenter/run-row unknown handling. No runtime UI smoke or screenshot was run. |
| Empty/loading/error/offline/permission states | Not central | P088 does not add new UI workflow screens. Error/readback states are covered by presenter tests and receipts. |
| Accessibility/localization | Bounded risk | Run-row accessibility label includes implementation completion signal in tests. No localization pass was run. |
| Privacy/permissions/entitlements | No new scope found | No new entitlement or secret boundary was identified in the audited P088 surfaces. |
| Migration/persistence | Passed | Migration and DB repository tests cover prompt-level runtime receipts, receipt round-trip, idempotent/conflict behavior, provider independence, canonical link readback, and missing-link behavior. |
| Full gate | Not run | Same-tree P088 gate plus `fast` gate passed; `./scripts/test-gate.sh full` was not executed in this audit. |
| Remote UI smoke | Not run | Repository policy makes UI smoke remote-only and it was not explicitly requested. |

## Verification Log

| Command / Check | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` | Returned this R7 report path. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py .../088-code-writer-completion-contract-and-output-freshness.md` | Returned no prior proposal-review artifacts. |
| `git rev-parse HEAD` | `b18587e63e39913cd7cb611de88b090a8e8ad5ca`. |
| `git branch --show-current` | `main`. |
| `shasum -a 256 docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` | `4a2a5c99988d7991e967862158153c5438189d705a65eb1faf28c25a067cf690`. |
| `git status --short --branch` | Dirty worktree with P088 implementation/evidence plus unrelated existing local changes. |
| `rg` and targeted file reads across proposal, Rust, Swift, scripts, docs, and fixtures | Confirmed contract mapping, stale vocabulary cleanup, provider-independence coverage, and targeted retry readback. |
| `./scripts/test-gate.sh proposal-088` | Passed on the audited tree; final output included `proposal-088 static fixture checks passed` and `==> Proposal 088 gate passed`. |
| `./scripts/test-gate.sh fast` | Passed on the audited tree; XCTest run reported `** TEST SUCCEEDED **`. |
| `xcrun xcresulttool get test-results summary --path /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/fast-20260512-075901.xcresult` | `result=Passed`, `totalTestCount=179`, `failedTests=0`, `skippedTests=0`; one long outlier test consumed most of the duration. |

## Final Verdict

Overall Conformance: `Implemented`.

All 22 in-scope P088 acceptance requirements are implemented with direct code, fixture, and same-tree gate evidence. The two important prior blockers are now closed: provider-independence is fixture/test-backed for `claude`, `codex`, and `junie`, and targeted retry recovery carries preserved evidence into activation/readback instead of requiring forensic reconstruction.

Overall Implementation Readiness: `Ready with Risks`.

There are no critical or major implementation blockers in this audit. The remaining risks are closeout/readiness risks: the implementation is still a dirty/uncommitted worktree, the proposal remains `Draft`, `./scripts/test-gate.sh full` and remote UI smoke were not run, and the extra public known `completion_turn_result` value should be synchronized into durable API/reference truth during closeout.

Recommended next actions:

1. Close out the P088 branch by committing/preserving the implementation, proposal, fixtures, gate docs, and this audit report.
2. Align the public vocabulary documentation/reference for `generic_repair_already_failed_completion_contract_required`.
3. Preserve the green `proposal-088` and `fast` gate evidence in closeout notes; run `full` or remote UI smoke only if the release/handoff policy requires those broader checks.
