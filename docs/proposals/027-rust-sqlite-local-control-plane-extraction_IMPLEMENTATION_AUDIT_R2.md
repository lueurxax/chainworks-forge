# Proposal 027: Rust + SQLite Local Control Plane Extraction Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-rust-sqlite-local-control-plane-extraction.md` |
| Repository Root | `.` |
| Git SHA | `d3c5e22` |
| Working Tree | dirty (large pre-existing delta, including control-plane workspace and many unrelated app-side edits) |
| Audited At | `2026-04-11T22:00:41+0300` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

This `R2` pass supersedes `R1`. Two of the old blockers are now closed on the current tree: the daemon does boot successfully as a live local process with GraphQL listening on the configured loopback address, and the repository now has a dedicated `proposal-027` control-plane gate that passes. The implementation has also moved beyond the earlier schema-only state: projection tables and rebuild helpers are now present, and the background executor no longer ignores projection rebuild work.

The proposal is still not implemented end-to-end. The strongest remaining gaps are proposal-owned, not incidental: the ACP runtime manager still routes through stub adapters that return mock success, the northbound GraphQL path still reads canonical tables directly instead of consuming the daemon's projection layer, and there is still no executed parity-comparison harness proving that daemon shadow truth matches the current app-owned run/report/recovery semantics. Because explicit acceptance criteria remain unmet, this audit stays `Not Implemented / Not Ready` without needing a same-tree full regression roll-up.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Validated parity context and non-regression proof are still absent | High |
| Architecture | At Risk | ACP execution is still backed by stub adapters, and GraphQL bypasses the projection layer | High |
| Product | Incomplete | The daemon is buildable and bootable, but not yet ready for the parity-validation role promised by the proposal | High |
| UI | Acceptable | P027 correctly keeps the current app-owned shell live during parity | High |
| UX | Acceptable | User-visible daemon consumption is still intentionally deferred | High |
| Readiness | Not Ready | The repo-owned gate is now live, but it is still too thin to prove one bounded real parity slice | High |

## Proposal Contract

### Scope

- Build a local Rust daemon as a parity replica of the current client-owned control plane.
- Move durable truth for the replicated slice into SQLite plus local artifact storage.
- Preserve parity-phase ownership so the client stays canonical until later cutover.
- Stand up GraphQL and MCP northbound boundaries without making the SwiftUI client depend on them yet.

### Locked Decisions

- The topology remains local-first: one daemon, one SQLite database, one local file store.
- The workflow engine is product-owned rather than Temporal-backed.
- During P027, the daemon is shadow truth and the client remains canonical.
- Thin-client cutover, finalized GraphQL client consumption, and finalized MCP command exposure are deferred.

### Primary User Flows

1. Start and progress a run through the Rust daemon while preserving app-owned product authority during parity.
2. Process approval and retry actions through the daemon service layer and mirror resulting state safely.
3. Restart the daemon and recover incomplete work predictably from durable SQLite-backed state.
4. Expose run/idea/stage/artifact state through GraphQL and MCP so a later parity harness can validate daemon shadow truth.

### UI Commitments

- No user-visible thin-client cutover happens in P027.
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
- The local topology remains `daemon + SQLite + local file store` without a separate workflow platform.

### Test / Evidence Requirements

- Strong local evidence for the daemon path, including live executable proof where claims are end-to-end.
- Parity-comparison tooling for daemon shadow truth versus the current app-owned baseline.
- Since this audit does not land on a success-rollup verdict, same-tree full regression evidence was not required for this pass.

### Explicit Exclusions

- No thin-client cutover in P027.
- No final GraphQL client contract.
- No final MCP command exposure contract.
- No multi-node / remote-host platformization.

## Proposal Fidelity / Divergence

### Matches

- A separate Rust `control-plane/` workspace exists with daemon, database, engine, GraphQL, MCP, and ACP crates.
- The daemon is local-first and bootable as a single process with SQLite-backed persistence.
- Projection-related tables and rebuild helpers now exist.
- The current Swift app remains app-owned through `ExecutionService`, `RunReportBuilder`, and `WorkflowMapProjectionService`.
- The repository now contains a dedicated `proposal-027` control-plane gate.

### Divergences

- ACP execution is still backed by stub adapters that return mock successful results.
- GraphQL currently reads canonical tables directly instead of consuming daemon projection tables as shadow read models.
- No executed parity-comparison harness or behavioral diff was found.
- The repository documentation for test gates is still stale about what `proposal-027` means.

