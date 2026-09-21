# P039 Verification And Rollout

Revision `p039-r2`, 2026-09-20. Proof obligations, not test results.
Parent: [P039](../039-blocked-run-fork-and-canonical-carry-forward.md).

## 1. Delivery Slices

These are reviewable implementation boundaries, not an approved executable plan.
Write the task-level plan after written-spec review. No incomplete slice may
expose live continuation admission.

| Slice | Change | Exit proof |
| --- | --- | --- |
| I1: provider-free experiment | Domain/profile/planner/materializer against temporary DBs and two disposable repos, production admission off | H1, preserved dirty work, current catalog, fresh review/approval, no inherited authority |
| I2: durable operation | WAL-consistent migration backup, migration/DbWriter, journal/idempotency, source/idea fences, worker, atomic activation | Verified restore, crash/race matrix, exactly one successor, no old-source dispatch/deletion |
| I3: integration | MCP capabilities/dispatch, input resolution, GraphQL/report/release lineage, manifest-bound ordinary approval | Auth negatives, historical schema reader, stale readback, four-lane parity; no new UI command |
| I4: live enablement | Schema compatibility, admission gate, offline proof, independent execution-truth review, approved deployment, P095 canary | Preserved original, linked successor re-reviewed and waiting for new approval; separate headless live proof |

I1 cannot touch production tables or P095. Its fixture interfaces must be the ones
I2/I3 integrate; a fake importer is not evidence of production wiring. I1 green
does not make I4 Ready.

## 2. Implementation Map

Paths are repository-relative. Proposed additions are not implemented APIs.

| Component | Current owners / intended addition |
| --- | --- |
| Domain | `control-plane/crates/domain/src/commands.rs`, `run.rs`; new `run_continuation.rs`; lifecycle TTL/failed-terminal contracts |
| Compiler | `control-plane/crates/workflow/src/compiler.rs`, `plan.rs`, `definition.rs`; optional closed profile, topology/compatibility fixtures |
| DB | `control-plane/crates/db/migrations/`, `src/migrate.rs`, `repos/runs.rs`, `repos/scheduler.rs`, `repos/work_items.rs`; new `repos/run_continuations.rs`, registered writer ops |
| Engine | `command_handler.rs`, `work_queue.rs`, `orchestrator.rs`, `executor.rs`, `recovery.rs`; focused `run_continuation/` module, not a giant CommandHandler addition |
| Git/files | `engine/src/worktree.rs`, `worktree_fingerprint.rs`; pinned OID/ownership checks; existing changed-file fingerprint is not complete preservation |
| Inputs | Existing artifact/input resolver and output settlement; installed continuation input origin without fake provider rows or weakened freshness |
| Auth/MCP | `auth/src/lib.rs`, boundary matrix/embedded policy; `mcp-server/src/tools/runs.rs`, lifecycle dispatcher registration |
| Readback | `graphql-server/src/types/run.rs`, `schema.rs`, DB projections, MCP runs/reports, actual report/release writers; additive Swift read DTOs only as needed |
| Gates/docs | `scripts/test-gate.sh`, `docs/reference/test-gates.md`, P039 fixtures; reference promotion only after implementation acceptance |

No Go/Temporal extraction, P070 refactor, P038 compaction or UI command project is
required. Preserve other tasks' dirty work and do not retrofit their run state.

## 3. Gates

Planned aliases `proposal-039|p039` must be registered in I2, documented in I3,
and green before I4. They do not exist at this specification revision:

```bash
./scripts/test-gate.sh proposal-039
```

The gate uses managed Cargo, isolated repositories/databases and provider-free
fixtures covering domain, workflow, DB, engine, auth, MCP and GraphQL. Map every
CF case below to a test; absence/skipping fails the gate. It never starts a live
daemon, production DB, Apple Service/IDE, provider, network Git or local UI test.
Swift changes use repository build/focused gates; UI smoke remains remote-only.

Documentation checks cover line budgets, links, fixture JSON and
`./scripts/lint-rollout-contract docs/evidence/rollout-contract/p039-rollout-contract.json`.
They do not prove migration, execution behavior or live headless acceptance.

## 4. Acceptance Matrix

