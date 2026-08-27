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

| Current skill ID | Current form | Canonical bundle disposition |
|---|---|---|
| `proposal_review_triad` | inline compatibility alias | retire for new catalogs; historical snapshot compatibility only |
| `proposal_review_router_skill` | inline | `proposal-review-router` |
| `proposal_implementation_audit` | inline | `proposal-implementation-audit` |
| `docs_quality_guardian` | builtin | `docs-quality-guardian` |
| `orchestrator_core` | inline | `lead-orchestration` |
| `proposal_writer_core` | inline | `proposal-authoring` |
| `code_writer_core` | inline | `code-implementation` |
| `security_checker_core` | inline | `security-review` |
| `prepush_review_core` | inline | `prepush-review` |
| `github_commit_push` | inline | `github-delivery` |
| `connect_publisher` | inline | `connect-publishing` |
| `steward_core` | inline | `forge-steward` |

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
  plus exact source digest and revision identifier;
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
| `operator_directive` | authenticated `Operator` | `RunsAuthorityAppend` | MCP tool must be `runs.authority.append` |
| `approval_decision` | authenticated `Operator` that settled the approval | `ApprovalsResolve` | non-forgeable settled approval ID, decision, and settlement journal ID |
| `operator_idea` or `approved_proposal` base event | run compiler system identity | `RunPlanCompile` | frozen source digest and compile journal ID |

`approval_decision` is not a second write path. Approval settlement calls the
same admission service with an engine-issued transaction capability that is
bound to the authenticated approval command, principal, run, approval, and
decision. The authority append, approval state transition, command outcome, and
audit rows commit in one SQLite transaction. A caller-supplied source kind,
principal ID, capability, approval ID, or system identity is ignored or rejected
rather than trusted.

The semantic intent hash binds `run_id`, expected head, the adapter-derived
source kind, `directive_sha256`, and the sorted `supersedes` set.
`caller_request_id` uses the existing `command_idempotency` and
`command_request_aliases` contract:

- replay of the same request and intent returns the stored response;
- reuse of a request ID for different intent fails with
  `command_idempotency_conflict`;
- a different request ID that proposes an already active event digest fails
  with `authority_duplicate_event` and creates no second event;
- a stale expected head fails with `authority_head_conflict` and performs no
  append;
