# Proposal 027: Rust + SQLite Local Control Plane Extraction Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-rust-sqlite-local-control-plane-extraction.md` |
| Repository Root | `.` |
| Git SHA | `d3c5e22` |
| Working Tree | dirty (large pre-existing delta, including control-plane workspace and many unrelated app-side edits) |
| Audited At | `2026-04-12T09:06:27+0300` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

This `R4` pass supersedes `R3`. The basis improved materially on the same tree. The dedicated `proposal-027` gate now passes a richer control-plane slice with 13 executed tests across the `db`, `domain`, and `engine` crates, including projection-parity, approval/reject/retry command semantics, and startup-repair coverage. The MCP server no longer exposes an empty resource catalog, and the ACP adapter layer no longer returns hard-coded mock success; it now shells out to configured provider binaries.

That closes several old blockers from `R3`, but it still does not justify a success verdict. The proposal’s hardest bar is no longer "missing implementation plumbing"; it is "validated parity context." The current tree still does not prove one bounded real runtime-backed daemon slice end-to-end, the query layer still mixes projection-backed and canonical reads, and no broader daemon-vs-app non-regression harness was found for run/report/recovery semantics. `P027` is now meaningfully beyond the earlier red basis, but it is still short of the proposal’s own completion bar.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | The daemon path is substantially implemented, but the proposal’s validated parity proof bar is still incomplete | High |
| Architecture | At Risk | Query truth is still split and the ACP path is not yet proven by an executed live slice | High |
| Product | At Risk | The gate now covers core daemon semantics, but still does not prove broader run/report/recovery non-regression | High |
| UI | Acceptable | The app remains correctly app-owned during parity, matching the proposal | High |
| UX | Acceptable | No thin-client cutover was accidentally introduced | High |
| Readiness | Not Ready | The dedicated gate is useful now, but it still is not a sign-off-grade parity harness | High |

## Proposal Contract

### Scope

- Build a local Rust daemon as a parity replica of the current client-owned control plane.
- Move durable truth for the replicated slice into SQLite plus local artifact storage.
- Preserve parity-phase ownership so the client stays canonical until later cutover.
- Stand up GraphQL and MCP northbound boundaries without making the SwiftUI client depend on them yet.

### Locked Decisions

- The topology remains local-first: one daemon, one SQLite database, one local file store.
- The workflow engine is product-owned rather than Temporal-backed.
- During `P027`, the daemon is shadow truth and the client remains canonical.
- Thin-client cutover, finalized GraphQL client consumption, and finalized MCP command exposure are deferred.

### Primary User Flows

1. Start and progress a run through the Rust daemon while preserving app-owned product authority during parity.
2. Process approval and retry actions through the daemon service layer and mirror resulting state safely.
3. Restart the daemon and recover incomplete work predictably from durable SQLite-backed state.
4. Expose daemon state through GraphQL and MCP so a later parity harness can validate shadow truth before cutover.

### UI Commitments

- No user-visible thin-client cutover happens in `P027`.
- The current app-owned shell remains the live operator path during parity.

### UX Commitments

- Existing run/report/recovery semantics should not regress while the daemon replica is built.
- Parity validation should happen before any authority transfer.

### Acceptance Criteria

- Orchestration logic is executable in the Rust daemon end-to-end.
- SQLite is the durable local source of truth for at least one real workflow slice.
- Daemon projections correctly replicate run/stage/approval/artifact state and are verifiable through a parity comparison tool.
- Approval and retry commands are processed correctly through the daemon service layer under parity validation.
- Current product semantics do not regress.
- The system survives process restart with predictable local repair.
- The daemon can own workflow decisions end-to-end in a validated parity context while client transfer stays deferred.
- The local topology remains `daemon + SQLite + local file store` without an external workflow platform.

### Test / Evidence Requirements

- Strong local evidence for the daemon path, including live executable proof where claims are end-to-end.
- Parity-comparison tooling for daemon shadow truth versus the current app-owned baseline.
- Since this audit does not land on a success-rollup verdict, same-tree full regression evidence was not required for this pass.

### Explicit Exclusions

- No thin-client cutover in `P027`.
- No final GraphQL client contract.
- No final MCP command exposure contract.
- No multi-node / remote-host platformization.

## Proposal Fidelity / Divergence

### Matches

