# Proposal 075 Implementation Audit Report

## 0. Metadata
| Field | Value |
|---|---|
| Proposal | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md` |
| Proposal state | `Active` |
| Implementation target | branch `cw/implement-proposal-075-local-p/4aeb45a9` in explicit worktree |
| Compare base | `origin/main` merge-base `70b03d1af641bbcb76a745449cc18f9a8fddea4c` |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-075-local-p-4aeb45a9` |
| Git SHA | `c1f83656db3869eb1fc81070c14157897fe57098` |
| Working tree status | Clean before report generation |
| Audit timestamp | `2026-05-08T18:47:42Z` |
| Report path | `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling_IMPLEMENTATION_AUDIT_R1.md` |
| Platform/product scope | `data`, `worker`, `API`, `rollout`, `cross-stack` |

## 1. Verdict
- Overall Conformance: `Not Implemented`
- Overall Implementation Readiness: `Not Ready`
- Reviewer Selection Reuse: `Not reused`
- Audit Confidence: `High`
- Same-tree full regression / canonical gate: `Passed`
- Highest-risk blockers:
  1. Runtime writes are not actually routed through `DbWriter`; the implementation adds the gateway mostly inside `db` and tests while command, engine, and repository writes still use `SqlitePool` / `begin_immediate_with_retry` directly.
  2. High-volume runtime evidence producers are not wired to the new evidence spool path; `write_spool_file` and `insert_idempotent_via_dbwriter` are only used by db tests and diagnostics/orphan tooling, not by ACP/transcript/tool/runtime-event producers.
  3. The claimed fail-closed gate does not diff direct write call sites against the allowlist and accepts all temporary rollout bypasses at phase 8, so it can pass while the proposal's closeout routing promises remain unmet.

## 2. Prior Proposal-Review Reuse
- Prior artifacts found: none. `discover_prior_review.py` returned an empty artifact list for this proposal.
- Prior selected reviewers: none.
- Prior rejected close alternatives: none.
- Prior stacks / surfaces / risks: derived from current proposal and implementation only.
- Prior required changes before implementation: none found in reusable review artifacts.
- Reuse decision: `Not reused`.
- Delta from prior selection: full route performed from proposal plus implementation surfaces.
- Reasoning: no prior proposal-review selection artifact was available beside the proposal or through the helper.

## 3. Current Reviewer Routing
| Reviewer | Discipline / Stack | Why Selected | Evidence IDs | Reused From Proposal Review? | Notes |
|---|---|---|---|---|---|
| `rust_arch_reviewer` | Rust persistence/worker architecture | DbWriter, repository boundary, migrations, runtime write ownership | E1, E2, E3 | No | Primary implementation surface |
| `rust_reliability_reviewer` | Queueing, deadlines, shutdown, recovery | Bounded lanes, coalescing, orphan recovery, direct-write bypasses | E4, E5, E8 | No | Backpressure/recovery is central |
| `api_contract_reviewer` | GraphQL/MCP contract | `storageHealth` GraphQL and MCP diagnostic tools | E6, E7 | No | Proposal has explicit schema/API promises |
| `observability_rollout_reviewer` | Gates, metrics, diagnostics, rollout | P075 gate, storage health, baseline, readiness evidence | E8, E9, E10 | No | Gate and rollout are acceptance criteria |
| `chainworks_execution_truth_reviewer` | Durable Run/Stage/Agent/artifact truth | Runtime write routing and evidence metadata affect execution truth | E2, E3, E11 | No | Repo-local routing selects this for durable truth changes |

### Rejected Close Alternatives
| Reviewer | Why Rejected | Evidence IDs |
|---|---|---|
| `rust_performance_reviewer` | Hard cap reached; no explicit throughput benchmark target beyond contention/pressure metrics. Performance risk is covered through reliability and observability findings. | E4, E8 |
| `macos_ui_reviewer` | Proposal explicitly excludes SwiftUI/AppKit mutation/control changes; Swift app changes are incidental diagnostics-export context. | P075 lines 57-65 |