- concurrent appends against one head allow exactly one transaction to commit;
- append event, head CAS update, command outcome, and audit row commit in one
  SQLite transaction.

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
prepared|launching|provider_bound -> cancelled|failed_closed
prompt_committed|observing -> settled|reconciliation_required
reconciliation_required -> settled|failed_closed
```

Each transition is a compare-and-swap over execution ID, attempt generation,
current state, and lease epoch. A control-plane supervisor wrapper must register
its PID, process group, UID, process-birth fingerprint, and launch token against
the durable `launching` row before it execs the provider binary. If registration
or the `provider_bound` transition fails, the wrapper exits and the control
plane reaps the entire verified process group. Provider initialization and
prompt transport cannot start before `provider_bound` commits.

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
| `launching` without a registered wrapper | wait for bounded registration deadline, then fail closed; do not assume no process from elapsed time alone |
| `launching` or `provider_bound` with verified process and no prompt commit | terminate/reap it, then create a new attempt generation from current authority |
| `prompt_committed` or later | attach/reconcile by pinned turn identity; never send the prompt again |
| process identity ambiguous or cleanup unverified | permanent hold with no retry or new provider launch |

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

`runs.start` V2 requires a `RunStartContextSelectionV2` with the caller's
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
| `authority_directive_v1` | `directive_kind`, `conflict_key`, `enforcement`, tagged `value` | `directive_sha256` |
| `authority_event_v1` | event/run IDs, source kind/ref/digest/revision, directive, prior head, supersedes, accepted principal/capability/table revision, timestamps | `event_sha256` |
| `run_mission_v1` | run ID, objective, authoritative directives, scope, non-goals, success criteria, base head, source refs | `content_sha256` |
| `effective_run_mission_v1` | base mission digest, overlay head/revision, ordered active event IDs, effective objective/scope/non-goals/success criteria | `effective_mission_sha256` |
| `agent_role_v1` | agent ID, mission, owns, does-not-own, success criteria | `role_sha256` |
| `task_assignment_v1` | state/task IDs and purposes, done-when, typed input/blocker/output refs, consumers, permission profile ID, ordered skill IDs | `assignment_sha256` |
| `skill_bundle_manifest_v1` | skill ID/name/spec revision, immutable source tree, sorted files and totals | `bundle_sha256` |
| `skill_composition_v1` | ordered entries and aggregate resource totals | `skill_composition_sha256` |
| `prompt_envelope_manifest_v2` | ordered section names/digests/byte counts, exact prompt byte count, context generation and all protected component digests | `prompt_envelope_sha256` |
| `agent_context_candidate_manifest_v1` | complete behavior candidate identities defined below | `candidate_sha256` |
| `agent_quality_promotion_receipt_v1` | candidate/baseline/policy/suite identities, pair receipts, metrics, bounds, resource ratios, decision | `receipt_sha256` |
| `run_context_generation_binding_v2` | cutover generation and every promoted/compiled digest selected by run start | `binding_sha256` |
| `agent_context_envelope_v2` | generation binding, base/effective mission, authority projection, role, assignment, skill composition, input/output refs, runtime IDs | `envelope_sha256` |

All arrays in that table preserve declared order except these schema-declared
sets, which sort by raw canonical key: authority `supersedes` by event ID,
mission source refs by `(source_kind, source_id, revision)`, role ownership and
criteria strings by UTF-8 bytes, and skill manifest files by portable path.
Typed assignment inputs, blockers, outputs, and skill composition remain ordered
because prompt and workflow order are semantic.

### Run-Start Wire Shape

The existing MCP `runs.start` keeps its required UUIDv7 `idempotency_key` and
adds one required object after cutover:

```json
{
  "context_selection": {
    "schema_version": "run_start_context_selection_v2",
    "agent_context_envelope_schema": "agent_context_envelope_v2",
    "expected_cutover_generation": 7,
    "candidate_manifest_sha256": "sha256:...",
    "promotion_receipt_sha256": "sha256:...",
    "workflow_snapshot_sha256": "sha256:...",
    "catalog_snapshot_sha256": "sha256:...",
    "skill_registry_sha256": "sha256:..."
  }
}
```

The successful V2 result adds, without removing the existing run fields:

```json
{
  "context_admission": {
    "schema_version": "run_start_context_admission_v2",
    "run_id": "...",
    "cutover_generation": 7,
    "candidate_manifest_sha256": "sha256:...",
    "promotion_receipt_sha256": "sha256:...",
    "run_context_binding_sha256": "sha256:...",
    "agent_context_envelope_schema": "agent_context_envelope_v2"
  }
}
```

Malformed or unknown wire fields use JSON-RPC `-32602`. Auth denial uses the
existing `-32004` policy envelope. Version, generation, digest, or cutover-state
admission failures use JSON-RPC `-32009`, message `context admission denied`,
and this exact bounded data shape:

```json
{
  "schema_version": "agent_context_admission_error_v1",
  "classification": "agent_context_version_incompatible",
  "retryable": false,
  "expected_cutover_generation": 7,
  "expected_agent_context_envelope_schema": "agent_context_envelope_v2",
  "current_state": "open_v2",
  "request_id": "..."
}
```

`classification` is exactly one typed failure from the Runtime Failure
Contract. Authorized Operators receive safe expected generation/state fields;
other callers receive the ordinary non-enumerating policy denial. GraphQL
readback uses the same field names and enum values, and GraphQL errors place the
same object under `extensions.chainworks`; GraphQL does not expose a run-start
mutation.

## Canonical Bytes and Digest Contract

All cross-runtime structured digests use
`chainworks_canonical_digest_v1`:

1. Construct the contract payload without its self-digest field. The excluded
   field is explicit per schema: `directive_sha256`, `event_sha256`,
   `content_sha256`, `effective_mission_sha256`, `role_sha256`,
   `assignment_sha256`, `bundle_sha256`, `skill_composition_sha256`,
   `prompt_envelope_sha256`, `candidate_sha256`, `receipt_sha256`,
   `binding_sha256`, or `envelope_sha256`.
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

### Production Migration

- Every production `inline_skill` becomes a canonical external bundle.
- Long catalog prompts are decomposed into role, skill, runtime policy, and
  output-contract ownership.
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
description. It runs multiple times to expose trigger instability.

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
  registry, ordered skill composition, runtime tool policy, and provider adapter
  digests;
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

1. All deterministic gates and trigger evals pass before live calls begin.
2. Candidate hard failures are zero across every completed sample. Provider
   refusal caused by candidate behavior, malformed output, permission misuse,
   timeout after provider admission, and contract failure are behavioral
   results, not infrastructure exclusions.
3. For first-pass artifact validity, blocker precision, false-positive rate,
   and refinement cycles, a seeded paired bootstrap with 10,000 resamples
   computes a one-sided 95 percent confidence bound. The seed is the SHA-256 of
   the policy, suite, baseline, and candidate digests. Every delta is oriented
   so positive means the candidate is better. The candidate is non-inferior
   only when the fifth percentile is at least -0.02 for rate metrics and at
   least -0.25 for refinement cycles.
4. Every candidate declares one target metric and direction before results are
   visible. A rate target must improve by at least 0.05 absolute and a cycle
   target by at least 0.50 mean cycles; the fifth percentile of its
   improvement-oriented paired delta must be greater than zero. A correctness
   candidate may instead name one retained failure case: the baseline must
   fail at least three of five repetitions and the candidate must pass five of
   five.
5. Candidate median input-plus-output tokens, billed cost, and wall latency
   must each be no more than 1.15 times baseline. Their p95 values must each be
   no more than 1.25 times baseline. A missing supported usage or cost field is
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
declared before execution, and final decision. A pure offline evaluator must
reproduce the receipt decision from those bytes. Promotion consumes the receipt
by digest and never reinterprets mutable dashboard state. A candidate manifest
or observed-identity mismatch makes the receipt invalid rather than merely
stale.

### Production Promotion and Cutover

Promotion state is durable rather than inferred from files or the currently
running app. Candidate records move monotonically through:

```text
candidate_registered -> deterministic_passed -> live_pending
live_pending -> live_inconclusive -> live_pending
live_pending -> live_passed -> probe_active -> promoted
probe_active -> probe_failed -> superseded
candidate_registered|deterministic_passed|live_pending|live_inconclusive -> rejected
candidate_registered|deterministic_passed|live_pending|live_inconclusive -> superseded
```

Only the MCP command `runtime.agent_context.promote`, protected by the
`AgentContextPromoteV2` capability and Operator caller class, may move a
`live_passed` candidate to `probe_active`. It requires a lowercase UUIDv4
`caller_request_id`, candidate ID and digest, passing receipt digest, expected
candidate state, and expected cutover generation. Those fields form its
semantic intent hash. Command outcome, audit event, catalog and skill registry
heads, generation row, and cutover marker update commit atomically. Replay of the same request
returns the stored response; stale generation, mismatched intent, unauthorized
caller, concurrent loser, or a non-passing receipt fails without changing
production truth.

SQLite owns a singleton `agent_context_cutover_v1` row with:

- state `closed_v2_pending`, `probe_v2`, `open_v2`, or
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
closed_v2_pending --promote passing candidate--> probe_v2
open_v2 --promote passing candidate--> probe_v2
emergency_hold_v2 --promote forward fix--> probe_v2
probe_v2 --open with passing proof receipt--> open_v2
probe_v2|open_v2 --typed invariant breach--> emergency_hold_v2
```