- A separate Rust `control-plane/` workspace exists with daemon, database, engine, GraphQL, MCP, and ACP crates.
- The daemon is local-first and bootable as a single process with SQLite-backed persistence.
- The repository now contains a dedicated `proposal-027` control-plane gate, and that gate passes on the audited tree.
- Projection-related tests now cover run/stage parity, approval/reject/retry command semantics, and startup-repair behavior.
- The current Swift app remains app-owned through `ExecutionService`, `RecoveryCoordinator`, `RunReportBuilder`, and `WorkflowMapProjectionService`.
- MCP resource discovery now exists for the daemon-owned shadow-truth surfaces.

### Divergences

- No executed real runtime-backed daemon slice was found for the `InvokeAgent` path.
- Query truth is still mixed: list-style GraphQL surfaces are projection-backed, but single-run detail and subscription payload refresh still read canonical repos directly.
- MCP advertises resources, but no `resources/read` handler or equivalent resource retrieval surface was found.
- No broader daemon-vs-app behavioral diff or report-parity harness was found.

### Ambiguities / Evidence Gaps

- No proof was found that configured ACP binaries are present and exercised on the audited machine.
- No live client walkthrough against daemon-backed read models was found, but that remains intentionally deferred by proposal.

## Track 1: Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partially Implemented | 7 |
| Missing | 0 |
| Not Verifiable | 0 |

## Track 1: Requirement Audit

### REQ-001 Local Rust daemon exists and is executable as a local control-plane process
- Proposal Source: Outcome; §3.1; §5.1; AC-1
- Status: Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/daemon/src/main.rs:15-119`
  - `./scripts/test-gate.sh proposal-027` -> passed on the audited tree
  - Runtime smoke: launched `cargo run --quiet` with `GRAPHQL_ADDR=127.0.0.1:51291`; observed `ALIVE=True`, `CONNECTED=True`, `DB_EXISTS=True`
- Gap / Note: No live boot blocker remains on this tree.

### REQ-002 SQLite is the durable local source of truth for at least one real workflow slice
- Proposal Source: Outcome; §3.2; §6.1-§6.5; AC-2
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/db/migrations/001_initial.sql:2-123`
  - `control-plane/crates/db/migrations/002_projections.sql:1-120`
  - `control-plane/crates/db/tests/integration.rs:101-228`
  - Runtime smoke created `control-plane/chainworks-control-plane.db`
- Gap / Note: Durable SQLite truth is now well established for canonical tables and projection rebuilds, but the audit still did not find one executed real runtime-backed workflow slice proving that durable truth across a bounded end-to-end daemon flow.

### REQ-003 A product-owned orchestration engine exists with explicit transitions, command handling, background work, and projection refresh
- Proposal Source: §3.3; §5.2; §7.1-§7.3; AC-1
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/domain_engine.rs`
  - `control-plane/crates/engine/src/orchestrator.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/engine/src/executor.rs:83-229`
  - `control-plane/crates/engine/tests/integration.rs:92-314`
  - `./scripts/test-gate.sh proposal-027` -> passed with 13 executed tests
- Gap / Note: The engine is materially real now, including approval/retry and startup-repair behavior. It remains partial because the background `InvokeAgent` path is not yet backed by executed parity proof on a real ACP provider slice.

### REQ-004 The daemon preserves the northbound GraphQL + MCP boundary shape
- Proposal Source: §4; §5.1; §8.1
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:39-328`
  - `control-plane/crates/mcp-server/src/server.rs:79-212`
  - `control-plane/crates/daemon/src/main.rs:89-115`
  - Runtime smoke confirmed GraphQL was listening on loopback
- Gap / Note: The boundary shape is clearly present now: GraphQL queries/mutations/subscriptions exist, MCP tools exist, and MCP resources are advertised. It is still partial because no `resources/read` surface was found, and the minimum subscription/read picture remains incomplete for the proposal’s later thin-client boundary.

