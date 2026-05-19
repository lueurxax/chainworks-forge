# Proposal 087 Implementation Audit R5

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Audit report | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria_IMPLEMENTATION_AUDIT_R5.md` |
| Audit timestamp | 2026-05-16T11:34:38Z |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Implementation target | Working tree on `cw/implement-proposal-087-local-s/b4edcf82` |
| Git HEAD | `569b297e58582153eb3601f91d18e7aa97d9a6f2` |
| Compare base | Implicit current worktree; staged, unstaged, and untracked implementation files inspected |
| Proposal state | Active draft; no superseding proposal found in this audit |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Audit confidence | High for static/code/gate evidence; Medium for live daemon restart behavior because no live daemon restart was executed |

## Implementation Target

The audited tree contains a broad staged P087 implementation plus unstaged follow-up fixes and untracked P087 files. Important touched surfaces include:

- Rust DB/storage: `control-plane/crates/db/src/repos/projections.rs`, `storage_health.rs`, `writer.rs`, `hot_read_guard.rs`, `metrics.rs`, `hot_read_circuit.rs`, `maintenance.rs`, `projection_invalidation.rs`
- Rust MCP/GraphQL/API: `control-plane/crates/mcp-server/src/server.rs`, `tools/storage.rs`, `tools/runs.rs`, `tools/runtime.rs`, `control-plane/crates/graphql-server/src/schema.rs`, `types/storage.rs`, `types/artifact.rs`, `types/run.rs`
- Swift read client: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`, `DaemonLifecycleClient.swift`, `Chainworks Forge/Views/RunsHomeView.swift`
- Migrations/gate/docs/evidence: `control-plane/crates/db/migrations/056-060_p087*.sql`, `scripts/test-gate.sh`, `docs/evidence/p087/api/*`, rollout-contract fixtures, reference docs

Notable R5 delta from the previous failed audit state: the duplicate migration-version blocker is fixed. Current migration files are `056_p087_run_summary_hot_read_payloads.sql`, `057_p087_storage_tiering_projections.sql`, `058_p087_hot_read_refinements.sql`, `059_p087_projection_refinement.sql`, and `060_p087_projection_invalidation_lifecycle.sql`.

## Prior Proposal-Review Reuse

Reviewer-selection reuse: Not reused.

The skill helper found no prior P087 proposal-review packet:

```text
{"artifacts": [], "proposal_path": "...087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md", "repo_root": "...cw-implement-proposal-087-local-s-b4edcf82"}
```

Existing `IMPLEMENTATION_AUDIT` reports were ignored for reviewer selection per the audit workflow.

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `chainworks_execution_truth_reviewer` | P087 changes projection truth, MCP truth, run-list read truth, maintenance slots, and storage health readback. |
| `rust_reliability_reviewer` | Hot-read liveness, cancellation, circuit state, maintenance/reaper behavior, startup recovery, and restart projection freshness are reliability surfaces. |
| `rust_performance_reviewer` | The proposal is centered on hot read latency, `runs.list` budget, projection rebuild cost, WAL/write contention, and p95/p99 metrics. |
| `api_contract_reviewer` | GraphQL storage health, MCP tool/result contracts, capability inventory, and additive fixture compatibility changed. |
| `observability_rollout_reviewer` | P087 requires metrics, thresholds, rollout readback, evidence fixtures, and a canonical `proposal-087` gate. |

Rejected close alternatives:

- `rust_arch_reviewer`: relevant, but covered by execution truth and reliability lenses under the hard cap.
- `apple_arch_reviewer`: Swift read-client code changed, but the remaining risks are server/restart/gate/API first.
- `macos_ui_reviewer`: UI visual tokens are statically checked by the gate; no proposal-mandated new macOS workflow is the primary risk.
- `product_reviewer`: no explicit product metric, experiment, or user-value checkpoint was part of P087.

## Proposal Contract Summary

