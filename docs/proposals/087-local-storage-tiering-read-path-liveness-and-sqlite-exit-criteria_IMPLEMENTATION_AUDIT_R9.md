# Proposal 087 Implementation Audit R9

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Proposal state | Draft, treated as Active for implementation audit |
| Proposal source state evidence | Proposal front matter shows `Status | Draft` at lines 3-8 |
| Implementation target | Current worktree explicitly requested as `cw-implement-proposal-087-local-s-b4edcf82` |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Branch | `cw/implement-proposal-087-local-s/b4edcf82` |
| Current HEAD | `6bafc7c4c7007552539dd4681594879e8376343e` |
| Compare base | Implicit current worktree; no PR base or commit range supplied |
| Working tree status | Dirty: nine modified files plus untracked migration `control-plane/crates/db/migrations/061_p087_hot_read_promotion_budget.sql` before this audit report |
| Audit timestamp | `2026-05-17T13:37:12Z` |
| Report path helper result | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria_IMPLEMENTATION_AUDIT_R9.md` |
| Overall Conformance | Implemented |
| Overall Implementation Readiness | Ready with Risks |
| Reviewer Selection Reuse | Not reused |
| Audit confidence | High for proposal conformance; medium-high for release handoff |

## Prior Review Reuse

The prior-review discovery helper returned no proposal-review artifacts for this proposal. Earlier `IMPLEMENTATION_AUDIT` reports beside the proposal were ignored for reviewer selection, per the skill boundary.

Reviewer-selection reuse status: `Not reused`.

## Selected Reviewers

- `rust_arch_reviewer`: Rust control-plane crate boundaries, SQLite migrations, projection repositories, GraphQL/MCP read surfaces, and release receipt wiring are primary.
- `rust_reliability_reviewer`: P087 is about liveness, time budgets, cancellation, restart rebuild, invalidation drain/freeze, reaper behavior, and overload handling.
- `rust_performance_reviewer`: The proposal explicitly constrains hot read latency, N+1 enrichment, deep scans, and SQLite pressure.
- `api_contract_reviewer`: GraphQL storage health, MCP tools/resources, typed errors, rollout readback lanes, and compatibility fixtures are contract-bearing surfaces.
- `observability_rollout_reviewer`: Storage health, metrics, rollout readback, promotion budget, gate checks, and exit criteria dominate readiness.

Rejected close alternatives:

- `apple_arch_reviewer` / `macos_ui_reviewer`: Swift diagnostics are referenced by the gate, but the current implementation delta is Rust/backend/storage. P087 explicitly excludes new UI write controls.
- `rust_security_reviewer`: Auth/tool access and redaction are covered by focused P087 tests and minor input-length hardening; no new secret boundary, unsafe block, public parser, or broader auth redesign drives this audit.
- `product_reviewer`: The proposal is an operational storage contract, not a product-experiment or user-facing value proposal.

## Scope

Platform/product scope:

- Apple scope: macOS operator shell readback/diagnostics only; no new UI write controls are in scope.
- Backend/service scope: Rust control-plane data, API, read-path, worker/reaper, telemetry, and rollout surfaces.
- Product scope: operator trust in storage health and migration decision criteria, not end-user workflow UX.

Primary service flows:

1. `runs.list` returns all runs from compact projections without artifact/report/transcript detail enrichment.
2. MCP hot reads such as `tools/list`, `runtime.health`, `resources/read`, `storage.health`, and `runs.list` pass through guard, timeout, cancellation, and typed degraded-result behavior.
3. Projection invalidation rows are coalesced, drained by oldest source watermark, marked consumed, and frozen for operator repair on failure.
4. Daemon restart and maintenance reaper paths rebuild or repair compact projection state without blocking ordinary reads.
5. Storage health and rollout readback expose writer/WAL/spool/projection/hot-read metrics and a per-surface observe-to-enforce promotion budget.

## Proposal Contract Summary

P087 adopts Plan A: keep SQLite as compact canonical state, move high-volume evidence to file storage, serve hot reads from materialized/in-memory projections, and define explicit criteria for when SQLite stops being sufficient. The core proposal text says SQLite must not store high-volume event streams, while file storage owns ACP transcripts, traces, stream deltas, runtime bundles, reports, and large artifacts. Hot reads must avoid deep scans and N+1 attachment passes. `runs.list` specifically forbids artifact/report attachment passes, filesystem scans, transcript reads, compaction archive inspection, side-effect evidence readback, and non-materialized implementation self-assessment attachment. MCP read tools must have strict time budgets, fail fast, and not allow one stuck tool to block future control-plane reads. The proposal also requires storage health, metrics, restart rebuild, a `proposal-087` gate, and storage migration decision criteria backed by metrics rather than assumption.

Explicit exclusions/non-goals include no Postgres/RocksDB migration, no SQLite replacement, no undoing DbWriter, no durable side-effect ledger semantic change, no UI action-boundary change, no new UI write controls, no new ACP provider families, and no run compaction implementation.

## Fidelity Inventory

Matches:

- SQLite remains a compact state and metadata store; new migration state is projection/circuit budget metadata, not raw stream/event payload storage.
- File evidence remains pointer-based, with artifact metadata pointer fixtures and GraphQL/MCP compatibility checks.
- `runs.list` is projection-only via `projections::list_all_projection`, reading `runs`, `run_summaries`, and live approval counts without detail attachment.
- MCP hot reads are guarded, timed, cancellable, and typed on degraded/circuit behavior.
- Projection invalidation has coalescing, backlog throttling, drain priority, consumed markers, cursor freezing, and audited clear controls.
- Storage health exposes live rollout budget fields derived from circuit counters and canonical governed surfaces.
- Run-report and release-receipt lanes merge live P087 rollout readback fields.
- The canonical `proposal-087` gate passed on this tree and includes static checks for the newer promotion-budget and production-lane wiring.

Divergences:

- The proposal file remains `Draft`, but implementation exists and is auditable. This is documentation state divergence, not an implementation behavior gap.
- The rollout fixture named `p087-storage-tiering-full-surface.fixture.json` shows `p087_promotion_budget_met: true` with only two per-surface entries in the GraphQL lane, while production code enumerates six canonical surfaces. The gate now checks canonical surface enumeration in code, but still does not validate the fixture list length.

Ambiguities / Evidence Gaps:

- I did not run a live daemon against real workflows for the Phase 5 storage decision checkpoint. The proposal itself says Phase 5 requires real workflows and metric inspection before a migration decision.
- I did not run the Swift app full build or remote UI smoke gate because the proposal is backend/storage-led and the canonical P087 gate is the required proof path for this audit.

## Implementation Fingerprint

Stack tags:

- `rust-backend`
- `sqlite`
- `mcp`
- `graphql`
- `control-plane`
- `rollout-observability`

Surface tags:

- persistence migrations
- projection repositories
- hot-read guard
- maintenance reaper
- storage health readback
- MCP tools/resources
- GraphQL schema/types
- rollout contract lanes
- proposal gate and fixtures

Risk tags:

- hot-path read latency
- SQLite contention
- cancellation and timeout correctness
- projection freshness/drift
- rollout evidence fidelity
- dirty worktree handoff

Changed implementation files observed before writing this report:

- `control-plane/crates/db/src/repos/hot_read_circuit.rs`
- `control-plane/crates/db/src/repos/maintenance.rs`
- `control-plane/crates/db/src/repos/storage_health.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `docs/evidence/p087/api/mcp-storage-health-compatibility.fixture.json`
- `docs/evidence/rollout-contract/operator-readback/p087-storage-tiering-full-surface.fixture.json`
- `docs/reference/rust-control-plane.md`
- `scripts/test-gate.sh`
- `control-plane/crates/db/migrations/061_p087_hot_read_promotion_budget.sql`

