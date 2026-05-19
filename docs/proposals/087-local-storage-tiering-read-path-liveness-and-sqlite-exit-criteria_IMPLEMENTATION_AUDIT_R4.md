# Proposal 087 Implementation Audit R4

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Audit report | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria_IMPLEMENTATION_AUDIT_R4.md` |
| Audit timestamp | 2026-05-16T08:19:17Z |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Implementation target | Working tree on `cw/implement-proposal-087-local-s/b4edcf82` |
| Git HEAD | `569b297e58582153eb3601f91d18e7aa97d9a6f2` |
| Compare base | Implicit current worktree; staged, unstaged, and untracked implementation files inspected |
| Proposal state | Active draft; no superseding proposal found in this audit |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Audit confidence | High for build/gate blockers; Medium for runtime behavior because the target does not build cleanly |

## Implementation Target

The audited tree contains a broad staged implementation plus additional unstaged edits and untracked P087 files. Important touched surfaces include:

- Rust DB/storage: `control-plane/crates/db/src/repos/projections.rs`, `storage_health.rs`, `writer.rs`, `hot_read_guard.rs`, `metrics.rs`, `hot_read_circuit.rs`, `maintenance.rs`, `projection_invalidation.rs`
- Rust MCP/GraphQL/API: `control-plane/crates/mcp-server/src/server.rs`, `tools/storage.rs`, `tools/runs.rs`, `tools/runtime.rs`, `control-plane/crates/graphql-server/src/schema.rs`, `types/storage.rs`, `types/artifact.rs`, `types/run.rs`
- Swift read client: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`, `DaemonLifecycleClient.swift`, `Chainworks Forge/Views/RunsHomeView.swift`
- Migrations/gate/docs/evidence: `control-plane/crates/db/migrations/056-059_p087*.sql`, `scripts/test-gate.sh`, `docs/evidence/p087/api/*`, rollout-contract fixtures, reference docs

## Prior Proposal-Review Reuse

Reviewer-selection reuse: Not reused.

No prior P087 proposal-review packet was found by the skill helper or adjacent review-file discovery. Existing `IMPLEMENTATION_AUDIT` reports were ignored for reviewer selection per the audit workflow.

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `chainworks_execution_truth_reviewer` | P087 changes projection truth, MCP truth, run-list read truth, maintenance slots, and storage health readback. |
| `rust_reliability_reviewer` | Hot-read liveness, cancellation, circuit state, maintenance/reaper behavior, and startup recovery are reliability surfaces. |
| `rust_performance_reviewer` | The proposal is centered on hot read latency, `runs.list` budget, projection rebuild cost, WAL/write contention, and p95/p99 metrics. |
| `api_contract_reviewer` | GraphQL storage health, MCP tool/result contracts, capability inventory, and fixture compatibility changed. |
| `observability_rollout_reviewer` | P087 requires metrics, thresholds, rollout readback, evidence fixtures, and a canonical `proposal-087` gate. |

Rejected close alternatives:

- `rust_arch_reviewer`: relevant, but covered by the repo-local execution truth reviewer and hard-cap pressure.
- `apple_arch_reviewer`: Swift read-client code changed, but the blocking risks are server/gate/API first.
- `macos_ui_reviewer`: UI visual tokens are part of the gate, but no new proposal-mandated macOS screen or control is the primary risk.
- `product_reviewer`: no explicit product metrics or experiment checkpoint were requested.

## Proposal Contract Summary

P087 adopts a three-tier local storage model: compact canonical SQLite state, file-backed high-volume evidence/artifacts, and projection/cache-backed hot reads. It requires `runs.list`, MCP liveness tools, GraphQL live/read surfaces, storage health metrics, and exit criteria gates to prevent SQLite from becoming a read/write choke point. It explicitly does not replace SQLite, add RocksDB/Postgres, undo DbWriter, alter durable side-effect semantics, add UI write controls, or implement compaction itself.

Platform/product scope:

- Apple: macOS SwiftUI read-client consumption only; no new UI write controls.
- Backend/service: Rust daemon, SQLite persistence, GraphQL, MCP, storage health, maintenance, projections, and rollout gates.
- Cross-stack scope: macOS UI reads daemon-owned projections via GraphQL; MCP clients consume bounded read tools and diagnostics.

Primary implementation flows:

1. MCP liveness sequence: initialize, tools/list, `runs.list`, `runtime.health`, `storage.health`, and a resource/artifact metadata read return within budgets even under degraded writer or maintenance state.
2. Runs home and `runs.list` read from `ActiveRunIndex`/run projections without filesystem scans, N+1 attachments, transcript reads, or detail enrichment.
3. Storage health exposes writer, WAL, spool, projection lag/freshness, hot-read guard, and exit-threshold metrics through GraphQL and MCP.
4. Projection rebuild/invalidation survives restart and compact state changes without blocking read liveness.
5. Large evidence/artifact payloads remain file-spooled while SQLite stores bounded metadata and compact read models.

## Fidelity Inventory

Matches:

- New projection, hot-read circuit, maintenance, and freshness tables are introduced in P087 migration files.
- `projections.rs` adds run-list compact JSON fields, artifact noise summary, runtime health summary, and projection rebuild metrics.
- MCP/GraphQL storage health surfaces expose P087 diagnostics such as projection freshness, hot-read guards, rollout fields, and evaluated metrics.
- `scripts/test-gate.sh` includes a `proposal-087|p087` gate with migration-version checks, focused cargo tests, UI/schema/evidence checks, and fixture checks.
- Evidence fixtures exist under `docs/evidence/p087/api/` and rollout-contract paths.

Divergences:

- The canonical P087 gate fails immediately because two DB migration files share version `056`.
- The DB test target does not compile after the new cancellation-aware writer API; tests still pass `make_work(...)` into `DbWriter::submit`, and a `CoalescedEntry` test initializer lacks `cancellation_token`.
- The MCP server target does not compile because `tools::runtime::execute_with_name` is referenced but not implemented, `db::repos::storage_health::reset_read_path_metrics_for_tests` is referenced but not implemented, and a storage test calls `execute_with_writer` without the new cancellation token.
- The focused GraphQL storage-health test passes, but the broader P087 backend gate cannot reach runtime validation.

Ambiguities / Evidence Gaps:

- No live daemon runtime, UI runtime, or MCP end-to-end liveness proof could be obtained because the canonical gate and server targets are red.
- No same-tree full/proposal gate passed; readiness must fail closed.
- The proposal status is still Draft, but the implementation branch is broad enough to audit as an active implementation slice.

## Requirement Summary

| ID | Requirement | Status | Evidence |
|---|---|---|---|
| REQ-001 | SQLite remains compact canonical state and metadata; no high-volume event streams | Partially Implemented | Migrations and code add compact projection/cursor/maintenance tables, but duplicate migration version makes the DB evolution invalid. |
| REQ-002 | High-volume evidence and artifact payloads remain file-spooled with SQLite metadata only | Partially Implemented | Existing evidence spool and artifact pointer contracts remain; new fixtures assert redacted metadata pointers, but gates do not pass. |
| REQ-003 | `runs.list` is projection-only and meets read budget | Partially Implemented | `list_active_projection` reads `run_summaries` only and tests attempt a 500 ms budget, but MCP target does not compile and the gate cannot run the budget proof. |
| REQ-004 | MCP liveness gate passes for initialize/tools/list/runs.list/runtime.health/storage.health/resource read under degraded/maintenance conditions | Partially Implemented | Hot-read guard and tests exist, but `mcp-server` fails to compile, so the liveness behavior is not proven. |
| REQ-005 | GraphQL hot reads/subscriptions use projections/cache, not raw evidence scans | Partially Implemented | GraphQL runs list uses projection functions and `storage_health_v1` passes, but the broader backend stack is not build-clean. |
| REQ-006 | Storage health exposes writer pressure, WAL, spool, projection lag/freshness, and read-path metrics | Partially Implemented | `storage_health.rs` and GraphQL types expose many fields; DB/MCP test targets fail before the readback can be validated end to end. |
| REQ-007 | Storage exit criteria are documented and enforced by `proposal-087` gate | Partially Implemented | The gate exists and includes threshold/evidence checks, but it fails at migration-version preflight and cannot validate the implementation. |
| REQ-008 | Durable side-effect ledger, P038, and P086 can rely on storage tiering without increasing SQLite pressure | Not Verifiable | Runtime health includes unresolved side-effect and continuation counts, and artifact noise exists, but no successful integration proof is available. |

## Detailed Requirement Audit

### REQ-001 - Compact SQLite Ownership

Proposal source: sections 3.1, 5.1, 5.2, 18.

Status: Partially Implemented.

Evidence types: proposal, code, migration, tests-run.

Evidence references:

- `control-plane/crates/db/migrations/056_p087_storage_tiering_projections.sql` creates `projection_invalidation_log`, `projection_cursors`, `maintenance_operations`, `storage_health_snapshots`, and `hot_read_circuit_states`.
- `control-plane/crates/db/migrations/058_p087_projection_refinement.sql` creates `artifact_noise_summary` and `runtime_health_summary`.
- `./scripts/test-gate.sh proposal-087` fails on duplicate migration version `056`.