### Ambiguities / Evidence Gaps

- No live app walkthrough against daemon-backed read models exists, but that is deferred by proposal and therefore not treated as a conformance gap by itself.
- No executed restart-repair scenario was found beyond boot-time smoke and code inspection.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 3 |
| Partially Implemented | 6 |
| Missing | 2 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Local Rust daemon exists and is executable as a local control-plane process
- Proposal Source: Outcome; §3.1; §5.1; AC-1
- Status: Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/daemon/src/main.rs:15-119`
  - `./scripts/test-gate.sh proposal-027` -> passed on the audited tree
  - Runtime smoke: launched `cargo run --quiet` with `GRAPHQL_ADDR=127.0.0.1:51291`; observed `ALIVE=True`, `CONNECTED=True`, `DB_EXISTS=True`
- Gap / Note: This closes the live-boot blocker from `R1`.

### REQ-002 SQLite is the durable local source of truth for at least one real workflow slice
- Proposal Source: Outcome; §3.2; §6.1-§6.5; AC-2
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/db/migrations/001_initial.sql:2-123`
  - `control-plane/crates/db/migrations/002_projections.sql:1-120`
  - `control-plane/crates/db/tests/integration.rs:12-93`
  - Runtime smoke created `control-plane/chainworks-control-plane.db`
- Gap / Note: Durable SQLite truth is clearly present and now broader than `R1`, but the current executed proof only covers repository inserts/status updates plus daemon boot. It does not yet prove one bounded real workflow slice end-to-end.

### REQ-003 A product-owned orchestration engine exists with explicit transitions, command handling, background work, and projection refresh
- Proposal Source: §3.3; §5.2; §7.1-§7.2; AC-1
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/domain_engine.rs:21-188`
  - `control-plane/crates/engine/src/orchestrator.rs:22-138`
  - `control-plane/crates/engine/src/command_handler.rs:33-274`
  - `control-plane/crates/engine/src/executor.rs:83-229`
  - `./scripts/test-gate.sh proposal-027` -> domain/engine tests passed
- Gap / Note: The engine is now materially farther along than in `R1`: `InvokeAgent` persists `AgentExecution`, settles stages, rebuilds projections, and advances the run. It remains partial because the runtime execution path underneath still goes through stub adapters rather than a validated parity runtime.

### REQ-004 The daemon preserves the northbound GraphQL + MCP boundary shape
- Proposal Source: §4; §5.1; §8.1
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:23-333`
  - `control-plane/crates/mcp-server/src/server.rs:19-172`
  - `control-plane/crates/daemon/src/main.rs:89-115`
  - `./scripts/test-gate.sh proposal-027` -> control-plane gate passed
- Gap / Note: Both surfaces exist, but the contract is incomplete. GraphQL exposes run/stage/approval subscriptions, not the full minimum runtime/session status subscription set named by the proposal, and MCP `resources/list` still returns an empty array instead of the named domain resources from §8.1.

### REQ-005 Daemon projections replicate run/stage/approval/artifact state as verifiable shadow truth
- Proposal Source: Outcome; §5.2 Projection engine; §6.1-§6.3; §8.2; AC-3
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/db/migrations/002_projections.sql:52-120`
  - `control-plane/crates/db/src/repos/projections.rs:7-137`
  - `control-plane/crates/engine/src/executor.rs:188-223`
- Gap / Note: The projection layer is no longer missing. However, the proposal explicitly requires projections to be verifiable through a parity comparison tool, and no executed verifier or behavioral diff was found. GraphQL also still queries canonical repos directly for core reads instead of consuming the projection tables.

### REQ-006 Approval and retry commands are processed through the daemon service layer under parity validation
- Proposal Source: §5.2 Command handlers; §7.2; AC-4
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:77-220`
  - `control-plane/crates/graphql-server/src/schema.rs:141-233`
  - `control-plane/crates/mcp-server/src/server.rs:150-170`
- Gap / Note: Command handling is real, but the acceptance criterion is broader than mechanics. No parity-validation evidence was found proving that daemon-side approval and retry semantics match the current app-owned path.

### REQ-007 The client remains the canonical owner during parity and no thin-client cutover occurs in P027
- Proposal Source: §1; §3.4; §8.2; §11
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift:476-575`
  - `Chainworks Forge/Views/RunsHomeView.swift:9-17`
  - `Chainworks Forge/Views/IdeaListView.swift:5-13`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift:21-47`
  - `rg -n "GraphQL|graphql" 'Chainworks Forge' 'Chainworks ForgeTests' --glob '!**/Proposal033Tests.swift' --glob '!**/ProviderPlatformTests.swift'` -> no app-side control-plane client path found
