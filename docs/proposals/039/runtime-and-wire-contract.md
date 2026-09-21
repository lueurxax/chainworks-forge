# P039 Runtime And Wire Contract

Revision `p039-r2`, 2026-09-20. Reviewed planning contract, not implemented.
Parent: [P039](../039-blocked-run-fork-and-canonical-carry-forward.md).
This child owns the same-repository `implementation_restart_v1` path.

## 1. Authority And Admission

Writes require Operator principal class, an explicitly global principal
(`run_scope.is_none()`), the exact tool capability and BoundaryPolicy allowance.
An explicit empty scope `[]` is not global and is denied, including on replay.
Read-only operators, provider agents, observers and automation caller classes
cannot prepare, activate, reconcile or abort. A narrow explicit-v3 Operator
allowed to call `runs.start` does not automatically get continuation authority.
Register separate ToolIds; a broad boundary `runs.*` row is not a capability
grant. Explicit allowlists never expand on upgrade. Default-Operator opt-in must
be an explicit tested change, not an accidental class-wide wildcard.

Preview and detailed inspection require exact read capabilities and source access;
after activation detailed inspection also requires successor access. A scoped
principal may see compact linkage through existing authorized run reads, not
protected details of another run. Foreign and missing operation IDs have
indistinguishable denial envelopes. Authenticate every replay, not only creation.

Canonical tables, not summaries, reports or copied JSON, establish eligibility:

- source is blocked, without activated successor, in supported implementation
  preparation/loop/review, before release/publish/push admission;
- source frozen compiled topology/agent roles match the closed legacy adapter
  for `proposal_to_release`: required implementation gate, preparation then
  code_writer and implementation review; neither workflow ID nor state label
  alone proves the frontier. Unknown/changed shapes hold;
- same idea has no other dispatch-eligible run or live continuation operation;
- no pending/running/scheduled work, nonterminal agent, actionable approval,
  active prompt, unsettled repair lease/materialization or main-sync owns source;
- provider processes are settled, with no ambiguous identity, unresolved
  cancellation/shutdown signal or late-output settlement;
- ordinary release-effect ledger and headless Xcode attempts/project holds are
  known clear for source run and its effective resource;
- HEAD/index/files, selected canonical artifacts, old snapshot hashes and
  historical human decisions match the preview witness;
- target current compiled definitions, repository identity and delivery policy
  match expected hashes and the closed supported profile;
- target policies explicitly grant required headless operations and all skills,
  providers and effective worktree strategies compile under the current runtime.

Historical idle sessions/paused escalation rows may remain, but not live handles
or scheduled wakeups. The fence prevents later auto-resume. Unobservable process
or resource state holds. Recheck eligibility at reservation and activation.
Other ideas may continue: no globally idle daemon or second daemon is required.

## 2. MCP Surface

Strict input DTOs reject unknown fields, duplicate JSON keys and unknown versions.
UUIDs are canonical lowercase. Writes take `caller_request_id` UUIDv4, matching
current lifecycle command contracts. Register the tools in lifecycle dispatcher
idempotency so the generic P081 wrapper does not demand a second UUIDv7 key.

| Tool | Inputs | Result / effect |
| --- | --- | --- |
| `runs.continuation_preview` | `run_id`, `target`, `selection`, `profile`, optional `cursor`, `limit` | Read-only bounded plan/witness/digest and inventory page; no filesystem/Git mutation, DB write, provider or Apple call |
| `runs.continue_blocked` | Preview inputs, `expected_plan_sha256`, `caller_request_id`, `reason` | Journal/reserve and enqueue preparation only; return operation ID, not an activated run |
| `runs.continuation_get` | `operation_id`, optional `cursor`, `limit` | Canonical phase/holds and manifest page; no repair side effect |
| `runs.continuation_activate` | `operation_id`, `expected_version`, `expected_manifest_sha256`, `caller_request_id` | Verify prepared material, atomically create/link/queue one successor |
| `runs.continuation_reconcile` | `operation_id`, `expected_version`, `caller_request_id`, `reason` | Inspect interrupted effects and record verification/hold; no filesystem/Git mutation or dispatch |
| `runs.continuation_abort` | `operation_id`, `expected_version`, `caller_request_id`, `reason` | Pre-activation only, effects settled; release reservation, preserve files; no source retry |

