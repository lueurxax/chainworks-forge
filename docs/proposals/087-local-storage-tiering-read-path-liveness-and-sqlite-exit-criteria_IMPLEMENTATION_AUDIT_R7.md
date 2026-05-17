# Proposal 087 Implementation Audit R7

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Proposal state | Active proposal file, front-matter status `Draft` |
| Audit mode | `implementation-audit` |
| Audit timestamp | 2026-05-16T17:48:02Z |
| Implementation target | Worktree `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Branch | `cw/implement-proposal-087-local-s/b4edcf82` |
| Audited HEAD | `569b297e58582153eb3601f91d18e7aa97d9a6f2` |
| Compare base | Implicit current worktree and staged/unstaged diff |
| Working tree state | Dirty: staged and unstaged implementation changes plus untracked P087 files, prior audit reports, `.junie/plans/`, and `Chainworks ForgeTests/PreviewSupport+Tests.swift` |
| Report path helper | `report_path.py` selected this R7 report path |
| Overall conformance | Implemented |
| Overall implementation readiness | Ready with Risks |
| Audit confidence | High for proposal conformance; Medium-High for release readiness |

## Prior Proposal-Review Reuse

Reviewer-selection reuse: **Not reused**.

I ran the skill helper for prior proposal-review discovery. It returned no proposal-review artifacts for this proposal. Existing sibling `IMPLEMENTATION_AUDIT_R*.md` files were not used for reviewer selection, per the skill boundary; they were treated only as historical implementation context.

Selected reviewers:

| Reviewer | Reason |
|---|---|
| `rust_backend_arch_reviewer` | Rust daemon/database/work-queue architecture, projection ownership, and DbWriter integration are central. |
| `rust_reliability_reviewer` | Read-path liveness, startup rebuild, invalidation lifecycle, cancellation, maintenance reapers, and fail-fast behavior are central. |
| `rust_performance_reviewer` | The proposal commits to hot-read budgets, projection-only list reads, and storage exit thresholds. |
| `api_contract_reviewer` | MCP, GraphQL, artifact metadata pointer, typed error, and Swift diagnostics readback contracts changed. |
| `observability_rollout_reviewer` | Storage health, rollout fixtures, metrics, gate enforcement, and migration/registry readiness are central. |

Rejected close alternatives:

| Reviewer | Reason rejected |
|---|---|
| `rust_security_reviewer` | Auth/path-redaction behavior is present and regression-covered, but the implementation does not introduce unsafe code, secret handling, or a new broad public boundary. API and rollout reviewers cover the contract surface. |
| `macos_ui_reviewer` | Swift changes are diagnostic-query readback only; the proposal did not require new macOS UI workflow implementation. |
| `product_reviewer` | The proposal is an infrastructure contract with operational metrics, not a user-value experiment or product decision gate. |

## Proposal Contract Summary

Scope: adopt Plan A for local storage: SQLite stores compact canonical state and metadata; high-volume evidence stays in the filesystem; hot reads use projections/cache; MCP and GraphQL remain live under maintenance/degraded storage conditions; storage exit criteria are documented and gate-enforced.

Locked decisions:

- Keep SQLite local and do not migrate to Postgres/RocksDB in this proposal (`§4`, lines 125-139).
- SQLite owns compact state, metadata, projection cursors, health snapshots, and compact read models (`§3.1`, lines 67-84; `§5.1`, lines 143-155).
- File store owns raw transcripts, tool traces, stdout/stderr, stream deltas, runtime event bundles, and large reports/artifacts (`§3.2`, lines 86-109; `§5.3`, lines 171-181).
- `runs.list` must be projection-only and avoid N+1 detail attachment, filesystem scans, transcripts, compaction archive inspection, and side-effect evidence readback (`§6.1`, lines 185-205).
- MCP reads must have strict budgets, long operations must not block request handling, and health tools must fail fast with typed degraded status (`§6.2`, lines 207-218; `§13`, lines 422-445).
- Initial hot projections include active runs, approval inbox, runtime health, storage health, and artifact noise (`§7`, lines 239-330).
- Derived caches/projections must rebuild from SQLite/file metadata after restart and expose freshness/degraded metadata (`§8`, lines 333-343).
- Required metrics and storage exit criteria must be exposed and enforced by a gate (`§14`, lines 448-468; `§15`, lines 471-515; `§16-17`, lines 519-581).

Platform/product scope:

- Apple: macOS read-side client consumption only; no new UI write controls.
- Backend/service: Rust control-plane daemon, SQLite data model, MCP server, GraphQL server, storage metrics, migrations, and rollout gates.
- Product scope: operator trust and local system liveness, not a new workflow-facing feature.

Primary service flows audited:

1. MCP liveness sequence: `initialize` -> `tools/list` -> `runs.list` -> `runtime.health` -> `storage.health` -> `resources/read`.
2. Hot run listing: MCP and GraphQL list reads use `run_summaries` projection rows, not per-row detail enrichment.
3. Startup/restart projection recovery: recovery rebuilds run, stage, approval, artifact, artifact-noise, and runtime-health projections after daemon restart.
4. Maintenance and projection repair: long/recovery operations are tracked by maintenance slots and reaped without blocking hot reads.
5. Operator diagnostics: storage health and rollout fixtures expose writer/WAL/spool/projection/read-path/threshold state to GraphQL, MCP, and Swift diagnostics readback.

## Fidelity Inventory

Matches:

- P087 migrations add projection invalidation, cursors, maintenance operations, storage health snapshots, hot-read circuit state, run-summary hot-read fields, artifact-noise summary, runtime-health summary, hot-read retry/backoff, and consumed invalidation lifecycle (`057_p087...sql`, lines 1-67; `058_p087...sql`, lines 1-6; `059_p087...sql`, lines 1-35; `060_p087...sql`, lines 1-7).
- File-backed evidence remains the high-volume evidence contract: `evidence_spool_refs` stores metadata pointers while raw bytes live in files (`046_p075...sql`, lines 1-10), and DbWriter rules forbid raw evidence bytes in Class C payloads (`writer.rs`, lines 35-46).
- `runs.list` dispatches directly to `projections::list_active_projection` (`tools/runs.rs`, lines 379-382), and the projection query reads from `run_summaries` fields including compact completion/closeout summaries (`projections.rs`, lines 193-260).
- MCP hot reads are wrapped by `HotReadGuard`, budgets, latency metrics, timeout typed errors, and hot-read circuit status (`server.rs`, lines 533-587; `server.rs`, lines 881-920; `hot_read_guard.rs`, lines 82-91).
- Storage health exposes writer, WAL, projections, projection freshness, hot-read guards, read-path metrics, artifact-noise projection, maintenance reaper status, degraded state, and hot-read status (`storage_health.rs`, lines 406-470).
- Runtime health reads from `runtime_health_summary` and reports active sessions, hot-read circuit degradation, write-pressure flag, unresolved side-effects, and continuation counts (`storage_health.rs`, lines 742-767).
- Terminal run transitions now record projection invalidations in production paths (`runs.rs`, lines 162-186, 278-303, 378-396).
- Startup repair delegates to `projections::rebuild_all_for_run`, which rebuilds artifact-noise and runtime-health summaries too (`recovery.rs`, lines 1904-1909; `projections.rs`, lines 1168-1190).
- The canonical `proposal-087` gate checks migration uniqueness, targeted Rust tests, Swift diagnostics fields, GraphQL compatibility, P087 fixtures, metrics, operation registry entries, negative fixtures, and production invalidation wiring (`scripts/test-gate.sh`, lines 7828-8108).

Divergences:

- None that change an in-scope requirement status.

Ambiguities / evidence gaps:

- The successful proof is the canonical focused `proposal-087` gate, not `./scripts/test-gate.sh full` and not a live daemon restart on a packaged runtime.
- The worktree is dirty with mixed staged/unstaged changes and unrelated-looking untracked files. The audit treats the current worktree as the implementation target, but handoff should preserve all P087 files together.

## Requirement Summary

| Requirement | Status | Evidence |
|---|---|---|
| REQ-001 SQLite remains compact canonical state and metadata | Implemented | Migrations, DbWriter rules, gate static checks |
| REQ-002 High-volume evidence remains file-backed | Implemented | Evidence spool schema and writer constraints |
| REQ-003 `runs.list` is projection-only | Implemented | MCP and GraphQL code, tests, gate |
| REQ-004 MCP read-path liveness and typed fail-fast behavior | Implemented | Hot-read guard, typed errors, liveness tests |
| REQ-005 GraphQL hot reads and storage health are additive/projection-backed | Implemented | GraphQL schema/types/tests, Swift diagnostics query |
| REQ-006 Required hot projections exist | Implemented | Projection migrations/rebuild code |
| REQ-007 Projection cache/read models rebuild after restart | Implemented | Recovery code and integration test |
| REQ-008 Storage health metrics and exit criteria are exposed | Implemented | Storage health readback, thresholds, rollout fixture |
| REQ-009 Gate enforcement covers P087 regressions | Implemented | Same-tree `proposal-087` gate passed |
| REQ-010 P038/P078/P086 dependent readback can rely on the contract | Implemented | Runtime/side-effect/continuation counts, artifact-noise, compatibility tests |

## Detailed Requirement Audit

### REQ-001: SQLite compact canonical state and metadata

- Proposal source: `§3.1`, lines 67-84; `§5.1`, lines 143-155.
- Status: Implemented.
- Evidence types: code, migration, tests-run.
- Evidence references: projection and maintenance tables in `057_p087...sql`, lines 3-67; run-summary refinements and health summaries in `059_p087...sql`, lines 3-35; write operation registry entries in `write-operation-registry.toml`, lines 876-929.
- Mapping: P087 adds compact projection/invalidation/health/maintenance rows and keeps them behind DbWriter-registered operations.
- Gap/note: No high-volume canonical replacement is introduced.

### REQ-002: High-volume evidence remains file-backed

- Proposal source: `§3.2`, lines 86-109; `§5.2-5.3`, lines 157-181.
- Status: Implemented.
- Evidence types: code, migration, tests-run.
- Evidence references: `evidence_spool_refs` stores metadata pointers while raw bytes live in files (`046_p075...sql`, lines 1-10); writer comments forbid raw evidence bytes in Class C (`writer.rs`, lines 35-46); P087 gate fails on new high-volume evidence row patterns (`scripts/test-gate.sh`, lines 7889-8108).
- Mapping: P087 builds on the implemented P075 file-spool baseline and adds checks/fixtures around storage readback and artifact metadata pointer behavior.
- Gap/note: No new P087 table stores raw transcript/tool/stdout bytes.

### REQ-003: `runs.list` is projection-only

- Proposal source: `§6.1`, lines 185-205; acceptance criterion 3, line 591.
- Status: Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references: MCP `runs.list` calls `projections::list_active_projection` only (`tools/runs.rs`, lines 379-382); projection row fields are read from `run_summaries` (`projections.rs`, lines 193-260); test rejects detail attachments on `runs.list` (`tools/runs.rs`, lines 1487-1525); GraphQL list uses projection functions (`schema.rs`, lines 257-266); GraphQL test rejects per-row implementation/self-assessment/rollout/side-effect enrichment (`schema.rs`, lines 2918-2937).
- Mapping: List surfaces return compact projection fields and omit detail-only attachments unless they are pre-materialized projection summaries.
- Gap/note: None.

### REQ-004: MCP read-path liveness and typed fail-fast behavior

- Proposal source: `§6.2`, lines 207-218; `§13`, lines 422-445.
- Status: Implemented.
- Evidence types: code, tests-run, config.
- Evidence references: `tools/list` and `resources/read` are hot-read wrapped (`server.rs`, lines 332-363, 465-506); tool dispatch wraps hot-read tools with circuit checks, 500 ms probe budget, 10 second normal timeout, cancellation token, and latency metrics (`server.rs`, lines 881-920); hot-read tool set includes `runs.list`, `runtime.health`, `storage.health`, `resources.read`, and `tools.list` (`hot_read_guard.rs`, lines 82-91); typed errors carry `errorCode`, `requestId`, retry, and `hotRead` metadata (`tools/storage.rs`, lines 24-81).
- Mapping: Stuck or open hot-read surfaces return typed tool-result bodies instead of blocking ordinary read handling.
- Gap/note: Runtime proof is targeted unit/integration gate evidence, not a live daemon soak.

### REQ-005: GraphQL hot reads and storage health are additive/projection-backed

- Proposal source: `§6.3`, lines 220-235; acceptance criterion 5, line 593.
- Status: Implemented.
- Evidence types: code, schema, tests-run.
- Evidence references: GraphQL `runs` query uses projection functions (`schema.rs`, lines 257-266); `StorageHealth` keeps legacy `projections` as a complex field while adding projection freshness and hot-read guards (`types/storage.rs`, lines 204-245); parser preserves writer/WAL/projection/hot-read fields (`types/storage.rs`, lines 253-360); Swift diagnostics query includes `projectionFreshness`, `hotReadGuards`, `hotRead`, and `maintenanceReaper` per gate check (`scripts/test-gate.sh`, lines 7889-7898).
- Mapping: Additive GraphQL and Swift readback support storage diagnostics without replacing the existing storage-health projection field shape.
- Gap/note: No UI runtime screenshot was needed because no UI view behavior was introduced beyond diagnostics query coverage.

### REQ-006: Required hot projections exist

- Proposal source: `§7`, lines 239-330.
- Status: Implemented.
- Evidence types: code, migration, tests-run.
- Evidence references: `run_summaries` hot-read fields in `059_p087...sql`, lines 3-10; `artifact_noise_summary` in `059_p087...sql`, lines 12-20; `runtime_health_summary` in `059_p087...sql`, lines 22-35; existing projection owner inventory for runs, approvals, artifacts, side-effects, scheduler, and storage health in `query-projections-and-client-consumption-contract.md`, lines 81-103.
- Mapping: Active runs, approval inbox, runtime health, storage health, and artifact noise have projection-backed read paths or compact summaries.
- Gap/note: Storage health is computed/readback-backed rather than a single materialized table for every field, but the proposal allowed precomputed health snapshots and compact read models rather than mandating one table per metric.

### REQ-007: Projection cache/read models rebuild after restart

- Proposal source: `§8`, lines 333-343; required test line 568.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: startup projection rebuild helper delegates to `rebuild_all_for_run` (`recovery.rs`, lines 1904-1909); `rebuild_all_for_run` rebuilds run, stage, approval, artifact, artifact contract, artifact-noise, and runtime-health projections (`projections.rs`, lines 1168-1190); integration test seeds stale artifact-noise/runtime-health rows, runs startup repair, and asserts refreshed artifact counts plus side-effect/continuation counts (`integration.rs`, lines 439-574).
- Mapping: Restart recovery now refreshes the P087 projections that were most likely to go stale after daemon shutdown/restart.
- Gap/note: The test exercises restart repair logic in-process; it does not launch a packaged daemon binary.

### REQ-008: Storage health metrics and exit criteria are exposed

- Proposal source: `§14`, lines 448-468; `§15`, lines 471-515; acceptance criteria 6-7, lines 594-595.
- Status: Implemented.
- Evidence types: code, telemetry, tests-run, config.
- Evidence references: storage health reports writer alive/lanes/wait/transaction duration/rejection, WAL, projections, read-path metrics, reaper status, and hot-read state (`storage_health.rs`, lines 406-470); runtime-health summary exposes side-effect and continuation counts (`storage_health.rs`, lines 742-767); threshold table includes write wait, WAL, evidence orphan bytes, hot-read violations, reaper SLA, projection backlog, runs-list latency, and projection lag thresholds (`storage_health.rs`, lines 1201-1217); rollout readback exposes P087 status fields (`storage_health.rs`, lines 770-825); rollout fixture gate verifies required metrics (`scripts/test-gate.sh`, lines 8012-8073).
- Mapping: Exit criteria are operationalized as readback fields, thresholds, rollout evidence, and a failing gate when required fields/metrics disappear.
- Gap/note: The gate verifies metric presence and targeted SLO tests; real-run threshold decisions still require operational data, which the proposal explicitly placed in Phase 5.

### REQ-009: Gate enforcement covers P087 regressions

- Proposal source: `§16 Phase 4`, lines 544-549; `§17`, lines 560-581.
- Status: Implemented.
- Evidence types: tests-run, config.
- Evidence references: gate verifies unique DB migration versions (`scripts/test-gate.sh`, lines 7830-7843), requires non-zero Rust test selection (`scripts/test-gate.sh`, lines 7844-7860), runs P087 DB/MCP/auth/engine/GraphQL tests (`scripts/test-gate.sh`, lines 7862-7870), verifies UI/schema/evidence/registry/metrics/negative fixtures and invalidation wiring (`scripts/test-gate.sh`, lines 7872-8108).
- Mapping: The canonical proposal gate would fail on the historical duplicate migration issue, missing readback fields, missing metrics, missing maintenance registry entries, missing typed errors, and missing production invalidation wiring.
- Gap/note: Same-tree `proposal-087` gate passed during this audit.

### REQ-010: Dependent proposals can rely on the storage tiering contract

- Proposal source: `§10-12`, lines 379-418; acceptance criterion 8, line 596.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: runtime-health projection computes unresolved side-effect count and continuation active count (`projections.rs`, lines 696-714; `storage_health.rs`, lines 742-767); artifact-noise projection computes compaction readiness inputs (`projections.rs`, lines 597-657; `storage_health.rs`, lines 696-739); `rebuild_all_for_run` includes artifact-contract and artifact-noise projections (`projections.rs`, lines 1177-1186); gate runs P077 and P088 compatibility list/readback tests (`scripts/test-gate.sh`, lines 7864-7866).
- Mapping: P038 compaction readiness, P078 side-effect unresolved count, P086 continuation active count, and P077/P088 list compatibility can rely on compact projections rather than deep hot-read scans.
- Gap/note: P086-specific continuation status tooling was not separately live-tested here, but its required projection count is covered by runtime-health projection rebuild/readback.

## Reviewer / Lens Scorecard

| Lens | Conformance | Readiness | Top risk | Confidence |
|---|---|---|---|---|
| Proposal conformance | Implemented | Ready with Risks | Dirty mixed staged/unstaged handoff | High |
| Rust architecture | Pass | Ready with Risks | Projection invalidation now spans multiple repos; keep operation registry and migrations committed together | High |
| Rust reliability | Pass | Ready with Risks | Live restart/soak not run; targeted in-process restart proof exists | Medium-High |
| Rust performance | Pass | Ready with Risks | SLO evidence is targeted gate/load test, not production workload telemetry | Medium-High |
| API contract | Pass | Ready with Risks | Additive GraphQL/MCP fixtures cover compatibility; full client runtime not exercised | High |
| Observability/rollout | Pass | Ready with Risks | P087 gate is strong; full regression gate not run | Medium-High |

## Routed Specialist Findings

### READY-001: Dirty mixed worktree makes handoff fragile

- Reviewer: readiness audit
- Severity: Minor
- Confidence: High
- Related requirements: REQ-009
- Evidence types: diff, config
- Evidence references: `git status --short --branch` shows staged and unstaged modifications, untracked P087 migrations/modules/evidence, prior audit reports, `.junie/plans/`, and `Chainworks ForgeTests/PreviewSupport+Tests.swift`.
- Why it matters: The same-tree gate passed against the current mixed worktree. If only the staged subset or only the unstaged subset is handed off, the P087 implementation can lose critical pieces such as production invalidation wiring, diagnostics query fields, or gate checks.
- Recommended action: Before merge/commit, stage and review the complete P087 implementation as one coherent change set, and either exclude or explicitly classify unrelated untracked files.
- Acceptance criteria: `git status` contains only intentional P087 files plus the generated audit report, or unrelated files are moved out/committed separately; `./scripts/test-gate.sh proposal-087` still passes on that exact staged tree or final branch.

### OPS-001: Successful proof is focused, not full-release coverage

- Reviewer: `observability_rollout_reviewer`
- Severity: Note
- Confidence: High
- Related requirements: REQ-007, REQ-009
- Evidence types: tests-run
- Evidence references: same-tree `./scripts/test-gate.sh proposal-087` passed; no `./scripts/test-gate.sh full`, remote UI gate, packaged daemon restart, or live daemon soak was run during this audit.
- Why it matters: The proposal allows a canonical proposal gate for readiness, and that gate is strong enough to support `Ready with Risks`. It does not prove every unrelated repository regression or packaged-runtime integration path.
- Recommended action: Use `proposal-087` as the P087 sign-off gate, then run broader branch-level gates according to normal merge policy if this work is being promoted beyond proposal acceptance.
- Acceptance criteria: P087 remains green; broader gate requirements are either run or explicitly deferred by the branch owner.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Build or canonical gate status | Pass | `./scripts/test-gate.sh proposal-087` passed in the audited worktree |
| Core service flow validation | Pass | MCP liveness, projection-only list, runtime/storage health, typed errors, repair tools, restart rebuild covered by targeted tests |
| UI/UX states | Not in scope | No new user-facing UI workflow; Swift diagnostics query and projection-lag tokens checked by gate |
| Accessibility/localization/privacy/permissions | Not in scope for UI; auth covered | P087 storage tools operator-only tests passed |
| Critical tests executed | Pass | DB, MCP, auth, engine integration, GraphQL, compatibility tests |
| Full regression or canonical proposal gate on audited tree | Pass | Canonical `proposal-087` gate passed on HEAD `569b297e58582153eb3601f91d18e7aa97d9a6f2` with current worktree changes |
| Release/handoff hygiene | Risk | Dirty mixed staged/unstaged/untracked worktree remains |

## Verification Log

Commands and checks run in `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82`:

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...087...md`
  - Result: selected `..._IMPLEMENTATION_AUDIT_R7.md`.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...087...md`
  - Result: no prior proposal-review artifacts found.
- `./scripts/test-gate.sh proposal-087`
  - Result: passed.
  - Gate evidence observed:
    - DB migration versions verified.
    - DB P087 tests: 19 passed.
    - MCP P087 tests: 16 passed, including liveness under maintenance, hot-read circuit behavior, projection-only `runs.list`, typed storage-health errors, and seeded `runs.list` p95 budget.
    - Auth P087 tests: 2 passed, including operator-only storage tools.
    - Engine integration P087 restart rebuild test: 1 passed.
    - GraphQL storage-health compatibility tests: storage-health v1 and P087 tests passed.
    - P077/P088 list compatibility tests passed under the P087 gate.
    - UI/schema/evidence/rollout/static checks passed and printed `P087 UI, schema, and evidence verified`.
  - Non-blocking warnings: existing Rust dead-code warnings in `db`, `acp`, and `engine` crates.

## Final Verdict

Overall conformance: **Implemented**.

Overall implementation readiness: **Ready with Risks**.

P087 now satisfies the proposal contract with same-tree canonical gate evidence. The main remaining risk is release hygiene rather than conformance: the implementation is spread across staged, unstaged, and untracked files, so handoff must preserve the exact tree that passed the gate.

Recommended next actions:

1. Normalize the worktree for handoff: stage/commit all intentional P087 implementation, fixtures, migrations, docs, and this audit report together, and separate unrelated untracked files.
2. Re-run `./scripts/test-gate.sh proposal-087` after staging/commit.
3. Run broader branch gates only if this work is being promoted under normal merge policy beyond P087 acceptance.
