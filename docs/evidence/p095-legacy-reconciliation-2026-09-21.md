# P095 Authorized Legacy Lifecycle Reconciliation

Date: 2026-09-21. Result: the authorized historical-record correction is complete.
The subsequent carry-forward preview no longer reports `source_busy`; it reports
`continuation_budget_exceeded`. No successor was reserved or activated.

## Authority And Scope

The operator explicitly authorized correcting the 272 legacy provider records
and 159 active escalation ledgers identified in the
[initial live rollout](p039-live-rollout-2026-09-21.md). This was a scoped manual
maintenance operation, not a new generic recovery API or a guard exception.
The source run is `bd83a310-360f-4d45-b4c6-a94873de3733`.

Live MCP readback established the source and runtime state first. The available
MCP surface has no command for these legacy records. The process-absence command
requires a held cancellation intent, and none existed; no such intent was
fabricated. The correction therefore used a reviewed, run-specific SQLite
transaction after the explicit operator authorization and a copy rehearsal.

Before applying, all 272 associated agent executions were terminal with completion
timestamps and belonged to P095. All 159 active ledgers belonged to terminal
source stages: 84 completed and 75 skipped. There was no active source work,
agent or stage, cancellation intent, shutdown signal or interrupted receipt,
continuation operation or source fence. Effective-resource ownership was P095
only. Fresh runtime readback showed zero active sessions, continuations or
unresolved effects.

Independent process inventories at planning and immediately before applying
found no ACP provider process or daemon child. The 101 remaining provider-like
processes were classified by executable, command hash, ancestor identity and
working directory as unrelated desktop or terminal sessions. No source run
ID/worktree binding or source working directory was present. No process was
signalled. The new absence timestamp records verification now, not an invented
historical exit time or exit status. Terminal agents alone were not treated as
proof of process absence.

## Backup And Transaction

Private evidence directory, mode 0700:
`~/Library/Application Support/Chainworks Forge/maintenance/p095-legacy-20260921-01/`.
Its SQLite backup and JSON records are mode 0600; they contain no bearer token.

| File | SHA-256 |
| --- | --- |
| `before.sqlite` | `45b368f38694466e45ca680264f88828d60bf2294708a1f9fd528cb5760e4df7` |
| `plan.json` | `dbb432928d68abd44c9e3e62b17f571111aa940ed878927457a516161a2aa08c` |
| `applied.json` | `9d644929d72f75a4717310a0bfee4aaebdb3fbf4f2e1fe52e8725ed29a4c9e0e` |
| `maintenance-script.py` | `1e807dbaee1458c9dc65271f2c2ddc83313c298292088c19efc5b7efcda57227` |

SQLite's backup API produced a consistent snapshot. Full `integrity_check`
passed. A disposable copy successfully rehearsed the exact transaction and
passed integrity and foreign-key checks. A repeated apply was rejected. A
second copy with a changed source status rejected before any provider update.
Only these two disposable test copies were removed afterward; the original
backup, full before/after rows, runtime/process evidence and script are retained.

The live `BEGIN IMMEDIATE` transaction compared the source and candidate rows
with the verified plan, rechecked terminal ownership and updated exact row IDs.
It committed at `2026-09-21T19:50:21.456795+00:00`:

- 272 provider rows: `live/running` to `orphan_settled/absent_verified`.
- 159 terminal-stage ledger rows: `active` to `cancelled`.
- 159 appended `escalation.legacy_reconciled` events, each referencing the plan
  SHA-256 and stamped `redaction_v1`.
- Zero deleted rows; six previously paused ledgers unchanged.

Source run, workflow/catalog snapshots, stages, agent executions, work items,
approvals and session generations were unchanged by digest comparison, as were
other runs' provider and escalation rows. This is lifecycle metadata repair,
not database compaction, artifact deletion or source-run cancellation. It does
not fix the producer defect for future sessions or clean unrelated runs.

## Readback And Next Barrier

Independent SQLite readback confirmed the exact counts above. MCP `runs.get`
still reports the original blocked source, unchanged frozen hashes and no
successor. Its worktree remains clean at
`82b1d72581e6503e5e19a40e5a1164b2b9f2ae3b`.

One fresh read-only preview returned `continuation_budget_exceeded`, not
`source_busy`. The actual mandatory selection predicate selects 215 historical
artifact identities; `preview/selection.rs` currently allows at most 128.
This is a measured independent budget violation. The public hold response does
not expose its internal error suffix, and later limits/provenance have not been
claimed to pass. The request's `limit: 100` controls pagination, not this bound.
No historical finding was deleted, omitted or rebound to current bytes.

The next prerequisite is a bounded carry-forward representation for the real
historical reference set, with matching wire/runtime limits and preservation
tests. Raising a page size or clearing more lifecycle rows cannot resolve it.
Preparation, fresh review/approval, headless execution and proposal retirement
remain unproved.

The previously authorized temporary `p039-migration-operator` was recreated only
with its prior continuation/read capabilities, then removed. Existing principals
were preserved; a protected `tools/call` read with the removed token confirmed
revocation. No daemon restart/rebuild, Apple consent, project trust, approval,
provider invocation or continuation mutation occurred in this correction.
