# Blocked-Run Carry-Forward

The Rust control plane preserves an eligible blocked run's working material in
a distinct successor, compiled against current definitions and entering fresh
review. This is not retry, resume, a snapshot rewrite, or transfer of historical
execution authority.

Admission is disabled by default. This document describes implemented behavior;
deployment and live acceptance are separate from the
[offline integration evidence](../evidence/p039-durable-integration.md).

## Eligibility And Source Protection

The supported profile is `implementation_restart_v1`. Its closed legacy adapter
recognizes implementation preparation, implementation-loop and review frontiers
of the supported proposal-to-release topology. A matching workflow name or state
label alone is insufficient. Release/publish admission, unsupported frozen
topology, missing provenance or unavailable authenticated historical bytes hold
the operation.

Reservation and activation recheck canonical ownership under the database writer
transaction. Source work, shared effective checkout resources, indirect
stage/agent/session owners, pending approvals and unsettled provider, repair,
release, cancellation or headless effects prevent admission. Unknown ownership
is not absence. The same idea cannot acquire a second runnable head.

Persisted source fences protect ordinary command, dispatch, recovery and SQL
write paths. Activation makes the original run historical without rewriting its
blocked status or deleting its evidence. Fences do not expire by timeout.
Unrelated work can progress: an ordinary queue claim skips protected candidates
without changing their history. An unresolvable, otherwise unowned candidate can
be quarantined with `p039_queue_owner_unresolved`; this grants no dispatch rights.

## Operation Lifecycle

1. Preview reads current source files, index, canonical history and current
   target definitions. It writes no files, directories, journals or database
   rows. All inventory pages bind the same complete plan and source witness.
   Changed observations return a stale preview; insufficient evidence returns
   a typed hold, not an invented eligible plan.
2. Prepare reserves the source and idea, persists command identity and queues
   preparation. It does not create a successor. The tracked lane has one active
   preparation and at most three queued; the deadline includes queue wait.
3. The worker journals intent before each effect and checks its generation
   before further work or publication. Uncertain effects retain evidence and
   enter a hold. Restart does not replay them automatically.
4. Activation verifies the immutable manifest, source witness, target and current
   policy, then atomically installs one successor, input lineage, a historical
   source fence, a command result and exactly one `AdvanceRun`.
5. Reconciliation records verified state or a hold without repeating external
   effects. Pre-activation abort revokes the worker generation and releases the
   reservation only after worker/effect settlement. It preserves files and does
   not resume the source. Activated operations cannot be aborted as preparation.

## Files And Input Authority

The successor checkout is independent and pinned to the source HEAD, not silently
rebased onto current main. Preservation includes staged and unstaged bytes,
untracked product files, deletions, renames, executable modes and the original
index. Explicit machine-path exclusions are recorded, not deleted from source.
Unsafe links, unsupported Git filters/LFS/submodules and exceeded budgets hold
instead of silently discarding work or hydrating objects from the network.

Operation-owned private storage contains the checkout, preserved evidence and
metadata. Paths are server-owned; callers cannot supply arbitrary destinations.
The manifest binds content hashes and directory identities. The engine's
in-process metadata-root capability is tied to that manifest and the owned
execution; serialized fields, environment variables and paths alone grant no
authority. Ordinary ACP workspace confinement remains unchanged.

Installed inputs have two roles:

- `execution_seed` supplies the selected proposal until a valid fresh successor
  output takes precedence.
- `reference_only` retains mandatory findings and historical context, without
  satisfying provider-output contracts, gates or approval decisions.

Both roles retain immediate-source and original artifact ancestry. Installation
does not fabricate successful provider artifact rows. Fresh outputs require the
exact frozen producer and settled output evidence; fallback outputs also require
their bound authorized fallback decision.

Historical structured declarations resolve through retained, hash-verified
original catalogs, not the current catalog or a hash of a schema name. Removed
current declarations do not redefine stored inputs. Missing mandatory older
generations remain a hold. Arbitrary removed-original-tool compatibility is not
guaranteed, and legacy external skills without authenticated bytes are not
reconstructed from present-day files.

## Fresh Review And Approval

The successor uses current compiled definitions with explicit headless policy.
Old shell/Xcode labels, project trust, approvals, successful tests and reusable
provider sessions do not grant new execution authority.

Fresh review reads the successor checkout. Ordinary approval binds the exact
stage execution, manifest, proposal and snapshot identity. Code/index/snapshot
freshness remains required until matching persisted `PromptSent` evidence;
queue admission or snapshot publication is not proof that execution started.
Legitimate later implementation writes do not retroactively invalidate that
started entry. A later review/gate requires a new binding, not reuse of the old
grant. Engine-owned preparation snapshots are recognized only through the exact
activated profile and pinned artifact identity, never by task name alone.

Headless project trust and native consent remain separate requirements. There is
no copied trust, implicit native consent or IDE fallback. See
[xcode-headless-runtime.md](xcode-headless-runtime.md).

## MCP And Readback

| Tool | Purpose |
| --- | --- |
| `runs.continuation_preview` | Read-only plan, inventory and eligibility |
| `runs.continue_blocked` | Reserve and queue preparation |
| `runs.continuation_get` | Canonical operation and paginated manifest readback |
| `runs.continuation_activate` | Verify and atomically activate the successor |
| `runs.continuation_reconcile` | Record verification or a hold without effect replay |
| `runs.continuation_abort` | Revoke and settle pre-activation preparation |

Each tool requires its explicit capability. Mutations require a global Operator;
a restricted Operator, agent, observer or automation does not qualify. Existing
explicit allowlists and default principals do not expand on upgrade. Detailed
reads require source access and, after activation, successor access. Every replay
is freshly authorized; foreign and missing operation IDs have identical denials.

Strict versioned DTOs reject duplicate/unknown fields and malformed identifiers.
Mutations use caller-supplied lowercase UUIDv4 request IDs with durable intent and
result binding. Same-key replay returns the original outcome without another
successor or queue item, even after database reopen. An unknown outcome requires
the same request key, not an automatic new-key retry. A denied prepare can persist
its audit receipt while leaving source authority, files and index unchanged.

Compact `run_carry_forward_links_v1` readback has independent incoming and
outgoing links across GraphQL, MCP, reports and release receipts. Aborting a
middle run's outgoing operation clears that current link while preserving its
incoming history. Projections and notifications do not replace canonical links
or authorize dispatch; rebuilding them does not repeat activation.

## Admission And Recovery

`CHAINWORKS_RUN_CARRY_FORWARD_ENABLED` defaults to `false`. Enabled installations
require a private, owned, non-symlink `run-carry-forward` directory under the
daemon's application-support root. Disabled startup does not create that storage.
The service is shared by both northbound transports.

Disabling admission prevents new preparation and activation. It does not disable
persisted fences, readers, reconciliation, lineage or successor protection.
Migration requires a SQLite-consistent, verified backup; an older incompatible
daemon refuses the upgraded schema. Rollback uses a compatible diagnostic build,
not schema downgrade, source reactivation or restoring an old database over
newer unrelated work.

The retained `proposal-039|p039` gate covers offline integration; the narrower
`proposal-039-i1|p039-i1` alias covers only the first provider-free experiment.
Their scope is documented in [test-gates.md](test-gates.md#proposal-039p039).
Passing them does not establish native consent, live provider execution,
deployment or a completed production transfer.
