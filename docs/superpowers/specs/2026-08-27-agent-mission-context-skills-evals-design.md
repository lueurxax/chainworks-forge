# Agent Mission Context, Agent Skills, and Evaluation Design

Date: 2026-08-27
Status: Revised after proposal-readiness review; awaiting re-review

## Summary

Chainworks agents currently receive provider instructions, one resolved skill
fragment, task and artifact paths, materialized input artifacts, output
contracts, and runtime identifiers. They do not receive one compact,
control-plane-owned statement of the run mission, the purpose of the current
stage, their role in that mission, or the consumer of their result.

At the same time, most production skills in `examples/agents/agents.yaml` are
one-line `inline_skill` descriptions while detailed procedures and role rules
live in large per-agent prompts. Existing external skill bundles are split
between example and plugin trees, and their eval scenarios are declarative
documents rather than an executable quality system.

This design replaces that arrangement with three separate contracts:

1. a frozen base mission plus a Rust-owned durable authority overlay and
   per-invocation assignment context;
2. Agent Skills-compatible procedural bundles with one canonical source;
3. deterministic and live evaluation lanes that compare candidate behavior
   with a frozen baseline.

The new production path is mandatory for every newly compiled run. It has no
feature flag, disable switch, cohort, or legacy single-path fallback. Existing
context-absent frozen runs remain available for historical readback, but cannot
resume live provider or side-effect execution after V2 cutover.

## Problem Evidence

The current implementation has several structural failure modes.

- A normal task sees the Idea only when the task explicitly declares an Idea
  input. Later implementation and review agents usually infer the global goal
  from large proposal and review artifacts.
- Workflow states have labels and owners, and tasks have names, but neither has
  a typed purpose or completion outcome.
- Agent role, reusable procedure, runtime restrictions, output-shape advice,
  and historical corrections are mixed inside long catalog prompts.
- `external_skill` resolution reads the complete `SKILL.md` as raw prompt text,
  including frontmatter, and does not expose bundle resources through a
  versioned run-local manifest.
- Production agents primarily use `inline_skill`, so the skill layer has little
  practical effect on behavior.
- Existing skill scenarios are not executed against a baseline, repeated
  across production providers, or scored by retained assertions.

The retained P095 history provides a concrete authority failure. An older run
brief required a default-off canary while a later explicit operator directive
required mandatory activation with no feature flag. Reviewers continued to
reason from the stale brief. The model did not have a typed authority chain
that made the supersession visible and enforceable.

## Current Skill Inventory

The production catalog currently declares twelve skill IDs. Eleven are
`inline_skill` entries and one, `docs_quality_guardian`, is a hardcoded
`builtin_agent`. None of the Agent Skills-style bundles under `examples/skills`
or `plugins/proposal-lifecycle-review/skills` is used as an external production
bundle by the current catalog.

The migration inventory is:

| Current skill ID | Canonical disposition | V2 execution mode | Effect owner | Partial-failure rule |
|---|---|---|---|---|
| `proposal_review_triad` | retire for new catalogs; historical snapshot compatibility only | `historical_only` | none | V2 compile rejects any reference |
| `proposal_review_router_skill` | `proposal-review-router` | `proposal_review` | read-only reviewer fan-out | missing selected result prevents aggregate settlement |
| `proposal_implementation_audit` | `proposal-implementation-audit` | `implementation_audit` | read-only evidence inspection | incomplete evidence yields typed not-ready, never inferred pass |
| `docs_quality_guardian` | `docs-quality-guardian` | `docs_review`, `docs_closeout` | declared run-artifact/docs permission | partial write is non-current until output-contract settlement |
| `orchestrator_core` | `lead-orchestration` | `lead_orchestration` | idempotent control-plane commands | committed command receipt is reconciled; no blind retry |
| `proposal_writer_core` | `proposal-authoring` | `proposal_authoring` | declared proposal-artifact write | partial generation cannot become current |
| `code_writer_core` | `code-implementation` | `implementation` | workspace source/test writes | ambiguous write enters typed reconciliation |
| `security_checker_core` | `security-review` | `security_review` | read-only evidence inspection | missing scan/result blocks settlement |
| `prepush_review_core` | `prepush-review` | `prepush_review` | read-only evidence inspection | missing required proof blocks settlement |
| `github_commit_push` | `github-delivery` | `delivery` | existing Git/external side-effect authority | post-dispatch ambiguity reconciles by durable effect identity, never retry |
| `connect_publisher` | `connect-publishing` | `publishing` | existing network publication authority | post-dispatch ambiguity reconciles by durable effect identity, never retry |
| `steward_core` | `forge-steward` | `post_run_tuning` | recommendation artifacts only | cannot mutate or promote production inputs |

The example and plugin copies of proposal review and implementation audit have
different digests today. The canonical registry becomes the authoring source;
plugin packaging is generated from it and exact parity is enforced by a gate.

## Goals

- Make the global run mission visible and authoritative in every provider
  invocation.
- Tell every agent why the current task exists, what the agent owns, what it
  must not own, and what valid completion means.
- Separate role identity from reusable procedures and from runtime policy.
- Adopt Agent Skills bundle structure and validation for production skills.
- Freeze the base context, role, skill, resource, and eval provenance with the
  run, then pin the exact durable authority-overlay head used by each
  invocation.
- Turn historical Chainworks failures into executable regression cases.
- Measure whether skill, context, workflow, model, or budget changes improve
  behavior relative to a frozen baseline.
- Let Steward detect and propose tuning work without allowing autonomous
  mutation of canonical production skills.

## Non-Goals

- Add new agent IDs or reviewer disciplines.
- Add a runtime feature-management system.
- Let a provider choose its own role, authority order, permissions, or required
  outputs.
- Make untrusted artifacts part of system instructions.
- Automatically merge Steward-generated prompt, skill, model, or catalog
  changes.
- Run live provider benchmarks in every ordinary pull-request gate.
- Rewrite active frozen runs to use the new context format.

## Stabilization-Freeze Alignment

The roadmap freeze prohibits speculative context experiments and new agent
roles. This design is a correctness migration for existing production agents,
not a new role family or an optional experiment.

- Existing agent IDs and workflow ownership remain unchanged.
- The catalog gains typed role charters for those existing IDs.
- The compiler and executor gain one mandatory context contract.
- Candidate tuning is evaluated outside production before promotion.
- No runtime switch can select the old path for a newly compiled production
  run.

## Core Model

### AuthorityChainV1

The immutable `RunPlanSnapshot` contains only the compile-time base authority
events and `base_authority_chain_head`. Post-start authority does not mutate or
replace that snapshot. The Rust control plane owns a separate append-only
durable overlay in SQLite for accepted mid-run directives and approval-derived
authority events.

Every base or overlay entry is an `AuthorityEventV1` containing:

- `schema_version = authority_event_v1` and `authority_event_id`;
- `run_id` and previous chain head;
- `source_kind`, limited to `operator_idea`, `operator_directive`,
  `approval_decision`, or `approved_proposal`;
- one closed `AuthorityDirectiveV1` value and its semantic digest;
- durable source record identifier, optional bounded anchor within that record,
  source ordinal, exact source digest, and revision identifier;
- sorted unique `supersedes` authority event IDs;
- creation and acceptance timestamps;
- authenticated principal ID, principal class, capability, and live principal
  table revision used for admission;
- `event_sha256`, which binds the canonical event bytes and previous head and is
  itself the resulting authority-chain head.

Review reports, implementation artifacts, tool output, provider prose, and
workspace files are evidence, not authority events. They cannot silently
supersede operator intent or an accepted approval decision.

The overlay uses two authoritative tables:

- `run_authority_events`, an append-only event ledger whose first overlay event
  points to the frozen `base_authority_chain_head`;
- `run_authority_heads`, one current head and monotonic `context_revision` per
  run.

GraphQL and filesystem artifacts are readback only. They cannot update either
table.

#### Genesis and BaseAuthorityLedgerV1

The chain does not start from an empty string or an implementation-defined
constant. The compiler constructs one `AuthorityGenesisV1`:

```json
{
  "schema_version": "authority_genesis_v1",
  "run_id": "...",
  "idea_id": "...",
  "workflow_snapshot_sha256": "sha256:...",
  "catalog_snapshot_sha256": "sha256:...",
  "run_context_binding_sha256": "sha256:...",
  "genesis_sha256": "sha256:..."
}
```

`genesis_sha256` is the domain-separated canonical digest of those fields
without itself. It is the previous head for the first base event. It does not
include the complete `RunPlanSnapshot` or `RunMissionV1`, so neither digest is
circular.

The frozen snapshot contains one `BaseAuthorityLedgerV1` with
`schema_version`, the complete `AuthorityGenesisV1`, an ordered `events` array,
`base_authority_chain_head`, and `base_ledger_sha256`. The base event order is
normative: `operator_idea` precedes `approved_proposal`; within each source,
events preserve the source adapter's zero-based `source_ordinal`; ties are
invalid rather than sorted heuristically. Every event's `previous_chain_head`
equals genesis or the immediately preceding event hash. The base head is the
last event hash, or genesis when the array is empty.

Base source adapters are versioned and deterministic. They emit a closed
mapping receipt from each source span to directive, conflict key, and ordinal.
The compiler system identity, `RunPlanCompile` capability, source record,
digest, revision, and adapter version are persisted on each base event. Rust
and Swift readers verify genesis, event order, every link, and the ledger
digest before exposing effective authority.

The `runs.start` transaction inserts the Run, frozen base ledger, and exactly
one `run_authority_heads` row with `current_head =
base_authority_chain_head`, `context_revision = 0`, and the base-ledger digest.
It also commits the command outcome and projection intent. A run cannot exist
without this head row, and an overlay append cannot occur before that
transaction commits.

#### AuthorityDirectiveV1

Authority is not an arbitrary JSON document. `AuthorityDirectiveV1` is a
closed, unknown-field-denying tagged object:

```json
{
  "schema_version": "authority_directive_v1",
  "directive_kind": "execution_constraint",
  "conflict_key": "constraint.feature.p095.activation",
  "enforcement": "hard",
  "value": {
    "value_type": "boolean",
    "boolean_value": true
  },
  "directive_sha256": "sha256:..."
}
```

`directive_kind` is exactly one of `objective`, `scope`, `success_criterion`,
`execution_constraint`, or `priority`. `enforcement` is `hard` or `advisory`.
`conflict_key` is 1-128 lowercase ASCII characters matching
`[a-z0-9][a-z0-9._:-]*` and identifies the single logical authority slot that
the directive writes. Include/exclude forms for the same scope subject use the
same key, so they cannot coexist accidentally.

`value` is tagged by `value_type` and contains exactly one of:

- `boolean_value`, a JSON Boolean;
- `integer_value`, an interoperable JSON integer;
- `string_value`, 1-4096 UTF-8 bytes after decoding;
- `string_list_value`, 1-64 strings of at most 512 UTF-8 bytes each, with
  duplicates rejected and order always preserved;
- `scope_value`, a closed `{ "disposition": "include" | "exclude",
  "subject": string }` object whose subject is at most 512 UTF-8 bytes.

The complete canonical directive is capped at 8 KiB. Nulls, unknown fields,
unknown enums, non-integer numbers, empty values, duplicate keys, and fields for
a different `value_type` are invalid. `directive_sha256` uses the canonical
digest contract below over only `schema_version`, `directive_kind`,
`conflict_key`, `enforcement`, and `value`; provenance belongs to the enclosing
event. It is therefore a stable semantic digest distinct from the event hash
and authority-chain head.

The value-kind matrix is closed:

| `directive_kind` | Allowed `value_type` | Required conflict-key prefix |
|---|---|---|
| `objective` | `string` | `objective.` |
| `scope` | `scope` | `scope.` |
| `success_criterion` | `string` or `boolean` | `success.` |
| `execution_constraint` | `boolean`, `integer`, `string`, or `string_list` | `constraint.` |
| `priority` | `integer` | `priority.` |

There are no implicit cross-key conflicts in V1. Source adapters must map one
logical subject to one stable key under the required prefix; normative source
mapping fixtures cover Idea, proposal, operator-directive, and approval inputs.
Different active values for one key are the complete V1 definition of a
contradiction. This avoids locale-, provider-, or prose-dependent conflict
inference.

For each `conflict_key`, zero or one event may be active. A first write requires
an empty `supersedes` list. Replacing a value requires `supersedes` to contain
exactly the currently active event for that key. An event cannot supersede an
inactive event or an event with another key. The closed directive-kind invariant
table validates value type and key prefix. Free-text semantic conflict judgement
is never delegated to a provider.

#### Authority append command

The only external mutation is operator-only MCP `runs.authority.append`. It is
not exposed as a GraphQL mutation. The command requires:

```json
{
  "schema_version": "run_authority_append_request_v1",
  "idempotency_key": "lowercase UUIDv7",
  "run_id": "...",
  "caller_request_id": "lowercase UUIDv4",
  "expected_authority_chain_head": "sha256:...",
  "directive": {
    "schema_version": "authority_directive_v1",
    "directive_kind": "execution_constraint",
    "conflict_key": "constraint.feature.p095.activation",
    "enforcement": "hard",
    "value": { "value_type": "boolean", "boolean_value": true },
    "directive_sha256": "sha256:..."
  },
  "supersedes": []
}
```

The MCP adapter stamps `source_kind = operator_directive` and the durable
command-journal source record; neither is accepted from request JSON. Both
external directives and approval-derived events enter one Rust
`AuthorityAdmissionService`; repositories expose no lower-level public append.
The service reads the live principal table and applies this closed authorization
matrix before revealing run or head existence:

| Source | Required caller | Required capability | Additional provenance |
|---|---|---|---|
| `operator_directive` | authenticated `PrincipalClass::Operator` | `RunsAuthorityAppend` | MCP tool must be `runs.authority.append` and caller class is boundary-derived |
| `approval_decision` | authenticated `PrincipalClass::Operator` that settled the approval | `ApprovalsResolve` | non-forgeable settled approval ID, decision, and settlement journal ID |
| `operator_idea` or `approved_proposal` base event | run compiler system identity | `RunPlanCompile` | frozen source digest and compile journal ID |

`approval_decision` is not a second write path. Approval settlement calls the
same admission service with an engine-issued transaction capability that is
bound to the authenticated approval command, principal, run, approval, and
decision. The authority append, approval state transition, command outcome, and
audit rows commit in one SQLite transaction. A caller-supplied source kind,
principal ID, capability, approval ID, or system identity is ignored or rejected
rather than trusted.

#### ApprovalAuthorityBindingV1

An authority-bearing approval freezes one `ApprovalAuthorityTemplateV1` and its
first `ApprovalAuthorityBindingV1` in the same transaction that creates the
approval. The template binds approval ID, run ID, approval schema, presentation
payload digest, and a closed decision map independent of an authority head.
Every allowed decision maps either to one complete canonical
`AuthorityDirectiveV1` plus its conflict key, or to the explicit value
`no_authority_effect`. The template is immutable and has
`approval_authority_template_sha256`; approval prose is never reparsed during
binding or settlement.

Each binding is a durable generation that adds canonical decimal-string
`binding_generation`, the authority head observed at presentation, the exact
active event each authority-bearing decision would supersede, lifecycle state,
predecessor binding digest, presentation revision, and
`approval_authority_binding_sha256`. The decision map, heads, predecessor and
presentation revision never change after insertion. Lifecycle state is exactly
`presented`, `rebound`, `stale`, `superseded`, or `settled` and changes only by
CAS; each state transition retains the prior binding digest in an append-only
lifecycle event before storing the digest for the new state. Only the current
generation in `presented | rebound` is actionable.

A run may have at most 128 pending authority-bound approvals. Approval creation
that would exceed the cap fails before presentation. The indexed set processed
by an authority append is therefore bounded, and the append transaction must
visit exactly the recorded pending count before it can commit.

Every authority-head append transaction processes all still-pending bound
approvals for that run before it commits. For each approval it marks the prior
current generation `stale` and inserts exactly one successor under a unique
`(approval_id, authority_chain_head)` constraint:

- when all conflict keys referenced by the decision map retained the same
  active event, the successor is `rebound`, keeps the template, binds the new
  head, and remains actionable;
- when any referenced active event changed, the successor is `superseded`,
  contains no actionable decision bindings, sets the approval to
  `reissue_required`, and cannot settle or admit work.

Operator-only `approvals.reissue` is the sole transition from the current
`superseded` generation. It requires the expected generation/digest and current
authority head, rereads the immutable template, resolves every decision against
the current active set, and inserts one next `rebound` generation with an
incremented presentation revision. Reissue does not approve, reject, append
authority, or enqueue work; it only republishes the now-current consequences for
operator review. A concurrent head change or duplicate reissue allows one CAS
winner. The loser receives replay or a stale-binding error and never creates a
second current generation.

`approvals.resolve` for a bound approval requires the lowercase UUIDv4 caller
request ID, approval ID, decision, `binding_generation`,
`approval_authority_binding_sha256`, and `expected_authority_chain_head`. The
generation must be current and `presented | rebound`; its head must equal both
current SQLite truth and the head required by the selected mapping. The
internal transaction capability carries the authenticated principal and live
principal table revision from the boundary. In one transaction the service
validates the binding, performs the authority-head CAS, appends the derived
event when the mapping is authority-bearing, marks the binding `settled`,
settles the approval, commits command outcome, audit and projection rows, and
enqueues any resulting work through the execution-admission predicate. Any
failure changes none of those rows. An unbound, stale, superseded, or
`no_authority_effect` decision cannot append an event.

MCP `approvals.authority.get` and the GraphQL `Approval.authorityBinding` field
return the same `ApprovalAuthorityReadbackV1`: approval/template IDs and
digests, current binding generation/digest/state, presented and current heads,
presentation revision, sorted allowed decisions, `reissue_required`, and
projection health. They expose no directive value or inactive binding history.
Both surfaces are Operator-only; existing non-Operator approval/report
redaction remains unchanged. The ordinary approval list includes only bounded
current-binding summary fields so the UI can submit resolve or reissue without
guessing a head or digest.

The semantic intent hash binds `run_id`, expected head, the adapter-derived
source kind, `directive_sha256`, and the sorted `supersedes` set; it excludes
both transport and caller request IDs.
`caller_request_id` uses the existing `command_idempotency` and
`command_request_aliases` contract. The closed precedence is:

1. the same principal/request ID and same intent returns the stored response;
2. the same principal/request ID with different intent fails with
   `command_idempotency_conflict`;
3. a different request ID for the same principal, command, and complete intent
   replays the committed outcome and records a request alias;
4. absent a replay, a stale expected head fails with
   `authority_head_conflict`;
5. at the current head, a directive digest that is already active and does not
   perform the required replacement fails with `authority_duplicate_event`;
6. concurrent appends against one head allow exactly one complete transaction
   to commit; the loser observes replay, head conflict, or in-flight status
   according to the preceding rows.

Thus repeating the original request with its original expected head is an
alias replay, while submitting the same directive again against the new current
head is a semantic duplicate. Append event, head CAS, command outcome, audit,
and projection intent commit in one SQLite transaction.