Never accept source status overrides, SQL, destination IDs, command text,
source-root overrides, arbitrary state IDs, approval decisions, provider tokens,
Apple workspace IDs or deletion flags.

Implementation clarification (2026-09-20): preview returns one of the strict
`run_carry_forward_preview_page_v1`, `run_carry_forward_preview_stale_v1`, or
`run_carry_forward_preview_hold_v1` variants. The hold variant contains only
`schema_version`, authorized `source_run_id`, a nonempty bounded `holds` list
of closed-code denials, and `next_action` (`inspect`, `resolve_hold`, or
`refresh_preview`). Incomplete evidence never produces invented witness IDs,
empty stand-in hashes, an eligible plan, or an INTERNAL error. A page still
requires the complete observed witness; this variant grants no admission.

`target` fields: `workflow_yaml_path`, `agent_catalog_yaml_path`,
`expected_workflow_snapshot_hash`, `expected_catalog_snapshot_hash`,
`delivery_configuration_json`. Expected hashes may be omitted only for preview,
which returns the compiled hashes; prepare requires them. Definitions must be regular files in approved
repository definition roots, not arbitrary paths supplied by artifacts. Compile
with the same pure compiler/policy validation as StartRun; derive idea/workspace
from source. Split observational checks from effectful delivery preflight:
the existing writable-directory probe creates/deletes a file and must never run
during preview. Preview creates no directories, persistent spool/cache, journal
or lock files, including through Git helpers. A required write probe belongs only
to authorized preparation, after reservation and a journalled intent, within a
validated operation-owned destination. Preview reports such checks as pending;
it does not claim writability from a read-only permissions check.
Do not inherit old rollout waivers/permissive flags. Re-run delivery and
enforce-mode rollout preflight on the chosen proposal/current target at activation
and ordinary implementation admission. A historical pass is insufficient.

`selection` fields:

- `proposal_input`: tagged `{kind: artifact | carried_input, id: UUID}`;
- `reference_inputs`: unique tagged references, at most 128; the server adds mandatory
  unresolved finding/backlog inputs even when omitted here;
- `include_dirty_work`: must be true, false cannot mean discard;
- `excluded_workspace_paths`: at most 128 relative machine/generated paths,
  each with a nonempty reason and explicit manifest inclusion.

Resolve artifact paths server-side and hash bytes; supplied hashes cannot stand
in for observation. Excluding tracked product code/proposal/findings or
unclassified dirty/untracked data is denied. V1 machine-path allowlist is
`.antigravitycli/`, `.DS_Store`, `DerivedData/`, `.build/`, `node_modules/`,
`target/` and the source's generated `.chainworks/` subtree. Inventory exclusion
type/reason; it never permits source deletion or link traversal. A product file
under an otherwise generated prefix still cannot be excluded merely by prefix.

An artifact reference must belong to the immediate source run. A carried_input
reference must be an installed input owned by that immediate source's activated
continuation, with matching current bytes, logical role and schema. Retain both
the immediate owner and original artifact/manifest provenance; never fabricate a
provider artifact to make the second fork selectable. Validate the ancestry as an
acyclic chain, bounded to 32 hops; store ancestor digests rather than recursively
embedding prior manifests. Unknown/missing ownership or a reference-only entry
presented as the proposal seed holds.

Example activation request (future schema, not a live prepared operation):

```json
{
  "operation_id": "00000000-0000-4000-8000-000000000039",
  "expected_version": 3,
  "expected_manifest_sha256": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "caller_request_id": "00000000-0000-4000-8000-000000000040"
}
```

### Command Results And Transport

All four mutation tools return `run_continuation_command_result_v1`,
not the P083 lifecycle result DTO: that strict schema has no operation ID. Reuse
its idempotency machinery, not its output shape. Register output schemas for each
tool. Every result has exactly these required fields (nullable where specified):

- `schema_version`: `run_continuation_command_result_v1`;
- `status`: `accepted | denied | replayed`;
- `caller_request_id`: the validated request UUID;
- `journal_id`: UUID or null when no command journal entry exists;
- `operation`: null or `{operation_id, phase, version, successor_run_id}`;
  successor ID is null before activation;
