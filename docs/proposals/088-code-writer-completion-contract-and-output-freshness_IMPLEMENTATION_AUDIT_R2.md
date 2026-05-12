# Proposal 088 Implementation Audit R2

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md` |
| Proposal checksum | `fa6fa5f66c8ffd955d6fc1e6bb3d5011` |
| Proposal state | Draft, treated as Active for implementation audit (`Status = Draft`, proposal lines 3-11) |
| Audit timestamp | 2026-05-11T18:06:01Z |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| HEAD | `b18587e63e39913cd7cb611de88b090a8e8ad5ca` |
| Implementation target | Current dirty worktree, implicit compare base |
| Overall Conformance | Not Implemented |
| Overall Implementation Readiness | Not Ready |
| Audit confidence | High |

## Implementation Target

The implementation is an uncommitted worktree on `main`. The relevant P088 implementation surfaces are Rust control-plane changes in `control-plane/crates/acp`, `control-plane/crates/db`, `control-plane/crates/domain`, `control-plane/crates/engine`, `control-plane/crates/graphql-server`, `control-plane/crates/mcp-server`, new P088 evidence fixtures under `docs/evidence/088-code-writer-completion/`, and gate/reference updates in `scripts/test-gate.sh` and `docs/reference/test-gates.md`.

The proposal itself and prior audit R1 are untracked in this worktree, so the audit uses the current filesystem contents as the target truth.

## Prior Review Reuse

Reviewer-selection reuse: Not reused.

No prior proposal-review artifacts were found beside the proposal, in `<proposal>.review/`, or via the skill helper. The existing `IMPLEMENTATION_AUDIT_R1` was ignored for reviewer selection per the audit workflow.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | Engine, ACP, DB repository, migration, and crate boundary changes. |
| `rust_reliability_reviewer` | Retry/repair lifecycle, idempotency, replay, timeout/terminalization, and mutation guard behavior. |
| `api_contract_reviewer` | GraphQL, MCP, run-report, ACP metadata, and public readback vocabulary contracts. |
| `observability_rollout_reviewer` | Migration, gate, fixtures, runtime diagnostics, support readback, and rollout evidence. |
| `chainworks_execution_truth_reviewer` | Durable run/stage/agent execution truth, output settlement authority, receipt ownership, and artifact-contract linkage. |

Rejected close alternatives:

- `rust_security_reviewer`: P088 touches operator-only diagnostics and artifacts, but no new auth boundary or secret handling was evident in the audited slice.
- `macos_ui_reviewer` / `apple_arch_reviewer`: the proposal has operator readback/UI implications, but the implementation inspected here changes GraphQL/MCP/report payloads rather than Swift UI.
- `product_reviewer`: not selected because this audit is contract/readiness-focused, not metric or decision-gate design.

## Contract Summary

P088 is meant to close a specific `code_writer` failure class: real implementation work exists, but fresh structured completion outputs for the current attempt do not settle. The key locked commitments are:

- Freshness authority remains with existing declared-output settlement and validation; receipts are evidence only.
- Eligible `code_writer` attempts with real current-attempt work and missing outputs use one same-session `code_writer_completion_repair_v1` branch instead of generic repair.
- P037 stale active/idle terminalization must route P087-like active handoffs into the same P088 diagnosis/recovery path.
- `worktree_fingerprint_v1` proves current-attempt work vs inherited dirty work.
- Runtime receipts, completion text captures, output decisions, transcript absence, receipt artifacts, and failed-stage evidence must be durable and operator-inspectable.
- Public GraphQL/MCP/run-report readback must expose an additive `implementationCompletion` summary with closed known values plus forward-compatible unknown handling.

## Primary Flows

1. `code_writer` completes with missing or invalid structured outputs, but the terminal response is available.
2. `code_writer` produces implementation-owned worktree changes but no usable structured outputs, so the one-turn `code_writer_completion_repair_v1` branch is attempted.
3. A P087-like stale `implementation_active` handoff is idle/terminalized by P037 and routed into P088 settlement instead of staying active.
4. The operator inspects run-report, MCP, and GraphQL readback to identify fresh, stale, missing, and control-plane generated outputs plus the next action.
5. Replay/recovery re-drives the same settlement without partial writes or silently mutating receipt truth.

## Evidence Pack

Tests run:

- `./scripts/test-gate.sh proposal-088`
- Result: passed on this worktree.
- Scope executed by the gate: static fixture checks, `cargo test -p acp proposal_088_`, `cargo test -p db proposal_088_`, `cargo test -p engine proposal_088_`, `cargo test -p graphql-server proposal_088_`, and `cargo test -p mcp-server proposal_088_`.

Tests found:

- ACP capture tests for terminal-final-response preference, streamed tail fallback, and truncation metadata.
- DB tests for prompt-level runtime receipt rows and receipt/text/output-decision roundtrip.
- Engine unit tests for worktree fingerprint classification and P037 activation-source helper.
- GraphQL/MCP tests for raw `code_writer_completion_receipts` exposure.
- Static fixture checks under `docs/evidence/088-code-writer-completion/`.

Important evidence references:

- Proposal P087 fixture requirement: `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md:60-70`.
- Proposal repair state machine and P037 terminalization: `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md:269-359`.
- Proposal completion-text and transcript diagnostics: `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md:496-565`.
- Proposal transaction/replay/readback requirements: `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md:717-729`.
- Proposal `implementationCompletion` readback shape: `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md:778-863`.
- Engine P088 original-prompt fingerprint capture: `control-plane/crates/engine/src/executor.rs:4430-4468`.
- Engine P088 post-prompt fingerprint and repair eligibility: `control-plane/crates/engine/src/executor.rs:4837-4868`, `control-plane/crates/engine/src/executor.rs:5005-5023`.
- Engine repair runtime receipt persistence: `control-plane/crates/engine/src/executor.rs:5137-5172`.
- Engine completion receipt persistence and placeholder fields: `control-plane/crates/engine/src/executor.rs:8547-8715`.
- DB receipt upsert conflict logic: `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs:11-31`.
- GraphQL raw receipt exposure: `control-plane/crates/graphql-server/src/types/run.rs:68-69`, `control-plane/crates/graphql-server/src/types/run.rs:210-242`.
- MCP raw receipt exposure: `control-plane/crates/mcp-server/src/tools/runs.rs:825-840`, `control-plane/crates/mcp-server/src/tools/reports.rs:78-104`, `control-plane/crates/mcp-server/src/tools/reports.rs:649-660`.
- Gate static checks and focused tests: `scripts/test-gate.sh:6190-6275`.
- Gate reference claim: `docs/reference/test-gates.md:1894-1908`.

## Fidelity Inventory

Matches:

- Adds migration `051_p088_code_writer_completion_receipts.sql` with prompt-level runtime receipt identity and P088 receipt/text/output-decision tables.
- Captures `worktree_fingerprint_v1` before and after the original prompt for eligible `code_writer` attempts.
- Uses `code_writer_completion_repair_v1` as a distinct prompt for eligible current-attempt diffs.
- Persists completion text raw/redacted artifacts when captured text is available.
- Registers `proposal-088|p088` and adds deterministic fixture files.
- Keeps non-`code_writer` attempts outside the P088 receipt path via `agent_id == "code_writer"`.

Divergences:

- No evidence that P037 stale `implementation_active` attempts are terminalized into P088 settlement; P088 receipt logic is only on the successful `acp.execute(...)` result path.
- Public readback exposes raw `code_writer_completion_receipts`, not the required `implementationCompletion` summary with closed vocabularies and `known=false` unknown handling.
- Completion receipt writes are separate from runtime receipt writes, agent execution linkage, artifact-contract projection, and filesystem artifact writes.
- Receipt replay checks only `id` and output decisions; receipt/text capture drift can be silently overwritten.
- `transcript_status`, `transcript_absence_reason`, `receipt_artifact_path`, and `failed_stage_evidence_path` are persisted as `None`.
- Required typed session events `code_writer_completion_started`, `code_writer_completion_succeeded`, and `code_writer_completion_failed` are not present.
- Completion text status/source/absence vocabularies are narrower than the proposal and include implementation values (`absent`, `capped_stream`) outside the public contract.
- The gate documentation claims GraphQL/MCP/run-report `implementationCompletion` parity, but the test gate checks raw receipt surfaces instead.

Ambiguities / Evidence Gaps:

- Existing P053/P058 exact-path settlement may already reject stale outputs, but P088-specific tests do not prove stale previous-attempt files cannot satisfy current-attempt output settlement.
- No runtime replay of the P087 terminal-completed and 70c9 timeout canaries was executed; the current gate validates JSON fixtures and narrow unit tests.
- No startup-recovery path for `completion_receipt_partial_write` was found.

## Requirement Summary

| REQ | Requirement | Status |
|---|---|---|
| REQ-001 | Distinct `code_writer` completion failure family for changed worktree + missing outputs | Partially Implemented |
| REQ-002 | Stale previous-attempt files cannot satisfy current-attempt settlement | Not Verifiable |
| REQ-003 | Completion repair eligibility is based on original pre/post fingerprints and `current_attempt_diff` | Partially Implemented |
| REQ-004 | Pre-existing dirty timeout is not classified as current-attempt work completion | Partially Implemented |
| REQ-005 | Every such failure persists receipt plus transcript/runtime evidence or typed absence reasons | Partially Implemented |
| REQ-006 | `code_writer_completion_repair_v1` can recover eligible attempts within one repair budget | Partially Implemented |
| REQ-007 | Completion repair mutation guard fails closed with typed evidence | Partially Implemented |
| REQ-008 | Original and repair runtime receipts are separate rows and do not overwrite each other | Implemented |
| REQ-009 | SQLite receipt writes are transactional, idempotent, conflict-detecting, and canonically selected | Partially Implemented |
| REQ-010 | Terminal text is inspectable as redacted/raw text or typed absence independent of transcript | Partially Implemented |
| REQ-011 | Completion text captures source, byte limits, truncation flags, SHA, and typed truncation failures | Partially Implemented |
| REQ-012 | Prompt-side evidence records template id/version, prompt hash, redacted prompt, contract snapshot, reason | Partially Implemented |
| REQ-013 | `worktree_fingerprint_v1` explains path inclusion/status/digest and deterministic count derivation | Partially Implemented |
| REQ-014 | GraphQL/MCP/run-report readback has closed vocabularies and forward-compatible unknown handling | Missing |
| REQ-015 | Operator readback explains fresh/stale/missing/control-plane outputs across all surfaces | Partially Implemented |
| REQ-016 | Completed terminal responses with missing outputs classify as `terminal_response_completed_missing_required_outputs` | Implemented |
| REQ-017 | P087-like stale `implementation_active` attempts terminalize into P088 diagnosis/recovery | Missing |
| REQ-018 | Usable current-attempt final `CHAINWORKS_OUTPUT` materializes normally without repair | Implemented |
| REQ-019 | Required P087 terminal-completed and dirty-timeout fixtures exist before implementation | Implemented |
| REQ-020 | P088 readback does not claim closure for non-`code_writer` agents | Implemented |
| REQ-021 | `proposal-088|p088` gate is registered in script and reference docs | Implemented |
| REQ-022 | Targeted retries no longer require forensic digging | Partially Implemented |

## Detailed Requirement Audit

### REQ-001 Distinct Completion Failure Family

Status: Partially Implemented.

Evidence: The receipt builder maps missing outputs plus completed terminal receipt to `terminal_response_completed_missing_required_outputs` and missing outputs plus `current_attempt_diff` to `work_completed_missing_current_attempt_outputs` (`executor.rs:8610-8618`). This proves part of the normal completed-result path. It does not cover P037 active/timeout terminalization, and receipt fields are not surfaced as the required `implementationCompletion` diagnosis.

### REQ-002 Stale Output Freshness

Status: Not Verifiable.

Evidence: The implementation reuses declared-output settlement before repair, and output decisions carry `post_prompt_sha256`/`content_sha256`, but P088 tests do not exercise stale previous-attempt exact-path files. The proposal explicitly requires stale files not to satisfy current-attempt settlement (`proposal:995-996`).

### REQ-003 Fingerprint-Based Eligibility

Status: Partially Implemented.

Evidence: The engine captures pre/post fingerprints (`executor.rs:4430-4468`, `executor.rs:4837-4868`) and gates P088 repair on `WorkChangeKind::CurrentAttemptDiff` (`executor.rs:5005-5009`). Explicit historical `operator_retry_completion_recovery` evidence is not implemented, and edge cases in fingerprint classification can overcount inherited deletes/renames as current-attempt changes (`worktree_fingerprint.rs:195-209`).

### REQ-004 Preexisting Dirty Timeout Negative Case

Status: Partially Implemented.

Evidence: There is a unit test for unchanged preexisting dirty work and a static 70c9 fixture. However, the audited P088 settlement path runs after `acp.execute(...)` returns `Ok(result)`; timeout/error branches return before P088 post-fingerprint, receipt, and readback are produced. The P087 dirty timeout canary is fixture-only, not a runtime terminalization path.

### REQ-005 Receipt Plus Transcript/Runtime Evidence

Status: Partially Implemented.

Evidence: Completion receipts and text captures are persisted (`executor.rs:8547-8715`), and repair runtime receipts can be inserted (`executor.rs:5137-5172`). But `transcript_status`, `transcript_absence_reason`, `receipt_artifact_path`, and `failed_stage_evidence_path` are set to `None` (`executor.rs:8708-8711`), so operator evidence is incomplete.

### REQ-006 Dedicated Completion Repair Branch

Status: Partially Implemented.

Evidence: Eligible attempts use `code_writer_completion_repair_prompt(...)` instead of `output_contract_repair_prompt(...)` (`executor.rs:5017-5022`) and share the existing repair counter. Missing pieces include typed `code_writer_completion_*` session events (`proposal:767-776`), prior generic-failure handling, and closed-vocabulary result mapping.

### REQ-007 Mutation Guard

Status: Partially Implemented.

Evidence: The implementation captures a post-repair fingerprint and treats any current-attempt change as `unexpected_worktree_mutation_during_completion_repair` (`executor.rs:5184-5242`). It does not write failed-stage evidence, and the stored result value does not match the public `implementationCompletion.completion_turn_result` value `failed_unexpected_worktree_mutation` required by the readback contract (`proposal:804-814`).

### REQ-008 Prompt-Level Runtime Receipts

Status: Implemented.

Evidence: Migration changes the primary key to `(agent_execution_id, prompt_kind, turn_index)` (`051...sql:3-27`), the repository exposes prompt-level upsert/readback, and the gate passes the prompt-level persistence test.

### REQ-009 Transaction, Replay, Conflict, Canonical Readback

Status: Partially Implemented.

Evidence: Receipt/text/output decisions are written in one repository transaction (`code_writer_completion_receipts.rs:26-31`, `code_writer_completion_receipts.rs:34-155`). But runtime receipts are written separately, no `agent_executions` completion receipt FK/link exists, no artifact-contract projection is linked, and artifact writes happen before the DB upsert. Conflict detection compares only receipt id and output decisions (`code_writer_completion_receipts.rs:17-23`), so changed receipt fields or text captures can be overwritten.

### REQ-010 Terminal Completion Text

Status: Partially Implemented.

Evidence: Captured text is written to raw/redacted files (`executor.rs:8571-8591`) and recorded in text capture rows. Storage failures are swallowed by `.ok().flatten()`, redacted-only status is absent, and absence reasons are limited to `no_terminal_or_stream_text` / `empty_after_sanitization` / truncation fallback, not the required typed set.

### REQ-011 Capture Metadata and Typed Truncation

Status: Partially Implemented.

Evidence: ACP metadata records source, byte count, truncation flags, and SHA (`acp/lib.rs:337-351`; `acp/transport.rs:1032-1068`). The public source enum uses `capped_stream` rather than `session_update_stream` and absence/status vocabularies are not the proposal vocabulary.

### REQ-012 Prompt-Side Evidence

Status: Partially Implemented.

Evidence: Repair prompt evidence records template id/version, prompt hash, redacted prompt artifact, expected-output snapshot, and settlement reason (`executor.rs:5145-5163`). Original prompt evidence remains legacy-style and the required fields are not surfaced in the `implementationCompletion` summary.

### REQ-013 Worktree Fingerprint Artifact

Status: Partially Implemented.

Evidence: The artifact schema and deterministic summary are implemented in `worktree_fingerprint.rs`, and focused tests pass. However, preexisting deleted or renamed paths are classified as current-attempt changes before checking the baseline (`worktree_fingerprint.rs:195-209`, `worktree_fingerprint.rs:349-356`), which can violate the inherited dirty-work boundary for those path states.

### REQ-014 Public Closed-Vocabulary Readback

Status: Missing.

Evidence: GraphQL exposes `codeWriterCompletionReceipts` as strings and nested raw rows (`graphql types/run.rs:68-69`, `graphql types/run.rs:210-242`). MCP `runs.get/list` and reports add `code_writer_completion_receipts` arrays (`mcp tools/runs.rs:825-840`; `mcp tools/reports.rs:78-104`, `mcp tools/reports.rs:649-660`). No `implementationCompletion`, no `UNKNOWN` fallback wrapper, and no `known=false` marker were found.

### REQ-015 Fresh/Stale/Missing/Control-Plane Readback

Status: Partially Implemented.

Evidence: Raw receipt rows include fresh/stale/missing/control-plane counts and output decisions. The required cross-surface operator summary, next action, prompt-side fields, and unknown handling are missing.

### REQ-016 Terminal Completed Missing Outputs Classification

Status: Implemented.

Evidence: `failure_class` is `terminal_response_completed_missing_required_outputs` when missing outputs remain and the original runtime receipt status is `completed` (`executor.rs:8610-8614`). Focused GraphQL/MCP tests seed and read that value.

### REQ-017 P087 Active Handoff Closure

Status: Missing.

Evidence: The proposal requires stale `implementation_active` attempts to be terminalized into P088 (`proposal:302-317`, `proposal:1011`). The implementation has a helper that can label `p037_idle_terminalization`, but no P037/orchestrator path routes an active idle attempt through P088 settlement; P088 receipt creation occurs only after a completed ACP result path reaches import (`executor.rs:6053-6078`).

### REQ-018 Usable Current-Attempt Final Output Settles Normally

Status: Implemented.

Evidence: ACP selects completion text for extraction before repair and settles discovered artifacts through the declared-output path; repair is attempted only after validation still requires output contract repair (`executor.rs:5005-5022`).

### REQ-019 Fixtures

Status: Implemented.

Evidence: Required files exist under `docs/evidence/088-code-writer-completion/`, and the gate checks both the terminal-completed and 70c9 dirty-timeout fixture shapes (`scripts/test-gate.sh:6190-6240`).

### REQ-020 Non-`code_writer` Scope

Status: Implemented.

Evidence: P088 candidate selection requires `agent_id == "code_writer"` and declared outputs (`executor.rs:4430-4431`).

### REQ-021 Gate Registration

Status: Implemented.

Evidence: `proposal-088|p088` is implemented in `scripts/test-gate.sh:6190-6275` and documented in `docs/reference/test-gates.md:1894-1908`.

### REQ-022 Targeted Retry Debuggability

Status: Partially Implemented.

Evidence: Receipts improve debug data for completed result paths. The missing P037 active-handoff path, missing `implementationCompletion.next_operator_action`, placeholder transcript/evidence paths, and raw receipt arrays mean operators still need forensic digging for core canaries.

## Reviewer / Lens Scorecard

| Lens | Score | Top Risk | Confidence |
|---|---|---|---|
| Proposal conformance | Not Implemented | Missing P037 canary closure and public readback contract | High |
| Rust architecture | Not Ready | P088 truth is split across ad hoc helper writes rather than a single durable ownership boundary | High |
| Rust reliability | Not Ready | Replay/partial-write/idempotency and active timeout routes do not satisfy fail-closed semantics | High |
| API contract | Not Ready | Public surfaces expose raw receipts, not the required `implementationCompletion` contract | High |
| Observability / rollout | Not Ready | Gate passes while missing the largest behavioral canaries it claims to cover | High |
| Chainworks execution truth | Not Ready | Receipt is not linked to active agent execution/artifact-contract truth as required | High |

## Routed Specialist Findings

### REL-001 / CHAINWORKS-001: P087 active handoff can still miss P088 entirely

Reviewer: `rust_reliability_reviewer`, `chainworks_execution_truth_reviewer`  
Severity: Critical  
Confidence: High  
Related requirements: REQ-001, REQ-004, REQ-017, REQ-022  
Evidence types: proposal, code, tests-found, tests-run  
Evidence references: proposal lines 271-317 and 1011; `executor.rs:4430-4468`, `executor.rs:4837-4868`, `executor.rs:6053-6078`; `scripts/test-gate.sh:6190-6275`

Why it matters: The proposal exists to close the active completion-handoff gap, especially P087-like `implementation_active` attempts where useful work exists but outputs never settle. The current implementation records P088 evidence only after `acp.execute(...)` returns a normal result and the import path runs. Provider timeout/no-terminal-response paths return through error/runtime-facts handling before post-fingerprint capture, receipt creation, and completion readback. The 70c9 canary is represented as a JSON fixture, not as executed settlement behavior.

Recommended action: Add the P037 terminalization integration so stale active `code_writer` attempts with current-attempt work are routed into P088 settlement or explicitly classified as ineligible provider/terminalization failures. Add a focused test that simulates the P087 70c9 timeout path and proves no `work_completed_missing_current_attempt_outputs` is emitted for inherited dirty work.

Acceptance criteria:

- A stale active `code_writer` attempt with post-prompt current-attempt implementation changes receives P088 diagnosis/recovery.
- A timeout/no-terminal-text attempt with only inherited dirty work does not receive `work_completed_missing_current_attempt_outputs`.
- The run no longer remains indefinitely `implementation_active`.

### API-001: Public readback contract is the wrong shape

Reviewer: `api_contract_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-014, REQ-015, REQ-022  
Evidence types: proposal, code, tests-found, tests-run  
Evidence references: proposal lines 778-863; `graphql-server/src/types/run.rs:68-69`, `graphql-server/src/types/run.rs:210-242`; `mcp-server/src/tools/runs.rs:825-840`; `mcp-server/src/tools/reports.rs:78-104`, `mcp-server/src/tools/reports.rs:649-660`