An append that violates the per-key supersession rule or produces two active
values for one conflict key fails with
`agent_context_authority_conflict`. Rust and Swift reconstruct the active set by
verifying the hash chain, applying events in chain order, and replacing only the
declared conflict key. Shared vectors make the resulting ordered active-event
IDs and effective authority bytes normative.

#### Per-invocation pinning

Every `AgentExecution` owns one durable `InvocationDispatchIntentV2`. Its
`prepared` row pins these fields before any provider process can exist:

- run ID, stage/task ID, agent execution ID, attempt generation, and logical
  invocation idempotency key;
- immutable run snapshot hash, run context generation, candidate manifest and
  promotion receipt digests, and base authority head;
- overlay `authority_chain_head` and `context_revision`;
- effective mission digest;
- role, assignment, ordered aggregate skill, prompt-envelope manifest, and exact
  spooled prompt-byte digests;
- provider profile, adapter, model, effort, permission profile, and session
  binding fingerprint;
- dispatch state, lease owner/epoch/deadline, and bounded process/session
  identity fields.

Preparation is the invocation's authority linearization point. In one SQLite
transaction the worker claims the work item, reads the current authority head,
verifies the run-generation binding, materializes all context references, and
inserts the immutable execution pin plus `prepared` dispatch intent. If the head
or generation CAS loses, no pin is written and preparation restarts from current
truth. An authority append committed after `prepared` affects only later
invocations; it does not rewrite or invalidate the prepared one. A queued work
item has no pin and therefore observes the latest head when preparation begins.

The dispatch state machine is:

```text
prepared -> launching -> provider_bound -> prompt_committed -> observing -> settled
prepared|launching|provider_bound -> cancelled|failed_closed|identity_ambiguous_hold
prompt_committed|observing -> settled|reconciliation_required
reconciliation_required -> settled|failed_closed
identity_ambiguous_hold -> failed_closed
```

Each transition is a compare-and-swap over execution ID, attempt generation,
current state, and lease epoch. `identity_ambiguous_hold` is terminal for the
attempt and blocks work-item reclaim, retry, provider resurrection, skill-script
execution, and side-effect preparation for that execution until verified
clearance. It cannot expire on a timer.

Entering that state atomically writes `InvocationIdentityHoldV1` with run,
execution and attempt, provider-session ID and cancellation epoch, daemon and
supervisor instance IDs, last verified process fingerprint, bounded ambiguity
reason, required clearance action, created revision/time, state `held`, and
`dispatch_hold_sha256`. The object contains no launch credential or raw process
environment. It is immutable; clearance writes a linked settlement row rather
than editing the held evidence.

Before a worker can claim production work, the daemon writes one
`RuntimeSelfAdmissionReceiptV1` for its daemon instance. The receipt binds
daemon source/build/binary digest, executable canonical identity, PID/UID/birth
fingerprint, compiler/renderer/schema/tool-policy digests, and the candidate
manifest selected by the cutover generation. Startup compares every observed
identity with the promoted manifest. A mismatch enters the generation's durable
`emergency_hold_v2`; that daemon instance cannot claim or dispatch production
work. The execution-admission predicate rechecks the admitted daemon-instance
ID in every prepare and dispatch transaction.

The trusted supervisor allocates a logical provider-session row and
cancellation epoch before spawning. It generates a random 256-bit launch
credential from the OS CSPRNG, stores only its domain-separated hash with
execution ID, attempt generation, daemon instance, and a ten-second expiry, and
passes the plaintext once through an anonymous inherited pipe. It never appears
in argv, environment, files, logs, receipts, crash reports, or provider input.
The trusted wrapper consumes and closes the pipe before provider `exec`, then
atomically spends the credential while registering PID, process group, UID,
process-birth fingerprint, wrapper binary digest, and launch token hash against
the `launching` row. Reuse, wrong binding, or expiry fails registration and the
wrapper exits without executing the provider.

After provider initialization, the trusted adapter writes an
`ObservedProviderBindingReceiptV1`. It binds the dispatch pin and actual daemon
instance, wrapper and provider executable digests/versions, adapter build,
provider session ID, request transport, model, effort, permission profile,
MCP/tool inventory digest, runtime-home digest, and handshake transcript digest.
The receipt contains bounded identities, not credentials or raw transcript.
`provider_bound` commits only when the receipt exactly matches both the
candidate manifest and dispatch pin. An adapter that cannot prove an actual
required identity is unsupported for V2. Mismatch requires verified process
group termination; ambiguous cleanup enters `identity_ambiguous_hold`.
Provider prompt transport cannot start before `provider_bound` commits.

The engine commits `prompt_committed` with request-turn ID, prompt marker, and
provider-session binding immediately before the one allowed prompt send. A
crash from that point onward never causes blind resend. Recovery attaches using
the pinned provider/session/turn correlation when supported; otherwise it
settles `reconciliation_required` and requires typed operator recovery. Late
output from a cancelled or superseded generation is quarantined under the
existing execution-truth rules.

Crash outcomes are normative:

| Durable observation after restart | Recovery action |
|---|---|
| no dispatch row | no provider launch occurred through the V2 path; normal work-item reclaim may prepare once |
| `prepared` | reacquire lease and launch the same immutable pin |
| `launching` without a registered wrapper | inspect the supervisor spawn receipt; verified absence or verified reap settles `failed_closed`, while any ownership ambiguity enters `identity_ambiguous_hold` |
| `launching` or `provider_bound` with verified process and no prompt commit | terminate/reap it, then create a new attempt generation from current authority |
| `prompt_committed` or later | attach/reconcile by pinned turn identity; never send the prompt again |
| `identity_ambiguous_hold` or cleanup unverified | no retry or launch; require verified process-absence clearance |

Every launch attempt already has a provider-session ID and cancellation epoch,
so clearance reuses the existing Operator-only
`provider_session.mark_process_absent` contract and its P083 process-identity
policy. The command must reference the held session/epoch and current dispatch
hold digest. Its transaction marks process fate `absent_verified`, records the
operator and evidence digest, settles the held dispatch `failed_closed`, and
permits creation of a new attempt generation. It never resumes or reuses the
ambiguous process. Exact replay is idempotent; a different or stale hold cannot
clear the attempt.

The authority head is part of the provider-session binding fingerprint. A head
change prevents reuse or resurrection of a session that was primed with an
older mission. Historical readback and deterministic reconstruction use the
head stored on the execution, never the run's current head.

### RunMissionV1

`RunMissionV1` is the immutable compile-time base mission stored in the frozen
`RunPlanSnapshot`. `EffectiveRunMissionV1` is the per-invocation projection of
that base plus the pinned durable overlay head. The base object is never
rewritten when an overlay event is accepted.

Required fields:

```json
{
  "schema_version": "run_mission_v1",
  "run_id": "...",
  "objective": "...",
  "authoritative_directives": [],
  "in_scope": [],
  "non_goals": [],
  "success_criteria": [],
  "base_authority_chain_head": "...",
  "source_refs": [],
  "content_sha256": "sha256:..."
}
```

The effective object adds `authority_chain_head`, `context_revision`, and
`base_mission_sha256`. Its digest and the exact overlay event IDs used to build
it are stored on the execution.

The original operator Idea title and body remain available verbatim as a
frozen source. A normalized mission may improve readability, but it is never
allowed to erase or weaken the verbatim operator directives. Coverage checks
bind every normalized hard constraint to a source anchor.

### RunContextGenerationBindingV2

Every new production run stores one immutable
`RunContextGenerationBindingV2` inside its `RunPlanSnapshot`. It contains the
cutover generation, `AgentContextCandidateManifestV1` digest, promotion receipt
digest, workflow and catalog snapshot digests, skill-registry and ordered
resolved-skill digests, context compiler and prompt builder identities, and all
required wire-version discriminators.

`runs.start` V2 requires an `AgentContextStartSelectionV2` with the caller's
expected cutover generation and every promoted digest. Before the database
transaction, the Rust compiler reads workflow/catalog bytes from opened
descriptors, publishes referenced skill snapshots, and computes the candidate
binding without trusting path names after open. The transaction then:

1. verifies the live marker is `open_v2`, its generation equals the expected
   generation, and all supplied digests equal the promoted candidate;
2. verifies the compiled workflow, catalog, assignment templates, and skills
   are exact members of that candidate manifest; run-specific mission, input,
   blocker, and artifact generations are then bound separately in the frozen
   snapshot and invocation pins;
3. inserts the Run, frozen snapshot, generation binding, command-idempotency
   outcome, and run-start audit row atomically.

No run row exists before step 3 commits. A lost generation/digest CAS leaves
only unreferenced content-addressed files eligible for bounded cleanup. Exact
request replay returns the original run; a new request cannot reinterpret old
bytes under a newer generation. Promotion and `runs.start` serialize on the
same cutover row, so one commits first and the other either observes that exact
generation or fails without creating a mixed snapshot.

### AgentRoleV1

Each existing production agent receives a role charter in the catalog:

```yaml
role:
  mission: Implement the approved code-owned scope and leave verifiable evidence.
  owns:
    - production source changes
    - code-owned tests
    - implementation progress and self-assessment
  does_not_own:
    - proposal approval
    - documentation closeout
    - commit, push, publish, or release decisions
  success_criteria:
    - required code-owned scope is implemented
    - focused verification is reported truthfully
    - remaining work is classified by the correct owner
```

Role data is not a skill. It describes durable organizational responsibility,
not a procedure for completing a particular class of task. The compiled wire
object adds `schema_version = agent_role_v1`, `agent_id`, and `role_sha256` to
the five YAML fields shown above.

### TaskAssignmentV1

Each invocation receives a compiled assignment derived from the frozen
workflow state, task declaration, role charter, active artifact generations,
and output contracts.

Required fields:

```json
{
  "schema_version": "task_assignment_v1",
  "state_id": "state_7_implementation_started",
  "state_purpose": "Implement the human-approved proposal revision.",
  "task_name": "implement_approved_proposal",
  "task_purpose": "Close current code-owned work without broadening scope.",
  "done_when": [],
  "upstream_inputs": [],
  "downstream_consumers": [],
  "active_blockers": [],
  "required_outputs": [],
  "permission_profile_id": "CODE_WRITE",
  "ordered_skill_ids": ["code-implementation"],
  "assignment_sha256": "sha256:..."
}
```

Every referenced input and blocker includes its active generation and digest.
Superseded or stale artifacts can be shown as history but cannot appear as
active assignment truth.

### Workflow DSL Additions

Workflow states gain required production metadata:

```yaml
state_7_implementation_started:
  label: Implementation
  purpose: Implement the approved proposal revision in the dedicated worktree.
  outcome: Code-owned scope is implemented and verification evidence is current.
  owner: lead_orchestrator
```

Agent tasks gain:

```yaml
- agent: code_writer
  task: implement_approved_proposal
  purpose: Close current code-owned implementation and test blockers only.
  done_when:
    - implementation outputs satisfy their declared contracts
    - code-owned blockers are either closed or reported with evidence
```

The Rust and Swift workflow definitions, validators, compilers, inspectors, and
snapshot serializers must remain schema-compatible.

### PromptEnvelopeV2

The engine serializes one provider-neutral envelope in this order:

1. runtime and security invariants;
2. active authority chain and global mission;
3. role charter;
4. task assignment;
5. active skill bodies and brokered logical resource manifest;
6. input artifacts clearly fenced as untrusted data;
7. required outputs and validation contracts;
8. runtime invocation identifiers and freshness rules.

Mission, role, and assignment are protected sections. Artifact materialization
cannot displace or truncate them. Large artifacts are referenced by canonical
path and digest instead of being copied wholesale into the initial prompt.

Prompt diagnostics persist section byte counts, total byte count, context
version, and section digests. Raw sensitive content is not copied into metrics
or ordinary operator logs. `PromptEnvelopeManifestV2` is out-of-band evidence:
it references the rendered prompt bytes but is not embedded in those bytes, so
its exact-prompt digest is not circular.

## Version Discrimination and Wire Contract

The context-envelope version is independent of the existing
`run_plan_snapshot_format_version`. The latter continues to describe P066
catalog/toolchain-cache compatibility and is not renamed or overloaded.
`RunPlanSnapshot` gains the optional string field
`agent_context_envelope_schema` with this exact mapping:

| Existing `run_plan_snapshot_format_version` | `agent_context_envelope_schema` | Reader behavior |
|---|---|---|
| absent | absent | legacy pre-P066 snapshot; historical readback only after cutover |
| `1` | absent | P066 snapshot with no typed context; historical readback only after cutover |
| absent or `1` | `agent_context_envelope_v2` | typed V2 context reader and execution admission |
| any supported value | unknown non-null string | preserve bounded raw metadata but reject provider execution with `agent_context_version_incompatible` |
| unsupported non-null plan format | any | retain the existing unsupported-plan-format rejection |

A new production snapshot after cutover must contain
`agent_context_envelope_schema = agent_context_envelope_v2`. Absence never
defaults to V2. No numeric value, field presence heuristic, catalog field, or
prompt content may select a context reader.

### Normative Schemas

The implementation adds machine-readable JSON Schemas under
`docs/reference/agent-context/schemas/` and shared canonical-byte vectors under
`docs/reference/agent-context/vectors/`. These files, versioned with the daemon,
are the normative wire source for hand-written Rust and Swift types. Every
schema sets `additionalProperties: false`, lists every required field, and uses
an exact `const` discriminator. The parity gate verifies that both runtimes
accept and reject the same vector set; neither runtime-generated schema is the
source of truth.

Wire rules are uniform:

- field names are lowercase `snake_case` and case-sensitive;
- all fields are required unless the normative schema marks them optional;
- an optional absent value is omitted; JSON `null` is forbidden everywhere;
- required empty arrays remain present; no decoder synthesizes a default;
- enum strings are closed lowercase ASCII values and unknown values fail;
- IDs use their existing canonical wire form; command request IDs retain their
  existing per-command UUID version;
- hashed objects require their self-digest field on the wire but omit only that
  named field while computing it;
- timestamps, integers, ordering, duplicate-key rejection, and Unicode follow
  `chainworks_canonical_digest_v1` below.

The schema inventory is:

| Schema discriminator | Required payload fields | Self-digest field |
|---|---|---|
| `authority_genesis_v1` | run/idea IDs, workflow/catalog/binding digests | `genesis_sha256` |
| `base_authority_ledger_v1` | genesis, ordered base events, base head | `base_ledger_sha256` |
| `authority_directive_v1` | `directive_kind`, `conflict_key`, `enforcement`, tagged `value` | `directive_sha256` |
| `authority_event_v1` | event/run IDs, source kind/ref/anchor/ordinal/digest/revision, directive, prior head, supersedes, accepted principal/capability/table revision, timestamps | `event_sha256` |
| `approval_authority_template_v1` | approval/run/schema/presentation digest and head-independent decision map | `approval_authority_template_sha256` |
| `approval_authority_binding_v1` | approval/template IDs, binding generation/state, presented head, resolved decision map, predecessor and presentation revision | `approval_authority_binding_sha256` |
| `approval_authority_binding_event_v1` | approval/generation, prior/new state and digests, cause command/head and timestamp | `binding_event_sha256` |
| `approval_authority_readback_v1` | bounded current binding/template/head/actionability and projection health | none; readback schema |
| `run_mission_v1` | run ID, objective, authoritative directives, scope, non-goals, success criteria, base head, source refs | `content_sha256` |
| `effective_run_mission_v1` | base mission digest, overlay head/revision, ordered active event IDs, effective objective/scope/non-goals/success criteria | `effective_mission_sha256` |
| `agent_role_v1` | agent ID, mission, owns, does-not-own, success criteria | `role_sha256` |
| `task_assignment_v1` | state/task IDs and purposes, done-when, typed input/blocker/output refs, consumers, permission profile ID, ordered skill IDs | `assignment_sha256` |
| `skill_bundle_manifest_v1` | skill ID/name/spec revision, immutable source tree, sorted files and totals | `bundle_sha256` |
| `skill_composition_v1` | ordered entries and aggregate resource totals | `skill_composition_sha256` |
| `prompt_envelope_manifest_v2` | ordered section names/digests/byte counts, exact prompt byte count, context generation and all protected component digests | `prompt_envelope_sha256` |
| `invocation_dispatch_intent_v2` | execution/attempt/work, context/provider/session pins, lease/process and dispatch state | `dispatch_intent_sha256` |
| `invocation_identity_hold_v1` | execution/attempt/session/epoch, process evidence, ambiguity reason and clearance action | `dispatch_hold_sha256` |
| `runtime_self_admission_receipt_v1` | daemon instance/process/build and observed candidate identities | `self_admission_sha256` |
| `observed_provider_binding_receipt_v1` | dispatch, process, provider/model/tool/session and handshake identities | `provider_binding_sha256` |
| `skill_broker_grant_v1` | invocation/attempt/session/generation/composition scope and expiry | `grant_sha256` |
| `skill_script_execution_contract_v1` | script/input/sandbox/runtime/side-effect/retry policy | `script_contract_sha256` |
| `skill_script_execution_intent_v1` | grant/request/script/input/contract/sandbox and side-effect identity | `script_intent_sha256` |
| `skill_script_apply_journal_v1` | ordered write entries, pre/post/staging digests, directory capabilities and apply states | `apply_journal_sha256` |
| `skill_script_execution_receipt_v1` | grant/request/process/sandbox/output/staging/apply/cancellation evidence | `script_receipt_sha256` |
| `legacy_skill_migration_manifest_v1` | source revision/nodes, exact fragment partition, dispositions, modes/effects/failure policies and targets | `legacy_skill_migration_sha256` |
| `agent_context_candidate_manifest_v1` | complete behavior candidate identities defined below | `candidate_sha256` |
| `agent_quality_promotion_receipt_v1` | candidate/baseline/policy/suite identities, pair receipts, metrics, bounds, resource ratios, decision | `receipt_sha256` |
| `agent_context_probe_receipt_v1` | probe epoch/run/generation/candidate, observed identities, outcome and evidence | `probe_receipt_sha256` |
| `agent_context_proof_admission_v1` | admission/generation/candidate IDs, state, mint/consume command and proof run/epoch | `proof_admission_sha256` |
| `process_clearance_set_v1` | generation, sorted held sessions/epochs/hold and clearance digests, completeness revision | `process_clearance_set_sha256` |
| `agent_context_migration_phase_event_v1` | prior phase/head, migration version/checksum, daemon/command/invariant identities and timestamp | `migration_event_sha256` |
| `legacy_cutover_inventory_v1` | producer/process/effect/approval/retry inventory, settlement actions, blockers and freshness | `cutover_inventory_sha256` |
| `legacy_execution_settlement_v1` | run, prior state, retired work/approval/retry rows and preserved ledger digests | `settlement_sha256` |
| `agent_context_projection_intent_v1` | projection/entity, canonical source revision/head/payload and target/status | `projection_intent_sha256` |
| `mutation_projection_receipt_v1` | sorted projection intents, canonical revisions/heads and commit-time status | `projection_receipt_sha256` |
| `agent_context_cutover_readback_v2` | singleton cutover/migration/generation/proof/process-clearance/version/projection truth | none; readback schema |
| `agent_context_cursor_v1` | cursor scope/run/list, snapshot revision, expiry, last ordering tuple | none; opaque readback cursor |
| `agent_context_command_error_v1` | classification, retryability/mode, safe request and expected/current fields | none; error schema |
| `agent_context_start_selection_v2` | cutover revision/generation, versions and selected candidate/workflow/catalog/skill digests | `selection_sha256` |
| `run_context_generation_binding_v2` | cutover generation and every promoted/compiled digest selected by run start | `binding_sha256` |
| `agent_context_envelope_v2` | generation binding, base/effective mission, authority projection, role, assignment, skill composition, input/output refs, runtime IDs | `envelope_sha256` |

