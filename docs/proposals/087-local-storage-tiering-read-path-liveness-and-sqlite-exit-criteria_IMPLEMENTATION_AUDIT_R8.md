# Proposal 087 Implementation Audit R8

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria.md` |
| Worktree | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-087-local-s-b4edcf82` |
| Branch | `cw/implement-proposal-087-local-s/b4edcf82` |
| HEAD | `569b297e58582153eb3601f91d18e7aa97d9a6f2` |
| Audit timestamp | `2026-05-17T11:27:47Z` |
| Report path helper result | `docs/proposals/087-local-storage-tiering-read-path-liveness-and-sqlite-exit-criteria_IMPLEMENTATION_AUDIT_R8.md` |
| Overall conformance verdict | Implemented |
| Overall readiness verdict | Ready with Risks |
| Audit confidence | High for proposal conformance, medium-high for promotion readiness |

## Prior Review Reuse

No prior proposal-review artifacts were discovered for this proposal by the audit helper. I did not use earlier `IMPLEMENTATION_AUDIT` files for reviewer routing or verdict selection.

## Reviewer Routing

Selected reviewers:

- `rust_backend_arch_reviewer`: P087 is primarily a Rust control-plane storage, projection, GraphQL, MCP, and daemon lifecycle contract.
- `rust_reliability_reviewer`: The implementation changes hot-read liveness, timeout behavior, restart rebuild, invalidation backlog handling, and repair-slot recovery.
- `rust_performance_reviewer`: The proposal is explicitly about avoiding deep scans, N+1 enrichment, SQLite pressure, and slow hot reads.
- `api_contract_reviewer`: The implementation changes GraphQL storage health, MCP storage/read tools, typed errors, and compatibility fixtures.
- `observability_rollout_reviewer`: The implementation adds storage health, metrics, rollout readback, promotion gates, and operator decision evidence.

Rejected reviewers:

- `apple_arch_reviewer` / `macos_ui_reviewer`: Swift readback and sample-run support files changed, but P087 is not a UI feature proposal and the gate only requires additive Swift diagnostics/read-model tokens. I treat the Swift surface as readiness/scope hygiene, not as the main conformance lens.
- `rust_security_reviewer`: Operator-only tool registration, redaction, and capability gating are covered by auth/API evidence; no new secret-handling or external trust boundary dominated the proposal.
- `product_reviewer`: P087 defines internal storage viability and operational thresholds, not end-user product behavior.

## Proposal Contract Extract

The proposal requires:

- SQLite remains compact canonical state and metadata only; high-volume streams stay out of SQLite.
- File-backed evidence stores transcripts, traces, runtime bundles, reports, and large artifacts, with SQLite holding only pointers/metadata.
- Hot reads use projections, caches, or precomputed summaries rather than deep scans.
- `runs.list` is projection-only, with no N+1 artifact/report attachment, filesystem scans, transcript reads, compaction archive inspection, or side-effect evidence readback.
- MCP read tools return quickly, fail fast with typed degraded status, and do not allow one stuck read/tool to block future control-plane reads.
- Long maintenance work returns an operation id or accepted status instead of holding the read request.
- Hot projections and freshness metadata exist for active runs, approval inbox, storage health, compaction status, and side-effect unresolved counts.
- Cache/projection rebuild works after daemon restart.
- Storage health exposes write pressure, WAL/checkpoint state, evidence spool state, projection lag/freshness, hot-read guard state, and exit-threshold status.
- The `proposal-087` gate fails on liveness/read SLO regressions, high-volume SQLite evidence rows, detail attachments on `runs.list`, deep scans in hot reads, and unallowlisted write-budget bypasses.
- No UI write-control expansion, no Postgres/RocksDB migration, no durable side-effect ledger semantic change, and no ACP provider expansion.

## Implementation Fingerprint

The implementation introduces or modifies these main surfaces:

- SQLite migrations `057_p087_storage_tiering_projections.sql` through `060_p087_projection_invalidation_lifecycle.sql`.
- Projection and invalidation repositories in `control-plane/crates/db/src/repos/projections.rs`, `projection_invalidation.rs`, `storage_health.rs`, `maintenance.rs`, and related repository wiring.
- Hot-read guard, metrics, and writer/operation-registry updates in `control-plane/crates/db/src/hot_read_guard.rs`, `metrics.rs`, `writer.rs`, and `write-operation-registry.toml`.
- MCP runtime/read-path guard and tools in `control-plane/crates/mcp-server/src/server.rs`, `hot_read_guard.rs`, `tools/runs.rs`, `tools/runtime.rs`, and `tools/storage.rs`.
- GraphQL storage health, run projection, and artifact pointer schemas in `control-plane/crates/graphql-server/src/schema.rs` and `src/types/*.rs`.
- Engine restart rebuild proof in `control-plane/crates/engine/tests/integration.rs`.
- P087 fixtures under `docs/evidence/p087/` and rollout-contract fixtures under `docs/evidence/rollout-contract/`.
- The canonical gate in `scripts/test-gate.sh`.

