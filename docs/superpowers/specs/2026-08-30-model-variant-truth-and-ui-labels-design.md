# Codex Model Variant Truth and UI Labels

Date: 2026-08-30
Status: Draft; revised after proposal-readiness review

## Summary

Chainworks currently freezes the generic Codex model value `gpt-5.6` and
renders that request-derived value in Overview, Stages, and active-agent
readback. The value does not identify Sol, Terra, or Luna, and the runtime does
not durably distinguish a requested model/effort pair from the pair accepted by
the provider.

Fresh runs will use role-balanced, exact GPT-5.6 variant IDs and explicit
reasoning effort. Before any prompt is sent, the Codex ACP path will apply and
response-verify the exact model and effort, persist that accepted pair against
the specific task occurrence, and expose it through additive GraphQL and Swift
readback. Operator surfaces will visibly distinguish planned, configuring,
configured, prompt-sent, delivery-unknown, failed-before-prompt, and
legacy-unverified truth.

Legacy frozen runs keep their original bytes and previous adapter behavior.
They are labeled as planned/unverified instead of being guessed, rewritten, or
presented as provider-accepted truth.

## Problem and Root Cause

- All current Codex backend profiles declare `model: gpt-5.6`.
- The isolated Codex runtime intentionally removes the host user's model and
  effort overrides so catalog truth controls provider execution.
- Current `codex-acp` creates a session with its provider default and exposes
  model selection through the `model` session config option.
- Chainworks currently applies reasoning effort after `session/new` as a
  best-effort option, but does not apply the requested model option.
- The generic option resolver can use aliases and substring matching, and the
  required-option loop resolves every value from the original `session/new`
  response rather than the preceding configuration response.
- `AgentExecution` has one request-derived `model` field, no effort field, no
  accepted/requested distinction, and no stable task-occurrence identity.
- Stage topology associates executions to tasks by `agent_id`, so two tasks
  using one agent can receive each other's status, attempts, and model truth.
- Active-agent GraphQL and Swift readback omit effort. Existing compact UI rows
  also truncate the model identity and combine occurrence accessibility into
  the enclosing stage card.

The current UI therefore cannot honestly claim that a generic `gpt-5.6` run is
using Sol, Terra, or Luna, or that any displayed planned pair was accepted by
the provider.

## Goals

- Pin one exact GPT-5.6 variant and one explicit, role-appropriate effort for
  every fresh Codex backend profile.
- Preserve a balanced quality/cost policy by role.
- Keep fresh validation and required ACP negotiation separate from legacy
  frozen-snapshot replay.
- Fail session startup before prompt dispatch when exact model/effort
  negotiation, verification, or accepted-truth persistence fails.
- Persist planned/requested truth separately from response-verified accepted
  truth for the specific task occurrence.
- Preserve accepted-pair authority across reuse of the same live provider
  session without silently treating a new request as newly negotiated.
- Make prompt dispatch crash-consistent so operator copy distinguishes
  configured, sent, and delivery-unknown truth.
- Use one model-and-effort formatter across Overview, Stages, active-agent
  readback, Help, and accessibility output.
- Make legacy generic identity and effort explicitly planned/unverified.

## Non-Goals

- Add a runtime model-selection UI, feature flag, or disable path.
- Change provider families, agent IDs, temperature, turn budgets, escalation
  ordering, or permissions.
- Rewrite existing frozen workflow or catalog snapshot bytes.
- Infer a variant from the host Codex configuration.
- Change alias resolution for Claude or other providers.
- Introduce accepted-pair negotiation for non-Codex providers; their existing
  requested identity remains explicitly acceptance-unverified.
- Require a live-provider or remote-UI release gate for this bounded change.

## Approved Model and Effort Matrix

| Backend profile | Exact model | Approved effort | Rationale |
|---|---|---|---|
| `codex_orchestrator_high` | `gpt-5.6-sol` | `max` | Single lead authority and cross-stage decisions |
| `codex_architect_high` | `gpt-5.6-sol` | `xhigh` | Parallel architecture and API contract review |
| `codex_audit_high` | `gpt-5.6-sol` | `ultra` | Read-only final audit with independent evidence workstreams |
| `codex_writer_high` | `gpt-5.6-terra` | `high` | Iterative proposal authoring |
| `codex_builder_high` | `gpt-5.6-terra` | `high` | General implementation and fallback work |
| `codex_orchestrator_acp` | `gpt-5.6-terra` | `high` | Routine orchestration still requiring reliable tool judgment |
| `codex_ops_low` | `gpt-5.6-luna` | `medium` | Bounded operational work with a reasoning floor |

No other backend profile changes in this slice.

The current Codex ACP effort vocabulary is `low`, `medium`, `high`, `xhigh`,
`max`, and `ultra`. All six values remain recognized and tested. The approved
profile matrix intentionally starts at `medium`: no current Chainworks role is
safe to weaken merely to make every supported value appear in active catalog
assignments. The stable `codex_ops_low` profile ID is retained to avoid
reference churn; its frozen `effort` field, not the historical ID suffix, is
authoritative.

`ultra` is allowed only for `codex_audit_high`. It enables provider-internal
automatic task delegation, but does not create additional Chainworks stage or
agent authority and does not widen the audit agent's read-only permissions.

