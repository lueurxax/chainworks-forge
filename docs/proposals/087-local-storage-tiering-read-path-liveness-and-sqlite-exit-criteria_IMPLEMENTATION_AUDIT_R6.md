# Proposal 087 Implementation Audit R6

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Audit report | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria_IMPLEMENTATION_AUDIT_R6.md` |
| Audit timestamp | 2026-05-16T13:33:23Z |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Implementation target | Current dirty working tree on `cw/implement-proposal-087-local-s/b4edcf82` |
| Git HEAD | `569b297e58582153eb3601f91d18e7aa97d9a6f2` |
| Compare base | Implicit current worktree; staged, unstaged, and untracked implementation files inspected |
| Proposal state | Active draft; no superseding proposal found |
| Overall conformance | Implemented |
| Overall implementation readiness | Ready with Risks |
| Audit confidence | High for proposal-gate-backed behavior; Medium for live daemon behavior because no external daemon restart was executed |

## Implementation Target

The current working tree includes the broad P087 implementation plus follow-up fixes that directly address the R5 restart/reaper gaps:

- Startup recovery now calls `rebuild_startup_read_projections`, which delegates to `projections::rebuild_all_for_run`.
- A new engine integration test, `proposal_087_projection_cache_rebuilds_after_restart`, seeds stale P087 projections, runs startup repair, and asserts refreshed artifact-noise and runtime-health projections.
- `scripts/test-gate.sh proposal-087` now includes the engine P087 integration test filter.
- The daemon no longer starts the local periodic maintenance reaper helper and instead starts the repo-level `db::repos::maintenance::spawn_maintenance_reaper` once after the startup one-shot reaper.

Important touched surfaces remain:

- Rust DB/storage: `control-plane/crates/db/src/repos/projections.rs`, `storage_health.rs`, `writer.rs`, `hot_read_guard.rs`, `metrics.rs`, `hot_read_circuit.rs`, `maintenance.rs`, `projection_invalidation.rs`
- Rust engine/daemon: `control-plane/crates/engine/src/recovery.rs`, `control-plane/crates/engine/tests/integration.rs`, `control-plane/crates/daemon/src/main.rs`
- Rust MCP/GraphQL/API: `control-plane/crates/mcp-server/src/server.rs`, `tools/storage.rs`, `tools/runs.rs`, `tools/runtime.rs`, `control-plane/crates/graphql-server/src/schema.rs`, `types/storage.rs`, `types/artifact.rs`, `types/run.rs`
- Swift read client: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`, `DaemonLifecycleClient.swift`, `Chainworks Forge/Views/RunsHomeView.swift`
- Migrations/gate/docs/evidence: `control-plane/crates/db/migrations/056-060_p087*.sql`, `scripts/test-gate.sh`, `docs/evidence/p087/api/*`, rollout-contract fixtures, reference docs

## Prior Proposal-Review Reuse

Reviewer-selection reuse: Not reused.

The skill helper returned no prior P087 proposal-review artifacts:

```text
{"artifacts": [], "proposal_path": "...087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md", "repo_root": "...cw-implement-proposal-087-local-s-b4edcf82"}
```

Existing implementation-audit reports were ignored for reviewer selection, per the skill workflow. R5 findings were used only as current implementation context after reviewer routing.

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `chainworks_execution_truth_reviewer` | P087 changes projection truth, MCP truth, run-list read truth, maintenance slots, startup recovery, and storage health readback. |
| `rust_reliability_reviewer` | Hot-read liveness, cancellation, circuit state, maintenance/reaper behavior, restart projection rebuild, and startup recovery are reliability surfaces. |
| `rust_performance_reviewer` | The proposal is centered on hot read latency, `runs.list` budget, projection rebuild cost, WAL/write contention, and p95/p99 metrics. |
| `api_contract_reviewer` | GraphQL storage health, MCP tool/result contracts, capability inventory, and additive fixture compatibility changed. |
| `observability_rollout_reviewer` | P087 requires metrics, thresholds, rollout readback, evidence fixtures, and a canonical `proposal-087` gate. |

Rejected close alternatives:

- `rust_arch_reviewer`: relevant, but lower priority than execution truth/reliability under the hard cap.
- `apple_arch_reviewer`: Swift read-client code changed, but the primary risk surface is server/storage/restart behavior.
- `macos_ui_reviewer`: UI visual tokens are statically checked by the gate; no new proposal-mandated UI workflow is central.
- `product_reviewer`: P087 is storage/read-path infrastructure; no product metric or experiment checkpoint is central to routing.

## Proposal Contract Summary

P087 adopts a three-tier local storage model: compact authoritative SQLite state, file-backed high-volume evidence/artifacts, and projection/cache-backed hot reads. It requires `runs.list`, MCP liveness tools, GraphQL live/read surfaces, storage health metrics, restart projection rebuild, and exit criteria gates to prevent SQLite from becoming a read/write choke point. It explicitly does not replace SQLite, add RocksDB/Postgres, undo DbWriter, alter durable side-effect semantics, add UI write controls, or implement compaction itself.

Platform/product scope:

- Apple: macOS SwiftUI read-client consumption only; no new UI write controls.
- Backend/service: Rust daemon, SQLite persistence, GraphQL, MCP, storage health, maintenance, projections, startup recovery, and rollout gates.
- Cross-stack: macOS UI reads daemon-owned projections via GraphQL; MCP clients consume bounded read tools and diagnostics.

Primary implementation flows:

1. MCP liveness sequence: initialize, tools/list, `runs.list`, `runtime.health`, `storage.health`, and a resource/artifact metadata read return within budgets under degraded writer or maintenance state.
2. Runs home and `runs.list` read from run projections without filesystem scans, N+1 attachments, transcript reads, or detail enrichment.
3. Storage health exposes writer, WAL, spool, projection lag/freshness, hot-read guard, exit-threshold metrics, and repair/reaper status through GraphQL and MCP.
4. Startup recovery rebuilds P087 projections after restart so runtime-health and artifact-noise read models are not stale.
5. Large evidence/artifact payloads remain file-spooled while SQLite stores bounded metadata and compact read models.

## Fidelity Inventory

Matches:

- Valid P087 migration numbering now creates compact projection/maintenance/readback structures: `run_summaries` hot read payloads, `projection_invalidation_log`, `projection_cursors`, `maintenance_operations`, `storage_health_snapshots`, `hot_read_circuit_states`, `artifact_noise_summary`, and `runtime_health_summary`.
- `runs.list` reads `run_summaries` directly through `projections::list_active_projection` and omits per-row detail enrichment.
- MCP hot reads are guarded and timed: `tools/list`, `resources/read`, and hot tools use hot-read circuit checks, latency metrics, probe budgets, typed timeouts, and cancellation tokens.
- GraphQL `runs` uses projection list functions, and `storageHealth` uses a hot-read guard plus additive projection freshness, hot-read guard, maintenance, and rollout fields.
- Storage health exposes P087 writer/WAL/projection/evidence spool/read-path metrics, plus evaluated threshold bands.
- Swift read clients include P087 storage diagnostics fields and projection-lag presentation tokens.
- Startup recovery now rebuilds the full P087 projection set through `rebuild_all_for_run`, and the proposal gate proves that stale runtime-health and artifact-noise projections are refreshed after restart.
- The daemon current working tree has a single periodic maintenance reaper owner, with one startup one-shot reaper pass and one repo-level periodic reaper.
- `./scripts/test-gate.sh proposal-087` passed on this exact working tree.

Divergences:

- No proposal-conformance divergences remain in the current working tree.

Ambiguities / Evidence Gaps:

- No live daemon process was started and no external MCP/GraphQL client performed a real process restart.
- Remote UI tests were not run; P087 UI evidence is static/code-level through the canonical proposal gate.
- `./scripts/test-gate.sh full` was not run. The successful verdict is based on the repository's canonical `proposal-087` gate, which is allowed by the audit workflow for proposal-scoped readiness.
- The passing behavior depends on dirty working-tree changes, including unstaged changes in `control-plane/crates/engine/src/recovery.rs`, `control-plane/crates/engine/tests/integration.rs`, `control-plane/crates/daemon/src/main.rs`, and `scripts/test-gate.sh`.

## Requirement Summary

