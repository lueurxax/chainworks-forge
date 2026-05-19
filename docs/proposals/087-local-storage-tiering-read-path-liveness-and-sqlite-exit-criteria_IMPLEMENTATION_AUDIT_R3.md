# Proposal 087 Implementation Audit R3

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Audit report | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria_IMPLEMENTATION_AUDIT_R3.md` |
| Audit date | 2026-05-16 |
| Mode | proposal-implementation-audit / auto |
| Implementation target | worktree `.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Branch | `cw/implement-proposal-087-local-s/b4edcf82` |
| HEAD | `76fa1d1594e0a81aedd08823054bbb11926ed697` |
| Compare base | Implicit: current target worktree contents |
| Worktree status | Dirty implementation worktree; audit is read-only except this report |
| Proposal state | Active |
| Overall conformance | Implemented |
| Overall implementation readiness | Ready with Risks |
| Reviewer selection reuse | Not reused |
| Audit confidence | High for Rust/MCP/GraphQL contracts covered by gate; Medium for long-running live-daemon liveness beyond in-process tests |

## Implementation Target

The audit was run against the requested implementation worktree, not `main`:

- root: `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82`
- branch: `cw/implement-proposal-087-local-s/b4edcf82`
- HEAD: `76fa1d1594e0a81aedd08823054bbb11926ed697`

The report path was selected with the bundled report-path helper and was free before writing.

## Prior Proposal-Review Reuse

Reviewer-selection reuse: Not reused.

`discover_prior_review.py` found no prior proposal-review artifacts for this proposal. Prior `IMPLEMENTATION_AUDIT` reports were not used for reviewer selection because the skill says to ignore implementation-audit reports for selection unless explicitly requested.

## Selected Reviewers

- `rust_arch_reviewer`: Rust control-plane storage, projection, migration, and MCP tool boundaries.
- `rust_reliability_reviewer`: hot-read circuit, maintenance operation, degraded storage, projection repair, and restart behavior.
- `rust_performance_reviewer`: `runs.list` and MCP liveness budget enforcement.
- `api_contract_reviewer`: MCP/GraphQL readback compatibility, additive schema shape, P077/P088 compatibility.
- `observability_rollout_reviewer`: metrics, storage health, rollout evidence, gate coverage, migration/readiness.

Rejected close alternatives:

- `rust_security_reviewer`: auth checks exist for storage tools, but the proposal is not primarily about auth, secrets, unsafe code, or public-network input handling.
- `apple_arch_reviewer`, `macos_ui_reviewer`, `apple_ux_reviewer`: Swift UI/read-model touches are present, but P087 is backend/storage/read-path dominant and the gate statically validates the visible projection-lag tokens.
- `product_reviewer`: decision checkpoint and metrics exist, but user-value/product experimentation is not the implementation risk center.

## Proposal Contract Summary

P087 commits to a local-storage tiering boundary: SQLite owns compact canonical state, metadata, cursors, compact projections, and health snapshots, while high-volume evidence, logs, transcripts, tool traces, large runtime payloads, and compaction bundles stay in the file store (proposal lines 143-181).

The read-path contract is explicit: `runs.list` must be projection-only and must not perform N+1 artifact/report attachment, filesystem scans, transcript reads, compaction archive inspection, side-effect readback, or implementation self-assessment attachment unless pre-materialized in the projection (proposal lines 185-205).

The required hot projection set covers `ActiveRunIndex`, `ApprovalInboxProjection`, `RuntimeHealthProjection`, `StorageHealthProjection`, and `ArtifactNoiseProjection` with compact fields for runtime health, storage health, side-effect unresolved count, continuation activity, and artifact noise/compaction readiness (proposal lines 239-329).

The MCP liveness gate must cover `initialize`, `tools/list`, `runs.list`, `runtime.health`, `storage.health`, and one simple resource/artifact metadata read within configured budgets even when DbWriter is degraded, evidence spool has orphans, maintenance is running, or prior long work exists (proposal lines 422-444).

Storage health and diagnostics must expose DbWriter, queue depth, write wait, transaction duration, WAL, checkpoint, evidence spool, projection lag, hot cache rebuild duration, `runs.list` latency, and MCP liveness duration metrics (proposal lines 448-467). The `proposal-087` gate must fail on liveness/SLO regressions, high-volume SQLite rows, and `runs.list` detail attachments (proposal lines 544-581).