- `denial`: null or `{code, message}` using the section 7 closed reason set;
- `next_action`: `none | inspect | await_worker | refresh_preview | read_operation |
  explicit_reconcile | resolve_hold`;
- `replay_of`: null, or `accepted | denied` only when status is replayed.

Accepted means the command/phase was durably recorded, not that preparation,
abort settlement or review finished. Accepted has non-null operation/journal and
null denial. Denied has non-null denial; before reservation operation/journal
are null. A stale-version denial may return the authorized operation's current
summary, without changing it. Replayed carries the exact original payload and
journal IDs, changes only status/replay_of, and does not claim fresh operation
state; use get for current truth. Auth is rechecked before revealing any replay.

Illustrative outcome tuples, with the same full envelope above:

| Case | status / operation.phase / denial.code / next_action |
| --- | --- |
| Prepared request enqueued | accepted / preparing / null / await_worker |
| Same activation request re-read | replayed (replay_of=accepted) / activated / null / inspect |
| Busy source before reservation | denied / null / source_busy / resolve_hold |
| CAS mismatch | denied / current authorized phase / stale_operation_version / read_operation |
| Effect outcome durably recorded as uncertain | accepted / needs_reconciliation / null / explicit_reconcile |

Expected eligibility, capacity, CAS and rollout holds are JSON-RPC success with
this typed result, not INTERNAL errors. MCP uses the current text-content JSON
envelope; no new structuredContent dependency. Invalid inputs use -32602;
authentication/authorization uses the existing boundary -32004 envelope without
operation details. Unexpected faults use redacted -32603, never raw paths/SQL.
If commit outcome cannot be established, -32603 data is exactly
`{schema_version: p039_transport_error_v1, code: command_outcome_unknown,
next_action: replay_same_request}`. Never report a no-operation denial then.

After a transport loss/unknown commit, only the identical caller/request/intent
may be re-read through idempotency; no new key or automatic effect retry. A
recorded unknown Git/copy effect requires explicit reconciliation, not replaying
the effect. A known denial requires its stated next action and a new request
after changed inputs/version; repeating the old key retains the original denial.

## 3. Contracts And Readback

Define strict serde DTOs and schema/round-trip fixtures for these contracts.
Unknown write versions fail closed; readers show unknown phase as non-actionable
while retaining raw version identifiers for diagnostics.

### `run_carry_forward_plan_v1`

Fields: `schema_version`, `source_run_id`, `source_witness`, `target`, `profile`,
`selection`, `entries`, `workspace_snapshot`, `capability_delta`, `holds`,
`limits`, `plan_sha256`. Entries carry source artifact ID (nullable for files),
logical name, source-relative path, role, SHA-256, bytes/mode, source contract and
schema version, target logical name, reason and historical approval relation.

Preview uses `run_carry_forward_preview_page_v1`: `schema_version`, `plan_summary`,
`entries`, `entry_count`, `plan_sha256`, `next_cursor`. The plan digest covers the
entire canonical inventory, not merely the returned page. Order by normalized
source namespace, UTF-8 relative-path bytes and stable entry ID. This same total
order is used in hashing, including entries sharing a relative path. The summary
omits inventory bytes; cursor and page limits never alter plan identity.

For each page, resubmit the original preview inputs plus opaque cursor and limit.
The bounded cursor encodes version, normalized request digest, full plan digest
and last entry key (maximum 4 KiB). Treat it as untrusted: it cannot select roots,
alter selection or bypass access checks. Recompute/rehash the bounded plan on
each request with no persistent spool; any changed witness/input/plan returns
`run_carry_forward_preview_stale_v1` with fields `schema_version`,
`code=preview_stale`, `next_action=refresh_preview`, no mixed-generation entries.
First page uses null/absent cursor. No continuation operation is required to read
all pages. Invalid cursor/limit is -32602; stale is a typed JSON-RPC success.
Operation manifest paging uses the same total order and a manifest-digest-bound
cursor; get remains read-only and rejects stale cursors instead of mixing pages.