## Requirement Summary

| ID | Requirement | Status |
| --- | --- | --- |
| REQ-001 | SQLite stores compact canonical state and metadata only. | Implemented |
| REQ-002 | High-volume evidence and large artifacts stay file-backed with SQLite metadata/pointers only. | Implemented |
| REQ-003 | Hot reads use projections/cache/precomputed summaries rather than deep scans. | Implemented |
| REQ-004 | `runs.list` is projection-only and avoids detail attachments/scans. | Implemented |
| REQ-005 | MCP read tools have strict budgets, fail fast, and do not allow a stuck tool to block future reads. | Implemented |
| REQ-006 | Long maintenance operations use accepted/operation-id style readback, not blocking request handlers. | Implemented |
| REQ-007 | Hot projections and freshness metadata exist and can rebuild after restart. | Implemented |
| REQ-008 | Projection invalidation is bounded, drainable, and repairable. | Implemented |
| REQ-009 | Storage health exposes write pressure, WAL, spool, projection lag/freshness, liveness, and exit-threshold data. | Implemented |
| REQ-010 | Storage exit criteria are documented and enforced by a gate without making a premature migration decision. | Implemented |
| REQ-011 | Durable side-effect ledger, P038, and P086 can rely on the storage tiering contract without unexpected SQLite pressure. | Implemented |
| REQ-012 | Required P087 tests/static checks are present and pass on the audited tree. | Implemented |