### REQ-005 Daemon projections replicate run/stage/approval/artifact state as verifiable shadow truth
- Proposal Source: Outcome; §5.2 Projection engine; §6.1-§6.3; §8.2; AC-3
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/db/src/repos/projections.rs:11-179`
  - `control-plane/crates/db/src/repos/projections.rs:243-416`
  - `control-plane/crates/db/tests/integration.rs:96-228`
  - `control-plane/crates/graphql-server/src/schema.rs:59-93`
- Gap / Note: This requirement is stronger than in `R3`. Run/stage parity rebuild tests exist, and GraphQL now reads projection-backed approval/artifact/stage/run-list surfaces. It remains partial because a dedicated parity comparison tool or broader daemon-vs-app diff was not found, and single-run detail still bypasses the projection lane.

### REQ-006 Approval and retry commands are processed through the daemon service layer under parity validation
- Proposal Source: §5.2 Command handlers; §7.2; AC-4
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/tests/integration.rs:167-314`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/graphql-server/src/schema.rs:136-228`
  - `./scripts/test-gate.sh proposal-027` -> approval / reject / retry tests executed and passed
- Gap / Note: The daemon service layer clearly processes approve, reject, and retry semantics correctly. This remains partial rather than implemented because the proposal also asks for parity validation, and no executed cross-system or report-level parity proof was found for this command slice.

### REQ-007 The client remains the canonical owner during parity and no thin-client cutover occurs in P027
- Proposal Source: §1; §3.4; §8.2; §11
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift:476-620`
  - `Chainworks Forge/Views/RunsHomeView.swift:1-80`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift:21-107`
  - `rg -n "GraphQL|graphql" 'Chainworks Forge' 'Chainworks ForgeTests' --glob '!**/Proposal033Tests.swift' --glob '!**/ProviderPlatformTests.swift'` -> no app-side control-plane client path found
- Gap / Note: The app remains correctly app-owned for the parity phase.

### REQ-008 The system survives restart with predictable local repair
- Proposal Source: §5.2 Recovery and repair layer; §7.3; AC-6
- Status: Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/engine/src/recovery.rs`
  - `control-plane/crates/engine/tests/integration.rs:89-156`
  - `control-plane/crates/daemon/src/main.rs:79-87`
  - Runtime smoke confirmed successful daemon startup on the audited tree
- Gap / Note: Startup-repair behavior is directly exercised and no longer speculative.

### REQ-009 The daemon can own workflow decisions end-to-end in a validated parity context while client transfer stays deferred
- Proposal Source: Outcome; §8.2; §9.3; AC-7
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:90-199`
  - `control-plane/crates/acp/src/manager.rs:20-77`
  - `control-plane/crates/acp/src/adapters/claude.rs:18-119`
  - `control-plane/crates/acp/src/adapters/gemini.rs:18-119`
  - `control-plane/crates/acp/src/adapters/auggie.rs:18-119`
  - `control-plane/crates/acp/src/adapters/junie.rs:18-119`
- Gap / Note: This upgraded from the old `R3` blocker. The ACP path is no longer a mock-success stub; it now shells out to configured binaries. It remains partial because no executed `InvokeAgent` proof or validated real-runtime parity slice was found, and `BackgroundExecutor` still carries stub-default assumptions (`provider` fallback and hard-coded artifact contract metadata) in `control-plane/crates/engine/src/executor.rs:107-167`.

### REQ-010 The topology remains local-first: daemon + SQLite + local file store, without an external workflow platform
- Proposal Source: §4; §10; AC-8
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/daemon/src/main.rs:25-33`
  - `control-plane/Cargo.toml:1-19`
  - `control-plane/crates/db/migrations/001_initial.sql:66-84`
- Gap / Note: No external workflow platform or multi-service topology was introduced.

### REQ-011 Current run/report/recovery semantics are shown not to regress through parity validation
- Proposal Source: Outcome; §2; §12.3; AC-5
- Status: Partially Implemented
- Evidence Type: `tests-run`, `code`
- Evidence:
  - `./scripts/test-gate.sh proposal-027` -> passed with 13 executed tests
  - `control-plane/crates/db/tests/integration.rs:96-228`
  - `control-plane/crates/engine/tests/integration.rs:83-314`
  - `docs/reference/test-gates.md:364-392`
- Gap / Note: Non-regression proof is now materially stronger than `R3`: the gate documents and executes parity-style run/stage, approval/retry, and startup-repair coverage. It remains partial because no report-parity harness or broader daemon-vs-app behavioral diff was found.

## Track 2: Architecture Review

**Summary:** At Risk