Capability basis for this design is a local `session/new` probe against
`codex-acp 1.1.7` on 2026-08-30. It advertised all six effort values for Sol
and Terra, and all values through `max` for Luna. [OpenAI's GPT-5.6
guidance](https://developers.openai.com/api/docs/guides/latest-model) positions
Sol for flagship work, Terra for balanced work, Luna for efficient high-volume
work, and recommends reserving `max` for the hardest quality-first workloads.
Because provider capabilities can change independently of this catalog, live
option validation remains mandatory.

## Fresh and Frozen Compilation Boundary

Fresh catalog intake and frozen replay must become explicit compiler modes.
They may share parsing and plan construction, but they must not share an
implicit decision about whether the exact-pair contract applies.

### Fresh catalog intake

`compile()` validates the approved seven-profile matrix and rejects:

- generic `gpt-5.6` in an active Codex backend profile;
- an unapproved model/effort pair;
- Luna + `ultra`;
- `ultra` outside `codex_audit_high`.

The author-authored YAML does not contain a compatibility switch. After
validation, the compiler writes a compiler-owned marker into the immutable
catalog snapshot:

```json
{
  "chainworks_compiled": {
    "schema_version": 2,
    "provider_configuration_contract_version": "codex_exact_pair_v1"
  }
}
```

This is an abbreviated excerpt; the existing embedded-skill fields remain in
the same object. Only `chainworks_compiled.schema_version` advances from 1 to
2; the outer `catalog_snapshot_format_version` remains 2. The frozen catalog
bytes and resulting `RunPlan` carry the contract version into every
`ExecutionRequest`.

### Frozen snapshot replay

`compile_from_snapshot_json()` reads only the compiler-owned frozen marker:

- `codex_exact_pair_v1` requires the frozen pair to be structurally complete
  and uses required negotiation;
- a pre-change snapshot with schema v1 or no marker is
  `legacy_best_effort_v0`.

The replay path never re-applies the current seven-profile matrix. It does not
infer the contract from the current catalog, a model name, or application
defaults. Pre-change frozen snapshots therefore retain the old adapter
behavior: no required model operation and the prior best-effort effort
operation. Their snapshot bytes are not upgraded in place.

## Stable Task-Occurrence Identity

One `InvocationOccurrenceFactory` owns identity for every production
`WorkItemKind::InvokeAgent` producer. The only production enqueue entry point
is `WorkQueue::enqueue_invoke_agent(InvokeAgentEnvelopeV1)`. Generic enqueue
rejects `WorkItemKind::InvokeAgent`, and claim/start rejects a row whose typed
envelope is absent, malformed, or inconsistent. Producers may not hand-build
or clone an occurrence ID. The envelope constructor and identity fields are
private to the factory module; the raw transactional InvokeAgent insert is
crate-private to that queue path. Public generic queue methods accept every
other work kind but return a typed error for InvokeAgent, so production callers
cannot bypass the factory merely by serializing equivalent JSON.

`InvokeAgentEnvelopeV1` requires run ID, owner kind/ID, nullable stage execution
ID, compiled-task ID, task-occurrence ID, source kind/key, captured run dispatch
epoch, provider-configuration contract version, and the existing provider,
agent, session-reuse, and payload fields. The factory derives identity before
the queue row becomes visible; the claim path recomputes/validates the tuple
against durable owner truth before creating or reusing an `AgentExecution`.

Every source first receives an opaque `compiled_task_v1:<sha256>` ID. Static
and owner IDs hash frozen workflow coordinates; dynamic and mediation IDs hash
their durable materialization/mediation key plus the frozen binding. A concrete
invocation then receives `task_occurrence_v1:<sha256>` from
`durable owner scope + compiled_task_id`:

| Invoke source | Durable owner scope | Canonical source key |
|---|---|---|
| Static `sequence` / `parallel` / `then` | `stage_execution_id` | Frozen workflow hash, state ID, run block, lane, and lane ordinal |
| Owner-only compute state | `stage_execution_id` | Frozen workflow hash, state ID, and literal `owner` |
| Dynamic assignment | `stage_execution_id` | Durable `dynamic_materialization_records.id` |
| Lead conflict mediation | `lead_conflict_mediation.id` | Mediation task kind and frozen lead binding |
| Same-owner retry | Existing owner scope | Preserve the source occurrence ID |
| Backend-profile/provider fallback | Existing owner scope | Preserve the source occurrence ID |
| Targeted retry that creates a replacement stage | New `stage_execution_id` | Reuse/rederive the canonical compiled-task ID, then recompute occurrence; copied occurrence IDs are discarded |
| Loop re-entry | New `stage_execution_id` | Reuse the frozen compiled-task ID, then recompute occurrence from the new owner |
| `orchestrator.legacy_flat` | `stage_execution_id` | Literal `legacy_flat_v1` plus durable run workflow ID, stage ID, owner agent, and provider |

Dynamic materialization atomically persists `compiled_task_id`,
`task_occurrence_id`, and the work-item link in its existing durable record
before enqueue visibility. Dynamic topology is reconstructed from those rows,
not from agent-name or selection-order guesses.

The occurrence ID is copied into the work-queue payload, `ExecutionRequest`,
and `AgentExecution`. Two tasks in one stage that use the same agent receive
different IDs. A later loop iteration or replacement stage execution receives a
new ID, so attempts from different owner scopes cannot be merged. Pending
static/owner topology exposes `compiled_task_id`; its
`task_occurrence_id` remains `null` until a stage execution exists.

Topology association follows these rules:

1. Match executions to occurrences by `task_occurrence_id`.
2. Count attempts only within that occurrence.
3. Select the latest execution within that occurrence for requested/accepted
   readback.
4. For pre-change executions whose occurrence ID is `null`, allow the old
   `agent_id` fallback only when exactly one task in that stage uses that agent.
5. If a legacy stage is ambiguous, do not assign an execution to either task
   and do not guess runtime truth.

The gate maintains an inventory of every production `InvokeAgent` creation
site and proves that all nine current source classes delegate to the typed
factory. A recursive guard fails on direct generic enqueue or raw payload
construction, while behavior tests cover static, owner, dynamic, mediation,
same-owner retry, fallback, replacement-stage retry, loop, and `legacy_flat`.

## Codex ACP Negotiation

Exact-only negotiation applies only when the frozen contract version is
`codex_exact_pair_v1`. The generic resolver remains unchanged for Claude alias
matching and for `legacy_best_effort_v0`.

The fresh Codex session transaction is:

1. Send `session/new` with the requested exact model identity.
2. Exact-resolve the requested model against the returned `model` option. One
   unique case-insensitive full match on option `value` or display `name` is
   allowed; ambiguity, substring/token matching, and raw-value fallback are
   forbidden.
3. Send `session/set_config_option(model, resolved_model)`.
4. Require the response to contain updated `configOptions` and verify that the
   returned `model.currentValue` exactly equals the resolved requested model.
5. Replace the working option snapshot with that response. Exact-resolve the
   requested effort from its updated `reasoning_effort` option.
6. Send `session/set_config_option(reasoning_effort, resolved_effort)`.
7. Require the second response to contain updated `configOptions` and verify
   both final `model.currentValue` and `reasoning_effort.currentValue` against
   the requested exact pair.
8. Atomically persist generation-scoped
   `ProviderConfigurationAcceptanceV1` and execution-scoped
   `ProviderConfigurationReceiptV1`.
9. Permit `session/prompt` only after receipt persistence succeeds.

An empty/malformed option response, missing option, unknown value, incompatible
effort after model selection, send failure, provider rejection, current-value
mismatch, or persistence failure is a typed startup failure with zero prompt
dispatch. A successfully returned JSON-RPC response without matching
`currentValue` is not acceptance.

### Live-session reuse

A new prompt on an existing Codex session does not repeat `session/new`, so
the accepted pair is owned by the durable `SessionGeneration`, not only by the
first `AgentExecution`. The generation stores:

- provider-configuration contract version;
- accepted model and effort;
- provider-session ID and binding fingerprint;
- accepted-at timestamp;
- bounded `ProviderConfigurationAcceptanceV1` and its SHA-256 digest.

Before a reused prompt, the engine loads the active generation and requires all
of these values to match the live handle and current request: generation ID,
provider-session ID, provider, binding fingerprint, contract version, requested
model, and requested effort.

When they match, one transaction derives a new execution-bound
`ProviderConfigurationReceiptV1` from the generation acceptance and writes it
to the new `AgentExecution` with
`acceptance_source = reused_session_generation` and the source acceptance
digest. The derived receipt names the new agent execution and task occurrence;
it does not copy the first execution's IDs. That projection is
response-verified authority inherited from the same live provider session; it
is not a new negotiation.

When evidence is absent, stale, malformed, or mismatched, the manager closes
and invalidates the generation before any prompt. It then performs at most one
fresh-session fallback through the complete negotiation transaction. The old
session receives zero prompts. A fresh-session failure is returned normally and
is not retried again by this compatibility path.

Legacy v0 generations may be reused only by a
`legacy_best_effort_v0` execution. A `codex_exact_pair_v1` request never
inherits legacy-unverified generation evidence.

## Durable Runtime Truth

The next SQLite migration adds nullable columns to `agent_executions`:

| Column | Meaning |
|---|---|
| `task_occurrence_id` | Stable occurrence shared only within one owner scope |
| `requested_model` / `requested_effort` | Canonical pair requested for this execution |
| `accepted_model` / `accepted_effort` | Canonical response-verified pair; otherwise `null` |
| `accepted_model_wire_value` / `accepted_effort_wire_value` | Exact provider option values whose `currentValue` was verified |
| `provider_configuration_state` | `configuring`, `configured`, `failed_before_prompt`, `cancelled_before_prompt`, or `legacy_unverified`; `null` for non-Codex |
| `provider_configuration_verified_at` | Complete-pair verification time; otherwise `null` |
| `provider_configuration_receipt_json` | Bounded execution-scoped receipt |
| `acceptance_source` | `fresh_negotiation` or `reused_session_generation`; otherwise `null` |

The existing `model` column remains a compatibility projection of
`requested_model`; it is never redefined as accepted truth. Migration backfills
`requested_model = model` for historical rows, leaves effort and accepted
fields `null`, and marks historical Codex rows `legacy_unverified`. New writes
keep `model` and `requested_model` byte-equal.

The migration also:

- adds `prompt_dispatch_epoch INTEGER NOT NULL DEFAULT 0` to `runs`; only
  `DispatchInvalidationCoordinator` increments it, in the transaction that
  makes a run-wide cancellation or replacement visible. Scoped invalidation
  does not change the run epoch and instead relies on the bound
  stage/execution/work-item/generation predicates below;
- extends `session_generations` with contract version, canonical and wire
  accepted pairs, provider-session binding fingerprint, acceptance JSON/digest,
  and verified-at fields;
- extends existing prompt-level `agent_execution_runtime_receipts` rows with
  `task_occurrence_id`, `work_item_id`, `session_generation_id`,
  `provider_session_id`, contract version, captured dispatch epoch,
  `dispatch_state`, `dispatch_started_at`, `prompt_sent_at`,
  `dispatch_unknown_at`, and typed dispatch failure code;
- extends `dynamic_materialization_records` with nullable
  `compiled_task_id`, `task_occurrence_id`, and unique `work_item_id` referencing
  `work_items(id)`, plus an occurrence lookup index. Existing rows remain
  legacy-null; every new dynamic materialization writes all three with the
  existing record and queue row in one transaction.

`agent_execution_runtime_receipts` is the single prompt-turn ledger; no second
dispatch table is introduced. Its existing primary key
`(agent_execution_id, prompt_kind, turn_index)` is authoritative. New original
turns use `("original", 0)`. Existing completion repair uses
`("code_writer_completion_repair", 1)`; any further allowed turn receives the
next monotonic index and a typed prompt kind. Every new row starts
`not_started`, while pre-change rows retain nullable dispatch truth and render
delivery-unverified.

### Frozen wire contracts and hashing

`ProviderConfigurationAcceptanceV1` is generation-scoped and has exactly these
JSON keys:

```json
{
  "schema_version": "provider_configuration_acceptance_v1",
  "provider_configuration_contract_version": "codex_exact_pair_v1",
  "session_generation_id": "...",
  "provider_session_id": "...",
  "provider": "codex",
  "binding_fingerprint_sha256": "...",
  "requested_model": "gpt-5.6-terra",
  "requested_effort": "high",
  "accepted_model": "gpt-5.6-terra",
  "accepted_effort": "high",
  "accepted_model_wire_value": "...",
  "accepted_effort_wire_value": "...",
  "verified_at": "RFC3339"
}
```

Every key is required and contains a non-null string; both digest-shaped fields
are lowercase 64-character hex, `verified_at` is canonical UTC RFC 3339, and
unknown keys are rejected.

`accepted_model` and `accepted_effort` are canonical catalog values and equal
the requested exact pair after verification. Wire-value fields preserve the
exact option values selected and returned by provider `currentValue`; UI uses
canonical values and never substitutes display names or wire values.

`ProviderConfigurationReceiptV1` is execution-scoped and has exactly these
keys: `schema_version` with literal value
`provider_configuration_receipt_v1`,
`provider_configuration_contract_version`, `agent_execution_id`,
`task_occurrence_id`, nullable `session_generation_id`, nullable
`provider_session_id`, `provider`, nullable `binding_fingerprint_sha256`,
`requested_model`, `requested_effort`, nullable `accepted_model`, nullable
`accepted_effort`, nullable `accepted_model_wire_value`, nullable
`accepted_effort_wire_value`, `configuration_state`, nullable
`acceptance_source`, nullable `source_generation_acceptance_sha256`, nullable
`verified_at`, nullable `failure_code`, and the non-negative integer
`prompt_dispatch_count_at_receipt`.

For `configured`, both session IDs, binding fingerprint, all accepted/source
fields, source digest, and verification time are non-null, `failure_code` is
null, and the prompt count is zero. For `failed_before_prompt` or
`cancelled_before_prompt`, accepted/source/digest/time fields are null,
`failure_code` is non-null, the prompt count is zero, and session IDs plus the
binding fingerprint may be null only when settlement precedes their creation.
No unknown JSON keys are accepted. All receipt identifiers and requested values
must equal the owning execution row; all configured generation fields must
equal the referenced generation acceptance.

The generation digest is lowercase hex SHA-256 over UTF-8 RFC 8785 canonical
JSON of `ProviderConfigurationAcceptanceV1`; the digest itself is stored beside
and excluded from that object. `ProviderConfigurationAuthority` in engine is
the sole encoder/verifier. It recomputes the digest before generation insert,
before reuse projection, and when loading an active generation. Digest mismatch
or malformed/oversized JSON invalidates the generation and returns
`ACP_PROVIDER_CONFIGURATION_EVIDENCE_INVALID` before prompt dispatch.

Both configuration JSON objects are capped at 8 KiB and contain no complete
option catalog or raw JSON-RPC payload. The frozen runtime-receipt v1 top-level
key set is `schema_version`, `transport_family`, `provider`, `model`,
`provider_session_id`, `session_generation_id`, `status`, `failure_phase`,
`jsonrpc_error_code`, `provider_error_message_redacted`, `started_at`,
`completed_at`, `xcode_shim_injected`, `requires_xcode_host_execution`,
`handshake`, `counters`, `permission_roundtrips`, `first_events`, `last_events`,
`claude_diagnostics`, and `p079_unsafe_continuation`; its nested field schemas
remain byte-compatible with the current checked-in Rust types. The v1 decoder
keeps existing compatibility. Required fields are `schema_version`,
`transport_family`, `provider`, `status`, `started_at`,
`xcode_shim_injected`, `requires_xcode_host_execution`, `handshake`, and
`counters`. `model`, both session IDs, `failure_phase`, `jsonrpc_error_code`,
`provider_error_message_redacted`, and `completed_at` default to null; the three
event arrays default to empty; `claude_diagnostics` may be omitted or null; and
`p079_unsafe_continuation` defaults to false.

`AcpRuntimeReceipt` schema v2 uses integer `schema_version = 2`, preserves that
complete v1 shape, and adds two required keys:
`provider_configuration_receipt` and `prompt_turn`. The first is a valid
`ProviderConfigurationReceiptV1` object for an exact-contract Codex execution,
including failed/cancelled configuration, and explicit `null` for non-Codex or
legacy-v0 execution. `prompt_turn` has exactly the non-empty string
`prompt_kind`, non-negative integer `turn_index`, and `dispatch_state` in
`not_started`, `dispatch_pending`, `prompt_sent`, or `dispatch_unknown`; unknown
keys are rejected. The canonical v2 encoder emits every v1 top-level key:
nullable values are explicit null, arrays and booleans are explicit, and no
unknown top-level key is allowed. Decoder behavior is frozen:

| Runtime receipt input | Result |
|---|---|
| integer `schema_version = 1` | Decode as legacy; both new fields unavailable |
| v2 with all keys and a valid nested receipt/reference | Decode, authority-verify, then project |
| v2 with an omitted key, malformed nested object, or authority digest mismatch | `ACP_RUNTIME_RECEIPT_INVALID` |
| Any unsupported schema version | `ACP_RUNTIME_RECEIPT_UNSUPPORTED_VERSION` |

The implementation adds normative `additionalProperties: false` schemas at
`docs/reference/schemas/provider-configuration-acceptance-v1.schema.json`,
`docs/reference/schemas/provider-configuration-receipt-v1.schema.json`, and
`docs/reference/schemas/acp-runtime-receipt-v2.schema.json`, plus valid/invalid
fixtures. The
execution-row receipt and every present prompt-turn runtime receipt must agree
on execution, occurrence, requested/accepted pair, source digest, generation,
and provider-session binding. The durable prompt-turn row remains dispatch
authority; a v2 receipt is accepted only when its tuple and observed state equal
that row and it never mutates the row's dispatch state.
`ProviderConfigurationAuthority` performs the database-backed source-generation
digest comparison after structural decode and before any readback projection.

### Configuration and prompt-turn dispatch lifecycle

The engine inserts a fresh exact Codex execution with requested fields and
`provider_configuration_state = configuring` before ACP startup. Claim/start
atomically creates the execution and its `original/0` prompt-turn row in
`not_started`; non-Codex and legacy executions receive the same original row
with non-applicable/unverified configuration truth. A strict
provider-configuration sink on `AcpRuntimeManager`:

- after both option responses are verified, atomically writes generation
  acceptance and the execution receipt, then marks configuration `configured`;
- on negotiation failure, writes `failed_before_prompt` with null accepted
  fields and a typed receipt;
- on cancellation that wins before configuration completes, writes
  `cancelled_before_prompt`, keeps the original turn `not_started`, and marks
  the execution/work item cancelled in the same settlement transaction;
- returns `ACP_PROVIDER_CONFIGURATION_PERSISTENCE_FAILED` with zero prompts if
  authority persistence fails.

Every prompt turn independently follows:

```text
not_started -> dispatch_pending
dispatch_pending -> prompt_sent
dispatch_pending -> dispatch_unknown
```

The existing P079 repair lease and its
`code_writer_completion_repair/1` prompt-turn row transition in the same
transactions. Lease reservation creates `not_started`; dispatch permission
sets both to `dispatch_pending`; post-flush success sets both to `prompt_sent`;
any ambiguous outcome sets both to `dispatch_unknown`. P079 may no longer mark
its lease prompt-sent before transport.

`AcpRuntimeManager` owns one async `SessionPromptGate` per live generation.
The gate exists as soon as a generation ID is allocated, including while
configuration is in flight. Configuration settlement uses a CAS over the
captured run epoch and the same owner/execution/work-item status predicates as
prompt permission; when invalidation wins, the generation is closed and the
execution settles `cancelled_before_prompt` without projecting configured truth.
Prompt dispatch holds it from permit CAS through transport write/flush and the
final CAS. One `DispatchInvalidationCoordinator` owns run cancellation,
stage/execution replacement, targeted retry cancellation, work-item
cancellation, and direct provider-session shutdown. It acquires affected live
generation gates in sorted ID order before mutating durable owner state. A
run-wide cancellation also increments `runs.prompt_dispatch_epoch`; scoped
invalidation leaves unrelated stages on the current epoch. Direct
provider-session cancellation binds its service-assigned cancellation intent.
Raw cancel/supersede repository mutators are not callable from other production
paths. Thus either a prompt reaches durable `prompt_sent` before invalidation,
or invalidation wins and that prompt writes zero bytes.

The permit CAS returns `Applied`, `AlreadyMatching`, `Conflict`, or `Missing`.
It binds prompt kind/index, execution, occurrence, owning running work item,
active generation/provider session, contract/requested pair, captured run
dispatch epoch, `runs.status = running`, `agent_executions.status = running`,
`work_items.status = running`, and absence of a cancelling provider intent.
Exact Codex additionally requires `provider_configuration_state = configured`.
Only a newly `Applied` permit authorizes transport write.

`Conflict` or `Missing` from the initial permit CAS authorizes zero bytes and
settles as `ACP_PROMPT_DISPATCH_PREPARE_FAILED`; it does not create delivery
ambiguity. `AlreadyMatching` at the pending boundary means an earlier send may
have started and is converted to `dispatch_unknown`. Crash, send/flush error,
post-send persistence error, or `Conflict`/`Missing` from the final post-flush
CAS also settles that turn unknown, closes the generation, marks the associated
work item `Failed` with `prompt_delivery_reconciliation_required`, fails the
still-running execution with `failure_phase = prompt_dispatch`, and marks the
stage/run `Blocked` for operator inspection. Startup repair performs the same
settlement for stale pending turns. Every retry, targeted retry, fallback,
continuation, and startup requeue selector excludes an execution with any
unresolved unknown turn.

Terminal runtime-receipt persistence updates only the receipt/status columns of
the same prompt-turn tuple. It cannot advance or overwrite dispatch state; a
conflicting terminal upsert fails closed and leaves the prompt turn for startup
reconciliation.

A stale `not_started` turn with no matching live generation is provably
unprompted and may settle `prompt_dispatch_preparation` without ambiguity.
`No prompt sent` is derived only when no turn for the execution advanced past
`not_started`; it is not inferred from Codex configuration state. `Using` or
`Used` requires original turn `prompt_sent`. A repair turn is reported
separately, and unresolved `dispatch_unknown` dominates all aggregate copy.
Timeline progress remains advisory and cannot advance durable dispatch truth.

For `legacy_best_effort_v0`, new resumed attempts are
`legacy_unverified`: requested/planned values may be retained, accepted fields
remain `null`, and configuration follows the old adapter path. The shared
prompt-dispatch ledger still records new prompts. Existing historical rows
remain readable without a receipt or dispatch state.

## GraphQL and Swift Readback

Readback changes are additive. Existing `model` fields remain compatibility
aliases for requested/planned model and must not be used as actual truth by the
updated UI.

`AgentExecution` GraphQL adds:

- `taskOccurrenceId`;
- `requestedModel`, `requestedEffort`;
- `acceptedModel`, `acceptedEffort`;
- `providerConfigurationState`, `acceptanceSource`, and
  `providerConfigurationVerifiedAt`;
- non-null `promptDispatchSummary` and non-null `promptTurns`.

`ProviderPromptTurn` exposes non-null `promptKind` and `turnIndex`, plus nullable
`dispatchState`, `dispatchStartedAt`, `promptSentAt`, `dispatchUnknownAt`, and
`failureCode`. `ProviderPromptDispatchSummary` exposes:

- nullable original-turn state;
- nullable latest turn kind/index/state;
- non-null `deliveryTruth` in `not_started`, `original_pending`,
  `original_sent`, `repair_pending`, `repair_sent`, `unknown`, or
  `legacy_unverified`;
- non-null `noPromptSent` and `hasUnresolvedUnknown`.

An unresolved unknown turn always wins aggregation. Otherwise the greatest
turn index is latest, while original-turn sent truth remains independently
available. `noPromptSent` is true only when every present turn is
`not_started`; it is false for pending, sent, or unknown.

`RunStageTopologyOccurrence` adds:

- non-null immutable `presentationRowId` and `compiledTaskId`, plus nullable
  `taskOccurrenceId` and `activeExecutionId`;
- `executionProvider`;
- requested and accepted model/effort;
- provider-configuration state and prompt-dispatch summary.

Its existing `provider`, `model`, and `effort` fields continue to mean frozen
planned identity for compatibility. The new fields come only from the latest
execution matched by occurrence ID. Retry/fallback cannot overwrite another
same-agent task.

`GqlMediationExecutionAttempt` receives the same requested/accepted,
configuration, occurrence, and prompt-summary fields. MCP
`workflow_conflict.lead_mediation.execution_attempts`, the general MCP
execution-truth report, and run-report attempt objects expose byte-equivalent
snake-case `task_occurrence_id`, `requested_model`, `requested_effort`,
`accepted_model`, `accepted_effort`, `provider_configuration_state`,
`acceptance_source`, `provider_configuration_verified_at`,
`prompt_dispatch_summary`, and sanitized `prompt_turns`. They may not continue
to label request-derived `model` as runtime truth. Provider-session IDs and raw
receipt JSON retain their existing operator-only redaction boundary.

All new GraphQL scalar fields are nullable for historical/pre-configuration
rows; list/summary containers and their discriminator booleans are non-null.
When the schema-v1 query selects a nullable field, the response key must be
present with explicit `null`. Swift DTOs declare every `CodingKey` and use a
custom decoder that distinguishes `container.contains(key) == false` (typed
schema mismatch) from explicit null (valid according to the state matrix).
Checked-in GraphQL, MCP, and Swift fixtures cover historical Codex, non-Codex,
pre-session configuration failure, mediation, repair turn, and schema mismatch.

### Lockstep daemon schema

GraphQL rejects a document containing unknown fields; an old daemon does not
return those fields as `nil`. The updated app therefore requires lockstep
replacement of the bundled daemon rather than issuing a reduced legacy run
detail query.

Before the first updated run-detail query, the app performs a minimal
`providerExecutionTruthSchemaVersion` probe and requires value `1`. An
unknown/missing field triggers exactly one existing bundled-daemon replacement
and restart, followed by one probe retry. If the retry still fails, the app does
not issue the new readback document and renders the existing typed
`Daemon schema mismatch` recovery state. It never falls back to planned values
as runtime truth.

The Swift DTO fields remain nullable for historical database rows and
pre-configuration executions returned by a daemon that advertises schema v1.
Missing required fields after a successful v1 probe are a contract violation,
not legacy compatibility.

## UI Truth and Formatting Contract

One `ProviderExecutionIdentityFormatter` owns visual text, Help text, and
accessibility text. It accepts planned, requested, accepted, execution status,
provider-configuration state, and prompt-dispatch summary. It never promotes a
planned/requested value to accepted truth, and an unresolved unknown prompt
turn overrides ordinary execution status copy.

### Codex state matrix

| State | Runtime truth | Operator copy |
|---|---|---|
| Pending | Frozen planned pair; no execution | `Planned: Codex - GPT-5.6 Terra - High` |
| Configuring | Requested pair present; accepted pair absent | `Configuring: Codex - GPT-5.6 Terra - High` |
| Cancelled during configuration | Accepted pair absent; configuration is terminal | `Cancelled before prompt: Codex - GPT-5.6 Terra - High - No prompt sent` |
| Configured / not started | Response-verified pair; prompt not attempted | `Configured: Codex - GPT-5.6 Terra - High - Prompt not started` |
| Cancelled before prompt | Response-verified pair; dispatch remains `not_started` | `Cancelled before prompt: Codex - GPT-5.6 Terra - High - No prompt sent` |
| Dispatch pending | Response-verified pair; delivery not yet known | `Starting: Codex - GPT-5.6 Terra - High` |
| Prompt sent / running | Response-verified pair and durable prompt sent | `Using: Codex - GPT-5.6 Terra - High` |
| Prompt sent / completed | Response-verified pair and durable prompt sent | `Used: Codex - GPT-5.6 Terra - High` |
| Prompt sent / failed | Response-verified pair; execution failed later | `Used: Codex - GPT-5.6 Terra - High` plus failure status |
| Prompt sent / cancelled | Response-verified pair; execution cancelled later | `Cancelled: Codex - GPT-5.6 Terra - High` |
| Dispatch unknown | Response-verified pair; delivery ambiguous | `Prompt delivery unknown: Codex - GPT-5.6 Terra - High - Do not retry automatically` |
| Repair pending | Original prompt sent; repair turn pending | `Using: Codex - GPT-5.6 Terra - High - Repair starting` |
| Repair sent | Original and repair prompts durably sent | `Using: Codex - GPT-5.6 Terra - High - Repair prompt sent` |
| Repair unknown | Original sent; repair delivery ambiguous | `Repair prompt delivery unknown: Codex - GPT-5.6 Terra - High - Do not retry automatically` |
| Configuration failure | Requested pair present; accepted pair absent | `Configuration failed: GPT-5.6 Terra - High - No prompt sent` |
| Legacy generic | Frozen requested identity; provider acceptance unavailable | Status prefix plus `Codex - GPT-5.6 (variant unspecified) - High - Unverified` |
| Retry/fallback | Latest execution for the same task occurrence | Codex uses that execution's accepted pair and dispatch state |

If `configured` lacks either accepted field, the Codex readback is internally
inconsistent. It renders `Runtime identity unavailable`, exposes diagnostic
Help text, and must be caught by the gate. It must not fall back to planned
truth.

### Provider-neutral state matrix

`provider_configuration_state = null` is the expected non-Codex path, not an
error. Claude, Gemini, Auggie, and Junie retain their execution-request identity
and explicitly qualify provider acceptance as unavailable:

| State | Operator copy |
|---|---|
| Pending | `Planned: Claude - Opus - High` |
| Before or during dispatch | `Starting: Claude - Opus - High - Acceptance unverified` |
| Startup failure before dispatch | `Start failed: Claude - Opus - High - No prompt sent - Acceptance unverified` |
| Cancelled before prompt | `Cancelled before prompt: Claude - Opus - High - No prompt sent - Acceptance unverified` |
| Prompt sent / running | `Running: Claude - Opus - High - Acceptance unverified` |
| Prompt sent / completed | `Completed: Claude - Opus - High - Acceptance unverified` |
| Prompt sent / failed | `Failed: Claude - Opus - High - Acceptance unverified` |
| Prompt sent / cancelled | `Cancelled: Claude - Opus - High - Acceptance unverified` |
| Dispatch unknown | `Prompt delivery unknown: Claude - Opus - High - Do not retry automatically` |
| Repair pending/sent | Status prefix plus `Repair starting` or `Repair prompt sent` and `Acceptance unverified` |
| Repair unknown | `Repair prompt delivery unknown: Claude - Opus - High - Do not retry automatically` |
| Historical execution | Status prefix plus requested identity and `Delivery unverified` |

A Codex-to-non-Codex fallback uses the provider-neutral row and never inherits
the prior Codex accepted pair. A non-Codex-to-Codex fallback must complete the
exact Codex transaction. Missing model or effort segments are omitted, not
invented.

Cancellation while still `not_started` is provably unprompted. Cancellation
after `dispatch_pending` but before durable `prompt_sent` settles
`dispatch_unknown`; cancellation must not erase delivery ambiguity.

### Display mapping

| Raw model | Display value |
|---|---|
| `gpt-5.6-sol` | `GPT-5.6 Sol` |
| `gpt-5.6-terra` | `GPT-5.6 Terra` |
| `gpt-5.6-luna` | `GPT-5.6 Luna` |
| `gpt-5.6` | `GPT-5.6 (variant unspecified)` |
| unknown non-empty model | Preserve the exact raw value |

| Raw effort | Display value |
|---|---|
| `low` | `Low` |
| `medium` | `Medium` |
| `high` | `High` |
| `xhigh` | `Extra High` |
| `max` | `Max` |
| `ultra` | `Ultra` |
| unknown non-empty effort | Preserve the exact raw value |

The same formatter is used by:

- current/previous stage occurrence rows in Overview;
- the Stages topology surface;
- active-agent readback rows;
- Help and accessibility labels derived from those rows.

The formatter returns both `fullIdentity` and `compactIdentity`.
`fullIdentity` always preserves unknown raw values and is the sole source for
Help, accessibility, copy, and the detail popover. `compactIdentity` may use a
bounded `Custom model <digest-prefix>` fallback only when an unknown raw value
cannot fit; known Sol/Terra/Luna and effort labels are never abbreviated.

Model and effort receive a dedicated identity line below the task/title line.
Overview and active-agent rows allow the full value to wrap. Topology uses
`ViewThatFits` with a two-line full identity followed by the bounded compact
identity and an `info.circle` button. That button opens a selectable popover
with the complete identity and a copy action. Status, attempts, stage, task, and
session diagnostics occupy a separate secondary line.

Topology replaces per-card fixed slots with one deterministic global track
layout. The topology compiler emits `column`, `trackStart`, and `trackSpan` for
every stage from the existing transition graph. Natural card height is computed
from header chrome, metadata/transition rows, occurrence count, and bounded
two-line identity rows. Starting from the existing minimum track height, the
layout processes stages in `(column, trackStart, stageId)` order and distributes
any height deficit evenly across all tracks in that stage's span. The pass
repeats until every spanned card has sufficient height; identical input yields
identical track sizes.

Each card frame is the sum of its global track heights and inter-track gaps.
Cards publish bounds through anchor preferences. Connector source and target
centers are the actual `midTrailing` and `midLeading` points of those frames;
orthogonal branch junctions use the midpoint of the inter-column gap. The same
global frames drive manually paired branches, hit testing, focus, popovers, and
accessibility. No connector computes y-position from a fixed card-height
constant.

Each occurrence row owns its accessibility label. Stage cards contain child
accessibility elements rather than combining and swallowing occurrence labels.
`PresentationRowIdentity` is the sole encoder. It hashes domain-separated,
length-prefixed UTF-8 components and emits lowercase
`topology_row_v1:<sha256>`. Static/owner rows use run-plan snapshot hash, state
ID, and compiled-task ID; `legacy_flat` uses stage-execution ID and
compiled-task ID; dynamic rows use the durable dynamic-materialization ID and
compiled-task ID. The value exists before a row is first rendered and never
changes when a task occurrence or execution appears. SwiftUI uses only this
key, never `taskOccurrenceId`, agent ID, or a composite guess. Visual, Help,
popover, copy, and accessibility strings are generated from the same formatter
result.

## Failure Behavior

| Failure | Typed result | Prompt dispatch |
|---|---|---:|
| Fresh generic or unapproved catalog pair | compile failure | 0 |
| Model option/value unavailable | `ACP_CODEX_MODEL_UNAVAILABLE` | 0 |
| Model response lacks matching current value | `ACP_CODEX_MODEL_NOT_ACCEPTED` | 0 |
| Updated effort option/value unavailable | `ACP_CODEX_EFFORT_UNAVAILABLE` | 0 |
| Final response does not verify both values | `ACP_CODEX_EFFORT_NOT_ACCEPTED` | 0 |
| Accepted-truth persistence fails | `ACP_PROVIDER_CONFIGURATION_PERSISTENCE_FAILED` | 0 |
| Acceptance/receipt malformed or digest mismatch | `ACP_PROVIDER_CONFIGURATION_EVIDENCE_INVALID` | 0 |
| Cancellation wins during configuration | `cancelled_before_prompt` | 0 |
| Reused generation evidence mismatch | close generation and negotiate fresh once | 0 on old session |
| Dispatch permit loses to cancellation/ownership/epoch CAS | `ACP_PROMPT_DISPATCH_PREPARE_FAILED` | 0 |
| Transport send/flush fails after dispatch pending | `ACP_PROMPT_DISPATCH_UNKNOWN` | unknown |
| Prompt-sent persistence fails after transport success | `ACP_PROMPT_DISPATCH_UNKNOWN` | sent or unknown |
| Startup finds stale dispatch pending | `ACP_PROMPT_DISPATCH_UNKNOWN` | unknown |
| Unsupported/malformed runtime receipt | typed receipt failure; no projection | preserve turn ledger |
| Legacy generic frozen run | allowed as planned/unverified | shared ledger for each new attempt |

Configuration failures use `failure_phase = provider_configuration`, leave
accepted fields `null`, and may render the requested pair plus
`No prompt sent`. Dispatch failures use `failure_phase = prompt_dispatch`,
preserve the configured accepted pair, and never claim that no prompt was sent
after `dispatch_pending`. Unknown delivery atomically marks the owning work
item `Failed` and the stage/run `Blocked`. Missing accepted readback is never
inferred from the host configuration or planned catalog value.

## Canonical Verification Gate

Implementation adds one retained provider-free gate:

```bash
./scripts/test-gate.sh codex-model-truth
```

The gate is listed by `./scripts/test-gate.sh list` and documented in
`docs/reference/test-gates.md`. It runs executable tests rather than checking
test names or source strings.

| Owner | Required proof in `codex-model-truth` |
|---|---|
| `workflow` | Exact seven-profile matrix; fresh generic and invalid pair rejection; compiler-owned v2 marker; legacy v1 replay; stable distinct same-agent compiled-task IDs |
| `acp` fake provider | Response-closed negotiation; generation-bound reuse; prompt gate ordering; independent original/repair turn dispatch; cancellation-versus-send races; no fuzzy/raw fallback; Claude aliases unchanged |
| `db` + `engine` | All required table migrations/backfills; canonical JSON/digest validation; acceptance-to-receipt derivation; P079 lease/turn atomicity; four-result CAS; unknown-delivery hold and selector exclusion; typed enqueue/claim; all nine producer classes |
| `graphql-server` + `mcp-server` | Schema probe; SDL/JSON nullability; active/topology/mediation/report parity; prompt-turn aggregation; durable dynamic topology; planned values never populate accepted fields |
| Swift focused and hosted-view tests | Presence-aware DTO decoding; lockstep restart; complete state matrices; formatter parity; immutable row key; global-track branched geometry; long unknown values; popover/focus/copy/accessibility |

The fake ACP test matrix must include:

1. model success changes the effort option set and the adapter uses the updated
   set;
2. model JSON-RPC success with a mismatched `currentValue`;
3. effort JSON-RPC success with a mismatched final model or effort;
4. missing/malformed `configOptions` after either operation;
5. persistence-sink failure after provider acceptance;
6. a prompt counter proving every failure dispatched zero prompts;
7. a legacy generic request proving the old best-effort path remains reachable;
8. a Claude alias request proving generic alias matching did not change;
9. matching generation evidence projected before a reused prompt;
10. missing/mismatched generation evidence closes the old handle and sends the
    prompt only through one fresh negotiation;
11. original success followed by repair success proves independent durable
    prompt-turn rows and P079 lease parity;
12. `Applied`, `AlreadyMatching`, `Conflict`, and `Missing` CAS outcomes
    enforce the execution/occurrence/generation/request binding, with initial
    conflict proving zero bytes and post-flush conflict proving unknown delivery;
13. Claude, Gemini, Auggie, and Junie advance the shared prompt ledger while
    keeping provider-configuration truth non-applicable;
14. cancellation and both directions of cross-provider fallback preserve the
    state-matrix and occurrence-association rules;
15. crash before dispatch, crash after pending, send failure, and post-send
    persistence failure settle only the affected original/repair turn without
    replay;
16. cancellation during configuration, run/targeted-retry invalidation
    immediately before permit, and invalidation racing after permit are
    linearized by the generation gate; run-wide races additionally prove epoch
    fencing, while scoped races prove unrelated stages retain their epoch;
17. startup and every retry/requeue selector refuse unresolved unknown turns;
18. canonical JSON fixtures cover valid acceptance, digest mismatch,
    pre-session failure, malformed receipt, v1/v2 decode, and unsupported
    runtime-receipt version.

The gate must fail when its focused Swift result bundle reports zero tests. No
network, daemon, live provider, or remote UI host is required.

## Rollout

- There is no feature flag, disable path, or operator opt-in.
- Exact matrix validation and required negotiation apply automatically to newly
  compiled runs carrying `codex_exact_pair_v1`.
- Existing frozen runs remain on `legacy_best_effort_v0`; their bytes and
  behavior are not rewritten.
- The updated app requires provider-execution-truth schema v1. It performs the
  bounded bundled-daemon replacement and one readiness/probe retry before
  showing run detail; persistent mismatch fails visibly instead of issuing an
  incompatible GraphQL document.
- The current pre-change run displays generic model and effort only as
  planned/unverified after the updated app is installed.
- A normal later run may provide operational observation, but it is not a
  release prerequisite for this provider-free bounded change.

## Acceptance Checklist

- [ ] Fresh compilation freezes the approved exact matrix and the compiler-owned
      `codex_exact_pair_v1` marker.
- [ ] Frozen pre-change snapshots retain the old adapter path.
- [ ] Codex exact negotiation is response-closed and prompt-gated.
- [ ] Requested and accepted truth is durable, nullable, versioned, and tied to
      one stable task occurrence.
- [ ] Matching live-session generation evidence is projected before reuse;
      missing or mismatched evidence closes the old session with zero prompts
      on that handle and permits only one fully negotiated fresh fallback.
- [ ] Every original and repair prompt has an independent durable turn in the
      existing runtime-receipt table; P079 lease state changes atomically with
      that turn and never claims sent before transport flush.
- [ ] Dispatch and cancellation share the invalidation coordinator and one
      generation gate; run-wide invalidation additionally uses the durable
      epoch. A cancelled, terminal, stale-owner, or inactive-generation prompt
      writes zero bytes when invalidation wins.
- [ ] Unknown delivery marks the owning work item `Failed`, blocks its
      stage/run, and is excluded by all automatic retry, continuation, fallback,
      and startup-requeue selectors.
- [ ] Every one of the nine production `InvokeAgent` source classes delegates
      to typed enqueue/claim validation; `legacy_flat` is explicit, same-owner
      retry/fallback preserves identity, and a new stage execution recomputes it.
- [ ] Dynamic materialization persists compiled-task, occurrence, and work-item
      identity atomically and topology reads those fields instead of guesses.
- [ ] Acceptance and execution receipts use frozen exact JSON schemas,
      canonical values plus provider wire values, RFC 8785 hashing, verified
      digests, and the required v1/v2 decoder matrix.
- [ ] The app proves schema v1 before the new GraphQL document, performs at most
      one bundled-daemon replacement/retry, and fails visibly on persistent
      mismatch.
- [ ] GraphQL and Swift distinguish planned, configuring, configured but not
      started, prompt sent, delivery unknown, failure, cancellation, and legacy
      states without treating non-Codex null configuration as an error.
- [ ] GraphQL, mediation attempts, MCP execution truth, and run reports expose
      equivalent requested/accepted/configuration/prompt-turn truth; Swift
      treats an omitted selected key as schema mismatch and explicit null as
      state-dependent legacy truth.
- [ ] Codex/non-Codex fallback in either direction never inherits incompatible
      accepted truth and remains tied to the original occurrence.
- [ ] Full model/effort identity is identical across visual, Help, popover,
      copy, and accessibility output; bounded compact text never abbreviates a
      known Sol/Terra/Luna pair.
- [ ] Hosted-view tests prove immutable row identity and global-track geometry
      on the real branched topology with mixed 1/2/5-occurrence cards, long
      unknown values, preserved focus/popover state, no clipping, and connector
      centers derived from actual frames.
- [ ] `./scripts/test-gate.sh codex-model-truth` passes with nonzero Rust and
      Swift test execution.