All arrays in that table preserve declared order except these schema-declared
sets, which sort by raw canonical key: authority `supersedes` by event ID,
mission source refs by `(source_kind, source_id, revision)`, role ownership and
criteria strings by UTF-8 bytes, skill manifest files by portable path, legacy
source nodes by source-node ID, migration fragments by `fragment_id`, process
clearances by `(provider_session_id, cancellation_epoch)`, and projection
intents by `(projection_kind, entity_id)`. Typed assignment inputs, blockers,
outputs, skill composition, write-journal entries and connection edges remain
ordered because prompt, effect and keyset order are semantic.

### Northbound Mutation and Discovery Contract

All request and result schemas below live in the normative schema directory,
set `additionalProperties: false`, reject null, and cap the complete request at
64 KiB unless an existing lower cap applies. MCP state-changing tools retain
the existing boundary `idempotency_key` as a lowercase UUIDv7. Every new
context mutation, `approvals.reissue`, and the extended process-absence command
also require a lowercase UUIDv4 `caller_request_id` for
the durable semantic command contract. The two IDs are not aliases:

- `idempotency_key` caches one exact MCP tool request, including
  `caller_request_id`;
- `caller_request_id` identifies one durable command lease and participates in
  exact-request/conflict lookup, but is excluded from the semantic intent hash;
  a different caller request ID may therefore enter `command_request_aliases`
  when every command-specific semantic field is equal;
- `runs.start` retains only its existing UUIDv7 `idempotency_key`, which is also
  stored as its command-journal request ID.

The read-only MCP tool `runtime.agent_context.start_selection.get` takes
`agent_context_start_selection_request_v2` with exactly its discriminator and
required `workflow_id`. In one SQLite read transaction it returns
`AgentContextStartSelectionV2` with exactly these fields:

- `schema_version = agent_context_start_selection_v2`, `workflow_id`,
  `cutover_state = open_v2`, canonical decimal-string `cutover_generation` and
  `cutover_revision`;
- `context_schema_version`, `prompt_envelope_schema_version`,
  `role_schema_version`, `assignment_schema_version`,
  `skill_snapshot_schema_version`, `digest_contract_version`, and
  `promotion_policy_version`;
- `candidate_id`, `candidate_sha256`, `promotion_receipt_sha256`,
  `catalog_snapshot_sha256`, and `skill_registry_sha256`;
- `workflow_source_sha256`, `workflow_normalized_plan_sha256`, and ordered
  `assignment_templates`, whose closed entries contain `state_id`, `task_name`,
  and `assignment_template_sha256`;
- sorted unique `required_capability_versions` and `selection_sha256`.

`selection_sha256` covers that complete object without itself. No digest
required by `runs.start` is sourced from another health read or a mutable client
file. The tool returns a selection only while the marker is `open_v2`; other
cutover states return the authorized `-32009` admission envelope.
`runtime.health` advertises this tool and its schema version but is not itself
selection truth.

The exact northbound schemas are:

| Tool | Request schema and required fields | Result schema and required fields |
|---|---|---|
| `runtime.agent_context.start_selection.get` | `agent_context_start_selection_request_v2`: `schema_version`, `workflow_id`; no idempotency field because the tool is read-only | the complete `agent_context_start_selection_v2` object above |
| `runtime.agent_context.cutover.get` | `agent_context_cutover_get_request_v2`: only `schema_version`; read-only | `agent_context_cutover_readback_v2`: cutover/migration state and revision, generation/candidate state, proof-admission state/ID, probe epoch/run/latest receipt, process-clearance status/digest, allowed next actions, required versions, and current projection health |
| `approvals.authority.get` | `approval_authority_get_request_v1`: `schema_version`, `approval_id`; read-only | the complete `approval_authority_readback_v1` object defined above |
| `runs.authority.append` | `run_authority_append_request_v1`: `schema_version`, `idempotency_key`, `caller_request_id`, `run_id`, `expected_authority_chain_head`, `directive`, sorted `supersedes` | `run_authority_append_result_v1`: `status = applied | replayed`, `authority_event_id`, `previous_authority_chain_head`, `current_authority_chain_head`, decimal-string `context_revision`, `directive_sha256`, `canonical_request_id`, `journal_id` |
| `approvals.reissue` | `approval_authority_reissue_request_v1`: `schema_version`, both request IDs, `approval_id`, decimal-string `expected_binding_generation`, `expected_binding_sha256`, `expected_authority_chain_head` | `approval_authority_reissue_result_v1`: `status = rebound | replayed`, `approval_id`, decimal-string `binding_generation` and `presentation_revision`, `approval_authority_binding_sha256`, `binding_state = rebound`, `current_authority_chain_head`, `canonical_request_id`, `journal_id` |
| `approvals.resolve` when authority-bound | `schema_version = approval_resolve_authority_v1`, required `subject_kind = stage_approval`, `approval_id`, `resolution = approve | reject`, optional bounded `comment`, decimal-string `binding_generation`, `approval_authority_binding_sha256`, `expected_authority_chain_head`, `caller_request_id`; MCP also requires `idempotency_key` | `approval_resolve_authority_result_v1`: `status = applied | replayed`, `approval_id`, `resolution`, decimal-string `binding_generation`, `approval_authority_binding_sha256`, optional `authority_event_id`, `current_authority_chain_head`, decimal-string `context_revision`, `canonical_request_id`, `journal_id` |
| `runtime.agent_context.promote` | `agent_context_promote_request_v2`: `schema_version`, both request IDs, `candidate_id`, `candidate_sha256`, `promotion_receipt_sha256`, `expected_candidate_state = live_passed`, decimal-string `expected_cutover_generation`, `expected_cutover_revision` | `agent_context_promote_result_v2`: `status = probe_admitted | replayed`, `candidate_state = probe_active`, `cutover_state = probe_v2`, decimal-string `cutover_generation`, `cutover_revision`, `proof_admission_id`, `proof_admission_state = available`, `generation_row_sha256`, `canonical_request_id`, `journal_id` |
| `runtime.agent_context.probe_start` | `agent_context_probe_start_request_v2`: `schema_version`, both request IDs, decimal-string `expected_cutover_generation`, `expected_cutover_revision`, `candidate_sha256`, `proof_admission_id`, `probe_workflow_sha256` | `agent_context_probe_start_result_v2`: `status = started | replayed`, `proof_admission_id`, `proof_admission_state = consumed`, `proof_run_id`, decimal-string `probe_epoch`, `probe_state = running`, `cutover_generation`, `cutover_revision`, `canonical_request_id`, `journal_id` |
| `runtime.agent_context.reprobe` | `agent_context_reprobe_request_v2`: `schema_version`, both request IDs, decimal-string `expected_cutover_generation`, `expected_cutover_revision`, `candidate_id`, `candidate_sha256`, `expected_candidate_state = probe_active | promoted`, `reprobe_reason = infrastructure_inconclusive | process_hold_cleared`, `expected_probe_epoch`, conditional evidence digest, and `probe_workflow_sha256` | `agent_context_reprobe_result_v2`: `status = started | replayed`, `reprobe_reason`, `candidate_state = probe_active`, `cutover_state = probe_v2`, internally minted-and-consumed `proof_admission_id`, `proof_admission_state = consumed`, `proof_run_id`, decimal-string `probe_epoch`, `canonical_request_id`, `journal_id` |
| `runtime.agent_context.open` | `agent_context_open_request_v2`: `schema_version`, both request IDs, decimal-string `expected_cutover_generation`, `expected_cutover_revision`, `expected_probe_epoch`, `candidate_sha256`, `proof_run_id`, `probe_receipt_sha256` | `agent_context_open_result_v2`: `status = opened | replayed`, `candidate_state = promoted`, `cutover_state = open_v2`, decimal-string `cutover_generation`, `cutover_revision`, `probe_receipt_sha256`, `canonical_request_id`, `journal_id` |
| `provider_session.mark_process_absent` | `schema_version = provider_session_mark_process_absent_v2`, both request IDs, `provider_session_id`, integer `cancellation_epoch`, `dispatch_hold_sha256` | extended P083 result: `status = absent_verified | replayed`, `process_fate = absent_verified`, `agent_execution_id`, integer `attempt_generation`, `dispatch_state = failed_closed`, `cleared_dispatch_hold_sha256`, `generation_reprobe_status = not_required | waiting_for_other_clearances | ready`, optional `process_clearance_set_sha256`, `canonical_request_id`, `journal_id`; it does not mutate candidate/cutover state |
| `runs.start` | `run_start_request_v2`: `schema_version`, `idea_id`, `workflow_id`, `workflow_title`, `workspace_root`, `artifact_root`, `workflow_yaml_path`, `agent_catalog_yaml_path`, UUIDv7 `idempotency_key`, `context_selection`; only `delivery_configuration_json`, `review_routing_json`, and `rollout_contract_preflight_policy_json` are optional | `run_start_result_v2`: `status = started | replayed`, `run_id`, `idea_id`, `workflow_id`, `run_status`, `context_admission`, `canonical_request_id`, `journal_id` |

Every state-changing result schema in this table also requires
`projection_status = pending` and field `projection_receipt` containing one
complete immutable `mutation_projection_receipt_v1`. The receipt contains
exactly its schema version, sorted non-empty projection intents
(`projection_kind`, `entity_id`, `projection_intent_sha256`, canonical source
revision/head), `status_at_commit = pending`, and
`projection_receipt_sha256`. This is commit-time truth, not a live status field:
exact and alias replay return the original bytes even when projection workers
have since applied or degraded an intent. Current status is read only through
SQLite-backed readback surfaces.

For `agent_context_reprobe_request_v2`, `reprobe_reason =
infrastructure_inconclusive` requires `prior_probe_receipt_sha256` and forbids
`process_clearance_set_sha256`; it is valid only from `probe_v2 /
probe_inconclusive` with candidate state `probe_active`.
`process_hold_cleared` requires the complete `process_clearance_set_sha256` from
cutover readback, forbids the prior probe receipt, and is valid only from
`emergency_hold_v2 / emergency_hold` when every unresolved invariant belongs to
that process-only clearance set. The candidate may be `promoted` after a prior
open or already `probe_active` when the hold interrupted a proof epoch. The
service mints and consumes a proof admission inside the same transaction that
creates the proof run. No caller can mint, reuse, or separately consume a
reprobe admission.

The semantic intent hashes are closed. Lookup scope is authenticated principal
ID plus canonical command name; every listed field uses canonical wire bytes.
Neither request ID, transport metadata, projection status, journal ID, response
field, timestamp, or lease metadata is included:

| Command | Complete semantic intent fields |
|---|---|
| `runs.authority.append` | `run_id`, `expected_authority_chain_head`, adapter-derived `source_kind`, `directive_sha256`, sorted `supersedes` |
| `approvals.reissue` | `approval_id`, `expected_binding_generation`, `expected_binding_sha256`, `expected_authority_chain_head` |
| `approvals.resolve` | `approval_id`, `resolution`, canonical digest of present/absent bounded comment, `binding_generation`, `approval_authority_binding_sha256`, `expected_authority_chain_head` |
| `runtime.agent_context.promote` | `candidate_id`, `candidate_sha256`, `promotion_receipt_sha256`, `expected_candidate_state`, `expected_cutover_generation`, `expected_cutover_revision` |
| `runtime.agent_context.probe_start` | `expected_cutover_generation`, `expected_cutover_revision`, `candidate_sha256`, `proof_admission_id`, `probe_workflow_sha256` |
| `runtime.agent_context.reprobe` | `expected_cutover_generation`, `expected_cutover_revision`, `candidate_id`, `candidate_sha256`, `expected_candidate_state`, `reprobe_reason`, `expected_probe_epoch`, the one conditionally required evidence digest, `probe_workflow_sha256` |
| `runtime.agent_context.open` | `expected_cutover_generation`, `expected_cutover_revision`, `expected_probe_epoch`, `candidate_sha256`, `proof_run_id`, `probe_receipt_sha256` |
| `provider_session.mark_process_absent` | `provider_session_id`, integer `cancellation_epoch`, `dispatch_hold_sha256` |

`runs.start` intentionally has no second semantic-alias layer: its existing
UUIDv7 key and canonical MCP request hash are the command identity, and the
single-active-run invariant remains separate. The MCP canonical request hash
excludes only `idempotency_key`; it includes `caller_request_id`, principal,
token, caller class, boundary row and every request field exactly as the current
P081 contract requires. The command semantic hash excludes both request IDs,
which is what makes a different-ID alias replay possible without weakening the
MCP same-key conflict check.

All northbound command errors use a closed `agent_context_command_error_v1`
payload with `classification`, Boolean `retryable`, `retry_mode`, safe request
IDs and the bounded expected/current fields allowed below. Rust and Swift share
these exact outcomes:

| Condition | JSON-RPC/code | Classification | `retryable` / `retry_mode` |
|---|---|---|---|
| live principal/class/capability denial | `-32004` existing boundary code | `authorization_denied` | `false / terminal` |
| schema, bounds or conditional-field failure | `-32602` | `invalid_request` | `false / correct_payload` |
| same MCP key and same hash committed | success, original bytes | `exact_replay` | not applicable |
| same MCP key and same hash pending | `-32603 / IDEMPOTENCY_IN_FLIGHT` | `mcp_request_in_flight` | `true / exact_retry_after` |
| same MCP key and different hash | `-32603 / IDEMPOTENCY_CONFLICT` | `mcp_request_conflict` | `false / new_key_for_new_intent` |
| same caller request ID and same semantic intent committed | success, original bytes | `exact_replay` | not applicable |
| different caller request ID and same semantic intent committed | success, original bytes; alias row recorded separately | `alias_replay` | not applicable |
| same caller request ID and different command/intent | `-32009 / COMMAND_IDEMPOTENCY_CONFLICT` | `command_request_conflict` | `false / new_request_for_new_intent` |
| same semantic lease still pending | `-32009 / COMMAND_IN_FLIGHT` | `command_in_flight` | `true / exact_retry_after` |
| stale head/generation/revision/binding | `-32009 / STALE_PRECONDITION` | `stale_precondition` | `true / refresh_then_new_request` |
| current-head semantic duplicate | `-32009 / SEMANTIC_DUPLICATE` | `semantic_duplicate` | `false / terminal` |
| authorized resource is absent | existing `-32002` | `resource_missing` | `false / correct_identifier` |
| pending authority-bound approval cap reached | `-32009 / PENDING_APPROVAL_CAP_EXCEEDED` | `resource_limit` | `true / settle_existing_then_new_request` |
| malformed or wrong-scope connection cursor | `invalid_cursor / malformed, filter_changed` | `invalid_cursor` | `false / restart_pagination` |
| expired or unavailable cursor snapshot | `invalid_cursor / expired, snapshot_unavailable` | `invalid_cursor` | `true / restart_pagination` |
| required projection-intent write rolls back | `-32603 / PROJECTION_INTENT_COMMIT_FAILED` | `storage_not_committed` | `true / exact_retry` |
| committed outcome or alias row is corrupt | `-32603 / IDEMPOTENCY_REPLAY_CORRUPT` | `durable_truth_corrupt` | `false / operator_repair` |

Concurrent same-intent losers observe in-flight or alias replay according to
whether the winner committed; different-intent CAS losers observe
`stale_precondition`. No race-dependent generic `INTERNAL` outcome is allowed.

All UUID/ID strings are canonical lowercase ASCII and at most 200 bytes unless
their existing domain type is stricter; digests are exactly `sha256:` plus 64
lowercase hex characters. Revisions/generations that can exceed GraphQL/JSON
safe integers use canonical unsigned decimal strings with no sign or leading
zero. Paths retain the existing run-start canonicalization and 4096-byte cap.
Each optional configuration JSON string retains its current 64 KiB decoded cap.
`comment` is at most 4096 UTF-8 bytes. Error `path` is a JSON Pointer of at most
256 ASCII bytes and `reason` is exactly `missing_required`, `unknown_field`,
`wrong_type`, `invalid_format`, `out_of_bounds`, or
`conditional_field_missing`, not arbitrary parser text.

`context_selection` is exactly the complete object returned by the discovery
tool, including `selection_sha256`. The server recomputes and compares it under
the run-start transaction; the client does not edit or omit fields. A successful
`run_start_context_admission_v2` contains exactly `schema_version`, `run_id`,
canonical decimal-string `cutover_generation` and `cutover_revision`,
`candidate_sha256`, `promotion_receipt_sha256`, `workflow_source_sha256`,
`workflow_normalized_plan_sha256`, `catalog_snapshot_sha256`,
`skill_registry_sha256`, `binding_sha256`, `context_schema_version`, and
`base_authority_chain_head`.

The boundary authorization matrix is closed:

| Surface/tool | Required `PrincipalClass` | Required derived `CallerClass` | Required capability |
|---|---|---|---|
| MCP start-selection readback | `Operator` | `AgentOperator` | `RuntimeAgentContextStartSelectionGet` |
| MCP cutover singleton readback | `Operator` | `AgentOperator` | `RuntimeAgentContextCutoverGet` |
| MCP approval-authority readback | `Operator` | `AgentOperator` | `ApprovalsAuthorityRead` |
| MCP `runs.start` | `Operator` | `AgentOperator` | `RunsStart` |
| MCP authority append | `Operator` | `AgentOperator` | `RunsAuthorityAppend` |
| MCP approval reissue | `Operator` | `AgentOperator` | `ApprovalsReissue` |
| MCP promote | `Operator` | `AgentOperator` | `RuntimeAgentContextPromote` |
| MCP initial probe start | `Operator` | `AgentOperator` | `RuntimeAgentContextProbeStart` |
| MCP atomic reprobe | `Operator` | `AgentOperator` | `RuntimeAgentContextReprobe` |
| MCP open | `Operator` | `AgentOperator` | `RuntimeAgentContextOpen` |
| MCP process-absence clearance | `Operator` | `AgentOperator` | `ProviderSessionMarkProcessAbsent` |
| MCP approval resolution | `Operator` | `AgentOperator` | `ApprovalsResolve` |
| GraphQL cutover singleton readback | `Operator` | `UiOperator` | `RuntimeAgentContextCutoverGet` |
| GraphQL approval-authority readback | `Operator` | `UiOperator` | `ApprovalsAuthorityRead` |
| GraphQL approval reissue | `Operator` | `UiOperator` | `ApprovalsReissue` |
| GraphQL approval resolution | `Operator` | `UiOperator` | `ApprovalsResolve` |

