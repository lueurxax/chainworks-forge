# Trusted Headless Runtime Implementation

> Execute with test-driven development and independent review. Continue the
> existing isolated worktree; no commit, installation, service restart or run
> retry is part of this plan.

## Decision And Scope

On 2026-09-20 the operator accepted trusted local projects. Apple is not a
checkout-only sandbox. Keep explicit project selection, permission checks,
closed tool contracts, durable dispatch fencing and no automatic effect replay.
The standing instruction is to continue the migration, not request this trust
decision again. The parent specification owns the revised production boundary.

This slice replaces the experimental host/contract assumptions with reusable
runtime code and a journal-driven workspace preparation controller. It does not
silently enable unfenced broker/shim calls. The installed daemon stays unchanged
until integration and release acceptance are complete.

## Tasks

- [x] Update the four active specifications to record the trust decision and
  distinguish observable drift checks from atomic external-client isolation.
- [x] Add exact native headless host inspection and pinned project selection.
  Test missing/ambiguous hosts and projects, wrong UID/install, PID reuse,
  worktree selection, path escape and filesystem replacement.
- [x] Add domain-owned JCS encoding and a closed open/read adapter contract.
  Test duplicate keys, numeric precision, schema drift, explicit selectors,
  malformed responses and output bounds; preserve raw captures as history.
- [x] Implement workspace preparation using the injected effect journal:
  resolve/check, initialize/verify contract, prepare, durable fence, one open,
  validate structured mapping and current host, persist outcome, then return a
  private binding. Lost response/cancellation/unknown result never causes replay.
- [x] Exercise the controller with real SQLite/writer admission and fake Apple
  transport. Count outbound effects under repeat, fence failure, outcome failure,
  cancellation and changed host/project. Keep live fixtures untouched.
- [x] Add a dedicated installed-bridge transport with bounded framing, exact
  correlation, pinned initialization/schema discovery and no automatic retries.
- [x] Run affected tests and independent review; record exact source/evidence
  and remaining broker/session/coordinator/shim/release integration work.

Controller RED observed on September 20: 11 tests compiled, 2 rejection checks
passed and 9 behavior checks failed against the explicit unimplemented stub.
Failures were missing journal transitions/dispatch/binding and waiting for an
effect the stub never sent. This is a local disposable SQLite/fake-peer run,
not an Apple operation. Native transport framing tests were added separately.

Focused controller verification is now 13/13 after an additional behavioral RED
for a same-generation ID reused across two projects. The controller rejects the
second mapping, retains its uncertainty hold and revokes the prior binding.
Host/project verification is 13 unit + 7 integration tests. The domain suite is
357/357, including 16 contract tests. Its initial RED was a missing-module
compile check, not behavioral RED; a later mutation check proved three key
rejection tests and is not presented as retroactive TDD.

The new read-only native inspector executed successfully against Xcode 27A266a:
service build `1.0/25317000000000000`, four existing open projects, zero effects.
The IDE process was absent after this probe. This does not prove a new native
open/read, native permission identity or full migration acceptance.

Review corrections now have focused GREEN: controller 18/18, transport 9/9,
host/project 13+8, and domain 360/360 including 19 contract tests. Additional
behavioral RED covered shared mapping revocation, a failed post-open status,
bounded admission, relative legacy selection, the premature 60-second deadline,
and schema/policy drift. Cancellation coverage distinguishes queued local
abandonment from shared continuity loss during inspection or response waiting.
The broad run completed with 2,692 passed, 65 exact-baseline failures and three
ignored tests. An earlier attempt lost compiled artifacts mid-run and is retained
as inconclusive, not as a behavioral regression result.

After broad compilation, two additional behavioral RED tests showed that lost
dispatch acknowledgement or failed outcome persistence left old bindings usable.
Revocation now covers that entire interval under the access lock. Final controller
20/20 and workspace compilation/formatting pass; the runtime reviewer rechecked
the two changed files. The broad suite was not rerun after this bounded fix.
Both source versions and exact verification scope are retained in the
[checkpoint](../../evidence/xcode-headless-trusted-runtime-2026-09-20.md).

## Ownership

Host/project worker owns only `acp/src/xcode_headless_host.rs` and its integration
tests. Contract worker owns `domain/src/xcode_contract.rs`, its tests, export,
dependency/lock additions and `contracts/xcode-headless/v1/`. Documentation worker
owns the four active parent/runtime/wire/release specifications. Main implementer
owns this plan, ACP controller/exports and engine integration tests. No shared
source-file edits between workers.

## Verification

Use `scripts/cargo-managed`, the existing isolated journal cache, and locked
offline tests after the intentional narrow canonicalizer dependency fetch.
Set `CHAINWORKS_AUTO_CACHE_CLEANUP=0` for these verification commands and do not
start another managed invocation during a broad run. Keep normal sccache enabled.
The schema/runtime parity regression requires installed Node.js to exercise
actual ECMAScript regular-expression semantics; no package fetch is needed.
Focused tests must demonstrate RED before implementation and GREEN afterward.
Broader failures must be compared with the already recorded exact-HEAD baseline,
not fixed by relaxing unrelated contracts. Native status inspection is read-only;
it is not evidence of packaged consent or complete production migration.

## Next Integration Boundary

The controller must subsequently be connected before engine session-policy
evaluation and before both new and reused prompts. One runtime coordinator and
per-UID journal authority must cover broker, lifecycle and shim operations.
Only then replace the daemon's IDE resolver and raw forwarding route. The final
release still requires closed-IDE broker acceptance, packaged identity/consent,
cold start, canonical gates and workflow capability coverage.