## 4. Proposal Contract Summary
- In scope: Rust control-plane local persistence, DbWriter write gateway, write classes A-D, evidence spooling, projection coalescing, telemetry rollups, storageHealth GraphQL/MCP diagnostics, migrations, gates, rollout kill-switches, docs/reference truth.
- Out of scope: P078 durable side-effect ledger, P066 provider cache expansion, replacing SQLite, SwiftUI/AppKit mutation/control changes, remote services, workflow YAML, agent catalog, protobuf/OpenAPI.
- Locked decisions: SQLite remains compact canonical local state; high-volume raw runtime evidence must move to files; runtime writes must route through `DbWriter` or source-controlled temporary bypasses; no independent BEGIN IMMEDIATE retry primitive; diagnostics are additive.
- Primary implementation flows:
  1. Runtime command/canonical state mutation flows through `DbWriter` with class/lane/idempotency/deadline metadata.
  2. ACP/transcript/tool/runtime evidence writes file first, then Class C metadata through `DbWriter`.
  3. Repeated projection/status updates coalesce and flush on cadence/boundaries.
  4. Storage health consumers read GraphQL/MCP parity diagnostics with units/freshness/thresholds/kill-switch state.
  5. Proposal gate blocks unapproved direct writes and raw high-volume SQLite evidence persistence.
- Acceptance criteria: lines 713-726 in the proposal.
- Test / evidence requirements: lines 677-711 in the proposal.

## 5. Implementation Evidence Summary
- Changed files / modules / crates inspected: `control-plane/crates/db/src/writer.rs`, `write_class.rs`, `evidence_spool.rs`, `repos/evidence_spool_refs.rs`, `repos/storage_health.rs`, migrations `046`-`048`, `write-bypass-allowlist.toml`, `write-operation-registry.toml`, `scripts/test-gate.sh`, GraphQL schema, MCP storage tools, engine/command/repository write call sites, diagnostics bundle, README/reference docs.
- Tests found: `control-plane/crates/db/tests/proposal_075_dbwriter.rs`; db module tests for writer, write_class, evidence_spool, evidence_spool_refs, allowlist, operation_registry.
- Tests run: `./scripts/test-gate.sh proposal-075` on audited tree/HEAD; passed.
- Runtime checks: none; audit was code/test/gate based.
- Benchmarks: none found or run.
- API/schema/migration checks: code inspection of GraphQL JSON readback, MCP storage tool specs, migrations `046`-`048`.
- Rollout/telemetry checks: code inspection of P075 gate, storage health repo, baseline evidence placeholder, reference docs.
- Evidence gaps: no daemon runtime proof, no canary proof, no actual producer adopting the spool path, no diagnostics export proof.

Evidence IDs:
- E1: `control-plane/crates/db/src/writer.rs` implements DbWriter lanes/coalescing/shutdown/heartbeat.
- E2: `control-plane/crates/engine/src/command_handler.rs` still uses direct `begin_immediate_with_retry` for command writes at lines 1715-1717, 1966-1968, 2437-2442.
- E3: repositories still expose direct `SqlitePool` mutation paths, e.g. `repos/runs.rs` lines 20-23 and 153-171; `repos/stages.rs` lines 8-11 and 120-230.
- E4: `control-plane/crates/db/src/repos/evidence_spool_refs.rs` exposes DbWriter metadata helpers at lines 858-990.
- E5: `control-plane/crates/db/src/evidence_spool.rs` implements file write/checksum/fsync/rename/orphan sweep.
- E6: `control-plane/crates/graphql-server/src/schema.rs` exposes `storage_health` as `Json<serde_json::Value>` at lines 529-535.
- E7: `control-plane/crates/mcp-server/src/tools/storage.rs` defines storage diagnostic tools at lines 6-76.
- E8: `scripts/test-gate.sh` P075 gate logic at lines 5464-5588.
- E9: `docs/evidence/p075/phase1-baseline.md` remains `_TO FILL_` / capture pending at lines 1-82.
- E10: `README.md` still describes P075 as Phase 1 scaffold/deferred at line 112.
- E11: `control-plane/crates/engine/src/evidence.rs` still writes evidence packet JSON directly to SQLite at lines 130-131.

## 6. Proposal Fidelity / Divergence
### Matches
- DbWriter and write classification types exist with documented lanes, deadlines, validation, coalescing, shutdown admission, and WriteResult variants.
- Evidence spool metadata schema and repository exist, with strong path/checksum/summary validation.
- Evidence file spooling primitive performs temp write, checksum, fsync, rename, and parent fsync.
- GraphQL and MCP storage diagnostic entry points exist.
- Canonical P075 gate passes on the audited tree.

