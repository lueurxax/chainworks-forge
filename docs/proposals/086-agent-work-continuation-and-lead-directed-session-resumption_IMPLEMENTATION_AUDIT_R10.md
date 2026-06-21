# Proposal 086 Implementation Audit R10

## Metadata

- Proposal: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md`
- Report: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R10.md`
- Audit date: 2026-06-20
- Repository: `/Users/user/Documents/Chainworks Forge`
- Target tree: current working tree at HEAD `0e6482c82b588b74a76294a225e68286bfe37fa4`
- Verdict: **Partial implementation, Not Ready for proposal closeout**
- Scope note: report-only audit. The working tree was already dirty before this audit, including P086 implementation files, P082 closeout deletions/reference updates, and unrelated untracked audit reports. This audit did not normalize or revert those changes.

## Prior Review Reuse

- `discover_prior_review.py` found no proposal-review artifacts for this proposal.
- Existing implementation audit R9 was used only as historical context, not as reviewer substitution.
- R9's raw provider-session-id logging finding appears addressed in the current tree: `AcpRuntimeManager` now logs `requested_provider_session_ref` and builds mismatch messages from a SHA-256 based reference in `control-plane/crates/acp/src/manager.rs`.

## Specialist Coverage

Selected reviewers:

- `chainworks_execution_truth_reviewer`: run/workflow ownership, continuation modes, catalog truth, worker boundaries.
- `rust_reliability_reviewer`: crash/replay state machine, duplicate prompt prevention, process/orphan handling.
- `api_contract_reviewer`: MCP, GraphQL, schemas, receipt/readback contracts.
- `observability_rollout_reviewer`: metrics, evidence fixtures, operator report gates, rollout claims.
- `rust_security_reviewer`: auth/redaction, raw receipt storage, subprocess/session identifiers, filesystem/DB boundaries.

Rejected or deferred under the 5-reviewer cap:

- `apple-ui-ux`: Swift readback stayed passive and the canonical gate passed, but UI/UX was not treated as a Ready blocker because the proposal explicitly keeps SwiftUI read-only for mutations.
- `performance`: resource-bound concerns were inspected through reliability/security and gate evidence, but no Ready claim is made. A final Ready audit should include an explicit performance/resource pass if resurrection is enabled by default.

Security-sensitive diff status: triggered. Categories included auth, public ingress, filesystem/subprocess boundary, parser boundary, DoS/resource limits, privacy/redaction, and unsafe crypto/dependency surfaces.

## Proposal Summary

P086's remaining work is not the already shipped live-handle continuation baseline. The proposal requires successful `provider_session_resurrection`: start a new Chainworks-managed ACP process, attach/resume a recorded provider session id where the adapter supports that operation, verify requested-vs-actual identity before any prompt, persist attach proof and prompt-turn markers, preserve fail-closed unsupported behavior, expose typed readback/metrics, implement output-only recovery as a distinct no-source-edit mode, and prove crash/replay boundaries.

## Current Implementation Summary

Implemented or substantially present:

- ACP adapter capability type exists in `control-plane/crates/acp/src/adapters/mod.rs`, and Claude declares support through `resumeSessionId` in `control-plane/crates/acp/src/adapters/claude.rs`.
- `AcpRuntimeManager::attach_provider_session_for_resurrection` starts a new ordered ACP session, requires a provider session id, compares the returned session id before prompt, and uses redacted references in logs/errors.
- The continuation worker has a provider-session-resurrection branch in `control-plane/crates/engine/src/executor.rs`, writes a DB-backed raw v2 attach receipt, persists a redacted receipt artifact, records `provider_send` before prompt, and routes the prompt through the newly attached live handle.
- Daemon tests cover a resurrection happy path and identity mismatch before prompt in `control-plane/crates/daemon/tests/proposal_086_mcp_continuation_live_reuse.rs`.
- Raw v2 receipt storage moved to SQLite in `control-plane/crates/db/migrations/085_p086_raw_receipt_db_storage.sql`, reducing same-UID filesystem exposure.

## Findings

### BLOCKER-001: The shipped catalog still disables provider-session resurrection

Proposal requirement: at least Claude provider-session resurrection is implemented and enabled by catalog opt-in; admission must require `code_writer.continuation_capability.provider_session_resurrection.enabled=true`.

Evidence:

- `examples/agents/agents.yaml:2053-2058` declares `provider_session_resurrection.enabled: false`.
- The happy-path daemon test seeds its own catalog with `provider_session_resurrection.enabled=true`, so the test proves an override path, not that new runs using the shipped catalog can use the feature.

Impact:

- New runs compiled from the default catalog will fail closed before worker enqueue. That preserves safety, but it does not satisfy the proposal's completion goal or acceptance criterion 1.