These checks are conjunctive. An `Agent` principal also derives
`AgentOperator` on MCP, but fails the required principal class even if its
principal entry was mistakenly granted one of these capabilities. Automation,
Observer, ReadOnlyOperator, DeveloperBreakGlass, caller-class overrides, and
caller-supplied provenance cannot perform these mutations. GraphQL exposes no
run-start, append, promote, probe, reprobe, or open mutation.

Validation precedence is normative and shared by HTTP MCP and stdio MCP:

1. bounded JSON-RPC envelope parsing returns only standard parse/request errors;
2. live bearer principal resolution, principal enabled/revoked state, exact
   principal class, derived caller class, tool provenance, and capability are
   checked before tool-specific schema, run, candidate, or generation lookup;
3. the complete request schema and ID versions are validated; a missing
   `context_selection` is only `-32602`, never a version/state error;
4. path and identifier normalization completes without mutation;
5. the MCP UUIDv7 exact-request idempotency precheck returns cached success,
   `IDEMPOTENCY_IN_FLIGHT`, or `IDEMPOTENCY_CONFLICT` using the existing
   `-32603` structured envelope;
6. the durable UUIDv4 command lease applies exact-request and same-intent alias
   precedence before semantic state checks;
7. authorized semantic existence, expected-state, head, generation, receipt,
   and admission CAS checks execute in one command transaction.

Authorization denial uses the existing non-enumerating `-32004` policy
envelope. All post-authorization failures use the common command payload and
classification table above. Schema errors use `-32602` and add bounded
`agent_context_request_error_v1 { path, reason }` detail. Version, generation,
digest, binding, or cutover-state failures use `-32009`, message `context
admission denied`, and add `agent_context_admission_error_v1` detail with only
safe expected/current generation, revision, state, and schema fields.
Authorized missing resources use existing `-32002` plus classification
`resource_missing`. Optional details are absent rather than null. No error
includes raw paths, bearer material, launch credentials, prompts, directive
values, or evidence bytes.

The same vectors cover every malformed field, unauthorized principal/caller
combination, revoked or re-scoped principal, exact replay, alias replay,
in-flight lease, idempotency conflict, stale CAS, semantic duplicate, and
concurrent loser. Given a valid bounded JSON-RPC envelope, an old authorized app
omitting `context_selection` receives exactly `-32602` after cutover; an
unauthorized caller receives `-32004` regardless of whether its tool payload is
malformed.

## Canonical Bytes and Digest Contract

All cross-runtime structured digests use
`chainworks_canonical_digest_v1`:

1. Construct the contract payload without its self-digest field. The excluded
   field is explicit per schema: `genesis_sha256`, `base_ledger_sha256`,
   `directive_sha256`, `event_sha256`, `approval_authority_template_sha256`,
   `approval_authority_binding_sha256`, `binding_event_sha256`,
   `content_sha256`, `effective_mission_sha256`, `role_sha256`,
   `assignment_sha256`, `bundle_sha256`, `skill_composition_sha256`,
   `prompt_envelope_sha256`, `dispatch_intent_sha256`, `dispatch_hold_sha256`,
   `self_admission_sha256`,
   `provider_binding_sha256`, `grant_sha256`, `script_contract_sha256`,
   `script_intent_sha256`, `apply_journal_sha256`, `script_receipt_sha256`,
   `legacy_skill_migration_sha256`, `candidate_sha256`, `receipt_sha256`,
   `probe_receipt_sha256`, `proof_admission_sha256`,
   `process_clearance_set_sha256`, `migration_event_sha256`,
   `cutover_inventory_sha256`, `settlement_sha256`,
   `projection_intent_sha256`, `projection_receipt_sha256`,
   `selection_sha256`, `binding_sha256`, or `envelope_sha256`.
2. Normalize fields that are semantic sets by sorting them according to the
   schema's declared key before serialization. Arrays that express workflow or
   instruction order retain their declared order.
3. Serialize as RFC 8785 JSON Canonicalization Scheme bytes. Duplicate object
   keys are invalid. Floating-point numbers are prohibited in these contracts;
   JSON numeric fields are integers in the interoperable range
   `-9007199254740991...9007199254740991`. Larger signed or unsigned values use
   schema-declared canonical decimal strings. Timestamps are canonical UTC
   RFC3339 strings with exactly millisecond precision.
4. Preserve the input Unicode scalar sequence exactly. Do not apply NFC, NFD,
   case folding, or locale-sensitive transformation. Canonically equivalent but
   byte-distinct Unicode strings therefore have different digests by design.
5. Hash `domain || 0x00 || canonical_json`, where `domain` is the ASCII schema
   name and version, for example `chainworks:task_assignment_v1`.
6. Encode SHA-256 as lowercase hexadecimal with the `sha256:` prefix.

File-content digests hash exact file bytes with the domain
`chainworks:skill_resource_v1`. The skill bundle digest hashes the canonical
manifest, not a directory walk or host metadata. Authority event hashes also
bind the previous head digest, giving the overlay a verifiable hash chain.

Normative Rust and Swift fixtures include:

- shuffled object insertion order;
- ordered and set-valued arrays;
- composed and decomposed Unicode examples;
- quotes, control characters, slashes, and non-ASCII text;
- minimum and maximum interoperable JSON integers plus decimal-string 64-bit
  extrema;
- every excluded self-digest field;
- every existing legacy snapshot representation retained as opaque bytes.

Neither Foundation `JSONEncoder.sortedKeys` nor ordinary `serde_json`
serialization alone is considered the normative implementation. Both runtimes
must pass the shared canonical-byte fixtures.

## Legacy Snapshot to Context V2 Transition

Snapshots with an absent `agent_context_envelope_schema` remain byte-for-byte
legacy records under the existing plan-format reader selected by the mapping
above. They are never recompiled, normalized, or converted during readback.

| Surface | Legacy context-absent behavior | `agent_context_envelope_v2` behavior |
|---|---|---|
| Run mission | Idea appears only for selected tasks | frozen `RunMissionV1` base plus pinned `EffectiveRunMissionV1` overlay projection |
| Agent `prompt` | arbitrary injected catalog prompt | prohibited in production catalogs; content moves to role, skill, runtime policy, or output contract |
| `skill_ref` | one legacy skill identifier | replaced by catalog `allowed_skills` and task `skills` |
| `skill_role` | optional injected specialization | prohibited |
| `roles/*.md` | may append protected role text | not loaded as role authority; retained only as ordinary reference data when explicitly needed |
| hardcoded role maps | `proposalReviewModeMap` and Rust equivalent may inject role blocks | prohibited and removed from V2 resolution |
| `resolved_skill.injected_content` | precomposed prompt fragment | replaced by `SkillBundleSnapshotRef` plus parsed `SKILL.md` body |
| inline/builtin skills | accepted | rejected for a new production run |
| Role authority | prompt, skill role, and generic specialization may compete | exactly one `AgentRoleV1` protected section |

Migration is an explicit source rewrite of the canonical catalog and workflow.
Each current `skill_role` is translated into an explicit existing-agent role
charter and, where needed, a typed task mode. V2 compilation fails if any legacy
role or prompt field remains. It does not perform a heuristic runtime migration.

## Agent Skills Registry

### Standards Baseline

Bundles follow the official Agent Skills specification and guidance:

- <https://agentskills.io/specification>
- <https://agentskills.io/skill-creation/best-practices>
- <https://agentskills.io/skill-creation/optimizing-descriptions>
- <https://agentskills.io/skill-creation/evaluating-skills>

The repository pins one Agent Skills specification revision and vendors its
official validation corpus. A Rust `SkillBundleAdmissionV1` implementation is
the production admission authority on every host. A host-installed
`skills-ref` executable is never used to decide whether a run can start. Gates
may compare the Rust result with a repository-pinned `skills-ref` build, but a
missing or different host installation cannot change admission truth.

Core `SKILL.md` bodies follow progressive disclosure and target fewer than 500
lines and 5,000 tokens. Longer schemas, rubrics, examples, and templates belong
in bundle resources rather than the injected body.

### Canonical Layout

The repository-owned source is:

```text
.agents/skills/<skill-name>/
|-- SKILL.md
|-- references/
|-- scripts/
`-- assets/

.agent-evals/public/<skill-name>/
|-- evals.json
`-- fixtures/
```

Only files needed by a skill are present. `SKILL.md` contains the trigger
description and core procedure. Detailed schemas, rubrics, and examples move
to `references/`. Deterministic helpers move to `scripts/`. Templates and
static outputs move to `assets/`.

Public trigger and deterministic fixtures live outside the publishable bundle.
Promotion holdouts are not stored in either tree. They live as an encrypted,
content-addressed artifact in the Operator/CI-owned eval vault outside every
provider workspace and are released one case at a time only to the trusted eval
coordinator. A provider sees the current case input, never the holdout index,
other cases, expected labels, or decryption material.

Plugin publication copies are generated from the canonical source and checked
for exact digest parity. Example-only duplicate skill sources are removed.

### Validation

`SkillBundleAdmissionV1` validates the Agent Skills frontmatter and bundle
before creating a run:

- `name` matches the bundle directory and allowed name grammar;
- `description` explains both capability and trigger boundaries;
- unknown or malformed required metadata fails closed;
- `SKILL.md` and resources obey byte and file-count caps;
- absolute paths, traversal, symlink components, and root escape are rejected;
- executable scripts are explicitly inventoried and hashed;
- resource references resolve inside the bundle;
- duplicate canonical skill names are rejected.

Normative limits are:

- `SKILL.md`: at most 128 KiB and 500 lines;
- one resource file: at most 8 MiB;
- one bundle: at most 256 regular files and 32 MiB total exact bytes;
- one portable path: at most 240 ASCII bytes and eight components below the
  skill root;
- one task composition: at most four bundles, 64 MiB aggregate resources, and
  256 KiB aggregate injected `SKILL.md` bodies.

Size accounting includes `SKILL.md`, frontmatter, scripts, and every resource
byte. An executable Git mode is accepted only below `scripts/` and is recorded
in the manifest; executable files elsewhere fail admission.

Portable relative resource paths use the ASCII grammar
`[A-Za-z0-9._/-]+`, contain no empty, `.` or `..` component, and are sorted by
raw ASCII bytes in manifests. Production Git trees accept tree and regular-blob
modes only and reject symlink/submodule modes, path aliases, and case-fold
collisions. Test-only path fixtures additionally reject hard links, sockets,
devices, FIFOs, symlinks, and files that change while being read.

### Coherent Same-Byte Snapshot Publication

A live working-tree directory is never a production admission source because a
portable POSIX walk cannot prove one coherent generation during concurrent
rename/add/remove operations. Every promotable skill is committed first.
`SkillSourceTreeV1` identifies repository identity, commit OID, subtree path,
and exact Git tree OID. Dirty or path-only skill sources are permitted only in
test-only fixture admission and cannot produce a production candidate receipt.

Validation, hashing, copying, and publication operate on one immutable Git
object set through a structured Git object API owned by the Rust control plane,
not provider shell commands:

1. Resolve the declared commit and subtree once, require the expected tree OID,
   and retain object handles for the complete operation.
2. Enumerate that immutable tree recursively in raw portable-path byte order.
   Accept Git tree and blob objects only; reject symlink mode, submodule mode,
   duplicate/case-colliding paths, and any object missing by hash.
3. Read each blob by OID into a bounded staging spool once. Validate exact bytes,
   frontmatter, references, modes, per-file and aggregate limits, and compute
   resource digests from that same spool.
4. Build `SkillBundleManifestV1` with source commit/tree OIDs, normalized paths,
   byte lengths, exact content digests, and normalized modes. Compute
   `bundle_sha256` from its canonical bytes.
5. Write only the spooled bytes into a control-plane-owned temporary directory
   on the same filesystem, fsync files and directories, and perform a
   no-replace atomic rename to the content-addressed final snapshot path.
6. Commit the publication receipt and candidate/run reference only after the
   rename is durable. A database failure leaves an unreferenced
   content-addressed directory for bounded startup cleanup; it never exposes a
   partially published bundle.

If the final content-addressed directory already exists, admission verifies its
complete manifest before reuse. Any mismatch fails closed. Failure cleanup may
remove only the request-owned staging directory and never follows links.

The Swift `ExternalSkillLoader` becomes diagnostic/readback code for V2. It
does not independently admit or publish production bundles. Rust/Swift parity
fixtures consume the same published manifest and bytes, so installed tools,
working-tree races, directory enumeration order, and host filesystem timing
cannot change the decision.

The frontmatter is metadata and is not injected as raw prompt text. The engine
injects the exact admitted `SKILL.md` body from its content-addressed snapshot.
Providers cannot read `.agents/skills`, the Git object database, the eval vault,
or snapshot-storage paths. Resources are exposed as digest-bearing
`skill://<bundle-sha256>/<relative-path>` references through bounded runtime
tools `skills.resource.read` and, when the permission profile allows execution,
`skills.script.run`. Those tools reopen only the immutable object by digest,
verify the manifest entry and byte hash, cap output, and never resolve a mutable
authoring path. `skills.resource.read` is limited by the bundle manifest and the
P096 per-call/cumulative output budgets; `skills.script.run` is also subject to
the permission profile, runtime timeout, and P096 line/byte caps.

### Invocation-Scoped Skill Broker

Preparation creates one durable `SkillBrokerGrantV1` in the same transaction as
the invocation pin. It binds run, stage/task, agent execution, attempt
generation, daemon instance, provider session and session fingerprint, context
generation, permission profile, `SkillCompositionV1` digest, the exact allowed
resource/script manifest entries, dispatch states in which the grant is usable,
expiry, and revocation state. A 256-bit capability token is generated by the OS
CSPRNG; only its hash is stored. The trusted adapter installs the plaintext in
the isolated provider MCP transport, never in prompt text, argv, logs, receipts,
or workspace files. Settlement, cancellation, session replacement, generation
hold, or attempt replacement revokes it transactionally.

Broker tool payloads do not accept run ID, execution ID, attempt generation,
session ID, snapshot path, or bundle path. The authenticated grant and live ACP
session supply those values. `skills.resource.read` accepts only the logical
`skill://` URI and a bounded byte range. `skills.script.run` accepts only the
logical script URI, one contract-declared input object, and a UUIDv7
`idempotency_key`. Each call verifies token hash, active session fingerprint,
dispatch state, generation continuation state, permission profile, composition
membership, manifest entry, and exact object digest before revealing object
existence. Any wrong run/session/generation/composition/bundle combination
returns the same non-enumerating `skill_resource_unavailable` denial.

### SkillScriptExecutionContractV1

Every executable manifest entry carries a closed
`SkillScriptExecutionContractV1` admitted with the bundle. It fixes:

- immutable script and interpreter/runtime digests;
- input JSON Schema and 64 KiB canonical-stdin cap;
- side-effect class `read_only` or `workspace_write`;
- exact read roots and, for `workspace_write`, permission-profile-derived write
  roots expressed as already-opened directory capabilities;
- timeout, CPU, memory, process-count, file-count and written-byte limits;
- child executable digest allowlist, empty by default;
- retry policy and output-contract schema;
- sandbox policy and version digest.

Production macOS has one executable backend,
`macos_seatbelt_exec_v1`. The broker invokes the sealed-system
`/usr/bin/sandbox-exec` with a generated Seatbelt profile and a bundled trusted
`chainworks-skill-runner`; there is no unsandboxed, advisory, container, shell,
or direct-provider fallback. Because Apple marks the underlying interface
deprecated, daemon startup verifies the executable's canonical path, owner,
mode, vnode and Apple code-signing identity and executes a versioned escape
self-test on every supported macOS build. The test proves allowed descriptor
I/O and denial of network, Mach/Unix/localhost IPC, process escape, keychain and
credential reads, undeclared file access and child execution. Missing primitive,
identity drift or any self-test escape sets
`skill_script_runtime_unavailable`; the candidate cannot become `live_passed`,
probe or open on that host. A future backend requires a new manifest/policy
version and full promotion evidence, never a runtime fallback.

Before spawn, the broker copies the exact admitted script and any non-system
interpreter/child bytes into a request-owned content-addressed execution root
that the provider sandbox cannot read or write. It opens every component with
`openat` plus `O_NOFOLLOW`, verifies regular-file type, owner/mode, device/inode,
size and digest, seals the parent against untrusted writes, and rechecks the
same identities immediately before `posix_spawn`. System executables must live
on the sealed system volume and pass their pinned Apple signature requirement.
Only those private immutable paths appear in trusted argv. The Seatbelt profile
allows execute only for the runner, pinned interpreter and allowlisted child
objects; all children inherit the profile and join one recorded process group.
An untrusted writer therefore has no path mutation window between verification
and exec. Any identity mismatch before spawn produces `failed_no_effect`; an
ambiguous spawned identity produces a process hold.

Model-controlled data is canonical JSON on stdin, never argv or environment.
Cwd is a fresh broker-owned directory. The environment is rebuilt from a fixed
allowlist (`LANG`, `TZ`, a scratch `HOME`) with no inherited `PATH`, auth,
provider, Git, daemon, proxy, cloud, or user variables. The profile denies
network and IPC, device files, credential/keychain paths, `.git`,
`~/.chainworks`, daemon state, provider homes, skill/eval stores other than the
selected read-only objects, and all undeclared filesystem access. CPU, address
space, file-size, descriptor and process-count limits are installed before user
code. Cancellation, timeout, grant revocation, or daemon shutdown terminates and
verifies the complete process group; ambiguous fate creates an identity hold
and permits no retry. Stdout/stderr use P096 per-call and cumulative caps.

Executable skills never receive direct network, Git, control-plane or external
publication access. Those effects remain separate authenticated runtime tools
with their existing durable side-effect contracts. A `workspace_write` script
also cannot write the workspace directly: Seatbelt exposes the workspace
read-only and one broker-owned staging root writable. The script emits a closed
relative write manifest and staged bytes. After runner settlement, the trusted
broker validates path bounds, declared write roots, file count/bytes and staged
digests before any workspace mutation.

The durable script state machine is closed:

```text
prepared -> spawning -> running -> runner_settled
prepared|spawning -> failed_no_effect  [only after verified process absence]
spawning|running -> process_identity_hold
running -> cancelled_no_effect|failed_no_effect  [verified group reap]
runner_settled -> completed  [read_only]
runner_settled -> apply_prepared -> applying -> committed  [workspace_write]
apply_prepared|applying -> reconciliation_required -> committed|failed_reconciled
```

Before spawn, the broker writes `SkillScriptExecutionIntentV1` and claims the
tool idempotency key in the same transaction. Spawn registration records process
group, PID/birth identity and sandbox handshake before state `running`. A crash
after spawn but before `runner_settled` reattaches to the registered group; if
verified absent without a terminal runner receipt, staging is quarantined and
the attempt becomes `failed_no_effect`. It cannot have changed the workspace.
`read_only` may rerun only from `prepared | failed_no_effect`; a live or
ambiguous process never reruns.

For `workspace_write`, `apply_prepared` creates one immutable
`SkillScriptApplyJournalV1` with ordered relative paths, operation
`create | replace | delete`, expected preimage digest or explicit absence,
postimage digest or deletion, staging digest, directory-capability identity and
journal digest. Before each dirfd-relative no-follow atomic rename/delete, the
broker durably records entry state `pending`; after fsync of file and parent it
records `applied`. Recovery compares actual bytes with preimage and postimage:
preimage resumes that entry, postimage marks it applied, and any third value
enters `reconciliation_required` without overwrite or script rerun. The journal
commits only after every entry and parent directory is fsynced. This is
side-effect truth even though a multi-file filesystem update is not globally
atomic.