## Requirement Status

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Keep SQLite to compact canonical state and metadata; do not store high-volume event streams. | Implemented | Migrations add compact projection, cursor, health, maintenance, and pointer state. Gate checks fixture/schema behavior and rejects high-volume hot-read/evidence anti-patterns. |
| REQ-002 | Store high-volume evidence and large artifacts on disk, with SQLite holding metadata/pointers only. | Implemented | Artifact pointer GraphQL/MCP contract and fixture coverage prevent leaking raw file paths or payloads; storage health exposes spool metrics. |
| REQ-003 | Make `runs.list` projection-only and preserve full run history. | Implemented | `runs.list` calls `projections::list_all_projection`; the projection query uses `runs` plus `run_summaries` and no artifact/detail attachment path. MCP tests include projection-only list and seeded-load latency checks. |
| REQ-004 | Guard MCP hot reads with fail-fast liveness, timeout, and typed degraded results. | Implemented | `handle_hot_read_json_rpc` wraps reads in a hot-read guard, cancellation token, timeout, latency metrics, liveness metrics, and typed timeout/circuit results. |
| REQ-005 | Keep long maintenance operations asynchronous and preserve ordinary read liveness. | Implemented | Repair and projection-maintenance tools use operation/readback patterns; auth tests assert operator-only access; MCP tests verify liveness while maintenance is running. |
| REQ-006 | Implement hot projections and freshness metadata, including restart rebuild. | Implemented | Projection tables, freshness readback, storage health read models, restart rebuild tests, and GraphQL/MCP compatibility fixtures cover this. |
| REQ-007 | Add bounded projection invalidation with coalescing, oldest-source drain priority, consumed markers, and freeze-on-exhaustion behavior. | Implemented | `record_invalidation_internal` coalesces unconsumed same-key rows and enforces capacity; `get_drain_priority_queue`, `mark_consumed*_tx`, and `freeze_cursor_after_retry_exhaustion` are wired through projection drain/reaper code and covered by DB tests. |
| REQ-008 | Expose storage health for write pressure, WAL/checkpoints, spool health, projection lag/freshness, hot-read guards, repair slots, and exit thresholds. | Implemented | `storage_health.rs`, GraphQL storage types, MCP storage health, metrics declarations, fixtures, and gate checks cover the required health surfaces. |
| REQ-009 | Add and enforce the `proposal-087` gate. | Implemented | `scripts/test-gate.sh proposal-087` runs filtered DB/MCP/auth/engine/GraphQL tests plus static UI/schema/evidence checks and passed in this audit. |
| REQ-010 | Preserve compatibility for existing GraphQL/MCP consumers and adjacent P077/P088 readback contracts. | Implemented | Gate includes GraphQL storage-health compatibility, MCP storage-health compatibility, P077 closeout readback, and P088 implementation-completion list/get compatibility tests. |
| REQ-011 | Document and expose SQLite exit criteria without making a migration decision before real metrics. | Implemented with promotion risk | Proposal and reference docs retain the exit criteria, and rollout fixtures expose decision fields. Promotion thresholds are present, but some readback values are static policy targets rather than live telemetry-derived proof. |

## Service Flow Review

### `runs.list`

Conformance is strong. The MCP tool description is `List all runs`, and the handler calls `projections::list_all_projection`. That query reads `runs`, left joins `run_summaries`, left joins live approvals, exposes projection presence/lag, and does not attach artifacts, transcripts, reports, or detail evidence. This meets both the projection-only and full-history requirements.

### MCP hot reads and liveness

Conformance is strong. `handle_hot_read_json_rpc` checks the hot-read guard, applies a shorter probe timeout or normal timeout, scopes the read through `db::writer::CANCELLATION_TOKEN`, records hot-read latency, records explicit liveness gate timing for `runtime.health` and `tools.list`, reports typed timeout/circuit results, and records guard success or violation. Tests cover `runtime.health`, `tools/list`, `resources/read`, `storage.health`, circuit-open typed results, and liveness while maintenance is running.

### Projection invalidation lifecycle

Conformance is now strong. The invalidation repository coalesces unconsumed same-key rows, preserves consumed rows, records backlog metrics, throttles/freezes on capacity exhaustion, supports audited backlog/poison clearing, exposes drain priority ordered by oldest unconsumed watermark, and marks rows consumed after successful projection rebuilds. The maintenance reaper drains one pending source after reaping and freezes unknown or failed sources. This closes the earlier risk that invalidation bookkeeping existed without a production consumer.

### Storage health and rollout readback

Base conformance is good. The implementation exposes storage-tiering status, liveness status, projection rebuild status, hot-read enforcement status, compatibility status, per-tool circuit state, projection freshness, maintenance counts, reaper status, invalidation backlog status, rollout decision fields, required metrics, and negative fixtures.

The promotion-readiness risk is that `p087_would_open_rate`, `p087_flap_free_window_hours`, and `p087_min_hot_read_requests_per_surface` are emitted as constants/policy values in storage health. Metrics declarations include hot-read and circuit counters, but I did not find an implementation that derives those promotion values from a time-windowed live sample history. This does not block implementation conformance because Phase 5 real-run inspection remains a future storage decision checkpoint, but it should block treating the rollout fixture alone as proof that enforce-mode traffic has met the 48-hour decision budget.

