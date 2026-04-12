# Proposal 027: Rust + SQLite Local Control Plane Extraction Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-rust-sqlite-local-control-plane-extraction.md` |
| Repository Root | `.` |
| Git SHA | `d3c5e22` |
| Working Tree | dirty (large pre-existing delta, including proposal edits and an untracked `control-plane/` workspace) |
| Audited At | `2026-04-11T21:28:16+0300` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 027 is materially underway on the current tree, not absent. A real Rust workspace exists with a daemon crate, SQLite migration, orchestration modules, GraphQL schema, and MCP server. The Swift app also still matches the proposal's parity-phase ownership rule: it remains app-owned through `ExecutionService`, `RunReportBuilder`, and `WorkflowMapProjectionService`, with no thin-client cutover in place.

The implementation is still short of the proposal contract. The strongest blockers are concrete, not stylistic: the daemon does not boot successfully under live `cargo run` proof on the audited tree, `BackgroundExecutor` still stubs agent invocation instead of performing real ACP-backed execution and settlement, the promised projection/read-model layer is not present, and there is no proposal-specific parity comparison lane proving that current run/report/recovery semantics match the app-owned baseline. Because multiple explicit acceptance criteria remain unfulfilled, this audit does not require a full same-tree regression gate to fail closed; the proposal is not yet at a success-rollup boundary.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Projection/parity validation and end-to-end daemon ownership are still missing | High |
| Architecture | Risky | The daemon path still contains execution and projection stubs | High |
| Product | Incomplete | The proposal's "ready for parity checking / shadow execution" outcome is not yet true on this tree | High |
| UI | Deferred | P027 correctly defers thin-client UI cutover, and the current app still stays app-owned | High |
| UX | Deferred | User-visible daemon consumption is intentionally out of scope for P027 | High |
| Readiness | Not Ready | Live daemon boot fails, the Rust workspace has zero executed tests, and the repo's `proposal-027` gate points at a different proposal | High |

## Proposal Contract

### Scope

- Build a local Rust daemon as a parity replica of the current client-owned control plane.
- Move durable truth for the replicated slice into SQLite plus local artifact storage.
- Preserve parity-phase ownership: the client stays canonical until later cutover.
- Stand up GraphQL and MCP northbound boundaries without making the client depend on them yet.

### Locked Decisions

- The system remains local-first: one daemon, one SQLite database, one local file store.
- The workflow engine is product-owned, not Temporal or another external workflow platform.
- During P027, the daemon is shadow truth; the client remains the canonical owner.
- Thin-client cutover, GraphQL client migration, and finalized MCP exposure are explicitly deferred.

### Primary User Flows

1. Start and progress a run through the Rust daemon while preserving app-owned product authority during parity.
2. Process approval and retry actions through the daemon service layer and mirror resulting state safely.
3. Restart the daemon and recover incomplete work predictably from durable SQLite-backed state.
4. Expose run/idea/stage/artifact state northbound through GraphQL and MCP for parity checking and later cutover.

### UI Commitments

- No user-visible thin-client cutover happens in P027.
- The current app-owned shell remains the live operator path during parity.

### UX Commitments

- Existing run/report/recovery semantics should not regress while the daemon replica is built.
- Parity validation should happen before any authority transfer.

### Acceptance Criteria

- Orchestration logic is executable in the Rust daemon end-to-end.
- SQLite is the durable local source of truth for at least one real workflow slice.
- Daemon projections replicate run/stage/approval/artifact state and are verifiable through a parity comparison tool.
- Approval and retry commands work through the daemon service layer under parity validation.
- Current product semantics do not regress.
- The system survives process restart with predictable local repair.
- The daemon can own workflow decisions end-to-end in a validated parity context while client transfer stays deferred.
- Topology remains `daemon + SQLite + local file store` without a separate workflow platform.

### Test / Evidence Requirements

- Strong local evidence for the daemon path, including live executable proof where claims are end-to-end.
- Parity comparison tooling for daemon shadow truth versus the current app-owned baseline.
- Because the proposal is not yet in a success-rollup state, a full same-tree regression gate was not required for this audit pass.

### Explicit Exclusions

- No thin-client cutover in P027.
- No final GraphQL client contract.
- No final MCP command exposure contract.
- No multi-node / remote-host platformization.

## Proposal Fidelity / Divergence

### Matches

- A separate Rust `control-plane/` workspace exists with daemon, database, engine, GraphQL, and MCP crates.
- The daemon process shape in code is local-first and single-process.
- The current Swift app remains app-owned through `ExecutionService`, `RunReportBuilder`, and `WorkflowMapProjectionService`.
- The design does not introduce an external workflow platform.

