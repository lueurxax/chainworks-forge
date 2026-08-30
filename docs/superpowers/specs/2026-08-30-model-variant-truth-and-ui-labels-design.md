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
accepted, failed-before-prompt, and legacy-unverified truth.

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

Every compiled task gets a stable `compiled_task_id`. The value is an opaque
`compiled_task_v1:<sha256>` identifier derived from frozen inputs:

- workflow snapshot hash;
- state ID;
- run block (`run` or `run_after_approval`);
- lane (`sequence`, `parallel`, `then`, or dynamic assignment);
- lane ordinal or durable dynamic-assignment ID.

When a stage execution schedules that compiled task, the engine creates
`task_occurrence_id = task_occurrence_v1:<sha256>` from
`stage_execution_id + compiled_task_id`. The occurrence ID is copied into the
initial work-queue payload, `ExecutionRequest`, and `AgentExecution`, and is
preserved unchanged by retries and backend-profile/provider fallbacks within
that stage execution.

Two tasks in one stage that use the same agent receive different occurrence
IDs. A later loop iteration or replacement stage execution receives a new
occurrence ID, so attempts from different stage executions cannot be merged.
Pending topology can expose `compiled_task_id` while
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
8. Emit and durably persist `ProviderConfigurationReceiptV1`.
9. Permit `session/prompt` only after receipt persistence succeeds.

An empty/malformed option response, missing option, unknown value, incompatible
effort after model selection, send failure, provider rejection, current-value
mismatch, or persistence failure is a typed startup failure with zero prompt
dispatch. A successfully returned JSON-RPC response without matching
`currentValue` is not acceptance.

## Durable Runtime Truth

The next SQLite migration adds nullable columns to `agent_executions`:

| Column | Meaning |
|---|---|
| `task_occurrence_id` | Stable occurrence shared by retries/fallbacks |
| `requested_model` | Exact model requested for this execution |
| `requested_effort` | Exact effort requested for this execution |
| `accepted_model` | Response-verified model; otherwise `null` |
| `accepted_effort` | Response-verified effort; otherwise `null` |
| `provider_configuration_state` | `configuring`, `accepted`, `failed_before_prompt`, or `legacy_unverified`; `null` for non-applicable providers |
| `provider_configuration_verified_at` | Timestamp of complete pair verification; otherwise `null` |
| `provider_configuration_receipt_json` | Versioned bounded receipt; absent before configuration and on historical rows |

The existing `model` column remains a compatibility projection of
`requested_model`; it is never redefined as accepted truth. Migration backfills
`requested_model = model` for historical rows, leaves accepted fields and
requested effort `null`, and marks historical Codex rows
`legacy_unverified`. New writes keep `model` and `requested_model` byte-equal.

`ProviderConfigurationReceiptV1` contains:

- `schema_version = provider_configuration_receipt_v1`;
- frozen provider-configuration contract version;
- agent execution and task-occurrence IDs;
- requested model and effort;
- accepted model and effort, present only when the complete pair is verified;
- state, verification timestamp, and configuration failure code;
- `prompt_dispatch_count_at_receipt`, which must be `0`.

`provider_configuration_receipt_json` is capped at 8 KiB and never includes
the provider's complete option catalog or raw JSON-RPC payloads.

`AcpRuntimeReceipt` increments to schema version 2 and carries the same typed
provider-configuration receipt as an optional nested field. Schema v1 remains
decodable with the nested field absent. The immediate execution-row receipt and
the later terminal ACP receipt must agree byte-for-byte on requested/accepted
values and task occurrence.

### Write timing and failure atomicity

The engine inserts a fresh exact Codex execution with requested fields and
`provider_configuration_state = configuring` before ACP startup. It installs a
provider-configuration observation sink on `AcpRuntimeManager`:

- after both option responses are verified, but before prompt dispatch, the
  sink transaction writes accepted fields, `accepted`, timestamp, and receipt;
- on a negotiation failure, it writes `failed_before_prompt`, accepted fields
  remain `null`, and the receipt records the typed failure;
- if sink persistence fails, the transport returns
  `ACP_PROVIDER_CONFIGURATION_PERSISTENCE_FAILED` and does not send a prompt;
- terminal settlement retains the same receipt in `AcpRuntimeReceipt`.

Every pre-prompt negotiation error, including `session/new` failure, is
reported through the sink when possible. The engine error-settlement path is
the fallback writer when transport failure prevents the callback. Both paths
use a compare-and-set from `configuring`; a late failure cannot overwrite an
`accepted` receipt. Prompt dispatch requires exactly one successful
`configuring -> accepted` update.

For `legacy_best_effort_v0`, new resumed attempts are
`legacy_unverified`: requested/planned values may be retained, accepted fields
remain `null`, and prompt dispatch follows the old adapter path. Existing
historical rows remain readable without a receipt.

## GraphQL and Swift Readback

Readback changes are additive. Existing `model` fields remain compatibility
aliases for requested/planned model and must not be used as actual truth by the
updated UI.

`AgentExecution` GraphQL adds:

- `taskOccurrenceId`;
- `requestedModel`, `requestedEffort`;
- `acceptedModel`, `acceptedEffort`;
- `providerConfigurationState`;
- `providerConfigurationVerifiedAt`.

`RunStageTopologyOccurrence` adds:

- `compiledTaskId`, nullable `taskOccurrenceId`, and `activeExecutionId`;
- `executionProvider`;
- requested and accepted model/effort;
- `providerConfigurationState`.