There is no transition to a legacy/context-absent mode. The database migration
creates `closed_v2_pending`. `closed_v2_pending`, `probe_v2`, and
`emergency_hold_v2` reject ordinary production `runs.start`. Development
fixtures use a separate test-only database and entry point and cannot write
production run rows.

`probe_v2` permits exactly one Operator-only
`runtime.agent_context.probe_start` command with capability
`AgentContextProbeStartV2`. The command binds the proof admission ID, expected
generation, candidate digest, fixed read-only `agent-context-cutover-probe`
workflow digest, and UUIDv7 idempotency key. In one transaction it consumes the
unused proof admission ID, creates one real V2 run, and stores `proof_run_id`.
Exact replay returns that run; a different second request fails. Ordinary run
creation and every other candidate remain closed before, during, and after a
crash.

The probe workflow uses the production compiler, prompt renderer, skill broker,
provider adapter, and persistence path but has no code-write, Git, publish, or
external side-effect capability. Terminal success writes one immutable
`AgentContextProbeReceiptV1` that binds run, generation, candidate, actual
binary/context/skill identities, and required output proof. Operator-only
`runtime.agent_context.open`, with `AgentContextOpenV2`, lowercase UUIDv4
`caller_request_id`, expected generation, proof run ID, and passing proof
receipt digest, atomically moves `probe_active` to `promoted` and the marker to
`open_v2`. Failed proof automatically enters `emergency_hold_v2`.

