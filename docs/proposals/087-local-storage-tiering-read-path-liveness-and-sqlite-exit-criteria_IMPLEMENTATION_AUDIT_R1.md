# Proposal 087 Implementation Audit R1

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Proposal title | Local Storage Tiering, Read-Path Liveness, and SQLite Exit Criteria |
| Proposal status | Active proposal file, `Status: Draft` |
| Audit timestamp | 2026-05-15 |
| Audit mode | `proposal-implementation-audit` auto mode |
| Implementation target | Worktree `.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Branch | `cw/implement-proposal-087-local-s/b4edcf82` |
| HEAD | `76fa1d1594e0a81aedd08823054bbb11926ed697` |
| Compare base | Implicit, not provided by user |
| Working tree | Dirty implementation tree with staged, unstaged, and untracked P087 files; `control-plane/crates/mcp-server/src/tools/storage.rs` has both staged and unstaged changes |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Audit confidence | High for inspected Rust/API/read-path claims; medium for full end-to-end runtime behavior because no live MCP daemon sequence was executed |

## Prior Review Reuse

Reviewer-selection reuse: Not reused.

Discovery result: the bundled prior-review discovery helper found no prior proposal-review artifacts for P087. Existing implementation-audit reports were intentionally ignored for reviewer selection per skill rules.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | Storage tiering, projection ownership, crate/module boundaries, SQLite/file-spool contract |
| `rust_reliability_reviewer` | MCP liveness, circuit guard behavior, maintenance repair, degraded/read-fail behavior |
| `rust_performance_reviewer` | `runs.list` latency budget, N+1 attachment risk, hot-read SLOs |
| `api_contract_reviewer` | MCP JSON-RPC methods, GraphQL storage health/read models, artifact metadata pointer contracts |
| `observability_rollout_reviewer` | Canonical gate, metrics, rollout readback, exit criteria, health diagnostics |

Rejected close alternatives:

- `rust_security_reviewer`: auth/capability changes are present, but the audited proposal is not primarily a new trust boundary. Operator-only storage tools were covered by auth tests and API/rollout review was more directly relevant.
- `macos_ui_reviewer` / `apple_ux_reviewer`: Swift files expose additive read-state tokens, but P087's primary contract is backend storage/read liveness rather than a new user journey or visual redesign.
- `product_reviewer`: proposal contains exit thresholds and decision checkpoints, but the current implementation risk is whether telemetry/gates actually enforce them, which fits observability/rollout.

## Proposal Contract Summary

P087 formalizes local "Plan A" storage:

- SQLite stores compact canonical state, metadata, compact projections, and health snapshots only.
- High-volume evidence, transcripts, logs, tool traces, stream deltas, and large reports stay file-backed.
- Hot read surfaces, especially `runs.list`, MCP read tools, GraphQL live reads, and UI read models, use materialized projections, caches, or precomputed summaries.
- MCP liveness must cover `initialize`, `tools/list`, `runs.list`, `runtime.health`, `storage.health`, and one simple resource/artifact metadata read within budgets even during degraded writer/spool/maintenance conditions.
- Storage health must expose write pressure, WAL, spool, projection lag, read latency, and liveness metrics.
- A canonical `proposal-087` gate must enforce the storage/read-path contract, including no high-volume SQLite rows and no `runs.list` detail attachments.

Explicit non-goals: no Postgres/RocksDB migration now, no SQLite replacement, no new UI write controls, no ACP provider changes, and no implementation of run compaction itself.

Platform/product scope:

- Apple scope: macOS read-state presentation only, not primary.
- Backend/service scope: Rust control-plane data, MCP, GraphQL, persistence, metrics, and rollout.
- Product scope: storage exit decision criteria.

## Primary Service Flows

1. MCP caller requests `runs.list` and expects a compact projection-only active run list.
2. MCP caller performs the liveness sequence: `initialize`, `tools/list`, `runs.list`, `runtime.health`, `storage.health`, and a metadata/resource read.
3. Swift/GraphQL client reads active runs, projection lag, and storage health without deep scans or raw evidence reads.
4. Projection invalidation/rebuild records freshness, backlog, poison, and throttle state without blocking ordinary reads.
5. Operator inspects storage health and rollout readback to decide whether SQLite still meets local Plan A exit criteria.

## Fidelity Inventory

Matches:

- New P087 DB migrations add compact projection/health/maintenance/circuit tables rather than raw stream/evidence tables.
- The daemon wires a live `DbWriterHeartbeat` into MCP and GraphQL server construction.
- `storage.health` exposes a typed `storage_health.v1` payload with writer, WAL, projection, spool, hot-read guard, maintenance, threshold, and rollout sections.
- MCP storage maintenance tools and operator-only capability checks exist.
- The `proposal-087` gate exists and passed on the audited tree.

Divergences:

- MCP `runs.list` still performs per-run implementation self-assessment, rollout contract, code-writer completion, side-effect, and closeout attachment reads after loading `list_active_projection`.
- GraphQL `runs` uses projections but then performs the same kind of per-run enrichment through `runs_with_latest_summaries`.
- The MCP hot-read guard is only applied to tool dispatch, not to JSON-RPC `tools/list` or `resources/read`, even though the proposal's liveness gate explicitly includes both.
- The canonical gate does not execute the required live MCP liveness sequence and does not currently fail on the observed `runs.list` attachment path.
- Required projection set is only partly visible: active runs, approval inbox, artifact index, and storage health/freshness exist, but no concrete runtime health or artifact-noise projection matching P087 is evident.

Ambiguities / evidence gaps:

- No live daemon MCP sequence was run during this audit.
- No benchmark or integration proof was found for `runs.list` p95 <= 500 ms under real or seeded multi-run load.
- Runtime-health projection and artifact-noise projection may be represented by older diagnostics, but the implementation does not expose a clear P087 mapping.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | SQLite limited to compact canonical state, metadata, compact projections, and health snapshots | Implemented |
| REQ-002 | High-volume evidence remains file-spooled with metadata pointers only in DB/API list surfaces | Implemented |
| REQ-003 | `runs.list` is projection-only and has no N+1/detail attachments | Partially Implemented |
| REQ-004 | MCP liveness gate covers initialize, `tools/list`, hot read tools, and metadata/resource read within budgets | Partially Implemented |
| REQ-005 | GraphQL hot reads use projections/cache, not deep scans or per-row enrichment | Partially Implemented |
| REQ-006 | Required hot projection set and freshness/degraded metadata are implemented | Partially Implemented |
| REQ-007 | Storage health exposes required write/read/projection/spool metrics | Partially Implemented |
| REQ-008 | Storage exit criteria are documented and enforced by the canonical gate | Partially Implemented |
| REQ-009 | Maintenance/read-path behavior avoids blocking ordinary reads | Partially Implemented |
| REQ-010 | P078, P038, and P086 can rely on storage tiering without increasing SQLite pressure | Partially Implemented |

## Detailed Requirement Audit

### REQ-001 - SQLite Compact Ownership

- Proposal source: sections 3.1, 5.1, 5.2, 18.1 (`docs/proposals/087...md:67-85`, `143-169`, `589`).
- Status: Implemented.
- Evidence types: proposal, migration, code, tests-run.
- Evidence references:
  - New P087 migrations create projection cursors, invalidation log, maintenance operations, storage health snapshots, and hot-read circuit states: `control-plane/crates/db/migrations/056_p087_storage_tiering_projections.sql`, `057_p087_hot_read_refinements.sql`.
  - `./scripts/test-gate.sh proposal-087` verified migration version uniqueness and passed.
- Mapping: implementation adds compact coordination/health tables, not raw stream chunk tables.
- Gap/note: full repository-wide proof that no other changed writer introduced high-volume rows depends on static gate coverage rather than a semantic migration linter.

### REQ-002 - File-Backed Evidence and Metadata Pointers

- Proposal source: sections 3.2, 5.3, 18.2 (`docs/proposals/087...md:86-109`, `171-181`, `590`).
- Status: Implemented.
- Evidence types: code, schema, tests-run.
- Evidence references:
  - MCP artifact resource read returns an `artifact_metadata_pointer.v1` with checksum/size/route and redacts filesystem path/raw payload (`control-plane/crates/mcp-server/src/server.rs:537-562`).
  - The P087 gate checks artifact pointer fixtures and rejects MCP artifact metadata leaking `file_path` in the resource metadata path (`scripts/test-gate.sh:7871-7878`, `7908-7915`).
- Mapping: list/metadata surfaces expose pointers rather than payloads.
- Gap/note: this audit did not inspect every report/detail tool for payload behavior because P087's hot-path commitment is metadata/list/readback.

### REQ-003 - `runs.list` Projection-Only

- Proposal source: sections 6.1, 16 phase 2, 17, 18.3 (`docs/proposals/087...md:187-199`, `531-549`, `564-565`, `591`).
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - MCP `runs.list` starts from `projections::list_active_projection` (`control-plane/crates/mcp-server/src/tools/runs.rs:379-380`).
  - It then loops each row through `attach_implementation_self_assessment_summary` and `attach_closeout_readiness_summary` (`control-plane/crates/mcp-server/src/tools/runs.rs:381-390`).
  - `attach_implementation_self_assessment_summary` performs multiple per-run reads: self-assessment, rollout contract, code-writer receipts, canonical receipts, and side-effect readback (`control-plane/crates/mcp-server/src/tools/runs.rs:809-859`).
  - `attach_closeout_readiness_summary` performs another per-run closeout load (`control-plane/crates/mcp-server/src/tools/runs.rs:989-1020`).
  - Existing tests still assert self-assessment is included in `runs.list`, which is the opposite of P087 unless pre-materialized (`control-plane/crates/mcp-server/src/tools/runs.rs:1467-1495`).
- Mapping: the projection base exists, but the response is not projection-only.
- Gap/note: no query-count, filesystem-block, or latency-budget test was found for `runs.list`.

### REQ-004 - MCP Liveness Gate

- Proposal source: section 13 (`docs/proposals/087...md:422-444`).
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references:
  - Tool-level hot-read guard wraps `dispatch_tool_internal` for selected tool names (`control-plane/crates/mcp-server/src/server.rs:791-811`).
  - `tools/list` is a JSON-RPC method handled before tool dispatch and has no hot-read guard/timeout wrapper (`control-plane/crates/mcp-server/src/server.rs:320-333`).
  - `resources/read` is also a JSON-RPC method and directly calls `handle_resource_read`/`read_resource_for_principal` without the hot-read guard (`control-plane/crates/mcp-server/src/server.rs:436-452`, `479-517`).
  - `is_hot_read_tool` lists `"tools.list"` and `"artifacts.metadata.get"` (`control-plane/crates/mcp-server/src/hot_read_guard.rs:79-83`), but those names do not correspond to the actual JSON-RPC `tools/list` and `resources/read` paths.
  - The canonical gate passed but did not execute the end-to-end MCP sequence.
- Mapping: selected tool calls have circuit/timeout behavior; required non-tool liveness surfaces do not.
- Gap/note: no live normal-operation or degraded-operation liveness runtime proof was found.

### REQ-005 - GraphQL Hot Reads Use Projections/Cache

- Proposal source: sections 6.3, 18.5 (`docs/proposals/087...md:220-235`, `593`).
- Status: Partially Implemented.
- Evidence types: code, schema.
- Evidence references:
  - GraphQL `runs` starts from projection methods (`control-plane/crates/graphql-server/src/schema.rs:257-266`).
  - It then calls `runs_with_latest_summaries`, which loops each run and performs multiple per-run reads for implementation self-assessment, code-writer receipts, retry authority, workflow conflicts, closeout readiness, and side-effect readback (`control-plane/crates/graphql-server/src/schema.rs:1070-1135`).
  - GraphQL `storage_health` is guarded and timeout-wrapped (`control-plane/crates/graphql-server/src/schema.rs:581-623`).
- Mapping: storage health is reasonably aligned; GraphQL run list is not projection/cache-only.
- Gap/note: no GraphQL latency/read-budget test was found for active runs under load.

### REQ-006 - Hot Projection Set and Freshness

- Proposal source: sections 7 and 8 (`docs/proposals/087...md:239-352`).
- Status: Partially Implemented.
- Evidence types: code, migration, tests-found.
- Evidence references:
  - Active run projection is represented by `RunProjectionRow` and `list_active_projection` (`control-plane/crates/db/src/repos/projections.rs:127-153`, `181-235`).
  - Approval inbox projection exists (`control-plane/crates/db/src/repos/projections.rs:623-750`).
  - Projection freshness/backlog/poison/throttle readback exists in storage health (`control-plane/crates/db/src/repos/storage_health.rs:431-489`).
  - Search found no concrete `RuntimeHealthProjection` or `ArtifactNoiseProjection` implementation corresponding to the required P087 names/scope.
- Mapping: active runs, approval inbox, artifact index, and storage health/freshness are covered; the full required set is not clearly implemented.
- Gap/note: `ArtifactNoiseProjection` is explicitly required for P038 compaction readiness, but no matching projection surfaced in the audited code.

### REQ-007 - Storage Health and Metrics

- Proposal source: section 14 and acceptance criteria 6 (`docs/proposals/087...md:448-467`, `594`).
- Status: Partially Implemented.
- Evidence types: code, telemetry, tests-run.
- Evidence references:
  - `storage_health_with_writer` exposes writer, WAL, projection, hot-read guard, maintenance, evidence spool, write pressure, telemetry rollup, and thresholds (`control-plane/crates/db/src/repos/storage_health.rs:300-428`).
  - Required metric names are declared, including `runs_list_read_latency_ms` and `mcp_liveness_gate_duration_ms` (`control-plane/crates/db/src/metrics.rs:51-70`).
  - Actual hot-read recording is generic by tool surface (`control-plane/crates/db/src/metrics.rs:99-105`) and storage health exposes latency only through existing hot-read circuit rows (`control-plane/crates/db/src/repos/storage_health.rs:491-513`).
- Mapping: many health fields exist, but the required liveness-gate duration and explicit `runs.list` SLO metric are not proven as observable/enforced metrics.
- Gap/note: declaration alone is not enough to support the storage exit decision promised by the proposal.

### REQ-008 - Exit Criteria and Canonical Gate

- Proposal source: sections 15, 16 phase 4, 17, 18.7 (`docs/proposals/087...md:471-515`, `544-549`, `560-581`, `595`).
- Status: Partially Implemented.
- Evidence types: code, config, tests-run.
- Evidence references:
  - `proposal-087` gate exists and passed (`scripts/test-gate.sh:7783-8000`; verification log below).
  - Gate runs filtered cargo tests, then static/schema/fixture checks (`scripts/test-gate.sh:7817-7823`, `7824-7998`).
  - The gate does not execute the live MCP sequence from section 13 and did not catch the `runs.list` per-row attachment path described in REQ-003.
- Mapping: gate infrastructure exists, but enforcement is too weak for the proposal's central acceptance criteria.
- Gap/note: a passing gate currently cannot be treated as proof that P087's read-path liveness contract is implemented.

### REQ-009 - Maintenance Does Not Block Ordinary Reads

- Proposal source: sections 6.2, 13, 16 phase 2 (`docs/proposals/087...md:207-218`, `435-444`, `531-534`).
- Status: Partially Implemented.
- Evidence types: code, tests-run.
- Evidence references:
  - Maintenance tables and repair/clear tools exist; unit tests cover repair CAS/redaction/clear behavior.
  - The liveness guard is tool-local and does not cover the full JSON-RPC request loop, `tools/list`, or resource reads.
  - No integration test was found for read liveness while maintenance operation is running.
- Mapping: maintenance mechanics are present, but the ordinary-read liveness property remains unproven.
- Gap/note: this is a readiness blocker because proposal section 13 explicitly names maintenance-in-progress as a liveness condition.

### REQ-010 - Dependent Proposal Pressure Contract

- Proposal source: sections 10-12 and acceptance criteria 8 (`docs/proposals/087...md:379-418`, `596`).
- Status: Partially Implemented.
- Evidence types: code, inference.
- Evidence references:
  - P078 side-effect readback is still attached per run in MCP `runs.list` through `build_side_effect_readback` (`control-plane/crates/mcp-server/src/tools/runs.rs:856-858`, `897-947`).
  - GraphQL run list also attaches side-effect readback per run (`control-plane/crates/graphql-server/src/schema.rs:1135`).
  - Proposal expects unresolved side-effect count/status to be projected for hot reads (`docs/proposals/087...md:381-389`).
- Mapping: dependent data exists, but it is not consistently projected for hot list surfaces.
- Gap/note: P078/P038/P086 cannot safely rely on the P087 contract until list readbacks stop adding detail reads per row.

## Reviewer Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Central `runs.list` and MCP liveness acceptance criteria are incomplete | High |
| Rust architecture | Partial | Projection ownership is added but detail readbacks remain in hot list paths | High |
| Rust reliability | Not Ready | JSON-RPC liveness surfaces bypass the guard and no live degraded sequence is proven | High |
| Rust performance | Not Ready | N+1 enrichment remains on MCP and GraphQL list paths | High |
| API contract | Partial | Artifact pointer and storage schema are additive, but `tools/list`/`resources/read` are outside liveness enforcement | High |
| Observability/rollout | Not Ready | Gate passes without enforcing the proposal's primary liveness/SLO assertions | High |
| Overall readiness | Not Ready | Major findings remain despite passing `proposal-087` gate | High |

## Routed Specialist Findings

### REL-001 - MCP `runs.list` Still Performs Per-Run Detail Attachments

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-003, REQ-010
- Evidence types: proposal, code, tests-found
- Evidence references:
  - P087 forbids N+1 artifact/report/detail attachment and non-materialized implementation self-assessment on `runs.list` (`docs/proposals/087...md:187-199`, `531-549`).
  - MCP `runs.list` loops projection rows through per-run attachment functions (`control-plane/crates/mcp-server/src/tools/runs.rs:379-390`).
  - The attachment path performs multiple per-run DB reads and side-effect readback (`control-plane/crates/mcp-server/src/tools/runs.rs:809-859`, `897-947`, `989-1020`).
- Why it matters: this is the exact SQLite pressure and liveness failure mode P087 was written to prevent. A large active-run set multiplies DB reads and keeps dependent P078/P088/P077 readback work on the hot list path.
- Recommended action: pre-materialize only compact list-safe summaries into the active-run projection, or remove these fields from `runs.list` and keep them on `runs.get`/diagnostic tools.
- Acceptance criteria: `runs.list` returns directly from projection/cache rows, a query-count test proves no per-run attachment calls, and the P087 gate fails if `runs.list` invokes non-projection summary loaders.

### PERF-001 - GraphQL Active Runs Also Re-Enriches Each Projection Row

- Reviewer: `rust_performance_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-005, REQ-010
- Evidence types: proposal, code
- Evidence references:
  - GraphQL `runs` reads active projections, then calls `runs_with_latest_summaries` (`control-plane/crates/graphql-server/src/schema.rs:257-266`).
  - `runs_with_latest_summaries` performs per-run loads for self-assessment, code-writer completion, retry authority, workflow conflict, closeout readiness, and side-effect readback (`control-plane/crates/graphql-server/src/schema.rs:1070-1135`).