P087 adopts a three-tier local storage model: compact canonical SQLite state, file-backed high-volume evidence/artifacts, and projection/cache-backed hot reads. It requires `runs.list`, MCP liveness tools, GraphQL live/read surfaces, storage health metrics, and exit criteria gates to prevent SQLite from becoming a read/write choke point. It explicitly does not replace SQLite, add RocksDB/Postgres, undo DbWriter, alter durable side-effect semantics, add UI write controls, or implement compaction itself.

Platform/product scope:

- Apple: macOS SwiftUI read-client consumption only; no new UI write controls.
- Backend/service: Rust daemon, SQLite persistence, GraphQL, MCP, storage health, maintenance, projections, rollout gates.
- Cross-stack: macOS UI reads daemon-owned projections via GraphQL; MCP clients consume bounded read tools and diagnostics.

Primary implementation flows:

1. MCP liveness sequence: initialize, tools/list, `runs.list`, `runtime.health`, `storage.health`, and a resource/artifact metadata read return within budgets under degraded writer or maintenance state.
2. Runs home and `runs.list` read from `ActiveRunIndex`/run projections without filesystem scans, N+1 attachments, transcript reads, or detail enrichment.
3. Storage health exposes writer, WAL, spool, projection lag/freshness, hot-read guard, exit-threshold metrics, and repair/reaper status through GraphQL and MCP.
4. Projection rebuild/invalidation survives restart and compact state changes without blocking read liveness.
5. Large evidence/artifact payloads remain file-spooled while SQLite stores bounded metadata and compact read models.

## Fidelity Inventory

Matches:

- P087 migration numbering is now valid and creates compact projection/maintenance/readback structures: `run_summaries` hot read payloads, `projection_invalidation_log`, `projection_cursors`, `maintenance_operations`, `storage_health_snapshots`, `hot_read_circuit_states`, `artifact_noise_summary`, and `runtime_health_summary`.
- `runs.list` now reads `run_summaries` directly through `projections::list_active_projection` and omits per-row detail enrichment.
- MCP hot reads are guarded and timed: `tools/list`, `resources/read`, and hot tools use hot-read circuit checks, latency metrics, probe budgets, typed timeouts, and cancellation tokens.
- GraphQL `runs` uses projection list functions, and `storageHealth` uses a hot-read guard plus additive projection freshness, hot-read guard, maintenance, and rollout fields.
- Storage health exposes P087 writer/WAL/projection/evidence spool/read-path metrics, plus evaluated threshold bands.
- Swift read clients include P087 storage diagnostics fields and projection-lag presentation tokens.
- `./scripts/test-gate.sh proposal-087` passed on this exact working tree, including DB, MCP, auth, GraphQL, UI/static, fixture, rollout, and metric declaration checks.

Divergences:

- Startup projection rebuild is incomplete for the P087 projection set. Generic startup recovery calls `rebuild_operator_read_projections`, which rebuilds run summary, stage summaries, and approval inbox only; it does not rebuild P087 `artifact_noise_summary` or `runtime_health_summary` even though `rebuild_all_for_run` does.
- The P087 proposal gate does not contain a test that proves projection cache/read models rebuild after daemon restart. The only P087 restart-specific test found is the maintenance reaper orphan-slot test.
- The audited working tree starts two periodic maintenance reaper loops in the daemon: a local `spawn_maintenance_reaper` and `db::repos::maintenance::spawn_maintenance_reaper`.

Ambiguities / Evidence Gaps:

- No live daemon process was started and no real restart was executed during this audit. Restart behavior was assessed from startup code and test coverage.
- No remote UI test was run; P087 UI evidence is limited to static gate checks and code inspection.
- Phase 5's "run real workflows, inspect metrics, decide whether Plan A is sufficient" remains an operational checkpoint, not a completed long-run evidence artifact.

## Requirement Summary