### Divergences
- Runtime write routing is not adopted by actual command/engine/repository producers.
- Runtime high-volume evidence producer adoption is absent.
- P075 gate validates TOML shape and operation registry literals but does not diff direct write call sites against the allowlist or detect row-per-chunk evidence writes.
- GraphQL `storageHealth` is a JSON blob with `generatedAt/state/schemaVersion`, not the typed `StorageHealth` contract with `updatedAt`, `staleAfterMs`, `isStale`, `dbState`, `writer.lanes`, `projections`, `thresholds`, and degraded/unavailable semantics.
- MCP tools do not match the proposal input/output contract for `storage.write_pressure`, `storage.evidence_spool_summary`, or dry-run bounded reconciliation.
- Orphan sweep does not implement the proposal's 1000-file/64-MB per-pass bounds, terminal-run pending-delete behavior, or full orphan/checksum/pending-delete summary fields.
- Baseline evidence remains a placeholder that says Phase 2 must not begin until populated.

### Ambiguities / Evidence Gaps
- The proposal permits temporary bypasses during phased rollout, but Phase 8 says temporary entries are retired. Current gate treats `expires_after_phase = 8` as valid at phase 8.
- Some direct writes may be intentionally carried from prior proposals, but the implementation does not prove that they are classified, registered, measured, or bounded by DbWriter.

## 7. Requirement Summary
| Status | Count |
|---|---:|
| Implemented | 5 |
| Partially Implemented | 5 |
| Missing | 7 |
| Not Verifiable | 1 |
| Out of Scope | 0 |

## 8. Requirement Audit

### REQ-001 Runtime writes route through DbWriter or bounded allowlist
- Proposal Source: Goals lines 48-49; acceptance line 715; repository boundary lines 400-412.
- Status: `Partially Implemented`
- Implementation Mapping: DbWriter exists; allowlist exists with 34 bypasses; many real runtime write paths still use `SqlitePool`/`begin_immediate_with_retry` directly.
- Evidence Type: code, config, tests-run.
- Evidence: E1, E2, E3, E8.
- Gap / Note: The proposal allows temporary bypasses, but the closeout slice has not routed runtime command/repository producers through DbWriter, and the gate does not verify owner-to-call-site alignment.

### REQ-002 Write classes, lanes, deadlines, capacities, policies, idempotency, replay, results
- Proposal Source: write classes lines 77-133; lanes lines 140-184; deadlines/results lines 223-247; acceptance line 716.
- Status: `Implemented`
- Implementation Mapping: `write_class.rs`, `writer.rs`, operation registry.
- Evidence Type: code, tests-run.
- Evidence: `write_class.rs`; `writer.rs`; `./scripts/test-gate.sh proposal-075` passed.
- Gap / Note: Core type/system implementation is present.

### REQ-003 Barrier writes serialized, prioritized, measured, not starved
- Proposal Source: DbWriter lines 134-139; backpressure lines 194-212; acceptance line 717.
- Status: `Partially Implemented`
- Implementation Mapping: DbWriter executor prioritizes Class A lanes and records queue/tx timings in logs.
- Evidence Type: code, tests-run.
- Evidence: `writer.rs` lines 942-1155; proposal gate P075 tests pass.
- Gap / Note: Actual barrier command paths still bypass DbWriter, so the behavior is proven only for synthetic/db-test submissions, not real runtime barrier writes.

### REQ-004 High-volume runtime evidence spools to files with metadata, not row-per-chunk SQLite
- Proposal Source: goals line 50; evidence spooling lines 295-325; acceptance line 718.
- Status: `Missing`
- Implementation Mapping: File spool primitive and metadata repository exist.
- Evidence Type: code, tests-found.
- Evidence: E4, E5; search found `write_spool_file` / `insert_idempotent_via_dbwriter` only in db tests and storage diagnostics/orphan tooling.
- Gap / Note: No ACP/transcript/tool/stdout/runtime-event producer is wired to the spool path. Fake-stream tests do not prove runtime adoption.

### REQ-005 Evidence spool reader statuses
- Proposal Source: reader statuses lines 391-398; acceptance line 719.
- Status: `Partially Implemented`
- Implementation Mapping: `EvidenceSpoolRefStatus` enum includes the required statuses.
- Evidence Type: code, tests-run.
- Evidence: `repos/evidence_spool_refs.rs` lines 88-128; db tests pass.
- Gap / Note: Reader behavior for `legacy_absent`, `missing_file`, `checksum_mismatch`, and `pending_delete` is not surfaced end-to-end through storageHealth/GraphQL/MCP as proposed.

### REQ-006 Startup orphan recovery and manual reconcile diagnostics
- Proposal Source: orphan recovery lines 318-325; MCP reconcile lines 581-591; acceptance line 720.
- Status: `Partially Implemented`
- Implementation Mapping: `sweep_evidence_orphans` and `storage.reconcile_evidence_orphans` exist.
- Evidence Type: code, tests-run.
- Evidence: `evidence_spool.rs` lines 260-388; `mcp-server/src/tools/storage.rs` lines 63-72; P075 tests pass.
- Gap / Note: Sweep lacks per-pass caps, terminal-run delete scheduling, dry-run/maxfiles/runid parameters, and the proposed diagnostic counts.

