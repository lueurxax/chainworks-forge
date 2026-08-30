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
`WorkItemKind::InvokeAgent` producer. Producers may not hand-build or clone an
occurrence ID.

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
| Replacement or loop re-entry | New `stage_execution_id` | Reuse/rederive the canonical compiled-task ID, then recompute occurrence from the new owner; copied occurrence IDs are discarded |

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
site and proves that each delegates to `InvocationOccurrenceFactory`. It
covers static, owner, dynamic, mediation, retry, fallback, replacement, and
loop paths.

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
| `task_occurrence_id` | Stable occurrence shared by retries/fallbacks |
| `requested_model` | Exact model requested for this execution |
| `requested_effort` | Exact effort requested for this execution |
| `accepted_model` | Response-verified model; otherwise `null` |
| `accepted_effort` | Response-verified effort; otherwise `null` |
| `provider_configuration_state` | `configuring`, `configured`, `failed_before_prompt`, or `legacy_unverified`; `null` for non-applicable providers |
| `provider_configuration_verified_at` | Timestamp of complete pair verification; otherwise `null` |
| `provider_configuration_receipt_json` | Versioned bounded receipt; absent before configuration and on historical rows |
| `acceptance_source` | `fresh_negotiation` or `reused_session_generation`; otherwise `null` |
| `prompt_dispatch_state` | `not_started`, `dispatch_pending`, `prompt_sent`, or `dispatch_unknown` |
| `prompt_dispatch_started_at` | Durable boundary written before transport send |
| `prompt_sent_at` | Durable acknowledgement after successful transport send |

The existing `model` column remains a compatibility projection of
`requested_model`; it is never redefined as accepted truth. Migration backfills
`requested_model = model` for historical rows, leaves accepted fields and
requested effort `null`, and marks historical Codex rows
`legacy_unverified`. New writes keep `model` and `requested_model` byte-equal.

`prompt_dispatch_state` is provider-neutral and mandatory for every new
provider execution. Codex exact-pair negotiation adds configuration truth on
top of that shared dispatch ledger; Claude, Gemini, Auggie, and Junie keep
`provider_configuration_state = null`. Historical rows backfill
`prompt_dispatch_state = null` and remain explicitly delivery-unverified.

The same migration extends `session_generations` with nullable contract
version, accepted model/effort, provider-configuration acceptance/digest,
provider-session binding fingerprint, and verified-at fields. Pre-change
generations remain legacy-unverified and cannot authorize a v1 reused prompt.

`ProviderConfigurationAcceptanceV1` is generation-scoped. It contains the
contract version, generation ID, provider-session ID, provider/binding
fingerprint, accepted pair, and verification timestamp, but no agent execution
or task-occurrence ID. Fresh negotiation persists this acceptance and the first
execution receipt atomically.

`ProviderConfigurationReceiptV1` contains:

- `schema_version = provider_configuration_receipt_v1`;
- frozen provider-configuration contract version;
- agent execution and task-occurrence IDs;
- requested model and effort;
- accepted model and effort, present only when the complete pair is verified;
- state, acceptance source, verification timestamp, and configuration failure
  code;
- provider-session and session-generation binding;
- source generation-acceptance digest, present for successful fresh or reused
  acceptance and `null` for configuration failure;
- `prompt_dispatch_count_at_receipt`, which must be `0`.

`provider_configuration_receipt_json` is capped at 8 KiB and never includes
the provider's complete option catalog or raw JSON-RPC payloads.

`AcpRuntimeReceipt` increments to schema version 2 and carries the same typed
provider-configuration receipt as an optional nested field. Schema v1 remains
decodable with the nested field absent. The immediate execution-row receipt and
the later terminal ACP receipt must agree byte-for-byte on requested/accepted
values and task occurrence.

### Configuration and prompt-dispatch lifecycle

The engine inserts a fresh exact Codex execution with requested fields and
`provider_configuration_state = configuring` before ACP startup. It installs a
provider-configuration observation sink on `AcpRuntimeManager`:

- after both option responses are verified, but before prompt dispatch, the
  sink transaction writes the generation acceptance and an execution-bound
  receipt, then sets the execution configuration to `configured` and dispatch
  state to `not_started`;
- on a negotiation failure, it writes `failed_before_prompt`, accepted fields
  remain `null`, and the receipt records the typed failure;
- if sink persistence fails, the transport returns
  `ACP_PROVIDER_CONFIGURATION_PERSISTENCE_FAILED` and does not send a prompt;
- terminal settlement retains the same receipt in `AcpRuntimeReceipt`.

Every pre-prompt negotiation error, including `session/new` failure, is
reported through the sink when possible. The engine error-settlement path is
the fallback writer when transport failure prevents the callback. Both paths
use a compare-and-set from `configuring`; a late failure cannot overwrite a
`configured` receipt.