Roles: `execution_seed`, `reference_only`, `preserve_only`, `excluded`. Tagged
entry variants define mandatory fields; excluded entries have no hash where
reading content is forbidden, and can never satisfy target input requirements.

Witness includes old workflow/catalog hashes, stage/cursor, relevant journal,
approval and artifact-generation IDs, idea digest, HEAD/tree/index/content digest,
directory identities and settled-ownership/effect digest. Never compare timestamps
alone. Compare witness in reservation/activation transactions using fresh file
observations outside them; fence closes internal writers. Same-UID external
writers must be quiescent; this feature is not a filesystem sandbox.

Workspace snapshot includes original base branch/revision, HEAD, staged/unstaged
patch digests, untracked/deleted inventories, link targets, exclusions and target
content digest. Hash modes and exact bytes, with no line-ending/name rewriting.
Canonical hashing uses a versioned sorted JSON serializer: maps by key, entry
arrays by the total entry order above; omit self-hash and informational observation
timestamps. Include target, selection and limits. Source hash notation is
normalized explicitly; do not compare prefixed and unprefixed hashes as equal
without schema-aware parsing.

Repeated historical artifacts at one canonical path require verified generation
identity and supersession, not timestamp or UUID ordering. Current canonical
bytes belong only to the verified active generation. Older mandatory findings
must retain their own authenticated bytes beneath the source metadata root;
sharing the current path is acceptable only when the recorded digests are equal.
Missing older bytes, checksums or generation links produce a provenance hold.
A newer successful review is not a disposition of older unresolved findings.

A historically checksumless plain output can be freshly observed only through
its exact frozen declaration and persisted producing invocation. This neither
backfills the old row nor establishes a historical cryptographic approval binding.
Multiple unproven old identities at a reused path cannot all acquire today's
bytes. Any admitted observed seed still enters fresh review and approval.

### `run_carry_forward_manifest_v1`

Plan plus operation ID, reserved successor ID, private preservation/worktree/meta
root refs, preservation hash, verified entries, operation-owned Git pin ref,
step receipts, verification timestamps and manifest hash. It binds independent
copied bytes, not source pointers. IDs and paths are server-generated.

### `run_continuation_readback_v1`

Fields: `schema_version`, `operation_id`, `source_run_id`, `successor_run_id`
(null until activation), `phase`, `version`, `plan_sha256`, `manifest_sha256`,
`execution_disposition`, `verification_status`, `holds`, `next_actions`,
`journal_id`, `updated_at`, `projection_freshness`, `manifest_page`, `next_cursor`,
`admission` (`run_continuation_admission_v1`: enabled, reason, fences_enforced,
reconcile_available). This admission object is separate from P084 enforcement
enabled_state; both can legitimately differ.
Disposition: `source_reserved`, `source_historical`, `successor_pending_review`,
`aborted_source_blocked`; it is not a new RunStatus or test result.

GraphQL adds nullable `Run.continuedFromRunId`, `continuedAsRunId` and
`carryForwardReadback`; MCP/reports use snake_case equivalents. The latter is a
`run_carry_forward_links_v1` aggregate with exactly `schema_version`, `incoming`,
`outgoing`. Each direction is null or a compact continuation readback without a
manifest page. Incoming is the activated operation whose successor is this run;
outgoing is this run's unique non-aborted operation, including a reservation or
hold before activation. Aborted attempts are available by authorized operation
get/history, not silently substituted as current outgoing. The two run ID links
derive only from activated operations. In A -> B -> C, B can simultaneously have
activated incoming and needs_reconciliation outgoing; neither overwrites the
other. Do not overload same-run recovery/continuation fields or infer direction
from most-recent timestamps. Reports and actual
release receipts retain manifest/content digests, source schema identifiers and
lineage; activation never fabricates a release receipt. V1 resolves each historical
schema declaration through canonical input ancestry to the hash-verified original
frozen catalog, never the current target catalog. The individual
`source_schema_sha256` remains null when no separate declaration digest was
materialized; neither a schema-name hash nor a current declaration may substitute.
This is not a self-contained export independent of the retained original catalog.
Reopen/removed-current-declaration tests must prove that historical resolution.
Privileged details are paginated, not returned on
every list refresh. Old rows yield null, not inferred links.