## Platform / Product Scope

- Apple scope: macOS read-model/view affordance only, not a UI feature implementation.
- Backend/service scope: Rust control-plane data, MCP, GraphQL, persistence, health, and rollout gate.
- Data scope: SQLite migrations, compact projections, evidence-spool metadata, hot-read circuits, repair slots, and metrics.
- Rollout scope: proposal-specific gate plus evidence fixtures.

## Primary Service Flows Audited

1. Operator/MCP caller lists active runs through `runs.list` and receives only compact projection-backed fields.
2. GraphQL caller queries `runs` and receives projection rows without per-run detail enrichment.
3. MCP caller performs the liveness sequence while storage is degraded or maintenance is running.
4. Storage diagnostics expose writer/WAL/spool/projection/guard/metric readbacks without blocking read paths.
5. Projection rebuild and dependent proposal compatibility keep P077 closeout and P088 implementation-completion summaries available without reintroducing hot-list detail loaders.

## Fidelity Inventory

### Matches

- `runs.list` now directly returns `projections::list_active_projection` with no list-time detail attachment path (`control-plane/crates/mcp-server/src/tools/runs.rs:379-382`).
- `RunProjectionRow` includes projection-backed compact P087/P088 and P087/P077 list readbacks (`control-plane/crates/db/src/repos/projections.rs:158-164`).
- `list_active_projection` selects and parses `implementation_completion_json` and `closeout_readiness_summary_json` from `run_summaries` (`control-plane/crates/db/src/repos/projections.rs:198-270`).
- Projection readback summaries are refreshed on projection rebuild/write paths (`control-plane/crates/db/src/repos/projections.rs:584-624`).
- GraphQL `runs` maps projection rows directly through `GqlRun::from` (`control-plane/crates/graphql-server/src/schema.rs:257-266`, `control-plane/crates/graphql-server/src/types/run.rs:164-225`).
- MCP `runtime.health` exposes the required runtime-health fields (`control-plane/crates/mcp-server/src/tools/runtime.rs:19-99`).
- Storage health exposes projection freshness, hot-read guards, read-path metrics, and artifact-noise projection readback (`control-plane/crates/db/src/repos/storage_health.rs:300-318`, `control-plane/crates/db/src/repos/storage_health.rs:394-412`).
- Production tool dispatch records hot-read latency and runtime-health liveness duration (`control-plane/crates/mcp-server/src/server.rs:844-855`; metrics in `control-plane/crates/db/src/metrics.rs:100-120`).
- The proposal gate includes DB, MCP, P077, P088, auth, GraphQL storage-health, GraphQL P087, and static evidence/schema checks (`scripts/test-gate.sh:7818-8002`).

### Divergences

- `ArtifactNoiseProjection.supersededCount` is implemented as duplicate artifact-name excess, not as the generation-level supersession relation already exposed elsewhere (`control-plane/crates/db/src/repos/storage_health.rs:555-624`; `control-plane/crates/db/src/repos/projections.rs:848-893`). This is not enough to fail the proposal conformance roll-up because P087 names the field but does not define its source of truth, but it is a real dependent-proposal risk for P038 compaction semantics.

### Ambiguities / Evidence Gaps

- The MCP liveness proof is strong in the in-process MCP server tests and gate, but this audit did not run a live daemon with an actually in-flight long-running tool request. The implementation reports `singleRequestSerialized: false` and has per-tool timeout/circuit protection, but live transport saturation remains outside the executed evidence.
- The artifact-noise readback is shaped like a projection and is bounded by current gate coverage, but it is computed from live compact artifact metadata in `storage_health_with_writer`; there is no separate materialized artifact-noise table/cursor. The proposal allows in-memory/cache approaches but uses the word "projection", so this remains a naming/operational interpretation risk rather than a blocker.

## Requirement Summary

| Requirement | Status | Evidence |
| --- | --- | --- |
| REQ-001 SQLite owns compact canonical state/metadata only; high-volume evidence stays file-backed | Implemented | migration, code, gate |
| REQ-002 `runs.list` is projection-only and avoids deep enrichment/N+1/file reads | Implemented | code, tests-run |
| REQ-003 GraphQL hot reads use projection/cache surfaces, not per-row deep scans | Implemented | code, tests-run |
| REQ-004 Required hot projection/readback set is exposed | Implemented | code, tests-run, schema |
| REQ-005 MCP liveness sequence remains bounded under degraded/maintenance conditions | Implemented | code, tests-run |
| REQ-006 Storage health and read-path metrics expose required diagnostics | Implemented | code, telemetry, tests-run |
| REQ-007 `proposal-087` gate enforces tests, static checks, fixtures, and compatibility regressions | Implemented | config, tests-run |
| REQ-008 Dependent contracts can rely on P087 without reintroducing SQLite/read pressure | Implemented with bounded risk | code, tests-run, specialist finding API-001 |