- Gap / Note: The app is still correctly app-owned for parity phase.

### REQ-008 The system survives restart with predictable local repair
- Proposal Source: §5.2 Recovery and repair layer; §7.3; AC-6
- Status: Partially Implemented
- Evidence Type: `code`, `runtime`
- Evidence:
  - `control-plane/crates/engine/src/recovery.rs:26-115`
  - `control-plane/crates/daemon/src/main.rs:79-87`
  - Runtime smoke: daemon booted successfully and created a local database before shutdown
- Gap / Note: Recovery code exists and runs on startup, but there is still no executed restart-repair proof for an interrupted real workflow slice. The current evidence closes the boot failure from `R1`, not the full restart contract.

### REQ-009 The daemon can own workflow decisions end-to-end in a validated parity context while client transfer stays deferred
- Proposal Source: Outcome; §8.2; §9.3; AC-7
- Status: Missing
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:123-199`
  - `control-plane/crates/acp/src/manager.rs:21-63`
  - `control-plane/crates/acp/src/adapters/claude.rs:11-52`
  - `control-plane/crates/acp/src/adapters/gemini.rs:11-52`
- Gap / Note: The daemon now owns more of the workflow loop than it did in `R1`, but the ACP execution layer is still explicitly stubbed. A validated parity context requires real runtime-backed execution and proof that the daemon's decisions match the live app-owned semantics.

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
- Status: Missing
- Evidence Type: `tests-run`, `code`, `inference`
- Evidence:
  - `./scripts/test-gate.sh proposal-027` -> passes, but only runs low-level `cargo test --workspace`
  - `rg -n "parity|shadow truth|behavioral diff|golden|daemon" 'Chainworks ForgeTests' control-plane -g '*.rs'` -> no executed parity harness found
  - `control-plane/crates/db/tests/integration.rs:12-93`
  - `control-plane/crates/engine/src/domain_engine.rs:117-188`
- Gap / Note: The repo-owned gate now exists, but it does not yet prove non-regression against the app-owned baseline. No parity comparison tool, golden-run diff, or daemon-vs-client semantic proof was found.

## Architecture Review

**Summary:** At Risk

### ARCH-001 ACP execution is still semantically stubbed
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-003, REQ-009
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/acp/src/manager.rs:21-63`
  - `control-plane/crates/acp/src/adapters/claude.rs:11-52`
  - `control-plane/crates/acp/src/adapters/gemini.rs:11-52`
  - `control-plane/crates/engine/src/executor.rs:123-199`
- Why It Matters: The daemon now wires execution through `AcpRuntimeManager`, but the registered adapters still log and return mock success. That is enough for plumbing proof, not enough for a server-side parity replica that is supposed to validate current product semantics before cutover.
- Recommended Action: Replace the stub ACP adapters with real runtime-backed adapters for one bounded slice and prove that the resulting settlement/artifact path matches the app-owned baseline.

