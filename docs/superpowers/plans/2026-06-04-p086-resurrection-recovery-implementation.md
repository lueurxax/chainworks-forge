# P086 Resurrection Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Chainworks recover useful agent work after ACP/session-boundary failures without silently retrying, losing outputs, or mixing normal reuse with provider-session resurrection.

**Architecture:** Add an explicit continuation-mode classification layer before retry/reuse decisions, gate stage completion on required-output settlement, and route ambiguous prompt-boundary failures into output recovery or provider-session resurrection. Claude resurrection uses a new managed ACP process attached to the recorded provider session id, backed by prompt-turn correlation and session-store recovery evidence.

**Tech Stack:** Rust control-plane (`engine`, `acp`, `db`, `mcp-server`), SQLite migrations/repos, YAML frozen catalog gates, focused `cargo test -p engine` and `cargo test -p acp` tests.

---

### Task 1: Completion Gate And Mode Classification

**Files:**
- Modify: `control-plane/crates/engine/src/executor.rs`
- Modify or create tests under: `control-plane/crates/engine/tests/`

- [ ] Write failing tests proving a stage/agent execution cannot complete when required outputs are missing unless a typed recoverable blocker is persisted.
- [ ] Write failing tests proving `prompt_closed_during_stream`, `transport_closed`, `provider_timeout`, failed settlement, and cancellation are not eligible for silent `normal_live_reuse`.
- [ ] Add a small continuation-mode resolver with explicit modes: `normal_fresh_execution`, `normal_live_reuse`, `provider_session_resurrection`, `output_only_recovery`, `operator_action_required`.
- [ ] Wire the resolver into retry/reuse admission so ambiguous boundaries pick recovery/resurrection or fail closed instead of normal reuse.
- [ ] Run the focused engine tests and keep existing P058/P086/P090 tests green where directly affected.

### Task 2: Prompt-Turn Correlation And Recovery Receipts

**Files:**
- Modify: `control-plane/crates/engine/src/executor.rs`
- Modify DB repos/migrations only if an existing metadata/receipt field cannot carry the new evidence safely.
- Modify or create tests under: `control-plane/crates/engine/tests/`

- [ ] Write failing tests that recovered terminal output is rejected when stage execution id, agent execution id, request fingerprint, provider session id, or provider turn/request proof is missing or contradictory.
- [ ] Persist a prompt-turn marker before recovery/resurrection prompt send.
- [ ] Include prompt-turn marker, target execution ids, request fingerprint, and provider session id in recovery receipts/readback payloads.
- [ ] Require successful prompt-turn correlation before using recovered transcript/final output for settlement.

### Task 3: Claude Session-Store Recovery

**Files:**
- Modify: `control-plane/crates/acp/src/session.rs`
- Modify: `control-plane/crates/acp/src/manager.rs`
- Modify: `control-plane/crates/engine/src/executor.rs`
- Modify or create tests under: `control-plane/crates/acp/tests/` or inline module tests.

- [ ] Write failing tests for recovering a lost Claude terminal answer from session-store transcript only when it is bound to the target prompt marker/request fingerprint.
- [ ] Write failing tests for ambiguity: same provider session but different execution must fail closed.
- [ ] Add Claude session-store transcript lookup/digest/recovery result evidence.
- [ ] Expose recovered `CHAINWORKS_OUTPUT` or direct-file manifest to engine settlement only after ownership proof passes.

### Task 4: Provider-Session Resurrection Admission

**Files:**
- Modify: `control-plane/crates/acp/src/adapters/mod.rs`
- Modify: `control-plane/crates/acp/src/adapters/claude.rs`
- Modify: `control-plane/crates/acp/src/manager.rs`
- Modify: `control-plane/crates/engine/src/executor.rs`
- Modify tests under `control-plane/crates/daemon/tests/` and/or `control-plane/crates/engine/tests/`

- [ ] Write failing tests that unsupported adapters still reject resurrection.
- [ ] Write failing tests that frozen catalog opt-in is required.
- [ ] Implement a typed adapter capability contract and Claude attach/resume request/result.
- [ ] Start a new managed ACP process with requested Claude provider session id.
- [ ] Persist requested/actual session id proof before prompt send.
- [ ] Fail closed on unsupported, mismatch, missing/expired session, quota/auth, or unverifiable identity.

### Task 5: Output-Only Recovery

**Files:**
- Modify: `control-plane/crates/engine/src/executor.rs`
- Modify or create focused output settlement tests.

- [ ] Write failing tests that valid direct-file outputs from the prior attempt are preserved during repair.
- [ ] Write failing tests that recovery asks only for missing/invalid required outputs.
- [ ] Add pre/post source snapshot evidence for output-only recovery and default `changed_source_files == 0`.
- [ ] Allow source edits only when operator instruction explicitly permits them and receipt records changed source files.

### Task 6: Startup And Watchdog Recovery

**Files:**
- Modify: `control-plane/crates/engine/src/recovery.rs` or current recovery owner.
- Modify: `control-plane/crates/engine/src/executor.rs` if recovery remains executor-owned.
- Modify focused startup recovery tests.

- [ ] Write failing tests for running work item without live provider process.
- [ ] Write failing tests for completed agent execution with missing required outputs.
- [ ] Write failing tests for active session generation without active work item.
- [ ] Write failing tests for stale provider subprocess and stale targeted `advance_run`.
- [ ] Implement recovery classification and repair/requeue only through the correct mode.

### Task 7: Readback, Metrics, And Gate

**Files:**
- Modify GraphQL/MCP/report readback owners as needed.
- Modify docs/reference after behavior is implemented.
- Modify `scripts/test-gate.sh` only if a focused proposal gate must be added.

- [ ] Add readback fields for selected mode, rejected modes, provider session id, prompt-turn marker, session-store recovery result, recovered outputs, and fail-closed reason.
- [ ] Add metrics for resurrection requested/success/failure, output-only recovery requested/success/failure, and fresh retry avoided.
- [ ] Update reference docs after implementation.
- [ ] Run focused tests, `cargo build`, and the relevant proposal gate or documented focused equivalent.
