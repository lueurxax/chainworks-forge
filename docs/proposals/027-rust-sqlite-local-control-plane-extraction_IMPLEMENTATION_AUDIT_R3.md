# Proposal 027: Rust + SQLite Local Control Plane Extraction Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-rust-sqlite-local-control-plane-extraction.md` |
| Repository Root | `.` |
| Git SHA | `d3c5e22` |
| Working Tree | dirty (large pre-existing delta, including control-plane workspace and many unrelated app-side edits) |
| Audited At | `2026-04-12T01:36:47+0300` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

This `R3` pass supersedes `R2`. The basis has improved again on the same tree: the dedicated `proposal-027` gate now runs a richer control-plane slice with projection-parity and startup-repair integration tests, and the GraphQL layer now partially reads through projection tables for run and stage lists. That closes part of the earlier "too thin to trust" concern.

The verdict still does not roll to success. The remaining blockers are proposal-owned: ACP execution is still stubbed behind mock adapters, the northbound read layer is still split between projection tables and direct canonical-table reads, MCP resources are still absent, and the new parity tests still stop short of proving non-regression for the broader app-owned run/report/recovery semantics. `P027` is clearly progressing, but it is not yet the validated parity replica the proposal defines as done.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Validated end-to-end parity context is still missing | High |
| Architecture | At Risk | Runtime execution remains stubbed and northbound reads still mix projection and canonical truth lanes | High |
| Product | At Risk | New parity tests are useful but still too narrow to prove the proposal's non-regression bar | High |
| UI | Acceptable | The app correctly remains on the app-owned path during parity | High |
| UX | Acceptable | User-visible daemon consumption is still intentionally deferred | High |
| Readiness | Not Ready | The dedicated gate is now real, but it still does not prove a real runtime-backed parity slice | High |

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
- Projection-related tables and rebuild helpers now exist and are partially exercised by tests.
- The current Swift app remains app-owned through `ExecutionService`, `RunReportBuilder`, and `WorkflowMapProjectionService`.
- The repository now contains a dedicated `proposal-027` control-plane gate with focused tests.

### Divergences

- ACP execution is still backed by stub adapters that return mock successful results.
- GraphQL only partially consumes projection tables; it still mixes in direct canonical-table reads.
- MCP `resources/list` still returns an empty array instead of the domain resource set described by the proposal.
- The repo's gate-reference documentation is still stale about what `proposal-027` means.

### Ambiguities / Evidence Gaps

- No live app walkthrough against daemon-backed read models exists, but that is deferred by proposal and therefore not treated as a conformance gap by itself.
- No executed daemon-vs-app behavioral diff or report-parity proof was found.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partially Implemented | 6 |
| Missing | 1 |
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
- Gap / Note: No live boot blocker remains on this tree.

### REQ-002 SQLite is the durable local source of truth for at least one real workflow slice
- Proposal Source: Outcome; §3.2; §6.1-§6.5; AC-2
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/db/migrations/001_initial.sql:2-123`
  - `control-plane/crates/db/migrations/002_projections.sql:1-120`
  - `control-plane/crates/db/tests/integration.rs:13-228`
  - Runtime smoke created `control-plane/chainworks-control-plane.db`
- Gap / Note: Durable SQLite truth is now better covered than in `R2`, but the executed proof still stops at repository/projection behavior and boot smoke. It does not yet prove one bounded real workflow slice end-to-end through a real runtime-backed execution path.

### REQ-003 A product-owned orchestration engine exists with explicit transitions, command handling, background work, and projection refresh
- Proposal Source: §3.3; §5.2; §7.1-§7.2; AC-1
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/domain_engine.rs:21-188`
  - `control-plane/crates/engine/src/orchestrator.rs:22-138`
  - `control-plane/crates/engine/src/command_handler.rs:33-274`
  - `control-plane/crates/engine/src/executor.rs:83-229`
  - `./scripts/test-gate.sh proposal-027` -> executed 13 tests across `db` / `domain` / `engine`
- Gap / Note: The engine now has broader proof than in `R2`, including startup-repair and projection-parity tests. It remains partial because the runtime execution underneath still routes through stub adapters, so the full workflow loop is not yet a validated parity runtime.

### REQ-004 The daemon preserves the northbound GraphQL + MCP boundary shape
- Proposal Source: §4; §5.1; §8.1
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:39-95`
  - `control-plane/crates/mcp-server/src/server.rs:134-136`
  - `control-plane/crates/daemon/src/main.rs:89-115`
  - Runtime smoke confirmed GraphQL was listening on loopback
- Gap / Note: The shape is materially present, but not complete. GraphQL now uses projections for `runs` and `stages`, yet `run`, `approval_inbox`, and `artifacts` still read canonical repos directly. MCP resources remain empty, and the proposal's runtime/session subscription bar is still not met.

### REQ-005 Daemon projections replicate run/stage/approval/artifact state as verifiable shadow truth
- Proposal Source: Outcome; §5.2 Projection engine; §6.1-§6.3; §8.2; AC-3
- Status: Partially Implemented
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/db/migrations/002_projections.sql:52-120`
  - `control-plane/crates/db/src/repos/projections.rs:7-137`
  - `control-plane/crates/db/tests/integration.rs:96-228`
  - `control-plane/crates/graphql-server/src/schema.rs:59-95`