Its existing `provider`, `model`, and `effort` fields continue to mean frozen
planned identity for compatibility. The new fields come only from the latest
execution matched by occurrence ID. Retry/fallback cannot overwrite another
same-agent task.

The Swift DTOs query and decode every additive field. Missing fields from a
legacy daemon decode as `nil`; the presenter then uses planned/unverified copy,
never accepted/running copy.

## UI Truth and Formatting Contract

One `ProviderExecutionIdentityFormatter` owns visual text, Help text, and
accessibility text. It accepts planned, requested, accepted, execution status,
and provider-configuration state. It never promotes a planned/requested value
to accepted truth.

### Required state matrix

| State | Runtime truth | Operator copy |
|---|---|---|
| Pending | Frozen planned pair; no execution | `Planned: Codex - GPT-5.6 Terra - High` |
| Configuring | Requested pair present; accepted pair absent | `Configuring: Codex - GPT-5.6 Terra - High` |
| Running | Complete response-verified accepted pair | `Using: Codex - GPT-5.6 Terra - High` |
| Completed | Complete response-verified accepted pair | `Used: Codex - GPT-5.6 Terra - High` |
| Failed after prompt | Complete response-verified accepted pair; execution failure is separate | `Used: Codex - GPT-5.6 Terra - High` plus failure status |
| Configuration failure | Requested pair present; accepted pair absent | `Configuration failed: GPT-5.6 Terra - High - No prompt sent` |
| Legacy generic | Frozen planned identity; acceptance unavailable | `Planned (unverified): Codex - GPT-5.6 (variant unspecified) - High` |
| Retry/fallback | Latest accepted pair for the same task occurrence | `Using` or `Used` with that execution's provider and accepted pair |

If an `accepted` state lacks either accepted field, the readback is internally
inconsistent. It renders `Runtime identity unavailable`, exposes diagnostic
Help text, and must be caught by the gate. It must not fall back to planned
truth.

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

Model and effort receive a dedicated identity line below the task/title line.
That line may wrap but must not use one-line truncation or middle truncation.
Status, attempts, stage, task, and session diagnostics move to a separate
secondary line so they cannot displace the exact model/effort identity.

Each occurrence row owns its accessibility label. Stage cards contain child
accessibility elements rather than combining and swallowing occurrence labels.
Visual, Help, and accessibility strings are generated from the same formatter
result and therefore carry the same truth prefix and exact pair.

## Failure Behavior

| Failure | Typed result | Prompt dispatch |
|---|---|---:|
| Fresh generic or unapproved catalog pair | compile failure | 0 |
| Model option/value unavailable | `ACP_CODEX_MODEL_UNAVAILABLE` | 0 |
| Model response lacks matching current value | `ACP_CODEX_MODEL_NOT_ACCEPTED` | 0 |
| Updated effort option/value unavailable | `ACP_CODEX_EFFORT_UNAVAILABLE` | 0 |
| Final response does not verify both values | `ACP_CODEX_EFFORT_NOT_ACCEPTED` | 0 |
| Accepted-truth persistence fails | `ACP_PROVIDER_CONFIGURATION_PERSISTENCE_FAILED` | 0 |
| Legacy generic frozen run | allowed as planned/unverified | prior behavior |

Every runtime failure uses `failure_phase = provider_configuration`, leaves
accepted fields `null`, persists or carries a bounded failure receipt, and
renders the requested pair plus `No prompt sent`. Missing accepted readback is
never inferred from the host configuration or planned catalog value.

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
| `acp` fake provider | Model-first order; first response verification; effort resolution from updated options; final pair verification; no fuzzy/raw fallback; all typed zero-prompt failures; generic Claude alias resolver unchanged |
| `db` + `engine` | Migration/backfill; stage-execution-scoped occurrence creation; configuring/accepted/failure CAS writes; sink failure fail-closed; receipt v1/runtime receipt v2 parity; retry/fallback occurrence preservation; new-loop isolation; legacy null semantics |
| `graphql-server` | Additive active/topology fields; two same-agent tasks stay isolated; retries/fallbacks select only the same occurrence; planned values never populate accepted fields |
| Swift focused tests | DTO decoding; required state matrix; one formatter for Overview/Stages/active rows/Help/accessibility; nontruncated wrapping identity; failed-before-prompt copy |

The fake ACP test matrix must include:

1. model success changes the effort option set and the adapter uses the updated
   set;
2. model JSON-RPC success with a mismatched `currentValue`;
3. effort JSON-RPC success with a mismatched final model or effort;
4. missing/malformed `configOptions` after either operation;
5. persistence-sink failure after provider acceptance;
6. a prompt counter proving every failure dispatched zero prompts;
7. a legacy generic request proving the old best-effort path remains reachable;
8. a Claude alias request proving generic alias matching did not change.

The gate must fail when its focused Swift result bundle reports zero tests. No
network, daemon, live provider, or remote UI host is required.

## Rollout

- There is no feature flag, disable path, or operator opt-in.
- Exact matrix validation and required negotiation apply automatically to newly
  compiled runs carrying `codex_exact_pair_v1`.
- Existing frozen runs remain on `legacy_best_effort_v0`; their bytes and
  behavior are not rewritten.
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
- [ ] Two same-agent tasks, retries, and fallbacks cannot cross-associate.
- [ ] GraphQL and Swift distinguish planned, configuring, accepted, failure,
      and legacy states.
- [ ] Model/effort identity is not truncated and is identical across visual,
      Help, and accessibility output.
- [ ] `./scripts/test-gate.sh codex-model-truth` passes with nonzero Rust and
      Swift test execution.
