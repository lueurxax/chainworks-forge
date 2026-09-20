# Headless Xcode Integration Completion

Date: 2026-09-20. Base HEAD: `a096b8831f0db7f17d1ffe426851c49a65b8b000`.
The candidate is uncommitted in the existing isolated `xcode-headless-i1`
worktree. This records implementation and verification, not a deployed release.
Implementation and local verification are complete for the admitted trusted-local
scope. The workspace is not all-green; baseline failures and isolated follow-ups
are recorded below rather than hidden or treated as release acceptance.

## Implemented Route

The daemon no longer constructs the IDE-dependent backend. Its admitted path
uses the native headless service, a per-UID durable database authority and one
workspace coordinator shared by preparation, broker reads and canonical gates.
Missing authority or a disabled broker cannot fall back to IDE dispatch.

The engine pins the effective root/project and evaluates frozen capabilities
plus separately persisted project trust before session-policy evaluation. It
commits the exact invocation/generation before exposing provider access. Active
prompt ownership ends on success, error or cancellation; reused endpoints must
match a revalidated binding and their real runtime scope.

Provider MCP exposes closed `XcodeRead`; lifecycle workspace opening is internal.
Build/test goes through the authenticated canonical-gate shim with server-owned
arguments, root, environment and invocation authority. Raw Apple execution tools
are not admitted. UI tests remain remote-only. Project organization and scripts
are trusted local inputs, not an Apple-enforced checkout sandbox.

Opening, optional one-shot cold startup and gates are fenced by the durable
journal before effects. Unknown outcomes retain project holds. Neither repeat
operation keys nor new keys under a hold resend an operation. Gate settlement
requires process-group completion. Read/revalidation calls are capped at 60
seconds, preparation at 720 seconds, and invocation lifetime at 24 hours without
renewal from nested calls.

The local operator CLI supports explicit authority bootstrap, project trust,
private historical diagnostics and revision-checked reconciliation. Global
project trust requires global administration; run-limited diagnostics and
reconciliation do not imply it. Explicit v3 global administration uses a
non-default marker plus each specific capability. Old result schemas are read
independently of the current admitted dispatch manifest.

A separate release receipt/evidence schema and pure bounded validator check
source/artifact/contract identity, required capabilities, consent provenance and
unresolved attempt counts. Synthetic validator fixtures are not live evidence.
The validator consumes independently trusted candidate and collector inputs; it
cannot attest host observations or grant release authority itself.

## Review Corrections

Independent source review drove the following corrections and regression tests:

- Mapping revalidation revokes shared continuity after observed closure and
  cancellation; a subsequent same-generation reopen cannot revive old access.
- Cancellation subscriptions inspect their current state before admitting a
  gate, eliminating the subscribe-after-cancel race.
- Preparation, invocation and individual read deadlines are separate.
- Ordinary missing-authority startup does not create a partial bootstrap that
  prevents a later explicit setup.
- Cold startup retains the exact installation across launch and checks it before
  bridge connection/open; readiness needs safe structured status, not just a PID.
- The Xcode disable switch is checked before preparation and every provider
  dispatch path, including required shim attachment.
- Explicit v3 administrators can bootstrap, while run-scoped operators cannot
  grant or revoke shared project trust, even with a global marker in their set.
- Reused runtime IDs must be one-to-one; public errors are typed and sanitized.

Runtime/HTTP/admin source review: Peirce and Ohm. Their source checks are distinct
from the parent-run test executions. Release validation has a separate finite
source review with no remaining actionable P1/P2 findings within its reviewed
trusted-collector boundary. All listed review corrections are implemented and
their regressions pass.

## Verification

The managed test runner retains start/end times, exit codes, logs and source
hashes under `.superpowers/sdd/2026-09-20-xcode-headless-completion/`. Cargo uses
locked/offline dependencies, the shared gate cache and no automatic cache
cleanup. Live Junie smoke is disabled. No raw local Xcode tests or UI tests run.