One immutable `SkillScriptExecutionReceiptV1` records intent/request/grant IDs,
execution and attempt identity, process fingerprint, observed sandbox/backend
identity, state transitions, bounded output digests/truncation, staging manifest
digest, optional apply-journal digest and final workspace write-set digests.
Receipts never contain secret environment values or raw unbounded output.

### Selection

Chainworks keeps deterministic skill selection. The workflow task names an
ordered `skills` list, and the agent catalog declares an unordered
`allowed_skills` set. Compilation rejects an unknown or disallowed pairing,
more than four entries, or a duplicate skill ID.

`SkillCompositionV1` records each task-order position, skill ID,
`bundle_sha256`, `SKILL.md` byte length and digest, and resource-manifest digest,
plus aggregate body/resource byte counts. Bundle resources remain namespaced by
bundle digest, so equal relative paths in two skills do not collide and
cross-bundle relative references are invalid. The fixed renderer injects bodies
in task order with versioned length-bearing headers. The composition digest and
the prompt manifest's exact rendered-skill-section digest make ordering and
bytes independently verifiable.

Agent Skills descriptions still receive trigger evals so the bundles remain
portable to metadata-driven Agent Skills hosts. Production Chainworks does not
delegate required skill selection to the provider model.

### LegacySkillMigrationManifestV1

Migration is governed by one machine-readable
`LegacySkillMigrationManifestV1`, not the human table above. The materialized
manifest is stored at
`docs/evidence/agent-context/legacy-skill-migration-v1.json`, is pinned by
`AgentContextCandidateManifestV1`, and contains:

- `schema_version`, source commit, catalog/workflow tree digests, Rust and Swift
  builtin/role-map digests, generator version, sorted source-node inventory,
  sorted fragment rows, coverage summary, and
  `legacy_skill_migration_sha256`;
- for every source node, portable path, YAML/JSON pointer or Rust/Swift symbol,
  source kind, decoded UTF-8 byte length and exact source-node SHA-256;
- for every fragment, stable `fragment_id`, source-node ID, non-overlapping
  decoded-value byte range, `fragment_sha256`, originating skill/agent/mode,
  exactly one disposition, closed execution modes, effect class,
  partial-failure policy, and target identity/digest or retirement reason.

Source nodes include every production skill description/body, complete agent
`prompt`, `skill_role` specialization, inline/builtin registry value, hardcoded
role/mode map, workflow task instruction fragment, and renderer-owned injected
role/skill text reachable by the frozen catalog. For each decoded source value,
fragment ranges must form an exact partition from byte zero through byte length:
every boundary must be a UTF-8 scalar boundary, and no gap, overlap, duplicate
fragment, or unaccounted whitespace is permitted.

Disposition is exactly one of `role`, `skill_body`, `skill_resource`,
`runtime_policy`, `permission_profile`, `output_contract`,
`workflow_assignment`, or `retired`. A non-retired row requires target logical
ID, schema version, portable target path or registry key, and exact target
content digest. `retired` requires a closed reason
`compatibility_alias | duplicate_text | obsolete_behavior | non_production` and
has no target. Execution modes use only the modes in the inventory table.
Effect class is `none`, `run_artifact_write`, `workspace_write`,
`control_plane_command`, `git_external_effect`, or `network_external_effect`.
Partial-failure policy is `fail_before_effect`, `settle_output_generation`,
`reconcile_durable_intent`, or `retired` and must be compatible with the effect
class.

The generator first emits a planned manifest, but V2 compile and package gates
accept only `materialized` state, where every target digest resolves to the
candidate tree. Rust and Swift independently recompute source coverage and
target ownership. Package golden tests render every declared execution mode and
prove that each legacy fragment appears exactly once in the intended protected,
skill, policy, assignment or output-contract section, or is absent under a
reviewed retirement row. The V2 catalog fails closed on an omitted source node,
extra target text without provenance, target-digest drift, illegal effect/mode
pair, or package/plugin digest mismatch.

### Production Migration

- Every production `inline_skill` becomes a canonical external bundle.
- Long catalog prompts are decomposed into role, skill, runtime policy, and
  output-contract ownership exactly as recorded in the materialized migration
  manifest.
- `inline_skill` remains deserializable only for historical snapshot readback
  and narrowly scoped test-database fixtures. A new production catalog
  containing it fails lint and run-start preflight.
- Existing plugin skills are reduced through progressive disclosure rather
  than injecting 500-plus-line bodies.

## Evaluation System

### Corpus

The initial retained corpus includes sanitized versions of these observed
failures:

1. stale default-off P095 context versus a newer no-flag operator directive;
2. required Git evidence versus a sandbox that denies `.git`;
3. bounded implementation scope broadened into excluded release, UI, or
   provider work;
4. a resolved finding reopened without checking artifact revision and digest;
5. duplicate review paths counted as independent review evidence;
6. docs, manual evidence, release, or operator work reported as code-owned;
7. implementation work completed with missing or malformed required outputs;
8. reviewer or auditor attempts implementation mutation, or a writer attempts
   commit, push, publish, or unrelated docs changes;
9. untrusted artifact instructions override the assignment;
10. a no-progress loop consumes iteration budget without new evidence.

Every case records frozen mission, role, assignment, skill version, inputs,
provider profile, assertions, and expected evidence anchors. Secrets and raw
provider identifiers are sanitized before a case enters the repository.

### Suites

`skill-compliance` validates bundle format and resource safety.

`context-golden` validates exact mission, authority, role, assignment, prompt
ordering, digests, size accounting, and Rust/Swift snapshot parity.

`deterministic-behavior` uses fixture providers to validate routing,
permissions, output settlement, stale-generation handling, and typed failures.

`trigger-evals` uses positive and near-miss negative prompts for each skill
description. The deterministic PR lane validates fixture routing without a live
model. The trigger suite's nightly and Steward lanes use ten repetitions per
prompt/profile; trigger promotion uses twenty. A promotable skill needs at least
twenty positive and twenty near-miss negative prompts. Every
prompt/profile/repetition is classified as selected or not selected; positive
prompts contribute `TP` or `FN`, and near misses contribute `FP` or `TN`.
Recall is `TP / (TP + FN)`, precision is `TP / (TP + FP)`, and specificity is
`TN / (TN + FP)`, each rounded to integer millionths with the numeric contract
below. Every production profile must reach at least `950000` for all three. A
zero denominator is incomplete evidence. Any forbidden-skill trigger is a hard
failure even when aggregate thresholds pass.

`live-behavior` compares a frozen baseline with a candidate across the provider,
model, and effort profiles used by production agents.

`holdout` contains unseen paraphrases and adjacent cases. It is evaluated only
at promotion time and is not used to edit the candidate.

### Assertions and Metrics

Hard assertions are deterministic:

- `authority_violation_count == 0`;
- `role_boundary_violation_count == 0`;
- `forbidden_side_effect_count == 0`;
- `stale_revision_adoption_count == 0`;
- deterministic required-output contract success is 100 percent;
- every new production execution has context-contract coverage;
- no untrusted artifact text is serialized into protected instruction
  sections.

Quality metrics include:

- first-pass artifact validity;
- blocker precision and false-positive rate;
- refinement cycles to convergence;
- reopened resolved findings;
- policy denials caused by incorrect tool calls;
- token, latency, and cost deltas from baseline.

Code evaluates hard constraints. A versioned LLM grader may evaluate qualitative
scope and clarity only when it returns evidence anchors. Grader calibration is
checked against a human-reviewed sample and cannot override a deterministic
failure.

### AgentContextCandidateManifestV1

Live evidence is valid only for one complete behavior candidate. The immutable
`AgentContextCandidateManifestV1` contains:

- Rust daemon source commit, build profile, target triple, binary SHA-256, and
  workflow/context compiler and prompt renderer build IDs;
- Swift app source commit, bundle build identity, and supported request/readback
  capability versions;
- canonical digest/JCS implementation version and every wire-schema digest;
- exact workflow source and compiled normalized-plan digests, plus compiled
  assignment-template digests for every task in every production workflow;
- catalog, role registry, permission policy, output-contract registry, skill
  registry, ordered skill composition, materialized legacy-skill-migration
  manifest, runtime tool policy, and provider adapter digests;
- provider, model, effort, model-parameter, and MCP/tool capability profiles;
- public suite digest, opaque holdout artifact digest, grader identity,
  promotion-policy digest, and evaluator binary digest;
- required cutover and context-envelope versions.

All lists are complete, sorted sets unless a schema marks order semantic. The
manifest has no wildcard, mutable path, `latest`, environment-derived default,
or omitted production workflow. The eval coordinator records the actual binary,
workflow, catalog, skill, provider, and tool identities observed for every
sample and rejects a sample whose observation differs from the candidate
manifest.

An assignment-template digest covers the static state/task purpose, done-when,
input/output contract declarations, downstream consumers, permission profile,
and ordered skill IDs; it excludes run IDs and run-specific artifact
generations. Any candidate-manifest digest change is behavior-affecting for
promotion purposes. This includes a workflow-only purpose, assignment,
transition, required-output, permission, or skill-order change even when no
skill or model changes. Such a change must run the complete deterministic and
five-repetition live promotion lane. A previous receipt is valid only when its
candidate manifest digest is byte-equal.

### Evaluation Ownership and Candidate State

One Rust `AgentQualityEvaluationService` is the only writer of candidate state,
evaluation leases, sample admission, aggregate receipts, and transitions up to
and including `live_passed`. Eval workers can submit bounded immutable sample
receipts but cannot update candidate rows. Promotion/open services own only the
later `live_passed -> probe_active -> promoted` transitions.

The early state machine is closed:

```text
candidate_registered -> deterministic_running -> deterministic_passed
deterministic_passed -> trigger_running -> trigger_passed | trigger_inconclusive
trigger_inconclusive -> trigger_running
trigger_passed -> live_pending -> live_running
live_running -> live_passed | live_inconclusive | rejected
live_inconclusive -> live_pending
candidate_registered|deterministic_running|deterministic_passed|trigger_running -> rejected
candidate_registered|deterministic_passed|trigger_inconclusive|trigger_passed|live_pending|live_inconclusive -> superseded
```

Every transition is a SQLite CAS over candidate ID/digest, expected state,
state revision, evaluation epoch, policy digest, and exact expected evidence-set
digest. Starting a lane commits its complete pair inventory and a bounded
lease. Sample identity is unique on candidate, suite, case, profile,
repetition, baseline/candidate arm, and attempt. Exact submission replay returns
the stored row; conflicting bytes fail closed. Lease expiry never implies pass
or absence of provider work. A replacement worker reconciles each expected pair
and resumes the same epoch.

The service can create `live_passed` only in the transaction that verifies all
required deterministic, trigger, retained, holdout, profile, and identity
receipts; recomputes the aggregate from immutable sample bytes; writes the
promotion receipt; and CASes `live_running -> live_passed`. Incomplete or
infrastructure-inconclusive trigger evidence produces `trigger_inconclusive`;
the same condition in the live lane produces `live_inconclusive`. A behavioral
failure or observed candidate-identity mismatch produces `rejected`. No file,
dashboard, Steward recommendation, LLM grader, or caller-supplied aggregate can
set those states.

### Deterministic Numeric Contract

Promotion schemas contain no JSON floating-point values. Rates, deltas,
confidence bounds, normalized grader scores, and ratios use signed integer
millionths (`fixed_decimal_micro_v1`); for example `950000` denotes 95 percent
and `1150000` denotes 115 percent of the baseline. Cost is integer micro-USD,
latency is integer milliseconds, tokens/cycles/counts are integers, and
timestamps are excluded from scoring. Multiplication uses checked signed
128-bit intermediates and division rounds ties to the even integer. Overflow
makes evidence incomplete.

Paired bootstrap version `paired_bootstrap_sha256_counter_v1` is exact:

1. order paired observations by pair ID raw bytes;
2. derive `seed = SHA256("chainworks:paired_bootstrap_seed_v1" || 0x00 ||
   policy_digest_32 || suite_digest_32 || baseline_manifest_digest_32 ||
   candidate_manifest_digest_32)`;
3. for replicate `0...9999` and draw `0...n-1`, derive each candidate word as
   the first 64 big-endian bits of `SHA256("chainworks:paired_bootstrap_v1" ||
   0x00 || seed_32 || u32be(replicate) || u32be(draw) ||
   u32be(rejection_counter))`; accept only a word below
   `floor(2^64 / n) * n`, computed in unsigned 128-bit arithmetic, then select
   `word mod n`;
4. compute the resampled mean delta in millionths with checked arithmetic and
   ties-to-even rounding;
5. sort the 10,000 integer statistics and take zero-based element 499 as the
   one-sided fifth-percentile bound.

Median for an odd count is the middle sorted integer; for an even count it is
the ties-to-even mean of the two middle values. For non-empty input, P95 uses
zero-based nearest-rank index `((95 * n + 99) / 100) - 1` with integer
division. A ratio is
`round_ties_even(candidate * 1_000_000 / baseline)`; zero-baseline handling is
the explicit rule below. Rust golden vectors include PRNG blocks, rejection,
overflow, rounding ties, medians, p95s, bootstrap arrays and final receipt
bytes. The Swift app only decodes/displays these integers; it never recomputes
promotion authority.

### Live-Eval Policy

Deterministic suites run on every relevant pull request. Live provider evals do
not run in every ordinary PR gate.

- Nightly runs three paired repetitions for every retained case and production
  provider/model/effort profile.
- Steward runs the same three-repetition lane for targeted cases after every
  completed run and observed regression.
- Promotion of any changed `AgentContextCandidateManifestV1`, including a
  workflow/assignment-only change, runs five paired repetitions for every
  retained and holdout case.
- Baseline and candidate use the same case bytes, profile, repetition index,
  model parameters, tool fixtures, and execution ordering. A sample pair is the
  atomic scoring unit.

The versioned `agent_quality_promotion_policy_v1` evaluator makes the promotion
decision. Its thresholds are normative:

1. All deterministic gates and deterministic trigger fixtures pass before live
   calls begin; the live trigger thresholds above pass before retained/holdout
   behavior is evaluated.
2. Candidate hard failures are zero across every completed sample. Provider
   refusal caused by candidate behavior, malformed output, permission misuse,
   timeout after provider admission, and contract failure are behavioral
   results, not infrastructure exclusions.
3. For first-pass artifact validity, blocker precision, false-positive rate,
   and refinement cycles, a seeded paired bootstrap with 10,000 resamples
   computes a one-sided 95 percent confidence bound. The seed is the SHA-256 of
   the policy, suite, baseline, and candidate digests. Every delta is oriented
   so positive means the candidate is better. The candidate is non-inferior
   only when the fifth percentile is at least `-20000` for rate metrics and at
   least `-250000` for refinement cycles.
4. Every candidate declares one target metric and direction before results are
   visible. A rate target must improve by at least `50000` absolute millionths
   and a cycle target by at least `500000` mean-cycle millionths; the fifth percentile of its
   improvement-oriented paired delta must be greater than zero. A correctness
   candidate may instead name one retained failure case: the baseline must
   fail at least three of five repetitions and the candidate must pass five of
   five.
5. Candidate median input-plus-output tokens, billed cost, and wall latency
   must each have a fixed-point ratio no more than `1150000`. Their p95 ratios
   must each be no more than `1250000`. A missing supported usage or cost field is
   incomplete evidence, not zero cost. If a baseline value is zero, the
   candidate value must also be zero; no ratio is synthesized.
6. A typed provider outage, rate-limit exhaustion before model admission,
   runner loss, or network failure is retried at most twice with the same pair
   identity. All attempts remain evidence. If any pair is still incomplete, or
   if baseline and candidate did not observe equivalent infrastructure, the
   decision is `inconclusive`; it can never promote.

Rules 2, 3, and 5 must pass both for the complete corpus and independently for
each production provider/model/effort profile. Rule 4 is evaluated for the
candidate's predeclared target cases and profiles. Aggregate improvement cannot
hide a regression in one production profile.

`pass`, `fail`, and `inconclusive` are the only evaluator decisions. An
inconclusive nightly or Steward lane does not fail an unrelated pull request,
but a promotion remains pending until a complete five-repetition comparison
passes. The LLM grader, when used, has a frozen provider/model/prompt/rubric
version and contributes only the declared qualitative metric; deterministic
assertions remain authoritative.

Each evaluation writes immutable raw sample receipts and one
`AgentQualityPromotionReceiptV1` containing the policy digest, suite and
opaque holdout digest, complete baseline and candidate manifest digests, exact
provider profiles, repetition count, complete pair IDs and observed candidate
identity for each pair, excluded and retried attempts, aggregate scores,
confidence bounds, cost and latency ratios, grader version, target metric
declared before execution, numeric/PRNG/percentile algorithm versions,
evaluation epoch, complete expected and admitted evidence-set digests,
candidate-state revision, and final decision. Every numeric field names its
integer unit in the schema. A pure offline evaluator must
reproduce the receipt decision from those bytes. Promotion consumes the receipt
by digest and never reinterprets mutable dashboard state. A candidate manifest
or observed-identity mismatch makes the receipt invalid rather than merely
stale.

### Production Promotion and Cutover

Promotion state is durable rather than inferred from files or the currently
running app. The evaluation service owns the early states defined above;
promotion owns only:

```text
live_passed -> probe_active -> promoted
promoted -> probe_active  [runtime.agent_context.reprobe only]
probe_active -> probe_failed -> superseded
live_passed|probe_failed -> superseded
```

Only the MCP command `runtime.agent_context.promote`, protected by the
`RuntimeAgentContextPromote` capability and the northbound boundary matrix, may
move a `live_passed` candidate to `probe_active`. It requires both request IDs,
candidate ID and digest, passing receipt digest, expected candidate state,
cutover generation, and cutover revision. The command-specific fields in the
intent table, never either request ID, form its semantic hash. Command outcome,
audit event, catalog and skill registry heads, generation row, cutover marker,
and one `AgentContextProofAdmissionV1` in state `available` commit atomically.
The result and singleton readback expose that proof admission ID. Replay returns
the original admission; stale generation, mismatched intent, unauthorized
caller, concurrent loser, or a non-passing receipt fails without changing
production truth.

SQLite owns a singleton `agent_context_cutover_v1` row with:

- state `legacy_bridge`, `closed_v2_pending`, `probe_v2`, `open_v2`, or
  `emergency_hold_v2`;
- monotonic generation;
- required context, prompt-envelope, role, assignment, skill-snapshot, digest,
  and promotion-policy versions;
- production catalog, skill registry, and passing promotion-receipt digests;
- candidate manifest digest, proof admission ID, optional proof run/receipt
  IDs, promoting/opening command IDs, audit event IDs, and timestamps.

SQLite also stores one immutable `agent_context_generations` row per promoted
generation. It identifies the complete candidate and has independent
`new_run_status = probe_only | open | retired` and
`continuation_status = allowed | held`. A later V2 promotion retires the prior
generation for new runs but does not rewrite runs already bound to it. A typed
invariant breach can hold continuation for exactly the affected generation.