### REQ-007 Projection invalidations are coalesced with mandatory periodic and max-age flushes
- Proposal Source: coalescing lines 275-294; projection invalidation lines 425-433; acceptance line 721.
- Status: `Partially Implemented`
- Implementation Mapping: Class B coalescing exists inside DbWriter.
- Evidence Type: code, tests-run.
- Evidence: `writer.rs` lines 736-914; P075 Class B tests pass.
- Gap / Note: Real projection invalidation producers are not routed to DbWriter/coalescing; implementation only proves the primitive.

### REQ-008 Telemetry rollup with memory cap, TTL, logs, and no barrier priority
- Proposal Source: Class D lines 119-133; telemetry metrics lines 593-609; acceptance line 722.
- Status: `Missing`
- Implementation Mapping: constants and a `storage_write_pressure_snapshots` repository exist.
- Evidence Type: code.
- Evidence: `repos/storage_health.rs` lines 16-63; `writer.rs` constants.
- Gap / Note: No in-memory rollup producer, Class D snapshot submitter, TTL retention purge, drop counters, or runtime metrics pipeline is implemented.

### REQ-009 storageHealth GraphQL and MCP diagnostics expose typed units/freshness/thresholds/kill-switch/degraded cases
- Proposal Source: GraphQL contract lines 508-543; MCP contract lines 544-591; acceptance line 723.
- Status: `Missing`
- Implementation Mapping: GraphQL `storage_health` and MCP tools exist.
- Evidence Type: code, API/schema.
- Evidence: E6, E7; `storage_health.rs` lines 80-153.
- Gap / Note: The implementation returns generic JSON with missing typed fields and null writer heartbeat, and MCP outputs do not match the proposed input/output schema or GraphQL parity requirements.

### REQ-010 Forward-only migration and downgrade/re-upgrade behavior documented
- Proposal Source: data/schema lines 448-452; acceptance line 724.
- Status: `Partially Implemented`
- Implementation Mapping: migrations are present and reference docs mention new tables.
- Evidence Type: migration, docs.
- Evidence: migrations `046`, `047`, `048`; `docs/reference/rust-control-plane.md` lines 470-493.
- Gap / Note: Downgrade/re-upgrade behavior is not clearly documented in current reference truth beyond proposal text, and baseline/reference docs are inconsistent.

### REQ-011 P078 can use barrier/evidence foundation without SQLite becoming event stream
- Proposal Source: goals line 54; non-goals lines 57-65; acceptance line 725.
- Status: `Not Verifiable`
- Implementation Mapping: barrier/evidence primitives exist.
- Evidence Type: code, inference.
- Evidence: E1, E4, E5.
- Gap / Note: Since actual runtime producers and side-effect-adjacent truth are not routed, the audit cannot prove the foundation is usable without preserving the current direct-write/event-stream risks.

### REQ-012 Proposal gate fails on unapproved direct runtime writes and high-volume raw evidence persisted directly into SQLite
- Proposal Source: gate behavior lines 409-412; gate must prove lines 703-711; acceptance line 726.
- Status: `Missing`
- Implementation Mapping: `scripts/test-gate.sh proposal-075` passes.
- Evidence Type: code, tests-run.
- Evidence: E8; gate output: `P075 fail-closed registry check passed: 34 bypasses, 5 operations, 3 observed db/src operation literals`.
- Gap / Note: The gate does not scan/diff write call sites against the allowlist; it only validates TOML shape/expiry and observed `operation_name` literals under `db/src`.

### REQ-013 Diagnostics export includes storageHealth and evidence spool summary
- Proposal Source: diagnostics line 675; required tests line 696.
- Status: `Missing`
- Implementation Mapping: macOS diagnostics bundle still exports status/build/log/port/crash/system inputs only.
- Evidence Type: code.
- Evidence: `Chainworks Forge/Support/DiagnosticsBundle.swift` lines 111-210; search found no storageHealth/evidence spool integration.
- Gap / Note: Required evidence/test coverage is absent.

### REQ-014 Baseline capture before phased rollout/canary
- Proposal Source: telemetry baseline line 595; phases lines 821-824; open question resolution lines 847-852.
- Status: `Missing`
- Implementation Mapping: placeholder evidence file exists.
- Evidence Type: docs/evidence.
- Evidence: E9.
- Gap / Note: The file explicitly says capture is pending and Phase 2 must not begin until populated.

