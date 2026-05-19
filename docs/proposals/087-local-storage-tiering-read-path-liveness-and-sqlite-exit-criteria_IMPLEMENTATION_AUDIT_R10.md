# Proposal 087 Implementation Audit R10

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Proposal state | Draft, treated as Active for this implementation audit |
| Proposal state evidence | Proposal metadata says `Status | Draft` at line 6 |
| Implementation target | Current worktree requested as `cw-implement-proposal-087-local-s-b4edcf82` |
| Repository root | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Branch | `cw/implement-proposal-087-local-s/b4edcf82` |
| Current HEAD | `f4578567b0f120ed2cc020561fd39dba485bbe53` |
| Compare base | Implicit current worktree; no PR base or commit range supplied |
| Working tree before this report | Clean, tracking `origin/cw/implement-proposal-087-local-s/b4edcf82` |
| Audit timestamp | `2026-05-18T19:27:41Z` |
| Report path helper result | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria_IMPLEMENTATION_AUDIT_R10.md` |
| Overall Conformance | Implemented |
| Overall Implementation Readiness | Ready |
| Reviewer Selection Reuse | Not reused |
| Audit confidence | High |

## Prior Proposal-Review Reuse

The prior-review discovery helper returned no proposal-review artifacts for this proposal. Existing `IMPLEMENTATION_AUDIT` reports were ignored for reviewer selection, per the audit skill boundary.

Reviewer-selection reuse status: `Not reused`.

## Selected Reviewers

- `rust_arch_reviewer`: P087 is implemented through Rust control-plane repositories, migrations, GraphQL/MCP surfaces, and release readback wiring.
- `rust_reliability_reviewer`: P087 relies on liveness guards, cancellation, timeouts, reaper behavior, restart rebuild, invalidation drain/freeze, and overload handling.
- `rust_performance_reviewer`: The proposal constrains hot-read latency, projection-only reads, SQLite pressure, deep scans, and N+1 detail enrichment.
- `api_contract_reviewer`: MCP/GraphQL schema compatibility, typed errors, storage health shapes, and rollout readback lanes are contract-bearing.
- `observability_rollout_reviewer`: Storage health, metrics, rollout status, promotion budget, fixture evidence, and migration decision gates dominate readiness.

Rejected close alternatives:

- `apple_arch_reviewer` / `macos_ui_reviewer`: The proposal only touches macOS operator readback indirectly and explicitly excludes new UI write controls.
- `rust_security_reviewer`: Current P087 hardening includes public error/request-id redaction and input limits, but those are covered by the selected API/reliability lenses and focused tests; no new unsafe/auth boundary dominates the implementation.
- `product_reviewer`: This is an operational storage contract rather than a user-facing product or experiment proposal.

## Scope

Platform/product scope:

- Apple: macOS operator shell readback only; no iOS scope and no new UI write controls.
- Backend/service: Rust control-plane data, API, worker/reaper, telemetry, rollout, and storage-health scope.
- Product: operator confidence in local storage viability and migration decision criteria.

Primary service flows:

1. `runs.list` serves all runs from compact projections without artifact/report/transcript enrichment.
2. MCP hot reads, including initialize, `tools/list`, `runtime.health`, `resources/read`, `storage.health`, and `runs.list`, pass through guard, timeout, cancellation, typed error, and redaction behavior.
3. Projection invalidations coalesce, drain by oldest source watermark, mark consumed rows, freeze failed cursors, and expose operator repair controls.
4. Restart/reaper flows rebuild or repair compact projection state without blocking ordinary reads.
5. Storage health and rollout readback expose writer/WAL/spool/projection/hot-read metrics, all canonical promotion surfaces, and pass/hold decisions.

## Proposal Contract Summary

P087 formalizes Plan A for local storage: SQLite remains compact canonical state and metadata; high-volume evidence moves to file storage; hot reads use materialized/in-memory projections; MCP/GraphQL read paths remain live; and explicit metrics define when SQLite should be reviewed or replaced. It specifically requires projection-only `runs.list`, strict MCP read budgets, typed degraded status instead of hangs, projection freshness, restart rebuild, storage health, storage exit criteria, and a canonical proposal gate.

Explicit non-goals include no Postgres/RocksDB migration, no SQLite replacement, no undoing DbWriter, no durable side-effect ledger semantic change, no UI action-boundary change, no new UI write controls, no new ACP provider families, and no run compaction implementation.

## Fidelity Inventory

Matches:

- SQLite owns compact state, projections, cursors, hot-read counters, storage health snapshots, and metadata only.
- File-backed evidence remains pointer/metadata based, with artifact pointer fixtures and redaction checks.
- `runs.list` delegates to projection rows and avoids detail attachment.
- MCP hot reads have guard, timeout, cancellation, typed degraded response, request-id sanitization, and body-size protections.
- Projection invalidation is bounded, coalesced, drainable, consumable, frozen on failure, and repairable.
- Storage health computes live promotion readiness from canonical governed surfaces, request counts, would-open counts, first-observed time, and state-change time.
- The rollout fixture now lists all six canonical governed surfaces when it claims promotion readiness.
- The canonical P087 gate now checks promotion-budget consistency, fixture field names, production lane wiring, and canonical surface presence.
- The audited worktree is clean at the audited HEAD.

Divergences:

- The proposal file still says `Status | Draft`, while the implementation is complete and committed on the implementation branch. This is proposal lifecycle metadata, not an implementation gap.

Ambiguities / Evidence Gaps:

- Phase 5 real-workflow storage decision is not complete, but the proposal defines it as a future operational checkpoint. No migration decision is required for P087 implementation readiness.
- I did not run the full repository gate or remote UI smoke gate because the canonical `proposal-087` gate is the relevant same-tree acceptance path for this proposal.

## Implementation Fingerprint

Stack tags:

- `rust-backend`
- `sqlite`
- `mcp`
- `graphql`
- `control-plane`
- `rollout-observability`

Surface tags:

- migrations 050, 056-061
- projection repositories
- hot-read circuit and guard
- maintenance reaper and repair controls
- storage health readback
- MCP tools/resources
- GraphQL storage/run schema
- rollout contract lanes
- proposal gate and fixtures

Risk tags inspected:

- hot-path read latency
- SQLite contention
- cancellation and timeout behavior
- projection freshness/drift
- public readback redaction
- rollout promotion evidence
- release handoff cleanliness

## Requirement Summary

| ID | Requirement | Status |
| --- | --- | --- |
| REQ-001 | SQLite stores compact canonical state and metadata only. | Implemented |
| REQ-002 | High-volume evidence and large artifacts stay file-backed with SQLite metadata/pointers only. | Implemented |
| REQ-003 | Hot reads use projections/cache/precomputed summaries rather than deep scans. | Implemented |
| REQ-004 | `runs.list` is projection-only and avoids detail attachments/scans. | Implemented |
| REQ-005 | MCP read tools have strict budgets, fail fast, redact unsafe data, and do not allow one stuck read to block future reads. | Implemented |
| REQ-006 | Long maintenance work uses operation/readback patterns instead of blocking read handlers. | Implemented |
| REQ-007 | Hot projections and freshness metadata exist and rebuild after restart. | Implemented |
| REQ-008 | Projection invalidation is bounded, drainable, consumable, frozen on failure, and repairable. | Implemented |
| REQ-009 | Storage health exposes write pressure, WAL, spool, projection lag/freshness, liveness, and exit-threshold data. | Implemented |
| REQ-010 | Storage exit criteria and observe-to-enforce promotion gates are enforced by the proposal gate. | Implemented |
| REQ-011 | Durable side-effect ledger, P038, and P086 can rely on the storage tiering contract without unexpected SQLite pressure. | Implemented |
| REQ-012 | Required P087 tests/static checks pass on the audited tree. | Implemented |

## Detailed Requirement Audit

### REQ-001 - SQLite compact canonical state and metadata only

- Proposal source: sections 3.1 and 5.1, lines 67-84 and 145-156.
- Status: Implemented.
- Evidence types: proposal, migration, code, tests-run.
- Evidence references: migrations add compact projection, cursor, circuit, and health metadata; migration 061 adds only hot-read promotion counters and first-observed/state-change timestamps.
- Implementation mapping: P087 state is stored as compact rows and counters, not raw streams.
- Gap/note: None.

### REQ-002 - File-backed evidence with SQLite metadata/pointers

- Proposal source: section 3.2, lines 86-108.
- Status: Implemented.
- Evidence types: proposal, schema, fixture, tests-run.
- Evidence references: P087 gate verifies artifact metadata pointer fixtures, MCP artifact resource metadata redaction, and evidence spool metrics.
- Implementation mapping: large payloads remain file-backed while hot read paths expose compact pointer/readback fields.
- Gap/note: None.

### REQ-003 - Hot reads use projections/cache/precomputed summaries

- Proposal source: sections 3.3 and 6.3, lines 110-122 and 220-235.
- Status: Implemented.
- Evidence types: code, schema, tests-run.
- Evidence references: storage health and runtime health are projection-backed; GraphQL P087 tests include projection-only query and circuit-open behavior.
- Implementation mapping: GraphQL/MCP read surfaces consume compact projections and health summaries.
- Gap/note: None.

### REQ-004 - `runs.list` projection-only

- Proposal source: section 6.1, lines 187-205.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `projections::list_all_projection` reads compact run/projection rows; MCP and GraphQL tests assert no per-row detail enrichment; seeded MCP load stays under the 500 ms budget.
- Implementation mapping: `runs.list` returns projection rows and pre-materialized summary JSON only.
- Gap/note: None.

### REQ-005 - MCP read liveness, safety, and typed degradation

- Proposal source: sections 6.2 and 13, lines 207-218 and 422-445.
- Status: Implemented.
- Evidence types: code, tests-run, telemetry.
- Evidence references: hot-read handling applies guard, timeout, cancellation, success/violation accounting, liveness metrics, typed errors, request-id sanitization, and body-size rejection. MCP P087 tests cover initialize guarding, `tools/list`, `resources/read`, `runtime.health`, `storage.health`, request-id sanitization, and oversized body rejection.
- Implementation mapping: hot reads cannot hang indefinitely and surface degraded results in a controlled shape.
- Gap/note: None.

### REQ-006 - Maintenance operations do not block ordinary read liveness

- Proposal source: section 6.2 and phase 2, lines 207-218 and 529-534.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: maintenance repair tools return public DTO/readback, validate inputs before writer-slot use, redact internal operation IDs, and keep MCP liveness passing while maintenance runs.
- Implementation mapping: maintenance is modeled as operations and repair slots while hot reads remain separately guarded.
- Gap/note: None.

### REQ-007 - Hot projections and restart rebuild

- Proposal source: sections 7 and 8, lines 239-343.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: runtime health, storage health, artifact noise, run summaries, projection freshness, and restart rebuild are covered by focused DB/engine tests.
- Implementation mapping: compact projection rows can be rebuilt after restart and expose freshness/readback metadata.
- Gap/note: None.

### REQ-008 - Bounded projection invalidation and repair

- Proposal source: sections 8 and 17.2, lines 333-343 and 573-581.
- Status: Implemented.
- Evidence types: code, tests-run, telemetry.
- Evidence references: invalidation lifecycle tests cover coalescing, oldest-source drain priority, consumed rows, cursor freeze, oversized replacement safety, terminal writes at backlog capacity, and clear controls. Reaper drains pending invalidations and freezes failure cases.
- Implementation mapping: projection drift becomes visible, bounded, and operator-repairable.
- Gap/note: None.

### REQ-009 - Storage health and metrics

- Proposal source: section 14 and acceptance criteria, lines 448-468 and 589-595.
- Status: Implemented.
- Evidence types: code, schema, telemetry, fixture, tests-run.
- Evidence references: `storage_health.rs` exposes rollout hold/pass conditions, production enforce requirement, poisoned cursor/backlog holds, liveness mode redaction, per-surface promotion budget, and redacted maintenance/projection errors. Metrics are gate-checked.
- Implementation mapping: storage health covers writer/WAL/spool/projection/liveness/rollout surfaces and fails closed into hold/degraded states.
- Gap/note: None.

### REQ-010 - Exit criteria and observe-to-enforce gate

- Proposal source: sections 15, 16 phase 4, and 17, lines 471-581.
- Status: Implemented.
- Evidence types: proposal, code, fixture, tests-run.
- Evidence references: storage exit criteria are documented; promotion budget evaluates six canonical governed surfaces; fixture and gate assert pass only when promotion budget is met and all canonical surfaces are represented.
- Implementation mapping: operators get a metric-backed ready/hold decision rather than a static field presence signal.
- Gap/note: None.

### REQ-011 - Dependent proposal compatibility

- Proposal source: sections 10-12 and acceptance criterion 8, lines 379-420 and 596.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: P077 and P088 compatibility tests pass; run-report and release-receipt lanes merge live P087 readback fields.
- Implementation mapping: durable side-effect, compaction, and continuation/closeout readback can consume storage state without forcing deep hot reads.
- Gap/note: None.

### REQ-012 - Required tests and static checks

- Proposal source: section 17, lines 560-581.
- Status: Implemented.
- Evidence types: tests-run.
- Evidence references: fresh `./scripts/test-gate.sh proposal-087` passed on clean HEAD `f4578567b0f120ed2cc020561fd39dba485bbe53`.
- Implementation mapping: canonical same-tree gate is the successful acceptance evidence for this audit.
- Gap/note: None.

## Reviewer Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Pass | None remaining. | High |
| Rust architecture | Pass | None remaining. | High |
| Rust reliability | Pass | Process-death interleaves rely on retry/reaper replay, which is acceptable for P087. | High |
| Rust performance | Pass | No long-running benchmark, but P087 gate validates hot read budgets and regression checks. | High |
| API contract | Pass | GraphQL/MCP compatibility and redaction checks pass. | High |
| Observability/rollout | Pass | Promotion budget, full-surface fixture, and hold/pass conditions are gate-checked. | High |
| Release readiness | Pass | Worktree was clean before report; canonical gate passed. | High |

## Routed Specialist Findings

No routed specialist findings remain for this audit.

Informational notes:

- The proposal document still says `Status | Draft`. If the project uses proposal status as lifecycle truth, update status during proposal closeout rather than as part of this read-only audit.
- Phase 5 real workflow metric inspection remains an operational checkpoint before any storage migration decision; it is not an implementation gap for P087.

## Readiness Checklist

| Item | Status | Evidence |
| --- | --- | --- |
| Canonical proposal gate passed on audited tree/HEAD | Passed | `./scripts/test-gate.sh proposal-087` on HEAD `f4578567b0f120ed2cc020561fd39dba485bbe53` |
| Core service flows validated | Passed | DB, MCP, auth, engine, GraphQL, and static checks passed |
| `runs.list` projection-only behavior | Passed | MCP and GraphQL tests plus projection code |
| MCP liveness while maintenance runs | Passed | MCP P087 liveness tests |
| Storage health degraded/typed status | Passed | DB, MCP, and GraphQL P087 tests |
| Promotion budget full-surface evidence | Passed | Fixture lists all six canonical surfaces; gate validates representation and pass gating |
| Projection rebuild after restart | Passed | Engine integration P087 test |
| Evidence spool and artifact pointer contract | Passed | Static fixture/schema checks |
| API compatibility for adjacent readback contracts | Passed | P077 and P088 compatibility tests |
| Privacy/redaction risk | Passed | storage health, circuit violation, request-id, operation ID, and error redaction tests |
| Accessibility/localization/entitlements | Not applicable | No UI/UX or entitlement implementation surface in P087 |
| Handoff cleanliness | Passed | Worktree was clean before this report was written |

## Verification Log

Command run:

```text
./scripts/test-gate.sh proposal-087
```

Result: passed.

Observed output:

- `P087 DB migration versions verified`
- DB P087 filtered suite: `40 passed; 0 failed`
- MCP P087 filtered suite: `22 passed; 0 failed`
- P077 closeout readback compatibility: `1 passed; 0 failed`
- P088 implementation-completion readback compatibility: `1 passed; 0 failed`
- Auth P087 filtered suite: `2 passed; 0 failed`
- Engine restart rebuild integration: `1 passed; 0 failed`
- GraphQL storage-health v1 preservation: `1 passed; 0 failed`
- GraphQL P087 filtered suite: `5 passed; 0 failed`
- Static checks: `P087 UI, schema, and evidence verified`
- Final gate line: `==> Proposal 087 gate passed`

Warnings observed were non-fatal Rust warnings for dead code, an unused import in an unrelated filtered test, and lifetime syntax. They did not fail the gate.

## Final Verdict

Overall Conformance: Implemented.

Overall Implementation Readiness: Ready.

P087 satisfies its explicit implementation contract on the audited tree. The implementation has compact SQLite state, file-backed evidence contracts, projection-only `runs.list`, guarded/cancellable MCP hot reads, projection invalidation lifecycle controls, restart rebuild proof, storage health and redaction coverage, live full-surface promotion-budget readback, production run-report/release-receipt lane wiring, and a passing canonical proposal gate on a clean HEAD.

Recommended next action: move to proposal closeout/retirement workflow and update durable reference documentation/status as that workflow requires.