An authoritative prompt-dispatch sink in the shared ACP transport, separate
from best-effort timeline progress, owns this state machine for every provider:

```text
not_started -> dispatch_pending
dispatch_pending -> prompt_sent
dispatch_pending -> dispatch_unknown
```

For exact Codex, `provider_configuration_state = configured` is a precondition
for `not_started -> dispatch_pending`. For non-Codex and legacy execution,
configuration state remains non-applicable or unverified while the same prompt
state machine advances independently.

Before writing `session/prompt` to the transport, the engine persists
`dispatch_pending`. Only a newly `Applied` transition authorizes the write.
`AlreadyMatching` at this boundary means an earlier send may have started; it
is converted to `dispatch_unknown` and is never replayed automatically.

After the transport write and flush succeed, the strict sink persists
`prompt_sent` before best-effort timeline notification. A crash, send error,
sink error, `Conflict`, or `Missing` after `dispatch_pending` settles
`dispatch_unknown`, closes the live session, and forbids automatic replay.
Startup repair also converts stale `dispatch_pending` rows to
`dispatch_unknown`.

A stale `not_started` row with no matching live generation is provably
unprompted. Startup repair preserves its provider-configuration truth and
settles that execution with
`failure_phase = prompt_dispatch_preparation` and `Prompt not started`; a
later authorized retry must create or validate a session before dispatch.

Every lifecycle CAS returns exactly one typed result:
`Applied`, `AlreadyMatching`, `Conflict`, or `Missing`. Its predicate
binds agent execution, task occurrence, session generation, provider-session
ID, frozen contract version, requested model, and requested effort. Timeline
progress remains advisory and cannot advance durable dispatch truth.

`No prompt sent` is allowed only for `failed_before_prompt/not_started`.
`Using` and `Used` are allowed only after `prompt_sent`.
`dispatch_unknown` always renders ambiguous-delivery guidance and is not
eligible for blind retry.

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
- `promptDispatchState`, `promptDispatchStartedAt`, and `promptSentAt`.

`RunStageTopologyOccurrence` adds:

- `compiledTaskId`, nullable `taskOccurrenceId`, and `activeExecutionId`;
- `executionProvider`;
- requested and accepted model/effort;
- provider-configuration and prompt-dispatch states.

Its existing `provider`, `model`, and `effort` fields continue to mean frozen
planned identity for compatibility. The new fields come only from the latest
execution matched by occurrence ID. Retry/fallback cannot overwrite another
same-agent task.

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
provider-configuration state, and prompt-dispatch state. It never promotes a
planned/requested value to accepted truth.

### Codex state matrix

| State | Runtime truth | Operator copy |
|---|---|---|
| Pending | Frozen planned pair; no execution | `Planned: Codex - GPT-5.6 Terra - High` |
| Configuring | Requested pair present; accepted pair absent | `Configuring: Codex - GPT-5.6 Terra - High` |
| Configured / not started | Response-verified pair; prompt not attempted | `Configured: Codex - GPT-5.6 Terra - High - Prompt not started` |
| Cancelled before prompt | Response-verified pair; dispatch remains `not_started` | `Cancelled before prompt: Codex - GPT-5.6 Terra - High - No prompt sent` |
| Dispatch pending | Response-verified pair; delivery not yet known | `Starting: Codex - GPT-5.6 Terra - High` |
| Prompt sent / running | Response-verified pair and durable prompt sent | `Using: Codex - GPT-5.6 Terra - High` |
| Prompt sent / completed | Response-verified pair and durable prompt sent | `Used: Codex - GPT-5.6 Terra - High` |
| Prompt sent / failed | Response-verified pair; execution failed later | `Used: Codex - GPT-5.6 Terra - High` plus failure status |
| Prompt sent / cancelled | Response-verified pair; execution cancelled later | `Cancelled: Codex - GPT-5.6 Terra - High` |
| Dispatch unknown | Response-verified pair; delivery ambiguous | `Prompt delivery unknown: Codex - GPT-5.6 Terra - High - Do not retry automatically` |
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

Topology no longer assumes one fixed card height. Before column layout, the
presentation model calculates the required card height from header chrome,
metadata rows, transition rows, occurrence count, and the bounded two-line
identity row height. It converts that height into the existing card-height
units with ceiling division; connector columns consume the same computed slot
height. This keeps 1, 2, and 5 occurrence cards aligned without clipping.

Each occurrence row owns its accessibility label. Stage cards contain child
accessibility elements rather than combining and swallowing occurrence labels.
Swift row identity is `taskOccurrenceId ?? compiledTaskId`; it never falls
back to a composite agent/task guess. Visual, Help, popover, copy, and
accessibility strings are generated from the same formatter result.