### BLOCKER-002: MCP admission still uses provider-name static support, not the actual selected adapter capability

Proposal requirement: the selected ACP adapter must declare a supported, enabled `ProviderSessionResurrectionCapability` before MCP admission or worker dispatch can attempt resurrection.

Evidence:

- `control-plane/crates/mcp-server/src/tools/agents.rs:1115-1117` answers support through `acp::adapters::provider_session_resurrection_supported_for_provider(provider)`.
- `control-plane/crates/mcp-server/src/tools/agents.rs:2221-2233` passes that static provider-family boolean into the catalog gate.
- The actual runtime-manager adapter check exists separately at `control-plane/crates/acp/src/manager.rs:486-493`, but the MCP admission path does not call it before enqueue.

Impact:

- A catalog-enabled `provider_session_resurrection` for a provider name recognized as Claude can be admitted without proving the currently configured runtime manager has a working Claude adapter/binary/capability. The worker may fail later, but the proposal requires fail-closed before resurrection work is enqueued when adapter support cannot be proven.

### BLOCKER-003: Resurrection phase model and replay proof do not match the required state machine

Proposal requirement: durable `resurrection_phase` must use the typed matrix `queued`, `launching`, `launched`, `attaching`, `attached_unprompted`, `prompting`, `settling`, `failed_closed`, `completed`, with crash/replay rules for each phase and no duplicate prompt/fresh-retry fallback.

Evidence:

- Migration `079_p086_resurrection_state_and_idempotency.sql:13-23` uses `admitted` and omits `queued`; it also includes no `launched` or `attaching` update proof in the worker path.
- The worker transitions from `launching` at `control-plane/crates/engine/src/executor.rs:7409` directly to `attached_unprompted` at `control-plane/crates/engine/src/executor.rs:7509`; there is no durable `launched` or `attaching` boundary.
- `domain::continuation` defines `ContinuationMode` and `ContinuationStatus`, but no typed Rust `ResurrectionPhase` enum.
- `ContinuationRecord` and normal GraphQL continuation readback omit `resurrection_phase`; it is available through the special attach-receipt path, not the standard continuation status projection.
- Many phase-specific negative fixtures remain placeholders, including `resurrection-attaching-timeout-before-prompt.fixture.json`, `resurrection-prompting-timeout-no-duplicate-send.fixture.json`, and `terminal-idempotency-ledger-concurrent-replay-*.fixture.json`.

Impact:

- The implementation has important duplicate-send guards, but it does not prove the full proposal replay matrix. Crashes in launch/attach/prompt windows still lack complete proposal-shaped evidence.

### BLOCKER-004: Output-only recovery is distinguished in prompts/readback but not enforced as a no-source-edit lane

Proposal requirement: `output_only_recovery` must be distinct from implementation retry/resurrection, capture pre/post output inventory, set `output_only=true`, set `source_edit_allowance=false` unless explicitly allowed, and fail/flag source edits.

Evidence:

- The prompt adds "do not edit source files" when `continuation.mode == "output_only_recovery"` at `control-plane/crates/engine/src/executor.rs:5953-5961`.
- The live-handle receipt sets `output_only` and `source_edit_allowance` at `control-plane/crates/engine/src/executor.rs:8043-8045`.
- The settlement path counts changed files from worktree readback, but the audit found no fail-closed branch that rejects or marks an output-only recovery when changed source files are observed.
- `docs/evidence/rollout-contract/p086/negative/output-only-repair-violation-changed-source-files.fixture.json` is still a placeholder.

Impact:

- The mode is visible, but enforcement remains advisory. This does not satisfy acceptance criterion 6 or the output-only data/evidence requirements.

### BLOCKER-005: Session-store recovery is not integrated into P086 resurrection receipts

Proposal requirement: Claude session-store recovery must be a first-class path when terminal response is lost, recording store path/root, read time, owner source, digest/length, latest assistant text/tool activity, and recovery result without scraping unrelated transcript truth.

Evidence:

- The v2 receipt builder writes `session_store_transcript_path = null`, `session_store_transcript_digest = null`, and `session_store_recovery_result = "not_attempted"` at `control-plane/crates/engine/src/executor.rs:7668-7689`.
- ACP session-store helpers exist in `control-plane/crates/acp/src/session.rs`, but `ProviderSessionStoreCapture` only carries `provider`, `staging_root`, and `captured_subdirs` in `control-plane/crates/acp/src/lib.rs:343-349`.
- The daemon resurrection tests do not cover terminal-output recovery through the session-store path.

Impact:

- Acceptance criterion 8 remains unmet. The schema has fields for this path, but the actual resurrection flow does not populate or validate them.

### BLOCKER-006: Evidence gates pass while ignoring placeholder resurrection fixtures

Proposal requirement: the full acceptance gate must include the resurrection, output-only, crash/replay, readback/redaction, and negative fixtures listed by the proposal.