## Detailed Requirement Audit

### REQ-001 - SQLite compact canonical state and metadata only

- Proposal source: lines 67-84.
- Status: Implemented.
- Evidence types: proposal, migration, code, tests-run.
- Evidence references: migration `061_p087_hot_read_promotion_budget.sql` adds only `total_requests`, `total_would_open`, and `last_state_change_at_ms` to circuit state; storage/projection code stores compact counters, cursors, freshness, and summaries rather than raw streams.
- Implementation mapping: projection, circuit, maintenance, and storage-health tables model compact state.
- Gap/note: No raw high-volume stream table was introduced in the audited delta.

### REQ-002 - File-backed high-volume evidence with SQLite pointers/metadata

- Proposal source: lines 86-108.
- Status: Implemented.
- Evidence types: proposal, schema, tests-run, fixture.
- Evidence references: P087 gate verifies artifact metadata pointer fixtures, MCP artifact resource metadata redaction, evidence fixture presence, and required spool metrics.
- Implementation mapping: artifact metadata pointer contracts and storage health spool readback keep payload detail out of hot paths.
- Gap/note: Live file-spool load was not replayed in this audit, but the canonical proposal gate covers contract and fixture regression.

### REQ-003 - Hot reads use projections/cache/precomputed summaries

- Proposal source: lines 110-122 and lines 220-235.
- Status: Implemented.
- Evidence types: code, schema, tests-run.
- Evidence references: GraphQL storage types preserve projection/freshness readback; `storage_health.rs` builds compact storage-health JSON; GraphQL P087 tests include projection-only run query and hot-read circuit behavior.
- Implementation mapping: GraphQL/MCP read paths consume projection rows and compact health state.
- Gap/note: The audit did not find GraphQL/MCP hot-read filesystem scans in the inspected surfaces.

### REQ-004 - `runs.list` projection-only

- Proposal source: lines 187-205.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `projections::list_all_projection` reads `runs`, `run_summaries`, and live approvals at lines 215-302; MCP `runs.list` delegates to this projection path; MCP and GraphQL tests assert list behavior without per-row enrichment.
- Implementation mapping: `runs.list` returns compact projection fields, plus pre-materialized closeout/implementation summaries where present in projection JSON.
- Gap/note: None.

### REQ-005 - MCP read liveness and typed degraded behavior