| ID | Case | Required result |
| --- | --- | --- |
| CF-01 | P095-shaped source, old catalog, new explicit headless policy | Distinct linked ID/root/branch/catalog; fresh review; no code before new approval |
| CF-02 | Text/binary staged and unstaged edits, rename/delete/mode/untracked source | Preservation hashes and target final bytes match; original index/bytes untouched |
| CF-03 | Old green tests, approvals, active index, run-state and sessions | Historical-only, no new successful stage/output/approval/session authority |
| CF-04 | Concurrent prepares/operators, new keys and replay after TTL | One live operation per source/idea, no sibling/repeated effect |
| CF-05 | Crash before/after reserve, each Git/copy effect, manifest, activation commit/response | Hold/reconcile or exact prior result; no duplicate run/queue item, overwrite or cleanup |
| CF-06 | Retry/cancel/conflict/retrofit/approval/main-sync/escalation/dispatch/repair/recovery races | Guard wins or reservation denies busy source; stale owners cannot change historical execution |
| CF-07 | StartRun race, another runnable run; successor granted approval then blocks before producing any new proposal artifact | One runnable head; second fork selects its installed carried input with immediate-source and ancestral provenance, no fake provider rows |
| CF-08 | Unsettled/ambiguous provider, signal, repair, release or headless effect | Hold; no implicit kill/consent/retry |
| CF-09 | Source/index/ref/proposal/target definition changes after preview | Witness mismatch, original unchanged, no activation; explicit abort/new preview |
| CF-10 | Prepared file/root replaced, escape, symlink swap, external link, unowned destination | No follow/overwrite; hold; explicit machine-link exclusion does not copy target content |
| CF-11 | Partial copy, disk-full, limits/deadline, LFS/filter/submodule | Bounded hold, no truncation-as-success, network hydration or unsafe filter execution |
| CF-12 | Narrow/revoked Operator, absent versus empty/list run_scope, agent/observer/automation, foreign read/replay | Only absent scope qualifies as global; deny other writes/replays before business changes and data disclosure |
| CF-13 | Missing new headless grant but old shell/Xcode labels present | No inferred grant or retrofit; source hashes unchanged |
| CF-14 | Invalid profile/topology, writable review, bypass around manual gate | Compiler rejects, no arbitrary entrypoint |
| CF-15 | Historical approval lacks proposal-hash binding | Unproven diagnostic, never inherited granted decision |
| CF-16 | Review/tuple changes or rejection, with earlier grant retained at the same logical gate | Persisted exact-stage binding/supersession; old grant cannot satisfy transition/dispatch; legitimate post-grant code work does not retrospectively invalidate consumed entry |
| CF-17 | Unresolved human/scope findings; removed old schema/tool | Findings reach new reviewers/planner; readable history, no re-enabled tool/auto-waiver |
| CF-18 | Main differs from carried checkout; review/refinement/dynamic reviewer execution | Context/cwd/path expansion/Xcode read root is pinned successor checkout, metadata-only writes; no main fallback, silent rebase or old session |
| CF-19 | Notification/projection failure after activation | Canonical single successor; rebuild restores links; stale UI cannot authorize approval |
| CF-20 | Abort before worker claim, between verified steps, during effect, and after activation | Queue invalidation/generation revoke; fence retained through worker/effect settlement; no stale post-abort mutation or source auto-resume; activated abort denied |
| CF-21 | WAL contains committed uncheckpointed rows; backup/restore, migration, old rows, duplicates/FKs, interruption | Isolated restore contains committed rows; verified consistent backup required before migration; additive compatibility and fail-closed failures |
| CF-22 | Admission disabled; older daemon opens P039 DB | No new prepare/activation; fences/readers remain; older schema-incompatible daemon refused |
| CF-23 | Four readback lanes, pagination and secrets/runtime config | Normalized/redacted bounded readback, no private credential/path leak |
| CF-24 | First new task needs project trust or Apple consent | Separate legitimate grant or hold; no copied trust, unsafe allow-all or IDE fallback |
| CF-25 | Installed seed vs new valid output; reference-only with gate-like names | Typed input precedence, no reference/seed promotion to successful provider contract |
| CF-26 | Copy paused with queued preparations; overload; unrelated idea AdvanceRun | One active/three queued maximum; excess admission makes no fence; unrelated coordination/provider work progresses |
| CF-27 | Preview including delivery checks, multi-page inventory, source changes between pages | Zero filesystem/DB writes; all pages accessible before prepare, one full inventory digest, stale witness returns preview_stale |
| CF-28 | Accepted/replayed/denied/stale CAS/uncertain effect/unknown commit over MCP | Strict versioned result union and transport mapping; expected holds are not INTERNAL; uncertain outcome cannot cause a new-key retry |
| CF-29 | A -> B -> C, B incoming activated and outgoing needs_reconciliation; outgoing abort | Distinct directional readback on all lanes, no timestamp-based collapse; abort clears current outgoing but preserves incoming/history |

