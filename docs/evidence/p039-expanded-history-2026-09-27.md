# P039 Expanded Historical References

Date: 2026-09-27. Base: `b07bfbf4de8d0dfce1320b3e8c630ba593c0a132`.
Scope: isolated `codex/p039-carry-forward` worktree. This is a bounded
follow-up to the [P095 reference-budget hold](p095-legacy-reconciliation-2026-09-21.md).
It includes deployment evidence, but is not a completed P095 transfer receipt.

Status: merged and pushed as `b33159511aa1d9fc50ca58f51b018d2bcf70a041`.
Model-policy compatibility was integrated as
`dc4ea294f25ed1ce9de83e63b93430877e9a1be2`. The signed bundle is installed and
running at schema 102. Live P095 preview is held on historical artifact
provenance; the transfer is not complete. See the final cutover section below.

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

## Initial Verification Boundary

At the initial offline verification cutoff, no production rows were deleted or rewritten, no temporary principal was created,
no daemon was restarted, and no new P095 preview/reservation/activation or Apple
operation was submitted in this follow-up. Production queries were read-only.
No merge or push had occurred at that cutoff. Existing unrelated model/catalog edits stayed in
the primary worktree.

After deployment with the normal verified database backup, obtain a fresh P095
preview. A previous held preview is not a resumable preparation and supplies no
new plan hash. Later provenance, missing preserved generations, approval and
headless admission checks remain fail-closed and are not proved by these fixtures.
Full P039 closeout and proposal retirement remain separate.

## Pre-Cutover Verification

At 2026-09-27T16:14Z, GitHub `main` was read back at `dc4ea294...` after two
fast-forward integrations. The dirty primary checkout was not reset or merged;
only the 12 previously prepared model-policy files were transferred into the
isolated delivery branch. Antigravity's source-only review found no actionable
defects. Private SQLite state was not delegated to the A2A peer.

The old 1,994-byte pinned model policy rejected the user's updated 2,269-byte
matrix (`TargetProfileInvalid`). The synchronized Rust/Swift pins and catalog
update resolve that compatibility mismatch without changing historical snapshots.

Fresh integrated verification:

- `proposal-039`: exit 0, 354 Rust tests across 39 selections, zero failed/ignored;
  29 execution-coverage cases and 168 mapped tests; final coverage check PASS.
- `codex-planned-variant-slice`: exit 0, 17 Swift tests and 40 Rust tests. The
  first default-stack Rust run overflowed; the passing rerun used the established
  `RUST_MIN_STACK=8388608` setting. This is not a full-workspace test claim.
- `scripts/test-gate.sh build`: exit 0. A separate build bundle, without the
  test-only XCTest plugin, was used for packaging.
- Standalone daemon built with full `GIT_SHA=dc4ea294...` through managed Cargo.
  App and daemon were signed with the existing Developer ID team `R859U8DTY9`;
  `codesign --verify --deep --strict` passed for the staged installable bundle.
  Both launch-agent plists retain carry-forward enablement and the full build SHA.

| Local Evidence | SHA-256 |
| --- | --- |
| `/private/tmp/p039-delivery-20260927/integrated-p039-gate.log` | `49220b6d8777113684bfb76f7f17c7a72bd6e52a33606d9c9d1ddd683423e0a9` |
| `/private/tmp/p039-delivery-20260927/model-policy-gate.log` | `c7a9e4fadafd2d421d589e70d77c960b7653c2d2fdb6c7725a1e2b0b3f850e6d` |
| `/private/tmp/p039-delivery-20260927/app-build.log` | `8334ef6c69a133db7579451d26425ca891b4070f1104f8c05b13f19e4183a93f` |
| Staged signed daemon | `1f5daaab9e9b2706c4835b3b31db0bf23cc4b3183e475f5f21b5e00dc2cef567` |

Before cutover, the staged bundle was
`/private/tmp/p039-delivery-20260927/installable-Chainworks Forge.app`;
cutover moved it to `/Applications/Chainworks Forge.app`.
The previous installed bundle is preserved in the same private directory.
No invalid/test-only staged candidate was installed.

A private SQLite backup made via the backup API passed full `integrity_check`
at schema 101. Its SHA-256 is
`476cb3fc8037a0bf41e156e0d74a835bda7870c3403b6dba1ba5533c88405016`.
This was a diagnostic snapshot during unrelated P070 work, not the final
quiescent migration backup. A fresh verified backup is required before cutover.

The local preview harness opened that snapshot read-only/immutable and ran the
new engine without starting another daemon or dispatching a provider. Target
compilation passed and preview returned `ArtifactProvenanceInvalid`, not the
old reference-count or model-policy hold. This is **offline evidence only**.