`emergency_hold_v2` is not a feature toggle. It is entered only by a persisted
failed probe or a closed invariant detector for digest mismatch, unsupported
schema, execution-pin corruption, or unverified provider-process ownership. It
blocks new runs and new prompt/side-effect dispatch for affected generations;
an arbitrary operator preference cannot create it or clear it. Exit requires a
new passing candidate, a new generation, and a new proof. No hold path selects
legacy behavior.

The daemon advertises `AgentContextV2`, `PromptEnvelopeV2`,
`SkillBundleSnapshotV1`, `AgentQualityPromotionPolicyV1`, the supported
snapshot-read versions, and the cutover generation through MCP initialize,
`runtime.health`, and the GraphQL capability readback. The Swift app advertises
its supported request and readback versions on `runs.start`. After
`open_v2`, a start request must explicitly select the marker's required V2
versions; omission or mismatch fails with `agent_context_version_incompatible`.
Historical context-absent readback remains supported, but no surface can compile
or persist a new context-absent run.

Cutover uses a maintenance restart, not a mixed-version rolling window:

1. drain new-run admission and quiesce legacy executions and side effects under
   the rules below;
2. stop the old daemon, start the new daemon, apply the migration, and enter
   `closed_v2_pending`;
3. verify the new app/daemon capability handshake, canonical catalog and skill
   snapshot digests, and a complete passing promotion receipt;
4. execute the idempotent Operator promotion command to enter `probe_v2`;
5. start the one allowed probe, verify its terminal receipt, and execute the
   idempotent open command;
6. read back `open_v2`, its generation, and the proof receipt before ordinary
   run admission becomes available.

Initial legacy-to-V2 cutover requires no context-absent provider work or
unsettled external side effect to remain. The drain matrix is:

| Durable pre-cutover state | Required outcome before migration |
|---|---|
| queued/unprepared provider work | pause durably; it will not be executable after cutover |
| prepared but no provider process | cancel and settle the attempt |
| launching/provider-bound before prompt | cancel, verify process-group reap, and settle |
| prompt committed/observing | wait for terminal result or cancel with late-output quarantine; require terminal settlement |
| side-effect prepared but external write not started | cancel/release intent durably |
| external write started | complete or enter existing reconciliation state; unresolved state blocks cutover |
| terminal run/execution/side effect | unchanged and available for readback |

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
talking to the new daemon omits the required version selection and receives a
typed rejection. A new app and new daemon may start ordinary runs only in
`open_v2`. This compatibility matrix therefore has no state that can create a
new context-absent run after cutover.

| Swift app | Daemon/database | New-run result |
|---|---|---|
| old | old daemon with migrated database | daemon startup fails newer-than-binary preflight |
| new | old daemon before migration | app observes missing V2 capability and does not submit |
| old | new daemon, any V2 state | typed cutover/version rejection |
| new | new daemon, `closed_v2_pending` | `agent_context_cutover_not_ready` |
| new | new daemon, `probe_v2` | only the single operator proof command is admitted |
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