## Failure Behavior

| Failure | Typed result | Prompt dispatch |
|---|---|---:|
| Fresh generic or unapproved catalog pair | compile failure | 0 |
| Model option/value unavailable | `ACP_CODEX_MODEL_UNAVAILABLE` | 0 |
| Model response lacks matching current value | `ACP_CODEX_MODEL_NOT_ACCEPTED` | 0 |
| Updated effort option/value unavailable | `ACP_CODEX_EFFORT_UNAVAILABLE` | 0 |
| Final response does not verify both values | `ACP_CODEX_EFFORT_NOT_ACCEPTED` | 0 |
| Accepted-truth persistence fails | `ACP_PROVIDER_CONFIGURATION_PERSISTENCE_FAILED` | 0 |
| Reused generation evidence mismatch | close generation and negotiate fresh once | 0 on old session |
| Dispatch-pending CAS fails before send | `ACP_PROMPT_DISPATCH_PREPARE_FAILED` | 0 |
| Transport send/flush fails after dispatch pending | `ACP_PROMPT_DISPATCH_UNKNOWN` | unknown |
| Prompt-sent persistence fails after transport success | `ACP_PROMPT_DISPATCH_UNKNOWN` | sent or unknown |
| Startup finds stale dispatch pending | `ACP_PROMPT_DISPATCH_UNKNOWN` | unknown |
| Legacy generic frozen run | allowed as planned/unverified | shared ledger for each new attempt |

Configuration failures use `failure_phase = provider_configuration`, leave
accepted fields `null`, and may render the requested pair plus
`No prompt sent`. Dispatch failures use `failure_phase = prompt_dispatch`,
preserve the configured accepted pair, and never claim that no prompt was sent
after `dispatch_pending`. Missing accepted readback is never inferred from the
host configuration or planned catalog value.

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
| `acp` fake provider | Response-closed model/effort negotiation; generation-bound live reuse; mismatch closes old session and negotiates fresh once; strict prompt lifecycle; no fuzzy/raw fallback; Claude aliases unchanged |
| `db` + `engine` | Migration/backfill; generation-acceptance to execution-receipt derivation; provider-neutral prompt ledger; four-result CAS semantics; crash repair; all-producer occurrence inventory; static/owner/dynamic/mediation identity; retry/fallback preservation; replacement/loop regeneration |
| `graphql-server` | Schema version probe; additive active/topology fields; durable dynamic topology; two same-agent tasks stay isolated; planned values never populate accepted fields |
| Swift focused and hosted-view tests | Lockstep restart/failure behavior; Codex and provider-neutral matrices; formatter parity; adaptive 1/2/5 occurrence geometry; long unknown values; popover/Help/copy; separate accessibility children |

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
11. crash before dispatch, crash after `dispatch_pending`, send failure, and
    prompt-sent persistence failure settle to the specified states without
    blind replay;
12. `Applied`, `AlreadyMatching`, `Conflict`, and `Missing` CAS outcomes
    enforce the execution/occurrence/generation/request binding;
13. Claude, Gemini, Auggie, and Junie advance the shared prompt ledger while
    keeping provider-configuration truth non-applicable;
14. cancellation and both directions of cross-provider fallback preserve the
    state-matrix and occurrence-association rules.

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
- [ ] Prompt dispatch is durably `not_started`, `dispatch_pending`,
      `prompt_sent`, or `dispatch_unknown`; ambiguous delivery is never replayed
      automatically, including after a crash.
- [ ] Every production `InvokeAgent` producer delegates to the occurrence
      factory; two same-agent tasks cannot cross-associate, same-owner retries
      and fallbacks preserve identity, and replacement/loop executions do not.
- [ ] Dynamic assignment topology is reconstructed from durable materialization
      identity rather than selection order or agent name.
- [ ] The app proves schema v1 before the new GraphQL document, performs at most
      one bundled-daemon replacement/retry, and fails visibly on persistent
      mismatch.
- [ ] GraphQL and Swift distinguish planned, configuring, configured but not
      started, prompt sent, delivery unknown, failure, cancellation, and legacy
      states without treating non-Codex null configuration as an error.
- [ ] Codex/non-Codex fallback in either direction never inherits incompatible
      accepted truth and remains tied to the original occurrence.
- [ ] Full model/effort identity is identical across visual, Help, popover,
      copy, and accessibility output; bounded compact text never abbreviates a
      known Sol/Terra/Luna pair.
- [ ] Hosted-view tests prove adaptive topology geometry for 1, 2, and 5
      occurrences plus long unknown values, without clipping or connector
      drift.
- [ ] `./scripts/test-gate.sh codex-model-truth` passes with nonzero Rust and
      Swift test execution.
