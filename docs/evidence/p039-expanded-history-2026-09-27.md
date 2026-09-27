# P039 Expanded Historical References

Date: 2026-09-27. Base: `b07bfbf4de8d0dfce1320b3e8c630ba593c0a132`.
Scope: isolated, uncommitted `codex/p039-carry-forward` worktree. This is a bounded
follow-up to the [P095 reference-budget hold](p095-legacy-reconciliation-2026-09-21.md),
not a deployment or completed P095 transfer receipt.

Status: implemented and verified offline; not merged or deployed.

## Change

- Explicit client references remain bounded at 128. Server-expanded references
  use a shared 512 ceiling, plus one execution seed, across preview, finalization,
  manifests, installation, readback, agent inputs and subsequent carry-forward.
- Readers query one extra row and reject overflow instead of returning a prefix.
  Different historical identities are retained even when their content matches.
- Migration 102 preserves all installed/prepared rows and constraints while
  widening the ordinal ceiling from 128 to 512. Migration 101 is unchanged.
- Finalizer intent v2 stores the complete input set's canonical digest and count.
  Full descriptors remain in per-input intents and immutable manifest evidence;
  the 64 KiB database journal-row limit is unchanged.
- Agent references use lossless JSON batches in the existing sealed ACP snapshot
  handoff. Records retain input identity/name, historical path, full text, size
  and SHA-256. Snapshot/prompt limits and authority checks are not relaxed.
- Independent witness lists remain limited to 128 IDs. Read-only queries of the
  current local P095 database observed 49 journal IDs, 2 approvals and 27 contract
  generations, so these ceilings are not the reported 215-reference blocker.

The [reference](../reference/blocked-run-carry-forward.md) and
[runtime contract](../proposals/039/runtime-and-wire-contract.md) describe the
new bounds; neither promises arbitrary-size historical migration.

## Verification

Red/green sequence used real disposable Git checkouts and SQLite databases:

1. The 215-reference regression first failed with `ContinuationBudgetExceeded`.
2. After aligning count bounds, preparation exposed the independent 64 KiB
   `intent_json` CHECK failure. Compact digest-bound intent v2 corrected it.
3. Complete 215- and 512-reference roundtrips passed: prepare, activation,
   paged readback, full identity/provenance sets, all agent-visible snapshot
   contents and hashes, actual ACP request preflight, report fields and a second
   fork preview. Tampering beyond the old 128th entry is rejected.
4. A 513-reference source holds without a reservation or silent truncation.
5. Schema-101 fixtures upgrade losslessly to 102, retaining foreign keys,
   seed/ordinal/path uniqueness and installed-state checks. Ordinal 512 is
   accepted; 513 and negative ordinals are rejected. A schema-101 migrator
   refuses 102; no schema references to the temporary rebuild table remain.

Commands run through the managed Cargo policy:

```sh
cd control-plane
../scripts/cargo-managed test -p engine --test proposal_039_large_history -- --nocapture
../scripts/cargo-managed test --locked --offline -p db --test proposal_039_history_migration
cd ..
./scripts/test-gate.sh proposal-039
```

Canonical `proposal-039` gate: **exit 0**, **354 selected Rust tests passed**,
zero failed/ignored across 39 selections. Coverage-checker unit tests: **3/3**;
execution coverage: **29 cases, 168 distinct mapped test names**; rollout lint:
**PASS**. This is not a full-workspace test claim.

Final focused roundtrip recheck: **3 passed**, including rejection of 513 installed
reference rows rather than a truncated prefix. Final migration recheck: **1 passed**,
including schema-101 refusal and absence of temporary-table schema references.
These rechecks cover the last negative assertions added while the full gate ran;
production code was unchanged during that gate. Snapshot splitting and escaped
content roundtrips also passed in the gate's engine unit selection.

`git diff --check` passed. All 20 changed code/test/gate file hashes matched the
post-edit content manifest on recheck. Existing compiler warnings were not changed.

| Local Evidence | SHA-256 |
| --- | --- |
| Full Rust selection log, `/var/folders/43/6b8z356950q2rsbvnjmp8jd40000gn/T/p039-test-results.YwVVPK` | `8c89752aa3d3181d16ffc1487c29012b167913272733a0d46c8be62de4f79cb9` |
| Code/test/gate content manifest, `/private/tmp/p039-expanded-history-code-20260927.sha256` | `d38bbf0f33c597894c1805b43e6fe7e2944282286a6b2eb891c7473947a16088` |

Antigravity reviewed the scoped code through A2A and reported no actionable
defects. This is code-review evidence, not independent live or provider execution.

## Operational Boundary

No production rows were deleted or rewritten, no temporary principal was created,
no daemon was restarted, and no new P095 preview/reservation/activation or Apple
operation was submitted in this follow-up. Production queries were read-only.
No merge or push has occurred. Existing unrelated model/catalog edits stay in
the primary worktree.

After deployment with the normal verified database backup, obtain a fresh P095
preview. A previous held preview is not a resumable preparation and supplies no
new plan hash. Later provenance, missing preserved generations, approval and
headless admission checks remain fail-closed and are not proved by these fixtures.
Full P039 closeout and proposal retirement remain separate.