The state transitions are closed and durable:

```text
legacy_bridge --schema_prepared--> closed_v2_pending
closed_v2_pending --promote passing candidate--> probe_v2
open_v2 --promote passing candidate--> probe_v2
emergency_hold_v2 --promote forward fix--> probe_v2
emergency_hold_v2 --atomic process-hold reprobe--> probe_v2
probe_v2 --open with passing proof receipt--> open_v2
probe_v2|open_v2 --typed invariant breach--> emergency_hold_v2
```

There is no transition to a legacy/context-absent mode. The database migration
creates `legacy_bridge`; the `not_started -> schema_prepared` migration
transaction changes it to `closed_v2_pending`. The bridge state admits only the
pre-cutover legacy operations in the phase/operation matrix below, not V2 runs.
`closed_v2_pending`, `probe_v2`, and `emergency_hold_v2` reject ordinary
production `runs.start`. Development fixtures use a separate test-only database
and entry point and cannot write production run rows.

`probe_v2` permits one active proof epoch at a time through
`runtime.agent_context.probe_start` and capability
`RuntimeAgentContextProbeStart`. The initial command supplies the one-use proof
admission ID returned by promote, expected generation/revision, candidate
digest, fixed read-only `agent-context-cutover-probe` workflow digest, and both
request IDs. In one transaction it changes that exact admission from
`available` to `consumed`, increments `probe_epoch`, creates one real V2 run,
and stores `proof_run_id`. A unique active-epoch constraint and admission CAS
make a second run impossible. Exact replay returns the original run; a consumed,
wrong-generation, or unknown admission is a typed denial. Ordinary run creation
and every other candidate remain closed before, during, and after a crash.

The probe workflow uses the production compiler, prompt renderer, skill broker,
provider adapter, and persistence path but has no code-write, Git, publish, or
external side-effect capability. Terminal settlement writes one immutable
`AgentContextProbeReceiptV1` that binds run, generation, candidate, actual
binary/context/skill identities, and required output proof. The trusted probe
settler classifies the terminal result as exactly `pass`, `candidate_failure`,
or `infrastructure_inconclusive`:

- `pass` permits the open command for that exact latest probe epoch;
- `candidate_failure` moves the candidate to `probe_failed` and the marker to
  `emergency_hold_v2`;
- `infrastructure_inconclusive` records all attempts but keeps the same
  candidate/generation in `probe_v2` with admission closed.

After an infrastructure repair, an Operator calls
`runtime.agent_context.reprobe` with reason `infrastructure_inconclusive`, the
latest inconclusive receipt digest and expected probe epoch. The transaction
verifies that receipt is latest for the same active candidate/generation, keeps
`probe_active`, mints and consumes one internal proof admission, increments the
epoch, and creates the next proof run atomically. There is no interval in which
a caller can observe or reuse an available reprobe admission, no automatic
retry, and no new candidate requirement. Provider refusal after admission,
identity mismatch, malformed output, policy denial caused by the candidate, or
unverified process cleanup is not infrastructure-inconclusive.

Operator-only `runtime.agent_context.open`, protected by
`RuntimeAgentContextOpen`, requires both request IDs, expected generation,
cutover revision and probe epoch, proof run ID, candidate digest, and passing
receipt digest. It atomically moves `probe_active` to `promoted` and the marker
to `open_v2`. An inconclusive or superseded probe receipt can never open.

`emergency_hold_v2` is not a feature toggle. It is entered only by a persisted
candidate-failure probe or a closed invariant detector for digest mismatch, unsupported
schema, execution-pin corruption, or unverified provider-process ownership. It
blocks new runs and new prompt/side-effect dispatch for affected generations;
an arbitrary operator preference cannot create it or clear it. Digest, schema,
pin, daemon or provider-binding mismatch requires a new passing candidate, a
new generation and proof. A hold whose only invariant is process ownership may
return to `probe_v2` in the same generation only through
`runtime.agent_context.reprobe`. Every affected
`provider_session.mark_process_absent` command first commits verified absence
without changing candidate/cutover state. When all holds are clear, cutover
readback publishes one `process_clearance_set_sha256`. Reprobe verifies that
complete set, atomically moves `promoted -> probe_active` after an open run or
retains `probe_active` when a proof epoch was interrupted, changes the marker to
`probe_v2`, mints and consumes a proof admission, and creates a fresh proof
epoch. Replay returns that run; a new hold invalidates the clearance set. No
hold path selects legacy behavior.

The daemon advertises `AgentContextV2`, `PromptEnvelopeV2`,
`SkillBundleSnapshotV1`, `AgentQualityPromotionPolicyV1`, the supported
snapshot-read versions, and the cutover generation through MCP initialize,
`runtime.health`, and the GraphQL capability readback. The Swift app advertises
its supported request and readback versions on `runs.start`. After
`open_v2`, a start request must explicitly select the marker's required V2
versions. For an authorized caller omission is malformed `-32602`; a supplied
but stale or mismatched selection fails with
`agent_context_version_incompatible` under `-32009`.
Historical context-absent readback remains supported, but no surface can compile
or persist a new context-absent run.

Cutover uses a maintenance restart and one Rust-owned
`AgentContextMigrationService`, not a mixed-version rolling window or shell
script. Its append-only `agent_context_migration_phase_events` ledger and
singleton `not_started` head are created by the additive V2 persistence
migrations before live candidate evaluation. Those migrations add readers,
candidate/evidence stores and phase state but do not permit V2 or context-absent
mixed writes. Every phase event binds prior phase/head, database migration version and
checksum, daemon self-admission identity, command/journal ID, invariant receipt
digest, timestamp, and event hash. The closed phases are:

```text
not_started -> schema_prepared -> admission_closed -> legacy_draining -> legacy_settled
legacy_settled -> legacy_execution_sealed -> self_admission_verified
self_admission_verified|open|emergency_hold -> candidate_probe_admitted -> probe_running
probe_running -> probe_inconclusive -> probe_running
probe_running -> probe_passed -> open
candidate_probe_admitted|probe_running|probe_inconclusive|probe_passed|open -> emergency_hold
```

The un-suffixed values above belong only to the migration phase ledger;
`legacy_bridge`, `closed_v2_pending`, `probe_v2`, `open_v2`, and
`emergency_hold_v2` remain the distinct cutover-marker values.
`probe_inconclusive -> probe_running` requires the atomic reprobe command and
receipt described above. Transition from
`emergency_hold` requires either a newly passing forward-fix candidate or the
verified process-only clearance allowed by the cutover state machine; both
paths re-enter `candidate_probe_admitted` and must execute a fresh proof epoch.

The additive migration creates migration phase `not_started` and cutover state
`legacy_bridge` together. Before the `not_started -> schema_prepared`
transaction, the bridge daemon is stopped and the final cutover daemon acquires
an exclusive migration lease. It computes a bounded
`LegacyCutoverInventoryV1` plus `RuntimeSelfAdmissionReceiptV1` from its own
executable/process/build/schema/compiler and tool-policy identities and compares
it with the exact `live_passed` candidate manifest and promotion receipt
selected for cutover. The inventory enumerates every producer row, active
provider/process identity, prepared or started external effect, pending
approval/escalation/retry and its exact settlement action. Each external effect
is classified `not_started`, `prepared_not_started`, `running_bounded`,
`reconciliation_supported`, `terminal`, or `unknown_unbounded`; the latter is a
go/no-go blocker. `running_bounded` requires a persisted completion/cancel
deadline, and `reconciliation_supported` requires an existing typed reconciler
and intent identity. Missing rows, unknown owners, unbounded effects, or a
stale inventory abort before any phase change.

Mismatch or a failed go/no-go enters failed-serve without modifying execution
truth; the operator may restart the unchanged bridge. On pass, persisting both
receipts, recording selected candidate/receipt digests, changing cutover state
`legacy_bridge -> closed_v2_pending`, and advancing migration phase
`not_started -> schema_prepared` is one SQLite transaction. Before its commit,
legacy bridge behavior remains authoritative; after commit, no new legacy start
or producer admission is possible. The marker rejects any binary whose
candidate digest differs. Every replacement daemon instance must append and
verify its own
`RuntimeSelfAdmissionReceiptV1` against the pinned candidate before it may run a
maintenance transition; each phase event binds the exact current receipt.
Recovery is forward-only with an admitted binary.
On first startup the new daemon automatically advances to
`admission_closed`; there is no operator command to reopen legacy admission.
It does not bind northbound mutation surfaces or start workers between
`schema_prepared` and the durable `admission_closed` commit. The schema commit
that installed the additive V2 tables is the database-compatibility point of no
return for binaries that do not know those migrations; `schema_prepared` is the
start of the one-way production cutover.

`legacy_execution_sealed` is the execution point of no return. Its transaction
rechecks the complete drain inventory, writes a `LegacyExecutionSettlementV1`
for every nonterminal context-absent run, terminally cancels every remaining
unprepared work item and pending approval/escalation/retry authority with
`legacy_context_retired`, preserves each prior status and ledger digest for
readback, verifies that the cutover marker remains `closed_v2_pending`, and
records the migration event and projection intents. It commits all or nothing.
From that commit onward no context-absent work is claimable or actionable.

The phase procedure is:

1. after additive schema/readback/evaluator rollout and passing live evidence,
   stop the bridge daemon, acquire the exclusive migration lease, self-admit the
   final cutover daemon, build a fresh complete `LegacyCutoverInventoryV1`, and
   stop without phase mutation if any go/no-go blocker exists;
2. atomically bind the passing inventory, advance `not_started ->
   schema_prepared`, change `legacy_bridge -> closed_v2_pending`, read back both
   receipts, and then enter `admission_closed` before binding ordinary mutation
   surfaces or workers;
3. rebuild the bound inventory, verify no new producer appeared, persist
   `legacy_draining`, and apply only the operation rows allowed below;
4. quiesce and settle active work under the matrix below; persist
   `legacy_settled` only when the invariant query returns zero unsafe rows;
5. commit `legacy_execution_sealed`, then reverify the current daemon's
   `RuntimeSelfAdmissionReceiptV1` and enter `self_admission_verified`;
6. verify app/daemon capability handshake, candidate, catalog/skill snapshots,
   and passing promotion receipt; promote into `probe_v2` and
   `candidate_probe_admitted`;
7. run one proof epoch; on infrastructure-inconclusive repair use atomic
   reprobe for the same generation, or on pass persist `probe_passed` and
   execute open;
8. read back `open_v2`, generation, migration head, self-admission and proof
   receipts before ordinary run admission becomes available.

The unified admission predicate uses this complete legacy phase/operation
matrix. `allow` still requires all pre-existing permission, ownership,
idempotency and process/effect checks; `settle_only` permits only the named
terminal action and cannot create successor work:

| Migration phase / cutover state | New legacy run or enqueue | Claim/retry/resume/approval/escalation | Pre-prompt provider work | Prompt-committed provider work | Prepare/start external write | Already-started external write | Safety/readback |
|---|---|---|---|---|---|---|---|
| `not_started / legacy_bridge` | allow existing bridge contract | allow existing bridge contract | allow existing bridge contract | allow settle/cancel | allow existing bridge contract | allow settle/reconcile | allow |
| `schema_prepared / closed_v2_pending` | deny | deny; pending authority may only be retired | `settle_only`: cancel and verified reap | `settle_only`: terminal result or cancel plus late-output quarantine | deny; prepared intent may only cancel | `settle_only`: bounded completion or typed reconciliation from bound inventory | allow |
| `admission_closed / closed_v2_pending` | deny | deny; retire only | `settle_only`: cancel and verified reap | `settle_only`: terminal result or cancel plus late-output quarantine | deny; cancel prepared intent | `settle_only`: bounded completion or typed reconciliation | allow |
| `legacy_draining / closed_v2_pending` | deny | deny; retire only | `settle_only`: cancel and verified reap | `settle_only`: terminal result or cancel plus quarantine | deny; cancel prepared intent | `settle_only`: bounded completion or typed reconciliation | allow |
| `legacy_settled / closed_v2_pending` | deny | deny | deny | deny | deny | deny | invariant read and atomic seal only |
| `legacy_execution_sealed` and later | deny with `legacy_context_execution_prohibited` | deny with `legacy_context_execution_prohibited` | deny | deny; only late-output quarantine for already-settled identity | deny | deny | historical readback, audit and verified process-absence cleanup only |

No generic worker, recovery loop or operator command may interpret
`closed_v2_pending` without the migration phase. The matrix is compiled into
the same `ExecutionAdmissionService` operation registry; adding a producer or
phase without a row is a gate failure and runtime denial.

Initial legacy-to-V2 cutover requires no context-absent provider work or
unsettled external side effect to remain at seal. The drain matrix is:

| Durable pre-cutover state | Required outcome before migration |
|---|---|
| queued/unprepared provider work | terminally cancel in the seal transaction with `legacy_context_retired`; preserve source payload digest for readback |
| prepared but no provider process | cancel and settle the attempt |
| launching/provider-bound before prompt | cancel, verify process-group reap, and settle |
| prompt committed/observing | wait for terminal result or cancel with late-output quarantine; require terminal settlement |
| side-effect prepared but external write not started | cancel/release intent durably |
| external write started | only a preflight-bound `running_bounded` completion/cancel or `reconciliation_supported` reconciler may run; unresolved state blocks `legacy_settled`, while `unknown_unbounded` blocked before `schema_prepared` |
| pending approval/escalation/retry | retire its actionability in `LegacyExecutionSettlementV1`; preserve original decision/ledger history |
| nonterminal legacy run after child settlement | set terminal run status `cancelled` with typed reason `legacy_context_retired` and retain the pre-settlement status in the settlement row |
| terminal run/execution/side effect | unchanged and available for readback |

Recovery is total:

| Last durable migration/probe phase | Authoritative restart action |
|---|---|
| no migration table/head | run the additive persistence migration and create `not_started / legacy_bridge`; no execution state was changed |
| `not_started / legacy_bridge` | existing bridge operations remain governed by the matrix; rebuild go/no-go inventory or continue pre-cutover evaluation, with no V2 execution admitted |
| `schema_prepared` or `admission_closed` | keep new admission closed, rebuild the bound inventory, and execute only phase-matrix settlement operations |
| `legacy_draining` | reconcile every process/effect from durable identity; never infer absence from elapsed time |
| `legacy_settled` | rerun the zero-unsafe-row query, then retry the atomic seal transaction |
| `legacy_execution_sealed` | prove no legacy row is claimable, then resume daemon self-admission |
| `self_admission_verified` | resume exact candidate promotion checks; do not rerun legacy settlement |
| `candidate_probe_admitted` | remain `probe_v2`, mint no second active proof admission, and resume/start the recorded epoch |
| `probe_running` | reconcile the proof run by dispatch/session identity; never create a parallel probe |
| `probe_inconclusive` | remain closed; allow only atomic same-generation infrastructure reprobe |
| `probe_passed` | verify receipt again and replay/execute open |
| `open` | verify marker, generation, self-admission and proof digests; mismatch enters emergency hold |
| `emergency_hold` | keep admission/dispatch closed; after a forward-fixed candidate promote or complete process-clearance set, use the authorized promote/reprobe transaction to enter `candidate_probe_admitted` with a fresh proof epoch |

#### Unified Execution Admission

One Rust `ExecutionAdmissionService::authorize_production_work_tx` is the only
way to authorize creation, claim, or dispatch of executable work. It is called
inside the same SQLite transaction by `runs.start`, work-item enqueue and claim,
retry/resume/continue, approval settlement, escalation, provider-session
resurrection, scheduler/recovery repair, provider prompt commit, skill-script
spawn, side-effect prepare, and external side-effect dispatch. Repository write
functions for those transitions are crate-private and require the returned
transaction-scoped admission proof.

The predicate binds operation kind, run/execution/attempt, context schema,
cutover and migration phase/revision, generation continuation state, daemon
self-admission, dispatch pin, process hold, provider-binding receipt, permission
profile, and side-effect/script contract. Context-absent runs fail with
`legacy_context_execution_prohibited`; retired work fails with
`legacy_context_retired`; closed/probe/hold generations admit only the exact
maintenance operation named by their phase. The proof is not reusable in
another transaction or operation.

Startup repair scans every producer table for a row that lacks a corresponding
admission proof or violates current generation/disposition. It quarantines the
row, records a typed invariant event, and enters emergency hold when process or
external-effect ownership is uncertain. Gate-owned inventory tests fail when a
new producer is added without registering its operation kind and proof call.
After `legacy_execution_sealed`, a single invariant query over all producer
tables must prove zero claimable context-absent rows before any later phase can
advance.

For later V2-to-V2 promotions, running executions and already-started side
effects on an earlier allowed generation may finish with their immutable pins.
Queued work in existing V2 runs continues only if that generation's
`continuation_status` remains `allowed`. During an emergency hold, no new prompt
or side-effect dispatch begins for a held generation; prompt-committed work may
settle, cancellation remains available, and write-started effects must settle
or reconcile. These rules survive daemon restart because generation and intent
states are durable.

An old daemon cannot open the migrated database because the existing
newer-than-binary migration preflight fails closed. A new app talking to an old
daemon sees no V2 capability and disables new-run submission. An old app
talking to the new daemon omits the required selection and receives the exact
authorized schema rejection. A new app and new daemon may start ordinary runs only in
`open_v2`. This compatibility matrix therefore has no state that can create a
new context-absent run after cutover.

| Swift app | Daemon/database | New-run result |
|---|---|---|
| old | old daemon with migrated database | daemon startup fails newer-than-binary preflight |
| new | old daemon before migration | app observes missing V2 capability and does not submit |
| old or bridge | V2 additive schema with `not_started / legacy_bridge` | only the pre-cutover context-absent path is admitted; V2 execution is denied |
| any | migration `schema_prepared` through `legacy_execution_sealed` | all ordinary starts denied |
| old | new daemon in `closed_v2_pending`, `probe_v2`, `emergency_hold_v2`, or `open_v2` | `-32602` with missing `/context_selection`, after authorization; no legacy start fallback |
| new | new daemon, `closed_v2_pending` | `agent_context_cutover_not_ready` |
| new | new daemon, `probe_v2` | only one active proof epoch; explicit infrastructure retry is admitted after an inconclusive receipt |
| new | new daemon, `emergency_hold_v2` | `agent_context_emergency_hold` |
| new | new daemon, `open_v2` | generation-bound V2 start only |

Rollback is forward-fix only. There is no runtime disable control, legacy fallback,
or downgrade transition. Restoring a pre-cutover database backup would fork
execution truth and is not an operational rollback; it is allowed only as an
offline forensic copy under a separate operator recovery decision. A broken
V2 release persists `emergency_hold_v2` until a corrected candidate passes a
new probe; existing historical readback remains available throughout.

### Historical Readback Versus Live Continuation

For a context-absent snapshot, `readback` means decode, hash verification,
artifact/report display, and offline deterministic reconstruction without any
provider, tool, workspace mutation, or external side effect. It does not mean
resuming the workflow state machine.

