# P039 Integration Context

Prepared 2026-09-20 against HEAD
`8c40f71a03cfebff413fbad584131e0bcbe9859b` plus the preserved working tree.
Scope: specification refresh, not implementation or live migration.

## Current Owners

- Rust owns command journal, canonical SQLite runs, queue, orchestration and ACP
  providers. SwiftUI is GraphQL observation plus approve/reject only.
- StartRun compiles current workflow/catalog, creates per-run roots and enters
  plan.initial_state. It has no source/provenance/import inputs.
- Frozen catalog retrofit rejects permission-profile drift. Headless runtime
  requires explicit evaluated policy plus separately granted project trust and
  Apple consent; old tool labels do not substitute for it.
- P039 was a 353-line Draft before this revision. There is no current continuation
  command or implementation of continued_from/continued_as run lineage.
- P086 agent work continuation is a different same-run/session feature, not a
  blocked-run successor import mechanism.
- P064 main-sync/barrier tables and readback exist; runtime fence reuse needs
  owner-by-owner verification. P039 must not assume a schema enforces dispatch.
- `db/src/migrate.rs` has a schema-newer-than-binary startup rejection contract.

The intake baseline `.review-baselines/current-system-baseline.md` still names
Goose/three providers. For affected slices use current reference docs and code;
do not update unrelated baseline/UI/P070 work as part of this refresh.

## P095 Preservation Evidence

This section records historical evidence only; live eligibility must be re-read.

- Source run: `bd83a310-360f-4d45-b4c6-a94873de3733`.
- Readback: blocked/state_7_implementation_started, no pending/running work items,
  no unresolved ordinary side effects. This did not prove headless hold clearance.
- Frozen workflow hash:
  `2538c5505ea0d2385f8d34f5675a1b47ab89ec686249a1bbe11854bdd8316885`.
- Frozen catalog hash:
  `73c4dd59f1329505eb005a88b6bac28473ffbd33433acf3f91dbcfe27a2ebf18`.
- Clean worktree HEAD: `82b1d72581e6503e5e19a40e5a1164b2b9f2ae3b`.
- Preserved approved proposal SHA-256:
  `04ecdd50e44cbcae1d0f1ce1dab6cc31280cce615551cda0dd61c25ba4523d44`.
- Private copy: `.chainworks/backups/p095-preservation-20260920-4XfXrw/`.
- Manifest SHA-256:
  `07f9564bd6cc5564fbf71ba957bc71f5a3c5ddbfd69036092c10048a2da65b64`.
- 2,526 source files copied, 2 historical approvals, 49 command-journal rows;
  final checksum inventory verified 2,544 files including evidence/readbacks.
- Same-disk preservation, not independent full Git history or offsite backup.
- Source includes an external machine-config symlink under `.antigravitycli/`;
  preservation did not dereference it. Executable seed policy must handle it.

The local Ops freeze-task alias correction in `executor.rs` is uncommitted and
not deployed; it addresses duplicate approved-proposal discovery, not headless
permissions or carry-forward. P039 does not claim that correction is installed.

## Working Tree Boundary

Existing P103 Swift/UI/tests, P070 roadmap work, executor correction and other
proposal/headless edits belong to other work. P039 edits only its parent, children,
review-preparation artifacts and P039 design-only rollout fixtures. No daemon,
database, source run, Git ref or worktree is changed by this documentation task.