Evidence:

- `./scripts/test-gate.sh proposal-086` passed.
- `./scripts/test-gate.sh p086-continuation-negative-fixtures` also passed, but `scripts/test-gate.sh:10178-10196` checks only 16 older fixture filenames.
- Many newer required files under `docs/evidence/rollout-contract/p086/negative/` still contain `"placeholder negative fixture"`, including attach-receipt redaction, resurrection timeout, raw-session-id nondisclosure, output-only violation, stale-orphan reap, and terminal-idempotency replay fixtures.

Impact:

- Gate success is real for the covered paths, but it is not sufficient evidence for the proposal's complete acceptance matrix. This blocks Ready/closeout.

## Requirement Audit

| Requirement area | Status | Notes |
|---|---:|---|
| Distinct continuation modes | Partial | Modes exist, but `normal_fresh_execution` / `normal_live_reuse` are not modeled as first-class continuation-mode readback in the audited surfaces. |
| Adapter capability contract | Partial | Type and Claude declaration exist; MCP admission still uses static provider-name truth instead of selected adapter capability. |
| Frozen catalog gate | Partial | Gate exists and fails closed, but shipped catalog disables resurrection. |
| Claude resurrection happy path | Partial | Test proves attach/resume and identity match before prompt; default enablement and replay matrix remain incomplete. |
| Identity mismatch before prompt | Implemented for covered fixture | Daemon test verifies no prompt and no raw receipt on mismatch. |
| Prompt-turn marker before prompt | Partial | Receipt includes `prompt_turn_marker_id`; broader request/turn provider ids remain null in the tested path. |
| Output-only recovery | Partial | Prompt/readback visible; enforcement and negative evidence incomplete. |
| Session-store recovery | Not implemented for P086 | Schema fields exist but resurrection receipt path records `not_attempted`. |
| Durable phase/replay matrix | Partial | Important guards exist; required phase names, typed enum, and per-phase tests/evidence incomplete. |
| Readback/API parity | Partial | Attach receipt readback exists; standard continuation record omits `resurrection_phase`. |
| Metrics | Partial | Durable metric table/summary fields exist; full requested/success/failure/no-progress/fresh-retry-avoided semantics are not fully proven for resurrection. |
| Security/redaction | Partial | R9 raw-log issue appears fixed; DB raw receipts and access audit exist; several redaction/nondisclosure fixtures remain placeholders. |
| Canonical gates | Pass for current aliases | `proposal-086`, `p086-continuation-readback`, `p086-continuation-negative-fixtures`, and `p086-continuation-operator-report` passed, but the gates do not cover all proposal-required evidence. |

## Verification Log

- `./scripts/test-gate.sh proposal-086` -> passed.
  - Rust domain/acp/db/engine/mcp/graphql/daemon tests passed.
  - Swift readback target passed; Xcode reported `** TEST SUCCEEDED **`.
  - Note: selected Swift test invocation executed 0 app tests after build in this environment; treated as gate pass because the repository gate exited 0.
- `./scripts/test-gate.sh p086-continuation-readback` -> passed.
- `./scripts/test-gate.sh p086-continuation-negative-fixtures` -> passed.
- `./scripts/test-gate.sh p086-continuation-operator-report` -> passed.
- Static audit inspected proposal, R9, MCP admission, ACP manager/adapters, engine worker, DB migrations/repos, GraphQL readback, catalog YAML, schemas, evidence fixtures, and gate definitions.

## Security Summary

Security posture improved from R9:

- Provider session IDs are no longer plainly logged in the audited manager attach/mismatch path.
- Raw v2 receipts moved to SQLite (`p086_resurrection_raw_receipts`) instead of writing raw provider session ids into same-UID filesystem paths.
- MCP/GraphQL attach receipt access implements principal-aware projections and access audit rows.

Residual security/readiness risks:

- Redaction and raw-session-id nondisclosure negative fixtures remain placeholders for several lanes.
- MCP admission does not prove the actual selected adapter capability before enqueue.
- Full orphan-reap/unprompted timeout/replay evidence is not yet real for resurrection-enabled operation.

## Final Verdict

**Partial implementation, Not Ready.**

The current tree is materially ahead of R9: it now has a real Claude-style attach path, DB-backed raw v2 receipts, identity mismatch before prompt, and passing P086 gates. However, the implementation still does not satisfy the proposal as written because the default catalog disables resurrection, admission does not verify the actual selected adapter capability, the phase/replay matrix is incomplete, output-only recovery remains advisory, session-store recovery is not integrated into P086 resurrection, and many proposal-required negative fixtures are placeholders.

Closeout should wait until those blockers are fixed and the gates are expanded or supplemented so they fail on the currently ignored placeholder/resurrection matrix gaps.