Why it matters: Operators and clients were promised a stable additive `implementationCompletion` summary with closed known values and forward-compatible unknown handling. Instead GraphQL and MCP expose raw `code_writer_completion_receipts` arrays with unwrapped strings. The seeded tests even assert an unknown future string round-trips as a raw value, which contradicts the required GraphQL `UNKNOWN` fallback or string-preserving wrapper and MCP/run-report `known=false` metadata.

Recommended action: Implement a canonical `implementationCompletion` projector used by GraphQL run readback, MCP `runs.get/list`, and run-report JSON. Include `next_operator_action`, known/unknown metadata, prompt-side fields, completion text captures, and output freshness decisions. Keep raw receipts only as operator-debug detail if needed.

Acceptance criteria:

- GraphQL exposes the required summary field and handles unknown known-value enums via `UNKNOWN` or a string-preserving wrapper.
- MCP and run-report JSON preserve raw values and include `known=false` for unknown future values.
- Tests fail if only raw `code_writer_completion_receipts` are present.

### REL-002: Receipt persistence is not the atomic replay-safe owner promised by P088

Reviewer: `rust_reliability_reviewer`, `observability_rollout_reviewer`, `chainworks_execution_truth_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-005, REQ-009, REQ-012, REQ-022  
Evidence types: proposal, migration, code  
Evidence references: proposal lines 717-729; migration lines 65-144; `code_writer_completion_receipts.rs:11-31`, `code_writer_completion_receipts.rs:34-155`; `executor.rs:5137-5172`, `executor.rs:8547-8715`

Why it matters: P088 requires the completion receipt, text captures, output decisions, prompt-level runtime links, agent execution linkage, and artifact-contract projection to land in one DB transaction, with crash-safe partial-write behavior. Current code writes repair runtime receipts separately, writes completion text artifacts before DB upsert, persists receipts in an isolated repository transaction, has no `agent_executions` completion receipt FK/link, and leaves receipt/failed-stage artifact paths unset. Replay conflict detection checks only id and output decisions, allowing receipt and text-capture drift to overwrite prior evidence.

Recommended action: Move P088 settlement persistence into a single executor transaction that includes runtime prompt receipts, completion receipt/text/output rows, agent execution linkage, artifact projection, and failure evidence metadata. Compare canonical receipt and text capture payloads on replay, not only output decisions. Add crash/partial-write recovery behavior or explicit fail-closed readback.

Acceptance criteria:

- Byte-identical settlement replay succeeds without modifying evidence.
- Any receipt/text/output decision drift returns `completion_receipt_conflict` and performs no partial write.
- `completion_receipt_partial_write` is reachable and operator-visible when artifact/DB reconciliation is incomplete.
- `agent_executions` or an equivalent FK-backed reference identifies the canonical receipt.

### API-002 / REL-003: Completion text evidence vocabulary is incomplete and storage failures are hidden

Reviewer: `api_contract_reviewer`, `rust_reliability_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-010, REQ-011  
Evidence types: proposal, code  
Evidence references: proposal lines 511-549; `acp/src/lib.rs:315-351`; `acp/src/transport.rs:1032-1068`; `executor.rs:8571-8591`, `executor.rs:8806-8831`