## Detailed REQ Audit

### REQ-001: Storage tiering boundary

- Proposal source: lines 143-181.
- Status: Implemented.
- Evidence types: migration, code, tests-run, config.
- Implementation mapping:
  - Migration `056_p087_storage_tiering_projections.sql` adds compact invalidation cursors, repair slots, storage snapshots, hot-read circuit state, and compact run-list readback columns (`control-plane/crates/db/migrations/056_p087_storage_tiering_projections.sql:1-70`).
  - Evidence spooling remains the file-backed owner for large runtime evidence, with compact storage-health summaries surfaced through existing P075/P087 paths.
  - The gate includes negative rollout fixtures and static checks for P087 evidence/schema completeness (`scripts/test-gate.sh:7882-8002`).
- Gap / note: No blocker found.

### REQ-002: MCP `runs.list` projection-only contract

- Proposal source: lines 185-205.
- Status: Implemented.
- Evidence types: code, tests-found, tests-run.
- Implementation mapping:
  - `runs.list` calls `projections::list_active_projection` and serializes those rows directly (`control-plane/crates/mcp-server/src/tools/runs.rs:379-382`).
  - Heavy detail fields remain forbidden in the P087 test while compact P088 compatibility is allowed via the projection (`control-plane/crates/mcp-server/src/tools/runs.rs:1487-1525`).
  - The projection row carries `implementationCompletion`, `closeout_readiness_summary`, and the documented closeout alias from compact `run_summaries` JSON columns (`control-plane/crates/db/src/repos/projections.rs:158-164`, `control-plane/crates/db/src/repos/projections.rs:198-270`).
- Gap / note: No blocker found.

### REQ-003: GraphQL hot read projection-only contract

- Proposal source: lines 585-596, especially acceptance item 5.
- Status: Implemented.
- Evidence types: code, tests-run.
- Implementation mapping:
  - GraphQL `runs` uses `list_active_projection` / `list_by_idea_projection`, then maps projection rows into `GqlRun` (`control-plane/crates/graphql-server/src/schema.rs:257-266`).
  - `GqlRun::from(RunProjectionRow)` leaves detail-heavy fields null/empty for list reads while mapping compact projection fields (`control-plane/crates/graphql-server/src/types/run.rs:164-225`).
  - The P087 GraphQL test proves list reads do not attach implementation self-assessment, rollout, side-effect, or closeout detail readbacks per row (`control-plane/crates/graphql-server/src/schema.rs:2878-2937`).
- Gap / note: No blocker found.

### REQ-004: Required hot projection/readback set

- Proposal source: lines 239-329.
- Status: Implemented.
- Evidence types: code, schema, tests-run.
- Implementation mapping:
  - Active run readback is `RunProjectionRow` and `list_active_projection` (`control-plane/crates/db/src/repos/projections.rs:140-164`, `control-plane/crates/db/src/repos/projections.rs:198-270`).
  - Storage health reads projection freshness, hot-read guards, maintenance operations, read-path metrics, and artifact-noise readback (`control-plane/crates/db/src/repos/storage_health.rs:300-318`, `control-plane/crates/db/src/repos/storage_health.rs:394-412`).
  - Runtime health exposes active sessions, degraded flags, write pressure flags, side-effect unresolved count, and continuation active count (`control-plane/crates/mcp-server/src/tools/runtime.rs:74-90`).
  - Artifact noise exposes artifact count, superseded count, duplicate candidate count, archive-eligible count, and compaction recommended flag (`control-plane/crates/db/src/repos/storage_health.rs:555-624`).
- Gap / note: See API-001 for the bounded semantic risk in `supersededCount`.

### REQ-005: MCP liveness gate and bounded read budgets