| ID | Requirement | Status | Evidence |
|---|---|---|---|
| REQ-001 | SQLite remains compact canonical state and metadata; no high-volume event streams | Implemented | Proposal lines 67-84; migrations `056-060`; proposal gate migration/static checks passed. |
| REQ-002 | High-volume evidence and artifact payloads remain file-spooled with SQLite metadata/pointers | Implemented | Proposal lines 86-109; MCP/GraphQL artifact metadata pointer code and fixtures; proposal gate fixture checks passed. |
| REQ-003 | `runs.list` is projection-only and meets read budget | Implemented | Proposal lines 187-205 and 589-592; `list_active_projection`; MCP/GraphQL projection-only tests; seeded 500 ms test passed. |
| REQ-004 | MCP hot reads return quickly/fail fast and the liveness sequence is covered | Implemented | Proposal lines 207-218 and 422-444; hot-read guard, typed errors, liveness sequence tests; proposal gate passed. |
| REQ-005 | GraphQL hot reads/subscriptions use projections/cache and additive storage health fields | Implemented | Proposal lines 220-235 and 593-594; GraphQL runs/storage health code and tests; Swift read models updated. |
| REQ-006 | Required hot projections, storage health metrics, and exit thresholds are implemented | Implemented | Proposal lines 239-330 and 448-496; metrics declarations, storage health readback, runtime/artifact projections, rollout fixture; proposal gate passed. |
| REQ-007 | Projection rebuild can recover P087 read models after daemon restart | Partially Implemented | Proposal lines 536-542 and 562-568; `rebuild_all_for_run` can rebuild P087 projections, but generic startup rebuild omits them and no P087 restart rebuild test exists. |
| REQ-008 | Storage exit criteria are enforced by the canonical gate | Partially Implemented | Proposal lines 544-581; `proposal-087` gate passed, but it does not enforce the explicit restart projection rebuild test. |
| REQ-009 | P078/P038/P086 and side-effect/continuation readbacks can rely on storage tiering without extra SQLite pressure | Partially Implemented | Runtime/artifact projections exist and gate checks dependent readback fields, but stale/missing P087 projections after normal restart remain possible and duplicate reapers add avoidable writes. |

## Detailed Requirement Audit

### REQ-001 - Compact SQLite Ownership

Proposal source: sections 3.1, 3.3, 18; closest lines 67-84 and 110-121.

Status: Implemented.

Evidence types: proposal, migration, code, tests-run.

Evidence references:

- `control-plane/crates/db/migrations/057_p087_storage_tiering_projections.sql` creates projection cursors, invalidation log, maintenance operations, storage health snapshots, and hot-read circuit state.
- `control-plane/crates/db/migrations/059_p087_projection_refinement.sql` creates `artifact_noise_summary` and `runtime_health_summary`.
- `./scripts/test-gate.sh proposal-087` printed `P087 DB migration versions verified` and passed.

Implementation mapping: SQLite stores compact canonical and projection metadata. The audit did not find a new raw stream/event chunk table introduced by P087.

Gap / note: The gate passes the migration-version preflight that blocked R4.

### REQ-002 - File-Backed Evidence and Artifact Pointers

Proposal source: sections 3.2, 6.3, 18; closest lines 86-109, 235, and 590.

Status: Implemented.

Evidence types: proposal, code, schema, tests-run.

Evidence references:

- `control-plane/crates/mcp-server/src/server.rs` returns `artifact_metadata_pointer` for `artifact://` resources and redacts payload paths.
- `control-plane/crates/graphql-server/src/types/artifact.rs` exposes `artifact_metadata_pointer.v1`.
- `docs/evidence/p087/api/artifact-metadata-pointer-v1.fixture.json` exists and the P087 gate checks it.
- `./scripts/test-gate.sh proposal-087` passed the artifact pointer fixture and static resource checks.

Implementation mapping: Artifact resources expose metadata and authorized payload routes instead of embedding raw payloads in hot paths.

Gap / note: No live artifact read was executed outside the in-process gate.

### REQ-003 - Projection-Only `runs.list`

Proposal source: sections 6.1, 7.1, 15.1, 17.1, 18; closest lines 187-205, 243-262, 483, 564-565, and 591.

Status: Implemented.

Evidence types: proposal, code, tests-run.

Evidence references:

- `control-plane/crates/db/src/repos/projections.rs:198-257` reads active runs from `run_summaries`.
- `control-plane/crates/mcp-server/src/tools/runs.rs:1487-1526` asserts `runs.list` omits detail attachments and keeps only projection-backed compact summaries.
- `control-plane/crates/mcp-server/src/server.rs:2693-2730` seeds 10 runs, calls `runs.list` repeatedly, and asserts p95 under 500 ms.
- `control-plane/crates/graphql-server/src/schema.rs:257-266` routes GraphQL `runs` through `list_by_idea_projection` or `list_active_projection`.
- `control-plane/crates/graphql-server/src/schema.rs:2877-2937` asserts GraphQL `runs` does not per-row enrich implementation, rollout, side-effect, or closeout detail.
- `./scripts/test-gate.sh proposal-087` ran and passed the MCP and GraphQL P087 tests.

Implementation mapping: The hot run list is now served by materialized summary rows and measured by production hot-read wrappers.

Gap / note: Performance evidence is an in-process seeded test, not a live daemon benchmark.

### REQ-004 - MCP Read-Path Liveness

Proposal source: sections 6.2, 13, 17.1, 18; closest lines 207-218, 422-444, 566, and 592.

Status: Implemented.

Evidence types: proposal, code, tests-run.

Evidence references:

- `control-plane/crates/db/src/hot_read_guard.rs:91-167` implements observe/enforce modes, retry-after half-open transition, one-probe gating, and violation recording.
- `control-plane/crates/mcp-server/src/hot_read_guard.rs:81-90` identifies P087 hot read tools.
- `control-plane/crates/mcp-server/src/server.rs:332-348` guards `tools/list`; `451-477` guards `resources/read`; `844-920` guards hot tools with cancellation, timeout, success, and violation tracking.
- `control-plane/crates/mcp-server/src/server.rs:2568-2690` proves the liveness sequence survives a running maintenance operation and records metrics.
- `control-plane/crates/mcp-server/src/server.rs:2927-3026` covers initialize, tools/list, resource read, `runs.list`, `runtime.health`, `storage.health`, and read-path metric samples.
- `./scripts/test-gate.sh proposal-087` ran and passed 15 P087 MCP tests.

Implementation mapping: MCP reads are no longer an unbounded request-loop dependency; degraded storage health returns typed stale/degraded status instead of hanging.

Gap / note: This was verified in-process, not through an external MCP transport session.

### REQ-005 - GraphQL Projection Readback

Proposal source: sections 6.3, 7, 14, 18; closest lines 220-235, 239-330, 448-467, and 593-594.

Status: Implemented.

Evidence types: proposal, code, schema, tests-run.

Evidence references:

- `control-plane/crates/graphql-server/src/schema.rs:257-266` uses projection functions for run lists.
- `control-plane/crates/graphql-server/src/schema.rs:581-623` wraps GraphQL `storageHealth` in the hot-read guard and timeout path.
- `control-plane/crates/graphql-server/src/types/storage.rs:204-250` exposes additive storage health, projection freshness, hot-read guard, maintenance, and filtered freshness readback fields.
- `control-plane/crates/graphql-server/src/types/storage.rs:556-652` tests storage health v1 preservation and lower-case hot-read circuit status parsing.
- `Chainworks Forge/Support/DaemonLifecycleClient.swift:649-757` queries the additive P087 storage diagnostics fields.
- `./scripts/test-gate.sh proposal-087` ran and passed GraphQL storage tests and Swift/static diagnostics checks.

Implementation mapping: GraphQL list and storage readback paths are projection-oriented and additive, preserving legacy `projections` compatibility while adding P087 fields.

Gap / note: GraphQL subscriptions themselves were not runtime-tested in this audit; the audited evidence is query/schema/read-model focused.

### REQ-006 - Hot Projections, Storage Health, and Metrics

Proposal source: sections 7, 14, 15, 18; closest lines 239-330 and 448-496.

Status: Implemented.

Evidence types: proposal, code, telemetry, migration, tests-run.

Evidence references:

- `control-plane/crates/db/src/repos/projections.rs:198-257` implements ActiveRunIndex-style run list projection reads.
- `control-plane/crates/db/src/repos/projections.rs:935-965` implements approval inbox projection reads.
- `control-plane/crates/db/src/repos/projections.rs:570-618` rebuilds artifact noise projection.
- `control-plane/crates/db/src/repos/projections.rs:621-707` rebuilds runtime health projection.
- `control-plane/crates/db/src/repos/projections.rs:1069-1088` includes P087 artifact noise and runtime health in full run projection rebuilds.
- `control-plane/crates/db/src/repos/storage_health.rs:304-454` exposes writer, WAL, projections, freshness, hot-read guards, read-path metrics, artifact noise, maintenance, rollout, thresholds, and evaluated P087 metrics.
- `control-plane/crates/db/src/metrics.rs:61-93` declares P087 required metrics and `122-148` records hot-read and MCP liveness latency.
- `./scripts/test-gate.sh proposal-087` passed DB metrics, storage health, and rollout fixture checks.

Implementation mapping: The projection and metric surfaces required for storage exit decisions exist and are exercised by the proposal gate.

Gap / note: Long-run real-workflow metric thresholds remain a Phase 5 operational decision, not a code-level implementation proof.

### REQ-007 - Restart Projection Rebuild

Proposal source: section 16 Phase 3 and section 17.1; closest lines 536-542 and 562-568.

Status: Partially Implemented.

Evidence types: proposal, code, tests-found, inference.

Evidence references:

- `control-plane/crates/db/src/repos/projections.rs:1069-1088` can rebuild P087 `artifact_noise_summary` and `runtime_health_summary` through `rebuild_all_for_run`.
- `control-plane/crates/engine/src/recovery.rs:577-583` runs startup projection rebuild during recovery.
- `control-plane/crates/engine/src/recovery.rs:1904-1911` shows that generic startup rebuild only calls `rebuild_run_summary`, `rebuild_stage_summaries`, and `rebuild_approval_inbox`.
- `rg "proposal_087.*restart|restart.*projection|projection.*restart|startup.*projection|proposal_087.*rebuild"` found no P087 restart projection rebuild test; the only P087 restart-specific hit is `proposal_087_restart_reaper_orphans_only_stale_running_slots`.

Implementation mapping: The full rebuild primitive exists, and many write paths call it, but normal startup recovery does not rebuild the full P087 projection set.

Gap / note: After a normal daemon restart with stale or missing `artifact_noise_summary` or stale `runtime_health_summary`, storage health/runtime health can serve old compact readback until a later write path triggers `rebuild_all_for_run`.

### REQ-008 - Exit Criteria Gate

Proposal source: sections 15, 16 Phase 4, 17, 18; closest lines 471-496, 544-549, 560-581, and 595.

Status: Partially Implemented.

Evidence types: proposal, config, tests-run, tests-found.

Evidence references:

- `scripts/test-gate.sh:7783-8004` defines `proposal-087|p087`, duplicate migration checks, focused cargo tests, UI/schema/static checks, evidence fixture checks, rollout fixture checks, metric declaration checks, and capability inventory checks.
- `./scripts/test-gate.sh proposal-087` passed on the audited tree.
- The gate does not assert the explicit required test from proposal line 568: projection cache can rebuild after daemon restart.

Implementation mapping: The canonical gate is present, non-empty, and passes, but it does not enforce every required P087 test.

Gap / note: The passing gate is strong evidence for the hot-read and storage-health surfaces, but it is not sufficient to close the restart projection rebuild requirement.

### REQ-009 - Dependent Proposal Reliance Without Extra SQLite Pressure

Proposal source: sections 10-12 and 18; closest acceptance criterion line 596.

Status: Partially Implemented.

Evidence types: proposal, code, tests-run, inference.

Evidence references:

- `control-plane/crates/db/src/repos/projections.rs:657-675` computes side-effect unresolved and continuation active counts for runtime health.
- `control-plane/crates/db/src/repos/projections.rs:570-618` computes artifact noise fields needed for compaction readiness.
- `control-plane/crates/mcp-server/src/server.rs:2631-2649` asserts runtime health includes active sessions, hot-read circuit flags, side-effect unresolved count, and continuation active count.
- `control-plane/crates/mcp-server/src/server.rs:2676-2689` asserts storage health exposes artifact noise and read-path metrics after the liveness sequence.
- `control-plane/crates/daemon/src/main.rs:267` starts a local periodic maintenance reaper; `318` starts a second repo-level periodic maintenance reaper in the same daemon.