| ID | Requirement | Status | Evidence |
|---|---|---|---|
| REQ-001 | SQLite remains compact canonical state and metadata; no high-volume event streams | Implemented | Proposal lines 67-84; migrations `056-060`; migration/static checks passed. |
| REQ-002 | High-volume evidence and artifact payloads remain file-spooled with SQLite metadata/pointers | Implemented | Proposal lines 86-109; MCP/GraphQL metadata pointer code and P087 fixtures passed. |
| REQ-003 | `runs.list` is projection-only and meets read budget | Implemented | Proposal lines 187-205, 483, 564-565, 591; MCP/GraphQL projection-only and 500 ms p95 tests passed. |
| REQ-004 | MCP hot reads return quickly/fail fast and the liveness sequence is covered | Implemented | Proposal lines 207-218, 422-444, 566, 592; MCP liveness/degraded tests passed. |
| REQ-005 | GraphQL hot reads use projections/cache and expose additive storage health fields | Implemented | Proposal lines 220-235, 593-594; GraphQL P087 tests and Swift static checks passed. |
| REQ-006 | Required hot projections, storage health metrics, and exit thresholds are implemented | Implemented | Proposal lines 239-330, 448-496; DB/MCP/GraphQL/rollout/metrics gate checks passed. |
| REQ-007 | Projection cache/read models can rebuild after daemon restart | Implemented | Proposal lines 536-542, 562-568; `proposal_087_projection_cache_rebuilds_after_restart` passed and is included in the gate. |
| REQ-008 | Storage exit criteria are documented and enforced by `proposal-087` gate | Implemented | Proposal lines 544-581, 595; `scripts/test-gate.sh proposal-087` passed and now includes restart rebuild proof. |
| REQ-009 | P078/P038/P086 and side-effect/continuation readbacks can rely on storage tiering without extra SQLite pressure | Implemented | Runtime/artifact projections, restart rebuild proof, liveness metrics, and dependent P077/P088 compatibility checks passed. |

## Detailed Requirement Audit

### REQ-001 - Compact SQLite Ownership

Proposal source: sections 3.1, 3.3, 18; closest lines 67-84 and 110-121.

Status: Implemented.

Evidence types: proposal, migration, code, tests-run.

Evidence references:

- `control-plane/crates/db/migrations/057_p087_storage_tiering_projections.sql` creates projection cursors, invalidation log, maintenance operations, storage health snapshots, and hot-read circuit state.
- `control-plane/crates/db/migrations/059_p087_projection_refinement.sql` creates `artifact_noise_summary` and `runtime_health_summary`.
- `./scripts/test-gate.sh proposal-087` verified DB migration versions and passed.

Implementation mapping: SQLite stores compact canonical/projection metadata. The P087 gate did not detect new raw stream/event chunk tables.

### REQ-002 - File-Backed Evidence and Artifact Pointers

Proposal source: sections 3.2, 6.3, 18; closest lines 86-109, 235, 590.

Status: Implemented.

Evidence types: proposal, code, schema, tests-run.

Evidence references:

- `control-plane/crates/mcp-server/src/server.rs` returns `artifact_metadata_pointer` for `artifact://` resources and redacts payload paths.
- `control-plane/crates/graphql-server/src/types/artifact.rs` exposes `artifact_metadata_pointer.v1`.
- `docs/evidence/p087/api/artifact-metadata-pointer-v1.fixture.json` is checked by the P087 gate.
- `./scripts/test-gate.sh proposal-087` passed artifact pointer and static leakage checks.

Implementation mapping: Artifact resources expose metadata and authorized payload routes instead of embedding raw payloads in hot paths.

### REQ-003 - Projection-Only `runs.list`

Proposal source: sections 6.1, 7.1, 15.1, 17.1, 18; closest lines 187-205, 243-262, 483, 564-565, 591.

Status: Implemented.

Evidence types: proposal, code, tests-run.

Evidence references:

- `control-plane/crates/db/src/repos/projections.rs` reads active runs from `run_summaries`.
- `control-plane/crates/mcp-server/src/tools/runs.rs` asserts `runs.list` omits detail attachments and keeps only projection-backed compact summaries.
- `control-plane/crates/mcp-server/src/server.rs` includes the seeded 500 ms p95 `runs.list` test.
- `control-plane/crates/graphql-server/src/schema.rs` routes GraphQL `runs` through projection functions and tests no per-row enrichment.
- `./scripts/test-gate.sh proposal-087` passed the relevant MCP and GraphQL tests.

Implementation mapping: Hot run lists are served by materialized summary rows and measured by production hot-read wrappers.

### REQ-004 - MCP Read-Path Liveness

Proposal source: sections 6.2, 13, 17.1, 18; closest lines 207-218, 422-444, 566, 592.

Status: Implemented.

Evidence types: proposal, code, tests-run.

Evidence references:

- `control-plane/crates/db/src/hot_read_guard.rs` implements observe/enforce modes, retry-after half-open transition, one-probe gating, and violation recording.
- `control-plane/crates/mcp-server/src/hot_read_guard.rs` identifies P087 hot-read tools.
- `control-plane/crates/mcp-server/src/server.rs` guards `tools/list`, `resources/read`, and hot tools with cancellation, timeout, success, and violation tracking.
- `control-plane/crates/mcp-server/src/server.rs` covers the mandatory liveness sequence and running-maintenance scenario.
- `./scripts/test-gate.sh proposal-087` ran 15 P087 MCP tests successfully.

Implementation mapping: MCP reads are bounded by hot-read guard behavior; degraded storage health returns typed stale/degraded status instead of hanging.

### REQ-005 - GraphQL Projection Readback

Proposal source: sections 6.3, 7, 14, 18; closest lines 220-235, 239-330, 448-467, 593-594.

Status: Implemented.

Evidence types: proposal, code, schema, tests-run.

Evidence references:

- `control-plane/crates/graphql-server/src/schema.rs` uses projection functions for run lists and guards `storageHealth` with the hot-read guard and timeout.
- `control-plane/crates/graphql-server/src/types/storage.rs` exposes additive storage health, projection freshness, hot-read guard, maintenance, and filtered freshness readback fields.
- `control-plane/crates/graphql-server/src/types/storage.rs` tests storage health v1 preservation and lower-case hot-read circuit status parsing.
- `Chainworks Forge/Support/DaemonLifecycleClient.swift` queries the additive P087 storage diagnostics fields.
- `./scripts/test-gate.sh proposal-087` passed GraphQL storage tests and Swift/static diagnostics checks.

Implementation mapping: GraphQL list and storage readback paths are projection-oriented and additive, preserving legacy `projections` compatibility while adding P087 fields.

### REQ-006 - Hot Projections, Storage Health, and Metrics

Proposal source: sections 7, 14, 15, 18; closest lines 239-330 and 448-496.

Status: Implemented.

Evidence types: proposal, code, telemetry, migration, tests-run.

Evidence references:

- `control-plane/crates/db/src/repos/projections.rs` implements run-list, approval inbox, artifact noise, and runtime health projections.
- `control-plane/crates/db/src/repos/storage_health.rs` exposes writer, WAL, projections, freshness, hot-read guards, read-path metrics, artifact noise, maintenance, rollout, thresholds, and evaluated P087 metrics.
- `control-plane/crates/db/src/metrics.rs` declares and records P087 metrics including `runs_list_read_latency_ms` and `mcp_liveness_gate_duration_ms`.
- `./scripts/test-gate.sh proposal-087` passed DB metrics, storage health, rollout fixture, and metric declaration checks.

Implementation mapping: The projection and metric surfaces required for storage exit decisions exist and are exercised by the proposal gate.

### REQ-007 - Restart Projection Rebuild

Proposal source: section 16 Phase 3 and section 17.1; closest lines 536-542 and 562-568.

Status: Implemented.

Evidence types: proposal, code, tests-run.

Evidence references:

- `control-plane/crates/db/src/repos/projections.rs` includes P087 artifact noise and runtime health in `rebuild_all_for_run`.
- `control-plane/crates/engine/src/recovery.rs` calls `rebuild_startup_read_projections` after startup repair, and that helper delegates to `projections::rebuild_all_for_run`.
- `control-plane/crates/engine/tests/integration.rs` adds `proposal_087_projection_cache_rebuilds_after_restart`, which seeds stale P087 projections and verifies refreshed artifact-noise and runtime-health projections after startup repair.
- `scripts/test-gate.sh` includes `cargo test -p engine --test integration proposal_087 -- --nocapture`.
- `./scripts/test-gate.sh proposal-087` ran the engine test and it passed.