### ARCH-002 GraphQL still bypasses the daemon's projection layer
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-004, REQ-005
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:41-98`
  - `control-plane/crates/db/src/repos/projections.rs:7-137`
  - `control-plane/crates/db/migrations/002_projections.sql:52-120`
- Why It Matters: The proposal's parity phase wants verifiable shadow truth. If GraphQL continues reading canonical tables directly rather than the projection/read-model layer, projection drift cannot be observed in the way the proposal intends.
- Recommended Action: Move northbound read paths that are meant to validate shadow truth onto the projection tables and add rebuild/consistency tests around that contract.

## Product Review

**Summary:** At Risk

### PROD-001 The daemon is now bootable, but not yet "ready for parity checking, shadow execution, and later cutover"
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-005, REQ-009, REQ-011
- Evidence Type: `runtime`, `tests-run`, `code`
- Evidence:
  - Runtime smoke: `ALIVE=True`, `CONNECTED=True`, `DB_EXISTS=True`
  - `./scripts/test-gate.sh proposal-027` -> passed
  - `control-plane/crates/acp/src/adapters/claude.rs:11-52`
  - `control-plane/crates/acp/src/adapters/gemini.rs:11-52`
- Why It Matters: The proposal outcome is stronger than "daemon compiles and starts." It says the daemon is ready to be validated as a parity replica. That is not yet true while runtime execution is mocked and parity-diff proof is absent.
- Recommended Action: Promote the next milestone from infrastructure bring-up to validated bounded-slice parity: one real runtime-backed slice, one daemon-vs-app comparison path, one restart-repair proof.

## UI Review

**Summary:** Acceptable

No live proposal-specific UI finding. P027 intentionally defers thin-client cutover, and the current app still stays on the app-owned SwiftData/ExecutionService path consistent with that contract.

## UX Review

**Summary:** Acceptable

No live proposal-specific UX finding beyond the missing parity-validation story already captured under Product and Readiness. User-visible daemon consumption is intentionally deferred by this proposal.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The dedicated `proposal-027` gate exists now, but it is still too thin for proposal sign-off
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-001, REQ-011
- Evidence Type: `tests-run`, `code`
- Evidence:
  - `scripts/test-gate.sh:121-123`
  - `scripts/test-gate.sh:1390-1397`
  - `./scripts/test-gate.sh proposal-027` -> passed
- Why It Matters: This closes the gate-collision blocker from `R1`, but the new gate only executes `cargo test --workspace`. That proves low-level health, not the proposal's required parity behavior or non-regression semantics.
- Recommended Action: Expand `proposal-027` into a real proposal-owned proof lane: boot smoke, one bounded workflow slice, projection rebuild proof, and daemon-vs-app parity evidence.

### READY-002 The gate catalog docs are stale about what `proposal-027` means
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: REQ-011
- Evidence Type: `code`
- Evidence:
  - `scripts/test-gate.sh:1178-1181`
  - `docs/reference/test-gates.md:364-390`
- Why It Matters: The script now routes `proposal-027` to the Rust+SQLite control-plane gate and `proposal-027r` to the legacy renderer gate, but the gate reference doc still describes `proposal-027` as the renderer slice. That is a handoff and audit-readiness footgun.
- Recommended Action: Bring `docs/reference/test-gates.md` back into sync with the script so future audits and contributors use the correct proof lane.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | `control-plane` workspace tests compile and pass; app-side macOS build was not rerun because P027 does not yet cut the app over |
| Core user flow runtime-validated | Partial | Live daemon boot and GraphQL listen-path were validated, but no real bounded parity workflow slice was executed |
| Empty/loading/error states covered | Not Checked | Thin-client UI is deferred by proposal |
| Accessibility risk acceptable | Not Checked | No daemon-backed UI review required yet |
| Localization risk acceptable | Not Checked | Out of scope for the current parity replica stage |
| Critical tests executed | Partial | `./scripts/test-gate.sh proposal-027` passed; control-plane workspace ran 9 tests total |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not required for this non-success verdict |
| Privacy/permissions/entitlements reviewed | Not Checked | No new app-consumed permission surface was audited in this pass |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/027-rust-sqlite-local-control-plane-extraction.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `cargo test --workspace` in `control-plane/`
- runtime smoke: launch `cargo run --quiet` with `GRAPHQL_ADDR=127.0.0.1:51291`, probe TCP connectivity, verify DB file creation, terminate process
- `./scripts/test-gate.sh proposal-027`
- `rg -n "proposal-027|proposal-027r|PROPOSAL_027_TESTS" scripts/test-gate.sh docs/reference/test-gates.md`
- `rg -n "parity|shadow truth|behavioral diff|golden|daemon" 'Chainworks ForgeTests' control-plane -g '*.rs'`
- focused file reads in:
  - `control-plane/crates/daemon/src/main.rs`
  - `control-plane/crates/db/migrations/001_initial.sql`
  - `control-plane/crates/db/migrations/002_projections.sql`
  - `control-plane/crates/db/src/repos/projections.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/engine/src/domain_engine.rs`
  - `control-plane/crates/engine/src/orchestrator.rs`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/engine/src/recovery.rs`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/mcp-server/src/server.rs`
  - `control-plane/crates/acp/src/manager.rs`
  - `control-plane/crates/acp/src/adapters/claude.rs`
  - `control-plane/crates/acp/src/adapters/gemini.rs`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift`

## Recommended Next Actions

1. Replace the stub ACP adapters with one real runtime-backed execution slice and prove artifact/settlement truth through that path.
2. Add a bounded daemon-vs-app parity harness so `REQ-011` stops being an evidence gap.
3. Move GraphQL shadow-read validation onto the projection layer instead of direct canonical table scans.
4. Add an executed restart-repair proof for an interrupted run.
5. Sync `docs/reference/test-gates.md` with the new `proposal-027` / `proposal-027r` split.