- Gap / Note: This is stronger than `R2`. There is now direct parity-style proof for run/stage projections after rebuild, and GraphQL consumes projections for run/stage list surfaces. It remains partial because approval/artifact/recovery projection parity is not fully proved, and no broader daemon-vs-app comparison tool was found.

### REQ-006 Approval and retry commands are processed through the daemon service layer under parity validation
- Proposal Source: §5.2 Command handlers; §7.2; AC-4
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:77-220`
  - `control-plane/crates/graphql-server/src/schema.rs:101-233`
  - `control-plane/crates/mcp-server/src/server.rs:150-170`
- Gap / Note: Command handling is real, but the acceptance criterion includes parity validation. No executed proof was found showing daemon-side approval/retry semantics match the current app-owned path under a real runtime-backed slice.

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
- Status: Implemented
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `control-plane/crates/engine/src/recovery.rs:26-115`
  - `control-plane/crates/engine/tests/integration.rs:59-132`
  - `control-plane/crates/daemon/src/main.rs:79-87`
  - Runtime smoke confirmed successful daemon startup on the audited tree
- Gap / Note: `R3` closes the old uncertainty here. The recovery service is directly tested for stuck-running repair semantics and clean-run no-op behavior, which is enough to mark the startup-repair contract implemented.

### REQ-009 The daemon can own workflow decisions end-to-end in a validated parity context while client transfer stays deferred
- Proposal Source: Outcome; §8.2; §9.3; AC-7
- Status: Missing
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `control-plane/crates/engine/src/executor.rs:123-199`
  - `control-plane/crates/acp/src/manager.rs:21-63`
  - `control-plane/crates/acp/src/adapters/claude.rs:11-52`
  - `control-plane/crates/acp/src/adapters/gemini.rs:11-52`
  - `./scripts/test-gate.sh proposal-027` -> no real runtime-backed proof, only stub-backed tests
- Gap / Note: This remains the hard blocker. The daemon now owns more of the workflow loop than in earlier passes, but the ACP execution layer is still explicitly stubbed and therefore cannot establish the validated parity context required by the proposal.

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
  - `control-plane/crates/db/tests/integration.rs:96-228` (`test_projection_parity_after_rebuild`, `test_projection_list_before_rebuild_returns_run_with_zero_counts`)
  - `control-plane/crates/engine/tests/integration.rs:59-132` (`test_startup_repair_clears_stuck_running_stage`, `test_startup_repair_skips_clean_runs`)
- Gap / Note: This is improved from `R2`. The current tree now has direct parity-style proof for projections and startup repair. It is still partial because no report-parity or daemon-vs-app behavioral diff was found, so the broader non-regression bar remains unproven.

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
- Why It Matters: The daemon now wires execution through `AcpRuntimeManager`, but the registered adapters still log and return mock success. That is enough for plumbing proof, not enough for the server-side parity replica the proposal says must be validated before cutover.
- Recommended Action: Replace the stub ACP adapters with one real runtime-backed slice and use that slice as the first end-to-end parity proof lane.

### ARCH-002 Northbound reads still mix projection truth and canonical-table truth
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-004, REQ-005
- Evidence Type: `code`
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:59-95`
  - `control-plane/crates/db/src/repos/projections.rs:7-137`
  - `control-plane/crates/mcp-server/src/server.rs:134-136`
- Why It Matters: `R3` closes the older claim that GraphQL ignored projections entirely, but a mixed model is still risky. If some northbound reads validate shadow projections and others bypass them, projection drift becomes easy to miss and parity semantics become harder to reason about.
- Recommended Action: Make the intended shadow-read contract explicit per surface and move the relevant northbound readers fully onto projections where parity validation is the goal.

## Product Review

**Summary:** At Risk

### PROD-001 The new parity tests are meaningful, but they still stop short of the proposal’s non-regression bar
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-005, REQ-009, REQ-011
- Evidence Type: `tests-run`, `code`
- Evidence:
  - `./scripts/test-gate.sh proposal-027` -> passed with new projection/recovery integration coverage
  - `control-plane/crates/db/tests/integration.rs:96-228`
  - `control-plane/crates/engine/tests/integration.rs:59-132`
  - `control-plane/crates/acp/src/adapters/claude.rs:11-52`