Implementation mapping: Dependent readback fields exist and are gate-tested, but restart freshness and duplicate periodic reaper writes leave bounded reliability/pressure risk.

Gap / note: This is the same underlying gap as REL-001/OPS-001, not evidence that the side-effect ledger itself changed semantics.

## Reviewer / Lens Scorecard

| Lens | Conformance | Top risk | Confidence |
|---|---|---|---|
| Chainworks execution truth | Partial | Startup recovery rebuilds only the older operator projections, so P087 runtime/artifact read truth can be stale after restart. | High |
| Rust reliability | Not Ready | Restart projection rebuild is incomplete and untested; duplicate reaper loops add avoidable background writes. | High |
| Rust performance | Ready with risks | `runs.list` p95 budget passes in seeded tests, but duplicate reaper loops are avoidable storage pressure. | Medium |
| API contract | Ready with risks | MCP/GraphQL additive contracts and fixtures pass, but restart freshness is not covered by a contract test. | High |
| Observability / rollout | Partial | Metrics and rollout fixtures pass; the gate omits the explicit restart projection rebuild acceptance test. | High |

## Routed Specialist Findings

### REL-001 - Startup Recovery Does Not Rebuild the Full P087 Projection Set

Reviewer: `rust_reliability_reviewer`

Severity: Major

Confidence: High

Related proposal items: REQ-007, REQ-008, REQ-009; proposal lines 536-542, 562-568, 596.

Evidence types: proposal, code, tests-found.

Evidence references:

- Proposal line 541 requires cache rebuild on startup; line 568 requires a test that projection cache can rebuild after daemon restart.
- `control-plane/crates/db/src/repos/projections.rs:1069-1088` shows the full rebuild path includes `rebuild_artifact_noise_summary` and `rebuild_runtime_health_summary`.
- `control-plane/crates/engine/src/recovery.rs:577-583` invokes startup projection rebuild during recovery.
- `control-plane/crates/engine/src/recovery.rs:1904-1911` shows startup rebuild only calls run summary, stage summaries, and approval inbox.
- P087 restart-related search found no startup projection rebuild test beyond the maintenance reaper orphan-slot test.

Why it matters: P087 makes storage health, runtime health, artifact noise, P038 compaction readiness, side-effect unresolved count, and continuation active count depend on compact read models. If a daemon restarts with stale or missing P087 projection rows, hot reads can return old or empty state until some later write path triggers `rebuild_all_for_run`.

Recommended action: Make startup recovery call `rebuild_all_for_run` or a dedicated P087 startup rebuild that includes run summary, stage summaries, approval inbox, artifact index, artifact noise, runtime health, and any freshness cursors required by storage health. Add a focused P087 test that seeds stale/missing P087 projection rows, runs startup recovery or the startup rebuild helper, and asserts runtime health/artifact noise/freshness readback is rebuilt.

Acceptance criteria:

- A test named along the lines of `proposal_087_projection_cache_rebuilds_after_restart` fails before the fix and passes after it.
- `scripts/test-gate.sh proposal-087` includes that test.
- A normal startup path rebuilds or explicitly validates every P087 hot projection required for storage health and dependent readbacks.

### OPS-001 - The Daemon Starts Two Periodic Maintenance Reaper Loops

Reviewer: `observability_rollout_reviewer`

Severity: Minor

Confidence: High

Related proposal items: REQ-006, REQ-009; proposal lines 477-496, 531-534, 544-549.

Evidence types: code, diff, inference.

Evidence references:

- `control-plane/crates/daemon/src/main.rs:267` starts the local `spawn_maintenance_reaper`.
- `control-plane/crates/daemon/src/main.rs:318` also starts `db::repos::maintenance::spawn_maintenance_reaper`.
- `control-plane/crates/daemon/src/main.rs:564-574` defines the local 30-second loop.
- `control-plane/crates/db/src/repos/maintenance.rs:672-681` defines the repo-level 60-second loop.