After the migration creates `closed_v2_pending`, every action that could create
a provider invocation or side-effect intent for a context-absent run fails with
`legacy_context_execution_prohibited`. This includes approval settlement that
would advance into executable work, retry, resume, escalation, continuation,
and provider-session resurrection. There is no operator override. Read-only
queries and artifact export remain available, and fixture-only simulation runs
against a separate test database may exercise the legacy reader without a live
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
- `authority_directive_invalid`;
- `authority_append_unauthorized`;
- `authority_duplicate_event`;
- `authority_head_conflict`;
- `authority_supersession_invalid`;
- `agent_role_contract_missing`;
- `task_assignment_contract_invalid`;
- `skill_bundle_invalid`;
- `skill_source_not_immutable`;
- `skill_composition_invalid`;
- `skill_resource_unavailable`;
- `skill_snapshot_publication_failed`;
- `prompt_envelope_budget_exceeded`;
- `invocation_dispatch_reconciliation_required`;
- `agent_context_generation_conflict`;
- `agent_quality_promotion_inconclusive`;
- `agent_quality_promotion_receipt_invalid`;
- `agent_quality_promotion_state_conflict`;
- `agent_context_cutover_not_ready`;
- `agent_context_version_incompatible`;
- `agent_context_probe_already_consumed`;
- `agent_context_probe_failed`;
- `agent_context_emergency_hold`;
- `legacy_context_execution_prohibited`.

These failures occur before provider dispatch when possible. They preserve the
session and do not imply provider-output quarantine. Error readback gives the
operator the invalid contract path, safe identifier, and next action without
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
inputs. A projection write failure marks readback degraded and is repairable
from SQLite without changing the accepted authority chain.

SQLite is authoritative for:

- `run_authority_events` and `run_authority_heads`;
- `run_context_generation_bindings`, `agent_invocation_dispatches`, and
  execution context pins, including base/overlay heads, launch phase, and all
  effective context digests;
- skill snapshot publication receipts and the bundle digest referenced by each
  run snapshot;
- candidate manifests, immutable sample, promotion, and probe receipt indexes,
  promotion state, and command outcomes;
- `agent_context_generations` continuation policy and the singleton
  `agent_context_cutover_v1` state, generation, proof fence, and required
  versions.

Large skill resources, raw bounded eval evidence, and prompt evidence remain
file-spooled and content-addressed. SQLite references them by digest and stores
bounded health diagnostics. Missing or mismatched referenced bytes fail
readback or dispatch closed; they are never silently regenerated from a mutable
catalog.

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
  denial and broker-only runtime consumption.
- `agent-context`: canonical bytes and Rust/Swift digest parity for every
  normative schema; exact legacy pre-P066/P066 discriminator mapping and
  context-absent readback; unknown schema/default/null/error response matrices;
  rejection of every V2 legacy prompt, `skill_role`, inline, builtin, and
  hardcoded role path; exactly one role section; typed directive/conflict-key
  vectors; MCP and approval authorization; CAS, supersession, duplicate, and
  concurrent append; run-start/promotion generation race; invocation
  prepare/launch/prompt crash recovery; session binding; and denial of every
  context-absent live continuation command.
- `agent-evals`: all ten retained failures, holdout isolation, deterministic
  hard assertions, stable sample pairing, bounded retries, infra-inconclusive
  handling, seeded confidence bounds, cost/latency ceilings, and offline
  byte-for-byte promotion-receipt replay; complete candidate identity
  observation; invalidation after every behavior-input mutation, including
  workflow-only changes; and proof that candidate code cannot enumerate or read
  holdout custody.
- `agent-quality`: invokes the three gates rather than scanning source, rejects
  missing constituent evidence, and runs the old-app/new-app by
  old-daemon/new-daemon cutover matrix; `closed_v2_pending`, single-use
  `probe_v2`, `open_v2`, crash persistence, failed-probe and
  `emergency_hold_v2` transitions; active provider/side-effect drain outcomes;
  promotion/open command replay, stale generation, invalid receipt, and
  no-new-context-absent database assertions.

The implementation proposal gate must invoke these commands and prove their
behavioral test counts and receipt digests. It may not scan for test names or
fixture strings. A successful command that executes zero selected tests fails
the gate.

## Migration and Compatibility

1. Inventory production prompts, `skill_role`, inline skills, external bundles,
   hardcoded role maps, existing snapshot representations, active legacy runs,
   and duplicate sources without changing runtime behavior.