Why it matters: Completion text diagnostics are intended to make missing transcripts non-blocking for investigation. The implementation captures useful text in happy paths, but its status/source/absence vocabularies do not match the public contract: no `redacted_only`, no `session_update_stream`, no required absence reasons such as `provider_did_not_emit_text`, `redaction_failed`, or `storage_write_failed`. Artifact write errors are swallowed via `.ok().flatten()`, so a storage failure can degrade into missing paths rather than a typed absence reason.

Recommended action: Align ACP metadata and persisted readback with the proposal vocabulary, map truncation/storage/redaction failures to typed reasons, and preserve redacted-only evidence when raw capture is unavailable.

Acceptance criteria:

- Stored completion text status is `captured`, `redacted_only`, or `unavailable`.
- Stored capture source is one of the proposal values or an unknown-wrapped future value.
- Raw/redacted write failures produce typed absence reasons.
- Tests cover storage failure, redacted-only, and truncation-before-output paths.

### OPS-001: The P088 gate passes while not proving the documented contract

Reviewer: `observability_rollout_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-014, REQ-017, REQ-021, REQ-022  
Evidence types: config, tests-found, tests-run  
Evidence references: `docs/reference/test-gates.md:1894-1908`; `scripts/test-gate.sh:6190-6275`; GraphQL/MCP P088 tests

Why it matters: `docs/reference/test-gates.md` says the gate covers GraphQL/MCP/run-report `implementationCompletion` parity, but the tests only assert raw receipt exposure. The static fixture checks validate JSON seeds, not that the P087 active handoff is terminalized into runtime P088 diagnosis. This creates a false readiness signal: the gate passed in this audit, but the implementation is still missing major proposal commitments.

Recommended action: Update the gate to execute contract-level assertions for `implementationCompletion`, known/unknown vocabulary handling, P037 terminalization, receipt atomicity/conflict behavior, transcript absence reasons, and failed-stage evidence paths. Keep static fixture checks as preconditions, not as behavioral proof.

Acceptance criteria:

- The current raw-receipt-only readback implementation fails the updated gate.
- The gate proves the two P087 fixture shapes through runtime/settlement behavior or a deterministic integration harness.
- Gate reference documentation matches actual assertions.

### ARCH-001: Worktree fingerprint edge cases can turn inherited deletes/renames into current-attempt work

Reviewer: `rust_arch_reviewer`, `rust_reliability_reviewer`  
Severity: Minor  
Confidence: Medium  
Related requirements: REQ-003, REQ-004, REQ-013  
Evidence types: code  
Evidence references: proposal lines 342-359; `worktree_fingerprint.rs:183-209`, `worktree_fingerprint.rs:349-356`

Why it matters: The classifier checks `entry.is_renamed` and `entry.is_deleted` before comparing to the baseline. A file already deleted or renamed before the original prompt can therefore be counted as `DeletedAfterPrompt` or `RenamedAfterPrompt` in the post fingerprint, which contributes to `current_attempt_changed_path_count`. That weakens the core current-attempt vs inherited-dirty boundary.

Recommended action: Compare baseline identity/status before classifying rename/delete as after-prompt work, and add tests for inherited deleted and inherited renamed implementation-owned paths.

Acceptance criteria:

- Preexisting deleted/renamed dirty paths remain `preexisting_dirty` when unchanged after prompt.
- Only deletes/renames introduced after the prompt count as current-attempt changes.

## Readiness Decision

The proposal gate passed, but the implementation is not ready. The gate is narrower than the proposal and does not prove the core canary closure or public readback contract. Two explicit acceptance criteria are missing outright: public `implementationCompletion` readback with closed vocabularies, and P087-like stale `implementation_active` terminalization into P088 diagnosis/recovery. Several other requirements are only partially implemented because durable evidence and transaction ownership are incomplete.

Recommended next step: treat this as a partial implementation. Do not close P088 until the active-handoff terminalization path, canonical public readback summary, atomic receipt ownership, and completion-text evidence vocabulary are implemented and covered by the proposal gate.
