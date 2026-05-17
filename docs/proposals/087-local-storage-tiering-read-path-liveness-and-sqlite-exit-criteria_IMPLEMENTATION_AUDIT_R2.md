# Proposal 087 Implementation Audit R2

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Proposal title | Local Storage Tiering, Read-Path Liveness, and SQLite Exit Criteria |
| Proposal state | Active proposal file, `Status: Draft` |
| Audit timestamp | 2026-05-15 |
| Audit mode | `proposal-implementation-audit` auto mode |
| Implementation target | Worktree `.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Branch | `cw/implement-proposal-087-local-s/b4edcf82` |
| HEAD | `76fa1d1594e0a81aedd08823054bbb11926ed697` |
| Compare base | Implicit, not provided by user |
| Working tree | Dirty implementation tree with staged, unstaged, and untracked P087 files; this R2 report is the only audit file added by this audit turn |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Audit confidence | High for inspected MCP/GraphQL/read-path behavior and executed tests; medium for production runtime telemetry because no live daemon was exercised |

## Prior Review Reuse

Reviewer-selection reuse: Not reused.

The bundled prior-review discovery helper returned no prior proposal-review artifacts for P087. Existing implementation-audit reports were ignored for reviewer selection per the skill contract.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | Projection ownership, SQLite/file-spool boundaries, and hot-read model shape |
| `rust_reliability_reviewer` | MCP liveness, read timeouts, maintenance/degraded behavior, circuit behavior |
| `rust_performance_reviewer` | `runs.list` hot-path latency, N+1 elimination, read-budget evidence |
| `api_contract_reviewer` | MCP `runs.list`, `tools/list`, `resources/read`, GraphQL list/detail contract, compatibility with prior readbacks |
| `observability_rollout_reviewer` | Canonical gate, metrics, exit criteria, rollout health readback |

Rejected close alternatives:

- `rust_security_reviewer`: storage tools and runtime health touch capabilities, but no new public trust boundary or secret handling dominated this P087 slice.
- `macos_ui_reviewer` / `apple_ux_reviewer`: Swift changes are additive diagnostic/read-state presentation; the P087 blocker surface is Rust/API/storage.
- `product_reviewer`: storage exit decisions matter, but the concrete risk is instrumentation/gate enforcement rather than product metric design.

## Proposal Contract Summary

P087 formalizes local Plan A:

- SQLite owns compact canonical state, metadata, projection cursors, storage health snapshots, and compact read models.
- High-volume evidence, raw runtime streams, transcripts, tool traces, and large reports stay in the filesystem/spool.
- Hot reads use materialized projections, in-memory caches, cached summaries, or precomputed health snapshots.
- `runs.list` must be projection-only and must not perform N+1 detail attachment or implementation self-assessment attachment unless pre-materialized.
- MCP liveness must cover `initialize`, `tools/list`, `runs.list`, `runtime.health`, `storage.health`, and one simple resource/artifact metadata read within budgets while degraded/maintenance work is present.
- Required hot projections include `ActiveRunIndex`, `ApprovalInboxProjection`, `RuntimeHealthProjection`, `StorageHealthProjection`, and `ArtifactNoiseProjection`.
- Storage health/diagnostics must expose write pressure, WAL, spool, projection lag, read latency, and MCP liveness gate duration.
- The canonical `proposal-087` gate must enforce liveness/read SLOs, no high-volume SQLite rows, and no `runs.list` detail attachments.

Platform/product scope:

- Apple: macOS read-state presentation only.
- Backend/service: Rust MCP, GraphQL, SQLite, file-spool, projection, metrics, rollout.
- Product: storage exit criteria and operator diagnostic confidence.

## Primary Service Flows

1. MCP operator calls `runs.list` and receives compact active-run projection rows without per-run detail loaders.
2. MCP liveness path runs `initialize`, `tools/list`, `runs.list`, `runtime.health`, `storage.health`, and a metadata/resource read during degraded/maintenance conditions.
3. GraphQL active runs list reads projections without per-row self-assessment/completion/closeout enrichment.
4. Storage health reports write pressure, WAL/spool/projection state, hot-read guard state, and read-path metrics.
5. Dependent proposal readbacks remain compatible while P087 moves heavy detail out of hot list paths.

## Fidelity Inventory

Matches:

- MCP `runs.list` now directly serializes `projections::list_active_projection` without the previous per-row attachment loop (`control-plane/crates/mcp-server/src/tools/runs.rs:379-382`).
- GraphQL `runs` now returns projection rows directly for both active and idea-scoped lists (`control-plane/crates/graphql-server/src/schema.rs:257-266`).
- JSON-RPC `tools/list` and `resources/read` are now wrapped by `handle_hot_read_json_rpc` and governed as `tools.list` / `resources.read` surfaces (`control-plane/crates/mcp-server/src/server.rs:321-337`, `440-466`, `493-537`).
- `runtime.health` is registered and guarded as a hot-read MCP tool (`control-plane/crates/mcp-server/src/tools/runtime.rs:8-39`, `control-plane/crates/mcp-server/src/hot_read_guard.rs:81-90`).
- `proposal-087` gate passed on this audited tree, including new MCP liveness and projection-only tests.

Divergences:

- P087 is satisfied by dropping rich `runs.list` fields rather than pre-materializing list-safe summaries; this regresses P077/P088 `runs.list` readback contracts and tests.
- `runtime.health` exists, but it is not the `RuntimeHealthProjection` required by P087 section 7.3: it lacks runtime family, active sessions, degraded flags, write pressure flags, side-effect unresolved count, and continuation active count.
- `ArtifactNoiseProjection` required for P038 compaction readiness / inspectability warnings is still not present.
- `mcp_liveness_gate_duration_ms` is exposed and exercised by tests, but the only recorder call found is in a test harness, not a production/server liveness path.

Ambiguities / evidence gaps:

- No live daemon was started; liveness evidence is in-process MCP server tests rather than an HTTP/stdio daemon runtime trace.
- No p95 benchmark or seeded large-run latency test proves `runs.list` under the 500 ms review threshold.
- The proposal does not explicitly resolve the conflict with earlier P077/P088 list-field contracts, so implementation must either pre-materialize those fields or update/supersede the dependent references and tests.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | SQLite compact ownership and no raw high-volume stream tables | Implemented |
| REQ-002 | High-volume evidence is file-backed and list metadata uses pointers | Implemented |
| REQ-003 | MCP `runs.list` is projection-only with no detail attachments | Implemented |
| REQ-004 | GraphQL active runs list is projection-only/cache-backed | Implemented |
| REQ-005 | MCP liveness sequence covers required surfaces under maintenance/degraded conditions | Implemented |
| REQ-006 | Required hot projection set exists, including runtime health and artifact noise | Partially Implemented |
| REQ-007 | Storage health exposes required write/read/projection/spool metrics | Partially Implemented |
| REQ-008 | Canonical gate enforces P087 read-path/liveness requirements | Partially Implemented |
| REQ-009 | Dependent proposal/storage contracts can rely on the tiering change without regressions | Partially Implemented |

## Detailed Requirement Audit

### REQ-001 - SQLite Compact Ownership

- Proposal source: sections 3.1, 5.1, 5.2, 18.1 (`docs/proposals/087...md:67-85`, `143-169`, `589`).
- Status: Implemented.
- Evidence types: migration, code, tests-run.
- Evidence references:
  - P087 migrations add projection invalidation/cursor, maintenance, storage-health snapshot, and hot-read circuit tables.
  - `./scripts/test-gate.sh proposal-087` verified unique migration versions and passed.
- Mapping: new DB shape is compact coordination/readback state, not raw stream/event storage.
- Gap/note: no issue found for this requirement.

### REQ-002 - File-Backed Evidence and Metadata Pointers

- Proposal source: sections 3.2, 5.3, 18.2 (`docs/proposals/087...md:86-109`, `171-181`, `590`).
- Status: Implemented.
- Evidence types: code, schema, tests-run.
- Evidence references:
  - MCP artifact resource read exposes `artifact_metadata_pointer.v1` and redacts payload path/raw content.
  - P087 gate checks artifact pointer fixtures and rejects MCP metadata leaking `file_path` (`scripts/test-gate.sh:7871-7878`, `7908-7915`).
- Mapping: hot metadata/readback uses pointer shape rather than payload bytes.
- Gap/note: this audit did not re-review every non-hot report payload path.

### REQ-003 - MCP `runs.list` Projection-Only

- Proposal source: sections 6.1, 16 phase 2, 17, 18.3 (`docs/proposals/087...md:187-199`, `531-549`, `564-565`, `591`).
- Status: Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - MCP `runs.list` now calls `projections::list_active_projection` and returns `serde_json::to_value(items)` directly (`control-plane/crates/mcp-server/src/tools/runs.rs:379-382`).
  - New P087 test asserts forbidden detail fields are absent from `runs.list` (`control-plane/crates/mcp-server/src/tools/runs.rs:1487-1525`).
  - `proposal-087` gate ran this test and passed.
- Mapping: the prior N+1 attachment loop is gone from MCP list.
- Gap/note: p95 performance is still asserted indirectly through bounded tests, not a load benchmark.

### REQ-004 - GraphQL Active Runs Projection-Only

- Proposal source: sections 6.3, 18.5 (`docs/proposals/087...md:220-235`, `593`).
- Status: Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - GraphQL `runs` no longer calls `runs_with_latest_summaries`; it maps projection rows directly (`control-plane/crates/graphql-server/src/schema.rs:257-266`).
  - The removed per-row enrichment loop is visible in the unstaged diff.
  - New GraphQL test asserts list query does not populate per-row implementation/rollout/side-effect/closeout fields (`control-plane/crates/graphql-server/src/schema.rs:2874-2937`).
  - `proposal-087` gate ran the GraphQL P087 test and passed.
- Mapping: active run list no longer deep-enriches each row.
- Gap/note: detail query enrichment remains available via `run`, which is consistent with the proposal's list/detail split.

### REQ-005 - MCP Liveness Sequence

- Proposal source: section 13 (`docs/proposals/087...md:422-444`).
- Status: Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - `tools/list` is now guarded through `handle_hot_read_json_rpc` as `tools.list` (`control-plane/crates/mcp-server/src/server.rs:321-337`).
  - `resources/read` is guarded through the same wrapper as `resources.read` (`control-plane/crates/mcp-server/src/server.rs:440-466`).
  - Hot-read tool set includes `runs.list`, `tools.list`, `resources.read`, `runtime.health`, and `storage.health` (`control-plane/crates/mcp-server/src/hot_read_guard.rs:81-90`).
  - New tests cover the liveness sequence while a maintenance row is running, and open-circuit denial for `tools/list` and `resources/read` (`control-plane/crates/mcp-server/src/server.rs:2545-2697`).
  - `proposal-087` gate ran and passed those tests.
- Mapping: the R1 gap around JSON-RPC methods is closed at code and test level.
- Gap/note: this is not a live daemon/HTTP/stdio runtime trace.

### REQ-006 - Required Hot Projection Set

- Proposal source: section 7 (`docs/proposals/087...md:239-329`).
- Status: Partially Implemented.
- Evidence types: code, search, tests-found.
- Evidence references:
  - Active run projection and approval inbox projection exist.
  - Storage health projection/readback exists with freshness and hot-read state.
  - `runtime.health` tool exists but returns only status, request-loop availability, and hot-read circuit count (`control-plane/crates/mcp-server/src/tools/runtime.rs:19-39`), not the required runtime-family/session/degraded/write-pressure/side-effect/continuation projection fields (`docs/proposals/087...md:281-296`).
  - Search found no `ArtifactNoiseProjection` or equivalent projection with artifact count, superseded count, duplicate candidates, archive eligibility, and compaction recommendation (`docs/proposals/087...md:316-329`).
- Mapping: three of the required projection families are covered; runtime health and artifact noise remain incomplete.
- Gap/note: this is a proposal-conformance gap, not just a future enhancement.

### REQ-007 - Storage Health and Metrics

- Proposal source: section 14 (`docs/proposals/087...md:448-467`).
- Status: Partially Implemented.
- Evidence types: code, telemetry, tests-run.
- Evidence references:
  - Storage health now exposes `readPathMetrics.runsListReadLatencyP95Ms` and `readPathMetrics.mcpLivenessGateDurationP95Ms` (`control-plane/crates/db/src/repos/storage_health.rs:404-407`).
  - `runs.list` latency is recorded by the hot-read wrapper and copied into the explicit metric key (`control-plane/crates/db/src/metrics.rs:99-110`).
  - `mcp_liveness_gate_duration_ms` has a recorder/getter (`control-plane/crates/db/src/metrics.rs:113-139`), but the only call found during audit is in the P087 test harness (`control-plane/crates/mcp-server/src/server.rs:2613`).
- Mapping: fields exist and tests can populate them; production liveness-gate duration recording is not proven.
- Gap/note: storage exit decisions need real operational samples, not only test-local metrics.

### REQ-008 - Canonical Gate Enforcement

- Proposal source: sections 16 phase 4, 17, 18.7 (`docs/proposals/087...md:544-581`, `595`).
- Status: Partially Implemented.
- Evidence types: tests-run, code.
- Evidence references:
  - `./scripts/test-gate.sh proposal-087` passed on the audited tree.
  - The gate now runs P087 tests covering MCP liveness, JSON-RPC guard surfaces, and projection-only MCP/GraphQL list behavior.
  - The gate still does not catch sibling readback regressions from P077/P088; focused tests for those proposals fail, as recorded below.
- Mapping: P087-specific enforcement is much stronger than R1, but not sufficient for repository readiness.
- Gap/note: this is a readiness blocker because this proposal is changing shared `runs.list` contract semantics.

### REQ-009 - Dependent Proposal and Storage Contract Safety

- Proposal source: sections 10-12 and acceptance criteria 8 (`docs/proposals/087...md:379-418`, `596`).
- Status: Partially Implemented.
- Evidence types: code, tests-run.
- Evidence references:
  - P087 explicitly names P038/P086/P078, and the side-effect detail no longer rides MCP/GraphQL list paths.
  - However, the implementation breaks existing documented list readbacks from P077 and P088, which occupy the same hot-list/detail boundary.
  - P077 requires GraphQL and MCP expose closeout readiness fields for `runs.get` and `runs.list` (`docs/proposals/077...md:614`).
  - P088 requires `implementationCompletion` in run report, MCP `runs.get`/`runs.list`, and GraphQL readback (`docs/proposals/088...md:830`).
  - Focused P077 and P088 tests fail after the P087 list change.
- Mapping: P087's pressure goal is achieved by deleting list details rather than preserving dependent list contracts through projections/cached summaries.
- Gap/note: this is the highest-risk gap in R2.

## Reviewer Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Missing/incomplete required runtime-health and artifact-noise projections | High |
| Rust architecture | Partial | Projection families are uneven and `runtime.health` is a tool, not the specified projection | High |
| Rust reliability | Mostly pass | MCP JSON-RPC liveness gaps from R1 are closed in in-process tests | High |
| Rust performance | Mostly pass | N+1 list enrichments are removed; load/p95 evidence remains absent | Medium |
| API contract | Not Ready | P087 list fix regresses P077/P088 `runs.list` contracts and tests | High |
| Observability/rollout | Partial | P087 gate passes but production liveness-duration metric is not proven | Medium |
| Overall readiness | Not Ready | Focused non-P087 regression tests fail | High |

## Routed Specialist Findings

### API-001 - P087 Removes `runs.list` Fields Required by P077 and P088

- Reviewer: `api_contract_reviewer`
- Severity: Critical
- Confidence: High
- Related requirements: REQ-008, REQ-009
- Evidence types: proposal, code, tests-run.
- Evidence references:
  - P087 forbids list detail attachment unless pre-materialized (`docs/proposals/087...md:187-199`).
  - MCP `runs.list` now returns only projection rows (`control-plane/crates/mcp-server/src/tools/runs.rs:379-382`).
  - P077 still requires CloseoutReadinessSummaryAccessor fields for `runs.get` and `runs.list` (`docs/proposals/077...md:614`), and its focused test expects those fields on list (`control-plane/crates/mcp-server/tests/proposal_077_closeout_readback_parity.rs:157-207`).
  - P088 still requires `implementationCompletion` in MCP `runs.get`/`runs.list` and GraphQL readback (`docs/proposals/088...md:830`), and its focused test expects list fields (`control-plane/crates/mcp-server/tests/proposal_088_code_writer_completion_readback.rs:503-558`).
  - `cargo test -p mcp-server runs_get_and_list_expose_p077_documented_and_legacy_closeout_summary_names -- --nocapture` failed: list closeout summary was `Null`.
  - `cargo test -p mcp-server proposal_088_mcp_runs_get_and_list_expose_implementation_completion -- --nocapture` failed: list `implementationCompletion.ingestion_boundary_failure` was `Null`.
- Why it matters: P087 closes the hot-path pressure issue by changing public readback behavior instead of preserving list-safe summaries through projections. That makes the P087 branch non-mergeable against current repository truth even though the P087 gate is green.
- Recommended action: either pre-materialize the P077/P088 compact summaries into the active run projection/read model, or explicitly supersede/update the P077/P088 contracts, reference docs, and tests. Do not leave P087 green while sibling proposal tests are red.
- Acceptance criteria: P087 gate plus focused P077/P088 readback tests pass on the same tree, and `runs.list` remains projection-only by sourcing these fields from projection/cached summary rows only.

### ARCH-001 - Required Runtime and Artifact-Noise Projections Are Still Incomplete

- Reviewer: `rust_arch_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-006
- Evidence types: proposal, code, search.
- Evidence references:
  - P087 requires `RuntimeHealthProjection` fields for runtime family, active sessions, degraded flags, write pressure flags, side-effect unresolved count, and continuation active count (`docs/proposals/087...md:281-296`).
  - Current `runtime.health` returns `schemaVersion`, `status`, request-loop availability, and hot-read circuit count only (`control-plane/crates/mcp-server/src/tools/runtime.rs:19-39`).
  - P087 requires `ArtifactNoiseProjection` for artifact count, superseded count, duplicate candidates, archive eligibility, and compaction recommendation (`docs/proposals/087...md:316-329`).
  - Searches did not find an implementation matching `ArtifactNoiseProjection` or equivalent fields.