- Proposal source: lines 422-444.
- Status: Implemented.
- Evidence types: code, tests-run, telemetry.
- Implementation mapping:
  - Tool dispatch applies P087 probe/read timeouts and records per-tool latency (`control-plane/crates/mcp-server/src/server.rs:844-855`).
  - The liveness test exercises `initialize`, `tools/list`, `runs.list`, `runtime.health`, `storage.health`, and `resources/read` with a running maintenance operation and asserts read-path metrics are recorded (`control-plane/crates/mcp-server/src/server.rs:2539-2661`).
  - The seeded load test runs `runs.list` against 250 active runs and enforces the 500 ms budget with p95 metric recording (`control-plane/crates/mcp-server/src/server.rs:2664-2699`).
- Gap / note: Live-daemon prior-long-operation saturation was not separately executed outside the in-process gate.

### REQ-006: Storage health and metrics

- Proposal source: lines 448-467.
- Status: Implemented.
- Evidence types: code, telemetry, tests-run.
- Implementation mapping:
  - `storage_health_with_writer` includes DbWriter health, WAL, projections, projection freshness, hot-read guards, read-path metrics, artifact noise, maintenance operations, evidence spool, write pressure, and rollout (`control-plane/crates/db/src/repos/storage_health.rs:300-425`).
  - Metrics include `runs_list_read_latency_ms` and `mcp_liveness_gate_duration_ms` recording (`control-plane/crates/db/src/metrics.rs:100-120`).
  - Production MCP tool dispatch records `runtime.health` liveness duration (`control-plane/crates/mcp-server/src/server.rs:844-855`).
- Gap / note: No blocker found.

### REQ-007: Canonical P087 gate and evidence enforcement

- Proposal source: lines 544-581.
- Status: Implemented.
- Evidence types: config, tests-run.
- Implementation mapping:
  - `scripts/test-gate.sh proposal-087` verifies migration version uniqueness, DB P087 tests, MCP P087 tests, P077 closeout list compatibility, P088 implementation-completion list compatibility, auth tests, GraphQL storage-health preservation, GraphQL P087 tests, Swift read-model tokens, schema fixtures, rollout metrics, tool registry, and negative fixtures (`scripts/test-gate.sh:7788-8002`).
  - The gate rejects zero-test cargo filters (`scripts/test-gate.sh:7801-7815`).
- Tests run:
  - `./scripts/test-gate.sh proposal-087`
  - Result: PASS on HEAD `76fa1d1594e0a81aedd08823054bbb11926ed697`.
  - Key covered results: 14 DB P087 tests passed, 12 MCP P087 tests passed, P077 closeout compatibility test passed, P088 implementation-completion compatibility test passed, 2 auth P087 tests passed, GraphQL storage/P087 tests passed, static UI/schema/evidence checks passed.
- Gap / note: No blocker found.

### REQ-008: Dependent proposal compatibility and SQLite pressure

- Proposal source: lines 585-596, especially item 8.
- Status: Implemented with bounded risk.
- Evidence types: code, tests-run.
- Implementation mapping:
  - P077/P088 compatibility is preserved through compact projection-backed readbacks, not list-time detail loaders (`control-plane/crates/db/src/repos/projections.rs:158-164`, `control-plane/crates/db/src/repos/projections.rs:584-624`).
  - The P087 gate explicitly runs the P077 and P088 focused compatibility tests (`scripts/test-gate.sh:7818-7824`).
  - Runtime health exposes side-effect unresolved count for dependent side-effect ledger visibility (`control-plane/crates/mcp-server/src/tools/runtime.rs:53-90`).
- Gap / note: P038 compaction-readiness consumers should not rely on duplicate-name-derived `supersededCount` as if it were generation-level supersession until API-001 is addressed or documented.

## Reviewer / Lens Scorecard

| Lens | Status | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Implemented | None blocking | High |
| Rust architecture | Pass with risk | Artifact-noise projection semantics are not the same as generation supersession | High |
| Rust reliability | Pass | Live long-operation MCP saturation not separately executed | Medium |
| Rust performance | Pass | Budget evidence is in-process, not live-daemon benchmark | Medium-High |
| API contract | Pass with risk | `supersededCount` meaning may drift for P038 callers | Medium-High |
| Observability/rollout | Pass | Gate is strong; no full repo gate was run | High for proposal scope |
| Readiness | Ready with Risks | Bounded API/P038 risk remains | High |

## Routed Specialist Findings

