# P039 Implementation Audit R1

Date: 2026-09-20. Source baseline: `8c40f71a03cfebff413fbad584131e0bcbe9859b`.

## Verdict

**Partial implementation: provider-free I1 core only.** The proposal remains active.
There is no production continuation command, DB migration, successor activation,
provider dispatch or P095 migration. H1 remains **Partial**, not supported end to end.

The delivered interfaces are the real domain/workflow/engine core intended for
later integration, not a second fixture-only importer. Tests call these interfaces
using disposable repositories and a temporary SQLite database.

- [Implementation plan](../superpowers/plans/2026-09-20-p039-i1-carry-forward.md)
- [Measured evidence and limitations](../evidence/p039-i1-carry-forward.md)
- [Acceptance matrix](039/verification-and-rollout.md)

## Delivered

| Owner | Implemented behavior | Verification |
| --- | --- | --- |
| `domain::run_carry_forward` | Strict digests, input roles, fresh exact-stage/evidence-bound approval model | 4 contract tests; existing domain regression passed |
| `workflow::carry_forward` | Explicit closed target compilation, review predicates, gate dominance, metadata-only pre-gate authority, derived successor read roots | 11 profile tests; real new-run compiler used in composition |
| `engine::run_carry_forward` | Read-only HEAD/index/untracked inventory; bounded hashing; independent preservation/checkout; source re-observation; no overwrite/replay | 4 unit, 12 inventory and 5 materialization tests |
| SQLite composition | Source run, old approval/catalog and source files remain unchanged; current target needs distinct approval | 1 test, no fabricated successor/stage success |
| Test gate | `proposal-039-i1` / `p039-i1`, managed shared gate cache | 37 passed, no provider/Apple/UI work |

## Acceptance Coverage

| Cases | Current evidence | Remaining proof |
| --- | --- | --- |
| CF-02 | Local bytes, staged/unstaged binary edits, deletion, rename, mode, untracked/link material and type changes covered | Durable operation receipts/recovery |
| CF-01/03/15/16 | Current catalog, distinct evidence/approval identity, unchanged source history | Persisted successor, actual review, approval predicates and dispatch integration |
| CF-09/10/11 | Source witness refusal, no overwrite, no-follow local I/O, explicit unsupported Git forms, limits/deadlines | Fences, concurrent Git-path containment, crash/disk-full reconciliation |
| CF-13/14/18 | Compiler checks current headless policy, closed topology and derived static/dynamic reviewer strategy | Effective provider context/cwd/read-root and permission enforcement |
| CF-27 | Local preview is read-only and digest-bound | Delivery preflight, pagination and wire stale-preview contract |
| All other cases | Not implemented in this slice | I2/I3/I4 |

## Review Reconciliation

One fresh-context code reviewer inspected the I1 patch and rechecked fixes.
Eight findings were closed statically after RED/GREEN corrections: link chains,
destination ancestors, lazy fetch, split index, review routing, aggregate/deadline
bounds, sparse index semantics and staged deletion inventory. Follow-on regressions
cover file/directory replacement and expiry between receipt fsync and publication.

The ancestor finding is closed **only with quiescent external writers** over source,
destination and their ancestors. Path-based Git calls are not an atomic same-UID
sandbox. Deadline checks are cooperative, not cancellation of stuck OS I/O. A failed
operation may retain a receipt and must not be retried or activated by its presence.
These are explicit production integration requirements, not risk waivers.

## Next Slice

I2 owns additive storage, journalled effects, source/idea fences, bounded tracked
worker, abort/reconciliation and atomic successor activation. It must resolve Git
effect containment/ownership and apply consistent backup/migration gates before
production admission. I3 owns MCP/auth/readback, input origins/precedence and durable
approval enforcement. I4 owns separately authorized live acceptance.

No merge, push, daemon update or source cancellation was performed.

## Regression Status

Final I1 gate and alias: 37 passed each. Domain/workflow regression: 559 passed,
one baseline-confirmed prompt assertion. The broad diagnostic workspace run
finished with nine failing test targets; four individual auth/DB/workflow failures
were reproduced on unchanged HEAD. ACP environment and serial metrics reruns
passed, but remaining engine/MCP/parity/release failures were not fully classified.
This is not a green-workspace or merge-readiness verdict. See evidence for commands
and local logs. No failing production test was weakened or disabled for this slice.