### ARCH-001 The ACP execution path exists, but proposal-grade live parity proof is still absent
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-002, REQ-003, REQ-009
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/acp/src/manager.rs:20-77`
  - `control-plane/crates/acp/src/adapters/claude.rs:18-119`
  - `control-plane/crates/acp/src/adapters/gemini.rs:18-119`
  - `control-plane/crates/acp/src/adapters/auggie.rs:18-119`
  - `control-plane/crates/acp/src/adapters/junie.rs:18-119`
  - `control-plane/crates/engine/src/executor.rs:90-199`
- Why It Matters: The daemon is no longer blocked on fake adapter returns; it can now invoke configured ACP binaries. But architecture readiness still depends on executed proof that one bounded daemon-owned workflow slice works with a real runtime and preserves product semantics. That proof is still missing.
- Recommended Action: Add one audited `InvokeAgent` proof lane to `proposal-027`: start a bounded run, exercise a real or fixture ACP binary through `AcpRuntimeManager`, and validate resulting stage settlement plus artifact metadata.

### ARCH-002 Read truth is still split between projection-backed and canonical-backed paths
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-004, REQ-005, REQ-011
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:59-93`
  - `control-plane/crates/graphql-server/src/schema.rs:236-327`
  - `control-plane/crates/mcp-server/src/server.rs:79-212`
  - `control-plane/crates/db/src/repos/projections.rs:11-179`
- Why It Matters: Run lists, approval inbox, artifacts, and stage lists now read through projections, which is good progress. But `run(id)` and subscription refreshes still materialize from canonical repos, and MCP still stops at `resources/list` without a resource-read path. That leaves the parity replica without one clean shadow-read owner lane.
- Recommended Action: Finish the projection-backed read path for single-run detail and resource retrieval, or explicitly fence the remaining canonical reads out of proposal-owned parity claims until a later pass.

## Track 2: Product Review

**Summary:** At Risk

### PROD-001 The gate now proves core daemon semantics, but still not the proposal’s broader parity promise
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-005, REQ-006, REQ-009, REQ-011
- Evidence Type: `tests-run`, `code`
- Evidence:
  - `./scripts/test-gate.sh proposal-027` -> passed with 13 executed tests
  - `control-plane/crates/db/tests/integration.rs:96-228`
  - `control-plane/crates/engine/tests/integration.rs:83-314`
- Why It Matters: The proposal is not merely "daemon code exists." It is "daemon parity is validated before cutover." The current gate now proves meaningful internals, but it still does not establish broader operator-level non-regression for run/report/recovery semantics.
- Recommended Action: Add one proposal-owned parity harness that compares daemon output against the current app-owned baseline for a bounded run, including report/recovery artifacts, not just low-level state transitions.

## Track 2: UI Review

**Summary:** Acceptable

- No blocking UI finding. The app still owns the live operator shell, which matches the proposal’s parity-phase contract.

## Track 2: UX Review

**Summary:** Acceptable

- No blocking UX finding. No premature thin-client cutover or daemon-only operator flow was found on the current tree.

## Track 2: Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The dedicated `proposal-027` gate is real and useful now, but still not sign-off grade
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-001, REQ-005, REQ-009, REQ-011
- Evidence Type: `tests-run`, `runtime`, `design-reference`
- Evidence:
  - `./scripts/test-gate.sh proposal-027` -> passed
  - Runtime smoke: launched daemon successfully and confirmed loopback GraphQL connectivity
  - `docs/reference/test-gates.md:364-392`
- Why It Matters: `R4` closes the old handoff issues from `R2` and `R3`: the gate exists, its docs are in sync, and it runs a meaningful daemon slice. It is still not enough for proposal sign-off, because it does not yet include a real runtime-backed workflow proof or a daemon-vs-app parity diff.
- Recommended Action: Keep the current gate as the fast daemon confidence lane, then add a second proposal-owned parity lane or extend this one to cover one live runtime-backed slice plus baseline comparison output.

## Verification Snapshot

| Check | Result | Notes |
|---|---|---|
| Proposal-specific gate | Pass | `./scripts/test-gate.sh proposal-027` passed with 13 executed tests |
| Live daemon boot | Pass | Process stayed alive, accepted TCP on loopback GraphQL port, and created the SQLite DB |
| Real runtime-backed bounded slice | Partial | ACP adapter plumbing exists, but no executed parity proof was found |
| Same-tree full regression | Not Required | This audit does not land on a success-rollup verdict |

## Commands Run

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/027-rust-sqlite-local-control-plane-extraction.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/027-rust-sqlite-local-control-plane-extraction.md docs/proposals docs/reviews`
- `./scripts/test-gate.sh proposal-027`
- runtime smoke for `cargo run --quiet` in `control-plane/` with loopback GraphQL probe
- focused file reads under `control-plane/crates/{daemon,db,engine,graphql-server,mcp-server,acp}` and app-owned parity-path files under `Chainworks Forge/`