### REQ-015 Write operations use P061 retry primitive; no independent retry primitive
- Proposal Source: transaction rules lines 186-192.
- Status: `Implemented`
- Implementation Mapping: DbWriter closures and evidence metadata repos use `begin_immediate_with_retry`; no new retry primitive found for P075.
- Evidence Type: code.
- Evidence: `writer.rs` comments and execute path; `evidence_spool_refs.rs` lines 588-590, 681-683, 895-897.
- Gap / Note: Existing direct writes still use raw `pool.begin()` in many repos, but no separate P075 retry primitive was introduced.

### REQ-016 Evidence spool metadata schema and constraints
- Proposal Source: schema lines 454-508; evidence contract lines 327-390.
- Status: `Implemented`
- Implementation Mapping: migrations `046`, `048`; Rust validation.
- Evidence Type: migration, code, tests-run.
- Evidence: migrations `046` and `048`; `evidence_spool_refs.rs` validation tests passed.
- Gap / Note: Strong schema/validation implementation.

### REQ-017 MCP storage tools exist with dot-name/codex-compatible mapping
- Proposal Source: MCP contract lines 544-591.
- Status: `Implemented`
- Implementation Mapping: `storage.health`, `storage.write_pressure`, `storage.evidence_spool_summary`, `storage.reconcile_evidence_orphans` exist and canonical aliases are mapped.
- Evidence Type: code.
- Evidence: `mcp-server/src/tools/storage.rs` lines 6-76; `mcp-server/src/tools/mod.rs` lines 84-120.
- Gap / Note: Existence is implemented; schema/behavior parity is covered separately in REQ-009.

### REQ-018 Current docs/reference truth synchronized
- Proposal Source: proposal status line 6; Phase 8 line 828; acceptance line 724.
- Status: `Partially Implemented`
- Implementation Mapping: `docs/reference/rust-control-plane.md` has P075 sections.
- Evidence Type: docs.
- Evidence: `docs/reference/rust-control-plane.md` lines 361-420; README line 112.
- Gap / Note: Reference docs overclaim runtime routing and storageHealth heartbeat, while README still says runtime routing/spooling/GraphQL/MCP/gate are deferred.

## 9. Prior Review Finding Follow-Through
| Prior Finding / Required Change | Status | Evidence | Notes |
|---|---|---|---|
| No reusable prior proposal-review findings found | `Not Verifiable` | discovery helper returned no artifacts | Audit routed from current proposal and implementation |

## 10. Reviewer Scorecard
| Reviewer | Result | Confidence | Evidence Completeness | Critical | Major | Minor | Notes |
|---|---|---|---|---:|---:|---:|---|
| `rust_arch_reviewer` | `Fail` | High | High | 1 | 1 | 0 | Runtime ownership boundary not adopted |
| `rust_reliability_reviewer` | `Fail` | High | High | 0 | 2 | 0 | Spool/recovery primitives incomplete end-to-end |
| `api_contract_reviewer` | `Fail` | High | High | 1 | 1 | 0 | storageHealth/MCP contract shape mismatches |
| `observability_rollout_reviewer` | `Fail` | High | High | 1 | 2 | 1 | Gate/baseline/diagnostics gaps |
| `chainworks_execution_truth_reviewer` | `Fail` | High | Medium | 0 | 1 | 0 | Durable truth still bypasses proposed write gateway |

## 11. Routed Specialist Findings

### 11.1 Critical

#### ARCH-001 Runtime mutation paths still bypass the DbWriter gateway
- Reviewer: `rust_arch_reviewer`
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-001, REQ-003, REQ-007, REQ-011
- Evidence Type: code
- Evidence: `command_handler.rs` lines 1715-1717 and 1966-1968; `runs.rs` lines 20-23 and 153-171; `stages.rs` lines 8-11 and 120-230.
- Why It Matters: The proposal's central architecture is a single bounded, classified writer gateway. The current implementation proves the gateway in isolation but leaves real command/repository state mutations on direct pool paths, so priority lanes, deadlines, admission control, operation registry, and post-cancel observability do not govern core runtime writes.
- Recommended Action: Thread `DbWriter` through command/engine write ownership and convert runtime mutation producers one class at a time, leaving only explicit permanent or temporary bypasses with verified call-site coverage.
- Acceptance Criteria: Representative StartRun/Approve/Retry/Cancel/run/stage/agent/projection writes submit `WriteOperation`s through DbWriter; direct write inventory maps every remaining bypass to actual call sites; P075 tests include at least one real command/repository flow through DbWriter.

