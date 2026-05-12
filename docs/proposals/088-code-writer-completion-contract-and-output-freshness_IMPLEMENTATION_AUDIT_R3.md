# Proposal 088 Implementation Audit R3: Code-Writer Completion Contract, Output Freshness, and Repair Diagnostics

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` |
| Proposal status | Draft |
| Audit report | `docs/proposals/088-code-writer-completion-contract-and-output-freshness_IMPLEMENTATION_AUDIT_R3.md` |
| Audit timestamp | 2026-05-11T18:35:52Z |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| HEAD | `b18587e63e39913cd7cb611de88b090a8e8ad5ca` |
| Compare base | Implicit current worktree audit |
| Worktree state | Dirty; P088 implementation files, fixtures, proposal, and prior audit reports are uncommitted/untracked |
| Proposal checksum | `fa6fa5f66c8ffd955d6fc1e6bb3d5011` |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer-selection reuse | Not reused |
| Audit confidence | Medium-high |

## Implementation Target

The audit target is the current worktree on `main`, not a clean commit or PR diff. The implementation is primarily Rust control-plane code plus reference docs, tests, migrations, and deterministic evidence fixtures.

Primary touched surfaces:

- ACP runtime completion capture: `control-plane/crates/acp/src/lib.rs`, `control-plane/crates/acp/src/transport.rs`
- Engine settlement, repair, receipt persistence, fingerprinting: `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/engine/src/worktree_fingerprint.rs`
- Domain readback model: `control-plane/crates/domain/src/code_writer_completion.rs`, `control-plane/crates/domain/src/session.rs`
- SQLite migration and repositories: `control-plane/crates/db/migrations/051_p088_code_writer_completion_receipts.sql`, `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs`
- Public readback: `control-plane/crates/graphql-server/src/**`, `control-plane/crates/mcp-server/src/**`
- Gate and fixtures: `scripts/test-gate.sh`, `docs/evidence/088-code-writer-completion/**`

## Prior Proposal-Review Reuse

No prior proposal-review artifacts were found by the audit helper:

```text
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py \
  /Users/user/Documents/Chainworks Forge/docs/proposals/088-code-writer-completion-contract-and-output-freshness.md
```

Result: `artifacts: []`.

Reviewer-selection reuse: **Not reused**. Prior implementation audit reports were intentionally ignored for reviewer selection, per skill rules.

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_arch_reviewer` | P088 changes Rust crate boundaries across ACP, engine, DB, domain, GraphQL, and MCP. |
| `rust_reliability_reviewer` | The proposal centers on retries, completion repair, timeout/terminalization, idempotency, and recovery. |
| `api_contract_reviewer` | GraphQL, MCP, run-report JSON, closed vocabularies, and readback shape are mandatory. |
| `observability_rollout_reviewer` | The proposal requires durable evidence, operator diagnostics, gate registration, migration, and rollout/readiness proof. |
| `chainworks_execution_truth_reviewer` | The core contract concerns Run/Stage/Agent execution truth, artifacts, receipts, output settlement, and recovery state. |

Rejected close alternatives:

- `macos_ui_reviewer`: P088 requires operator readback, but this implementation slice does not add or verify macOS SwiftUI screens.
- `apple_arch_reviewer`: no Swift app state/provider implementation was audited.
- `product_reviewer`: no product metrics or experiment decision checkpoint are central in this implementation audit.
- `security_reviewer`: no auth, secret, public-input, or unsafe boundary changed materially in the audited slice.
- `performance_reviewer`: no latency/throughput targets or benchmark commitments are in scope.
- Go reviewers: no `go.mod` or Go implementation surface is present.

## Proposal State And Contract Summary

Proposal state: **Active/Draft** for audit purposes.

The proposal scopes P088 to the `code_writer` completion-handoff failure class: real implementation work exists, but fresh structured outputs for the current attempt do not settle. It explicitly excludes weakening output contracts, provider-specific hotfixes, broad non-`code_writer` missing-output handling, and silent historical repair.

Key contract points:

- Materialize P087 terminal-completed missing-output and 70c9 dirty-worktree timeout fixtures before implementation (`docs/proposals/...088...md:60-70`).
- Persist `code_writer_completion_receipt_v1` with activation source, work-change classification, output freshness decisions, completion repair diagnostics, transcript/runtime evidence, and receipt/failure artifact paths (`docs/proposals/...088...md:221-267`, `496-565`, `731-766`).
- Enter P088 from normal declared-output settlement failure or P037 idle/terminalization, and run one same-session `code_writer_completion_repair_v1` turn only for eligible `current_attempt_diff` attempts (`docs/proposals/...088...md:269-359`).
- Use pre/post worktree fingerprints to distinguish current-attempt work from inherited dirty work (`docs/proposals/...088...md:304-359`).
- Write receipt, prompt-level text captures, output decisions, runtime receipt links, agent execution linkage, and artifact-contract projection transactionally, with idempotent replay, conflict detection, canonical readback, and partial-write recovery (`docs/proposals/...088...md:717-729`).
- Add typed session events `code_writer_completion_started`, `code_writer_completion_succeeded`, and `code_writer_completion_failed` (`docs/proposals/...088...md:767-776`).
- Expose `implementationCompletion` with closed/forward-compatible vocabularies in run report, MCP `runs.get`/`runs.list`, and GraphQL (`docs/proposals/...088...md:778-863`).

## Platform/Product Scope

- Apple platform scope: macOS operator readback is referenced by the proposal, but no macOS UI implementation was audited. Apple scope is therefore **Ambiguous/Not implemented in this slice**.
- Backend/service scope: Rust control-plane worker, ACP transport, SQLite persistence, GraphQL API, MCP API, run report data, diagnostics, migration, and rollout gate.
- Product scope: operator trust/recovery diagnostics for blocked implementation runs.

## Primary Service Flows

1. `code_writer` original prompt runs, ACP completion text is captured, declared outputs are settled, and usable current-attempt outputs advance through the normal path without completion repair.
2. `code_writer` does current-attempt implementation work, required outputs are missing/stale, P088 writes fingerprints/receipt/evidence, and one same-session `code_writer_completion_repair_v1` turn attempts final output publication.
3. A provider timeout/no terminal text run with only preexisting dirty work is classified as preexisting dirty/provider failure, not current-attempt completion.
4. Public operator readback retrieves `implementationCompletion` and detailed receipt/output decisions through GraphQL, MCP run/report resources, and run reports.
5. Receipt replay persists idempotently for byte-identical evidence and fails closed on drift.

## Fidelity Inventory

### Matches

- P087 terminal and 70c9 dirty-worktree fixtures exist under `docs/evidence/088-code-writer-completion/`.
- ACP completion capture stores terminal final response, streamed tail, byte counts, truncation flags, and extraction SHA (`control-plane/crates/acp/src/lib.rs:320-368`; `control-plane/crates/acp/src/transport.rs:1014-1128`).
- Engine captures pre/post original worktree fingerprints and post-error fingerprints for P088 candidates (`control-plane/crates/engine/src/executor.rs:4430-4462`, `4582-4664`, `4933-4979`).
- Engine uses `current_attempt_diff` to choose the P088-specific completion prompt and records one repair turn in the existing budget (`control-plane/crates/engine/src/executor.rs:5080-5559`).
- Engine persists completion receipts, text captures, output decisions, prompt evidence, failed-stage evidence path, and runtime receipt links (`control-plane/crates/engine/src/executor.rs:8717-8933`; `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:44-77`, `79-218`).
- GraphQL/MCP expose `implementationCompletion` and raw completion receipt readback (`control-plane/crates/graphql-server/src/schema.rs:153-160`, `893-900`; `control-plane/crates/mcp-server/src/tools/reports.rs:78-105`, `655-676`; `control-plane/crates/mcp-server/src/tools/runs.rs:825-858`).
- The proposal gate is registered and passed on the audited tree.

### Divergences

- Receipt storage lacks several proposal-minimum receipt fields as first-class fields: `completion_mode`, `published_at`, `missing_outputs`, `stale_outputs`, `completion_repair_turn_count`, and `generic_repair_turn_count`.
- The implementation writes `code_writer_completion_receipt_links`, but run-level readback still loads all run receipts directly and `implementationCompletion` chooses `max(created_at)`, not the active/canonical receipt link (`control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:234-248`; `control-plane/crates/domain/src/code_writer_completion.rs:233-241`).
- Artifact/text/failed-stage evidence files are written before the DB transaction and failures are swallowed with `.ok()`; no `completion_receipt_partial_write` recovery path was found (`control-plane/crates/engine/src/executor.rs:8743-8765`, `8899-8917`).
- Completion text and transcript absence vocabularies do not match the proposal. ACP only has `no_terminal_or_stream_text` and `empty_after_sanitization`; transcript code emits `transcript_artifact_not_persisted` and `transcript_not_collected` (`control-plane/crates/acp/src/lib.rs:330-368`; `control-plane/crates/engine/src/executor.rs:9053-9069`).
- Typed `code_writer_completion_*` session events are absent; the implementation still emits generic `output_contract_repair_*` events (`control-plane/crates/domain/src/session.rs:13-27`; `control-plane/crates/engine/src/executor.rs:5178-5559`).
- `operator_retry_completion_recovery` was not found in implementation code.

### Ambiguities / Evidence Gaps

- P037 integration is partly present as activation classification and ACP error-path receipt persistence, but the gate does not drive a stale `implementation_active` run through the actual P037 terminalization lifecycle.
- `completion_mode` appears represented indirectly by output decisions and prompt kind, but the proposal requires it as receipt data.
- The run-report/MCP/GraphQL readback can explain output decisions through raw receipts, but the additive `implementationCompletion` summary exposes counts, not per-output detail.
- Provider-independence tests for `junie`, `claude`, and `codex` were not found in the P088 gate; current tests are provider-agnostic/unit-style.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | Distinct `code_writer` completion failure family | Implemented |
| REQ-002 | Stale previous-attempt files cannot satisfy current attempt | Implemented |
| REQ-003 | Completion repair eligibility uses current-attempt fingerprints | Partially Implemented |
| REQ-004 | Preexisting dirty timeout is not current-attempt completion | Implemented |
| REQ-005 | Completion receipt plus transcript/runtime/text evidence | Partially Implemented |
| REQ-006 | `code_writer_completion_repair_v1` branch and one-turn budget | Partially Implemented |
| REQ-007 | Completion repair mutation guard fails closed | Partially Implemented |
| REQ-008 | Original and repair runtime receipts are separate | Implemented |
| REQ-009 | Transactional, idempotent, conflict-detecting, canonical receipt writes | Partially Implemented |
| REQ-010 | Terminal text inspectable independent of transcript | Partially Implemented |
| REQ-011 | Completion capture metadata and typed truncation failures | Partially Implemented |
| REQ-012 | Prompt-side evidence | Implemented |
| REQ-013 | `worktree_fingerprint_v1` artifacts | Implemented |
| REQ-014 | Public readback closed vocabularies and unknown handling | Implemented |
| REQ-015 | Operator readback explains fresh/stale/missing/control-plane outputs | Implemented |
| REQ-016 | Completed terminal response plus missing output classification | Implemented |
| REQ-017 | P087-like stale `implementation_active` terminalizes into P088 | Partially Implemented |
| REQ-018 | Usable final `CHAINWORKS_OUTPUT` follows normal settlement | Implemented |
| REQ-019 | Required P087 evidence fixtures exist | Implemented |
| REQ-020 | No non-`code_writer` missing-output closure claim | Implemented |
| REQ-021 | Proposal gate registered, documented, runnable, and covering required cases | Partially Implemented |
| REQ-022 | Targeted retries no longer require forensic digging | Partially Implemented |

## Detailed Requirement Audit

### REQ-001: Distinct `code_writer` completion failure family

- Source: acceptance 1, lines 995-996; state machine lines 269-359.
- Status: Implemented.
- Evidence: `code`.
- Mapping: P088 candidate is gated to `agent_id == "code_writer"` with declared outputs (`executor.rs:4430-4431`); failure class distinguishes terminal-completed missing outputs and current-attempt diffs (`executor.rs:8795-8804`); public status derives from receipt/failure data (`code_writer_completion.rs:335-360`).
- Gap: none blocking for this requirement.

### REQ-002: Stale previous-attempt files cannot satisfy current attempt

- Source: acceptance 2, lines 996-997; exact-path model lines 99-107.
- Status: Implemented.
- Evidence: `code`, `tests-found`.
- Mapping: settlement code records `StaleExpectedOutput` decisions and P088 counts stale decisions separately (`executor.rs:5043`, `8947-8950`); stale outputs are exposed through receipt output decisions.
- Gap: P036-shaped end-to-end integration coverage is still weaker than the proposal test list, but the settlement rule itself is implemented.

### REQ-003: Eligibility uses current-attempt fingerprints

- Source: acceptance 3, lines 997-998; eligibility lines 342-359.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: original pre/post fingerprints are captured (`executor.rs:4430-4462`, `4933-4979`); completion repair is eligible only when post-original fingerprint reports `CurrentAttemptDiff` (`executor.rs:5080-5090`); worktree tests cover current/new/modified and inherited dirty cases.
- Gap: `operator_retry_completion_recovery` is not implemented, despite being a declared activation source and exception path.

### REQ-004: Preexisting dirty timeout is not current-attempt completion

- Source: acceptance 4, lines 998-999; work-change rules lines 350-359.
- Status: Implemented.
- Evidence: `code`, `tests-run`, `fixture`.
- Mapping: `p087-70c9-dirty-worktree-timeout.fixture.json` exists; worktree tests cover inherited dirty, delete, and rename as preexisting; `next_operator_action` maps `preexisting_dirty_work` to `do_not_retry_preexisting_dirty_timeout` (`code_writer_completion.rs:419-420`).
- Gap: none blocking for this requirement.

### REQ-005: Completion receipt plus transcript/runtime/text evidence

- Source: acceptance 5, lines 999-1000; receipt fields lines 221-267; diagnostics lines 496-565.
- Status: Partially Implemented.
- Evidence: `code`, `migration`, `tests-run`.
- Mapping: migration creates completion receipt, text capture, output decision, and receipt link tables; engine persists receipt artifacts, failed-stage evidence, runtime receipts, text captures, and transcript status (`executor.rs:8717-8933`); DB tests round-trip receipt/text/output decisions.
- Gaps: first-class receipt fields `completion_mode`, output arrays for missing/stale, `published_at`, and repair turn counts are absent; typed text/transcript absence vocabularies diverge from the proposal; artifact persistence errors are hidden.

### REQ-006: `code_writer_completion_repair_v1` branch and one-turn budget

- Source: acceptance 6, lines 1000-1001; repair lifecycle lines 269-326.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: eligible code-writer repair uses `code_writer_completion_repair_prompt` instead of the generic prompt and increments the shared repair count once (`executor.rs:5120-5559`); prompt evidence records `code_writer_completion_repair_v1`.
- Gaps: required typed session events are absent; generic-already-failed path `generic_repair_already_failed_completion_contract_required` was not found; historical `operator_retry_completion_recovery` is absent.

### REQ-007: Completion repair mutation guard fails closed

- Source: acceptance 7, lines 1001-1002; mutation guard lines 328-340.
- Status: Partially Implemented.
- Evidence: `code`, `tests-found`.
- Mapping: pre/post completion-repair fingerprints are captured and any current-attempt changed path after repair sets `unexpected_worktree_mutation_during_completion_repair`, preventing merge of repair output (`executor.rs:5238-5532`); result normalizes to `failed_unexpected_worktree_mutation` (`executor.rs:9044-9051`).
- Gaps: the P088 gate did not exercise the promised unexpected-mutation negative fixture; if fingerprint capture fails, the internal result `completion_repair_mutation_guard_unavailable` is not in the public closed vocabulary.

### REQ-008: Separate runtime receipts

- Source: acceptance 8, lines 1002-1003; transaction rules lines 725-728.
- Status: Implemented.
- Evidence: `migration`, `code`, `tests-run`.
- Mapping: migration keys runtime receipts by `(agent_execution_id, prompt_kind, turn_index)`; `upsert_with_runtime_receipts` writes original and repair prompt receipts in one DB transaction (`code_writer_completion_receipts.rs:44-77`); DB tests verify original and repair prompt-level rows.
- Gap: none blocking for this requirement.

### REQ-009: Transactional, idempotent, conflict-detecting, canonical receipt writes

- Source: acceptance 9, lines 1003-1004; transaction/replay lines 717-729.
- Status: Partially Implemented.
- Evidence: `migration`, `code`, `tests-run`.
- Mapping: DB upsert conflict-checks drift before write (`code_writer_completion_receipts.rs:21-34`, `52-59`); receipt, text captures, output decisions, receipt link, and runtime prompt rows are written inside `upsert_with_runtime_receipts` (`code_writer_completion_receipts.rs:62-77`, `79-218`); replay conflict is tested.
- Gaps: readback does not select via `code_writer_completion_receipt_links`; `implementationCompletion` chooses latest receipt by `created_at`; artifact writes happen before DB transaction and no `completion_receipt_partial_write` startup recovery was found.

### REQ-010: Terminal text inspectable independent of transcript

- Source: acceptance 10, lines 1004-1005; diagnostics lines 498-545.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: raw/redacted completion text artifact files are written when captured (`executor.rs:8620-8660`); text capture rows store artifact paths independent of transcript (`executor.rs:8996-9022`).
- Gaps: write failures are swallowed; no `redacted_only` completion status exists; text absence reasons are ACP-internal `no_terminal_or_stream_text`/`empty_after_sanitization`, not proposal vocabulary.

### REQ-011: Completion capture metadata and typed truncation failures

- Source: acceptance 11, lines 1005-1006; capture rules lines 520-549.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: ACP stores capture source, byte limit, byte count, truncation flags, extraction SHA, and chooses terminal final response before streamed tail (`acp/src/lib.rs:320-368`; `acp/src/transport.rs:1014-1128`); tests cover terminal preference, streamed tail, and truncation metadata.
- Gaps: `session_update_stream` source is absent; absence reasons do not include `provider_did_not_emit_text`; ingestion failure only maps truncation cases and does not classify final-text collection, extraction, or usable-payload settlement failures.

### REQ-012: Prompt-side evidence

- Source: acceptance 12, lines 1006-1007; runtime evidence lines 500-501 and 747-752.
- Status: Implemented.
- Evidence: `code`, `tests-run`, `fixture`.
- Mapping: repair prompt receipt records prompt kind, template id/version, prompt hash, redacted prompt artifact path, expected-output snapshot hash/path, and repair reason (`executor.rs:5120-5190`; `code_writer_completion_receipts.rs:267-289`); prompt-side evidence fixture and DB readback test exist.
- Gap: none blocking for this requirement.

### REQ-013: `worktree_fingerprint_v1` artifacts

- Source: acceptance 13, lines 1007-1008; schema lines 361-365.
- Status: Implemented.
- Evidence: `code`, `tests-run`, `fixture`.
- Mapping: engine captures and persists deterministic fingerprints for original and repair phases; tests cover sorted paths, derived counts, current-attempt diffs, inherited dirty work, deletes, renames, and generated/control-plane exclusions.
- Gap: none blocking for this requirement.

### REQ-014: Public readback closed vocabularies and unknown handling

- Source: acceptance 14, lines 1008-1009; readback shape lines 778-863.
- Status: Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: domain wraps known/unknown values in `PublicEnumReadback` (`code_writer_completion.rs:120-133`); closed value sets are defined for status, ingestion boundary, completion turn result, and next operator action (`code_writer_completion.rs:186-231`); GraphQL/MCP tests verify unknown future values preserve raw with `known=false`.
- Gap: canonical readback selection is covered under REQ-009.

### REQ-015: Operator readback explains output freshness/control-plane status

- Source: acceptance 15, lines 1009-1010; readback fields lines 830-860.
- Status: Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: raw receipt readback exposes output decisions with settlement source, validation status, and rejection reason; `implementationCompletion` exposes fresh/stale/missing/control-plane counts in GraphQL, MCP reports, and MCP runs.
- Gap: the summary is count-based; exact per-output explanation requires reading raw receipt details.

### REQ-016: Completed terminal response plus missing outputs classification

- Source: acceptance 16, lines 1010-1011.
- Status: Implemented.
- Evidence: `code`, `tests-run`, `fixture`.
- Mapping: receipt failure class maps terminal `completed` plus missing outputs to `terminal_response_completed_missing_required_outputs` before current-attempt diff handling (`executor.rs:8795-8804`); P088 gate fixture and GraphQL test assert the value.
- Gap: none blocking for this requirement.

### REQ-017: P087-like stale `implementation_active` terminalizes into P088

- Source: acceptance 17, lines 1011-1012; P037 bridge lines 118-122 and 304-318.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`, `inference`.
- Mapping: error-path ACP failures for P088 candidates capture post fingerprints and persist a skipped-no-live-session receipt (`executor.rs:4491-4675`); activation source maps failed idle/progress runtime receipt plus current-attempt diff to `p037_idle_terminalization` (`executor.rs:9084-9096`); a unit test covers activation classification.
- Gaps: no full stale `implementation_active` lifecycle/canary fixture was found; `auto_requeue_active_prompt_close` can requeue before the P088 error receipt path executes (`executor.rs:4493-4512`, `3349-3435`), so the actual P037 terminalization-to-P088 path is not fully proven.

### REQ-018: Usable final `CHAINWORKS_OUTPUT` follows normal settlement

- Source: acceptance 18, lines 1012-1013; normal path lines 124-130.
- Status: Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: original result is settled through `settle_agent_outputs_from_discovery_decisions` before repair eligibility is considered (`executor.rs:4980-5055`); repair only runs when validation requires output contract repair (`executor.rs:5070-5090`).
- Gap: the proposal's named normal-materialization fixture is not present in `docs/evidence/088-code-writer-completion/`.

### REQ-019: Required P087 evidence fixtures exist

- Source: acceptance 19, lines 1013-1014; fixture requirement lines 60-70.
- Status: Implemented.
- Evidence: `fixture`, `tests-run`.
- Mapping: `p087-terminal-completed-missing-outputs.fixture.json` and `p087-70c9-dirty-worktree-timeout.fixture.json` exist; gate static checks validate their key fields.
- Gap: none for the two mandatory P087 seed fixtures.

### REQ-020: No non-`code_writer` closure claim

- Source: acceptance 20, lines 1014-1015; non-goal lines 10-11.
- Status: Implemented.
- Evidence: `code`.
- Mapping: P088 candidate is restricted to `agent_id == "code_writer" && !declared_outputs.is_empty()` (`executor.rs:4430-4431`).
- Gap: none blocking for this requirement.

### REQ-021: Proposal gate registered, documented, runnable, and covering required cases

- Source: acceptance 21, lines 1015-1016; test gate list lines 955-989.
- Status: Partially Implemented.
- Evidence: `tests-run`, `config`, `docs`.
- Mapping: `./scripts/test-gate.sh proposal-088` exists, is documented, and passed on this tree. The gate runs static fixture checks plus ACP, DB, engine, GraphQL, and MCP P088 tests.
- Gaps: current fixture directory contains six fixtures, not the full promised gate matrix; missing named coverage includes P036/P086/P087 end-to-end shapes, generic-already-failed handling, partial-write recovery, provider-independence, normal-materialization fixture, mutation negative fixture, docs-only eligibility, generated-evidence-only ineligibility, and ingestion-boundary failure fixtures.

### REQ-022: Targeted retries no longer require forensic digging

- Source: acceptance 22, line 1016; rollout sequence lines 878-884.
- Status: Partially Implemented.
- Evidence: `code`, `tests-run`.
- Mapping: operator readback now exposes receipt summaries, output freshness counts, text captures, prompt evidence, and next operator action across GraphQL/MCP/report paths.
- Gaps: canonical readback, partial-write recovery, typed absence vocabulary, typed session events, and the absent `operator_retry_completion_recovery` path still force manual interpretation for important failure modes.

## Reviewer/Lens Scorecard

| Lens | Score | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Explicit transaction/readback/evidence/event requirements remain incomplete. | Medium-high |
| Rust architecture | Partial | Completion receipt behavior spans engine, DB, domain, and public APIs but canonical ownership is not yet clean. | Medium-high |
| Rust reliability | Not Ready | Partial-write recovery and P037 terminalization are not fully implemented/proven. | Medium |
| API contract | Not Ready | Public readback exists, but canonical selection and vocabularies diverge from the proposal. | High |
| Observability/rollout | Not Ready | Gate passes, but promised canary/fixture matrix is incomplete. | Medium-high |
| Execution truth | Partial | Receipt/link tables exist, but readback does not consume the canonical link and artifacts can drift from DB truth. | High |

## Routed Specialist Findings

### READY-001: P037 stale-active terminalization is only partially proven

- Reviewer: `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-017, REQ-022
- Evidence: `code`, `tests-run`, `inference`
- Evidence references: `control-plane/crates/engine/src/executor.rs:4491-4675`, `9084-9096`; `scripts/test-gate.sh:6188-6324`
- Why it matters: P088's original problem includes stale `implementation_active` attempts that never enter completion diagnostics. The implementation can classify an idle/progress failure as `p037_idle_terminalization`, but the gate does not exercise the actual P037 supervision lifecycle from active stale run to P088 settlement. The active-prompt-close auto-requeue path can also return before P088 error receipt persistence.
- Recommended action: Add a true P087-like integration/canary fixture that starts from stale `implementation_active`, terminalizes through P037, persists the P088 receipt, and proves the run no longer remains active. Cover the auto-requeue branch explicitly.
- Acceptance criteria: A failing stale-active attempt with current-attempt diff and missing outputs ends with `activation_source=p037_idle_terminalization`, a resolvable P088 receipt, terminal execution state, and no indefinite active projection.

### REL-001: Canonical readback ignores the receipt link table

- Reviewer: `chainworks_execution_truth_reviewer`, `api_contract_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-009, REQ-014, REQ-022
- Evidence: `code`, `migration`
- Evidence references: `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:199-215`, `234-248`; `control-plane/crates/domain/src/code_writer_completion.rs:233-241`
- Why it matters: The migration and repository write `code_writer_completion_receipt_links`, but run-level readback still loads every receipt by `run_id`, and `implementationCompletion` selects the newest `created_at`. The proposal requires selecting the receipt linked from the current active `agent_execution_id` or active artifact-contract generation, with older rows as audit history.
- Recommended action: Add a canonical readback query that joins through `code_writer_completion_receipt_links` and/or active artifact-source generation, then make GraphQL/MCP/run-report summaries use that canonical receipt. Keep full historical receipt lists separate.
- Acceptance criteria: A run with multiple historical completion receipts projects only the linked active receipt into `implementationCompletion`, while older receipts remain available in audit history.

### REL-002: Artifact evidence writes are not recovered transactionally

- Reviewer: `rust_reliability_reviewer`, `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-005, REQ-009, REQ-010, REQ-022
- Evidence: `code`
- Evidence references: `control-plane/crates/engine/src/executor.rs:8743-8765`, `8899-8917`; proposal transaction rules `docs/proposals/...088...md:717-729`
- Why it matters: Raw/redacted completion text, receipt JSON, and failed-stage evidence are written before the DB transaction, and write failures are converted to `None` with `.ok()`. The proposal requires crash-between-artifact-and-DB handling to fail closed as `completion_receipt_partial_write` until startup recovery reconciles or marks the receipt unusable.
- Recommended action: Stop swallowing artifact write errors for required evidence, persist explicit storage failure/partial-write state, and add startup reconciliation for orphaned or missing P088 artifacts.
- Acceptance criteria: Simulated artifact write failure or crash-before-DB produces durable `completion_receipt_partial_write` or typed storage failure, and readback does not present incomplete evidence as clean.

### API-001: Completion text and transcript evidence vocabularies diverge from the proposal

- Reviewer: `api_contract_reviewer`, `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-005, REQ-010, REQ-011
- Evidence: `code`
- Evidence references: `control-plane/crates/acp/src/lib.rs:320-368`; `control-plane/crates/engine/src/executor.rs:9025-9069`
- Why it matters: Operators need typed evidence to distinguish provider did not emit text, capture was disabled/failed, storage failed, or session reuse lacked terminal capture. Current values include `no_terminal_or_stream_text`, `empty_after_sanitization`, `transcript_artifact_not_persisted`, and `transcript_not_collected`, which are not the proposal vocabulary and make cross-surface readback harder to automate.
- Recommended action: Align completion and transcript status/reason enums with the proposal, including `redacted_only`, `provider_did_not_emit_text`, `provider_did_not_supply`, `capture_disabled`, `capture_failed`, `storage_write_failed`, and `session_reuse_without_terminal_capture`.
- Acceptance criteria: GraphQL/MCP/report readback serializes proposal-approved absence reasons; unknown future values still preserve `raw` plus `known=false`.

### API-002: P088 typed session events are missing

- Reviewer: `api_contract_reviewer`, `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-006, REQ-021
- Evidence: `code`
- Evidence references: `control-plane/crates/domain/src/session.rs:13-27`; `control-plane/crates/engine/src/executor.rs:5178-5559`; proposal `docs/proposals/...088...md:767-776`
- Why it matters: The proposal requires `code_writer_completion_started`, `code_writer_completion_succeeded`, and `code_writer_completion_failed` as typed specializations of the repair lifecycle. The implementation currently emits only generic `output_contract_repair_*` events, so operators and diagnostics cannot reliably distinguish P088 completion recovery from generic output repair via session history.
- Recommended action: Add the three typed events, persist them alongside existing repair lifecycle budget/generation details, and update tests/readback fixtures.
- Acceptance criteria: Eligible completion recovery emits `code_writer_completion_started`; success emits `code_writer_completion_succeeded`; failure/mutation/skipped cases emit `code_writer_completion_failed` with generation id and repair turn count.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Canonical proposal gate passed on audited tree | Passed | `./scripts/test-gate.sh proposal-088` |
| Build/test coverage for ACP capture | Passed | 3 ACP P088 tests passed |
| DB migration/receipt replay coverage | Passed | 3 DB P088 tests passed |
| Engine fingerprint/activation coverage | Passed | 6 engine P088 tests passed |
| GraphQL readback coverage | Passed | 1 GraphQL P088 test passed |
| MCP report/runs readback coverage | Passed | 2 MCP P088 tests passed |
| Full regression suite | Not run | Proposal gate only |
| P037 stale-active lifecycle validation | Incomplete | Unit activation helper only; no full lifecycle/canary |
| Receipt canonical readback | Incomplete | Link table written but not consumed for summary projection |
| Partial-write recovery | Missing | No `completion_receipt_partial_write` path found |
| Typed session events | Missing | Only generic repair events exist |
| UI/accessibility/localization/privacy/entitlements | Not applicable to audited code slice | No macOS UI implementation audited |

## Verification Log

| Command / Check | Result |
|---|---|
| `git status --short --branch` | Dirty `main` worktree with P088 implementation files and docs. |
| `python3 .../report_path.py ...088...md` | Selected `docs/proposals/088-code-writer-completion-contract-and-output-freshness_IMPLEMENTATION_AUDIT_R3.md`. |
| `python3 .../discover_prior_review.py ...088...md` | No prior proposal-review artifacts found. |
| Proposal/source inspection | Confirmed proposal is Draft and extracted contract from lines 1-11, 60-70, 221-359, 496-565, 717-863, 880-1016. |
| Targeted code searches/reads | Inspected ACP capture, engine executor, worktree fingerprint, DB migration/repo/tests, domain readback, GraphQL/MCP readback, and gate. |
| `./scripts/test-gate.sh proposal-088` | Passed. Static fixture checks passed. ACP: 3 passed. DB: 3 passed. Engine: 6 passed. GraphQL: 1 passed. MCP: 2 passed. Warnings were non-blocking dead-code/lifetime warnings. |

## Final Verdict

Overall conformance is **Partial**. The implementation now covers a substantial portion of P088: fingerprints, P087 fixtures, completion capture, receipt persistence, output freshness counts, prompt evidence, readback surfaces, replay conflict detection, and a passing proposal gate.

Overall readiness is **Not Ready**. The remaining gaps are proposal-level contract gaps, not polish: canonical receipt readback ignores the link table, partial artifact/DB write recovery is absent, P037 stale-active terminalization is not fully proven, completion/transcript vocabularies diverge, and P088 typed session events are missing.

Recommended next actions:

1. Fix canonical readback to select through the active receipt link/artifact generation, and add a multi-receipt test.
2. Add `completion_receipt_partial_write` handling and stop silently swallowing required evidence write failures.
3. Implement proposal-aligned completion/transcript absence vocabularies and typed P088 session events.
4. Add a true P037 stale `implementation_active` terminalization canary plus the missing gate fixtures from the proposal test matrix.