2. Publish normative JSON Schemas/vectors and add Rust and Swift readers for
   legacy pre-P066, P066, and `agent_context_envelope_v2`. Keep production
   writes on the old path until every discriminator/error parity gate passes.
3. Add `AuthorityDirectiveV1`, the Rust-owned authority overlay, the single
   admission service, execution pins, operator command, and approval settlement
   integration with authorization, CAS, idempotency, conflict, and parity tests.
4. Pin the Agent Skills baseline; move evals outside bundles; add immutable Git
   tree admission, content-addressed publication, ordered composition, and
   broker-only resource/script consumption. Keep the Swift loader diagnostic.
5. Rewrite every production role, `skill_role`, prompt, inline/builtin skill,
   workflow assignment, and skill composition into the canonical V2 sources.
6. Add `RunContextGenerationBindingV2`, durable invocation dispatch intents,
   supervisor registration, prompt commit fencing, and crash-recovery tests.
7. Replace prompt assembly with `PromptEnvelopeV2`, include all pin digests in
   session binding, and add bounded cross-surface readback.
8. Add the protected holdout vault, historical corpus, complete candidate
   manifest, pure promotion evaluator, deterministic gates, candidate/probe
   state stores, and exact capability handshake.
9. Run the complete five-repetition baseline/candidate live evaluation and
   retain a passing `AgentQualityPromotionReceiptV1` for the exact production
   candidate manifest.
10. Close run admission and satisfy every initial legacy provider/side-effect
    drain row. Stop the old daemon, start the new daemon, apply the migration,
    and verify durable `closed_v2_pending` plus app/daemon capabilities.
11. Execute promotion to `probe_v2`, consume the one proof admission, verify the
    probe receipt, execute open, and read back `open_v2` before allowing an
    ordinary start.
12. Prove the mixed-version rejection, generation-CAS, emergency-hold, and
    context-absent live-execution denial matrices.
13. Run a small roadmap proposal end to end and let Steward perform the first
    post-run analysis.

There is no mixed production period selected by a flag. Before migration,
development fixtures exercise all readers. From `closed_v2_pending` onward,
context-absent live execution is prohibited. Ordinary production run creation
is closed until `open_v2`; afterward every new run binds the exact open V2
generation and the database has no context-absent write path. Existing legacy
snapshots remain immutable readback records only.

## Acceptance Criteria

1. Every newly compiled production invocation contains
   `agent_context_envelope_v2`, a valid immutable `RunMissionV1` base, pinned
   `EffectiveRunMissionV1`, `AgentRoleV1`, `TaskAssignmentV1`, ordered
   `SkillCompositionV1`, `PromptEnvelopeV2`, and a durable generation binding.
2. Every production agent has an explicit role charter and no production agent
   uses `inline_skill`, `builtin_agent`, `skill_role`, an arbitrary catalog
   prompt, `roles/*.md` role authority, or a hardcoded role map.
3. `AuthorityDirectiveV1` rejects unknown fields/enums, invalid bounds, invalid
   conflict keys, and ambiguous values; Rust and Swift reconstruct identical
   active authority bytes from every normative event-log vector.
4. MCP and approval-derived authority use one live-principal admission service.
   Unauthorized approval, spoofed provenance, semantic duplicate, stale head,
   invalid supersession, contradiction, and losing concurrent append create no
   event, approval transition, or head change; exact replay is idempotent.
5. An accepted mid-run directive changes only invocations that prepare after its
   durable head. A running or historically reconstructed invocation uses its
   original head and byte-identical effective context without rewriting
   `RunPlanSnapshot`.
6. Every production skill passes Agent Skills metadata, immutable Git
   tree/object validation, exact resource limits, and same-byte publication.
   Working-tree add/remove/rename/write races cannot affect selected or
   published bytes.
7. Multi-skill prompt assembly is ordered and byte-exact, resources are
   digest-namespaced, and providers can consume them only through the bounded
   broker. Source bundles, snapshot storage, and holdout custody are
   inaccessible to provider tools.