Implementation mapping: The current startup recovery path rebuilds the full P087 read-projection set after recovery work, including runtime-health state that depends on catchup work enqueued during startup.

### REQ-008 - Exit Criteria Gate

Proposal source: sections 15, 16 Phase 4, 17, 18; closest lines 471-496, 544-549, 560-581, 595.

Status: Implemented.

Evidence types: proposal, config, tests-run.

Evidence references:

- `scripts/test-gate.sh` defines `proposal-087|p087`, duplicate migration checks, DB/MCP/auth/engine/GraphQL tests, UI/schema/static checks, evidence fixture checks, rollout fixture checks, metric declaration checks, and capability inventory checks.
- `./scripts/test-gate.sh proposal-087` passed on the audited tree.
- The gate now includes the restart projection rebuild test that was missing in R5.

Implementation mapping: The canonical proposal gate is non-empty, covers the explicit P087 test set, and passes on the current working tree.

### REQ-009 - Dependent Proposal Reliance Without Extra SQLite Pressure

Proposal source: sections 10-12 and 18; closest acceptance criterion line 596.

Status: Implemented.

Evidence types: proposal, code, tests-run.

Evidence references:

- Runtime health projection computes side-effect unresolved and continuation active counts.
- Artifact noise projection computes compaction-relevant artifact counts.
- MCP liveness tests assert runtime-health and artifact-noise/read-path fields.
- The new restart-rebuild test proves stale runtime-health and artifact-noise projections are refreshed after startup recovery.
- The daemon current working tree starts a single periodic maintenance reaper owner.

Implementation mapping: P078/P038/P086 readback dependencies can rely on projection-backed summaries without hot reads performing deep evidence scans.

## Reviewer / Lens Scorecard

| Lens | Conformance | Top risk | Confidence |
|---|---|---|---|
| Chainworks execution truth | Implemented | Passing evidence is for the dirty working tree, so staging/handoff must preserve the unstaged fixes. | High |
| Rust reliability | Implemented | Live external restart was not executed, but in-process restart recovery proof is now gate-backed. | High |
| Rust performance | Implemented | Seeded `runs.list` p95 proof passes; no long-run live workload metrics were gathered. | Medium |
| API contract | Implemented | Additive MCP/GraphQL contracts and fixtures pass. | High |
| Observability / rollout | Implemented | Canonical P087 gate passes; full regression and live daemon smoke were not run. | High |

## Routed Specialist Findings

No Critical or Major routed findings remain for the current working tree.

### READY-001 - Passing Evidence Depends on Dirty Working-Tree Fixes

Reviewer: `observability_rollout_reviewer`

Severity: Minor

Confidence: High

Related proposal items: REQ-007, REQ-008, REQ-009.

Evidence types: diff, tests-run.

Evidence references:

- `git status --short --branch` shows unstaged changes in `control-plane/crates/engine/src/recovery.rs`, `control-plane/crates/engine/tests/integration.rs`, `control-plane/crates/daemon/src/main.rs`, and `scripts/test-gate.sh`.
- `./scripts/test-gate.sh proposal-087` passed against the full current working tree.

Why it matters: The implementation is ready as a working-tree state, but handoff/merge readiness depends on staging the unstaged fixes that close the R5 restart/reaper findings.

Recommended action: Before commit/PR, stage or otherwise preserve all current working-tree fixes that participated in the passing proposal gate.

Acceptance criteria: A clean commit or PR diff contains the restart rebuild helper/test, the P087 gate's engine test invocation, and the single periodic maintenance reaper owner.

### READY-002 - Live Daemon Restart and Full Regression Were Not Run

Reviewer: `rust_reliability_reviewer`

Severity: Note

Confidence: High

Related proposal items: REQ-004, REQ-007, REQ-008.

Evidence types: tests-run, inference.

Evidence references:

- `./scripts/test-gate.sh proposal-087` passed and includes in-process restart recovery proof.
- No external daemon process, MCP client, GraphQL client, remote UI test, or `./scripts/test-gate.sh full` run was executed during this audit.

Why it matters: The canonical proposal gate is sufficient for proposal-scoped readiness, but a live restart smoke would still reduce residual integration risk before a high-confidence release handoff.

Recommended action: Treat live daemon restart smoke and full regression as optional final hardening, not blockers for P087 implementation conformance.

Acceptance criteria: If required for release, run a daemon restart smoke that validates MCP `runtime.health`, `storage.health`, `runs.list`, and GraphQL `storageHealth` after restart.

## Readiness Checklist

| Check | Status | Evidence / note |
|---|---|---|
| Proposal path resolved | Pass | Proposal exists in target worktree. |
| Report path resolved | Pass | Helper returned this R6 path; it did not exist before writing. |
| Prior proposal review discovered | Pass | No prior proposal-review artifacts found; reviewer reuse `Not reused`. |
| Same-tree canonical proposal gate | Pass | `./scripts/test-gate.sh proposal-087` passed on the audited working tree and HEAD. |
| Build/compile evidence | Pass with warnings | P087 cargo tests compiled and passed; existing warnings remain for unused variables/dead code/lifetime syntax. |
| MCP liveness primary flow | Pass | In-process liveness sequence and degraded storage health typed error tests passed. |
| `runs.list` hot read budget | Pass | Seeded MCP test asserts p95 below 500 ms and records read metrics. |
| GraphQL additive contract | Pass | Storage health v1/additive tests and static fixture checks passed. |
| Swift read-client/UI static checks | Pass | Gate found P087 diagnostics fields and projection-lag tokens. |
| Restart projection rebuild | Pass | `proposal_087_projection_cache_rebuilds_after_restart` passed and is included in the P087 gate. |
| Periodic maintenance reaper ownership | Pass | Current working tree has one periodic repo-level reaper plus one startup one-shot reaper. |
| Remote UI tests | Not run | Repo policy says UI tests are remote-only; P087 did not require a remote UI run for this audit. |
| Full regression suite | Not run | Canonical `proposal-087` gate passed; `./scripts/test-gate.sh full` was not run. |
| Live daemon runtime | Not run | No live process or external MCP/GraphQL client session was started. |

## Verification Log

Commands and outcomes:

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py <proposal>`: returned `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria_IMPLEMENTATION_AUDIT_R6.md`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py <proposal>`: returned no prior proposal-review artifacts.
- `git rev-parse HEAD`: `569b297e58582153eb3601f91d18e7aa97d9a6f2`.
- `git status --short --branch`: target is `cw/implement-proposal-087-local-s/b4edcf82` with staged, unstaged, and untracked P087 implementation files.
- `./scripts/test-gate.sh proposal-087`: passed.
  - DB P087 tests: 19 passed.
  - MCP P087 tests: 15 passed.
  - Auth P087 tests: 2 passed.
  - Engine P087 restart-rebuild integration test: 1 passed.
  - GraphQL storage/P087 tests: passed.
  - P077/P088 compatibility checks included by the P087 gate: passed.
  - Static UI/schema/evidence/rollout fixture checks: passed.
- Focused code inspection:
  - Proposal lines 55-140, 185-335, 420-596.
  - P087 gate implementation in `scripts/test-gate.sh`.
  - Hot-read guard, MCP dispatch, GraphQL storage, storage health, projections, maintenance, startup recovery, daemon composition, and restart integration test code.
- Not run:
  - `./scripts/test-gate.sh full`
  - remote UI tests
  - live daemon restart with external MCP/GraphQL clients

## Final Verdict

P087 is implemented in the current working tree. The prior R5 blockers are resolved: startup recovery now rebuilds the full P087 projection set, the proposal gate proves restart projection rebuild behavior, and the duplicate periodic maintenance reaper loop is removed from the current working tree.

Overall readiness is Ready with Risks rather than fully Ready because the audit did not run a live external daemon restart, remote UI tests, or the full repository gate, and because the passing evidence depends on dirty working-tree fixes that must be preserved in handoff. There are no remaining Critical or Major proposal-conformance blockers.