- Why it matters: P087 is not only a liveness wrapper. It defines a durable projection contract for runtime health and compaction/readability pressure. Without those projections, P038/P086/P078-style work can still drift back into ad hoc detail reads.
- Recommended action: add explicit projection/read-model rows for runtime health and artifact noise, or document an existing one-to-one equivalent with tests proving required fields and rebuild/freshness behavior.
- Acceptance criteria: storage/reference docs and tests show all five required P087 projection families with named fields or exact equivalents.

### OPS-001 - `mcp_liveness_gate_duration_ms` Is Test-Populated, Not Operationally Proven

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-007, REQ-008
- Evidence types: code, telemetry, tests-run.
- Evidence references:
  - P087 requires MCP liveness gate duration as a storage health metric (`docs/proposals/087...md:448-467`).
  - Metric storage/getter exists (`control-plane/crates/db/src/metrics.rs:113-139`) and storage health exposes it (`control-plane/crates/db/src/repos/storage_health.rs:404-407`).
  - The only `record_mcp_liveness_gate_duration` call found is inside the P087 unit/integration test sequence (`control-plane/crates/mcp-server/src/server.rs:2613`).
- Why it matters: exit criteria require operational signal from real runs. A metric that appears only during tests cannot tell operators whether normal MCP liveness is degrading.
- Recommended action: record liveness gate duration from a real health probe, startup self-check, scheduled daemon monitor, or explicit operator diagnostic command, then expose freshness/source in storage health.
- Acceptance criteria: after a daemon/server liveness probe runs outside tests, storage health shows non-null liveness duration with source/freshness metadata.