The final record links the source manifest and each log digest. Earlier I1,
foundation, journal and trusted-runtime checkpoints remain historical and have
not been relabelled as evidence for this combined implementation.

| Verification | Result |
| --- | --- |
| Complete Rust workspace compile check | Pass |
| Controller/preparation, journal adapter, recovery | 30 + 16 + 6 passed |
| Xcode operator auth, engine admin, daemon Xcode paths | 6 + 4 + 36 passed |
| Canonical gate/shim/socket suite | 44 passed |
| Host startup/readiness, project trust, catalog | 8 + 11 + 10 passed |
| Workspace coordinator | 27 passed in broad run |
| Release evidence validator | 12 passed in broad run |
| Final execution-root/session contract | 2 passed |
| Broad ACP/daemon/DB/domain/engine run | 2810 passed, 68 failed, 3 ignored |
| Adjacent auth/workflow/MCP run | 711 passed, 15 failed, 1 ignored |
| Formatting and tracked diff whitespace | Pass |

Counts overlap where a focused suite is also part of a broad run; they must not
be summed as unique tests. The broad run returned 101, not a successful gate:

- All 65 failures from the previously verified exact-HEAD baseline recur.
- The older `readonly_worktree_agrees_across_process_session_shim_reuse_and_resurrection`
  fixture expected shim prompt reuse that the new route deliberately prohibits.
  The final two tests retain ordinary non-shim reuse/resurrection and prove fresh
  shim sessions, stable roots, revoked grants and denial before an extra prompt.
  They pass. Production behavior was not weakened to satisfy the old expectation.
- `metrics::tests::proposal_058_required_metric_names_are_declared` failed its
  global-counter assertion in parallel, then passed alone. Its source is identical
  to HEAD. Concurrent global resets are a plausible cause, not a verified fix.
- `p091_startup_orphan_repair_terminalizes_stale_active_authority` observed Pending
  instead of Skipped in parallel, then passed alone. That test changes global
  repair-mode environment variables. The cause is not established by the rerun.
  Neither isolated pass makes the original broad run green.

All 15 adjacent failures were independently reproduced in the preserved exact-HEAD
archive `/private/tmp/cw-headless-baseline-TXiXVF`: one auth bootstrap schema test,
12 report-shape tests, one steward test and one existing Git-prompt catalog test.
All 1,810 tracked archive blobs matched HEAD before and after these checks.
No unrelated baseline test or production contract was changed.

Only the execution-root test file changed while the broad run was in progress;
its old compiled result and the final two-test rerun are retained separately.
Production sources did not change during that run. Subsequent focused/gate runs
record their exact before/after source hashes.

The final native metadata-only observation succeeded: headless service build
`1.0/25317000000000000`, Xcode build `27A266a`, four already-open projects and zero
effects sent. It proves current service inspection, not a combined provider run,
cold launch, project-open consent or checkout isolation. The final process probe
at 10:03:24 UTC found no exact `Xcode` IDE process (exit 1, zero matches).

Machine-readable records:
[source identity](xcode-headless-completion-source-2026-09-20.json) and
[verification, baseline comparisons and native observation](xcode-headless-completion-verification-2026-09-20.json).

## Deployment Boundary

No installed daemon restart, live database migration, project trust/Apple grant,
new live project open, workflow retry, commit, merge or push was performed.
The original checkout remains separate from this candidate. Xcode IDE was absent
when checked before the final read-only observation lane.

Actual combined broker/provider acceptance, a controlled native cold start and
packaged launch/consent still require the authorized installed-host lane. They
are not implementation stubs and are not asserted to pass from fixtures. No
release receipt or full-migration release pass is fabricated from these results.

See [runtime and operator guide](../reference/xcode-headless-runtime.md),
[broker reference](../reference/xcode-mcp-bridge-pool.md) and the
[completion plan](../superpowers/plans/2026-09-20-xcode-headless-completion.md).