Projections report freshness; stale manifest-bound approval data is non-actionable.
Projection refresh cannot change execution truth. Old clients ignore additive
fields; versioned historical input schemas remain readable even when current
catalogs remove an output/tool. Historical readability confers no dispatch rights.

## 4. Persistence And Ownership

Add registered migration logical ID `p039_run_carry_forward_v1`. Allocate its
numeric filename from the actual implementation registry; no destructive down
migration or old snapshot/status rewrite.

| Table | Minimum authority and constraints |
| --- | --- |
| `run_continuations` | Operation PK; source/idea IDs; reserved successor unique; activated successor FK nullable/unique; caller fingerprint/request ID/hash; profile/phase/version; plan/manifest refs/hashes; compiled target snapshot refs; journal ID; holds/times |
| `run_continuation_steps` | Operation FK + step key unique; planned/dispatching/verified/unknown; worker generation; intent hash; expected destination identity; receipt hash/times |
| `run_continuation_inputs` | Input ID PK; operation FK + ordinal unique; successor FK nullable until activation; role; source artifact/schema/hash; target logical name/path/hash; installed flag; historical relation |
| `run_execution_fences` | Source run PK; operation FK; generation; reserved/historical; creation time; never released by TTL alone |
| `run_continuation_approval_bindings` | Approval ID PK/FK; successor and stage-execution FKs; logical stage; immutable evidence tuple/digest; generation; current/superseded; consumed transition ID nullable |
| `run_continuation_commands` | Immutable original result keyed by caller fingerprint/request UUID, with command and canonical intent digest; committed with the phase change or pre-admission denial; no TTL deletion |

Partial unique indexes allow one non-aborted operation per source and one
preparing/prepared/aborting/needs_reconciliation reservation per idea. Activated source
uniqueness is permanent, but does not forbid a later linear continuation of its
blocked successor. Failed preparation keeps reservation
until explicit safe abort. Lineage comes from the operation row, not two
independently mutable Run columns. One runnable head per idea excludes the
historical-fenced source without making blocked a terminal RunStatus.

Extend both StartRun and reservation to use the same atomic idea guard. Recheck
queue claim/pre-dispatch; do not assume existing Rust StartRun enforces the
single-active-idea rule. Source rows remain visible as blocked historical truth;
scheduler/capacity/actionability use execution disposition and cannot resume them.
Do not repair or rewrite unrelated existing multi-run history in this migration.

All writes use registered DbWriter operations, compact refs/digests and bounded
transactions. Large file/snapshot bytes are spooled outside transactions.
Foreign-key/index/old-null-row tests are mandatory.

### Approval Binding

When a P039 successor enters its implementation gate, create approval and binding
atomically for that exact stage execution. Unique current binding per successor
and logical gate; the tuple is the parent section 6 evidence tuple. A later tuple
or gate entry supersedes the binding, not the historical approval decision.
Keep any old granted row as history and create a new approval ID/generation.
Resolution checks active binding/version and freshly verified tuple, then commits
decision and binding CAS through the ordinary approval command transaction.
Transition evaluation for P039 gates requires the current exact stage-execution
binding, not any prior granted/rejected approval with the logical stage ID.
Missing/stale/superseded bindings are non-actionable across MCP and GraphQL.

Before the first post-gate dispatch, revalidate/consume that entry authorization
against the same starting tuple. Once consumed, legitimate new implementation
outputs/code changes belong to its new execution lineage, not to an immutable
forever-code-hash constraint. Returning to the gate, replacing proposal inputs
or changing frozen policy requires a new binding/review. Legacy non-P039 approval
semantics are unchanged; no new binding is synthesized for old grants.

## 5. State Machine And Linearization

Phases: `preparing`, `prepared`, `aborting`, `needs_reconciliation`, `activated`, `aborted`.
Version increases on each committed transition. Eligibility failure before
reservation creates no operation. The preparation worker has no provider access.

1. Preview reads/compiles/selects; no source hold or new run is implied.
2. Reserve recomputes/compares plan, authenticates, takes source/idea guard,
   checks quiescence, inserts operation + reserved fence, queues one preparation
   operation-owned item and journals atomically. Its payload references operation
   ID, with no source run_id ownership; it cannot be mistaken for runnable source
   work. No successor Run or AdvanceRun exists yet.