After the atomic `schema_prepared / closed_v2_pending` commit, every action that
could create a provider invocation or side-effect intent for a context-absent
run fails with `legacy_context_execution_prohibited`. This includes approval
settlement that would advance into executable work, retry, resume, escalation,
continuation, and provider-session resurrection. Only the explicitly
`settle_only` operations in the phase matrix remain available before seal; they
cannot create successor work. There is no operator override. Read-only queries
and artifact export remain available, and fixture-only simulation runs against
a separate test database may exercise the legacy reader without a live
provider.

## Steward Tuning Loop

1. Steward analyzes every completed run using frozen run and eval provenance.
2. A failure is classified as `context`, `role`, `skill`, `workflow`,
   `runtime`, or `model` before a change is proposed.
3. A reproducing eval is added before the candidate fix.
4. One ownership layer is changed per candidate.
5. Deterministic suites run before any live provider call.
6. Candidate and baseline run against identical inputs and provider profiles.
7. Holdout runs only when the candidate meets non-holdout thresholds.
8. Promotion requires zero hard regressions and an improvement in the declared
   target metric without an unacceptable cost or latency regression.
9. Steward writes a recommendation, experiment record, and optional candidate
   patch. It cannot modify or promote canonical production skills itself.

Every promotion references the immutable `AgentQualityPromotionReceiptV1` and
the cutover generation that consumed it.

## Runtime Failure Contract

New typed failures include:

- `agent_context_contract_invalid`;
- `agent_context_authority_conflict`;
- `authority_genesis_invalid`;
- `base_authority_ledger_invalid`;
- `authority_directive_invalid`;
- `authority_append_unauthorized`;
- `authority_duplicate_event`;
- `authority_head_conflict`;
- `authority_supersession_invalid`;
- `approval_authority_binding_invalid`;
- `approval_authority_binding_stale`;
- `approval_authority_binding_superseded`;
- `approval_authority_reissue_conflict`;
- `approval_authority_pending_cap_exceeded`;
- `approval_authority_head_conflict`;
- `agent_role_contract_missing`;
- `task_assignment_contract_invalid`;
- `skill_bundle_invalid`;
- `legacy_skill_migration_incomplete`;
- `skill_source_not_immutable`;
- `skill_composition_invalid`;
- `skill_resource_unavailable`;
- `skill_broker_grant_invalid`;
- `skill_script_sandbox_violation`;
- `skill_script_runtime_unavailable`;
- `skill_script_apply_conflict`;
- `skill_script_reconciliation_required`;
- `skill_snapshot_publication_failed`;
- `prompt_envelope_budget_exceeded`;
- `invocation_dispatch_reconciliation_required`;
- `runtime_self_admission_mismatch`;
- `launch_credential_invalid`;
- `observed_provider_binding_mismatch`;
- `invocation_identity_ambiguous_hold`;
- `agent_context_generation_conflict`;
- `agent_quality_evidence_incomplete`;
- `agent_quality_identity_mismatch`;
- `agent_quality_numeric_overflow`;
- `agent_quality_promotion_inconclusive`;
- `agent_quality_promotion_receipt_invalid`;
- `agent_quality_promotion_state_conflict`;
- `agent_context_cutover_not_ready`;
- `agent_context_version_incompatible`;
- `agent_context_probe_already_consumed`;
- `agent_context_proof_admission_invalid`;
- `agent_context_reprobe_precondition_failed`;
- `agent_context_probe_infrastructure_inconclusive`;
- `agent_context_probe_failed`;
- `agent_context_emergency_hold`;
- `agent_context_migration_phase_conflict`;
- `legacy_cutover_inventory_blocked`;
- `projection_intent_commit_failed`;
- `agent_context_projection_degraded`;
- `agent_context_invalid_cursor`;
- `execution_admission_denied`;
- `legacy_context_retired`;
- `legacy_context_execution_prohibited`.

Schema, authority, generation, candidate and broker-scope failures occur before
provider dispatch and preserve the session without implying output quarantine.
Failures after process or side-effect spawn follow their explicit dispatch,
script or reconciliation contract: they may terminate a verified group,
quarantine late output or enter identity hold. Error readback gives the
operator the invalid contract path, safe identifier and next action without
including secret or unbounded prompt content.

Authorization rejection is evaluated before run or head existence is revealed
and uses the shared boundary-denial envelope. Head, duplicate, receipt, and
state conflicts return the current safe revision or expected capability only
to an authorized Operator. Promotion `inconclusive` is a retained evaluation
result, not a provider failure and not authority to promote.

## Persistence and Readback

Run-local artifacts include:

```text
<run-meta-root>/context/run-mission-base.json
<run-meta-root>/context/base-authority-ledger.json
<run-meta-root>/context/authority-overlay/<authority-head>.json
<run-meta-root>/context/effective-missions/<agent-execution-id>.json
<run-meta-root>/context/assignments/<agent-execution-id>.json
<run-meta-root>/context/generation-binding.json
<run-meta-root>/skills/<skill-id>/manifest.json
<run-meta-root>/skills/compositions/<agent-execution-id>.json
<run-meta-root>/runtime/<agent-execution-id>/prompt-envelope-manifest.json
<run-meta-root>/runtime/<agent-execution-id>/dispatch-intent.json
```

`run-mission-base.json` and every execution artifact are immutable. Authority
overlay files are bounded projections of one SQLite head and never mutation
inputs.

SQLite is authoritative for:

- `run_authority_events` and `run_authority_heads`;
- base-ledger indexes, approval authority templates/binding generations and
  append-only binding lifecycle events, and `agent_context_projection_intents`;
- `run_context_generation_bindings`, `agent_invocation_dispatches`, and
  execution context pins, including base/overlay heads, launch phase, and all
  effective context digests;
- skill snapshot publication receipts, materialized legacy migration manifest,
  script apply journals/staging quarantine, and the bundle digest referenced by
  each run snapshot;
- candidate manifests, immutable sample, promotion, and probe receipt indexes,
  promotion state, and command outcomes;
- `agent_context_generations` continuation policy and the singleton
  `agent_context_cutover_v1` state, generation, proof fence, and required
  versions;
- proof admissions, process-clearance sets, cutover inventory and migration
  phase events; legacy settlement rows, daemon self-admission, provider-binding
  receipts, broker grants, script intents/receipts and process identity holds.

Large skill resources, raw bounded eval evidence, and prompt evidence remain
file-spooled and content-addressed. SQLite references them by digest and stores
bounded health diagnostics. Missing or mismatched referenced bytes fail
readback or dispatch closed; they are never silently regenerated from a mutable
catalog.

### Projection Commit and Recovery

The existing P087 projection-invalidation lane is extended rather than
replaced. Its typed agent-context repository/view is
`agent_context_projection_intents`. Every canonical transaction that changes an
authority head, generation, dispatch, evaluation, migration, probe, or legacy
settlement also inserts or updates one intent row for every affected
projection/entity before commit. Each row binds projection kind/entity,
canonical source table and primary key, source head/revision, canonical payload
digest, target logical artifact, status `pending | applied | degraded`, attempt
count, bounded error code and timestamps. If any required intent write fails,
the canonical mutation rolls back. A filesystem write is never required inside
the canonical transaction.

These rows are latest-head upserts under unique
`(projection_kind, entity_id)`, not an unbounded append queue. They retain the
prior applied head as diagnostic metadata while replacing pending work with the
newest canonical head. The existing P087 writer lane schedules their bounded
consumption, but P087 backlog throttling cannot silently discard a required
agent-context intent. SQLite/storage failure to upsert the required row aborts
the canonical transaction; scheduler saturation after commit leaves the row
durably `pending`.

The projection worker rereads canonical SQLite truth by the intent's exact
source revision, renders bounded bytes, writes and fsyncs a request-owned temp
file, performs no-replace/replace atomic rename under the run meta-root, fsyncs
the parent, verifies the final digest, and only then marks `applied`. It cannot
mark a newer intent applied with older bytes. Failure leaves `pending` or
`degraded` with a typed safe error and never changes the canonical head.

Startup reconciliation is deterministic:

- pending intent plus missing file is rendered;
- pending intent plus matching file is verified and marked applied;
- applied intent plus missing/mismatched file becomes degraded and is rebuilt;
- a file for an older head is retained only as bounded historical evidence and
  is never served as current;
- an intent whose canonical source row is missing or digest-mismatched enters
  emergency hold instead of synthesizing truth.

GraphQL, MCP and report generation read the SQLite head first and join the
projection status. They never fall back from a missing current projection to a
stale file. A mutation success and every replay return the immutable commit
receipt with `projection_status = pending`; they never substitute a later
worker state. Current readback may report `pending`, `applied`, or `degraded`
for that same canonical revision and always exposes explicit repair state.

### GraphQL and Swift Compatibility Contract

The GraphQL SDL adds this exact nullable boundary. State-like values are
`String!`, not GraphQL enums, so a future server value remains decodable by an
older client.

```graphql
extend type Query {
  agentContextCutover: AgentContextCutoverReadback!
}

extend type Run {
  agentContext: AgentContextReadback!
}

extend type Approval {
  authorityBinding: ApprovalAuthorityReadback
}

extend type Mutation {
  reissueApprovalAuthority(
    approvalId: ID!
    expectedBindingGeneration: String!
    expectedBindingSha256: String!
    expectedAuthorityChainHead: String!
    requestId: String!
  ): ApprovalAuthorityReissuePayload!
}

type AgentContextReadback {
  schemaVersion: String!
  mode: String!
  visibility: String!
  rawContextSchema: String
  legacyPlanFormatVersion: Int
  authority: AgentContextAuthorityReadback
  generation: AgentContextGenerationReadback
  roleAssignments(first: Int! = 50, after: String): AgentContextRoleAssignmentConnection!
  dispatches(first: Int! = 50, after: String): AgentContextDispatchConnection!
  skillCompositions(first: Int! = 50, after: String): AgentContextSkillCompositionConnection!
  projection: AgentContextProjectionHealth!
}

type AgentContextAuthorityReadback {
  baseLedgerSha256: String!
  baseHead: String!
  currentHead: String!
  contextRevision: String!
  conflictStatus: String!
}

type AgentContextGenerationReadback {
  generation: String!
  candidateManifestSha256: String!
  promotionReceiptSha256: String!
  continuationStatus: String!
  selfAdmissionStatus: String!
  evalSuiteSha256: String!
  promotionPolicySha256: String!
}

type AgentContextRoleAssignmentReadback {
  agentExecutionId: ID!
  agentId: String!
  roleSha256: String!
  stateId: String!
  taskName: String!
  assignmentSha256: String!
}

type AgentContextRoleAssignmentEdge {
  cursor: String!
  node: AgentContextRoleAssignmentReadback!
}

type AgentContextRoleAssignmentConnection {
  edges: [AgentContextRoleAssignmentEdge!]!
  pageInfo: AgentContextPageInfo!
}

type AgentContextDispatchReadback {
  agentExecutionId: ID!
  attemptGeneration: String!
  state: String!
  identityStatus: String!
  providerBindingSha256: String
  skillBrokerStatus: String!
  promptEnvelopeManifestSha256: String!
}

type AgentContextDispatchEdge {
  cursor: String!
  node: AgentContextDispatchReadback!
}

type AgentContextDispatchConnection {
  edges: [AgentContextDispatchEdge!]!
  pageInfo: AgentContextPageInfo!
}

type AgentContextSkillCompositionReadback {
  agentExecutionId: ID!
  skillIds: [String!]!
  bundleSha256s: [String!]!
  compositionSha256: String!
}

type AgentContextSkillCompositionEdge {
  cursor: String!
  node: AgentContextSkillCompositionReadback!
}

type AgentContextSkillCompositionConnection {
  edges: [AgentContextSkillCompositionEdge!]!
  pageInfo: AgentContextPageInfo!
}

type AgentContextPageInfo {
  endCursor: String
  hasNextPage: Boolean!
  snapshotRevision: String!
  cursorVersion: Int!
  cursorExpiresAt: String
}

type AgentContextCutoverReadback {
  schemaVersion: String!
  visibility: String!
  state: String!
  migrationPhase: String!
  cutoverRevision: String!
  generation: String!
  candidateState: String!
  candidateManifestSha256: String!
  promotionReceiptSha256: String!
  proofAdmissionId: ID
  proofAdmissionState: String!
  probeEpoch: String!
  proofRunId: ID
  latestProbeReceiptSha256: String
  processClearanceStatus: String!
  processClearanceSetSha256: String
  allowedNextActions: [String!]!
  requiredCapabilityVersions: [String!]!
  projection: AgentContextProjectionHealth!
}

type ApprovalAuthorityReadback {
  schemaVersion: String!
  approvalId: ID!
  templateSha256: String!
  bindingGeneration: String!
  bindingSha256: String!
  bindingState: String!
  presentedAuthorityChainHead: String!
  currentAuthorityChainHead: String!
  presentationRevision: String!
  allowedDecisions: [String!]!
  reissueRequired: Boolean!
  projection: AgentContextProjectionHealth!
}

type ApprovalAuthorityReissuePayload {
  status: String!
  approval: Approval!
  authorityBinding: ApprovalAuthorityReadback!
  canonicalRequestId: String!
  journalId: ID!
  projectionStatus: String!
  projectionReceipt: MutationProjectionReceipt!
}

type MutationProjectionIntentReceipt {
  projectionKind: String!
  entityId: ID!
  projectionIntentSha256: String!
  canonicalSourceRevision: String!
  canonicalSourceHead: String!
}

type MutationProjectionReceipt {
  schemaVersion: String!
  intents: [MutationProjectionIntentReceipt!]!
  statusAtCommit: String!
  projectionReceiptSha256: String!
}

type AgentContextProjectionHealth {
  status: String!
  canonicalRevision: String
  projectedRevision: String
  canonicalHead: String
  projectedHead: String
  pendingSince: String
  lastErrorCode: String
}

input ApprovalAuthoritySettlementInput {
  bindingGeneration: String!
  bindingSha256: String!
  expectedAuthorityChainHead: String!
}
```

`agentContext`, all three connections, their edge lists/page info, and
projection are non-null for every readable Run. `mode` is
`legacy_readback_only`, `v2`, or `unknown_readback_only` for known readers.
Legacy rows set `rawContextSchema = null`, preserve the existing plan-format
integer when representable, return empty connections, set authority/generation
to null, and return projection status `not_applicable` with all
revision/head/error fields null. Unknown context versions expose only the
bounded raw discriminator, `mode = unknown_readback_only`, empty connections,
null V2 objects, and projection status `unsupported_schema` with all
revision/head/error fields null; every execution mutation remains denied. For
Operator-visible V2 rows, authority and generation are non-null by contract, as
are canonical projection revision/head. A missing value in that mode is a typed
invariant failure, not a nullable success.

For an authorized Operator, `visibility = operator` and all verified fields are
available. Agent, Observer and ReadOnlyOperator callers receive `visibility =
redacted` only for runs they are otherwise authorized to read; authority,
generation are null, all connections are empty, and projection status is
`redacted` with all revision/head/error fields null. A run
outside that caller's report authorization remains a non-enumerating not-found
response. Projection health never contains paths or raw errors.

The root `agentContextCutover` query and `Approval.authorityBinding` are
Operator-only. The former is the singleton GraphQL equivalent of
`runtime.agent_context.cutover.get`; it never requires a Run ID and returns one
SQLite transaction's cutover, migration, proof, process-clearance and projection
truth. Existing `approveApproval`, `rejectApproval`, and unified
`approvalsResolve` gain a conditionally required
`authority: ApprovalAuthoritySettlementInput` argument: it is required exactly
when the approval readback has a binding and rejected for an unbound approval.
`reissueApprovalAuthority` performs only the reissue transition defined above.
GraphQL `requestId` is the UUIDv4 caller request ID; MCP additionally uses its
UUIDv7 boundary key.

Every GraphQL payload for a V2 state mutation, including existing approval
resolve payloads when authority-bound, contains `projectionStatus = pending`
and the complete `MutationProjectionReceipt`. Exact and alias replay return the
original payload bytes. A caller obtains later `applied | degraded` state only
from the corresponding readback object.

Connection pagination follows one `agent_context_cursor_v1` contract shared by
the three fields. `first` defaults to 50 and must be `1...100`; `after` is an
opaque base64url canonical JSON cursor of at most 2048 bytes and expires after
one hour. It binds cursor version/scope, authenticated run, list kind, a
monotonic `snapshot_revision`, expiry, and the last ordering tuple. Ordering is
`agent_execution_id` for role/assignment and skill composition, and
`(prepared_revision, agent_execution_id, attempt_generation)` for dispatches.
The first page pins the maximum creation revision; later pages filter out rows
created after it. Readback identities are append-only, so concurrent inserts
cannot appear mid-snapshot or cause a skip/duplicate. A cursor for another
run/list, malformed/expired cursor, or unavailable snapshot returns the shared
`invalid_cursor` error with reason `filter_changed | malformed | expired |
snapshot_unavailable`; there is no offset fallback.

Swift drains pages until `hasNextPage = false`, rejects repeated cursors or node
identities, and never treats a first page as a complete list. Legacy and unknown
connections return `edges = []`, `endCursor = null`, `hasNextPage = false`,
`cursorVersion = 1`, no expiry, and the run read transaction's revision.
Redacted connections return the same shape with `snapshotRevision = "0"` so
pagination does not reveal hidden row activity. Golden vectors cover zero, one,
100, 101 and 10,000 rows plus inserts concurrent with every page boundary.

Swift uses lossless string wrappers with `.known` and `.unknown(rawValue)` for
every state-like field; it does not use exhaustive generated enums. Non-null
field absence is a typed transport/schema error and cannot default to healthy.
Golden compatibility fixtures cover legacy pre-P066, P066, V2 healthy,
V2 pending/degraded projection, unknown context/schema/status values, redacted
rows, and newer-server/older-client decoding. The macOS surface labels unknown
or degraded truth and never enables actions from it.

GraphQL, MCP reports, run reports, and the macOS read-only run surface expose:

- context schema version;
- base and pinned authority-chain heads, context revision, and conflict status;
- role and assignment identifiers;
- ordered skill IDs, bundle digests, aggregate composition digest, and broker
  health;
- prompt-envelope manifest digest;
- candidate manifest, promotion receipt, run generation, and dispatch phase;
- eval-suite and promotion-policy versions associated with the production
  skill/model promotion;
- current cutover state, proof state, generation continuation status, and
  required capability versions.

Operator readback is bounded and authorization-aware. It does not expose full
operator directives, raw prompts, secrets, or unrestricted artifact content to
non-operator principals.

## Gates

Add canonical gates:

```text
./scripts/test-gate.sh agent-skill-compliance
./scripts/test-gate.sh agent-context
./scripts/test-gate.sh agent-evals
./scripts/test-gate.sh agent-quality
```

`agent-quality` aggregates the three deterministic gates and is required when a
change touches workflow definitions, catalog definitions, prompt assembly,
skill bundles, production agent bindings, or eval assertions.

The gates execute these behavioral proofs:

