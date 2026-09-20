# Headless Offline Foundations Review

Reviewed source baseline: `a096b8831f0db7f17d1ffe426851c49a65b8b000` plus the
uncommitted offline-foundations patch in the isolated worktree.
Reviewer: `01a0bb09-d0a6-7550-8c75-74dc61120f0f` (Maxwell), read-only.
Scope: shared execution-root changes, standalone effect journal, migration,
tests and associated documentation. Previously reviewed I1 work was excluded.

## Verdict

No actionable blocking finding in the offline foundations patch. This is not
I2 admission, full-migration approval or proof of Apple filesystem confinement.
The reviewer inspected source and parent test logs without running additional
builds, tests, providers, Apple tools or services. No files were changed.

The review found no demonstrated defect in root-selection fallback/early
rejection, journal transaction ownership, dispatch/hold atomicity, revision
checks, uncertainty recovery, alias/path-replacement holds or historical schema
retention through the supported component APIs. Deferred production integration
was not treated as already implemented or as independently verified.

## Coverage Follow-Up

The reviewer suggested a nonblocking end-to-end regression for successful
read-only worktree sessions. The parent subsequently added
[`execution_root_runtime.rs`](../../control-plane/crates/acp/tests/execution_root_runtime.rs).
It uses the real ACP manager/transport and a synthetic local provider, with an
in-memory grant store, no shim socket and no Apple process. It verifies:

- Actual process cwd, `session/new` cwd and child shim-root environment agree.
- A second prompt reuses the same child without a second `session/new`.
- Resurrection launches a new child with the requested provider-session ID and
  the same worktree scope, then sends a successful prompt.
- Prompt activation occurs three times; closing each session removes its grant.

The added test passed. This closes the suggested coverage gap in a parent-run
fixture, not a second independent review or a live provider acceptance test.
It does not test changed-root reuse, committed headless tickets or confinement.

## Residual Scope

The production controller, coordinator, preparation/ticket wiring, admitted
facade, authorization and cutover remain unimplemented. The two DB and four
engine regression failures reproduce on clean HEAD; they were not skipped or
silently fixed. See the [checkpoint](xcode-headless-offline-2026-09-19.md) for
the final verification boundary and source manifest.
