# P039 Durable Integration Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. Continue in the existing isolated P039 worktree.

**Goal:** Complete blocked-run carry-forward without transferring historical execution authority.

**Architecture:** Keep the provider-free filesystem core from I1. SQLite owns reservation, fences, generation, idempotency, approval bindings and lineage. A separate tracked worker owns preparation; activation alone creates a successor. Public adapters consume one canonical service/readback.

**Tech Stack:** Rust, sqlx SQLite, Tokio, existing MCP/GraphQL, managed Cargo.

**Spec:** `docs/proposals/039-blocked-run-fork-and-canonical-carry-forward.md` and its `039/` children.

## Global Constraints

- Default `CHAINWORKS_RUN_CARRY_FORWARD_ENABLED=false`.
- No production DB, daemon, provider, Apple, remote Git or UI use in offline gates.
- One active preparation plus three queued; preparation deadline includes queue wait.
- No automatic effect retry, activation, source resume, fence expiry or copied approval authority.
- Preserve unrelated main checkout changes; this worktree owns only P039.
- All new writes registered with DbWriter. Restore proof before migration.
- Live canary and integration into main require their explicit operational scope.

## Review Focus

- A backup made while WAL contains committed rows must restore those rows without the original sidecars (Task 1).
- A stale worker must not publish after abort or restart, even with a surviving directory (Tasks 2-3).
- A previously authorized request must be reauthorized before replay disclosure (Task 4).
- A second continuation must retain both incoming and outgoing lineage without promoting a historical result (Task 5).
- Approval of an earlier tuple or gate execution must not authorize the first new code-writing dispatch (Task 5).

## Task 1: Consistent Migration Backup

**Files:** `db/src/migrate.rs` (paths relative to `control-plane/crates/`).
**Interface:** Existing `run_preflight(database_url, backup_dir)` retains its signature; backup failure blocks migration.

- [x] Add `cf21_version_100_upgrade_restores_wal_history_without_source_sidecars`: real backup and independent restore retain committed uncheckpointed rows.
- [x] Record the backup behavioral RED/GREEN in `backup-report.md`; managed `migrate::tests` and migration-backup integration tests pass.
- [x] Replace main-file copy with SQLite `VACUUM INTO ?` using a unique private pending path; verify integrity, fsync, exclusive publication and parent fsync. Close all verification connections before publication.
- [x] Add backup collision/failure negatives and run all `migrate::tests` (17 unit and seven migration integration tests, including the independently identified relative-filename regression; backup-report.md).

## Task 2: Durable Reservation And Source Fences

**Files:** new `db/migrations/101_p039_run_carry_forward_v1.sql`, `db/src/repos/run_continuations.rs`, `db/tests/proposal_039_storage.rs`; update repos module, write-operation registry, run/queue owners.
**Interfaces:** `reserve`, `find`, `check_source_guard`, versioned step/phase transitions, transaction-local activation/abort. Inputs use UUIDs and bounded digests/refs, not filesystem bytes. Unique source/idea reservations and immutable request results survive TTL.

- [x] Storage proofs cover migration tables/FKs, legacy nullable rows, duplicate source/idea requests, source guard before queue mutation, abort generation, startup unknown effects and directional links.
- [x] Implement schema with constraints and partial unique indexes, registered immediate transactions and explicit transition CAS. The approved isolated expansion has 115 SQL guards over 39 tables.
- [x] Wire source/idea admission at canonical run and work-queue boundaries; verify admission-owner parity and queue progress without changing protected history.
- [x] GREEN: storage 13, expanded fences 21, checked admission four and queue progress 12. Independent F1/F2 reruns accepted the corrections; the combined gate includes these targets.

## Task 3: Service And Tracked Preparation Worker

**Files:** `engine/src/run_carry_forward/` service/worker/selection modules; existing command handler, executor and recovery owners; engine integration tests.
**Interfaces:** Read-only preview returns a complete digest-bound plan; reserve accepts the identical witnessed plan; worker uses operation/generation; activate accepts exact version and manifest digest. Reconcile is read-only with respect to external effects.

- [x] Phase-transition, crash-boundary, source-drift, held-effect, queue-capacity, unrelated-progress and read-only preview paging proofs are included in the combined gate.
- [x] Resolve canonical ownership/provenance from DB, compile current target with the closed I1 profile, retain no historical approval authority. Missing authenticated historical material remains a hold.
- [x] Journal effect intent before materialization, check generation before effects/after waits/publication, hold on uncertain outcomes, recover without repeating effects. Reliability follow-up binds the successor index, reserves a transferable lane permit before DB reservation, and reconciles only settled verified aborts. Scoped follow-up: 87 tests pass; independent recheck accepts R1/R2/R3 with 43 focused tests passing and unchanged scoped hashes.
- [x] Atomically activate one successor plus installed inputs, source historical fence, lineage, one AdvanceRun and journal result. Abort revokes generation before settlement. Six rollback boundaries, reopen/replay and competing-operator storage tests pass.
- [x] GREEN: selected managed engine `proposal_039_*` targets plus mapped source-owner regressions in the final 348-test canonical run.

## Task 4: Auth And Wire Adapters

