# P047 Session Reuse And Cancellation Settlement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Proposal 047 in the Rust control-plane with durable session lineage, transport-backed live ACP session reuse, generation-scoped budget evaluation, and two-phase cancellation settlement with northbound read parity.

**Architecture:** Land P047 in three vertical slices. First add the canonical schema/domain/repo surface for session lineage and execution provenance. Then switch ACP from one-shot execution to manager-owned live reusable sessions and integrate executor-side reuse policy/budget logic. Finally add cancellation settlement, work-item cleanup, and GraphQL/MCP reader wiring around canonical run truth plus projection summaries.

**Tech Stack:** Rust workspace (`domain`, `db`, `engine`, `acp`, `workflow`, `graphql-server`, `mcp-server`), SQLite migrations, Tokio async runtime, async-graphql, serde JSON.

---

### Task 1: Schema And Domain Foundations

**Files:**
- Create: `control-plane/crates/db/migrations/006_session_lineage.sql`
- Create: `control-plane/crates/domain/src/session.rs`
- Modify: `control-plane/crates/domain/src/lib.rs`
- Modify: `control-plane/crates/domain/src/agent.rs`
- Modify: `control-plane/crates/domain/src/run.rs`
- Create: `control-plane/crates/db/src/repos/sessions.rs`
- Modify: `control-plane/crates/db/src/repos/mod.rs`
- Modify: `control-plane/crates/db/src/repos/agent_executions.rs`
- Test: `control-plane/crates/db/tests/integration.rs`

- [ ] Add failing DB/domain tests that prove legacy `session_lineages` migration compatibility, session generation persistence, and agent execution provenance fields.
- [ ] Run focused DB tests and verify they fail for missing session schema/repo support.
- [ ] Implement `006_session_lineage.sql` with legacy rename, new canonical lineage/generation/event tables, run settlement log column, and agent execution provenance columns.
- [ ] Implement `domain::session` types plus `AgentExecution` and `Run` extensions required by P047.
- [ ] Implement `db::repos::sessions` and expand `agent_executions` repo read/write support for provenance fields.
- [ ] Re-run focused DB tests until green.

### Task 2: ACP Live Session Ownership And Reuse Flow

**Files:**
- Modify: `control-plane/crates/acp/src/manager.rs`
- Modify: `control-plane/crates/acp/src/session.rs`
- Modify: `control-plane/crates/acp/src/transport.rs`
- Modify: `control-plane/crates/acp/src/lib.rs`
- Create: `control-plane/crates/engine/src/session/mod.rs`
- Create: `control-plane/crates/engine/src/session/policy.rs`
- Create: `control-plane/crates/engine/src/session/fingerprint.rs`
- Create: `control-plane/crates/engine/src/session/budget.rs`
- Modify: `control-plane/crates/engine/src/executor.rs`
- Modify: `control-plane/crates/engine/src/lib.rs`
- Modify: `control-plane/crates/workflow/src/compiler.rs`
- Modify: `control-plane/crates/workflow/src/plan.rs`
- Test: `control-plane/crates/acp/tests/integration.rs`
- Test: `control-plane/crates/engine/tests/integration.rs`

- [ ] Add failing ACP/engine tests for manager-owned active session handles, fresh-vs-reused policy decisions, missing-live-handle fail-closed behavior, and budget-driven compact/invalidate outcomes.
- [ ] Run the focused ACP/engine tests and confirm they fail for the expected missing behavior.
- [ ] Refactor ACP transport into reusable primitives: initialize/start, prompt existing session, close session, shutdown process.
- [ ] Make `AcpRuntimeManager` own active live sessions keyed by session generation, with explicit close/invalidate paths.
- [ ] Implement engine `session` module for invocation owner keys, binding fingerprints, reuse policy, and budget evaluation.
- [ ] Integrate executor flow so agent executions persist provenance before prompt dispatch and reuse only succeeds when policy and live handle both match.
- [ ] Re-run focused ACP/engine tests until green.

### Task 3: Cancellation Settlement And Northbound Readers

**Files:**
- Create: `control-plane/crates/engine/src/cancellation.rs`
- Modify: `control-plane/crates/engine/src/command_handler.rs`
- Modify: `control-plane/crates/engine/src/executor.rs`
- Modify: `control-plane/crates/db/src/work_item.rs`
- Modify: `control-plane/crates/db/src/repos/runs.rs`
- Modify: `control-plane/crates/db/src/repos/projections.rs`
- Modify: `control-plane/crates/graphql-server/src/schema.rs`
- Modify: `control-plane/crates/graphql-server/src/types/run.rs`
- Modify: `control-plane/crates/mcp-server/src/tools/runs.rs`
- Test: `control-plane/crates/db/tests/integration.rs`
- Test: `control-plane/crates/engine/tests/integration.rs`

- [ ] Add failing tests for Phase 1 and Phase 2 cancellation settlement, work-item cleanup to `Cancelled`, canonical single-run log exposure, and projection-backed list summaries.
- [ ] Run the focused tests and verify failure is caused by the missing settlement/read-model behavior.
- [ ] Implement `engine::cancellation` two-phase settlement flow and wire `CancelRun` through it.
- [ ] Add `WorkItemStatus::Cancelled`, `Run.cancellation_settlement_log`, repo persistence, and projection summary derivation.
- [ ] Wire GraphQL and MCP reads so single-run access uses canonical run truth and list access remains projection-backed summary-only.
- [ ] Re-run focused tests until green.

### Task 4: Proposal Gate And Regression Proof

**Files:**
- Modify: `scripts/test-gate.sh`
- Modify: `docs/reference/test-gates.md`
- Test: `control-plane/crates/db/tests/integration.rs`
- Test: `control-plane/crates/acp/tests/integration.rs`
- Test: `control-plane/crates/engine/tests/integration.rs`

- [ ] Add the `proposal-047` gate definition matching the proposal.
- [ ] Run the minimal focused commands that prove the new lineage, ACP reuse, and cancellation settlement paths.
- [ ] Run `./scripts/test-gate.sh proposal-047` and fix any residual failures.
- [ ] Capture the exact verification commands and final pass/fail status in the turn summary.
