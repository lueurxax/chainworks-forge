# Headless Journal Integration And Native Compatibility Check

2026-09-20. Isolated worktree, HEAD
`a096b8831f0db7f17d1ffe426851c49a65b8b000`, plus uncommitted changes.
This continues the [September 19 checkpoint](xcode-headless-offline-2026-09-19.md).
It does not replace the installed daemon or certify the full migration.

## Implemented Dependency

The [journal integration plan](../superpowers/plans/2026-09-20-xcode-headless-journal-integration.md)
connects the existing durable repository to the engine's shared writer:

- ACP owns the IO-neutral `XcodeEffectJournal` boundary; engine supplies a fixed
  run/owner adapter. The same owner string in another run cannot read or mutate
  the original attempt. ACP acquires no DB dependency.
- All mutations use registered Class A barriers and the injected writer. There
  is no raw-pool fallback. A repeated nonce returns the historical/in-flight
  attempt, not a second dispatch authorization.
- Only completion and cancellation are admitted during writer drain. New
  preparation, dispatch and startup recovery are denied.
- Daemon startup awaits bounded restart recovery before executor/request
  admission. It is deliberately separate from the live `StartupRepair` work
  item: live repair must not relabel an operation that may still be executing.
- Recovery processes at most 100 revisions per transaction, preserves project
  holds and stops on error. Its bookkeeping key hashes the ordered revision
  batch to fit the existing writer's 1,024-byte limit. This is not an authority
  or canonical-wire digest.

These adapters are not yet connected to Apple dispatch. Stored outcome/proof
assertions still require validation by the future trusted operation adapter.
The per-UID authority and common broker/shim coordinator are not implemented.

## Native Compatibility Observation

This was a small, separately authorized native Codex MCP check against the
already-open successful I1 fixtures under
`/private/tmp/cw-i1-f39670ec-af8f-4032-bda7-73f04d419f54`, A and B only.
No new project, build, test, preview, script, provider run or production DB
operation was requested.

1. Initial A read was denied because the signed Codex agent lacked Apple consent.
2. One open request on that already-open A project returned a pending-approval
   error. It was not repeated, and that response is not recorded as a success.
3. The operator approved Codex in Apple's MCP menu-bar UI. Subsequent status
   reported no pending agents, Codex approved and unsafe allow-all disabled.
4. Reads using either the canonical `/private/tmp` project path or the service's
   `/tmp` spelling as `workspaceIdentifier` returned unknown-identifier errors.
5. `XcodeListWorkspaces` returned a human-readable message, not structured mapping
   objects. Explicit IDs from that native diagnostic listing returned the correct
   `CW_I1_A` and `CW_I1_B` sentinels.

This refutes the path-selector assumption for the exercised native connector
path despite the advertised schema accepting an absolute path. It does not
attribute the rejection independently to Apple's underlying service, and it
does not make prose parsing an admitted production binding mechanism.
No reconnect was tested today. All eight original fixture seed hashes still
match. The IDE process was not observed before or after the probe; no command
launched it. Historical I1 attempt/report files were not modified.

The complete seven native tool calls' arguments and returned results are kept
privately in `/private/tmp/cw-headless-20260920.IgV3gq/native-tool-trace.json`
(directory 0700, file 0600), SHA-256
`e7e28d28ba6de2e156ab88852ddd652c9544935c897ebff369a6accb5486a565`.
This connector does not expose raw JSON-RPC envelopes/headers; the trace does
not fabricate them. No raw trace is committed or published.

## Verification

Focused checks completed before the full suite:

| Check | Result |
| --- | --- |
| Fixed-scope engine journal adapter | 14 passed |
| DB journal, including bounded candidate inventory | 28 passed |
| Startup recovery, including 101 rows, closed writer, later-batch failure and live repair exclusion | 6 passed |
| DB writer tests, including settlement-only drain | 24 passed |
| Operation registry tests | 14 passed |

Missing-API and behavior RED logs are retained. The 101-row regression exposed
the oversized bookkeeping key before the digest correction. A static SQLite
trigger verifies rollback of the whole batch when its second row fails. The
test checks the injected error specifically, not merely any error.
After independent review, two additional tests passed: failure in batch two
preserves the first 100 committed transitions and all holds, and a later startup
pass processes only the remaining row; live repair leaves dispatched attempts
unchanged. The complete adapter/recovery rerun passed 20 tests.

The complete affected-crate run (ACP, domain, DB, engine and daemon) finished:
2,623 passed, 65 failed, three existing tests ignored. The run used serial test
execution, managed isolated caches, locked/offline dependencies and an unset
`CHAINWORKS_JUNIE_ACP_LIVE_SMOKE` switch. This is not an all-green suite. The two review-added
tests were verified in the later focused run, not the earlier broad compilation.

| Failing target | Current result | Exact-HEAD baseline |
| --- | --- | --- |
| DB library | 442 passed, 2 failed | Same two failures reproduced September 19; new writer test passes |
| Engine library | 568 passed, 4 failed | Same counts and names reproduced September 19 |
| Engine integration | 165 passed, 48 failed | Same counts and complete failure-name set reproduced September 20 |
| Engine P041 parity | 18 passed, 3 failed | Same counts and complete failure-name set reproduced September 20 |
| Engine release | 4 passed, 8 failed | Same counts and complete failure-name set reproduced September 20 |

Before the new baseline runs, all 1,810 tracked archive entries matched the
source HEAD's Git blob hashes. The baseline runs used a separate managed target,
not a revert or shared-target overwrite. All 59 newly observed integration,
parity and release failures reproduced there. The six earlier library failures
are retained in the [September 19 verification](xcode-headless-offline-verification-2026-09-19.json).
No failure newly attributable to this patch was observed; unrelated baseline
failures were not repaired or suppressed. Typical observed baseline failures
include incomplete frozen snapshot fixtures, missing artifact source claims,
queue eligibility expectations and missing model-policy fixture files.

Managed workspace check, workspace formatting and tracked diff whitespace
checks pass. Existing warning messages were not rewritten. The
[independent component review](xcode-headless-journal-review-2026-09-20.md)
found no actionable correctness defect and records two remaining combined
lifecycle test gaps. No full-migration or merge-readiness claim follows.

The [41-file source manifest](xcode-headless-journal-source-2026-09-20.json) and
[verification record](xcode-headless-journal-verification-2026-09-20.json) pin the
uncommitted inputs, result counts, failure names and local log hashes. Logs remain
under `.superpowers/sdd/2026-09-20-xcode-headless-journal-integration/`.
The installed Chainworks daemon, its production DB and authorization policy,
and the original checkout were not modified. The operator's separate Apple
Codex consent is recorded above. No deployment, commit, merge or push was performed.

## Admission Decision Still Open

Apple's [official session](https://developer.apple.com/videos/play/wwdc2026/8007/)
describes agent/folder permissions and access to workspace references. Those
permissions and successful A/B reads do not establish checkout-only execution
confinement, atomic ID/path validation or permanently non-reusable IDs.

The operator has been asked whether to adopt an explicit trusted-local-project
model or retain the strict production isolation contract. No answer has been
inferred from Apple UI consent. The existing runtime/wire/release requirements
therefore remain unchanged, and no production capability has been admitted.

Full replacement still needs a production controller, shared coordinator and
per-UID journal authority, preparation before session reuse, committed runtime
tickets, a closed tool facade, privileged diagnostics/reconciliation, legacy
cutover and packaged live acceptance. A useful working migration cannot be
claimed merely by denying all required operations.