**Files:** domain carry-forward DTOs; auth tool/capability registry; MCP dispatcher and new continuation tool module; schema/auth/transport tests.
**Interfaces:** Six tools and strict versioned schemas from runtime contract section 2. Writes use lifecycle UUIDv4 idempotency, not generic UUIDv7 wrapping.

- [x] RED strict DTO, duplicate-key, bounded paging, auth class/scope/capability, foreign/missing operation, replay-after-revocation and unknown-commit envelope tests; wire-report.md records the focused results and stub-vs-real-service boundary.
- [x] Implement exact explicit capabilities and BoundaryPolicy checks; preserve existing explicit allowlists and default grants. Focused auth negatives pass in the composed gate.
- [x] Connect tools to the canonical service, with typed expected denials and redacted unexpected errors. Actual HTTP service test covers durable denial, stale CAS, abort and freshly authorized replay.
- [x] GREEN: managed domain/auth/MCP proofs, including 24 MCP integration tests in the canonical run. Independent source/regression recheck accepts API-039-01/02/03; ET01 also proves actual service activation, post-commit lost acknowledgement and reopen replay without duplicate dispatch.

## Task 5: Input, Approval And Readback Integration

**Files:** engine typed input resolver/orchestrator/approval owner; db approval bindings; GraphQL run type; MCP run/report/release receipt readbacks.
**Interfaces:** Active validated output precedes installed execution seed; references never satisfy output predicates. Binding is exact stage execution plus immutable tuple, consumed at first post-gate dispatch. `run_carry_forward_links_v1` has independent incoming/outgoing slots.

- [x] Input precedence, reference exclusion, exact approval generation/supersession/consume, stale bytes, A-to-B-to-C, four-lane readback and rebuild proofs are included in the canonical gate.
- [x] Persist fresh approval with binding in the ordinary transaction; require exact current binding on resolution and first dispatch, preserving legacy semantics. Started authority requires persisted matching PromptSent.
- [x] Read installed inputs without fabricated provider artifacts; add canonical lineage projections and additive API/report fields. ET02 historical structured-reference and ET04 post-abort incoming-link proofs are independently accepted within their stated limits.
- [x] GREEN focused tests in db/engine/graphql-server/mcp-server, including the full 14-test ordinary approval runtime target.

Progress: fresh runtime-input suite is 9/9 GREEN, including actual mixed-output
review-to-gate and authorized fallback enqueue/claim/import. Nonempty report and
receipt file/reopen proof is GREEN and independently reviewed. Historical source
compatibility has a scoped 48/48 preview proof and explicit fail-closed limits.
Independent approval review found premature first-dispatch authority and a
later-gate repair gap; both are corrected and independently accepted. The full
ordinary runtime target passed 14/14, zero filtered, with real fixture PromptSent.
Fresh-output F01/F02 code and regression assertions were independently accepted;
that review did not rerun tests or accept historical preview/approval ordering.
The ordinary ACP protocol fixture exposed the external operation-owned metadata
root versus existing workspace-confinement mismatch. The narrowly bound,
non-serialized engine approval and stale-directory-identity correction passed
seven ACP plus one engine test, independently rerun and accepted. No wider
cwd/workspace, arbitrary path, environment grant or serialized authority is allowed.

## Task 6: Audit, Admission And Documentation Closeout

**Files:** `scripts/test-gate.sh`, proof evidence, current audit, stable reference owners and indexes.

- [x] Register `proposal-039|p039` and a CF-01..CF-29 executable test mapping; missing/ignored/filter-only executions fail the checker. The final frozen canonical run passed 348 Rust tests, three checker tests and all 164 distinct required test names across 29 cases, including the corrected producer inventory.
- [x] Run focused gate, broad workspace regressions, all-targets compilation and formatting/diff checks; distinguish baseline failures with exact evidence. Broad: 4011/81/3 ignored; 80 failures reproduce on baseline and the introduced inventory failure is fixed. Daemon units: 137/137. Host-global daemon startup and app-blocked guardrails remain excluded, not passed.
- [x] Independent whole-feature offline execution-truth synthesis accepts candidate 3 with no scoped P039 finding open. Important findings and the final inventory regression have recorded fixes and verification; this is not full-migration or release readiness.
- [x] Obtain exact I4 scope, merge/push the accepted source, deploy the signed daemon and verify schema-100 backup/restore and schema-101 readiness. The narrow temporary operator was separately authorized and removed after readback.
- [ ] Complete live P095 acceptance. Its first preview returned `source_busy`: 272 legacy provider records lack process identities and 159 historical escalation ledgers remain active. No continuation was reserved; see [live evidence](../../evidence/p039-live-rollout-2026-09-21.md). Do not infer process absence, provider/Apple consent or permission for bulk reconciliation.
- [ ] Only after implementation acceptance: use proposal-implementation-closeout inventory, promote implemented truth, repair references, retire proposal-only artifacts, run doc/link checks and retained gate aliases.

## Completion Accounting

Tasks 1-5 and Task 6's offline verification/review are accepted. Task 6 remains
open only for the separately authorized live lane and subsequent retirement.
Broad failure accounting is complete, not green. Stable reference/index and P070
dependency updates are prepared without retiring the proposal. Offline success
is not live canary success. Source `366643e2244f4e8820f16a8a5279eb2b9de018a2`
is merged, pushed and deployed. The actual P095 preview exposed legacy
quiescence prerequisites; preparation, activation and retirement remain open.