- Proposal source: lines 207-218 and lines 422-445.
- Status: Implemented.
- Evidence types: code, tests-run, telemetry.
- Evidence references: `handle_hot_read_json_rpc` checks guard state, applies 500 ms probe or 10 s normal timeout, scopes reads through cancellation, records hot-read/liveness metrics, and returns typed timeout/circuit results at lines 533-597.
- Implementation mapping: MCP server wraps hot read methods through guard, timeout, cancellation, success/violation accounting, and typed response bodies.
- Gap/note: None.

### REQ-006 - Long maintenance tools do not block ordinary read liveness

- Proposal source: lines 207-218 and lines 529-534.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: maintenance repair validates input lengths before writer-slot use at lines 177-205; reaper/repair operations use repository transactions and operation readback; MCP tests include liveness while maintenance is running and storage repair typed errors.
- Implementation mapping: long repair/maintenance work is modeled as operations, while hot reads remain guarded.
- Gap/note: None.

### REQ-007 - Hot projections and restart rebuild

- Proposal source: lines 239-343 and lines 536-542.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: projection repositories expose run summary projections, projection freshness, rebuild methods, and restart rebuild test `proposal_087_projection_cache_rebuilds_after_restart`.
- Implementation mapping: active/list projections, approval/readback projection data, storage health projection, and runtime health summary are exposed through compact read models.
- Gap/note: None.

### REQ-008 - Bounded projection invalidation and repair

- Proposal source: lines 333-343 and lines 573-581.
- Status: Implemented.
- Evidence types: code, tests-run, telemetry.
- Evidence references: `record_invalidation_internal` coalesces unconsumed same-key rows and throttles/freezes at capacity at lines 70-168; clear backlog/poison record command journal entries at lines 170-276; consumed markers and oldest-drain priority are at lines 278-351; freeze-on-exhaustion is at lines 353-390. The reaper drains oldest invalidation source and freezes failures at maintenance lines 454-515.
- Implementation mapping: projection invalidation lifecycle is production-wired through terminal writes, projection rebuilds, reaper drain, clear tools, and capability registration.
- Gap/note: None.

### REQ-009 - Storage health and metrics

- Proposal source: lines 448-468 and lines 589-595.
- Status: Implemented.
- Evidence types: code, telemetry, schema, fixture, tests-run.
- Evidence references: `storage_health.rs` emits P087 storage/liveness/list/rebuild/enforcement/compatibility/maintenance/invalidation status and rollout contract fields at lines 816-863. Promotion budget logic enumerates canonical hot-read surfaces and derives total requests, would-open rate, flap-free window, and readiness at lines 866-986.
- Implementation mapping: GraphQL/MCP storage health expose writer/WAL/spool/projection/hot-read/rollout readback, and metrics names are checked by the gate.
- Gap/note: The fixture still needs stronger validation against the canonical surface list; see `OPS-001`.

### REQ-010 - Storage exit criteria and gate enforcement

- Proposal source: lines 471-515 and lines 544-549.
- Status: Implemented.
- Evidence types: proposal, code, tests-run.
- Evidence references: proposal documents warning/critical thresholds; `scripts/test-gate.sh proposal-087` runs migration uniqueness, focused Rust tests, compatibility tests, fixture checks, static scans, promotion-budget wiring checks, and passed on this tree.
- Implementation mapping: storage-health readback and the proposal gate enforce Plan A liveness/read-path expectations and expose when storage migration review is warranted.
- Gap/note: Phase 5 real workflow metrics remain future operational work by proposal design.

### REQ-011 - Dependent proposals can rely on the storage contract

- Proposal source: lines 379-420 and lines 596.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: gate includes P077 and P088 compatibility checks; release receipt and run-report lanes merge live P087 readback (`reports.rs` lines 912-936 and `executor.rs` lines 10241-10268), preserving downstream closeout/release evidence paths.
- Implementation mapping: dependent readback lanes can consume storage tiering status without adding SQLite pressure from detail reads.
- Gap/note: None.