Bounded snapshot inspection found 125 `proposal_review_v1` records at six reused
canonical paths, with no historical SHA-256 values. The 27 summary generations
also reuse one canonical path; the available generation rows cover summaries,
not all required historical review inputs. Implementation plan/backlog each have
two records with different generic provider contracts. These facts are consistent
with the existing generation/provenance checks rejecting the migration. The
public hold code does not identify a unique first failed check. Current bytes
must not be relabeled as the content of older versions, and historical rows must
not be deleted merely to obtain an eligible preview.

At the cutoff, MCP still reports P070 B
`2478fbf5-3eb8-4e9d-b805-8b463edf5345` running, and its provider log is advancing.
The old app-owned daemon remains healthy at `b07bfbf4...`, schema 101. It has not
been interrupted. No live P095 preview, reservation, activation, approval, or
Apple operation was performed in this delivery follow-up. A temporarily scoped
operator was used for readback; its revocation is recorded separately below.

The [ACP dependency audit](acp-dependency-audit-2026-09-27.md) records independent
provider upgrade candidates and initialize-only checks. Neither the signed
bundle nor those checks prove a completed P095 transfer. Proposal retirement is
still withheld.

### Temporary Access Closeout

The temporary `p039-migration-operator` was removed from the auth source. The
initial immediate check ran before the daemon's default two-second principal
reload and still succeeded. A bounded verification then rotated the same ID to
a fresh **read-only `runtime.health` credential**, observed that reload, removed
it, and confirmed denial of an authenticated MCP tool call after reload. This
also displaced the earlier write-capable credential from the loaded table.
Only the original two principals remain; their policies were not changed.
No credential was printed or retained in evidence.

## Live Cutover

After the earlier waiting cutoff, P070 B settled. MCP returned no active runs,
zero active provider sessions, and zero unresolved side effects. A fresh SQLite
backup passed `integrity_check` at schema 101:
`/private/tmp/p039-delivery-20260927/verified-quiescent-pre-v102.sqlite`, SHA-256
`f1b4d7feac8f44731653400cb998bb79fe4e624f6435f49e8bd992cd57de3327`.

The app exited; its remaining daemon received SIGTERM and exited before install.
The signed standalone build bundle replaced `/Applications/Chainworks Forge.app`;
the previous original bundle remains in the private delivery directory.
The three ACP adapter upgrades and global initialize checks are recorded in the
[dependency audit](acp-dependency-audit-2026-09-27.md).

At `2026-09-27T16:24:45.509756Z`, the app-owned daemon started successfully.
`/ready` returned `ready`, schema/binary schema 102, exact build SHA
`dc4ea294f25ed1ce9de83e63b93430877e9a1be2`, healthy headless Xcode broker, and
zero active leases/backend sessions. Startup also produced its automatic
schema-101-to-102 backup, independently opened read-only/immutable and verified
at schema 101 with a passing full `integrity_check`. No Xcode IDE or Apple
project operation was launched.
The application connected and displayed its existing run list after refresh.

MCP `storage.health` reported `HEALTHY`, fresh data, writer alive, queue depth 0,
transaction p95 5 ms, and no subquery failures. The pre-existing `run_summaries`
`projection_error` remains visible and is **not fixed** by this deployment.

One fresh authorized live `runs.continuation_preview` used the original P095
delivery identity, current canonical definitions, and the same approved proposal
artifact. It returned `run_carry_forward_preview_hold_v1` with
`artifact_provenance_invalid` and `next_action=resolve_hold`. Response evidence:
`/private/tmp/p039-delivery-20260927/1790526365361604000-runs_continuation_preview-response.json`,
SHA-256 `7dd1dc0c9d3908d51bd4e6cc3237035fc84ab1128f17489059685481f7322c5f`.

Readback afterwards confirmed P095 remains blocked, with no successor or
carry-forward operation, and unchanged frozen hashes:

- Workflow: `2538c5505ea0d2385f8d34f5675a1b47ab89ec686249a1bbe11854bdd8316885`.
- Catalog: `73c4dd59f1329505eb005a88b6bac28473ffbd33433acf3f91dbcfe27a2ebf18`.

No preparation, activation, approval, historical-row deletion, checksum backfill,
or artifact rewrite followed the hold. The expanded-history fix and model/ACP
updates are deployed; P095 transfer and full P039 retirement remain blocked on
independently verifiable historical preservation, not the old 128-reference cap.

Further read-only inspection found preserved `undeclared_envelope_outputs` JSON
files that may be recovery candidates. Their artifact rows have no execution ID
or historical checksum, and envelope content is not necessarily identical to the
canonical filesystem output. Existence alone does not establish a unique binding
to each required historical generation. This is not evidence that all old bytes
are lost, and no timestamp-based binding or fabricated checksum was installed.

The scoped `p039-migration-operator` was recreated for this cutover/readback,
then removed again. An authenticated call using that credential was denied after
the principal reload interval. Only the original principals remain, with their
policies unchanged. No temporary administrative access is left active.