- Why it matters: P087's GraphQL commitment is projection/cache-backed hot reads. The current implementation preserves the old rich-detail list behavior and can still turn UI run lists into a multi-query fanout.
- Recommended action: split GraphQL list from detail enrichment, or materialize compact list fields into projections with freshness metadata.
- Acceptance criteria: GraphQL active runs resolver has no per-run loader loop for list-only fields, and a test/benchmark covers active-run list latency under seeded multi-run load.

### REL-002 - MCP Liveness Guard Misses `tools/list` and Resource Metadata Reads

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-004, REQ-009
- Evidence types: proposal, code
- Evidence references:
  - P087's minimum liveness gate includes `initialize`, `tools/list`, `runs.list`, `runtime.health`, `storage.health`, and a simple resource/artifact metadata read (`docs/proposals/087...md:422-440`).
  - `tools/list` is handled as a JSON-RPC method outside tool dispatch (`control-plane/crates/mcp-server/src/server.rs:320-333`).
  - `resources/read` directly invokes the resource read path outside tool dispatch (`control-plane/crates/mcp-server/src/server.rs:436-452`, `479-517`).
  - The hot-read guard only wraps `dispatch_tool_internal` for selected tool names (`control-plane/crates/mcp-server/src/server.rs:791-811`).
  - `is_hot_read_tool` names `"tools.list"` and `"artifacts.metadata.get"`, but the actual surfaces are JSON-RPC `tools/list` and `resources/read` (`control-plane/crates/mcp-server/src/hot_read_guard.rs:79-83`).
