# Headless Runtime And Effect Contract

Revision 4 (R4), 2026-09-20. Normative production-target child of the
[headless design](2026-09-19-xcode-headless-mcp-design.md).
R4 records the parent's R8 approval of `trusted_local_project_v1` in this
prospective contract, not a claim of completed implementation or live acceptance.
Owns review items P1-01, P1-02, P1-03 and the dispatch part of P1-05.

## Iteration Applicability

This contract applies to production admission in I2-I4. It does not require the
full coordinator, DB journal or verified-binding types before implementing
[I1's isolated open/read experiment](2026-09-19-xcode-headless-iteration-1.md).
I1 uses only an experimental observation, a restricted fixture harness and a
local open-intent/outcome record with no automatic replay or production grants.

Before any production provider lease or lifecycle open, satisfy the applicable
rules below, including coordinated durable lifecycle effects even if that work
must move ahead of the broader I3 capability expansion. The observed I1 results
inform review of these contracts; they do not turn an experimental observation
into a verified ticket. The user explicitly approved trusted-project admission
on September 20 instead of adversarial-checkout confinement. Types and ownership
below remain design intent until implemented and verified; historical I1
contracts/evidence are not rewritten. Live host operations are not authorized.

## Types And Authority

Shared value types belong to `domain::xcode_runtime`. They contain no IO,
credentials, process handles or dependency on ACP. Runtime guards belong to ACP.

| Type | Required fields and invariants |
| --- | --- |
| `ResolvedExecutionRoot` | version=1, original repository canonical path, effective canonical path, root device/inode, frozen strategy, root kind (`repository` or `worktree`); constructed once by engine |
| `CanonicalProjectKey` | version=1, UID and canonical project path slot, with pinned project device/inode; not run ID, execution root, PID, developer directory or display name |
| `XcodeServiceGeneration` | version=1, UID, host boot identity, PID, process-start identity, canonical developer directory, executable identity, bundle/build identity; missing facts reject construction |
| `VerifiedWorkspaceBinding` | version=1, resolved execution root, project key, service generation, upstream opaque workspace ID, initial structured open ID/path evidence, mapping revision, operator project-trust admission reference, policy digest, manifest digest, `trust_policy_id=trusted_local_project_v1`, `trust_policy_digest`, scheme/destination context when applicable |
| `PreparedXcodeExecution` | opaque, non-cloneable ACP guard owning a preparation ID, binding, provisional lease/backend references, coordinator permit and expiry |
| `CommittedXcodeTicket` | opaque ACP ticket linking preparation, invocation, session generation, binding digest and active prompt ownership |

Constructors validate canonical paths and their filesystem identity. A caller
cannot manufacture a verified binding from deserialized observations. The engine
passes the same `ResolvedExecutionRoot` to provider cwd, MCP and shim. Registry
hints select a project within it; they cannot replace root or permission authority.
Operator trust must explicitly cover that local root/project and UID. Provider
assertions, legacy catalog flags and possession of a path do not create trust.
The wire bundle owns the versioned trust policy and its canonical digest; the
digest identifies the policy, not an Apple sandbox proof or a permission grant.

The binding digest excludes transient preparation ID, bearer, timestamps, lease
ID, backend PID and observation counters. It includes the service generation,
upstream workspace ID, initial mapping and revision, project/root identities,
operator trust admission
and actual permission/tool/manifest/trust-policy digests. Stable revalidation can
therefore reuse a session without ignoring observable meaningful changes.
Mapping revision changes on observed close, remap or lost continuity evidence;
it does not claim to count unobserved external workspace changes.

## Prepare Before Reuse

The existing `ensure_policy` decision must move after Xcode preparation. Every
prompt, including a reused provider session, follows the same sequence:

| Step | Owner | Result / failure rule |
| --- | --- | --- |
| Resolve | Engine | Produce execution root, evaluated permissions/tool allowlist and invocation owner; require explicit matching operator project trust; do not yet choose reusable session |
| Prepare | ACP runtime controller | Acquire coordinator access, verify pinned host generation, bind exact workspace from structured open ID/path, check fresh exact open-path status and scope, obtain project-ready guard |
| Decide | Engine session policy | Include prepared binding digest in fingerprint, then decide reuse/reset using existing durable generation policy |
| Commit | ACP manager | Compare current generation/binding with preparation; install or adopt matching HTTP leases and shim grant atomically in live manager state |
| Admit | ACP manager | Revalidate current authority, then permit `session/new` or `session/prompt`; a reused session has no alternate shortcut |

An invocation without Xcode retains its existing non-Xcode path. For an Xcode
invocation, absence of a valid ticket at `start_session` or `prompt_session` is
an error, not a request to attach later.

Preparation may borrow references to an existing lineage's lease only after
revalidation. Reuse is allowed only when that live session's actual endpoint,
ticket, scope and digest match. A fresh endpoint cannot silently replace a lease
inside an already initialized provider session; mismatch forces session reset.

Commit compares the expected engine session generation and preparation epoch.
No project/permission validation is deferred until after the first prompt.
If authority changes after the policy decision, invalidate that generation and
abort. Do not fall back to a stale session or repeatedly re-prepare in one prompt.

Prepare does not publish an active lease or make a provider grant usable. A
cancelled/expired prepare releases only its own provisional references. The
manager owns a bounded cleanup queue for asynchronous teardown; guard drop
signals that owner instead of attempting asynchronous work in `Drop`.

Crash between the engine DB decision and live commit leaves a generation with
no usable ticket. Recovery invalidates it through the existing missing-live-
handle path. Crash after commit still loses all in-memory tickets; persisted
observations cannot authorize reconstruction. Prepared bridge cleanup never
closes a borrowed workspace or stops the shared Xcode service.

## One Workspace Access Owner

`WorkspaceAccessCoordinator` is an ACP service created once in daemon wiring
and injected as the same `Arc` into the broker, shim dispatcher and preparation
controller. Neither broker nor shim maintains an independent project lock.
The coordinator is the only constructor of read and mutate permits.

Per-database singleton locking in `daemon::supervisor` is insufficient: two
daemons with different SQLite files can target the same Xcode project. Headless
admission therefore also requires a per-UID coordinator singleton:

- Use a fixed host-user authority directory under
  `~/Library/Application Support/Chainworks Forge/xcode-runtime/`, independent
  of daemon port, selected Xcode installation and database configuration.
- Open a UID-owned, non-symlink `coordinator.lock` with restrictive permissions
  and hold an exclusive kernel lock for the coordinator lifetime. Never unlink
  the lock file on release. A second coordinator cannot admit MCP or shim work.
- Under that lock, a durable authority record pins the canonical database
  identity that owns the operation journal. Different-DB admission fails with
  `authority_mismatch`; switching DB is explicit operator migration, never a
  way to lose unresolved effects. Missing/corrupt authority after prior setup
  fails closed. Bootstrap writes the authority before admitting any effect.
- An in-memory DB or alternate injected lock root is allowed only for fixtures
  with fake host/process transports. It cannot enable live Xcode dispatch.

This is cooperation among Chainworks processes, not a lock on third-party
Apple clients. External clients are not assumed to respect this lock and are
not sandboxed. Their observable drift invalidates admission; unobserved changes
between checks remain an explicitly accepted trusted-project risk.

Permit key is `CanonicalProjectKey`. Lookup also tracks pinned file identity
across aliases/rename; observed path replacement or a second root must not let
cooperating clients bypass a held project. This is not protection against
malicious filesystem races. Root identity belongs to binding validation, not a
separate lock key.
Owner is an unforgeable runtime identity
for the invocation and session lineage. A shim grant is explicitly joined to
that owner and project at manager commit; a provider-supplied string is not
reentrancy proof. Nested gate/shim commands borrow the owner's permit instead
of acquiring a second, conflicting permit.

| Permit rule | Contract |
| --- | --- |
| Read | Concurrent only with other reads and no unresolved project effect |
| Mutate | Exclusive across MCP, shim and internal project lifecycle operations |
| Upgrade | No in-place read-to-mutate upgrade; prepare a new authorized invocation |
| Lifetime | Active prepare/prompt and outstanding operations, not an idle cached session |
| Fairness | FIFO queue; after a writer queues, later unrelated readers cannot pass it |
| Bounds | Existing configured queue timeout/capacity; expired owner/permit cannot be renewed by a late request |
| Reentrancy | Existing owner's already-authorized nested operation only; cannot extend prompt deadline or effect class |

The maximum hold follows the existing invocation/watchdog deadline. Provider
disconnect/cancellation/revocation stops new dispatch immediately. Release a
permit only after in-flight work is known finished or a durable uncertainty
hold has replaced it. Killing a bridge is not proof Apple's operation stopped.
An authorized reconciliation reader may inspect a held project through a
special internal read permit bound to the held attempt; normal providers may
not. It cannot mutate the project or settle the hold without effect evidence.
On coordinator crash, kernel locks disappear but journal recovery restores
uncertainty holds before accepting any new project access.

Service startup single-flight is separate from project permits. Lock order is
coordinator admission, service startup if needed, project permit, backend pump.
Never wait for the project permit while holding a backend response lock. Idle
leases retain identity but must reacquire/revalidate before another prompt.

## Durable Effect Attempts

The broker is not an exactly-once executor for arbitrary Apple operations.
Transport request IDs are not idempotency keys, and a fresh nonce cannot prove a
fresh logical intention. This design provides durable dispatch fencing,
same-operation dedupe and an uncertainty hold, not an exactly-once claim.

`domain::xcode_effect` owns states; `db::repos::xcode_effect_attempts` owns SQL and
CAS transitions. An engine-injected `XcodeEffectJournal` trait gives ACP access
without an ACP-to-DB dependency. Observations are projections, not the journal.

One versioned migration adds attempts and project holds. Each attempt has:
attempt UUID, server-issued nonce, caller operation key, run/owner lineage,
originating invocation, canonical project key, target digest, binding digest,
normalized request digest, effect origin/class, state/revision,
created/dispatched/completed times,
bounded redacted result or error, result digest, uncertainty reason, and optional
reconciliation evidence reference. Project holds reference attempt IDs, never
only expiring timestamps. Retain dedupe tombstones through run retention; never
purge an unresolved attempt or hold automatically. Origin is lifecycle, MCP or
shim. A provider/shim effect requires a verified binding digest. For internal
workspace open only, that field may be null: its target digest pins the resolved
root/project, operator trust admission and trust-policy digest, evaluated
permissions, expected installation/UID and observed service generation when
present. This permits durable startup fencing before a workspace exists without
pretending startup already has a verified binding. Model approval alone cannot
authorize that open; actual project trust and Apple consent are still required.

The nonce is the server-issued attempt UUID, not a credential, and is durably
stored so a retried prepare can return it. Required uniqueness is
`(owner_lineage, project_key, caller_operation_key)` and attempt UUID.
A changed digest under the same key/nonce is rejected. The caller
operation key persists across retry and session reset; JSON-RPC ID does not.
The prepare API returns the existing attempt for a repeated key. If a client
loses that key, it must query its attempts, not manufacture a retry identity.

| State | Permitted transition and meaning |
| --- | --- |
| `prepared` | Durable normalized intent and nonce issued; no host bytes sent |
| `dispatched` | CAS from prepared, commit durable dispatch fence and project hold before sending bytes |
| `succeeded` | Persisted validated terminal success; clear hold only when operation completion is established |
| `failed` | Proven terminal failure with no unresolved partial effect; otherwise classify as unknown |
| `unknown` | Dispatch may have happened; no replay, no fresh conflicting dispatch, preserve project hold |
| `reconciled` | Authorized readback proves applied, not applied, or partial outcome and that no operation remains in flight |

`prepared` may terminate as `failed` when cancelled before dispatch. On restart,
all nonterminal `dispatched` attempts become `unknown` before admission. A crash
after fence commit but before sending is intentionally conservative: unknown
is safer than a duplicate effect. A DB write failure before the fence means no
dispatch; failure persisting the outcome leaves the durable hold in place.

An ordinary MCP `isError` does not prove no partial effect. Async job/session
creation is not terminal completion. The tool adapter must have a tested
completion/reconciliation contract or it is not an admitted mutating tool.

Repeated commits of a nonce return its stored terminal result (with the current
JSON-RPC ID), its in-progress state, or `outcome_unknown`. They never resend the
host command. Different nonce/key on a held project is also denied. Client
disconnect after dispatch does not cancel outcome persistence.

Shim effects use the same journal and permits. The shim transport creates one
stable operation key before its first request and retains it for transport
retry; manager ownership binds the key. Nested commands within an authorized
test gate belong to the same parent operation and cannot acquire unrelated
effect authority. Missing journal integration disables the effectful route.

Internal warm workspace open uses a deterministic service-generation/project
key. Cold startup uses the persisted coordinator startup-attempt ID and exact
project target; the resulting service generation is recorded as outcome, not
invented in advance. Retries reuse that attempt until reconciled. After an
uncertain open, preserve any captured structured open ID/path result and inspect
service status through authorized reconciliation before any further open.
Path-only status cannot reconstruct a lost workspace ID or by itself establish
effect completion. If outcome evidence is insufficient, retain the hold; never
guess the mapping or close-and-reopen to manufacture certainty.

Reconciliation is an operator-authorized, revision-checked engine command, not
a provider tool or a free-text acknowledgement. It records the inspected state,
active-operation check, effect-specific evidence and operator identity. If
evidence cannot distinguish the outcome, the hold remains. A permitted later
retry is a new linked attempt after reconciliation, not replay of the old one.
General recovery of arbitrary build scripts remains outside automated scope.

## Trusted-Project Admission And Observable Drift

Admission requires explicit operator trust of the exact local project under
`trusted_local_project_v1`, pinned canonical execution root/project identities,
UID/installation/service generation, evaluated permissions and an explicit
effective tool allowlist. Unknown profiles, absent trust, digest mismatches and
unclassified tools fail closed. Trust never permits automatic Apple permission
grants or bypasses the invocation's permission restrictions.

Initial binding requires a structured open result with both the nonempty
workspace ID and exact canonical requested path. Neither an opaque ID alone,
an echoed request nor human-readable list prose establishes that association.
Under the project permit immediately before each scoped dispatch, recheck:

- The same pinned service generation and root/project filesystem identities.
- Fresh structured service status confirming the exact canonical path is still
  open; missing, unreadable or ambiguous required status fails closed.
- Current operator trust, evaluated permission/tool policy and manifest digests,
  including any known mapping-invalidating events and scoped handle provenance.

The September 20 user-supplied native status observation, recorded in the parent,
has `openWorkspaces` entries with `path`, `displayName`, `activeSchemeName` and
no workspace ID. Path presence plus stable service generation is continuity
evidence, not a complete current ID-to-path re-query. Retain the initial mapping;
do not invent an ID-bearing status schema or parse prose as substitute proof.
Always send the explicit bound ID upstream. Absolute-path selectors failed the
previous native read checkpoint and are not a fallback for missing ID evidence.

Observed disappearance, restart, close/remap, identity change or trust/policy
revocation invalidates leases, handles and prepared operations before dispatch.
A TTL cache is not sufficient. No close/reopen broker tools are exposed, and
close/reopen is not a validation or recovery shortcut. New lifecycle open remains
a separately authorized, journalled effect with the no-resend rules above.

External close/reopen between probes, same-generation hostile remap and malicious
concurrent filesystem changes can escape these checks. The approved model accepts
that risk; it requires neither non-reassignable IDs nor atomic expected-binding
validation nor an Apple executor-side checkout sandbox. `realpath`, ancestor,
symlink and deny-rule checks on explicit arguments are routing/accident guards,
not race-resistant confinement. Workspace references and authorized build scripts
may access outside the checkout; existing evaluated policy still governs explicit
facade arguments and which effect classes are admitted.

There is no new generic file-edit proxy. Every admitted tool still needs a closed,
tested request/result/error adapter, scope/handle validation and, for mutators,
completion/reconciliation rules plus the durable journal. Unknown tools remain
absent from `tools/list` and denied on direct call. Canonical repository gates and
remote-only UI policy are unchanged. Required workflow capabilities must pass
their actual route acceptance; hiding them cannot produce a release pass.

## Acceptance

- Assert prepare/validation occurs before every `ensure_policy`, `session/new`
  and reused `session/prompt`; stale root/service/policy sends zero prompt bytes.
- Inject cancellation/crash at every prepare/decision/commit boundary; verify
  no leaked permit, callable provisional token or implicit provider launch.
- Run broker and shim contention fixtures together, including nested commands,
  writer fairness, idle-session release and two different-DB daemon processes.
- Fault-inject before/after durable dispatch, host write, terminal response and
  outcome commit; count host effects, not just returned status. Repeated nonce
  and a new nonce while held must produce no second dispatch.
- Test missing/mismatched operator trust and policy, initial open ID/path errors,
  explicit ID injection, path-only fresh status, observed disappearance/restart,
  known remap and observable root/project/symlink drift. Invalid admission sends
  zero scoped dispatch bytes; no probe/permission path can auto-grant access.
- Cover foreign/null/default selectors, disallowed paths and unknown tools.
  Tools without closed tested adapters are absent from `tools/list` and denied
  on direct call. A path-only status fixture must not be called ID-mapping proof.
- Record the blind interval: unchanged path status cannot distinguish an external
  close/reopen or hostile remap between probes. Tests establish the supported
  observable checks, not adversarial filesystem or external-client isolation.
- Restart with unresolved effects, a different DB and truncated observation
  history. Admission must use journal/authority truth, not observation presence.