#### API-001 storageHealth GraphQL/MCP contract is not the proposed typed parity surface
- Reviewer: `api_contract_reviewer`
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-009, REQ-017
- Evidence Type: code, API/schema
- Evidence: GraphQL returns `Json<serde_json::Value>` at `schema.rs` lines 529-535; `storage_health.rs` emits `schemaVersion/state/generatedAt` and null heartbeat fields at lines 80-153; MCP `storage.write_pressure` returns only `{ "latestSnapshot": latest }` at `storage.rs` lines 55-59.
- Why It Matters: Operators and clients cannot rely on the typed `StorageHealth` contract promised by the proposal, and MCP/GraphQL parity is not meaningful when the surfaces expose different shapes and omit mandatory freshness, thresholds, projections, degraded/unavailable, and lane health fields.
- Recommended Action: Implement typed GraphQL objects/enums or a fully documented versioned JSON schema equivalent to the proposal, then make MCP output structurally equivalent and test the full contract.
- Acceptance Criteria: `storageHealth` exposes updatedAt/staleAfterMs/isStale/dbState/writer/wal/projections/evidenceSpool/killSwitches/thresholds as proposed or an explicitly versioned accepted replacement; MCP tools accept the proposed inputs and return parity units/enums; contract tests fail on missing fields.

#### OPS-001 P075 gate passes without checking the direct-write or raw-evidence promises
- Reviewer: `observability_rollout_reviewer`
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-001, REQ-012
- Evidence Type: code, tests-run
- Evidence: `scripts/test-gate.sh` lines 5464-5588; gate output `P075 fail-closed registry check passed: 34 bypasses, 5 operations, 3 observed db/src operation literals`.
- Why It Matters: The gate is the proposal's enforcement mechanism, but it only validates allowlist/registry shape and operation literals under `db/src`. It does not diff direct write call sites against allowlist patterns, does not verify retirement of temporary bypasses at Phase 8, and does not detect row-per-chunk evidence persistence.
- Recommended Action: Add a fail-closed source scan/inventory that maps direct write owners to allowlist entries and detects high-volume evidence tables/fields or stream-chunk insertions outside the spool path.
- Acceptance Criteria: Introducing an unallowlisted `pool.begin`, `execute(pool)` write, or raw evidence chunk insert causes `./scripts/test-gate.sh proposal-075` to fail; `expires_after_phase = 8` temporary rollout entries are retired or treated as expired after closeout.

### 11.2 Major

#### REL-001 Evidence spool primitives are not connected to actual high-volume producers
- Reviewer: `rust_reliability_reviewer`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-004, REQ-006, REQ-011
- Evidence Type: code, tests-found
- Evidence: `write_spool_file` and `insert_idempotent_via_dbwriter` are referenced in db tests and storage/orphan tooling only; no engine/ACP/transcript/tool producer references were found. `engine/src/evidence.rs` still writes stage evidence JSON directly into `stage_executions.evidence_packet_json` at lines 130-131.
- Why It Matters: The proposal is meant to stop SQLite from becoming a runtime event stream. Without producer adoption, the new spool path is dormant and cannot reduce actual write pressure.
- Recommended Action: Canary one real ACP/transcript/tool/runtime-event producer through `write_spool_file` plus Class C metadata, then migrate remaining high-volume producers behind kill-switches.
- Acceptance Criteria: A representative runtime workflow produces evidence files under `artifact_root/evidence/runs/...` and compact `evidence_spool_refs`; SQLite row counts stay one-per-logical-object; tests cover the real producer path, not only a fake stream.

#### REL-002 Orphan recovery and reconcile tooling do not meet bounded recovery semantics
- Reviewer: `rust_reliability_reviewer`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-006
- Evidence Type: code
- Evidence: `evidence_spool.rs` lines 260-388; `mcp-server/src/tools/storage.rs` lines 63-72.
- Why It Matters: Unbounded sweeps can become operational hazards, and terminal-run delete scheduling/checksum mismatch accounting are part of the recovery contract that lets operators trust file/metadata divergence handling.
- Recommended Action: Implement run-scoped, dry-run-capable, bounded reconcile with max files/bytes, checksum mismatch reporting, terminal-run pending delete scheduling, and resumable maintenance state.
- Acceptance Criteria: MCP accepts `runid`, `dryrun`, and `maxfiles`; sweeps stop at 1000 files or 64 MB by default; summaries report scanned/recovered/checksum_mismatch/scheduled_delete/skipped/errors and do not block startup-critical paths.

