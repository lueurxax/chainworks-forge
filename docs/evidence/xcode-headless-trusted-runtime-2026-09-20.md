# Trusted Headless Runtime Checkpoint

Date: 2026-09-20. Base HEAD: `a096b8831f0db7f17d1ffe426851c49a65b8b000`.
Uncommitted implementation in the existing isolated `xcode-headless-i1` worktree.
This is a component checkpoint, not production admission, complete migration or
release acceptance. The original checkout and installed daemon remain unchanged.

## Approved Boundary

The operator explicitly accepted `trusted_local_project_v1`. The parent R8 and
runtime/wire/release R4 record that decision without rewriting historical I1
evidence. Apple is not claimed to confine adversarial projects to a checkout.
Project references and scripts may access outside it; external Apple clients and
changes between observations are not locked out. Per-project operator admission,
Apple consent, evaluated permissions, exact routing, uncertainty holds and the
prohibition on automatic resend remain required.

The immutable policy definition is part of the closed contract bundle. Its
domain-separated JCS digest is pinned by each manifest entry and included in the
controller target/binding identity. This identifies the policy; it is neither an
operator admission record nor an Apple permission grant.

## Implemented Components

- Native headless host inspection identifies the selected installation, UID,
  boot/process-start generation, service executable and tested build. It parses
  bounded structured service status without IDE windows, AppleScript or process
  command-line inference. Missing service requires action; automatic cold start
  is not implemented.
- Project selection pins effective-root and project filesystem identities.
  Relative selectors, including legacy `workspace:<relative>`, resolve within
  the effective checkout; absolute legacy paths are remapped without falling
  back to the original repository. Ambiguity and observed replacement fail.
- Domain-owned canonical encoding and closed open/read adapters reject duplicate
  JSON keys, unsafe numeric values, schema drift, implicit upstream selectors,
  malformed envelopes and unbounded results. The captured Apple tool schemas
  remain byte-preserved evidence, not invented APIs.
- The preparation controller uses the engine-injected durable journal before
  one workspace open. Unknown outcomes retain a hold. Repeated operation keys
  do not reopen or reconstruct authority from a stored success. The returned
  private binding exposes a local alias and inserts the exact upstream ID.
- A dedicated stdio bridge verifies initialization and discovered schemas, uses
  bounded framing and exact response correlation, and inherits the controller's
  deadline. It does not pool, reconnect or retry. Cancellation poisons the stream.

The controller is internal and deliberately not wired into production dispatch.
Its local serialization is not the specified cross-route coordinator. A string
trust-policy ID in its request is not production project-trust admission.

## Review Corrections

Independent runtime review identified five defects: sibling bindings surviving
observed closure; conflicting IDs surviving a failed post-open status check;
unbounded capacity waiters; a premature 60-second transport timeout; and rejected
relative legacy workspace selectors. Fixes revoke shared binding validity on
read failure/cancellation, revoke current-generation mappings on uncertain open,
bound total admission to eight drivers, inherit the invocation deadline, and
resolve legacy relative paths within the effective root.

The follow-up review found that queued cancellation could revoke shared state
outside the access lock. Shared revocation is now scoped inside that lock and
published before its release. Queued abandonment poisons only the queued binding;
cancellation during inspection or response waiting revokes shared continuity.
The sequenced regression failed before this correction and passes afterward.

Independent contract review identified absent immutable policy pins and a
JSON-Schema organization-path pattern that disagreed with the Rust adapter for
Unicode line separators. The bundle now includes the policy definition/pins and
separator-independent path checks with parity cases.

Revoked mappings remain unusable in this component; operator reconciliation and
fresh mapping-revision admission are future coordinator work. No cleanup or
reopen is used to manufacture certainty.

Final source re-review closed with no remaining actionable findings in scope:

- Runtime reviewer `01a0bd73-f958-7952-95b2-1a57c550a7bb`: native host/project,
  controller/journal usage and dedicated bridge; five original findings plus
  the follow-up revocation race resolved. Independently checked all 55 hashes
  in the source manifest, without duplicate entries, against the recorded HEAD.
- Contract reviewer `01a0bd75-a284-7513-b332-488781549b78`: closed adapters,
  canonical encoding, policy bundle and schema parity; both findings resolved.
  Requested documenting Node as a test prerequisite, recorded below.

These were independent source reviews, not independently repeated broad test
runs, a complete security audit or production acceptance.

A final main-implementer check found two more variants of retained old access:
lost acknowledgement after a committed dispatch fence and failed persistence of
a successful open. Both new regression tests failed because an old binding still
sent a read while the durable hold remained. Revocation guards are now armed
before dispatch acknowledgement and remain armed through successful settlement.
They revoke old/new mappings before releasing access on error or cancellation.
The runtime reviewer rechecked this narrow correction and found no actionable
issue; exactly two files changed from the earlier manifest, with 53 unchanged.

## Verification

Verification completed for this component. Local logs are under
`.superpowers/sdd/2026-09-20-xcode-headless-trusted-runtime/`.

Focused results: final controller 20/20, transport 9/9, host/project 13+8, complete
domain suite 360/360 including 19 contract tests. The ECMAScript path-schema
parity test requires installed Node.js; no package or dependency was fetched for
the review fixes. Tested Node version: `v26.3.1`, available on `PATH`. A missing
Node executable is an explicit test failure, not a skipped parity check.

The complete affected-crate run finished with 2,692 passed, 65 failed and three
ignored tests across 67 targets. Its complete failure-name set exactly matches
the prior exact-HEAD baseline: no unmatched or absent failures and no unexecuted
targets. It is not an all-green result. The existing failures were not repaired
or suppressed. Workspace compilation, workspace formatting and tracked diff
whitespace checks pass.

That broad run predates the final journal-acknowledgement correction. Its
[55-file source manifest](xcode-headless-trusted-runtime-source-2026-09-20.json)
is retained unchanged. Only the controller and its test file then changed;
the final full 20-case controller suite and workspace compilation passed on
the [final source manifest](xcode-headless-trusted-runtime-final-source-2026-09-20.json).
The broad suite was not rerun after this bounded internal-adapter correction.
Do not attribute its 18-case controller binary to the later 20-case source.
The [verification record](xcode-headless-trusted-runtime-verification-2026-09-20.json)
pins both source identities, counts, failure names and log hashes.

Behavioral RED was observed for controller dispatch/binding, shared revocation,
failed post-open status, bounded admission, relative legacy selection and the
60-second transport timeout. The domain adapter initially had only a missing-API
compile RED; its later three-failure mutation check is not retroactive TDD.

The first broad attempt is inconclusive: compiled artifacts disappeared while
tests were executing, leaving later targets unexecuted. Those targets are not
counted as behavioral failures. The final run disables supported automatic cache
cleanup for its managed commands, preserves normal sccache, and runs serially.
No global cache configuration or cache contents were manually changed.

## Native Observation

The new read-only inspector succeeded against Xcode `27A266a`, service build
`1.0/25317000000000000`. It saw four already-open projects and sent zero effects.
The Xcode IDE process was absent afterward. This proves native metadata/status
inspection only, not a new open/read, packaged permission identity or cold start.
No fixture was opened, no Apple grant was issued, and no service was restarted
by this slice. Earlier separately authorized native traces remain private and
are described in the journal-integration checkpoint.

## Remaining Migration

Production replacement still requires the per-UID database authority and common
coordinator, preparation before session-policy reuse, committed prompt tickets,
closed broker/shim dispatch through that same authority, explicit project-trust
admission, privileged diagnostics/reconciliation, legacy cutover, packaged Apple
identity/consent, cold start and workflow capability coverage. Canonical gates
and remote-only UI-test policy remain unchanged. No commit, merge, push, live DB
migration or deployment was performed.
