# Headless I1 Offline Implementation Evidence

2026-09-19. H1: **not_run**. Production admission: **unchanged**.
Offline verification passed after one independent whole-change review and one
fix pass. All five findings were addressed; fixes were verified by the executor,
not a second reviewer. Task 6 has not started.
This is not live acceptance, full-migration readiness, or a confinement proof.

## Scope And Provenance

Implementation is a private, explicitly invoked Cargo example:
`control-plane/crates/acp/examples/xcode_headless_i1.rs`, its private modules,
three synthetic/captured fixtures, and example registration in ACP Cargo.toml.
No ACP library export, engine/daemon route, production broker/shim, DB/auth,
Swift source, catalog, service configuration or permission change was made.

Baseline HEAD: `a096b8831f0db7f17d1ffe426851c49a65b8b000`.
Changes are uncommitted in the isolated `xcode-headless-i1` worktree, not the
original checkout. HEAD alone does not identify these build inputs.
The [machine-readable manifest](xcode-headless-i1-offline-2026-09-19.json)
pins all 573 build-input files and the final test-log checksums.
The reviewed parent/I1 specifications are unchanged:

| Input | SHA-256 |
| --- | --- |
| Parent R3 | `7657816a074c7278566325a9286bf52d1f7801b18c9c3eeef8ded8f75cd7115c` |
| I1 R3 | `509ca0ba4d2f97e5b71505092537f15cf1ab67d9af29962371feab59523c4045` |
| 2025-06-18 capture | `e00d534d7dbbc705ecc2211da80759281d8f42f423c408f67529a535723ab020` |
| 2024-11-05 capture | `8f6c31fbccae1cac96e10f6018724bf57534b4cbae7226fcc9c7e0d5302981a0` |

Both captures were parsed, hashed and compared structurally before mechanically
extracting the two tool definitions. These remain metadata-only captures, not
successful open/read evidence.

## Verification

All Cargo selections use `scripts/cargo-managed`, `--locked --offline`.
No live smoke environment variable was enabled.

| Selection | Result |
| --- | --- |
| `test -p acp --example xcode_headless_i1` | 33 passed |
| `test -p acp --lib --tests -- --test-threads=1` with `CHAINWORKS_JUNIE_ACP_LIVE_SMOKE` unset | 347 passed, 1 ignored; includes the 33 example tests |
| `check -p acp --example xcode_headless_i1` | exit 0 |
| `fmt --all -- --check` | exit 0 |
| `git diff --check` | exit 0 |
| Production source search for `xcode_headless_i1` | no matches in ACP/engine/daemon source |

Initial untouched concurrent ACP baseline failed one environment-sensitive
PATH/sccache test (195 passed, 1 failed). Its serial rerun passed all 314 existing
tests, with one existing ignored test. No production test was changed or skipped.
Normal synthetic Python process startup uses a 5-second budget after a 500-ms
startup assumption failed; the intentional request-timeout scenario remains
short and does not change the live request budget.

## Coverage And Limits

| Case | Offline evidence | Live status |
| --- | --- | --- |
| I1-01 | Root strategy, required worktree, ambiguity, symlinks, exact fixed fixtures, overwrite refusal | offline case |
| I1-02 | Synthetic UID/path/start/build/IDE/status decisions | not_run |
| I1-03 | Intent before two opens; actual fake process pipes; no replay | not_run |
| I1-04 | Optional mapping unverified; conflicting mapping rejected | not_run |
| I1-05 | Explicit IDs, fixed range, A/B/A sentinel and metadata validation | not_run |
| I1-06 | Two owned fake bridge processes, two opens total and six reads | not_run |
| I1-07 | Injected intent/result failure, lost/malformed response, cancellation, generation change, transport limits and nonzero exit | offline fault fixtures only |
| I1-08 | Closed CLI, source/fixture/approval comparisons, private records, public redaction, no production wiring | real host post-state not_run |

The fake process never contacts Apple or opens a project. Additional review
tests write actual local intent/result files and inject EIO immediately before
file/directory sync. They assert zero opens on intent failure and exactly one
open on result-sync failure, with retained uncertainty. A real owned fake process
receives exactly 13 bytes in the partial-write case. Lost/malformed-response and
cancelled-request cases retain the intent and verify owned-child reaping.
These do not simulate a power failure of the host filesystem. Runtime reports
leave offline cases NotRun until closeout composes this separate test evidence.

## Independent Review Disposition

| Finding | Fix and verification |
| --- | --- |
| Queued invalidation processed after dispatch | Drain already-queued messages before writing and during close. Response-then-notification regression RED -> GREEN; no open bytes reach the fake peer. Ordinary EOF remains a recorded process exit, not an RPC failure. |
| Rejected result evidence discarded | Retain bounded types of known fields and payload hashes; persist sanitized mapping deviations. Decoder RED -> GREEN and durable mapping regression pass. No foreign strings or arbitrary field names retained. |
| Ending host not observed but marked Pass | Failed/unclean attempts mark I1-02 Unverified; happy-path final observation remains required. Failure-path assertion RED -> GREEN. |
| Missing transport/storage boundary coverage | File/dir-sync EIO and partial-write tests RED -> GREEN; additional real owned-process cancellation/reap and malformed/lost-response tests pass. |
| Missing evidence parent on first prepare | Create the requested evidence hierarchy with symlink checks. First-use regression RED -> GREEN; treated as Important because the documented command failed. |

Reviewer: `01a0baa8-daf8-77e3-bb47-3d5138e6c910`. Read-only review; no
independent test rerun, live host probe, source edit or secondary delegation.

## Decisions And Next Gate

- No commits/merges/publication; keep the execution ledger while work is uncommitted.
- Managed shared-cache tests required approved host execution; no live lanes enabled.
- Existing environment-sensitive ACP regression runs serially without changing it.
- CLI status field names are conservative, unobserved adapter assumptions. Missing
  keys block inspection; no prose or workspace inventory supplies authority.
- Offline and live results are composed explicitly, never inferred from each other.
- Apple status/consent/project acceptance/reconnect still require authorized live
  observation. Production confinement/leases/packaged identity are outside I1.
- Hostile same-user filesystem tampering is outside the experimental trust model.
  Local interlocks are not an adversarial security boundary.
- Sync-boundary fault injection is not power-loss proof; the passing regression
  was observed by the executor, not independently rerun by the reviewer.
- Pending-message draining is not an atomic service-side fence. A race after the
  local observation remains a later authority question, not a confinement claim.

The next gate is preparation and read-only inspection of an actual fresh
fixture pair. Opening those fixtures
still requires separate approval of the exact paths and launch identity.
Task 6, I2-I4, production isolation, and all future-stage review findings remain
open. No OS permission administration, IDE launch, live open, service restart,
build or test through Apple MCP has been performed.