### Divergences

- Live daemon boot currently fails before establishing durable local operation.
- Agent invocation and projection rebuilding are still stubbed / unimplemented in the daemon executor.
- The promised projection/read-model and parity comparison layer is absent.
- The repository's `proposal-027` proof gate is wired to the unrelated renderer proposal, not this control-plane extraction proposal.

### Ambiguities / Evidence Gaps

- No executed parity harness or behavioral diff was found for comparing daemon shadow truth against the live app-owned path.
- No live app walkthrough against daemon-backed reads exists, but that is deferred by proposal and therefore not treated as a conformance gap by itself.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 2 |
| Partially Implemented | 6 |
| Missing | 3 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Local Rust daemon process exists and is executable as the local control plane
- Proposal Source: Outcome; §3.1; §5.1; AC-1
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/daemon/src/main.rs:15-119`
  - `cargo test --workspace` in `control-plane/` completed successfully, proving the daemon crate compiles on the audited tree
  - Live runtime proof: `cargo run --quiet` in `control-plane/` exited immediately with `error returned from database: (code: 14) unable to open database file`
- Gap / Note: The daemon binary and startup path exist in code, but the strongest live proof currently fails before the service reaches a running state.

### REQ-002 SQLite is the durable local source of truth for at least one real workflow slice
- Proposal Source: Outcome; §3.2; §6.1-§6.5; AC-2
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/db/migrations/001_initial.sql:2-123`
  - `control-plane/crates/daemon/src/main.rs:41-43`
- Gap / Note: The schema covers `runs`, `stage_executions`, `agent_executions`, `approvals`, `artifacts`, `work_items`, and `command_journal`, but it does not include the proposal's broader table groups such as `session_lineages`, `aggregate_settlements`, `background_leases`, `startup_repairs`, `runtime_invocations`, or the projection/read tables promised in §6.3. The audited tree also does not prove one real slice end-to-end because the daemon fails to boot live.

### REQ-003 A product-owned orchestration engine exists with explicit transitions, command handling, and background work
- Proposal Source: §3.3; §5.2; §7.1-§7.2; AC-1
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/domain_engine.rs:1-106`
  - `control-plane/crates/engine/src/orchestrator.rs:16-137`
  - `control-plane/crates/engine/src/command_handler.rs:18-274`
  - `control-plane/crates/engine/src/executor.rs:15-130`
- Gap / Note: The orchestration skeleton is real, but it remains incomplete relative to the proposal contract. `InvokeAgent` is still a stub and `RebuildProjection` is explicitly ignored in the executor, so the daemon does not yet execute the full workflow loop described in §7.2.

### REQ-004 The daemon preserves the northbound GraphQL + MCP boundary shape
- Proposal Source: §4; §5.1; §8.1
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:23-333`
  - `control-plane/crates/mcp-server/src/server.rs:13-172`
  - `control-plane/crates/daemon/src/main.rs:89-115`
- Gap / Note: Both surfaces exist, but the shape is incomplete. GraphQL currently exposes run/stage/approval streams only, not the proposal's minimum runtime/session status subscription. MCP `resources/list` returns an empty array even though §8.1 names canonical `idea://`, `run://`, `artifact://`, and `report://` resources.

### REQ-005 Daemon projections replicate run/stage/approval/artifact state as verifiable shadow truth
- Proposal Source: Outcome; §5.2 Projection engine; §6.1-§6.3; §8.2; AC-3
- Status: Missing
- Evidence Type: `code`, `inference`
- Evidence:
  - `control-plane/crates/db/migrations/001_initial.sql:1-123`
  - `control-plane/crates/graphql-server/src/schema.rs:41-98`
  - `control-plane/crates/engine/src/executor.rs:120-124`
  - `rg -n "run_summaries|stage_summaries|approval_inbox|artifact_index|proposal_loop_metrics|recovery_recommendations|session_lineages|aggregate_settlements|background_leases|startup_repairs|runtime_invocations" control-plane`
- Gap / Note: The promised projection/read-model tables and rebuild path are absent. GraphQL currently reads canonical repo tables directly, `approval_inbox` is only a direct query name rather than a persisted read model, and `RebuildProjection` is not implemented. No parity comparison tool or shadow-truth verifier was found.