### API-001: `ArtifactNoiseProjection.supersededCount` is a duplicate-name proxy, not generation supersession

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related proposal items: REQ-004, REQ-008; proposal lines 316-329 and 585-596.
- Evidence types: code, schema.
- Evidence references:
  - Artifact-noise query computes `superseded_count` as `SUM(duplicate_count - 1)` over duplicate artifact names (`control-plane/crates/db/src/repos/storage_health.rs:555-624`).
  - The existing artifact projection already exposes generation-level supersession via `supersedes_artifact_generation_id` (`control-plane/crates/db/src/repos/projections.rs:848-893`).
  - Artifact contract readback also exposes `supersedes_generation_id` as `supersedes_artifact_generation_id` / `supersedes` (`control-plane/crates/db/src/repos/artifact_contracts.rs:1372-1379`).
- Why it matters:
  - P087 says `ArtifactNoiseProjection` is used for P038 compaction readiness and must contain a `superseded count`. If downstream consumers interpret that as actual generation supersession, duplicate-name excess can overcount or undercount. Same-name artifacts are not necessarily superseded generations; superseded generations can also be represented by contract metadata that the current query ignores.
- Recommended action:
  - Either compute `supersededCount` from `artifact_contract_generations.supersedes_generation_id` / active generation state, or explicitly rename/document the field as a duplicate-name estimate and add a separate generation-level `supersededGenerationCount`.
- Acceptance criteria:
  - Add a test that creates one duplicate-name non-superseding artifact and one actual superseding generation, then proves `ArtifactNoiseProjection` reports the intended counts.
  - Update the P087/P038 reference docs or GraphQL/MCP schema fixture to define the field semantics.

### PERF-001: Liveness evidence is strong for the in-process server but not a live daemon with an active long-running request

- Reviewer: `rust_performance_reviewer`
- Severity: Minor
- Confidence: Medium
- Related proposal items: REQ-005; proposal lines 422-444.
- Evidence types: tests-run, code.
- Evidence references:
  - The liveness test covers the required request sequence with a running maintenance operation and metric recording (`control-plane/crates/mcp-server/src/server.rs:2539-2661`).
  - Tool dispatch enforces P087 budgets and records latency (`control-plane/crates/mcp-server/src/server.rs:844-855`).
  - `runtime.health` reports `singleRequestSerialized: false` (`control-plane/crates/mcp-server/src/tools/runtime.rs:91-94`).
- Why it matters:
  - The proposal explicitly mentions a prior long operation still being in progress. The gate models this through maintenance state and the in-process server, but does not start a live daemon and hold an actual long-running MCP request while the liveness sequence runs.
- Recommended action:
  - Add a future smoke test that starts the daemon, initiates or simulates a long-running maintenance tool, then executes the liveness sequence over the same transport.
- Acceptance criteria:
  - Live daemon liveness check records `mcp_liveness_gate_duration_ms` and proves `initialize`, `tools/list`, `runs.list`, `runtime.health`, `storage.health`, and `resources/read` stay within configured budgets while a long operation is active.

## Tests And Validation Run

Executed from:

`/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82`

Command:

```bash
./scripts/test-gate.sh proposal-087
```

Result: PASS.

Observed gate coverage:

- P087 DB migration versions verified.
- `-p db proposal_087`: 14 tests passed.
- `-p mcp-server proposal_087`: 12 tests passed.
- `-p mcp-server runs_get_and_list_expose_p077_documented_and_legacy_closeout_summary_names`: 1 test passed.
- `-p mcp-server proposal_088_mcp_runs_get_and_list_expose_implementation_completion`: 1 test passed.
- `-p auth proposal_087`: 2 tests passed.
- `-p graphql-server --lib storage_health_v1`: 1 test passed.
- `-p graphql-server --lib proposal_087`: 3 tests passed.
- Static UI, schema, rollout, metric, tool-registry, and negative fixture checks passed.

Warnings observed were existing compiler warnings about unused/dead code in `acp`, `db`, and `engine`; they did not fail the gate.

## Final Verdict

Overall conformance: Implemented.

Overall implementation readiness: Ready with Risks.

The prior blockers around P077/P088 list compatibility and missing runtime/storage projection readbacks are addressed in the audited tree, and the canonical `proposal-087` gate passes on the same HEAD. The implementation is ready to hand off for proposal closeout or merge-readiness review, with one bounded follow-up risk: define and test `ArtifactNoiseProjection.supersededCount` semantics before allowing P038 compaction logic to treat it as generation-level supersession truth.