Use deterministic crash/race injection for CF-05/06, not timing sleeps. Unknown
outcome counts as correct only when the expected result is a hold, not migration.

## 5. Rollout And Rollback

Add migration `p039_run_carry_forward_v1`; do not rewrite old status/snapshots.
Current `db/src/migrate.rs::write_backup` copies the main file and compares size;
it is not sufficient proof of a WAL-consistent snapshot. I2 must replace that
prerequisite with a SQLite-consistent backup operation, durably publish it, and
verify integrity plus an isolated restore of committed uncheckpointed rows in
CF-21. Backup/verification failure blocks migration. Do not introduce ad hoc
live-file copying as a P039 fallback. Pin manifest readers independently of the
current target catalog.

Default `CHAINWORKS_RUN_CARRY_FORWARD_ENABLED=false`. Disable affects new prepare
and activation only; it never disables persisted fences, existing input readers,
lineage/idempotency/reconciliation or successor execution protection. No
permissive bypass. Expose mode/revision in operator readback.

Existing `db/src/migrate.rs` rejects schema newer than the binary. Register the
new migration and prove that an older binary cannot serve the upgraded DB before
admission. Keep the daemon's database/root locks and generation identity. A
rollback build must understand P039 schema, fences and input origins; disabling
the feature is not permission to launch the previous incompatible executable.

Rollback stops new admission, drains/holds preparation and retains all operations,
files/refs/lineage/fences. Deploy a compatible diagnostic build. No source
reactivation, successor deletion, schema downgrade or old-DB restore over newer
unrelated work. Existing successors use ordinary run policy; feature disable
must not silently cancel them.

The existing rollout `operator_readback_v1.enabled_state` describes enforcement,
not P039 admission; keep its producer semantics. P039 uses its own typed
`admission` object in continuation readback (`enabled`, `reason`,
`fences_enforced`, `reconcile_available`). Do not inject new meanings into the
shared rollout DTO. CF-22/23 must show enforcement enabled alongside continuation
admission disabled on all lanes, with fences and reconciliation still available.

Bounded-label metrics (no paths, IDs, hashes or principals):

- `run_continuation_admission_total{result,reason}`;
- `run_continuation_phase_duration_ms{phase}`;
- `run_continuation_hold_total{reason}`;
- `run_continuation_source_guard_denied_total{surface}`;
- `run_continuation_preserved_bytes_total`;
- `run_continuation_activation_total{result}`.

Holds older than 15 minutes are operator-visible, never auto-released. Detailed
IDs belong only in privileged evidence. The
[rollout declaration](../../evidence/rollout-contract/p039-rollout-contract.json)
and fixtures below are expected shapes, not actual live measurements/results.

## 6. Live P095 Acceptance

After I1-I3 review/offline proof, confirm exact live scope. The old preservation
and approval do not authorize new native consent, provider execution, release,
push, project trust or deleting source material.

1. Re-read P095 canonical ownership, ordinary/headless effects and source files.
   Reverify preserved proposal/current bytes; the old snapshot is not authority.
2. Preview current definitions/profile/skills and headless policy, capability
   delta, selected proposal, mandatory findings and machine-path exclusions.
3. Prepare into new roots; inspect verified manifest/preservation receipts and
   unchanged source branch/index/files/state.
4. Activate once; read back linked IDs, distinct roots, new hashes and one review
   work item. Do not interrupt P070 or other runs with an unsolicited restart.
5. Observe fresh review/new approval. Missing project trust/native consent is a
   separate hold, not failed preservation and not permission for IDE fallback.
6. After the new approval and any required trust/consent, observe one current-
   policy implementation attempt and its own outputs. Keep source/archive pinned.

Report preservation, activation, review/approval and headless acceptance separately.
Only completed applicable stages can be called migrated/accepted. No bulk cleanup.
