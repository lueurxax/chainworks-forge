# Headless Production Path Implementation Plan

> Execute continuously with superpowers:executing-plans and TDD. Independent
> coordinator, HTTP and canonical-gate workers have disjoint ownership. The
> user's September 20 instruction explicitly rejects intermediate stop points.

## Goal And Authority

Replace the daemon's IDE-dependent dispatch with the verified headless runtime,
including session preparation/reuse, authenticated broker reads, canonical build
gates, durable effects, operator controls and migration checks. Implement to the
approved parent R8 and runtime/wire/release R4; preserve prior evidence as history.
No implicit Apple consent, live unknown-effect retry, commit, push or deployment.
Implementation authorization does not substitute for exact project admission or
an operator-confirmed production cutover.

## Decisions

- Keep existing HTTP lease URLs and provider adapter capability negotiation.
  Replace the production backend and attachment authority, not the ACP protocol.
- Prepare a non-serializable invocation guard before `ensure_policy`; include its
  binding fingerprint in the session policy. Commit the exact invocation and
  generation before either new or reused provider prompts. Idle leases confer no
  active prompt authority.
- Keep the closed open/read Apple contract as the initial admitted surface.
  Filesystem editing remains the agent's existing permission-controlled route.
  Build/test goes through authenticated canonical test gates, not raw Apple
  execution tools or a newly invented MCP gate facade.
- One coordinator instance and journal authority cover lifecycle, broker reads
  and shim gates. Exact project trust is a separately persisted operator action;
  no path, policy label or provider assertion grants it.
- Cached live mapping observations may support revalidation, never persisted
  results alone. Revoked/uncertain mappings cannot be revived by reconnecting.
- An effectful gate is one journalled host script invocation. Its host subprocess
  runs without provider shim credentials, so nested build commands do not acquire
  conflicting authority. The gate itself remains canonical and remote UI rules
  remain enforced.
- Existing legacy resolver fixtures and historical observation DTOs may remain
  for readback/testing, but daemon production wiring has no IDE fallback.

## Work And Ownership

- [x] Coordinator worker: `acp/src/xcode_coordinator.rs`, matching integration
  tests. Kernel singleton, durable DB authority, bounded fair project permits,
  lifetime-bound nested owner access, conflicting alias/inode checks.
- [x] HTTP worker: `daemon/src/xcode_broker_http.rs` only. Unique-key bounded
  request decoding, closed JSON-RPC, static errors, coarse public health.
- [x] Gate worker: `acp/src/xcode_shim.rs`, `daemon/src/xcode_shim_socket.rs`,
  `scripts/test-gate.sh`, gate tests. Authenticated canonical script routing,
  server-bound invocation and stable operation key, parent authority hook.
- [x] Main: `acp/src/xcode_headless_runtime.rs`, controller/transport adaptation,
  manager/broker/engine integration. Opaque preparation and committed tickets,
  actual pre-policy integration, no alternate reuse shortcut or legacy fallback.
- [x] Main: engine journal factory/hold checks, exact project trust and privileged
  reconciliation, daemon authority/cutover wiring and CLI operator operations.
- [x] Main: cold service startup, capability coverage and installed identity
  diagnostics without automatic grants or replay.
- [x] Main: regression verification, independent integrated review, implemented
  reference docs and exact [completion evidence](../../evidence/xcode-headless-completion-2026-09-20.md).
- [ ] Installed-host acceptance and cutover: separately authorized combined
  provider run, packaged consent and cold-start evidence. Not performed by this
  implementation pass; not an unfinished production-code placeholder.

Implemented release support includes a separate closed receipt/evidence schema
and bounded validator. It validates independently supplied candidate identities,
required capabilities and journal counts; it neither gathers live observations
nor manufactures a release pass. The final offline record is distinct from a
release receipt and from an operator-authorized installed-host cutover.

## Test Sequence

Every implementation slice starts with a failing behavior check, then the
smallest production change and a focused green run. Fixtures use real SQLite,
kernel locks and Unix sockets, with fake Apple/provider boundaries. The driver
must prove: no open before durable fence; no read/effect without committed active
ownership; same-operation replay sends zero host effects; an uncertain outcome
blocks subsequent access; cancellation persists outcomes; project/service/policy
changes force reset; reuse retains its actual endpoint or resets the session.

Coordinator tests use disposable files and child lock contenders. Gate tests use
temporary canonical scripts and assert exact arguments, environment, output and
effect counts. HTTP tests invoke the in-process router with real lease checks.
Daemon tests assert production selection uses headless authority and cannot fall
back to an IDE target. Limited operators cannot inspect or reconcile attempts.
Historical attempts remain readable independent of current dispatch admission.

Run managed Cargo with `CHAINWORKS_AUTO_CACHE_CLEANUP=0` and the shared gate target
`xcode-headless-completion-20260920`; preserve normal sccache. Serialize Cargo
invocations among workers. Use locked/offline dependencies. Test logs belong to
`.superpowers/sdd/2026-09-20-xcode-headless-completion/`. Compare broader failures
with the already recorded exact-HEAD baseline instead of changing unrelated
contracts. No raw xcodebuild or local UI test execution.

## Completion

The final report must distinguish implementation, offline proof and live
acceptance, but it is not a stop permission after an arbitrary component. Continue
through every unchecked implementation item. Ask only for a concrete external
action that cannot be performed under the current authority, then continue all
unblocked work. Do not mark migration/release accepted while required workflow
capabilities or production transition evidence are missing.