Why it matters: P087 is specifically about controlling storage pressure and keeping maintenance from interfering with ordinary hot reads. Two periodic loops perform the same reaper work, create extra `restart_reaper` writes, and can add avoidable SQLite contention/metric noise.

Recommended action: Pick a single owner for periodic maintenance reaping. Prefer the repo-level helper if the daemon no longer needs custom cadence behavior, or keep the daemon helper and remove the second spawn. Add a small daemon composition test or static gate check so this does not regress.

Acceptance criteria:

- The daemon starts exactly one periodic maintenance reaper loop.
- Startup still runs the one-shot P087 reaper pass.
- P087 storage health rollout readback still exposes `p087_restart_reaper_last_run`.

## Readiness Checklist

| Check | Status | Evidence / note |
|---|---|---|
| Proposal path resolved | Pass | Proposal exists in target worktree. |
| Report path resolved | Pass | Helper returned this R5 path; it did not exist before writing. |
| Prior proposal review discovered | Pass | No prior proposal-review artifacts found; reviewer reuse `Not reused`. |
| Same-tree canonical proposal gate | Pass | `./scripts/test-gate.sh proposal-087` passed on the audited working tree and HEAD. |
| Build/compile evidence | Pass with warnings | P087 cargo tests compiled and passed; warnings remain for unused variables/dead code/lifetime syntax. |
| MCP liveness primary flow | Pass | In-process liveness sequence and degraded storage health typed error tests passed. |
| `runs.list` hot read budget | Pass | Seeded MCP test asserts p95 below 500 ms and records read metrics. |
| GraphQL additive contract | Pass | Storage health v1/additive tests and static fixture checks passed. |
| Swift read-client/UI static checks | Pass | Gate found P087 diagnostics fields and projection-lag tokens. |
| Restart projection rebuild | Fail | Startup code omits new P087 artifact/runtime projections; no P087 restart rebuild test found. |
| Duplicate maintenance loops | Risk | Two periodic reaper loops are started in the daemon. |
| Remote UI tests | Not run | Repo policy says UI tests are remote-only; P087 did not require a remote UI run for this audit. |
| Full regression suite | Not run | The canonical proposal gate passed, but `./scripts/test-gate.sh full` was not run. |
| Live daemon runtime | Not run | No live process or external MCP/GraphQL client session was started. |

## Verification Log

Commands and outcomes:

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py <proposal>`: returned `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria_IMPLEMENTATION_AUDIT_R5.md`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py <proposal>`: returned no prior proposal-review artifacts.
- `git status --short --branch`: target is `cw/implement-proposal-087-local-s/b4edcf82` with staged, unstaged, and untracked P087 implementation files.
- `./scripts/test-gate.sh proposal-087`: passed.
  - DB P087 tests: 19 passed.
  - MCP P087 tests: 15 passed.
  - Auth P087 tests: 2 passed.
  - GraphQL storage/P087 tests: passed.
  - P077/P088 compatibility checks included by the P087 gate: passed.
  - Static UI/schema/evidence/rollout fixture checks: passed.
- Focused code inspection:
  - Proposal lines 55-140, 185-335, 420-596.
  - P087 gate implementation in `scripts/test-gate.sh:7783-8004`.
  - Hot-read guard, MCP dispatch, GraphQL storage, storage health, projections, maintenance, startup recovery, and daemon composition code.
- Not run:
  - `./scripts/test-gate.sh full`
  - remote UI tests
  - live daemon restart with external MCP/GraphQL clients

## Final Verdict

P087 is substantially implemented, and the current `proposal-087` gate passes on the audited working tree. The implementation now covers compact storage tiers, projection-only `runs.list`, MCP hot-read liveness, GraphQL additive storage health, storage metrics, evidence fixtures, typed errors, and read-path budget tests.

It is not ready for final sign-off because the proposal explicitly required restart projection rebuild proof, and the current startup path does not rebuild the full P087 projection set. Fix REL-001 first, include that proof in `./scripts/test-gate.sh proposal-087`, and remove the duplicate periodic maintenance reaper loop from OPS-001 before treating P087 as implemented/ready.