8. `prepared` invocation creation atomically pins generation, authority,
   assignment, skill, prompt, provider, and session-binding truth before launch.
   Crash-before-launch, launch-before-prompt, prompt-committed, ambiguous
   process, and late-output tests produce the specified durable outcomes with no
   blind prompt resend.
9. `runs.start` atomically binds the open cutover generation, complete candidate
   manifest, promotion receipt, workflow/catalog bytes, skill registry, and
   assignment templates; invocation pins bind run-specific assignments.
   Concurrent promotion/start cannot create a mixed-generation run, and exact
   start replay returns the original run.
10. Rust and Swift accept and reject identical normative schema/canonical-byte
    vectors for legacy pre-P066, P066, and V2 readers, every hashed contract,
    unknown discriminator, null/default behavior, and exact MCP/GraphQL error
    payload.
11. The ten initial historical cases fail against their retained faulty baseline
   where applicable and pass against the candidate implementation.
12. Deterministic `agent-quality` gates pass on the implementation tree and no
    selected test lane succeeds with zero tests.
13. Production promotion uses a complete five-repetition passing receipt whose
    decision is reproduced offline, with zero hard regression, the declared
    target improvement, non-inferiority confidence bounds, and cost/latency
    ceilings all satisfied.
14. Changing any daemon/compiler/app/schema/workflow/assignment/catalog/role/
    permission/output-contract/skill/tool/provider/model/eval input changes the
    candidate manifest and invalidates the receipt. Workflow-only changes run
    the full live lane, and candidate code cannot enumerate holdout cases.
15. Promotion keeps ordinary admission closed across crashes through
    `closed_v2_pending` and single-use `probe_v2`; only a passing proof receipt
    can open `open_v2`. Failed proof or typed invariant breach persists
    `emergency_hold_v2` and requires a forward-fixed generation and new proof.
16. Every active provider and side-effect state satisfies the initial drain and
    later V2 continuation matrices. No unverified process or external write is
    ignored to complete cutover.
17. Old/new app and daemon tests prove no combination can create a
    context-absent run after cutover; stale, duplicate, concurrent, and invalid
    promote/probe/open commands leave generation and admission state unchanged.
18. Context-absent snapshots remain byte-identical and fully readable, but every
    command capable of live provider/tool/workspace/side-effect continuation is
    rejected with `legacy_context_execution_prohibited` and has no operator
    override.
19. No feature flag, environment switch, arbitrary operator disable action,
    database downgrade, or legacy fallback can bypass V2. Emergency hold closes
    admission only on typed evidence and never selects another implementation.
20. A small roadmap run reaches its expected terminal state and Steward records
    context, role, skill, convergence, and quality evidence for it.

## Readiness Review Disposition

| Finding | Resolution in this revision |
|---|---|
| Prior `P0-01` | Frozen mission base plus Rust-owned SQLite overlay remains the single authority model |
| `P1-01` | Closed `AuthorityDirectiveV1`, semantic conflict/supersession keys, canonical bytes, bounds, and one live-principal admission service for MCP and approvals |
| `P1-02` | Atomic `RunContextGenerationBindingV2` plus crash-consistent durable invocation preparation/launch/prompt state machine |
| `P1-03` | Independent `agent_context_envelope_schema`, explicit existing snapshot mapping, normative JSON Schemas/vectors, and exact cross-surface errors |
| `P1-04` | Immutable Git tree/blob capture, normative limits, deterministic multi-skill composition, and broker-only consumption of admitted bytes |
| `P1-05` | Complete `AgentContextCandidateManifestV1`, workflow-triggered promotion, observed identity checks, and provider-inaccessible holdout custody |
| `P1-06` | Durable closed/probe/open/emergency-hold states, one proof run, crash persistence, and explicit active provider/side-effect outcomes |
| `P1-07` | Context-absent snapshots are readback-only after migration; every live continuation path fails closed without override |

## Implementation Boundary

This document specifies the revised architecture but does not approve code
changes while proposal-readiness re-review is pending. After a ready verdict,
the next artifact is a file-by-file implementation plan with tests ordered
before production migration. Implementation must preserve unrelated dirty work
and must not begin until that plan has been reviewed.
