# P088 Current-Head Closeout

Date: 2026-09-02

Recommendation: `promote_to_reference_owned`

## Evaluated Tree

- Base HEAD: `bfc94aa68ccf135ad397abd869765cd993a62ff1`
- Branch: `main`
- Worktree: intentionally dirty with the bounded closeout patch and unrelated
  pre-existing changes
- Evaluation environments: isolated worktree at `/private/tmp/cw-p088-closeout`,
  followed by the integrated main worktree

The first gate execution against the base tree exposed two compatibility
regressions rather than missing P088 product scope:

1. startup receipt recovery was disabled by a default scan bound of `0`;
2. two P088 tests attempted to claim newly enqueued work inside the indexed
   scheduler's current whole-second exclusion window.

The closeout patch restores the bounded default startup scan to 256 directory
entries, counts every inspected directory entry against that bound, and updates
the two tests to cross the scheduler cutoff before claiming work. No new runtime
feature or authority surface is introduced. It also repairs the default fast
selection so the already-tagged `CodexModelVariantTruthTests` suite is present in
both `FastGate.xctestplan` and the script's executable `FAST_TESTS` list.

The first integrated-main attempt reported five missing quota-reset repository
functions even though those functions were present in the dirty source tree.
The shared gate target had just compiled the clean isolated worktree and reused
stale `db` metadata because the dirty source files had older modification times.
Touching only those two source inputs forced the expected rebuild; no quota code
was changed, and every required gate then passed on the integrated main tree.

## Compared Sources

- historical P088 proposal and implementation audits R1 through R7;
- `docs/reference/output-contracts-failure-evidence-and-recovery.md`;
- `docs/reference/current-system-baseline.md`;
- `docs/reference/test-gates.md`;
- `control-plane/crates/domain/src/code_writer_completion.rs`;
- `control-plane/crates/db/src/repos/code_writer_completion_receipts.rs`;
- `control-plane/crates/engine/src/executor.rs`;
- `control-plane/crates/engine/src/recovery.rs`;
- GraphQL, MCP, report, and Swift implementation-completion readback surfaces;
- P088 fixtures under this evidence directory.

## Final Gate Results

| Command | Result | Notes |
| --- | --- | --- |
| `./scripts/test-gate.sh proposal-088` | Passed | Passed in the isolated proof tree and again on the integrated dirty main worktree after cache invalidation. |
| `./scripts/test-gate.sh p088` | Passed | Passed in both trees; the alias validates durable reference vocabulary without the retired proposal file. |
| `./scripts/test-gate.sh fast` | Passed | Passed in both trees. On integrated main, the macOS build, embedded Rust daemon build, and 111 tests in 12 suites passed, including `CodexModelVariantTruthTests`. |
| `./scripts/test-gate.sh guardrails` | Failed (optional) | Cache lifecycle, tag/test-plan synchronization, and proposal-number/roadmap integrity passed. Boundary coverage compares committed `origin/main...HEAD`; local main is 46 commits ahead with earlier in-scope boundary changes lacking a committed matrix/no-op marker. |

Xcode emitted a warning that local CoreSimulator `1171.2.0` is older than the
Xcode beta build's `1171.6.0` support. The gate targets macOS, continued to build
and test, and completed successfully; this warning is not a P088 blocker.

## Conformance Result

P088 behavior is implemented in the evaluated tree:

- original-prompt and post-prompt worktree fingerprints distinguish
  current-attempt implementation changes from inherited dirty work;
- eligible attempts receive at most one bounded same-session completion turn;
- stale, generated-only, and preexisting-only evidence fails closed;
- canonical receipts, prompt/runtime evidence, and per-output decisions persist
  with idempotent conflict detection;
- crash-partial receipt writes are recovered by a bounded default-on startup
  scan;
- stale-active canary and explicit targeted recovery preserve activation and
  retry-authority evidence;
- GraphQL, MCP, reports, and Swift readback use linked canonical receipt truth
  and preserve unknown future enum values.

The durable contract is now complete in
`docs/reference/output-contracts-failure-evidence-and-recovery.md`. The roadmap,
current-system baseline, test-gate reference, root README, and dependent proposal
links point to reference-owned truth. `proposal-088` and `p088` remain retained
historical test aliases; the proposal and audit files are retired from the
active documentation tree and remain available in Git history.

## External Status

- Latest published `origin/main` GitHub Actions run: passed for
  `20e2acbf33931151d6117cca2b4e3553a473f712` on 2026-08-30.
- Local `main` is 46 commits ahead of `origin/main` and zero commits behind.
- Current evaluated base and closeout patch: not published, so external CI has
  not evaluated this exact tree.
- App Store Connect: `unknown`; no current authenticated readback was available
  during this closeout.

Local verification and external release health are intentionally reported as
separate facts.

## Remaining Risks and Blockers

There is no remaining P088 behavior blocker in the evaluated tree. Residual
operational risks are:

- the integrated closeout patch remains uncommitted alongside the existing
  dirty main-worktree overlay and must be staged selectively;
- the exact integrated commit still needs normal external CI after publication;
- the optional aggregate `guardrails` command remains red until boundary
  coverage is evaluated against an updated published base or the earlier
  committed boundary changes receive their own truthful coverage evidence;
- shared Cargo gate targets can reuse stale cross-worktree metadata when a dirty
  source file predates an artifact compiled from another worktree;
- the local CoreSimulator/Xcode beta versions should be aligned before running
  simulator-only gates.

These risks do not require P088 to remain an active implementation proposal.