#### OPS-002 Baseline/canary evidence is still a placeholder
- Reviewer: `observability_rollout_reviewer`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-014
- Evidence Type: docs/evidence
- Evidence: `docs/evidence/p075/phase1-baseline.md` lines 1-82 says capture is pending and all values are `_TO FILL_`.
- Why It Matters: The proposal uses baseline metrics to judge thresholds and canary promotion. Without baseline/canary evidence, pressure regressions and rollout decisions are subjective.
- Recommended Action: Capture the prescribed file-backed workload baseline and commit actual p50/p95/retry/WAL/direct-write inventory values before claiming Phase 2+ readiness.
- Acceptance Criteria: Baseline file has real values, capture metadata, workload description, and direct-write inventory counts; rollout/canary evidence compares against warn/critical thresholds.

#### OPS-003 Diagnostics export does not include storageHealth or evidence spool summary
- Reviewer: `observability_rollout_reviewer`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-013
- Evidence Type: code
- Evidence: `DiagnosticsBundleBuilder.export` at `Chainworks Forge/Support/DiagnosticsBundle.swift` lines 111-210; no storageHealth/evidence spool integration found by search.
- Why It Matters: Support/debuggability was part of the required gate coverage. Without export inclusion, operators cannot preserve P075 storage state in the standard diagnostic bundle.
- Recommended Action: Add storageHealth, latest storage write pressure, and evidence spool summary snapshots to diagnostics export with redaction and tests.
- Acceptance Criteria: Diagnostics bundle contains stable storage files; tests assert presence/absence flags and redaction behavior; export remains useful when daemon storageHealth is unavailable.

#### TRUTH-001 Reference docs overstate implemented runtime truth
- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-018
- Evidence Type: docs, code
- Evidence: `docs/reference/rust-control-plane.md` lines 365-391 claims all non-test runtime writes route through DbWriter and `storageHealth.writer.alive` surfaces heartbeats; code evidence shows direct runtime writes and `storage_health.rs` sets `writer.alive` to null. README line 112 still says runtime routing/spooling/GraphQL/MCP/gate are deferred.
- Why It Matters: Chainworks reference docs are durable system truth. Overclaiming completed routing and diagnostics can cause future proposals and closeout decisions to build on behavior that is not actually present.
- Recommended Action: Either complete runtime adoption or downgrade docs to describe implemented primitives versus incomplete producer adoption, and reconcile README/reference statements.
- Acceptance Criteria: Current docs match code: no claim of full runtime DbWriter routing until real producers are routed; storageHealth claims match actual fields; README no longer contradicts reference docs.

### 11.3 Minor

#### OPS-004 P075 test/gate labels still say "Phase 1" while running later-slice checks
- Reviewer: `observability_rollout_reviewer`
- Severity: `Minor`
- Confidence: `High`
- Related Proposal Items / REQs: REQ-012, REQ-018
- Evidence Type: code, tests-found
- Evidence: `scripts/test-gate.sh` lines 5467-5487 labels the gate "Phase 1" and writer "phase-1 pass-through"; `proposal_075_dbwriter.rs` header lines 1-8 says Phase 3+ tests are deferred even though later tests exist below.
- Why It Matters: Stale labels make audit/debug evidence harder to interpret and hide whether the gate is inventory, fail-closed, or closeout.
- Recommended Action: Rename gate/test labels to reflect the implemented closeout slice and enforcement mode.
- Acceptance Criteria: Gate output and test module comments accurately describe current behavior and enforcement coverage.

### 11.4 Notes
- The core DbWriter and evidence metadata validation code is materially stronger than a scaffold. The blockers are adoption, enforcement, and contract/readback completeness, not absence of all primitives.