## Specialist Findings

### OPS-001 - Rollout promotion fields are present but not live telemetry proof

- Severity: Major for production promotion, not a conformance blocker.
- Confidence: High.
- Reviewer lens: `observability_rollout_reviewer`, `rust_performance_reviewer`.
- Evidence: `control-plane/crates/db/src/repos/storage_health.rs` emits `p087_would_open_rate: 0.0`, `p087_flap_free_window_hours: 48`, and `p087_min_hot_read_requests_per_surface: 100` as fixed readback values. `control-plane/crates/db/src/metrics.rs` declares hot-read circuit counters and latency/sample helpers, but does not derive a rolling would-open rate or flap-free window from recorded traffic.
- Impact: Operators could mistake field presence and a passing fixture for evidence that the live enforce-mode cutover budget has actually been satisfied.
- Acceptance condition: Derive these promotion fields from persisted or bounded in-memory hot-read/circuit sample history, or explicitly label them as policy targets/unverified until real-run telemetry proves them.

### READY-001 - The worktree contains mixed, unstaged, and out-of-scope changes

- Severity: Minor for implementation conformance, major for handoff hygiene.
- Confidence: High.
- Reviewer lens: `rust_backend_arch_reviewer`.
- Evidence: `git status --short --branch` shows a broad dirty tree, untracked prior audit reports, `.junie/plans/`, `CHAINWORKS_OUTPUT`, Swift sample-run/read-start files, and P087 Rust/doc/evidence changes in the same worktree.
- Impact: The current tree passes the P087 gate, but the final review/merge unit needs an intentional staged or committed slice. The untracked Swift files are outside P087's stated non-goal of no new UI write controls and should be either justified, separately proposed, or excluded.
- Acceptance condition: Stage or commit the coherent P087 implementation only, classify/remove unrelated generated output and Swift sample-run files, then rerun `./scripts/test-gate.sh proposal-087`.

### READY-002 - Fresh focused gate passed; broader regression remains outside this audit

- Severity: Minor.
- Confidence: High.
- Reviewer lens: `rust_reliability_reviewer`.
- Evidence: The canonical `proposal-087` gate passed in this same tree during this audit. I did not run the full Swift app gate, full control-plane cargo workspace, or remote UI smoke gate as part of R8.
- Impact: The proposal has sufficient focused proof for `Ready with Risks`, but branch promotion to a wider integration milestone should still run the repository's broader sign-off path.
- Acceptance condition: For merge/release promotion, run the broader requested gate set, especially if keeping the Swift file changes in scope.

### REL-001 - Cursor freeze after projection rebuild failure is best-effort follow-up work

- Severity: Note.
- Confidence: Medium.
- Reviewer lens: `rust_reliability_reviewer`.
- Evidence: `rebuild_all_for_run` calls `freeze_cursor_after_retry_exhaustion` after a rebuild error is observed. That protects the visible cursor state after the failure path returns, and drain/reaper tests cover it.
- Impact: A process death between rebuild failure and the follow-up freeze could leave the cursor unfrozen until the next retry/reaper pass. This is acceptable for the current proposal because the reaper path and retry cycle make the condition visible eventually, but it is worth tracking if operators require immediate poison readback on every failed consumer attempt.
- Acceptance condition: Either keep the current retry/reaper contract documented, or make failure marking atomic with the failed consumer transaction where feasible.

## Verification Log

Fresh verification run in this audit:

```text
./scripts/test-gate.sh proposal-087
```

Result: passed.

Observed proof points:

- P087 migration version uniqueness check passed.
- DB P087 suite: 25 tests passed, 0 failed.
- MCP P087 suite: 16 tests passed, 0 failed.
- P077 compatibility list/get check: 1 test passed, 0 failed.
- P088 compatibility list/get check: 1 test passed, 0 failed.
- Auth P087 suite: 2 tests passed, 0 failed.
- Engine restart projection rebuild integration: 1 test passed, 0 failed.
- GraphQL `storage_health_v1`: 1 test passed, 0 failed.
- GraphQL P087 suite: 5 tests passed, 0 failed.
- Static UI/schema/evidence checks printed `P087 UI, schema, and evidence verified`.
- Final gate line: `==> Proposal 087 gate passed`.

Warnings observed were existing Rust warnings for unused/dead code and lifetime syntax; none failed the gate.

## Final Verdict

P087 is implemented. The implementation now has the required projection-only read path, hot-read guard/liveness behavior, projection invalidation drain/freeze lifecycle, storage health/readback surface, compatibility fixtures, and canonical proposal gate.

Readiness is `Ready with Risks`, not unconditional `Ready`, because rollout-promotion fields for would-open rate and flap-free window are currently policy-shaped/static readback values rather than live telemetry-derived proof, and the dirty worktree still needs a coherent final staging/merge slice.