### REQ-006 Approval and retry commands are processed through the daemon service layer under parity validation
- Proposal Source: §5.2 Command handlers; §7.2; AC-4
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:77-220`
  - `control-plane/crates/graphql-server/src/schema.rs:141-233`
  - `control-plane/crates/mcp-server/src/server.rs:150-170`
- Gap / Note: Approve/reject/retry commands exist, but the implementation is narrower than the proposal's service-layer contract and no parity-validation evidence exists. Retry currently clones a new `StageExecution` after settling the old one as skipped; no broader agent-retry / snapshot-clone / parity-check path was found.

### REQ-007 The client remains the canonical owner during parity and no thin-client cutover occurs in P027
- Proposal Source: §1; §3.4; §8.2; §11
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift:476-575`
  - `Chainworks Forge/Views/RunsHomeView.swift:9-17`
  - `Chainworks Forge/Views/IdeaListView.swift:5-13`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift:21-47`
  - `rg -n "GraphQL|graphql" 'Chainworks Forge' 'Chainworks ForgeTests' --glob '!**/Proposal033Tests.swift' --glob '!**/ProviderPlatformTests.swift'` returned no app-side control-plane client path
- Gap / Note: This is aligned with the rewritten proposal. The app still uses `SwiftData + ExecutionService` and app-owned read/report/recovery paths rather than daemon-backed thin-client rendering.

### REQ-008 The system survives restart with predictable local repair
- Proposal Source: §5.2 Recovery and repair layer; §7.3; AC-6
- Status: Partially Implemented
- Evidence Type: `code`, `runtime`
- Evidence:
  - `control-plane/crates/engine/src/recovery.rs:13-116`
  - `control-plane/crates/daemon/src/main.rs:79-87`
  - Live runtime proof: `cargo run --quiet` never reached a successful startup state because database opening failed first
- Gap / Note: A recovery service exists in code and enqueues startup repair, but the audited tree does not prove restart-safe behavior end-to-end. The live daemon fails before successful startup, and the repair logic itself is still narrow: it only blocks `Running` stages and re-enqueues `AdvanceRun`.

### REQ-009 The daemon can own workflow decisions end-to-end in a validated parity context while client transfer stays deferred
- Proposal Source: Outcome; §8.2; §9.3; AC-7
- Status: Missing
- Evidence Type: `code`, `inference`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:79-103`
  - `control-plane/crates/engine/src/domain_engine.rs:20-61`
  - `control-plane/crates/engine/src/orchestrator.rs:47-133`
- Gap / Note: The current daemon does not yet own a real end-to-end workflow loop. Agent invocation is still stubbed, outcomes are not durably materialized from real ACP execution, and no validated parity context was found.