3. Prepare claims generation, journals effect intent, preserves source, pins
   source OID, provisions distinct checkout at that OID and applies verified
   overlay. Copy selected inputs into reserved metadata roots; verify source
   stability, copied bytes, identities and receipts; publish manifest/prepared.
4. Explicit activate rehashes prepared material and checks target/trust diagnostics
   outside the writer transaction. One bounded transaction rechecks fence/version,
   source/idea admission and witness (excluding this operation's own journal and
   projection-refresh bookkeeping), marks source historical, inserts successor
   with current frozen definitions/worktree/entry, registers seed metadata, sets
   lineage, enqueues one AdvanceRun, settles journal and marks activated.
5. After commit publish invalidations/notifications. Failure here requires
   projection rebuild, never another activation or AdvanceRun.

Only activation transfers execution ownership. Source commands that could change
execution (retry, resume, cancel, approval/conflict resolve, catalog retrofit,
main-sync, continuation) return `source_continued` after activation, or
`continuation_in_progress` while reserved, before business mutation.
Reads and evidence-only diagnostics remain available.

Enforce fence in command handlers, all enqueue/claim paths, orchestrator advance,
provider pre-dispatch, output-repair/artifact settlement, escalation timers and
startup/host recovery. Stale workers cannot publish authoritative outputs after
fencing; late bytes are quarantined as diagnostics. P064 table existence is not
proof of enforcement; P039 supplies and tests these checks.

Preparation has its own bounded async execution lane: one active copy worker and
at most three queued operations per daemon. Check the queue budget atomically
before reservation; overload returns `continuation_capacity` without a fence.
The current executor runs non-InvokeAgent items inline; do not add a 15-minute
copy there. Use a tracked task, blocking I/O isolation and semaphore, retaining
DbWriter budgets. Other ideas' AdvanceRun and provider work must progress while
copying is paused. The preparation deadline includes queue wait. Worker generation
and operation phase are checked before every effect, after waits and before
publication; only its current generation can advance the operation.

External source edits invalidate the witness and keep a hold/copies. Show bounded
changed-path diagnostics, require abort/new preview; never reset source bytes.

## 6. Files And Input Publication

Owned storage: `<daemon-app-support>/run-carry-forward/<operation_id>/`, mode 0700,
private evidence 0600. The reserved successor gets distinct metadata, provider
output, worktree and branch identities derived server-side. Reject overlap with
source, another run, Git metadata or existing unowned destinations. Existing
same-operation paths require matching ownership and receipts, not mere existence.

Implementation clarification (2026-09-20): the daemon-owned application-support
parent replaces the original repository-local parent to avoid source/destination
overlap when the source checkout is the repository root. An operator provisions
this private parent before enabling admission. Startup validates owner, mode and
every path component without following links; neither preview nor a request may
create or select another storage parent. Existing operation paths remain durable
canonical references, not values inferred from the current caller or working directory.

Preservation never dereferences symlinks. Executable copies permit only relative
links normalized inside the new checkout, outside protected/runtime roots.
External/absolute links in explicitly excluded machine paths stay historical-only;
other unsafe links hold. Preserve target strings, not outside contents. Never
copy `.git`, credentials, runtime homes, sessions, locks or special devices into
the successor. Use no-follow directory-relative I/O and verify identities around
open/copy/publish; reject path replacement races.

Preserve tracked code/deletions, binary changes, mode bits, staged/unstaged patches
and selected non-ignored untracked files. Target checkout starts at source HEAD
with clean index plus unstaged overlay representing final source bytes. Original
index semantics live in preservation only. Excluded tracked machine entries
produce explicit receipt-listed target deletions, not silent omission. Existing
credential/runtime deny rules apply; unclassified dirty/untracked data holds
rather than disappearing. Do not claim arbitrary source is provably secret-free.

Temporary writes remain under owned roots, stream-hash, fsync content/parent and
publish atomically without replacement. Journal intent before Git effects.
Create a unique branch at immutable OID, never moving main. Disable hooks,
external filters, recursive submodule/LFS and network hydration. Missing objects
or unsupported filters hold. No source commit/rebase is performed.