- Why it matters: the implementation can report hot-read enforcement while two surfaces required by the proposal bypass the enforcement path entirely.
- Recommended action: add method/resource-level liveness wrappers for `tools/list`, `resources/read` metadata reads, and any equivalent GraphQL/MCP hot metadata path.
- Acceptance criteria: a live MCP integration test opens or degrades the hot-read circuit and verifies typed bounded behavior for `tools/list`, `runs.list`, `runtime.health`, `storage.health`, and artifact metadata read.

### OPS-001 - The Canonical Gate Does Not Enforce the Central Liveness/SLO Contract

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-004, REQ-008, REQ-009
- Evidence types: tests-run, code
- Evidence references:
  - The gate runs filtered cargo tests for DB, MCP, auth, and GraphQL plus static/schema/fixture checks (`scripts/test-gate.sh:7817-7998`).
  - The gate passed on the audited tree.
  - The gate did not execute the section 13 sequence or fail on the `runs.list` attachment implementation that directly violates section 6.1.
- Why it matters: P087's acceptance criteria depend on the gate as the durable enforcement mechanism. A green gate that misses the core failure modes gives false confidence.
- Recommended action: extend `proposal-087` with an integration harness that starts the daemon/server path or unit-drives the JSON-RPC dispatcher through the exact liveness sequence under normal, maintenance-running, and degraded/slow read scenarios.
- Acceptance criteria: the gate fails when `runs.list` performs detail attachment, when `tools/list` or `resources/read` lack read budgets, or when liveness metrics/SLOs are absent.