Gap / note: The schema shape is aligned with compact metadata/projection storage, but the migration stream is invalid because both `056_p087_run_summary_hot_read_payloads.sql` and `056_p087_storage_tiering_projections.sql` exist.

### REQ-002 - File-Backed Evidence and Artifact Store

Proposal source: sections 3.2, 5.3, 10, 11, 12, 18.

Status: Partially Implemented.

Evidence types: code, tests-found, config.

Evidence references:

- `control-plane/crates/mcp-server/src/server.rs` returns `artifact_metadata_pointer` for artifact resources instead of raw file paths.
- `docs/evidence/p087/api/artifact-metadata-pointer-v1.fixture.json` exists and asserts redacted pointer shape.
- `scripts/test-gate.sh` checks the artifact metadata pointer fixture and rejects MCP resource file-path leakage.

Gap / note: The contract is present in code and fixture form, but no passing gate or runtime proof confirms it across GraphQL/MCP because P087 gates are blocked.

### REQ-003 - Projection-Only `runs.list`

Proposal source: sections 6.1, 7.1, 16 phase 2, 17.1, 18.

Status: Partially Implemented.

Evidence types: code, tests-found, tests-run.

Evidence references:

- `control-plane/crates/db/src/repos/projections.rs` `list_active_projection` selects from `run_summaries` and no longer performs detail readbacks.
- `control-plane/crates/mcp-server/src/server.rs` records `runs.list` hot-read latency and wraps hot read tools in a timeout/circuit path.
- `control-plane/crates/mcp-server/src/server.rs` has `proposal_087_runs_list_seeded_load_stays_under_500ms_and_records_p95`, but the `mcp-server` target does not compile.

Gap / note: The implementation is shaped toward projection-only reads, but the budget proof is unavailable and the server cannot build.

### REQ-004 - MCP Liveness Gate

Proposal source: sections 6.2, 13, 16 phase 1/2, 17.1, 18.

Status: Partially Implemented.

Evidence types: code, tests-found, tests-run.

Evidence references:

- `control-plane/crates/db/src/hot_read_guard.rs` implements observe/enforce modes and hot-read circuit admission.
- `control-plane/crates/mcp-server/src/server.rs` wraps `tools/list`, `resources/read`, and hot read tools with circuit checks and timeouts.
- `cd control-plane && cargo test -p mcp-server proposal_087 -- --nocapture` fails to compile because `execute_with_name` and test helper references are missing/stale.

Gap / note: The liveness behavior is not executable in the audited tree.

### REQ-005 - GraphQL Projection Hot Reads

Proposal source: sections 6.3, 7, 8, 18.

Status: Partially Implemented.

Evidence types: code, tests-run.

Evidence references:

- `control-plane/crates/graphql-server/src/schema.rs` uses `projections::list_by_idea_projection` and `projections::list_active_projection` for the `runs` query.
- `control-plane/crates/graphql-server/src/schema.rs` wraps `storage_health` in a hot-read guard and timeout.
- `cd control-plane && cargo test -p graphql-server --lib storage_health_v1 -- --nocapture` passed 1 test.

Gap / note: The local GraphQL storage type test passes, but this is only a narrow slice. Full proposal behavior remains blocked by migration and Rust target failures.

### REQ-006 - Storage Health and Metrics

Proposal source: sections 7.4, 14, 15, 18.

Status: Partially Implemented.

Evidence types: code, tests-found, tests-run.

Evidence references:

- `control-plane/crates/db/src/repos/storage_health.rs` returns writer, WAL, projections, projection freshness, hot-read guards, read-path metrics, evidence spool, maintenance operations, rollout, thresholds, and evaluated P087 metrics.
- `control-plane/crates/db/src/metrics.rs` declares P087 metrics including `runs_list_read_latency_ms` and `mcp_liveness_gate_duration_ms`.
- `cd control-plane && cargo test -p db proposal_087 -- --nocapture` fails to compile in DB tests after the writer API change.

Gap / note: The metrics/readback design is present, but not proven in a build-clean backend.

### REQ-007 - Exit Criteria Gate

Proposal source: sections 15, 16 phase 4/5, 17, 18.

Status: Partially Implemented.

Evidence types: code, tests-run, config.

Evidence references:

- `scripts/test-gate.sh` adds `proposal-087|p087`, duplicate migration detection, P087 cargo tests, schema/static checks, fixture checks, rollout fixture checks, and metrics declaration checks.
- `./scripts/test-gate.sh proposal-087` fails with `FAILED: duplicate DB migration version 056: 056_p087_storage_tiering_projections.sql, 056_p087_run_summary_hot_read_payloads.sql`.

Gap / note: The gate exists, but it is red on the audited tree and therefore does not provide acceptance evidence.

### REQ-008 - P078/P038/P086 Reliance Without Extra SQLite Pressure

Proposal source: sections 10, 11, 12, 18.

Status: Not Verifiable.

Evidence types: code, inference, tests-found.

Evidence references:

- `control-plane/crates/db/src/repos/projections.rs` rebuilds `runtime_health_summary` with side-effect unresolved and continuation active counts.
- `control-plane/crates/db/src/repos/projections.rs` rebuilds `artifact_noise_summary` for compaction readiness.
- `control-plane/crates/db/src/repos/storage_health.rs` exposes `artifactNoiseProjection` and runtime health projection fields.

Gap / note: These projections exist in code, but no successful integration or proposal gate proves the dependent P078/P038/P086 flows can rely on them.

## Reviewer / Lens Scorecard

| Lens | Conformance | Top risk | Confidence |
|---|---|---|---|
| Chainworks execution truth | Partial | Projection and MCP truth cannot be accepted while migrations/gates are red. | High |
| Rust reliability | Not Ready | Cancellation-aware API changes left tests and MCP wiring stale; liveness cannot be executed. | High |
| Rust performance | Not Ready | `runs.list` budget tests exist but do not compile/run. | High |
| API contract | Not Ready | MCP runtime/storage APIs reference missing or stale functions. | High |
| Observability/rollout | Not Ready | `proposal-087` gate fails before backend validation. | High |

## Routed Specialist Findings

### READY-001 - Duplicate DB Migration Version Blocks the Canonical P087 Gate

Reviewer: `observability_rollout_reviewer`

Severity: Critical

Confidence: High

Related proposal items: REQ-001, REQ-007

Evidence types: migration, tests-run

Evidence references:

- `control-plane/crates/db/migrations/056_p087_run_summary_hot_read_payloads.sql`
- `control-plane/crates/db/migrations/056_p087_storage_tiering_projections.sql`
- `scripts/test-gate.sh` lines 7783-7798
- `./scripts/test-gate.sh proposal-087`

Why it matters: The repository's canonical P087 gate fails before any backend tests execute, and migration ordering is a release boundary for persisted storage changes.

Recommended action: Renumber the new migration sequence so every migration version is unique and ensure any overlapping `ALTER TABLE run_summaries ADD COLUMN ...` statements are consolidated or guarded for already-applied schemas.

Acceptance criteria: `./scripts/test-gate.sh proposal-087` passes the migration-version preflight and reaches the focused backend tests.

### REL-001 - Cancellation-Aware Writer API Changes Leave DB Tests Non-Compiling

Reviewer: `rust_reliability_reviewer`

Severity: Critical

Confidence: High

Related proposal items: REQ-004, REQ-006, REQ-007

Evidence types: code, tests-run

Evidence references:

- `control-plane/crates/db/src/repos/storage_health.rs` lines 1169 and 1251
- `control-plane/crates/db/src/writer.rs` lines 2315-2324
- `cd control-plane && cargo test -p db proposal_087 -- --nocapture`

Why it matters: P087 relies on cancellation and liveness behavior. The audited DB test target cannot compile because tests still pass a boxed cancellable work item into `DbWriter::submit`, whose public signature expects a one-argument closure, and a coalescing-buffer test constructs `CoalescedEntry` without `cancellation_token`.

Recommended action: Align tests and helper APIs with the final cancellation model. Either keep `DbWriter::submit` accepting ordinary closures and update tests to pass ordinary closures directly, or expose a separate cancellable submit helper and update all callers consistently. Add `cancellation_token` to direct `CoalescedEntry` test construction.

Acceptance criteria: `cd control-plane && cargo test -p db proposal_087 -- --nocapture` compiles and runs the filtered tests.

### API-001 - MCP P087 Runtime/Storage Contract Does Not Compile

Reviewer: `api_contract_reviewer`

Severity: Critical

Confidence: High

Related proposal items: REQ-003, REQ-004, REQ-006

Evidence types: code, tests-run

Evidence references:

- `control-plane/crates/mcp-server/src/server.rs` line 964 references `tools::runtime::execute_with_name`.
- `control-plane/crates/mcp-server/src/tools/runtime.rs` exposes `execute`, not `execute_with_name`.
- `control-plane/crates/mcp-server/src/server.rs` lines 2865 and 2920 reference `db::repos::storage_health::reset_read_path_metrics_for_tests`.
- `control-plane/crates/mcp-server/src/tools/storage.rs` line 650 calls `execute_with_writer` without the new cancellation token.
- `cd control-plane && cargo test -p mcp-server proposal_087 -- --nocapture`

Why it matters: MCP is one of P087's primary read-path liveness surfaces. Missing/stale function references mean the MCP liveness gate and read-tool contract are not executable.

Recommended action: Add or remove `execute_with_name` consistently, restore or replace read-path metric reset helpers, and update all `execute_with_writer` call sites for the cancellation-token signature.

Acceptance criteria: `cd control-plane && cargo test -p mcp-server proposal_087 -- --nocapture` compiles and runs the filtered tests, and `runtime.health`, `storage.health`, and `runs.list` return typed payloads through the production tool path.

### OPS-001 - Gate Evidence Exists but Is Not Acceptance Evidence

Reviewer: `observability_rollout_reviewer`

Severity: Major

Confidence: High

Related proposal items: REQ-006, REQ-007

Evidence types: config, tests-run

Evidence references:

- `docs/evidence/p087/api/*`
- `docs/evidence/rollout-contract/operator-readback/p087-storage-tiering-full-surface.fixture.json`
- `scripts/test-gate.sh` lines 7882-8000
- `./scripts/test-gate.sh proposal-087`

Why it matters: The fixture and rollout-readback artifacts are useful only after the gate can validate them on the same tree. Right now the gate exits before checking them, so they cannot be treated as acceptance proof.

Recommended action: After fixing migration and compile blockers, rerun `proposal-087` and only then treat the fixtures as current evidence.

Acceptance criteria: `./scripts/test-gate.sh proposal-087` completes all backend, UI/schema, fixture, rollout, and negative-fixture checks successfully on the audited HEAD/worktree.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Build or canonical gate status | Failed | `./scripts/test-gate.sh proposal-087` fails on duplicate migration version `056`. |
| Core MCP liveness validation | Failed | `mcp-server` P087 target does not compile. |
| GraphQL storage health validation | Partial | `graphql-server --lib storage_health_v1` passed 1 test. |
| Runtime/live daemon validation | Not run | Blocked by gate/build failures. |
| UI/UX runtime, empty/loading/error/offline/permission states | Not run | Not practical because backend gate is red. |
| Accessibility/localization/privacy/permissions/entitlements | Not assessed beyond code/static scope | No UI runtime proof collected. |
| Critical tests executed | Failed/partial | P087 gate failed; DB and MCP focused tests failed to compile; GraphQL storage-health focused test passed. |
| Full regression or canonical full/proposal gate passed | Failed | No same-tree successful full or proposal gate evidence exists. |

## Verification Log

| Command | Result | Notes |
|---|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...087...md` | Pass | Selected R4 report path. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...087...md` | Pass | No prior proposal-review artifacts found. |
| `./scripts/test-gate.sh proposal-087` | Fail | Immediate failure: duplicate DB migration version `056`. |
| `cd control-plane && cargo test -p db proposal_087 -- --nocapture` | Fail | DB lib test target fails to compile due writer/cancellation API mismatches in tests. |
| `cd control-plane && cargo test -p mcp-server proposal_087 -- --nocapture` | Fail | MCP target fails to compile due missing/stale runtime/storage helpers and cancellation-token call-site mismatch. |
| `cd control-plane && cargo test -p graphql-server --lib storage_health_v1 -- --nocapture` | Pass | 1 storage-health preservation test passed. |

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

Highest-risk blockers:

1. Duplicate migration version `056` blocks the canonical `proposal-087` gate before any backend validation.
2. DB P087 tests do not compile after the cancellation-aware writer changes.
3. MCP P087 tests do not compile due missing/stale runtime and storage helper APIs.
4. No same-tree full/proposal gate or live runtime evidence exists.

Recommended next actions:

1. Fix migration numbering and duplicate `run_summaries` column additions.
2. Make the cancellation-token API internally consistent across DB writer, DB tests, MCP storage tools, and test helpers.
3. Repair MCP runtime/storage function references and rerun `cargo test -p db proposal_087`, `cargo test -p mcp-server proposal_087`, and `./scripts/test-gate.sh proposal-087`.
4. Treat GraphQL/MCP/UI fixture evidence as acceptance evidence only after the canonical P087 gate passes on the same worktree.