## 12. Readiness Checklist
| Check | Status | Evidence / Note |
|---|---|---|
| Build or canonical gate status | Pass | `./scripts/test-gate.sh proposal-075` passed on `c1f83656...` |
| Proposal contract satisfied | Fail | Missing runtime routing, producer adoption, gate enforcement, typed diagnostics |
| Prior review blockers addressed | Not Verifiable | No prior review artifacts found |
| Tests cover committed behavior | Fail | Tests cover db primitives/fake stream, not real runtime producers or typed API parity |
| Critical tests executed | Pass | P075 gate executed |
| Core service flow runtime/integration validated where needed | Fail | No daemon/runtime producer validation |
| Empty/loading/error/offline/permission states covered where relevant | Not Applicable | No UI in proposal scope |
| Accessibility and localization risk acceptable where relevant | Not Applicable | No UI in proposal scope |
| API/schema compatibility acceptable | Fail | GraphQL/MCP shape mismatch |
| Migration/rollback path acceptable | Partial | Migrations exist; downgrade/re-upgrade behavior and baseline incomplete |
| Telemetry/observability sufficient | Fail | Telemetry rollup, baseline, diagnostics export, gate enforcement incomplete |
| Security/privacy risk acceptable | Partial | Strong path/summary validation; runtime exposure not fully wired |
| Privacy/permissions/entitlements reviewed where relevant | Not Applicable | No entitlement changes |
| Performance risk acceptable | Not Verifiable | No canary/baseline/bench evidence |
| Full regression suite or canonical full/proposal gate passed on audited tree/HEAD | Pass | P075 canonical gate passed |
| Release/handoff evidence sufficient | Fail | Implementation not ready despite gate pass |

## 13. Product / Metrics Overlay
- Leading metric: `storage_health_rollout_pass_rate` from proposal rollout contract.
- Guardrail metric: Class A latency, write-lock wait, queue depth, WAL size, orphan bytes below warn/critical thresholds.
- Decision checkpoint: Promote producer kinds only after canary soak and storageHealth/MCP parity.
- Rollout recommendation: Hold. Complete producer routing and enforcement gate before closeout.
- Instrumentation gaps: baseline capture, telemetry rollup producer, storageHealth typed thresholds, diagnostics export.

## 14. Verification Log
- Commands run:
  - `git status --short --branch`
  - `git merge-base origin/main HEAD`
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...`
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...`
  - focused `rg`/`nl` inspections across proposal, DbWriter, evidence spool, repos, GraphQL, MCP, gate, docs, diagnostics
  - `./scripts/test-gate.sh proposal-075` -> passed
- Files inspected:
  - `docs/proposals/075-local-persistence-write-budget-and-evidence-spooling.md`
  - `control-plane/crates/db/src/writer.rs`
  - `control-plane/crates/db/src/write_class.rs`
  - `control-plane/crates/db/src/evidence_spool.rs`
  - `control-plane/crates/db/src/repos/evidence_spool_refs.rs`
  - `control-plane/crates/db/src/repos/storage_health.rs`
  - `control-plane/crates/db/migrations/046_p075_evidence_spool_refs.sql`
  - `control-plane/crates/db/migrations/047_p075_storage_write_pressure_snapshots.sql`
  - `control-plane/crates/db/migrations/048_p075_evidence_path_constraints.sql`
  - `control-plane/crates/db/write-bypass-allowlist.toml`
  - `control-plane/crates/db/write-operation-registry.toml`
  - `control-plane/crates/db/tests/proposal_075_dbwriter.rs`
  - `scripts/test-gate.sh`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/mcp-server/src/tools/storage.rs`
  - `control-plane/crates/mcp-server/src/tools/mod.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/engine/src/evidence.rs`
  - `control-plane/crates/db/src/repos/runs.rs`
  - `control-plane/crates/db/src/repos/stages.rs`
  - `docs/evidence/p075/phase1-baseline.md`
  - `docs/reference/rust-control-plane.md`
  - `docs/reference/test-gates.md`
  - `README.md`
  - `Chainworks Forge/Support/DiagnosticsBundle.swift`
- Artifacts inspected:
  - No prior proposal-review artifacts found.
- Commands not run and why:
  - Full app/UI gates: not relevant to P075 and local UI smoke is disallowed by repository policy unless explicitly requested.
  - Live daemon/canary workload: not practical for a read-only proposal audit and no prepared runtime target was supplied.
  - Benchmarks/load tests: proposal gate did not define an executable benchmark command; no benchmark suite found for P075.

## 15. Recommended Next Actions
- MUST-01: Route at least the canonical command/run/stage/agent/projection mutation paths through `DbWriter`, or explicitly prove and gate each remaining temporary bypass.
- MUST-02: Wire one real high-volume runtime evidence producer to file spooling plus Class C metadata, then migrate remaining producers behind documented kill-switches.
- MUST-03: Replace the P075 gate's TOML-only check with fail-closed direct-write and raw-evidence call-site detection.
- MUST-04: Implement the typed GraphQL/MCP storageHealth parity contract, including freshness, thresholds, degraded/unavailable cases, writer lanes, projection health, and reconcile inputs.
- MUST-05: Populate baseline/canary evidence and include storage diagnostics in the diagnostics export.
- SHOULD-01: Reconcile README and reference docs so they distinguish implemented primitives from unimplemented runtime adoption.