Activation installs new input IDs in `run_continuation_inputs`, with
`origin=carry_forward_input`, source schema/hash provenance and operation owner.
Do not insert rows with fabricated agent/stage IDs into the existing `artifacts`
table, whose provider metadata columns are non-null. Extend typed input lookup
and input readback explicitly; an active validated new-run provider output takes
precedence over a same-name seed, otherwise only an installed execution_seed can
fulfil input lookup. Reference-only rows cannot satisfy transition predicates.
Provider-output freshness checks remain unchanged. Reference bundles occupy a
separate input namespace, never the active output index. No old
approved_proposal/review/test contract is active.
Regenerate run-state/active-index projections from new canonical DB truth.

Old absolute paths inside historical documents stay text. A separate ID/relative
path map locates copied references. Do not silently rewrite historical bytes or
interpret their paths as writable new destinations.

## 7. Bounds, Replay And Recovery

V1 ceilings: 50,000 inventoried entries, 2 GiB preservation, 256 MiB/file,
128 reference artifacts, 1 MiB request, 64 KiB summary, 100 default/500 maximum
manifest page, 1 MiB streaming buffer. Exceeding any limit holds; no silent
truncation. Detailed inventory is spooled/paginated, not a hot-list payload.
Preview deadline 120 s, preparation 15 min, activation verification 120 s.
Writer commits retain normal DbWriter deadlines. Timeout cannot release a fence
or prove a child/process group has stopped.

Lifecycle idempotency TTL is 300 s, failed-terminal automatic retry forbidden,
plus permanent operation/request/source uniqueness. Prepare request hash binds
source/target/profile/selection/plan; other writes bind operation/version/manifest
and action, all scoped to caller fingerprint. Same request replays its original
operation/result after transport TTL. Different intent under that ID conflicts
without disclosure. New ID cannot create a sibling for reserved/continued source.

On restart unfinished dispatching steps become unknown/needs_reconciliation.
Never repeat Git/materialization or activate automatically. Reconcile only reads
owned paths/refs/receipts and verifies process settlement. Complete material and
witness may restore prepared; otherwise retain hold. Partial copies are not a
manifest. Once effects are known settled, explicit abort is allowed; a fresh
preview can start a different preparation with new roots/IDs. Unknown effects
forbid abort. No lease expiry alone grants recovery authority.

Abort is forbidden after activation. Earlier abort first atomically enters
aborting, increments/revokes worker generation and invalidates all queued
operation-owned preparation items; retain the source fence. Cooperative worker
cancellation cannot authorize another effect. If a child/effect was already
dispatched, record/reconcile its outcome and verify worker/child settlement.
Only after no worker, queue claim or unknown effect remains may a second bounded
transaction mark aborted and remove reservation, leaving source blocked, never
automatically running. Reconciliation may finish this abort but cannot restore
prepared while abort intent is active. Stale worker completion cannot overwrite
aborting/aborted; it may append only quarantined diagnostic receipts. Copies,
worktrees and refs remain pinned by operation ID; cleanup is a separate future
contract. An aborted preparation is not a runnable successor.

Reason codes: `source_not_blocked`, `source_busy`, `source_continued`,
`continuation_in_progress`, `unsupported_frontier`, `target_profile_invalid`,
`target_policy_incompatible`, `source_changed`, `prepared_material_changed`,
`artifact_provenance_invalid`, `unsafe_path`, `destination_conflict`,
`headless_effect_unresolved`, `effect_outcome_unknown`,
`historical_binding_unproven`, `continuation_budget_exceeded`,
`idempotency_conflict`, `stale_operation_version`, `continuation_capacity`, `storage_unavailable`,
`rollout_hold`, `preview_stale`. Unproven historical binding is informational when fresh-review
admission is otherwise valid; uncertain effects and missing authority always hold.

## 8. Completion

Require schema fixtures, auth negatives, exact-once DB/filesystem crash tests,
complete source-fence owner coverage, preservation/independence tests and
four-lane readback parity. A JSON manifest alone does not implement this contract.