### REQ-012 - Required tests and static checks pass

- Proposal source: lines 560-581.
- Status: Implemented.
- Evidence types: tests-run.
- Evidence references: fresh `./scripts/test-gate.sh proposal-087` passed in this audit. DB P087 tests: 28 passed; MCP P087 tests: 16 passed; P077 compatibility: 1 passed; P088 compatibility: 1 passed; auth P087: 2 passed; engine restart rebuild: 1 passed; GraphQL storage-health v1: 1 passed; GraphQL P087: 5 passed; static checks printed `P087 UI, schema, and evidence verified`.
- Implementation mapping: canonical P087 gate is the successful same-tree proof for conformance and readiness roll-up.
- Gap/note: Warnings were non-fatal dead-code/lifetime/unused warnings.

## Reviewer Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Implemented | Proposal document still says Draft, but implementation satisfies explicit contract. | High |
| Rust architecture | Pass | Dirty worktree needs a coherent final stage/commit slice. | High |
| Rust reliability | Pass | Failure freeze after rebuild remains best-effort between failure and next reaper/retry, but tested recovery exists. | Medium-high |
| Rust performance | Pass | Hot-read budget is covered by tests/metrics, not long-running benchmarks. | Medium-high |
| API contract | Pass | Fixture/gate validation around per-surface rollout evidence can be tightened. | Medium-high |
| Observability/rollout | Ready with risk | Production code computes live promotion budget, but one fixture can still overstate full-surface proof. | Medium |
| Release readiness | Ready with Risks | Same-tree gate passed; worktree is dirty and not yet a clean handoff artifact. | Medium-high |

## Routed Specialist Findings

### OPS-001 - Rollout fixture can still overstate full-surface promotion proof

- Reviewer: `observability_rollout_reviewer`
- Severity: Minor
- Confidence: High
- Related requirements: REQ-009, REQ-010, REQ-012
- Evidence types: code, fixture, tests-run
- Evidence references: production code enumerates six canonical surfaces at `storage_health.rs` lines 876-986 and test `proposal_087_promotion_budget_false_positive_subset_surfaces` requires missing surfaces to keep `promotion_budget_met` false at lines 1656-1703. The rollout fixture GraphQL lane shows `p087_promotion_budget_met: true` with only two `p087_per_surface_promotion_budget` entries at fixture lines 41-48. The gate checks field presence and code enumeration at `scripts/test-gate.sh` lines 8048-8214, but does not validate fixture per-surface cardinality against `CANONICAL_HOT_READ_SURFACES`.
- Why it matters: Operators and future audits may treat the fixture as concrete full-surface evidence even though the production code now has a stricter canonical-surface contract.
- Recommended action: Update the rollout fixture to include all canonical governed surfaces when claiming `p087_promotion_budget_met: true`, and extend the gate to fail if the fixture's ready lane has fewer entries than the canonical set.
- Acceptance criteria: `p087-storage-tiering-full-surface.fixture.json` lists all canonical governed surfaces or marks the fixture as partial; the static gate validates the list length or explicit partial status.

### READY-001 - Current implementation is not yet a clean handoff unit

- Reviewer: release readiness
- Severity: Minor
- Confidence: High
- Related requirements: REQ-012
- Evidence types: git status, tests-run
- Evidence references: pre-report `git status --short --branch` showed nine modified implementation/evidence/reference/gate files and untracked migration `061_p087_hot_read_promotion_budget.sql`.
- Why it matters: The implementation passes the canonical P087 gate, but final review/merge should happen against an intentional staged or committed slice so gate evidence maps cleanly to the merge artifact.
- Recommended action: Stage or commit the coherent P087 implementation, including migration 061 and updated fixtures/gate, then rerun `./scripts/test-gate.sh proposal-087`.
- Acceptance criteria: Worktree is clean or contains only intentionally unstaged audit reports, and the canonical P087 gate passes on the final staged/committed implementation.