- Why It Matters: The proposal outcome is no longer just "infrastructure exists." It is "the daemon is ready to be validated as a parity replica." The new tests prove useful sub-slices, but they do not yet show that real runtime-backed execution preserves the current app-owned semantics.
- Recommended Action: Expand the current parity harness into one bounded end-to-end slice that includes real ACP execution, settlement, projections, and a daemon-vs-app comparison result.

## UI Review

**Summary:** Acceptable

No live proposal-specific UI finding. P027 intentionally defers thin-client cutover, and the current app still stays on the app-owned SwiftData/ExecutionService path consistent with that contract.

## UX Review

**Summary:** Acceptable

No live proposal-specific UX finding beyond the missing full parity-validation story already captured under Product and Readiness. User-visible daemon consumption is intentionally deferred by this proposal.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The dedicated `proposal-027` gate is now materially useful, but still not sign-off grade
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-001, REQ-011
- Evidence Type: `tests-run`, `code`
- Evidence:
  - `scripts/test-gate.sh:121-123`
  - `scripts/test-gate.sh:1390-1397`
  - `control-plane/crates/db/tests/integration.rs:96-228`
  - `control-plane/crates/engine/tests/integration.rs:59-132`
  - `./scripts/test-gate.sh proposal-027` -> passed
- Why It Matters: This closes part of the `R2` readiness gap. The gate is now more than a compile sweep. It still is not enough for proposal sign-off because it does not prove a real runtime-backed parity slice or broad non-regression.
- Recommended Action: Keep the current gate, but add one higher-fidelity proof layer on top of it: runtime-backed execution plus daemon-vs-app comparison for one bounded workflow slice.

### READY-002 Gate-reference docs are still stale about what `proposal-027` means
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: REQ-011
- Evidence Type: `code`
- Evidence:
  - `scripts/test-gate.sh:1178-1181`
  - `docs/reference/test-gates.md:364-390`
- Why It Matters: The script now routes `proposal-027` to the Rust+SQLite control-plane gate, but the reference doc still describes `proposal-027` as the artifact renderer slice. That creates avoidable audit and handoff confusion.
- Recommended Action: Sync `docs/reference/test-gates.md` with the script and preserve the renderer slice under its `proposal-027r` name there as well.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | `control-plane` workspace tests compile and pass; app-side macOS build was not rerun because P027 does not yet cut the app over |
| Core user flow runtime-validated | Partial | Live daemon boot and GraphQL listen-path were validated, but no real runtime-backed parity workflow slice was executed |
| Empty/loading/error states covered | Not Checked | Thin-client UI is deferred by proposal |
| Accessibility risk acceptable | Not Checked | No daemon-backed UI review required yet |
| Localization risk acceptable | Not Checked | Out of scope for the current parity replica stage |
| Critical tests executed | Partial | `proposal-027` gate now passes with 13 tests including projection/recovery integration coverage |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not required for this non-success verdict |
| Privacy/permissions/entitlements reviewed | Not Checked | No new app-consumed permission surface was audited in this pass |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/027-rust-sqlite-local-control-plane-extraction.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `./scripts/test-gate.sh proposal-027`
- runtime smoke: launch `cargo run --quiet` with `GRAPHQL_ADDR=127.0.0.1:51291`, probe TCP connectivity, verify DB file creation, terminate process
- `rg -n "Stub adapter|stub execute|mock result|proposal-027r|Artifact content rendering gate|parity|behavioral diff|shadow truth" control-plane docs/reference/test-gates.md 'Chainworks ForgeTests' -g '*.rs' -g '*.md'`
- focused file reads in:
  - `control-plane/crates/daemon/src/main.rs`
  - `control-plane/crates/db/migrations/001_initial.sql`
  - `control-plane/crates/db/migrations/002_projections.sql`
  - `control-plane/crates/db/src/repos/projections.rs`
  - `control-plane/crates/db/tests/integration.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/engine/src/domain_engine.rs`
  - `control-plane/crates/engine/src/orchestrator.rs`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/engine/src/recovery.rs`
  - `control-plane/crates/engine/tests/integration.rs`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/mcp-server/src/server.rs`
  - `control-plane/crates/acp/src/manager.rs`
  - `control-plane/crates/acp/src/adapters/claude.rs`
  - `control-plane/crates/acp/src/adapters/gemini.rs`
  - `docs/reference/test-gates.md`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift`

## Recommended Next Actions

1. Replace the stub ACP adapters with one real runtime-backed execution slice and prove artifact/settlement truth through that path.
2. Add a daemon-vs-app parity comparison for one bounded workflow slice so `REQ-009` and `REQ-011` can move out of red.
3. Finish the northbound shadow-read contract: decide which GraphQL reads must consume projections and remove the mixed truth lane.
4. Add parity proof for report/recovery outputs, not just stage/run projections and startup repair.
5. Sync `docs/reference/test-gates.md` with the current `proposal-027` / `proposal-027r` split.
