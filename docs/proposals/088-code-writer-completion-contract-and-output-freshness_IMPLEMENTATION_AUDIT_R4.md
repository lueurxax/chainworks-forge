# Proposal 088 Implementation Audit R4: Code-Writer Completion Contract, Output Freshness, and Repair Diagnostics

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` |
| Proposal status | Draft |
| Audit report | `docs/proposals/088-code-writer-completion-contract-and-output-freshness_IMPLEMENTATION_AUDIT_R4.md` |
| Audit timestamp | 2026-05-11T19:16:27Z |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| HEAD | `b18587e63e39913cd7cb611de88b090a8e8ad5ca` |
| Compare base | Implicit current worktree audit |
| Worktree state | Dirty; P088 Rust/control-plane files are mixed with unrelated Swift/P036-looking local changes and prior audit/proposal files |
| Proposal checksum | `fa6fa5f66c8ffd955d6fc1e6bb3d5011` |
| Overall conformance | Not Implemented |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Not reused |
| Audit confidence | Medium-high |

## Implementation Target

The audit target is the current dirty worktree on `main`, not a clean PR branch or commit range.

Primary P088 implementation surfaces inspected:

- ACP runtime completion capture: `control-plane/crates/acp/src/lib.rs`, `control-plane/crates/acp/src/transport.rs`
- Engine settlement, repair, receipt persistence, P037/error-path handling, and worktree fingerprinting: `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/engine/src/worktree_fingerprint.rs`
- Domain readback and session events: `control-plane/crates/domain/src/code_writer_completion.rs`, `control-plane/crates/domain/src/session.rs`
- SQLite migration and repositories: `control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql`, `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs`, `control-plane/crates/db/src/repos/sessions.rs`
- Public readback: `control-plane/crates/graphql-server/src/**`, `control-plane/crates/mcp-server/src/**`
- Gate/docs/fixtures: `scripts/test-gate.sh`, `docs/reference/test-gates.md`, `docs/evidence/088-code-writer-completion/**`
- macOS operator surface searched for P088 readback strings: `Chainworks Forge/**`, `Chainworks ForgeTests/**`

## Prior Proposal-Review Reuse

The prior-review discovery helper found no proposal-review artifacts for this proposal:

```text
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py \
  /Users/user/Documents/Chainworks Forge/docs/proposals/088-code-writer-completion-contract-and-output-freshness.md
```

Result: `artifacts: []`.

Reviewer-selection reuse: **Not reused**. Prior `IMPLEMENTATION_AUDIT` reports were used only as historical context, not as reviewer-selection artifacts.

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_reliability_reviewer` | P088 is centered on timeout/terminalization, same-session repair, replay, idempotency, and startup/partial-write recovery. |
| `api_contract_reviewer` | GraphQL, MCP, run-report JSON, public vocabularies, and receipt schema compatibility are mandatory proposal outputs. |
| `observability_rollout_reviewer` | The proposal requires deterministic fixtures, durable evidence, migration/gate registration, operator diagnostics, and rollout sequencing. |
| `chainworks_execution_truth_reviewer` | The core behavior changes Run/Stage/Agent execution truth, output settlement, receipt links, runtime receipts, failed-stage evidence, and recovery state. |
| `macos_ui_reviewer` | Proposal line 883 explicitly requires operator readback/UI to display the new failure family, and GraphQL exposure is justified by operator UI scope at lines 862-863. |

Rejected close alternatives:

- `rust_arch_reviewer`: relevant but dropped under the hard cap; the audited architecture risk is covered by execution-truth, reliability, and API lenses.
- `apple_arch_reviewer`: Swift app state/provider architecture is not where the P088 contract is implemented; the only Apple-specific question is missing operator UI readback.
- `product_reviewer`: no metric or experiment decision gate is central beyond operator-readback completeness.
- `security_reviewer`: no auth, secret, unsafe, or public-input security boundary materially changes in this P088 slice.
- `performance_reviewer`: no performance targets or benchmarks are committed by the proposal.
- Go reviewers: no Go implementation surface is present.

## Proposal State And Contract Summary

Proposal state: **Active/Draft** for audit purposes.

P088 is meant to close a specific `code_writer` handoff failure: the agent did useful implementation work, but fresh required structured outputs for the current attempt did not settle. The proposal does not merely ask for a new label. It requires durable receipts, prompt-level completion text, output freshness decisions, P037 idle terminalization entry, same-session `code_writer_completion_repair_v1`, mutation guard, canonical readback, and operator-facing diagnostics.

Key proposal commitments:

- Materialize deterministic P087 terminal-completed missing-output and 70c9 dirty-worktree timeout fixtures under `docs/evidence/088-code-writer-completion/` before implementation (`docs/proposals/...088...md:60-70`).
- Persist `code_writer_completion_receipt_v1` with the minimum receipt fields, output decisions, activation source, ingestion-boundary failure, missing/stale outputs, completion mode, turn counts, and status (`docs/proposals/...088...md:221-267`).
- Enter P088 from normal settlement failure, P037 idle terminalization, or explicit operator retry recovery (`docs/proposals/...088...md:269-276`).
- Use one same-session `code_writer_completion_repair_v1` turn, not the generic repair prompt, and preserve the one-turn total budget (`docs/proposals/...088...md:278-326`).
- Require current-attempt worktree fingerprints for eligibility and exclude inherited dirty work (`docs/proposals/...088...md:342-359`).
- Persist completion text, transcript attribution or absence reason, failed-stage evidence, and separate original/repair runtime receipts (`docs/proposals/...088...md:496-565`).
- Make DB writes transactional, idempotent, conflict-detecting, canonically linked, and fail closed on artifact/DB partial writes until startup recovery reconciles them (`docs/proposals/...088...md:717-729`).
- Add typed session events `code_writer_completion_started`, `code_writer_completion_succeeded`, and `code_writer_completion_failed` (`docs/proposals/...088...md:767-776`).
- Expose additive `implementationCompletion` in run report, MCP `runs.get`/`runs.list`, and GraphQL with closed/unknown-safe vocabularies (`docs/proposals/...088...md:778-863`).
- Update operator readback/UI before targeted retries (`docs/proposals/...088...md:875-884`).

## Platform/Product Scope

- Apple platform scope: **macOS** operator readback is explicitly in scope. The audited implementation has no P088 macOS UI/readback strings or tests.
- Backend/service scope: Rust control-plane worker, ACP transport, SQLite persistence, GraphQL API, MCP API, run-report data, diagnostics, migration, and gate coverage.
- Product scope: operator trust and recovery for blocked implementation runs, especially distinguishing provider failure from completion-handoff failure without manual forensics.

## Primary Service Flows

1. A `code_writer` attempt reaches terminal settlement, fresh current-attempt outputs materialize normally, and P088 repair is not used.
2. A `code_writer` attempt changes implementation-owned files but misses required outputs, writes P088 evidence, and runs exactly one same-session `code_writer_completion_repair_v1` finalization turn.
3. A provider timeout/no-terminal-text attempt with only inherited dirty work is classified as preexisting dirty/provider failure, not current-attempt completion.
4. A stale `implementation_active` P087-like attempt is terminalized by P037 supervision into the same P088 diagnosis/recovery path.
5. Operators read `implementationCompletion` and detailed receipt/output decisions through GraphQL, MCP, run reports, and the macOS operator UI.

## Fidelity Inventory

### Matches

- Twelve deterministic fixtures now exist under `docs/evidence/088-code-writer-completion/`, including the two required P087 shapes plus mutation, ingestion-boundary, partial-write, prompt-side, public-enum, normal-materialization, and fingerprint fixtures.
- ACP captures terminal final response and streamed tail text with byte limits, byte counts, truncation flags, and extraction SHA (`control-plane/crates/acp/src/transport.rs:1014-1128`; `control-plane/crates/acp/src/lib.rs:322-368`).
- Engine captures P088 pre/post worktree fingerprints, uses `current_attempt_diff` for completion-repair eligibility, and persists receipt/text/output decisions (`control-plane/crates/engine/src/executor.rs:4488-4694`, `5208-5688`, `8876-9154`).
- SQLite upserts are conflict-detecting and transactional for receipt rows, text captures, output decisions, runtime prompt receipts, and receipt links (`control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:44-77`, `79-218`).
- Canonical readback via `code_writer_completion_receipt_links` has been added and is used by GraphQL/MCP `implementationCompletion` summary projection (`control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:251-272`; `control-plane/crates/graphql-server/src/schema.rs:153-162`; `control-plane/crates/mcp-server/src/tools/runs.rs:825-864`; `control-plane/crates/mcp-server/src/tools/reports.rs:668-676`).
- Typed code-writer completion session event types and DB string mappings exist (`control-plane/crates/domain/src/session.rs:13-30`, `102-123`; `control-plane/crates/db/src/repos/sessions.rs:636-652`).
- The focused proposal gate passes on this tree: `./scripts/test-gate.sh proposal-088`.

### Divergences

- The receipt model still omits first-class proposal-minimum fields: `completion_mode`, `published_at`, `completion_repair_turn_count`, `generic_repair_turn_count`, `missing_outputs`, and `stale_outputs` (`docs/proposals/...088...md:221-265`; actual model at `control-plane/crates/domain/src/code_writer_completion.rs:6-46`).
- `operator_retry_completion_recovery` and `generic_repair_already_failed_completion_contract_required` were not found in implementation code, only proposal/fixture context.
- Ingestion-boundary readback declares all proposal values, but the engine only produces truncation cases (`control-plane/crates/domain/src/code_writer_completion.rs:197-205`; `control-plane/crates/engine/src/executor.rs:9318-9327`).
- Completion text capture source has implementation value `capped_stream`, while the proposal names `terminal_final_response`, `streamed_update_tail`, and `session_update_stream` (`control-plane/crates/acp/src/lib.rs:322-328`; proposal lines 513-518).
- Partial-write handling now records `completion_receipt_partial_write` in the same persistence call, but artifact files are still written before the DB transaction and no startup recovery/reconciler was found (`control-plane/crates/engine/src/executor.rs:8902-9145`; proposal lines 717-725).
- `list_canonical_by_run` falls back to `list_by_run` when receipt links are absent, which can project historical/unlinked rows as canonical instead of failing closed or returning no active canonical row (`control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:251-272`).
- A no-live-session skip still records the generic `OutputContractRepairSkipped` event rather than a typed `code_writer_completion_failed` event (`control-plane/crates/engine/src/executor.rs:5689-5735`).
- No macOS operator UI/readback implementation for P088 was found; `rg` over `Chainworks Forge/**` and `Chainworks ForgeTests/**` excluding assets returned no P088/readback strings.

### Ambiguities / Evidence Gaps

- P037 integration is partially represented by error-path receipt persistence and activation-source classification, but no live stale `implementation_active` supervision path was proven.
- The proposal gate includes static fixture checks for several scenarios that look like behavioral guarantees; those fixtures do not by themselves execute the lifecycle.
- The audit did not run the full repository gate because conformance is already blocked by missing/partial requirements. The focused P088 gate did pass.
- Provider-specific evidence for the `junie`, `claude`, and `codex` reproduction family is not exercised by the current P088 gate.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | Distinct `code_writer` completion failure family | Implemented |
| REQ-002 | Stale previous-attempt files cannot satisfy current attempt | Implemented |
| REQ-003 | Completion repair eligibility uses current-attempt fingerprints | Partially Implemented |
| REQ-004 | Preexisting dirty timeout is not current-attempt completion | Implemented |
| REQ-005 | Completion receipt plus transcript/runtime/text evidence | Partially Implemented |
| REQ-006 | `code_writer_completion_repair_v1` branch and one-turn budget | Partially Implemented |
| REQ-007 | Completion repair mutation guard fails closed | Implemented |
| REQ-008 | Original and repair runtime receipts are separate | Implemented |
| REQ-009 | Transactional, idempotent, conflict-detecting, canonical receipt writes | Partially Implemented |
| REQ-010 | Terminal text inspectable independent of transcript | Partially Implemented |
| REQ-011 | Completion capture metadata and typed truncation failures | Partially Implemented |
| REQ-012 | Prompt-side evidence | Implemented |
| REQ-013 | `worktree_fingerprint_v1` artifacts | Implemented |
| REQ-014 | Public readback closed vocabularies and unknown handling | Implemented |
| REQ-015 | Run report/MCP/GraphQL explain fresh/stale/missing/control-plane outputs | Implemented |
| REQ-016 | Completed terminal response plus missing output classification | Implemented |
| REQ-017 | P087-like stale `implementation_active` terminalizes into P088 | Partially Implemented |
| REQ-018 | Usable final `CHAINWORKS_OUTPUT` follows normal settlement | Implemented |
| REQ-019 | Required P087 evidence fixtures exist | Implemented |
| REQ-020 | No non-`code_writer` missing-output closure claim | Implemented |
| REQ-021 | Proposal gate registered, documented, and runnable | Implemented |
| REQ-022 | Targeted retries no longer require forensic digging | Partially Implemented |
| REQ-023 | macOS operator UI displays the new failure family | Missing |

## Detailed Requirement Audit

### REQ-001: Distinct `code_writer` completion failure family

- Source: acceptance 1, proposal lines 995-996; state-machine scope lines 269-359.
- Status: Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: P088 candidate handling is limited to `code_writer` attempts with declared outputs; receipt `failure_class` distinguishes `terminal_response_completed_missing_required_outputs`, `work_completed_missing_current_attempt_outputs`, and generic missing outputs (`control-plane/crates/engine/src/executor.rs:8972-8986`).
- Gap: none blocking for this requirement.

### REQ-002: Stale previous-attempt files cannot satisfy current attempt

- Source: acceptance 2, proposal line 996.
- Status: Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: settlement decisions are persisted per output with pre/post/content hashes and rejection reasons (`control-plane/crates/domain/src/code_writer_completion.rs:67-79`; `control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql:126-138`). The P088 gate includes DB round-trip and readback tests.
- Gap: no blocking gap found in the audited code path.

### REQ-003: Completion repair eligibility uses current-attempt fingerprints

- Source: acceptance 3, proposal line 997; eligibility rules lines 342-359.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`, `search`.
- Mapping: worktree fingerprinting and tests cover current-attempt diffs versus inherited dirty work. Repair eligibility is based on `current_attempt_diff`.
- Gap: the explicit historical exception `activation_source=operator_retry_completion_recovery` was not found in implementation code, so the proposal's preserved-evidence retry path is absent.

### REQ-004: Preexisting dirty timeout is not current-attempt completion

- Source: acceptance 4, proposal line 998; work-change classification lines 350-359.
- Status: Implemented.
- Evidence: `fixture`, `code`, `tests-run`.
- Mapping: `p087-70c9-dirty-worktree-timeout.fixture.json` exists; worktree tests cover inherited dirty/deleted/renamed paths; public next operator action maps `preexisting_dirty_work` to `do_not_retry_preexisting_dirty_timeout` (`control-plane/crates/domain/src/code_writer_completion.rs:419-420`).
- Gap: none blocking for this requirement.

### REQ-005: Completion receipt plus transcript/runtime/text evidence

- Source: acceptance 5, proposal line 999; receipt fields lines 221-267; diagnostics lines 496-565.
- Status: Partially Implemented.
- Evidence: `migration`, `code`, `tests-run`.
- Mapping: migration and repository persist receipts, text captures, output decisions, receipt links, and prompt-level runtime receipts (`control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql:65-156`; `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:44-77`). Engine writes receipt artifacts, failed-stage evidence, transcript status, and completion text rows (`control-plane/crates/engine/src/executor.rs:8876-9154`).
- Gap: required first-class fields are absent (`completion_mode`, `published_at`, repair/generic turn counts, `missing_outputs`, `stale_outputs`), and the full proposal absence-reason vocabulary is not implemented.

### REQ-006: `code_writer_completion_repair_v1` branch and one-turn budget

- Source: acceptance 6, proposal line 1000; lifecycle lines 278-326.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: eligible attempts record `CodeWriterCompletionStarted`, run the completion prompt, and record success/failure events (`control-plane/crates/engine/src/executor.rs:5208-5688`). Prompt evidence stores `code_writer_completion_repair_v1`.
- Gap: `generic_repair_already_failed_completion_contract_required` and the historical operator retry path are not implemented. The no-live-session skip path records only the generic skipped repair event.

### REQ-007: Completion repair mutation guard fails closed

- Source: acceptance 7, proposal line 1001; mutation guard lines 328-340.
- Status: Implemented.
- Evidence: `code`, `fixture`, `tests-run`.
- Mapping: completion repair captures pre/post fingerprints; unexpected mutation records failure and a typed `CodeWriterCompletionFailed` event with `failed_unexpected_worktree_mutation`; unavailable mutation guard is normalized to the same public result (`control-plane/crates/engine/src/executor.rs:5238-5432`, `9287-9297`).
- Gap: the gate includes a static mutation-negative fixture, but the status is based on direct code evidence rather than an end-to-end mutation test.

### REQ-008: Original and repair runtime receipts are separate

- Source: acceptance 8, proposal line 1002; transaction rules lines 725-728.
- Status: Implemented.
- Evidence: `migration`, `code`, `tests-run`.
- Mapping: runtime receipts are keyed by `(agent_execution_id, prompt_kind, turn_index)` and are written through `upsert_with_runtime_receipts` (`control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql:3-27`; `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:44-77`). DB tests cover original and repair prompt-level rows.
- Gap: none blocking for this requirement.

### REQ-009: Transactional, idempotent, conflict-detecting, canonical receipt writes

- Source: acceptance 9, proposal line 1003; transaction/replay rules lines 717-729.
- Status: Partially Implemented.
- Evidence: `migration`, `code`, `tests-run`.
- Mapping: repository conflict-checks replay drift and writes receipt/text/output/runtime/link rows in one DB transaction (`control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:52-77`, `199-215`). Canonical readback via receipt links and a DB regression test were added (`control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:251-272`; `control-plane/crates/db/tests/proposal_088_persistence.rs:586-631`).
- Gap: artifact writes still occur before the DB transaction; no startup recovery/reconciler was found for crash-between-artifact-and-DB; `list_canonical_by_run` falls back to all run receipts when links are absent.

### REQ-010: Terminal text inspectable independent of transcript

- Source: acceptance 10, proposal line 1004; diagnostics lines 509-542.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: original and repair completion text artifacts are written as raw/redacted text when captured, and capture rows store artifact paths independent of transcript status (`control-plane/crates/engine/src/executor.rs:8770-8828`, `8987-9016`, `9218-9245`).
- Gap: storage failure is classified, but the full typed absence vocabulary is missing. `raw_capture_disabled`, `redaction_failed`, and `redacted_storage_write_failed` are not represented.

### REQ-011: Completion capture metadata and typed truncation failures

- Source: acceptance 11, proposal line 1005; capture rules lines 513-549.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: ACP capture records source, byte limits, captured bytes, truncation flags, extraction SHA, and terminal-final-response preference (`control-plane/crates/acp/src/transport.rs:1014-1128`). Tests cover terminal final response, streamed tail, and truncation metadata.
- Gap: `session_update_stream` is not implemented as a source; implementation exposes `capped_stream`, which is not in the proposal vocabulary. Ingestion-boundary failure producers cover only truncation, not `acp_final_text_not_collected`, `chainworks_output_not_extracted`, or `declared_output_settlement_rejected_usable_payload` (`control-plane/crates/engine/src/executor.rs:9318-9327`).

### REQ-012: Prompt-side evidence

- Source: acceptance 12, proposal line 1006.
- Status: Implemented.
- Evidence: `code`, `tests-run`, `fixture`.
- Mapping: prompt evidence records template id/version, prompt hash, redacted prompt artifact, expected-output snapshot hash/path, and repair/settlement reason. The fixture and gate check `prompt_template_id = code_writer_completion_repair_v1`.
- Gap: none blocking for this requirement.

### REQ-013: `worktree_fingerprint_v1` artifacts

- Source: acceptance 13, proposal line 1007.
- Status: Implemented.
- Evidence: `code`, `tests-run`, `fixture`.
- Mapping: worktree fingerprint tests cover deterministic sorting, count derivation, current-attempt diffs, inherited dirty work, deletes, renames, and generated/control-plane exclusions. The fixture is validated by the P088 gate (`scripts/test-gate.sh:6252-6263`).
- Gap: none blocking for this requirement.

### REQ-014: Public readback closed vocabularies and unknown handling

- Source: acceptance 14, proposal line 1008; readback vocabularies lines 778-863.
- Status: Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: public enum wrappers define closed known values plus `unknown` for status, ingestion-boundary failure, completion turn result, and next operator action (`control-plane/crates/domain/src/code_writer_completion.rs:186-231`). GraphQL/MCP tests pass through `proposal-088`.
- Gap: producers for some ingestion-boundary values are missing, but the public vocabulary/unknown-handling wrapper exists.

### REQ-015: Run report/MCP/GraphQL explain fresh/stale/missing/control-plane outputs

- Source: acceptance 15, proposal line 1009.
- Status: Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: detailed receipts expose output decisions with hashes, settlement source, validation status, and rejection reason, while the `implementationCompletion` summary exposes counts (`control-plane/crates/domain/src/code_writer_completion.rs:67-79`, `160-184`; `control-plane/crates/graphql-server/src/schema.rs:153-162`; `control-plane/crates/mcp-server/src/tools/runs.rs:825-864`).
- Gap: macOS UI display is handled separately as REQ-023.

### REQ-016: Completed terminal response plus missing output classification

- Source: acceptance 16, proposal line 1010.
- Status: Implemented.
- Evidence: `code`, `fixture`, `tests-run`.
- Mapping: terminal-completed missing outputs map to `terminal_response_completed_missing_required_outputs` (`control-plane/crates/engine/src/executor.rs:8972-8976`) and the P087 terminal-completed fixture is required by the gate (`scripts/test-gate.sh:6188-6192`).
- Gap: none blocking for this requirement.

### REQ-017: P087-like stale `implementation_active` terminalizes into P088

- Source: acceptance 17, proposal line 1011; state-machine line 310; rollout line 880.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: ACP execute errors for P088 candidates persist a P088 receipt with `skipped_no_live_session`, and activation-source helper maps recoverable idle/progress runtime receipts plus `current_attempt_diff` and missing outputs to `p037_idle_terminalization` (`control-plane/crates/engine/src/executor.rs:4488-4694`, `9330-9343`; test at `control-plane/crates/engine/src/executor.rs:11028-11065`).
- Gap: no audited code/test proves the actual stale `implementation_active` P037 supervisor terminalizes a live run into this settlement path instead of remaining active. This is still the core unsolved lifecycle risk.

### REQ-018: Usable final `CHAINWORKS_OUTPUT` follows normal settlement

- Source: acceptance 18, proposal line 1012; state-machine line 308.
- Status: Implemented.
- Evidence: `code`, `fixture`, `tests-run`.
- Mapping: normal materialization fixture is required by the gate, and the repair branch is bypassed when outputs settle successfully (`scripts/test-gate.sh:6213-6216`; `control-plane/crates/engine/src/executor.rs:5435-5474`).
- Gap: no blocking gap found.

### REQ-019: Required P087 evidence fixtures exist

- Source: acceptance 19, proposal line 1013; fixture seed lines 60-70.
- Status: Implemented.
- Evidence: `fixture`, `tests-run`.
- Mapping: both `p087-terminal-completed-missing-outputs.fixture.json` and `p087-70c9-dirty-worktree-timeout.fixture.json` exist and are required by the gate (`scripts/test-gate.sh:6188-6197`).
- Gap: none blocking for this requirement.

### REQ-020: No non-`code_writer` missing-output closure claim

- Source: acceptance 20, proposal line 1014.
- Status: Implemented.
- Evidence: `code`.
- Mapping: P088 candidate selection is tied to `code_writer` handling and public P088 readback is not generalized to all missing-output agents.
- Gap: no blocking gap found.

### REQ-021: Proposal gate registered, documented, and runnable

- Source: acceptance 21, proposal line 1015.
- Status: Implemented.
- Evidence: `config`, `tests-run`.
- Mapping: `proposal-088|p088` exists in `scripts/test-gate.sh:6180-6347`; docs reference exists at `docs/reference/test-gates.md:1894-1909`; `./scripts/test-gate.sh proposal-088` passed.
- Gap: gate depth is a readiness risk, not a registration gap.

### REQ-022: Targeted retries no longer require forensic digging

- Source: acceptance 22, proposal line 1016; rollout line 884.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`, `search`.
- Mapping: GraphQL/MCP/run-report readback now exposes status, failure class, output counts, prompt evidence, text captures, receipt artifacts, and next operator action.
- Gap: historical `operator_retry_completion_recovery` is not implemented; P037 stale-active terminalization is not proven; macOS operator UI is missing; some ingestion-boundary causes cannot be produced. Operators can see more than before, but still cannot rely on P088 alone for targeted retry decisions.

### REQ-023: macOS operator UI displays the new failure family

- Source: rollout line 883 and GraphQL/UI-scope line 862.
- Status: Missing.
- Evidence: `search`.
- Mapping: none found. A focused search for `implementationCompletion`, `code_writer_completion`, `completion_receipt`, `terminal_response_completed_missing_required_outputs`, and related P088 strings in `Chainworks Forge/**` and `Chainworks ForgeTests/**` returned no matches when assets were excluded.
- Gap: the macOS operator app does not display the new failure family, output freshness explanation, next action, or receipt links.

## Reviewer / Lens Scorecard

| Lens | Conformance | Readiness | Top risk | Confidence |
|---|---|---|---|---|
| Proposal conformance | Not Implemented | Not Ready | Missing macOS UI plus partial P037/partial-write/receipt contract | High |
| Rust reliability | Partial | Not Ready | P037 stale-active lifecycle and crash recovery are not proven end to end | Medium-high |
| API contract | Partial | Not Ready | Public vocabulary exists, but producers/schema fields are incomplete | High |
| Observability/rollout | Partial | Not Ready | Gate passes but over-relies on static fixtures for lifecycle claims | Medium-high |
| Chainworks execution truth | Partial | Not Ready | Active canonical link improved, but no startup recovery and operator retry path absent | Medium-high |
| macOS UI | Missing | Not Ready | Operator app has no P088 readback display | High |

## Routed Specialist Findings

### READY-001: P037 stale `implementation_active` recovery is still not proven

- Reviewer: `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Critical
- Confidence: Medium-high
- Related requirements: REQ-017, REQ-022
- Evidence: `code`, `tests-run`
- Evidence references: `control-plane/crates/engine/src/executor.rs:4488-4694`, `control-plane/crates/engine/src/executor.rs:9330-9343`, `control-plane/crates/engine/src/executor.rs:11028-11065`
- Why it matters: the proposal exists because useful implementation runs can stay stuck or become unreadable at the completion handoff. Current code classifies an idle-shaped receipt as `p037_idle_terminalization` and persists receipts on ACP execute errors, but the audit did not find a live stale-active supervisor path that takes an `implementation_active` run through P037 into P088 settlement.
- Recommended action: add and test the actual P037 supervisor transition for stale `implementation_active` code-writer attempts with current-attempt diffs and missing outputs.
- Acceptance criteria: a focused integration test starts from a stale active execution, terminalizes it through P037 into P088, writes the receipt/evidence, closes or invalidates the active generation per policy, and exposes `p037_idle_terminalization` in readback.

### REL-001: Partial-write recovery is same-call classification, not crash recovery

- Reviewer: `rust_reliability_reviewer`, `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-009, REQ-010
- Evidence: `code`, `migration`
- Evidence references: `control-plane/crates/engine/src/executor.rs:8902-9145`, `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:44-77`, proposal lines 717-725
- Why it matters: P088 explicitly requires a crash between artifact write and DB transaction to fail closed to `completion_receipt_partial_write` until startup recovery reconciles or marks the receipt unusable. Current code records partial-write errors it sees during one persistence call, but artifact writes still happen before the DB transaction and no startup recovery was found.
- Recommended action: add a startup reconciliation path or move enough artifact intent/state into the transaction to make crash recovery deterministic.
- Acceptance criteria: a crash/restart test with artifact written but DB receipt absent results in an unusable/partial-write readback, not a silent missing receipt or successful canonical projection.

### API-001: Receipt/readback contract still omits committed fields and producers

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-005, REQ-006, REQ-011, REQ-022
- Evidence: `code`, `search`
- Evidence references: proposal lines 221-265, 513-542; `control-plane/crates/domain/src/code_writer_completion.rs:6-46`; `control-plane/crates/acp/src/lib.rs:322-335`; `control-plane/crates/engine/src/executor.rs:9318-9327`
- Why it matters: operators and clients can only make targeted retry decisions if the schema actually distinguishes completion mode, turn counts, missing/stale output lists, activation source, capture source, and ingestion boundary. Several values are declared in public vocabulary but cannot be produced, while some required receipt fields do not exist.
- Recommended action: add the missing receipt fields/producers or explicitly amend the proposal before claiming completion.
- Acceptance criteria: receipt rows and public readback can represent `completion_mode`, `published_at`, repair/generic turn counts, missing/stale output names, `operator_retry_completion_recovery`, all committed ingestion-boundary failures, and the proposal-approved capture sources.

### API-002: Canonical readback falls back to non-canonical run receipts

- Reviewer: `api_contract_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-009, REQ-015
- Evidence: `code`, `tests-run`
- Evidence references: `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:251-272`, `control-plane/crates/db/tests/proposal_088_persistence.rs:586-631`, proposal lines 717-724
- Why it matters: the new receipt-link readback fixes the previous latest-created-at bug when links exist. But if links are absent, `list_canonical_by_run` returns all run receipts, allowing historical/unlinked rows to become the projected summary again. That conflicts with the proposal rule that only the active artifact-contract generation is canonical.
- Recommended action: make absent-link behavior explicit: return no canonical receipt, synthesize unknown/unavailable readback, or run a migration/recovery step that creates/marks links before projection.
- Acceptance criteria: a test with only unlinked historical receipts does not project the newest historical row as current canonical `implementationCompletion`.

### UI-001: macOS operator UI does not display P088 diagnostics

- Reviewer: `macos_ui_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-023, REQ-022
- Evidence: `search`
- Evidence references: proposal lines 862-883; `rg` over `Chainworks Forge/**` and `Chainworks ForgeTests/**` excluding assets returned no P088/readback matches
- Why it matters: the proposal includes operator readback/UI scope, and targeted retry usefulness depends on operators seeing the new failure family and next action in the app. GraphQL/MCP support alone does not satisfy the macOS operator app commitment.
- Recommended action: add the P088 summary and receipt/output-detail display to the relevant run/timeline/inspector UI, with tests or screenshots proving the failure family and next action are visible.
- Acceptance criteria: a fixture or mocked run with P088 receipt data renders status, failure class, output freshness counts/details, receipt/evidence links, and next operator action in the macOS operator UI.

### OPS-001: The focused gate passes but overstates lifecycle coverage

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-007, REQ-009, REQ-017, REQ-021, REQ-022
- Evidence: `config`, `tests-run`
- Evidence references: `scripts/test-gate.sh:6180-6347`, `docs/reference/test-gates.md:1894-1909`
- Why it matters: `proposal-088` passes, but several scenarios are static fixture existence/string checks rather than executed lifecycle tests: mutation-negative, ingestion-boundary failures, partial-write recovery, and P037 stale-active terminalization. A passing gate can therefore suggest readiness while the highest-risk behaviors remain unproven.
- Recommended action: keep the static fixture checks, but add executable tests for the lifecycle-critical paths or narrow the gate documentation so it does not imply coverage it does not provide.
- Acceptance criteria: the gate executes behavioral tests for P037 terminalization, crash/partial-write recovery, mutation guard, and ingestion-boundary producers, or the readiness documentation clearly labels those as fixture-only checks.

## Readiness Checklist

| Check | Status | Notes |
|---|---|---|
| Unique versioned audit report path created | Pass | R4 path selected by helper |
| Prior proposal-review artifacts discovered/reused | Pass | None found; reuse not applicable |
| Proposal contract extracted before implementation verdict | Pass | Key lines recorded above |
| Focused proposal gate run on same tree | Pass | `./scripts/test-gate.sh proposal-088` passed |
| All in-scope REQ items implemented | Fail | REQ-023 Missing; multiple partial requirements |
| macOS operator UI readback present | Fail | No P088 UI strings/tests found |
| P037 stale-active lifecycle proven | Fail | Classification exists; lifecycle transition not proven |
| Startup partial-write recovery proven | Fail | No recovery/reconciler found |
| Full regression gate run | Not run | Not required for a Not Ready verdict; conformance already blocked |

## Verification Log

| Command / Check | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...088...md` | Selected `..._IMPLEMENTATION_AUDIT_R4.md` |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...088...md` | `artifacts: []` |
| `git status --short --branch && git rev-parse HEAD && md5 -q ...088...md` | Branch `main`, dirty worktree, HEAD `b18587e63e39913cd7cb611de88b090a8e8ad5ca`, checksum `fa6fa5f66c8ffd955d6fc1e6bb3d5011` |
| Focused source reads with `nl`/`sed` over ACP, engine, DB, domain, GraphQL, MCP, gate, proposal | Evidence cited in this report |
| `rg` for missing receipt/activation/vocabulary terms | `operator_retry_completion_recovery`, `completion_mode`, turn-count fields, and some ingestion producers absent from implementation code |
| `rg` for P088/operator readback strings in `Chainworks Forge/**` and `Chainworks ForgeTests/**`, excluding assets | No matches |
| `find docs/evidence/088-code-writer-completion -maxdepth 1 -type f` | 12 fixture files present |
| `./scripts/test-gate.sh proposal-088` | Passed |

## Verdict

The implementation is materially stronger than R3: canonical receipt-link readback, typed session events, richer fixture/gate registration, and partial-write classification have been added. It still does **not** close P088.

The remaining blockers are not cosmetic. The proposal's core lifecycle claim depends on stale `implementation_active` attempts terminalizing through P037 into P088, and that is only partially represented. The receipt/readback contract is still missing fields and producers that targeted retry decisions need. Partial-write recovery is not startup recovery. The macOS operator UI commitment is missing entirely.

Final verdict: **Not Ready / Not Implemented** for proposal closeout. Keep the P088 gate result as useful focused evidence, but do not treat it as completion proof until the missing UI and lifecycle/recovery contract gaps above are closed.