### ARCH-001 - Required Hot Projection Set Is Incomplete or Unmapped

- Reviewer: `rust_arch_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-006, REQ-010
- Evidence types: proposal, code, search
- Evidence references:
  - P087 requires `ActiveRunIndex`, `ApprovalInboxProjection`, `RuntimeHealthProjection`, `StorageHealthProjection`, and `ArtifactNoiseProjection` (`docs/proposals/087...md:239-329`).
  - Active runs and approval inbox projections are present (`control-plane/crates/db/src/repos/projections.rs:127-153`, `623-750`).
  - Storage freshness/readback exists (`control-plane/crates/db/src/repos/storage_health.rs:431-489`).
  - No concrete runtime-health projection or artifact-noise projection surfaced in audited code searches.
- Why it matters: P038/P086/P078 are supposed to depend on projected compact state rather than detail scans. Missing projection ownership leaves future work with no clear storage contract.
- Recommended action: either implement the missing projection rows/caches or document the exact existing projection/read model that satisfies each required P087 projection.
- Acceptance criteria: reference docs and code expose a one-to-one mapping for all required P087 hot projections, with rebuild/freshness tests.

### OPS-002 - Required Read/Liveness Metrics Are Declared but Not Proven Observable or Enforced

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-007, REQ-008
- Evidence types: telemetry, code
- Evidence references:
  - Metric names include `runs_list_read_latency_ms` and `mcp_liveness_gate_duration_ms` (`control-plane/crates/db/src/metrics.rs:51-70`).
  - Hot-read latency recording is generic by tool surface (`control-plane/crates/db/src/metrics.rs:99-105`).
  - Storage health exposes hot-read latency only for rows present in `hot_read_circuit_states` (`control-plane/crates/db/src/repos/storage_health.rs:491-513`).
- Why it matters: P087's SQLite exit decision depends on observing `runs.list` p95 and MCP liveness duration. Declared names without exercised readback/enforcement do not support the decision checkpoint.
- Recommended action: record and expose explicit `runs.list` p95 and full liveness gate duration, then make the proposal gate assert the values and thresholds.
- Acceptance criteria: storage health/GraphQL/MCP diagnostics show non-null metrics after the gate scenario runs, and the gate fails when the latency budget is exceeded.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Canonical proposal gate passed on audited tree | Passed | `./scripts/test-gate.sh proposal-087` passed on HEAD `76fa1d1594e0a81aedd08823054bbb11926ed697` |
| Core MCP liveness runtime sequence validated | Missing | No live `initialize` -> `tools/list` -> `runs.list` -> `runtime.health` -> `storage.health` -> metadata read sequence was executed |
| `runs.list` projection-only proof | Failed | Code inspection shows per-run detail attachments after projection read |
| GraphQL hot-read projection-only proof | Failed | Code inspection shows per-run enrichment loop |
| Storage health typed schema/readback | Passed with risk | Schema/readback tests passed; metrics/liveness duration enforcement incomplete |
| Maintenance-running liveness proof | Missing | No integration scenario found or executed |
| Empty/loading/error/offline/permission UI states | Not primary scope | Swift changes are additive diagnostic presentation, not P087 primary acceptance |
| Accessibility/localization/privacy/entitlements | Not primary scope | No new UI control or permission boundary is central to P087 |
| Critical tests executed | Passed | Filtered P087 cargo tests and static gate passed |
| Full regression suite | Not run | Proposal gate was run; full gate was not run because readiness is blocked by major findings |

## Verification Log

| Command / inspection | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py <proposal>` | No prior proposal-review artifacts found |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py <proposal>` | Selected this R1 report path |
| `git rev-parse HEAD && git branch --show-current` | `76fa1d1594e0a81aedd08823054bbb11926ed697`, `cw/implement-proposal-087-local-s/b4edcf82` |
| `git status --short --branch` | Dirty implementation tree with staged, unstaged, and untracked P087 files |
| Focused proposal and code inspection with `nl -ba` / `rg` | Found P087 contract lines, MCP/GraphQL list attachment paths, liveness guard scope, metrics declarations, and gate contents |
| `./scripts/test-gate.sh proposal-087` | Passed. DB: 14 P087 tests passed. MCP: 7 P087 tests passed. Auth: 2 P087 tests passed. GraphQL storage tests passed. UI/schema/evidence static checks passed. Compiler warnings observed but no test failures. |

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

The implementation has meaningful P087 infrastructure: compact storage tables, projection invalidation/freshness, storage health readback, hot-read circuit primitives, maintenance repair tools, and a canonical gate. However, the central read-path guarantees are not yet implemented strongly enough. MCP and GraphQL run lists still perform per-run detail enrichment after projection reads; the MCP liveness guard does not cover `tools/list` or `resources/read`; and the green `proposal-087` gate does not enforce the liveness/SLO scenario that P087 requires.

Recommended next actions:

1. Make MCP and GraphQL list surfaces projection-only, with list-safe summaries pre-materialized or moved to detail tools.
2. Guard and test the actual JSON-RPC liveness surfaces, including `tools/list` and metadata `resources/read`.
3. Extend `proposal-087` to run the mandatory liveness sequence under normal/degraded/maintenance conditions and fail on detail attachments.
4. Complete or explicitly map the required runtime-health and artifact-noise projections.
5. Expose exercised, non-null `runs.list` latency and MCP liveness gate duration metrics for exit-criteria decisions.