- `agent-skill-compliance`: the pinned Agent Skills corpus; unknown metadata;
  traversal, Git symlink/submodule/special-mode, case-collision, and every
  normative cap; immutable tree/blob identity; concurrent working-tree
  add/remove/rename/write that cannot affect the selected tree; stable raw-byte
  manifest ordering; publication/orphan cleanup; ordered multi-skill rendering,
  namespaced duplicate resources, aggregate digest; source/snapshot/eval-vault
  denial and broker-only runtime consumption; materialized legacy migration
  coverage for every source node/fragment, disposition, execution mode, effect
  class and partial-failure policy, including omitted/duplicated/overlapping/
  digest-drift failures; package golden rendering for every declared mode;
  wrong grant/run/session/generation/composition non-enumerating denial; on
  every supported macOS build, `macos_seatbelt_exec_v1` path/signature/vnode and
  startup escape self-test, plus proof that unavailable or changed enforcement
  has no fallback; script stdin/argv/env/cwd, immutable verify-to-exec identity,
  read-only workspace/staging-only writes and child-process allowlist;
  credential, network, IPC, symlink, replacement-race and escape attacks;
  cancellation group reap and crash at every spawn/runner-settlement/apply-
  journal entry/fsync boundary, proving no unjournaled workspace effect or blind
  rerun for either script side-effect class.
- `agent-context`: canonical bytes and Rust/Swift digest parity for every
  normative schema; genesis/base-ledger order, empty/non-empty base chains and
  atomic head initialization; bounded approval creation, authority-head
  rebinding, unchanged-key `rebound`, changed-key `superseded`, Operator-only
  read/reissue, old-generation settlement denial, and concurrent reissue/
  settlement atomicity; exact legacy pre-P066/P066 discriminator mapping and
  context-absent readback; unknown schema/default/null/error response matrices;
  rejection of every V2 legacy prompt, `skill_role`, inline, builtin, and
  hardcoded role path; exactly one role section; typed directive/conflict-key
  vectors; per-command intent-field and closed error/retry matrices in Rust and
  Swift, including same-ID conflict, different-ID alias, pending winner/loser
  and post-commit replay for every mutation; full MCP request/result/error and
  selection-discovery vectors;
  PrincipalClass plus CallerClass/capability checks, AgentOperator confusion,
  live revocation/re-scope, malformed-before/after-auth precedence; CAS,
  supersession, duplicate, and concurrent append;
  run-start/promotion generation race; daemon self-admission, launch credential
  secrecy/replay/expiry, observed provider mismatch, invocation
  prepare/launch/prompt crash recovery, identity hold/verified clearance,
  session binding; a black-box Operator sequence of promote, cutover read,
  initial probe, exact replay, inconclusive settlement, atomic reprobe, open,
  process-only hold clearance and same-generation reprobe without direct DB
  access; every legacy phase/operation matrix cell and go/no-go inventory effect
  class across crash/restart; GraphQL legacy/V2/unknown/degraded decoding;
  mutation pending-receipt exact replay, singleton cutover readback, projection
  crash reconciliation, and keyset pages of 0/1/100/101/10,000 rows with
  concurrent inserts, invalid cursors, no skip/duplicate/truncation; and denial
  of every context-absent live continuation command and producer path.
- `agent-evals`: all ten retained failures, holdout isolation, deterministic
  hard assertions, exact trigger repetitions and per-profile precision/recall/
  specificity thresholds, single-writer state CAS and crash recovery, stable
  sample pairing, bounded retries, infra-inconclusive handling, fixed-point
  overflow/ties/median/p95 and SHA-256 counter bootstrap vectors, cost/latency
  ceilings, and offline byte-for-byte promotion-receipt replay; complete candidate identity
  observation; invalidation after every behavior-input mutation, including
  workflow-only changes; and proof that candidate code cannot enumerate or read
  holdout custody.
- `agent-quality`: invokes the three gates rather than scanning source, rejects
  missing constituent evidence, and runs the old-app/new-app by
  old-daemon/new-daemon cutover matrix; `closed_v2_pending`, one active probe
  epoch with single-use admissions, both same-generation reprobe reasons,
  `open_v2`, crash persistence, candidate-failed probe and
  `emergency_hold_v2` transitions; active provider/side-effect drain outcomes;
  every migration phase/operation crash/restart row, pre-cutover blocker
  rejection before phase mutation, atomic point-of-no-return legacy settlement,
  zero claimable legacy work across the complete producer inventory, unified
  transactional admission proof, promotion/probe/reprobe/open command replay,
  stale generation, invalid receipt, required projection-intent rollback,
  bounded current readback and no-new-context-absent database assertions.

The implementation proposal gate must invoke these commands and prove their
behavioral test counts and receipt digests. It may not scan for test names or
fixture strings. A successful command that executes zero selected tests fails
the gate.

## Migration and Compatibility

1. Inventory production prompts, `skill_role`, inline skills, external bundles,
   hardcoded role maps, existing snapshot representations, active legacy runs,
   all execution/side-effect producers, and duplicate sources without changing
   runtime behavior. Refresh the affected baseline from
   `docs/reference/current-system-baseline.md` and retain the routed review pack
   beside the implementation proposal.
2. Publish normative JSON Schemas/vectors and add Rust and Swift readers for
   legacy pre-P066, P066, and `agent_context_envelope_v2`. Keep production
   writes on the old path until every discriminator/error parity gate passes.
3. Add genesis/base-ledger serialization, `AuthorityDirectiveV1`,
   `ApprovalAuthorityTemplateV1`, versioned `ApprovalAuthorityBindingV1`,
   bounded rebinding/readback/reissue, atomic run head initialization, the
   Rust-owned authority overlay, and the single authority admission service with
   complete CAS/idempotency/auth parity tests.
4. Publish exact northbound discovery/request/result/error schemas and boundary
   rows. Implement PrincipalClass plus CallerClass/capability checks, coherent
   start selection, per-command semantic intent fields, the closed retry matrix,
   and shared MCP/Swift vectors before exposing mutations.
5. Pin the Agent Skills baseline; generate the complete planned legacy-fragment
   migration manifest; move evals outside bundles; add immutable Git tree
   admission, content-addressed publication, ordered composition,
   invocation-scoped broker grants, and the versioned macOS Seatbelt runner,
   startup self-test, script/apply journals and recovery. Keep the Swift loader
   diagnostic.
6. Rewrite every production role, `skill_role`, prompt, inline/builtin skill,
   workflow assignment, and skill composition exactly from the planned rows,
   then materialize target digests and require independent Rust/Swift fragment
   coverage plus all-mode package golden tests before V2 compile accepts it.
7. Add `RunContextGenerationBindingV2`, daemon self-admission, durable dispatch
   intents, one-use wrapper credentials, provider-binding receipts, identity
   holds/clearance, prompt fencing, and crash-recovery tests.
8. Replace prompt assembly with `PromptEnvelopeV2`, include all pin digests in
   session binding, add the additive V2 persistence/migration-ledger schema,
   atomic latest-head projection intents and immutable mutation receipts,
   startup reconciliation, singleton cutover readback, bounded keyset
   connections, and the exact GraphQL/Swift compatibility contract. The
   migration head remains `not_started` and V2 execution is not yet admitted.
9. Add the protected holdout vault, historical corpus, complete candidate
   manifest, single-writer evaluation service, fixed-point evaluator, exact
   trigger policy, deterministic gates, candidate/probe state stores, and
   capability handshake.
10. Run the complete five-repetition baseline/candidate live evaluation and
   retain a passing `AgentQualityPromotionReceiptV1` for the exact production
   candidate manifest.
11. Stop the bridge daemon, self-admit the final candidate binary, build and
    verify the complete go/no-go inventory and every external-effect
    classification/settlement action, then atomically advance
    `not_started / legacy_bridge -> schema_prepared / closed_v2_pending`; verify
    `admission_closed` before binding mutation surfaces or workers.
12. Drain active provider/effect truth, persist `legacy_settled`, execute the
    all-or-nothing `legacy_execution_sealed` settlement, and prove zero legacy
    producer row remains claimable.
13. Verify runtime self-admission, execute promotion to `probe_v2`, consume one
    proof epoch, classify its receipt, and execute open only after pass.
    Infrastructure-inconclusive or process-clearance proof uses the atomic
    same-generation reprobe command.
14. Read back `open_v2`, migration/projection heads, generation and proof/self-
    admission receipts before allowing an ordinary start.
15. Prove mixed-version errors, complete northbound auth/idempotency vectors,
    generation-CAS, migration-phase recovery, emergency hold, and
    context-absent live-execution denial matrices.
16. Run a small roadmap proposal end to end and let Steward perform the first
    post-run analysis.

There is no mixed production execution period or selectable flag. While the
migration head is `not_started`, the bridge release supports V2 read/eval
infrastructure but admits only the existing context-absent execution path. Once
`schema_prepared` commits, no new context-absent start is possible; once
`legacy_execution_sealed` commits, no context-absent continuation is possible.
Ordinary production run creation remains closed until `open_v2`; afterward
every new run binds the exact open V2 generation and the database has no
context-absent write path. Existing legacy snapshots remain immutable readback
records only.

## Acceptance Criteria

1. Every newly compiled production invocation contains
   `agent_context_envelope_v2`, a valid immutable `RunMissionV1` base, pinned
   `EffectiveRunMissionV1`, `AgentRoleV1`, `TaskAssignmentV1`, ordered
   `SkillCompositionV1`, `PromptEnvelopeV2`, and a durable generation binding.
2. Every production agent has an explicit role charter and no production agent
   uses `inline_skill`, `builtin_agent`, `skill_role`, an arbitrary catalog
   prompt, `roles/*.md` role authority, or a hardcoded role map.
3. Rust and Swift reconstruct identical genesis, ordered base ledger, empty-base
   head, base events and overlay chain. `runs.start` cannot commit a Run without
   atomically initializing `run_authority_heads` at revision zero.
4. `AuthorityDirectiveV1` rejects unknown fields/enums, invalid bounds, invalid
   conflict keys and ambiguous values; semantic digest, active-set and
   supersession vectors are byte-identical across runtimes.
5. `ApprovalAuthorityTemplateV1` freezes decision semantics before presentation,
   while MCP and GraphQL expose the exact current binding generation, digest and
   head. Every authority-head change creates exactly one successor generation:
   unchanged conflict keys produce actionable `rebound`, changed keys produce
   non-actionable `superseded`, and only Operator reissue can create the next
   actionable generation. An old/stale/superseded generation never settles or
   admits work. Concurrent append, reissue and settlement commit their complete
   command/audit/projection transaction or nothing.
6. Every northbound tool passes exact request/result/error vectors and the
   ordered validation matrix. MCP mutation requires `PrincipalClass::Operator`,
   derived `CallerClass::AgentOperator` and its capability; an `Agent` principal
   cannot inherit mutation authority through the shared caller class. Revoked,
   disabled and re-scoped principals fail before schema or resource disclosure.
7. Coherent start-selection readback supplies every promoted digest. The
   published per-command semantic-intent table excludes both request IDs.
   UUIDv7 exact-request replay, UUIDv4 exact/alias replay, same-ID different-
   intent conflict, in-flight winner/loser, semantic duplicate, stale CAS and
   concurrent-loser outcomes match the closed Rust/Swift error/retry matrix for
   every mutation and never double-apply it.
8. An accepted mid-run directive changes only invocations that prepare after its
   durable head. A running or historically reconstructed invocation uses its
   original head and byte-identical effective context without rewriting
   `RunPlanSnapshot`.
9. Every production skill passes Agent Skills metadata, immutable Git
   tree/object validation, exact resource limits, and same-byte publication.
   The materialized migration manifest accounts for every byte of every legacy
   source fragment exactly once as a typed target or reviewed retirement and
   preserves execution mode, effect owner and partial-failure semantics;
   package goldens cover every mode. Working-tree add/remove/rename/write races
   cannot affect selected or published bytes.
10. Multi-skill prompt assembly is ordered and byte-exact. Broker grants are
    bound to active execution/attempt/session/generation/composition truth;
    wrong-scope requests reveal no object existence, and source bundles,
    snapshot storage and holdout custody remain inaccessible.
11. `SkillScriptExecutionContractV1` tests prove fixed stdin/argv/env/cwd and
    the exact `macos_seatbelt_exec_v1` startup identity/escape self-test on every
    supported macOS build, with no advisory or unsandboxed fallback. Immutable
    verify-to-exec identity, deny-by-default filesystem/network/IPC/credential/
    child-process confinement, read-only workspace plus staging-only writes,
    bounded output and full process-group cancellation hold under replacement
    races. Crash at every spawn, runner-settlement and apply-journal/fsync
    boundary either resumes the durable entry, proves no effect, or enters typed
    reconciliation without overwrite or script rerun.
12. `prepared` invocation creation atomically pins generation, authority,
   assignment, skill, prompt, provider, and session-binding truth before launch.
   Crash-before-launch, launch-before-prompt, prompt-committed, ambiguous
   process, and late-output tests produce the specified durable outcomes with no
   blind prompt resend.
13. A daemon without a matching self-admission receipt cannot claim work. A
    launch credential is secret, one-use, expiry-bound and replay-proof; actual
    daemon/provider/model/tool identity must match an observed provider-binding
    receipt before `prompt_committed`. Identity ambiguity blocks every retry
    until the existing process-absence command verifies clearance.
14. `runs.start` atomically binds the open cutover generation, complete candidate
   manifest, promotion receipt, workflow/catalog bytes, skill registry, and
   assignment templates; invocation pins bind run-specific assignments.
   Concurrent promotion/start cannot create a mixed-generation run, and exact
   start replay returns the original run.
15. Rust and Swift accept and reject identical normative schema/canonical-byte
    vectors for legacy pre-P066, P066, and V2 readers, every hashed contract,
    unknown discriminator, null/default behavior, and exact MCP/GraphQL error
    payload.
16. The exact GraphQL SDL and Swift DTO fixtures decode legacy, V2, unknown
    version/status, redacted and degraded rows without optimistic defaults or
    exhaustive-enum failure. Singleton cutover readback and all three cursor
    connections are bounded; 0/1/100/101/10,000-row and concurrent-insert
    vectors have no skip, duplicate, hidden truncation or offset fallback.
17. Every canonical state mutation commits every required projection intent
    atomically and returns `projection_status = pending` plus the complete
    immutable commit receipt. Exact and alias replay remain byte-identical after
    worker progress. Crash and filesystem-failure tests prove current readback
    returns the SQLite head with pending/applied/degraded health and never a
    stale file; inability to persist an intent rolls back the mutation.
18. The ten initial historical cases fail against their retained faulty baseline
   where applicable and pass against the candidate implementation.
19. Deterministic `agent-quality` gates pass on the implementation tree and no
    selected test lane succeeds with zero tests.
20. The Rust evaluation service is the only early candidate-state writer.
    Trigger repetition/precision/recall/specificity, fixed-point arithmetic,
    SHA-256 counter bootstrap, rounding, percentile and crash/CAS vectors always
    produce one byte-identical receipt and decision; incomplete, inconclusive or
    identity-mismatched evidence cannot create `live_passed`.
21. Production promotion uses a complete five-repetition passing receipt with
    zero hard regression, declared target improvement, non-inferiority bounds
    and cost/latency ceilings. Changing any daemon/compiler/app/schema/workflow/assignment/catalog/role/
    permission/output-contract/skill/tool/provider/model/eval input changes the
    candidate manifest and invalidates the receipt. Workflow-only changes run
    the full live lane, and candidate code cannot enumerate holdout cases.
22. Promotion keeps ordinary admission closed across crashes through
    `closed_v2_pending` and one active probe epoch. A black-box Operator can use
    only documented MCP/readback fields to promote, read and consume the initial
    proof admission, replay it, settle an inconclusive probe, atomically reprobe,
    open, clear a process-only hold and atomically reprobe the same generation.
    Candidate failure or another typed invariant persists `emergency_hold_v2`;
    only a current passing receipt can open `open_v2`.
23. Failure at every migration/probe phase has the documented restart action,
    and every producer operation is decided by the single phase/operation
    matrix. A missing, stale, unknown or unbounded external-effect inventory
    blocks before phase mutation. The `legacy_execution_sealed` transaction is
    an atomic, auditable point of no return and leaves no pending legacy
    approval/work/retry falsely runnable.
24. Every work producer uses the same transaction-scoped execution-admission
    predicate. Inventory and startup tests prove zero claimable context-absent
    row after seal and quarantine any producer lacking proof.
25. Every active provider and side-effect state satisfies the initial drain and
    later V2 continuation matrices. Each prepared/started effect has one bounded
    settle, cancel or typed reconcile action and each operation is fault-tested
    in every phase. No unverified process or external write is ignored to
    complete cutover.
26. Old/new app and daemon tests prove no combination can create a
    context-absent run after cutover; stale, duplicate, concurrent, and invalid
    promote/probe/open commands leave generation and admission state unchanged.
    Context-absent snapshots remain byte-identical and fully readable, but every
    command capable of live provider/tool/workspace/side-effect continuation is
    rejected with `legacy_context_execution_prohibited` and has no operator
    override. No feature flag, environment switch, arbitrary operator disable action,
    database downgrade, or legacy fallback can bypass V2. Emergency hold closes
    admission only on typed evidence and never selects another implementation.
27. A small roadmap run reaches its expected terminal state and Steward records
    context, role, skill, convergence, and quality evidence for it.

## Readiness Review Disposition

| Finding | Resolution in this revision |
|---|---|
| Prior `P0-01` | Frozen mission base plus Rust-owned SQLite overlay remains the single authority model |
| Prior readiness rounds | Directive, generation, discriminator, immutable skills, candidate identity, dispatch recovery, deterministic evaluator, cutover states and legacy readback contracts remain retained |
| Latest `P1-01` | Per-command semantic-intent fields exclude both request IDs; the closed Rust/Swift command error and retry matrix defines exact, alias, conflict, in-flight, stale and concurrent outcomes |
| Latest `P1-02` | Operator MCP/GraphQL binding readback, bounded head-driven `rebound`/`superseded` generations, explicit reissue and old-generation settlement denial complete the approval lifecycle |
| Latest `P1-03` | Promote/readback expose the one-use proof admission; one atomic reprobe command safely mints and consumes admissions for both inconclusive and process-clearance paths |
| Latest `P1-04` | `legacy_bridge`, pre-cutover inventory go/no-go and one phase-by-producer-operation matrix remove the `closed_v2_pending` cutoff ambiguity |
| Latest `P1-05` | Materialized `LegacySkillMigrationManifestV1` partitions every legacy source byte into one disposition with mode, effect and partial-failure ownership, enforced by package goldens |
| Latest `P1-06` | `macos_seatbelt_exec_v1`, startup escape self-test, immutable verify-to-exec root, staging-only writes and durable apply-journal recovery make executable confinement testable and fail-closed |
| Latest `P1-07` | Every mutation has an immutable pending projection receipt; singleton cutover readback and bounded snapshot keyset connections define exact MCP/GraphQL/Swift projection truth |

## Implementation Boundary

This document specifies the revised architecture but does not approve code
changes while proposal-readiness re-review is pending. After a ready verdict,
the next artifact is a file-by-file implementation plan with tests ordered
before production migration. Implementation must preserve unrelated dirty work
and must not begin until that plan has been reviewed.