### REQ-010 The topology remains local-first: daemon + SQLite + local file store, without an external workflow platform
- Proposal Source: §4; §10; AC-8
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/daemon/src/main.rs:25-33`
  - `control-plane/crates/db/migrations/001_initial.sql:66-84`
  - `control-plane/Cargo.toml:1-19`
- Gap / Note: The current implementation does stay within the proposal's intended local topology. No Temporal or separate workflow platform dependency was introduced.

### REQ-011 Current run/report/recovery semantics are shown not to regress through parity validation
- Proposal Source: Outcome; §2; §12.3; AC-5
- Status: Missing
- Evidence Type: `code`, `tests-run`, `inference`
- Evidence:
  - `cargo test --workspace` in `control-plane/` -> all crates compiled, but `0` tests executed across the workspace
  - `rg -n "parity|shadow truth|behavioral diff|golden run|comparison tool|projection drift|verifiable shadow" control-plane 'Chainworks Forge' 'Chainworks ForgeTests' --glob '!control-plane/target/**'`
  - `scripts/test-gate.sh:1178`
  - `Chainworks ForgeTests/Proposal027Tests.swift:1-120`
- Gap / Note: No proposal-specific parity harness or behavioral diff was found. The repo's existing `proposal-027` gate and `Proposal027Tests` belong to the unrelated renderer proposal, not this Rust/SQLite control-plane extraction effort.

## Architecture Review

### ARCH-001 Background execution is still a skeleton, not a real control-plane executor
- Severity: Major
- Confidence: High
- Related Proposal Items / REQ IDs: REQ-003, REQ-009
- Evidence Type: `code`
- Evidence References:
  - `control-plane/crates/engine/src/executor.rs:79-103`
  - `control-plane/crates/engine/src/executor.rs:120-124`
- Why It Matters: The proposal's core value is that the daemon can actually hold workflow truth. With `InvokeAgent` still stubbed and `RebuildProjection` explicitly unimplemented, the current service cannot act as a faithful parity replica of the app-owned control plane.
- Recommended Action: Replace the executor stub with a real ACP-backed invocation path that persists `AgentExecution`, artifacts, receipts, and settlement updates, then wire projection rebuilding off those mutations.

### ARCH-002 The promised projection/read-model layer has not been built
- Severity: Major
- Confidence: High
- Related Proposal Items / REQ IDs: REQ-005
- Evidence Type: `code`, `inference`
- Evidence References:
  - `docs/proposals/027-rust-sqlite-local-control-plane-extraction.md:331-355` (proposal target tables)
  - `control-plane/crates/db/migrations/001_initial.sql:1-123` (actual migration)
  - `control-plane/crates/graphql-server/src/schema.rs:41-98`
  - `control-plane/crates/engine/src/executor.rs:120-124`
- Why It Matters: P027 now explicitly avoids cutover, so the daemon's value is shadow truth plus parity checking. Without persisted read models and rebuild tooling, GraphQL is just reading canonical rows directly and the system has no projection drift guardrail.
- Recommended Action: Add the promised projection tables, materialize them from canonical mutations, and make GraphQL consume those read models rather than direct table scans.

## Product Review

### PROD-001 The implementation is not yet "ready for parity checking, shadow execution, and later cutover"
- Severity: Major
- Confidence: High
- Related Proposal Items / REQ IDs: REQ-005, REQ-009, REQ-011
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence References:
  - `docs/proposals/027-rust-sqlite-local-control-plane-extraction.md:54-61`
  - `cargo test --workspace` in `control-plane/` -> `0` tests executed
  - `cargo run --quiet` in `control-plane/` -> database open failure before steady state
  - `scripts/test-gate.sh:1178`
  - `Chainworks ForgeTests/Proposal027Tests.swift:1-120`
- Why It Matters: The proposal's product outcome is not merely "Rust code exists". It is "the daemon is ready to be validated against the current client". The audited tree is not there yet.
- Recommended Action: Land a proposal-owned proof lane for this control-plane slice, add at least one bounded parity harness over a real workflow slice, and make the live daemon bootable on the default developer path before treating the effort as validation-ready.

## UI Review

No active proposal-specific UI finding. The rewritten proposal correctly defers thin-client UI migration, and the current app still remains on the app-owned `ExecutionService` / `SwiftData` path as intended for P027.

## UX Review

No active proposal-specific UX finding beyond the product/readiness gaps already captured above. P027 intentionally defers user-visible daemon consumption; the current shortfall is not UX polish, it is missing parity infrastructure.

## Readiness Review

### READY-001 Live daemon startup is currently red on the audited tree
- Severity: Critical
- Confidence: High
- Related Proposal Items / REQ IDs: REQ-001, REQ-008
- Evidence Type: `runtime`
- Evidence References:
  - `cargo run --quiet` in `control-plane/`
  - Output: `Error: error returned from database: (code: 14) unable to open database file`
- Why It Matters: The proposal explicitly commits to executable local daemon behavior. A compile-clean binary that cannot reach a running state is not good enough for parity validation or restart-recovery claims.
- Recommended Action: Fix the SQLite boot contract first and add a deterministic smoke test that proves daemon startup against a fresh local database path.

### READY-002 There is no trustworthy proposal-owned proof lane for this proposal
- Severity: Major
- Confidence: High
- Related Proposal Items / REQ IDs: REQ-011
- Evidence Type: `code`, `tests-run`
- Evidence References:
  - `scripts/test-gate.sh:121`
  - `scripts/test-gate.sh:1178`
  - `scripts/test-gate.sh:1390-1399`
  - `Chainworks ForgeTests/Proposal027Tests.swift:1-120`
  - `cargo test --workspace` in `control-plane/` -> `0` tests executed
- Why It Matters: The repository already uses proposal-owned gates as the canonical proof surface. For this proposal number, the existing gate is wired to the unrelated JSON/Markdown renderer proposal, while the Rust workspace itself currently executes zero tests. That makes the implementation hard to trust and easy to over-claim.
- Recommended Action: Create a dedicated control-plane proposal gate and rename or renumber the existing renderer gate to remove the collision. The new lane should cover daemon boot, one real workflow slice, parity shadow outputs, and restart repair.

## Recommended Next Actions

1. Make `cargo run` succeed on a fresh local database path and lock that with a smoke test.
2. Replace `InvokeAgent` and `RebuildProjection` stubs with real persisted execution/projection flows.
3. Add the promised projection/read-model tables plus rebuild tooling.
4. Add a proposal-owned parity harness for one bounded real slice and stop reusing the unrelated renderer `proposal-027` gate.
5. Only after those are green, reassess whether the daemon is genuinely "ready for parity checking" on the current tree.