### PERF-001 - `runs.list` p95 Budget Is Not Proved Under Load

- Reviewer: `rust_performance_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-003, REQ-007, REQ-008
- Evidence types: proposal, tests-run.
- Evidence references:
  - P087 says `runs.list` p95 > 500 ms after projection-only implementation opens storage review (`docs/proposals/087...md:483`).
  - Current tests prove no detail attachments and bounded in-process liveness sequence, but no seeded multi-run latency benchmark or p95 threshold assertion was found.
- Why it matters: removing N+1 calls is necessary but not sufficient for the explicit exit-criteria decision. The proposal needs a measurable SLO, not just structural proof.
- Recommended action: add a focused benchmark/integration test with a realistic active-run count and assert `runs.list` p95 or a deterministic budget threshold.
- Acceptance criteria: P087 evidence includes `runs.list` latency samples under seeded load and fails when the budget regresses.

### READY-001 - Canonical P087 Gate Is Green, but Same-Tree Focused Regression Tests Fail

- Reviewer: readiness
- Severity: Critical
- Confidence: High
- Related requirements: REQ-008, REQ-009
- Evidence types: tests-run.
- Evidence references:
  - `./scripts/test-gate.sh proposal-087` passed.
  - `cargo test -p mcp-server runs_get_and_list_expose_p077_documented_and_legacy_closeout_summary_names -- --nocapture` failed in `proposal_077_closeout_readback_parity.rs`.
  - `cargo test -p mcp-server proposal_088_mcp_runs_get_and_list_expose_implementation_completion -- --nocapture` failed in `proposal_088_code_writer_completion_readback.rs`.
- Why it matters: a green proposal-specific gate is insufficient when the implementation breaks existing focused proposal tests on the same shared public API.
- Recommended action: resolve the P077/P088 contract conflict before closeout. The likely target is projection-backed compact summaries in `RunProjectionRow`, not reintroducing N+1 loaders.
- Acceptance criteria: P087, P077 closeout readback, and P088 code-writer completion readback focused tests all pass on the same tree.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Canonical P087 gate | Passed | `./scripts/test-gate.sh proposal-087` passed |
| MCP `runs.list` projection-only | Passed | Direct projection serialization and P087 unit test |
| GraphQL active runs projection-only | Passed | Direct projection mapping and P087 GraphQL test |
| MCP liveness sequence under maintenance/degraded conditions | Passed in-process | P087 MCP server test sequence passed |
| JSON-RPC `tools/list` and `resources/read` hot-read guarded | Passed | P087 guard-denial tests passed |
| Required runtime/artifact-noise projections | Failed/partial | `runtime.health` is minimal; artifact-noise projection absent |
| Operational liveness duration metric | Partial | Metric exists but recorder found only in tests |
| Cross-proposal API regression | Failed | P077 and P088 focused tests fail |
| Full regression suite | Not run | Already blocked by focused failures |
| UI/accessibility/localization/privacy/entitlements | Not primary scope | No new user write control or permission boundary in P087 |

## Verification Log

| Command / inspection | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py <proposal>` | Selected R2 report path |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py <proposal>` | No prior proposal-review artifacts found |
| `git rev-parse HEAD && git branch --show-current && git status --short --branch` | HEAD `76fa1d1594e0a81aedd08823054bbb11926ed697`, branch `cw/implement-proposal-087-local-s/b4edcf82`, dirty tree |
| Focused code inspection with `rg` / `nl -ba` | Verified updated MCP/GraphQL list paths, JSON-RPC liveness guard, runtime tool, metrics, projection gaps, and dependent P077/P088 contracts |
| `./scripts/test-gate.sh proposal-087` | Passed. DB P087 tests: 14 passed. MCP P087 tests: 11 passed. Auth P087 tests: 2 passed. GraphQL P087/storage tests passed. UI/schema/evidence static checks passed. Warnings only. |
| `cargo test -p mcp-server runs_get_and_list_expose_p077_documented_and_legacy_closeout_summary_names -- --nocapture` | Failed. `runs.list` closeout summary was `Null`; expected P077 readiness generation value. |
| `cargo test -p mcp-server proposal_088_mcp_runs_get_and_list_expose_implementation_completion -- --nocapture` | Failed. `runs.list` `implementationCompletion.ingestion_boundary_failure` was `Null`; expected `extraction_input_truncated`. |

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

R2 shows real progress over R1: MCP and GraphQL hot lists are now projection-only, `tools/list` and `resources/read` are guarded, `runtime.health` exists, and the P087 gate now exercises the central liveness path. The branch still is not ready because it achieves projection-only `runs.list` by dropping fields required by existing P077/P088 contracts and focused tests. It also still lacks the full required hot projection set and does not prove operational liveness-duration telemetry outside the test harness.

Recommended next actions:

1. Preserve P077/P088 `runs.list` compatibility with projection-backed compact summaries, or explicitly supersede/update those contracts and tests.
2. Implement or map the missing `RuntimeHealthProjection` and `ArtifactNoiseProjection`.
3. Record MCP liveness gate duration in a production diagnostic/probe path, not only tests.
4. Add seeded `runs.list` latency evidence for the 500 ms p95 storage-review threshold.
5. Require P087 plus the affected P077/P088 focused tests before closeout.