### REL-001 - Rebuild-failure cursor freeze remains eventually visible rather than atomic

- Reviewer: `rust_reliability_reviewer`
- Severity: Note
- Confidence: Medium
- Related requirements: REQ-008
- Evidence types: code, tests-run
- Evidence references: projection rebuild failure invokes freeze logic after the failed rebuild path, and drain/reaper tests cover visible frozen state. The proposal requires stale/liveness visibility, not atomic poison marking for every possible process-death interleave.
- Why it matters: A process death between rebuild failure and follow-up freeze could delay poison readback until retry/reaper replay.
- Recommended action: Keep the current retry/reaper contract documented, or make failure marking atomic with the failed consumer path if future operator requirements demand immediate poison readback.
- Acceptance criteria: No action required for P087; document as acceptable eventual-visibility behavior or add an atomic failure-marker in a follow-up.

## Readiness Checklist

| Item | Status | Evidence |
| --- | --- | --- |
| Canonical proposal gate passed on audited tree/HEAD | Passed | `./scripts/test-gate.sh proposal-087` passed on HEAD `6bafc7c4...` plus current working tree |
| Core service flows validated by focused tests | Passed | DB 28/28, MCP 16/16, engine restart rebuild 1/1, GraphQL P087 5/5 |
| `runs.list` projection-only behavior | Passed | MCP and GraphQL tests plus code inspection |
| MCP liveness while maintenance is running | Passed | MCP P087 liveness tests |
| Storage health degraded/typed status | Passed | DB, MCP, and GraphQL P087 tests |
| Projection cache rebuild after restart | Passed | Engine integration P087 test |
| Evidence spool and artifact pointer contract | Passed | Static fixture/schema checks in proposal gate |
| API compatibility for adjacent readback contracts | Passed | P077 and P088 compatibility tests in proposal gate |
| Accessibility/localization/privacy/entitlements | Not applicable | No UI/UX or entitlement surface in P087 implementation scope |
| Full broader regression suite | Not run | Canonical proposal gate was run and passed; broader full gate is a release-management follow-up |
| Dirty worktree handoff | Risk | Implementation files remain unstaged/uncommitted before this report |

## Verification Log

Command run:

```text
./scripts/test-gate.sh proposal-087
```

Result: passed.

Key observed output:

- `P087 DB migration versions verified`
- DB P087 filtered suite: `28 passed; 0 failed`
- MCP P087 filtered suite: `16 passed; 0 failed`
- P077 closeout readback compatibility: `1 passed; 0 failed`
- P088 implementation-completion readback compatibility: `1 passed; 0 failed`
- Auth P087 filtered suite: `2 passed; 0 failed`
- Engine restart rebuild integration: `1 passed; 0 failed`
- GraphQL storage-health v1 preservation: `1 passed; 0 failed`
- GraphQL P087 filtered suite: `5 passed; 0 failed`
- Static checks: `P087 UI, schema, and evidence verified`
- Final gate line: `==> Proposal 087 gate passed`

Warnings observed were existing non-fatal Rust warnings for dead code, unused imports in unrelated filtered tests, and lifetime syntax. They did not fail the gate.

## Final Verdict

Overall Conformance: Implemented.

Overall Implementation Readiness: Ready with Risks.

P087's explicit implementation contract is satisfied on the audited tree: compact SQLite state, file-backed evidence, projection-only `runs.list`, guarded/cancellable MCP hot reads, projection invalidation drain/freeze, storage health metrics, live promotion-budget readback, production run-report/release-receipt lane wiring, and the canonical gate are all present and verified.

The remaining risks are handoff and evidence-fidelity risks, not proposal-conformance gaps. Before merge, cleanly stage/commit the P087 slice and tighten the full-surface rollout fixture/gate so the fixture cannot claim promotion-ready status with a partial governed-surface list.
